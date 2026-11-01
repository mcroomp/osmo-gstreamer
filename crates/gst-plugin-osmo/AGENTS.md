# GStreamer plugin

Read the [root instructions](../../AGENTS.md) first. This file applies to this
crate and its descendants.

## Responsibility

- Keep the plugin a thin native adapter to the Rust camera library. Do not add
  Python, subprocess bridges or a second protocol/connection implementation.
- Preserve plugin `osmo`, element `osmosrc` and library `gstosmo` naming unless a
  deliberate API change is approved. Metadata must identify the project as
  unofficial and must not imply vendor certification.
- Advertise only supported output formats. The skeleton advertises Annex-B H.264
  access-unit caps but explicitly fails startup; do not simulate successful
  streaming or auto-connect while inspecting or registering the plugin.
- Implement state changes, cancellation, backpressure and teardown using
  GStreamer conventions. Report actionable errors on the bus. Connection loss
  does not authorize reconnects, network changes or capture startup.

## Validation and distribution

- Use matching GStreamer runtime/development SDKs, version 1.24+; the Windows
  build uses x64 MSVC. See the [root guide](../../README.md#gstreamer-plugin).
- Test registration, negotiated caps, state-change failures and cleanup without
  hardware. Validate dynamic loading with `gst-inspect-1.0 osmosrc`.
- From the root, run `cargo test --locked --workspace`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings` and
  `cargo fmt --all -- --check`.
- Review plugin metadata and release binaries for unofficial wording, required
  dependency notices and private build paths. Do not bundle vendor software,
  local camera recordings, debug symbols or unreviewed SDK assets.
