use crate::{Error, Result};

#[derive(Default)]
pub struct WireWriter {
    bytes: Vec<u8>,
}

impl WireWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn write_u16_le(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_u32_le(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

pub struct WireReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> WireReader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_bytes(1)?[0])
    }

    pub fn read_u16_le(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32_le(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_bytes(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(Error::Protocol("wire length overflow"))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or(Error::Protocol("truncated wire value"))?;
        self.position = end;
        Ok(bytes)
    }

    pub fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.position..]
    }

    pub fn finish(self) -> Result<()> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Protocol("trailing bytes after wire value"))
        }
    }
}

/// Encodes a protocol value into a composable wire writer.
pub trait WireEncode {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()>;

    fn encode(&self) -> Result<Vec<u8>> {
        let mut writer = WireWriter::new();
        self.write_to(&mut writer)?;
        Ok(writer.into_inner())
    }
}

/// Decodes a protocol value from a composable wire reader.
///
/// `Context` supports payloads whose layout depends on the typed request.
pub trait WireDecode<Context = ()>: Sized {
    fn read_from(reader: &mut WireReader<'_>, context: &Context) -> Result<Self>;

    fn decode(bytes: &[u8], context: &Context) -> Result<Self> {
        let mut reader = WireReader::new(bytes);
        let value = Self::read_from(&mut reader, context)?;
        reader.finish()?;
        Ok(value)
    }
}

/// Converts a protocol value represented by one wire byte.
///
/// Implemented automatically for copyable types with standard byte conversions
/// whose decoding errors convert into the library's `Error`.
pub trait WireU8: Sized {
    fn from_wire_u8(value: u8) -> Result<Self>;
    fn to_wire_u8(&self) -> u8;
}

impl<T> WireU8 for T
where
    T: Copy + Into<u8> + TryFrom<u8>,
    Error: From<T::Error>,
{
    fn from_wire_u8(value: u8) -> Result<Self> {
        Self::try_from(value).map_err(Error::from)
    }

    fn to_wire_u8(&self) -> u8 {
        (*self).into()
    }
}

impl<T: WireU8> WireEncode for T {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_u8(self.to_wire_u8());
        Ok(())
    }
}

impl<T: WireU8> WireDecode for T {
    fn read_from(reader: &mut WireReader<'_>, _context: &()) -> Result<Self> {
        Self::from_wire_u8(reader.read_u8()?)
    }
}
