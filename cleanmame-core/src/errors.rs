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
    #[error("could not determine the per-user cache directory")]
    CacheDirectoryUnavailable,
    #[error("no cached mame.xml is available and no MAME executable was provided")]
    MameXmlUnavailable,
    #[error("failed to run {executable} -listxml (exited with {status})")]
    MameExecution { executable: PathBuf, status: String },
    #[error("MAME -listxml output was not valid UTF-8: {0}")]
    MameEncoding(#[source] std::string::FromUtf8Error),
    #[error("failed to download CatVer from {url}: {source}")]
    CatverDownload {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("downloaded CatVer content was not valid UTF-8: {0}")]
    CatverEncoding(#[source] std::string::FromUtf8Error),
    #[error("downloaded CatVer did not contain any category entries")]
    InvalidCatverDownload,
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
