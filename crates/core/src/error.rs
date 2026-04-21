use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("database opened read-only")]
    ReadOnly,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

impl Error {
    /// Stable machine-readable code. Used by the CLI to map onto exit codes and
    /// by the desktop app to branch on error classes.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io(_) => "io",
            Error::Sqlite(_) => "sql",
            Error::ReadOnly => "readonly",
            Error::NotFound(_) => "not_found",
            Error::Invalid(_) => "invalid",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
