//! Camera parameter GET/TLV response and restricted single-parameter SET codecs.

use crate::{
    Error, Result,
    protocol::{Query, commands::reply::CommandReadReply, identity::Endpoint},
};
use binrw::{
    BinRead, BinWrite, binrw, binwrite,
    meta::{ReadMagic, WriteMagic},
};
use std::{array::from_fn, marker::PhantomData};

/// Typed single-parameter GET. The type fixes the echoed ID and reply layout.
///
/// ```
/// use unofficial_osmo_camera::{
///     ControlSession, Result,
///     protocol::commands::camera::cmd_02_8e_parameters::{EisValue, Get},
/// };
/// fn read(session: &mut ControlSession) -> Result<()> {
///     let reply = session.execute_query::<Get<EisValue>>()?;
///     Ok(())
/// }
/// ```
///
/// Operation support is not enforced by the type bounds. Model-specific coverage
/// is provided separately by [`crate::protocol::capabilities`].
#[binwrite]
#[derive(Clone, Copy, Debug)]
#[bw(little, magic = b"\x02\x8e")]
pub struct Get<T: Parameter>(
    #[bw(calc = [0u8, 1])] [u8; 2],
    #[bw(calc = T::ID)] u16,
    #[bw(ignore)] PhantomData<T>,
);

