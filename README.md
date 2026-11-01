# osmo-gstreamer

Rust-first unofficial Osmo Nano integration, with a Python testing wrapper
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

The Rust library implements LAN preview and optional BLE setup for Osmo Nano.
The library exposes a path-independent `CameraProfile` that can be loaded from
a file or TOML text in memory. Repository tools store one profile per camera as
`.credentials/<profile>.toml`; each profile contains the Wi-Fi network and
password, BLE pairing identity, camera Wi-Fi MAC, and optional camera IP.
GStreamer reads the selected profile from that gitignored directory.
The source starts a camera session only when the pipeline transitions to its
active state with `allow-writes=true`. Plugin registration and inspection do
not connect to a camera. Initial profile-based connections may initialize over
BLE when the saved LAN pairing port is unreachable, as described below. There
is no automatic reconnect or fallback after startup or streaming failure.

The implementation is unofficial and model-specific; the repository does not
claim an official protocol specification or a supported firmware matrix. CI
tests are hardware-free and do not establish device compatibility. The Python
testing wrapper remains a separate Cargo project and currently exposes only
`version()`. Camera protocol and lifecycle behavior belong in the Rust library,
not in either adapter.

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
`pkg-config`; BLE builds also require `libdbus-1-dev`. On Ubuntu or Debian
(including WSL), `bash ./setup.sh` installs the system build dependencies and
the pinned Rust toolchain. It uses sudo only for apt packages; run it as your
normal user from any directory. Setup is safe to rerun.

```powershell
cargo build --locked -p gst-plugin-osmo
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
$env:GST_PLUGIN_PATH = (Resolve-Path .\target\debug).Path
gst-inspect-1.0 osmosrc
```

Windows builds produce `target\debug\gstosmo.dll`; Linux builds produce
`target/debug/libgstosmo.so`. Plugin tests verify registration, H.264 caps, settings validation, and error
reporting without camera hardware. Hardware testing is not part of CI.

### Viewing the live stream

GStreamer does not include a full graphical pipeline editor. `gst-launch-1.0`
is its command-line pipeline runner, and `autovideosink` opens a native window
to display the decoded video. `gst-play-1.0` is a simple UI for files and
playable URIs, not for launching this camera-source pipeline.

From the repository root, run the pairing command. On first use it prompts for
any missing Wi-Fi SSID and password (password input is hidden), then stores all
camera settings in `.credentials\camera.toml`. Use `--camera PROFILE` to keep
separate profiles for multiple cameras. It waits for any required approval on
the camera and configures the camera to join that Wi-Fi network; it does not
change the computer's Wi-Fi settings:

```powershell
cargo run --locked -p osmo-camera --features ble --bin osmo-pair
cargo run --locked -p osmo-camera --features ble --bin osmo-pair -- --camera office
```

The pairing utility prints the camera's Wi-Fi MAC and looks for a matching
IPv4 entry in the local OS neighbor table (Windows or Linux). This is a
best-effort cache lookup, not a network scan or a guarantee that the entry is
current. A unique match is stored as the profile's `camera_ip` unless an existing
saved IP differs; in that case both addresses are reported and the saved value
is retained. Verify the address before streaming. BLE provides the Wi-Fi MAC,
not a supported DHCP-IP query. If there is no unique entry, check the router's DHCP leases
using the reported MAC and add `camera_ip = "..."` to the profile file. The
password is stored as plaintext in this Git-ignored file. On first use with
legacy `.credentials` files, the utility imports their settings into the
profile and removes those old files after saving the profile.

A profile uses this TOML shape (values below are placeholders):

```toml
version = 1
wifi_ssid = "your-network"
wifi_password = "your-password"
pairing_identity = "00000000000000000000000000000001"
wifi_mac = "AA:BB:CC:DD:EE:FF"
camera_ip = "192.0.2.10"
```

Keep the profile local and do not commit it; `.gitignore` excludes
`.credentials`.

After building the plugin, run this from the configured Command Prompt and
repository root:

```cmd
set GST_PLUGIN_PATH=%CD%\target\debug
gst-launch-1.0 -v osmosrc camera-profile=camera allow-writes=true ! h264parse ! avdec_h264 ! videoconvert ! queue ! autovideosink sync=true
```

Starting this pipeline connects to the camera and performs the explicitly
approved preview-session writes; plugin inspection does not. Stop the pipeline
with Ctrl+C to close the session cleanly. On Windows, `d3d12videosink sync=true`
can replace `autovideosink sync=true`. The decoded-video `queue` separates
decoding from display scheduling; it does not prevent camera-side pauses or
compressed-source queue overflow. The profile is plaintext and must remain
local; `.credentials` is ignored by Git.

The viewer needs the H.264 parser, software decoder and selected video sink
plugins in addition to the development SDK. On Debian/Ubuntu, install
`gstreamer1.0-plugins-bad`, `gstreamer1.0-libav` and
`gstreamer1.0-plugins-good` for the example pipeline; `setup.sh` prepares
build/test dependencies, not every optional playback plugin.

