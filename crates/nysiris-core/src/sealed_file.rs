use std::path::Path;

use chacha20poly1305::{
    aead::{Aead, Payload},
    Key, KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;

use crate::error::{CoreError, Result};

const MAGIC: &[u8; 5] = b"NYSF1";

/// Seal arbitrary bytes into a self-contained blob: magic + nonce + ciphertext.
pub fn seal_bytes(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| CoreError::Crypto("sealed-file encryption failed".into()))?;

    let mut out = Vec::with_capacity(MAGIC.len() + nonce.len() + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Open a sealed blob. Fails closed on a bad magic, short input, wrong key, or
/// mismatched associated data.
pub fn open_bytes(key: &[u8; 32], aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < MAGIC.len() + 24 {
        return Err(CoreError::Crypto("sealed file is too short".into()));
    }
    if &sealed[..MAGIC.len()] != MAGIC {
        return Err(CoreError::Crypto("sealed file has a bad magic".into()));
    }
    let nonce_start = MAGIC.len();
    let ct_start = nonce_start + 24;
    let nonce = &sealed[nonce_start..ct_start];
    let ciphertext = &sealed[ct_start..];
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| CoreError::Crypto("sealed file could not be opened".into()))
}

/// Write a sealed blob. This is intentionally a simple overwrite for phase 0;
/// replace with an atomic rename + fsync before production.
pub fn write_sealed(path: &Path, key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let sealed = seal_bytes(key, aad, plaintext)?;
    std::fs::write(path, sealed)?;
    restrict_permissions(path);
    Ok(())
}

fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Read and open a sealed blob written by [`write_sealed`].
pub fn read_sealed(path: &Path, key: &[u8; 32], aad: &[u8]) -> Result<Vec<u8>> {
    let sealed = std::fs::read(path)?;
    open_bytes(key, aad, &sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sealed_bytes_round_trip_and_reject_wrong_aad() {
        let key = [7u8; 32];
        let sealed = seal_bytes(&key, b"label", b"hello").unwrap();
        assert!(!sealed.windows(5).any(|w| w == b"hello"));
        assert_eq!(open_bytes(&key, b"label", &sealed).unwrap(), b"hello");
        assert!(open_bytes(&key, b"other", &sealed).is_err());
    }
}
