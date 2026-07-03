use crate::analysis;
use crate::db;
use crate::error::{AnalysisStatus, CommandError, DimensionValueType, TagSource};
use crate::library::{discover, open};
use crate::models::{self, MlModelStatus};
use crate::state::AppState;
use crate::tags;
use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter, Manager, State};

pub use crate::tags::{clear_user_tag_for_dimension, write_user_tag};

/// Sample identifier as used in command signatures. Exposed to TypeScript as
/// a plain `number` while staying `i64` on the Rust side to match the schema.
#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[specta(transparent)]
pub struct SampleId(#[specta(type = Number<i64>)] pub i64);

#[derive(Debug, Serialize, Type)]
pub struct SampleRow {
    #[specta(type = Number<i64>)]
    pub id: i64,
    pub filename: String,
    pub relative_path: String,
    pub format: Option<String>,
    #[specta(type = Option<Number<i64>>)]
    pub size_bytes: Option<i64>,
    #[specta(type = Option<Number<i64>>)]
    pub duration_ms: Option<i64>,
    #[specta(type = Option<Number<i64>>)]
    pub sample_rate: Option<i64>,
    #[specta(type = Option<Number<i64>>)]
    pub bit_depth: Option<i64>,
    #[specta(type = Option<Number<i64>>)]
    pub channels: Option<i64>,
    pub analysis_status: AnalysisStatus,
    pub tags: Vec<SampleTag>,
    pub conflicts: Vec<TagConflict>,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct SampleTag {
    pub dimension: String,
    pub value: String,
    pub source: TagSource,
    pub confidence: Option<f64>,
    pub is_primary: bool,
}

#[derive(Debug, Serialize, Type)]
pub struct TagConflict {
    pub dimension: String,
    pub candidates: Vec<SampleTag>,
}

#[derive(Debug, Serialize, Type)]
pub struct TagDimension {
    pub name: String,
    pub value_type: DimensionValueType,
    pub values: Vec<String>,
}

#[derive(Debug, Serialize, Type)]
pub struct PlaybackSample {
    #[specta(type = Number<i64>)]
    pub id: i64,
    pub filename: String,
    pub path: String,
    #[specta(type = Option<Number<i64>>)]
    pub duration_ms: Option<i64>,
    pub waveform_data: Option<Vec<u8>>,
    pub is_loop: bool,
}

/// One tag row joined with its dimension, as fetched for building
/// `SampleRow.tags` and computing conflicts.
#[derive(Debug, Clone)]
struct TagRow {
    dimension: String,
    value_type: DimensionValueType,
    value: String,
    source: TagSource,
    confidence: Option<f64>,
    is_primary: bool,
}

/// Clone the pool out of the state mutex so commands never hold the guard
/// across their queries.
async fn pool_from_state(state: &State<'_, AppState>) -> Result<sqlx::SqlitePool, CommandError> {
    let guard = state.db.lock().await;
    guard.as_ref().cloned().ok_or(CommandError::NoLibraryOpen)
}

#[tauri::command]
#[specta::specta]
pub fn get_ml_model_status() -> Result<MlModelStatus, CommandError> {
    let model_dir = models::clap_model_dir()?;
    let essentia_model_dir = models::essentia_model_dir()?;
    models::ml_model_status_for_dirs(&model_dir, &essentia_model_dir)
}

#[tauri::command]
#[specta::specta]
pub async fn download_ml_model(app: AppHandle) -> Result<MlModelStatus, CommandError> {
    let model_dir = models::clap_model_dir()?;
    let essentia_model_dir = models::essentia_model_dir()?;
    models::download_ml_models_to_dirs(&app, &model_dir, &essentia_model_dir).await
}

#[tauri::command]
#[specta::specta]
pub async fn open_library(
    path: String,
    state: State<'_, AppState>,
) -> Result<open::LibraryMeta, CommandError> {
    let db_path = PathBuf::from(&path).join("library.db");
    let pool = db::open_pool(&db_path).await?;
    let meta = open::open_or_create_library(&path, &pool).await?;

    *state.db.lock().await = Some(pool);
    *state.library_root.lock().await = Some(PathBuf::from(path));

    Ok(meta)
}

#[tauri::command]
#[specta::specta]
pub async fn start_discovery(
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), CommandError> {
    let pool = pool_from_state(&state).await?;
    let root = {
        let guard = state.library_root.lock().await;
        guard.as_ref().ok_or(CommandError::NoLibraryOpen)?.clone()
    };
    let cancellation = Arc::new(AtomicBool::new(false));
    *state.discovery_cancel.lock().await = Some(cancellation.clone());

    tokio::spawn(async move {
        let result = discover::discover_audio_files(&root, &pool, app.clone(), cancellation).await;
        *app.state::<AppState>().discovery_cancel.lock().await = None;

        if let Err(e) = result {
            if !matches!(e, CommandError::DiscoveryCancelled { .. }) {
                eprintln!("Discovery error: {e}");
            }
        }
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_discovery(state: State<'_, AppState>) -> Result<(), CommandError> {
    if let Some(cancellation) = state.discovery_cancel.lock().await.as_ref() {
        cancellation.store(true, Ordering::Relaxed);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn start_analysis(
    reanalyze: bool,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<(), CommandError> {
    let pool = pool_from_state(&state).await?;
    let cancellation = Arc::new(AtomicBool::new(false));
    *state.analysis_cancel.lock().await = Some(cancellation.clone());

    tokio::spawn(async move {
        let result = async {
            if reanalyze {
                analysis::requeue_all_samples(&pool).await?;
            }
            analysis::run_pending_analysis(&pool, app.clone(), cancellation).await
        }
        .await;
        *app.state::<AppState>().analysis_cancel.lock().await = None;

        if let Err(e) = result {
            eprintln!("Analysis error: {e}");
            app.emit(
                "analysis-failed",
                serde_json::json!({ "error": e.to_string() }),
            )
            .ok();
        }
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn cancel_analysis(state: State<'_, AppState>) -> Result<(), CommandError> {
    if let Some(cancellation) = state.analysis_cancel.lock().await.as_ref() {
        cancellation.store(true, Ordering::Relaxed);
    }

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_samples(state: State<'_, AppState>) -> Result<Vec<SampleRow>, CommandError> {
    let pool = pool_from_state(&state).await?;
    sample_rows(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_sample(
    sample_id: SampleId,
    state: State<'_, AppState>,
) -> Result<SampleRow, CommandError> {
    let pool = pool_from_state(&state).await?;
    sample_row(&pool, sample_id.0)
        .await?
        .ok_or_else(|| CommandError::Other("Unknown sample".to_string()))
}

#[derive(sqlx::FromRow)]
struct SampleDbRow {
    id: i64,
    filename: String,
    relative_path: String,
    format: Option<String>,
    size_bytes: Option<i64>,
    duration_ms: Option<i64>,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
    channels: Option<i64>,
    analysis_status: AnalysisStatus,
}

pub async fn sample_rows(pool: &sqlx::SqlitePool) -> Result<Vec<SampleRow>, CommandError> {
    let rows = sqlx::query_as!(
        SampleDbRow,
        "SELECT id, filename, relative_path, format, size_bytes, duration_ms, sample_rate, bit_depth, channels, analysis_status as \"analysis_status: AnalysisStatus\"
         FROM samples ORDER BY relative_path",
    )
    .fetch_all(pool)
    .await?;

    let tag_rows = sqlx::query!(
        "SELECT t.sample_id as \"sample_id!: i64\",
                d.name as dimension,
                d.value_type as \"value_type: DimensionValueType\",
                COALESCE(dv.value, CAST(t.numeric_value AS TEXT), t.text_value) as \"value: String\",
                t.source as \"source: TagSource\",
                t.confidence,
                t.is_primary as \"is_primary: bool\"
         FROM tags t
         JOIN dimensions d ON d.id = t.dimension_id
         LEFT JOIN dimension_values dv ON dv.id = t.value_id
         ORDER BY t.sample_id, d.sort_order, t.is_primary DESC, value",
    )
    .fetch_all(pool)
    .await?;

    let mut tags_by_sample: HashMap<i64, Vec<TagRow>> = HashMap::new();
    for row in tag_rows {
        let Some(value) = row.value else { continue };
        tags_by_sample
            .entry(row.sample_id)
            .or_default()
            .push(TagRow {
                dimension: row.dimension,
                value_type: row.value_type,
                value,
                source: row.source,
                confidence: row.confidence,
                is_primary: row.is_primary,
            });
    }

    Ok(rows
        .into_iter()
        .map(|row| {
            let tag_rows = tags_by_sample.remove(&row.id).unwrap_or_default();
            build_sample_row(row, tag_rows)
        })
        .collect())
}

async fn sample_row(
    pool: &sqlx::SqlitePool,
    sample_id: i64,
) -> Result<Option<SampleRow>, CommandError> {
    let row = sqlx::query_as!(
        SampleDbRow,
        "SELECT id, filename, relative_path, format, size_bytes, duration_ms, sample_rate, bit_depth, channels, analysis_status as \"analysis_status: AnalysisStatus\"
         FROM samples WHERE id = ?",
        sample_id,
    )
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else { return Ok(None) };
    let tag_rows = tag_rows_for_sample(pool, sample_id).await?;
    Ok(Some(build_sample_row(row, tag_rows)))
}

async fn tag_rows_for_sample(
    pool: &sqlx::SqlitePool,
    sample_id: i64,
) -> Result<Vec<TagRow>, CommandError> {
    let rows = sqlx::query!(
        "SELECT t.sample_id as \"sample_id!: i64\",
                d.name as dimension,
                d.value_type as \"value_type: DimensionValueType\",
                COALESCE(dv.value, CAST(t.numeric_value AS TEXT), t.text_value) as \"value: String\",
                t.source as \"source: TagSource\",
                t.confidence,
                t.is_primary as \"is_primary: bool\"
         FROM tags t
         JOIN dimensions d ON d.id = t.dimension_id
         LEFT JOIN dimension_values dv ON dv.id = t.value_id
         WHERE t.sample_id = ?
         ORDER BY d.sort_order, t.is_primary DESC, value",
        sample_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.value.map(|value| TagRow {
                dimension: row.dimension,
                value_type: row.value_type,
                value,
                source: row.source,
                confidence: row.confidence,
                is_primary: row.is_primary,
            })
        })
        .collect())
}

fn build_sample_row(row: SampleDbRow, tag_rows: Vec<TagRow>) -> SampleRow {
    let conflicts = conflicts_from_tag_rows(&tag_rows);
    let tags = tag_rows.into_iter().map(sample_tag_from_row).collect();
    SampleRow {
        id: row.id,
        filename: row.filename,
        relative_path: row.relative_path,
        format: row.format,
        size_bytes: row.size_bytes,
        duration_ms: row.duration_ms,
        sample_rate: row.sample_rate,
        bit_depth: row.bit_depth,
        channels: row.channels,
        analysis_status: row.analysis_status,
        tags,
        conflicts,
    }
}

fn sample_tag_from_row(row: TagRow) -> SampleTag {
    SampleTag {
        dimension: row.dimension,
        value: row.value,
        source: row.source,
        confidence: row.confidence,
        is_primary: row.is_primary,
    }
}

/// A dimension is in conflict when it is single-valued, has no user override,
/// and the automatic sources disagree (more than one distinct value).
fn conflicts_from_tag_rows(tag_rows: &[TagRow]) -> Vec<TagConflict> {
    let mut conflicts: Vec<TagConflict> = Vec::new();
    let mut seen_dimensions: Vec<&str> = Vec::new();

    // Rows are ordered by dimension sort_order, so iterating preserves it.
    for row in tag_rows {
        if seen_dimensions.contains(&row.dimension.as_str()) {
            continue;
        }
        seen_dimensions.push(&row.dimension);

        if matches!(row.value_type, DimensionValueType::MultiEnum) {
            continue;
        }

        let dimension_rows: Vec<&TagRow> = tag_rows
            .iter()
            .filter(|candidate| candidate.dimension == row.dimension)
            .collect();
        if dimension_rows
            .iter()
            .any(|candidate| matches!(candidate.source, TagSource::User))
        {
            continue;
        }

        let mut distinct_values: Vec<&str> = dimension_rows
            .iter()
            .map(|candidate| candidate.value.as_str())
            .collect();
        distinct_values.sort_unstable();
        distinct_values.dedup();
        if distinct_values.len() < 2 {
            continue;
        }

        let mut candidates: Vec<SampleTag> = dimension_rows
            .into_iter()
            .cloned()
            .map(sample_tag_from_row)
            .collect();
        candidates.sort_by(|a, b| a.value.cmp(&b.value));
        conflicts.push(TagConflict {
            dimension: row.dimension.clone(),
            candidates,
        });
    }

    conflicts
}

pub async fn conflicts_for_sample(
    pool: &sqlx::SqlitePool,
    sample_id: i64,
) -> Result<Vec<TagConflict>, CommandError> {
    let tag_rows = tag_rows_for_sample(pool, sample_id).await?;
    Ok(conflicts_from_tag_rows(&tag_rows))
}

#[tauri::command]
#[specta::specta]
pub async fn list_tag_dimensions(
    state: State<'_, AppState>,
) -> Result<Vec<TagDimension>, CommandError> {
    let pool = pool_from_state(&state).await?;
    tag_dimensions(&pool).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_user_tag(
    sample_id: SampleId,
    dimension: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let pool = pool_from_state(&state).await?;
    tags::write_user_tag(&pool, sample_id.0, &dimension, &value).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_user_tag_bulk(
    sample_ids: Vec<SampleId>,
    dimension: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let pool = pool_from_state(&state).await?;
    for sample_id in sample_ids {
        tags::write_user_tag(&pool, sample_id.0, &dimension, &value).await?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn clear_user_tag(
    sample_id: SampleId,
    dimension: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let pool = pool_from_state(&state).await?;
    let dimension_id = dimension_id_by_name(&pool, &dimension).await?;
    tags::clear_user_tag_for_dimension(&pool, sample_id.0, dimension_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn clear_user_tag_bulk(
    sample_ids: Vec<SampleId>,
    dimension: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let pool = pool_from_state(&state).await?;
    let dimension_id = dimension_id_by_name(&pool, &dimension).await?;
    for sample_id in sample_ids {
        tags::clear_user_tag_for_dimension(&pool, sample_id.0, dimension_id).await?;
    }
    Ok(())
}

async fn dimension_id_by_name(
    pool: &sqlx::SqlitePool,
    dimension: &str,
) -> Result<i64, CommandError> {
    let row = sqlx::query!(
        "SELECT id as \"id!: i64\" FROM dimensions WHERE name = ?",
        dimension,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CommandError::Other("Unknown dimension".to_string()))?;
    Ok(row.id)
}

#[tauri::command]
#[specta::specta]
pub async fn get_sample_playback(
    sample_id: SampleId,
    state: State<'_, AppState>,
) -> Result<PlaybackSample, CommandError> {
    let pool = pool_from_state(&state).await?;
    let library_root = {
        let guard = state.library_root.lock().await;
        guard.as_ref().ok_or(CommandError::NoLibraryOpen)?.clone()
    };

    playback_sample(&pool, &library_root, sample_id.0).await
}

pub async fn playback_sample(
    pool: &sqlx::SqlitePool,
    library_root: &Path,
    sample_id: i64,
) -> Result<PlaybackSample, CommandError> {
    let row = sqlx::query!(
        "SELECT id as \"id!: i64\", filename, path, duration_ms, waveform_data
         FROM samples
         WHERE id = ?",
        sample_id,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CommandError::Other("Unknown sample".to_string()))?;

    let canonical_root = library_root.canonicalize()?;
    let canonical_sample = PathBuf::from(&row.path).canonicalize()?;
    if !canonical_sample.starts_with(&canonical_root) {
        return Err(CommandError::Other(
            "Sample is outside the opened library".to_string(),
        ));
    }

    let loop_type = sqlx::query!(
        "SELECT 1 as \"exists!: i64\"
         FROM tags t
         JOIN dimensions d ON d.id = t.dimension_id
         JOIN dimension_values dv ON dv.id = t.value_id
         WHERE t.sample_id = ?
           AND t.is_primary = 1
           AND d.name = 'Type'
           AND dv.value = 'loop'
         LIMIT 1",
        sample_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(PlaybackSample {
        id: row.id,
        filename: row.filename,
        path: canonical_sample.to_string_lossy().to_string(),
        duration_ms: row.duration_ms,
        waveform_data: row.waveform_data,
        is_loop: loop_type.is_some(),
    })
}

pub async fn tag_dimensions(pool: &sqlx::SqlitePool) -> Result<Vec<TagDimension>, CommandError> {
    let dimensions = sqlx::query!(
        "SELECT id as \"id!: i64\", name, value_type as \"value_type: DimensionValueType\"
         FROM dimensions
         ORDER BY sort_order, name",
    )
    .fetch_all(pool)
    .await?;

    let value_rows = sqlx::query!(
        "SELECT dimension_id as \"dimension_id!: i64\", value
         FROM dimension_values
         ORDER BY value",
    )
    .fetch_all(pool)
    .await?;

    let mut values_by_dimension: HashMap<i64, Vec<String>> = HashMap::new();
    for row in value_rows {
        values_by_dimension
            .entry(row.dimension_id)
            .or_default()
            .push(row.value);
    }

    Ok(dimensions
        .into_iter()
        .map(|dimension| TagDimension {
            name: dimension.name,
            value_type: dimension.value_type,
            values: values_by_dimension
                .remove(&dimension.id)
                .unwrap_or_default(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::get_ml_model_status;
    use std::time::Duration;

    #[tokio::test]
    async fn ml_model_status_command_returns_without_tauri_app_handle() {
        let status = tokio::time::timeout(Duration::from_secs(1), async { get_ml_model_status() })
            .await
            .expect("status command timed out")
            .expect("status command failed");

        assert!(status.path.contains("larger_clap_music"));
    }
}
