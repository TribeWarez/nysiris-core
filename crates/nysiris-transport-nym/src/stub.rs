use std::path::Path;
use std::time::Duration;

use nysiris_core::error::{CoreError, Result};
use nysiris_core::transport::Transport;

use crate::{InboundMessage, READY_TIMEOUT};

#[derive(Clone, Debug, Default)]
pub struct NymTransport;

impl NymTransport {
    pub fn connect(_data_dir: impl AsRef<Path>, _gateway: Option<String>) -> Result<Self> {
        let _ = READY_TIMEOUT;
        Err(CoreError::Transport(
            "nysiris-transport-nym was compiled without the nym feature; rebuild with --features nym".into(),
        ))
    }

    pub fn is_available() -> bool {
        false
    }

    pub fn address(&self) -> &str {
        "unavailable"
    }

    pub fn recv_timeout(&self, _timeout: Duration) -> Result<Option<InboundMessage>> {
        Ok(None)
    }

    pub fn shutdown(&self) {}
}

impl Transport for NymTransport {
    fn name(&self) -> &'static str {
        "nym-unavailable"
    }

    fn send(&self, _recipient: &str, _payload: Vec<u8>) -> Result<()> {
        Err(CoreError::Transport(
            "Nym transport is unavailable; rebuild ncc-daemon with --features nym".into(),
        ))
    }
}
