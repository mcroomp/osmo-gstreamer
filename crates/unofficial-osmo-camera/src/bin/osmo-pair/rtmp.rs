use unofficial_osmo_camera::{
    CameraProfile, ControlSession, Error, PreviewCancellation, Result, RtmpConfig,
    protocol::commands::media::cmd_02_8e_streaming_control::MediaStreamingStateReply,
};

pub(super) enum Action {
    Setup(RtmpConfig),
    State,
    Stop,
}

trait RtmpControl {
    fn enable(&mut self) -> Result<()>;
    fn configure(&mut self, settings: &RtmpConfig) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn state(&mut self) -> Result<MediaStreamingStateReply>;
    fn stop(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;
}

impl RtmpControl for ControlSession {
    fn enable(&mut self) -> Result<()> {
        self.enable_rtmp(true)
    }

    fn configure(&mut self, settings: &RtmpConfig) -> Result<()> {
        self.configure_rtmp(settings, true)
    }

    fn start(&mut self) -> Result<()> {
        self.start_rtmp(true)
    }

    fn state(&mut self) -> Result<MediaStreamingStateReply> {
        self.rtmp_state()
    }

    fn stop(&mut self) -> Result<()> {
        self.stop_rtmp(true)
    }

    fn close(&mut self) -> Result<()> {
        ControlSession::close(self)
    }
}

pub(super) fn run(profile: &CameraProfile, action: &Action) -> Result<()> {
    let mut session = ControlSession::connect(
        super::camera_config(profile)?,
        &PreviewCancellation::default(),
    )?;
    execute(&mut session, action)
}

fn execute(session: &mut impl RtmpControl, action: &Action) -> Result<()> {
    let result = match action {
        Action::Setup(settings) => setup(session, settings),
        Action::State => state(session, "Current"),
        Action::Stop => {
            log::info!("Explicitly stopping RTMP and quitting broadcast mode");
            session.stop().inspect(|()| {
                println!("RTMP stop/quit confirmed; prior Wi-Fi/recording state is not restored.");
            })
        }
    };
    if let Err(error) = &result {
        log::error!("RTMP operation failed: {error}");
    }
    let closed = session.close();
    match (result, closed) {
        (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn setup(session: &mut impl RtmpControl, settings: &RtmpConfig) -> Result<()> {
    let result = (|| {
        log::info!("Entering RTMP broadcast mode; Wi-Fi role and control connectivity may change");
        session.enable()?;
        state(session, "Before configuration")?;
        log::info!("Configuring RTMP destination (redacted)");
        session.configure(settings)?;
        log::info!("Requesting RTMP publication");
        session.start()?;
        state(session, "After start")?;
        println!(
            "RTMP start accepted; receiver video is NOT verified. Publication is left running.\n\
             Use --rtmp-stop to stop/quit explicitly; it will pair and reapply saved Wi-Fi first."
        );
        Ok(())
    })();
    if let Err(primary) = result {
        log::error!("RTMP setup failed: {primary}; requesting stop/quit on the existing owner");
        return match session.stop() {
            Ok(()) => {
                log::info!("RTMP stop/quit confirmed after setup failure");
                Err(primary)
            }
            Err(cleanup) => {
                log::error!("RTMP stop/quit NOT confirmed; no reconnect attempted: {cleanup}");
                Err(Error::cleanup(primary, cleanup))
            }
        };
    }
    Ok(())
}

fn state(session: &mut impl RtmpControl, label: &str) -> Result<()> {
    let reply = session.state()?;
    let summary = state_summary(&reply);
    println!("{label} {summary}");
    log::info!("{label} {summary}");
    match reply {
        MediaStreamingStateReply::State(_) => Ok(()),
        MediaStreamingStateReply::Rejected(_) => {
            Err(Error::Protocol("camera rejected the RTMP state query"))
        }
    }
}

fn state_summary(reply: &MediaStreamingStateReply) -> String {
    match reply {
        MediaStreamingStateReply::State(bytes) => {
            let hex = bytes
                .iter()
                .map(|byte| format!("{byte:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("RTMP state (9 raw bytes, opaque): {hex}")
        }
        MediaStreamingStateReply::Rejected(bytes) => match bytes.first() {
            Some(status) => format!(
                "RTMP state rejected: status_byte=0x{status:02X}, reply_length={} (remaining payload omitted)",
                bytes.len()
            ),
            None => "RTMP state rejected: missing status byte".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unofficial_osmo_camera::StreamingResolution;

    #[derive(Default)]
    struct MockControl {
        calls: Vec<&'static str>,
        fail_at: Option<usize>,
        stop_fails: bool,
        close_fails: bool,
        reject_state: bool,
    }

    impl MockControl {
        fn record(&mut self, operation: &'static str) -> Result<()> {
            self.calls.push(operation);
            if self.fail_at == Some(self.calls.len()) {
                Err(Error::Protocol("synthetic operation failure"))
            } else {
                Ok(())
            }
        }
    }

    impl RtmpControl for MockControl {
        fn enable(&mut self) -> Result<()> {
            self.record("enable")
        }

        fn configure(&mut self, _: &RtmpConfig) -> Result<()> {
            self.record("configure")
        }

        fn start(&mut self) -> Result<()> {
            self.record("start")
        }

        fn state(&mut self) -> Result<MediaStreamingStateReply> {
            self.record("state")?;
            Ok(if self.reject_state {
                MediaStreamingStateReply::Rejected(vec![0xd6])
            } else {
                MediaStreamingStateReply::State([0; 9])
            })
        }

        fn stop(&mut self) -> Result<()> {
            self.record("stop")?;
            if self.stop_fails {
                Err(Error::WorkerTerminated)
            } else {
                Ok(())
            }
        }

        fn close(&mut self) -> Result<()> {
            self.record("close")?;
            if self.close_fails {
                Err(Error::WorkerTerminated)
            } else {
                Ok(())
            }
        }
    }

    fn setup_action() -> Action {
        Action::Setup(
            RtmpConfig::new(
                "rtmp://example.invalid/live/synthetic-key",
                StreamingResolution::Hd720,
                2000,
            )
            .unwrap(),
        )
    }

    #[test]
    fn successful_setup_leaves_publication_running_and_closes_control_once() {
        let mut owner = MockControl::default();
        execute(&mut owner, &setup_action()).unwrap();
        assert_eq!(
            owner.calls,
            ["enable", "state", "configure", "start", "state", "close"]
        );
    }

    #[test]
    fn every_setup_failure_stops_and_closes_without_retry_or_later_steps() {
        let steps = ["enable", "state", "configure", "start", "state"];
        for fail_at in 1..=steps.len() {
            let mut owner = MockControl {
                fail_at: Some(fail_at),
                ..Default::default()
            };
            assert!(execute(&mut owner, &setup_action()).is_err());
            let expected = [&steps[..fail_at], &["stop", "close"]].concat();
            assert_eq!(owner.calls, expected);
        }
    }

    #[test]
    fn state_rejections_abort_setup_and_preserve_cleanup_failures() {
        let mut owner = MockControl {
            reject_state: true,
            stop_fails: true,
            close_fails: true,
            ..Default::default()
        };
        let error = execute(&mut owner, &setup_action()).unwrap_err();
        assert_eq!(owner.calls, ["enable", "state", "stop", "close"]);
        let Error::Cleanup { primary, cleanup } = error else {
            panic!("expected close failure to accompany the setup result");
        };
        assert!(matches!(*primary, Error::Cleanup { .. }));
        assert!(matches!(*cleanup, Error::WorkerTerminated));
    }

    #[test]
    fn standalone_state_and_stop_never_enable_configure_or_start() {
        let mut owner = MockControl::default();
        execute(&mut owner, &Action::State).unwrap();
        assert_eq!(owner.calls, ["state", "close"]);
        owner.calls.clear();
        execute(&mut owner, &Action::Stop).unwrap();
        assert_eq!(owner.calls, ["stop", "close"]);
        owner.calls.clear();
        owner.reject_state = true;
        assert!(execute(&mut owner, &Action::State).is_err());
        assert_eq!(owner.calls, ["state", "close"]);
    }

    #[test]
    fn structured_state_reports_exactly_nine_hex_bytes_without_interpretation() {
        assert_eq!(
            state_summary(&MediaStreamingStateReply::State([
                0, 1, 2, 3, 4, 5, 6, 0xab, 0xff
            ])),
            "RTMP state (9 raw bytes, opaque): 00 01 02 03 04 05 06 AB FF"
        );
    }

    #[test]
    fn rejected_state_reports_status_without_dumping_unknown_payloads() {
        let mut bytes = vec![0xd6];
        bytes.extend_from_slice(b"synthetic-private-data");
        let text = state_summary(&MediaStreamingStateReply::Rejected(bytes));
        assert!(text.contains("status_byte=0xD6"));
        assert!(!text.contains("synthetic-private-data"));
        assert_eq!(
            state_summary(&MediaStreamingStateReply::Rejected(vec![])),
            "RTMP state rejected: missing status byte"
        );
    }
}
