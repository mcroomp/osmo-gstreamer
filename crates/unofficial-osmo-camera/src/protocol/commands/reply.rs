//! Shared status-bearing payload types used by command replies.

#[cfg(test)]
use crate::protocol::decode;
use crate::{
    Error, Result,
    protocol::{ResponseStatus, binrw_error},
};
use binrw::{
    BinRead, BinResult, BinWrite, Endian,
    helpers::until_eof,
    meta::{ReadMagic, WriteMagic},
};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    io::{Read, Seek, SeekFrom, Write},
};

/// Command-qualified read outcome with opaque rejection bytes by default.
///
/// `T` owns the command magic and the successful status field. Rejection data
/// uses `R` only when the command has a meaningful, established rejection layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandReadReply<T, R = RemainingBytes> {
    Accepted(T),
    Rejected {
        status: ResponseStatus,
        extension: R,
    },
}

impl<T: ReadMagic<MagicType = [u8; 2]>, R> ReadMagic for CommandReadReply<T, R> {
    type MagicType = [u8; 2];
    const MAGIC: Self::MagicType = T::MAGIC;
}

impl<T: WriteMagic<MagicType = [u8; 2]>, R> WriteMagic for CommandReadReply<T, R> {
    type MagicType = [u8; 2];
    const MAGIC: Self::MagicType = T::MAGIC;
}

impl<T, R> BinRead for CommandReadReply<T, R>
where
    T: for<'a> BinRead<Args<'a> = ()> + ReadMagic<MagicType = [u8; 2]>,
    R: for<'a> BinRead<Args<'a> = ()>,
{
    type Args<'a> = ();

    fn read_options<S: Read + Seek>(reader: &mut S, endian: Endian, (): ()) -> BinResult<Self> {
        let start = reader.stream_position()?;
        let command = <[u8; 2]>::read_options(reader, endian, ())?;
        if command != T::MAGIC {
            return Err(binrw_error(
                start,
                Error::Protocol("typed reply command mismatch"),
            ));
        }
        let status = ResponseStatus::read_options(reader, endian, ())?;
        if status == ResponseStatus::Success {
            reader.seek(SeekFrom::Start(start))?;
            Ok(Self::Accepted(T::read_options(reader, endian, ())?))
        } else {
            Ok(Self::Rejected {
                status,
                extension: R::read_options(reader, endian, ())?,
            })
        }
    }
}

impl<T, R> BinWrite for CommandReadReply<T, R>
where
    T: for<'a> BinWrite<Args<'a> = ()> + WriteMagic<MagicType = [u8; 2]>,
    R: for<'a> BinWrite<Args<'a> = ()>,
{
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        (): (),
    ) -> BinResult<()> {
        match self {
            Self::Accepted(value) => value.write_options(writer, endian, ()),
            Self::Rejected { status, extension } => {
                if *status == ResponseStatus::Success {
                    return Err(binrw_error(
                        writer.stream_position()?,
                        Error::Protocol("rejected read cannot have success status"),
                    ));
                }
                T::MAGIC.write_options(writer, endian, ())?;
                status.write_options(writer, endian, ())?;
                extension.write_options(writer, endian, ())
            }
        }
    }
}

/// A reply with a leading status byte and a preserved rejection extension.
///
/// ```text
/// reply {
///     status: u8
///     accepted_data: T       | present when status == 0
///     rejection_extension: [u8; remaining] | present when status != 0
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CameraReadReply<T> {
    Accepted(T),
    Rejected {
        status: ResponseStatus,
        extension: RemainingBytes,
    },
}

impl<T> BinWrite for CameraReadReply<T>
where
    for<'a> T: BinWrite<Args<'a> = ()>,
{
    type Args<'a> = ();

    fn write_options<W: Write + Seek>(
        &self,
        writer: &mut W,
        endian: Endian,
        (): (),
    ) -> BinResult<()> {
        match self {
            Self::Accepted(value) => {
                ResponseStatus::Success.write_options(writer, endian, ())?;
                value.write_options(writer, endian, ())
            }
            Self::Rejected { status, extension } => {
                if *status == ResponseStatus::Success {
                    return Err(binrw_error(
                        writer.stream_position()?,
                        Error::Protocol("rejected read cannot have success status"),
                    ));
                }
                status.write_options(writer, endian, ())?;
                extension.write_options(writer, endian, ())
            }
        }
    }
}

