use crate::analysis;
use crate::db;
use crate::error::{AnalysisStatus, CommandError, DimensionValueType, TagSource};
use crate::library::{discover, open};
use crate::state::AppState;
use serde::Serialize;
use specta::Type;
use specta_typescript::Number;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Serialize, Type)]
pub struct SampleRow {
    #[specta(type = Number<i64>)]
    pub id: i64,
    pub filename: String,
    pub relative_path: String,
    pub format: Option<String>,
    #[specta(type = Option<Number<i64>>)]
    pub size_bytes: Option<i64>,
    pub analysis_status: AnalysisStatus,
    pub tags: Vec<SampleTag>,
}

#[derive(Debug, Serialize, Type)]
pub struct SampleTag {
    pub dimension: String,
    pub value: String,
    pub source: TagSource,
    pub confidence: Option<f64>,
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
    let pool = {
        let guard = state.db.lock().await;
        guard.as_ref().ok_or(CommandError::NoLibraryOpen)?.clone()
    };
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
    let pool = {
        let guard = state.db.lock().await;
        guard.as_ref().ok_or(CommandError::NoLibraryOpen)?.clone()
    };

    tokio::spawn(async move {
        if reanalyze {
            if let Err(e) = analysis::requeue_all_samples(&pool).await {
                eprintln!("Analysis requeue error: {e}");
                return;
            }
        }
        if let Err(e) = analysis::run_pending_analysis(&pool, app).await {
            eprintln!("Analysis error: {e}");
        }
    });

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_samples(state: State<'_, AppState>) -> Result<Vec<SampleRow>, CommandError> {
    let guard = state.db.lock().await;
    let pool = guard.as_ref().ok_or(CommandError::NoLibraryOpen)?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: i64,
        filename: String,
        relative_path: String,
        format: Option<String>,
        size_bytes: Option<i64>,
        analysis_status: AnalysisStatus,
    }

    let rows = sqlx::query_as!(
        Row,
        "SELECT id, filename, relative_path, format, size_bytes, analysis_status as \"analysis_status: AnalysisStatus\"
         FROM samples ORDER BY relative_path",
    )
    .fetch_all(pool)
    .await?;

    let mut samples = Vec::with_capacity(rows.len());
    for r in rows {
        let tags = sample_tags(pool, r.id).await?;
        samples.push(SampleRow {
            id: r.id,
            filename: r.filename,
            relative_path: r.relative_path,
            format: r.format,
            size_bytes: r.size_bytes,
            analysis_status: r.analysis_status,
            tags,
        });
    }

    Ok(samples)
}

#[tauri::command]
#[specta::specta]
pub async fn set_user_tag(
    sample_id: i32,
    dimension: String,
    value: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let guard = state.db.lock().await;
    let pool = guard.as_ref().ok_or(CommandError::NoLibraryOpen)?;
    write_user_tag(pool, i64::from(sample_id), &dimension, &value).await
}

#[tauri::command]
#[specta::specta]
pub async fn clear_user_tag(
    sample_id: i32,
    dimension: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let guard = state.db.lock().await;
    let pool = guard.as_ref().ok_or(CommandError::NoLibraryOpen)?;
    let sample_id = i64::from(sample_id);

    let user_source = TagSource::User;
    sqlx::query!(
        "DELETE FROM tags
         WHERE sample_id = ?
           AND source = ?
           AND dimension_id = (SELECT id FROM dimensions WHERE name = ?)",
        sample_id,
        user_source,
        dimension,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn sample_tags(
    pool: &sqlx::SqlitePool,
    sample_id: i64,
) -> Result<Vec<SampleTag>, CommandError> {
    let rows = sqlx::query!(
        "SELECT d.name as dimension,
                COALESCE(dv.value, CAST(t.numeric_value AS TEXT), t.text_value) as \"value: String\",
                t.source as \"source: TagSource\",
                t.confidence
         FROM tags t
         JOIN dimensions d ON d.id = t.dimension_id
         LEFT JOIN dimension_values dv ON dv.id = t.value_id
         WHERE t.sample_id = ?
         ORDER BY d.sort_order, value",
        sample_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.value.map(|value| SampleTag {
                dimension: row.dimension,
                value,
                source: row.source,
                confidence: row.confidence,
            })
        })
        .collect())
}

pub async fn write_user_tag(
    pool: &sqlx::SqlitePool,
    sample_id: i64,
    dimension_name: &str,
    value: &str,
) -> Result<(), CommandError> {
    let dimension = sqlx::query!(
        "SELECT id as \"id!: i64\", value_type as \"value_type: DimensionValueType\" FROM dimensions WHERE name = ?",
        dimension_name,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| CommandError::Other(format!("Unknown dimension: {dimension_name}")))?;

    clear_user_tag_for_dimension(pool, sample_id, dimension.id).await?;

    let user_source = TagSource::User;
    let now = unix_now();
    match dimension.value_type {
        DimensionValueType::Enum | DimensionValueType::MultiEnum => {
            let value_row = sqlx::query!(
                "SELECT id FROM dimension_values WHERE dimension_id = ? AND value = ?",
                dimension.id,
                value,
            )
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| {
                CommandError::Other(format!(
                    "Unknown value {value} for dimension {dimension_name}"
                ))
            })?;

            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, value_id, source, confidence, created_at)
                 VALUES (?, ?, ?, ?, NULL, ?)",
                sample_id,
                dimension.id,
                value_row.id,
                user_source,
                now,
            )
            .execute(pool)
            .await?;
        }
        DimensionValueType::Numeric => {
            let numeric_value = value
                .parse::<f64>()
                .map_err(|_| CommandError::Other(format!("{value} is not a valid number")))?;
            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, numeric_value, source, confidence, created_at)
                 VALUES (?, ?, ?, ?, NULL, ?)",
                sample_id,
                dimension.id,
                numeric_value,
                user_source,
                now,
            )
            .execute(pool)
            .await?;
        }
        DimensionValueType::Text => {
            sqlx::query!(
                "INSERT INTO tags
                 (sample_id, dimension_id, text_value, source, confidence, created_at)
                 VALUES (?, ?, ?, ?, NULL, ?)",
                sample_id,
                dimension.id,
                value,
                user_source,
                now,
            )
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

pub async fn clear_user_tag_for_dimension(
    pool: &sqlx::SqlitePool,
    sample_id: i64,
    dimension_id: i64,
) -> Result<(), CommandError> {
    let user_source = TagSource::User;
    sqlx::query!(
        "DELETE FROM tags WHERE sample_id = ? AND dimension_id = ? AND source = ?",
        sample_id,
        dimension_id,
        user_source,
    )
    .execute(pool)
    .await?;
    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}
