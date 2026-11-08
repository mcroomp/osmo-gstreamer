//! Hardware-neutral recording and preview metadata types.
//!
//! Model capabilities supply dimensions, combinations and option meanings.
//! These types neither encode a request nor establish hardware support.

use crate::protocol::commands::camera::cmd_02_18_video_format_set::{
    VideoFrameRateCode, VideoResolutionCode,
};

/// Recording aspect ratio, distinct from portrait/landscape orientation locks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordingAspect {
    SixteenByNine,
    FourByThree,
}

/// Model-scoped dimensions and aspect associated with a lossless wire code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordingResolution {
    pub code: VideoResolutionCode,
    pub dimensions: (u16, u16),
    pub aspect: RecordingAspect,
}

/// A model's recording combination, not a safe SET or exact physical cadence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordingFormat {
    pub resolution: RecordingResolution,
    pub frame_rate: VideoFrameRateCode,
    /// Menu label, which need not equal the exact physical frame rate.
    pub nominal_fps: u16,
    /// Model-interpreted factor; its presence does not imply a wire layout.
    pub slow_motion_factor: Option<u8>,
}

/// Model-scoped preview observation, not an available-output selector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewObservation {
    pub recording_aspect: RecordingAspect,
    pub dimensions: (u16, u16),
    pub nominal_fps: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_accepts_codes_and_combinations_outside_nano_capabilities() {
        let format = RecordingFormat {
            resolution: RecordingResolution {
                code: VideoResolutionCode::Unknown(0xab),
                dimensions: (5120, 2880),
                aspect: RecordingAspect::SixteenByNine,
            },
            frame_rate: VideoFrameRateCode::Unknown(0xee),
            nominal_fps: 72,
            slow_motion_factor: Some(3),
        };
        assert_eq!(format.resolution.code.wire(), 0xab);
        assert_eq!(format.frame_rate, VideoFrameRateCode::Unknown(0xee));
        assert_eq!(format.slow_motion_factor, Some(3));
        let preview = PreviewObservation {
            recording_aspect: RecordingAspect::FourByThree,
            dimensions: (1440, 1080),
            nominal_fps: 25,
        };
        assert_eq!(preview.dimensions, (1440, 1080));
    }
}
