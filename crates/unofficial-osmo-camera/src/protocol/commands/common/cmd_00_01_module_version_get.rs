//! Camera-module version query on the common command set.

use crate::{
    Error,
    protocol::{
        Query,
        commands::reply::{CameraRawData, CommandReadReply},
        identity::Endpoint,
    },
};
use binrw::{BinWrite, binrw};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    str::from_utf8,
};

/// Read the camera-module product label and four-byte module version.
///
/// APP -> Camera | 0x00/0x01
///
/// ```text
/// request {}
/// reply {
///     status: u8
///     unknown: u8
///     product_label: [u8; 16] | printable ASCII through the first NUL
///     unknown: [u8; 4]
///     version: [u8; 4]
///     extension: [u8; remaining]
/// }
/// ```
#[derive(Clone, Copy, Debug, BinWrite, Default)]
#[bw(magic = b"\x00\x01")]
pub struct CameraModuleVersionGet;

impl Query for CameraModuleVersionGet {
    type Response = CommandReadReply<CameraModuleVersionReply>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }
}

/// Accepted module-version body, preserving the original fields and extension.
///
/// Camera -> APP | 0x00/0x01 | Module version reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         format: u8, product_label: [u8; 16] | printable ASCII through first NUL
///         unknown: [u8; 4], version: [u8; 4],
///         extension: [u8; remaining] | opaque }
/// ```
///
/// At least 25 accepted-body bytes are required. `firmware()` returns the
/// four version bytes in reverse wire order; `BinWrite` preserves wire order.
/// The Nano layout is 30 data bytes after status, with five bytes beyond this
/// decoder's prefix. The other words are not assigned hardware/bootloader
/// meanings, and no product label is hardcoded.
#[binrw]
#[derive(Clone, PartialEq, Eq)]
#[brw(magic = b"\x00\x01")]
pub struct CameraModuleVersionReply {
    #[br(temp, assert(status == 0, "accepted reply requires success status"))]
    #[bw(calc = 0u8)]
    status: u8,
    format: u8,
    #[br(assert(product_label.iter().take_while(|&&byte| byte != 0)
        .all(|byte| (b' '..=b'~').contains(byte)), Error::Protocol("invalid module hardware ASCII")))]
    product_label: [u8; 16],
    unknown: [u8; 4],
    version: [u8; 4],
    extension: CameraRawData,
}

impl CameraModuleVersionReply {
    /// Printable product label through the first NUL, without interpreting a model.
    pub fn product_label(&self) -> &str {
        let length = self
            .product_label
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(self.product_label.len());
        from_utf8(&self.product_label[..length])
            .expect("product label validated as printable ASCII")
    }

    /// Compatibility name for the product label, not a hardware revision field.
    pub fn hardware(&self) -> &str {
        self.product_label()
    }

    pub fn firmware(&self) -> [u8; 4] {
        let mut version = self.version;
        version.reverse();
        version
    }
}

impl Debug for CameraModuleVersionReply {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("CameraModuleVersionReply")
            .field("hardware", &self.product_label())
            .field("firmware", &self.firmware())
            .field("extension", &self.extension)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        CommandPacket,
        commands::reply::{CommandReadReply, decode_accepted, decode_payload},
    };

    #[test]
    fn module_version_fields_and_extension_round_trip() {
        let _query = CameraModuleVersionGet;

        let mut payload = vec![0; 27];
        payload[2..8].copy_from_slice(b"CAM001");
        payload[22..26].copy_from_slice(&[4, 3, 2, 1]);
        payload[26] = 0xee;
        let reply =
            decode_payload::<CommandReadReply<CameraModuleVersionReply>>(&payload, ()).unwrap();
        let CommandReadReply::Accepted(value) = reply else {
            panic!("expected accepted module-version reply");
        };
        assert_eq!(value.product_label(), "CAM001");
        assert_eq!(value.hardware(), "CAM001");
        assert_eq!(value.firmware(), [1, 2, 3, 4]);
        assert_eq!(CommandPacket::payload(&value).unwrap()[1..], payload[1..]);
    }

    #[test]
    fn invalid_module_version_bytes_are_rejected() {
        let _query = CameraModuleVersionGet;
        let mut payload = vec![0; 25];
        payload[1] = 0xff;
        assert!(
            decode_payload::<CommandReadReply<CameraModuleVersionReply>>(&payload, ()).is_err()
        );
        assert!(
            decode_payload::<CommandReadReply<CameraModuleVersionReply>>(&[0; 24], ()).is_err()
        );
    }

    #[test]
    fn module_version_preserves_every_field_and_only_validates_label_before_nul() {
        let mut bytes = vec![0xa5; 29];
        bytes[1..17].copy_from_slice(b"SYNTHETIC MODULE");
        bytes[21..25].copy_from_slice(&[0x12, 0x34, 0x56, 0x78]);
        let value = decode_accepted::<CameraModuleVersionReply>(&bytes, ()).unwrap();
        assert_eq!(value.product_label(), "SYNTHETIC MODULE");
        assert_eq!(value.firmware(), [0x78, 0x56, 0x34, 0x12]);
        assert_eq!(CommandPacket::payload(&value).unwrap()[1..], bytes);

        bytes[4] = 0;
        bytes[5..17].fill(0xff);
        let value = decode_accepted::<CameraModuleVersionReply>(&bytes, ()).unwrap();
        assert_eq!(value.product_label(), "SYN");
        assert_eq!(CommandPacket::payload(&value).unwrap()[1..], bytes);
        for length in 0..25 {
            assert!(decode_accepted::<CameraModuleVersionReply>(&bytes[..length], ()).is_err());
        }

        for byte in u8::MIN..=u8::MAX {
            bytes[1] = byte;
            let decoded = decode_accepted::<CameraModuleVersionReply>(&bytes, ());
            if byte == 0 || (b' '..=b'~').contains(&byte) {
                assert_eq!(
                    CommandPacket::payload(&decoded.unwrap()).unwrap()[1..],
                    bytes
                );
            } else {
                assert!(matches!(
                    decoded,
                    Err(Error::Protocol("invalid module hardware ASCII"))
                ));
            }
        }
    }
}