/// Single-parameter SET envelope; the payload is the same struct used for GET.
/// Wrapping does not validate fields. Use payload constructors for checked values.
/// Encoding alone grants no hardware permission.
///
/// ```
/// use unofficial_osmo_camera::protocol::{
///     encode,
///     commands::camera::cmd_02_8e_parameters::{AddCustomModeValue, Set},
/// };
/// let setting = Set::new(AddCustomModeValue::new(4, 3)?);
/// assert_eq!(encode(&setting)?, [2, 0x8e, 1, 1, 0x2f, 0, 2, 4, 3]);
/// # Ok::<(), unofficial_osmo_camera::Error>(())
/// ```
///
/// Operation support is not enforced by the type bounds. Callers are responsible
/// for selecting supported SET operations.
#[binwrite]
#[derive(Clone, Debug, PartialEq, Eq)]
#[bw(little, magic = b"\x02\x8e")]
pub struct Set<T: Parameter>(#[bw(calc = [1u8, 1])] [u8; 2], T);

/// Fixed legacy request for IDs `0000` and `0001`.
///
/// This compatibility name confirms neither reversal behavior nor support for
/// both IDs. Parameter `0001` has no established typed layout.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\x8e")]
pub struct AdvancedLiveReversalGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

#[cfg(test)]
mod tests;

/// Advanced-mode code: GET 0/1, SET 0..=2; not live reversal.
///
/// SET also updates voice control after the primary service call.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0000u16)]
pub struct AdvanceModeValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for AdvanceModeValue {}

impl AdvanceModeValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 2 {
            return Err(Error::UnsupportedValue {
                field: "camera advanced-mode code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0002u16)]
pub struct BeautyValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for BeautyValue {}

impl BeautyValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0003u16)]
pub struct FpvControlValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for FpvControlValue {}

impl FpvControlValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// Raw stream-quality code: SET 0=Default, 1=Normal, 2=High.
///
/// No bitrate or resolution mapping is assigned; GET retains other codes.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0006u16)]
pub struct StreamQualityValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for StreamQualityValue {}

impl StreamQualityValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 2 {
            return Err(Error::UnsupportedValue {
                field: "camera stream-quality code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0007u16)]
pub struct PreviousModeValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for PreviousModeValue {}

/// Recording stabilization code; Nano SET accepts 0..=7, not just 0/1.
///
/// A zero GET code alone does not establish that stabilization is disabled.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0008u16)]
pub struct EisValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for EisValue {}

impl EisValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 7 {
            return Err(Error::UnsupportedValue {
                field: "camera stabilization code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Parameter FOV code; SET accepts 0..=5, with legacy candidate labels.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0009u16)]
pub struct FieldOfViewValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for FieldOfViewValue {}

impl FieldOfViewValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 5 {
            return Err(Error::UnsupportedValue {
                field: "camera field-of-view code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// One-byte voice-control code; the SET constructor accepts 0/1.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x000au16)]
pub struct VoiceControlValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for VoiceControlValue {}

impl VoiceControlValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera voice-control code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x000bu16)]
pub struct QuickModeSwitchValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for QuickModeSwitchValue {}

/// SET accepts 0/1; language labels remain unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x000eu16)]
pub struct VoiceLanguageValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for VoiceLanguageValue {}

impl VoiceLanguageValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera voice-language code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Raw ISO-limit code, distinct from user ISO; GET is a cached byte.
///
/// SET replaces an indexed byte in native state; physical ISO labels,
/// index meanings and active mode restrictions are unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x000fu16)]
pub struct IsoLimitValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for IsoLimitValue {}

impl IsoLimitValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0011u16)]
pub struct ScreenSleepValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for ScreenSleepValue {}

impl ScreenSleepValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0012u16)]
pub struct AutoOffValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for AutoOffValue {}

impl AutoOffValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0015u16)]
pub struct SpotMeteringValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for SpotMeteringValue {}

impl SpotMeteringValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera spot-metering code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Read-only code: provider 1 becomes 2; other provider values become 0.
///
/// This is not a lens capability enumeration.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x001eu16)]
pub struct LensControlValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for LensControlValue {}

/// Unmapped one-byte control; SET codes are 1, 2, 4, 8 or 16.
///
/// Those wire codes map to native ordinals 0..=4, not a bitmask SET.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x001fu16)]
pub struct Unknown001fValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for Unknown001fValue {}

impl Unknown001fValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if !matches!(code, 1 | 2 | 4 | 8 | 16) {
            return Err(Error::UnsupportedValue {
                field: "camera parameter 001f code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Raw one-byte audio-channel code; channel labels remain unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0020u16)]
pub struct RecordingAudioChannelValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for RecordingAudioChannelValue {}

impl RecordingAudioChannelValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// One-byte tracking code; the SET constructor retains the 0/1 compatibility subset.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0024u16)]
pub struct SelfieFollowingValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for SelfieFollowingValue {}

impl SelfieFollowingValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera selfie-following code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Unmapped one-byte control: SET 0/1; GET tests whether native value is 1.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0028u16)]
pub struct Unknown0028Value {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for Unknown0028Value {}

impl Unknown0028Value {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera parameter 0028 code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Independent preference: Nano SET accepts 0/1; GET retains the raw byte.
///
/// The two preference meanings are unspecified; this is not an EIS mode.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0030u16)]
pub struct StabilizationPreferenceValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for StabilizationPreferenceValue {}

impl StabilizationPreferenceValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera stabilization-preference code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Image-adjustment code; SET 0..=3, without assigned image-effect labels.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0033u16)]
pub struct ImageAdjustmentValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for ImageAdjustmentValue {}

impl ImageAdjustmentValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 3 {
            return Err(Error::UnsupportedValue {
                field: "camera image-adjustment code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Shutter-limit code: SET 0..=5, not a physical exposure time.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0034u16)]
pub struct ShutterLimitValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for ShutterLimitValue {}

impl ShutterLimitValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 5 {
            return Err(Error::UnsupportedValue {
                field: "camera shutter-limit code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Video-enhancement code; SET 0/1, with mode availability unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0035u16)]
pub struct VideoQualityEnhancementValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for VideoQualityEnhancementValue {}

impl VideoQualityEnhancementValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 1 {
            return Err(Error::UnsupportedValue {
                field: "camera video-enhancement code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Raw mirror code forwarded unchanged; affected output is unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0038u16)]
pub struct SelfieMirrorValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for SelfieMirrorValue {}

impl SelfieMirrorValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { code })
    }
}

/// Recording-plan code; SET 0..=2, with no assigned plan labels.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x003du16)]
pub struct RecordingPlanValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for RecordingPlanValue {}

impl RecordingPlanValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 2 {
            return Err(Error::UnsupportedValue {
                field: "camera recording-plan code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Raw live-view stabilization code, distinct from parameter `0008`.
///
/// Nano SET accepts 0..=7. Availability and effects on preview are unverified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x003eu16)]
pub struct LiveViewEisValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub code: u8,
}

impl Parameter for LiveViewEisValue {}

impl LiveViewEisValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(code: u8) -> Result<Self> {
        if code > 7 {
            return Err(Error::UnsupportedValue {
                field: "camera live-view stabilization code",
                value: code,
            });
        }
        Ok(Self { code })
    }
}

/// Legacy LED value; individual bit meanings are unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0010u16)]
pub struct LedValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub value: u16,
}

impl Parameter for LedValue {}

impl LedValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(value: u16) -> Result<Self> {
        Ok(Self { value })
    }
}

/// Legacy interval value; time units remain unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0017u16)]
pub struct CaptureMinimumIntervalValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub value: u16,
}

impl Parameter for CaptureMinimumIntervalValue {}

/// Four-byte raw GET record; inspected stores assign only its low LE16.
///
/// Low-word codes are 0, 300, 1200, 3600 or 65535; units and the
/// remaining two GET bytes are unspecified. The SET constructor accepts only
/// those five codes, zero-extended into the four-byte wire value.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0018u16)]
pub struct LoopDurationValue {
    #[br(temp, assert(length == 4, "camera parameter value width mismatch"))]
    #[bw(calc = 4u8)]
    length: u8,
    pub value: u32,
}

impl Parameter for LoopDurationValue {}

impl LoopDurationValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(value: u32) -> Result<Self> {
        if !matches!(value, 0 | 300 | 1200 | 3600 | 65535) {
            return Err(Error::Unsupported("camera loop-duration code"));
        }
        Ok(Self { value })
    }
}

/// LE32 aspect type; SET rejects values above 255 to avoid truncation.
///
/// The native provider uses one byte; physical ratio labels are unspecified.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0046u16)]
pub struct CaptureAspectTypeValue {
    #[br(temp, assert(length == 4, "camera parameter value width mismatch"))]
    #[bw(calc = 4u8)]
    length: u8,
    pub value: u32,
}

impl Parameter for CaptureAspectTypeValue {}

impl CaptureAspectTypeValue {
    /// Construct a SET payload with a supported value; binrw decodes GET independently.
    pub fn new(value: u32) -> Result<Self> {
        if value > u32::from(u8::MAX) {
            return Err(Error::Unsupported("camera aspect code would be truncated"));
        }
        Ok(Self { value })
    }
}

/// Read-only derived orientation record, not rotation-lock configuration.
#[binrw]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0014u16)]
pub struct RotationStateValue {
    #[br(temp, assert(length == 3, "camera parameter value width mismatch"))]
    #[bw(calc = 3u8)]
    length: u8,
    pub reserved_before: u8,
    pub orientation_code: u8,
    pub reserved_after: u8,
}

impl Parameter for RotationStateValue {}

/// Nine-byte short-video GET: flags, LE32 value, LE32 value.
///
/// The SET constructor supplies only flag 1 with a raw original-duration value
/// and a zero inactive word. Units and the other write branch are unknown.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0029u16)]
pub struct ShortVideoParametersValue {
    #[br(temp, assert(length == 9, "camera parameter value width mismatch"))]
    #[bw(calc = 9u8)]
    length: u8,
    pub flag: u8,
    pub value: u32,
    pub added_duration: u32,
}

impl Parameter for ShortVideoParametersValue {}

impl ShortVideoParametersValue {
    /// Construct the supported flag-1 SET without added duration.
    pub fn new(value: u32) -> Result<Self> {
        Ok(Self {
            flag: 1,
            value,
            added_duration: 0,
        })
    }
}

/// Read-only 13-byte storage outcome; individual field meanings are unknown.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0032u16)]
pub struct StorageOutcomeValue {
    #[br(temp, assert(length == 13, "camera parameter value width mismatch"))]
    #[bw(calc = 13u8)]
    length: u8,
    pub bytes: [u8; 13],
}

impl Parameter for StorageOutcomeValue {}

/// One indexed beauty entry; effect labels and scales remain unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct BeautyEffectEntry {
    pub index: u16,
    pub length: u8,
    pub value: u8,
}

/// LE16 count 15, followed by 15 entries: LE16 index, length 1, raw byte.
///
/// The SET constructor builds this canonical full record, not partial patches.
/// Per-index effect names and scales remain unspecified.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0039u16)]
pub struct BeautyEffectValue {
    #[br(temp, assert(length == 62, "camera parameter value width mismatch"))]
    #[bw(calc = 62u8)]
    length: u8,
    pub count: u16,
    pub entries: [BeautyEffectEntry; 15],
}

impl Parameter for BeautyEffectValue {}

impl BeautyEffectValue {
    /// Construct all fifteen indexed SET entries from their raw values.
    pub fn new(values: [u8; 15]) -> Result<Self> {
        Ok(Self {
            count: 15,
            entries: from_fn(|index| BeautyEffectEntry {
                index: index as u16,
                length: 1,
                value: values[index],
            }),
        })
    }
}

/// Sensitivity record: tag 1, raw code; SET forwards the second byte.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x003au16)]
pub struct AutofocusSensitivityValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub tag: u8,
    pub code: u8,
}

impl Parameter for AutofocusSensitivityValue {}

impl AutofocusSensitivityValue {
    /// Construct a tag-1 SET record.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { tag: 1, code })
    }
}

/// Strategy record: tag 1, raw code; SET forwards the second byte.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x003bu16)]
pub struct AutofocusStrategyValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub tag: u8,
    pub code: u8,
}

impl Parameter for AutofocusStrategyValue {}

impl AutofocusStrategyValue {
    /// Construct a tag-1 SET record.
    pub fn new(code: u8) -> Result<Self> {
        Ok(Self { tag: 1, code })
    }
}

/// Unmapped LE16 code: 0, 5, 10, 15, 30, 60, 120 or 300.
///
/// These map to native ordinals 0..=7; no physical unit is assigned.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x003cu16)]
pub struct Unknown003cValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub code: u16,
}

impl Parameter for Unknown003cValue {}

impl Unknown003cValue {
    /// Construct a SET with one of the established sparse codes.
    pub fn new(code: u16) -> Result<Self> {
        if !matches!(code, 0 | 5 | 10 | 15 | 30 | 60 | 120 | 300) {
            return Err(Error::Unsupported("camera parameter 003c code"));
        }
        Ok(Self { code })
    }
}

/// Write-only camera streaming-parameter trigger.
///
/// The SET constructor builds 11 bytes, all zero except byte 3 equal to 1:
/// a zero-destination profile subset. Other fields are not forwarded by
/// this camera path. This is not RTMP configuration, start or stop, and
/// does not establish an applied setting or active-mode support.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0019u16)]
pub struct Unknown0019Value {
    #[br(temp, assert(length == 11, "camera parameter value width mismatch"))]
    #[bw(calc = 11u8)]
    length: u8,
    pub reserved_before: [u8; 3],
    pub flag: u8,
    pub reserved_after: [u8; 7],
}

impl Parameter for Unknown0019Value {}

impl Unknown0019Value {
    /// Construct the supported trigger, not RTMP configuration or control.
    pub fn new() -> Result<Self> {
        Ok(Self {
            reserved_before: [0; 3],
            flag: 1,
            reserved_after: [0; 7],
        })
    }
}

/// Legacy identifier; not in the confirmed Nano catalogue.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0021u16)]
pub struct P2PShareValue {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(bytes.len()).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    #[br(count = length)]
    pub bytes: Vec<u8>,
}

impl Parameter for P2PShareValue {}

/// Legacy identifier; the Nano case is a no-op.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0025u16)]
pub struct VideoBeautifyValue {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(bytes.len()).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    #[br(count = length)]
    pub bytes: Vec<u8>,
}

impl Parameter for VideoBeautifyValue {}

/// Width-specific TC data; no physical interpretation is assigned.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[br(import(length: u8))]
#[brw(little)]
pub enum TcData {
    #[br(pre_assert(length == 3))]
    Byte(u8),
    #[br(pre_assert(length == 6))]
    Word(u32),
}

impl TcData {
    fn record_length(&self) -> u8 {
        match self {
            Self::Byte(_) => 3,
            Self::Word(_) => 6,
        }
    }
}

/// Write-only TC control: tag 1, selector, then raw data.
///
/// Selectors 1..=3 carry one raw byte (three bytes total); selector 4
/// carries four raw bytes (six total). Physical meanings are unspecified.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0026u16)]
pub struct Unknown0026Value {
    #[br(temp)]
    #[bw(calc = data.record_length())]
    length: u8,
    pub tag: u8,
    pub selector: u8,
    #[br(args(length))]
    pub data: TcData,
}

impl Parameter for Unknown0026Value {}

impl Unknown0026Value {
    /// Construct a selector-1..=3 byte SET or selector-4 word SET.
    pub fn new(selector: u8, data: TcData) -> Result<Self> {
        if !matches!(
            (selector, &data),
            (1..=3, TcData::Byte(_)) | (4, TcData::Word(_))
        ) {
            return Err(Error::Unsupported("camera TC SET record"));
        }
        Ok(Self {
            tag: 1,
            selector,
            data,
        })
    }
}

/// Length-prefixed name bytes; GET retains unvalidated contents.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalibrationName {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(bytes.len()).map_err(|_| Error::LimitExceeded("calibration name exceeds 255 bytes")))]
    length: u8,
    #[br(count = length)]
    pub bytes: Vec<u8>,
}

impl CalibrationName {
    /// Construct a nonempty printable ASCII SET name, at most 127 characters.
    pub fn new(name: &str) -> Result<Self> {
        let mut bytes = name.as_bytes().to_vec();
        bytes.push(0);
        let name = Self { bytes };
        name.validate()?;
        Ok(name)
    }

    fn validate(&self) -> Result<()> {
        let Some((&0, text)) = self.bytes.split_last() else {
            return Err(Error::Unsupported("calibration name must end in NUL"));
        };
        if text.is_empty()
            || text.len() > 127
            || !text.iter().all(|byte| matches!(byte, b' '..=b'~'))
        {
            return Err(Error::Unsupported("calibration name must be bounded ASCII"));
        }
        Ok(())
    }

    fn wire_length(&self) -> usize {
        1 + self.bytes.len()
    }
}

/// Raw calibration record; the SET constructor validates byte structure only.
///
/// The SET constructor supplies only operation 3: zero metadata, a length-prefixed
/// NUL-terminated nonempty ASCII label, LE16 extra length 4, a raw LE16
/// focal-length code and two zero bytes. Labels have at most 127 chars.
/// The label is parser-required but is not forwarded by this operation.
/// Printable ASCII, zero metadata and zero padding are host restrictions.
/// This does not establish units, a calibration procedure, provider
/// support or safe execution. Operations 0..=2 remain unavailable because
/// the inspected providers terminate on their native selectors.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[br(assert(usize::from(length) == 8 + name.wire_length()))]
#[brw(little, magic = 0x002bu16)]
pub struct LensUserCalibrationValue {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(8 + name.wire_length()).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    pub operation: u8,
    pub metadata: u8,
    pub name: CalibrationName,
    pub extra_length: u16,
    pub focal_code: u16,
    pub reserved: u16,
}

impl Parameter for LensUserCalibrationValue {}

impl LensUserCalibrationValue {
    /// Construct only the supported action-3 focal-length SET.
    pub fn new(name: CalibrationName, focal_code: u16) -> Result<Self> {
        name.validate()?;
        Ok(Self {
            operation: 3,
            metadata: 0,
            name,
            extra_length: 4,
            focal_code,
            reserved: 0,
        })
    }
}

/// Read-only packed state, byte length, then an opaque NUL-terminated string.
///
/// The raw record is retained even when its internal lengths are invalid.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x002cu16)]
pub struct LensUserCalibrationStateValue {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(bytes.len()).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    #[br(count = length)]
    pub bytes: Vec<u8>,
}

impl Parameter for LensUserCalibrationStateValue {}

/// Raw calibration-file action, LE16 data length, then named records.
///
/// Operations 0/2 carry one length-prefixed NUL-terminated nonempty
/// ASCII name; operation 1 carries two. Each name has at most 127 chars,
/// and the complete value is at most 255 bytes. Other operations,
/// trailing data and inconsistent lengths are rejected.
/// Nonempty printable ASCII names are a conservative host restriction.
/// This checks the byte contract, not file roles, permitted paths,
/// calibration prerequisites, provider support or a safe procedure.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[br(assert(usize::from(length) == 3 + usize::from(data_length)))]
#[br(assert(usize::from(data_length) == Self::data_wire_length(&name, second_name.as_ref())))]
#[brw(little, magic = 0x002du16)]
pub struct LensUserCalibrationFileValue {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(3 + Self::data_wire_length(name, second_name.as_ref())).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    pub operation: u8,
    #[br(temp)]
    #[bw(try_calc = u16::try_from(Self::data_wire_length(name, second_name.as_ref())).map_err(|_| Error::LimitExceeded("calibration-file data exceeds 65535 bytes")))]
    data_length: u16,
    pub name: CalibrationName,
    #[br(if(operation == 1))]
    pub second_name: Option<CalibrationName>,
}

impl Parameter for LensUserCalibrationFileValue {}

impl LensUserCalibrationFileValue {
    /// Construct action 0/2 with one name, or action 1 with two names.
    pub fn new(
        operation: u8,
        name: CalibrationName,
        second_name: Option<CalibrationName>,
    ) -> Result<Self> {
        if operation > 2 || (operation == 1) != second_name.is_some() {
            return Err(Error::Unsupported("camera calibration-file SET variant"));
        }
        name.validate()?;
        if let Some(second) = &second_name {
            second.validate()?;
        }
        if 3 + Self::data_wire_length(&name, second_name.as_ref()) > usize::from(u8::MAX) {
            return Err(Error::Unsupported(
                "calibration-file record exceeds TLV length",
            ));
        }
        Ok(Self {
            operation,
            name,
            second_name,
        })
    }

    fn data_wire_length(name: &CalibrationName, second_name: Option<&CalibrationName>) -> usize {
        name.wire_length() + second_name.map_or(0, CalibrationName::wire_length)
    }
}

/// Write-only custom-mode record: raw index, operation 0..=3.
///
/// The SET constructor builds exactly two bytes. Index availability and
/// persistence effects remain mode-dependent and are not inferred.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x002fu16)]
pub struct AddCustomModeValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub index: u8,
    pub operation: u8,
}

