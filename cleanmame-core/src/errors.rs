use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, CleanMameError>;

#[derive(Debug, Error)]
pub enum CleanMameError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse XML: {0}")]
    Xml(String),
    #[error("failed to parse INI: {0}")]
    Ini(String),
    #[error("ROM '{0}' was not found")]
    RomNotFound(String),
    #[error("ROM entry '{0}' has no file path")]
    MissingPath(String),
}

pub(crate) fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> CleanMameError {
    CleanMameError::Io {
        path: path.into(),
        source,
    }
}
