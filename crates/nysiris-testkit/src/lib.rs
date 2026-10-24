//! In-memory transport for tests and the Phase 0 mock daemon mode.

use std::sync::{Arc, Mutex};

use nysiris_core::error::Result;
use nysiris_core::transport::{SentPayload, Transport};

#[derive(Clone, Default)]
pub struct MemoryTransport {
    sent: Arc<Mutex<Vec<SentPayload>>>,
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sent(&self) -> Vec<SentPayload> {
        self.sent.lock().expect("transport lock").clone()
    }

    pub fn take(&self) -> Vec<SentPayload> {
        std::mem::take(&mut *self.sent.lock().expect("transport lock"))
    }

    pub fn len(&self) -> usize {
        self.sent.lock().expect("transport lock").len()
    }

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
