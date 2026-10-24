use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};
use crate::ids::{dm_conversation_id, normalize_pubkey_hex, now_ms};

/// A locally saved peer. Contacts are local metadata; there is no directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contact {
    pub petname: String,
    /// ed25519 chat public key, lowercase hex.
    pub pubkey: String,
    /// Nym transport address: `<identity>.<encryption>@<gateway>`.
    pub nym_address: String,
    pub created_at: u64,
}

impl Contact {
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
        if nym_address.is_empty() {
            return Err(CoreError::InvalidInput("Nym address is required".into()));
        }
        if nym_address.chars().count() > 256 {
            return Err(CoreError::InvalidInput("Nym address is too long".into()));
        }
        Ok(Self {
            petname: petname.to_string(),
            pubkey,
            nym_address: nym_address.to_string(),
            created_at: now_ms(),
        })
    }

    pub fn conversation_id(&self) -> String {
        dm_conversation_id(&self.pubkey)
    }
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
    }
}
