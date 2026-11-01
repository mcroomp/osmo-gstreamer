#[path = "osmo-pair/neighbors.rs"]
mod neighbors;
#[path = "osmo-pair/timing.rs"]
mod timing;

use osmo_camera::{
    CameraConfig, CameraProfile, CameraSession, Error as CameraError,
    ble::{BleConfig, BleSession, NetworkCleanup, WifiNetwork},
    protocol::commands::{reply::CameraReadReply, wifi::cmd_07_0c_mac_get::WifiMacGet},
};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    net::IpAddr,
    path::{Path, PathBuf},
};

const CREDENTIALS_DIRECTORY: &str = ".credentials";
const DEFAULT_PROFILE: &str = "camera";
const SMOKE_TEST_FRAMES: usize = 3;
const SMOKE_TEST_FRAME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const USAGE: &str = "Usage: osmo-pair [--camera PROFILE] [--smoke-test | --timing-test SECONDS (1-300)] [--lan-only]";

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .try_init()?;
    let options = parse_options(std::env::args().skip(1))?;
    let credentials_dir = PathBuf::from(CREDENTIALS_DIRECTORY);
    let profile_path = credentials_dir.join(format!("{}.toml", options.profile_name));
    let (mut profile, legacy_files) =
        load_profile(&credentials_dir, &profile_path, &options.profile_name)?;
    if options.smoke_test || options.timing_seconds.is_some() {
        preview_diagnostic(&mut profile, &profile_path, &options)?;
        return Ok(());
    }

    let network = prompt_for_missing_network(&mut profile)?;
    profile.save_to_file(&profile_path)?;
    remove_migrated_files(&legacy_files)?;

    eprintln!("Starting BLE pairing and camera Wi-Fi association.");
    eprintln!("Approve the pairing on the camera if prompted.");
    let mut config = BleConfig::new(profile.pairing_identity.clone());
    config.network = Some(network);
    config.network_cleanup = NetworkCleanup::Preserve;
    let mut session = BleSession::connect(config)?;
    profile.pairing_identity = Some(session.pairing_identity().to_owned());
    profile.save_to_file(&profile_path)?;

    let response = session.execute_query(WifiMacGet)?;
    let mac = match response {
        CameraReadReply::Accepted(response) => response.address,
        CameraReadReply::Rejected { .. } => {
            return Err("Camera rejected the Wi-Fi MAC address query.".into());
        }
    };
    session.close()?;

    profile.wifi_mac = Some(format_mac(mac));
    println!("Camera Wi-Fi MAC: {}", profile.wifi_mac.as_deref().unwrap());
    match neighbors::find_ipv4_for_mac(mac) {
        Ok(addresses) if addresses.len() == 1 => {
            let address = IpAddr::V4(addresses[0]);
            match profile.camera_ip {
                Some(saved) if saved != address => println!(
                    "Neighbor cache reports {address}, but the profile retains its existing IP {saved}; verify both before use."
                ),
                _ => {
                    profile.camera_ip = Some(address);
                    println!(
                        "Found cached neighbor-table IP {address}; verify it is current before use."
                    );
                }
            }
        }
        Ok(addresses) if addresses.is_empty() => {
            println!(
                "No IPv4 neighbor-table entry matches this MAC; check the router's DHCP leases."
            );
        }
        Ok(addresses) => {
            println!(
                "Multiple IPv4 neighbor-table entries match this MAC: {}. Check the router's DHCP leases.",
                addresses
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        Err(error) => {
            eprintln!("Could not read the local IPv4 neighbor table: {error}");
        }
    }

    profile.save_to_file(&profile_path)?;
    println!(
        "Camera profile saved to {}. The camera remains on the configured network.",
        profile_path.display()
    );
    Ok(())
}

struct Options {
    profile_name: String,
    smoke_test: bool,
    timing_seconds: Option<u64>,
    lan_only: bool,
}

fn parse_options(mut args: impl Iterator<Item = String>) -> Result<Options, Box<dyn Error>> {
    let mut options = Options {
        profile_name: DEFAULT_PROFILE.to_owned(),
        smoke_test: false,
        timing_seconds: None,
        lan_only: false,
    };
    let mut associate = false;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--camera" => {
                let name = args.next().ok_or(USAGE)?;
                CameraProfile::validate_name(&name)?;
                options.profile_name = name;
            }
            "--associate" => associate = true,
            "--smoke-test" => options.smoke_test = true,
            "--lan-only" => options.lan_only = true,
            "--timing-test" => {
                let seconds: u64 = args.next().ok_or(USAGE)?.parse()?;
                if !(1..=300).contains(&seconds) {
                    return Err("Timing duration must be 1-300 seconds.".into());
                }
                options.timing_seconds = Some(seconds);
            }
            _ => return Err(USAGE.into()),
        }
    }
    if options.timing_seconds.is_some() && (options.smoke_test || associate) {
        return Err("Use --timing-test without --smoke-test or --associate.".into());
    }
    if options.lan_only && (!options.smoke_test && options.timing_seconds.is_none() || associate) {
        return Err(
            "--lan-only requires --smoke-test or --timing-test, without --associate.".into(),
        );
    }
    Ok(options)
}

