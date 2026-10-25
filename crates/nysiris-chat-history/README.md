# nysiris-chat-history

[![crates.io](https://img.shields.io/crates/v/nysiris-chat-history.svg)](https://crates.io/crates/nysiris-chat-history)
[![docs.rs](https://docs.rs/nysiris-chat-history/badge.svg)](https://docs.rs/nysiris-chat-history)

Encrypted, append-only local chat history. Flat files only — no SQLite, and no
plaintext message bytes on disk.

Part of the [Nysiris-Core](https://github.com/TribeWarez/nysiris-core) workspace.

## Install

```toml
[dependencies]
nysiris-chat-history = "0.1"
```

## How it stores records

- One journal per conversation, named from a BLAKE2b-256 hash of the
  conversation id, so filenames never reveal who is talking.
- Each record is length-prefixed, sealed with XChaCha20-Poly1305, and bound to the
  journal filename as AEAD associated data, so a record cannot be moved between
  conversations.
- Every append is flushed and `fsync`ed, and journals are created `0600` on
  Unix.
- Opening a journal with the wrong key fails closed. Truncated or relocated
  records are rejected with an error instead of being silently skipped.
- Legacy journals written with the older 8-byte filename hash are still read, so
  a local history survives the widening to a 128-bit filename.

## API

- `History::open(dir, key)` — load every journal in a directory.
- `History::append(&message)` — returns `false` for a duplicate message id, so
  replayed traffic is not stored twice.
- `History::list(conversation_id, limit)` — newest-last messages for one
  conversation.
- `History::all`, `count`, `contains`, `clear_conversation`, `clear_all`.

The key is supplied by the application. Keep it out of logs, out of the journal
directory, and back it up: losing it loses the history.

## Security

Experimental storage code, not independently audited. Records are encrypted, but
file sizes and timestamps still leak metadata. Report vulnerabilities privately
through
[SECURITY.md](https://github.com/TribeWarez/nysiris-core/blob/main/SECURITY.md).

## License

Apache-2.0. See
[LICENSE](https://github.com/TribeWarez/nysiris-core/blob/main/LICENSE).