impl Parameter for AddCustomModeValue {}

impl AddCustomModeValue {
    /// Construct a SET for an operation in 0..=3.
    ///
    pub fn new(index: u8, operation: u8) -> Result<Self> {
        if operation > 3 {
            return Err(Error::Unsupported("camera custom-mode SET record"));
        }
        Ok(Self { index, operation })
    }
}

/// Write-only synchronization: source 0..=3, destination 0..=3, operation.
///
/// Operation 1 starts. The canonical stop record is `[0, 0, 2]`.
/// Endpoint labels, accepted combinations and completion remain unspecified.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0031u16)]
pub struct StorageSynchronizationValue {
    #[br(temp, assert(length == 3, "camera parameter value width mismatch"))]
    #[bw(calc = 3u8)]
    length: u8,
    pub source: u8,
    pub destination: u8,
    pub operation: u8,
}

impl Parameter for StorageSynchronizationValue {}

impl StorageSynchronizationValue {
    /// Construct a bounded start (1) or canonical stop (2) SET.
    pub fn new(source: u8, destination: u8, operation: u8) -> Result<Self> {
        if !matches!(
            (source, destination, operation),
            (0..=3, 0..=3, 1) | (0, 0, 2)
        ) {
            return Err(Error::Unsupported("camera storage synchronization record"));
        }
        Ok(Self {
            source,
            destination,
            operation,
        })
    }
}

