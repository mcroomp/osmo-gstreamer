//! Camera-module version query on the common command set.

use crate::{
    Error, Result,
    protocol::{
        Query, WireDecode, WireEncode, WireReader, WireWriter,
        commands::reply::{CameraRawData, CameraReadReply},
        identity::{Command, CommonCommand, Endpoint},
    },
};

/// Read camera-module hardware text and the four-byte module version.
///
/// APP -> Camera | 0x00/0x01
///
/// ```text
/// request {}
/// reply {
///     status: u8
///     unknown: u8
///     hardware: [u8; 16] | printable ASCII through the first NUL
///     unknown: [u8; 4]
///     version: [u8; 4]
///     extension: [u8; remaining]
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct CameraModuleVersionGet;

impl WireEncode for CameraModuleVersionGet {
    fn write_to(&self, _writer: &mut WireWriter) -> Result<()> {
        Ok(())
    }
}

impl Query for CameraModuleVersionGet {
    type Response = CameraReadReply<CameraModuleVersion>;

    fn receiver(&self) -> Endpoint {
        Endpoint::Camera
    }

    fn command(&self) -> Command {
        Command::Common(CommonCommand::ModuleVersion)
    }
}

/// Accepted module-version body, preserving the original fields and extension.
///
/// Camera -> APP | 0x00/0x01 | Module version reply
///
/// ```text
/// reply { status: u8 | handled by CameraReadReply
///         unknown: u8, hardware: [u8; 16] | printable ASCII through first NUL
///         unknown: [u8; 4], version: [u8; 4],
///         extension: [u8; remaining] | opaque }
/// ```
///
/// At least 25 accepted-body bytes are required. `firmware()` returns the
/// four version bytes in reverse wire order; `WireEncode` preserves wire order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CameraModuleVersion {
    hardware: String,
    firmware: [u8; 4],
    raw: CameraRawData,
}

impl CameraModuleVersion {
    pub fn hardware(&self) -> &str {
        &self.hardware
    }

    pub fn firmware(&self) -> [u8; 4] {
        self.firmware
    }
}

impl WireEncode for CameraModuleVersion {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        self.raw.write_to(writer)
    }
}

impl<Q> WireDecode<Q> for CameraModuleVersion {
    fn read_from(reader: &mut WireReader<'_>, query: &Q) -> Result<Self> {
        let raw = CameraRawData::read_from(reader, query)?;
        if raw.0.len() < 25 {
            return Err(Error::Protocol("truncated camera module version"));
        }
        let text = raw.0[1..17]
            .split(|byte| *byte == 0)
            .next()
            .ok_or(Error::Protocol("missing module hardware text"))?;
        if !text.iter().all(|byte| (32..=126).contains(byte)) {
            return Err(Error::Protocol("invalid module hardware ASCII"));
        }
        let hardware = std::str::from_utf8(text)
            .map_err(|_| Error::Protocol("invalid module hardware ASCII"))?
            .to_owned();
        let firmware = [raw.0[24], raw.0[23], raw.0[22], raw.0[21]];
        Ok(Self {
            hardware,
            firmware,
            raw,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Query, WireDecode};

    #[test]
    fn module_version_query_and_fields_round_trip() {
        let query = CameraModuleVersionGet;
        assert_eq!(query.receiver(), Endpoint::Camera);
        assert_eq!(query.command().wire(), (0, 1));
        assert!(query.encode().unwrap().is_empty());

        let mut payload = vec![0; 27];
        payload[2..8].copy_from_slice(b"CAM001");
        payload[22..26].copy_from_slice(&[4, 3, 2, 1]);
        payload[26] = 0xee;
        let reply = CameraReadReply::<CameraModuleVersion>::decode(&payload, &query).unwrap();
        let CameraReadReply::Accepted(value) = reply else {
            panic!("expected accepted module-version reply");
        };
        assert_eq!(value.hardware(), "CAM001");
        assert_eq!(value.firmware(), [1, 2, 3, 4]);
        assert_eq!(value.encode().unwrap(), payload[1..]);
    }

    #[test]
    fn invalid_module_version_bytes_are_rejected() {
        let query = CameraModuleVersionGet;
        let mut payload = vec![0; 25];
        payload[1] = 0xff;
        assert!(CameraReadReply::<CameraModuleVersion>::decode(&payload, &query).is_err());
        assert!(CameraReadReply::<CameraModuleVersion>::decode(&[0; 24], &query).is_err());
    }
}
