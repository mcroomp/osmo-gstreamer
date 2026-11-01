//! Application pairing and raw BLE pairing-arm body.

use crate::{
    Result,
    protocol::{WireDecode, WireEncode, WireReader, WireWriter, commands::reply::CameraRawData},
};
use num_enum::{FromPrimitive, IntoPrimitive};

use super::packed_string::write_packed_string;

pub const PAIRING_ID: u16 = 0x8092;
const PAIRING_ARM_PREFIX: u8 = 1;
const PAIRING_ARM_UNKNOWN: u8 = 0;
const CLIENT_NAME: &str = "osmo";

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PairingState {
    Approved = 1,
    AwaitingApproval = 2,
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
/// Strings longer than 255 UTF-8 bytes are rejected before encoding.
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
