use crate::{
    CameraConfig, CameraSession, Error, PreviewCancellation, Result,
    ble::{BleCancellation, BleConfig, BleSession, WifiNetwork},
    installation_identity::validate_identity,
    protocol::PAIRING_PORT,
};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    fmt, fs,
    io::{self, Write},
    net::{IpAddr, SocketAddr, TcpStream},
    path::Path,
    thread,
    time::{Duration, Instant},
};

/// Validated settings for one camera, independent of any repository layout.
///
/// Profiles can be loaded from a caller-selected file or TOML text in memory.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CameraProfile {
    #[serde(default = "current_version")]
    version: u8,
    /// Wi-Fi network name used during camera association.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_ssid: Option<String>,
    /// Wi-Fi password used during camera association.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_password: Option<String>,
    /// Pairing identity reused for authorized BLE sessions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairing_identity: Option<String>,
    /// Camera's Wi-Fi MAC address, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wifi_mac: Option<String>,
    /// Camera's LAN IP address, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_ip: Option<IpAddr>,
}

impl Default for CameraProfile {
    fn default() -> Self {
        Self {
            version: current_version(),
            wifi_ssid: None,
            wifi_password: None,
            pairing_identity: None,
            wifi_mac: None,
            camera_ip: None,
        }
    }
}

impl CameraProfile {
    /// Starts an explicitly approved preview, initializing through BLE only
    /// when the saved LAN pairing port is unreachable. Never reconnects after
    /// startup or playback failure.
    pub fn start_preview(
        &mut self,
        approved: bool,
        cancellation: &PreviewCancellation,
    ) -> Result<CameraSession> {
        if !approved {
            return Err(Error::InvalidConfig {
                field: "allow-writes",
                reason: "camera initialization and preview require explicit approval",
            });
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let host = self.camera_ip.ok_or(Error::InvalidConfig {
            field: "camera profile",
            reason: "preview requires a verified camera_ip",
        })?;
        if host.is_unspecified() || host.is_multicast() {
            return Err(Error::InvalidConfig {
                field: "camera_ip",
                reason: "an explicit unicast LAN camera address is required",
            });
        }
        self.to_toml()?;
        match TcpStream::connect_timeout(
            &SocketAddr::new(host, PAIRING_PORT),
            Duration::from_secs(3),
        ) {
            Ok(probe) => {
                drop(probe);
                info!("Camera pairing port is reachable; skipping BLE initialization");
                let identity = self.pairing_identity.clone().ok_or(Error::InvalidConfig {
                    field: "camera profile",
                    reason: "LAN preview requires pairing_identity",
                })?;
                return CameraSession::start_preview_cancellable(
                    CameraConfig {
                        host,
                        pairing_identity: identity,
                        allow_writes: true,
                    },
                    cancellation,
                );
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::ConnectionRefused
                        | io::ErrorKind::HostUnreachable
                        | io::ErrorKind::NetworkUnreachable
                ) =>
            {
                warn!(
                    "Camera pairing port is unreachable ({error}); initializing once through BLE"
                );
            }
            Err(error) => return Err(Error::io("probe the camera pairing port", error)),
        }
        if cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let network = WifiNetwork::new(
            self.wifi_ssid.clone().ok_or(Error::InvalidConfig {
                field: "camera profile",
                reason: "BLE initialization requires wifi_ssid",
            })?,
            self.wifi_password.clone().ok_or(Error::InvalidConfig {
                field: "camera profile",
                reason: "BLE initialization requires wifi_password",
            })?,
        )?;
        let mut config = BleConfig::new(self.pairing_identity.clone());
        config.network = Some(network);
        let mut ble =
            BleSession::connect_cancellable(config, &BleCancellation::for_preview(cancellation))?;
        self.pairing_identity = Some(ble.pairing_identity().to_owned());
        let startup = (|| {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                if cancellation.is_cancelled() {
                    return Err(Error::Cancelled);
                }
                match TcpStream::connect_timeout(
                    &SocketAddr::new(host, PAIRING_PORT),
                    Duration::from_secs(1),
                ) {
                    Ok(probe) => {
                        drop(probe);
                        break;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::TimedOut
                                | io::ErrorKind::ConnectionRefused
                                | io::ErrorKind::HostUnreachable
                                | io::ErrorKind::NetworkUnreachable
                        ) =>
                    {
                        if Instant::now() >= deadline {
                            return Err(Error::Timeout {
                                operation: "LAN readiness after BLE initialization; verify the saved camera_ip",
                            });
                        }
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(error) => {
                        return Err(Error::io(
                            "check LAN readiness after BLE initialization",
                            error,
                        ));
                    }
                }
            }
            info!("Camera LAN pairing port is ready after BLE initialization");
            ble.start_preview_cancellable(host, cancellation)
        })();
        match startup {
            Ok(mut session) => {
                session.ble_owner = Some(ble);
                Ok(session)
            }
            Err(primary) => match ble.close() {
                Ok(()) => Err(primary),
                Err(cleanup) => Err(Error::cleanup(primary, cleanup)),
            },
        }
    }

    /// Read and validate a profile from a caller-selected path.
    pub fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let contents = fs::read_to_string(path)?;
        Self::from_toml(&contents)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
    }

    /// Parse and validate a profile from TOML text in memory.
    pub fn from_toml(contents: &str) -> Result<Self> {
        let profile: Self = toml::from_str(contents).map_err(|_| Error::InvalidConfig {
            field: "camera profile",
            reason: "invalid TOML configuration",
        })?;
        if profile.version != current_version() {
            return Err(Error::InvalidConfig {
                field: "camera profile version",
                reason: "unsupported version",
            });
        }
        if let Some(identity) = &profile.pairing_identity {
            validate_identity(identity)?;
        }
        if let Some(mac) = &profile.wifi_mac {
            parse_mac(mac)?;
        }
        Ok(profile)
    }

