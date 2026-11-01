//! Camera command payloads.

pub mod cmd_02_09_live_view_source;
pub mod cmd_02_0c_playback;
pub mod cmd_02_18_video_format_set;
pub mod cmd_02_29_user_shutter_get;
pub mod cmd_02_2b_user_iso_get;
pub mod cmd_02_4b_photo_interval_get;
pub mod cmd_02_57_language_get;
pub mod cmd_02_5f_aeb_get;
pub mod cmd_02_63_video_caption_get;
pub mod cmd_02_69_ae_lock_get;
pub mod cmd_02_80_camera_state;
pub mod cmd_02_8e_parameters_get;
pub mod cmd_02_a0_audio_get;
pub mod cmd_02_da_storage_location_get;
pub mod cmd_02_e0_capabilities_get;
pub mod cmd_02_e5_watermark_get;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::commands::reply::CameraReadReply;
    use crate::protocol::{Query, WireDecode};

    fn check<Q: Query>(query: Q, id: u8, payload: &[u8]) {
        assert_eq!(
            query.receiver(),
            crate::protocol::identity::Endpoint::Camera
        );
        assert_eq!(query.command().wire(), (0x02, id));
        assert_eq!(query.encode().unwrap(), payload);
    }

    #[test]
    fn camera_gets_encode_their_exact_command_payloads() {
        check(cmd_02_2b_user_iso_get::CameraUserIsoGet, 0x2b, &[]);
        check(cmd_02_29_user_shutter_get::CameraUserShutterGet, 0x29, &[]);
        check(cmd_02_69_ae_lock_get::CameraAeLockGet, 0x69, &[]);
        check(cmd_02_a0_audio_get::CameraAudioGet, 0xa0, &[]);
        check(
            cmd_02_da_storage_location_get::CameraStorageLocationGet,
            0xda,
            &[0; 7],
        );
        check(cmd_02_5f_aeb_get::CameraAebGet, 0x5f, &[]);
        check(cmd_02_57_language_get::CameraLanguageGet, 0x57, &[]);
        check(
            cmd_02_63_video_caption_get::CameraVideoCaptionGet,
            0x63,
            &[],
        );
        check(
            cmd_02_4b_photo_interval_get::CameraPhotoIntervalGet,
            0x4b,
            &[],
        );
        check(
            cmd_02_e5_watermark_get::CameraWatermarkGet,
            0xe5,
            &[0x20, 0, 0, 0, 0, 0, 0],
        );
    }

    #[test]
    fn scalar_and_fixed_prefix_replies_retain_extensions() {
        let ae = CameraReadReply::<cmd_02_69_ae_lock_get::CameraAeLockValue>::decode(
            &[0, 3, 0xaa],
            &cmd_02_69_ae_lock_get::CameraAeLockGet,
        )
        .unwrap();
        let CameraReadReply::Accepted(ae) = ae else {
            panic!("expected accepted AE-lock reply");
        };
        assert!(ae.lock_bits_set());
        assert_eq!(ae.raw_bytes(), [3, 0xaa]);

        let audio = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 0xaa];
        let reply = CameraReadReply::<cmd_02_a0_audio_get::CameraAudioData>::decode(
            &audio,
            &cmd_02_a0_audio_get::CameraAudioGet,
        )
        .unwrap();
        let CameraReadReply::Accepted(audio) = reply else {
            panic!("expected accepted audio reply");
        };
        assert_eq!(audio.data_prefix().len(), 15);
        assert_eq!(audio.extension(), [0xaa]);
        assert!(
            CameraReadReply::<cmd_02_a0_audio_get::CameraAudioData>::decode(
                &[0; 15],
                &cmd_02_a0_audio_get::CameraAudioGet,
            )
            .is_err()
        );
    }
}
