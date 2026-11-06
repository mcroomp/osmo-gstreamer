use super::identity::{Command, Endpoint};
use binrw::{BinRead, BinWrite};
use std::time::Duration;

const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(10);

/// Routing metadata and typed payload codecs for an implemented read request.
pub trait Query: for<'a> BinWrite<Args<'a> = ()> + Sized {
    type Response: BinRead;

    fn response_args(&self) -> <Self::Response as BinRead>::Args<'_>;

    fn receiver(&self) -> Endpoint;
    fn command(&self) -> Command;

    fn timeout(&self) -> Duration {
        DEFAULT_QUERY_TIMEOUT
    }
}
