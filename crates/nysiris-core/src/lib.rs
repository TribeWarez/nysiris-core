#![warn(missing_docs)]
//! Shared, UI-agnostic primitives for Nysiris native clients.
//!
//! This crate intentionally has no Nym, Qt, Python, or storage-backend
//! dependency. It owns the chat identity, the DM wire framing, and the
//! transport trait that adapters implement.

/// Locally saved peers, petnames, and Nym addresses.
pub mod contact;
/// Per-conversation summaries for roster and chat-list views.
pub mod conversation;
/// Sealed direct-message encryption.
pub mod crypto;
/// Shared error and result types.
pub mod error;
/// Capability-key rooms, invites, and room message sealing.
pub mod group;
/// The ed25519 chat identity.
pub mod identity;
/// Conversation, room, and message id helpers.
pub mod ids;
/// Stored message types and the signed DM inner envelope.
pub mod message;
/// Encrypted single-blob file helpers.
pub mod sealed_file;
/// The transport seam that adapters (Nym, in-memory) implement.
pub mod transport;

pub use contact::Contact;
pub use conversation::{ConversationSummary, GroupSummary};
pub use crypto::{SealedDm, DM_AAD};
pub use error::{CoreError, Result};
pub use group::{
    Group, GroupInner, GroupMember, SealedGroupMessage, GROUP_ID_BYTES, GROUP_KEY_BYTES,
    MAX_GROUP_MEMBERS,
};
pub use identity::ChatIdentity;
pub use message::{ChatMessage, Direction, DmInner, MessageState, MAX_BODY_BYTES};
pub use transport::{SentPayload, Transport};
