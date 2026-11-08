# osmo-gstreamer for DJI Osmo Nano

Rust-first unofficial Osmo Nano integration, with a Python testing wrapper
and a native GStreamer plugin. Licensed under [MIT](LICENSE). Some protocol
information draws on the independently published documentation from
[luisreysan/osmocam](https://github.com/luisreysan/osmocam), credited in the
[protocol notes](PROTOCOL.md#recording-formats-and-live-preview-capabilities)
with source revisions and compatibility limitations.

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
| Rust library | [crates/unofficial-osmo-camera](crates/unofficial-osmo-camera) | Protocol, transport, camera ownership and lifecycle |
| Python wrapper | [python/unofficial-osmo-camera](python/unofficial-osmo-camera) | Thin PyO3/maturin bindings for testing the Rust library |
| GStreamer plugin | [crates/gst-plugin-unofficial-osmo](crates/gst-plugin-unofficial-osmo) | Adapt the Rust library to a GStreamer source |

The Rust library supports LAN preview and optional BLE setup for Osmo Nano.
The GStreamer element is `osmosrc`; the Python testing wrapper imports as
`osmo_camera` and exposes native control sessions, raw and typed protocol
bindings, and an evidence-aware capability view.

Camera operations require explicit authorization. Plugin inspection does not
connect to hardware, and failures never trigger automatic reconnection.
Compatibility is model-specific, with no verified firmware support matrix.
CI tests are hardware-free and do not establish device compatibility.

## Rust

Requires a native linker (MSVC build tools on Windows).
The [toolchain file](rust-toolchain.toml) pins Rust 1.99.1 with rustfmt and
Clippy; the library's minimum supported Rust version is 1.92.
Run these commands from the repository root:

```powershell
cargo test --locked
cargo fmt --all -- --check
cargo clippy --locked -p unofficial-osmo-camera --all-targets -- -D warnings
```

The default workspace member is the base library, so these tests do not need
GStreamer installed.

### Protocol notes

Detailed wire contracts and compatibility evidence live in [PROTOCOL.md](PROTOCOL.md):

- [Parameter and command codecs](PROTOCOL.md#parameter-and-command-codecs):
  typed GET/SET payloads, framing, validation and unknown-value preservation.
- [Model capabilities](PROTOCOL.md#model-capabilities):
  model/firmware scope, source attribution and recording/live-preview candidates.
- [Preview command contracts](PROTOCOL.md#preview-command-contracts):
  endpoint distinctions, payloads and current lifecycle behavior.

These notes do not establish device availability or grant hardware authorization.
The setup and usage sections below retain their explicit operation approvals.

### Explicit RTMP controls (experimental)

`ControlSession` exposes `enable_rtmp`, `configure_rtmp`, `start_rtmp`,
`rtmp_state` and `stop_rtmp`. These use media recipient `0x08`, not the
camera parameter endpoint `0x01`. Payload codecs live under
`protocol::commands::media`; the write methods require explicit approval
on each call. Raw RTMP writes remain blocked; the exact running-state GET
is available through the typed query API.

```rust
use unofficial_osmo_camera::{
    ControlSession, Result, RtmpConfig, StreamingResolution,
};

fn configure_and_start(session: &mut ControlSession) -> Result<()> {
    let settings = RtmpConfig::new(
        "rtmp://example.invalid/live/operator-issued-key",
        StreamingResolution::FullHd1080,
        2000, // kbps: illustrative encoding value, not a recommended quality setting
    )?;
    session.configure_rtmp(&settings, true)?;
    session.start_rtmp(true)?;
    let state = session.rtmp_state()?; // raw state, not proof of receiver video
    let _ = state;
    Ok(())
}
```

Before that function, explicitly call `session.enable_rtmp(true)` to enter
broadcast mode. **Mode entry can change the camera's Wi-Fi role and break
the current control connection.** Establish/recheck an authorized uplink
and control session after mode entry before configuring and starting.
The API does not join networks, provide a recovery transport, reconnect
automatically, or run an RTMP receiver.

Use only a destination and publishing key you control. Plain `rtmp://`
and `rtmps://` addresses are encoded without a trailing NUL, with at most
244 bytes to avoid truncating the downstream record length. TLS operation
is unverified. Debug formatting redacts destination bytes; the settings
still contain the key in memory and on the control transport.

The high-level configuration uses default FPS, zero-valued unknown fields
and adaptive bitrate off. `RtmpConfig::new` requires a nonzero `u16`
`bitrate_kbps`; that encoding bound is not a supported operating range.
`.with_adaptive_bitrate(true)` requests its flag,
not a guaranteed adaptive policy. Resolution selectors 4/10 request the
720/1080 size families; rotation can swap width and height. No safe bitrate
range, delivered frame rate, 4K RTMP mode or working Nano publication is
promised. Local readiness and successful command replies are not evidence
of receiver acceptance: verify decodable camera frames, dimensions and
measured rate independently.

Call `session.stop_rtmp(true)` explicitly to stop and quit broadcast mode.
It is not a minimal pause and can affect recording and optional service
cleanup; prior Wi-Fi/recording state is not automatically restored.
Closing/dropping the control owner does **not** stop RTMP or reconnect
to do so. Preview and RTMP write lifecycles on the same typed owner are
mutually exclusive until explicitly stopped; low-level raw mode changes
are outside this bookkeeping. Failures are returned without an automatic
retry. Tests use synthetic payloads and an in-memory control worker;
hardware publication and external media-route acceptance remain unverified.

#### RTMP setup with `osmo-pair`

The CLI accepts the destination as a parameter and always performs full BLE
pairing and saved Wi-Fi association before each RTMP operation. Approve pairing
on the camera if prompted. Use a verified `camera_ip` in the selected profile.
These operations cannot be combined with preview diagnostics or `--lan-only`.

Run an RTMP receiver separately. For a local, single-publisher viewer, FFplay
supports RTMP listen mode (no GStreamer or separate server is needed):

```powershell
ffplay -listen 1 -timeout 120 -i "rtmp://0.0.0.0:31935/live/local-test-key"
```

Then, in another terminal at the repository root, replace the example address
below with the computer's camera-reachable LAN IP, not the camera IP or
`0.0.0.0`. Use the same publishing path/key:

```powershell
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair -- --camera camera --rtmp-setup "rtmp://192.0.2.20:31935/live/local-test-key"
```

`--rtmp-setup URL` explicitly approves entering broadcast mode, querying/logging
the nine opaque state bytes before configuration, configuring, starting, and
querying/logging state again. Defaults request the 720 size family at 2000 kbps;
use `--rtmp-resolution 1080` and/or `--rtmp-bitrate KBPS` to override. These are
experimental encoding choices, not established safe ranges or quality claims.
The application does not print or save the destination/key; **URL arguments
can still appear in shell history and process listings**.

Successful setup leaves publication running after the control owner closes,
without stopping or restoring camera state. Verify decoded video at the
receiver independently. The following commands each pair/reapply Wi-Fi first,
which can itself disrupt an existing stream:

```powershell
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair -- --rtmp-state
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair -- --rtmp-stop
```

State-query rejections report only their first status byte and reply length.
Setup failures attempt stop/quit on the same owner, explicitly report an
unconfirmed stop, close the owner and return failure. They do not reconnect,
retry RTMP commands, or recover the camera's network after broadcast-mode
entry. No working Nano publication is promised. Run `osmo-pair --help` for
options without connecting to hardware.

## Python testing wrapper

Requires Python 3.10+ and [uv](https://docs.astral.sh/uv/).
The uv workspace shares one `.venv` and `uv.lock` at the repository root,
including when commands target the Python package in its subdirectory.
Select `.venv\Scripts\python.exe` in VS Code on Windows (`.venv/bin/python`
on Linux/macOS). Use the repository-root environment; do not relocate virtual
environments, because they contain location-specific paths.
GStreamer is not required. Standard Python builds include the Rust BLE feature.
Builds without `ble` reject approved BLE-first startup with `UnsupportedError`,
not a LAN-only fallback. Linux builds with BLE require `libdbus-1-dev` and
`pkg-config`. BLE operations are explicitly opt-in at runtime.
uv uses the default `microsoft-python` package index
at `https://packagefeedproxy.microsoft.io/pypi/simple/`.

```bash
uv sync --locked --link-mode copy
uv run --project python/unofficial-osmo-camera --no-sync maturin develop --locked \
  --manifest-path python/unofficial-osmo-camera/Cargo.toml
uv run --project python/unofficial-osmo-camera --no-sync python -m unittest discover \
  -s python/unofficial-osmo-camera/tests
uv run --project python/unofficial-osmo-camera --no-sync ruff check python/unofficial-osmo-camera
uv run --project python/unofficial-osmo-camera --no-sync ruff format --check python/unofficial-osmo-camera
cargo fmt --manifest-path python/unofficial-osmo-camera/Cargo.toml -- --check
```

Repeat `uv sync --locked --link-mode copy` after tooling lockfile updates;
`--no-sync` uses the installed tools without updating them. Copy mode avoids
hardlink warnings when the cache and workspace are on different drives.

### Raw and typed protocol bindings

`osmo_camera.protocol.raw` exposes lossless DUML `Message` values, explicit
`MessageContext` framing and bounded `Request` exploration. Payloads and unknown
identities remain uninterpreted; frame decoding still checks lengths/checksums.
`osmo_camera.protocol.typed` exposes known wire contracts: capability topics,
subscriptions, replies, notifications, tables and open resolution/frame-rate
codes. "Typed" means verified wire layout and applicable envelope validation,
not verified support on every model or firmware. Model support/evidence belongs
in the Rust capabilities catalog. Generic resolution dimensions do not establish
camera support or live-preview dimensions.

Both layers use Rust codecs and the same native session owner. Typed requests
convert with `.to_raw(timeout=...)`; replies and notifications decoded from raw
frames retain those frames in `.raw` and return them with `.to_raw()`, including
transport sequence, flags, unknown values and opaque bytes. Body-only
notifications decoded with `.decode(body)` have `.raw is None` and require an
explicit `raw.MessageContext` for `.to_raw(context)`; headers are never invented.
Use explicit `osmo_camera.protocol.raw` and `osmo_camera.protocol.typed`
namespaces.
Encoding/decoding sends nothing, and payloads are not included in object
representations or codec errors.

For an already explicitly authorized, open `session`, the layers interoperate
without a second connection:

```python
from osmo_camera.protocol import typed

subscription = typed.CapabilitySubscribe(typed.CapabilityTopic.VIDEO_FORMATS, 1)
reply_frame = session.request_raw(
    subscription.to_raw(timeout=3.0), approve_unclassified=True
)
reply = subscription.decode_reply(reply_frame)
reply.require_success()
assert reply.to_raw().encode() == reply_frame.encode()
```

`session.subscribe(subscription, approve_unclassified=True)` uses that same
raw exchange path and returns the typed reply directly. Raw requests still use
Rust's exact known-read allowlist, 0.1..30-second timeout bounds and session-level
plus per-request approvals for unclassified operations. Media-write restrictions
cannot be bypassed by choosing the raw layer.

### Capability enumeration

`ControlSession.connect(profile_path, approved=True, allow_unclassified=True)`
loads an existing caller-selected camera profile through Rust. Connecting
performs LAN pairing/app registration and a country GET; it does not discover a
camera, join a network, initialize BLE, start preview or change capture mode.
The profile must already contain a verified LAN address and pairing identity.
Use the session as a context manager for explicit teardown. Blocking operations
release the GIL; a `Cancellation` token can be signaled from another thread.
Native failures surface as `CameraError` subclasses, not successful empty results.

To explicitly prepare the camera before LAN control, opt into BLE-first startup:

```python
from osmo_camera import ControlSession

with ControlSession.connect(
    profile_path,
    approved=True,
    allow_unclassified=True,
    ble_first=True,
    approve_ble=True,
) as session:
    # Use raw or typed requests on this same native owner.
    ...
```

Every enabled startup begins with BLE discovery, saved-identity pairing/wake
and a successful live health reply. A saved camera Wi-Fi MAC is checked when
present, before associating with the saved network. Rust verifies another BLE
health reply, explicitly closes BLE, and waits at most 10 seconds for the saved
LAN pairing endpoint before opening one LAN control session. The native ownership
lease is retained across the transport handoff. Pairing approval on the camera
may still be needed. It does not reset credentials, modify the
profile, switch the computer's network, or retry a failed control connection.
The profile must already contain a verified LAN address, pairing identity and
complete saved Wi-Fi settings. Initial pairing remains a separate explicit step.
`CameraProfile::connect_control_via_ble` supplies this lifecycle to native callers;
`CameraConfig` connections remain LAN-only. Rust lifecycle logging reports stages
without publishing network identifiers or credentials.
BLE-first startup is not an automatic recovery loop or a guarantee for every
power/idle state. A failed stage surfaces its error; another attempt needs
explicit caller action.

### Unified capability view

`osmo_camera.capabilities` exposes Rust-backed model metadata and immutable
camera evidence, without duplicating the catalog in Python:

```python
from osmo_camera.capabilities import (
    Availability,
    CapabilityCatalog,
    CaptureMode,
    resolve_capabilities,
)

catalog = CapabilityCatalog.nano()  # Explicit model selection, not detection.
view = resolve_capabilities(catalog=catalog, mode=CaptureMode.VIDEO)
assert view.recording_formats.state is Availability.MISSING
assert view.recording_formats.current is None
candidates = view.recording_formats.published
```

This example is entirely offline. Pass decoded `CapabilityNotification` values
as `resolve_capabilities(snapshots, ...)` to attach camera evidence without
opening a connection. For an already authorized, open `session`, collect through
the same native owner:

```python
from osmo_camera.capabilities import CapabilityCatalog, CaptureMode
from osmo_camera.enumerate_capabilities import collect_capabilities

view = collect_capabilities(
    session,
    catalog=CapabilityCatalog.nano(),
    mode=CaptureMode.VIDEO,
    timeout=15.0,
    approve_subscriptions=True,
    approve_acknowledgments=True,
)
current = view.recording_formats.current
candidates = view.recording_formats.published
```

Each successful call returns one immutable `Capabilities` view. Missing-topic timeouts
produce incomplete views, not a different return type or a successful static
fallback. Transport failures, rejections, cancellation and malformed data raise.
The native control owner retains inbound frames across subscription exchanges,
idle ticks and receipt ACKs. Its queue is bounded to 1,000 messages; callers
must consume notifications, and overflow raises rather than silently dropping
evidence.
The three topic views are `recording_formats`, `slow_motion_ratios` and
`capture_aspects`; `view.topics` iterates them in that order. Each retains its
original `.notification`, including `.notification.raw` when decoded from a
framed message.

| Topic state | `.current` | Meaning |
|-------------|------------|---------|
| `Availability.MISSING` | `None` | No notification received; support is unconfirmed |
| `Availability.AVAILABLE` | Tuple of rows, possibly empty | Camera reports an available table in its current state |
| `Availability.UNAVAILABLE` | Empty tuple | Camera explicitly reports this topic unavailable in its current state |
| `Availability.UNKNOWN` | `None` | Notification received with an unknown availability value; `.error` retains the interpretation failure |

`.published` stays separate even when a live table excludes those candidates.
`view.complete` is true only when every topic is available or explicitly
unavailable; it does not mean every format is supported or tested.
`view.missing` lists topics with no notification, while `view.unresolved` also
includes unknown availability. Check `.state`, not just whether `.current` is
empty. Duplicate known-topic snapshots require explicit caller selection, not
guessed sequence chronology. Offline resolution retains unknown-topic
notifications in `.other_notifications`; live collection rejects unknown topics
explicitly rather than treating them as one of the three required snapshots.

Without mode context, published recording candidates retain all their individual
mode scopes. `CaptureMode` preserves unknown byte codes in `.code` and exposes
compatibility labels in `.name`. Selecting a mode filters only the published
recording-format candidates: it neither reads nor changes the actual camera
mode, and it does not filter camera-reported rows. Slow-motion candidates retain
their catalog scope and are derived from Rust's published recording formats;
each candidate keeps its contributing `.formats`. Published recording aspects are **not**
mapped onto raw aspect codes: that wire equivalence is not established.

`view.command_coverage` retains access, evidence and typed firmware scope
(`MATCHES`, `DIFFERENT`, `UNKNOWN`, `UNSPECIFIED`). A scope match is not device
confirmation or write permission. `view.preview_observations` contains published
observations, never stream measurements or selectable output modes. Source URLs
remain on `catalog.sources`. Model and firmware context are never inferred.
Pass a non-empty `firmware` string for exact comparison with a command's stated
scope; omit it when unknown. Context validation occurs before subscriptions.

This Python API intentionally has no backward-compatibility aliases. Import
messages and requests through `protocol.raw`, known contracts through
`protocol.typed`, and decode notifications with `CapabilityNotification.from_raw`.

The opt-in runner subscribes to all three known capability topics and
validates recording-format, slow-motion and raw aspect rows using Rust:

```bash
uv run --project python/unofficial-osmo-camera --no-sync python -m osmo_camera.enumerate_capabilities \
  --profile .credentials/example.toml --timeout 15 \
  --approve-registration --approve-subscriptions --approve-acks
```

Add `--catalog nano`, optionally `--capture-mode 0x01 --firmware VERSION`, to
include explicitly scoped published candidates in the CLI output.
Add `--ble-first --approve-ble` to prepare through BLE before each enumeration.
Both flags are required together and additionally authorize camera Wi-Fi
association; the existing registration/subscription/ACK approvals remain required.

Run only with explicit device authorization, on the already configured LAN,
after closing every other camera owner. No hardware operations occur on import
or during normal tests. The three approval flags authorize registration,
subscriptions and per-notification receipt ACKs respectively. Subscriptions may
outlive the session; no unsubscribe contract is established. The enumeration
window is bounded to 1..30 seconds after connection, and native connection and
cleanup bounds remain in force. No reconnect, mode switch or metadata fallback
occurs. Missing topics remain explicit in the returned capability view.
The runner exits 0 only for a complete view, 1 for incomplete/unknown results or
camera errors, and 2 for invalid arguments before connecting. It prints partial
observations when available; published candidates never count as received topics.
Unavailable and available-empty tables are distinct. Notifications are matched
by topic name, not an assumed mapping from
sequence to subscription ID. Snapshots are not atomic across mode changes.
The output includes decoded rows, state/errors and optional published-candidate
counts, not raw captures, addresses or pairing data.

Native `.pyi` declarations are generated from the PyO3 binding annotations with
the optional, MIT/Apache-2.0 `pyo3-stub-gen` dependency. It is not enabled in
normal extension builds. Regenerate after binding changes and check for drift:

```bash
uv run --project python/unofficial-osmo-camera --no-sync cargo run --locked \
  --manifest-path python/unofficial-osmo-camera/Cargo.toml --features stubgen --bin stub-gen
uv run --project python/unofficial-osmo-camera --no-sync cargo run --locked \
  --manifest-path python/unofficial-osmo-camera/Cargo.toml --features stubgen --bin stub-gen -- --check
```

The generated native stub is excluded from Ruff's source formatting; its exact
generated contents and native export/property coverage have separate checks.

Re-run `maturin develop` after changing Rust code. For a distributable wheel
and source archive:

```bash
uv run --project python/unofficial-osmo-camera --no-sync maturin build --locked \
  --release --sdist --manifest-path python/unofficial-osmo-camera/Cargo.toml --out dist
```

Before publication, also verify that the source archive builds independently.
Extract it outside the checkout, then run `maturin build --locked --release`
**from the extracted archive root**, which contains its generated
`pyproject.toml`. That file selects the nested standalone Cargo manifest and
Python source directory. Building the archive's workspace-root Cargo manifest
directly is not the Python build entry point. Use the existing uv-managed
maturin executable; do not initialize a second project or change package feeds.
Install the resulting wheel into a separate `uv venv` and run the same offline
test suite without the editable package. Review both archive and wheel contents
for notices, generated stubs, local evidence, caches, debug symbols and private
metadata paths; successful compilation alone is not a release audit.

## GStreamer plugin

Requires the GStreamer **runtime and development SDK**, version 1.24+.
On Windows, run `setup.cmd` from an existing Command Prompt. It installs
missing build tools and the GStreamer SDK, configures the current shell, and
syncs the locked Python environment. Some prerequisites, including pkgconf's
machine-wide installer, may prompt for administrator approval. Setup stops if
an installer fails or is cancelled; approve the required installation and rerun
setup to continue.

```cmd
setup.cmd
```

The Windows setup regression tests run without installing software or accessing
hardware: `powershell -NoProfile -File .\scripts\test-setup.ps1`.

The default per-user install location is
`%LOCALAPPDATA%\Programs\gstreamer\1.0\msvc_x86_64`. If GStreamer is already
installed elsewhere, set `GSTREAMER_ROOT_X86_64` to that location before
running setup.

On Ubuntu or Debian (including WSL), run `bash ./setup.sh` as your normal user
to install build dependencies and the pinned Rust toolchain. It uses sudo for
apt packages. On other Linux distributions, install the GStreamer and
plugins-base development packages and `pkg-config`; BLE builds also require
the D-Bus development libraries. Both setup scripts are safe to rerun.

```powershell
cargo build --locked -p gst-plugin-unofficial-osmo
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
$env:GST_PLUGIN_PATH = (Resolve-Path .\target\debug).Path
gst-inspect-1.0 osmosrc
```

Windows builds produce `target\debug\gstosmo.dll`; Linux builds produce
`target/debug/libgstosmo.so`. Plugin tests do not require camera hardware.

### Viewing the live stream

From the repository root, run the pairing command. On first use it prompts for
any missing Wi-Fi SSID and password (password input is hidden), then stores all
camera settings in `.credentials\camera.toml`. Use `--camera PROFILE` to keep
separate profiles for multiple cameras. It waits for any required approval on
the camera and configures the camera to join that Wi-Fi network; it does not
change the computer's Wi-Fi settings:

```powershell
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair -- --camera office
```

Verify the saved `camera_ip` before streaming; automatic address lookup is
best-effort on Windows and Linux and is unavailable on macOS. If necessary,
check the router's DHCP leases using the reported camera MAC and set
`camera_ip` in the profile. Profiles contain plaintext credentials: keep them
local and never commit or share them.

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

After building the plugin, Windows users can run `view-nano.cmd` to open the
live viewer using the saved `camera`
profile after asking for startup approval. It locates the installed GStreamer
runtime and debug plugin automatically, even when launched from another
directory. Use `view-nano.cmd office` for another profile or
`view-nano.cmd --check` to check prerequisites without connecting. It does not
install dependencies or build the plugin.

The equivalent manual pipeline, from a configured Command Prompt at the
repository root, is:

```cmd
set GST_PLUGIN_PATH=%CD%\target\debug
gst-launch-1.0 -v osmosrc camera-profile=camera allow-writes=true ! h264parse ! avdec_h264 ! videoconvert ! queue ! autovideosink sync=true
```

Starting this pipeline connects to the camera and performs the explicitly
approved preview-session writes; plugin inspection does not. Stop the pipeline
with Ctrl+C to close the session cleanly. On Windows, `d3d12videosink sync=true`
can replace `autovideosink sync=true`.

`allow-writes=true` also authorizes BLE initialization and reapplying the
profile's Wi-Fi settings if the saved LAN pairing port is unreachable.

The viewer needs the H.264 parser, software decoder and selected video sink
plugins in addition to the development SDK. On Debian/Ubuntu, install
`gstreamer1.0-plugins-bad`, `gstreamer1.0-libav` and
`gstreamer1.0-plugins-good` for the example pipeline; `setup.sh` prepares
build/test dependencies, not every optional playback plugin.

### Diagnostics

To diagnose camera LAN preview startup without opening the GStreamer viewer,
run the pairing utility's opt-in smoke test:

```powershell
$env:RUST_LOG = "unofficial_osmo_camera=debug,osmo_pair=info"
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair -- --smoke-test
```

It uses the selected profile (add `--camera PROFILE` if needed), receives three
H.264 access units, and closes the session. This checks raw video delivery,
not GStreamer decoding or display. Both diagnostics may initialize over BLE,
reapply Wi-Fi settings, and save a new pairing identity when LAN is unreachable.
Add `--lan-only` to prohibit BLE initialization, Wi-Fi changes, and profile
updates. Failures do not trigger automatic reconnection.

For a longer timing diagnostic, run:

```powershell
$env:RUST_LOG = "osmo_pair=debug,unofficial_osmo_camera=info"
cargo run --locked -p unofficial-osmo-camera --features ble --bin osmo-pair -- --timing-test 60
```

The duration is 1-300 seconds after startup. This logs video arrival timing and
gaps without decoding, displaying, or saving video.

### Experimental preview configuration diagnostic

With explicit authorization to change preview settings on a real camera and
`ffprobe` on PATH, the opt-in example compares a baseline, three quality
settings, and five stream-output settings:

```powershell
cargo run --locked -p unofficial-osmo-camera --features ble --example preview-configs -- camera --approve-experimental-writes
```

It reports dimensions, arrival FPS, bitrate, and gaps without saving captures.
The writes are experimental: requested settings and replies do not establish
device acceptance. No LCD or recording-format settings are changed.
Final requests select Normal/Auto, **not restoration of the original settings**.
Errors stop further configuration writes and attempt session teardown;
settings can remain changed. The same profile-based BLE initialization
behavior applies.

### Logging

Use `RUST_LOG` for CLI diagnostics or set `GST_DEBUG=osmo-camera:6` before
launching a GStreamer pipeline. Logs go to stderr or the host logger, not
automatically to files. Review logs before sharing them.

At `unofficial_osmo_camera=debug`, BLE and LAN control diagnostics include
every decoded/sent message's direction, numeric endpoints, command, ID, flags
and payload length, including unmatched replies and notifications.
At `unofficial_osmo_camera=trace`, they also include BLE packet lengths and
LAN UDP packet types, lengths, sequence numbers and elapsed timing. Logger
timestamps provide BLE timing. These metadata diagnostics exclude payload
bytes, pairing identities, RTMP addresses/keys and network addresses; they are
not raw traffic captures. A sent message is not proof of camera acceptance.
The host must enable the log level and supply timestamps; for CLI runs, use
`RUST_LOG=unofficial_osmo_camera=trace,osmo_pair=info`.

### Current limitations

- Targeted compatibility is Osmo Nano; no firmware support matrix is certified.
- LAN reachability does not guarantee that preview startup will succeed.
- Video queue overflow stops the session rather than silently dropping frames.
- Camera pauses remain visible. Timestamps reflect receiver arrival time,
  not camera capture time, and cannot reconstruct missing video.
- Cleanup can fail or time out; startup, streaming and cleanup failures are
  reported separately. No failure triggers an automatic reconnect.

## Development and CI

[CI](.github/workflows/ci.yml) runs hardware-free Rust and Python tests on
Linux, Windows, and macOS, with GStreamer checks on Linux. It also checks
formatting, linting, documentation, and the Rust minimum supported version.

Keep Cargo lockfiles and `uv.lock` checked in and use locked resolution.
Run the relevant checks above before submitting changes. See the
[contributor rules](AGENTS.md) for component-specific requirements.

## Releases

[Release automation](.github/workflows/release.yml) runs when an authorized
maintainer pushes a `vX.Y.Z` tag. It publishes `unofficial-osmo-camera` and
`gst-plugin-unofficial-osmo` to crates.io, then publishes a GitHub Release containing
the two `.crate` source archives, checksums, and native archives for:

- Windows x64 MSVC: `gstosmo.dll` and `osmo-pair.exe`.
- Linux x64 and ARM64: `libgstosmo.so` and `osmo-pair`.
- macOS Intel and Apple Silicon: `libgstosmo.dylib` and `osmo-pair`.

Native archives include the plugin under `lib`, the pairing utility under
`bin`, [README.md](README.md), [PROTOCOL.md](PROTOCOL.md), licenses, third-party
notices, and `BUILD-INFO.txt` with runtime requirements. GStreamer itself and
Python wheels are not included.

Maintainers must configure protected release tags and the `CargoPublish`
environment with approval and a narrowly scoped `CARGO_REGISTRY_TOKEN`.
Before approving publication, align crate versions and lockfiles, run the
component checks, and audit the exact source and native artifacts for
provenance, licenses, secrets, and private paths. A successful build is not
legal or device-compatibility clearance.

See the [release workflow](.github/workflows/release.yml) for publishing and
retry behavior. Publishing is not transactional; some artifacts may already
be public if a later step fails. Offline release tests are available via
`bash scripts/test-release.sh` (Git Bash on Windows).

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
$env:GST_PLUGIN_PATH = (Resolve-Path .\osmo-gstreamer-0.2.0-x86_64-pc-windows-msvc\lib).Path
gst-inspect-1.0 osmosrc
```

Inspection does not connect to hardware. Run the extracted pairing utility
from your chosen working directory only when explicitly authorizing camera
setup; profiles remain local under `.credentials`.
