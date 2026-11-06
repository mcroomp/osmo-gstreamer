use std::{
    error::Error,
    io::Write,
    process::{Command as Process, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use unofficial_osmo_camera::protocol::encode;
use unofficial_osmo_camera::{
    AccessUnit, CameraProfile, CameraSession, Error as CameraError,
    protocol::{
        ExplorationRequest, Flags,
        commands::camera::{
            cmd_02_1a_live_view_quality_set::{LiveViewQuality, LiveViewQualitySet},
            cmd_02_4c_live_view_output_set::{LiveViewStreamFormat, LiveViewStreamOutputSet},
        },
        identity::{CameraCommand, Command, Endpoint},
    },
};

const SAMPLE: Duration = Duration::from_secs(8);
const SETTLE: Duration = Duration::from_secs(2);
const MAX_PROBE_BYTES: usize = 4 * 1024 * 1024;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
struct Measurements {
    start: Option<Instant>,
    first: Option<Instant>,
    last: Option<Instant>,
    frames: u64,
    bytes: u64,
    gaps: u64,
    probe: Vec<u8>,
}

impl Measurements {
    fn observe(&mut self, unit: AccessUnit) -> unofficial_osmo_camera::Result<()> {
        if self.start.is_none_or(|start| unit.received_at < start) {
            return Ok(());
        }
        if self.last.is_some_and(|last| {
            unit.received_at.saturating_duration_since(last) >= Duration::from_millis(100)
        }) {
            self.gaps += 1;
        }
        self.first.get_or_insert(unit.received_at);
        self.last = Some(unit.received_at);
        self.frames += 1;
        self.bytes += unit.bytes.len() as u64;
        if self.probe.is_empty() && unit.keyframe {
            if unit.bytes.len() > MAX_PROBE_BYTES {
                return Err(CameraError::LimitExceeded(
                    "diagnostic keyframe exceeds 4 MiB",
                ));
            }
            self.probe = unit.bytes;
        }
        Ok(())
    }

    fn fps(&self) -> Result<f64> {
        let elapsed = self
            .last
            .zip(self.first)
            .map(|(last, first)| last.saturating_duration_since(first).as_secs_f64())
            .filter(|elapsed| *elapsed > 0.0)
            .ok_or("not enough frames to measure delivered FPS")?;
        Ok(self.frames.saturating_sub(1) as f64 / elapsed)
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.len() == 2 || args.len() == 3 && args[2] == "--remaining-output-only")
        || args[1] != "--approve-experimental-writes"
    {
        return Err("Usage: preview-configs PROFILE --approve-experimental-writes [--remaining-output-only]".into());
    }
    let remaining_only = args.len() == 3;
    CameraProfile::validate_name(&args[0])?;
    let available = Process::new("ffprobe")
        .arg("-version")
        .stdout(Stdio::null())
        .status()?;
    if !available.success() {
        return Err("ffprobe is not available".into());
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .try_init()?;
    eprintln!(
        "EXPERIMENT: request flags 0x40, not a verified write contract. Reply bytes are NOT interpreted as acceptance."
    );
    if remaining_only {
        eprintln!(
            "Remaining-output-only: baseline, SdHf, HdHf. No quality, Auto or retry writes; original settings cannot be restored."
        );
    } else {
        eprintln!(
            "Original settings cannot be read back. Final requested settings: Normal/Auto, NOT restoration of originals."
        );
    }
    eprintln!("One owner; no LCD/recording writes; no video files; stop on transport errors.");
    let path = std::path::Path::new(".credentials").join(format!("{}.toml", args[0]));
    let mut profile = CameraProfile::from_file(&path)?;
    let mut session = profile.start_preview(true, &Default::default())?;
    if let Err(error) = profile.save_to_file(&path) {
        let cleanup = session.close();
        return match cleanup {
            Ok(()) => Err(error.into()),
            Err(cleanup) => {
                Err(format!("save initialized profile: {error}; cleanup: {cleanup}").into())
            }
        };
    }
    let reader = session.reader();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let data = Arc::new(Mutex::new(Measurements::default()));
    let worker_data = data.clone();
    let drain = thread::spawn(move || -> unofficial_osmo_camera::Result<()> {
        while !worker_stop.load(Ordering::Acquire) {
            match reader.receive_timeout(Duration::from_millis(100)) {
                Ok(Some(unit)) => worker_data
                    .lock()
                    .map_err(|_| CameraError::InvalidState("diagnostic measurements poisoned"))?
                    .observe(unit)?,
                Ok(None) => {
                    return Err(CameraError::InvalidState("preview ended during experiment"));
                }
                Err(CameraError::Timeout {
                    operation: "a live preview access unit",
                }) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    });
    let result = if remaining_only {
        remaining_trials(&mut session, &data)
    } else {
        trials(&mut session, &data)
    };
    stop.store(true, Ordering::Release);
    let drained: Result<()> = match drain.join() {
        Ok(result) => result.map_err(Into::into),
        Err(_) => Err("diagnostic drain thread panicked".into()),
    };
    let cleanup = session.close();
    if let Err(error) = &result {
        eprintln!("EXPERIMENT FAILED: {error}");
    }
    if let Err(error) = &drained {
        eprintln!("DRAIN FAILED: {error}");
    }
    if let Err(error) = &cleanup {
        eprintln!("SESSION CLOSE REPORTED: {error}");
    }
    result?;
    drained?;
    cleanup?;
    Ok(())
}

fn remaining_trials(session: &mut CameraSession, data: &Arc<Mutex<Measurements>>) -> Result<()> {
    measure("new baseline; current settings unknown", data)?;
    for format in [LiveViewStreamFormat::SdHf, LiveViewStreamFormat::HdHf] {
        apply(
            session,
            CameraCommand::LiveViewOutputSet,
            encode(&LiveViewStreamOutputSet { format })?,
        )?;
        measure(
            &format!("quality unchanged; output={format:?} requested"),
            data,
        )?;
    }
    Ok(())
}

fn trials(session: &mut CameraSession, data: &Arc<Mutex<Measurements>>) -> Result<()> {
    measure("baseline; original settings unknown", data)?;
    for quality in [
        LiveViewQuality::Normal,
        LiveViewQuality::Fine,
        LiveViewQuality::SuperFine,
    ] {
        apply(
            session,
            CameraCommand::LiveViewQualitySet,
            encode(&LiveViewQualitySet { quality })?,
        )?;
        measure(
            &format!("quality={quality:?}; original output unknown"),
            data,
        )?;
    }
    apply(
        session,
        CameraCommand::LiveViewQualitySet,
        encode(&LiveViewQualitySet {
            quality: LiveViewQuality::Normal,
        })?,
    )?;
    for format in [
        LiveViewStreamFormat::Auto,
        LiveViewStreamFormat::SdSf,
        LiveViewStreamFormat::HdSf,
        LiveViewStreamFormat::SdHf,
        LiveViewStreamFormat::HdHf,
    ] {
        apply(
            session,
            CameraCommand::LiveViewOutputSet,
            encode(&LiveViewStreamOutputSet { format })?,
        )?;
        measure(
            &format!("quality=Normal requested; output={format:?}"),
            data,
        )?;
    }
    apply(
        session,
        CameraCommand::LiveViewOutputSet,
        encode(&LiveViewStreamOutputSet {
            format: LiveViewStreamFormat::Auto,
        })?,
    )?;
    measure(
        "final Normal/Auto requested; not verified restoration",
        data,
    )
}

fn apply(session: &mut CameraSession, command: CameraCommand, payload: Vec<u8>) -> Result<()> {
    println!(
        "REQUEST command={command:?} flags=0x40 payload_bytes={}",
        payload.len()
    );
    let reply = session.request_raw(
        ExplorationRequest {
            receiver: Endpoint::Camera,
            command: Command::Camera(command),
            flags: Flags::REQUEST,
            payload,
            timeout: Duration::from_secs(3),
        },
        true,
        true,
    )?;
    println!(
        "REPLY command={command:?} flags={:#04x} payload_bytes={} first_byte={:?} (uninterpreted)",
        reply.flags.0,
        reply.payload.len(),
        reply.payload.first()
    );
    Ok(())
}

fn measure(label: &str, data: &Arc<Mutex<Measurements>>) -> Result<()> {
    thread::sleep(SETTLE);
    *data
        .lock()
        .map_err(|_| "diagnostic measurements poisoned")? = Measurements {
        start: Some(Instant::now()),
        ..Default::default()
    };
    thread::sleep(SAMPLE);
    let measured = std::mem::take(
        &mut *data
            .lock()
            .map_err(|_| "diagnostic measurements poisoned")?,
    );
    let fps = measured.fps()?;
    if measured.probe.is_empty() {
        return Err("no keyframe available for resolution inspection".into());
    }
    let resolution = probe(&measured.probe)?;
    println!(
        "RESULT {label}: {resolution}; delivered_fps={fps:.3}; frames={}; Mbps={:.3}; gaps_100ms={}",
        measured.frames,
        measured.bytes as f64 * 8.0 / SAMPLE.as_secs_f64() / 1e6,
        measured.gaps
    );
    Ok(())
}

fn probe(bytes: &[u8]) -> Result<String> {
    let mut child = Process::new("ffprobe")
        .args([
            "-v",
            "fatal",
            "-f",
            "h264",
            "-i",
            "pipe:0",
            "-show_entries",
            "stream=codec_name,width,height",
            "-of",
            "csv=p=0",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let write = child
        .stdin
        .take()
        .ok_or("missing ffprobe stdin")?
        .write_all(bytes);
    if let Err(error) = write {
        child.kill()?;
        child.wait()?;
        return Err(error.into());
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            return Err("ffprobe timed out".into());
        }
        thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let output = String::from_utf8(output.stdout)?;
    let fields: Vec<_> = output.trim().split(',').collect();
    if fields.len() != 3
        || fields[0] != "h264"
        || fields[1].parse::<u32>()? == 0
        || fields[2].parse::<u32>()? == 0
    {
        return Err("ffprobe did not identify a valid H.264 resolution".into());
    }
    Ok(format!("{}x{}", fields[1], fields[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{decode, encode};

    #[test]
    fn delivered_fps_excludes_startup_and_does_not_use_nominal_caps() {
        let start = Instant::now();
        let mut measured = Measurements {
            start: Some(start),
            ..Default::default()
        };
        for index in 0..25 {
            measured
                .observe(AccessUnit {
                    bytes: vec![0; 10],
                    frame_number: index,
                    received_at: start
                        + Duration::from_secs(2)
                        + Duration::from_nanos(index * 1_000_000_000 / 24),
                    camera_metadata: 0,
                    keyframe: false,
                    discontinuity: false,
                })
                .unwrap();
        }
        assert!((measured.fps().unwrap() - 24.0).abs() < 0.001);
        assert_eq!(measured.bytes, 250);
        assert_eq!(measured.gaps, 0);
    }
}