/// Write-only motionlapse preview: raw code, action (0 preview, 1 interrupt).
/// Current-mode support and the raw code's physical meaning are unspecified.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0036u16)]
pub struct TimelapseTrackPreviewValue {
    #[br(temp, assert(length == 2, "camera parameter value width mismatch"))]
    #[bw(calc = 2u8)]
    length: u8,
    pub code: u8,
    pub flag: u8,
}

impl Parameter for TimelapseTrackPreviewValue {}

impl TimelapseTrackPreviewValue {
    /// Construct a SET with a 0/1 flag; code semantics remain unspecified.
    pub fn new(code: u8, flag: u8) -> Result<Self> {
        if flag > 1 {
            return Err(Error::Unsupported("camera motionlapse preview record"));
        }
        Ok(Self { code, flag })
    }
}

/// Write-only one-byte motionlapse mode code; available modes are unspecified.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = 0x0037u16)]
pub struct TimelapseTrackSetValue {
    #[br(temp, assert(length == 1, "camera parameter value width mismatch"))]
    #[bw(calc = 1u8)]
    length: u8,
    pub mode: u8,
}

impl Parameter for TimelapseTrackSetValue {}

impl TimelapseTrackSetValue {
    /// Construct a one-byte mode SET without assigning physical mode labels.
    pub fn new(mode: u8) -> Result<Self> {
        Ok(Self { mode })
    }
}

