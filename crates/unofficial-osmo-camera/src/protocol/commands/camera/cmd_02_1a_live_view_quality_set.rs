//! Encoding-only live-view quality payloads, distinct from recording quality.

use binrw::{BinRead, BinWrite};

/// APP -> Camera | 0x02/0x1A | request: one quality byte.
///
/// Reply layout, request flags, model support and readback are unspecified.
/// This encoding-only type is not a read-only query and authorizes no writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite)]
#[bw(magic = b"\x02\x1a")]
pub struct LiveViewQualitySet {
    pub quality: LiveViewQuality,
}

/// Observed quality labels without a resolution, frame-rate or bitrate mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum LiveViewQuality {
    Normal = 0,
    Fine = 1,
    SuperFine = 2,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Error,
        protocol::{
            CommandPacket, Flags, Message, ResponseStatus,
            identity::{CameraCommand, Command, Endpoint},
        },
    };

    #[test]
    fn host_quality_labels_remain_encoding_only_one_byte_payloads() {
        for (quality, byte) in [
            (LiveViewQuality::Normal, 0),
            (LiveViewQuality::Fine, 1),
            (LiveViewQuality::SuperFine, 2),
        ] {
            assert_eq!(LiveViewQualitySet { quality }.payload().unwrap(), [byte]);
        }
        assert_eq!(LiveViewQualitySet::COMMAND_ID, [2, 0x1a]);
    }

    #[test]
    fn unknown_status_is_rejected_without_discarding_opaque_bytes() {
        let reply = Message {
            sender: Endpoint::Camera,
            receiver: Endpoint::App,
            id: 0x1234,
            flags: Flags::ACKNOWLEDGMENT,
            command: Command::Camera(CameraCommand::LiveViewQualitySet),
            payload: vec![0xe0, 0xa5],
        };
        assert!(matches!(
            reply.require_success(),
            Err(Error::CameraRejected {
                command: Command::Camera(CameraCommand::LiveViewQualitySet),
                status: ResponseStatus::Unknown(0xe0),
            })
        ));
        assert_eq!(reply.payload, [0xe0, 0xa5]);
    }
}
