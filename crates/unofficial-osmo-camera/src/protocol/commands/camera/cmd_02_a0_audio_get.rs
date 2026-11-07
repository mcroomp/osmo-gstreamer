//! Audio GET payload and response.

use crate::{
    Error,
    protocol::{
        Query,
        commands::reply::{CommandReadReply, RemainingBytes},
        identity::Endpoint,
    },
};
use binrw::{BinWrite, binrw};

/// Read audio parameters.
///
/// APP -> Camera | 0x02/0xA0
///
/// ```text
/// request {}
/// reply {
///     status: u8 | reject if nonzero
///     packed_prefix: le_u16 | bit meanings unspecified
///     flags: u8 | raw bit field
///     gain_code: u8 | units and range unspecified
///     zero: [0; 4]
///     masked_code: u8 | raw value with only bits 1..4 retained
///     opaque: [u8; 18]
///     extension: [u8; remaining]
/// }
/// ```
#[derive(Clone, Copy, Debug, Default, BinWrite)]
#[bw(magic = b"\x02\xa0")]
pub struct CameraAudioGet;

const AUDIO_PREFIX_LENGTH: usize = 15;
const AUDIO_FLAGS_OFFSET: usize = 2;
const AUDIO_RECORDING_GAIN_OFFSET: usize = 3;
const AUDIO_LOW_FLAGS_MASK: u8 = 0x03;
const AUDIO_AUTO_GAIN_MASK: u8 = 0x08;

/// Accepted audio data, excluding the outer status byte.
///
/// Camera -> APP | 0x02/0xA0 | Audio reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         data: [u8; 15] | compatibility decoder prefix, fields partly unmapped
///         extension: [u8; remaining] | opaque }
/// ```
///
/// The Nano data layout is 27 bytes: packed LE16, flags, gain code,
/// four zero bytes, one masked code, then 18 opaque bytes.
/// This decoder accepts the existing 15-byte prefix and preserves the
/// additional 12 bytes as extension; 15 is not the total Nano reply size.
/// A successful read does not establish that preview media carries audio.
/// Nor does a zero status guarantee that every returned field was refreshed.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\xa0")]
pub struct CameraAudioReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(raw.0.len() >= AUDIO_PREFIX_LENGTH, Error::Protocol("truncated CameraAudioReply")))]
    raw: RemainingBytes,
}

impl CameraAudioReply {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }

    pub fn data_prefix(&self) -> &[u8] {
        &self.raw_bytes()[..AUDIO_PREFIX_LENGTH]
    }

    pub fn extension(&self) -> &[u8] {
        &self.raw_bytes()[AUDIO_PREFIX_LENGTH..]
    }

    pub fn byte_2_low_bits_set(&self) -> bool {
        self.raw_bytes()[AUDIO_FLAGS_OFFSET] & AUDIO_LOW_FLAGS_MASK != 0
    }

    /// Return the raw gain code; units, range and physical effect are unspecified.
    pub fn recording_gain_code(&self) -> u8 {
        self.raw_bytes()[AUDIO_RECORDING_GAIN_OFFSET]
    }

    /// Legacy bit-3 view, not confirmation of automatic-gain behavior or support.
    pub fn auto_gain_control_bit_set(&self) -> bool {
        self.raw_bytes()[AUDIO_FLAGS_OFFSET] & AUDIO_AUTO_GAIN_MASK != 0
    }
}

impl Query for CameraAudioGet {
    type Response = CommandReadReply<CameraAudioReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket,
        commands::reply::{CommandReadReply, decode_payload},
    };

    #[test]
    fn synthetic_audio_flags_gain_and_extensions_round_trip() {
        let mut bytes = vec![0; 1 + AUDIO_PREFIX_LENGTH];
        bytes.extend_from_slice(&[0xa5, 0x5a]);
        for flags in [0, 1, 2, 4, 8, 0x80, 0xff] {
            bytes[1 + AUDIO_FLAGS_OFFSET] = flags;
            bytes[1 + AUDIO_RECORDING_GAIN_OFFSET] = flags ^ 0xff;
            let reply = decode_payload::<CommandReadReply<CameraAudioReply>>(&bytes, ()).unwrap();
            let CommandReadReply::Accepted(value) = &reply else {
                panic!("expected accepted audio");
            };
            assert_eq!(value.byte_2_low_bits_set(), flags & 3 != 0);
            assert_eq!(value.auto_gain_control_bit_set(), flags & 8 != 0);
            assert_eq!(value.recording_gain_code(), flags ^ 0xff);
            assert_eq!(value.data_prefix().len(), AUDIO_PREFIX_LENGTH);
            assert_eq!(value.extension(), [0xa5, 0x5a]);
            assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        }
        for length in 0..=AUDIO_PREFIX_LENGTH {
            assert!(
                decode_payload::<CommandReadReply<CameraAudioReply>>(&bytes[..length], ()).is_err()
            );
        }
        let rejected = b"\xdfsynthetic";
        assert_eq!(
            CommandPacket::payload(
                &decode_payload::<CommandReadReply<CameraAudioReply>>(rejected, ()).unwrap()
            )
            .unwrap(),
            rejected
        );
    }

    #[test]
    fn full_audio_body_preserves_twelve_bytes_beyond_the_compatibility_prefix() {
        let mut bytes = vec![0; 28];
        bytes[1..5].copy_from_slice(&[0x34, 0x12, 0x89, 0x7f]);
        bytes[9] = 0x1e;
        bytes[10..].copy_from_slice(&[0xa5; 18]);
        let reply = decode_payload::<CommandReadReply<CameraAudioReply>>(&bytes, ()).unwrap();
        let CommandReadReply::Accepted(value) = &reply else {
            panic!("expected accepted audio");
        };
        assert_eq!(value.recording_gain_code(), 0x7f);
        assert_eq!(value.data_prefix().len(), 15);
        assert_eq!(value.extension(), [0xa5; 12]);
        assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
    }
}
