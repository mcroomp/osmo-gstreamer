# Project-wide contributor instructions

## Scope and component rules

These rules apply to the entire repository. Read this file and the relevant
component instructions before editing:

- [Rust camera library](crates/unofficial-osmo-camera/AGENTS.md)
- [Python testing wrapper](python/unofficial-osmo-camera/AGENTS.md)
- [GStreamer plugin](crates/gst-plugin-unofficial-osmo/AGENTS.md)

Component rules supplement these rules; they cannot relax publication,
provenance or safety requirements. Use [README.md](README.md) for build commands.

## Unofficial, compatibility-only project

- This is an independent, unofficial interoperability project. It is not
  affiliated with, endorsed by or supported by DJI. Use product names only to
  identify compatibility; do not use vendor logos or imply certification.
- Interoperability may involve lawful observation, study and testing, and
  applicable law may provide a narrow decompilation exception. Neither
  compatibility purpose nor open-source publication is blanket permission;
  follow the [interoperability policy](INTEROPERABILITY.md) and request
  maintainer review when authorization or legal scope is uncertain.
- As a separate project safety rule, do not bypass authentication, pairing
  approvals, encryption, access controls, DRM, firmware protections or vendor
  service restrictions merely on the basis of an interoperability purpose.
- Do not describe undocumented behavior as an official specification. Preserve
  model/firmware limitations and distinguish documented contracts, observations
  and unverified assumptions.

## Public-source and provenance boundary

- Write original code. Do not copy vendor application/firmware code, decompiled
  output, disassembly, leaked material or proprietary SDK content into this repo.
  Do not add vendor binaries, APKs, firmware images, memory dumps or screenshots
  of proprietary internals.
- Public availability is not permission to redistribute. Before importing code,
  fixtures, assets or documentation, verify their origin, license and
  redistribution rights; preserve required attribution and notices. Unknown or
  incompatible provenance blocks import, not just publication.
- No automatic migration from another repository. Review each proposed import
  for provenance, license, sensitive data and compatibility-only scope first.
  Do not turn private experimental notes into public specifications.
- Document necessary interoperability facts in original wording and retain
  source links, revisions and applicable model/firmware scope. Prefer public,
  authorized documentation and independently generated synthetic fixtures.
  Authorized device observations must be minimal, sanitized and attributable
  to a stated evidence category; they are not permission to publish captures.
- Do not commit raw traffic captures, pairing tokens, credentials, credential-
  bearing payloads, private keys, device/network identifiers or personal data.
  Do not move restricted material into issues, PRs, CI output or package metadata.
- If provenance, authorization, terms or distribution rights are uncertain,
  stop the affected work and request maintainer review. Do not assume an
  interoperability exception resolves the issue; follow the
  [interoperability policy](INTEROPERABILITY.md) and request case-specific
  review.

## Architecture and hardware safety

- Rust owns protocol, transport, pairing, connection ownership and lifecycle.
  Python and GStreamer are thin independent adapters to the Rust library.
- Hardware operations require explicit authorization; default to offline tests.
  Never connect, capture, write settings, change networks or reconnect silently.
  Use one owner per camera, bounded operations and explicit teardown.
- Surface errors and unsupported behavior. Do not return success-shaped stubs,
  infer undocumented semantics or add transport fallbacks without approval.
- Source documentation describes public API contracts. Keep discovery workflows,
  local machine paths and private observations out of library documentation.

## Tooling, validation and release

- In every Rust file, place module documentation first, then a consolidated
  block of `use` statements, then item documentation and declarations. Use
  imports instead of fully qualified names in declarations, attributes and
  executable code; use import aliases to resolve naming collisions. Let rustfmt
  standardize import layout. Keep conditional imports with their `cfg`
  attributes. Each test block and nested module must likewise have one
  consolidated import block at its beginning, before helpers and tests; do not
  scatter imports through the block or place them inside individual tests.
  Command modules put the documented main command struct immediately after
  the imports, before supporting types and constants.

- Prefer Bash shell scripts and CI steps. Use PowerShell only for operations
  that require Windows-specific tooling.

- Use the pinned Rust toolchain and uv for Python. The Python project uses the
  named Microsoft package feed; do not silently switch indexes, disable TLS
  verification or add unreviewed package sources.
- Keep Cargo and uv lockfiles checked in and consistent; normal CI/builds use
  locked resolution. Validate the changed component with existing tests,
  rustfmt/Clippy or Ruff; do not introduce duplicate build tooling.
- Preserve SHA-pinned actions, minimal CI permissions and hardware-free CI.
  Never publish packages, push changes or create releases without approval.
- Before public release, review the exact Git changes, fixtures, documentation,
  source archive, wheel/plugin contents and generated metadata for provenance,
  notices, secrets and private build paths. Ignore rules are not a release audit.
  Do not ship test recordings, local evidence, caches or debug symbols.
