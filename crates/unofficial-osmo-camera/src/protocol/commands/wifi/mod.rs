//! Wi-Fi command payloads.

pub mod cmd_07_07_ssid;
pub mod cmd_07_0c_mac_get;
pub mod cmd_07_0e_password;
pub mod cmd_07_19_country_get;
pub mod cmd_07_39_get_role;
pub mod cmd_07_45_pair;
pub mod cmd_07_46_pair_approval;
pub mod cmd_07_47_join_network;
pub mod cmd_07_48_set_role;
mod packed_string;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket, ExplorationRequest, Flags, Message, Query,
        commands::reply::{CameraRawData, CameraReadReply, CommandReadReply, decode_payload},
        decode, encode,
        identity::{Command, Endpoint, WifiCommand},
    };
    use binrw::Endian;
    use cmd_07_0c_mac_get::WifiMacReply;
    use cmd_07_0e_password::WifiPasswordReply;
    use cmd_07_07_ssid::WifiSsidReply;
    use cmd_07_45_pair::PairingResponse;
    use cmd_07_46_pair_approval::PairingApprovalNotification;
    use std::{io::Cursor, time::Duration};

    #[test]
    fn packed_strings_and_pairing_payloads_preserve_lengths_and_bytes() {
        let mut writer = Cursor::new(Vec::new());
        packed_string::write_packed_string(&"osmo", &mut writer, Endian::Little, ()).unwrap();
        assert_eq!(writer.into_inner(), [4, b'o', b's', b'm', b'o']);
        assert_eq!(
            CommandPacket::payload(&cmd_07_45_pair::PairingRequest { identity: "id" }).unwrap(),
            [2, b'i', b'd', 4, b'o', b's', b'm', b'o']
        );
        let payload = CommandPacket::payload(&cmd_07_47_join_network::JoinNetworkRequest {
            ssid: "net",
            password: "password",
        })
        .unwrap();
        assert_eq!(
            payload,
            [
                3, b'n', b'e', b't', 8, b'p', b'a', b's', b's', b'w', b'o', b'r', b'd'
            ]
        );
        let too_long = "x".repeat(usize::from(u8::MAX) + 1);
        assert!(
            encode(&cmd_07_45_pair::PairingRequest {
                identity: &too_long
            })
            .is_err()
        );
        for (ssid, password) in [(&*too_long, "ok"), ("ok", &*too_long)] {
            assert!(
                encode(&cmd_07_47_join_network::JoinNetworkRequest { ssid, password }).is_err()
            );
        }
    }

    #[test]
    fn wifi_queries_and_writes_follow_read_policy() {
        fn check<Q: Query>(query: Q) {
            let request = ExplorationRequest {
                receiver: query.receiver(),
                command: query.command(),
                flags: Flags::REQUEST,
                payload: query.payload().unwrap(),
                timeout: Duration::from_secs(1),
            };
            assert!(request.is_known_read());
            request.validate(false, false).unwrap();
        }
        check(cmd_07_07_ssid::WifiSsidGet);
        check(cmd_07_0c_mac_get::WifiMacGet);
        check(cmd_07_19_country_get::WifiCountryGet);
        check(cmd_07_39_get_role::WifiRoleGet::default());
        for (command, payload) in [
            (
                WifiCommand::Password,
                encode(&cmd_07_0e_password::WifiPasswordGet).unwrap(),
            ),
            (
                WifiCommand::SetRole,
                encode(&cmd_07_48_set_role::WifiRoleSetRequest {
                    role: cmd_07_39_get_role::WifiRole::Station,
                })
                .unwrap(),
            ),
            (
                WifiCommand::Pair,
                encode(&cmd_07_45_pair::PairingRequest { identity: "id" }).unwrap(),
            ),
            (
                WifiCommand::JoinNetwork,
                encode(&cmd_07_47_join_network::JoinNetworkRequest {
                    ssid: "net",
                    password: "password",
                })
                .unwrap(),
            ),
        ] {
            let request = ExplorationRequest {
                receiver: Endpoint::Wifi,
                command: Command::Wifi(command),
                flags: Flags::REQUEST,
                payload,
                timeout: Duration::from_secs(1),
            };
            assert!(!request.is_known_read());
            assert!(request.validate(false, false).is_err());
        }
    }

    #[test]
    fn string_responses_preserve_tails_and_rejections() {
        let bytes = b"\x00\x03net\xff\xab";
        let reply = decode_payload::<CommandReadReply<WifiSsidReply>>(bytes, ()).unwrap();
        assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        let CommandReadReply::Accepted(value) = reply else {
            panic!("expected accepted")
        };
        assert_eq!(value.ssid, "net");
        assert_eq!(value.extension.0, [0xff, 0xab]);
        let reply = decode_payload::<CommandReadReply<WifiPasswordReply>>(bytes, ()).unwrap();
        assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        assert!(!format!("{reply:?}").contains("net"));
        for bytes in [&[][..], &[0][..], &[0, 3, b'n'][..], &[0, 1, 0xff][..]] {
            assert!(decode_payload::<CommandReadReply<WifiSsidReply>>(bytes, ()).is_err());
            assert!(decode_payload::<CommandReadReply<WifiPasswordReply>>(bytes, ()).is_err());
        }
        for status in u8::MIN..=u8::MAX {
            if status == 0 {
                continue;
            }
            let bytes = [status, 0xff, 0xab];
            let reply = decode_payload::<CommandReadReply<WifiPasswordReply>>(&bytes, ()).unwrap();
            assert!(matches!(reply, CommandReadReply::Rejected { .. }));
            assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        }
        let empty = decode_payload::<CommandReadReply<WifiSsidReply>>(&[0, 0], ()).unwrap();
        assert_eq!(CommandPacket::payload(&empty).unwrap(), [0, 0]);
    }

    #[test]
    fn mac_response_has_exact_length_and_preserves_rejection_extension() {
        let bytes = [0, 1, 2, 3, 4, 5, 6];
        let reply = decode_payload::<CommandReadReply<WifiMacReply>>(&bytes, ()).unwrap();
        assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        for length in 0..bytes.len() {
            assert!(
                decode_payload::<CommandReadReply<WifiMacReply>>(&bytes[..length], ()).is_err()
            );
        }
        assert!(decode_payload::<CommandReadReply<WifiMacReply>>(&[0; 8], ()).is_err());
        let bytes = [0xe0, 0xab];
        let reply = decode_payload::<CommandReadReply<WifiMacReply>>(&bytes, ()).unwrap();
        assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
    }

    #[test]
    fn pairing_responses_and_notifications_preserve_unknown_values_and_tails() {
        for value in u8::MIN..=u8::MAX {
            let bytes = [0, value, 0xab];
            let reply = decode::<CameraReadReply<PairingResponse>>(&bytes, ()).unwrap();
            assert_eq!(encode(&reply).unwrap(), bytes);
            let bytes = [value, 0xab];
            let notification = Message::request(
                Endpoint::App,
                1,
                Command::Wifi(WifiCommand::PairApproval),
                &bytes,
            )
            .decode_command::<PairingApprovalNotification>()
            .unwrap();
            assert_eq!(CommandPacket::payload(&notification).unwrap(), bytes);
        }
        assert!(decode::<CameraReadReply<PairingResponse>>(&[0], ()).is_err());
        assert!(decode::<PairingApprovalNotification>(&[7, 0x46,], ()).is_err());
        let bytes = [0xe0, 0xab];
        let reply = decode::<CameraReadReply<PairingResponse>>(&bytes, ()).unwrap();
        assert_eq!(encode(&reply).unwrap(), bytes);
        let pairing = cmd_07_45_pair::PairingRequest {
            identity: "private-identity",
        };
        assert!(!format!("{pairing:?}").contains("private-identity"));
        let join = cmd_07_47_join_network::JoinNetworkRequest {
            ssid: "private-network",
            password: "private-password",
        };
        assert!(!format!("{join:?}").contains("private-"));
        assert_eq!(
            encode(&cmd_07_45_pair::BlePairingArmRequest).unwrap(),
            [1, 0]
        );
        let retry = CameraReadReply::<CameraRawData>::Rejected {
            status: cmd_07_47_join_network::JOIN_RETRY_STATUS,
            extension: CameraRawData(vec![cmd_07_47_join_network::JOIN_RETRY_DETAIL]),
        };
        assert_eq!(encode(&retry).unwrap(), [1, 0xff]);
    }
}