fn preview_diagnostic(
    profile: &mut CameraProfile,
    path: &Path,
    options: &Options,
) -> Result<(), CameraError> {
    let run = |session| match options.timing_seconds {
        Some(seconds) => timing::run(session, seconds),
        None => smoke_test(session),
    };
    if options.lan_only {
        log::info!("LAN-only diagnostic: no BLE initialization or Wi-Fi changes");
        return run(CameraSession::start_preview(camera_config(profile)?)?);
    }
    let mut session = profile.start_preview(true, &osmo_camera::PreviewCancellation::default())?;
    if let Err(error) = profile.save_to_file(path) {
        let primary = CameraError::io("save the initialized camera profile", error);
        return match session.close() {
            Ok(()) => Err(primary),
            Err(cleanup) => Err(CameraError::cleanup(primary, cleanup)),
        };
    }
    run(session)
}

fn smoke_test(mut session: CameraSession) -> Result<(), CameraError> {
    log::info!("LAN camera preview smoke-test startup succeeded");
    println!(
        "[2/3] Preview startup succeeded; waiting for {SMOKE_TEST_FRAMES} video access units."
    );

    let reader = session.reader();
    let receive_result = (|| -> Result<(), CameraError> {
        for index in 1..=SMOKE_TEST_FRAMES {
            let unit =
                reader
                    .receive_timeout(SMOKE_TEST_FRAME_TIMEOUT)?
                    .ok_or(CameraError::Timeout {
                        operation: "the preview stream to remain active",
                    })?;
            log::info!(
                "Received smoke-test access unit {index}/{SMOKE_TEST_FRAMES}: {} bytes, frame {}, keyframe={}",
                unit.bytes.len(),
                unit.frame_number,
                unit.keyframe
            );
            println!(
                "  Access unit {index}/{SMOKE_TEST_FRAMES}: {} bytes, frame {}, keyframe={}",
                unit.bytes.len(),
                unit.frame_number,
                unit.keyframe
            );
        }
        Ok(())
    })();

    println!("[3/3] Closing the preview session...");
    let close_result = session.close();
    match (receive_result, close_result) {
        (Ok(()), Ok(())) => {
            log::info!("LAN camera preview smoke test passed");
            println!(
                "Smoke test passed: received {SMOKE_TEST_FRAMES} H.264 access units and closed the session."
            );
            Ok(())
        }
        (Err(primary), Err(cleanup)) => {
            let error = CameraError::cleanup(primary, cleanup);
            log::error!("Preview smoke test and cleanup failed: {error}");
            Err(error)
        }
        (Err(error), Ok(())) | (Ok(()), Err(error)) => {
            log::error!("Preview smoke test failed: {error}");
            Err(error)
        }
    }
}

fn camera_config(profile: &CameraProfile) -> Result<CameraConfig, CameraError> {
    let host = profile.camera_ip.ok_or(CameraError::InvalidConfig {
        field: "camera profile",
        reason: "preview diagnostics require camera_ip",
    })?;
    let pairing_identity = profile
        .pairing_identity
        .clone()
        .ok_or(CameraError::InvalidConfig {
            field: "camera profile",
            reason: "preview diagnostics require pairing_identity",
        })?;
    Ok(CameraConfig {
        host,
        pairing_identity,
        allow_writes: true,
    })
}

fn load_profile(
    credentials_dir: &Path,
    profile_path: &Path,
    profile_name: &str,
) -> Result<(CameraProfile, Vec<PathBuf>), Box<dyn Error>> {
    match CameraProfile::from_file(profile_path) {
        Ok(profile) => Ok((profile, Vec::new())),
        Err(error)
            if error.kind() == io::ErrorKind::NotFound && profile_name == DEFAULT_PROFILE =>
        {
            let (profile, migrated_files) = migrate_legacy_profile(credentials_dir)?;
            Ok((profile, migrated_files))
        }
        Err(error) => Err(error.into()),
    }
}

fn migrate_legacy_profile(
    credentials_dir: &Path,
) -> Result<(CameraProfile, Vec<PathBuf>), Box<dyn Error>> {
    let mut profile = CameraProfile::default();
    let mut migrated_files = Vec::new();
    let network_path = credentials_dir.join("wifi-network.txt");
    if network_path.exists() {
        let contents = fs::read_to_string(&network_path)?;
        let mut lines = contents.lines();
        profile.wifi_ssid = lines
            .next()
            .map(|line| line.trim_end_matches('\r').to_owned());
        profile.wifi_password = lines
            .next()
            .map(|line| line.trim_end_matches('\r').to_owned());
        if lines.next().is_some() || profile.wifi_ssid.is_none() || profile.wifi_password.is_none()
        {
            return Err("Legacy Wi-Fi file must contain exactly two lines.".into());
        }
        migrated_files.push(network_path);
    }

    let identity_path = credentials_dir.join("identity.txt");
    if identity_path.exists() {
        profile.pairing_identity = Some(fs::read_to_string(&identity_path)?.trim().to_owned());
        migrated_files.push(identity_path);
    }

    let ip_path = credentials_dir.join("camera-ip.txt");
    if ip_path.exists() {
        let value = fs::read_to_string(&ip_path)?;
        profile.camera_ip = Some(value.trim().parse()?);
        migrated_files.push(ip_path);
    }
    profile.to_toml()?;
    Ok((profile, migrated_files))
}

