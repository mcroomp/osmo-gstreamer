//! Pairing-approval notification.

use crate::{
    Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};
use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PairingApproval {
    Accepted00 = 0,
    Accepted01 = 1,
    #[num_enum(catch_all)]
    Unknown(u8),
}

/// Receive pairing approval, retaining unknown values and trailing bytes.
///
/// Wifi -> APP | 0x07/0x46 | Approval notification
///
/// ```text
/// notification { approval: u8, extension: [u8; remaining] | opaque }
/// reply { status: u8 = 0x00 | shared ResponseStatus acknowledgment }
/// ```
///
/// The acknowledgment travels from APP to the notification sender.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingApprovalNotification {
    pub approval: PairingApproval,
    pub extension: CameraRawData,
}

impl WireDecode for PairingApprovalNotification {
    fn read_from(reader: &mut WireReader<'_>, context: &()) -> Result<Self> {
        Ok(Self {
            approval: PairingApproval::read_from(reader, context)?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for PairingApprovalNotification {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        self.approval.write_to(writer)?;
        self.extension.write_to(writer)
    }
}
