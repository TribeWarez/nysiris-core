//! Attachment refs carried inside the sealed DM inner envelope.
//!
//! Byte-exact mirror of Nysiris' `crates/social-format/src/attach.rs` and
//! `web/src/domain/attachments.mjs`. The blob itself is stored by the Nysiris
//! provider; only these refs travel inside the sealed box, so attachment keys
//! stay confidential end-to-end.
//!
//! Canonical signature bytes, appended after the rest of the inner envelope:
//!
//! ```text
//! per ref: id(32) || mime_len(1) || mime || name_len(1) || name || size_be64(8) || key(32)
//! (empty when there are no attachments — attachment-free DMs sign as before)
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// Max ciphertext blob size: 256 KiB (nonce + Poly1305 overhead included).
pub const MAX_ATTACHMENT_BYTES: u64 = 262_144;
/// Max attachments carried by one DM.
pub const MAX_ATTACHMENTS_PER_MESSAGE: usize = 3;
/// Max sanitized filename length in chars.
pub const MAX_FILENAME_CHARS: usize = 80;

/// Whitelisted attachment MIME types.
pub const ALLOWED_MIMES: [&str; 7] = [
    "image/jpeg",
    "image/png",
    "image/webp",
    "image/gif",
    "text/plain",
    "text/markdown",
    "application/pdf",
];

/// Attachment metadata as carried in `atts` of a signed DM inner envelope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    /// Content address: SHA-256(ciphertext blob), lowercase hex (64 chars).
    pub id: String,
    /// Sanitized original filename.
    pub name: String,
    /// Whitelisted MIME type.
    pub mime: String,
    /// Plaintext size in bytes.
    pub size: u64,
    /// File key: 32 bytes, lowercase hex (64 chars).
    pub key: String,
}

fn hex32(value: &str, what: &str) -> Result<String> {
    let raw = hex::decode(value.trim())
        .map_err(|_| CoreError::InvalidInput(format!("{what} must be hex")))?;
    if raw.len() != 32 {
        return Err(CoreError::InvalidInput(format!(
            "{what} must be 32 bytes, got {}",
            raw.len()
        )));
    }
    Ok(hex::encode(raw))
}

impl AttachmentRef {
    /// Validate and normalize an attachment ref. Fail-closed: any defect is an
    /// error, never a partially rendered attachment.
    pub fn parse(value: &Self) -> Result<Self> {
        if !ALLOWED_MIMES.contains(&value.mime.as_str()) {
            return Err(CoreError::InvalidInput(format!(
                "attachment MIME not allowed: {}",
                value.mime
            )));
        }
        if value.name.is_empty()
            || value.name.chars().count() > MAX_FILENAME_CHARS
            || value.name.len() > 255
            || value.name.contains(['/', '\\', '\0'])
            || value.name.chars().any(char::is_control)
        {
            return Err(CoreError::InvalidInput(
                "attachment name is not a sanitized filename".into(),
            ));
        }
        if value.size > MAX_ATTACHMENT_BYTES {
            return Err(CoreError::InvalidInput(format!(
                "attachment too large (max {MAX_ATTACHMENT_BYTES} bytes)"
            )));
        }
        Ok(Self {
            id: hex32(&value.id, "attachment id")?,
            key: hex32(&value.key, "attachment key")?,
            name: value.name.clone(),
            mime: value.mime.clone(),
            size: value.size,
        })
    }
}

/// Validate and normalize an optional attachment list (absent means none).
pub fn parse_attachments(atts: &[AttachmentRef]) -> Result<Vec<AttachmentRef>> {
    if atts.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(CoreError::InvalidInput(format!(
            "at most {MAX_ATTACHMENTS_PER_MESSAGE} attachments per DM"
        )));
    }
    atts.iter().map(AttachmentRef::parse).collect()
}

/// Canonical bytes of an attachment list for signature binding.
pub fn canonical_attachments(atts: &[AttachmentRef]) -> Vec<u8> {
    let mut out = Vec::new();
    for a in atts {
        let id = hex::decode(a.id.trim()).unwrap_or_default();
        let key = hex::decode(a.key.trim()).unwrap_or_default();
        out.extend_from_slice(&id);
        out.push(a.mime.len() as u8);
        out.extend_from_slice(a.mime.as_bytes());
        out.push(a.name.len() as u8);
        out.extend_from_slice(a.name.as_bytes());
        out.extend_from_slice(&a.size.to_be_bytes());
        out.extend_from_slice(&key);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> AttachmentRef {
        AttachmentRef {
            id: "ab".repeat(32),
            name: "photo.png".into(),
            mime: "image/png".into(),
            size: 1234,
            key: "cd".repeat(32),
        }
    }

    #[test]
    fn canonical_is_empty_and_deterministic() {
        assert!(canonical_attachments(&[]).is_empty());
        assert_eq!(
            canonical_attachments(&[sample()]),
            canonical_attachments(&[sample()])
        );
        let mut changed = sample();
        changed.size += 1;
        assert_ne!(
            canonical_attachments(&[sample()]),
            canonical_attachments(&[changed])
        );
    }

    #[test]
    fn rejects_bad_refs() {
        let mut bad = sample();
        bad.mime = "application/octet-stream".into();
        assert!(AttachmentRef::parse(&bad).is_err());
        let mut bad = sample();
        bad.name = "../evil".into();
        assert!(AttachmentRef::parse(&bad).is_err());
        let mut bad = sample();
        bad.id = "zz".into();
        assert!(AttachmentRef::parse(&bad).is_err());
        let mut bad = sample();
        bad.size = MAX_ATTACHMENT_BYTES + 1;
        assert!(AttachmentRef::parse(&bad).is_err());
        let too_many: Vec<_> = (0..4).map(|_| sample()).collect();
        assert!(parse_attachments(&too_many).is_err());
    }
}
