//! Pairing-approval notification.

use crate::{
    Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};
use num_enum::{FromPrimitive, IntoPrimitive};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
/// Raw decision codes with legacy names, not an authorization result enum.
///
/// The current workflow accepts both 0 and 1. Their equivalence as approval
/// is not established by the byte contract; the names are retained for API
/// compatibility. Receipt acknowledgment is separate from the decision.
pub enum PairingApproval {
    Accepted00 = 0,
    Accepted01 = 1,
    #[num_enum(catch_all)]
    Unknown(u8),
}

/// Receive a pairing decision byte, retaining unknown values and trailing bytes.
///
/// Wifi -> APP | 0x07/0x46 | Approval notification
///
/// ```text
/// notification { decision: u8, extension: [u8; remaining] | opaque }
/// reply { status: u8 = 0x00 | shared ResponseStatus acknowledgment }
/// ```
///
/// The acknowledgment travels from APP to the notification sender and only
/// confirms receipt. There is no leading status byte in the notification.
/// Nano decision output includes codes 0/1/2/5; an optional second byte is
/// retained without assigning it authorization or readiness semantics.
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