/// Opaque value for the fixed legacy pair or explicitly decoded raw records.
///
/// This type has no typed GET/SET eligibility and is not a typed reply fallback.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawParameterValue {
    #[br(temp)]
    #[bw(try_calc = u8::try_from(bytes.len()).map_err(|_| Error::LimitExceeded("camera TLV exceeds 255 bytes")))]
    length: u8,
    #[br(count = length)]
    pub bytes: Vec<u8>,
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

/// Derived orientation reported by camera parameter `0x0014`.
///
/// The targeted Nano layout reports `[0, orientation_code, 0]`. This is not
/// the internal lock/configured-orientation/shooting-orientation record:
/// those inputs can be transformed before the reply is built. Codes 0..=3
/// occur on the selected path; physical directions and degree mappings are
/// unspecified. Other byte values remain representable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct CameraRotationState {
    /// Zero in the targeted layout; not a rotation-lock mode.
    pub reserved_before: u8,
    pub orientation_code: u8,
    /// Zero in the targeted layout; not a raw sensor angle.
    pub reserved_after: u8,
}

/// Wire identity of a supported parameter payload.
///
/// Callers may implement this trait for custom payloads. Both GET and SET use
/// these codec bounds; they do not enforce operation support. Implementations
/// do not grant hardware authorization or extend the raw-read allowlist.
///
/// ```
/// use binrw::{BinRead, BinWrite};
/// use unofficial_osmo_camera::protocol::{
///     encode,
///     commands::camera::cmd_02_8e_parameters::{
///         Get, Parameter,
///     },
/// };
///
/// #[derive(BinRead, BinWrite)]
/// #[brw(little, magic = 0xfffeu16)]
/// struct CustomParameter {
///     #[brw(magic = 1u8)]
///     code: u8,
/// }
///
/// impl Parameter for CustomParameter {}
///
/// assert_eq!(CustomParameter::ID, 0xfffe);
/// assert_eq!(encode(&Get::<CustomParameter>::default())?, [2, 0x8e, 0, 1, 0xfe, 0xff]);
/// # Ok::<(), unofficial_osmo_camera::Error>(())
/// ```
pub trait Parameter:
    for<'a> BinRead<Args<'a> = ()>
    + for<'a> BinWrite<Args<'a> = ()>
    + ReadMagic<MagicType = u16>
    + WriteMagic<MagicType = u16>
{
    /// Parameter wire ID, sourced from the payload's binrw write magic.
    const ID: u16 = <Self as WriteMagic>::MAGIC;
}

