use blake2::{digest::consts::U32, Blake2b, Digest};
use chacha20poly1305::{
    aead::{Aead, Payload},
    Key, KeyInit, XChaCha20Poly1305, XNonce,
};
use ed25519_dalek::VerifyingKey;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::error::{CoreError, Result};
use crate::identity::ChatIdentity;

/// Domain separator for the sealed DM key derivation.
pub const DM_KEY_DOMAIN: &[u8] = b"nysiris-chat-v1/dm";
/// AAD used for the DM sealed box.
pub const DM_AAD: &[u8] = b"nysiris-chat-v1/dm";

type Blake2b256 = Blake2b<U32>;

/// A sealed direct message, as serialized and handed to the transport.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SealedDm {
    /// Envelope version. Currently `1`.
    pub v: u8,
    /// Ephemeral X25519 public key, hex.
    pub epub: String,
    /// XChaCha20-Poly1305 nonce, hex (24 bytes).
    pub nonce: String,
    /// Ciphertext, hex.
    pub ciphertext: String,
}

fn kdf(shared: &[u8; 32], label: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(label);
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

/// Seal plaintext to an ed25519 public key using an ephemeral X25519 key.
///
/// This mirrors the Nysiris browser DM construction: ed25519 public key to
/// Montgomery X25519, ephemeral X25519 DH, BLAKE2b-256 KDF, XChaCha20-Poly1305.
pub fn seal_dm(recipient_pub_hex: &str, aad: &[u8], plaintext: &[u8]) -> Result<SealedDm> {
    let raw_key = crate::ids::pubkey_bytes(recipient_pub_hex)?;
    let verifying = VerifyingKey::from_bytes(&raw_key)
        .map_err(|e| CoreError::InvalidKey(format!("bad ed25519 point: {e}")))?;
    let recipient_mont = verifying.to_montgomery().to_bytes();

    let ephemeral = StaticSecret::random_from_rng(rand::rngs::OsRng);
    let epub = PublicKey::from(&ephemeral);
    let shared = ephemeral.diffie_hellman(&PublicKey::from(recipient_mont));
    let key = kdf(shared.as_bytes(), DM_KEY_DOMAIN);

    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CoreError::Crypto("sealing failed".into()))?;

    Ok(SealedDm {
        v: 1,
        epub: hex::encode(epub.to_bytes()),
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
    })
}

/// Open a sealed DM with the recipient's ed25519 identity.
pub fn open_dm(identity: &ChatIdentity, sealed: &SealedDm, aad: &[u8]) -> Result<Vec<u8>> {
    if sealed.v != 1 {
        return Err(CoreError::Crypto(format!(
            "unsupported sealed DM version {}",
            sealed.v
        )));
    }
    let epub = decode_hex::<32>(&sealed.epub, "ephemeral public key")?;
    let nonce = decode_hex::<24>(&sealed.nonce, "nonce")?;
    let ciphertext = hex::decode(sealed.ciphertext.trim())
        .map_err(|_| CoreError::Crypto("ciphertext is not hex".into()))?;

    let secret = StaticSecret::from(identity.scalar_bytes());
    let shared = secret.diffie_hellman(&PublicKey::from(epub));
    let key = kdf(shared.as_bytes(), DM_KEY_DOMAIN);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad,
            },
        )
        .map_err(|_| CoreError::Crypto("DM could not be opened (wrong key or tampered)".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_dm_round_trips_between_ed25519_keys() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let sealed = seal_dm(&bob.public_hex(), DM_AAD, b"hello bob").unwrap();
        let opened = open_dm(&bob, &sealed, DM_AAD).unwrap();
        assert_eq!(opened, b"hello bob");
        assert!(open_dm(&alice, &sealed, DM_AAD).is_err());
    }

    #[test]
    fn sealed_dm_rejects_tampering_and_wrong_aad() {
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let mut sealed = seal_dm(&bob.public_hex(), DM_AAD, b"hello").unwrap();
        sealed.ciphertext = format!("00{}", sealed.ciphertext);
        assert!(open_dm(&bob, &sealed, DM_AAD).is_err());

        let sealed = seal_dm(&bob.public_hex(), DM_AAD, b"hello").unwrap();
        assert!(open_dm(&bob, &sealed, b"different-aad").is_err());
    }
}
