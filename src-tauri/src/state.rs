use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::Mutex;

pub struct AppState {
    pub db: Mutex<Option<SqlitePool>>,
    pub library_root: Mutex<Option<PathBuf>>,
    pub discovery_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl Default for AppState {
    fn default() -> Self {
        AppState {
            db: Mutex::new(None),
            library_root: Mutex::new(None),
            discovery_cancel: Mutex::new(None),
        }
    }
}
