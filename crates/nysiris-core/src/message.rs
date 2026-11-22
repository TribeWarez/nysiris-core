//! Signed DM inner envelope, byte-exact with the Nysiris browser client.
//!
//! The envelope is JSON inside the sealed box: `{v, from, ts, msgId, bodyB64,
//! atts?, sig}`. Signature bytes are
//! `fly-social-v1/dm-inner || from(32) || ts_be64 || msgId(16) || body || atts`,
//! where `atts` is the canonical attachment list (empty for none). Version 2
//! adds attachments; a version-1 envelope with no `atts` verifies identically.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::attach::{canonical_attachments, parse_attachments, AttachmentRef};
use crate::error::{CoreError, Result};
use crate::identity::ChatIdentity;
use crate::ids::{msg_id_bytes, now_ms, pubkey_bytes, random_hex};

/// Maximum accepted message body, in bytes. Keeps one message inside a single
/// Nym Sphinx payload after sealing and framing.
pub const MAX_BODY_BYTES: usize = 1800;
/// Domain separator mixed into the DM inner-envelope signature.
pub const DM_INNER_DOMAIN: &[u8] = b"fly-social-v1/dm-inner";
/// Version of the signed DM inner envelope (`atts` support).
pub const DM_INNER_VERSION: u8 = 2;

mod b64 {
    use super::*;

    pub(super) fn serialize<S: Serializer>(
        bytes: &[u8],
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&BASE64.encode(bytes))
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<Vec<u8>, D::Error> {
        let value = String::deserialize(d)?;
        BASE64
            .decode(value.trim())
            .map_err(|_| serde::de::Error::custom("bodyB64 is not base64"))
    }
}

/// Whether a stored message was sent or received by this identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Received from the peer.
    Incoming,
    /// Sent by this identity.
    Outgoing,
}

/// Delivery state recorded alongside a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageState {
    /// Handed to the transport without an error.
    Sent,
    /// Waiting for an outbox retry.
    Queued,
    /// The transport rejected the message.
    Failed,
}

/// A locally stored chat message. The body is plaintext only inside the
/// encrypted history journal and in memory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Sender-generated message id, lowercase hex.
    pub msg_id: String,
    /// `dm:<peer pubkey>` for direct messages, `group:<room id>` for rooms.
    pub conversation_id: String,
    /// Sender's ed25519 public key, lowercase hex.
    pub from: String,
    /// Recipient public key for DMs, room id for room messages.
    pub to: String,
    /// Message text. Never persist this unencrypted.
    pub body: String,
    /// Sender-supplied Unix timestamp in milliseconds.
    pub at: u64,
    /// Whether this identity sent or received the message.
    pub direction: Direction,
    /// Delivery state at the time the record was written.
    pub state: MessageState,
}

impl ChatMessage {
    /// Build a received 1:1 message.
    pub fn incoming(from: &str, to: &str, msg_id: &str, body: &str, at: u64) -> Self {
        Self {
            msg_id: msg_id.to_lowercase(),
            conversation_id: crate::ids::dm_conversation_id(from),
            from: from.to_lowercase(),
            to: to.to_lowercase(),
            body: body.to_string(),
            at,
            direction: Direction::Incoming,
            state: MessageState::Sent,
        }
    }

    /// Build a sent 1:1 message.
    pub fn outgoing(from: &str, to: &str, msg_id: &str, body: &str, at: u64) -> Self {
        Self {
            msg_id: msg_id.to_lowercase(),
            conversation_id: crate::ids::dm_conversation_id(to),
            from: from.to_lowercase(),
            to: to.to_lowercase(),
            body: body.to_string(),
            at,
            direction: Direction::Outgoing,
            state: MessageState::Sent,
        }
    }

    /// Build a received room message. `to` is the room id.
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

    /// Build a sent room message. `to` is the room id.
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

    /// The other party: `from` for received messages, `to` for sent ones.
    pub fn peer_pub(&self) -> &str {
        match self.direction {
            Direction::Incoming => &self.from,
            Direction::Outgoing => &self.to,
        }
    }
}

/// Signed plaintext carried inside the sealed DM envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DmInner {
    /// Envelope version: 1 (legacy, no attachments) or 2 (with `atts`).
    pub v: u8,
    /// Sender ed25519 public key, lowercase hex.
    pub from: String,
    /// Sender-supplied Unix timestamp in milliseconds.
    pub ts: u64,
    /// Sender-generated message id, lowercase hex (16 bytes).
    #[serde(rename = "msgId")]
    pub msg_id: String,
    /// Message bytes, standard padded base64 on the wire as `bodyB64`.
    #[serde(rename = "bodyB64", with = "b64")]
    pub body: Vec<u8>,
    /// Attachment refs (keys included — this envelope is sealed).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub atts: Vec<AttachmentRef>,
    /// ed25519 signature over the domain-separated envelope fields.
    pub sig: String,
}

fn inner_sign_bytes(
    from_bytes: &[u8; 32],
    ts: u64,
    msg_id_bytes: &[u8; 16],
    body: &[u8],
    atts: &[AttachmentRef],
) -> Vec<u8> {
    let att_bytes = canonical_attachments(atts);
    let mut out =
        Vec::with_capacity(DM_INNER_DOMAIN.len() + 32 + 8 + 16 + body.len() + att_bytes.len());
    out.extend_from_slice(DM_INNER_DOMAIN);
    out.extend_from_slice(from_bytes);
    out.extend_from_slice(&ts.to_be_bytes());
    out.extend_from_slice(msg_id_bytes);
    out.extend_from_slice(body);
    out.extend_from_slice(&att_bytes);
    out
}

