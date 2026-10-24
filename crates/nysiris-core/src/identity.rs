use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand::RngCore;

use crate::crypto::{open_dm, seal_dm, SealedDm};
use crate::error::{CoreError, Result};

/// The chat identity: an ed25519 keypair whose public key is the stable chat
/// id. This is separate from the Nym transport identity.
#[derive(Clone)]
pub struct ChatIdentity {
    signing: SigningKey,
}

impl fmt::Debug for ChatIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatIdentity")
            .field("public_hex", &self.public_hex())
            .finish_non_exhaustive()
    }
}

impl ChatIdentity {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self::from_secret_bytes(bytes)
    }

    pub fn from_secret_bytes(bytes: [u8; 32]) -> Self {
        Self {
            signing: SigningKey::from_bytes(&bytes),
        }
    }

    pub fn from_secret_hex(value: &str) -> Result<Self> {
        let raw = hex::decode(value.trim())
            .map_err(|_| CoreError::InvalidKey("secret key is not hex".into()))?;
        if raw.len() != 32 {
            return Err(CoreError::InvalidKey(format!(
                "secret key must be 32 bytes, got {}",
                raw.len()
            )));
        }
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&raw);
        Ok(Self::from_secret_bytes(bytes))
    }

    /// Secret serialization. Callers must encrypt it at rest.
    pub fn secret_bytes(&self) -> [u8; 32] {
        self.signing.to_bytes()
    }

    pub fn secret_hex(&self) -> String {
        hex::encode(self.secret_bytes())
    }

    pub fn public_bytes(&self) -> [u8; 32] {
        self.signing.verifying_key().to_bytes()
    }

    pub fn public_hex(&self) -> String {
        hex::encode(self.public_bytes())
    }

    /// Curve25519 scalar bytes for X25519 DH, derived from the ed25519 secret.
    pub(crate) fn scalar_bytes(&self) -> [u8; 32] {
        self.signing.to_scalar_bytes()
    }

    pub fn sign_hex(&self, message: &[u8]) -> String {
        hex::encode(self.signing.sign(message).to_bytes())
    }

    pub fn verify_hex(public_hex: &str, message: &[u8], signature_hex: &str) -> Result<()> {
        let key_bytes = crate::ids::pubkey_bytes(public_hex)?;
        let key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| CoreError::InvalidKey(format!("bad ed25519 point: {e}")))?;
        let sig_bytes = hex::decode(signature_hex.trim())
            .map_err(|_| CoreError::Crypto("signature is not hex".into()))?;
        if sig_bytes.len() != 64 {
            return Err(CoreError::Crypto(format!(
                "signature must be 64 bytes, got {}",
                sig_bytes.len()
            )));
        }
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&sig_bytes);
        key.verify_strict(message, &Signature::from_bytes(&sig))
            .map_err(|e| CoreError::Crypto(format!("bad signature: {e}")))
    }

    pub fn seal_to(
        &self,
        recipient_pub_hex: &str,
        aad: &[u8],
        plaintext: &[u8],
    ) -> Result<SealedDm> {
        seal_dm(recipient_pub_hex, aad, plaintext)
    }

    pub fn open_sealed(&self, sealed: &SealedDm, aad: &[u8]) -> Result<Vec<u8>> {
        open_dm(self, sealed, aad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_round_trips_and_signs() {
        let id = ChatIdentity::from_secret_bytes([9u8; 32]);
        let restored = ChatIdentity::from_secret_hex(&id.secret_hex()).unwrap();
        assert_eq!(id.public_hex(), restored.public_hex());
        let sig = id.sign_hex(b"hello");
        ChatIdentity::verify_hex(&id.public_hex(), b"hello", &sig).unwrap();
        assert!(ChatIdentity::verify_hex(&id.public_hex(), b"hello!", &sig).is_err());
    }
}
