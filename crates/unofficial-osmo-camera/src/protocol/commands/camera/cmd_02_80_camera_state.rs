//! Camera state notification prefix.

use crate::{
    Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};

pub const PLAYBACK_STATE_MASK: u32 = 0x4000_0000;
pub const RECORDING_PHASE_MASK: u32 = 0x0000_00c0;
pub const CAMERA_STATE_PREFIX_LENGTH: usize = std::mem::size_of::<u32>();
pub const CAMERA_CONTENT_MODE_OFFSET: usize = 4;
pub const CAMERA_CAPTURE_MODE_OFFSET: usize = 57;
pub const CAMERA_FULL_STATUS_MIN_LENGTH: usize = CAMERA_CAPTURE_MODE_OFFSET + 1;
const CAMERA_CONTENT_MODE_EXTENSION_OFFSET: usize =
    CAMERA_CONTENT_MODE_OFFSET - CAMERA_STATE_PREFIX_LENGTH;
const CAMERA_CAPTURE_MODE_EXTENSION_OFFSET: usize =
    CAMERA_CAPTURE_MODE_OFFSET - CAMERA_STATE_PREFIX_LENGTH;

/// Legacy phase labels for the two-bit code in state bits 6..7.
///
/// The wire bit positions are established; the names do not prove every
/// transition's physical recording state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraRecordingPhase {
    Idle,
    Starting,
    Recording,
    Finalizing,
}

/// Compatibility labels for the content-mode byte at payload offset 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraContentMode {
    /// Wire value 0; the combined label is a legacy interpretation.
    PhotoOrTimelapse,
    /// Wire value 1.
    Video,
    /// Wire value 2.
    Playback,
    /// Any other wire value, retained unchanged.
    Unknown(u8),
}

impl From<u8> for CameraContentMode {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::PhotoOrTimelapse,
            1 => Self::Video,
            2 => Self::Playback,
            other => Self::Unknown(other),
        }
    }
}

/// Legacy Nano capture-mode labels for raw payload offset 57.
///
/// These codes label a model-qualified candidate view; they do not establish
/// that any mode is available for a particular firmware or current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, num_enum::FromPrimitive, num_enum::IntoPrimitive)]
#[repr(u8)]
pub enum NanoCaptureMode {
    SlowMotion = 0x00,
    Video = 0x01,
    Timelapse = 0x02,
    Photo = 0x05,
    Hyperlapse = 0x0a,
    SuperNight = 0x28,
    #[num_enum(catch_all)]
    Unknown(u8),
}

/// Read the known camera state word, retaining all other telemetry as opaque bytes.
///
/// Camera -> APP | 0x02/0x80 | Notification
///
/// ```text
/// notification {
///     state: le_u32 | bit 30 playback; bits 6-7 raw recording code
///     extension: [u8; remaining] | opaque
/// }
/// ```
///
/// At least four bytes are required by this prefix decoder. The full Nano
/// layout is 60 bytes, with no leading command-status byte. It includes mode
/// at offset 4, storage bytes at 5..=20, an opaque LE16 at 29, packed bytes at
/// 32/33, eight opaque bytes at 49..=56 and a raw mode code at 57.
/// State bit 30 can be set for content-mode code 2 or 7; code 7 remains
/// unknown to the legacy content-mode enum and must not be discarded.
/// Counters, elapsed-time units and capture-mode meanings remain unspecified.
/// No query or notification reply is defined.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraStateNotification {
    pub state: u32,
    pub extension: CameraRawData,
}

impl CameraStateNotification {
    pub fn playback_mode_active(&self) -> bool {
        self.state & PLAYBACK_STATE_MASK != 0
    }

    pub fn recording_phase(&self) -> CameraRecordingPhase {
        match self.state & RECORDING_PHASE_MASK {
            0x00 => CameraRecordingPhase::Idle,
            0x40 => CameraRecordingPhase::Starting,
            0x80 => CameraRecordingPhase::Recording,
            _ => CameraRecordingPhase::Finalizing,
        }
    }

