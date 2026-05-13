use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::path::Path;
use std::str::FromStr;

pub async fn open_pool(db_path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePool::connect_with(opts).await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}
