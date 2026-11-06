//! Recording-format codecs for camera command 0x02/0x18.

use crate::protocol::encode;
use crate::protocol::{
    ExplorationRequest, Flags,
    identity::{CameraCommand, Command, Endpoint},
};
use crate::{Error, Result};
use binrw::{BinRead, BinWrite};
use std::time::Duration;

const SPEED_RATIO_MASK: u16 = 0x003f;

/// Lossless wire resolution identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BinRead, BinWrite)]
#[repr(transparent)]
pub struct VideoResolutionCode(pub u8);

impl From<u8> for VideoResolutionCode {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

impl From<VideoResolutionCode> for u8 {
    fn from(value: VideoResolutionCode) -> Self {
        value.0
    }
}

impl VideoResolutionCode {
    pub const MAX: Self = Self(253);
    /// Wire code 254.
    pub const UNSET: Self = Self(254);

    /// Constant-time lookup for identifiers with known dimensions.
    pub fn info(self) -> Option<&'static VideoResolutionInfo> {
        VIDEO_RESOLUTION_LOOKUP[usize::from(self.0)]
    }
}

/// Optional dimensions associated with a wire resolution identifier.
///
/// `None` means the dimensions are unspecified. This table does not indicate
/// device support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoResolutionInfo {
    pub code: VideoResolutionCode,
    pub dimensions: Option<(u16, u16)>,
}

const fn resolution(code: u8, dimensions: Option<(u16, u16)>) -> VideoResolutionInfo {
    VideoResolutionInfo {
        code: VideoResolutionCode(code),
        dimensions,
    }
}

/// Dimension entries keyed by wire identifier; ordering is not part of the protocol.
pub static VIDEO_RESOLUTIONS: [VideoResolutionInfo; 35] = [
    resolution(0, Some((640, 480))),
    resolution(4, Some((1280, 720))),
    resolution(8, Some((1920, 960))),
    resolution(10, Some((1920, 1080))),
    // The selector conditions for this ID are unspecified.
    resolution(16, Some((3840, 2160))),
    // The selector conditions for this ID are unspecified.
    resolution(22, Some((4096, 2160))),
    resolution(24, Some((2704, 1520))),
    resolution(26, Some((640, 512))),
    resolution(27, Some((4608, 2160))),
    resolution(28, Some((4608, 2592))),
    resolution(31, Some((2720, 1530))),
    resolution(32, Some((5280, 2160))),
    resolution(33, Some((5280, 2972))),
    resolution(34, Some((3840, 1572))),
    resolution(35, Some((5760, 3240))),
    resolution(36, Some((6016, 3200))),
    resolution(37, Some((2048, 1080))),
    resolution(39, Some((5120, 2880))),
    resolution(45, Some((2688, 1512))),
    resolution(48, Some((4000, 3000))),
    resolution(50, Some((2880, 1620))),
    resolution(52, Some((2720, 2040))),
    resolution(55, Some((7680, 4320))),
    resolution(56, Some((5472, 3078))),
    resolution(65, Some((4096, 3072))),
    resolution(66, Some((1080, 1920))),
    resolution(67, Some((1512, 2688))),
    resolution(68, Some((5472, 3648))),
    resolution(69, Some((864, 480))),
    // The code label does not establish exact dimensions.
    resolution(71, None),
    resolution(95, Some((2688, 2016))),
    resolution(97, Some((5120, 2700))),
    // The code label does not establish exact dimensions.
    resolution(99, None),
    resolution(100, Some((1280, 1024))),
    resolution(101, Some((640, 340))),
];

/// Source-labelled Nano video-resolution identifiers.
///
/// Labels are model-specific external candidates; exact dimensions and
/// device/mode support are not encoded here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum NanoVideoResolutionCode {
    #[brw(magic = 0x10u8)]
    Uhd4k16By9,
    #[brw(magic = 0x2du8)]
    Qhd2700p16By9,
    #[brw(magic = 0x0au8)]
    FullHd1080p16By9,
    #[brw(magic = 0x67u8)]
    Uhd4k4By3,
    #[brw(magic = 0x5fu8)]
    Qhd2700p4By3,
    #[brw(magic = 0x0cu8)]
    FullHd1080p4By3,
    Unknown(u8),
}

