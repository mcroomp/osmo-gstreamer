# osmo-gstreamer for DJI Osmo Nano

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
| Rust library | [crates/unofficial-osmo-camera](crates/unofficial-osmo-camera) | Protocol, transport, camera ownership and lifecycle |
| Python wrapper | [python/unofficial-osmo-camera](python/unofficial-osmo-camera) | Thin PyO3/maturin bindings for testing the Rust library |
| GStreamer plugin | [crates/gst-plugin-unofficial-osmo](crates/gst-plugin-unofficial-osmo) | Adapt the Rust library to a GStreamer source |

The Rust library supports LAN preview and optional BLE setup for Osmo Nano.
The GStreamer element is `osmosrc`; the Python testing wrapper imports as
`osmo_camera` and currently exposes only `version()`.

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

Model and firmware coverage lives separately from command codecs in
`protocol::capabilities`. `NANO_10_00_31_22` includes 37 non-placeholder camera
parameter IDs (29 GET and 33 SET cases), plus 20 exact read contracts across
camera, Wi-Fi, common and media commands. Parameter entries retain the targeted
profile evidence; other entries are explicitly compatibility contracts, not
verified firmware availability. `lookup(endpoint, command, selector)` returns
operation coverage and evidence. `require_query::<Get<T>>()` rejects
legacy-only and write-only parameter reads for this profile. Generic
`Get<T>` and `Set<T>` use the same `Parameter` bounds without enforcing operation
support. Existing legacy
names remain compatibility APIs, not evidence of Nano support. Value
meanings, safe SET payloads and current-state availability remain separate
questions; no new automatic requests or writes are introduced.

Each parameter has one payload struct shared by GET and SET, such as
`EisValue` or `RotationStateValue`. `Get<T>` fixes the requested ID and response
type; `Set<T>` privately holds a checked payload of the same type. There is no
large parameter-value enum, optional selector/value union, or separate SET
payload type. Callers are responsible for selecting supported operations and may
implement the parameter traits for custom payloads; this does not grant hardware
authorization or extend the raw-read allowlist. Callers may define independent
`CameraCapabilities` tables for other model/firmware scopes; missing entries
remain unknown and never inherit Nano support. Profile selection is explicit,
not automatic device detection or a persisted credential-profile setting.

GET replies are `CommandReadReply<ParameterReply<T>>`: accepted replies expose
`value: T`; rejected replies retain status and opaque extensions. The accepted
envelope requires operation 0, count 1, the type's echoed ID and its established
payload layout. Unknown codes remain raw, but unexpected widths/layouts now
return errors instead of silently falling back to an untyped value.
Raw exploration remains explicit and approval-gated; it is not a typed fallback.
The fixed legacy two-ID request is `AdvancedLiveReversalGet`, with an
`AdvancedLiveReversalReply` response containing `advanced_mode` and opaque
`legacy_value` fields. No runtime-selected batch API is exposed.
Each parameter value struct owns its complete TLV, with the parameter ID declared as
binrw `magic` and a checked or calculated length. GET selectors obtain the ID
through `Parameter::ID`, defaulted from binrw's `WriteMagic` trait.
The separate capability table records model-specific command/selector coverage
without coupling codecs to a model; it uses `Parameter::ID` for parameter entries
and `Query` constants for fixed read requests.
Wire serialization remains explicit and
little-endian, not a memory-layout cast.

`RotationStateValue` (`0014`) is read-only.
Its `RotationStateValue` contains
`reserved_before`, `orientation_code`, and `reserved_after`; the targeted
layout is `[0, code, 0]`. This derived orientation is not a raw rotation-lock
configuration or sensor reading. Physical direction/degree mappings remain
unspecified, unknown codes remain raw, and no autorotation SET is exposed.
The former `mode_code`/`device_angle_code`/`sensor_angle_code` field names
have been replaced to avoid implying those unsupported meanings.

