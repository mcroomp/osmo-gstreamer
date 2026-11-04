//! Camera parameter GET/TLV response and restricted single-parameter SET codecs.

use crate::{
    Error, Result,
    protocol::{
        Query, WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::CameraReadReply,
        identity::{CameraCommand, Command, Endpoint},
    },
};
use num_enum::{FromPrimitive, IntoPrimitive};

const PARAMETER_GET_OPERATION: u8 = 0;
const PARAMETER_SET_OPERATION: u8 = 1;
const ADVANCED_LIVE_REVERSAL_IDS: [CameraParameter; 2] = [
    CameraParameter::AdvanceMode,
    CameraParameter::Unknown(0x0001),
];

/// Parameter identifiers used by compatibility codecs.
///
/// [`Self::NANO_PARAMETERS`] is the bounded Nano 10.00.31.22 operation catalogue.
/// Other variants retain legacy API names, not confirmed Nano support.
/// Catalogue membership does not guarantee availability in every camera state
/// or configuration. Unknown product extensions remain [`Self::Unknown`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum CameraParameter {
    /// One-byte advanced-mode code; not a confirmed live-reversal setting.
    AdvanceMode = 0x0000,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    Beauty = 0x0002,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    FpvControl = 0x0003,
    /// Raw stream-quality code; no bitrate or resolution mapping is assigned.
    StreamQuality = 0x0006,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    PreviousMode = 0x0007,
    Eis = 0x0008,
    FieldOfView = 0x0009,
    VoiceControl = 0x000a,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    QuickModeSwitch = 0x000b,
    VoiceLanguage = 0x000e,
    /// Raw ISO-limit code, distinct from the user-ISO GET command.
    IsoLimit = 0x000f,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    Led = 0x0010,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    ScreenSleep = 0x0011,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    AutoOff = 0x0012,
    /// Read-only three-byte rotation record; angle units remain unspecified.
    Unknown0014 = 0x0014,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    SpotMetering = 0x0015,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    CaptureMinimumInterval = 0x0017,
    LoopDuration = 0x0018,
    /// Opaque write operation; no meaningful GET value is established.
    Unknown0019 = 0x0019,
    /// Read-only lens-control code, not a lens capability enumeration.
    LensControl = 0x001e,
    /// One-byte code with unmapped meaning.
    Unknown001f = 0x001f,
    RecordingAudioChannel = 0x0020,
    /// Legacy identifier; not in the confirmed Nano catalogue.
    P2PShare = 0x0021,
    SelfieFollowing = 0x0024,
    /// Legacy identifier; the Nano case is a no-op, not a supported beauty control.
    VideoBeautify = 0x0025,
    /// Opaque write operation; the complete value contract remains unspecified.
    Unknown0026 = 0x0026,
    /// One-byte code with unmapped meaning.
    Unknown0028 = 0x0028,
    /// Nine-byte record, not a single duration scalar.
    ShortVideoParameters = 0x0029,
    LensUserCalibration = 0x002b,
    /// Read-only, variable-length calibration state; not a scalar.
    LensUserCalibrationState = 0x002c,
    LensUserCalibrationFile = 0x002d,
    AddCustomMode = 0x002f,
    StabilizationPreference = 0x0030,
    /// Write-only storage synchronization request, distinct from its outcome.
    StorageSynchronization = 0x0031,
    /// Read-only 13-byte storage outcome record.
    StorageOutcome = 0x0032,
    ImageAdjustment = 0x0033,
    /// Raw shutter-limit code, not a physical exposure time.
    ShutterLimit = 0x0034,
    VideoQualityEnhancement = 0x0035,
    TimelapseTrackPreview = 0x0036,
    TimelapseTrackSet = 0x0037,
    SelfieMirror = 0x0038,
    /// GET returns a 62-byte counted record, not one beauty enum byte.
    BeautyEffect = 0x0039,
    /// Two-byte record; do not treat it as a bare sensitivity byte.
    AutofocusSensitivity = 0x003a,
    /// Two-byte record; do not treat it as a bare strategy byte.
    AutofocusStrategy = 0x003b,
    /// Two-byte record with unmapped meaning.
    Unknown003c = 0x003c,
    RecordingPlan = 0x003d,
    /// Raw live-view stabilization code, distinct from parameter `0008`.
    LiveViewEis = 0x003e,
    /// Four-byte aspect-type value; physical aspect-ratio labels are unspecified.
    CaptureAspectType = 0x0046,
    #[num_enum(catch_all)]
    Unknown(u16),
}

