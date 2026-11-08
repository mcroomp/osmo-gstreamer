//! Recording-format codecs for camera command 0x02/0x18.

use crate::{
    Error, Result,
    protocol::{CommandPacket, ExplorationRequest, Flags, identity::Endpoint},
};
use binrw::{BinRead, BinWrite};
use std::{io::Cursor, time::Duration};

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
/// No default format is supplied. Use `VideoFormatGet` in the sibling
/// `cmd_02_19_video_format_get` module to obtain the current value.
/// This type only encodes the
/// five-byte request payload after the two-byte command prefix; framing and
/// delivery are handled by the transport layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[bw(magic = b"\x02\x18")]
pub struct VideoFormatSet {
    format: VideoFormat,
}

const SPEED_RATIO_MASK: u16 = 0x003f;

/// All currently known recording-resolution identifiers, not a support list.
///
/// Size labels describe known dimensions, not availability on every model.
/// Codes without established dimensions retain numeric names; all other values
/// use the lossless `Unknown` fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, BinRead, BinWrite)]
pub enum VideoResolutionCode {
    #[brw(magic = 0u8)]
    Size640x480,
    #[brw(magic = 4u8)]
    Size1280x720,
    #[brw(magic = 8u8)]
    Size1920x960,
    #[brw(magic = 10u8)]
    Size1920x1080,
    #[brw(magic = 12u8)]
    Size1920x1440,
    #[brw(magic = 16u8)]
    Size3840x2160,
    #[brw(magic = 22u8)]
    Size4096x2160,
    #[brw(magic = 24u8)]
    Size2704x1520,
    #[brw(magic = 26u8)]
    Size640x512,
    #[brw(magic = 27u8)]
    Size4608x2160,
    #[brw(magic = 28u8)]
    Size4608x2592,
    #[brw(magic = 31u8)]
    Size2720x1530,
    #[brw(magic = 32u8)]
    Size5280x2160,
    #[brw(magic = 33u8)]
    Size5280x2972,
    #[brw(magic = 34u8)]
    Size3840x1572,
    #[brw(magic = 35u8)]
    Size5760x3240,
    #[brw(magic = 36u8)]
    Size6016x3200,
    #[brw(magic = 37u8)]
    Size2048x1080,
    #[brw(magic = 39u8)]
    Size5120x2880,
    #[brw(magic = 45u8)]
    Size2688x1512,
    #[brw(magic = 48u8)]
    Size4000x3000,
    #[brw(magic = 50u8)]
    Size2880x1620,
    #[brw(magic = 52u8)]
    Size2720x2040,
    #[brw(magic = 55u8)]
    Size7680x4320,
    #[brw(magic = 56u8)]
    Size5472x3078,
    #[brw(magic = 65u8)]
    Size4096x3072,
    #[brw(magic = 66u8)]
    Size1080x1920,
    #[brw(magic = 67u8)]
    Size1512x2688,
    #[brw(magic = 68u8)]
    Size5472x3648,
    #[brw(magic = 69u8)]
    Size864x480,
    #[brw(magic = 71u8)]
    Code71,
    #[brw(magic = 95u8)]
    Size2688x2016,
    #[brw(magic = 97u8)]
    Size5120x2700,
    #[brw(magic = 99u8)]
    Code99,
    #[brw(magic = 100u8)]
    Size1280x1024,
    #[brw(magic = 101u8)]
    Size640x340,
    #[brw(magic = 103u8)]
    Size3840x2880,
    /// Legacy sentinel label for wire code 253.
    #[brw(magic = 253u8)]
    Max,
    /// Legacy sentinel label for wire code 254.
    #[brw(magic = 254u8)]
    Unset,
    Unknown(u8),
}

impl From<u8> for VideoResolutionCode {
    fn from(value: u8) -> Self {
        Self::read_le(&mut Cursor::new([value])).expect("resolution fallback accepts every byte")
    }
}

impl From<VideoResolutionCode> for u8 {
    fn from(value: VideoResolutionCode) -> Self {
        value.wire()
    }
}

impl VideoResolutionCode {
    pub const MAX: Self = Self::Max;
    /// Wire code 254.
    pub const UNSET: Self = Self::Unset;

