use crate::error::CommandError;
use serde::Serialize;
use specta::Type;
use specta_typescript::Number;
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Type)]
pub struct LibraryMeta {
    pub root_path: String,
    #[specta(type = Number<i64>)]
    pub created_at: i64,
    #[specta(type = Option<Number<i64>>)]
    pub last_discovered_at: Option<i64>,
}

pub async fn open_or_create_library(
    root: &str,
    pool: &SqlitePool,
) -> Result<LibraryMeta, CommandError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    sqlx::query!(
        "INSERT OR IGNORE INTO library_meta (id, root_path, created_at) VALUES (1, ?, ?)",
        root,
        now,
    )
    .execute(pool)
    .await?;

    let row = sqlx::query!(
        "SELECT root_path, created_at, last_discovered_at FROM library_meta WHERE id = 1"
    )
    .fetch_one(pool)
    .await?;

    Ok(LibraryMeta {
        root_path: row.root_path,
        created_at: row.created_at,
        last_discovered_at: row.last_discovered_at,
    })
}
