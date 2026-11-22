//! Sealed DM encryption, byte-exact with the Nysiris browser client.
//!
//! ```text
//! sender:   eph = random x25519 keypair; epub = X(eph)
//!           shared = X(eph, montgomery(recipient_ed_pub))
//! receiver: shared = X(montgomery(own_ed_priv), epub)
//! key       = BLAKE2b-256("fly-social-v1/dm" || shared)
//! ct        = XChaCha20-Poly1305(key, nonce).encrypt(plaintext)
//! ```
//!
//! The provider sees only `{epubHex, nonceHex, ciphertextB64}`; there is no
//! version field and no AAD. This is sender-forward-secret but not ratcheted:
//! compromise of the recipient's long-term identity key exposes past DMs.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use blake2::{digest::consts::U32, Blake2b, Digest};
use chacha20poly1305::{aead::Aead, Key, KeyInit, XChaCha20Poly1305, XNonce};
use ed25519_dalek::VerifyingKey;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::{CoreError, Result};
use crate::identity::ChatIdentity;

/// Domain separator for the sealed DM key derivation (shared with the browser).
pub const DM_KEY_DOMAIN: &[u8] = b"fly-social-v1/dm";

type Blake2b256 = Blake2b<U32>;

/// A sealed direct message, as serialized and handed to the transport.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SealedDm {
    /// Ephemeral X25519 public key, lowercase hex (32 bytes).
    #[serde(rename = "epubHex")]
    pub epub_hex: String,
    /// XChaCha20-Poly1305 nonce, lowercase hex (24 bytes).
    #[serde(rename = "nonceHex")]
    pub nonce_hex: String,
    /// Ciphertext, standard padded base64 (nonce-free, tag included).
    #[serde(rename = "ciphertextB64")]
    pub ciphertext_b64: String,
}

fn kdf(shared: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(DM_KEY_DOMAIN);
    hasher.update(shared);
    hasher.finalize().into()
}

fn decode_hex<const N: usize>(value: &str, what: &str) -> Result<[u8; N]> {
    let raw =
        hex::decode(value.trim()).map_err(|_| CoreError::Crypto(format!("{what} is not hex")))?;
    if raw.len() != N {
        return Err(CoreError::Crypto(format!(
            "{what} must be {N} bytes, got {}",
            raw.len()
        )));
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&raw);
    Ok(out)
}

fn recipient_montgomery(recipient_pub_hex: &str) -> Result<[u8; 32]> {
    let raw_key = crate::ids::pubkey_bytes(recipient_pub_hex)?;
    let verifying = VerifyingKey::from_bytes(&raw_key)
        .map_err(|e| CoreError::InvalidKey(format!("bad ed25519 point: {e}")))?;
    Ok(verifying.to_montgomery().to_bytes())
}

fn seal_with_key(
    recipient_mont: [u8; 32],
    ephemeral: StaticSecret,
    nonce: [u8; 24],
    plaintext: &[u8],
) -> Result<SealedDm> {
    let epub = PublicKey::from(&ephemeral);
    let shared = ephemeral.diffie_hellman(&PublicKey::from(recipient_mont));
    let key = kdf(shared.as_bytes());
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| CoreError::Crypto("sealing failed".into()))?;
    Ok(SealedDm {
        epub_hex: hex::encode(epub.to_bytes()),
        nonce_hex: hex::encode(nonce),
        ciphertext_b64: BASE64.encode(ciphertext),
    })
}

/// Seal plaintext to an ed25519 public key using a fresh ephemeral X25519 key.
/// This is the exact construction used by `sealDm` in the Nysiris browser.
pub fn seal_dm(recipient_pub_hex: &str, plaintext: &[u8]) -> Result<SealedDm> {
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    seal_dm_with(
        recipient_pub_hex,
        plaintext,
        StaticSecret::random_from_rng(rand::rngs::OsRng),
        nonce,
    )
}

/// Deterministic sealing for shared conformance vectors. The caller supplies
/// the ephemeral X25519 secret and nonce; production code must use `seal_dm`.
pub fn seal_dm_with(
    recipient_pub_hex: &str,
    plaintext: &[u8],
    ephemeral_secret: StaticSecret,
    nonce: [u8; 24],
) -> Result<SealedDm> {
    seal_with_key(
        recipient_montgomery(recipient_pub_hex)?,
        ephemeral_secret,
        nonce,
        plaintext,
    )
}

/// Open a sealed DM with the recipient's ed25519 identity.
pub fn open_dm(identity: &ChatIdentity, sealed: &SealedDm) -> Result<Vec<u8>> {
    let epub = decode_hex::<32>(&sealed.epub_hex, "ephemeral public key")?;
    let nonce = decode_hex::<24>(&sealed.nonce_hex, "nonce")?;
    let ciphertext = BASE64
        .decode(sealed.ciphertext_b64.trim())
        .map_err(|_| CoreError::Crypto("ciphertext is not base64".into()))?;

    let secret = StaticSecret::from(identity.scalar_bytes());
    let shared = secret.diffie_hellman(&PublicKey::from(epub));
    let key = kdf(shared.as_bytes());
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(XNonce::from_slice(&nonce), ciphertext.as_slice())
        .map_err(|_| CoreError::Crypto("DM could not be opened (wrong key or tampered)".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_dm_round_trips_between_ed25519_keys() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let sealed = seal_dm(&bob.public_hex(), b"hello bob").unwrap();
        assert_eq!(open_dm(&bob, &sealed).unwrap(), b"hello bob");
        assert!(open_dm(&alice, &sealed).is_err());
        // The browser wire form carries exactly three keys.
        let json: serde_json::Value = serde_json::to_value(&sealed).unwrap();
        let mut keys: Vec<_> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["ciphertextB64", "epubHex", "nonceHex"]);
    }

    #[test]
    fn sealed_dm_rejects_tampering_and_foreign_keys() {
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let mut sealed = seal_dm(&bob.public_hex(), b"hello").unwrap();
        sealed.ciphertext_b64.push('A');
        assert!(open_dm(&bob, &sealed).is_err());

        let sealed = seal_dm(&bob.public_hex(), b"hello").unwrap();
        let mut legacy = serde_json::to_value(&sealed).unwrap();
        legacy["v"] = serde_json::json!(1);
        assert!(serde_json::from_value::<SealedDm>(legacy).is_err());
    }
}
