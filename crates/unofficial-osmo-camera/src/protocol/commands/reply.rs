//! Shared status-bearing payload types used by command replies.

use crate::{
    Error,
    protocol::{ResponseStatus, binrw_error},
};
use binrw::{BinRead, BinResult, BinWrite, Endian};
use std::io::{Read, Seek, Write};

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
        extension: CameraRawData,
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
                extension: CameraRawData::read_options(reader, endian, ())?,
            });
        }
        Ok(Self::Accepted(T::read_options(reader, endian, args)?))
    }
}

/// Opaque payload bytes retained without interpreting unmapped fields.
#[derive(Clone, PartialEq, Eq, BinRead, BinWrite)]
pub struct CameraRawData(#[br(parse_with = binrw::helpers::until_eof)] pub Vec<u8>);

impl std::fmt::Debug for CameraRawData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraRawData")
            .field("length", &self.0.len())
            .finish()
    }
}

macro_rules! camera_read_query {
    ($(#[$meta:meta])* $name:ident, $command:expr, $payload:expr, $response:ty) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, binrw::BinWrite)]
        #[bw(map = |_: &Self| -> &[u8] { $payload })]
        pub struct $name;

        impl $crate::protocol::Query for $name {
            type Response = $crate::protocol::commands::reply::CameraReadReply<$response>;

            fn response_args(&self) {}

            fn receiver(&self) -> $crate::protocol::identity::Endpoint {
                $crate::protocol::identity::Endpoint::Camera
            }

            fn command(&self) -> $crate::protocol::identity::Command {
                $command
            }
        }
    };
}
pub(crate) use camera_read_query;

macro_rules! camera_data {
    ($(#[$meta:meta])* $name:ident, $minimum:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, binrw::BinRead, binrw::BinWrite)]
        #[br(try_map = |raw: $crate::protocol::commands::reply::CameraRawData| {
            if raw.0.len() < $minimum {
                Err($crate::Error::Protocol(concat!("truncated ", stringify!($name))))
            } else {
                Ok(Self { raw })
            }
        })]
        pub struct $name {
            raw: $crate::protocol::commands::reply::CameraRawData,
        }

        impl $name {
            pub fn raw_bytes(&self) -> &[u8] {
                &self.raw.0
            }
        }
    };
}
pub(crate) use camera_data;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn rejected_read_keeps_status_and_opaque_extension() {
        let reply = decode::<CameraReadReply<CameraRawData>>(b"\xe3\x01\x02", ()).unwrap();
        assert_eq!(
            reply,
            CameraReadReply::Rejected {
                status: ResponseStatus::Unknown(0xe3),
                extension: CameraRawData(vec![1, 2]),
            }
        );
        assert_eq!(encode(&reply).unwrap(), b"\xe3\x01\x02");
    }

    #[test]
    fn rejected_read_cannot_use_success_status() {
        let reply = CameraReadReply::<CameraRawData>::Rejected {
            status: ResponseStatus::Success,
            extension: CameraRawData(vec![]),
        };
        assert!(encode(&reply).is_err());
    }
}
