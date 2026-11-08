//! Explicit BLE-first preparation followed by one LAN control connection.

use crate::{
    CameraConfig, CameraProfile, ControlSession, Error, PreviewCancellation, Result,
    ble::{BleCancellation, BleConfig, BleQuery, BleSession, NetworkCleanup, WifiNetwork},
    installation_identity::validate_identity,
    protocol::{
        CommandPacket, PAIRING_PORT,
        commands::{reply::CommandReadReply, wifi::cmd_07_0c_mac_get::WifiMacGet},
    },
};
use std::{
    io,
    net::{SocketAddr, TcpStream},
    thread,
    time::{Duration, Instant},
};

const LAN_READY_TIMEOUT: Duration = Duration::from_secs(10);
const LAN_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const LAN_POLL_INTERVAL: Duration = Duration::from_millis(100);

impl CameraProfile {
    /// Prepare over BLE, verify live health, associate with the saved network,
    /// close BLE, wait for LAN readiness, then open one control session.
    ///
    /// Requires explicit approval and a previously paired, complete profile.
    /// Reuses the saved identity without resetting it or updating the profile.
    /// A saved Wi-Fi MAC, when present, must match before network association.
    /// On-device pairing approval may still be required. The computer's network
    /// is unchanged; no preview, mode change or failed-session reconnect occurs.
    pub fn connect_control_via_ble(
        &self,
        approved: bool,
        cancellation: &PreviewCancellation,
    ) -> Result<ControlSession> {
        if !approved {
            return Err(Error::InvalidConfig {
                field: "BLE-first startup",
                reason: "pairing, wake, network association and LAN registration require explicit approval",
            });
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        self.to_toml()?;
        let host = self.camera_ip.ok_or(Error::InvalidConfig {
            field: "camera profile",
            reason: "BLE-first control requires a verified camera_ip",
        })?;
        if host.is_unspecified() || host.is_multicast() {
            return Err(Error::InvalidConfig {
                field: "camera_ip",
                reason: "an explicit unicast LAN camera address is required",
            });
        }
        let identity = self.pairing_identity.clone().ok_or(Error::InvalidConfig {
            field: "camera profile",
            reason: "BLE-first control requires an existing pairing_identity; pair explicitly first",
        })?;
        validate_identity(&identity)?;
        let network = WifiNetwork::new(
            self.wifi_ssid.clone().ok_or(Error::InvalidConfig {
                field: "camera profile",
                reason: "BLE-first control requires wifi_ssid",
            })?,
            self.wifi_password.clone().ok_or(Error::InvalidConfig {
                field: "camera profile",
                reason: "BLE-first control requires wifi_password",
            })?,
        )?;
        let expected_mac = self.wifi_mac_bytes()?;

        log::info!("BLE-first control: pairing/wake and live BLE health verification");
        // No network is supplied until the BLE health check and identity check pass.
        let mut ble = BleSession::connect_cancellable(
            BleConfig::new(Some(identity.clone())),
            &BleCancellation::for_preview(cancellation),
        )?;
        let preparation = (|| {
            if let Some(expected) = expected_mac {
                match ble.execute_query::<WifiMacGet>()? {
                    CommandReadReply::Accepted(reply) => verify_mac(expected, reply.address)?,
                    CommandReadReply::Rejected { status, .. } => {
                        return Err(Error::CameraRejected {
                            command: WifiMacGet.command(),
                            status,
                        });
                    }
                }
            }
            log::info!("BLE-first control: BLE verified; associating with the saved network");
            ble.join_network(network, NetworkCleanup::Preserve)?;
            ble.query(BleQuery::WifiCountry)?.require_success()?;
            ble.ownership_lease()
        })();
        let cleanup = ble.close();
        let lease = finish_preparation(preparation, cleanup)?;
        log::info!("BLE-first control: BLE closed; waiting for the saved LAN endpoint");
        wait_for_lan(LAN_READY_TIMEOUT, cancellation, |timeout| {
            TcpStream::connect_timeout(&SocketAddr::new(host, PAIRING_PORT), timeout).map(drop)
        })?;
        log::info!("BLE-first control: LAN ready; opening one control session");
        ControlSession::connect_with_lease(
            CameraConfig {
                host,
                pairing_identity: identity,
                allow_writes: true,
            },
            cancellation,
            Some(lease),
        )
    }
}

fn verify_mac(expected: [u8; 6], actual: [u8; 6]) -> Result<()> {
    if expected != actual {
        return Err(Error::InvalidConfig {
            field: "camera profile",
            reason: "BLE camera Wi-Fi identity does not match the saved profile",
        });
    }
    Ok(())
}

fn finish_preparation<T>(result: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
    }
}

