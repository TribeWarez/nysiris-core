use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentPayload {
    pub recipient: String,
    pub payload: Vec<u8>,
}

/// Transport adapter seam. The real implementation uses `nym-sdk`; tests use
/// an in-memory transport. The core never opens a socket itself.
pub trait Transport: Send + Sync {
    fn name(&self) -> &'static str {
        "unknown"
    }

    fn send(&self, recipient: &str, payload: Vec<u8>) -> Result<()>;
}
