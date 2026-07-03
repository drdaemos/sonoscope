use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub db: Mutex<Option<SqlitePool>>,
    pub library_root: Mutex<Option<PathBuf>>,
    pub discovery_cancel: Mutex<Option<Arc<AtomicBool>>>,
    pub analysis_cancel: Mutex<Option<Arc<AtomicBool>>>,
}
