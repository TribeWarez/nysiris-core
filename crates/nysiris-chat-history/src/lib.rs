//! Encrypted, append-only local chat history.
//!
//! One journal per conversation, named by a BLAKE2b hash of the conversation
//! id. Each record is a length-prefixed sealed blob; the journal filename is
//! bound into the AEAD associated data so records cannot be moved between
//! conversations. No SQLite and no plaintext message bytes on disk.

use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use blake2::{digest::consts::U32, Blake2b, Digest};
use nysiris_core::error::{CoreError, Result};
use nysiris_core::message::ChatMessage;
use nysiris_core::sealed_file::{open_bytes, seal_bytes};

type Blake2b256 = Blake2b<U32>;

const MAX_RECORD_BYTES: usize = 1_000_000;

#[derive(Debug)]
pub struct History {
    dir: PathBuf,
    key: [u8; 32],
    messages: Vec<ChatMessage>,
    ids: HashSet<String>,
}

impl History {
    pub fn open(dir: impl AsRef<Path>, key: [u8; 32]) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let mut messages = Vec::new();
        let mut ids = HashSet::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("log") {
                continue;
            }
            let loaded = load_file(&path, &key)?;
            for message in loaded {
                if ids.insert(message.msg_id.clone()) {
                    messages.push(message);
                }
            }
        }
        messages.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.msg_id.cmp(&b.msg_id)));
        Ok(Self {
            dir,
            key,
            messages,
            ids,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn all(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn count(&self) -> usize {
        self.messages.len()
    }

    pub fn contains(&self, msg_id: &str) -> bool {
        self.ids.contains(&msg_id.to_lowercase())
    }

    /// Append a message. Returns `false` when the id is already present.
    pub fn append(&mut self, message: &ChatMessage) -> Result<bool> {
        let msg_id = message.msg_id.to_lowercase();
        if self.ids.contains(&msg_id) {
            return Ok(false);
        }
        let name = file_name(&message.conversation_id);
        let path = self.dir.join(&name);
        append_record(&path, &self.key, name.as_bytes(), message)?;
        self.ids.insert(msg_id);
        self.messages.push(message.clone());
        self.messages
            .sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.msg_id.cmp(&b.msg_id)));
        Ok(true)
    }

    /// Newest-last messages for one conversation.
    pub fn list(&self, conversation_id: &str, limit: usize) -> Vec<ChatMessage> {
        let mut rows: Vec<_> = self
            .messages
            .iter()
            .filter(|m| m.conversation_id == conversation_id)
            .cloned()
            .collect();
        if limit > 0 && rows.len() > limit {
            rows = rows.split_off(rows.len() - limit);
        }
        rows
    }

    pub fn clear_all(&mut self) -> Result<()> {
        for entry in std::fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) == Some("log") {
                std::fs::remove_file(path)?;
            }
        }
        self.messages.clear();
        self.ids.clear();
        Ok(())
    }

    pub fn clear_conversation(&mut self, conversation_id: &str) -> Result<()> {
        let path = self.dir.join(file_name(conversation_id));
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        self.messages
            .retain(|m| m.conversation_id != conversation_id);
        self.ids = self.messages.iter().map(|m| m.msg_id.clone()).collect();
        Ok(())
    }
}

fn file_name(conversation_id: &str) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update(b"nysiris-chat-history/v1/");
    hasher.update(conversation_id.as_bytes());
    let digest = hasher.finalize();
    format!("{}.log", hex::encode(&digest[..16]))
}

/// Early development builds used an 8-byte hash. Accept those journals so a
/// local history survives the widening to 128 bits.
fn legacy_file_name(conversation_id: &str) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update(b"nysiris-chat-history/v1/");
    hasher.update(conversation_id.as_bytes());
    let digest = hasher.finalize();
    format!("{}.log", hex::encode(&digest[..8]))
}

fn append_record(path: &Path, key: &[u8; 32], aad: &[u8], message: &ChatMessage) -> Result<()> {
    let record = serde_json::to_vec(message)?;
    if record.len() > MAX_RECORD_BYTES {
        return Err(CoreError::History("record is too large".into()));
    }
    let sealed = seal_bytes(key, aad, &record)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
    }
    file.write_all(&(sealed.len() as u32).to_be_bytes())?;
    file.write_all(&sealed)?;
    file.flush()?;
    file.sync_data()?;
    Ok(())
}

fn load_file(path: &Path, key: &[u8; 32]) -> Result<Vec<ChatMessage>> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| CoreError::History("history file has no name".into()))?;
    let aad = name.as_bytes();
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut bytes)?;

    let mut offset = 0usize;
    let mut out = Vec::new();
    while offset < bytes.len() {
        if bytes.len() - offset < 4 {
            return Err(CoreError::History(format!(
                "truncated record header in {name}"
            )));
        }
        let len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;
        if len == 0 || len > MAX_RECORD_BYTES || bytes.len() - offset < len {
            return Err(CoreError::History(format!("bad record length in {name}")));
        }
        let plain = open_bytes(key, aad, &bytes[offset..offset + len])?;
        offset += len;
        let message: ChatMessage = serde_json::from_slice(&plain)?;
        if file_name(&message.conversation_id) != name
            && legacy_file_name(&message.conversation_id) != name
        {
            return Err(CoreError::History(format!(
                "record belongs to another conversation in {name}"
            )));
        }
        out.push(message);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nysiris_core::message::{ChatMessage, Direction, MessageState};
    use tempfile::TempDir;

    fn message(id: &str, text: &str) -> ChatMessage {
        ChatMessage {
            msg_id: id.to_string(),
            conversation_id: "dm:aabb".into(),
            from: "aa".repeat(32),
            to: "bb".repeat(32),
            body: text.into(),
            at: 1,
            direction: Direction::Incoming,
            state: MessageState::Sent,
        }
    }

    #[test]
    fn history_persists_encrypted_and_dedupes() {
        let dir = TempDir::new().unwrap();
        let key = [7u8; 32];
        let mut history = History::open(dir.path(), key).unwrap();
        assert!(history.append(&message("01", "hello")).unwrap());
        assert!(!history.append(&message("01", "hello")).unwrap());
        assert!(history.append(&message("02", "world")).unwrap());

        let raw = std::fs::read(history.dir().join(file_name("dm:aabb"))).unwrap();
        assert!(!raw.windows(5).any(|w| w == b"hello"));
        assert!(!raw.windows(5).any(|w| w == b"world"));

        let reopened = History::open(dir.path(), key).unwrap();
        let rows = reopened.list("dm:aabb", 0);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].body, "world");
    }

    #[test]
    fn wrong_key_fails_closed() {
        let dir = TempDir::new().unwrap();
        let mut history = History::open(dir.path(), [7u8; 32]).unwrap();
        history.append(&message("01", "hello")).unwrap();
        assert!(History::open(dir.path(), [8u8; 32]).is_err());
    }
}
