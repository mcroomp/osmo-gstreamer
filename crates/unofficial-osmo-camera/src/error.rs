use crate::protocol::{ResponseStatus, identity::Command};
use std::{
    io,
    sync::{Arc, mpsc::RecvTimeoutError},
    time::SystemTimeError,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Operation cancelled")]
    Cancelled,
    #[cfg(feature = "ble")]
    #[error("Could not {operation}: {source}")]
    Bluetooth {
        operation: &'static str,
        #[source]
        source: Arc<btleplug::Error>,
    },
    #[error("Invalid {field}: {reason}")]
    InvalidConfig {
        field: &'static str,
        reason: &'static str,
    },
    #[error("Could not {operation}: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: Arc<io::Error>,
    },
    #[error("Cannot acquire camera ownership; close the existing C#/Rust owner first: {source}")]
    Ownership {
        #[source]
        source: Arc<io::Error>,
    },
    #[error("Invalid camera protocol data: {0}")]
    Protocol(&'static str),
    #[error("Unsupported camera feature: {0}")]
    Unsupported(&'static str),
    #[error("Unsupported {field}: {value:#04x}")]
    UnsupportedValue { field: &'static str, value: u8 },
    #[error("Camera limit exceeded: {0}")]
    LimitExceeded(&'static str),
    #[error("Invalid camera state: {0}")]
    InvalidState(&'static str),
    #[error("Timed out waiting for {operation}; no automatic reconnect")]
    Timeout { operation: &'static str },
    #[error("No matching reply for {command:?}; no automatic restart")]
    ReplyTimeout { command: Command },
    #[error("Camera rejected {command:?}: status {status}")]
    CameraRejected {
        command: Command,
        status: ResponseStatus,
    },
    #[error("Invalid reply to {command:?}: {reason}")]
    InvalidReply {
        command: Command,
        reason: &'static str,
    },
    #[error("Camera worker startup did not complete: {source}")]
    WorkerStartup {
        #[source]
        source: RecvTimeoutError,
    },
    #[error("Camera I/O thread panicked")]
    WorkerPanicked,
    #[error("Camera I/O thread terminated unexpectedly; no automatic reconnect")]
    WorkerTerminated,
    #[error("System clock is before the Unix epoch: {source}")]
    Clock {
        #[source]
        source: Arc<SystemTimeError>,
    },
    #[error(
        "Compressed preview queue overflow ({max_units} pictures / {max_bytes} bytes); stopped without dropping dependent pictures"
    )]
    QueueOverflow { max_units: usize, max_bytes: usize },
    #[error("{primary}; cleanup also failed: {cleanup}")]
    Cleanup {
        #[source]
        primary: Box<Error>,
        cleanup: Box<Error>,
    },
}

impl Error {
    pub fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io {
            operation,
            source: Arc::new(source),
        }
    }
    pub fn cleanup(primary: Self, cleanup: Self) -> Self {
        Self::Cleanup {
            primary: Box::new(primary),
            cleanup: Box::new(cleanup),
        }
    }
}

impl From<std::convert::Infallible> for Error {
    fn from(value: std::convert::Infallible) -> Self {
        match value {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn io_causes_survive_cloning_and_include_operation_context() {
        let error = Error::io(
            "receive a UDP datagram",
            io::Error::from(io::ErrorKind::ConnectionReset),
        );
        let cloned = error.clone();
        let Error::Io { source, .. } = &cloned else {
            panic!("Expected a typed I/O error");
        };
        assert_eq!(source.kind(), io::ErrorKind::ConnectionReset);
        assert!(cloned.source().is_some());
        assert!(cloned.to_string().contains("receive a UDP datagram"));
    }

    #[test]
    fn cleanup_preserves_both_failures_and_primary_source() {
        let error = Error::cleanup(
            Error::Cancelled,
            Error::Timeout {
                operation: "playback restoration",
            },
        );
        let Error::Cleanup { primary, cleanup } = &error else {
            panic!("Expected both errors");
        };
        assert!(matches!(**primary, Error::Cancelled));
        assert!(matches!(
            **cleanup,
            Error::Timeout {
                operation: "playback restoration"
            }
        ));
        assert!(error.source().is_some());
        assert!(error.to_string().contains("cleanup also failed"));
    }
}
