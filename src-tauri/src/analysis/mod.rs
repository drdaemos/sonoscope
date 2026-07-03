use crate::error::{AnalysisStatus, CommandError, TagSource};
use crate::models;
use crate::tags::{self, TagLookup};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const DEFAULT_ANALYSIS_BATCH_SIZE: usize = 16;
/// Generous ceiling per response line: the first batch also pays model load,
/// which can take minutes on CPU-only machines.
const DEFAULT_RESPONSE_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Serialize)]
struct AnalyzeRequest {
    id: String,
    path: String,
    relative_path: String,
}

#[derive(Debug, Serialize)]
struct AnalyzeBatchRequest<'a> {
    requests: &'a [AnalyzeRequest],
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
    async fn spawn(
        clap_model_dir: PathBuf,
        essentia_model_dir: PathBuf,
    ) -> Result<Self, CommandError> {
        let analyzer_dir = analyzer_dir();
        let mut child = Command::new("uv")
            .args(["run", "python", "-m", "sonoscope_analyzer.main"])
            .current_dir(&analyzer_dir)
            .env("SONOSCOPE_CLAP_MODEL_PATH", clap_model_dir)
            .env("SONOSCOPE_CLAP_LOCAL_ONLY", "1")
            .env("SONOSCOPE_ESSENTIA_MODEL_DIR", essentia_model_dir)
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

    async fn analyze_batch(
        &mut self,
        requests: &[AnalyzeRequest],
    ) -> Result<Vec<AnalyzeResponse>, CommandError> {
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let line = serde_json::to_string(&AnalyzeBatchRequest { requests })
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

        let timeout = response_timeout();
        let mut responses = Vec::with_capacity(requests.len());
        for _request in requests {
            let response_line = tokio::time::timeout(timeout, self.stdout.next_line())
                .await
                .map_err(|_| {
                    CommandError::Analysis(format!(
                        "analyzer response timed out after {}s",
                        timeout.as_secs()
                    ))
                })?
                .map_err(|e| {
                    CommandError::Analysis(format!("failed reading analyzer response: {e}"))
                })?
                .ok_or_else(|| {
                    CommandError::Analysis("analyzer exited before response".to_string())
                })?;

            responses.push(
                serde_json::from_str(&response_line).map_err(|e| {
                    CommandError::Analysis(format!("invalid analyzer response: {e}"))
                })?,
            );
        }

        Ok(responses)
    }

    async fn shutdown(&mut self) {
        let _ = self.stdin.write_all(b"{\"shutdown\": true}\n").await;
        let _ = self.stdin.flush().await;
        let _ = self.child.wait().await;
    }

    /// Force-stop a sidecar whose stdout stream can no longer be trusted
    /// (e.g. after a timeout the pipe may be mid-line).
    async fn kill(&mut self) {
        let _ = self.child.kill().await;
    }
}

pub async fn run_pending_analysis(
    pool: &SqlitePool,
    app: AppHandle,
    cancellation: Arc<AtomicBool>,
) -> Result<u32, CommandError> {
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

    let lookup = tags::load_tag_lookup(pool).await?;
    let clap_model_dir = models::clap_model_dir()?;
    let essentia_model_dir = models::essentia_model_dir()?;
    let mut sidecar = AnalyzerSidecar::spawn(clap_model_dir, essentia_model_dir).await?;

    let batch_size = analysis_batch_size();
    for batch in samples.chunks(batch_size) {
        if cancellation.load(Ordering::Relaxed) {
            sidecar.shutdown().await;
            app.emit(
                "analysis-cancelled",
                serde_json::json!({ "processed": analyzed, "total": total }),
            )
            .ok();
            return Ok(analyzed);
        }

        for sample in batch {
            mark_sample_status(pool, sample.id, AnalysisStatus::Analysing).await?;
        }

        let requests = batch
            .iter()
            .map(|sample| AnalyzeRequest {
                id: sample.id.to_string(),
                path: sample.path.clone(),
                relative_path: sample.relative_path.clone(),
            })
            .collect::<Vec<_>>();
        let response = sidecar.analyze_batch(&requests).await;

        match response {
            Ok(responses) => {
                let mut responses_by_id = responses
                    .into_iter()
                    .map(|response| (response.id.clone(), response))
                    .collect::<HashMap<_, _>>();
                for sample in batch {
                    let response = responses_by_id.remove(&sample.id.to_string());
                    process_sample_response(pool, &lookup, sample, response).await?;
                    analyzed += 1;
                    emit_analysis_progress(&app, analyzed, total);
                }
            }
            Err(e) => {
                for sample in batch {
                    mark_sample_status(pool, sample.id, AnalysisStatus::Failed).await?;
                    eprintln!("Analyzer batch error for sample {}: {e}", sample.id);
                    analyzed += 1;
                    emit_analysis_progress(&app, analyzed, total);
                }
                // The stdout stream may be desynchronized after a failed batch;
                // restart the sidecar before processing the next one.
                sidecar.kill().await;
                sidecar = AnalyzerSidecar::spawn(
                    models::clap_model_dir()?,
                    models::essentia_model_dir()?,
                )
                .await?;
            }
        }
    }

    sidecar.shutdown().await;
    app.emit(
        "analysis-complete",
        serde_json::json!({ "processed": analyzed, "total": total }),
    )
    .ok();

    Ok(analyzed)
}