To diagnose camera LAN preview startup without opening the GStreamer viewer,
run the pairing utility's opt-in smoke test:

```powershell
$env:RUST_LOG = "osmo_camera=debug,osmo_pair=info"
cargo run --locked -p osmo-camera --features ble --bin osmo-pair -- --smoke-test
```

It uses the selected profile (add `--camera PROFILE` if needed). Preview
connections first check the saved IP's pairing TCP port. If reachable they use
LAN directly; otherwise they wake/pair through BLE and reapply the profile's
Wi-Fi settings once before starting preview. After BLE initialization they wait
up to ten seconds for LAN readiness. The BLE owner stays alive until preview
is closed. Missing BLE credentials and stale IPs are explicit errors.
Use `--lan-only` with either diagnostic to prohibit BLE initialization and
Wi-Fi changes. Playback failures never trigger initialization or reconnect.
It reports startup errors, then waits up to five seconds per access unit for
three H.264 access units before closing the session. This checks camera
registration and raw video delivery, but does not test GStreamer decoding or
display.

For a longer timing diagnostic, run:

```powershell
$env:RUST_LOG = "osmo_pair=debug,osmo_camera=info"
cargo run --locked -p osmo-camera --features ble --bin osmo-pair -- --timing-test 60
```

The duration is 1-300 seconds after preview startup. Flip the camera during
the test and correlate the timestamps with the action. This mode logs per-frame
arrival intervals, drift relative to the viewer's nominal 30000/1001 fps,
queue age, keyframes, discontinuities, and H.264 parameter-set changes. It
reports gaps of at least 100 ms and summarizes cadence once per second.
It consumes video without decoding, displaying, or saving it. By default it
uses the same reachability-gated initialization described above and saves any
new pairing identity to the profile; `--lan-only` leaves the profile unchanged.
Silence is reported while
the existing session remains alive; protocol failures remain terminal, and
the session is explicitly closed on completion or failure.
Frame numbers are local receiver counters, not camera counters. A keyframe,
parameter-set change, or arrival gap is evidence to correlate, not proof of
an orientation-triggered restart. This test isolates delivery timing from
GStreamer decoding and display scheduling.

The GStreamer source derives PTS/DTS from monotonic receiver arrival time,
anchored to pipeline running time on the first frame. Camera pauses remain
in the timeline instead of accumulating delay against a fixed frame counter.
Arrival gaps of at least 100 ms (or three nominal frame intervals, whichever
is longer) mark the resumed buffer as discontinuous without dropping it or
reconnecting. `framerate` still controls nominal caps and buffer duration;
timestamps are receiver timing, not camera capture timestamps. Network jitter
and decoding latency can still cause late frames.

GStreamer's `allow-writes=true` also authorizes reachability-gated BLE
initialization using the selected profile. The Rust library exposes this flow
through `CameraProfile::start_preview`; the lower-level `CameraConfig` entry
points remain explicitly LAN-only for callers managing initialization themselves.

The CLI uses Rust's standard `log` facade and writes to stderr; `RUST_LOG`
controls its verbosity. When the plugin is loaded by GStreamer, it forwards
Rust log records into GStreamer's `osmo-camera` debug category unless the host
application already configured a Rust logger. To see detailed plugin and
camera lifecycle diagnostics in the GStreamer console, set
`GST_DEBUG=osmo-camera:6` before launching the pipeline. Logs intentionally
exclude pairing identities, Wi-Fi credentials, and protocol payloads.
Detailed diagnostics include preview command IDs, endpoints, response status,
transport sequences, elapsed time, and retransmission counts. Reply timeouts
include buffered-message and control-window state. Complete video arrival gaps
of at least 100 ms are warnings; cleanup stages log their own failures separately
from startup or streaming failures. Set `RUST_LOG=osmo_camera=trace` for CLI
control-feedback details, or `GST_DEBUG=osmo-camera:7` for GStreamer.
Logs go to stderr/the host logger, not automatically to files; redirect stderr
to a local file when retaining diagnostics. Do not publish logs without review.

### Current limitations

- Targeted compatibility is Osmo Nano; no firmware support matrix is certified.
- Preview startup can fail waiting for playback confirmation or live-view
  command replies even when the pairing TCP port is reachable. Reachability
  does not guarantee camera readiness; errors remain explicit.
- Video can arrive while startup is still waiting for a reply. The bounded
  compressed queue (32 pictures / 16 MiB) can then overflow before the caller
  starts consuming. Queue overflow stops the session rather than silently
  dropping dependent H.264 pictures.
- Camera-side pauses/reconfiguration remain visible. Arrival-based timestamps
  preserve elapsed time but cannot reconstruct missing video.
- Cleanup can fail or time out; startup, streaming and cleanup failures are
  reported separately. No failure triggers an automatic reconnect.

## Development and CI

