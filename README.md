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

The `02:8E` parameter codec exposes `CameraParameter::NANO_PARAMETERS` for
the Nano 10.00.31.22 profile: 37 non-placeholder IDs, including 29 GET and
33 SET cases. `nano_access()` distinguishes operation coverage;
`single_nano()` rejects legacy-only and write-only GETs. Existing legacy
names remain compatibility APIs, not evidence of Nano support. Value
meanings, safe SET payloads and current-state availability remain separate
questions; no new automatic requests or writes are introduced.

## Python testing wrapper

Requires Python 3.10+ and [uv](https://docs.astral.sh/uv/).
GStreamer is not required. uv uses the default `microsoft-python` package index
at `https://packagefeedproxy.microsoft.io/pypi/simple/`.

```powershell
uv sync --locked --project .\python\unofficial-osmo-camera
uv run --project .\python\unofficial-osmo-camera --no-sync maturin develop --locked --manifest-path .\python\unofficial-osmo-camera\Cargo.toml
uv run --project .\python\unofficial-osmo-camera --no-sync python -m unittest discover -s .\python\unofficial-osmo-camera\tests
uv run --project .\python\unofficial-osmo-camera --no-sync ruff check .\python\unofficial-osmo-camera
uv run --project .\python\unofficial-osmo-camera --no-sync ruff format --check .\python\unofficial-osmo-camera
cargo fmt --manifest-path .\python\unofficial-osmo-camera\Cargo.toml -- --check
```

Re-run `maturin develop` after changing Rust code. For a distributable wheel
and source archive:

```powershell
uv run --project .\python\unofficial-osmo-camera --no-sync maturin build --locked --release --sdist --manifest-path .\python\unofficial-osmo-camera\Cargo.toml --out .\dist
```

## GStreamer plugin

Requires the GStreamer **runtime and development SDK**, version 1.24+.
On Windows, run `setup.cmd` from an existing Command Prompt. It installs
missing build tools and the GStreamer SDK, configures the current shell, and
syncs the locked Python environment. Some prerequisites may require elevation.

```cmd
setup.cmd
```

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

After building the plugin, run this from the configured Command Prompt and
repository root:

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
