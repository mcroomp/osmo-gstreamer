//! Pairing-approval notification.

use crate::protocol::commands::reply::CameraRawData;
use binrw::{BinRead, BinWrite};

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
/// Raw decision codes with legacy names, not an authorization result enum.
///
/// The current workflow accepts both 0 and 1. Their equivalence as approval
/// is not established by the byte contract; the names are retained for API
/// compatibility. Receipt acknowledgment is separate from the decision.
pub enum PairingApproval {
    #[brw(magic = 0u8)]
    Accepted00,
    #[brw(magic = 1u8)]
    Accepted01,
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
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct PairingApprovalNotification {
    pub approval: PairingApproval,
    pub extension: CameraRawData,
}
