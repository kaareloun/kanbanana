#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("corrupt data in the database: {0}")]
    Corrupt(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("no data directory for the database, set KANBANANA_DB")]
    NoDataDir,
}

pub type Result<T> = std::result::Result<T, Error>;
