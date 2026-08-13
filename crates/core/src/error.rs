#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse {kind} error: {value}")]
    Parse { kind: &'static str, value: String },
    #[error("could not determine data directory")]
    NoDataDir,
    #[error("invalid migration version: {0}")]
    MigrationVersion(i64),
    #[error("database version {0} is newer than code expects")]
    DatabaseVersion(i64), 
}

pub type Result<T> = std::result::Result<T, Error>;
