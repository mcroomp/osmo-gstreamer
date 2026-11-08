//! Strict binary encoding and decoding boundaries.

use crate::{Error, Result};
use binrw::{BinRead, BinWrite, Endian};
use std::{io::Cursor, sync::Arc};

/// Encode one complete little-endian value without exposing partial output.
pub fn encode<T>(value: &T) -> Result<Vec<u8>>
where
    for<'a> T: BinWrite<Args<'a> = ()>,
{
    let mut writer = Cursor::new(Vec::new());
    value
        .write_options(&mut writer, Endian::Little, ())
        .map_err(|source| codec_error("encode", source))?;
    Ok(writer.into_inner())
}

/// Decode one complete value using its native binrw arguments.
///
/// Attributes can override little-endian byte order. Trailing bytes are rejected.
pub fn decode<T: BinRead>(bytes: &[u8], args: T::Args<'_>) -> Result<T> {
    let mut reader = Cursor::new(bytes);
    let value = T::read_options(&mut reader, Endian::Little, args)
        .map_err(|source| codec_error("decode", source))?;
    if reader.position() != bytes.len() as u64 {
        return Err(Error::Protocol("trailing bytes after binary value"));
    }
    Ok(value)
}

pub(crate) fn binrw_error(pos: u64, error: Error) -> binrw::Error {
    binrw::Error::Custom {
        pos,
        err: Box::new(error),
    }
}

