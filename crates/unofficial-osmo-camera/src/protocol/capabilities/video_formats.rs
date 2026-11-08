//! Nano recording metadata, not an encoder configuration or firmware allowlist.
//!
//! Dimensions and normal/slow-motion/time-lapse limits follow
//! [DJI specifications](https://www.dji.com/nano/specs), consulted 2026-11-07.
//! Wire IDs, nominal FPS labels and SuperNight combinations follow
//! [independent MIT-licensed observations](https://github.com/luisreysan/osmocam/blob/2b1b8a1f94f59578f5092d562fa937fafd0d3a74/docs/PROTOCOL.md#72-video-format).
//! The observations do not identify firmware. No external code or captures are
//! included. Current-mode capability notifications take precedence over this
//! published metadata; missing entries do not establish device rejection.
//!
//! The same public observations describe an empty recording-format GET (`02/19`)
//! returning a five-byte format value. Nano nominal 24/25/30/48/50/60 labels use
//! frame-rate codes 1..=6; exact cadence can be fractional.
//!
//! Nano Wi-Fi preview observations are not selectable output modes. The SD/HD
//! labels in `02/4C`, LCD-output labels, stream-quality codes and GStreamer caps
//! do not establish a numeric 720p/1080p preview selector. Read actual dimensions
//! from the received bitstream.
//!
//! Public Nano notes report a one-byte `0046` aspect TLV with labels 1=4:3 and
//! 2=16:9, conflicting with the existing four-byte compatibility codec. Preserve
//! that codec until model/firmware-specific reconciliation; do not infer a new
//! layout or safe aspect SET from these notes.

use crate::{
    Error, Result,
    protocol::{
        commands::camera::{
            cmd_02_18_video_format_set::{
                VideoFormat, VideoFormatOptions, VideoFormatSet, VideoFrameRateCode,
                VideoResolutionCode,
            },
            cmd_02_80_camera_state::NanoCaptureMode,
        },
        video_formats::{
            PreviewObservation, RecordingAspect, RecordingFormat, RecordingResolution,
        },
    },
};

/// Six officially listed recording sizes with independently reported wire IDs.
///
/// The generic `VIDEO_RESOLUTIONS` table is deliberately not used as a Nano list.
pub const NANO_RECORDING_RESOLUTIONS: [RecordingResolution; 6] = [
    RecordingResolution {
        code: VideoResolutionCode::Size3840x2160,
        dimensions: (3840, 2160),
        aspect: RecordingAspect::SixteenByNine,
    },
    RecordingResolution {
        code: VideoResolutionCode::Size3840x2880,
        dimensions: (3840, 2880),
        aspect: RecordingAspect::FourByThree,
    },
    RecordingResolution {
        code: VideoResolutionCode::Size2688x1512,
        dimensions: (2688, 1512),
        aspect: RecordingAspect::SixteenByNine,
    },
    RecordingResolution {
        code: VideoResolutionCode::Size2688x2016,
        dimensions: (2688, 2016),
        aspect: RecordingAspect::FourByThree,
    },
    RecordingResolution {
        code: VideoResolutionCode::Size1920x1080,
        dimensions: (1920, 1080),
        aspect: RecordingAspect::SixteenByNine,
    },
    RecordingResolution {
        code: VideoResolutionCode::Size1920x1440,
        dimensions: (1920, 1440),
        aspect: RecordingAspect::FourByThree,
    },
];

/// Published recording-mode metadata, not runtime mode availability.
pub const NANO_RECORDING_MODES: [NanoCaptureMode; 5] = [
    NanoCaptureMode::Video,
    NanoCaptureMode::SlowMotion,
    NanoCaptureMode::SuperNight,
    NanoCaptureMode::Timelapse,
    NanoCaptureMode::Hyperlapse,
];

/// Source of published dimensions and normal/slow-motion/time-lapse limits.
pub const NANO_SPECIFICATIONS_SOURCE: &str = "https://www.dji.com/nano/specs";
/// Firmware-unspecified source of wire IDs and recording/preview observations.
pub const NANO_FORMAT_OBSERVATIONS_SOURCE: &str = "https://github.com/luisreysan/osmocam/blob/2b1b8a1f94f59578f5092d562fa937fafd0d3a74/docs/PROTOCOL.md#72-video-format";

const NANO_FRAME_RATES: [(VideoFrameRateCode, u16); 8] = [
    (VideoFrameRateCode::Fps24000Over1001, 24),
    (VideoFrameRateCode::Fps25, 25),
    (VideoFrameRateCode::Fps30000Over1001, 30),
    (VideoFrameRateCode::Fps48000Over1001, 48),
    (VideoFrameRateCode::Fps50, 50),
    (VideoFrameRateCode::Fps60000Over1001, 60),
    (VideoFrameRateCode::Fps120000Over1001, 120),
    (VideoFrameRateCode::Fps240, 240),
];

