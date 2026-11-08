//! Unofficial Osmo Nano interoperability library.
//!
//! Rust owns protocol, transport, camera ownership and bounded lifecycles.
//! [`CameraProfile`] is a path-independent configuration model; with the `ble`
//! feature its explicitly approved preview startup can initialize over BLE when
//! the saved LAN pairing port is unreachable. [`CameraConfig`] entry points
//! remain LAN-only. Established-session failures never trigger reconnects.
//! With `ble`, [`CameraProfile`] provides `connect_control_via_ble`: BLE
//! preparation and verification, then saved-network association and one LAN
//! control connection.
//! Callers must authorize hardware operations and explicitly close sessions.

#[cfg(feature = "ble")]
pub mod ble;
mod camera_profile;
pub mod control;
#[cfg(feature = "ble")]
mod control_startup;
mod error;
mod installation_identity;
pub mod preview;
pub mod protocol;
pub mod rtmp;
mod session;
#[cfg(test)]
mod tests;

pub use camera_profile::CameraProfile;
pub use error::{Error, Result};
pub use preview::AccessUnit;
pub use rtmp::{RtmpConfig, StreamingResolution};
pub use session::{
    CameraConfig, CameraSession, ControlSession, PreviewCancellation, PreviewCommandEvent,
};

/// Returns the native library version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod preview_reader_timeout_tests {
    use super::*;

    fn access_unit() -> AccessUnit {
        AccessUnit {
            bytes: vec![0, 0, 1, 0x65],
            frame_number: 1,
            received_at: std::time::Instant::now(),
            camera_metadata: 0,
            keyframe: true,
            discontinuity: false,
        }
    }

    #[test]
    fn timeout_waits_for_data_and_returns_queued_units() {
        let reader = PreviewReader::default();
        assert!(matches!(
            reader.receive_timeout(Duration::from_millis(100)),
            Err(Error::Timeout {
                operation: "a live preview access unit"
            })
        ));
        reader.push(access_unit()).unwrap();
        assert_eq!(
            reader
                .receive_timeout(Duration::from_millis(100))
                .unwrap()
                .unwrap()
                .frame_number,
            1
        );
    }

    #[test]
    fn timeout_rejects_values_outside_supported_range() {
        let reader = PreviewReader::default();
        assert!(reader.receive_timeout(Duration::ZERO).is_err());
        assert!(reader.receive_timeout(Duration::from_secs(31)).is_err());
    }
}

#[cfg(test)]
mod version_tests {
    #[test]
    fn reports_package_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
    }
}
use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

fn validate_receive_timeout(timeout: Duration) -> Result<()> {
    const MIN: Duration = Duration::from_millis(100);
    const MAX: Duration = Duration::from_secs(30);
    if !(MIN..=MAX).contains(&timeout) {
        return Err(Error::InvalidConfig {
            field: "receive timeout",
            reason: "must be 0.1..30 seconds",
        });
    }
    Ok(())
}

pub struct CameraOwnerLease {
    _file: File,
}
impl CameraOwnerLease {
    pub fn acquire() -> Result<Self> {
        Self::acquire_at(&Self::directory()?.join("camera-owner.lock"))
    }
    fn directory() -> Result<PathBuf> {
        #[cfg(windows)]
        let directory = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(target_os = "macos")]
        let directory = std::env::var_os("HOME").map(|home| {
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
        });
        #[cfg(not(any(windows, target_os = "macos")))]
        let directory = std::env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            });
        directory
            .map(|path| path.join("Osmo"))
            .ok_or(Error::InvalidConfig {
                field: "camera lease directory",
                reason: "a per-user application data directory is required",
            })
    }
    fn acquire_at(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path.parent().ok_or(Error::InvalidConfig {
            field: "camera lease path",
            reason: "missing parent directory",
        })?)
        .map_err(|error| Error::io("create the camera ownership directory", error))?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let file = options.open(path).map_err(|source| Error::Ownership {
            source: Arc::new(source),
        })?;
        #[cfg(not(windows))]
        file.try_lock().map_err(|error| Error::Ownership {
            source: Arc::new(std::io::Error::other(error)),
        })?;
        Ok(Self { _file: file })
    }
}