const RESOLUTION_ID_COUNT: usize = u8::MAX as usize + 1;
static VIDEO_RESOLUTION_LOOKUP: [Option<&VideoResolutionInfo>; RESOLUTION_ID_COUNT] =
    build_resolution_lookup();

const fn build_resolution_lookup() -> [Option<&'static VideoResolutionInfo>; RESOLUTION_ID_COUNT] {
    let mut lookup = [None; RESOLUTION_ID_COUNT];
    let mut index = 0;
    while index < VIDEO_RESOLUTIONS.len() {
        let entry = &VIDEO_RESOLUTIONS[index];
        assert!(
            entry.code.0 != VideoResolutionCode::MAX.0
                && entry.code.0 != VideoResolutionCode::UNSET.0,
            "sentinel in resolution table"
        );
        let id = entry.code.0 as usize;
        assert!(lookup[id].is_none(), "duplicate resolution wire ID");
        lookup[id] = Some(entry);
        index += 1;
    }
    lookup
}

/// Named frame-rate code values. The variants do not establish device support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum VideoFrameRateCode {
    #[brw(magic = 0u8)]
    Unspecified,
    #[brw(magic = 1u8)]
    Fps24000Over1001,
    #[brw(magic = 2u8)]
    Fps25,
    #[brw(magic = 3u8)]
    Fps30000Over1001,
    #[brw(magic = 4u8)]
    Fps48000Over1001,
    #[brw(magic = 5u8)]
    Fps50,
    #[brw(magic = 6u8)]
    Fps60000Over1001,
    #[brw(magic = 7u8)]
    Fps120000Over1001,
    #[brw(magic = 8u8)]
    Fps240,
    #[brw(magic = 10u8)]
    Fps100,
    #[brw(magic = 11u8)]
    Fps96000Over1001,
    #[brw(magic = 13u8)]
    Fps24,
    #[brw(magic = 14u8)]
    Fps30,
    #[brw(magic = 15u8)]
    Fps48,
    #[brw(magic = 16u8)]
    Fps60,
    #[brw(magic = 24u8)]
    Fps120,
    #[brw(magic = 25u8)]
    Fps96,
    #[brw(magic = 26u8)]
    Fps72Code26,
    #[brw(magic = 27u8)]
    Fps72Code27,
    #[brw(magic = 28u8)]
    Fps75,
    #[brw(magic = 0x13u8)]
    Fps200,
    Unknown(u8),
}

/// Opaque one-byte field; its physical meanings and device support are unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[repr(transparent)]
pub struct VideoFovCode(pub u8);

impl From<u8> for VideoFovCode {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

/// Preserved format options, with only the speed-ratio bit boundary identified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct VideoFormatOptions(pub u16);

impl VideoFormatOptions {
    pub fn speed_ratio_code(self) -> u16 {
        self.0 & SPEED_RATIO_MASK
    }

