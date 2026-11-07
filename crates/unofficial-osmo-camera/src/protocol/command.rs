use super::{CommandBody, Message, MessageContext, decode, encode, identity::Command};
use crate::{Error, Result};
use binrw::{BinRead, BinWrite, meta::WriteMagic};

/// Complete DUML command bodies with command-only binrw magic.
///
/// Fixed or variable payload fields follow the two command bytes. Framing
/// metadata and authorization are supplied by the caller, never by this trait.
pub trait CommandPacket:
    for<'a> BinWrite<Args<'a> = ()> + WriteMagic<MagicType = [u8; 2]> + Sized
{
    const COMMAND_ID: [u8; 2] = <Self as WriteMagic>::MAGIC;

    fn command(&self) -> Command {
        Command::from_wire(Self::COMMAND_ID[0], Self::COMMAND_ID[1])
    }

    fn payload(&self) -> Result<Vec<u8>> {
        let body = decode::<CommandBody>(&encode(self)?, ())?;
        if body.command != self.command() {
            return Err(Error::Protocol(
                "command body magic disagrees with its identity",
            ));
        }
        Ok(body.payload)
    }

    fn frame(&self, context: MessageContext) -> Result<Message> {
        let message = context.frame(self)?;
        if message.command != self.command() {
            return Err(Error::Protocol(
                "command body magic disagrees with its identity",
            ));
        }
        Ok(message)
    }
}

impl<T> CommandPacket for T where
    T: for<'a> BinWrite<Args<'a> = ()> + WriteMagic<MagicType = [u8; 2]>
{
}

