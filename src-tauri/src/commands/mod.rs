use crate::db;
use crate::error::{AnalysisStatus, CommandError};
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

    Ok(rows
        .into_iter()
        .map(|r| SampleRow {
            id: r.id,
            filename: r.filename,
            relative_path: r.relative_path,
            format: r.format,
            size_bytes: r.size_bytes,
            analysis_status: r.analysis_status,
        })
        .collect())
}
