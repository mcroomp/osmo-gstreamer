//! Application pairing and raw BLE pairing-arm body.

use crate::{
    Error, Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};
use num_enum::{FromPrimitive, IntoPrimitive};

use super::packed_string::write_packed_string;

pub const PAIRING_ID: u16 = 0x8092;
const PAIRING_ARM_PREFIX: u8 = 1;
const PAIRING_ARM_UNKNOWN: u8 = 0;
const CLIENT_NAME: &str = "osmo";
const MAX_PAIRING_IDENTITY_BYTES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PairingState {
    /// State 1, accepted by the current pairing workflow.
    Approved = 1,
    /// State 2; continue waiting rather than treating it as completion.
    AwaitingApproval = 2,
    /// Other raw states, including 5/6; never treated as approval.
    #[num_enum(catch_all)]
    Unknown(u8),
}

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
pub struct PairingRequest<'a> {
    pub identity: &'a str,
}

impl std::fmt::Debug for PairingRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingRequest { identity: [REDACTED] }")
    }
}

impl WireEncode for PairingRequest<'_> {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        if self.identity.len() > MAX_PAIRING_IDENTITY_BYTES {
            return Err(Error::InvalidConfig {
                field: "pairing identity",
                reason: "must not exceed 32 UTF-8 bytes",
            });
        }
        write_packed_string(self.identity, writer)?;
        write_packed_string(CLIENT_NAME, writer)
    }
}

/// Accepted pairing response body, inside `CameraReadReply<PairingResponse>`.
///
/// Wifi -> APP | 0x07/0x45 | Pairing reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         state: u8, extension: [u8; remaining] | opaque }
/// ```
///
/// Unknown states are retained, not treated as approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingResponse {
    pub state: PairingState,
    pub extension: CameraRawData,
}

impl WireDecode for PairingResponse {
    fn read_from(reader: &mut WireReader<'_>, context: &()) -> Result<Self> {
        Ok(Self {
            state: PairingState::read_from(reader, context)?,
            extension: CameraRawData::read_from(reader, context)?,
        })
    }
}

impl WireEncode for PairingResponse {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        self.state.write_to(writer)?;
        self.extension.write_to(writer)
    }
}

/// Arm pairing using the raw BLE characteristic, not a DUML command.
///
/// APP -> BLE GATT pairing characteristic 0xFFF4 | No DUML set/ID
///
/// ```text
/// characteristic_write { unknown_prefix: u8 = 0x01, unknown: u8 = 0x00 }
/// reply { bytes: [u8; remaining] | no application reply defined }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct BlePairingArmRequest;

impl WireEncode for BlePairingArmRequest {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(PAIRING_ARM_PREFIX);
        writer.write_u8(PAIRING_ARM_UNKNOWN);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_identity_limit_counts_encoded_bytes() {
        for identity in ["x".repeat(32), "\u{00e9}".repeat(16)] {
            let bytes = PairingRequest {
                identity: &identity,
            }
            .encode()
            .unwrap();
            assert_eq!(bytes[0], 32);
            assert_eq!(bytes.len(), 38);
        }
        for identity in ["x".repeat(33), "\u{00e9}".repeat(17)] {
            assert!(
                PairingRequest {
                    identity: &identity
                }
                .encode()
                .is_err()
            );
        }
    }
}
