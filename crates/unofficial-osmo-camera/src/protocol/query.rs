//! Typed reads with command-matched replies and bounded timeouts.

use super::{CommandPacket, Flags, Message, MessageContext, identity::Endpoint};
use crate::{Error, Result};
use binrw::{BinRead, meta::ReadMagic};
use std::time::Duration;

const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// A parameterless typed read request, constructed internally by the session.
///
/// The query type encodes command set, command ID and payload; callers use
/// `execute_query::<Q>()` without constructing a request value.
pub trait Query: CommandPacket + Default + Sized {
    type Response: for<'a> BinRead<Args<'a> = ()> + ReadMagic<MagicType = [u8; 2]>;
    /// Fixed read payload used by model tables; parameter queries encode fields.
    const REQUEST_PAYLOAD: &'static [u8] = &[];

    fn receiver(&self) -> Endpoint;
    fn request(&self, id: u16) -> Result<Message> {
        self.frame(MessageContext {
            sender: Endpoint::App,
            receiver: self.receiver(),
            id,
            flags: Flags::REQUEST,
        })
    }

    fn decode_reply(&self, reply: &Message) -> Result<Self::Response> {
        if !reply.flags.is_response() {
            return Err(Error::InvalidReply {
                command: self.command(),
                reason: "missing response flag",
            });
        }
        if <Self::Response as ReadMagic>::MAGIC != Self::COMMAND_ID {
            return Err(Error::Protocol("query and reply command magic disagree"));
        }
        reply.decode_command::<Self::Response>()
    }

    fn timeout(&self) -> Duration {
        DEFAULT_QUERY_TIMEOUT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        DUML_COMMAND, DUML_SET, MIN_ID, MessageContext,
        commands::{
            camera::{
                cmd_02_8e_parameters::{EisValue, Get},
                cmd_02_e0_capabilities_get::EiRangeGet,
                *,
            },
            common::{
                cmd_00_01_module_version_get::CameraModuleVersionGet,
                cmd_00_2b_session_keepalive::{
                    SessionKeepaliveOpenRequest, SessionKeepaliveRequest,
                },
                *,
            },
            media::*,
            reply::CommandReadReply,
            wifi::{cmd_07_39_get_role::WifiRoleGet, *},
        },
        decode, encode,
        identity::Command,
    };
    use binrw::{BinWrite, meta::WriteMagic};

    fn check_frame<Q: Query>(query: Q, expected_command: Command, expected_payload: &[u8]) {
        let typed = query.request(MIN_ID).unwrap();
        let raw = Message::request(query.receiver(), MIN_ID, expected_command, expected_payload);
        assert_eq!(typed, raw);
        let bytes = encode(&typed).unwrap();
        assert_eq!(bytes, encode(&raw).unwrap());
        let body = encode(&query).unwrap();
        assert_eq!(&bytes[DUML_SET..bytes.len() - 2], body);
        assert_eq!(bytes[DUML_COMMAND], expected_command.wire().1);
        assert_eq!(decode::<Message>(&bytes, ()).unwrap(), typed);
    }

    fn check_reply_magic<Q: Query>()
    where
        Q::Response: for<'a> BinWrite<Args<'a> = ()> + WriteMagic<MagicType = [u8; 2]>,
    {
        assert_eq!(Q::COMMAND_ID, <Q::Response as ReadMagic>::MAGIC);
        assert_eq!(Q::COMMAND_ID, <Q::Response as WriteMagic>::MAGIC);
        let query = Q::default();
        let mut message = Message::request(Endpoint::App, 0x1234, query.command(), &[0xe3, 0xa5]);
        message.sender = query.receiver();
        message.flags = Flags::RESPONSE;
        let reply = query.decode_reply(&message).unwrap();
        let bytes = encode(&reply).unwrap();
        assert_eq!(bytes, [Q::COMMAND_ID.as_slice(), &[0xe3, 0xa5]].concat());
        let framed = MessageContext {
            sender: message.sender,
            receiver: message.receiver,
            id: message.id,
            flags: message.flags,
        }
        .frame(&reply)
        .unwrap();
        assert_eq!(framed, message);
        for length in 0..3 {
            assert!(decode::<Q::Response>(&bytes[..length], ()).is_err());
        }
        let mut wrong = bytes;
        wrong[1] ^= 0xff;
        assert!(decode::<Q::Response>(&wrong, ()).is_err());
        message.command = Command::from_wire(Q::COMMAND_ID[0] ^ 0xff, Q::COMMAND_ID[1]);
        assert!(query.decode_reply(&message).is_err());
    }