Binary command payloads, numeric wire types, DUML/UDP framing, ACKs and media
headers use `binrw`. Field types own their wire representation; fixed layouts
use derives, while checksum coverage, request-dependent validation and bitmap
algorithms remain explicit. Closed numeric enums use `#[brw(repr = u8)]`
(or `u16`); open enums use variant `magic` attributes followed by a raw
`Unknown(u8)` fallback. Unknown codes and opaque extensions stay lossless.
Fixed command layouts can use `magic`; derived writers calculate length
prefixes without exposing them as caller-settable fields. Maps remain useful
for lossless raw representations and checked conversions.
The module-version reply reads its fixed fields directly; `product_label()`
names the text accurately, while `hardware()` remains a compatibility alias.

There are no project-specific reader/writer types. Use
`BinRead`/`BinWrite` with standard cursors for composition. The free boundary
functions `protocol::encode(&value)` and `protocol::decode::<T>(bytes, args)`
use those traits directly with little-endian defaults. Encoding returns only
complete encoded values; decoding rejects trailing bytes. Arguments are native binrw
arguments: `()` for ordinary payloads or `(&request,)` for contextual replies.
Typed DUML command structs encode the contiguous `[command_set, command_id, payload...]`
DUML body. Their binrw magic contains only `[command_set, command_id]` and
supplies the default `CommandPacket::COMMAND_ID`. Fixed payload bytes are separate
calculated fields; fixed read payload constants also feed capability matching.
Default `CommandPacket` and `Query` trait methods derive routing identity,
construct requests, expose payload-only bytes
for raw APIs, and decode replies while checking their command identity.
`MessageContext` supplies sender, receiver, sequence and flags independently
of the body; DUML framing calculates length and checksums and composes the
command body once.
`Get<T>` and `Set<T>` include the camera command prefix as well as their parameter
envelope. The application device-info response also encodes its command prefix.
Startup/write requests, BLE keepalive/setup and notifications follow the same
command-only magic structure. Raw BLE pairing-arm bytes are a GATT characteristic
write, not a DUML command, and deliberately have no command prefix. Each command
module starts with module documentation, followed by imports, then the main
command struct's documentation and declaration, before supporting value types.

Typed `Query::Response` is the complete command reply. GET and Reply structs
declare the same two-byte command-only magic; `Query` checks their agreement
and decodes the complete reply without adding another prefix. Header metadata
remains separate in `MessageContext`, and status is an ordinary reply field,
not magic. Command-specific accepted fields live directly in `…Reply` structs,
without redundant `…Data` holders. `CommandReadReply<T, R = RemainingBytes>` wraps
them in `Accepted(T)` or `Rejected { status, extension }`: rejections preserve
opaque bytes by default, with a custom rejection type only for a useful,
established layout. Genuinely reused parameter, accessory and media data types
remain separate. `CommandReply<C, T>` remains available for raw write acknowledgments.
Paired read-command types use matching descriptive `…Get` and `…Reply` names
(for example, `CameraAudioGet` and `CameraAudioReply`). Write requests and
reusable parameter value types retain their operation-specific names. Sessions use
`execute_query::<Q>()` with no request argument. Direct `protocol::encode(&query)`
now returns command bytes plus payload; use `query.payload()` only when an
explicitly raw API already takes the command separately.
Command declarations and opaque response structs are explicit; the former
`camera_read_query!` and `camera_data!` generation macros are removed.
Command codecs use native binrw fields, calculated constants and lengths,
and field assertions rather than mapping entire structs through wire tuples.
Opaque replies keep their raw byte accessors and uninterpreted extensions.
`RemainingBytes<const MIN: usize = 0>` reads opaque tails through EOF of a bounded
payload reader and enforces the minimum on construction, decoding and encoding.
Private storage is exposed through `as_slice()` or consumed with `into_vec()`.
`RemainingBytes` allows empty tails/rejections; `RemainingBytes<1>` requires data.
Larger fixed minimums use the same type; embedded lengths/counts retain explicit
command-specific checks. Construct values with `RemainingBytes::<MIN>::new(bytes)`.
Custom codecs remain for shared contextual replies and checked packed strings;
accessory and media-state alternatives use derived binrw enums. Media rejections
read directly through EOF with branch assertions; calibration-file records share
their nested data-length calculation between construction and wire codecs.

