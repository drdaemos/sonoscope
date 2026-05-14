use crate::error::{AnalysisStatus, CommandError, DimensionValueType, TagSource};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[derive(Debug, Serialize)]
struct AnalyzeRequest {
    id: String,
    path: String,
    relative_path: String,
}

#[derive(Debug, Deserialize)]
struct AnalyzeResponse {
    id: String,
    status: AnalyzeResponseStatus,
    tags: Vec<TagCandidate>,
    file_meta: Option<FileMeta>,
    waveform_data: Option<Vec<u8>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum AnalyzeResponseStatus {
    Ok,
    Error,
}

#[derive(Debug, Deserialize)]
struct TagCandidate {
    dimension: String,
    value: String,
    source: TagSource,
    confidence: f64,
}

#[derive(Debug, Deserialize)]
struct FileMeta {
    format: Option<String>,
    duration_ms: Option<i64>,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
    channels: Option<i64>,
}

#[derive(Debug)]
struct PendingSample {
    id: i64,
    path: String,
    relative_path: String,
}

struct AnalyzerSidecar {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl AnalyzerSidecar {
    async fn spawn() -> Result<Self, CommandError> {
        let analyzer_dir = analyzer_dir();
        let mut child = Command::new("uv")
            .args(["run", "python", "-m", "sonoscope_analyzer.main"])
            .current_dir(&analyzer_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                CommandError::Analysis(format!(
                    "failed to spawn analyzer in {}: {e}",
                    analyzer_dir.display()
                ))
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CommandError::Analysis("failed to open analyzer stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CommandError::Analysis("failed to open analyzer stdout".to_string()))?;
        let mut sidecar = AnalyzerSidecar {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
        };

        let ready_line = sidecar
            .stdout
            .next_line()
            .await
            .map_err(|e| {
                CommandError::Analysis(format!("failed reading analyzer ready line: {e}"))
            })?
            .ok_or_else(|| CommandError::Analysis("analyzer exited before ready".to_string()))?;
        let ready: serde_json::Value = serde_json::from_str(&ready_line)
            .map_err(|e| CommandError::Analysis(format!("invalid analyzer ready line: {e}")))?;
        if ready.get("ready").and_then(|value| value.as_bool()) != Some(true) {
            return Err(CommandError::Analysis(format!(
                "unexpected analyzer ready line: {ready_line}"
            )));
        }

        Ok(sidecar)
    }

    async fn analyze(&mut self, request: &AnalyzeRequest) -> Result<AnalyzeResponse, CommandError> {
        let line = serde_json::to_string(request)
            .map_err(|e| CommandError::Analysis(format!("failed to serialize request: {e}")))?;
        self.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| CommandError::Analysis(format!("failed writing analyzer request: {e}")))?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(|e| CommandError::Analysis(format!("failed writing analyzer newline: {e}")))?;
        self.stdin.flush().await.map_err(|e| {
            CommandError::Analysis(format!("failed flushing analyzer request: {e}"))
        })?;

        let response_line = self
            .stdout
            .next_line()
            .await
            .map_err(|e| CommandError::Analysis(format!("failed reading analyzer response: {e}")))?
            .ok_or_else(|| CommandError::Analysis("analyzer exited before response".to_string()))?;

        serde_json::from_str(&response_line)
            .map_err(|e| CommandError::Analysis(format!("invalid analyzer response: {e}")))
    }

    async fn shutdown(&mut self) {
        let _ = self.stdin.write_all(b"{\"shutdown\": true}\n").await;
        let _ = self.stdin.flush().await;
        let _ = self.child.wait().await;
    }
}

pub async fn run_pending_analysis(pool: &SqlitePool, app: AppHandle) -> Result<u32, CommandError> {
    let samples = pending_samples(pool).await?;
    let total = samples.len() as u32;
    let mut analyzed = 0_u32;
    if total == 0 {
        app.emit(
            "analysis-complete",
            serde_json::json!({ "processed": analyzed, "total": total }),
        )
        .ok();
        return Ok(analyzed);
    }

    let mut sidecar = AnalyzerSidecar::spawn().await?;

    for sample in samples {
        mark_sample_status(pool, sample.id, AnalysisStatus::Analysing).await?;
        let request = AnalyzeRequest {
            id: sample.id.to_string(),
            path: sample.path,
            relative_path: sample.relative_path,
        };
        let response = sidecar.analyze(&request).await;

        match response {
            Ok(response) if response.status == AnalyzeResponseStatus::Ok => {
                persist_analysis_response(pool, sample.id, &response).await?;
                mark_sample_status(pool, sample.id, AnalysisStatus::Done).await?;
            }
            Ok(response) => {
                mark_sample_status(pool, sample.id, AnalysisStatus::Failed).await?;
                eprintln!(
                    "Analyzer failed for sample {} (response {}): {}",
                    sample.id,
                    response.id,
                    response
                        .error
                        .unwrap_or_else(|| "unknown error".to_string())
                );
            }
            Err(e) => {
                mark_sample_status(pool, sample.id, AnalysisStatus::Failed).await?;
                eprintln!("Analyzer error for sample {}: {e}", sample.id);
            }
        }

        analyzed += 1;
        app.emit(
            "analysis-progress",
            serde_json::json!({ "processed": analyzed, "total": total }),
        )
        .ok();
    }

    sidecar.shutdown().await;
    app.emit(
        "analysis-complete",
        serde_json::json!({ "processed": analyzed, "total": total }),
    )
    .ok();

    Ok(analyzed)
}

pub async fn requeue_all_samples(pool: &SqlitePool) -> Result<(), CommandError> {
    let pending = AnalysisStatus::Pending;
    sqlx::query!("UPDATE samples SET analysis_status = ?", pending)
        .execute(pool)
        .await?;
    Ok(())
}

async fn pending_samples(pool: &SqlitePool) -> Result<Vec<PendingSample>, CommandError> {
    let pending = AnalysisStatus::Pending;
    let rows = sqlx::query!(
        "SELECT id as \"id!: i64\", path, relative_path FROM samples WHERE analysis_status = ? ORDER BY relative_path",
        pending,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| PendingSample {
            id: row.id,
            path: row.path,
            relative_path: row.relative_path,
        })
        .collect())
}

async fn mark_sample_status(
    pool: &SqlitePool,
    sample_id: i64,
    status: AnalysisStatus,
) -> Result<(), CommandError> {
    sqlx::query!(
        "UPDATE samples SET analysis_status = ? WHERE id = ?",
        status,
        sample_id,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn persist_analysis_response(
    pool: &SqlitePool,
    sample_id: i64,
    response: &AnalyzeResponse,
) -> Result<(), CommandError> {
    if let Some(file_meta) = &response.file_meta {
        sqlx::query!(
            "UPDATE samples
             SET format = COALESCE(?, format),
                 duration_ms = COALESCE(?, duration_ms),
                 sample_rate = COALESCE(?, sample_rate),
                 bit_depth = COALESCE(?, bit_depth),
                 channels = COALESCE(?, channels)
             WHERE id = ?",
            file_meta.format,
            file_meta.duration_ms,
            file_meta.sample_rate,
            file_meta.bit_depth,
            file_meta.channels,
            sample_id,
        )
        .execute(pool)
        .await?;
    }
    if let Some(waveform_data) = &response.waveform_data {
        sqlx::query!(
            "UPDATE samples SET waveform_data = ? WHERE id = ?",
            waveform_data,
            sample_id,
        )
        .execute(pool)
        .await?;
    }

    delete_auto_tags(pool, sample_id).await?;
    for tag in &response.tags {
        insert_tag_candidate(pool, sample_id, tag).await?;
    }
    mark_auto_primary_tags(pool, sample_id).await?;

    Ok(())
}

async fn delete_auto_tags(pool: &SqlitePool, sample_id: i64) -> Result<(), CommandError> {
    let user_source = TagSource::User;
    sqlx::query!(
        "DELETE FROM tags WHERE sample_id = ? AND source != ?",
        sample_id,
        user_source,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_tag_candidate(
    pool: &SqlitePool,
    sample_id: i64,
    tag: &TagCandidate,
) -> Result<(), CommandError> {
    let dimension = sqlx::query!(
        "SELECT id, value_type as \"value_type: DimensionValueType\" FROM dimensions WHERE name = ?",
        tag.dimension,
    )
    .fetch_optional(pool)
    .await?;

    let Some(dimension) = dimension else {
        return Ok(());
    };

    let now = now_unix();
    match dimension.value_type {
        DimensionValueType::Enum | DimensionValueType::MultiEnum => {
            let value = sqlx::query!(
                "SELECT id FROM dimension_values WHERE dimension_id = ? AND value = ?",
                dimension.id,
                tag.value,
            )
            .fetch_optional(pool)
            .await?;
            if let Some(value) = value {
                sqlx::query!(
                    "INSERT OR IGNORE INTO tags
                     (sample_id, dimension_id, value_id, source, confidence, created_at, is_primary)
                     VALUES (?, ?, ?, ?, ?, ?, 0)",
                    sample_id,
                    dimension.id,
                    value.id,
                    tag.source,
                    tag.confidence,
                    now,
                )
                .execute(pool)
                .await?;
            }
        }
        DimensionValueType::Numeric => {
            if let Ok(numeric_value) = tag.value.parse::<f64>() {
                sqlx::query!(
                    "INSERT INTO tags
                     (sample_id, dimension_id, numeric_value, source, confidence, created_at, is_primary)
                     VALUES (?, ?, ?, ?, ?, ?, 0)",
                    sample_id,
                    dimension.id,
                    numeric_value,
                    tag.source,
                    tag.confidence,
                    now,
                )
                .execute(pool)
                .await?;
            }
        }
        DimensionValueType::Text => {
            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, text_value, source, confidence, created_at, is_primary)
                 VALUES (?, ?, ?, ?, ?, ?, 0)",
                sample_id,
                dimension.id,
                tag.value,
                tag.source,
                tag.confidence,
                now,
            )
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

async fn mark_auto_primary_tags(pool: &SqlitePool, sample_id: i64) -> Result<(), CommandError> {
    let user_source = TagSource::User;
    let rows = sqlx::query!(
        "SELECT DISTINCT dimension_id as \"dimension_id!: i64\"
         FROM tags
         WHERE sample_id = ?
           AND source != ?",
        sample_id,
        user_source,
    )
    .fetch_all(pool)
    .await?;

    for row in rows {
        mark_auto_primary_for_dimension(pool, sample_id, row.dimension_id).await?;
    }

    Ok(())
}

async fn mark_auto_primary_for_dimension(
    pool: &SqlitePool,
    sample_id: i64,
    dimension_id: i64,
) -> Result<(), CommandError> {
    sqlx::query!(
        "UPDATE tags SET is_primary = 0 WHERE sample_id = ? AND dimension_id = ? AND source != 'user'",
        sample_id,
        dimension_id,
    )
    .execute(pool)
    .await?;

    let has_user_primary = sqlx::query!(
        "SELECT 1 as \"exists!: i64\"
         FROM tags
         WHERE sample_id = ?
           AND dimension_id = ?
           AND source = 'user'
           AND is_primary = 1
         LIMIT 1",
        sample_id,
        dimension_id,
    )
    .fetch_optional(pool)
    .await?;
    if has_user_primary.is_some() {
        return Ok(());
    }

    let candidate = sqlx::query!(
        "SELECT id as \"id!: i64\"
         FROM tags
         WHERE sample_id = ?
           AND dimension_id = ?
           AND source != 'user'
         ORDER BY confidence DESC, id ASC
         LIMIT 1",
        sample_id,
        dimension_id,
    )
    .fetch_optional(pool)
    .await?;
    if let Some(candidate) = candidate {
        sqlx::query!("UPDATE tags SET is_primary = 1 WHERE id = ?", candidate.id)
            .execute(pool)
            .await?;
    }
    Ok(())
}

fn analyzer_dir() -> PathBuf {
    std::env::var("SONOSCOPE_ANALYZER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../analyzer"))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::{AnalyzeRequest, AnalyzeResponse, AnalyzerSidecar};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn deserialize_response_with_waveform_data() {
        let response: AnalyzeResponse = serde_json::from_str(
            r#"{
                "id": "sample-1",
                "status": "ok",
                "tags": [],
                "file_meta": null,
                "waveform_data": [0, 128, 255],
                "error": null
            }"#,
        )
        .unwrap();

        assert_eq!(response.waveform_data, Some(vec![0, 128, 255]));
    }

    #[tokio::test]
    #[ignore = "requires spawning the uv-managed analyzer process"]
    async fn sidecar_returns_heuristic_tags() {
        let dir = TempDir::new().unwrap();
        let sample_path = dir.path().join("kick_loop_120bpm.wav");
        fs::write(&sample_path, b"not real audio").unwrap();

        let mut sidecar = AnalyzerSidecar::spawn().await.expect("spawn analyzer");
        let response = sidecar
            .analyze(&AnalyzeRequest {
                id: "sample-1".to_string(),
                path: sample_path.to_string_lossy().to_string(),
                relative_path: "Drums/Kicks/kick_loop_120bpm.wav".to_string(),
            })
            .await
            .expect("analyze sample");
        sidecar.shutdown().await;

        let tags = response
            .tags
            .iter()
            .map(|tag| (tag.dimension.as_str(), tag.value.as_str()))
            .collect::<Vec<_>>();
        assert!(tags.contains(&("Instrument", "kick")));
        assert!(tags.contains(&("Type", "loop")));
        assert!(tags.contains(&("Tempo", "120")));
    }
}