    /// Serialize the validated profile as TOML.
    pub fn to_toml(&self) -> Result<String> {
        let contents = toml::to_string_pretty(self).map_err(|_| Error::InvalidConfig {
            field: "camera profile",
            reason: "could not serialize configuration",
        })?;
        Self::from_toml(&contents)?;
        Ok(contents)
    }

    /// Check a profile name before using it as part of a path.
    pub fn validate_name(name: &str) -> Result<()> {
        if name.is_empty()
            || name.len() > 64
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(Error::InvalidConfig {
                field: "camera profile name",
                reason: "use 1-64 letters, digits, hyphens, or underscores",
            });
        }
        Ok(())
    }

    /// Atomically save the profile to a caller-selected path.
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let path = path.as_ref();
        let contents = self
            .to_toml()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(contents.as_bytes())?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    /// Return the Wi-Fi MAC address as six bytes when the profile contains it.
    pub fn wifi_mac_bytes(&self) -> Result<Option<[u8; 6]>> {
        self.wifi_mac.as_deref().map(parse_mac).transpose()
    }
}

impl fmt::Debug for CameraProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CameraProfile")
            .field("version", &self.version)
            .field("wifi_ssid", &self.wifi_ssid)
            .field(
                "wifi_password",
                &self.wifi_password.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "pairing_identity",
                &self.pairing_identity.as_ref().map(|_| "[REDACTED]"),
            )
            .field("wifi_mac", &self.wifi_mac.as_ref().map(|_| "[REDACTED]"))
            .field("camera_ip", &self.camera_ip)
            .finish()
    }
}

fn current_version() -> u8 {
    1
}

fn parse_mac(value: &str) -> Result<[u8; 6]> {
    let bytes: Vec<_> = value
        .split(':')
        .map(|part| u8::from_str_radix(part, 16))
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error::InvalidConfig {
            field: "camera Wi-Fi MAC",
            reason: "expected six colon-separated hexadecimal bytes",
        })?;
    bytes.try_into().map_err(|_| Error::InvalidConfig {
        field: "camera Wi-Fi MAC",
        reason: "expected six colon-separated hexadecimal bytes",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_requires_approval_valid_ip_and_respects_cancellation_before_io() {
        let mut profile = CameraProfile::default();
        let cancellation = crate::PreviewCancellation::default();
        assert!(matches!(
            profile.start_preview(false, &cancellation),
            Err(Error::InvalidConfig {
                field: "allow-writes",
                ..
            })
        ));
        cancellation.cancel();
        assert!(matches!(
            profile.start_preview(true, &cancellation),
            Err(Error::Cancelled)
        ));
        let cancellation = crate::PreviewCancellation::default();
        assert!(matches!(
            profile.start_preview(true, &cancellation),
            Err(Error::InvalidConfig {
                field: "camera profile",
                ..
            })
        ));
        for host in ["0.0.0.0", "224.0.0.1", "::"] {
            profile.camera_ip = Some(host.parse().unwrap());
            assert!(matches!(
                profile.start_preview(true, &cancellation),
                Err(Error::InvalidConfig {
                    field: "camera_ip",
                    ..
                })
            ));
        }
    }

    #[test]
    fn config_round_trips_without_exposing_credentials_in_debug() {
        let profile = CameraProfile {
            wifi_ssid: Some("synthetic network".into()),
            wifi_password: Some("synthetic-password".into()),
            pairing_identity: Some("0123456789abcdef0123456789abcdef".into()),
            wifi_mac: Some("02:00:00:00:00:AB".into()),
            camera_ip: Some("192.0.2.15".parse().unwrap()),
            ..CameraProfile::default()
        };
        let serialized = profile.to_toml().unwrap();
        let decoded = CameraProfile::from_toml(&serialized).unwrap();
        assert_eq!(decoded.wifi_ssid, profile.wifi_ssid);
        assert_eq!(decoded.wifi_password, profile.wifi_password);
        assert_eq!(decoded.pairing_identity, profile.pairing_identity);
        assert_eq!(
            decoded.wifi_mac_bytes().unwrap(),
            Some([0x02, 0, 0, 0, 0, 0xab])
        );
        assert_eq!(decoded.camera_ip, profile.camera_ip);
        let debug = format!("{decoded:?}");
        assert!(!debug.contains("synthetic-password"));
        assert!(!debug.contains("0123456789abcdef"));
    }

    #[test]
    fn rejects_invalid_identity_mac_and_unknown_fields() {
        assert!(CameraProfile::from_toml("pairing_identity = \"invalid\"").is_err());
        assert!(CameraProfile::from_toml("wifi_mac = \"not-a-mac\"").is_err());
        assert!(CameraProfile::from_toml("unknown = true").is_err());
    }

    #[test]
    fn saves_over_an_existing_file_and_loads_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("camera.toml");
        let mut profile = CameraProfile {
            wifi_ssid: Some("first network".into()),
            ..CameraProfile::default()
        };
        profile.save_to_file(&path).unwrap();
        profile.wifi_ssid = Some("second network".into());
        profile.save_to_file(&path).unwrap();
        assert_eq!(
            CameraProfile::from_file(path).unwrap().wifi_ssid.as_deref(),
            Some("second network")
        );
    }
}
