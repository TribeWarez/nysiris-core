/// Errors shared by the Nysiris native core.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// A key, public point, or secret is malformed.
    #[error("invalid key: {0}")]
    InvalidKey(String),
    /// Input failed validation (size limits, addressing, unknown member).
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Encryption, decryption, or signature verification failed.
    #[error("cryptographic failure: {0}")]
    Crypto(String),
    /// Local filesystem failure.
    #[error("io failure: {0}")]
    Io(#[from] std::io::Error),
    /// Envelope or journal serialization failure.
    #[error("json failure: {0}")]
    Json(#[from] serde_json::Error),
    /// The transport refused or failed to deliver a payload.
    #[error("transport failure: {0}")]
    Transport(String),
    /// A contact, room, or other referenced item is not present locally.
    #[error("not found: {0}")]
    NotFound(String),
    /// A history journal is missing, truncated, tampered with, or unreadable.
    #[error("history failure: {0}")]
    History(String),
}

/// Convenience alias for results produced by this crate.
pub type Result<T> = std::result::Result<T, CoreError>;
