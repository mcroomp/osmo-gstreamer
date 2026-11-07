//! Camera state notification prefix.

use crate::protocol::{commands::reply::RemainingBytes, decode};
use binrw::{BinRead, BinWrite};
use std::mem::size_of;

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
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(little)]
#[brw(magic = b"\x02\x80")]
pub struct CameraStateNotification {
    pub state: u32,
    pub extension: RemainingBytes,
}

pub const PLAYBACK_STATE_MASK: u32 = 0x4000_0000;
pub const RECORDING_PHASE_MASK: u32 = 0x0000_00c0;
pub const CAMERA_STATE_PREFIX_LENGTH: usize = size_of::<u32>();
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum NanoCaptureMode {
    #[brw(magic = 0x00u8)]
    SlowMotion,
    #[brw(magic = 0x01u8)]
    Video,
    #[brw(magic = 0x02u8)]
    Timelapse,
    #[brw(magic = 0x05u8)]
    Photo,
    #[brw(magic = 0x0au8)]
    Hyperlapse,
    #[brw(magic = 0x28u8)]
    SuperNight,
    Unknown(u8),
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
            .as_slice()
            .get(CAMERA_CONTENT_MODE_EXTENSION_OFFSET)
            .copied()
            .map(CameraContentMode::from)
    }

    /// Return the model-qualified capture-mode code when payload offset 57 is present.
    pub fn nano_capture_mode(&self) -> Option<NanoCaptureMode> {
        self.extension
            .as_slice()
            .get(CAMERA_CAPTURE_MODE_EXTENSION_OFFSET)
            .copied()
            .map(|code| {
                decode::<NanoCaptureMode>(&[code], ())
                    .expect("capture-mode fallback accepts every byte")
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Result,
        protocol::{
            CommandPacket, Message,
            identity::{Command, Endpoint},
        },
    };

    fn decode_state(payload: &[u8]) -> Result<CameraStateNotification> {
        Message::request(Endpoint::App, 1, Command::from_wire(2, 0x80), payload).decode_command()
    }

    #[test]
    fn state_prefix_retains_unknown_bits_and_tail() {
        let bytes = [0x21, 0x43, 0x65, 0xc0, 0xde, 0xad];
        let value = decode_state(&bytes).unwrap();
        assert!(value.playback_mode_active());
        assert_eq!(CommandPacket::payload(&value).unwrap(), bytes);
        assert_eq!(value.extension.as_slice(), [0xde, 0xad]);
        for length in 0..CAMERA_STATE_PREFIX_LENGTH {
            assert!(decode_state(&bytes[..length]).is_err());
        }
        let value = CameraStateNotification {
            state: !PLAYBACK_STATE_MASK,
            extension: RemainingBytes::<0>::new(vec![]).unwrap(),
        };
        assert!(!value.playback_mode_active());
    }

    #[test]
    fn status_views_require_only_their_own_offsets_and_preserve_unknowns() {
        let mut bytes = vec![0; CAMERA_FULL_STATUS_MIN_LENGTH];
        bytes[0] = 0x80;
        bytes[CAMERA_CONTENT_MODE_OFFSET] = 1;
        bytes[CAMERA_CAPTURE_MODE_OFFSET] = 0x28;
        let value = decode_state(&bytes).unwrap();
        assert_eq!(value.recording_phase(), CameraRecordingPhase::Recording);
        assert_eq!(value.content_mode(), Some(CameraContentMode::Video));
        assert_eq!(value.nano_capture_mode(), Some(NanoCaptureMode::SuperNight));
        assert_eq!(CommandPacket::payload(&value).unwrap(), bytes);

        let short = decode_state(&[0x40, 0, 0, 0, 0]).unwrap();
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
        let value = decode_state(&unknown).unwrap();
        assert_eq!(value.recording_phase(), CameraRecordingPhase::Finalizing);
        assert_eq!(value.content_mode(), Some(CameraContentMode::Unknown(0xff)));
        assert_eq!(
            value.nano_capture_mode(),
            Some(NanoCaptureMode::Unknown(0xfe))
        );
        assert_eq!(CommandPacket::payload(&value).unwrap(), unknown);
    }

    #[test]
    fn playback_flag_does_not_require_the_legacy_mode_two_label() {
        let mut bytes = vec![0; 60];
        bytes[..4].copy_from_slice(&PLAYBACK_STATE_MASK.to_le_bytes());
        bytes[CAMERA_CONTENT_MODE_OFFSET] = 7;
        let value = decode_state(&bytes).unwrap();
        assert!(value.playback_mode_active());
        assert_eq!(value.content_mode(), Some(CameraContentMode::Unknown(7)));
        assert_eq!(CommandPacket::payload(&value).unwrap(), bytes);
    }
}