/// Nano operation coverage, not an assurance of current-state availability.
///
/// Write coverage does not supply a safe SET value or authorize transmission.
/// `WriteOnly` means no meaningful GET value is established; it need not mean
/// every GET request is rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NanoParameterAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

impl NanoParameterAccess {
    pub fn can_read(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }

    pub fn can_write(self) -> bool {
        matches!(self, Self::WriteOnly | Self::ReadWrite)
    }
}

impl CameraParameter {
    /// All 37 non-placeholder Nano parameter cases in numeric order.
    ///
    /// This covers 29 value-producing GET cases and 33 write cases, with
    /// overlap. It excludes legacy-only IDs, the `0025` no-op, and unverified
    /// extensions. It is not a request to probe all IDs or change settings.
    pub const NANO_PARAMETERS: &'static [Self] = &[
        Self::AdvanceMode,
        Self::StreamQuality,
        Self::Eis,
        Self::FieldOfView,
        Self::VoiceControl,
        Self::VoiceLanguage,
        Self::IsoLimit,
        Self::Unknown0014,
        Self::LoopDuration,
        Self::Unknown0019,
        Self::LensControl,
        Self::Unknown001f,
        Self::RecordingAudioChannel,
        Self::SelfieFollowing,
        Self::Unknown0026,
        Self::Unknown0028,
        Self::ShortVideoParameters,
        Self::LensUserCalibration,
        Self::LensUserCalibrationState,
        Self::LensUserCalibrationFile,
        Self::AddCustomMode,
        Self::StabilizationPreference,
        Self::StorageSynchronization,
        Self::StorageOutcome,
        Self::ImageAdjustment,
        Self::ShutterLimit,
        Self::VideoQualityEnhancement,
        Self::TimelapseTrackPreview,
        Self::TimelapseTrackSet,
        Self::SelfieMirror,
        Self::BeautyEffect,
        Self::AutofocusSensitivity,
        Self::AutofocusStrategy,
        Self::Unknown003c,
        Self::RecordingPlan,
        Self::LiveViewEis,
        Self::CaptureAspectType,
    ];

    pub fn nano_access(self) -> Option<NanoParameterAccess> {
        use NanoParameterAccess::*;
        match self {
            Self::Unknown0014
            | Self::LensControl
            | Self::LensUserCalibrationState
            | Self::StorageOutcome => Some(ReadOnly),
            Self::Unknown0019
            | Self::Unknown0026
            | Self::LensUserCalibration
            | Self::LensUserCalibrationFile
            | Self::AddCustomMode
            | Self::StorageSynchronization
            | Self::TimelapseTrackPreview
            | Self::TimelapseTrackSet => Some(WriteOnly),
            Self::AdvanceMode
            | Self::StreamQuality
            | Self::Eis
            | Self::FieldOfView
            | Self::VoiceControl
            | Self::VoiceLanguage
            | Self::IsoLimit
            | Self::LoopDuration
            | Self::Unknown001f
            | Self::RecordingAudioChannel
            | Self::SelfieFollowing
            | Self::Unknown0028
            | Self::ShortVideoParameters
            | Self::StabilizationPreference
            | Self::ImageAdjustment
            | Self::ShutterLimit
            | Self::VideoQualityEnhancement
            | Self::SelfieMirror
            | Self::BeautyEffect
            | Self::AutofocusSensitivity
            | Self::AutofocusStrategy
            | Self::Unknown003c
            | Self::RecordingPlan
            | Self::LiveViewEis
            | Self::CaptureAspectType => Some(ReadWrite),
            _ => None,
        }
    }

    /// Fixed GET value width for the Nano profile, excluding the TLV header.
    ///
    /// `None` means variable-length (`002C`) or no confirmed value-producing GET.
    /// This metadata does not change the generic decoder's opaque-byte retention.
    pub fn nano_get_value_length(self) -> Option<usize> {
        match self {
            Self::AdvanceMode
            | Self::StreamQuality
            | Self::Eis
            | Self::FieldOfView
            | Self::VoiceControl
            | Self::VoiceLanguage
            | Self::IsoLimit
            | Self::LensControl
            | Self::Unknown001f
            | Self::RecordingAudioChannel
            | Self::SelfieFollowing
            | Self::Unknown0028
            | Self::StabilizationPreference
            | Self::ImageAdjustment
            | Self::ShutterLimit
            | Self::VideoQualityEnhancement
            | Self::SelfieMirror
            | Self::RecordingPlan
            | Self::LiveViewEis => Some(1),
            Self::AutofocusSensitivity | Self::AutofocusStrategy | Self::Unknown003c => Some(2),
            Self::Unknown0014 => Some(3),
            Self::LoopDuration | Self::CaptureAspectType => Some(4),
            Self::ShortVideoParameters => Some(9),
            Self::StorageOutcome => Some(13),
            Self::BeautyEffect => Some(62),
            _ => None,
        }
    }
}