impl<T: BinRead> BinRead for CameraReadReply<T> {
    type Args<'a> = T::Args<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: Endian,
        args: Self::Args<'_>,
    ) -> BinResult<Self> {
        let status = ResponseStatus::read_options(reader, endian, ())?;
        if status != ResponseStatus::Success {
            return Ok(Self::Rejected {
                status,
                extension: RemainingBytes::read_options(reader, endian, ())?,
            });
        }
        Ok(Self::Accepted(T::read_options(reader, endian, args)?))
    }
}

/// Opaque bytes read through EOF of a bounded payload reader.
///
/// Construction, decoding and encoding require at least `MIN` bytes.
/// The default minimum is zero, so optional tails and rejections may be empty.
/// Retains unmapped fields without interpretation; not for live transport readers.
#[derive(Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct RemainingBytes<const MIN: usize = 0> {
    #[br(parse_with = until_eof, assert(bytes.len() >= MIN, Error::Protocol("remaining bytes are shorter than the required minimum")))]
    #[bw(assert(bytes.len() >= MIN, Error::Protocol("remaining bytes are shorter than the required minimum")))]
    bytes: Vec<u8>,
}

impl<const MIN: usize> RemainingBytes<MIN> {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < MIN {
            return Err(Error::Protocol(
                "remaining bytes are shorter than the required minimum",
            ));
        }
        Ok(Self { bytes })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
pub(crate) fn decode_payload<T>(bytes: &[u8], (): ()) -> Result<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + ReadMagic<MagicType = [u8; 2]>,
{
    let mut body = T::MAGIC.to_vec();
    body.extend_from_slice(bytes);
    decode(&body, ())
}

#[cfg(test)]
pub(crate) fn decode_accepted<T>(bytes: &[u8], (): ()) -> Result<T>
where
    T: for<'a> BinRead<Args<'a> = ()> + ReadMagic<MagicType = [u8; 2]>,
{
    let mut payload = vec![0];
    payload.extend_from_slice(bytes);
    decode_payload(&payload, ())
}

impl<const MIN: usize> Debug for RemainingBytes<MIN> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("RemainingBytes")
            .field("length", &self.bytes.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};
    use binrw::binrw;
    use std::io::Cursor;

    #[test]
    fn remaining_bytes_enforce_each_minimum_at_every_boundary() {
        fn check<const MIN: usize>() {
            for length in [MIN, MIN + 1, MIN + 3] {
                let bytes = vec![0xaa; length];
                let value = RemainingBytes::<MIN>::new(bytes.clone()).unwrap();
                assert_eq!(value.as_slice(), bytes);
                assert_eq!(value.clone().into_vec(), bytes);
                assert_eq!(encode(&value).unwrap(), bytes);
                assert_eq!(decode::<RemainingBytes<MIN>>(&bytes, ()).unwrap(), value);
                assert_eq!(
                    format!("{value:?}"),
                    format!("RemainingBytes {{ length: {} }}", bytes.len())
                );
                let mut framed = vec![0xa5; 3];
                framed.extend_from_slice(&bytes);
                let mut reader = Cursor::new(&framed);
                reader.set_position(3);
                let decoded =
                    RemainingBytes::<MIN>::read_options(&mut reader, Endian::Big, ()).unwrap();
                assert_eq!(decoded, value);
                assert_eq!(reader.position(), framed.len() as u64);
                let mut writer = Cursor::new(vec![0xa5; 3]);
                writer.set_position(3);
                decoded.write_options(&mut writer, Endian::Big, ()).unwrap();
                assert_eq!(writer.into_inner(), framed);
            }
            for length in 0..MIN {
                let bytes = vec![0xaa; length];
                assert!(RemainingBytes::<MIN>::new(bytes.clone()).is_err());
                assert!(decode::<RemainingBytes<MIN>>(&bytes, ()).is_err());
                let invalid = RemainingBytes::<MIN> { bytes };
                assert!(encode(&invalid).is_err());
                assert!(
                    invalid
                        .write_options(&mut Cursor::new(Vec::new()), Endian::Big, ())
                        .is_err()
                );
            }
        }
        check::<0>();
        check::<1>();
        check::<2>();
        check::<6>();
        check::<15>();
    }