/// Enumerate published combinations without transmitting or validating a SET.
///
/// Only the five documented/reported video capture modes have metadata.
/// Photo and unknown modes return an error rather than inheriting Video's list.
/// 4:3 prerequisites and mode side effects remain separate, unverified contracts.
pub fn nano_recording_formats(mode: NanoCaptureMode) -> Result<Vec<RecordingFormat>> {
    if !NANO_RECORDING_MODES.contains(&mode) {
        return Err(Error::Unsupported(
            "no published Nano recording-format metadata for this mode",
        ));
    }
    let mut formats = Vec::new();
    for resolution in NANO_RECORDING_RESOLUTIONS {
        let wide = resolution.aspect == RecordingAspect::SixteenByNine;
        for (frame_rate, nominal_fps) in NANO_FRAME_RATES {
            let normal = nominal_fps <= 60;
            let slow_motion = wide
                && (nominal_fps == 120
                    || nominal_fps == 240 && resolution.code == VideoResolutionCode::Size1920x1080);
            let permitted = mode == NanoCaptureMode::Video
                && normal
                && !(resolution.code == VideoResolutionCode::Size3840x2880 && nominal_fps == 60)
                || mode == NanoCaptureMode::SuperNight && wide && nominal_fps <= 30
                || matches!(
                    mode,
                    NanoCaptureMode::Timelapse | NanoCaptureMode::Hyperlapse
                ) && wide
                    && matches!(nominal_fps, 25 | 30)
                || mode == NanoCaptureMode::SlowMotion && slow_motion;
            if permitted {
                formats.push(RecordingFormat {
                    resolution,
                    frame_rate,
                    nominal_fps,
                    slow_motion_factor: (mode == NanoCaptureMode::SlowMotion)
                        .then_some(if nominal_fps == 240 { 8 } else { 4 }),
                });
            }
        }
    }
    Ok(formats)
}

/// Observations from the linked protocol notes, with no identified firmware.
///
/// Actual dimensions must be read from received H.264 SPS/decoder output.
/// Neither stream-quality codes, LCD labels nor GStreamer caps establish them.
pub const NANO_PREVIEW_OBSERVATIONS: [PreviewObservation; 2] = [
    PreviewObservation {
        recording_aspect: RecordingAspect::SixteenByNine,
        dimensions: (1280, 720),
        nominal_fps: 30,
    },
    PreviewObservation {
        recording_aspect: RecordingAspect::FourByThree,
        dimensions: (960, 720),
        nominal_fps: 30,
    },
];

/// Return byte 4 as the Nano slow-motion factor candidate.
///
/// Call only for a Nano slow-motion format. Other models can place this field
/// differently, and this value does not validate the mode or format.
pub fn nano_slow_motion_factor(format: VideoFormat) -> u8 {
    (format.options.0 >> 8) as u8
}