    /// Raw wire identifier, independent of any model's capabilities.
    pub const fn wire(self) -> u8 {
        match self {
            Self::Size640x480 => 0,
            Self::Size1280x720 => 4,
            Self::Size1920x960 => 8,
            Self::Size1920x1080 => 10,
            Self::Size1920x1440 => 12,
            Self::Size3840x2160 => 16,
            Self::Size4096x2160 => 22,
            Self::Size2704x1520 => 24,
            Self::Size640x512 => 26,
            Self::Size4608x2160 => 27,
            Self::Size4608x2592 => 28,
            Self::Size2720x1530 => 31,
            Self::Size5280x2160 => 32,
            Self::Size5280x2972 => 33,
            Self::Size3840x1572 => 34,
            Self::Size5760x3240 => 35,
            Self::Size6016x3200 => 36,
            Self::Size2048x1080 => 37,
            Self::Size5120x2880 => 39,
            Self::Size2688x1512 => 45,
            Self::Size4000x3000 => 48,
            Self::Size2880x1620 => 50,
            Self::Size2720x2040 => 52,
            Self::Size7680x4320 => 55,
            Self::Size5472x3078 => 56,
            Self::Size4096x3072 => 65,
            Self::Size1080x1920 => 66,
            Self::Size1512x2688 => 67,
            Self::Size5472x3648 => 68,
            Self::Size864x480 => 69,
            Self::Code71 => 71,
            Self::Size2688x2016 => 95,
            Self::Size5120x2700 => 97,
            Self::Code99 => 99,
            Self::Size1280x1024 => 100,
            Self::Size640x340 => 101,
            Self::Size3840x2880 => 103,
            Self::Max => 253,
            Self::Unset => 254,
            Self::Unknown(code) => code,
        }
    }