fn wait_for_lan(
    timeout: Duration,
    cancellation: &PreviewCancellation,
    mut probe: impl FnMut(Duration) -> io::Result<()>,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(Error::Timeout {
                operation: "LAN readiness after BLE preparation; verify the saved camera_ip",
            });
        }
        let result = probe(remaining.min(LAN_PROBE_TIMEOUT));
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        match result {
            Ok(()) if Instant::now() < deadline => return Ok(()),
            Ok(()) => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::HostUnreachable
                        | io::ErrorKind::NetworkUnreachable
                ) => {}
            Err(error) => {
                return Err(Error::io(
                    "check LAN readiness after BLE preparation",
                    error,
                ));
            }
        }
        thread::sleep(LAN_POLL_INTERVAL.min(deadline.saturating_duration_since(Instant::now())));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> CameraProfile {
        CameraProfile::from_toml(
            "camera_ip = \"192.0.2.15\"\n\
             pairing_identity = \"0123456789abcdef0123456789abcdef\"\n\
             wifi_ssid = \"synthetic network\"\n\
             wifi_password = \"synthetic-password\"\n",
        )
        .unwrap()
    }

    #[test]
    fn preflight_rejects_unapproved_cancelled_and_incomplete_profiles_before_io() {
        let cancellation = PreviewCancellation::default();
        assert!(matches!(
            profile().connect_control_via_ble(false, &cancellation),
            Err(Error::InvalidConfig {
                field: "BLE-first startup",
                ..
            })
        ));
        cancellation.cancel();
        assert!(matches!(
            profile().connect_control_via_ble(true, &cancellation),
            Err(Error::Cancelled)
        ));
        let cancellation = PreviewCancellation::default();
        let mut incomplete = [profile(), profile(), profile(), profile(), profile()];
        incomplete[0].camera_ip = None;
        incomplete[1].pairing_identity = None;
        incomplete[2].wifi_ssid = None;
        incomplete[3].wifi_password = None;
        incomplete[4].pairing_identity = Some("invalid".into());
        for incomplete in incomplete {
            assert!(matches!(
                incomplete.connect_control_via_ble(true, &cancellation),
                Err(Error::InvalidConfig { .. })
            ));
        }
        for address in ["0.0.0.0", "224.0.0.1", "::"] {
            let mut invalid = profile();
            invalid.camera_ip = Some(address.parse().unwrap());
            assert!(matches!(
                invalid.connect_control_via_ble(true, &cancellation),
                Err(Error::InvalidConfig {
                    field: "camera_ip",
                    ..
                })
            ));
        }
    }

    #[test]
    fn saved_camera_identity_mismatch_is_an_explicit_sanitized_failure() {
        verify_mac([2, 0, 0, 0, 0, 1], [2, 0, 0, 0, 0, 1]).unwrap();
        assert!(matches!(
            verify_mac([2, 0, 0, 0, 0, 1], [2, 0, 0, 0, 0, 2]),
            Err(Error::InvalidConfig {
                field: "camera profile",
                ..
            })
        ));
    }

    #[test]
    fn preparation_and_cleanup_failures_both_survive() {
        finish_preparation(Ok(()), Ok(())).unwrap();
        assert!(matches!(
            finish_preparation::<()>(Err(Error::Cancelled), Ok(())),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            finish_preparation(Ok(()), Err(Error::WorkerTerminated)),
            Err(Error::WorkerTerminated)
        ));
        assert!(matches!(
            finish_preparation::<()>(Err(Error::Cancelled), Err(Error::WorkerTerminated)),
            Err(Error::Cleanup { .. })
        ));
    }

    #[test]
    fn readiness_checks_are_bounded_and_retry_only_transient_probe_failures() {
        let cancellation = PreviewCancellation::default();
        let start = Instant::now();
        let mut attempts = 0;
        wait_for_lan(Duration::from_millis(250), &cancellation, |timeout| {
            assert!(timeout <= Duration::from_millis(250));
            attempts += 1;
            if attempts == 1 {
                Err(io::ErrorKind::ConnectionRefused.into())
            } else {
                Ok(())
            }
        })
        .unwrap();
        assert_eq!(attempts, 2);
        assert!(start.elapsed() >= LAN_POLL_INTERVAL);
        let start = Instant::now();
        assert!(matches!(
            wait_for_lan(Duration::from_millis(30), &cancellation, |timeout| {
                assert!(timeout <= Duration::from_millis(30));
                Err(io::ErrorKind::TimedOut.into())
            }),
            Err(Error::Timeout { .. })
        ));
        assert!(start.elapsed() < Duration::from_millis(250));
        let mut attempts = 0;
        assert!(matches!(
            wait_for_lan(Duration::from_secs(1), &cancellation, |_| {
                attempts += 1;
                Err(io::ErrorKind::PermissionDenied.into())
            }),
            Err(Error::Io { .. })
        ));
        assert_eq!(attempts, 1);
    }

    #[test]
    fn readiness_never_accepts_a_cancelled_or_late_probe() {
        let cancellation = PreviewCancellation::default();
        assert!(matches!(
            wait_for_lan(Duration::from_secs(1), &cancellation, |_| {
                cancellation.cancel();
                Ok(())
            }),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            wait_for_lan(Duration::from_secs(1), &cancellation, |_| panic!(
                "cancelled before probe"
            )),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            wait_for_lan(
                Duration::from_millis(1),
                &PreviewCancellation::default(),
                |_| {
                    thread::sleep(Duration::from_millis(5));
                    Ok(())
                }
            ),
            Err(Error::Timeout { .. })
        ));
    }
}
