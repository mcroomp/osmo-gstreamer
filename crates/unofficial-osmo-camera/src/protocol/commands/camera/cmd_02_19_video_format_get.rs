//! Recording-format GET codec; separate from live-preview output settings.
//!
//! Layout provenance and model/firmware scope are documented in
//! [`crate::protocol::capabilities::video_formats`].

use super::cmd_02_18_video_format_set::VideoFormat;
use crate::protocol::{
    Query,
    commands::reply::{CommandReadReply, RemainingBytes},
    identity::Endpoint,
};
use binrw::{BinWrite, binrw};

/// Read the recording format for the current capture mode.
///
/// APP -> Camera | 0x02/0x19 | empty request.
/// Accepted replies contain status zero, a five-byte format, and any opaque tail.
/// A format read does not enumerate supported combinations or preview dimensions.
///
/// ```
/// use unofficial_osmo_camera::{
///     ControlSession, Result,
///     protocol::commands::{
///         camera::cmd_02_19_video_format_get::{VideoFormatGet, VideoFormatReply},
///         reply::CommandReadReply,
///     },
/// };
/// fn read(session: &mut ControlSession) -> Result<CommandReadReply<VideoFormatReply>> {
///     session.execute_query::<VideoFormatGet>()
/// }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\x19")]
pub struct VideoFormatGet;

/// Accepted recording-format reply, retaining unknown codes, options and tails.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(little, magic = b"\x02\x19")]
pub struct VideoFormatReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    pub format: VideoFormat,
    pub extension: RemainingBytes,
}

impl Query for VideoFormatGet {
    type Response = CommandReadReply<VideoFormatReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket, ExplorationRequest, Flags, Message, ResponseStatus,
        commands::camera::cmd_02_18_video_format_set::{
            VideoFormatSet, VideoFrameRateCode, VideoResolutionCode,
        },
        decode, encode,
        identity::{CameraCommand, Command},
    };
    use std::time::Duration;

    #[test]
    fn query_identity_and_read_approval_are_exact() {
        let query = VideoFormatGet;
        assert_eq!(encode(&query).unwrap(), [2, 0x19]);
        let message = query.request(0x1234).unwrap();
        assert_eq!(message.receiver, Endpoint::Camera);
        assert_eq!(
            message.command,
            Command::Camera(CameraCommand::VideoFormatGet)
        );
        assert!(message.payload.is_empty());
        let request = ExplorationRequest {
            receiver: message.receiver,
            command: message.command,
            flags: message.flags,
            payload: message.payload,
            timeout: Duration::from_secs(1),
        };
        request.validate(false, false).unwrap();
        for changed in [
            ExplorationRequest {
                payload: vec![0],
                ..request.clone()
            },
            ExplorationRequest {
                receiver: Endpoint::LiveView,
                ..request.clone()
            },
            ExplorationRequest {
                flags: Flags::RESPONSE,
                ..request
            },
        ] {
            assert!(!changed.is_known_read());
            assert!(changed.validate(false, false).is_err());
        }
    }

    #[test]
    fn accepted_reply_preserves_unknown_format_bits_and_extensions() {
        let bytes = [2, 0x19, 0, 0xfc, 0xfd, 0xa5, 0x34, 0x92, 0xaa, 0xbb];
        let reply = decode::<CommandReadReply<VideoFormatReply>>(&bytes, ()).unwrap();
        assert_eq!(encode(&reply).unwrap(), bytes);
        let CommandReadReply::Accepted(reply) = reply else {
            panic!("expected accepted format")
        };
        assert_eq!(reply.format.resolution, VideoResolutionCode::Unknown(0xfc));
        assert_eq!(reply.format.frame_rate, VideoFrameRateCode::Unknown(0xfd));
        assert_eq!(reply.format.options.0, 0x9234);
        assert_eq!(reply.extension.as_slice(), [0xaa, 0xbb]);
        let set = VideoFormatSet::from_current(
            reply.format,
            VideoResolutionCode::Size3840x2160,
            VideoFrameRateCode::Fps30000Over1001,
        );
        assert_eq!(set.payload().unwrap(), [0x10, 3, 0xa5, 0x34, 0x92]);
        for length in 0..8 {
            assert!(decode::<CommandReadReply<VideoFormatReply>>(&bytes[..length], ()).is_err());
        }
        assert!(decode::<CommandReadReply<VideoFormatReply>>(&bytes[..8], ()).is_ok());
    }

    #[test]
    fn rejected_reply_is_lossless_and_query_checks_command_and_flags() {
        let mut message = Message::request(
            Endpoint::App,
            0x1234,
            VideoFormatGet.command(),
            &[0xe3, 0xa5],
        );
        message.sender = Endpoint::Camera;
        message.flags = Flags::RESPONSE;
        let reply = VideoFormatGet.decode_reply(&message).unwrap();
        assert!(matches!(
            reply,
            CommandReadReply::Rejected {
                status: ResponseStatus::Unknown(0xe3),
                ..
            }
        ));
        assert_eq!(encode(&reply).unwrap(), [2, 0x19, 0xe3, 0xa5]);
        message.command = Command::Camera(CameraCommand::VideoFormatSet);
        assert!(VideoFormatGet.decode_reply(&message).is_err());
        message.command = VideoFormatGet.command();
        message.flags = Flags::REQUEST;
        assert!(VideoFormatGet.decode_reply(&message).is_err());
    }
}
