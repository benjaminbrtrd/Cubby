use std::path::PathBuf;

/// The error type of every fallible function in cubby-core.
///
/// Variants carry enough context to be shown to the user as-is (the UI displays `to_string()`).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Couldn't reach {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("{url} returned HTTP {status}")]
    HttpStatus { url: String, status: u16 },

    #[error("{path} is damaged (expected SHA-1 {expected}, got {actual})")]
    HashMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("Couldn't access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Couldn't read the data from {what}: {source}")]
    Json {
        what: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("Couldn't read the settings in {path}: {message}")]
    Toml { path: PathBuf, message: String },

    #[error("Couldn't find {0}")]
    NotFound(String),

    #[error("{0}")]
    Auth(String),

    #[error("{0}")]
    Java(String),

    #[error("{0}")]
    Launch(String),

    #[error("Operation canceled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io { path: path.into(), source }
    }

    pub fn json(what: impl Into<String>, source: serde_json::Error) -> Self {
        Self::Json { what: what.into(), source }
    }

    pub fn http(url: impl Into<String>, source: reqwest::Error) -> Self {
        Self::Http { url: url.into(), source }
    }
}