/// Legacy field-of-view labels for one-byte parameter `0x0009`.
///
/// These names are compatibility candidates, not a guarantee of camera
/// support or a mapping for the separate `0x02/0x18` format field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CameraFieldOfView {
    UltraWide = 0,
    Wide = 1,
    Standard = 2,
    NaturalWide = 5,
    #[num_enum(catch_all)]
    Unknown(u8),
}

/// Legacy stabilization labels for one-byte parameter `0x0008`.
///
/// Names do not establish device support or observed stabilization behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CameraStabilizationMode {
    Off = 0,
    RockSteady = 1,
    HorizonSteady = 2,
    RockSteadyPlus = 3,
    HorizonBalancing = 4,
    #[num_enum(catch_all)]
    Unknown(u8),
}

impl WireEncode for CameraParameter {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u16_le((*self).into());
        Ok(())
    }
}

impl WireDecode for CameraParameter {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        Ok(Self::from(reader.read_u16_le()?))
    }
}

/// One parameter record returned by a camera parameter GET.
///
/// ID and value length are wire fields; the bytes remain uninterpreted until
/// a caller selects a view. No semantic view establishes model support.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraParameterValue {
    pub parameter: CameraParameter,
    pub bytes: Vec<u8>,
}

/// Three raw codes from parameter `0x0014`, without angle units or lock labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraRotationState {
    pub mode_code: u8,
    pub device_angle_code: u8,
    pub sensor_angle_code: u8,
}

impl WireEncode for CameraParameterValue {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let length = u8::try_from(self.bytes.len())
            .map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes"))?;
        self.parameter.write_to(writer)?;
        writer.write_u8(length);
        writer.write_bytes(&self.bytes);
        Ok(())
    }
}

impl CameraParameterValue {
    /// Return a scalar code only for parameters with a one-byte value layout.
    ///
    /// Codes are not durations, physical ISO values or cross-model enum labels.
    pub fn scalar_code(&self) -> Option<u8> {
        match self.bytes.as_slice() {
            [value]
                if self.parameter.nano_get_value_length() == Some(1)
                    || matches!(
                        self.parameter,
                        CameraParameter::Beauty
                            | CameraParameter::FpvControl
                            | CameraParameter::PreviousMode
                            | CameraParameter::QuickModeSwitch
                            | CameraParameter::ScreenSleep
                            | CameraParameter::AutoOff
                            | CameraParameter::SpotMetering
                    ) =>
            {
                Some(*value)
            }
            _ => None,
        }
    }

    /// Return the two-byte LED value; individual bit meanings are unspecified.
    pub fn led_raw(&self) -> Option<u16> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::Led, [low, high]) => Some(u16::from_le_bytes([*low, *high])),
            _ => None,
        }
    }

    /// Return the capture-interval value without assuming a time unit.
    pub fn capture_minimum_interval_raw(&self) -> Option<u16> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::CaptureMinimumInterval, [low, high]) => {
                Some(u16::from_le_bytes([*low, *high]))
            }
            _ => None,
        }
    }

    /// Return the four-byte loop-duration value without assuming a time unit.
    pub fn loop_duration_raw(&self) -> Option<u32> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::LoopDuration, [a, b, c, d]) => {
                Some(u32::from_le_bytes([*a, *b, *c, *d]))
            }
            _ => None,
        }
    }

    pub fn rotation_state(&self) -> Option<CameraRotationState> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::Unknown0014, [mode, device, sensor]) => Some(CameraRotationState {
                mode_code: *mode,
                device_angle_code: *device,
                sensor_angle_code: *sensor,
            }),
            _ => None,
        }
    }

    /// Apply legacy labels to one-byte FOV data; unknown codes are retained.
    pub fn field_of_view(&self) -> Option<CameraFieldOfView> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::FieldOfView, [value]) => Some(CameraFieldOfView::from(*value)),
            _ => None,
        }
    }

    /// Apply legacy labels to one-byte stabilization data; unknown codes are retained.
    pub fn stabilization_mode(&self) -> Option<CameraStabilizationMode> {
        match (self.parameter, self.bytes.as_slice()) {
            (CameraParameter::Eis, [value]) => Some(CameraStabilizationMode::from(*value)),
            _ => None,
        }
    }

    /// Legacy nonzero test for selected parameters, not a validated state enum.
    pub fn enabled(&self) -> Option<bool> {
        match (self.parameter, self.bytes.as_slice()) {
            (
                CameraParameter::VoiceControl
                | CameraParameter::QuickModeSwitch
                | CameraParameter::SpotMetering
                | CameraParameter::SelfieFollowing,
                [value],
            ) => Some(*value != 0),
            _ => None,
        }
    }
}

