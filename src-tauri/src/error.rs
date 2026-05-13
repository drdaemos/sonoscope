use serde::{Deserialize, Serialize};
use specta::Type;
use specta_typescript::Number;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum AnalysisStatus {
    Pending,
    Analysing,
    Done,
    Failed,
}

#[derive(Debug, thiserror::Error, Serialize, Type)]
pub enum CommandError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("No library is open")]
    NoLibraryOpen,
    #[error("Discovery cancelled after {count} files")]
    DiscoveryCancelled {
        #[specta(type = Number<u32>)]
        count: u32,
    },
    #[error("{0}")]
    Other(String),
}

impl From<sqlx::Error> for CommandError {
    fn from(e: sqlx::Error) -> Self {
        CommandError::Database(e.to_string())
    }
}

impl From<std::io::Error> for CommandError {
    fn from(e: std::io::Error) -> Self {
        CommandError::Io(e.to_string())
    }
}
