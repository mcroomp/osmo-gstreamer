use crate::{
    Error, Result,
    protocol::{WireReader, WireWriter},
};

pub(super) fn write_packed_string(value: &str, writer: &mut WireWriter) -> Result<()> {
    let length = u8::try_from(value.len())
        .map_err(|_| Error::LimitExceeded("packed string exceeds 255 UTF-8 bytes"))?;
    writer.write_u8(length);
    writer.write_bytes(value.as_bytes());
    Ok(())
}

pub(super) fn read_packed_string(reader: &mut WireReader<'_>) -> Result<String> {
    let length = usize::from(reader.read_u8()?);
    String::from_utf8(reader.read_bytes(length)?.to_vec())
        .map_err(|_| Error::Protocol("Wi-Fi string is not UTF-8"))
}
