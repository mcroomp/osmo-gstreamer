use crate::{gst, gst_base};
use gst::{glib, prelude::*, subclass::prelude::*};
use gst_base::{
    prelude::*,
    subclass::{base_src::CreateSuccess, prelude::*},
};
use osmo_camera::{CameraProfile, CameraSession, PreviewCancellation, PreviewReader};
use std::{
    path::PathBuf,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

const FPS_NUMERATOR: i32 = 30000;
const FPS_DENOMINATOR: i32 = 1001;
const CREDENTIALS_DIRECTORY: &str = ".credentials";
const DEFAULT_CAMERA_PROFILE: &str = "camera";

struct Settings {
    allow_writes: bool,
    framerate: gst::Fraction,
    camera_profile: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            allow_writes: false,
            framerate: gst::Fraction::new(FPS_NUMERATOR, FPS_DENOMINATOR),
            camera_profile: DEFAULT_CAMERA_PROFILE.to_owned(),
        }
    }
}
#[derive(Default)]
struct State {
    session: Option<CameraSession>,
    reader: Option<PreviewReader>,
    starting: Option<PreviewCancellation>,
    timing: Option<ArrivalTiming>,
    framerate: Option<gst::Fraction>,
}

mod imp {
    use super::*;
    #[derive(Default)]
    pub struct OsmoSrc {
        settings: Mutex<Settings>,
        state: Mutex<State>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for OsmoSrc {
        const NAME: &'static str = "GstOsmoSrc";
        type Type = super::OsmoSrc;
        type ParentType = gst_base::PushSrc;
    }
    impl ObjectImpl for OsmoSrc {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_live(true);
            obj.set_format(gst::Format::Time);
            obj.set_do_timestamp(false);
        }
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: LazyLock<Vec<glib::ParamSpec>> = LazyLock::new(|| {
                vec![
                glib::ParamSpecBoolean::builder("allow-writes").nick("Approve session/preview startup")
                    .blurb("Approves BLE wake and saved Wi-Fi association if LAN is unreachable, then preview startup; never arbitrary commands")
                    .default_value(false).mutable_ready().build(),
                gst::ParamSpecFraction::builder("framerate").nick("Nominal frame rate")
                    .blurb("Nominal caps and buffer duration; PTS/DTS preserve elapsed receiver arrival time, not camera capture time")
                    .minimum(gst::Fraction::new(1, 1)).maximum(gst::Fraction::new(120, 1))
                    .default_value(gst::Fraction::new(FPS_NUMERATOR, FPS_DENOMINATOR)).mutable_ready().build(),
                glib::ParamSpecString::builder("camera-profile").nick("Camera settings profile")
                    .blurb("Selects a profile in the gitignored .credentials directory")
                    .default_value(DEFAULT_CAMERA_PROFILE).mutable_ready().build(),
            ]
            });
            PROPERTIES.as_ref()
        }
        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let mut settings = self.settings.lock().expect("settings poisoned");
            match pspec.name() {
                "allow-writes" => settings.allow_writes = value.get().expect("property type"),
                "framerate" => settings.framerate = value.get().expect("property type"),
                "camera-profile" => {
                    settings.camera_profile = value
                        .get::<Option<String>>()
                        .expect("property type")
                        .unwrap_or_else(|| DEFAULT_CAMERA_PROFILE.to_owned());
                }
                _ => unreachable!("unknown source property"),
            }
        }
        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let settings = self.settings.lock().expect("settings poisoned");
            match pspec.name() {
                "allow-writes" => settings.allow_writes.to_value(),
                "framerate" => settings.framerate.to_value(),
                "camera-profile" => settings.camera_profile.to_value(),
                _ => unreachable!("unknown source property"),
            }
        }
    }
    impl GstObjectImpl for OsmoSrc {}
    impl ElementImpl for OsmoSrc {
        fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
            static METADATA: LazyLock<gst::subclass::ElementMetadata> = LazyLock::new(|| {
                gst::subclass::ElementMetadata::new(
                    "Osmo Nano LAN camera source",
                    "Source/Video",
                    "Independent Rust camera owner; Annex-B H.264 access units",
                    "osmo-gstreamer contributors",
                )
            });
            Some(&*METADATA)
        }
        fn pad_templates() -> &'static [gst::PadTemplate] {
            static TEMPLATES: LazyLock<Vec<gst::PadTemplate>> = LazyLock::new(|| {
                let caps = gst::Caps::builder("video/x-h264")
                    .field("stream-format", "byte-stream")
                    .field("alignment", "au")
                    .build();
                vec![
                    gst::PadTemplate::new(
                        "src",
                        gst::PadDirection::Src,
                        gst::PadPresence::Always,
                        &caps,
                    )
                    .expect("static source caps"),
                ]
            });
            TEMPLATES.as_ref()
        }
    }
    impl BaseSrcImpl for OsmoSrc {
        fn start(&self) -> Result<(), gst::ErrorMessage> {
            let (profile_name, framerate) = {
                let settings = self.settings.lock().expect("settings poisoned");
                if !settings.allow_writes {
                    return Err(gst::error_msg!(
                        gst::ResourceError::Settings,
                        ["Set allow-writes=true to explicitly approve camera preview startup"]
                    ));
                }
                (settings.camera_profile.clone(), settings.framerate)
            };
            let mut profile =
                load_camera_profile(&PathBuf::from(CREDENTIALS_DIRECTORY), &profile_name).map_err(
                    |error| {
                        log::error!("Could not load camera profile '{profile_name}': {error}");
                        gst::error_msg!(gst::ResourceError::Settings, ["{error}"])
                    },
                )?;
            log::info!("Starting the GStreamer camera source");
            let cancellation = PreviewCancellation::default();
            {
                let mut state = self.state.lock().expect("source state poisoned");
                if state.session.is_some() || state.starting.is_some() {
                    return Err(gst::error_msg!(
                        gst::ResourceError::Busy,
                        ["Camera source is already active"]
                    ));
                }
                state.starting = Some(cancellation.clone());
            }
            let session = profile.start_preview(true, &cancellation);
            let mut state = self.state.lock().expect("source state poisoned");
            state.starting = None;
            let session = session.map_err(startup_error)?;
            state.reader = Some(session.reader());
            state.session = Some(session);
            state.timing = None;
            state.framerate = Some(framerate);
            log::info!("GStreamer camera source preview is ready");
            Ok(())
        }
        fn stop(&self) -> Result<(), gst::ErrorMessage> {
            log::info!("Stopping the GStreamer camera source");
            let session = {
                let mut state = self.state.lock().expect("source state poisoned");
                if let Some(starting) = &state.starting {
                    starting.cancel();
                }
                if let Some(reader) = state.reader.take() {
                    reader.set_flushing(true);
                }
                state.timing = None;
                state.session.take()
            };
            if let Some(mut session) = session {
                session.close().map_err(|error| {
                    log::error!("Could not close the GStreamer camera source: {error}");
                    gst::error_msg!(gst::ResourceError::Close, ["{error}"])
                })?;
            }
            log::debug!("GStreamer camera source stopped");
            Ok(())
        }
        fn is_seekable(&self) -> bool {
            false
        }
        fn caps(&self, filter: Option<&gst::Caps>) -> Option<gst::Caps> {
            let framerate = self.settings.lock().expect("settings poisoned").framerate;
            let caps = gst::Caps::builder("video/x-h264")
                .field("stream-format", "byte-stream")
                .field("alignment", "au")
                .field("framerate", framerate)
                .build();
            Some(filter.map_or_else(|| caps.clone(), |filter| caps.intersect(filter)))
        }
        fn unlock(&self) -> Result<(), gst::ErrorMessage> {
            let state = self.state.lock().expect("source state poisoned");
            if let Some(starting) = &state.starting {
                starting.cancel();
            }
            if let Some(reader) = &state.reader {
                reader.set_flushing(true);
            }
            Ok(())
        }
        fn unlock_stop(&self) -> Result<(), gst::ErrorMessage> {
            if let Some(reader) = &self.state.lock().expect("source state poisoned").reader {
                reader.set_flushing(false);
            }
            Ok(())
        }
    }
    impl PushSrcImpl for OsmoSrc {
        fn create(
            &self,
            _buffer: Option<&mut gst::BufferRef>,
        ) -> Result<CreateSuccess, gst::FlowError> {
            let reader = self
                .state
                .lock()
                .expect("source state poisoned")
                .reader
                .clone()
                .ok_or(gst::FlowError::Flushing)?;
            let unit = match reader.receive() {
                Ok(Some(unit)) => unit,
                Ok(None) => return Err(gst::FlowError::Eos),
                Err(osmo_camera::Error::Cancelled) => return Err(gst::FlowError::Flushing),
                Err(error) => {
                    log::error!("GStreamer source could not read preview data: {error}");
                    gst::element_error!(self.obj(), gst::ResourceError::Read, ["{error}"]);
                    return Err(gst::FlowError::Error);
                }
            };
            log::debug!(
                "Delivering H.264 access unit: {} bytes, frame {}, keyframe={}",
                unit.bytes.len(),
                unit.frame_number,
                unit.keyframe
            );
            let mut state = self.state.lock().expect("source state poisoned");
            let framerate = state.framerate.ok_or(gst::FlowError::Flushing)?;
            if state.timing.is_none() {
                let running = self
                    .obj()
                    .current_running_time()
                    .ok_or(gst::FlowError::Flushing)?;
                let elapsed = gst::ClockTime::try_from(unit.received_at.elapsed())
                    .map_err(|_| gst::FlowError::Error)?;
                state.timing = Some(ArrivalTiming::new(
                    unit.received_at,
                    running.saturating_sub(elapsed),
                ));
            }
            let duration = timestamp_offset(1, framerate).ok_or(gst::FlowError::Error)?;
            let (pts, arrival_gap) = state
                .timing
                .as_mut()
                .expect("arrival timing initialized")
                .timestamp(unit.received_at, duration)
                .map_err(|reason| {
                    log::error!("Could not timestamp preview frame: {reason}");
                    gst::element_error!(self.obj(), gst::StreamError::Failed, ["{reason}"]);
                    gst::FlowError::Error
                })?;
            if arrival_gap {
                log::info!(
                    "Preserving preview arrival gap at frame {}; PTS={pts}",
                    unit.frame_number
                );
            }
            let mut buffer = gst::Buffer::from_mut_slice(unit.bytes);
            let writable = buffer.get_mut().expect("new buffer uniquely owned");
            writable.set_pts(pts);
            writable.set_dts(pts);
            writable.set_duration(duration);
            writable.set_offset(unit.frame_number);
            writable.set_offset_end(unit.frame_number + 1);
            if !unit.keyframe {
                writable.set_flags(gst::BufferFlags::DELTA_UNIT);
            }
            if unit.discontinuity || arrival_gap {
                writable.set_flags(gst::BufferFlags::DISCONT);
            }
            Ok(CreateSuccess::NewBuffer(buffer))
        }
    }
}