/// Accepted TLV records, excluding the outer status byte.
///
/// Camera -> APP | 0x02/0x8E | Parameter GET reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         operation: u8 = 0x00, count: u8,
///         records: [{ id: le_u16, length: u8, value: [u8; length] }; count] }
/// ```
///
/// Operation, count, echoed IDs and order must match the request exactly.
/// Trailing records/bytes are rejected. Value widths and unknown bytes are retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraParameterValues(pub Vec<CameraParameterValue>);

impl WireEncode for CameraParameterValues {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let count =
            u8::try_from(self.0.len()).map_err(|_| Error::LimitExceeded("too many camera TLVs"))?;
        writer.write_u8(PARAMETER_GET_OPERATION);
        writer.write_u8(count);
        for value in &self.0 {
            value.write_to(writer)?;
        }
        Ok(())
    }
}

/// Read camera parameters by their LE16 identifiers, in request order.
///
/// APP -> Camera | 0x02/0x8E
///
/// ```text
/// request { operation: u8 = 0, count: u8, ids: [le_u16; count] }
/// reply {
///     status: u8
///     operation: u8 = 0
///     count: u8
///     records: [{ id: le_u16, length: u8, value: [u8; length] }]
/// }
/// ```
///
/// Encoding a named identifier does not establish that a given camera handles
/// it. Rejections are retained without trying to decode a successful TLV body.
/// The byte width is explicit in every returned record; the decoder preserves
/// it instead of truncating to a convenience scalar or enum.
#[derive(Clone, Debug)]
pub struct CameraParametersGet {
    parameters: Vec<CameraParameter>,
}

/// Encode one state-changing parameter: operation `1`, count `1`, then a TLV.
///
/// APP -> Camera | 0x02/0x8E | success reply: status byte `00`, not GET TLVs.
///
/// Boolean variants encode one byte; code variants imply no physical unit.
/// LED encodes a little-endian `u16`. Field-of-view uses the parameter namespace,
/// not the video-format namespace. `EisEnabled` emits raw codes 0/1; its name
/// does not confirm the physical stabilization mode. Support, value meanings
/// and current-mode capabilities are not validated.
///
/// This type is not a read-only [`Query`] and authorizes no transmission.
/// Rotation, loop duration and video beautify have no SET variant because
/// their complete model-specific write contracts remain unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraParameterSet {
    BeautyCode(u8),
    FpvControlCode(u8),
    EisEnabled(bool),
    FieldOfView(CameraFieldOfView),
    VoiceControl(bool),
    VoiceLanguageCode(u8),
    IsoLimitCode(u8),
    Led(u16),
    ScreenSleepCode(u8),
    AutoOffCode(u8),
    SpotMetering(bool),
    SelfieFollowing(bool),
}

impl CameraParameterSet {
    fn value(self) -> CameraParameterValue {
        let (parameter, bytes) = match self {
            Self::BeautyCode(code) => (CameraParameter::Beauty, vec![code]),
            Self::FpvControlCode(code) => (CameraParameter::FpvControl, vec![code]),
            Self::EisEnabled(enabled) => (CameraParameter::Eis, vec![u8::from(enabled)]),
            Self::FieldOfView(value) => (CameraParameter::FieldOfView, vec![value.into()]),
            Self::VoiceControl(enabled) => (CameraParameter::VoiceControl, vec![u8::from(enabled)]),
            Self::VoiceLanguageCode(code) => (CameraParameter::VoiceLanguage, vec![code]),
            Self::IsoLimitCode(code) => (CameraParameter::IsoLimit, vec![code]),
            Self::Led(value) => (CameraParameter::Led, value.to_le_bytes().to_vec()),
            Self::ScreenSleepCode(code) => (CameraParameter::ScreenSleep, vec![code]),
            Self::AutoOffCode(code) => (CameraParameter::AutoOff, vec![code]),
            Self::SpotMetering(enabled) => (CameraParameter::SpotMetering, vec![u8::from(enabled)]),
            Self::SelfieFollowing(enabled) => {
                (CameraParameter::SelfieFollowing, vec![u8::from(enabled)])
            }
        };
        CameraParameterValue { parameter, bytes }
    }
}

impl WireEncode for CameraParameterSet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(PARAMETER_SET_OPERATION);
        writer.write_u8(1);
        self.value().write_to(writer)
    }
}

