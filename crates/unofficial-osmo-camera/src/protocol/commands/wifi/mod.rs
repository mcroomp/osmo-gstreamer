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
        Query, ResponseStatus, WireDecode, WireEncode, WireWriter,
        commands::reply::{CameraRawData, CameraReadReply},
        identity::{Command, Endpoint, WifiCommand},
    };

    #[test]
    fn packed_strings_and_pairing_payloads_preserve_lengths_and_bytes() {
        let mut writer = WireWriter::new();
        packed_string::write_packed_string("osmo", &mut writer).unwrap();
        assert_eq!(writer.into_inner(), [4, b'o', b's', b'm', b'o']);
        assert_eq!(
            cmd_07_45_pair::PairingRequest { identity: "id" }
                .encode()
                .unwrap(),
            [2, b'i', b'd', 4, b'o', b's', b'm', b'o']
        );
        let payload = cmd_07_47_join_network::JoinNetworkRequest {
            ssid: "net",
            password: "password",
        }
        .encode()
        .unwrap();
        assert_eq!(
            payload,
            [
                3, b'n', b'e', b't', 8, b'p', b'a', b's', b's', b'w', b'o', b'r', b'd'
            ]
        );
        let too_long = "x".repeat(usize::from(u8::MAX) + 1);
        assert!(
            cmd_07_45_pair::PairingRequest {
                identity: &too_long
            }
            .encode()
            .is_err()
        );
        for (ssid, password) in [(&*too_long, "ok"), ("ok", &*too_long)] {
            assert!(
                cmd_07_47_join_network::JoinNetworkRequest { ssid, password }
                    .encode()
                    .is_err()
            );
        }
    }

    #[test]
    fn role_and_pairing_status_values_preserve_unknowns() {
        for (byte, role) in [
            (0, cmd_07_39_get_role::WifiRole::Hotspot),
            (1, cmd_07_39_get_role::WifiRole::Station),
        ] {
            assert_eq!(
                cmd_07_39_get_role::WifiRole::decode(&[byte], &()).unwrap(),
                role
            );
            assert_eq!(role.encode().unwrap(), [byte]);
            assert_eq!(
                cmd_07_48_set_role::WifiRoleSetRequest { role }
                    .encode()
                    .unwrap(),
                [byte]
            );
        }
        assert!(cmd_07_39_get_role::WifiRole::decode(&[2], &()).is_err());
        assert_eq!(
            cmd_07_45_pair::PairingState::from(0xff),
            cmd_07_45_pair::PairingState::Unknown(0xff)
        );
        assert_eq!(
            cmd_07_46_pair_approval::PairingApproval::from(0xff),
            cmd_07_46_pair_approval::PairingApproval::Unknown(0xff)
        );
        assert_eq!(ResponseStatus::Success.encode().unwrap(), [0]);
    }

    #[test]
    fn wifi_queries_keep_exact_routing_payloads_and_read_policy() {
        use crate::protocol::{ExplorationRequest, Flags};
        use std::time::Duration;

        fn check<Q: Query>(query: Q, command: WifiCommand, payload: &[u8]) {
            assert_eq!(query.receiver(), Endpoint::Wifi);
            assert_eq!(query.command(), Command::Wifi(command));
            assert_eq!(query.encode().unwrap(), payload);
            let request = ExplorationRequest {
                receiver: query.receiver(),
                command: query.command(),
                flags: Flags::REQUEST,
                payload: query.encode().unwrap(),
                timeout: Duration::from_secs(1),
            };
            assert!(request.is_known_read());
            request.validate(false, false).unwrap();
        }
        check(cmd_07_07_ssid::WifiSsidGet, WifiCommand::Ssid, &[]);
        check(cmd_07_0c_mac_get::WifiMacGet, WifiCommand::Mac, &[]);
        check(
            cmd_07_19_country_get::WifiCountryGet,
            WifiCommand::Country,
            &[],
        );
        check(cmd_07_39_get_role::WifiRoleGet, WifiCommand::GetRole, &[0]);
        for (command, payload) in [
            (
                WifiCommand::Password,
                cmd_07_0e_password::WifiPasswordRequest.encode().unwrap(),
            ),
            (
                WifiCommand::SetRole,
                cmd_07_48_set_role::WifiRoleSetRequest {
                    role: cmd_07_39_get_role::WifiRole::Station,
                }
                .encode()
                .unwrap(),
            ),
            (
                WifiCommand::Pair,
                cmd_07_45_pair::PairingRequest { identity: "id" }
                    .encode()
                    .unwrap(),
            ),
            (
                WifiCommand::JoinNetwork,
                cmd_07_47_join_network::JoinNetworkRequest {
                    ssid: "net",
                    password: "password",
                }
                .encode()
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
        use cmd_07_0e_password::WifiPasswordResponse;
        use cmd_07_07_ssid::WifiSsidResponse;

        let bytes = b"\x00\x03net\xff\xab";
        let reply = CameraReadReply::<WifiSsidResponse>::decode(bytes, &()).unwrap();
        assert_eq!(reply.encode().unwrap(), bytes);
        let CameraReadReply::Accepted(value) = reply else {
            panic!("expected accepted")
        };
        assert_eq!(value.ssid, "net");
        assert_eq!(value.extension.0, [0xff, 0xab]);
        let reply = CameraReadReply::<WifiPasswordResponse>::decode(bytes, &()).unwrap();
        assert_eq!(reply.encode().unwrap(), bytes);
        assert!(!format!("{reply:?}").contains("net"));
        for bytes in [&[][..], &[0][..], &[0, 3, b'n'][..], &[0, 1, 0xff][..]] {
            assert!(CameraReadReply::<WifiSsidResponse>::decode(bytes, &()).is_err());
            assert!(CameraReadReply::<WifiPasswordResponse>::decode(bytes, &()).is_err());
        }
        for status in u8::MIN..=u8::MAX {
            if ResponseStatus::from(status) == ResponseStatus::Success {
                continue;
            }
            let bytes = [status, 0xff, 0xab];
            let reply = CameraReadReply::<WifiPasswordResponse>::decode(&bytes, &()).unwrap();
            assert!(matches!(reply, CameraReadReply::Rejected { .. }));
            assert_eq!(reply.encode().unwrap(), bytes);
        }
        let empty = CameraReadReply::<WifiSsidResponse>::decode(&[0, 0], &()).unwrap();
        assert_eq!(empty.encode().unwrap(), [0, 0]);
    }

    #[test]
    fn mac_response_has_exact_length_and_preserves_rejection_extension() {
        use cmd_07_0c_mac_get::{WifiMacGet, WifiMacResponse};

        let bytes = [0, 1, 2, 3, 4, 5, 6];
        let reply = CameraReadReply::<WifiMacResponse>::decode(&bytes, &WifiMacGet).unwrap();
        assert_eq!(reply.encode().unwrap(), bytes);
        for length in 0..bytes.len() {
            assert!(
                CameraReadReply::<WifiMacResponse>::decode(&bytes[..length], &WifiMacGet).is_err()
            );
        }
        assert!(CameraReadReply::<WifiMacResponse>::decode(&[0; 8], &WifiMacGet).is_err());
        let bytes = [0xe0, 0xab];
        let reply = CameraReadReply::<WifiMacResponse>::decode(&bytes, &WifiMacGet).unwrap();
        assert_eq!(reply.encode().unwrap(), bytes);
    }

    #[test]
    fn pairing_responses_and_notifications_preserve_unknown_values_and_tails() {
        use cmd_07_45_pair::PairingResponse;
        use cmd_07_46_pair_approval::PairingApprovalNotification;

        for value in u8::MIN..=u8::MAX {
            let bytes = [0, value, 0xab];
            let reply = CameraReadReply::<PairingResponse>::decode(&bytes, &()).unwrap();
            assert_eq!(reply.encode().unwrap(), bytes);
            let bytes = [value, 0xab];
            let notification = PairingApprovalNotification::decode(&bytes, &()).unwrap();
            assert_eq!(notification.encode().unwrap(), bytes);
        }
        assert!(CameraReadReply::<PairingResponse>::decode(&[0], &()).is_err());
        assert!(PairingApprovalNotification::decode(&[], &()).is_err());
        let bytes = [0xe0, 0xab];
        let reply = CameraReadReply::<PairingResponse>::decode(&bytes, &()).unwrap();
        assert_eq!(reply.encode().unwrap(), bytes);
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
            cmd_07_45_pair::BlePairingArmRequest.encode().unwrap(),
            [1, 0]
        );
        let retry = CameraReadReply::<CameraRawData>::Rejected {
            status: cmd_07_47_join_network::JOIN_RETRY_STATUS,
            extension: CameraRawData(vec![cmd_07_47_join_network::JOIN_RETRY_DETAIL]),
        };
        assert_eq!(retry.encode().unwrap(), [1, 0xff]);
    }
}
