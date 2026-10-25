# nysiris-chat-core

[![crates.io](https://img.shields.io/crates/v/nysiris-chat-core.svg)](https://crates.io/crates/nysiris-chat-core)
[![docs.rs](https://docs.rs/nysiris-chat-core/badge.svg)](https://docs.rs/nysiris-chat-core)

Peer-to-peer chat engine for Nysiris-native clients: sealed 1:1 messages and
capability-key rooms with client-side fan-out over the `Transport` trait.

There is no chat service and no server-side history. Every message this engine
sends or accepts is written to the local encrypted journal from
`nysiris-chat-history`.

Part of the [Nysiris-Core](https://github.com/TribeWarez/nysiris-core) workspace.

## Install

```toml
[dependencies]
nysiris-chat-core = "0.1"
nysiris-core = "0.1"
nysiris-chat-history = "0.1"
```

Add `nysiris-transport-nym` with `features = ["nym"]` for a real transport, or
`nysiris-testkit` for tests.

## What you get

- `ChatEngine` — contacts, rooms, sending, receiving, and history queries in one
  place.
- `send_dm` and `receive` for sealed direct messages; `receive_any` also accepts
  room traffic.
- `create_group`, `join_group`, `group_invite`, `send_group`, and
  `receive_group` for rooms. Nobody holds an authoritative roster.
- `GroupSendReport` — the sealed message plus how many members accepted it and
  which failed, so a partial fan-out is visible instead of silent.
- Duplicate inbound messages are dropped by message id, so replays do not grow
  the history.
- `conversations`, `group_summaries`, `history_for`, and `group_history` for UI
  state.

## Example

```rust,no_run
use std::sync::Arc;

use nysiris_chat_core::ChatEngine;
use nysiris_chat_history::History;
use nysiris_core::{ChatIdentity, Contact};
use nysiris_testkit::MemoryTransport;

fn main() -> nysiris_core::Result<()> {
    let identity = ChatIdentity::from_secret_bytes([1u8; 32]);
    let history = History::open("./history", [2u8; 32])?;
    let mut engine = ChatEngine::new(
        identity,
        Vec::new(),
        Vec::new(),
        history,
        Arc::new(MemoryTransport::new()),
        "me.enc@gw",
    );

    // The peer's ed25519 public key and Nym address arrive out of band in a
    // signed invite.
    let bob_pubkey = "a".repeat(64);
    engine.add_contact(Contact::new("Bob", &bob_pubkey, "bob.enc@gw")?);
    let _sent = engine.send_dm(&bob_pubkey, "hello")?;
    Ok(())
}
```

Room messages are sealed once with the room key and sent to each member
individually; rooms are capped at 16 members, and removing a member requires a
new room key.

## Security

Experimental protocol code, not independently audited. Invite handling, room key
rotation, and message replay are the areas to review first. Report
vulnerabilities privately through
[SECURITY.md](https://github.com/TribeWarez/nysiris-core/blob/main/SECURITY.md).

## License

Apache-2.0. See
[LICENSE](https://github.com/TribeWarez/nysiris-core/blob/main/LICENSE).