- Native and Python Cargo lockfiles plus `uv.lock` are checked in; CI uses
  locked dependency resolution.
- [CI](.github/workflows/ci.yml) uses SHA-pinned actions, read-only permissions,
  timeouts, cancellation of superseded runs and Rust/uv caches.
- Core and Python tests run on Linux, Windows and macOS. Python bindings cover
  Python 3.10 and 3.15, including installation tests of release wheels. The
  BLE-enabled library and pairing utility tests run on all three platforms.
- GStreamer registration, caps, timestamp behavior, startup errors and dynamic plugin loading are
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

## Releases

[Release automation](.github/workflows/release.yml) runs when an authorized
maintainer pushes a `vX.Y.Z` tag. It publishes `osmo-camera` and
`gst-plugin-osmo` to crates.io, then publishes a GitHub Release containing
the two `.crate` source archives, checksums, and native archives for:

- Windows x64 MSVC: `gstosmo.dll` and `osmo-pair.exe`.
- Linux x64 and ARM64: `libgstosmo.so` and `osmo-pair`.
- macOS Intel and Apple Silicon: `libgstosmo.dylib` and `osmo-pair`.

Python wheels are not published by this workflow. Each native archive includes
the plugin under `lib`, the pairing utility under `bin`, the MIT license, generated third-party notices,
this guide, and `BUILD-INFO.txt` recording its build/runtime baseline.
GStreamer, vendor software, credentials, recordings and debug symbols are not
bundled. Native tests and plugin inspection are hardware-free; successful
builds do not certify camera or firmware compatibility. macOS does not support
the pairing utility's automatic neighbor-table lookup; set the camera IP
manually as described above.

### One-time maintainer setup

1. Log in to [crates.io](https://crates.io), verify your account email, and
   confirm that both crate names are available or owned by your account.
2. Create an [API token](https://crates.io/settings/tokens) with the narrowest
   permissions sufficient to create/publish these two crates and an appropriate
   expiration. Add it as the GitHub Actions secret `CARGO_REGISTRY_TOKEN`,
   in the repository's `CargoPublish` environment. Never commit or share
   the token; revoke it immediately if exposed.
3. Configure the GitHub Actions `CargoPublish` environment with required maintainer
   approval and allow only release tags. Protect `v*` tags using a repository
   ruleset so only authorized maintainers can create them.

Build jobs have read-only repository permissions and no publishing token.
The publishing job waits for every native build and source-package verification
and, when configured, environment approval. Review the exact tagged changes,
source archive listings, dependency notices, native dependencies, generated
metadata and downloadable artifacts before approving. A tag alone is not a
provenance or redistribution audit.

### Cutting a release

Update the workspace version in `Cargo.toml` and the plugin's exact
`osmo-camera` dependency version in `crates/gst-plugin-osmo/Cargo.toml`.
Refresh both Cargo lockfiles as needed and run the documented component checks.
Keep both crate `LICENSE` copies identical to the root MIT license.
The crates inherit author, homepage, repository and README metadata from the
workspace; each declares its own docs.rs URL, keywords and categories.
Release validation requires these fields and the MIT SPDX license declaration.
The tag must match both crate versions and the plugin dependency exactly.
Commit and push the reviewed changes, then, with maintainer approval:

```powershell
git tag -a v0.1.0 -m "Release 0.1.0"
git push origin v0.1.0
```

Rust crates publish in dependency order. Publication is not transactional:
if the second crate or GitHub upload fails, the first crate may already be
public. Re-run the failed workflow after resolving the error. Already-published
crate versions are accepted only if their checksum matches the source artifact;
different contents require a new version/tag. Draft GitHub uploads can be retried,
but published release assets are not overwritten.

Run `pwsh -File .\scripts\test-release.ps1` to test tag validation, publishing
order, retry checksums and missing-token failures with synthetic registry
responses. These tests never upload crates or access camera hardware.

### Installing native archives

Install a GStreamer runtime matching the version recorded in `BUILD-INFO.txt`,
or a separately validated compatible runtime, for the archive's architecture.
Windows requires the MSVC distribution and any required MSVC runtime.
Linux archives are built on Ubuntu 24.04 and require compatible glibc,
GStreamer and D-Bus system libraries; they are not universal Linux binaries.
macOS archives use Homebrew GStreamer and require compatible Homebrew library
locations for their architecture.

Extract the archive, make the runtime libraries available to the OS loader,
and set `GST_PLUGIN_PATH` to its `lib` directory. On Windows, for example:

```powershell
$env:PATH = "$env:GSTREAMER_ROOT_X86_64\bin;$env:PATH"
$env:GST_PLUGIN_PATH = (Resolve-Path .\osmo-gstreamer-0.1.0-x86_64-pc-windows-msvc\lib).Path
gst-inspect-1.0 osmosrc
```

Inspection does not connect to hardware. Run the extracted pairing utility
from your chosen working directory only when explicitly authorizing camera
setup; profiles remain local under `.credentials`.