    /// Replaces the six-bit code, not a physical ratio or validated camera mode.
    pub fn with_speed_ratio_code(self, code: u8) -> Result<Self> {
        if u16::from(code) > SPEED_RATIO_MASK {
            return Err(Error::UnsupportedValue {
                field: "video speed-ratio code",
                value: code,
            });
        }
        Ok(Self((self.0 & !SPEED_RATIO_MASK) | u16::from(code)))
    }
}

/// Five-byte recording-format value.
///
/// This decodes exactly one format span, not a complete notification. Device
/// support and accepted combinations are unspecified.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
pub struct VideoFormat {
    pub resolution: VideoResolutionCode,
    pub frame_rate: VideoFrameRateCode,
    pub fov: VideoFovCode,
    pub options: VideoFormatOptions,
}

impl VideoFormat {
    /// Return byte 4 as the Nano slow-motion factor candidate.
    ///
    /// Call only for a Nano slow-motion format. Other models place this field
    /// differently, and this value does not validate the mode or format.
    pub fn nano_slow_motion_factor(self) -> u8 {
        (self.options.0 >> 8) as u8
    }
}

/// Encodes a recording-format request while preserving selected current fields.
///
/// APP -> Camera | 0x02/0x18 | Video format SET
///
/// ```text
/// request {
///     resolution: u8  | wire resolution identifier
///     frame_rate: u8  | protocol code
///     fov: u8
///     options: le_u16 | low six bits are the speed-ratio code
/// }
/// reply { bytes: [u8; remaining] | unspecified }
/// ```
///
/// No default format or read request is supplied. This type only encodes the
/// five-byte request body; command delivery is handled by the transport layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
pub struct VideoFormatSet {
    format: VideoFormat,
}

impl VideoFormatSet {
    /// Changes resolution/rate, preserving current FOV and all option bits.
    pub fn from_current(
        current: VideoFormat,
        resolution: VideoResolutionCode,
        frame_rate: VideoFrameRateCode,
    ) -> Self {
        Self {
            format: VideoFormat {
                resolution,
                frame_rate,
                ..current
            },
        }
    }

    pub fn with_fov(mut self, fov: VideoFovCode) -> Self {
        self.format.fov = fov;
        self
    }

    pub fn with_speed_ratio_code(mut self, code: u8) -> Result<Self> {
        self.format.options = self.format.options.with_speed_ratio_code(code)?;
        Ok(self)
    }

    pub fn format(&self) -> VideoFormat {
        self.format
    }

