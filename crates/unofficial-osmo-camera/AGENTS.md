# Rust camera library

Read the [root instructions](../../AGENTS.md) first. This file applies to this
crate and its descendants.

## Responsibility

- Expose pure Rust APIs; do not depend on Python, GStreamer, a GUI or a CLI.
- Keep protocol, transport, camera ownership and lifecycle here, not in
  adapters. The current implementation targets Osmo Nano LAN preview and
  BLE operations; do not imply broader model or firmware support. BLE support
  is always compiled in, but hardware operations require explicit approval.
- Use typed commands, endpoints, statuses and payloads; keep wire encoding and
  decoding at explicit boundaries. Preserve unknown values without inventing
  meanings. Separate codecs from transport and lifecycle orchestration.

## Evidence and tests

- Add original, minimal interoperability contracts with provenance and
  model/firmware limitations. Do not add decompiled implementations, vendor
  payload dumps or private exploration records.
- Prefer synthetic byte-exact and malformed-input tests. Fixtures must contain
  no real identity, credential, capture or proprietary artifact.
- Bound timeouts, buffering and retries; support cancellation and explicit
  cleanup. A failed connection must not trigger an automatic reconnect.
- Public rustdoc explains caller-visible contracts and uncertainty, not internal
  discovery methods or machine-specific paths.

## Validation

From the repository root:

```powershell
cargo test --locked -p unofficial-osmo-camera
cargo clippy --locked -p unofficial-osmo-camera --all-targets -- -D warnings
cargo fmt --all -- --check
```

Linux builds and checks require `libdbus-1-dev` and `pkg-config`.
Preserve Rust 1.92 compatibility and independence from the GStreamer SDK.
When APIs change, rebuild and test affected adapters within the requested scope.
Do not modify, rebuild or test Python bindings or alter Python environments
unless explicitly requested; follow the root Python-work restriction.