impl CameraParametersGet {
    /// Construct a GET only for a value-producing Nano catalogue entry.
    ///
    /// Legacy-only and write-only identifiers are rejected before encoding.
    pub fn single_nano(parameter: CameraParameter) -> Result<Self> {
        if !parameter
            .nano_access()
            .is_some_and(NanoParameterAccess::can_read)
        {
            return Err(Error::Unsupported("parameter is not a confirmed Nano GET"));
        }
        Self::single(parameter)
    }

    /// Compatibility constructor; legacy identifiers do not imply Nano support.
    pub fn single(parameter: CameraParameter) -> Result<Self> {
        if matches!(parameter, CameraParameter::Unknown(_)) {
            return Err(Error::InvalidConfig {
                field: "camera parameter GET",
                reason: "unknown parameters require approved raw exploration",
            });
        }
        if parameter.nano_access() == Some(NanoParameterAccess::WriteOnly) {
            return Err(Error::Unsupported(
                "parameter has no confirmed Nano GET value",
            ));
        }
        Ok(Self {
            parameters: vec![parameter],
        })
    }

    /// Compatibility request for IDs `0000` and `0001`.
    ///
    /// The name does not confirm a reversal feature or support for both IDs.
    pub fn advanced_live_reversal() -> Self {
        Self {
            parameters: ADVANCED_LIVE_REVERSAL_IDS.to_vec(),
        }
    }

    /// Compatibility request for ID `000B`; support and meaning are unconfirmed.
    pub fn quick_mode_switch() -> Self {
        Self {
            parameters: vec![CameraParameter::QuickModeSwitch],
        }
    }
}

impl WireEncode for CameraParametersGet {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        let count = u8::try_from(self.parameters.len())
            .map_err(|_| Error::LimitExceeded("too many camera parameters"))?;
        writer.write_u8(PARAMETER_GET_OPERATION);
        writer.write_u8(count);
        for parameter in &self.parameters {
            parameter.write_to(writer)?;
        }
        Ok(())
    }
}

impl Query for CameraParametersGet {
    type Response = CameraReadReply<CameraParameterValues>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }

    fn command(&self) -> Command {
        Command::Camera(CameraCommand::Parameters)
    }
}

