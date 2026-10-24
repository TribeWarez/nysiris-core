use blake2::{digest::consts::U32, Blake2b, Digest};
use chacha20poly1305::{
    aead::{Aead, Payload},
    Key, KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::contact::Contact;
use crate::error::{CoreError, Result};
use crate::identity::ChatIdentity;
use crate::ids::{msg_id_bytes, normalize_pubkey_hex, now_ms, random_hex};

pub const GROUP_ID_BYTES: usize = 16;
pub const GROUP_KEY_BYTES: usize = 32;
pub const MAX_GROUP_MEMBERS: usize = 16;
pub const MAX_GROUP_NAME_CHARS: usize = 80;
pub const GROUP_MESSAGE_DOMAIN: &[u8] = b"nysiris-chat-v1/group-message";
pub const GROUP_INVITE_DOMAIN: &[u8] = b"nysiris-chat-v1/group-invite";
pub const GROUP_ENC_DOMAIN: &[u8] = b"nysiris-chat-v1/group-enc";
pub const GROUP_INNER_VERSION: u8 = 1;

type Blake2b256 = Blake2b<U32>;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMember {
    pub petname: String,
    pub pubkey: String,
    pub nym_address: String,
}

impl GroupMember {
    pub fn new(petname: &str, pubkey: &str, nym_address: &str) -> Result<Self> {
        let petname = petname.trim();
        if petname.is_empty() || petname.chars().count() > 64 {
            return Err(CoreError::InvalidInput(
                "member petname must be 1..=64 characters".into(),
            ));
        }
        let nym_address = nym_address.trim();
        if nym_address.is_empty() || nym_address.chars().count() > 256 {
            return Err(CoreError::InvalidInput(
                "member Nym address is invalid".into(),
            ));
        }
        Ok(Self {
            petname: petname.to_string(),
            pubkey: normalize_pubkey_hex(pubkey)?,
            nym_address: nym_address.to_string(),
        })
    }

    pub fn from_contact(contact: &Contact) -> Result<Self> {
        Self::new(&contact.petname, &contact.pubkey, &contact.nym_address)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub key: String,
    pub members: Vec<GroupMember>,
    pub created_at: u64,
}

impl Group {
    /// Create a local room including `own` and at least one peer.
    pub fn create(name: &str, mut members: Vec<GroupMember>, own: GroupMember) -> Result<Self> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > MAX_GROUP_NAME_CHARS {
            return Err(CoreError::InvalidInput(
                "room name must be 1..=80 characters".into(),
            ));
        }
        members.retain(|m| m.pubkey != own.pubkey);
        members.push(own);
        dedupe_members(&mut members)?;
        if members.len() < 2 {
            return Err(CoreError::InvalidInput(
                "a room needs at least one peer besides you".into(),
            ));
        }
        if members.len() > MAX_GROUP_MEMBERS {
            return Err(CoreError::InvalidInput(format!(
                "a room is limited to {MAX_GROUP_MEMBERS} members"
            )));
        }
        let mut key_bytes = [0u8; GROUP_KEY_BYTES];
        rand::rngs::OsRng.fill_bytes(&mut key_bytes);
        Ok(Self {
            id: random_hex(GROUP_ID_BYTES),
            name: name.to_string(),
            key: hex::encode(key_bytes),
            members,
            created_at: now_ms(),
        })
    }

    pub fn conversation_id(&self) -> String {
        format!("group:{}", self.id)
    }

    pub fn member(&self, pubkey: &str) -> Option<&GroupMember> {
        let key = pubkey.trim().to_lowercase();
        self.members.iter().find(|m| m.pubkey == key)
    }

    pub fn validate(&self) -> Result<()> {
        let id = hex::decode(&self.id)
            .map_err(|_| CoreError::InvalidInput("room id is not hex".into()))?;
        if id.len() != GROUP_ID_BYTES {
            return Err(CoreError::InvalidInput("room id must be 16 bytes".into()));
        }
        let key = hex::decode(&self.key)
            .map_err(|_| CoreError::InvalidInput("room key is not hex".into()))?;
        if key.len() != GROUP_KEY_BYTES {
            return Err(CoreError::InvalidInput("room key must be 32 bytes".into()));
        }
        if self.name.trim().is_empty() || self.name.chars().count() > MAX_GROUP_NAME_CHARS {
            return Err(CoreError::InvalidInput("room name is invalid".into()));
        }
        if self.members.len() < 2 || self.members.len() > MAX_GROUP_MEMBERS {
            return Err(CoreError::InvalidInput(
                "room member count is invalid".into(),
            ));
        }
        let mut seen = std::collections::HashSet::new();
        for member in &self.members {
            GroupMember::new(&member.petname, &member.pubkey, &member.nym_address)?;
            if !seen.insert(member.pubkey.clone()) {
                return Err(CoreError::InvalidInput("duplicate room member".into()));
            }
        }
        Ok(())
    }

    pub fn invite(&self, inviter: &ChatIdentity) -> Result<String> {
        self.validate()?;
        let inviter_pub = inviter.public_hex();
        let inviter_member = self
            .member(&inviter_pub)
            .cloned()
            .ok_or_else(|| CoreError::InvalidInput("inviter is not a room member".into()))?;
        let mut members = sorted_members(&self.members);
        // Keep the unsigned form deterministic for byte-exact verification.
        members.sort_by(|a, b| a.pubkey.cmp(&b.pubkey));
        let unsigned = GroupInviteUnsigned {
            v: GROUP_INNER_VERSION,
            group_id: self.id.clone(),
            name: self.name.clone(),
            key: self.key.clone(),
            members,
            inviter: inviter_member,
            created_at: now_ms(),
        };
        let bytes = invite_sign_bytes(&unsigned)?;
        let invite = GroupInvite {
            unsigned,
            sig: inviter.sign_hex(&bytes),
        };
        Ok(serde_json::to_string(&invite)?)
    }

    pub fn from_invite(json: &str, own_pubkey: &str) -> Result<Self> {
        let invite: GroupInvite = serde_json::from_str(json)?;
        invite.verify()?;
        let own = normalize_pubkey_hex(own_pubkey)?;
        if !invite.unsigned.members.iter().any(|m| m.pubkey == own) {
            return Err(CoreError::InvalidInput(
                "this identity is not a member of the invited room".into(),
            ));
        }
        if invite.unsigned.members.len() < 2 || invite.unsigned.members.len() > MAX_GROUP_MEMBERS {
            return Err(CoreError::InvalidInput(
                "invite member count is invalid".into(),
            ));
        }
        let group = Self {
            id: invite.unsigned.group_id,
            name: invite.unsigned.name,
            key: invite.unsigned.key,
            members: invite.unsigned.members,
            created_at: invite.unsigned.created_at,
        };
        group.validate()?;
        Ok(group)
    }
}

fn dedupe_members(members: &mut Vec<GroupMember>) -> Result<()> {
    let mut seen = std::collections::HashSet::new();
    members.retain(|m| seen.insert(m.pubkey.clone()));
    for member in members {
        normalize_pubkey_hex(&member.pubkey)?;
    }
    Ok(())
}

fn sorted_members(members: &[GroupMember]) -> Vec<GroupMember> {
    let mut out = members.to_vec();
    out.sort_by(|a, b| a.pubkey.cmp(&b.pubkey));
    out
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GroupInviteUnsigned {
    v: u8,
    group_id: String,
    name: String,
    key: String,
    members: Vec<GroupMember>,
    inviter: GroupMember,
    created_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct GroupInvite {
    #[serde(flatten)]
    unsigned: GroupInviteUnsigned,
    sig: String,
}

impl GroupInvite {
    fn verify(&self) -> Result<()> {
        if self.unsigned.v != GROUP_INNER_VERSION {
            return Err(CoreError::InvalidInput(
                "unsupported room invite version".into(),
            ));
        }
        let bytes = invite_sign_bytes(&self.unsigned)?;
        ChatIdentity::verify_hex(&self.unsigned.inviter.pubkey, &bytes, &self.sig)?;
        if !self
            .unsigned
            .members
            .iter()
            .any(|m| m.pubkey == self.unsigned.inviter.pubkey)
        {
            return Err(CoreError::InvalidInput(
                "invite signer is not in the member list".into(),
            ));
        }
        Ok(())
    }
}

fn invite_sign_bytes(unsigned: &GroupInviteUnsigned) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(unsigned)?;
    let mut out = Vec::with_capacity(GROUP_INVITE_DOMAIN.len() + body.len());
    out.extend_from_slice(GROUP_INVITE_DOMAIN);
    out.extend_from_slice(&body);
    Ok(out)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupInner {
    pub v: u8,
    pub group_id: String,
    pub from: String,
    pub ts: u64,
    #[serde(rename = "msgId")]
    pub msg_id: String,
    pub body: String,
    pub sig: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealedGroupMessage {
    pub v: u8,
    pub gid: String,
    #[serde(rename = "msgId")]
    pub msg_id: String,
    pub nonce: String,
    pub ciphertext: String,
}

fn group_key_bytes(key_hex: &str) -> Result<[u8; 32]> {
    let raw = hex::decode(key_hex.trim())
        .map_err(|_| CoreError::InvalidKey("room key is not hex".into()))?;
    if raw.len() != 32 {
        return Err(CoreError::InvalidKey("room key must be 32 bytes".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}

fn group_enc_key(key_hex: &str) -> Result<[u8; 32]> {
    let key = group_key_bytes(key_hex)?;
    let mut hasher = Blake2b256::new();
    hasher.update(GROUP_ENC_DOMAIN);
    hasher.update(key);
    Ok(hasher.finalize().into())
}

fn group_aad(gid_hex: &str, msg_id_hex: &str) -> Result<Vec<u8>> {
    let gid = hex::decode(gid_hex.trim())
        .map_err(|_| CoreError::InvalidInput("room id is not hex".into()))?;
    if gid.len() != GROUP_ID_BYTES {
        return Err(CoreError::InvalidInput("room id must be 16 bytes".into()));
    }
    let msg_id = msg_id_bytes(msg_id_hex)?;
    let mut out = Vec::with_capacity(gid.len() + msg_id.len());
    out.extend_from_slice(&gid);
    out.extend_from_slice(&msg_id);
    Ok(out)
}

pub fn pack_group_inner(
    identity: &ChatIdentity,
    gid: &str,
    msg_id: &str,
    ts: u64,
    body: &str,
) -> Result<String> {
    if body.is_empty() || body.len() > crate::message::MAX_BODY_BYTES {
        return Err(CoreError::InvalidInput(
            "room message body has an invalid size".into(),
        ));
    }
    let gid_bytes = hex::decode(gid.trim())
        .map_err(|_| CoreError::InvalidInput("room id is not hex".into()))?;
    if gid_bytes.len() != GROUP_ID_BYTES {
        return Err(CoreError::InvalidInput("room id must be 16 bytes".into()));
    }
    let msg_id_bytes = msg_id_bytes(msg_id)?;
    let from = identity.public_hex();
    let mut bytes = Vec::with_capacity(GROUP_MESSAGE_DOMAIN.len() + 16 + 32 + 8 + 16 + body.len());
    bytes.extend_from_slice(GROUP_MESSAGE_DOMAIN);
    bytes.extend_from_slice(&gid_bytes);
    bytes.extend_from_slice(&crate::ids::pubkey_bytes(&from)?);
    bytes.extend_from_slice(&ts.to_be_bytes());
    bytes.extend_from_slice(&msg_id_bytes);
    bytes.extend_from_slice(body.as_bytes());
    let inner = GroupInner {
        v: GROUP_INNER_VERSION,
        group_id: gid.to_lowercase(),
        from,
        ts,
        msg_id: msg_id.to_lowercase(),
        body: body.to_string(),
        sig: identity.sign_hex(&bytes),
    };
    Ok(serde_json::to_string(&inner)?)
}

pub fn unpack_group_inner(json: &str, expected_gid: &str) -> Result<GroupInner> {
    let inner: GroupInner = serde_json::from_str(json)?;
    if inner.v != GROUP_INNER_VERSION {
        return Err(CoreError::InvalidInput(
            "unsupported room inner version".into(),
        ));
    }
    if inner.group_id != expected_gid.to_lowercase() {
        return Err(CoreError::InvalidInput(
            "room inner is for another room".into(),
        ));
    }
    let gid = hex::decode(&inner.group_id)
        .map_err(|_| CoreError::InvalidInput("room id is not hex".into()))?;
    if gid.len() != GROUP_ID_BYTES {
        return Err(CoreError::InvalidInput("room id must be 16 bytes".into()));
    }
    let from = normalize_pubkey_hex(&inner.from)?;
    let msg_id = msg_id_bytes(&inner.msg_id)?;
    if inner.body.is_empty() || inner.body.len() > crate::message::MAX_BODY_BYTES {
        return Err(CoreError::InvalidInput(
            "room body has an invalid size".into(),
        ));
    }
    let mut bytes =
        Vec::with_capacity(GROUP_MESSAGE_DOMAIN.len() + 16 + 32 + 8 + 16 + inner.body.len());
    bytes.extend_from_slice(GROUP_MESSAGE_DOMAIN);
    bytes.extend_from_slice(&gid);
    bytes.extend_from_slice(&crate::ids::pubkey_bytes(&from)?);
    bytes.extend_from_slice(&inner.ts.to_be_bytes());
    bytes.extend_from_slice(&msg_id);
    bytes.extend_from_slice(inner.body.as_bytes());
    ChatIdentity::verify_hex(&from, &bytes, &inner.sig)?;
    Ok(GroupInner { from, ..inner })
}

pub fn seal_group(
    key_hex: &str,
    gid: &str,
    msg_id: &str,
    inner_json: &str,
) -> Result<SealedGroupMessage> {
    let key = group_enc_key(key_hex)?;
    let aad = group_aad(gid, msg_id)?;
    let mut nonce = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: inner_json.as_bytes(),
                aad: &aad,
            },
        )
        .map_err(|_| CoreError::Crypto("room seal failed".into()))?;
    Ok(SealedGroupMessage {
        v: GROUP_INNER_VERSION,
        gid: gid.to_lowercase(),
        msg_id: msg_id.to_lowercase(),
        nonce: hex::encode(nonce),
        ciphertext: hex::encode(ciphertext),
    })
}

pub fn open_group(key_hex: &str, sealed: &SealedGroupMessage) -> Result<String> {
    if sealed.v != GROUP_INNER_VERSION {
        return Err(CoreError::Crypto("unsupported sealed room version".into()));
    }
    let key = group_enc_key(key_hex)?;
    let aad = group_aad(&sealed.gid, &sealed.msg_id)?;
    let nonce = hex::decode(sealed.nonce.trim())
        .map_err(|_| CoreError::Crypto("room nonce is not hex".into()))?;
    if nonce.len() != 24 {
        return Err(CoreError::Crypto("room nonce must be 24 bytes".into()));
    }
    let ciphertext = hex::decode(sealed.ciphertext.trim())
        .map_err(|_| CoreError::Crypto("room ciphertext is not hex".into()))?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let plain = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: &aad,
            },
        )
        .map_err(|_| CoreError::Crypto("room message could not be opened".into()))?;
    String::from_utf8(plain).map_err(|_| CoreError::InvalidInput("room inner is not UTF-8".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(seed: u8, name: &str) -> GroupMember {
        GroupMember::new(
            name,
            &ChatIdentity::from_secret_bytes([seed; 32]).public_hex(),
            &format!("{name}.enc@gw"),
        )
        .unwrap()
    }

    #[test]
    fn group_seal_open_and_signature_round_trip() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let own = member(1, "Alice");
        let group = Group::create("Room", vec![member(2, "Bob")], own).unwrap();
        let inner = pack_group_inner(&alice, &group.id, &"ab".repeat(16), 123, "hi room").unwrap();
        let sealed = seal_group(&group.key, &group.id, &"ab".repeat(16), &inner).unwrap();
        let opened = open_group(&group.key, &sealed).unwrap();
        let verified = unpack_group_inner(&opened, &group.id).unwrap();
        assert_eq!(verified.body, "hi room");
        assert_eq!(verified.from, alice.public_hex());
        assert!(unpack_group_inner(&opened, &"cd".repeat(16)).is_err());
        assert!(open_group(&hex::encode([9u8; 32]), &sealed).is_err());

        let wrong = pack_group_inner(&bob, &group.id, &"ef".repeat(16), 123, "spoof").unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&wrong).unwrap();
        value["body"] = serde_json::json!("tampered");
        assert!(unpack_group_inner(&value.to_string(), &group.id).is_err());
    }

    #[test]
    fn invite_round_trips_and_binds_identity() {
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        let bob = ChatIdentity::from_secret_bytes([2u8; 32]);
        let group = Group::create("Room", vec![member(2, "Bob")], member(1, "Alice")).unwrap();
        let invite = group.invite(&alice).unwrap();
        let joined = Group::from_invite(&invite, &bob.public_hex()).unwrap();
        assert_eq!(joined.id, group.id);
        assert_eq!(joined.key, group.key);
        assert_eq!(joined.members.len(), 2);
        assert!(Group::from_invite(&invite, &"aa".repeat(32)).is_err());
        let carol = ChatIdentity::from_secret_bytes([9u8; 32]);
        assert!(group.invite(&carol).is_err());
    }
}