const QUEUE_UNITS: usize = 32;
const QUEUE_BYTES: usize = 16 * 1024 * 1024;
#[derive(Default)]
struct QueueState {
    units: VecDeque<AccessUnit>,
    bytes: usize,
    flushing: bool,
    terminal: Option<Result<()>>,
}
#[derive(Default)]
struct Queue {
    state: Mutex<QueueState>,
    changed: Condvar,
}

#[derive(Clone, Default)]
pub struct PreviewReader {
    queue: Arc<Queue>,
}
impl PreviewReader {
    pub fn receive(&self) -> Result<Option<AccessUnit>> {
        let mut state = self.queue.state.lock().expect("preview queue poisoned");
        loop {
            if state.flushing {
                return Err(Error::Cancelled);
            }
            if let Some(Err(error)) = &state.terminal {
                return Err(error.clone());
            }
            if let Some(unit) = state.units.pop_front() {
                state.bytes -= unit.bytes.len();
                return Ok(Some(unit));
            }
            if state.terminal.is_some() {
                return Ok(None);
            }
            state = self
                .queue
                .changed
                .wait(state)
                .expect("preview queue poisoned");
        }
    }
    /// Receives one preview access unit, waiting up to the supplied timeout.
    /// A cleanly closed stream returns `None`; silence past the deadline is an
    /// error so callers can distinguish an idle stream from normal completion.
    pub fn receive_timeout(&self, timeout: Duration) -> Result<Option<AccessUnit>> {
        crate::validate_receive_timeout(timeout)?;
        let deadline = std::time::Instant::now() + timeout;
        let mut state = self.queue.state.lock().expect("preview queue poisoned");
        loop {
            if state.flushing {
                return Err(Error::Cancelled);
            }
            if let Some(Err(error)) = &state.terminal {
                return Err(error.clone());
            }
            if let Some(unit) = state.units.pop_front() {
                state.bytes -= unit.bytes.len();
                return Ok(Some(unit));
            }
            if state.terminal.is_some() {
                return Ok(None);
            }
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(Error::Timeout {
                    operation: "a live preview access unit",
                });
            }
            let (next_state, wait) = self
                .queue
                .changed
                .wait_timeout(state, remaining)
                .expect("preview queue poisoned");
            state = next_state;
            if wait.timed_out() && state.units.is_empty() && state.terminal.is_none() {
                return Err(Error::Timeout {
                    operation: "a live preview access unit",
                });
            }
        }
    }
    pub fn set_flushing(&self, flushing: bool) {
        self.queue
            .state
            .lock()
            .expect("preview queue poisoned")
            .flushing = flushing;
        self.queue.changed.notify_all();
    }
    fn push(&self, unit: AccessUnit) -> Result<()> {
        let mut state = self.queue.state.lock().expect("preview queue poisoned");
        if state.terminal.is_some() {
            return Err(Error::InvalidState("Preview queue is already closed"));
        }
        if state.units.len() == QUEUE_UNITS || unit.bytes.len() > QUEUE_BYTES - state.bytes {
            let error = Error::QueueOverflow {
                max_units: QUEUE_UNITS,
                max_bytes: QUEUE_BYTES,
            };
            state.terminal = Some(Err(error.clone()));
            self.queue.changed.notify_all();
            return Err(error);
        }
        state.bytes += unit.bytes.len();
        state.units.push_back(unit);
        self.queue.changed.notify_all();
        Ok(())
    }
    fn finish(&self, result: Result<()>) {
        let mut state = self.queue.state.lock().expect("preview queue poisoned");
        state.terminal = Some(result);
        self.queue.changed.notify_all();
    }
}
