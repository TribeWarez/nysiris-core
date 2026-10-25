# nysiris-testkit

[![crates.io](https://img.shields.io/crates/v/nysiris-testkit.svg)](https://crates.io/crates/nysiris-testkit)
[![docs.rs](https://docs.rs/nysiris-testkit/badge.svg)](https://docs.rs/nysiris-testkit)

In-memory transport and fixtures for tests that use the
[Nysiris-Core](https://github.com/TribeWarez/nysiris-core) chat crates. Nothing
here touches the network, Nym, or the filesystem unless your test asks it to.

## Install

```toml
[dev-dependencies]
nysiris-testkit = "0.1"
```

## What you get

- `MemoryTransport` — a `Transport` implementation that records what was sent.
- `sent()` — inspect recorded `SentPayload`s without consuming them.
- `take()` — drain the recorded sends, which is how a test plays the other side
  of a conversation.
- `len()` and `is_empty()` — quick assertions on delivery counts.

`MemoryTransport` is cheap to clone and share through `Arc<dyn Transport>`, so
two `ChatEngine`s can be wired together in a single test. Because it never
drops or reorders, it models an ideal network; ack and retry behaviour has to be
tested separately.

## Example

```rust,no_run
use nysiris_core::Transport;
use nysiris_testkit::MemoryTransport;

fn main() -> nysiris_core::Result<()> {
    let transport = MemoryTransport::new();
    transport.send("bob.enc@gw", b"sealed bytes".to_vec())?;
    assert_eq!(transport.len(), 1);

    let delivered = transport.take();
    assert_eq!(delivered.len(), 1);
    Ok(())
}
```

## Security

Test-only code. Do not use `MemoryTransport` outside tests or examples. See
[SECURITY.md](https://github.com/TribeWarez/nysiris-core/blob/main/SECURITY.md)
for the repository's reporting process.

## License

Apache-2.0. See
[LICENSE](https://github.com/TribeWarez/nysiris-core/blob/main/LICENSE).
