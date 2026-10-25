#!/usr/bin/env bash
#
# Verifies that the Nym version pins in crates/nysiris-transport-nym/Cargo.toml
# keep a from-scratch resolve on the mainnet-verified set in Cargo.lock.
#
# Why this exists: the Nym 1.22.x patch line on crates.io is internally
# inconsistent. nym-client-core 1.22.2 requires the yanked tokio_with_wasm
# 0.10.1, so a fresh resolve falls back to nym-client-core 1.22.1, which does
# not compile against the 1.22.2 nym-crypto / nym-client-core-config-types
# siblings. Cargo ignores optional dependencies that no enabled feature
# references, so the exact pins only take effect through the `nym` feature -
# the same switch a downstream consumer flips. This script reproduces that
# consumer path in a throwaway project that has no lockfile.
#
# Run: bash scripts/check-nym-pins.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Git Bash on Windows hands Cargo a /c/... path it cannot read; normalize to a
# drive-letter path when cygpath is available (no-op on Linux CI).
if command -v cygpath >/dev/null 2>&1; then
  root="$(cygpath -m "$root")"
fi
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

mkdir -p "$work/src"
cat > "$work/Cargo.toml" <<EOF
[workspace]

[package]
name = "pin-probe"
version = "0.0.0"
edition = "2021"

[dependencies]
nysiris-transport-nym = { path = "$root/crates/nysiris-transport-nym", features = ["nym"] }
EOF
: > "$work/src/lib.rs"

cargo generate-lockfile --manifest-path "$work/Cargo.toml" >/dev/null

# Sorted name=version pairs for every nym* package in a lockfile.
nym_versions() {
  awk '
    /^\[\[package\]\]/ { name = "" }
    /^name = "/ {
      name = $0
      sub(/^name = "/, "", name)
      sub(/"$/, "", name)
      next
    }
    /^version = "/ {
      if (name != "" && name ~ /^nym/) {
        version = $0
        sub(/^version = "/, "", version)
        sub(/"$/, "", version)
        print name "=" version
      }
    }
  ' "$1" | sort
}

nym_versions "$root/Cargo.lock" > "$work/expected.txt"
nym_versions "$work/Cargo.lock" > "$work/actual.txt"

if [ ! -s "$work/actual.txt" ]; then
  echo "error: the probe resolved no nym crates; is the nym feature still wired up?" >&2
  exit 1
fi

if ! diff -u "$work/expected.txt" "$work/actual.txt"; then
  echo >&2
  echo "error: a from-scratch resolve does not reproduce the pinned Nym set." >&2
  echo "       Update the exact pins in crates/nysiris-transport-nym/Cargo.toml" >&2
  echo "       and Cargo.lock together, then re-test with --features nym." >&2
  exit 1
fi

echo "ok: $(wc -l < "$work/actual.txt") nym crates resolve to the Cargo.lock versions"
