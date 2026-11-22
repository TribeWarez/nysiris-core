//! Regenerate the shared DM conformance fixture.
//!
//! `cargo run -p nysiris-core --example dm_vectors > crates/nysiris-core/tests/fixtures/dm-v2.json`
//!
//! Inputs are fixed; the outputs are the exact JSON inner envelope and sealed
//! box that both this crate and the Nysiris browser must produce.

use nysiris_core::attach::AttachmentRef;
use nysiris_core::crypto::{seal_dm_with, DM_KEY_DOMAIN};
use nysiris_core::identity::ChatIdentity;
use nysiris_core::message::{pack_dm_inner_at, DM_INNER_DOMAIN, DM_INNER_VERSION};
use x25519_dalek::StaticSecret;

#[allow(clippy::too_many_arguments)]
fn vector(
    name: &str,
    sender: &ChatIdentity,
    recipient: &ChatIdentity,
    ephemeral_secret: [u8; 32],
    nonce: [u8; 24],
    msg_id: &str,
    ts: u64,
    body: &[u8],
    atts: &[AttachmentRef],
) -> serde_json::Value {
    let inner = pack_dm_inner_at(sender, msg_id, ts, body, atts).unwrap();
    let sealed = seal_dm_with(
        &recipient.public_hex(),
        inner.as_bytes(),
        StaticSecret::from(ephemeral_secret),
        nonce,
    )
    .unwrap();
    serde_json::json!({
        "name": name,
        "bodyHex": hex::encode(body),
        "atts": atts,
        "inner": inner,
        "sealed": sealed,
    })
}

fn main() {
    let sender = ChatIdentity::from_secret_bytes([1u8; 32]);
    let recipient = ChatIdentity::from_secret_bytes([2u8; 32]);
    let ephemeral_secret = [3u8; 32];
    let nonce = [4u8; 24];
    let msg_id = "ab".repeat(16);
    let ts = 1_700_000_000_123u64;
    let att = AttachmentRef {
        id: "ab".repeat(32),
        name: "photo.png".into(),
        mime: "image/png".into(),
        size: 1234,
        key: "cd".repeat(32),
    };
    let fixture = serde_json::json!({
        "format": "nysiris-dm-v2",
        "domains": { "seal": String::from_utf8_lossy(DM_KEY_DOMAIN), "inner": String::from_utf8_lossy(DM_INNER_DOMAIN) },
        "version": DM_INNER_VERSION,
        "senderPriv": sender.secret_hex(),
        "recipientPriv": recipient.secret_hex(),
        "ephemeralPriv": hex::encode(ephemeral_secret),
        "nonceHex": hex::encode(nonce),
        "msgId": msg_id,
        "ts": ts,
        "vectors": [
            vector("plain", &sender, &recipient, ephemeral_secret, nonce, &msg_id, ts, b"hello from nysiris-core", &[]),
            vector("attachments", &sender, &recipient, ephemeral_secret, nonce, &msg_id, ts, b"with an attachment", &[att]),
        ],
    });
    println!("{}", serde_json::to_string_pretty(&fixture).unwrap());
}