    /// Constant-time lookup for identifiers with known dimensions.
    pub fn info(self) -> Option<&'static VideoResolutionInfo> {
        VIDEO_RESOLUTION_LOOKUP[usize::from(self.wire())]
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

const fn resolution(
    code: VideoResolutionCode,
    dimensions: Option<(u16, u16)>,
) -> VideoResolutionInfo {
    VideoResolutionInfo { code, dimensions }
}

/// Dimension entries keyed by wire identifier; ordering is not part of the protocol.
pub static VIDEO_RESOLUTIONS: [VideoResolutionInfo; 37] = [
    resolution(VideoResolutionCode::Size640x480, Some((640, 480))),
    resolution(VideoResolutionCode::Size1280x720, Some((1280, 720))),
    resolution(VideoResolutionCode::Size1920x960, Some((1920, 960))),
    resolution(VideoResolutionCode::Size1920x1080, Some((1920, 1080))),
    resolution(VideoResolutionCode::Size1920x1440, Some((1920, 1440))),
    // The selector conditions for this ID are unspecified.
    resolution(VideoResolutionCode::Size3840x2160, Some((3840, 2160))),
    // The selector conditions for this ID are unspecified.
    resolution(VideoResolutionCode::Size4096x2160, Some((4096, 2160))),
    resolution(VideoResolutionCode::Size2704x1520, Some((2704, 1520))),
    resolution(VideoResolutionCode::Size640x512, Some((640, 512))),
    resolution(VideoResolutionCode::Size4608x2160, Some((4608, 2160))),
    resolution(VideoResolutionCode::Size4608x2592, Some((4608, 2592))),
    resolution(VideoResolutionCode::Size2720x1530, Some((2720, 1530))),
    resolution(VideoResolutionCode::Size5280x2160, Some((5280, 2160))),
    resolution(VideoResolutionCode::Size5280x2972, Some((5280, 2972))),
    resolution(VideoResolutionCode::Size3840x1572, Some((3840, 1572))),
    resolution(VideoResolutionCode::Size5760x3240, Some((5760, 3240))),
    resolution(VideoResolutionCode::Size6016x3200, Some((6016, 3200))),
    resolution(VideoResolutionCode::Size2048x1080, Some((2048, 1080))),
    resolution(VideoResolutionCode::Size5120x2880, Some((5120, 2880))),
    resolution(VideoResolutionCode::Size2688x1512, Some((2688, 1512))),
    resolution(VideoResolutionCode::Size4000x3000, Some((4000, 3000))),
    resolution(VideoResolutionCode::Size2880x1620, Some((2880, 1620))),
    resolution(VideoResolutionCode::Size2720x2040, Some((2720, 2040))),
    resolution(VideoResolutionCode::Size7680x4320, Some((7680, 4320))),
    resolution(VideoResolutionCode::Size5472x3078, Some((5472, 3078))),
    resolution(VideoResolutionCode::Size4096x3072, Some((4096, 3072))),
    resolution(VideoResolutionCode::Size1080x1920, Some((1080, 1920))),
    resolution(VideoResolutionCode::Size1512x2688, Some((1512, 2688))),
    resolution(VideoResolutionCode::Size5472x3648, Some((5472, 3648))),
    resolution(VideoResolutionCode::Size864x480, Some((864, 480))),
    // The code label does not establish exact dimensions.
    resolution(VideoResolutionCode::Code71, None),
    resolution(VideoResolutionCode::Size2688x2016, Some((2688, 2016))),
    resolution(VideoResolutionCode::Size5120x2700, Some((5120, 2700))),
    // The code label does not establish exact dimensions.
    resolution(VideoResolutionCode::Code99, None),
    resolution(VideoResolutionCode::Size1280x1024, Some((1280, 1024))),
    resolution(VideoResolutionCode::Size640x340, Some((640, 340))),
    resolution(VideoResolutionCode::Size3840x2880, Some((3840, 2880))),
];

const RESOLUTION_ID_COUNT: usize = u8::MAX as usize + 1;
static VIDEO_RESOLUTION_LOOKUP: [Option<&VideoResolutionInfo>; RESOLUTION_ID_COUNT] =
    build_resolution_lookup();

const fn build_resolution_lookup() -> [Option<&'static VideoResolutionInfo>; RESOLUTION_ID_COUNT] {
    let mut lookup = [None; RESOLUTION_ID_COUNT];
    let mut index = 0;
    while index < VIDEO_RESOLUTIONS.len() {
        let entry = &VIDEO_RESOLUTIONS[index];
        assert!(
            entry.code.wire() != VideoResolutionCode::MAX.wire()
                && entry.code.wire() != VideoResolutionCode::UNSET.wire(),
            "sentinel in resolution table"
        );
        let id = entry.code.wire() as usize;
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

    /// Replace raw options without applying any model's interpretation or limits.
    pub fn with_options(mut self, options: VideoFormatOptions) -> Self {
        self.format.options = options;
        self
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
            command: self.command(),
            flags,
            payload: self.payload()?,
            timeout,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket, decode, encode,
        identity::{CameraCommand, Command},
    };
    use std::fmt::Debug;

    const CURRENT: [u8; 5] = [0x16, 0x03, 0xa5, 0xff, 0x93];
    const TARGET: VideoResolutionCode = VideoResolutionCode::Size640x340;

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
            assert_eq!(format.resolution.wire(), code);
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
            T: Debug + PartialEq,
            for<'a> T: BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()>,
        {
            assert_eq!(decode::<T>(&[byte], ()).unwrap(), value);
            assert_eq!(encode(&value).unwrap(), [byte]);
            assert!(decode::<T>(&[], ()).is_err());
            assert!(decode::<T>(&[byte, 0], ()).is_err());
        }

        for code in u8::MIN..=u8::MAX {
            let resolution = VideoResolutionCode::from(code);
            assert_byte_round_trip(resolution, code);
            assert_eq!(resolution.wire(), code);
            assert_eq!(u8::from(resolution), code);
            assert_byte_round_trip(decode::<VideoFrameRateCode>(&[code], ()).unwrap(), code);
            assert_byte_round_trip(VideoFovCode(code), code);
        }
    }

    #[test]
    fn named_resolution_variants_cover_the_catalog_and_preserve_unknown_codes() {
        assert_eq!(VIDEO_RESOLUTIONS.len(), 37);
        for byte in u8::MIN..=u8::MAX {
            let code = VideoResolutionCode::from(byte);
            let entry = VIDEO_RESOLUTIONS
                .iter()
                .find(|entry| entry.code.wire() == byte);
            assert_eq!(code.info(), entry);
            match (entry, byte) {
                (Some(entry), _) => {
                    assert_eq!(code, entry.code);
                    assert!(!matches!(code, VideoResolutionCode::Unknown(_)));
                }
                (None, 253) => assert_eq!(code, VideoResolutionCode::Max),
                (None, 254) => assert_eq!(code, VideoResolutionCode::Unset),
                (None, _) => assert_eq!(code, VideoResolutionCode::Unknown(byte)),
            }
        }
        assert_eq!(VideoResolutionCode::Code71.info().unwrap().dimensions, None);
        assert_eq!(VideoResolutionCode::Code99.info().unwrap().dimensions, None);
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
    fn set_preserves_current_fields_and_encodes_wire_codes() {
        let set = VideoFormatSet::from_current(current(), TARGET, VideoFrameRateCode::Fps30);
        assert_eq!(
            CommandPacket::payload(&set).unwrap(),
            [0x65, 0x0e, 0xa5, 0xff, 0x93]
        );
        assert_eq!(set.format().fov, current().fov);
        assert_eq!(set.format().options, current().options);
        assert_eq!(
            CommandPacket::payload(&set.with_fov(VideoFovCode(4))).unwrap(),
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
            CommandPacket::payload(&set.with_speed_ratio_code(0x02).unwrap()).unwrap(),
            [0x65, 0x0e, 0xa5, 0xc2, 0x93]
        );
    }

    #[test]
    fn raw_options_accept_all_bits_without_model_limits_and_preserve_other_fields() {
        let set =
            VideoFormatSet::from_current(current(), TARGET, VideoFrameRateCode::Fps120000Over1001);
        for word in u16::MIN..=u16::MAX {
            let bytes = word.to_le_bytes();
            let updated = set.with_options(VideoFormatOptions(word));
            assert_eq!(
                updated.payload().unwrap(),
                [0x65, 7, 0xa5, bytes[0], bytes[1]]
            );
            assert_eq!(updated.format().options.0, word);
        }
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
        assert_eq!(request.payload, CommandPacket::payload(&set).unwrap());
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
