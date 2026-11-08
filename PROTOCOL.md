# Protocol notes

These original notes describe this unofficial project's wire contracts and API
choices, not an official vendor specification. Encoding compatibility is not
evidence of model/firmware availability, safe settings or operation authorization.
Follow the [interoperability policy](INTEROPERABILITY.md) and
[contributor rules](AGENTS.md). No raw captures, credentials or device identifiers
belong in these notes.

See the [README](README.md) for installation, Python APIs, camera ownership,
explicitly authorized startup and experimental RTMP usage.

- [Parameter and command codecs](#parameter-and-command-codecs)
- [Model capabilities](#model-capabilities)
  - [Recording formats and live-preview capabilities](#recording-formats-and-live-preview-capabilities)
- [Preview command contracts](#preview-command-contracts)

## Parameter and command codecs

Each parameter has one payload struct shared by GET and SET, such as
`EisValue` or `RotationStateValue`. `Get<T>` fixes the requested ID and response
type; `Set<T>` privately holds the supplied payload of the same type. There is no
large parameter-value enum, optional selector/value union, or separate SET
payload type. Callers are responsible for selecting supported operations and may
implement the parameter traits for custom payloads; this does not grant hardware
authorization or extend the raw-read allowlist. Callers may define independent
`CameraCapabilities` tables for other model/firmware scopes; missing entries
remain unknown and never inherit another scope's support. Profile selection is
explicit, not automatic device detection or a persisted credential-profile setting.

GET replies are `CommandReadReply<ParameterReply<T>>`: accepted replies expose
`value: T`; rejected replies retain status and opaque extensions. The accepted
envelope requires operation 0, count 1, the type's echoed ID and its established
payload layout. Unknown codes remain raw, but unexpected widths/layouts
return errors instead of silently falling back to an untyped value.
Raw exploration remains explicit and approval-gated; it is not a typed fallback.
The fixed two-ID request is `AdvancedLiveReversalGet`, with an
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

The capability profile marks `RotationStateValue` (`0014`) read-only.
The struct contains
`reserved_before`, `orientation_code`, and `reserved_after`; the targeted
layout is `[0, code, 0]`. This derived orientation is not a raw rotation-lock
configuration or sensor reading. Physical direction/degree mappings remain
unspecified and unknown codes remain raw. There is no checked SET constructor;
generic `Set<T>` can still encode this type without enforcing operation coverage.

The separate `SensorAspectLockSet` codec uses camera expansion command
`02:FF`, selector `001F`: `SensorAspectLock::Landscape` (`0`),
`Portrait` (`1`) or `Automatic` (`2`). Its encoded
payload is exactly 34 bytes: `00 1F 00 mode` and thirty
zero bytes. The capability profile marks this selector write-only; it does
not add a query, grant write approval or change preview startup.

```rust
use unofficial_osmo_camera::protocol::{
    CommandPacket,
    commands::camera::cmd_02_ff_sensor_aspect_lock_set::{
        SensorAspectLock, SensorAspectLockSet,
    },
};

let request = SensorAspectLockSet::new(SensorAspectLock::Landscape);
let payload = request.payload().expect("fixed sensor-aspect-lock payload");
assert_eq!(payload.len(), 34);
assert_eq!(&payload[..4], &[0, 0x1f, 0, 0]);
```

Encoding does not transmit the request. Use the existing explicitly approved
write transport when authorized. An ACK confirms handoff, not completed frame
reconfiguration. This aspect lock is not a general pixel-autorotation switch:
gravity detection, upside-down rotation, EIS and client display transforms
are separate, and current-mode restrictions/persistence remain unverified.

Binary command payloads, numeric wire types, DUML/UDP framing, ACKs and media
headers use `binrw`. Field types own their wire representation; fixed layouts
use derives, while checksum coverage, request-dependent validation and bitmap
algorithms remain explicit. Closed numeric enums use `#[brw(repr = u8)]`
(or `u16`); open enums use variant `magic` attributes followed by a raw
`Unknown(u8)` fallback. Unknown codes and opaque extensions stay lossless.
Fixed command layouts can use `magic`; derived writers calculate length
prefixes without exposing them as caller-settable fields.
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
`MessageContext` supplies sender, receiver, message ID and flags independently
of the body; DUML framing calculates length and checksums and composes the
command body once.
`Get<T>` and `Set<T>` include the camera command prefix as well as their parameter
envelope. The application device-info response also encodes its command prefix.
Startup/write requests, BLE keepalive/setup and notifications follow the same
command-only magic structure. Raw BLE pairing-arm bytes are a GATT characteristic
write, not a DUML command, and deliberately have no command prefix.

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
returns command bytes plus payload; use `query.payload()` only when an
explicitly raw API already takes the command separately.
Command declarations and opaque response structs are explicit.
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

Closed numeric enums use binrw for wire conversion rather than generated
primitive `From`/`Into`/`TryFrom` implementations. Open types such as
`VideoResolutionCode` expose explicit `From<u8>` and `.wire()` helpers.
No numeric-enum conversion dependency is required.
Unsupported closed-enum codes and malformed native enum variants produce
`Error::WireCodec`, including malformed accessory and media-state layouts.
Branch errors remain available in the codec error's source; directly raised
domain validation errors retain their existing library variants. Codec errors retain their source
without displaying potentially sensitive details in the top-level message.
Frame lengths and counts remain checked before reading variable bodies.
FEC, incremental framing and H.264 bitstream algorithms remain explicit.

```rust
use unofficial_osmo_camera::protocol::commands::camera::cmd_02_8e_parameters::{
    CameraStabilizationMode, EisValue, Get, ParameterReply, Set,
};
use unofficial_osmo_camera::{
    ControlSession, Result,
    protocol::commands::reply::CommandReadReply,
};

fn read_setting(session: &mut ControlSession) -> Result<CommandReadReply<ParameterReply<EisValue>>> {
    session.execute_query::<Get<EisValue>>()
}

fn write_setting(session: &mut ControlSession, approved: bool) -> Result<()> {
    session.execute_parameter_set(
        Set::new(EisValue::new(CameraStabilizationMode::RockSteady)?),
        approved,
    )
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

Capability reads also have independent query types: `EiRangeGet`,
`RecommendedEiGet`, `LensInformationGet` and `AccessoriesGet`.
For example, `session.execute_query::<AccessoriesGet>()` returns
`AccessoriesReply`, whose `.result` holds the outer `CameraReadReply`.
The first three return `CommandReadReply<EiRangeReply>`,
`CommandReadReply<RecommendedEiReply>` and `CommandReadReply<LensInformationReply>`;
accepted replies expose opaque, nonempty bodies through `.raw_bytes()`.
Accessories retain outer and inner rejections separately and validate
operation/selector before interpreting the inner status. On outer rejection,
`reply.result` is `CameraReadReply::Rejected { .. }`; on inner rejection, it is
`CameraReadReply::Accepted(AccessoriesData::Rejected { .. })`.
The capability query and value definitions are explicit structs without custom
macros. Both LAN and BLE typed query entry points take only the query type.

All 37 targeted parameter entries document their value shape or an explicit unknown.
There are **33 available checked SET constructors**, with restricted subsets of some
write branches. Fixed record widths are expressed by the binrw payload types.
TC records have two possible widths, selected by their second byte.
Covered writes include
raw audio/mirror codes, bounded image/shutter/recording controls, tagged
autofocus records, sparse `001F`/`003C` codes and capture-aspect values.
Capture-aspect SET accepts only 0..=255 despite its four-byte wire width.
`BeautyEffectValue` carries the complete 62-byte indexed record; its SET constructor
builds the canonical count, indices and one-byte entries.
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

The calibration `002B` constructor supplies only action 3's raw focal-length record: zero
metadata, a byte-length-prefixed NUL-terminated label, LE16 extra length 4,
raw LE16 focal code and two zero bytes. Units and supported codes are unknown.
Actions 0..=2 remain outside the constructor subset; their behavior is not
established by this API. The `002D` constructor permits actions 0/2
with one name, or 1 with two names,
behind an exact LE16 data length. Names are nonempty printable ASCII, at most
127 characters each; total values cannot exceed 255 bytes. These are byte-level
checks, not a calibration/file-operation procedure, path policy or provider
availability guarantee. No apply/rename/delete meanings are assigned.

The four GET-only IDs have no checked SET constructor. They remain manually
encodable through `Set<T>`, which does not enforce operation support.
Unknown codes in supported GET layouts remain raw without granting a checked
write; unsupported layouts are explicit errors.
Duration GET retains all four bytes, but only its low word has
an established conversion. Known widths/codes never imply current-mode
availability, physical semantics, safe retries or an applied setting.

Checked constructors include recording EIS
(`EisValue::new`, 0..=7), independent live-view EIS (`LiveViewEisValue::new`, 0..=7),
stabilization preference (`StabilizationPreferenceValue::new`, 0/1), and stream
quality (`StreamQualityValue::new`, 0/1/2). Mode labels and availability are not
inferred from those ranges. Both EIS payloads take `CameraStabilizationMode`
and expose a `mode: CameraStabilizationMode` field. For example,
`EisValue::new(CameraStabilizationMode::RockSteady)` and
`LiveViewEisValue::new(CameraStabilizationMode::RockSteady)` encode the same
one-byte mode under their separate IDs (`0008` / `003E`).
Codes 5..=7 use explicit `Unknown05`, `Unknown06` and `Unknown07` variants;
their physical mode names are not confirmed. Other codes retain `Unknown(u8)`.
Constructors reject `Unknown(8..=255)`, while
manually constructed payloads and GET decoding preserve every byte. The
existing names are compatibility candidates, not proven effective modes.
The recording and live-view parameters use separate IDs. Their shared codec
does not implement firmware-specific GET fallbacks.
For Boolean compatibility parameters, pass `u8::from(enabled)` to their
constructor; do not treat EIS codes 0/1 as a guaranteed physical on/off switch.
The field-of-view SET constructor restricts codes to 0..=5 and the
voice-language SET constructor to 0/1; invalid values
return an error at construction. GET decoding continues to preserve
unknown codes without applying SET limits. These APIs only encode payloads;
they neither send requests nor authorize changing a camera setting.

## Model capabilities

Model and firmware coverage lives separately from command codecs in
`protocol::capabilities`. `NANO_10_00_31_22` includes 37 non-placeholder camera
parameter IDs (29 GET and 33 SET cases), plus 21 exact read contracts across
camera, Wi-Fi, common and media commands. Parameter entries retain the targeted
profile evidence; other entries are explicitly compatibility contracts, not
verified firmware availability.
`lookup(receiver, command, selector) -> Option<&CommandCapability>` returns
matching operation coverage and evidence, or `None` for an unknown entry.
`require_query::<Get<T>>()` rejects parameter reads not allowed by this profile. Generic
`Get<T>` and `Set<T>` use the same `Parameter` bounds without enforcing operation
support. Compatibility codec names are not evidence of Nano support. Value
meanings, safe SET payloads and current-state availability remain separate
questions; looking up coverage sends no requests or writes.

Evidence for recording/live-view EIS payloads and accepted code ranges, and
the sensor-aspect-lock payload/mode codes, is maintainer-reported authorized
experimentation on an Osmo Nano running firmware **10.00.31.22**. This is a
device-observation evidence category, not an official vendor specification or
verification of other models/firmware. Other parameter entries describe
implementation coverage without asserting device-observation evidence.
No raw captures, credentials or device identifiers are included.

### Recording formats and live-preview capabilities

Recording settings, Wi-Fi preview, LCD output and experimental RTMP profiles
are separate namespaces. `capabilities::video_formats` supplies six Nano
recording dimensions, nominal frame-rate codes and published mode combinations.
Shared `RecordingAspect`, `RecordingResolution`, `RecordingFormat` and
`PreviewObservation` types live in `protocol::video_formats` and have no Nano
support restrictions. Resolution identifiers use the lossless
`VideoResolutionCode`, an enum covering all currently known resolution codes
with an `Unknown(u8)` fallback, not a Nano-specific protocol enum.
`NANO_RECORDING_RESOLUTIONS` selects six named variants from that enum.
Nano mode combinations and option-byte interpretation remain in capabilities;
they are not applied by generic recording GET/SET codecs.
The resolution catalog includes 1920x1440
(`0C`) and 3840x2880 (`67`) from the cited public Nano observations.
Catalog entries do not imply that every camera supports them. Construct
known codes with variants such as `VideoResolutionCode::Size3840x2160`;
use `VideoResolutionCode::from(byte)` for raw input and `.wire()` for the byte.
Normal Video has 35 resolution/rate pairs: 24/25/30/48/50/60 for all six
sizes except 4K 4:3, which stops at 50 fps. Slow Motion has three 120-fps
4x combinations and 1080p 240-fps 8x; SuperNight has the three 16:9 sizes at
24/25/30, and Timelapse/Hyperlapse use those sizes at 25/30.
Photo and unknown capture modes do not inherit Video metadata.

Dimensions and normal/slow-motion/time-lapse limits follow
[DJI specifications](https://www.dji.com/nano/specs), consulted 2026-11-07.
Wire IDs, SuperNight combinations and preview observations follow the original,
independently published [MIT-licensed Nano protocol notes at revision
2b1b8a1](https://github.com/luisreysan/osmocam/blob/2b1b8a1f94f59578f5092d562fa937fafd0d3a74/docs/PROTOCOL.md).
No external code or captures were imported. Those observations do not identify
firmware and are not an official protocol specification or runtime allowlist.

`VideoFormatGet` (`02/19`, empty request) is an exact compatibility read.
Accepted replies expose `format: VideoFormat` and preserve opaque extensions;
rejected replies preserve status and rejection bytes. `VideoFormatSet` (`02/18`)
can replace resolution/rate while retaining current FOV/options.
Nano nominal 24/25/30/48/50/60 use wire codes 1..=6, not the generic integer-rate
variants with codes 13..=16. Nominal 60 may be 59.94 fps. The generic resolution
table is not a Nano support list. The capability helper
`capabilities::video_formats::with_nano_slow_motion_factor(set, 4|8)` replaces
only byte 4, independently of the low-six-bit speed-ratio code; callers must
select a compatible Slow Motion mode and resolution/rate separately.
`nano_slow_motion_factor(format)` in the same capability module reads that
model-specific byte.
Generic `VideoFormatSet::with_options` accepts all raw option bits without
Nano factor validation.

```rust
use unofficial_osmo_camera::{
    ControlSession, Error, Result,
    protocol::{
        commands::{
            camera::{
                cmd_02_18_video_format_set::VideoFormat,
                cmd_02_19_video_format_get::VideoFormatGet,
            },
            reply::CommandReadReply,
        },
        identity::{CameraCommand, Command},
    },
};

fn read_recording_format(session: &mut ControlSession) -> Result<VideoFormat> {
    match session.execute_query::<VideoFormatGet>()? {
        CommandReadReply::Accepted(reply) => Ok(reply.format),
        CommandReadReply::Rejected { status, .. } => Err(Error::CameraRejected {
            command: Command::Camera(CameraCommand::VideoFormatGet),
            status,
        }),
    }
}
```

`common::cmd_00_99_capability_topics` exposes explicit `CapabilitySubscribe`
requests for `camcap_video_format`, `camcap_slowmotion_ratio` and
`camcap_capture_aspect_type`. Supply a caller-selected subscription ID and
send `exploration_request(timeout)` through `ControlSession::request_raw`
with both approvals. Subscription replies use `CapabilitySubscriptionReply`.
They are not passive GETs, are not added to the read allowlist and are never
sent during startup. No unsubscribe or session-scoped subscription lifetime
is promised.

Decode received `00/99` bodies as `CapabilityNotification`; its typed table
methods preserve unknown row values, check lengths/counts and distinguish
an available empty table from an explicitly unavailable one. Unknown topics
remain lossless but cannot be interpreted as a known table. After validating
a request from recipient `28` to APP, explicitly call
`acknowledge_capability_notification(&message, true)` to queue its receipt ACK.
The ACK retains the inbound ID and does not await another command reply.
Decoding installs no automatic ACK policy or capability cache and sends no mode
switch or setting write.
Camera-reported rows describe current availability. Published candidates remain
separate and never fill missing live topics.

`CaptureAspectTypeValue` (`0046`) is a four-byte TLV without
ratio labels. Public notes report one-byte values 1=4:3 and 2=16:9, so this
current codec has no established equivalence to those aspect labels and is not
a verified aspect selector. It is not the `02/FF` portrait/landscape sensor lock.

Public Wi-Fi preview observations are 1280x720 for 16:9 and 960x720 for 4:3,
nominally 30 fps; `NANO_PREVIEW_OBSERVATIONS` records these as observations,
not configurable output modes. Measure actual dimensions from H.264 SPS or
decoder output. `StreamQualityValue`, `02/1A` quality and `02/4C` SD/HD/LCD
labels have no verified numeric preview-size mapping. GStreamer `framerate`
only supplies nominal caps/timing; it does not reconfigure the camera.
[DJI does not list Nano as supporting Mimo live
streaming](https://support.dji.com/help/content?customId=01700006728&spaceId=17&re=FI&lang=en&paperDocType=ARTICLE);
experimental RTMP 720/1080 selectors do not prove preview availability.
Format changes can interrupt preview. These format helpers only encode payloads;
they do not retry requests, reconnect, change timeouts or infer settings.

## Preview command contracts

The current codecs distinguish camera parameters at recipient `01`, media
streaming at `08`, and live-view control at `41`. Their byte layouts are not
interchangeable. Constructor bounds describe restricted compatibility subsets,
not guaranteed runtime availability. Manual payloads remain permissive by design.

- `02:09` emits ten zero bytes and action `03`/`04`.
- `09:A8` encodes the fixed `LiveViewIFrameRequest`.
- `02:1A` remains an encoding-only quality payload, separate from `02:8E/0006`.
- `07:47` retries matching `01 FF` replies under an existing bounded host
  policy, not proof of transient failure.
- `03:DA` is an unconfirmed compatibility prelude, not verified physical
  gimbal initialization.

These API contracts do not establish a firmware support matrix, safe retry
semantics or working media delivery. Encoding alone sends no requests or writes
and grants no hardware authorization.

`LiveViewControlRequest` exposes an `action` of `LiveViewControlAction::Start`
or `Stop`. Use `LiveViewControlRequest::start()` / `stop()`, or `new(action)`,
to encode its eleven-byte payload. `LiveViewIFrameRequest::default()` encodes
the ten-byte I-frame payload.

The automatic preview startup sends start followed by an I-frame request.
That sequence does not send the control codec's stop request or a quality write.
`02:1A` is a manually encodable, unverified command; no encoder settings or
automatic reconnection are inferred.
