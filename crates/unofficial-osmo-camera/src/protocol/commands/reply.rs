//! Shared status-bearing payload types used by command replies.

use crate::{
    Error, Result,
    protocol::{ResponseStatus, WireDecode, WireEncode, WireReader, WireWriter},
};

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

impl<T: WireEncode> WireEncode for CameraReadReply<T> {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        match self {
            Self::Accepted(value) => {
                ResponseStatus::Success.write_to(writer)?;
                value.write_to(writer)
            }
            Self::Rejected { status, extension } => {
                if *status == ResponseStatus::Success {
                    return Err(Error::Protocol("rejected read cannot have success status"));
                }
                status.write_to(writer)?;
                extension.write_to(writer)
            }
        }
    }
}

impl<Q, T: WireDecode<Q>> WireDecode<Q> for CameraReadReply<T> {
    fn read_from(reader: &mut WireReader<'_>, query: &Q) -> Result<Self> {
        let status = ResponseStatus::read_from(reader, &())?;
        if status != ResponseStatus::Success {
            let length = reader.remaining().len();
            return Ok(Self::Rejected {
                status,
                extension: CameraRawData(reader.read_bytes(length)?.to_vec()),
            });
        }
        Ok(Self::Accepted(T::read_from(reader, query)?))
    }
}

/// Opaque payload bytes retained without interpreting unmapped fields.
#[derive(Clone, PartialEq, Eq)]
pub struct CameraRawData(pub Vec<u8>);

impl std::fmt::Debug for CameraRawData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraRawData")
            .field("length", &self.0.len())
            .finish()
    }
}

impl WireEncode for CameraRawData {
    fn write_to(&self, writer: &mut WireWriter) -> Result<()> {
        writer.write_bytes(&self.0);
        Ok(())
    }
}

impl<Q> WireDecode<Q> for CameraRawData {
    fn read_from(reader: &mut WireReader<'_>, _query: &Q) -> Result<Self> {
        let length = reader.remaining().len();
        Ok(Self(reader.read_bytes(length)?.to_vec()))
    }
}

macro_rules! camera_read_query {
    ($(#[$meta:meta])* $name:ident, $command:expr, $payload:expr, $response:ty) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        pub struct $name;

        impl $crate::protocol::WireEncode for $name {
            fn write_to(
                &self,
                writer: &mut $crate::protocol::WireWriter,
            ) -> $crate::Result<()> {
                writer.write_bytes($payload);
                Ok(())
            }
        }

        impl $crate::protocol::Query for $name {
            type Response = $crate::protocol::commands::reply::CameraReadReply<$response>;

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
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name {
            raw: $crate::protocol::commands::reply::CameraRawData,
        }

        impl $crate::protocol::WireEncode for $name {
            fn write_to(
                &self,
                writer: &mut $crate::protocol::WireWriter,
            ) -> $crate::Result<()> {
                self.raw.write_to(writer)
            }
        }

        impl<Q> $crate::protocol::WireDecode<Q> for $name {
            fn read_from(
                reader: &mut $crate::protocol::WireReader<'_>,
                query: &Q,
            ) -> $crate::Result<Self> {
                let raw =
                    $crate::protocol::commands::reply::CameraRawData::read_from(reader, query)?;
                if raw.0.len() < $minimum {
                    return Err($crate::Error::Protocol(concat!(
                        "truncated ",
                        stringify!($name)
                    )));
                }
                Ok(Self { raw })
            }
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

    #[test]
    fn rejected_read_keeps_status_and_opaque_extension() {
        let reply = CameraReadReply::<CameraRawData>::decode(b"\xe3\x01\x02", &()).unwrap();
        assert_eq!(
            reply,
            CameraReadReply::Rejected {
                status: ResponseStatus::Unknown(0xe3),
                extension: CameraRawData(vec![1, 2]),
            }
        );
        assert_eq!(reply.encode().unwrap(), b"\xe3\x01\x02");
    }

    #[test]
    fn rejected_read_cannot_use_success_status() {
        let reply = CameraReadReply::<CameraRawData>::Rejected {
            status: ResponseStatus::Success,
            extension: CameraRawData(vec![]),
        };
        assert!(reply.encode().is_err());
    }
}
