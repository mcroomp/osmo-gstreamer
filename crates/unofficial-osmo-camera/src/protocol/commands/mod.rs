//! Payload codecs grouped by command set and numeric command identity.

pub mod camera;
pub mod camera_wake;
pub mod common;
pub mod gimbal;
pub mod live_view;
pub mod media;
pub mod reply;
pub mod wifi;

#[cfg(test)]
mod tests {
    use super::{
        camera::{
            cmd_02_0c_playback::{PlaybackMode, PlaybackRequest},
            cmd_02_4c_live_view_output_set::{
                LcdOutputFormat, LcdOutputSet, LiveViewStreamFormat, LiveViewStreamOutputSet,
            },
        },
        camera_wake::cmd_53_10_wake::BleWakeRequest,
        common::{
            cmd_00_2b_session_keepalive::{SessionKeepaliveOpenRequest, SessionKeepaliveRequest},
            cmd_00_88_app_presence::AppPresenceRequest,
        },
        gimbal::cmd_03_da_initialize::GimbalInitializeRequest,
        live_view::cmd_09_a8_live_view_control::LiveViewIFrameRequest,
        wifi::{cmd_07_39_get_role::WifiRoleGet, cmd_07_45_pair::BlePairingArmRequest},
    };
    use crate::protocol::encode;
    use binrw::{BinWrite, Endian};
    use std::io::Cursor;

    fn assert_encoding<T>(value: T, expected: &[u8])
    where
        for<'a> T: BinWrite<Args<'a> = ()>,
    {
        assert_eq!(encode(&value).unwrap(), expected);
        for endian in [Endian::Little, Endian::Big] {
            let mut writer = Cursor::new(vec![0xa5; expected.len() + 4]);
            writer.set_position(2);
            value.write_options(&mut writer, endian, ()).unwrap();
            assert_eq!(writer.position(), 2 + expected.len() as u64);
            let bytes = writer.into_inner();
            assert_eq!(&bytes[..2], &[0xa5; 2]);
            assert_eq!(&bytes[2..2 + expected.len()], expected);
            assert_eq!(&bytes[2 + expected.len()..], &[0xa5; 2]);
        }
    }

    #[test]
    fn fixed_commands_write_exact_bytes_at_nonzero_offsets() {
        assert_encoding(BleWakeRequest::default(), &[0x53, 0x10, 0, 0, 0, 0]);
        assert_encoding(SessionKeepaliveRequest::default(), &[0, 0x2b, 1, 1]);
        assert_encoding(SessionKeepaliveOpenRequest::default(), &[0, 0x2b, 4, 0]);
        assert_encoding(
            AppPresenceRequest::default(),
            &[
                0, 0x88, 0x17, 0, 0x46, 0x23, 0x7c, b'A', b'P', b'P', 0, 0, 0, 0, 0, 2,
            ],
        );
        assert_encoding(
            GimbalInitializeRequest::default(),
            &[3, 0xda, 5, 0xff, 0xff, 0xff, 0xff],
        );
        assert_encoding(
            LiveViewIFrameRequest::default(),
            &[9, 0xa8, 0, 4, 2, 0, 0, 0, 0, 0, 0, 0],
        );
        assert_encoding(WifiRoleGet::default(), &[7, 0x39, 0]);
        assert_encoding(BlePairingArmRequest, &[1, 0]);
        for (mode, code) in [(PlaybackMode::Leave, 0), (PlaybackMode::Enter, 1)] {
            assert_encoding(PlaybackRequest { mode }, &[2, 0x0c, 1, 1, 0, code]);
        }
    }

    #[test]
    fn output_formats_preserve_prefixes_suffixes_and_zero_padding() {
        for (code, format) in [
            LiveViewStreamFormat::Auto,
            LiveViewStreamFormat::SdSf,
            LiveViewStreamFormat::HdSf,
            LiveViewStreamFormat::SdHf,
            LiveViewStreamFormat::HdHf,
        ]
        .into_iter()
        .enumerate()
        {
            assert_encoding(
                LiveViewStreamOutputSet { format },
                &[2, 0x4c, 4, 0, 0, code as u8, 0, 0x20],
            );
        }
        for (code, format) in [
            LcdOutputFormat::Auto,
            LcdOutputFormat::Hd720p30,
            LcdOutputFormat::Hd720p60,
            LcdOutputFormat::FullHd1080p30,
            LcdOutputFormat::FullHd1080p60,
            LcdOutputFormat::AutoWithGoggles,
            LcdOutputFormat::AutoWithoutGoggles,
            LcdOutputFormat::AutoWithApp,
        ]
        .into_iter()
        .enumerate()
        {
            assert_encoding(
                LcdOutputSet { format },
                &[2, 0x4c, 1, code as u8, 0, 0, 0, 0],
            );
        }
    }
}