    /// Apply compatibility labels when payload offset 4 is present.
    pub fn content_mode(&self) -> Option<CameraContentMode> {
        self.extension
            .0
            .get(CAMERA_CONTENT_MODE_EXTENSION_OFFSET)
            .copied()
            .map(CameraContentMode::from)
    }

    /// Return the model-qualified capture-mode code when payload offset 57 is present.
    pub fn nano_capture_mode(&self) -> Option<NanoCaptureMode> {
        self.extension
            .0
            .get(CAMERA_CAPTURE_MODE_EXTENSION_OFFSET)
            .copied()
            .map(NanoCaptureMode::from)
    }
}

impl WireDecode for CameraStateNotification {
    fn read_from(reader: &mut WireReader<'_>, context: &()) -> Result<Self> {
        Ok(Self {
            state: reader.read_u32_le()?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for CameraStateNotification {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u32_le(self.state);
        self.extension.write_to(writer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_prefix_retains_unknown_bits_and_tail() {
        let bytes = [0x21, 0x43, 0x65, 0xc0, 0xde, 0xad];
        let value = CameraStateNotification::decode(&bytes, &()).unwrap();
        assert!(value.playback_mode_active());
        assert_eq!(value.encode().unwrap(), bytes);
        assert_eq!(value.extension.0, [0xde, 0xad]);
        for length in 0..CAMERA_STATE_PREFIX_LENGTH {
            assert!(CameraStateNotification::decode(&bytes[..length], &()).is_err());
        }
        let value = CameraStateNotification {
            state: !PLAYBACK_STATE_MASK,
            extension: CameraRawData(vec![]),
        };
        assert!(!value.playback_mode_active());
    }

    #[test]
    fn status_views_require_only_their_own_offsets_and_preserve_unknowns() {
        let mut bytes = vec![0; CAMERA_FULL_STATUS_MIN_LENGTH];
        bytes[0] = 0x80;
        bytes[CAMERA_CONTENT_MODE_OFFSET] = 1;
        bytes[CAMERA_CAPTURE_MODE_OFFSET] = 0x28;
        let value = CameraStateNotification::decode(&bytes, &()).unwrap();
        assert_eq!(value.recording_phase(), CameraRecordingPhase::Recording);
        assert_eq!(value.content_mode(), Some(CameraContentMode::Video));
        assert_eq!(value.nano_capture_mode(), Some(NanoCaptureMode::SuperNight));
        assert_eq!(value.encode().unwrap(), bytes);

        let short = CameraStateNotification::decode(&[0x40, 0, 0, 0, 0], &()).unwrap();
        assert_eq!(short.recording_phase(), CameraRecordingPhase::Starting);
        assert_eq!(
            short.content_mode(),
            Some(CameraContentMode::PhotoOrTimelapse)
        );
        assert_eq!(short.nano_capture_mode(), None);

        let mut unknown = vec![0; CAMERA_FULL_STATUS_MIN_LENGTH];
        unknown[0] = 0xc0;
        unknown[CAMERA_CONTENT_MODE_OFFSET] = 0xff;
        unknown[CAMERA_CAPTURE_MODE_OFFSET] = 0xfe;
        let value = CameraStateNotification::decode(&unknown, &()).unwrap();
        assert_eq!(value.recording_phase(), CameraRecordingPhase::Finalizing);
        assert_eq!(value.content_mode(), Some(CameraContentMode::Unknown(0xff)));
        assert_eq!(
            value.nano_capture_mode(),
            Some(NanoCaptureMode::Unknown(0xfe))
        );
        assert_eq!(value.encode().unwrap(), unknown);
    }

    #[test]
    fn playback_flag_does_not_require_the_legacy_mode_two_label() {
        let mut bytes = vec![0; 60];
        bytes[..4].copy_from_slice(&PLAYBACK_STATE_MASK.to_le_bytes());
        bytes[CAMERA_CONTENT_MODE_OFFSET] = 7;
        let value = CameraStateNotification::decode(&bytes, &()).unwrap();
        assert!(value.playback_mode_active());
        assert_eq!(value.content_mode(), Some(CameraContentMode::Unknown(7)));
        assert_eq!(value.encode().unwrap(), bytes);
    }
}
