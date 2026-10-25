use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::ids::{dm_conversation_id, normalize_pubkey_hex, now_ms};

/// A locally saved peer. Contacts are local metadata; there is no directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    /// Local display name for this peer. Never sent on the wire.
    pub petname: String,
    /// ed25519 chat public key, lowercase hex.
    pub pubkey: String,
    /// Nym transport address: `<identity>.<encryption>@<gateway>`.
    pub nym_address: String,
    /// Unix milliseconds when the contact was saved.
    pub created_at: u64,
}

impl Contact {
    /// Validate and normalize a contact.
    ///
    /// The petname is trimmed and limited to 64 characters, the public key is
    /// lowercased and length-checked, and the Nym address is trimmed and
    /// limited to 256 characters.
    pub fn new(petname: &str, pubkey: &str, nym_address: &str) -> Result<Self> {
        let petname = petname.trim();
        if petname.is_empty() {
            return Err(CoreError::InvalidInput("petname is required".into()));
        }
        if petname.chars().count() > 64 {
            return Err(CoreError::InvalidInput(
                "petname must be 64 characters or fewer".into(),
            ));
        }
        let pubkey = normalize_pubkey_hex(pubkey)?;
        let nym_address = nym_address.trim();
        validate_nym_address(nym_address)?;
        Ok(Self {
            petname: petname.to_string(),
            pubkey,
            nym_address: nym_address.to_string(),
            created_at: now_ms(),
        })
    }

    /// Conversation id for the DM with this contact.
    pub fn conversation_id(&self) -> String {
        dm_conversation_id(&self.pubkey)
    }
}

/// Validate the `<identity>.<encryption>@<gateway>` shape of a Nym address.
///
/// This is a syntax check, not a network lookup. It accepts base58-style
/// identity/encryption keys and a non-empty gateway label, including the
/// `mock:`/`@mock` addresses used by the in-memory transport.
pub fn validate_nym_address(value: &str) -> Result<()> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CoreError::InvalidInput("Nym address is required".into()));
    }
    if value.chars().count() > 256 {
        return Err(CoreError::InvalidInput("Nym address is too long".into()));
    }
    if value.starts_with("mock:") && value.len() > 5 {
        return Ok(());
    }
    let (local, gateway) = value
        .split_once('@')
        .ok_or_else(|| CoreError::InvalidInput("Nym address needs a gateway".into()))?;
    if gateway.is_empty() || gateway.contains('@') {
        return Err(CoreError::InvalidInput(
            "Nym address has an invalid gateway".into(),
        ));
    }
    let (identity, encryption) = local
        .split_once('.')
        .ok_or_else(|| CoreError::InvalidInput("Nym address needs identity.encryption".into()))?;
    if identity.is_empty() || encryption.is_empty() || encryption.contains('.') {
        return Err(CoreError::InvalidInput(
            "Nym address has an invalid local part".into(),
        ));
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '@' | ':' | '_' | '-'))
    {
        return Err(CoreError::InvalidInput(
            "Nym address contains invalid characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contacts_are_normalized_and_validated() {
        let c = Contact::new(" Alice ", &"AB".repeat(32), " id.enc@gw ").unwrap();
        assert_eq!(c.petname, "Alice");
        assert_eq!(c.pubkey, "ab".repeat(32));
        assert_eq!(c.nym_address, "id.enc@gw");
        assert!(Contact::new("", &"ab".repeat(32), "id.enc@gw").is_err());
        assert!(Contact::new("Alice", "zz", "id.enc@gw").is_err());
        assert!(Contact::new("Alice", &"ab".repeat(32), "id@gw").is_err());
        assert!(Contact::new("Alice", &"ab".repeat(32), "id.enc").is_err());
        assert!(Contact::new("Alice", &"ab".repeat(32), "id.enc@gw extra").is_err());
        assert!(Contact::new("Alice", &"ab".repeat(32), "id..enc@gw").is_err());
        assert!(Contact::new("Alice", &"ab".repeat(32), "mock:abcdef").is_ok());
        assert!(Contact::new("Alice", &"ab".repeat(32), "id.enc@gw").is_ok());
    }
}
