use crate::error::Result;

/// A payload recorded by a transport, kept for tests and outbox inspection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentPayload {
    /// Recipient transport address, as given to [`Transport::send`].
    pub recipient: String,
    /// Opaque sealed bytes handed to the transport.
    pub payload: Vec<u8>,
}

/// Transport adapter seam. The real implementation uses `nym-sdk`; tests use
/// an in-memory transport. The core never opens a socket itself.
pub trait Transport: Send + Sync {
    /// Short name for diagnostics, for example `nym` or `memory`.
    fn name(&self) -> &'static str {
        "unknown"
    }

    /// Deliver one opaque payload to a transport address.
    ///
    /// An `Ok(())` return means the transport accepted the payload, not that the
    /// peer received or read it. Nym queues may hold it for some time.
    fn send(&self, recipient: &str, payload: Vec<u8>) -> Result<()>;
}
