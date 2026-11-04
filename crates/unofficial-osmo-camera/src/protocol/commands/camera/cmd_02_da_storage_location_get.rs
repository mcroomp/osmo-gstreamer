//! Storage-location GET payload and response.

use crate::{
    Error, Result,
    protocol::{
        WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::{CameraRawData, camera_read_query},
        identity::{CameraCommand, Command},
    },
};

/// Accepted Nano storage list, excluding outer status.
///
/// Sequential data: `zero[3], count: u8, zero: u8, entries: [u8; count]`.
/// Count is 1..=4. The selected storage is always the first entry; available
/// alternatives follow. Physical device names for the codes are unspecified.
/// Received header bytes and trailing data are preserved, not normalized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraStorageData {
    raw: CameraRawData,
}

impl CameraStorageData {
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

impl<Q> WireDecode<Q> for CameraStorageData {
    fn read_from(reader: &mut WireReader<'_>, query: &Q) -> Result<Self> {
        let raw = CameraRawData::read_from(reader, query)?;
        if raw.0.len() < 6
            || !(1..=4).contains(&raw.0[3])
            || raw.0.len() < 5 + usize::from(raw.0[3])
        {
            return Err(Error::Protocol("invalid camera storage list"));
        }
        Ok(Self { raw })
    }
}

impl WireEncode for CameraStorageData {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        self.raw.write_to(writer)
    }
}

camera_read_query!(
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
    CameraStorageLocationGet,
    Command::Camera(CameraCommand::StorageLocation),
    &[0; 7],
    CameraStorageData
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::commands::reply::CameraReadReply;

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
                let reply = CameraReadReply::<CameraStorageData>::decode(&bytes, &()).unwrap();
                let CameraReadReply::Accepted(value) = &reply else {
                    panic!("expected accepted storage list");
                };
                assert_eq!(value.entry_count(), count);
                assert_eq!(value.code(), selected);
                assert_eq!(value.entries(), &bytes[6..]);
                assert!(value.extension().is_empty());
                assert_eq!(reply.encode().unwrap(), bytes);
                for end in 0..bytes.len() {
                    assert!(
                        CameraReadReply::<CameraStorageData>::decode(&bytes[..end], &()).is_err()
                    );
                }
            }
        }
        for count in [0, 5, 255] {
            let bytes = [0, 0, 0, 0, count, 0, 1];
            assert!(CameraReadReply::<CameraStorageData>::decode(&bytes, &()).is_err());
        }
        for bytes in [
            &[0xe3][..],
            &[0xe3, 0xa5][..],
            &[0, 0, 0, 0, 1, 0, 0xff, 0xa5][..],
        ] {
            let reply = CameraReadReply::<CameraStorageData>::decode(bytes, &()).unwrap();
            assert_eq!(reply.encode().unwrap(), bytes);
            if let CameraReadReply::Accepted(value) = reply {
                assert_eq!(value.entries(), [0xff]);
                assert_eq!(value.extension(), [0xa5]);
            }
        }
    }
}
