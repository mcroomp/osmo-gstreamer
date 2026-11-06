//! Camera parameter GET/TLV response and restricted single-parameter SET codecs.

use crate::protocol::binrw_error;
use crate::protocol::{decode, encode};
use crate::{
    Error, Result,
    protocol::{
        Query,
        commands::reply::CameraReadReply,
        identity::{CameraCommand, Command, Endpoint},
    },
};
use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::io::{Read, Seek};

const PARAMETER_GET_OPERATION: u8 = 0;
const PARAMETER_SET_OPERATION: u8 = 1;
const ADVANCED_LIVE_REVERSAL_IDS: [CameraParameter; 2] = [
    CameraParameter::AdvanceMode,
    CameraParameter::from_id(0x0001),
];

/// Numeric parameter ID, including unrecognized model extensions.
///
/// Named constants retain their existing spelling for compatibility.
/// [`Self::NANO_PARAMETERS`] describes the bounded Nano 10.00.31.22 catalogue;
/// other named IDs do not establish Nano support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct CameraParameter(u16);

impl CameraParameter {
    pub const fn from_id(id: u16) -> Self {
        Self(id)
    }

    pub const fn id(self) -> u16 {
        self.0
    }
}

impl From<u16> for CameraParameter {
    fn from(id: u16) -> Self {
        Self::from_id(id)
    }
}

impl From<CameraParameter> for u16 {
    fn from(parameter: CameraParameter) -> Self {
        parameter.id()
    }
}