    #[binrw]
    #[derive(Clone, Debug, PartialEq, Eq)]
    #[brw(magic = b"\x02\xda")]
    struct SyntheticReply {
        #[br(temp, assert(status == 0))]
        #[bw(calc = 0u8)]
        status: u8,
        value: u8,
    }

    #[test]
    fn command_outcomes_keep_magic_status_and_rejection_data_separate() {
        let accepted = CommandReadReply::<SyntheticReply>::Accepted(SyntheticReply { value: 7 });
        assert_eq!(encode(&accepted).unwrap(), [2, 0xda, 0, 7]);
        assert_eq!(
            decode::<CommandReadReply<SyntheticReply>>(&[2, 0xda, 0, 7], ()).unwrap(),
            accepted
        );
        let rejected = CommandReadReply::<SyntheticReply>::Rejected {
            status: ResponseStatus::Unknown(0xe3),
            extension: RemainingBytes::<0>::new(vec![0xaa, 0xbb]).unwrap(),
        };
        assert_eq!(encode(&rejected).unwrap(), [2, 0xda, 0xe3, 0xaa, 0xbb]);
        assert_eq!(
            decode::<CommandReadReply<SyntheticReply>>(&[2, 0xda, 0xe3, 0xaa, 0xbb], ()).unwrap(),
            rejected
        );
        let typed = CommandReadReply::<SyntheticReply, u8>::Rejected {
            status: ResponseStatus::Unknown(0xe3),
            extension: 5,
        };
        assert_eq!(
            decode::<CommandReadReply<SyntheticReply, u8>>(&encode(&typed).unwrap(), ()).unwrap(),
            typed
        );
        for bytes in [&[2, 0xda][..], &[2, 0xda, 0][..], &[2, 0xdb, 0xe3][..]] {
            assert!(decode::<CommandReadReply<SyntheticReply>>(bytes, ()).is_err());
        }
        assert!(
            encode(&CommandReadReply::<SyntheticReply>::Rejected {
                status: ResponseStatus::Success,
                extension: RemainingBytes::<0>::new(vec![]).unwrap(),
            })
            .is_err()
        );
        let mut reader = Cursor::new(vec![0xa5, 2, 0xda, 0, 7]);
        reader.set_position(1);
        assert_eq!(
            CommandReadReply::<SyntheticReply>::read_options(&mut reader, Endian::Little, ())
                .unwrap(),
            accepted
        );
        assert_eq!(reader.position(), 5);
    }

    #[test]
    fn rejected_read_keeps_status_and_opaque_extension() {
        let reply = decode::<CameraReadReply<RemainingBytes>>(b"\xe3\x01\x02", ()).unwrap();
        assert_eq!(
            reply,
            CameraReadReply::Rejected {
                status: ResponseStatus::Unknown(0xe3),
                extension: RemainingBytes::<0>::new(vec![1, 2]).unwrap(),
            }
        );
        assert_eq!(encode(&reply).unwrap(), b"\xe3\x01\x02");
    }

    #[test]
    fn rejected_read_cannot_use_success_status() {
        let reply = CameraReadReply::<RemainingBytes>::Rejected {
            status: ResponseStatus::Success,
            extension: RemainingBytes::<0>::new(vec![]).unwrap(),
        };
        assert!(encode(&reply).is_err());
    }
}
