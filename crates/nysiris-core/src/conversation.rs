use serde::{Deserialize, Serialize};

use crate::message::ChatMessage;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub peer_pub: String,
    pub petname: Option<String>,
    pub last_message: Option<ChatMessage>,
    pub message_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupSummary {
    pub id: String,
    pub name: String,
    pub member_count: usize,
    pub members: Vec<crate::group::GroupMember>,
    pub last_message: Option<ChatMessage>,
    pub message_count: usize,
}
