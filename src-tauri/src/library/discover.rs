use crate::error::{AnalysisStatus, CommandError};
use sqlx::SqlitePool;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &["wav", "aiff", "aif", "flac", "mp3", "ogg"];
const PROGRESS_INTERVAL: u32 = 50;

/// Public entry point — wraps the DB logic and emits Tauri events.
pub async fn discover_audio_files(
    root: &Path,
    pool: &SqlitePool,
    app: AppHandle,
    cancellation: Arc<AtomicBool>,
) -> Result<u32, CommandError> {
    let result = run_discovery_cancellable(root, pool, cancellation, |n| {
        app.emit("discovery-progress", serde_json::json!({ "count": n }))
            .ok();
    })
    .await;

    match result {
        Ok(count) => {
            app.emit("discovery-complete", serde_json::json!({ "total": count }))
                .ok();
            Ok(count)
        }
        Err(CommandError::DiscoveryCancelled { count }) => {
            app.emit("discovery-cancelled", serde_json::json!({ "count": count }))
                .ok();
            Err(CommandError::DiscoveryCancelled { count })
        }
        Err(e) => Err(e),
    }
}

/// Core discovery logic. All inserts are wrapped in a single transaction so
/// that an interrupted scan leaves the database in its pre-scan state.
pub async fn run_discovery(
    root: &Path,
    pool: &SqlitePool,
    on_progress: impl Fn(u32),
) -> Result<u32, CommandError> {
    run_discovery_cancellable(root, pool, Arc::new(AtomicBool::new(false)), on_progress).await
}

/// Core discovery logic with cancellation support. Returning before commit drops
/// the transaction, so cancellation preserves the pre-scan database state.
pub async fn run_discovery_cancellable(
    root: &Path,
    pool: &SqlitePool,
    cancellation: Arc<AtomicBool>,
    on_progress: impl Fn(u32),
) -> Result<u32, CommandError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    let mut tx = pool.begin().await?;
    let mut count: u32 = 0;

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if cancellation.load(Ordering::Relaxed) {
            return Err(CommandError::DiscoveryCancelled { count });
        }

        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        let Some(ext) = ext else { continue };
        if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        let abs_path = path.to_string_lossy().to_string();
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let format = ext.clone();
        let size_bytes = entry.metadata().ok().map(|m| m.len() as i64);
        let status = AnalysisStatus::Pending;

        sqlx::query!(
            "INSERT OR IGNORE INTO samples
             (path, filename, relative_path, format, size_bytes, analysis_status, discovered_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            abs_path,
            filename,
            relative_path,
            format,
            size_bytes,
            status,
            now,
            now,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            "UPDATE samples SET last_seen_at = ? WHERE path = ?",
            now,
            abs_path,
        )
        .execute(&mut *tx)
        .await?;

        count += 1;
        if count % PROGRESS_INTERVAL == 0 {
            on_progress(count);
        }
    }

    sqlx::query!(
        "UPDATE library_meta SET last_discovered_at = ? WHERE id = 1",
        now,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(count)
}
