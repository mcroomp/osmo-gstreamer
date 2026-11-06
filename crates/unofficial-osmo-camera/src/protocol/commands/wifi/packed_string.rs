use crate::{Error, protocol::binrw_error};
use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::io::{Read, Seek, Write};

pub(super) fn write_packed_string<T: AsRef<str>, W: Write + Seek>(
    value: &T,
    writer: &mut W,
    endian: Endian,
    (): (),
) -> BinResult<()> {
    let value = value.as_ref();
    let pos = writer.stream_position()?;
    let length = u8::try_from(value.len()).map_err(|_| {
        binrw_error(
            pos,
            Error::LimitExceeded("packed string exceeds 255 UTF-8 bytes"),
        )
    })?;
    length.write_options(writer, endian, ())?;
    writer.write_all(value.as_bytes())?;
    Ok(())
}

pub(super) fn read_packed_string<R: Read + Seek>(
    reader: &mut R,
    endian: Endian,
    (): (),
) -> BinResult<String> {
    let pos = reader.stream_position()?;
    let length = usize::from(u8::read_options(reader, endian, ())?);
    let bytes = Vec::<u8>::read_options(
        reader,
        endian,
        binrw::VecArgs {
            count: length,
            inner: (),
        },
    )?;
    String::from_utf8(bytes)
        .map_err(|_| binrw_error(pos, Error::Protocol("Wi-Fi string is not UTF-8")))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    #[test]
    fn oversized_string_reports_its_actual_offset_without_writing() {
        let mut writer = Cursor::new(vec![0xa5; 4]);
        writer.set_position(3);
        let error =
            write_packed_string(&"x".repeat(256), &mut writer, Endian::Little, ()).unwrap_err();
        let binrw::Error::Custom { pos, err } = error else {
            panic!("expected a custom length error");
        };
        assert_eq!(pos, 3);
        assert!(matches!(
            err.downcast_ref::<Error>(),
            Some(Error::LimitExceeded(
                "packed string exceeds 255 UTF-8 bytes"
            ))
        ));
        assert_eq!(writer.position(), 3);
        assert_eq!(writer.into_inner(), [0xa5; 4]);
    }
}