/// A full reply body for a command, retaining the existing payload codec.
///
/// Nested accepted/rejected values do not repeat the command prefix.
#[binrw::binread]
pub struct CommandReply<C: CommandPacket, T: for<'a> BinRead<Args<'a> = ()>> {
    #[br(temp, assert(command == C::COMMAND_ID, "typed reply command mismatch"))]
    command: [u8; 2],
    pub value: T,
    #[br(ignore)]
    marker: std::marker::PhantomData<C>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        DUML_SET, Flags,
        commands::{
            camera::{
                cmd_02_0c_playback::{PlaybackMode, PlaybackRequest},
                cmd_02_1a_live_view_quality_set::{LiveViewQuality, LiveViewQualitySet},
                cmd_02_4c_live_view_output_set::{
                    LcdOutputFormat, LcdOutputSet, LiveViewStreamFormat, LiveViewStreamOutputSet,
                },
                cmd_02_09_live_view_source::{
                    LiveViewGate, LiveViewSourceRequest, LiveViewToggleRequest,
                },
                cmd_02_80_camera_state::CameraStateNotification,
            },
            camera_wake::cmd_53_10_wake::BleWakeRequest,
            common::cmd_00_88_app_presence::AppPresenceRequest,
            gimbal::cmd_03_da_initialize::GimbalInitializeRequest,
            live_view::cmd_09_a8_start_transmission::StartTransmissionRequest,
            media::{
                cmd_02_8e_streaming_control::{
                    MediaStreamingControl, MediaStreamingControlRequest,
                },
                cmd_02_e1_broadcast_mode::EnterBroadcastMode,
            },
            reply::RemainingBytes,
            wifi::{
                cmd_07_0e_password::WifiPasswordGet,
                cmd_07_39_get_role::WifiRole,
                cmd_07_45_pair::PairingRequest,
                cmd_07_46_pair_approval::{PairingApproval, PairingApprovalNotification},
                cmd_07_47_join_network::JoinNetworkRequest,
                cmd_07_48_set_role::WifiRoleSetRequest,
            },
        },
        identity::Endpoint,
    };

    fn check<T: CommandPacket>(packet: T, identity: [u8; 2], payload: &[u8]) {
        assert_eq!(T::COMMAND_ID, identity);
        assert_eq!(<T as WriteMagic>::MAGIC, identity);
        assert_eq!(packet.payload().unwrap(), payload);
        for context in [
            MessageContext {
                sender: Endpoint::App,
                receiver: Endpoint::Camera,
                id: 0x8092,
                flags: Flags::REQUEST,
            },
            MessageContext {
                sender: Endpoint::Camera,
                receiver: Endpoint::App,
                id: 0x1234,
                flags: Flags::ACKNOWLEDGMENT,
            },
        ] {
            let message = packet.frame(context).unwrap();
            let raw = Message {
                sender: context.sender,
                receiver: context.receiver,
                id: context.id,
                flags: context.flags,
                command: Command::from_wire(identity[0], identity[1]),
                payload: payload.to_vec(),
            };
            assert_eq!(message, raw);
            let bytes = encode(&message).unwrap();
            assert_eq!(bytes, encode(&raw).unwrap());
            assert_eq!(&bytes[DUML_SET..bytes.len() - 2], encode(&packet).unwrap());
            assert_eq!(decode::<Message>(&bytes, ()).unwrap(), raw);
        }
    }

    #[test]
    fn startup_write_and_notification_packets_preserve_payloads_and_frame_identity_once() {
        check(BleWakeRequest::default(), [0x53, 0x10], &[0; 4]);
        check(
            AppPresenceRequest::default(),
            [0, 0x88],
            &[
                0x17, 0, 0x46, 0x23, 0x7c, b'A', b'P', b'P', 0, 0, 0, 0, 0, 2,
            ],
        );
        check(
            GimbalInitializeRequest::default(),
            [3, 0xda],
            &[5, 255, 255, 255, 255],
        );
        check(
            StartTransmissionRequest::default(),
            [9, 0xa8],
            &[0, 4, 2, 0, 0, 0, 0, 0, 0, 0],
        );
        check(
            LiveViewSourceRequest::default(),
            [2, 9],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3],
        );
        check(
            LiveViewToggleRequest {
                gate: LiveViewGate::Disable,
            },
            [2, 9],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4],
        );
        check(
            PlaybackRequest {
                mode: PlaybackMode::Enter,
            },
            [2, 0x0c],
            &[1, 1, 0, 1],
        );
        check(
            LiveViewQualitySet {
                quality: LiveViewQuality::Fine,
            },
            [2, 0x1a],
            &[1],
        );
        check(
            LiveViewStreamOutputSet {
                format: LiveViewStreamFormat::HdSf,
            },
            [2, 0x4c],
            &[4, 0, 0, 2, 0, 0x20],
        );
        check(
            LcdOutputSet {
                format: LcdOutputFormat::Hd720p60,
            },
            [2, 0x4c],
            &[1, 2, 0, 0, 0, 0],
        );
        check(
            PairingRequest { identity: "id" },
            [7, 0x45],
            &[2, b'i', b'd', 4, b'o', b's', b'm', b'o'],
        );
        check(WifiPasswordGet, [7, 0x0e], &[]);
        check(
            WifiRoleSetRequest {
                role: WifiRole::Station,
            },
            [7, 0x48],
            &[1],
        );
        check(
            JoinNetworkRequest {
                ssid: "net",
                password: "",
            },
            [7, 0x47],
            &[3, b'n', b'e', b't', 0],
        );
        check(EnterBroadcastMode::default(), [2, 0xe1], &[0x1a]);
        check(
            MediaStreamingControlRequest {
                control: MediaStreamingControl::Start,
            },
            [2, 0x8e],
            &[1, 1, 0x1a, 0, 1, 1, 0, 0],
        );
        check(
            MediaStreamingControlRequest {
                control: MediaStreamingControl::StopAndQuit,
            },
            [2, 0x8e],
            &[1, 1, 0x1a, 0, 1, 2, 0, 0],
        );
        check(
            CameraStateNotification {
                state: 0x4000_0000,
                extension: RemainingBytes::<0>::new(vec![0xa5]).unwrap(),
            },
            [2, 0x80],
            &[0, 0, 0, 0x40, 0xa5],
        );
        check(
            PairingApprovalNotification {
                approval: PairingApproval::Unknown(5),
                extension: RemainingBytes::<0>::new(vec![0xab]).unwrap(),
            },
            [7, 0x46],
            &[5, 0xab],
        );
    }

    #[test]
    fn notification_decoding_cannot_accept_another_command_with_the_same_payload() {
        let packet = PairingApprovalNotification {
            approval: PairingApproval::Accepted01,
            extension: RemainingBytes::<0>::new(vec![]).unwrap(),
        };
        let mut message = packet
            .frame(MessageContext {
                sender: Endpoint::Wifi,
                receiver: Endpoint::App,
                id: 1,
                flags: Flags::REQUEST,
            })
            .unwrap();
        assert!(
            message
                .decode_command::<PairingApprovalNotification>()
                .is_ok()
        );
        message.command = Command::from_wire(7, 0x45);
        assert!(
            message
                .decode_command::<PairingApprovalNotification>()
                .is_err()
        );
    }
}
