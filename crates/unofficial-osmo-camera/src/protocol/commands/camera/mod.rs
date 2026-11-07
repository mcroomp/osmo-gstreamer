//! Camera command payloads.
//!
//! Compatibility scope is Osmo Nano. Typed encoders do not establish device
//! support or authorize writes. Unknown codes/bits remain raw; legacy semantic
//! helper names are not additional guarantees about physical camera behavior.
//! Request encodings describe bytes emitted by this library, not universal
//! acceptance rules. Reply decoders state their own validated layouts.

pub mod cmd_02_09_live_view_control;
pub mod cmd_02_0c_playback;
pub mod cmd_02_18_video_format_set;
pub mod cmd_02_1a_live_view_quality_set;
pub mod cmd_02_29_user_shutter_get;
pub mod cmd_02_2b_user_iso_get;
pub mod cmd_02_4b_photo_interval_get;
pub mod cmd_02_4c_live_view_output_set;
pub mod cmd_02_57_language_get;
pub mod cmd_02_5f_aeb_get;
pub mod cmd_02_63_video_caption_get;
pub mod cmd_02_69_ae_lock_get;
pub mod cmd_02_80_camera_state;
pub mod cmd_02_8e_parameters;
/// Compatibility path for the parameter GET API.
pub mod cmd_02_8e_parameters_get {
    pub use super::cmd_02_8e_parameters::*;
}
pub mod cmd_02_a0_audio_get;
pub mod cmd_02_da_storage_location_get;
pub mod cmd_02_e0_capabilities_get;
pub mod cmd_02_e5_watermark_get;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket,
        commands::reply::{CommandReadReply, decode_payload},
    };

    #[test]
    fn scalar_and_fixed_prefix_replies_retain_extensions() {
        let ae = decode_payload::<CommandReadReply<cmd_02_69_ae_lock_get::CameraAeLockReply>>(
            &[0, 3, 0xaa],
            (),
        )
        .unwrap();
        let CommandReadReply::Accepted(ae) = ae else {
            panic!("expected accepted AE-lock reply");
        };
        assert!(ae.lock_bits_set());
        assert_eq!(ae.raw_bytes(), [3, 0xaa]);

        assert!(
            decode_payload::<CommandReadReply::<cmd_02_69_ae_lock_get::CameraAeLockReply>>(
                &[0],
                ()
            )
            .is_err()
        );
    }

    #[test]
    fn raw_caption_and_ae_codes_are_not_normalized_by_boolean_views() {
        for code in u8::MIN..=u8::MAX {
            let bytes = [0, code, 0xa5];
            let caption = decode_payload::<
                CommandReadReply<cmd_02_63_video_caption_get::CameraVideoCaptionReply>,
            >(&bytes, ())
            .unwrap();
            let ae = decode_payload::<CommandReadReply<cmd_02_69_ae_lock_get::CameraAeLockReply>>(
                &bytes,
                (),
            )
            .unwrap();
            assert_eq!(CommandPacket::payload(&caption).unwrap(), bytes);
            assert_eq!(CommandPacket::payload(&ae).unwrap(), bytes);
            let CommandReadReply::Accepted(caption) = caption else {
                panic!("expected caption code");
            };
            let CommandReadReply::Accepted(ae) = ae else {
                panic!("expected AE code");
            };
            assert_eq!(caption.enabled_bit_set(), code & 0x80 != 0);
            assert_eq!(ae.lock_bits_set(), code & 3 != 0);
        }
    }
}