pub(crate) fn codec_error(operation: &'static str, source: binrw::Error) -> Error {
    if let Some(original) = source.custom_err::<Error>() {
        original.clone()
    } else {
        Error::WireCodec {
            operation,
            source: Arc::new(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        commands::{
            camera::{
                cmd_02_0c_playback::PlaybackMode,
                cmd_02_1a_live_view_quality_set::LiveViewQuality,
                cmd_02_4c_live_view_output_set::{LcdOutputFormat, LiveViewStreamFormat},
                cmd_02_8e_parameters::{CameraFieldOfView, CameraStabilizationMode},
                cmd_02_09_live_view_control::LiveViewControlAction,
                cmd_02_18_video_format_set::{VideoFrameRateCode, VideoResolutionCode},
                cmd_02_80_camera_state::NanoCaptureMode,
            },
            common::cmd_00_32_activation_status::ActivationState,
            reply::CameraReadReply,
            wifi::{
                cmd_07_39_get_role::WifiRole, cmd_07_45_pair::PairingState,
                cmd_07_46_pair_approval::PairingApproval,
            },
        },
        identity::*,
        *,
    };
    use std::{error::Error as _, fmt::Debug};

    fn assert_byte_codes<T>(valid: impl Fn(u8) -> bool)
    where
        T: Debug,
        for<'a> T: BinRead<Args<'a> = ()> + BinWrite<Args<'a> = ()>,
    {
        for byte in u8::MIN..=u8::MAX {
            let result = decode::<T>(&[byte], ());
            assert_eq!(result.is_ok(), valid(byte), "wire code {byte:#04x}");
            if let Ok(value) = result {
                assert_eq!(encode(&value).unwrap(), [byte]);
            }
        }
        assert!(decode::<T>(&[], ()).is_err());
        assert!(decode::<T>(&[0, 0], ()).is_err());
    }

    #[test]
    fn lossless_enums_preserve_every_byte_without_primitive_conversions() {
        assert_byte_codes::<Endpoint>(|_| true);
        assert_byte_codes::<CommandSet>(|_| true);
        assert_byte_codes::<CommonCommand>(|_| true);
        assert_byte_codes::<CameraCommand>(|_| true);
        assert_byte_codes::<GimbalCommand>(|_| true);
        assert_byte_codes::<WifiCommand>(|_| true);
        assert_byte_codes::<LiveViewCommand>(|_| true);
        assert_byte_codes::<CameraWakeCommand>(|_| true);
        assert_byte_codes::<ResponseStatus>(|_| true);
        assert_byte_codes::<PacketType>(|_| true);
        assert_byte_codes::<PairingState>(|_| true);
        assert_byte_codes::<PairingApproval>(|_| true);
        assert_byte_codes::<ActivationState>(|_| true);
        assert_byte_codes::<NanoCaptureMode>(|_| true);
        assert_byte_codes::<VideoResolutionCode>(|_| true);
        assert_byte_codes::<VideoFrameRateCode>(|_| true);
        assert_byte_codes::<CameraFieldOfView>(|_| true);
        assert_byte_codes::<CameraStabilizationMode>(|_| true);
        assert_eq!(
            decode::<PairingState>(&[1], ()).unwrap(),
            PairingState::Approved
        );
        assert_eq!(
            decode::<PairingState>(&[2], ()).unwrap(),
            PairingState::AwaitingApproval
        );
        assert_eq!(
            decode::<PairingState>(&[0xe0], ()).unwrap(),
            PairingState::Unknown(0xe0)
        );
    }

    #[test]
    fn closed_enums_reject_every_unsupported_code() {
        assert_byte_codes::<MediaType>(|byte| [0x11, 0x12, 0x24].contains(&byte));
        assert_byte_codes::<AckEncoding>(|byte| byte <= 1);
        assert_byte_codes::<AvcNal>(|byte| [1, 5, 7, 8, 9].contains(&byte));
        assert_byte_codes::<HevcNal>(|byte| [32, 33, 34].contains(&byte));
        assert_byte_codes::<WifiRole>(|byte| byte <= 1);
        assert_byte_codes::<PlaybackMode>(|byte| byte <= 1);
        assert_byte_codes::<LiveViewControlAction>(|byte| [3, 4].contains(&byte));
        assert_byte_codes::<LiveViewQuality>(|byte| byte <= 2);
        assert_byte_codes::<LiveViewStreamFormat>(|byte| byte <= 4);
        assert_byte_codes::<LcdOutputFormat>(|byte| byte <= 7);
        for value in u16::MIN..=u16::MAX {
            let result = decode::<RetryPhase>(&value.to_le_bytes(), ());
            assert_eq!(result.is_ok(), value <= 3);
            if let Ok(phase) = result {
                assert_eq!(encode(&phase).unwrap(), value.to_le_bytes());
            }
        }
    }

    #[test]
    fn command_identity_uses_native_binrw_for_every_set_and_id() {
        for set in u8::MIN..=u8::MAX {
            for id in u8::MIN..=u8::MAX {
                let command = Command::from_wire(set, id);
                assert_eq!(command.wire(), (set, id));
                assert_eq!(encode(&command).unwrap(), [set, id]);
            }
        }
        assert_eq!(
            Command::from_wire(2, 0x8e),
            Command::Camera(CameraCommand::Parameters)
        );
    }

    #[derive(Debug, PartialEq, BinRead, BinWrite)]
    struct DerivedValue {
        byte: u8,
        word: u16,
    }

    #[test]
    fn native_binrw_values_compose_with_standard_cursors() {
        let value = DerivedValue {
            byte: 7,
            word: 0x1234,
        };
        let mut writer = Cursor::new(Vec::new());
        0xaau8.write_le(&mut writer).unwrap();
        value.write_le(&mut writer).unwrap();
        0xbbu8.write_le(&mut writer).unwrap();
        assert_eq!(writer.get_ref(), &[0xaa, 7, 0x34, 0x12, 0xbb]);
        writer.set_position(1);
        assert_eq!(DerivedValue::read_le(&mut writer).unwrap(), value);
        assert_eq!(u8::read_le(&mut writer).unwrap(), 0xbb);
        assert!(decode::<DerivedValue>(writer.get_ref(), ()).is_err());
    }

    #[test]
    fn context_arguments_and_domain_errors_survive_status_replies() {
        #[derive(Debug, PartialEq, BinRead)]
        #[br(little, import(expected: &u16))]
        #[br(assert(word == *expected, Error::Protocol("word does not match request")))]
        struct ContextValue {
            word: u16,
        }
        let expected = 0x1234;
        assert_eq!(
            decode::<CameraReadReply<ContextValue>>(&[0, 0x34, 0x12], (&expected,)).unwrap(),
            CameraReadReply::Accepted(ContextValue { word: expected })
        );
        assert!(matches!(
            decode::<CameraReadReply<ContextValue>>(&[0, 0x35, 0x12], (&expected,)),
            Err(Error::Protocol("word does not match request"))
        ));
        assert!(matches!(
            decode::<CameraReadReply<ContextValue>>(&[0xe3, 0xaa], (&expected,)).unwrap(),
            CameraReadReply::Rejected { .. }
        ));
    }

    #[test]
    fn attributes_override_endianness_and_codec_errors_retain_sources() {
        #[derive(BinRead, BinWrite)]
        #[brw(big)]
        struct BigEndian(u16);
        let value = decode::<BigEndian>(&[0x12, 0x34], ()).unwrap();
        assert_eq!(value.0, 0x1234);
        assert_eq!(encode(&value).unwrap(), [0x12, 0x34]);
        let error = decode::<DerivedValue>(&[7, 0x34], ()).unwrap_err();
        assert!(matches!(
            error,
            Error::WireCodec {
                operation: "decode",
                ..
            }
        ));
        assert!(error.source().is_some());
    }

    #[test]
    fn failed_encoding_returns_no_partial_payload_and_redacts_assertion_data() {
        #[derive(BinWrite)]
        struct CheckedValue {
            prefix: u8,
            #[bw(assert(*value == 0, "synthetic-sensitive-value"))]
            value: u8,
        }
        let error = encode(&CheckedValue {
            prefix: 7,
            value: 1,
        })
        .unwrap_err();
        assert!(matches!(
            error,
            Error::WireCodec {
                operation: "encode",
                ..
            }
        ));
        assert!(!error.to_string().contains("synthetic-sensitive-value"));
    }
}
