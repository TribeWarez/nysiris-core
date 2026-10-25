#![warn(missing_docs)]
//! In-memory transport for tests and the Phase 0 mock daemon mode.

use std::sync::{Arc, Mutex};

use nysiris_core::error::Result;
use nysiris_core::transport::{SentPayload, Transport};

/// A transport that records what it was asked to send, without any network.
#[derive(Clone, Default)]
pub struct MemoryTransport {
    sent: Arc<Mutex<Vec<SentPayload>>>,
}

impl MemoryTransport {
    /// Create an empty transport.
    pub fn new() -> Self {
        Self::default()
    }

    /// A copy of everything recorded so far, without draining it.
    pub fn sent(&self) -> Vec<SentPayload> {
        self.sent.lock().expect("transport lock").clone()
    }

    /// Drain and return everything recorded so far.
    pub fn take(&self) -> Vec<SentPayload> {
        std::mem::take(&mut *self.sent.lock().expect("transport lock"))
    }

    /// Number of recorded payloads.
    pub fn len(&self) -> usize {
        self.sent.lock().expect("transport lock").len()
    }

    /// Whether nothing has been recorded.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Transport for MemoryTransport {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn send(&self, recipient: &str, payload: Vec<u8>) -> Result<()> {
        self.sent.lock().expect("transport lock").push(SentPayload {
            recipient: recipient.to_string(),
            payload,
        });
        Ok(())
    }
}
