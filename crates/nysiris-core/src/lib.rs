//! Shared, UI-agnostic primitives for Nysiris native clients.
//!
//! This crate intentionally has no Nym, Qt, Python, or storage-backend
//! dependency. It owns the chat identity, the DM wire framing, and the
//! transport trait that adapters implement.

pub mod contact;
pub mod conversation;
pub mod crypto;
pub mod error;
pub mod group;
pub mod identity;
pub mod ids;
pub mod message;
pub mod sealed_file;
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
