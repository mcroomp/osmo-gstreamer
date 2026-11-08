//! Encoding-only stream and LCD output variants for camera command 0x02/0x4C.

use binrw::{BinRead, BinWrite, binwrite};

/// APP -> Camera | 0x02/0x4C | request: `[4, 0, 0, format, 0, 0x20]`.
///
/// The fixed bytes do not establish dimensions or bitrate. Reply layout,
/// flags, model support and readback are unspecified. Encoding authorizes no write.
/// Determine delivered dimensions from the received bitstream, not this selector.
#[binwrite]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[bw(magic = b"\x02\x4c")]
pub struct LiveViewStreamOutputSet {
    #[bw(calc = [4u8, 0u8, 0u8])]
    prefix: [u8; 3],
    pub format: LiveViewStreamFormat,
    #[bw(calc = [0u8, 0x20])]
    suffix: [u8; 2],
}

/// APP -> Camera | 0x02/0x4C | request: `[1, format, 0, 0, 0, 0]`.
///
/// Receiver selection is model-dependent; flags, reply layout and support
/// remain unspecified. This is not a read-only query or permission to write.
#[binwrite]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[bw(magic = b"\x02\x4c")]
pub struct LcdOutputSet {
    #[bw(calc = [1u8])]
    prefix: [u8; 1],
    #[bw(pad_after = 4)]
    pub format: LcdOutputFormat,
}

/// Stream-output labels; pixel dimensions and frame rates remain unresolved.
///
/// These identifiers do not share the LCD-output namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum LiveViewStreamFormat {
    Auto = 0,
    SdSf = 1,
    HdSf = 2,
    SdHf = 3,
    HdHf = 4,
}

/// LCD-output labels, not stream-resolution identifiers or guaranteed support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, BinWrite, BinRead)]
#[brw(repr = u8)]
#[repr(u8)]
pub enum LcdOutputFormat {
    Auto = 0,
    Hd720p30 = 1,
    Hd720p60 = 2,
    FullHd1080p30 = 3,
    FullHd1080p60 = 4,
    AutoWithGoggles = 5,
    AutoWithoutGoggles = 6,
    AutoWithApp = 7,
}

#[cfg(test)]
mod tests {
    use super::{
        super::{
            cmd_02_1a_live_view_quality_set::{LiveViewQuality, LiveViewQualitySet},
            cmd_02_09_live_view_control::LiveViewControlRequest,
        },
        *,
    };
    use crate::protocol::{
        CommandPacket, ExplorationRequest, Flags,
        identity::{CameraCommand, Command, Endpoint},
    };
    use std::time::Duration;

    #[test]
    fn live_view_encoders_never_become_approved_reads() {
        let fixtures = [
            (
                CameraCommand::LiveViewControl,
                CommandPacket::payload(&LiveViewControlRequest::start()).unwrap(),
            ),
            (
                CameraCommand::LiveViewControl,
                CommandPacket::payload(&LiveViewControlRequest::stop()).unwrap(),
            ),
            (
                CameraCommand::LiveViewQualitySet,
                LiveViewQualitySet {
                    quality: LiveViewQuality::Normal,
                }
                .payload()
                .unwrap(),
            ),
            (
                CameraCommand::LiveViewOutputSet,
                LiveViewStreamOutputSet {
                    format: LiveViewStreamFormat::Auto,
                }
                .payload()
                .unwrap(),
            ),
            (
                CameraCommand::LiveViewOutputSet,
                LcdOutputSet {
                    format: LcdOutputFormat::Auto,
                }
                .payload()
                .unwrap(),
            ),
        ];
        for (command, payload) in fixtures {
            let request = ExplorationRequest {
                receiver: Endpoint::Camera,
                command: Command::Camera(command),
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
}
