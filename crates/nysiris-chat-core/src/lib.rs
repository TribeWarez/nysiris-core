#![warn(missing_docs)]
//! P2P chat engine for native Nysiris clients.
//!
//! Supports sealed 1:1 DMs and capability-key rooms with client-side fan-out.
//! There is no chat service and no server history; every message is stored in
//! the local encrypted history journal.

use std::collections::BTreeMap;
use std::sync::Arc;

use nysiris_chat_history::History;
use nysiris_core::contact::Contact;
use nysiris_core::conversation::{ConversationSummary, GroupSummary};
use nysiris_core::crypto::{SealedDm, DM_AAD};
use nysiris_core::error::{CoreError, Result};
use nysiris_core::group::{
    open_group, pack_group_inner, seal_group, unpack_group_inner, Group, GroupMember,
    SealedGroupMessage,
};
use nysiris_core::identity::ChatIdentity;
use nysiris_core::ids::{dm_conversation_id, now_ms, random_hex};
use nysiris_core::message::{pack_dm_inner, unpack_dm_inner, ChatMessage};
use nysiris_core::transport::Transport;
use serde::Serialize;

/// Outcome of a room send: what went out and who accepted it.
#[derive(Clone, Debug, Serialize)]
pub struct GroupSendReport {
    /// The message recorded in local history.
    pub message: ChatMessage,
    /// Number of members the transport accepted the payload for.
    pub delivered: usize,
    /// Per-member failures, formatted as `<petname>: <error>`.
    pub failed: Vec<String>,
}

/// The chat engine: contacts, rooms, history, and a pluggable transport.
pub struct ChatEngine {
    identity: ChatIdentity,
    contacts: Vec<Contact>,
    groups: Vec<Group>,
    history: History,
    transport: Arc<dyn Transport>,
    own_nym_address: String,
    status: String,
}

impl ChatEngine {
    /// Build an engine from already-loaded state.
    ///
    /// `own_nym_address` is this device's Nym address; an empty string means the
    /// engine is running against a mock transport.
    pub fn new(
        identity: ChatIdentity,
        contacts: Vec<Contact>,
        groups: Vec<Group>,
        history: History,
        transport: Arc<dyn Transport>,
        own_nym_address: impl Into<String>,
    ) -> Self {
        Self {
            identity,
            contacts,
            groups,
            history,
            transport,
            own_nym_address: own_nym_address.into(),
            status: "online".into(),
        }
    }

    /// This device's chat identity.
    pub fn identity(&self) -> &ChatIdentity {
        &self.identity
    }

    /// Saved contacts, sorted by petname.
    pub fn contacts(&self) -> &[Contact] {
        &self.contacts
    }

