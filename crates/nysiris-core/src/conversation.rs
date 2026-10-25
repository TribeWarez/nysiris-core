use serde::{Deserialize, Serialize};

use crate::message::ChatMessage;

/// Roster entry for a 1:1 conversation, derived from stored history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationSummary {
    /// Conversation id, `dm:<peer pubkey>`.
    pub id: String,
    /// The peer's ed25519 public key, lowercase hex.
    pub peer_pub: String,
    /// Saved petname, if the peer is still in the contact list.
    pub petname: Option<String>,
    /// Most recent message, if the conversation has any.
    pub last_message: Option<ChatMessage>,
    /// Total stored messages in this conversation.
    pub message_count: usize,
}

/// Roster entry for a room, derived from local state and history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupSummary {
    /// Room id, 16 bytes as lowercase hex.
    pub id: String,
    /// Room name chosen at creation time.
    pub name: String,
    /// Number of members, including this identity.
    pub member_count: usize,
    /// Local copy of the member list. There is no authoritative server roster.
    pub members: Vec<crate::group::GroupMember>,
    /// Most recent message, if the room has any.
    pub last_message: Option<ChatMessage>,
    /// Total stored messages in this room.
    pub message_count: usize,
}