struct ArrivalTiming {
    origin: Instant,
    running_origin: gst::ClockTime,
    previous: Instant,
}

impl ArrivalTiming {
    fn new(origin: Instant, running_origin: gst::ClockTime) -> Self {
        Self {
            origin,
            running_origin,
            previous: origin,
        }
    }

    fn timestamp(
        &mut self,
        arrival: Instant,
        nominal_duration: gst::ClockTime,
    ) -> Result<(gst::ClockTime, bool), &'static str> {
        let gap = arrival
            .checked_duration_since(self.previous)
            .ok_or("receiver arrival time moved backwards")?;
        let elapsed = arrival
            .checked_duration_since(self.origin)
            .ok_or("receiver arrival time precedes the timeline origin")?;
        let offset = gst::ClockTime::try_from(elapsed)
            .map_err(|_| "receiver elapsed time exceeds GStreamer timestamp range")?;
        let pts = self
            .running_origin
            .checked_add(offset)
            .ok_or("preview timestamp overflow")?;
        // Avoid classifying the expected cadence of low-framerate streams as a pause.
        let threshold = Duration::from_millis(100)
            .max(Duration::from_nanos(nominal_duration.nseconds()).saturating_mul(3));
        self.previous = arrival;
        Ok((pts, gap >= threshold))
    }
}