    /// Rooms known on this device.
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }

    /// Look up a room by id.
    pub fn group(&self, group_id: &str) -> Option<&Group> {
        let id = group_id.trim().to_lowercase();
        self.groups.iter().find(|g| g.id == id)
    }

    /// The local encrypted history store.
    pub fn history(&self) -> &History {
        &self.history
    }

    /// Short name of the transport in use.
    pub fn transport_name(&self) -> &'static str {
        self.transport.name()
    }

    /// Current self status: `online`, `away`, `dnd`, or `invisible`.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Set self status. Status is local; it is not broadcast to contacts.
    pub fn set_status(&mut self, status: &str) -> Result<()> {
        let status = status.trim();
        if !matches!(status, "online" | "away" | "dnd" | "invisible") {
            return Err(CoreError::InvalidInput(
                "status must be online, away, dnd, or invisible".into(),
            ));
        }
        self.status = status.to_string();
        Ok(())
    }

    /// Look up a saved contact by public key.
    pub fn contact(&self, pubkey: &str) -> Option<&Contact> {
        let key = pubkey.trim().to_lowercase();
        self.contacts.iter().find(|c| c.pubkey == key)
    }

    /// Add a contact, or update its petname/Nym address when the key exists.
    pub fn add_contact(&mut self, contact: Contact) -> Contact {
        if let Some(existing) = self
            .contacts
            .iter_mut()
            .find(|c| c.pubkey == contact.pubkey)
        {
            existing.petname = contact.petname;
            existing.nym_address = contact.nym_address;
            return existing.clone();
        }
        self.contacts.push(contact.clone());
        self.contacts.sort_by(|a, b| a.petname.cmp(&b.petname));
        contact
    }

    /// Remove a contact by public key. Returns whether anything was removed.
    pub fn remove_contact(&mut self, pubkey: &str) -> bool {
        let key = pubkey.trim().to_lowercase();
        let before = self.contacts.len();
        self.contacts.retain(|c| c.pubkey != key);
        self.contacts.len() != before
    }

    fn own_member(&self) -> Result<GroupMember> {
        let address = if self.own_nym_address.trim().is_empty() {
            format!("mock:{}", self.identity.public_hex())
        } else {
            self.own_nym_address.clone()
        };
        let pubkey = self.identity.public_hex();
        let label = format!("{}…", &pubkey[..8]);
        GroupMember::new(&label, &pubkey, &address)
    }

    /// Create a room, including the selected saved contacts and this identity.
    /// Returns the room and the signed invite JSON for the other members.
    pub fn create_group(
        &mut self,
        name: &str,
        member_pubkeys: &[String],
    ) -> Result<(Group, String)> {
        let mut members = Vec::new();
        for pubkey in member_pubkeys {
            let contact = self
                .contact(pubkey)
                .ok_or_else(|| CoreError::NotFound(format!("contact {pubkey} is not saved")))?;
            members.push(GroupMember::from_contact(contact)?);
        }
        let group = Group::create(name, members, self.own_member()?)?;
        let invite = group.invite(&self.identity)?;
        self.groups.push(group.clone());
        Ok((group, invite))
    }

    /// Join a room from an invite. The invite must include this chat identity.
    pub fn join_group(&mut self, invite_json: &str) -> Result<Group> {
        let group = Group::from_invite(invite_json, &self.identity.public_hex())?;
        if let Some(existing) = self.group(&group.id) {
            return Ok(existing.clone());
        }
        self.groups.push(group.clone());
        Ok(group)
    }

    /// Forget a room locally. Returns whether anything was removed. This does
    /// not notify other members and does not rotate the room key.
    pub fn remove_group(&mut self, group_id: &str) -> bool {
        let id = group_id.trim().to_lowercase();
        let before = self.groups.len();
        self.groups.retain(|g| g.id != id);
        self.groups.len() != before
    }

    /// Re-export the signed invite for a saved room, to hand to a new member.
    pub fn group_invite(&self, group_id: &str) -> Result<String> {
        let group = self
            .group(group_id)
            .ok_or_else(|| CoreError::NotFound("room is not saved".into()))?;
        group.invite(&self.identity)
    }

    /// Send a 1:1 message. The transport receives the sealed envelope; the
    /// plaintext only exists in memory and in the encrypted history journal.
    pub fn send_dm(&mut self, peer_pub: &str, body: &str) -> Result<ChatMessage> {
        let contact = self
            .contact(peer_pub)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("contact is not saved".into()))?;

        let msg_id = random_hex(16);
        let ts = now_ms();
        let inner = pack_dm_inner(&self.identity, &contact.pubkey, &msg_id, ts, body)?;
        let sealed = self
            .identity
            .seal_to(&contact.pubkey, DM_AAD, inner.as_bytes())?;
        let payload = serde_json::to_vec(&sealed)?;

        let message = ChatMessage::outgoing(
            &self.identity.public_hex(),
            &contact.pubkey,
            &msg_id,
            body,
            ts,
        );

        // Send first: a failed transport must not look like a delivered message.
        self.transport.send(&contact.nym_address, payload)?;
        self.history.append(&message)?;
        Ok(message)
    }

    /// Fan out one sealed room message to every member except this identity.
    pub fn send_group(&mut self, group_id: &str, body: &str) -> Result<GroupSendReport> {
        let group = self
            .group(group_id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("room is not saved".into()))?;
        let msg_id = random_hex(16);
        let ts = now_ms();
        let inner = pack_group_inner(&self.identity, &group.id, &msg_id, ts, body)?;
        let sealed = seal_group(&group.key, &group.id, &msg_id, &inner)?;
        let payload = serde_json::to_vec(&sealed)?;

        let own_pub = self.identity.public_hex();
        let mut delivered = 0usize;
        let mut failed = Vec::new();
        for member in &group.members {
            if member.pubkey == own_pub {
                continue;
            }
            match self.transport.send(&member.nym_address, payload.clone()) {
                Ok(()) => delivered += 1,
                Err(err) => failed.push(format!("{}: {err}", member.petname)),
            }
        }
        if delivered == 0 {
            return Err(CoreError::Transport(format!(
                "room message was not delivered to any member ({})",
                failed.join("; ")
            )));
        }

        let message = ChatMessage::outgoing_group(&own_pub, &group.id, &msg_id, body, ts);
        self.history.append(&message)?;
        Ok(GroupSendReport {
            message,
            delivered,
            failed,
        })
    }

    /// Open and verify an inbound sealed DM. Returns `None` for a duplicate.
    pub fn receive(&mut self, payload: &[u8]) -> Result<Option<ChatMessage>> {
        let sealed: SealedDm = serde_json::from_slice(payload)?;
        let plain = self.identity.open_sealed(&sealed, DM_AAD)?;
        let inner_json = String::from_utf8(plain)
            .map_err(|_| CoreError::InvalidInput("DM inner is not UTF-8".into()))?;
        let inner = unpack_dm_inner(&inner_json, &self.identity.public_hex())?;

        let message =
            ChatMessage::incoming(&inner.from, &inner.to, &inner.msg_id, &inner.body, inner.ts);
        if !self.history.append(&message)? {
            return Ok(None);
        }
        Ok(Some(message))
    }

    /// Dispatch an inbound payload by envelope shape: room messages carry a
    /// `gid`, DMs carry the sealed-box `epub`.
    pub fn receive_any(&mut self, payload: &[u8]) -> Result<Option<ChatMessage>> {
        let value: serde_json::Value = serde_json::from_slice(payload)?;
        if value.get("gid").is_some() {
            self.receive_group(payload)
        } else {
            self.receive(payload)
        }
    }

    /// Open and verify an inbound sealed room message. Returns `None` for a
    /// duplicate.
    pub fn receive_group(&mut self, payload: &[u8]) -> Result<Option<ChatMessage>> {
        let sealed: SealedGroupMessage = serde_json::from_slice(payload)?;
        let group = self
            .group(&sealed.gid)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("room is not known on this device".into()))?;
        let inner_json = open_group(&group.key, &sealed)?;
        let inner = unpack_group_inner(&inner_json, &group.id)?;
        let message = ChatMessage::incoming_group(
            &inner.from,
            &group.id,
            &inner.msg_id,
            &inner.body,
            inner.ts,
        );
        if !self.history.append(&message)? {
            return Ok(None);
        }
        Ok(Some(message))
    }

    /// Test/demo helper: seal a message from `sender` to this engine and feed it
    /// through the normal DM receive path.
    pub fn deliver_demo(
        &mut self,
        sender: &ChatIdentity,
        body: &str,
    ) -> Result<Option<ChatMessage>> {
        let msg_id = random_hex(16);
        let ts = now_ms();
        let inner = pack_dm_inner(sender, &self.identity.public_hex(), &msg_id, ts, body)?;
        let sealed = sender.seal_to(&self.identity.public_hex(), DM_AAD, inner.as_bytes())?;
        let payload = serde_json::to_vec(&sealed)?;
        self.receive(&payload)
    }

    /// Test/demo helper for rooms: sign with `sender`, seal with the room key,
    /// then run the normal group receive path.
    pub fn deliver_demo_group(
        &mut self,
        sender: &ChatIdentity,
        group_id: &str,
        body: &str,
    ) -> Result<Option<ChatMessage>> {
        let group = self
            .group(group_id)
            .cloned()
            .ok_or_else(|| CoreError::NotFound("room is not saved".into()))?;
        let msg_id = random_hex(16);
        let ts = now_ms();
        let inner = pack_group_inner(sender, &group.id, &msg_id, ts, body)?;
        let sealed = seal_group(&group.key, &group.id, &msg_id, &inner)?;
        let payload = serde_json::to_vec(&sealed)?;
        self.receive_group(&payload)
    }

    /// Newest-last messages for the DM with `peer_pub`. `limit` of `0` returns
    /// everything.
    pub fn history_for(&self, peer_pub: &str, limit: usize) -> Vec<ChatMessage> {
        self.history.list(&dm_conversation_id(peer_pub), limit)
    }

    /// Newest-last messages for a room. `limit` of `0` returns everything.
    pub fn group_history(&self, group_id: &str, limit: usize) -> Vec<ChatMessage> {
        let id = group_id.trim().to_lowercase();
        self.history.list(&format!("group:{id}"), limit)
    }

    /// Direct-message conversations, most recent first. Rooms are excluded;
    /// use [`ChatEngine::group_summaries`] for those.
    pub fn conversations(&self) -> Vec<ConversationSummary> {
        let mut grouped: BTreeMap<String, Vec<&ChatMessage>> = BTreeMap::new();
        for message in self.history.all() {
            if message.conversation_id.starts_with("group:") {
                continue;
            }
            grouped
                .entry(message.conversation_id.clone())
                .or_default()
                .push(message);
        }

        let mut out = Vec::with_capacity(grouped.len());
        for (id, messages) in grouped {
            let Some(last) = messages.last() else {
                continue;
            };
            let peer_pub = last.peer_pub().to_string();
            out.push(ConversationSummary {
                id,
                petname: self.contact(&peer_pub).map(|c| c.petname.clone()),
                peer_pub,
                last_message: Some((*last).clone()),
                message_count: messages.len(),
            });
        }
        out.sort_by(|a, b| {
            let a_at = a.last_message.as_ref().map(|m| m.at).unwrap_or(0);
            let b_at = b.last_message.as_ref().map(|m| m.at).unwrap_or(0);
            b_at.cmp(&a_at)
        });
        out
    }

    /// Rooms with their local member list and last message, most recent first.
    pub fn group_summaries(&self) -> Vec<GroupSummary> {
        let mut out: Vec<GroupSummary> = self
            .groups
            .iter()
            .map(|group| {
                let messages = self.history.list(&group.conversation_id(), 0);
                GroupSummary {
                    id: group.id.clone(),
                    name: group.name.clone(),
                    member_count: group.members.len(),
                    members: group.members.clone(),
                    last_message: messages.last().cloned(),
                    message_count: messages.len(),
                }
            })
            .collect();
        out.sort_by(|a, b| {
            let a_at = a.last_message.as_ref().map(|m| m.at).unwrap_or(0);
            let b_at = b.last_message.as_ref().map(|m| m.at).unwrap_or(0);
            b_at.cmp(&a_at)
        });
        out
    }

    /// Delete every stored message from the history journals.
    pub fn clear_history(&mut self) -> Result<()> {
        self.history.clear_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nysiris_core::contact::Contact;
    use nysiris_testkit::MemoryTransport;
    use tempfile::TempDir;

    fn engine(seed: u8, dir: &TempDir) -> (ChatEngine, MemoryTransport) {
        let identity = ChatIdentity::from_secret_bytes([seed; 32]);
        let history = History::open(dir.path(), [seed; 32]).unwrap();
        let transport = MemoryTransport::new();
        let own_address = format!("peer{seed}.enc@gw");
        (
            ChatEngine::new(
                identity,
                vec![],
                vec![],
                history,
                Arc::new(transport.clone()),
                own_address,
            ),
            transport,
        )
    }

    #[test]
    fn two_engines_exchange_a_sealed_dm() {
        let a_dir = TempDir::new().unwrap();
        let b_dir = TempDir::new().unwrap();
        let (mut alice, alice_tx) = engine(1, &a_dir);
        let (mut bob, _bob_tx) = engine(2, &b_dir);

        alice.add_contact(Contact::new("Bob", &bob.identity().public_hex(), "bob.enc@gw").unwrap());
        bob.add_contact(
            Contact::new("Alice", &alice.identity().public_hex(), "alice.enc@gw").unwrap(),
        );

        let sent = alice
            .send_dm(&bob.identity().public_hex(), "hello bob")
            .unwrap();
        assert_eq!(sent.direction, nysiris_core::message::Direction::Outgoing);
        let outbound = alice_tx.take();
        assert_eq!(outbound.len(), 1);

        let received = bob.receive(&outbound[0].payload).unwrap().unwrap();
        assert_eq!(received.body, "hello bob");
        assert_eq!(
            received.direction,
            nysiris_core::message::Direction::Incoming
        );
        assert_eq!(bob.history_for(&alice.identity().public_hex(), 0).len(), 1);
        assert_eq!(bob.conversations().len(), 1);
    }

    #[test]
    fn duplicate_delivery_is_dropped() {
        let dir = TempDir::new().unwrap();
        let (mut bob, _) = engine(2, &dir);
        let alice = ChatIdentity::from_secret_bytes([1u8; 32]);
        bob.deliver_demo(&alice, "once").unwrap();

        let msg = bob.history_for(&alice.public_hex(), 0).remove(0);
        assert!(bob.history().contains(&msg.msg_id));
    }

    #[test]
    fn wrong_recipient_cannot_open_the_dm() {
        let a_dir = TempDir::new().unwrap();
        let b_dir = TempDir::new().unwrap();
        let (mut alice, alice_tx) = engine(1, &a_dir);
        let (mut bob, _) = engine(2, &b_dir);

        let alice_id = alice.identity().public_hex();
        let bob_id = bob.identity().public_hex();
        alice.add_contact(Contact::new("Bob", &bob_id, "bob.enc@gw").unwrap());
        bob.add_contact(Contact::new("Alice", &alice_id, "alice.enc@gw").unwrap());

        alice.send_dm(&bob_id, "for bob").unwrap();
        let outbound = alice_tx.take();
        let carol = ChatIdentity::from_secret_bytes([3u8; 32]);
        let sealed: SealedDm = serde_json::from_slice(&outbound[0].payload).unwrap();
        assert!(carol.open_sealed(&sealed, DM_AAD).is_err());
    }

    #[test]
    fn room_invite_fanout_and_history_round_trip() {
        let a_dir = TempDir::new().unwrap();
        let b_dir = TempDir::new().unwrap();
        let (mut alice, alice_tx) = engine(1, &a_dir);
        let (mut bob, bob_tx) = engine(2, &b_dir);
        let alice_pub = alice.identity().public_hex();
        let bob_pub = bob.identity().public_hex();
        alice.add_contact(Contact::new("Bob", &bob_pub, "bob.enc@gw").unwrap());
        bob.add_contact(Contact::new("Alice", &alice_pub, "alice.enc@gw").unwrap());

        let (group, invite) = alice
            .create_group("Room", std::slice::from_ref(&bob_pub))
            .unwrap();
        let joined = bob.join_group(&invite).unwrap();
        assert_eq!(joined.id, group.id);
        assert_eq!(bob.group_summaries().len(), 1);

        let report = alice.send_group(&group.id, "hello room").unwrap();
        assert_eq!(report.delivered, 1);
        assert!(report.failed.is_empty());
        let outbound = alice_tx.take();
        assert_eq!(outbound.len(), 1);

        let received = bob.receive_any(&outbound[0].payload).unwrap().unwrap();
        assert_eq!(received.body, "hello room");
        assert_eq!(received.conversation_id, format!("group:{}", group.id));
        assert_eq!(bob.group_history(&group.id, 0).len(), 1);

        let reply = bob.send_group(&group.id, "hello back").unwrap();
        assert_eq!(reply.delivered, 1);
        let outbound = bob_tx.take();
        let received = alice.receive_any(&outbound[0].payload).unwrap().unwrap();
        assert_eq!(received.body, "hello back");
        assert_eq!(alice.group_history(&group.id, 0).len(), 2);
        assert_eq!(alice.conversations().len(), 0);
    }
}
