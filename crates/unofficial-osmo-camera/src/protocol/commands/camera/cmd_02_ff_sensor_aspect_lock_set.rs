//! Sensor aspect-lock SET codec for camera expansion command 0x02/0xFF.
//!
//! Model-specific evidence and limitations live in
//! [`crate::protocol::capabilities`].

use binrw::{BinRead, BinWrite, binwrite};

/// Select a forced or automatic portrait/landscape aspect.
///
/// APP -> Camera | 0x02/0xFF | expansion selector 0x001F.
///
/// The encoded payload is exactly 34 bytes:
/// SET operation zero, little-endian selector, one mode byte and thirty zeros.
/// This is not a counted camera-parameter request.
///
/// An aspect lock does not disable gravity detection, EIS, upside-down pixel
/// rotation or client display transforms. A success ACK only confirms handoff;
/// mode restrictions, applied frame orientation and persistence remain unverified.
/// Encoding does not authorize transmission or initiate a setting change.
///
/// ```
/// use unofficial_osmo_camera::protocol::{
///     CommandPacket,
///     commands::camera::cmd_02_ff_sensor_aspect_lock_set::{
///         SensorAspectLock, SensorAspectLockSet,
///     },
/// };
///
/// let request = SensorAspectLockSet::new(SensorAspectLock::Landscape);
/// let payload = request.payload()?;
/// assert_eq!(payload.len(), 34);
/// assert_eq!(&payload[..4], &[0, 0x1f, 0, 0]);
/// # Ok::<(), unofficial_osmo_camera::Error>(())
/// ```
#[binwrite]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[bw(little, magic = b"\x02\xff")]
pub struct SensorAspectLockSet {
    #[bw(calc = 0u8)]
    operation: u8,
    #[bw(calc = Self::SELECTOR)]
    selector: u16,
    pub mode: SensorAspectLock,
    #[bw(calc = [0u8; 30])]
    opaque_slot_tail: [u8; 30],
}

impl SensorAspectLockSet {
    /// Expansion selector, distinct from the 0x02/0x8E parameter namespace.
    pub const SELECTOR: u16 = 0x001f;

    pub const fn new(mode: SensorAspectLock) -> Self {
        Self { mode }
    }
}

/// Aspect application modes, not four-way orientation or rotation degrees.
///
/// Only these three SET values are established; other bytes are invalid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum SensorAspectLock {
    Landscape = 0,
    Portrait = 1,
    Automatic = 2,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Error,
        protocol::{
            CommandPacket, Flags, Message, MessageContext, ResponseStatus, decode, encode,
            identity::{CameraCommand, Command, Endpoint},
        },
    };
    use binrw::Endian;
    use std::io::Cursor;

    #[test]
    fn all_modes_encode_exact_command_prefix_and_34_byte_payload() {
        for (mode, code) in [
            (SensorAspectLock::Landscape, 0),
            (SensorAspectLock::Portrait, 1),
            (SensorAspectLock::Automatic, 2),
        ] {
            let request = SensorAspectLockSet::new(mode);
            let mut expected = [0; 36];
            expected[..6].copy_from_slice(&[2, 0xff, 0, 0x1f, 0, code]);
            assert_eq!(encode(&request).unwrap(), expected);
            assert_eq!(request.payload().unwrap(), expected[2..]);
            assert_eq!(request, SensorAspectLockSet { mode });
            assert_eq!(SensorAspectLockSet::COMMAND_ID, [2, 0xff]);
            for endian in [Endian::Little, Endian::Big] {
                let mut writer = Cursor::new(vec![0xa5; 40]);
                writer.set_position(2);
                request.write_options(&mut writer, endian, ()).unwrap();
                assert_eq!(writer.position(), 38);
                let bytes = writer.into_inner();
                assert_eq!(&bytes[..2], &[0xa5; 2]);
                assert_eq!(&bytes[2..38], &expected);
                assert_eq!(&bytes[38..], &[0xa5; 2]);
            }
        }
    }

    #[test]
    fn modes_reject_unknown_codes_and_malformed_encodings() {
        for code in u8::MIN..=u8::MAX {
            let decoded = decode::<SensorAspectLock>(&[code], ());
            if code <= 2 {
                assert_eq!(encode(&decoded.unwrap()).unwrap(), [code]);
            } else {
                assert!(decoded.is_err());
            }
        }
        assert!(decode::<SensorAspectLock>(&[], ()).is_err());
        assert!(decode::<SensorAspectLock>(&[0, 0], ()).is_err());
    }

    #[test]
    fn typed_command_frames_to_camera_without_duplicating_the_prefix() {
        let request = SensorAspectLockSet::new(SensorAspectLock::Portrait);
        let command = Command::Camera(CameraCommand::Expansion);
        assert_eq!(request.command(), command);
        assert_eq!(Command::from_wire(2, 0xff), command);
        let framed = request
            .frame(MessageContext {
                sender: Endpoint::App,
                receiver: Endpoint::Camera,
                id: 0x1234,
                flags: Flags::REQUEST,
            })
            .unwrap();
        let raw = Message::request(
            Endpoint::Camera,
            0x1234,
            command,
            &request.payload().unwrap(),
        );
        assert_eq!(framed, raw);
        assert_eq!(
            decode::<Message>(&encode(&framed).unwrap(), ()).unwrap(),
            raw
        );
    }

    #[test]
    fn rejected_ack_does_not_become_success_or_discard_opaque_bytes() {
        let request = SensorAspectLockSet::new(SensorAspectLock::Automatic);
        let mut reply = Message::request(Endpoint::App, 0x1234, request.command(), &[0xe3, 0xa5]);
        reply.sender = Endpoint::Camera;
        reply.flags = Flags::ACKNOWLEDGMENT;
        assert!(matches!(
            reply.require_success(),
            Err(Error::CameraRejected {
                command: Command::Camera(CameraCommand::Expansion),
                status: ResponseStatus::Unknown(0xe3),
            })
        ));
        assert_eq!(reply.payload, [0xe3, 0xa5]);
        reply.payload = vec![0];
        assert!(reply.require_success().is_ok());
    }
}
