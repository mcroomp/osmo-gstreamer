//! Storage-location GET payload and response.

use crate::{
    Error,
    protocol::{
        Query,
        commands::reply::{CommandReadReply, RemainingBytes},
        identity::Endpoint,
    },
};
use binrw::{binrw, binwrite};

/// Read the storage-location code.
///
/// APP -> Camera | 0x02/0xDA
///
/// ```text
/// request { operation: u8 = 0, trailing: [0; 6] }
/// reply { status: u8, zero_prefix: [0; 3], count: u8, zero: u8,
///         entries: [u8; count], extension: [u8; remaining] }
/// ```
///
/// A bare zero-status ACK is not sufficient for this typed data reply.
#[binwrite]
#[derive(Clone, Copy, Debug, Default)]
#[bw(magic = b"\x02\xda")]
pub struct CameraStorageLocationGet {
    #[bw(calc = <Self as Query>::REQUEST_PAYLOAD)]
    payload: &'static [u8],
}

/// Accepted Nano storage list after a successful status.
///
/// Sequential data: `zero[3], count: u8, zero: u8, entries: [u8; count]`.
/// Count is 1..=4. The selected storage is always the first entry; available
/// alternatives follow. Physical device names for the codes are unspecified.
/// Received header bytes and trailing data are preserved, not normalized.
#[binrw]
#[derive(Clone, Debug, PartialEq, Eq)]
#[brw(magic = b"\x02\xda")]
pub struct CameraStorageLocationReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    #[br(assert(raw.0.len() >= 6 && (1..=4).contains(&raw.0[3]) && raw.0.len() >= 5 + usize::from(raw.0[3]), Error::Protocol("invalid camera storage list")))]
    raw: RemainingBytes,
}

impl CameraStorageLocationReply {
    pub fn raw_bytes(&self) -> &[u8] {
        &self.raw.0
    }

    /// Return the selected entry's low two bits, not an arbitrary list member.
    pub fn code(&self) -> u8 {
        self.raw_bytes()[5] & 3
    }

    pub fn entry_count(&self) -> u8 {
        self.raw.0[3]
    }

    /// Return selected storage first, retaining all bits of every entry.
    pub fn entries(&self) -> &[u8] {
        &self.raw.0[5..5 + usize::from(self.entry_count())]
    }

    pub fn extension(&self) -> &[u8] {
        &self.raw.0[5 + usize::from(self.entry_count())..]
    }
}

impl Query for CameraStorageLocationGet {
    type Response = CommandReadReply<CameraStorageLocationReply>;
    const REQUEST_PAYLOAD: &'static [u8] = &[0; 7];

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket, Flags, Message, Query,
        commands::reply::{CommandReadReply, decode_payload},
        encode,
        identity::Endpoint,
    };
    use binrw::meta::{ReadMagic, WriteMagic};

    #[test]
    fn get_and_reply_share_only_command_magic() {
        assert_eq!(CameraStorageLocationGet::COMMAND_ID, [2, 0xda]);
        assert_eq!(<CameraStorageLocationReply as ReadMagic>::MAGIC, [2, 0xda]);
        assert_eq!(<CameraStorageLocationReply as WriteMagic>::MAGIC, [2, 0xda]);
        let mut message = Message::request(
            Endpoint::App,
            1,
            CameraStorageLocationGet::default().command(),
            &[0, 0, 0, 0, 1, 0, 3, 0xa5],
        );
        message.flags = Flags::RESPONSE;
        let reply = CameraStorageLocationGet::default()
            .decode_reply(&message)
            .unwrap();
        let CommandReadReply::Accepted(data) = &reply else {
            panic!("expected accepted storage data");
        };
        assert_eq!(data.code(), 3);
        assert_eq!(data.extension(), [0xa5]);
        assert_eq!(
            encode(&reply).unwrap(),
            [2, 0xda, 0, 0, 0, 0, 1, 0, 3, 0xa5]
        );
        assert_eq!(reply.payload().unwrap(), message.payload);
    }

    #[test]
    fn selected_storage_is_first_and_count_bounds_the_list() {
        for count in 1..=4u8 {
            for selected in 0..4u8 {
                let mut bytes = vec![0, 0, 0, 0, count, 0, selected];
                bytes.extend(
                    (0..4u8)
                        .filter(|code| *code != selected)
                        .take(usize::from(count - 1)),
                );
                let reply =
                    decode_payload::<CommandReadReply<CameraStorageLocationReply>>(&bytes, ())
                        .unwrap();
                let CommandReadReply::Accepted(value) = &reply else {
                    panic!("expected accepted storage list");
                };
                assert_eq!(value.entry_count(), count);
                assert_eq!(value.code(), selected);
                assert_eq!(value.entries(), &bytes[6..]);
                assert!(value.extension().is_empty());
                assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
                for end in 0..bytes.len() {
                    assert!(
                        decode_payload::<CommandReadReply<CameraStorageLocationReply>>(
                            &bytes[..end],
                            ()
                        )
                        .is_err()
                    );
                }
            }
        }
        for count in [0, 5, 255] {
            let bytes = [0, 0, 0, 0, count, 0, 1];
            assert!(
                decode_payload::<CommandReadReply<CameraStorageLocationReply>>(&bytes, ()).is_err()
            );
        }
        for bytes in [
            &[0xe3][..],
            &[0xe3, 0xa5][..],
            &[0, 0, 0, 0, 1, 0, 0xff, 0xa5][..],
        ] {
            let reply =
                decode_payload::<CommandReadReply<CameraStorageLocationReply>>(bytes, ()).unwrap();
            assert_eq!(CommandPacket::payload(&reply).unwrap(), bytes);
            if let CommandReadReply::Accepted(value) = reply {
                assert_eq!(value.entries(), [0xff]);
                assert_eq!(value.extension(), [0xa5]);
            }
        }
    }
}
