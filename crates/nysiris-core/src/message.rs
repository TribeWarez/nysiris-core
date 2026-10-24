use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::identity::ChatIdentity;
use crate::ids::{dm_conversation_id, msg_id_bytes, normalize_pubkey_hex};

pub const MAX_BODY_BYTES: usize = 1800;
pub const DM_INNER_DOMAIN: &[u8] = b"nysiris-chat-v1/dm-inner";
pub const DM_INNER_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    Sent,
    Queued,
    Failed,
}

/// A locally stored chat message. The body is plaintext only inside the
/// encrypted history journal and in memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub msg_id: String,
    pub conversation_id: String,
    pub from: String,
    pub to: String,
    pub body: String,
    pub at: u64,
    pub direction: Direction,
    pub state: MessageState,
}

impl ChatMessage {
    pub fn incoming(from: &str, to: &str, msg_id: &str, body: &str, at: u64) -> Self {
        Self {
            msg_id: msg_id.to_lowercase(),
            conversation_id: dm_conversation_id(from),
            from: from.to_lowercase(),
            to: to.to_lowercase(),
            body: body.to_string(),
            at,
            direction: Direction::Incoming,
            state: MessageState::Sent,
        }
    }

    pub fn outgoing(from: &str, to: &str, msg_id: &str, body: &str, at: u64) -> Self {
        Self {
            msg_id: msg_id.to_lowercase(),
            conversation_id: dm_conversation_id(to),
            from: from.to_lowercase(),
            to: to.to_lowercase(),
            body: body.to_string(),
            at,
            direction: Direction::Outgoing,
            state: MessageState::Sent,
        }
    }

    pub fn incoming_group(from: &str, gid: &str, msg_id: &str, body: &str, at: u64) -> Self {
        Self {
            msg_id: msg_id.to_lowercase(),
            conversation_id: format!("group:{}", gid.to_lowercase()),
            from: from.to_lowercase(),
            to: gid.to_lowercase(),
            body: body.to_string(),
            at,
            direction: Direction::Incoming,
            state: MessageState::Sent,
        }
    }

    pub fn outgoing_group(from: &str, gid: &str, msg_id: &str, body: &str, at: u64) -> Self {
        Self {
            msg_id: msg_id.to_lowercase(),
            conversation_id: format!("group:{}", gid.to_lowercase()),
            from: from.to_lowercase(),
            to: gid.to_lowercase(),
            body: body.to_string(),
            at,
            direction: Direction::Outgoing,
            state: MessageState::Sent,
        }
    }

    pub fn peer_pub(&self) -> &str {
        match self.direction {
            Direction::Incoming => &self.from,
            Direction::Outgoing => &self.to,
        }
    }
}

/// Signed plaintext carried inside the sealed DM envelope.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DmInner {
    pub v: u8,
    pub from: String,
    pub to: String,
    pub ts: u64,
    #[serde(rename = "msgId")]
    pub msg_id: String,
    pub body: String,
    pub sig: String,
}

fn inner_sign_bytes(
    from_hex: &str,
    to_hex: &str,
    ts: u64,
    msg_id_hex: &str,
    body: &str,
) -> Result<Vec<u8>> {
    let from = crate::ids::pubkey_bytes(from_hex)?;
    let to = crate::ids::pubkey_bytes(to_hex)?;
    let msg_id = msg_id_bytes(msg_id_hex)?;
    let mut out = Vec::with_capacity(DM_INNER_DOMAIN.len() + 32 + 32 + 8 + 16 + body.len());
    out.extend_from_slice(DM_INNER_DOMAIN);
    out.extend_from_slice(&from);
    out.extend_from_slice(&to);
    out.extend_from_slice(&ts.to_be_bytes());
    out.extend_from_slice(&msg_id);
    out.extend_from_slice(body.as_bytes());
    Ok(out)
}

/// Sign and JSON-pack the inner DM envelope.
pub fn pack_dm_inner(
    identity: &ChatIdentity,
    to_pub_hex: &str,
    msg_id_hex: &str,
    ts: u64,
    body: &str,
) -> Result<String> {
    if body.is_empty() {
        return Err(CoreError::InvalidInput("message body is empty".into()));
    }
    if body.len() > MAX_BODY_BYTES {
        return Err(CoreError::InvalidInput(format!(
            "message body exceeds {MAX_BODY_BYTES} bytes"
        )));
    }
    let from = identity.public_hex();
    let to = normalize_pubkey_hex(to_pub_hex)?;
    msg_id_bytes(msg_id_hex)?;
    let bytes = inner_sign_bytes(&from, &to, ts, msg_id_hex, body)?;
    let inner = DmInner {
        v: DM_INNER_VERSION,
        from,
        to,
        ts,
        msg_id: msg_id_hex.to_lowercase(),
        body: body.to_string(),
        sig: identity.sign_hex(&bytes),
    };
    Ok(serde_json::to_string(&inner)?)
}

/// Verify and unpack an inner DM envelope. Fails closed on any defect.
pub fn unpack_dm_inner(json: &str, own_pub_hex: &str) -> Result<DmInner> {
    let inner: DmInner = serde_json::from_str(json)?;
    if inner.v != DM_INNER_VERSION {
        return Err(CoreError::InvalidInput(format!(
            "unsupported DM inner version {}",
            inner.v
        )));
    }
    let from = normalize_pubkey_hex(&inner.from)?;
    let to = normalize_pubkey_hex(&inner.to)?;
    let own = normalize_pubkey_hex(own_pub_hex)?;
    if to != own {
        return Err(CoreError::InvalidInput(
            "DM inner envelope is addressed to another key".into(),
        ));
    }
    if inner.body.is_empty() || inner.body.len() > MAX_BODY_BYTES {
        return Err(CoreError::InvalidInput(
            "DM body has an invalid size".into(),
        ));
    }
    msg_id_bytes(&inner.msg_id)?;
    let bytes = inner_sign_bytes(&from, &to, inner.ts, &inner.msg_id, &inner.body)?;
    ChatIdentity::verify_hex(&from, &bytes, &inner.sig)?;
    Ok(DmInner { from, to, ..inner })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_envelope_round_trips_and_binds_recipient() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let id = "ab".repeat(16);
        let packed = pack_dm_inner(&alice, &bob.public_hex(), &id, 123, "hello").unwrap();
        let opened = unpack_dm_inner(&packed, &bob.public_hex()).unwrap();
        assert_eq!(opened.body, "hello");
        assert_eq!(opened.from, alice.public_hex());
        assert!(unpack_dm_inner(&packed, &alice.public_hex()).is_err());
    }

    #[test]
    fn tampered_body_fails_verification() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let id = "ab".repeat(16);
        let packed = pack_dm_inner(&alice, &bob.public_hex(), &id, 123, "hello").unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&packed).unwrap();
        value["body"] = serde_json::json!("hello!");
        assert!(unpack_dm_inner(&value.to_string(), &bob.public_hex()).is_err());
    }
}
