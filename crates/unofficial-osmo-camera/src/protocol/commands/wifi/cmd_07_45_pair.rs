//! Application pairing and raw BLE pairing-arm body.

use super::packed_string::write_packed_string;
use crate::{Error, protocol::commands::reply::CameraRawData};
use binrw::{BinRead, BinWrite, binwrite};
use std::fmt::{Debug, Formatter, Result as FmtResult};

/// Pair the application identity with the camera.
///
/// APP -> Wifi | 0x07/0x45 | BLE or TCP pairing
///
/// ```text
/// request {
///     identity_length: u8
///     identity: [u8; identity_length] | UTF-8, sensitive
///     name_length: u8 = 0x04
///     name: [0x6F, 0x73, 0x6D, 0x6F] | ASCII osmo
/// }
/// reply { status: u8 | reject if nonzero
///         state: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// Nano pairing limits the identity to 32 encoded bytes; the client name is
/// the library's fixed `osmo` label, not a requirement inferred for all peers.
/// A state reply is distinct from a later decision notification or its receipt.
#[binwrite]
#[bw(assert(identity.len() <= MAX_PAIRING_IDENTITY_BYTES, Error::InvalidConfig {
    field: "pairing identity", reason: "must not exceed 32 UTF-8 bytes",
}))]
#[bw(magic = b"\x07\x45")]
pub struct PairingRequest<'a> {
    #[bw(write_with = write_packed_string)]
    pub identity: &'a str,
    #[bw(try_calc = u8::try_from(CLIENT_NAME.len()))]
    client_name_length: u8,
    #[bw(calc = CLIENT_NAME.as_bytes())]
    client_name: &'static [u8],
}

pub const PAIRING_ID: u16 = 0x8092;
const CLIENT_NAME: &str = "osmo";
const MAX_PAIRING_IDENTITY_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub enum PairingState {
    /// State 1, accepted by the current pairing workflow.
    #[brw(magic = 1u8)]
    Approved,
    /// State 2; continue waiting rather than treating it as completion.
    #[brw(magic = 2u8)]
    AwaitingApproval,
    /// Other raw states, including 5/6; never treated as approval.
    Unknown(u8),
}

impl Debug for PairingRequest<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("PairingRequest { identity: [REDACTED] }")
    }
}

/// Accepted pairing response body, inside `CameraReadReply<PairingResponse>`.
///
/// Wifi -> APP | 0x07/0x45 | Pairing reply
///
/// ```text
/// reply { status: u8 | handled by CommandReadReply
///         state: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// Unknown states are retained, not treated as approval.
#[derive(Clone, Debug, PartialEq, Eq, BinRead, BinWrite)]
pub struct PairingResponse {
    pub state: PairingState,
    pub extension: CameraRawData,
}

/// Arm pairing using the raw BLE characteristic, not a DUML command.
///
/// APP -> BLE GATT pairing characteristic 0xFFF4 | No DUML set/ID
///
/// ```text
/// characteristic_write { unknown_prefix: u8 = 0x01, unknown: u8 = 0x00 }
/// reply { bytes: [u8; remaining] | no application reply defined }
/// ```
#[derive(Clone, Copy, Debug, BinWrite)]
#[bw(magic = b"\x01\x00")]
pub struct BlePairingArmRequest;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::CommandPacket;

    #[test]
    fn pairing_identity_limit_counts_encoded_bytes() {
        for identity in ["x".repeat(32), "\u{00e9}".repeat(16)] {
            let bytes = CommandPacket::payload(&PairingRequest {
                identity: &identity,
            })
            .unwrap();
            assert_eq!(bytes[0], 32);
            assert_eq!(bytes.len(), 38);
        }
        for identity in ["x".repeat(33), "\u{00e9}".repeat(17)] {
            assert!(
                CommandPacket::payload(&PairingRequest {
                    identity: &identity
                })
                .is_err()
            );
        }
    }
}