    #[test]
    fn every_get_reply_pair_owns_matching_command_only_magic() {
        check_reply_magic::<cmd_00_01_module_version_get::CameraModuleVersionGet>();
        check_reply_magic::<cmd_02_19_video_format_get::VideoFormatGet>();
        check_reply_magic::<cmd_02_29_user_shutter_get::CameraUserShutterGet>();
        check_reply_magic::<cmd_02_2b_user_iso_get::CameraUserIsoGet>();
        check_reply_magic::<cmd_02_4b_photo_interval_get::CameraPhotoIntervalGet>();
        check_reply_magic::<cmd_02_57_language_get::CameraLanguageGet>();
        check_reply_magic::<cmd_02_5f_aeb_get::CameraAebGet>();
        check_reply_magic::<cmd_02_63_video_caption_get::CameraVideoCaptionGet>();
        check_reply_magic::<cmd_02_69_ae_lock_get::CameraAeLockGet>();
        check_reply_magic::<Get<EisValue>>();
        check_reply_magic::<cmd_02_8e_parameters::AdvancedLiveReversalGet>();
        check_reply_magic::<cmd_02_a0_audio_get::CameraAudioGet>();
        check_reply_magic::<cmd_02_da_storage_location_get::CameraStorageLocationGet>();
        check_reply_magic::<cmd_02_e5_watermark_get::CameraWatermarkGet>();
        check_reply_magic::<cmd_02_e0_capabilities_get::EiRangeGet>();
        check_reply_magic::<cmd_02_e0_capabilities_get::RecommendedEiGet>();
        check_reply_magic::<cmd_02_e0_capabilities_get::LensInformationGet>();
        check_reply_magic::<cmd_02_e0_capabilities_get::AccessoriesGet>();
        check_reply_magic::<cmd_02_8e_streaming_control::MediaStreamingStateGet>();
        check_reply_magic::<cmd_07_07_ssid::WifiSsidGet>();
        check_reply_magic::<cmd_07_0c_mac_get::WifiMacGet>();
        check_reply_magic::<cmd_07_19_country_get::WifiCountryGet>();
        check_reply_magic::<WifiRoleGet>();
    }

    #[test]
    fn typed_bodies_preserve_raw_frames_with_exactly_one_command_prefix() {
        check_frame(
            EiRangeGet::default(),
            Command::from_wire(2, 0xe0),
            &[2, 1, 1],
        );
        check_frame(WifiRoleGet::default(), Command::from_wire(7, 0x39), &[0]);
        check_frame(CameraModuleVersionGet, Command::from_wire(0, 1), &[]);
        check_frame(
            Get::<EisValue>::default(),
            Command::from_wire(2, 0x8e),
            &[0, 1, 8, 0],
        );
    }

    #[test]
    fn command_magic_is_only_identity_and_context_controls_header_not_payload() {
        assert_eq!(<EiRangeGet as WriteMagic>::MAGIC, [2, 0xe0]);
        assert_eq!(<SessionKeepaliveRequest as WriteMagic>::MAGIC, [0, 0x2b]);
        assert_eq!(
            <SessionKeepaliveOpenRequest as WriteMagic>::MAGIC,
            [0, 0x2b]
        );
        for context in [
            MessageContext {
                sender: Endpoint::App,
                receiver: Endpoint::Session,
                id: MIN_ID,
                flags: Flags::REQUEST,
            },
            MessageContext {
                sender: Endpoint::Session,
                receiver: Endpoint::App,
                id: 0x1234,
                flags: Flags::ACKNOWLEDGMENT,
            },
        ] {
            let message = context.frame(&SessionKeepaliveRequest::default()).unwrap();
            assert_eq!(message.sender, context.sender);
            assert_eq!(message.receiver, context.receiver);
            assert_eq!(message.id, context.id);
            assert_eq!(message.flags, context.flags);
            assert_eq!(message.command, Command::from_wire(0, 0x2b));
            assert_eq!(message.payload, [1, 1]);
            let bytes = encode(&message).unwrap();
            assert_eq!(decode::<Message>(&bytes, ()).unwrap(), message);
            assert_eq!(&bytes[DUML_SET..bytes.len() - 2], [0, 0x2b, 1, 1]);
            let open = context
                .frame(&SessionKeepaliveOpenRequest::default())
                .unwrap();
            assert_eq!(open.command, message.command);
            assert_eq!(open.payload, [4, 0]);
        }
    }

    #[test]
    fn reply_status_and_payload_cannot_hide_a_wrong_command_or_request_flag() {
        let query = EiRangeGet::default();
        let mut reply = Message::request(Endpoint::App, MIN_ID, query.command(), &[0, 0xff]);
        reply.flags = Flags::RESPONSE;
        let crate::protocol::commands::reply::CommandReadReply::Accepted(value) =
            query.decode_reply(&reply).unwrap()
        else {
            panic!("expected accepted data");
        };
        assert_eq!(value.raw_bytes(), [0xff]);
        assert!(WifiRoleGet::default().decode_reply(&reply).is_err());
        for command in [Command::from_wire(7, 0xe0), Command::from_wire(2, 0xe1)] {
            let changed = Message {
                command,
                ..reply.clone()
            };
            assert!(query.decode_reply(&changed).is_err());
        }
        reply.flags = Flags::REQUEST;
        assert!(query.decode_reply(&reply).is_err());
        reply.flags = Flags::RESPONSE;
        reply.payload = vec![0xe3, 0xaa];
        assert!(matches!(
            query.decode_reply(&reply).unwrap(),
            CommandReadReply::Rejected { .. }
        ));
        reply.command = WifiRoleGet::default().command();
        assert!(query.decode_reply(&reply).is_err());
    }
}