Numeric enums no longer provide generated primitive `From`/`Into`/`TryFrom`
implementations, and no numeric-enum conversion dependency is required.
Unsupported closed-enum codes and malformed native enum variants produce
`Error::WireCodec`, including malformed accessory and media-state layouts.
Branch errors remain available in the codec error's source; directly raised
domain validation errors retain their existing library variants. Codec errors retain their source
without displaying potentially sensitive details in the top-level message.
Frame lengths and counts remain checked before reading variable bodies.
FEC, incremental framing and H.264 bitstream algorithms remain explicit.

```rust
use unofficial_osmo_camera::protocol::commands::camera::cmd_02_8e_parameters::{
    EisValue, Get, ParameterReply, Set,
};
use unofficial_osmo_camera::{
    ControlSession, Result,
    protocol::commands::reply::CommandReadReply,
};

fn read_setting(session: &mut ControlSession) -> Result<CommandReadReply<ParameterReply<EisValue>>> {
    session.execute_query::<Get<EisValue>>()
}

fn write_setting(session: &mut ControlSession, approved: bool) -> Result<()> {
    session.execute_parameter_set(Set::new(EisValue::new(7)?), approved)
}
```

Each writable payload has a `new` constructor beside its struct,
such as `AddCustomModeValue::new(raw_index, operation)`. These constructors
are for SET callers only: named arguments build the supported record, including
its canonical tags and reserved fields. `Set::new(value)` is an infallible
wrapper: it preserves fields without validation or normalization. Callers
using public fields or GET values directly are responsible for SET validity.
Binrw owns GET decoding and the wire layout; it does not call `new` or apply
SET restrictions.
Multi-field records expose named fields rather than opaque byte arrays:
TC uses `TcData::Byte`/`Word`, beauty uses fifteen raw effect values, and
calibration uses `CalibrationName::new` plus an operation or focal code.
Raw types without a `Parameter` implementation cannot instantiate `Set<T>`.
Operation metadata describes established support, not permission to execute.
Use `Set::value()` or `into_value()` to access
the payload. Neither wrapping nor encoding reapplies constructor checks;
wire-length and layout errors still surface during encoding.
`ControlSession::execute_parameter_set` requires explicit approval on each call
and accepts only an exact one-byte successful ACK; rejections and malformed
replies are errors. It does not retry, reconnect or confirm the setting's effect.
The old `CameraParameter`, `CameraParameterValue`, `CameraParametersGet`,
`CameraParameterSet` and parameter-specific `set` wrappers are removed.

Capability reads also have independent query types: `EiRangeGet`,
`RecommendedEiGet`, `LensInformationGet` and `AccessoriesGet`.
For example, `session.execute_query::<AccessoriesGet>()` returns
`AccessoriesReply`; the first three return status-bearing replies with
distinct opaque, nonempty value types. Accessories retain outer and inner
rejections separately and validate operation/selector before interpreting the
inner status. Outer rejection uses the shared `CameraReadReply::Rejected`;
inner rejection is `CameraReadReply::Accepted(AccessoriesData::Rejected { .. })`.
The capability query and value definitions are explicit structs without custom
macros. The previous `CapabilityQuery` selector enum is removed.
Both LAN and BLE typed query entry points now take only the query type.

All 37 targeted parameter entries document their value shape or an explicit unknown.
There are **33 available checked SET codecs**, with restricted subsets of some
write branches. Fixed record widths are expressed by the binrw payload types.
TC records have two possible widths, selected by their second byte.
Covered writes include
raw audio/mirror codes, bounded image/shutter/recording controls, tagged
autofocus records, sparse `001F`/`003C` codes and capture-aspect values.
Capture-aspect SET accepts only 0..=255 despite its four-byte wire width.
`BeautyEffectValue` carries the complete 62-byte indexed record; checked SET
requires the canonical count, indices and one-byte entries.
Individual effect labels/scales remain unknown, and partial beauty patches
are not exposed.

