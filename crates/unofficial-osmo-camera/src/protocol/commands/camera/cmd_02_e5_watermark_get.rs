//! Watermark GET payload.

use crate::{
    Error,
    protocol::{
        Query,
        commands::reply::{CommandReadReply, RemainingBytes},
        identity::Endpoint,
        u16_at,
    },
};
use binrw::{binrw, binwrite};

/// Read model-specific watermark settings, not feature availability.
///
/// APP -> Camera | 0x02/0xE5
///
/// ```text
/// request { format_and_operation: u8 = 0x20, trailing: [0; 6] }
/// reply { status: u8 | reject if nonzero
///         flags: le_u16, length: le_u16, opaque: u8
///         value: [u8; length], trailing: [u8; remaining] }
/// ```
///
/// The low request nibble selects GET; high nibble 2 selects this reply
/// format. A zero-length value still requires five data bytes after status.
/// This does not confirm overlay placement, text encoding or mode support.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\xe5")]
pub struct CameraWatermarkGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

const WATERMARK_HEADER_LENGTH: usize = 5;
const WATERMARK_PHOTOS_MASK: u8 = 0x01;
const WATERMARK_VIDEOS_MASK: u8 = 0x02;
const WATERMARK_LIVE_VIEW_MASK: u8 = 0x04;

/// Accepted format-2 watermark layout after a successful status.
///
/// Sequential data: `flags: le_u16, length: le_u16, opaque: u8,
/// value: [u8; length]`, then any retained trailing bytes.
/// Neither the value's encoding nor individual flag meanings are established.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\xe5")]
pub struct CameraWatermarkReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(raw.as_slice().len() >= WATERMARK_HEADER_LENGTH + usize::from(u16_at(raw.as_slice(), 2)), Error::Protocol("truncated watermark metadata value")))]
    raw: RemainingBytes<WATERMARK_HEADER_LENGTH>,
}

impl CameraWatermarkReply {
    pub fn raw_bytes(&self) -> &[u8] {
        self.raw.as_slice()
    }

    /// Return the low flag byte for compatibility; use `raw_flags_word` for all bits.
    pub fn raw_flags(&self) -> u8 {
        self.raw_bytes()[0]
    }

    pub fn raw_flags_word(&self) -> u16 {
        u16_at(self.raw.as_slice(), 0)
    }

    pub fn value_length(&self) -> u16 {
        u16_at(self.raw.as_slice(), 2)
    }

    pub fn opaque_byte(&self) -> u8 {
        self.raw.as_slice()[4]
    }

    /// Return uninterpreted value bytes, without assuming text or a terminator.
    pub fn value(&self) -> &[u8] {
        &self.raw.as_slice()[WATERMARK_HEADER_LENGTH..self.value_end()]
    }

    pub fn trailing_bytes(&self) -> &[u8] {
        &self.raw.as_slice()[self.value_end()..]
    }

    fn value_end(&self) -> usize {
        WATERMARK_HEADER_LENGTH + usize::from(self.value_length())
    }

    /// Legacy bit-0 view; this is not confirmation of photo rendering behavior.
    pub fn enabled_for_photos(&self) -> bool {
        self.raw_flags() & WATERMARK_PHOTOS_MASK != 0
    }

    /// Legacy bit-1 view; this is not confirmation of video rendering behavior.
    pub fn enabled_for_videos(&self) -> bool {
        self.raw_flags() & WATERMARK_VIDEOS_MASK != 0
    }

    /// Legacy bit-2 view; this is not confirmation of preview rendering behavior.
    pub fn enabled_for_live_view(&self) -> bool {
        self.raw_flags() & WATERMARK_LIVE_VIEW_MASK != 0
    }

    /// Compatibility tail after the low flag byte, including the rest of the header.
    pub fn extension(&self) -> &[u8] {
        &self.raw_bytes()[1..]
    }
}

impl Query for CameraWatermarkGet {
    type Response = CommandReadReply<CameraWatermarkReply>;
    const REQUEST_PAYLOAD: &'static [u8] = &[0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];

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
    fn synthetic_watermark_flags_and_tail_round_trip() {
        let _query = CameraWatermarkGet::default();
        for flags in [0, 1, 2, 4, 0x80, 0xff] {
            let bytes = [0, flags, 0xa5, 2, 0, 0x5a, 0x12, 0x34, 0xcc];
            let reply =
                decode_payload::<CommandReadReply<CameraWatermarkReply>>(&bytes, ()).unwrap();
            let CommandReadReply::Accepted(value) = &reply else {
                panic!("expected accepted watermark");
            };
            assert_eq!(value.raw_flags(), flags);
            assert_eq!(value.enabled_for_photos(), flags & 1 != 0);
            assert_eq!(value.enabled_for_videos(), flags & 2 != 0);
            assert_eq!(value.enabled_for_live_view(), flags & 4 != 0);
            assert_eq!(value.raw_flags_word(), u16::from_le_bytes([flags, 0xa5]));
            assert_eq!(value.value_length(), 2);
            assert_eq!(value.opaque_byte(), 0x5a);
            assert_eq!(value.value(), [0x12, 0x34]);
            assert_eq!(value.trailing_bytes(), [0xcc]);
            assert_eq!(value.extension(), &bytes[2..]);
            assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
        }
        for bytes in [
            &[][..],
            &[0][..],
            &[0, 1, 0, 0][..],
            &[0, 1, 0, 0, 0][..],
            &[0, 1, 0, 1, 0, 0][..],
            &[0, 1, 0, 0xff, 0xff, 0][..],
        ] {
            assert!(decode_payload::<CommandReadReply<CameraWatermarkReply>>(bytes, ()).is_err());
        }
        for bytes in [
            &[0, 0x80, 0, 0, 0, 0][..],
            &[0xdf][..],
            &[0xdf, 0xa5, 0x5a][..],
        ] {
            assert_eq!(
                CommandPacket::payload(
                    &decode_payload::<CommandReadReply<CameraWatermarkReply>>(bytes, ()).unwrap()
                )
                .unwrap(),
                bytes
            );
        }
    }
}