    /// Builds a request with the caller-selected flags and timeout. Creating it
    /// does not grant transport approval or establish device support.
    pub fn exploration_request(
        &self,
        flags: Flags,
        timeout: Duration,
    ) -> Result<ExplorationRequest> {
        Ok(ExplorationRequest {
            receiver: Endpoint::Camera,
            command: Command::Camera(CameraCommand::VideoFormatSet),
            flags,
            payload: encode(&self)?,
            timeout,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    const CURRENT: [u8; 5] = [0x16, 0x03, 0xa5, 0xff, 0x93];
    const TARGET: VideoResolutionCode = VideoResolutionCode(0x65);

    fn current() -> VideoFormat {
        decode::<VideoFormat>(&CURRENT, ()).unwrap()
    }

    #[test]
    fn format_round_trips_unknown_fields_and_requires_exact_length() {
        assert_eq!(encode(&current()).unwrap(), CURRENT);
        for length in 0..CURRENT.len() {
            assert!(decode::<VideoFormat>(&CURRENT[..length], ()).is_err());
        }
        let mut extra = CURRENT.to_vec();
        extra.push(0);
        assert!(decode::<VideoFormat>(&extra, ()).is_err());
    }

    #[test]
    fn format_preserves_every_resolution_and_frame_rate_code() {
        for code in u8::MIN..=u8::MAX {
            let bytes = [code, code, code, 0x34, 0x12];
            let format = decode::<VideoFormat>(&bytes, ()).unwrap();
            assert_eq!(format.resolution.0, code);
            assert_eq!(encode(&format.frame_rate).unwrap(), [code]);
            assert_eq!(format.fov.0, code);
            assert_eq!(format.options.0, 0x1234);
            assert_eq!(encode(&format).unwrap(), bytes);
        }
    }

    #[test]
    fn source_byte_types_round_trip_all_codes_independently() {
        fn assert_byte_round_trip<T>(value: T, byte: u8)
        where
            T: std::fmt::Debug + PartialEq,
            for<'a> T: BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()>,
        {
            assert_eq!(decode::<T>(&[byte], ()).unwrap(), value);
            assert_eq!(encode(&value).unwrap(), [byte]);
            assert!(decode::<T>(&[], ()).is_err());
            assert!(decode::<T>(&[byte, 0], ()).is_err());
        }

        for code in u8::MIN..=u8::MAX {
            assert_byte_round_trip(VideoResolutionCode(code), code);
            assert_byte_round_trip(decode::<VideoFrameRateCode>(&[code], ()).unwrap(), code);
            assert_byte_round_trip(VideoFovCode(code), code);
        }
    }

    #[test]
    fn source_options_round_trip_all_bits_as_little_endian() {
        for word in u16::MIN..=u16::MAX {
            let bytes = word.to_le_bytes();
            let value = decode::<VideoFormatOptions>(&bytes, ()).unwrap();
            assert_eq!(value.0, word);
            assert_eq!(encode(&value).unwrap(), bytes);
        }
        assert!(decode::<VideoFormatOptions>(&[], ()).is_err());
        assert!(decode::<VideoFormatOptions>(&[0x34], ()).is_err());
        assert!(decode::<VideoFormatOptions>(&[0x34, 0x12, 0], ()).is_err());
    }

    #[test]
    fn nano_resolution_labels_and_slow_motion_factor_remain_model_qualified() {
        let nano_format = decode::<VideoFormat>(&[0x10, 0x08, 0, 0, 4], ()).unwrap();
        assert_eq!(nano_format.nano_slow_motion_factor(), 4);
        assert_eq!(encode(&nano_format).unwrap(), [0x10, 0x08, 0, 0, 4]);

        let other_layout = decode::<VideoFormat>(&[0x10, 0x08, 0, 8, 0], ()).unwrap();
        assert_eq!(other_layout.nano_slow_motion_factor(), 0);
        assert_eq!(encode(&other_layout).unwrap(), [0x10, 0x08, 0, 8, 0]);
    }

    #[test]
    fn set_preserves_current_fields_and_encodes_wire_codes() {
        let set = VideoFormatSet::from_current(current(), TARGET, VideoFrameRateCode::Fps30);
        assert_eq!(encode(&set).unwrap(), [0x65, 0x0e, 0xa5, 0xff, 0x93]);
        assert_eq!(set.format().fov, current().fov);
        assert_eq!(set.format().options, current().options);
        assert_eq!(
            encode(&set.with_fov(VideoFovCode(4))).unwrap(),
            [0x65, 0x0e, 0x04, 0xff, 0x93]
        );
    }

    #[test]
    fn speed_updates_preserve_every_other_bit_and_reject_truncation() {
        let set = VideoFormatSet::from_current(current(), TARGET, VideoFrameRateCode::Fps30);
        for code in u8::MIN..=u8::MAX {
            let result = set.with_speed_ratio_code(code);
            if u16::from(code) <= SPEED_RATIO_MASK {
                let format = result.unwrap().format();
                assert_eq!(format.options.speed_ratio_code(), u16::from(code));
                assert_eq!(
                    format.options.0 & !SPEED_RATIO_MASK,
                    current().options.0 & !SPEED_RATIO_MASK
                );
                assert_eq!(format.fov, current().fov);
            } else {
                assert!(matches!(result, Err(Error::UnsupportedValue { .. })));
            }
        }
        assert_eq!(
            encode(&set.with_speed_ratio_code(0x02).unwrap()).unwrap(),
            [0x65, 0x0e, 0xa5, 0xc2, 0x93]
        );
    }

    #[test]
    fn set_request_uses_the_video_format_command_identity() {
        let set = VideoFormatSet::from_current(current(), TARGET, VideoFrameRateCode::Fps30);
        let request = set
            .exploration_request(Flags::REQUEST, Duration::from_secs(1))
            .unwrap();
        assert_eq!(request.receiver, Endpoint::Camera);
        assert_eq!(
            request.command,
            Command::Camera(CameraCommand::VideoFormatSet)
        );
        assert_eq!(request.command.wire(), (0x02, 0x18));
        assert_eq!(request.payload, encode(&set).unwrap());
        assert!(!request.is_known_read());
        assert!(request.validate(false, false).is_err());
        assert!(request.validate(true, false).is_err());
        assert!(request.validate(false, true).is_err());
        request.validate(true, true).unwrap();
        let invalid = set
            .exploration_request(Flags::REQUEST, Duration::ZERO)
            .unwrap();
        assert!(invalid.validate(true, true).is_err());
    }
}