impl WireDecode<CameraParametersGet> for CameraParameterValues {
    fn read_from(reader: &mut WireReader<'_>, query: &CameraParametersGet) -> Result<Self> {
        if reader.read_u8()? != PARAMETER_GET_OPERATION
            || usize::from(reader.read_u8()?) != query.parameters.len()
        {
            return Err(Error::Protocol("camera GET operation/count mismatch"));
        }
        let mut values = Vec::new();
        for expected in &query.parameters {
            let parameter = CameraParameter::read_from(reader, &())?;
            if parameter != *expected {
                return Err(Error::Protocol("camera GET parameter/order mismatch"));
            }
            let length = usize::from(reader.read_u8()?);
            let bytes = reader.read_bytes(length)?.to_vec();
            values.push(CameraParameterValue { parameter, bytes });
        }
        if !reader.remaining().is_empty() {
            return Err(Error::Protocol("trailing camera GET TLVs"));
        }
        Ok(Self(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WireDecode;

    #[test]
    fn nano_catalogue_covers_exactly_the_nonplaceholder_ids_in_order() {
        let expected = [
            0x0000u16, 0x0006, 0x0008, 0x0009, 0x000a, 0x000e, 0x000f, 0x0014, 0x0018, 0x0019,
            0x001e, 0x001f, 0x0020, 0x0024, 0x0026, 0x0028, 0x0029, 0x002b, 0x002c, 0x002d, 0x002f,
            0x0030, 0x0031, 0x0032, 0x0033, 0x0034, 0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x003a,
            0x003b, 0x003c, 0x003d, 0x003e, 0x0046,
        ];
        let actual: Vec<u16> = CameraParameter::NANO_PARAMETERS
            .iter()
            .copied()
            .map(u16::from)
            .collect();
        assert_eq!(actual, expected);
        for id in u16::MIN..=u16::MAX {
            let parameter = CameraParameter::from(id);
            assert_eq!(
                parameter.nano_access().is_some(),
                expected.contains(&id),
                "{id:04x}"
            );
            assert_eq!(parameter.encode().unwrap(), id.to_le_bytes());
            assert_eq!(
                CameraParameter::decode(&id.to_le_bytes(), &()).unwrap(),
                parameter
            );
        }
    }

    #[test]
    fn nano_get_set_coverage_and_widths_are_separate_from_legacy_labels() {
        let get_only = [0x0014u16, 0x001e, 0x002c, 0x0032];
        let set_only = [
            0x0019u16, 0x0026, 0x002b, 0x002d, 0x002f, 0x0031, 0x0036, 0x0037,
        ];
        let mut reads = 0;
        let mut writes = 0;
        for &parameter in CameraParameter::NANO_PARAMETERS {
            let id = u16::from(parameter);
            let access = parameter.nano_access().unwrap();
            assert_eq!(access.can_read(), !set_only.contains(&id));
            assert_eq!(access.can_write(), !get_only.contains(&id));
            reads += usize::from(access.can_read());
            writes += usize::from(access.can_write());
            if access.can_read() {
                let expected_width = match id {
                    0x0014 => Some(3),
                    0x0018 | 0x0046 => Some(4),
                    0x0029 => Some(9),
                    0x002c => None,
                    0x0032 => Some(13),
                    0x0039 => Some(62),
                    0x003a..=0x003c => Some(2),
                    _ => Some(1),
                };
                assert_eq!(parameter.nano_get_value_length(), expected_width);
            } else {
                assert_eq!(parameter.nano_get_value_length(), None);
                assert!(CameraParametersGet::single(parameter).is_err());
            }
        }
        assert_eq!((reads, writes), (29, 33));
        for id in [
            1u16, 2, 3, 7, 0xb, 0x10, 0x11, 0x12, 0x15, 0x17, 0x21, 0x25, 0x47, 0xffff,
        ] {
            let parameter = CameraParameter::from(id);
            assert_eq!(parameter.nano_access(), None);
            assert!(CameraParametersGet::single_nano(parameter).is_err());
        }
    }

    #[test]
    fn every_nano_get_uses_le16_ids_and_round_trips_request_matched_tlvs() {
        for &parameter in CameraParameter::NANO_PARAMETERS {
            if !parameter.nano_access().unwrap().can_read() {
                continue;
            }
            let query = CameraParametersGet::single_nano(parameter).unwrap();
            let id = u16::from(parameter).to_le_bytes();
            assert_eq!(query.encode().unwrap(), [0, 1, id[0], id[1]]);
            let length = parameter.nano_get_value_length().unwrap_or(7);
            let record = CameraParameterValue {
                parameter,
                bytes: vec![0xa5; length],
            };
            assert_eq!(record.scalar_code(), (length == 1).then_some(0xa5));
            let expected = CameraReadReply::Accepted(CameraParameterValues(vec![record]));
            let bytes = expected.encode().unwrap();
            assert_eq!(
                CameraReadReply::<CameraParameterValues>::decode(&bytes, &query).unwrap(),
                expected
            );
            assert!(
                CameraReadReply::<CameraParameterValues>::decode(&bytes[..bytes.len() - 1], &query)
                    .is_err()
            );
            let rejection = [0xe3, 0xa5];
            assert_eq!(
                CameraReadReply::<CameraParameterValues>::decode(&rejection, &query)
                    .unwrap()
                    .encode()
                    .unwrap(),
                rejection
            );
        }
    }

    #[test]
    fn nano_catalogue_does_not_authorize_writes_or_change_request_tuple_checks() {
        use crate::protocol::{ExplorationRequest, Flags};
        use std::time::Duration;

        for &parameter in CameraParameter::NANO_PARAMETERS {
            let [low, high] = u16::from(parameter).to_le_bytes();
            let request = ExplorationRequest {
                receiver: Endpoint::Camera,
                command: Command::Camera(CameraCommand::Parameters),
                flags: Flags::REQUEST,
                payload: vec![0, 1, low, high],
                timeout: Duration::from_secs(1),
            };
            let readable = parameter.nano_access().unwrap().can_read();
            assert_eq!(request.is_known_read(), readable);
            assert_eq!(request.validate(false, false).is_ok(), readable);
            for bad in [
                ExplorationRequest {
                    payload: vec![1, 1, low, high, 1, 0],
                    ..request.clone()
                },
                ExplorationRequest {
                    payload: vec![0, 1, low, high, 0],
                    ..request.clone()
                },
                ExplorationRequest {
                    receiver: Endpoint::Wifi,
                    ..request.clone()
                },
                ExplorationRequest {
                    flags: Flags::RESPONSE,
                    ..request
                },
            ] {
                assert!(!bad.is_known_read());
                for (session, per_request) in [(false, false), (true, false), (false, true)] {
                    assert!(bad.validate(session, per_request).is_err());
                }
            }
        }
    }

    #[test]
    fn legacy_read_approval_is_not_a_nano_support_claim() {
        use crate::protocol::{ExplorationRequest, Flags};
        use std::time::Duration;

        for id in [2u16, 3, 7, 0xb, 0x10, 0x11, 0x12, 0x15, 0x17, 0x21, 0x25] {
            let parameter = CameraParameter::from(id);
            assert_eq!(parameter.nano_access(), None);
            let request = ExplorationRequest {
                receiver: Endpoint::Camera,
                command: Command::Camera(CameraCommand::Parameters),
                flags: Flags::REQUEST,
                payload: CameraParametersGet::single(parameter)
                    .unwrap()
                    .encode()
                    .unwrap(),
                timeout: Duration::from_secs(1),
            };
            assert!(request.is_known_read());
            assert!(request.validate(false, false).is_ok());
        }
    }

    #[test]
    fn parameter_sets_encode_value_widths_and_require_write_approval() {
        use crate::protocol::{ExplorationRequest, Flags};
        use std::time::Duration;

        let fixtures: &[(CameraParameterSet, &[u8])] = &[
            (CameraParameterSet::EisEnabled(true), &[1, 1, 8, 0, 1, 1]),
            (CameraParameterSet::EisEnabled(false), &[1, 1, 8, 0, 1, 0]),
            (
                CameraParameterSet::IsoLimitCode(0xfe),
                &[1, 1, 0x0f, 0, 1, 0xfe],
            ),
            (
                CameraParameterSet::Led(0x1234),
                &[1, 1, 0x10, 0, 2, 0x34, 0x12],
            ),
        ];
        for (value, expected) in fixtures {
            let payload = value.encode().unwrap();
            assert_eq!(payload, *expected);
            let request = ExplorationRequest {
                receiver: Endpoint::Camera,
                command: Command::Camera(CameraCommand::Parameters),
                flags: Flags::REQUEST,
                payload,
                timeout: Duration::from_secs(1),
            };
            assert!(!request.is_known_read());
            for (session, per_request) in [(false, false), (true, false), (false, true)] {
                assert!(request.validate(session, per_request).is_err());
            }
            request.validate(true, true).unwrap();
        }
    }

    #[test]
    fn scalar_codes_are_not_coerced_to_booleans_or_truncated() {
        let value = CameraParameterValue {
            parameter: CameraParameter::IsoLimit,
            bytes: vec![0xfe],
        };
        assert_eq!(value.scalar_code(), Some(0xfe));
        assert_eq!(value.enabled(), None);
        for bytes in [vec![], vec![0xfe, 0xff]] {
            let wrong = CameraParameterValue {
                bytes,
                ..value.clone()
            };
            assert_eq!(wrong.scalar_code(), None);
            assert_eq!(wrong.enabled(), None);
        }
    }

    #[test]
    fn synthetic_structured_values_require_exact_ids_and_widths() {
        for (parameter, bytes) in [
            (CameraParameter::Led, vec![0x34, 0x12]),
            (CameraParameter::CaptureMinimumInterval, vec![0x34, 0x12]),
            (CameraParameter::LoopDuration, vec![0x78, 0x56, 0x34, 0x12]),
            (CameraParameter::Unknown0014, vec![0xfe, 2, 0]),
        ] {
            let value = CameraParameterValue { parameter, bytes };
            assert_eq!(value.scalar_code(), None);
            match parameter {
                CameraParameter::Led => assert_eq!(value.led_raw(), Some(0x1234)),
                CameraParameter::CaptureMinimumInterval => {
                    assert_eq!(value.capture_minimum_interval_raw(), Some(0x1234))
                }
                CameraParameter::LoopDuration => {
                    assert_eq!(value.loop_duration_raw(), Some(0x12345678))
                }
                CameraParameter::Unknown0014 => assert_eq!(
                    value.rotation_state(),
                    Some(CameraRotationState {
                        mode_code: 0xfe,
                        device_angle_code: 2,
                        sensor_angle_code: 0,
                    })
                ),
                _ => unreachable!(),
            }
            for length in 0..=value.bytes.len() + 1 {
                if length == value.bytes.len() {
                    continue;
                }
                let wrong = CameraParameterValue {
                    bytes: vec![0; length],
                    ..value.clone()
                };
                assert_eq!(wrong.led_raw(), None);
                assert_eq!(wrong.capture_minimum_interval_raw(), None);
                assert_eq!(wrong.loop_duration_raw(), None);
                assert_eq!(wrong.rotation_state(), None);
            }
            let unknown = CameraParameterValue {
                parameter: CameraParameter::Unknown(u16::MAX),
                ..value
            };
            assert_eq!(unknown.led_raw(), None);
            assert_eq!(unknown.capture_minimum_interval_raw(), None);
            assert_eq!(unknown.loop_duration_raw(), None);
            assert_eq!(unknown.rotation_state(), None);
        }
    }

    #[test]
    fn synthetic_model_specific_tlvs_round_trip_and_reject_truncation() {
        for parameter in [CameraParameter::VideoBeautify, CameraParameter::Unknown0014] {
            let query = CameraParametersGet::single(parameter).unwrap();
            for length in [0, 1, 3, usize::from(u8::MAX)] {
                let value = CameraParameterValue {
                    parameter,
                    bytes: vec![0xfe; length],
                };
                let expected = CameraReadReply::Accepted(CameraParameterValues(vec![value]));
                let encoded = expected.encode().unwrap();
                assert_eq!(
                    CameraReadReply::<CameraParameterValues>::decode(&encoded, &query).unwrap(),
                    expected
                );
                for end in 0..encoded.len() {
                    assert!(
                        CameraReadReply::<CameraParameterValues>::decode(&encoded[..end], &query)
                            .is_err()
                    );
                }
            }
        }
        assert!(
            CameraParameterValue {
                parameter: CameraParameter::VideoBeautify,
                bytes: vec![0; usize::from(u8::MAX) + 1],
            }
            .encode()
            .is_err()
        );
        assert!(
            CameraParameterValues(vec![
                CameraParameterValue {
                    parameter: CameraParameter::Beauty,
                    bytes: vec![0],
                };
                usize::from(u8::MAX) + 1
            ])
            .encode()
            .is_err()
        );
    }

    #[test]
    fn parameter_replies_must_match_requested_ids_in_order() {
        let query = CameraParametersGet::advanced_live_reversal();
        let ordered = [0, 0, 2, 0, 0, 1, 0xa5, 1, 0, 1, 0x5a];
        let reply = CameraReadReply::<CameraParameterValues>::decode(&ordered, &query).unwrap();
        assert_eq!(reply.encode().unwrap(), ordered);
        let reordered = [0, 0, 2, 1, 0, 1, 0x5a, 0, 0, 1, 0xa5];
        assert!(CameraReadReply::<CameraParameterValues>::decode(&reordered, &query).is_err());
    }

    #[test]
    fn unknown_single_parameters_and_malformed_replies_are_rejected() {
        assert!(CameraParametersGet::single(CameraParameter::Unknown(0x1234)).is_err());
        let query = CameraParametersGet::quick_mode_switch();
        for bytes in [
            &[0, 0, 0, 0x0b, 0, 1, 1][..],
            &[0, 0, 2, 0x0b, 0, 1, 1][..],
            &[0, 0, 1, 0x0b, 0, 2, 1][..],
            &[0, 0, 1, 0x0b, 0, 1, 1, 0xff][..],
        ] {
            assert!(CameraReadReply::<CameraParameterValues>::decode(bytes, &query).is_err());
        }
    }

    #[test]
    fn parameter_reply_preserves_rejection_and_exposes_one_byte_flags() {
        let query = CameraParametersGet::quick_mode_switch();
        let reply =
            CameraReadReply::<CameraParameterValues>::decode(&[0, 0, 1, 0x0b, 0, 1, 1], &query)
                .unwrap();
        let CameraReadReply::Accepted(values) = reply else {
            panic!("expected accepted parameter reply");
        };
        assert_eq!(values.0[0].enabled(), Some(true));
        assert_eq!(values.encode().unwrap(), [0, 1, 0x0b, 0, 1, 1]);

        let rejected =
            CameraReadReply::<CameraParameterValues>::decode(b"\xe3opaque", &query).unwrap();
        assert_eq!(rejected.encode().unwrap(), b"\xe3opaque");
    }

    #[test]
    fn fov_and_stabilization_labels_require_their_exact_one_byte_tlv() {
        let value = |parameter, bytes: &[u8]| CameraParameterValue {
            parameter,
            bytes: bytes.to_vec(),
        };
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0x05]).field_of_view(),
            Some(CameraFieldOfView::NaturalWide)
        );
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0xFE]).field_of_view(),
            Some(CameraFieldOfView::Unknown(0xFE))
        );
        assert_eq!(
            value(CameraParameter::Eis, &[0x03]).stabilization_mode(),
            Some(CameraStabilizationMode::RockSteadyPlus)
        );
        assert_eq!(
            value(CameraParameter::Eis, &[0xFE]).stabilization_mode(),
            Some(CameraStabilizationMode::Unknown(0xFE))
        );
        assert_eq!(value(CameraParameter::Eis, &[0]).field_of_view(), None);
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0, 1]).field_of_view(),
            None
        );
    }
}