fn load_camera_profile(
    credentials_dir: &std::path::Path,
    profile_name: &str,
) -> Result<CameraProfile, Box<dyn std::error::Error>> {
    CameraProfile::validate_name(profile_name)?;
    let path = credentials_dir.join(format!("{profile_name}.toml"));
    let profile = CameraProfile::from_file(&path)?;
    profile.camera_ip.ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} must define camera_ip", path.display()),
        )
    })?;
    profile.pairing_identity.as_ref().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{} must define pairing_identity", path.display()),
        )
    })?;
    Ok(profile)
}

fn startup_error(error: osmo_camera::Error) -> gst::ErrorMessage {
    log::error!("GStreamer camera source startup failed: {error}");
    match error {
        osmo_camera::Error::InvalidConfig { .. } => {
            gst::error_msg!(gst::ResourceError::Settings, ["{error}"])
        }
        osmo_camera::Error::Ownership { .. } => {
            gst::error_msg!(gst::ResourceError::Busy, ["{error}"])
        }
        _ => gst::error_msg!(gst::ResourceError::OpenRead, ["{error}"]),
    }
}

fn timestamp_offset(frame: u64, rate: gst::Fraction) -> Option<gst::ClockTime> {
    let ns = u128::from(frame)
        * u128::from(gst::ClockTime::SECOND.nseconds())
        * u128::try_from(rate.denom()).ok()?
        / u128::try_from(rate.numer()).ok()?;
    u64::try_from(ns)
        .ok()
        .filter(|ns| *ns != u64::MAX)
        .map(gst::ClockTime::from_nseconds)
}

