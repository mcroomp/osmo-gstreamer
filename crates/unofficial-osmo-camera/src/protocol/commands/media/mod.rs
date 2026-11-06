//! Media-endpoint streaming contracts, separate from camera-endpoint commands.
//!
//! These are unofficial interoperability encodings, not a guarantee of Nano
//! runtime support. A successful command reply does not establish publication.

pub mod cmd_02_8e_streaming_control;
pub mod cmd_02_e1_broadcast_mode;
pub mod cmd_08_78_streaming_settings;

#[cfg(test)]
mod tests {
    use super::{
        cmd_02_8e_streaming_control::*, cmd_02_e1_broadcast_mode::EnterBroadcastMode,
        cmd_08_78_streaming_settings::*,
    };
    use crate::protocol::{
        Flags, Message, Query,
        identity::{Command, Endpoint},
    };
    use crate::protocol::{decode, encode};

    fn profile() -> MediaStreamingProfile {
        MediaStreamingProfile {
            resolution: StreamingResolution::FullHd1080,
            bitrate_kbps: 2000,
            flags: 0x0100,
            fps_selector: 0,
            unknown_6: 0,
            unknown_7: 0,
            orientation_flags: 0,
            destination: b"rtmp://example.invalid/live".to_vec(),
        }
    }

    #[test]
    fn exact_mode_settings_start_stop_and_query_bytes() {
        assert_eq!(encode(&EnterBroadcastMode).unwrap(), [0x1a]);
        assert_eq!(
            encode(&MediaStreamingControl::Start).unwrap(),
            [1, 1, 0x1a, 0, 1, 1, 0, 0]
        );
        assert_eq!(
            encode(&MediaStreamingControl::StopAndQuit).unwrap(),
            [1, 1, 0x1a, 0, 1, 2, 0, 0]
        );
        assert_eq!(encode(&MediaStreamingStateGet).unwrap(), [0, 1, 0x1c, 0]);
        let bytes = encode(&MediaStreamingSettings { profile: profile() }).unwrap();
        assert_eq!(bytes.len(), 41);
        assert_eq!(
            &bytes[..14],
            [0, 38, 0, 10, 0xd0, 7, 0, 1, 0, 0, 0, 0, 27, 0]
        );
        assert_eq!(&bytes[14..], b"rtmp://example.invalid/live");
    }

    #[test]
    fn profile_lengths_are_bounded_by_bytes_and_unknown_fields_are_preserved() {
        let mut p = profile();
        p.destination.resize(MAX_DESTINATION_BYTES, b'x');
        let bytes = encode(&MediaStreamingSettings { profile: p.clone() }).unwrap();
        assert_eq!(&bytes[..3], [0, 255, 0]);
        assert_eq!(&bytes[12..14], [244, 0]);
        p.destination.push(b'x');
        assert!(encode(&p).is_err());
        p.destination = b"rtmps://example.invalid/live".to_vec();
        p.unknown_6 = 0x5a;
        p.unknown_7 = 0xa5;
        p.orientation_flags = 0xdf;
        p.fps_selector = 3;
        p.flags = 0x9876;
        assert_eq!(
            &encode(&p).unwrap()[3..9],
            [0x76, 0x98, 3, 0x5a, 0xa5, 0xdf]
        );
        p.destination = format!("rtmp://{}", "\u{00e9}".repeat(120)).into_bytes();
        assert!(encode(&p).is_err());
    }