impl<T: Parameter> Default for Get<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

/// Accepted GET body, including the single-record envelope.
///
/// Unsupported lengths/layouts are errors, not a raw fallback or a value
/// coerced into the requested type. Unknown codes in supported layouts remain
/// raw. Rejection status/extensions are handled by [`CommandReadReply`].
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = b"\x02\x8e")]
pub struct ParameterReply<T: Parameter> {
    #[br(temp, assert(header == [0, 0, 1], "accepted parameter reply header mismatch"))]
    #[bw(calc = [0u8, 0, 1])]
    header: [u8; 3],
    pub value: T,
}

impl<T: Parameter> Query for Get<T> {
    type Response = CommandReadReply<ParameterReply<T>>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

impl<T: Parameter> Set<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn value(&self) -> &T {
        &self.0
    }

    pub fn into_value(self) -> T {
        self.0
    }
}

/// Fixed legacy pair: one typed advanced-mode value and one opaque value.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = b"\x02\x8e")]
pub struct AdvancedLiveReversalReply {
    #[br(temp, assert(header == [0, 0, 2], "accepted legacy reply header mismatch"))]
    #[bw(calc = [0u8, 0, 2])]
    header: [u8; 3],
    pub advanced_mode: AdvanceModeValue,
    #[br(temp, assert(legacy_id == 1))]
    #[bw(calc = 1u16)]
    legacy_id: u16,
    pub legacy_value: RawParameterValue,
}

impl Query for AdvancedLiveReversalGet {
    type Response = CommandReadReply<AdvancedLiveReversalReply>;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x00, 0x02, 0x00, 0x00, 0x01, 0x00];

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}