glib::wrapper! {
    pub struct OsmoSrc(ObjectSubclass<imp::OsmoSrc>)
        @extends gst_base::PushSrc, gst_base::BaseSrc, gst::Element, gst::Object;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "osmo-gst-credentials-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn rational_timestamps_have_no_accumulated_rounding() {
        let fps = gst::Fraction::new(FPS_NUMERATOR, FPS_DENOMINATOR);
        assert_eq!(
            timestamp_offset(30000, fps).unwrap().nseconds(),
            1001 * gst::ClockTime::SECOND.nseconds()
        );
        assert!(timestamp_offset(u64::MAX, fps).is_none());
    }

    #[test]
    fn arrival_timestamps_preserve_repeated_camera_pauses() {
        let origin = Instant::now();
        let running_origin = gst::ClockTime::from_seconds(2);
        let duration =
            timestamp_offset(1, gst::Fraction::new(FPS_NUMERATOR, FPS_DENOMINATOR)).unwrap();
        let mut timing = ArrivalTiming::new(origin, running_origin);
        let mut elapsed = Duration::ZERO;
        for frame in 0..1800 {
            let pause = frame > 0 && frame % 300 == 0;
            if frame > 0 {
                elapsed += if pause {
                    Duration::from_millis(950)
                } else {
                    Duration::from_nanos(duration.nseconds())
                };
            }
            let (pts, discontinuity) = timing.timestamp(origin + elapsed, duration).unwrap();
            assert_eq!(
                pts,
                running_origin + gst::ClockTime::try_from(elapsed).unwrap()
            );
            assert_eq!(discontinuity, pause);
        }
    }

    #[test]
    fn arrival_timestamps_handle_bursts_low_cadence_and_invalid_times() {
        let origin = Instant::now();
        let duration = gst::ClockTime::from_mseconds(500);
        let mut timing = ArrivalTiming::new(origin, gst::ClockTime::ZERO);
        assert_eq!(
            timing
                .timestamp(origin + Duration::from_millis(500), duration)
                .unwrap(),
            (gst::ClockTime::from_mseconds(500), false)
        );
        assert_eq!(
            timing
                .timestamp(origin + Duration::from_millis(501), duration)
                .unwrap(),
            (gst::ClockTime::from_mseconds(501), false)
        );
        assert!(timing.timestamp(origin, duration).is_err());
        let mut overflow = ArrivalTiming::new(origin, gst::ClockTime::from_nseconds(u64::MAX - 1));
        assert!(
            overflow
                .timestamp(origin + Duration::from_secs(1), duration)
                .is_err()
        );
    }

    #[test]
    fn camera_config_loads_from_a_single_profile_file() {
        let directory = temp_dir();
        std::fs::create_dir(&directory).unwrap();
        let mut profile = CameraProfile::default();
        profile.camera_ip = Some("192.0.2.10".parse().unwrap());
        profile.pairing_identity = Some("0123456789abcdef0123456789abcdef".into());
        profile.save_to_file(directory.join("camera.toml")).unwrap();
        assert_eq!(
            load_camera_profile(&directory, "camera").unwrap().camera_ip,
            Some("192.0.2.10".parse::<IpAddr>().unwrap()),
        );
        assert!(load_camera_profile(&directory, "../outside").is_err());
        std::fs::write(
            directory.join("camera.toml"),
            "camera_ip = \"not-an-address\"\n",
        )
        .unwrap();
        let error = match load_camera_profile(&directory, "camera") {
            Ok(_) => panic!("invalid IP address should prevent profile loading"),
            Err(error) => error,
        };
        assert_eq!(
            error.downcast::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::InvalidData
        );
        profile.camera_ip = None;
        profile.save_to_file(directory.join("camera.toml")).unwrap();
        assert!(matches!(
            load_camera_profile(&directory, "camera"),
            Err(error) if error.to_string().contains("camera_ip")
        ));
        profile.camera_ip = Some("192.0.2.10".parse().unwrap());
        profile.pairing_identity = None;
        profile.save_to_file(directory.join("camera.toml")).unwrap();
        assert!(matches!(
            load_camera_profile(&directory, "camera"),
            Err(error) if error.to_string().contains("pairing_identity")
        ));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_is_live_and_refuses_unapproved_startup() {
        gst::init().unwrap();
        let source: OsmoSrc = glib::Object::new();
        assert!(source.is_live());
        assert!(!source.property::<bool>("allow-writes"));
        assert!(source.set_state(gst::State::Paused).is_err());
        source.set_state(gst::State::Null).unwrap();
    }

    #[test]
    fn typed_startup_errors_use_standard_gstreamer_resource_codes() {
        gst::init().unwrap();
        let pipeline = gst::Pipeline::new();
        let source: OsmoSrc = glib::Object::new();
        pipeline.add(&source).unwrap();
        let bus = pipeline.bus().unwrap();
        for (error, expected) in [
            (
                osmo_camera::Error::InvalidConfig {
                    field: "allow-writes",
                    reason: "explicit approval required",
                },
                gst::ResourceError::Settings,
            ),
            (
                osmo_camera::Error::Ownership {
                    source: std::sync::Arc::new(std::io::ErrorKind::WouldBlock.into()),
                },
                gst::ResourceError::Busy,
            ),
            (
                osmo_camera::Error::io(
                    "connect to the camera",
                    std::io::ErrorKind::ConnectionRefused.into(),
                ),
                gst::ResourceError::OpenRead,
            ),
        ] {
            source.post_error_message(startup_error(error));
            let message = bus.pop_filtered(&[gst::MessageType::Error]).unwrap();
            let gst::MessageView::Error(message) = message.view() else {
                panic!("Expected an error message");
            };
            assert!(message.error().matches(expected));
            assert!(!message.error().message().is_empty());
        }
    }
}