    #[test]
    fn derived_profile_lengths_are_little_endian_and_failures_are_atomic() {
        use crate::Error;
        use binrw::{BinWrite, Endian};
        use std::io::Cursor;

        for destination in [
            b"rtmp://x".to_vec(),
            format!("rtmps://{}", "\u{00e9}".repeat(118)).into_bytes(),
        ] {
            let mut profile = profile();
            profile.destination = destination;
            let settings = MediaStreamingSettings { profile };
            let expected = encode(&settings).unwrap();
            assert_eq!(
                usize::from(u16::from_le_bytes([expected[1], expected[2]])),
                expected.len() - 3
            );
            assert_eq!(
                usize::from(u16::from_le_bytes([expected[12], expected[13]])),
                expected.len() - 14
            );
            let mut writer = Cursor::new(Vec::new());
            settings
                .write_options(&mut writer, Endian::Big, ())
                .unwrap();
            assert_eq!(writer.into_inner(), expected);
        }

        let mut profile = profile();
        profile.destination.resize(MAX_DESTINATION_BYTES + 1, b'x');
        let settings = MediaStreamingSettings { profile };
        let mut writer = std::io::Cursor::new(Vec::new());
        writer.get_mut().extend_from_slice(&[0xa5]);
        assert!(matches!(
            encode(&settings).map(|bytes| writer.get_mut().extend(bytes)),
            Err(Error::InvalidConfig {
                field: "RTMP destination",
                ..
            })
        ));
        assert_eq!(writer.into_inner(), [0xa5]);
    }

    #[test]
    fn plain_destination_rejects_json_wrong_schemes_nuls_and_empty_hosts() {
        for bytes in [
            b"".as_slice(),
            b"rtmp://",
            b"rtmps://",
            b"https://example.invalid/live",
            b"rtmp://example.invalid/\0key",
            br#"{"rtmpAddress":"rtmp://example.invalid/live"}"#,
        ] {
            let mut p = profile();
            p.destination = bytes.to_vec();
            assert!(encode(&p).is_err());
            assert!(encode(&MediaStreamingSettings { profile: p }).is_err());
        }
    }

    #[test]
    fn state_success_is_structured_and_rejections_are_not_fake_successes() {
        let bytes = [0, 0, 1, 0x1c, 0, 9, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        assert_eq!(
            decode::<MediaStreamingStateReply>(&bytes, ()).unwrap(),
            MediaStreamingStateReply::State([1, 2, 3, 4, 5, 6, 7, 8, 9])
        );
        for end in 0..bytes.len() {
            assert!(decode::<MediaStreamingStateReply>(&bytes[..end], ()).is_err());
        }
        for index in 2..6 {
            let mut invalid = bytes;
            invalid[index] ^= 0x40;
            assert!(decode::<MediaStreamingStateReply>(&invalid, ()).is_err());
        }
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(decode::<MediaStreamingStateReply>(&trailing, ()).is_err());
        assert_eq!(
            decode::<MediaStreamingStateReply>(&[0xd6], ()).unwrap(),
            MediaStreamingStateReply::Rejected(vec![0xd6])
        );
        assert_eq!(
            decode::<MediaStreamingStateReply>(&[0, 1], ()).unwrap(),
            MediaStreamingStateReply::Rejected(vec![0, 1])
        );
    }

    #[test]
    fn media_frames_use_recipient_08_and_normal_reply_correlation() {
        for (command, payload) in [
            (
                super::cmd_02_e1_broadcast_mode::COMMAND,
                encode(&EnterBroadcastMode).unwrap(),
            ),
            (
                super::cmd_08_78_streaming_settings::COMMAND,
                encode(&MediaStreamingSettings { profile: profile() }).unwrap(),
            ),
            (
                super::cmd_02_8e_streaming_control::COMMAND,
                encode(&MediaStreamingControl::Start).unwrap(),
            ),
        ] {
            let sent = Message::request(Endpoint::Media, 123, command, &payload);
            let decoded = Message::scan(&encode(&sent).unwrap());
            assert_eq!(decoded.len(), 1);
            assert_eq!(decoded[0].receiver, Endpoint::Media);
            assert_eq!(decoded[0].command, command);
            assert_eq!(decoded[0].payload, payload);
            let mut reply = Message {
                sender: Endpoint::Media,
                receiver: sent.sender,
                flags: Flags::RESPONSE,
                payload: vec![0],
                ..sent.clone()
            };
            assert!(reply.is_reply_to(&sent));
            reply.id += 1;
            assert!(!reply.is_reply_to(&sent));
            reply.id = sent.id;
            reply.sender = Endpoint::Camera;
            assert!(!reply.is_reply_to(&sent));
        }
        assert_eq!(MediaStreamingStateGet.receiver(), Endpoint::Media);
        assert_eq!(
            MediaStreamingStateGet.command(),
            Command::from_wire(2, 0x8e)
        );
    }
}
