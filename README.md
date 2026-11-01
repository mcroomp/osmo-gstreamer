# osmo-gstreamer

Rust-first foundation for Osmo camera integration, with a Python testing wrapper
and a native GStreamer plugin. Licensed under [MIT](LICENSE).

## Unofficial interoperability project

This independent project is not affiliated with, endorsed by or supported by
DJI. DJI and Osmo names identify intended compatibility only; no certification
or official protocol specification is claimed.

The scope is interoperability with authorized, user-controlled devices, not
circumvention of access controls or redistribution of vendor software.
Contributions must be independently authored or appropriately licensed, with
traceable provenance. Vendor binaries, decompiled code, private captures and
sensitive data do not belong in public source, packages or CI output.
See the [contributor rules](AGENTS.md) and their linked component instructions.
These safeguards do not constitute legal advice or guarantee legal clearance.

## Components

| Component | Location | Responsibility |
|-----------|----------|----------------|
| Rust library | [crates/osmo-camera](crates/osmo-camera) | Protocol, transport, camera ownership and lifecycle |
| Python wrapper | [python/osmo-camera](python/osmo-camera) | Thin PyO3/maturin bindings for testing the Rust library |
| GStreamer plugin | [crates/gst-plugin-osmo](crates/gst-plugin-osmo) | Adapt the Rust library to a GStreamer source |

This is a fresh skeleton: no camera implementation has been migrated.
The library and Python wrapper currently expose only `version()`.
The plugin registers as `osmo`, with element `osmosrc` and an H.264 source pad.
It explicitly fails on startup because streaming is not implemented; it does not
connect to a camera or emit simulated video.

The base library has no Python or GStreamer dependencies. The wrapper is a
separate Cargo project so Python tooling and build outputs stay independent.
Future camera behavior belongs in Rust, not in either adapter.

## Rust

Requires a native linker (MSVC build tools on Windows).
The [toolchain file](rust-toolchain.toml) pins Rust 1.99.1 with rustfmt and
Clippy; the library's minimum supported Rust version is 1.92.
Run these commands from the repository root:

```powershell
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked -p osmo-camera --all-targets -- -D warnings
```

The default workspace member is the base library, so these tests do not need
GStreamer installed.

## Python testing wrapper

Requires Python 3.10+ and [uv](https://docs.astral.sh/uv/).
GStreamer is not required. uv uses the default `microsoft-python` package index
at `https://packagefeedproxy.microsoft.io/pypi/simple/`; this selects a package
feed, not a Microsoft-specific interpreter.
Tool versions are resolved from that feed; maturin currently resolves to 1.15.0.

```powershell
uv sync --locked --project .\python\osmo-camera
uv run --project .\python\osmo-camera --no-sync maturin develop --locked --manifest-path .\python\osmo-camera\Cargo.toml
uv run --project .\python\osmo-camera --no-sync python -m unittest discover -s .\python\osmo-camera\tests
uv run --project .\python\osmo-camera --no-sync ruff check .\python\osmo-camera
uv run --project .\python\osmo-camera --no-sync ruff format --check .\python\osmo-camera
cargo fmt --manifest-path .\python\osmo-camera\Cargo.toml -- --check
```

Re-run `maturin develop` after changing Rust code. For a distributable wheel
and source archive:

```powershell
uv run --project .\python\osmo-camera --no-sync maturin build --locked --release --sdist --manifest-path .\python\osmo-camera\Cargo.toml --out .\dist
```

## GStreamer plugin

Requires the GStreamer **runtime and development SDK**, version 1.24+.
On Windows, run `setup.cmd` from Command Prompt. It installs missing Rustup,
uv and pkgconf tools with WinGet, ensures MSVC C++ Build Tools and the pinned
Rust toolchain are available, calls the GStreamer setup script, and syncs the
locked Python environment. GStreamer installs per-user through WinGet as an
x64 MSVC development install. Some prerequisites may require elevation.

```cmd
setup.cmd
```

The default per-user install location is
`%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64`. If GStreamer is already
installed elsewhere, set `GSTREAMER_ROOT_X86_64` to that location before
running setup. If development files are missing, the GStreamer script forces
WinGet to rerun the pinned installer in development mode. WinGet verifies the
installer against its package manifest. The batch file leaves the current
Command Prompt configured; run it from an existing Command Prompt rather than
double-clicking it. Setup is safe to rerun: installed tools and configured
environment paths are reused.
On Linux, install the GStreamer and plugins-base development packages and
`pkg-config`.

```powershell
cargo build --locked -p gst-plugin-osmo
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
$env:GST_PLUGIN_PATH = (Resolve-Path .\target\debug).Path
gst-inspect-1.0 osmosrc
```

Windows builds produce `target\debug\gstosmo.dll`; Linux builds produce
`target/debug/libgstosmo.so`. Plugin tests verify registration, H.264 caps and
explicit failure of the unimplemented source. Hardware testing is not part of
this skeleton.

## Development and CI

- Native and Python Cargo lockfiles plus `uv.lock` are checked in; CI uses
  locked dependency resolution.
- [CI](.github/workflows/ci.yml) uses SHA-pinned actions, read-only permissions,
  timeouts, cancellation of superseded runs and Rust/uv caches.
- Core and Python tests run on Linux, Windows and macOS. Python bindings cover
  Python 3.10 and 3.15, including installation tests of release wheels.
- GStreamer registration, caps, startup errors and dynamic plugin loading are
  checked on Linux. The Rust 1.92 minimum is checked separately for the core.
- Rustfmt, strict Clippy, warning-free Rust documentation and Ruff enforce
  quality; actionlint checks the workflow itself. Python uses current PyO3,
  maturin and uv rather than subprocess
  bridges or a second protocol implementation.
- [Dependabot](.github/dependabot.yml) groups weekly Cargo, uv and GitHub
  Actions updates. Pull requests receive dependency vulnerability review.
- [EditorConfig](.editorconfig) and [Git attributes](.gitattributes) keep
  formatting and line endings consistent across platforms.

CI pins uv 0.13.6. To intentionally refresh dependencies, run `cargo update`
in each Cargo project and `uv lock --upgrade --project .\python\osmo-camera`,
then run the checks above. Update the toolchain pin and its CI entries together.
Publishing packages or creating a GitHub repository is not automated here.
