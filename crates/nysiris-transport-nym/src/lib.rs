//! Nym transport adapter.
//!
//! The real `nym-sdk` implementation is behind the `nym` feature so the fast
//! default workspace build stays light. Without it, `NymTransport::connect`
//! fails explicitly rather than pretending to deliver messages.

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InboundMessage {
    pub payload: Vec<u8>,
    pub sender_tag: Option<[u8; 16]>,
}

#[cfg(feature = "nym")]
mod real;
#[cfg(not(feature = "nym"))]
mod stub;

#[cfg(feature = "nym")]
pub use real::NymTransport;
#[cfg(not(feature = "nym"))]
pub use stub::NymTransport;

pub(crate) const READY_TIMEOUT: Duration = Duration::from_secs(180);