/// Replace the Nano byte-4 Slow Motion factor, preserving every other byte.
///
/// Only the independently reported factors 4 and 8 are accepted. The caller
/// must select a Slow Motion mode and a compatible resolution/rate separately.
/// This does not set the unrelated low-six-bit speed-ratio code or transmit.
pub fn with_nano_slow_motion_factor(set: VideoFormatSet, factor: u8) -> Result<VideoFormatSet> {
    if !matches!(factor, 4 | 8) {
        return Err(Error::UnsupportedValue {
            field: "Nano slow-motion factor",
            value: factor,
        });
    }
    let options = VideoFormatOptions((set.format().options.0 & 0x00ff) | (u16::from(factor) << 8));
    Ok(set.with_options(options))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{CommandPacket, decode, encode};
    use std::collections::HashSet;

    #[test]
    fn all_six_nano_sizes_have_byte_exact_distinct_codes_and_dimensions() {
        let expected = [
            (0x10, (3840, 2160)),
            (0x67, (3840, 2880)),
            (0x2d, (2688, 1512)),
            (0x5f, (2688, 2016)),
            (0x0a, (1920, 1080)),
            (0x0c, (1920, 1440)),
        ];
        for (entry, (code, dimensions)) in NANO_RECORDING_RESOLUTIONS.iter().zip(expected) {
            assert_eq!(encode(&entry.code).unwrap(), [code]);
            assert_eq!(entry.code.wire(), code);
            assert_eq!(entry.dimensions, dimensions);
            assert_eq!(entry.code.info().unwrap().dimensions, Some(dimensions));
        }
    }

    #[test]
    fn nano_resolutions_are_an_explicit_subset_of_the_protocol_enum() {
        let expected = [
            VideoResolutionCode::Size3840x2160,
            VideoResolutionCode::Size3840x2880,
            VideoResolutionCode::Size2688x1512,
            VideoResolutionCode::Size2688x2016,
            VideoResolutionCode::Size1920x1080,
            VideoResolutionCode::Size1920x1440,
        ];
        assert_eq!(NANO_RECORDING_RESOLUTIONS.map(|entry| entry.code), expected);
        for code in [
            VideoResolutionCode::Size640x480,
            VideoResolutionCode::Size7680x4320,
        ] {
            assert!(code.info().is_some());
            assert!(
                !NANO_RECORDING_RESOLUTIONS
                    .iter()
                    .any(|entry| entry.code == code)
            );
        }
    }

    #[test]
    fn nano_slow_motion_factor_interprets_only_the_model_specific_byte() {
        let format = decode::<VideoFormat>(&[0x10, 0x08, 0, 0, 4], ()).unwrap();
        assert_eq!(nano_slow_motion_factor(format), 4);
        assert_eq!(encode(&format).unwrap(), [0x10, 0x08, 0, 0, 4]);

        let other_layout = decode::<VideoFormat>(&[0x10, 0x08, 0, 8, 0], ()).unwrap();
        assert_eq!(nano_slow_motion_factor(other_layout), 0);
        assert_eq!(encode(&other_layout).unwrap(), [0x10, 0x08, 0, 8, 0]);
    }

    #[test]
    fn nano_slow_motion_updates_only_byte_four_and_checks_reported_factors() {
        let current = decode::<VideoFormat>(&[0x16, 0x03, 0xa5, 0xff, 0x93], ()).unwrap();
        let set = VideoFormatSet::from_current(
            current,
            VideoResolutionCode::Size640x340,
            VideoFrameRateCode::Fps120000Over1001,
        );
        for factor in u8::MIN..=u8::MAX {
            let result = with_nano_slow_motion_factor(set, factor);
            if matches!(factor, 4 | 8) {
                let updated = result.unwrap();
                assert_eq!(updated.payload().unwrap(), [0x65, 7, 0xa5, 0xff, factor]);
                assert_eq!(nano_slow_motion_factor(updated.format()), factor);
            } else {
                assert!(matches!(result, Err(Error::UnsupportedValue { .. })));
            }
        }
    }

    #[test]
    fn normal_video_has_exactly_35_unique_pairs_and_no_4k_four_by_three_at_60() {
        let formats = nano_recording_formats(NanoCaptureMode::Video).unwrap();
        let mut pairs = HashSet::new();
        for entry in &formats {
            pairs.insert((
                encode(&entry.resolution.code).unwrap()[0],
                entry.nominal_fps,
            ));
            assert!(entry.slow_motion_factor.is_none());
            assert!([24, 25, 30, 48, 50, 60].contains(&entry.nominal_fps));
        }
        assert_eq!(formats.len(), 35);
        assert_eq!(pairs.len(), 35);
        for resolution in NANO_RECORDING_RESOLUTIONS {
            let code = encode(&resolution.code).unwrap()[0];
            for rate in [24, 25, 30, 48, 50, 60] {
                assert_eq!(pairs.contains(&(code, rate)), !(code == 0x67 && rate == 60));
            }
        }
    }

    #[test]
    fn mode_metadata_uses_nano_rate_codes_and_never_inherits_unknown_modes() {
        for (mode, count, rates) in [
            (NanoCaptureMode::SuperNight, 9, &[24, 25, 30][..]),
            (NanoCaptureMode::Timelapse, 6, &[25, 30][..]),
            (NanoCaptureMode::Hyperlapse, 6, &[25, 30][..]),
        ] {
            let formats = nano_recording_formats(mode).unwrap();
            assert_eq!(formats.len(), count);
            for entry in formats {
                assert_eq!(entry.resolution.aspect, RecordingAspect::SixteenByNine);
                assert!(rates.contains(&entry.nominal_fps));
                assert!(entry.slow_motion_factor.is_none());
            }
        }
        let slow = nano_recording_formats(NanoCaptureMode::SlowMotion).unwrap();
        assert_eq!(slow.len(), 4);
        for entry in slow {
            let code = encode(&entry.frame_rate).unwrap()[0];
            assert_eq!(
                (code, entry.slow_motion_factor),
                if entry.nominal_fps == 240 {
                    (8, Some(8))
                } else {
                    (7, Some(4))
                }
            );
        }
        for (rate, nominal) in NANO_FRAME_RATES[..6].iter().zip([24, 25, 30, 48, 50, 60]) {
            assert_eq!(rate.1, nominal);
        }
        let codes: Vec<_> = NANO_FRAME_RATES[..6]
            .iter()
            .map(|(rate, _)| encode(rate).unwrap()[0])
            .collect();
        assert_eq!(codes, [1, 2, 3, 4, 5, 6]);
        assert!(nano_recording_formats(NanoCaptureMode::Photo).is_err());
        assert!(nano_recording_formats(NanoCaptureMode::Unknown(0x34)).is_err());
        assert_eq!(NANO_PREVIEW_OBSERVATIONS[1].dimensions, (960, 720));
    }
}