async fn process_sample_response(
    pool: &SqlitePool,
    lookup: &TagLookup,
    sample: &PendingSample,
    response: Option<AnalyzeResponse>,
) -> Result<(), CommandError> {
    match response {
        Some(response) if response.status == AnalyzeResponseStatus::Ok => {
            persist_analysis_response(pool, lookup, sample.id, &response).await?;
            mark_sample_status(pool, sample.id, AnalysisStatus::Done).await?;
        }
        Some(response) => {
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
        None => {
            mark_sample_status(pool, sample.id, AnalysisStatus::Failed).await?;
            eprintln!("Analyzer returned no response for sample {}", sample.id);
        }
    }
    Ok(())
}

fn emit_analysis_progress(app: &AppHandle, processed: u32, total: u32) {
    app.emit(
        "analysis-progress",
        serde_json::json!({ "processed": processed, "total": total }),
    )
    .ok();
}

fn response_timeout() -> std::time::Duration {
    let secs = std::env::var("SONOSCOPE_ANALYZER_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_RESPONSE_TIMEOUT_SECS);
    std::time::Duration::from_secs(secs)
}

fn analysis_batch_size() -> usize {
    std::env::var("SONOSCOPE_ANALYSIS_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_ANALYSIS_BATCH_SIZE)
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
    lookup: &TagLookup,
    sample_id: i64,
    response: &AnalyzeResponse,
) -> Result<(), CommandError> {
    let mut tx = pool.begin().await?;

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
        .execute(&mut *tx)
        .await?;
    }
    if let Some(waveform_data) = &response.waveform_data {
        sqlx::query!(
            "UPDATE samples SET waveform_data = ? WHERE id = ?",
            waveform_data,
            sample_id,
        )
        .execute(&mut *tx)
        .await?;
    }

    let user_source = TagSource::User;
    sqlx::query!(
        "DELETE FROM tags WHERE sample_id = ? AND source != ?",
        sample_id,
        user_source,
    )
    .execute(&mut *tx)
    .await?;

    for tag in &response.tags {
        tags::insert_auto_tag(
            &mut *tx,
            lookup,
            sample_id,
            &tag.dimension,
            &tag.value,
            &tag.source,
            tag.confidence,
        )
        .await?;
    }

    let rows = sqlx::query!(
        "SELECT DISTINCT dimension_id as \"dimension_id!: i64\"
         FROM tags
         WHERE sample_id = ?
           AND source != ?",
        sample_id,
        user_source,
    )
    .fetch_all(&mut *tx)
    .await?;
    for row in rows {
        tags::mark_auto_primary_for_dimension(&mut *tx, sample_id, row.dimension_id).await?;
    }

    tx.commit().await?;
    Ok(())
}

fn analyzer_dir() -> PathBuf {
    std::env::var("SONOSCOPE_ANALYZER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| Path::new(env!("CARGO_MANIFEST_DIR")).join("../analyzer"))
}

#[cfg(test)]
mod tests {
    use super::{AnalyzeBatchRequest, AnalyzeRequest, AnalyzeResponse, AnalyzerSidecar};
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

    #[test]
    fn serialize_batch_request_wraps_requests() {
        let requests = vec![
            AnalyzeRequest {
                id: "1".to_string(),
                path: "/audio/kick.wav".to_string(),
                relative_path: "kick.wav".to_string(),
            },
            AnalyzeRequest {
                id: "2".to_string(),
                path: "/audio/snare.wav".to_string(),
                relative_path: "snare.wav".to_string(),
            },
        ];

        let value: serde_json::Value = serde_json::from_str(
            &serde_json::to_string(&AnalyzeBatchRequest {
                requests: &requests,
            })
            .unwrap(),
        )
        .unwrap();

        assert_eq!(value["requests"][0]["id"], "1");
        assert_eq!(value["requests"][1]["relative_path"], "snare.wav");
    }

    #[tokio::test]
    #[ignore = "requires spawning the uv-managed analyzer process"]
    async fn sidecar_returns_heuristic_tags() {
        let dir = TempDir::new().unwrap();
        let sample_path = dir.path().join("kick_loop_120bpm.wav");
        fs::write(&sample_path, b"not real audio").unwrap();

        let model_dir = dir
            .path()
            .join("models")
            .join("laion")
            .join("larger_clap_music");
        let essentia_model_dir = dir
            .path()
            .join("models")
            .join("essentia")
            .join("fs_loop_ds-msd-musicnn");
        let mut sidecar = AnalyzerSidecar::spawn(model_dir, essentia_model_dir)
            .await
            .expect("spawn analyzer");
        let responses = sidecar
            .analyze_batch(&[AnalyzeRequest {
                id: "sample-1".to_string(),
                path: sample_path.to_string_lossy().to_string(),
                relative_path: "Drums/Kicks/kick_loop_120bpm.wav".to_string(),
            }])
            .await
            .expect("analyze sample");
        let response = responses.first().expect("one response");
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