/// A signed, JSON-packed inner envelope and its random 16-byte message id.
#[derive(Clone, Debug)]
pub struct PackedDm {
    /// JSON envelope, ready to be sealed with `seal_dm`.
    pub json: String,
    /// Lowercase hex 16-byte id — the conversation dedupe key.
    pub msg_id: String,
}

fn pack_at(
    identity: &ChatIdentity,
    msg_id_hex: &str,
    ts: u64,
    body: &[u8],
    atts: &[AttachmentRef],
) -> Result<String> {
    if body.is_empty() || body.len() > MAX_BODY_BYTES {
        return Err(CoreError::InvalidInput(
            "DM body has an invalid size".into(),
        ));
    }
    let atts = parse_attachments(atts)?;
    let from = identity.public_hex();
    let from_bytes = pubkey_bytes(&from)?;
    let id = msg_id_bytes(msg_id_hex)?;
    let bytes = inner_sign_bytes(&from_bytes, ts, &id, body, &atts);
    let inner = DmInner {
        v: DM_INNER_VERSION,
        from,
        ts,
        msg_id: msg_id_hex.trim().to_lowercase(),
        body: body.to_vec(),
        atts,
        sig: identity.sign_hex(&bytes),
    };
    Ok(serde_json::to_string(&inner)?)
}

/// Sign a 1:1 message with a fresh id and local timestamp. The sender's
/// identity is proven inside the sealed box only; the provider cannot see it.
pub fn pack_dm_inner(
    identity: &ChatIdentity,
    body: &[u8],
    atts: &[AttachmentRef],
) -> Result<PackedDm> {
    let msg_id = random_hex(16);
    Ok(PackedDm {
        json: pack_at(identity, &msg_id, now_ms(), body, atts)?,
        msg_id,
    })
}

/// Deterministic packing for shared conformance vectors. Production code must
/// use `pack_dm_inner` so ids and timestamps are fresh.
pub fn pack_dm_inner_at(
    identity: &ChatIdentity,
    msg_id_hex: &str,
    ts: u64,
    body: &[u8],
    atts: &[AttachmentRef],
) -> Result<String> {
    pack_at(identity, msg_id_hex, ts, body, atts)
}

/// Verify and unpack an inner DM envelope. Fails closed on any defect.
pub fn unpack_dm_inner(json: &str) -> Result<DmInner> {
    let inner: DmInner = serde_json::from_str(json)?;
    if inner.v != DM_INNER_VERSION && inner.v != 1 {
        return Err(CoreError::InvalidInput(format!(
            "unsupported DM inner version {}",
            inner.v
        )));
    }
    if inner.body.is_empty() || inner.body.len() > MAX_BODY_BYTES {
        return Err(CoreError::InvalidInput(
            "DM body has an invalid size".into(),
        ));
    }
    let from = crate::ids::normalize_pubkey_hex(&inner.from)?;
    let from_bytes = pubkey_bytes(&from)?;
    let id = msg_id_bytes(&inner.msg_id)?;
    let atts = parse_attachments(&inner.atts)?;
    let bytes = inner_sign_bytes(&from_bytes, inner.ts, &id, &inner.body, &atts);
    ChatIdentity::verify_hex(&from, &bytes, &inner.sig)?;
    Ok(DmInner {
        from,
        atts,
        ..inner
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inner_envelope_round_trips_and_binds_sender() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let packed = pack_dm_inner(&alice, b"hello", &[]).unwrap();
        let opened = unpack_dm_inner(&packed.json).unwrap();
        assert_eq!(opened.body, b"hello");
        assert_eq!(opened.from, alice.public_hex());
        assert_eq!(opened.v, 2);
    }

    #[test]
    fn tampered_body_and_attachments_fail_verification() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let packed = pack_dm_inner(&alice, b"hello", &[]).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&packed.json).unwrap();
        value["bodyB64"] = serde_json::json!("aGVsbG8h");
        assert!(unpack_dm_inner(&value.to_string()).is_err());

        let att = AttachmentRef {
            id: "ab".repeat(32),
            name: "x.png".into(),
            mime: "image/png".into(),
            size: 1,
            key: "cd".repeat(32),
        };
        let with_att = pack_dm_inner(&alice, b"hello", std::slice::from_ref(&att)).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&with_att.json).unwrap();
        value["atts"][0]["size"] = serde_json::json!(2);
        assert!(unpack_dm_inner(&value.to_string()).is_err());
    }

    #[test]
    fn version_one_messages_verify_unchanged() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let packed = pack_dm_inner_at(&alice, &"ab".repeat(16), 123, b"hello", &[]).unwrap();
        let mut legacy: serde_json::Value = serde_json::from_str(&packed).unwrap();
        legacy["v"] = serde_json::json!(1);
        legacy.as_object_mut().unwrap().remove("atts");
        // The signature does not bind the version byte, so a v1 relabel still
        // verifies — this is the compatibility rule shared with the browser.
        assert!(unpack_dm_inner(&legacy.to_string()).is_ok());
    }
}
