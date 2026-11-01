use super::{
    WireDecode, WireEncode,
    identity::{Command, Endpoint},
};
use std::time::Duration;

const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Routing metadata and typed payload codecs for an implemented read request.
pub trait Query: WireEncode + Sized {
    type Response: WireDecode<Self>;

    fn receiver(&self) -> Endpoint;
    fn command(&self) -> Command;

    fn timeout(&self) -> Duration {
        DEFAULT_QUERY_TIMEOUT
    }
}