Additional checked shapes are duration's five four-byte raw codes;
the flag-1 nine-byte short-video record;
TC tag 1 with selector 1..=3 and one byte, or selector 4 and four bytes;
custom-mode `[raw_index, operation_0_to_3]`; storage-sync start
`[source_0_to_3, destination_0_to_3, 1]` or canonical stop `[0, 0, 2]`;
motionlapse preview/interrupt `[raw_code, 0_or_1]`; and a raw motionlapse
mode byte. These describe byte contracts, not physical mode labels,
calibration procedures or completion guarantees. Duration SET zero-extends
its low word as a host choice; arbitrary duration GET bytes are not replayable.
Short-video added-duration writes remain outside the checked subset.

Camera `0019` exposes only an 11-byte zero-destination streaming-parameter
trigger: all bytes zero except byte 3 equal to 1. This camera path does not
forward a destination, resolution or bitrate; the codec does not configure,
start or stop RTMP. Other profile variants remain unchecked.

Calibration `002B` permits only action 3's raw focal-length record: zero
metadata, a byte-length-prefixed NUL-terminated label, LE16 extra length 4,
raw LE16 focal code and two zero bytes. Units and supported codes are unknown.
Actions 0..=2 remain blocked because their native selectors can terminate the
provider process. `002D` permits actions 0/2 with one name, or 1 with two names,
behind an exact LE16 data length. Names are nonempty printable ASCII, at most
127 characters each; total values cannot exceed 255 bytes. These are byte-level
checks, not a calibration/file-operation procedure, path policy or provider
availability guarantee. No apply/rename/delete meanings are assigned.

The four GET-only IDs cannot become SETs. Unknown codes in supported GET layouts
remain raw without granting a checked write; unsupported layouts are explicit errors.
Duration GET retains all four bytes, but only its low word has
an established conversion. Known widths/codes never imply current-mode
availability, physical semantics, safe retries or an applied setting.

Checked constructors include recording EIS
(`EisValue::new`, 0..=7), independent live-view EIS (`LiveViewEisValue::new`, 0..=7),
stabilization preference (`StabilizationPreferenceValue::new`, 0/1), and stream
quality (`StreamQualityValue::new`, 0/1/2). Mode labels and availability are not
inferred from those ranges. To encode a boolean compatibility value, pass
`u8::from(enabled)` to the relevant constructor; EIS codes 0/1 are not a physical
on/off guarantee. Nano field-of-view SET codes are
restricted to 0..=5 and voice-language SET codes to 0/1; invalid values now
return an error at construction. GET decoding continues to preserve
unknown codes without applying SET limits. These APIs only encode payloads;
they neither send requests nor authorize changing a camera setting.

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
and adaptive bitrate off. `.with_adaptive_bitrate(true)` requests its flag,
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
on Linux/macOS). An older nested `.venv` is no longer used; do not move it,
because virtual environments contain location-specific paths.
GStreamer is not required. uv uses the default `microsoft-python` package index
at `https://packagefeedproxy.microsoft.io/pypi/simple/`.

```powershell
uv sync --locked --link-mode copy
uv run --project .\python\unofficial-osmo-camera --no-sync maturin develop --locked --manifest-path .\python\unofficial-osmo-camera\Cargo.toml
uv run --project .\python\unofficial-osmo-camera --no-sync python -m unittest discover -s .\python\unofficial-osmo-camera\tests
uv run --project .\python\unofficial-osmo-camera --no-sync ruff check .\python\unofficial-osmo-camera
uv run --project .\python\unofficial-osmo-camera --no-sync ruff format --check .\python\unofficial-osmo-camera
cargo fmt --manifest-path .\python\unofficial-osmo-camera\Cargo.toml -- --check
```

Repeat `uv sync --locked --link-mode copy` after tooling lockfile updates;
`--no-sync` uses the installed tools without updating them. Copy mode avoids
hardlink warnings when the cache and workspace are on different drives.

Re-run `maturin develop` after changing Rust code. For a distributable wheel
and source archive:

```powershell
uv run --project .\python\unofficial-osmo-camera --no-sync maturin build --locked --release --sdist --manifest-path .\python\unofficial-osmo-camera\Cargo.toml --out .\dist
```

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
`bin`, licenses, third-party notices, and `BUILD-INFO.txt` with runtime
requirements. GStreamer itself and Python wheels are not included.

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
