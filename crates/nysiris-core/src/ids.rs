use rand::RngCore;

use crate::error::{CoreError, Result};

/// Milliseconds since the Unix epoch. Local ordering only; never trusted as a
/// network timestamp.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Cryptographically random bytes, hex-encoded.
pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Normalize and validate a 32-byte public key (64 hex chars).
pub fn normalize_pubkey_hex(value: &str) -> Result<String> {
    let clean = value.trim().to_lowercase();
    let raw =
        hex::decode(&clean).map_err(|_| CoreError::InvalidKey("public key is not hex".into()))?;
    if raw.len() != 32 {
        return Err(CoreError::InvalidKey(format!(
            "public key must be 32 bytes, got {}",
            raw.len()
        )));
    }
    Ok(clean)
}

/// Decode a normalized 32-byte hex key.
pub fn pubkey_bytes(value: &str) -> Result<[u8; 32]> {
    let clean = normalize_pubkey_hex(value)?;
    let mut out = [0u8; 32];
    out.copy_from_slice(
        &hex::decode(clean).map_err(|_| CoreError::InvalidKey("public key is not hex".into()))?,
    );
    Ok(out)
}

/// Decode a 16-byte message id (32 hex chars).
pub fn msg_id_bytes(value: &str) -> Result<[u8; 16]> {
    let clean = value.trim().to_lowercase();
    let raw =
        hex::decode(&clean).map_err(|_| CoreError::InvalidInput("message id is not hex".into()))?;
    if raw.len() != 16 {
        return Err(CoreError::InvalidInput(format!(
            "message id must be 16 bytes, got {}",
            raw.len()
        )));
    }
    let mut out = [0u8; 16];
    out.copy_from_slice(&raw);
    Ok(out)
}

/// Conversation id for a direct chat. The peer public key is the stable id.
pub fn dm_conversation_id(peer_pub_hex: &str) -> String {
    format!("dm:{}", peer_pub_hex.trim().to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pubkeys_and_message_ids_are_validated() {
        assert!(normalize_pubkey_hex(&"AB".repeat(32)).is_ok());
        assert!(normalize_pubkey_hex("ab").is_err());
        assert!(msg_id_bytes(&"cd".repeat(16)).is_ok());
        assert!(msg_id_bytes("cd").is_err());
    }

    #[test]
    fn conversation_ids_are_stable() {
        assert_eq!(dm_conversation_id("AABB"), "dm:aabb");
    }
}