macro_rules! camera_parameters {
    (
        scalar { $( $(#[$scalar_doc:meta])* $scalar:ident = $scalar_id:literal, )* }
        word { $( $(#[$word_doc:meta])* $word:ident = $word_id:literal, )* }
        dword { $( $(#[$dword_doc:meta])* $dword:ident = $dword_id:literal, )* }
        bytes { $( $(#[$bytes_doc:meta])* $bytes:ident = $bytes_id:literal, )* }
    ) => {
        #[allow(non_upper_case_globals)]
        impl CameraParameter {
            $( $(#[$scalar_doc])* pub const $scalar: Self = Self($scalar_id); )*
            $( $(#[$word_doc])* pub const $word: Self = Self($word_id); )*
            $( $(#[$dword_doc])* pub const $dword: Self = Self($dword_id); )*
            $( $(#[$bytes_doc])* pub const $bytes: Self = Self($bytes_id); )*

            fn is_known(self) -> bool {
                match self {
                    $( Self::$scalar => true, )*
                    $( Self::$word => true, )*
                    $( Self::$dword => true, )*
                    $( Self::$bytes => true, )*
                    _ => false,
                }
            }

            fn scalar_width(self) -> Option<usize> {
                match self {
                    $( Self::$scalar => Some(1), )*
                    $( Self::$word => Some(2), )*
                    $( Self::$dword => Some(4), )*
                    _ => None,
                }
            }
        }
    };
}

camera_parameters! {
    scalar {
        /// Advanced-mode code: GET 0/1, SET 0..=2; not live reversal.
        ///
        /// SET also updates voice control after the primary service call.
        AdvanceMode = 0x0000,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        Beauty = 0x0002,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        FpvControl = 0x0003,
        /// Raw stream-quality code: SET 0=Default, 1=Normal, 2=High.
        ///
        /// No bitrate or resolution mapping is assigned; GET retains other codes.
        StreamQuality = 0x0006,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        PreviousMode = 0x0007,
        /// Recording stabilization code; Nano SET accepts 0..=7, not just 0/1.
        ///
        /// A zero GET code alone does not establish that stabilization is disabled.
        Eis = 0x0008,
        /// Parameter FOV code; SET accepts 0..=5, with legacy candidate labels.
        FieldOfView = 0x0009,
        /// One-byte voice-control code; checked SET accepts 0/1.
        VoiceControl = 0x000a,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        QuickModeSwitch = 0x000b,
        /// SET accepts 0/1; language labels remain unspecified.
        VoiceLanguage = 0x000e,
        /// Raw ISO-limit code, distinct from user ISO; GET is a cached byte.
        ///
        /// SET replaces an indexed byte in native state; physical ISO labels,
        /// index meanings and active mode restrictions are unspecified.
        IsoLimit = 0x000f,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        ScreenSleep = 0x0011,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        AutoOff = 0x0012,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        SpotMetering = 0x0015,
        /// Read-only code: provider 1 becomes 2; other provider values become 0.
        ///
        /// This is not a lens capability enumeration.
        LensControl = 0x001e,
        /// Unmapped one-byte control; SET codes are 1, 2, 4, 8 or 16.
        ///
        /// Those wire codes map to native ordinals 0..=4, not a bitmask SET.
        Unknown001f = 0x001f,
        /// Raw one-byte audio-channel code; channel labels remain unspecified.
        RecordingAudioChannel = 0x0020,
        /// One-byte tracking code; checked SET retains the 0/1 compatibility subset.
        SelfieFollowing = 0x0024,
        /// Unmapped one-byte control: SET 0/1; GET tests whether native value is 1.
        Unknown0028 = 0x0028,
        /// Independent preference: Nano SET accepts 0/1; GET retains the raw byte.
        ///
        /// The two preference meanings are unspecified; this is not an EIS mode.
        StabilizationPreference = 0x0030,
        /// Image-adjustment code; SET 0..=3, without assigned image-effect labels.
        ImageAdjustment = 0x0033,
        /// Shutter-limit code: SET 0..=5, not a physical exposure time.
        ShutterLimit = 0x0034,
        /// Video-enhancement code; SET 0/1, with mode availability unspecified.
        VideoQualityEnhancement = 0x0035,
        /// Raw mirror code forwarded unchanged; affected output is unspecified.
        SelfieMirror = 0x0038,
        /// Recording-plan code; SET 0..=2, with no assigned plan labels.
        RecordingPlan = 0x003d,
        /// Raw live-view stabilization code, distinct from parameter `0008`.
        ///
        /// Nano SET accepts 0..=7. Availability and effects on preview are unverified.
        LiveViewEis = 0x003e,
    }
    word {
        /// Legacy LED value; individual bit meanings are unspecified.
        Led = 0x0010,
        /// Legacy interval value; time units remain unspecified.
        CaptureMinimumInterval = 0x0017,
    }
    dword {
        /// Four-byte raw GET record; inspected stores assign only its low LE16.
        ///
        /// Low-word codes are 0, 300, 1200, 3600 or 65535; units and the
        /// remaining two GET bytes are unspecified. Checked SET accepts only
        /// those five codes, zero-extended into the four-byte wire value.
        LoopDuration = 0x0018,
        /// LE32 aspect type; SET rejects values above 255 to avoid truncation.
        ///
        /// The native provider uses one byte; physical ratio labels are unspecified.
        CaptureAspectType = 0x0046,
    }
    bytes {
        /// Read-only three-byte rotation record; angle units remain unspecified.
        Unknown0014 = 0x0014,
        /// Nine-byte short-video GET: flags, LE32 value, LE32 value.
        ///
        /// Checked SET exposes only flag 1 with a raw original-duration value
        /// and a zero inactive word. Units and the other write branch are unknown.
        ShortVideoParameters = 0x0029,
        /// Read-only 13-byte storage outcome; individual field meanings are unknown.
        StorageOutcome = 0x0032,
        /// LE16 count 15, followed by 15 entries: LE16 index, length 1, raw byte.
        ///
        /// Checked SET accepts this canonical full record, not partial patches.
        /// Per-index effect names and scales remain unspecified.
        BeautyEffect = 0x0039,
        /// Sensitivity record: tag 1, raw code; SET forwards the second byte.
        AutofocusSensitivity = 0x003a,
        /// Strategy record: tag 1, raw code; SET forwards the second byte.
        AutofocusStrategy = 0x003b,
        /// Unmapped LE16 code: 0, 5, 10, 15, 30, 60, 120 or 300.
        ///
        /// These map to native ordinals 0..=7; no physical unit is assigned.
        Unknown003c = 0x003c,
        /// Write-only camera streaming-parameter trigger.
        ///
        /// Checked SET accepts only 11 bytes, all zero except byte 3 equal to 1:
        /// a zero-destination profile subset. Other fields are not forwarded by
        /// this camera path. This is not RTMP configuration, start or stop, and
        /// does not establish an applied setting or active-mode support.
        Unknown0019 = 0x0019,
        /// Legacy identifier; not in the confirmed Nano catalogue.
        P2PShare = 0x0021,
        /// Legacy identifier; the Nano case is a no-op.
        VideoBeautify = 0x0025,
        /// Write-only TC control: tag 1, selector, then raw data.
        ///
        /// Selectors 1..=3 carry one raw byte (three bytes total); selector 4
        /// carries four raw bytes (six total). Physical meanings are unspecified.
        Unknown0026 = 0x0026,
        /// Raw calibration record; checked SET validates byte structure only.
        ///
        /// Checked SET exposes only operation 3: zero metadata, a length-prefixed
        /// NUL-terminated nonempty ASCII label, LE16 extra length 4, a raw LE16
        /// focal-length code and two zero bytes. Labels have at most 127 chars.
        /// The label is parser-required but is not forwarded by this operation.
        /// Printable ASCII, zero metadata and zero padding are host restrictions.
        /// This does not establish units, a calibration procedure, provider
        /// support or safe execution. Operations 0..=2 remain unavailable because
        /// the inspected providers terminate on their native selectors.
        LensUserCalibration = 0x002b,
        /// Read-only packed state, byte length, then an opaque NUL-terminated string.
        ///
        /// The raw record is retained even when its internal lengths are invalid.
        LensUserCalibrationState = 0x002c,
        /// Raw calibration-file action, LE16 data length, then named records.
        ///
        /// Operations 0/2 carry one length-prefixed NUL-terminated nonempty
        /// ASCII name; operation 1 carries two. Each name has at most 127 chars,
        /// and the complete value is at most 255 bytes. Other operations,
        /// trailing data and inconsistent lengths are rejected.
        /// Nonempty printable ASCII names are a conservative host restriction.
        /// This checks the byte contract, not file roles, permitted paths,
        /// calibration prerequisites, provider support or a safe procedure.
        LensUserCalibrationFile = 0x002d,
        /// Write-only custom-mode record: raw index, operation 0..=3.
        ///
        /// Checked SET requires exactly two bytes. Index availability and
        /// persistence effects remain mode-dependent and are not inferred.
        AddCustomMode = 0x002f,
        /// Write-only synchronization: source 0..=3, destination 0..=3, operation.
        ///
        /// Operation 1 starts. The canonical stop record is `[0, 0, 2]`.
        /// Endpoint labels, accepted combinations and completion remain unspecified.
        StorageSynchronization = 0x0031,
        /// Write-only motionlapse preview: raw code, action (0 preview, 1 interrupt).
        /// Current-mode support and the raw code's physical meaning are unspecified.
        TimelapseTrackPreview = 0x0036,
        /// Write-only one-byte motionlapse mode code; available modes are unspecified.
        TimelapseTrackSet = 0x0037,
    }
}

/// A lossless ID and wire value, shared by GET replies and SET inputs.
///
/// Declared scalar layouts use `U8`, `U16` or `U32`. Structured records,
/// unknown IDs and mismatched widths use `Bytes` without interpreting fields.
/// Integer values encode little-endian; enum memory layout is not a wire codec.
/// [`CameraParameterSet::try_from`] checks the ID, shape and value before SET.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(map = |tlv: ParameterTlv| Self::from_raw(tlv.parameter, tlv.bytes))]
#[bw(map = Self::tlv)]
pub enum CameraParameterValue {
    U8(CameraParameter, u8),
    U16(CameraParameter, u16),
    U32(CameraParameter, u32),
    Bytes(CameraParameter, Vec<u8>),
}

impl CameraParameterValue {
    pub fn parameter(&self) -> CameraParameter {
        match self {
            Self::U8(parameter, _)
            | Self::U16(parameter, _)
            | Self::U32(parameter, _)
            | Self::Bytes(parameter, _) => *parameter,
        }
    }

    /// Decode known integer shapes; retain everything else byte-for-byte.
    ///
    /// No SET bounds are applied. Oversized byte records remain representable
    /// but cannot be encoded as a TLV with a one-byte length.
    pub fn from_raw(parameter: CameraParameter, bytes: Vec<u8>) -> Self {
        match (parameter.scalar_width(), bytes.as_slice()) {
            (Some(1), [value]) => Self::U8(parameter, *value),
            (Some(2), [_, _]) => Self::U16(parameter, crate::protocol::u16_at(&bytes, 0)),
            (Some(4), [_, _, _, _]) => Self::U32(parameter, crate::protocol::u32_at(&bytes, 0)),
            _ => Self::Bytes(parameter, bytes),
        }
    }

    /// Return the value bytes only, without the ID or TLV length.
    pub fn value_bytes(&self) -> Vec<u8> {
        match self {
            Self::U8(_, value) => vec![*value],
            Self::U16(_, value) => encode(&value).expect("integer encodes into memory"),
            Self::U32(_, value) => encode(&value).expect("integer encodes into memory"),
            Self::Bytes(_, bytes) => bytes.clone(),
        }
    }

    /// Return a raw one-byte code, not a physical unit or Boolean state.
    pub fn scalar_code(&self) -> Option<u8> {
        match self {
            Self::U8(_, value) => Some(*value),
            _ => None,
        }
    }

    fn value_length(&self) -> usize {
        match self {
            Self::U8(..) => 1,
            Self::U16(..) => 2,
            Self::U32(..) => 4,
            Self::Bytes(_, bytes) => bytes.len(),
        }
    }

    fn tlv(&self) -> ParameterTlv {
        ParameterTlv {
            parameter: self.parameter(),
            bytes: self.value_bytes(),
        }
    }
}

#[binrw::binrw]
#[brw(little)]
struct ParameterTlv {
    parameter: CameraParameter,
    #[br(temp)]
    #[bw(try_calc = u8::try_from(bytes.len()).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    #[br(count = length)]
    bytes: Vec<u8>,
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

    /// Width of an available checked SET codec, not all firmware write cases.
    ///
    /// `None` means no single fixed width: unavailable or a multi-width codec.
    /// A width does not establish active-mode support or authorize transmission.
    pub fn nano_checked_set_value_length(self) -> Option<usize> {
        match self.nano_checked_set_value_lengths() {
            [length] => Some(*length),
            _ => None,
        }
    }

    /// Available checked SET widths; an empty slice means no checked codec.
    ///
    /// Width alone is insufficient: conversion also validates tags, selectors,
    /// bounds and cross-field constraints. This authorizes no transmission.
    pub fn nano_checked_set_value_lengths(self) -> &'static [usize] {
        match self {
            Self::AdvanceMode
            | Self::StreamQuality
            | Self::Eis
            | Self::FieldOfView
            | Self::VoiceControl
            | Self::VoiceLanguage
            | Self::IsoLimit
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
            | Self::LiveViewEis
            | Self::TimelapseTrackSet => &[1],
            Self::AutofocusSensitivity
            | Self::AutofocusStrategy
            | Self::Unknown003c
            | Self::AddCustomMode
            | Self::TimelapseTrackPreview => &[2],
            Self::LoopDuration | Self::CaptureAspectType => &[4],
            Self::BeautyEffect => &[62],
            Self::ShortVideoParameters => &[9],
            Self::StorageSynchronization => &[3],
            Self::Unknown0026 => &[3, 6],
            Self::Unknown0019 => &[11],
            Self::LensUserCalibration => &LENS_CALIBRATION_SET_WIDTHS,
            Self::LensUserCalibrationFile => &LENS_CALIBRATION_FILE_SET_WIDTHS,
            _ => &[],
        }
    }
}

/// Legacy field-of-view labels for one-byte parameter `0x0009`.
///
/// These names are compatibility candidates, not a guarantee of camera
/// support or a mapping for the separate `0x02/0x18` format field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CameraFieldOfView {
    #[brw(magic = 0u8)]
    UltraWide,
    #[brw(magic = 1u8)]
    Wide,
    #[brw(magic = 2u8)]
    Standard,
    #[brw(magic = 5u8)]
    NaturalWide,
    Unknown(u8),
}

/// Legacy stabilization labels for one-byte parameters `0x0008` and `0x003E`.
///
/// Names do not establish device support or observed stabilization behavior.
/// Nano SET accepts additional raw codes 5..=7, which remain [`Self::Unknown`]
/// rather than acquiring unverified physical mode names. Unknown GET codes
/// are preserved even when they fall outside that SET range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum CameraStabilizationMode {
    #[brw(magic = 0u8)]
    Off,
    #[brw(magic = 1u8)]
    RockSteady,
    #[brw(magic = 2u8)]
    HorizonSteady,
    #[brw(magic = 3u8)]
    RockSteadyPlus,
    #[brw(magic = 4u8)]
    HorizonBalancing,
    Unknown(u8),
}

/// Three raw codes from parameter `0x0014`, without angle units or lock labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraRotationState {
    pub mode_code: u8,
    pub device_angle_code: u8,
    pub sensor_angle_code: u8,
}

impl CameraParameterValue {
    /// Return the two-byte LED value; individual bit meanings are unspecified.
    pub fn led_raw(&self) -> Option<u16> {
        match self {
            Self::U16(CameraParameter::Led, value) => Some(*value),
            _ => None,
        }
    }

    /// Build the complete indexed beauty record without assigning effect labels.
    pub fn beauty_effect(values: [u8; 15]) -> Self {
        let mut bytes = [0; 62];
        crate::protocol::put16(&mut bytes, 0, 15);
        for ((entry, value), index) in bytes[2..]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(values)
            .zip(0_u16..)
        {
            crate::protocol::put16(entry, 0, index);
            entry[2] = 1;
            entry[3] = value;
        }
        Self::Bytes(CameraParameter::BeautyEffect, bytes.to_vec())
    }

    /// Build the original-duration branch with an explicit integer code.
    ///
    /// No time unit is assigned. Flag 1 selects this value; the inactive word
    /// is zero. This does not expose the separate added-duration write branch.
    pub fn short_video_original_duration_raw(code: u32) -> Self {
        let mut bytes = [0; 9];
        bytes[0] = 1;
        bytes[1..5].copy_from_slice(&encode(&code).expect("integer encodes into memory"));
        Self::Bytes(CameraParameter::ShortVideoParameters, bytes.to_vec())
    }

    /// View a canonical full beauty record; malformed GET records stay retained.
    pub fn beauty_effect_values(&self) -> Option<[u8; 15]> {
        let Self::Bytes(CameraParameter::BeautyEffect, bytes) = self else {
            return None;
        };
        if bytes.len() != 62 || crate::protocol::u16_at(bytes, 0) != 15 {
            return None;
        }
        let mut values = [0; 15];
        for (index, entry) in bytes[2..].as_chunks::<4>().0.iter().enumerate() {
            if usize::from(crate::protocol::u16_at(entry, 0)) != index || entry[2] != 1 {
                return None;
            }
            values[index] = entry[3];
        }
        Some(values)
    }

    /// Return the capture-interval value without assuming a time unit.
    pub fn capture_minimum_interval_raw(&self) -> Option<u16> {
        match self {
            Self::U16(CameraParameter::CaptureMinimumInterval, value) => Some(*value),
            _ => None,
        }
    }

    /// Return the four-byte loop-duration value without assuming a time unit.
    pub fn loop_duration_raw(&self) -> Option<u32> {
        match self {
            Self::U32(CameraParameter::LoopDuration, value) => Some(*value),
            _ => None,
        }
    }

    pub fn rotation_state(&self) -> Option<CameraRotationState> {
        let Self::Bytes(CameraParameter::Unknown0014, bytes) = self else {
            return None;
        };
        let [mode, device, sensor] = bytes.as_slice() else {
            return None;
        };
        Some(CameraRotationState {
            mode_code: *mode,
            device_angle_code: *device,
            sensor_angle_code: *sensor,
        })
    }

    /// Apply legacy labels to one-byte FOV data; unknown codes are retained.
    pub fn field_of_view(&self) -> Option<CameraFieldOfView> {
        match self {
            Self::U8(CameraParameter::FieldOfView, value) => Some(
                decode::<CameraFieldOfView>(&[*value], ())
                    .expect("field-of-view fallback accepts every byte"),
            ),
            _ => None,
        }
    }

    /// Apply legacy labels to recording or live-view stabilization data.
    ///
    /// Unknown codes are retained. The controls are independent, and a zero
    /// recording-stabilization GET code does not prove a disabled physical state.
    pub fn stabilization_mode(&self) -> Option<CameraStabilizationMode> {
        match self {
            Self::U8(CameraParameter::Eis | CameraParameter::LiveViewEis, value) => Some(
                decode::<CameraStabilizationMode>(&[*value], ())
                    .expect("stabilization-mode fallback accepts every byte"),
            ),
            _ => None,
        }
    }

    /// Legacy nonzero test for selected parameters, not a validated state enum.
    pub fn enabled(&self) -> Option<bool> {
        match self {
            Self::U8(
                CameraParameter::VoiceControl
                | CameraParameter::QuickModeSwitch
                | CameraParameter::SpotMetering
                | CameraParameter::SelfieFollowing,
                value,
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
#[derive(Clone, Debug, PartialEq, Eq, BinWrite)]
#[bw(try_map = Self::wire_fields)]
pub struct CameraParameterValues(pub Vec<CameraParameterValue>);

impl CameraParameterValues {
    fn wire_fields(&self) -> Result<(u8, u8, &[CameraParameterValue])> {
        let count =
            u8::try_from(self.0.len()).map_err(|_| Error::LimitExceeded("too many camera TLVs"))?;
        Ok((PARAMETER_GET_OPERATION, count, &self.0))
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
#[binrw::binwrite]
#[derive(Clone, Debug)]
#[bw(little, magic = 0u8)]
pub struct CameraParametersGet {
    #[bw(try_calc = u8::try_from(parameters.len())
        .map_err(|_| Error::LimitExceeded("too many camera parameters")))]
    count: u8,
    parameters: Vec<CameraParameter>,
}

/// Encode one state-changing parameter: operation `1`, count `1`, then a TLV.
///
/// APP -> Camera | 0x02/0x8E | success reply: status byte `00`, not GET TLVs.
///
/// Wraps the same [`CameraParameterValue`] enum returned by GET. Checked
/// conversion accepts only established SET codecs and validates known Nano
/// 10.00.31.22 bounds; it does not establish current-mode availability or effects.
/// Read-only IDs, unsupported shapes and incomplete write contracts cannot
/// become checked SETs. Byte records still require their ID-specific validation.
///
/// Compatibility constructors such as `EisCode` and `EisEnabled` retain their
/// existing call syntax and validate at encoding time. `EisEnabled` emits raw
/// codes 0/1, not a physical on/off guarantee. GET values remain lossless.
///
/// This type is not a read-only [`Query`] and authorizes no transmission.
/// Rotation is read-only and the legacy video-beautify ID has no checked SET.
/// Loop-duration SET uses a checked code, not an unchecked replay of its GET.
///
/// ```
/// use unofficial_osmo_camera::protocol::{
///     encode,
///     commands::camera::cmd_02_8e_parameters::{
///         CameraParameter, CameraParameterSet, CameraParameterValue,
///     },
/// };
///
/// let value = CameraParameterValue::U8(CameraParameter::Eis, 7); // Also a possible decoded GET value.
/// let set = CameraParameterSet::try_from(value)?;
/// assert_eq!(encode(&set)?, [1, 1, 8, 0, 1, 7]); // Encoding only; no device access.
/// # Ok::<(), unofficial_osmo_camera::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, BinWrite)]
#[bw(try_map = Self::wire_fields)]
pub struct CameraParameterSet(CameraParameterValue);

impl CameraParameterSet {
    pub fn value(&self) -> &CameraParameterValue {
        &self.0
    }

    pub fn into_value(self) -> CameraParameterValue {
        self.0
    }

    fn validate(&self) -> Result<()> {
        if let Some(expected) = self.0.parameter().nano_get_value_length()
            && self.0.value_length() != expected
        {
            return Err(Error::Unsupported("camera parameter SET value width"));
        }
        let bound = match &self.0 {
            CameraParameterValue::U8(CameraParameter::Beauty, _)
            | CameraParameterValue::U8(CameraParameter::FpvControl, _)
            | CameraParameterValue::U8(CameraParameter::IsoLimit, _)
            | CameraParameterValue::U16(CameraParameter::Led, _)
            | CameraParameterValue::U8(CameraParameter::ScreenSleep, _)
            | CameraParameterValue::U8(CameraParameter::AutoOff, _) => None,
            CameraParameterValue::U8(CameraParameter::AdvanceMode, code) => {
                Some((*code, 2, "camera advanced-mode code"))
            }
            CameraParameterValue::U8(CameraParameter::RecordingAudioChannel, _)
            | CameraParameterValue::U8(CameraParameter::SelfieMirror, _) => None,
            CameraParameterValue::U8(CameraParameter::Unknown001f, code) => {
                if !matches!(code, 1 | 2 | 4 | 8 | 16) {
                    return Err(Error::UnsupportedValue {
                        field: "camera parameter 001f code",
                        value: *code,
                    });
                }
                None
            }
            CameraParameterValue::U8(CameraParameter::Unknown0028, code) => {
                Some((*code, 1, "camera parameter 0028 code"))
            }
            CameraParameterValue::U8(CameraParameter::ImageAdjustment, code) => {
                Some((*code, 3, "camera image-adjustment code"))
            }
            CameraParameterValue::U8(CameraParameter::ShutterLimit, code) => {
                Some((*code, 5, "camera shutter-limit code"))
            }
            CameraParameterValue::U8(CameraParameter::VideoQualityEnhancement, code) => {
                Some((*code, 1, "camera video-enhancement code"))
            }
            CameraParameterValue::U8(CameraParameter::RecordingPlan, code) => {
                Some((*code, 2, "camera recording-plan code"))
            }
            CameraParameterValue::Bytes(CameraParameter::AutofocusSensitivity, bytes)
            | CameraParameterValue::Bytes(CameraParameter::AutofocusStrategy, bytes) => {
                if bytes[0] != 1 {
                    return Err(Error::UnsupportedValue {
                        field: "camera autofocus record tag",
                        value: bytes[0],
                    });
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::Unknown003c, bytes) => {
                if !matches!(
                    crate::protocol::u16_at(bytes, 0),
                    0 | 5 | 10 | 15 | 30 | 60 | 120 | 300
                ) {
                    return Err(Error::Unsupported("camera parameter 003c code"));
                }
                None
            }
            CameraParameterValue::U32(CameraParameter::CaptureAspectType, code) => {
                if *code > u32::from(u8::MAX) {
                    return Err(Error::Unsupported("camera aspect code would be truncated"));
                }
                None
            }
            CameraParameterValue::U32(CameraParameter::LoopDuration, code) => {
                if !matches!(code, 0 | 300 | 1200 | 3600 | 65535) {
                    return Err(Error::Unsupported("camera loop-duration code"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::Unknown0026, bytes) => {
                if !matches!(bytes.as_slice(), [1, 1..=3, _] | [1, 4, _, _, _, _]) {
                    return Err(Error::Unsupported("camera TC SET record"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::Unknown0019, bytes) => {
                if bytes.as_slice() != [0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0] {
                    return Err(Error::Unsupported("camera streaming-parameter SET subset"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::LensUserCalibration, bytes) => {
                validate_lens_calibration(bytes)?;
                None
            }
            CameraParameterValue::Bytes(CameraParameter::LensUserCalibrationFile, bytes) => {
                validate_lens_calibration_file(bytes)?;
                None
            }
            CameraParameterValue::Bytes(CameraParameter::TimelapseTrackPreview, bytes) => {
                if !matches!(bytes.as_slice(), [_, 0..=1]) {
                    return Err(Error::Unsupported("camera motionlapse preview record"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::TimelapseTrackSet, bytes) => {
                if bytes.len() != 1 {
                    return Err(Error::Unsupported("camera motionlapse mode record"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::StorageSynchronization, bytes) => {
                if !matches!(bytes.as_slice(), [0..=3, 0..=3, 1] | [0, 0, 2]) {
                    return Err(Error::Unsupported("camera storage synchronization record"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::AddCustomMode, bytes) => {
                if !matches!(bytes.as_slice(), [_, 0..=3]) {
                    return Err(Error::Unsupported("camera custom-mode SET record"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::ShortVideoParameters, bytes) => {
                if bytes[0] != 1 || bytes[5..] != [0; 4] {
                    return Err(Error::Unsupported("camera short-video SET variant"));
                }
                None
            }
            CameraParameterValue::Bytes(CameraParameter::BeautyEffect, _) => {
                if self.0.beauty_effect_values().is_none() {
                    return Err(Error::Unsupported("camera beauty record is not canonical"));
                }
                None
            }
            CameraParameterValue::U8(CameraParameter::StreamQuality, code) => {
                Some((*code, 2, "camera stream-quality code"))
            }
            CameraParameterValue::U8(CameraParameter::Eis, code) => {
                Some((*code, 7, "camera stabilization code"))
            }
            CameraParameterValue::U8(CameraParameter::LiveViewEis, code) => {
                Some((*code, 7, "camera live-view stabilization code"))
            }
            CameraParameterValue::U8(CameraParameter::StabilizationPreference, code) => {
                Some((*code, 1, "camera stabilization-preference code"))
            }
            CameraParameterValue::U8(CameraParameter::FieldOfView, code) => {
                Some((*code, 5, "camera field-of-view code"))
            }
            CameraParameterValue::U8(CameraParameter::VoiceLanguage, code) => {
                Some((*code, 1, "camera voice-language code"))
            }
            CameraParameterValue::U8(CameraParameter::VoiceControl, code) => {
                Some((*code, 1, "camera voice-control code"))
            }
            CameraParameterValue::U8(CameraParameter::SpotMetering, code) => {
                Some((*code, 1, "camera spot-metering code"))
            }
            CameraParameterValue::U8(CameraParameter::SelfieFollowing, code) => {
                Some((*code, 1, "camera selfie-following code"))
            }
            _ => return Err(Error::Unsupported("parameter has no established SET codec")),
        };
        if let Some((code, maximum, field)) = bound
            && code > maximum
        {
            return Err(Error::UnsupportedValue { field, value: code });
        }
        Ok(())
    }
}

const LENS_CALIBRATION_SET_WIDTHS: [usize; 127] = {
    let mut widths = [0; 127];
    let mut index = 0;
    while index < widths.len() {
        widths[index] = index + 11;
        index += 1;
    }
    widths
};

const LENS_CALIBRATION_FILE_SET_WIDTHS: [usize; 250] = {
    let mut widths = [0; 250];
    let mut index = 0;
    while index < widths.len() {
        widths[index] = index + 6;
        index += 1;
    }
    widths
};

fn calibration_name_tail(bytes: &[u8]) -> Result<&[u8]> {
    let (&length, bytes) = bytes
        .split_first()
        .ok_or(Error::Unsupported("missing calibration name length"))?;
    let (name, tail) = bytes
        .split_at_checked(usize::from(length))
        .ok_or(Error::Unsupported("truncated calibration name"))?;
    let Some((&0, text)) = name.split_last() else {
        return Err(Error::Unsupported("calibration name must end in NUL"));
    };
    if text.is_empty() || text.len() > 127 || !text.iter().all(|byte| matches!(byte, b' '..=b'~')) {
        return Err(Error::Unsupported("calibration name must be bounded ASCII"));
    }
    Ok(tail)
}

fn validate_lens_calibration(bytes: &[u8]) -> Result<()> {
    let [3, 0, name_and_extra @ ..] = bytes else {
        return Err(Error::Unsupported("camera calibration SET variant"));
    };
    let extra = calibration_name_tail(name_and_extra)?;
    if !matches!(extra, [4, 0, _, _, 0, 0]) {
        return Err(Error::Unsupported("calibration focal-length record"));
    }
    Ok(())
}

fn validate_lens_calibration_file(bytes: &[u8]) -> Result<()> {
    if bytes.len() > usize::from(u8::MAX) {
        return Err(Error::Unsupported(
            "calibration-file record exceeds TLV length",
        ));
    }
    let [operation @ 0..=2, low, high, data @ ..] = bytes else {
        return Err(Error::Unsupported("camera calibration-file SET variant"));
    };
    if usize::from(crate::protocol::u16_at(&[*low, *high], 0)) != data.len() {
        return Err(Error::Unsupported("calibration-file data length mismatch"));
    }
    let tail = calibration_name_tail(data)?;
    let tail = if *operation == 1 {
        calibration_name_tail(tail)?
    } else {
        tail
    };
    if !tail.is_empty() {
        return Err(Error::Unsupported("trailing calibration-file data"));
    }
    Ok(())
}

impl TryFrom<CameraParameterValue> for CameraParameterSet {
    type Error = Error;

    fn try_from(value: CameraParameterValue) -> Result<Self> {
        let set = Self(value);
        set.validate()?;
        Ok(set)
    }
}

impl CameraParameterSet {
    fn wire_fields(&self) -> Result<(u8, u8, &CameraParameterValue)> {
        self.validate()?;
        Ok((PARAMETER_SET_OPERATION, 1, &self.0))
    }
}

macro_rules! compatibility_parameter_setters {
    ($( $name:ident($argument:ident: $type:ty) => $value:expr; )*) => {
        #[allow(non_snake_case)]
        impl CameraParameterSet {
            $(
                /// Compatibility constructor; validates when encoded.
                ///
                /// Prefer checked conversion from [`CameraParameterValue`].
                pub fn $name($argument: $type) -> Self {
                    Self($value)
                }
            )*
        }
    };
}

compatibility_parameter_setters! {
    BeautyCode(code: u8) => CameraParameterValue::U8(CameraParameter::Beauty, code);
    FpvControlCode(code: u8) => CameraParameterValue::U8(CameraParameter::FpvControl, code);
    StreamQualityCode(code: u8) => CameraParameterValue::U8(CameraParameter::StreamQuality, code);
    EisEnabled(enabled: bool) => CameraParameterValue::U8(CameraParameter::Eis, u8::from(enabled));
    EisCode(code: u8) => CameraParameterValue::U8(CameraParameter::Eis, code);
    LiveViewEisCode(code: u8) => CameraParameterValue::U8(CameraParameter::LiveViewEis, code);
    StabilizationPreferenceCode(code: u8) => CameraParameterValue::U8(CameraParameter::StabilizationPreference, code);
    FieldOfView(value: CameraFieldOfView) => CameraParameterValue::U8(CameraParameter::FieldOfView, encode(&value).expect("one-byte enum encodes into memory")[0]);
    VoiceControl(enabled: bool) => CameraParameterValue::U8(CameraParameter::VoiceControl, u8::from(enabled));
    VoiceLanguageCode(code: u8) => CameraParameterValue::U8(CameraParameter::VoiceLanguage, code);
    IsoLimitCode(code: u8) => CameraParameterValue::U8(CameraParameter::IsoLimit, code);
    Led(value: u16) => CameraParameterValue::U16(CameraParameter::Led, value);
    ScreenSleepCode(code: u8) => CameraParameterValue::U8(CameraParameter::ScreenSleep, code);
    AutoOffCode(code: u8) => CameraParameterValue::U8(CameraParameter::AutoOff, code);
    SpotMetering(enabled: bool) => CameraParameterValue::U8(CameraParameter::SpotMetering, u8::from(enabled));
    SelfieFollowing(enabled: bool) => CameraParameterValue::U8(CameraParameter::SelfieFollowing, u8::from(enabled));
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
        if !parameter.is_known() {
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

impl Query for CameraParametersGet {
    type Response = CameraReadReply<CameraParameterValues>;

    fn response_args(&self) -> (&Self,) {
        (self,)
    }

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }

    fn command(&self) -> Command {
        Command::Camera(CameraCommand::Parameters)
    }
}

impl BinRead for CameraParameterValues {
    type Args<'a> = (&'a CameraParametersGet,);

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        (query,): Self::Args<'_>,
    ) -> BinResult<Self> {
        let pos = reader.stream_position()?;
        if u8::read_options(reader, endian, ())? != PARAMETER_GET_OPERATION
            || usize::from(u8::read_options(reader, endian, ())?) != query.parameters.len()
        {
            return Err(binrw_error(
                pos,
                Error::Protocol("camera GET operation/count mismatch"),
            ));
        }
        let mut values = Vec::new();
        for expected in &query.parameters {
            let value = CameraParameterValue::read_options(reader, endian, ())?;
            if value.parameter() != *expected {
                return Err(binrw_error(
                    pos,
                    Error::Protocol("camera GET parameter/order mismatch"),
                ));
            }
            values.push(value);
        }
        let mut trailing = [0];
        if reader.read(&mut trailing)? != 0 {
            return Err(binrw_error(
                pos,
                Error::Protocol("trailing camera GET TLVs"),
            ));
        }
        Ok(Self(values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn get_count_is_calculated_and_checked_without_truncating_ids() {
        for count in [0, 1, 255] {
            let query = CameraParametersGet {
                parameters: vec![CameraParameter::Led; count],
            };
            let bytes = encode(&query).unwrap();
            assert_eq!(bytes.len(), 2 + 2 * count);
            assert_eq!(&bytes[..2], &[0, count as u8]);
            assert!(
                bytes[2..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .all(|id| id == &[0x10, 0])
            );
        }
        let query = CameraParametersGet {
            parameters: vec![CameraParameter::Led; 256],
        };
        let mut writer = std::io::Cursor::new(Vec::new());
        writer.get_mut().extend_from_slice(&[0xa5]);
        assert!(matches!(
            encode(&query).map(|bytes| writer.get_mut().extend(bytes)),
            Err(Error::LimitExceeded("too many camera parameters"))
        ));
        assert_eq!(writer.into_inner(), [0xa5]);
    }

    #[test]
    fn shared_values_decode_to_variants_and_reuse_the_same_tlv_for_sets() {
        let fixtures = [
            (
                CameraParameterValue::U8(CameraParameter::StreamQuality, 2),
                vec![6, 0, 1, 2],
            ),
            (
                CameraParameterValue::U8(CameraParameter::Eis, 7),
                vec![8, 0, 1, 7],
            ),
            (
                CameraParameterValue::U8(CameraParameter::LiveViewEis, 6),
                vec![0x3e, 0, 1, 6],
            ),
            (
                CameraParameterValue::U8(CameraParameter::StabilizationPreference, 1),
                vec![0x30, 0, 1, 1],
            ),
            (
                CameraParameterValue::U16(CameraParameter::Led, 0x1234),
                vec![0x10, 0, 2, 0x34, 0x12],
            ),
        ];
        for (value, tlv) in fixtures {
            assert_eq!(decode::<CameraParameterValue>(&tlv, ()).unwrap(), value);
            assert_eq!(encode(&value).unwrap(), tlv);
            let set = CameraParameterSet::try_from(value.clone()).unwrap();
            assert_eq!(set.value(), &value);
            assert_eq!(encode(&set).unwrap(), [vec![1, 1], tlv].concat());
            assert_eq!(set.into_value(), value);
        }
        let query = CameraParametersGet::single_nano(CameraParameter::Eis).unwrap();
        let reply =
            decode::<CameraReadReply<CameraParameterValues>>(&[0, 0, 1, 8, 0, 1, 7], (&query,))
                .unwrap();
        let CameraReadReply::Accepted(CameraParameterValues(values)) = reply else {
            panic!("expected accepted parameter reply");
        };
        assert_eq!(values, [CameraParameterValue::U8(CameraParameter::Eis, 7)]);
        assert_eq!(
            encode(&CameraParameterSet::try_from(values.into_iter().next().unwrap()).unwrap())
                .unwrap(),
            [1, 1, 8, 0, 1, 7]
        );
    }

    #[test]
    fn shared_values_preserve_every_parameter_id_and_do_not_serialize_fallback_tags() {
        for id in u16::MIN..=u16::MAX {
            let [low, high] = id.to_le_bytes();
            let bytes = [low, high, 1, 0xa5];
            let value = decode::<CameraParameterValue>(&bytes, ()).unwrap();
            assert_eq!(u16::from(value.parameter()), id);
            assert_eq!(value.value_bytes(), [0xa5]);
            assert_eq!(encode(&value).unwrap(), bytes);
        }
    }

    #[test]
    fn generic_shapes_cannot_bypass_id_width_or_write_validation() {
        assert_eq!(CameraParameter::from_id(8), CameraParameter::Eis);
        assert_eq!(CameraParameter::Eis.id(), 8);
        for value in [
            CameraParameterValue::U8(CameraParameter::Led, 1),
            CameraParameterValue::U16(CameraParameter::Eis, 1),
            CameraParameterValue::U32(CameraParameter::Eis, 1),
            CameraParameterValue::U16(CameraParameter::AutofocusSensitivity, 1),
            CameraParameterValue::U8(CameraParameter::LensControl, 0),
            CameraParameterValue::U8(CameraParameter::from_id(0x1234), 0),
            CameraParameterValue::U16(CameraParameter::from_id(0x1234), 0),
            CameraParameterValue::U32(CameraParameter::from_id(0x1234), 0),
        ] {
            assert!(CameraParameterSet::try_from(value).is_err());
        }
        for &parameter in CameraParameter::NANO_PARAMETERS {
            for length in 0..=usize::from(u8::MAX) {
                let value = CameraParameterValue::Bytes(parameter, vec![0; length]);
                let mut writer = std::io::Cursor::new(Vec::new());
                writer.get_mut().push(0xa5);
                let result =
                    encode(&CameraParameterSet(value)).map(|bytes| writer.get_mut().extend(bytes));
                if parameter.scalar_width().is_some() {
                    assert!(result.is_err(), "{parameter:?} / {length}");
                }
                if result.is_err() {
                    assert_eq!(writer.get_ref().as_slice(), [0xa5]);
                } else {
                    assert!(parameter.nano_checked_set_value_lengths().contains(&length));
                }
            }
        }
    }

    #[test]
    fn shared_set_conversion_checks_received_codes_without_losing_get_values() {
        for code in u8::MIN..=u8::MAX {
            for (value, maximum) in [
                (
                    CameraParameterValue::U8(CameraParameter::StreamQuality, code),
                    2,
                ),
                (CameraParameterValue::U8(CameraParameter::Eis, code), 7),
                (
                    CameraParameterValue::U8(CameraParameter::LiveViewEis, code),
                    7,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::StabilizationPreference, code),
                    1,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::FieldOfView, code),
                    5,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::VoiceLanguage, code),
                    1,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::VoiceControl, code),
                    1,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::SpotMetering, code),
                    1,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::SelfieFollowing, code),
                    1,
                ),
            ] {
                let bytes = encode(&value).unwrap();
                let decoded = decode::<CameraParameterValue>(&bytes, ()).unwrap();
                assert_eq!(decoded, value);
                let result = CameraParameterSet::try_from(decoded);
                if code <= maximum {
                    let set = result.unwrap();
                    assert_eq!(encode(&set).unwrap(), [vec![1, 1], bytes].concat());
                } else {
                    assert!(
                        matches!(result, Err(Error::UnsupportedValue { value, .. }) if value == code)
                    );
                }
            }
        }
    }

    #[test]
    fn shared_set_conversion_rejects_readonly_unknown_and_incomplete_contracts() {
        let expected = [
            0x0000, 0x0006, 0x0008, 0x0009, 0x000a, 0x000e, 0x000f, 0x0018, 0x0019, 0x001f, 0x0020,
            0x0024, 0x0026, 0x0028, 0x0029, 0x002b, 0x002d, 0x002f, 0x0030, 0x0031, 0x0033, 0x0034,
            0x0035, 0x0036, 0x0037, 0x0038, 0x0039, 0x003a, 0x003b, 0x003c, 0x003d, 0x003e, 0x0046,
        ];
        let actual: Vec<u16> = CameraParameter::NANO_PARAMETERS
            .iter()
            .copied()
            .filter(|parameter| !parameter.nano_checked_set_value_lengths().is_empty())
            .map(u16::from)
            .collect();
        assert_eq!(actual, expected);
        for &parameter in CameraParameter::NANO_PARAMETERS {
            let value = match parameter {
                CameraParameter::Unknown001f => {
                    CameraParameterValue::U8(CameraParameter::Unknown001f, 1)
                }
                CameraParameter::AutofocusSensitivity => {
                    CameraParameterValue::Bytes(CameraParameter::AutofocusSensitivity, vec![1, 0])
                }
                CameraParameter::AutofocusStrategy => {
                    CameraParameterValue::Bytes(CameraParameter::AutofocusStrategy, vec![1, 0])
                }
                CameraParameter::BeautyEffect => CameraParameterValue::beauty_effect([0; 15]),
                CameraParameter::ShortVideoParameters => {
                    CameraParameterValue::short_video_original_duration_raw(0)
                }
                CameraParameter::AddCustomMode => {
                    CameraParameterValue::Bytes(CameraParameter::AddCustomMode, vec![0, 0])
                }
                CameraParameter::Unknown0026 => {
                    CameraParameterValue::Bytes(CameraParameter::Unknown0026, vec![1, 1, 0])
                }
                CameraParameter::Unknown0019 => CameraParameterValue::Bytes(
                    CameraParameter::Unknown0019,
                    vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
                ),
                CameraParameter::LensUserCalibration => CameraParameterValue::Bytes(
                    CameraParameter::LensUserCalibration,
                    vec![3, 0, 2, b'a', 0, 4, 0, 0, 0, 0, 0],
                ),
                CameraParameter::LensUserCalibrationFile => CameraParameterValue::Bytes(
                    CameraParameter::LensUserCalibrationFile,
                    vec![0, 3, 0, 2, b'a', 0],
                ),
                CameraParameter::TimelapseTrackPreview => {
                    CameraParameterValue::Bytes(CameraParameter::TimelapseTrackPreview, vec![0, 0])
                }
                CameraParameter::TimelapseTrackSet => {
                    CameraParameterValue::Bytes(CameraParameter::TimelapseTrackSet, vec![0])
                }
                CameraParameter::StorageSynchronization => CameraParameterValue::Bytes(
                    CameraParameter::StorageSynchronization,
                    vec![0, 0, 2],
                ),
                _ => CameraParameterValue::from_raw(
                    parameter,
                    vec![0; parameter.nano_get_value_length().unwrap_or(0)],
                ),
            };
            let writable = !parameter.nano_checked_set_value_lengths().is_empty();
            if writable {
                assert!(parameter.nano_access().unwrap().can_write());
                assert!(
                    parameter
                        .nano_checked_set_value_lengths()
                        .contains(&value.value_length())
                );
            }
            assert_eq!(
                CameraParameterSet::try_from(value).is_ok(),
                writable,
                "{parameter:?}"
            );
        }
        for value in [
            CameraParameterValue::Bytes(CameraParameter::from_id(0x1234), vec![0]),
            CameraParameterValue::Bytes(CameraParameter::Eis, vec![1]),
            CameraParameterValue::from_raw(CameraParameter::Eis, vec![]),
            CameraParameterValue::from_raw(CameraParameter::Eis, vec![1, 0]),
            CameraParameterValue::U32(CameraParameter::LoopDuration, 1),
            CameraParameterValue::U32(CameraParameter::CaptureAspectType, 256),
        ] {
            assert!(matches!(
                CameraParameterSet::try_from(value),
                Err(Error::Unsupported(_))
            ));
        }
    }

    #[test]
    fn newly_checked_scalar_codes_are_exhaustive_and_raw_gets_stay_lossless() {
        for code in u8::MIN..=u8::MAX {
            for (value, maximum) in [
                (
                    CameraParameterValue::U8(CameraParameter::AdvanceMode, code),
                    2,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::Unknown0028, code),
                    1,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::ImageAdjustment, code),
                    3,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::ShutterLimit, code),
                    5,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::VideoQualityEnhancement, code),
                    1,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::RecordingPlan, code),
                    2,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::RecordingAudioChannel, code),
                    u8::MAX,
                ),
                (
                    CameraParameterValue::U8(CameraParameter::SelfieMirror, code),
                    u8::MAX,
                ),
            ] {
                let bytes = encode(&value).unwrap();
                assert_eq!(decode::<CameraParameterValue>(&bytes, ()).unwrap(), value);
                assert_eq!(
                    CameraParameterSet::try_from(value.clone()).is_ok(),
                    code <= maximum
                );
                if code > maximum {
                    let mut writer = std::io::Cursor::new(Vec::new());
                    writer.get_mut().push(0xa5);
                    assert!(
                        encode(&CameraParameterSet(value))
                            .map(|bytes| writer.get_mut().extend(bytes))
                            .is_err()
                    );
                    assert_eq!(writer.get_ref().as_slice(), [0xa5]);
                }
            }
            assert_eq!(
                CameraParameterSet::try_from(CameraParameterValue::U8(
                    CameraParameter::Unknown001f,
                    code
                ))
                .is_ok(),
                matches!(code, 1 | 2 | 4 | 8 | 16)
            );
        }
    }

    #[test]
    fn sparse_word_codes_and_aspect_values_cannot_be_truncated() {
        for code in u16::MIN..=u16::MAX {
            let value = CameraParameterValue::Bytes(
                CameraParameter::Unknown003c,
                code.to_le_bytes().to_vec(),
            );
            assert_eq!(
                CameraParameterSet::try_from(value).is_ok(),
                matches!(code, 0 | 5 | 10 | 15 | 30 | 60 | 120 | 300)
            );
        }

        for code in 0..=u32::from(u8::MAX) {
            let value = CameraParameterValue::U32(CameraParameter::CaptureAspectType, code);
            let set = CameraParameterSet::try_from(value).unwrap();
            let mut expected = vec![1, 1, 0x46, 0, 4];
            expected.extend(code.to_le_bytes());
            assert_eq!(encode(&set).unwrap(), expected);
        }
        for code in [256, 65535, 65536, u32::MAX] {
            let value = CameraParameterValue::U32(CameraParameter::CaptureAspectType, code);
            assert!(CameraParameterSet::try_from(value.clone()).is_err());
            assert_eq!(
                decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
                value
            );
        }
        let set = CameraParameterSet::try_from(CameraParameterValue::Bytes(
            CameraParameter::Unknown003c,
            300_u16.to_le_bytes().to_vec(),
        ))
        .unwrap();
        assert_eq!(encode(&set).unwrap(), [1, 1, 0x3c, 0, 2, 0x2c, 1]);
    }

    #[test]
    fn loop_duration_set_checks_codes_and_never_replays_unknown_get_upper_bytes() {
        for code in u16::MIN..=u16::MAX {
            let value = CameraParameterValue::U32(CameraParameter::LoopDuration, u32::from(code));
            let set = CameraParameterSet::try_from(value);
            assert_eq!(set.is_ok(), matches!(code, 0 | 300 | 1200 | 3600 | 65535));
            if let Ok(set) = set {
                let [low, high] = code.to_le_bytes();
                assert_eq!(encode(&set).unwrap(), [1, 1, 0x18, 0, 4, low, high, 0, 0]);
            }
        }
        for code in [0x10000, 0x10000 | 300, u32::MAX] {
            let value = CameraParameterValue::U32(CameraParameter::LoopDuration, code);
            assert_eq!(
                decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
                value
            );
            assert!(CameraParameterSet::try_from(value.clone()).is_err());
            let mut writer = std::io::Cursor::new(Vec::new());
            writer.get_mut().push(0xa5);
            assert!(
                encode(&CameraParameterSet(value))
                    .map(|bytes| writer.get_mut().extend(bytes))
                    .is_err()
            );
            assert_eq!(writer.get_ref().as_slice(), [0xa5]);
        }
    }

    #[test]
    fn streaming_parameter_set_only_exposes_the_zero_destination_trigger() {
        let canonical = vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        let value = CameraParameterValue::Bytes(CameraParameter::Unknown0019, canonical.clone());
        let set = CameraParameterSet::try_from(value).unwrap();
        let mut expected = vec![1, 1, 0x19, 0, 11];
        expected.extend(&canonical);
        assert_eq!(encode(&set).unwrap(), expected);
        assert_eq!(
            CameraParameter::Unknown0019.nano_checked_set_value_lengths(),
            [11]
        );
        for index in 0..canonical.len() {
            for byte in u8::MIN..=u8::MAX {
                let mut bytes = canonical.clone();
                bytes[index] = byte;
                let valid = bytes == canonical;
                let value = CameraParameterValue::Bytes(CameraParameter::Unknown0019, bytes);
                assert_eq!(
                    decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
                    value
                );
                assert_eq!(CameraParameterSet::try_from(value.clone()).is_ok(), valid);
                if !valid {
                    let mut writer = std::io::Cursor::new(Vec::new());
                    writer.get_mut().push(0xa5);
                    assert!(
                        encode(&CameraParameterSet(value))
                            .map(|bytes| writer.get_mut().extend(bytes))
                            .is_err()
                    );
                    assert_eq!(writer.get_ref().as_slice(), [0xa5]);
                }
            }
        }
        for length in 0..=usize::from(u8::MAX) {
            let mut bytes = canonical.clone();
            bytes.resize(length, 0);
            assert_eq!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::Unknown0019,
                    bytes
                ))
                .is_ok(),
                length == canonical.len()
            );
        }
    }

    #[test]
    fn calibration_focal_length_checks_all_codes_and_parser_required_names() {
        for code in u16::MIN..=u16::MAX {
            let [low, high] = code.to_le_bytes();
            let bytes = vec![3, 0, 2, b'a', 0, 4, 0, low, high, 0, 0];
            let set = CameraParameterSet::try_from(CameraParameterValue::Bytes(
                CameraParameter::LensUserCalibration,
                bytes.clone(),
            ))
            .unwrap();
            assert_eq!(
                encode(&set).unwrap(),
                [vec![1, 1, 0x2b, 0, 11], bytes].concat()
            );
        }
        for length in 0_usize..=255 {
            let mut bytes = vec![3, 0, u8::try_from(length).unwrap()];
            bytes.extend(vec![b'a'; length.saturating_sub(1)]);
            if length > 0 {
                bytes.push(0);
            }
            bytes.extend([4, 0, 0, 0, 0, 0]);
            assert_eq!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::LensUserCalibration,
                    bytes
                ))
                .is_ok(),
                (2..=128).contains(&length)
            );
        }
    }

    #[test]
    fn calibration_records_reject_unsafe_actions_bad_lengths_and_trailing_data_before_writing() {
        for operation in u8::MIN..=u8::MAX {
            let bytes = vec![operation, 0, 2, b'a', 0, 4, 0, 0, 0, 0, 0];
            assert_eq!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::LensUserCalibration,
                    bytes
                ))
                .is_ok(),
                operation == 3
            );
        }
        let focal = vec![3, 0, 2, b'a', 0, 4, 0, 0, 0, 0, 0];
        let file = vec![0, 3, 0, 2, b'a', 0];
        for (parameter, canonical, protected) in [
            (
                CameraParameter::LensUserCalibration,
                focal,
                vec![1, 2, 4, 5, 6, 9, 10],
            ),
            (
                CameraParameter::LensUserCalibrationFile,
                file,
                vec![1, 2, 3, 5],
            ),
        ] {
            for index in protected {
                for byte in u8::MIN..=u8::MAX {
                    if byte == canonical[index] {
                        continue;
                    }
                    let mut bytes = canonical.clone();
                    bytes[index] = byte;
                    let value = CameraParameterValue::from_raw(parameter, bytes);
                    assert_eq!(
                        decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
                        value
                    );
                    assert!(CameraParameterSet::try_from(value.clone()).is_err());
                    let mut writer = std::io::Cursor::new(Vec::new());
                    writer.get_mut().push(0xa5);
                    assert!(
                        encode(&CameraParameterSet(value))
                            .map(|bytes| writer.get_mut().extend(bytes))
                            .is_err()
                    );
                    assert_eq!(writer.get_ref().as_slice(), [0xa5]);
                }
            }
            for length in 0..canonical.len() {
                assert!(
                    CameraParameterSet::try_from(CameraParameterValue::from_raw(
                        parameter,
                        canonical[..length].to_vec()
                    ))
                    .is_err()
                );
            }
            let mut trailing = canonical;
            trailing.push(0);
            assert!(
                CameraParameterSet::try_from(CameraParameterValue::from_raw(parameter, trailing))
                    .is_err()
            );
        }
        for invalid_name in [0, 0x1f, 0x7f, 0x80, 0xff] {
            let value = CameraParameterValue::Bytes(
                CameraParameter::LensUserCalibrationFile,
                vec![0, 3, 0, 2, invalid_name, 0],
            );
            assert!(CameraParameterSet::try_from(value).is_err());
        }
    }

    #[test]
    fn calibration_file_checks_each_action_two_names_and_every_supported_width() {
        for operation in u8::MIN..=u8::MAX {
            let bytes = if operation == 1 {
                vec![1, 6, 0, 2, b'a', 0, 2, b'b', 0]
            } else {
                vec![operation, 3, 0, 2, b'a', 0]
            };
            let result = CameraParameterSet::try_from(CameraParameterValue::Bytes(
                CameraParameter::LensUserCalibrationFile,
                bytes.clone(),
            ));
            assert_eq!(result.is_ok(), operation <= 2);
            if let Ok(set) = result {
                let mut expected = vec![1, 1, 0x2d, 0, u8::try_from(bytes.len()).unwrap()];
                expected.extend(bytes);
                assert_eq!(encode(&set).unwrap(), expected);
            }
        }
        for first in 1_usize..=128 {
            for second in 1_usize..=128 {
                let mut data = vec![u8::try_from(first + 1).unwrap()];
                data.extend(vec![b'a'; first]);
                data.push(0);
                data.push(u8::try_from(second + 1).unwrap());
                data.extend(vec![b'b'; second]);
                data.push(0);
                let length = data.len();
                let mut bytes = vec![1];
                bytes.extend(u16::try_from(length).unwrap().to_le_bytes());
                bytes.extend(data);
                assert_eq!(
                    validate_lens_calibration_file(&bytes).is_ok(),
                    first <= 127 && second <= 127 && length + 3 <= 255
                );
            }
        }
        for &width in CameraParameter::LensUserCalibrationFile.nano_checked_set_value_lengths() {
            let bytes = if width <= 132 {
                let chars = width - 5;
                let mut bytes = vec![0];
                bytes.extend(u16::try_from(width - 3).unwrap().to_le_bytes());
                bytes.push(u8::try_from(chars + 1).unwrap());
                bytes.extend(vec![b'a'; chars]);
                bytes.push(0);
                bytes
            } else {
                let first = (width - 8).min(127);
                let second = width - 7 - first;
                let mut bytes = vec![1];
                bytes.extend(u16::try_from(width - 3).unwrap().to_le_bytes());
                bytes.push(u8::try_from(first + 1).unwrap());
                bytes.extend(vec![b'a'; first]);
                bytes.push(0);
                bytes.push(u8::try_from(second + 1).unwrap());
                bytes.extend(vec![b'b'; second]);
                bytes.push(0);
                bytes
            };
            assert_eq!(bytes.len(), width);
            assert!(validate_lens_calibration_file(&bytes).is_ok(), "{width}");
        }
    }

    #[test]
    fn short_video_set_only_exposes_the_original_duration_branch() {
        for code in [0, 1, 0x0102_0304, u32::MAX] {
            let value = CameraParameterValue::short_video_original_duration_raw(code);
            let set = CameraParameterSet::try_from(value).unwrap();
            let mut expected = vec![1, 1, 0x29, 0, 9, 1];
            expected.extend(code.to_le_bytes());
            expected.extend([0; 4]);
            assert_eq!(encode(&set).unwrap(), expected);
        }
        for flags in u8::MIN..=u8::MAX {
            let mut bytes = [0; 9];
            bytes[0] = flags;
            let value =
                CameraParameterValue::Bytes(CameraParameter::ShortVideoParameters, bytes.to_vec());
            assert_eq!(
                CameraParameterSet::try_from(value.clone()).is_ok(),
                flags == 1
            );
            assert_eq!(
                decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
                value
            );
        }
        for position in 5..9 {
            let mut bytes = [0; 9];
            bytes[0] = 1;
            bytes[position] = 1;
            let value =
                CameraParameterValue::Bytes(CameraParameter::ShortVideoParameters, bytes.to_vec());
            let mut writer = std::io::Cursor::new(Vec::new());
            writer.get_mut().push(0xa5);
            assert!(
                encode(&CameraParameterSet(value))
                    .map(|bytes| writer.get_mut().extend(bytes))
                    .is_err()
            );
            assert_eq!(writer.get_ref().as_slice(), [0xa5]);
        }
    }

    #[test]
    fn custom_mode_set_preserves_indices_and_requires_a_complete_bounded_operation() {
        for index in u8::MIN..=u8::MAX {
            for operation in u8::MIN..=u8::MAX {
                let value = CameraParameterValue::Bytes(
                    CameraParameter::AddCustomMode,
                    vec![index, operation],
                );
                let set = CameraParameterSet::try_from(value);
                assert_eq!(set.is_ok(), operation <= 3);
                if let Ok(set) = set {
                    assert_eq!(encode(&set).unwrap(), [1, 1, 0x2f, 0, 2, index, operation]);
                }
            }
        }
        for bytes in [vec![], vec![0], vec![0, 0, 0]] {
            assert!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::AddCustomMode,
                    bytes
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn motionlapse_records_validate_actions_without_normalizing_mode_codes() {
        for code in u8::MIN..=u8::MAX {
            let mode = CameraParameterValue::Bytes(CameraParameter::TimelapseTrackSet, vec![code]);
            assert_eq!(
                encode(&CameraParameterSet::try_from(mode).unwrap()).unwrap(),
                [1, 1, 0x37, 0, 1, code]
            );
            for action in u8::MIN..=u8::MAX {
                let preview = CameraParameterValue::Bytes(
                    CameraParameter::TimelapseTrackPreview,
                    vec![code, action],
                );
                let result = CameraParameterSet::try_from(preview);
                assert_eq!(result.is_ok(), action <= 1);
                if let Ok(set) = result {
                    assert_eq!(encode(&set).unwrap(), [1, 1, 0x36, 0, 2, code, action]);
                }
            }
        }
        for bytes in [vec![], vec![0], vec![0, 0, 0]] {
            assert!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::TimelapseTrackPreview,
                    bytes
                ))
                .is_err()
            );
        }
        for bytes in [vec![], vec![0, 0]] {
            assert!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::TimelapseTrackSet,
                    bytes
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn storage_sync_start_bounds_endpoints_and_stop_is_canonical() {
        for source in 0..=3 {
            for destination in 0..=3 {
                let value = CameraParameterValue::Bytes(
                    CameraParameter::StorageSynchronization,
                    vec![source, destination, 1],
                );
                assert_eq!(
                    encode(&CameraParameterSet::try_from(value).unwrap()).unwrap(),
                    [1, 1, 0x31, 0, 3, source, destination, 1]
                );
            }
        }
        for invalid in 4..=u8::MAX {
            for bytes in [vec![invalid, 0, 1], vec![0, invalid, 1]] {
                assert!(
                    CameraParameterSet::try_from(CameraParameterValue::Bytes(
                        CameraParameter::StorageSynchronization,
                        bytes
                    ))
                    .is_err()
                );
            }
        }
        for operation in u8::MIN..=u8::MAX {
            let value = CameraParameterValue::Bytes(
                CameraParameter::StorageSynchronization,
                vec![0, 0, operation],
            );
            assert_eq!(
                CameraParameterSet::try_from(value).is_ok(),
                matches!(operation, 1 | 2)
            );
        }
        for bytes in [
            vec![],
            vec![0, 0],
            vec![0, 0, 2, 0],
            vec![1, 0, 2],
            vec![0, 1, 2],
        ] {
            assert!(
                CameraParameterSet::try_from(CameraParameterValue::Bytes(
                    CameraParameter::StorageSynchronization,
                    bytes
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn tc_variable_widths_match_selector_and_keep_raw_data() {
        assert_eq!(
            CameraParameter::Unknown0026.nano_checked_set_value_lengths(),
            [3, 6]
        );
        assert_eq!(
            CameraParameter::Unknown0026.nano_checked_set_value_length(),
            None
        );
        for tag in [0, 1, 255] {
            for selector in u8::MIN..=u8::MAX {
                for length in [0, 1, 2, 3, 4, 5, 6, 7, 255] {
                    let mut bytes = vec![255; length];
                    if let Some(first) = bytes.first_mut() {
                        *first = tag;
                    }
                    if let Some(second) = bytes.get_mut(1) {
                        *second = selector;
                    }
                    let valid = tag == 1
                        && ((length == 3 && (1..=3).contains(&selector))
                            || (length == 6 && selector == 4));
                    let value = CameraParameterValue::Bytes(CameraParameter::Unknown0026, bytes);
                    assert_eq!(CameraParameterSet::try_from(value).is_ok(), valid);
                }
            }
        }
        for code in u8::MIN..=u8::MAX {
            for selector in 1..=4 {
                let mut bytes = vec![1, selector, code];
                if selector == 4 {
                    bytes.extend([code; 3]);
                }
                let length = if selector == 4 { 6 } else { 3 };
                let value =
                    CameraParameterValue::Bytes(CameraParameter::Unknown0026, bytes.clone());
                let mut expected = vec![1, 1, 0x26, 0, length];
                expected.extend(bytes);
                assert_eq!(
                    encode(&CameraParameterSet::try_from(value).unwrap()).unwrap(),
                    expected
                );
            }
        }
    }

    #[test]
    fn autofocus_records_require_tag_one_without_normalizing_raw_codes() {
        for tag in u8::MIN..=u8::MAX {
            for code in [0, 255] {
                for value in [
                    CameraParameterValue::Bytes(
                        CameraParameter::AutofocusSensitivity,
                        vec![tag, code],
                    ),
                    CameraParameterValue::Bytes(
                        CameraParameter::AutofocusStrategy,
                        vec![tag, code],
                    ),
                ] {
                    assert_eq!(CameraParameterSet::try_from(value).is_ok(), tag == 1);
                }
            }
        }
        for code in u8::MIN..=u8::MAX {
            for value in [
                CameraParameterValue::Bytes(CameraParameter::AutofocusSensitivity, vec![1, code]),
                CameraParameterValue::Bytes(CameraParameter::AutofocusStrategy, vec![1, code]),
            ] {
                let id = u16::from(value.parameter()).to_le_bytes();
                assert_eq!(
                    encode(&CameraParameterSet::try_from(value).unwrap()).unwrap(),
                    [1, 1, id[0], id[1], 2, 1, code]
                );
            }
        }
    }

    #[test]
    fn full_beauty_records_validate_every_count_index_and_length_field() {
        let values = [0xfe; 15];
        let value = CameraParameterValue::beauty_effect(values);
        assert_eq!(value.beauty_effect_values(), Some(values));
        let set = CameraParameterSet::try_from(value.clone()).unwrap();
        let encoded = encode(&set).unwrap();
        assert_eq!(&encoded[..5], [1, 1, 0x39, 0, 62]);
        assert_eq!(&encoded[5..7], [15, 0]);
        assert_eq!(encoded.len(), 67);
        assert_eq!(
            decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
            value
        );
        let CameraParameterValue::Bytes(CameraParameter::BeautyEffect, bytes) = value else {
            panic!("expected beauty record");
        };
        for position in [0, 1]
            .into_iter()
            .chain((0..15).flat_map(|index| [2 + index * 4, 3 + index * 4, 4 + index * 4]))
        {
            let mut malformed = bytes.clone();
            malformed[position] ^= 0x80;
            let value = CameraParameterValue::Bytes(CameraParameter::BeautyEffect, malformed);
            assert_eq!(value.beauty_effect_values(), None);
            assert!(CameraParameterSet::try_from(value.clone()).is_err());
            assert_eq!(
                decode::<CameraParameterValue>(&encode(&value).unwrap(), ()).unwrap(),
                value
            );
        }
    }

    #[test]
    fn shared_value_layouts_preserve_integer_shapes_records_and_mismatched_widths() {
        for &parameter in CameraParameter::NANO_PARAMETERS {
            for length in 0..=u8::MAX {
                let [low, high] = u16::from(parameter).to_le_bytes();
                let mut bytes = vec![low, high, length];
                bytes.extend(vec![0xa5; usize::from(length)]);
                let value = decode::<CameraParameterValue>(&bytes, ()).unwrap();
                assert_eq!(value.parameter(), parameter);
                assert_eq!(encode(&value).unwrap(), bytes);
                assert_eq!(
                    matches!(value, CameraParameterValue::Bytes(..)),
                    parameter.scalar_width() != Some(usize::from(length))
                );
            }
        }
        assert_eq!(
            decode::<CameraParameterValue>(&[0x14, 0, 3, 0xfe, 2, 0], ()).unwrap(),
            CameraParameterValue::Bytes(CameraParameter::Unknown0014, vec![0xfe, 2, 0])
        );
        assert_eq!(
            decode::<CameraParameterValue>(&[0x46, 0, 4, 0x78, 0x56, 0x34, 0x12], ()).unwrap(),
            CameraParameterValue::U32(CameraParameter::CaptureAspectType, 0x12345678)
        );
        for bytes in [
            &[][..],
            &[8][..],
            &[8, 0][..],
            &[8, 0, 2, 1][..],
            &[8, 0, 0, 1][..],
        ] {
            assert!(decode::<CameraParameterValue>(bytes, ()).is_err());
        }
    }

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
            assert_eq!(encode(&parameter).unwrap(), id.to_le_bytes());
            assert_eq!(
                decode::<CameraParameter>(&id.to_le_bytes(), ()).unwrap(),
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
            assert_eq!(encode(&query).unwrap(), [0, 1, id[0], id[1]]);
            let length = parameter.nano_get_value_length().unwrap_or(7);
            let record = CameraParameterValue::from_raw(parameter, vec![0xa5; length]);
            assert_eq!(record.scalar_code(), (length == 1).then_some(0xa5));
            let expected = CameraReadReply::Accepted(CameraParameterValues(vec![record]));
            let bytes = encode(&expected).unwrap();
            assert_eq!(
                decode::<CameraReadReply::<CameraParameterValues>>(&bytes, (&query,)).unwrap(),
                expected
            );
            assert!(
                decode::<CameraReadReply::<CameraParameterValues>>(
                    &bytes[..bytes.len() - 1],
                    (&query,)
                )
                .is_err()
            );
            let rejection = [0xe3, 0xa5];
            assert_eq!(
                encode(
                    &decode::<CameraReadReply::<CameraParameterValues>>(&rejection, (&query,))
                        .unwrap()
                )
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
                payload: encode(&CameraParametersGet::single(parameter).unwrap()).unwrap(),
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
            (CameraParameterSet::EisCode(7), &[1, 1, 8, 0, 1, 7]),
            (
                CameraParameterSet::LiveViewEisCode(7),
                &[1, 1, 0x3e, 0, 1, 7],
            ),
            (
                CameraParameterSet::StabilizationPreferenceCode(1),
                &[1, 1, 0x30, 0, 1, 1],
            ),
            (
                CameraParameterSet::StreamQualityCode(2),
                &[1, 1, 6, 0, 1, 2],
            ),
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
            let payload = encode(&value).unwrap();
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
    fn bounded_parameter_sets_check_every_code_before_appending_bytes() {
        for code in u8::MIN..=u8::MAX {
            for (value, id, maximum) in [
                (CameraParameterSet::StreamQualityCode(code), 0x06, 2),
                (CameraParameterSet::EisCode(code), 0x08, 7),
                (CameraParameterSet::LiveViewEisCode(code), 0x3e, 7),
                (
                    CameraParameterSet::StabilizationPreferenceCode(code),
                    0x30,
                    1,
                ),
                (
                    CameraParameterSet::FieldOfView(
                        decode::<CameraFieldOfView>(&[code], ()).unwrap(),
                    ),
                    0x09,
                    5,
                ),
                (CameraParameterSet::VoiceLanguageCode(code), 0x0e, 1),
            ] {
                let mut writer = std::io::Cursor::new(Vec::new());
                writer.get_mut().push(0xa5);
                let result = encode(&value).map(|bytes| writer.get_mut().extend(bytes));
                if code <= maximum {
                    result.unwrap();
                    assert_eq!(writer.get_ref().as_slice(), [0xa5, 1, 1, id, 0, 1, code]);
                } else {
                    assert!(
                        matches!(result, Err(Error::UnsupportedValue { value, .. }) if value == code)
                    );
                    assert_eq!(writer.get_ref().as_slice(), [0xa5]);
                }
            }
        }
    }

    #[test]
    fn boolean_eis_compatibility_encoding_matches_its_raw_code_subset() {
        for (enabled, code) in [(false, 0), (true, 1)] {
            assert_eq!(
                encode(&CameraParameterSet::EisEnabled(enabled)).unwrap(),
                encode(&CameraParameterSet::EisCode(code)).unwrap()
            );
        }
    }

    #[test]
    fn bounded_set_codes_do_not_restrict_or_normalize_get_codes() {
        for parameter in [
            CameraParameter::StreamQuality,
            CameraParameter::Eis,
            CameraParameter::LiveViewEis,
            CameraParameter::StabilizationPreference,
            CameraParameter::FieldOfView,
            CameraParameter::VoiceLanguage,
        ] {
            let query = CameraParametersGet::single_nano(parameter).unwrap();
            let [low, high] = u16::from(parameter).to_le_bytes();
            for code in u8::MIN..=u8::MAX {
                let bytes = [0, 0, 1, low, high, 1, code];
                let reply =
                    decode::<CameraReadReply<CameraParameterValues>>(&bytes, (&query,)).unwrap();
                assert_eq!(encode(&reply).unwrap(), bytes);
                let CameraReadReply::Accepted(values) = reply else {
                    panic!("expected accepted parameter reply");
                };
                assert_eq!(values.0[0].scalar_code(), Some(code));
                if matches!(
                    parameter,
                    CameraParameter::Eis | CameraParameter::LiveViewEis
                ) {
                    assert_eq!(
                        values.0[0].stabilization_mode(),
                        Some(decode::<CameraStabilizationMode>(&[code], ()).unwrap())
                    );
                } else {
                    assert_eq!(values.0[0].stabilization_mode(), None);
                }
            }
            for bytes in [&[0xe3][..], &[0xe3, 0xa5][..]] {
                let reply =
                    decode::<CameraReadReply<CameraParameterValues>>(bytes, (&query,)).unwrap();
                assert!(matches!(reply, CameraReadReply::Rejected { .. }));
                assert_eq!(encode(&reply).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn scalar_codes_are_not_coerced_to_booleans_or_truncated() {
        let value = CameraParameterValue::U8(CameraParameter::IsoLimit, 0xfe);
        assert_eq!(value.scalar_code(), Some(0xfe));
        assert_eq!(value.enabled(), None);
        for bytes in [vec![], vec![0xfe, 0xff]] {
            let wrong = CameraParameterValue::from_raw(value.parameter(), bytes);
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
            let value = CameraParameterValue::from_raw(parameter, bytes);
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
            for length in 0..=value.value_length() + 1 {
                if length == value.value_length() {
                    continue;
                }
                let wrong = CameraParameterValue::from_raw(parameter, vec![0; length]);
                assert_eq!(wrong.led_raw(), None);
                assert_eq!(wrong.capture_minimum_interval_raw(), None);
                assert_eq!(wrong.loop_duration_raw(), None);
                assert_eq!(wrong.rotation_state(), None);
            }
            let unknown = CameraParameterValue::Bytes(
                CameraParameter::from_id(u16::MAX),
                value.value_bytes(),
            );
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
                let value = CameraParameterValue::from_raw(parameter, vec![0xfe; length]);
                let expected = CameraReadReply::Accepted(CameraParameterValues(vec![value]));
                let encoded = encode(&expected).unwrap();
                assert_eq!(
                    decode::<CameraReadReply::<CameraParameterValues>>(&encoded, (&query,))
                        .unwrap(),
                    expected
                );
                for end in 0..encoded.len() {
                    assert!(
                        decode::<CameraReadReply::<CameraParameterValues>>(
                            &encoded[..end],
                            (&query,)
                        )
                        .is_err()
                    );
                }
            }
        }
        assert!(
            encode(&CameraParameterValue::Bytes(
                CameraParameter::VideoBeautify,
                vec![0; usize::from(u8::MAX) + 1]
            ))
            .is_err()
        );
        assert!(
            encode(&CameraParameterValues(vec![
                CameraParameterValue::U8(
                    CameraParameter::Beauty,
                    0
                );
                usize::from(u8::MAX) + 1
            ]))
            .is_err()
        );
    }

    #[test]
    fn parameter_replies_must_match_requested_ids_in_order() {
        let query = CameraParametersGet::advanced_live_reversal();
        let ordered = [0, 0, 2, 0, 0, 1, 0xa5, 1, 0, 1, 0x5a];
        let reply = decode::<CameraReadReply<CameraParameterValues>>(&ordered, (&query,)).unwrap();
        assert_eq!(encode(&reply).unwrap(), ordered);
        let reordered = [0, 0, 2, 1, 0, 1, 0x5a, 0, 0, 1, 0xa5];
        assert!(decode::<CameraReadReply::<CameraParameterValues>>(&reordered, (&query,)).is_err());
    }

    #[test]
    fn unknown_single_parameters_and_malformed_replies_are_rejected() {
        assert!(CameraParametersGet::single(CameraParameter::from_id(0x1234)).is_err());
        let query = CameraParametersGet::quick_mode_switch();
        for bytes in [
            &[0, 0, 0, 0x0b, 0, 1, 1][..],
            &[0, 0, 2, 0x0b, 0, 1, 1][..],
            &[0, 0, 1, 0x0b, 0, 2, 1][..],
            &[0, 0, 1, 0x0b, 0, 1, 1, 0xff][..],
        ] {
            assert!(decode::<CameraReadReply::<CameraParameterValues>>(bytes, (&query,)).is_err());
        }
    }

    #[test]
    fn parameter_reply_preserves_rejection_and_exposes_one_byte_flags() {
        let query = CameraParametersGet::quick_mode_switch();
        let reply =
            decode::<CameraReadReply<CameraParameterValues>>(&[0, 0, 1, 0x0b, 0, 1, 1], (&query,))
                .unwrap();
        let CameraReadReply::Accepted(values) = reply else {
            panic!("expected accepted parameter reply");
        };
        assert_eq!(values.0[0].enabled(), Some(true));
        assert_eq!(encode(&values).unwrap(), [0, 1, 0x0b, 0, 1, 1]);

        let rejected =
            decode::<CameraReadReply<CameraParameterValues>>(b"\xe3opaque", (&query,)).unwrap();
        assert_eq!(encode(&rejected).unwrap(), b"\xe3opaque");
    }

    #[test]
    fn fov_and_stabilization_labels_require_their_exact_one_byte_tlv() {
        let value =
            |parameter, bytes: &[u8]| CameraParameterValue::from_raw(parameter, bytes.to_vec());
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
        for parameter in [CameraParameter::Eis, CameraParameter::LiveViewEis] {
            for bytes in [&[][..], &[0, 1][..]] {
                assert_eq!(value(parameter, bytes).stabilization_mode(), None);
            }
            assert_eq!(
                value(parameter, &[7]).stabilization_mode(),
                Some(CameraStabilizationMode::Unknown(7))
            );
        }
        assert_eq!(value(CameraParameter::Eis, &[0]).field_of_view(), None);
        assert_eq!(
            value(CameraParameter::FieldOfView, &[0, 1]).field_of_view(),
            None
        );
    }
}
