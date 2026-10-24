/// Errors shared by the Nysiris native core.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid key: {0}")]
    InvalidKey(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("cryptographic failure: {0}")]
    Crypto(String),
    #[error("io failure: {0}")]
    Io(#[from] std::io::Error),
    #[error("json failure: {0}")]
    Json(#[from] serde_json::Error),
    #[error("transport failure: {0}")]
    Transport(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("history failure: {0}")]
    History(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