fn remove_migrated_files(paths: &[PathBuf]) -> io::Result<()> {
    for path in paths {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn prompt_for_missing_network(profile: &mut CameraProfile) -> Result<WifiNetwork, Box<dyn Error>> {
    if profile
        .wifi_ssid
        .as_deref()
        .is_none_or(|value| value.is_empty())
    {
        profile.wifi_ssid = Some(prompt_line("Wi-Fi network name (SSID): ")?);
    }
    if profile
        .wifi_password
        .as_deref()
        .is_none_or(|value| value.is_empty())
    {
        profile.wifi_password = Some(rpassword::prompt_password("Wi-Fi password: ")?);
    }
    Ok(WifiNetwork::new(
        profile.wifi_ssid.clone().ok_or("Wi-Fi SSID is required")?,
        profile
            .wifi_password
            .clone()
            .ok_or("Wi-Fi password is required")?,
    )?)
}

fn prompt_line(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    while value.ends_with(['\n', '\r']) {
        value.pop();
    }
    if value.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a non-empty Wi-Fi SSID is required",
        ));
    }
    Ok(value)
}

fn format_mac(mac: [u8; 6]) -> String {
    mac.iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_select_profile_and_smoke_test_mode() {
        let defaults = parse_options(std::iter::empty()).unwrap();
        assert_eq!(defaults.profile_name, DEFAULT_PROFILE);
        assert!(!defaults.smoke_test);
        assert!(!defaults.lan_only);

        let options = parse_options(
            [
                "--camera".to_owned(),
                "living-room".to_owned(),
                "--smoke-test".to_owned(),
            ]
            .into_iter(),
        )
        .unwrap();
        assert_eq!(options.profile_name, "living-room");
        assert!(options.smoke_test);
        assert!(
            parse_options(["--camera".to_owned(), "../outside".to_owned()].into_iter()).is_err()
        );
    }

    #[test]
    fn smoke_test_requires_camera_ip_and_pairing_identity() {
        let mut profile = CameraProfile::default();
        assert!(camera_config(&profile).is_err());
        profile.camera_ip = Some("192.0.2.10".parse().unwrap());
        assert!(camera_config(&profile).is_err());
        profile.pairing_identity = Some("0123456789abcdef0123456789abcdef".into());
        let config = camera_config(&profile).unwrap();
        assert_eq!(config.host, "192.0.2.10".parse::<IpAddr>().unwrap());
        assert!(config.allow_writes);
    }

    #[test]
    fn timing_options_are_bounded_and_do_not_associate() {
        let options = parse_options(["--timing-test".into(), "60".into()].into_iter()).unwrap();
        assert_eq!(options.timing_seconds, Some(60));
        assert!(!options.smoke_test);
        assert!(
            parse_options(["--timing-test".into(), "60".into(), "--lan-only".into()].into_iter())
                .unwrap()
                .lan_only
        );
        assert!(parse_options(["--lan-only".into()].into_iter()).is_err());
        for seconds in ["0", "301", "invalid"] {
            assert!(parse_options(["--timing-test".into(), seconds.into()].into_iter()).is_err());
        }
        assert!(parse_options(["--timing-test".into()].into_iter()).is_err());
        for conflicting in ["--smoke-test", "--associate"] {
            assert!(
                parse_options(
                    ["--timing-test".into(), "60".into(), conflicting.into()].into_iter()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn migrates_legacy_settings_to_a_single_profile() {
        let directory = std::env::temp_dir().join(format!(
            "osmo-pair-migrate-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        fs::write(
            directory.join("wifi-network.txt"),
            "synthetic-ssid\nsynthetic-password\n",
        )
        .unwrap();
        fs::write(
            directory.join("identity.txt"),
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap();
        fs::write(directory.join("camera-ip.txt"), "192.0.2.10\n").unwrap();

        let (profile, files) = migrate_legacy_profile(&directory).unwrap();
        assert_eq!(profile.wifi_ssid.as_deref(), Some("synthetic-ssid"));
        assert_eq!(profile.wifi_password.as_deref(), Some("synthetic-password"));
        assert_eq!(profile.camera_ip, Some("192.0.2.10".parse().unwrap()));
        assert_eq!(files.len(), 3);
        profile.save_to_file(directory.join("camera.toml")).unwrap();
        remove_migrated_files(&files).unwrap();
        assert!(!directory.join("identity.txt").exists());
        assert!(!directory.join("wifi-network.txt").exists());
        assert!(CameraProfile::from_file(directory.join("camera.toml")).is_ok());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn formats_mac_without_secret_or_identity_data() {
        assert_eq!(format_mac([0x02, 0, 0, 0, 0, 0xab]), "02:00:00:00:00:AB");
    }
}
