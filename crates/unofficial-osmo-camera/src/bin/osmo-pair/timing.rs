use std::time::{Duration, Instant};
use unofficial_osmo_camera::{AccessUnit, CameraSession, Error, Result, preview::nals};

const POLL: Duration = Duration::from_millis(100);
const GAP_WARNING: Duration = Duration::from_millis(100);

#[derive(Default)]
struct Timing {
    first: Option<(Instant, u64)>,
    previous: Option<(Instant, u64)>,
    frames: u64,
    gaps: u64,
    max_gap: Duration,
    drift_ms: f64,
    parameter_changes: u64,
    sps: Option<Vec<u8>>,
    pps: Option<Vec<u8>>,
}

impl Timing {
    fn observe(&mut self, unit: &AccessUnit) {
        let first = *self
            .first
            .get_or_insert((unit.received_at, unit.frame_number));
        let gap = self.previous.map_or(Duration::ZERO, |(at, _)| {
            unit.received_at.saturating_duration_since(at)
        });
        self.max_gap = self.max_gap.max(gap);
        if gap >= GAP_WARNING {
            self.gaps += 1;
            log::warn!(
                "Timing: arrival gap {:.3} ms at local frame {}",
                gap.as_secs_f64() * 1000.0,
                unit.frame_number
            );
        }
        if let Some((at, number)) = self.previous
            && (unit.received_at < at || number.checked_add(1) != Some(unit.frame_number))
        {
            log::warn!(
                "Timing: local frame/time continuity changed: previous={number}, current={}",
                unit.frame_number
            );
        }
        for nal in nals(&unit.bytes) {
            let cached = match nal.bytes[0] & 0x1f {
                7 => &mut self.sps,
                8 => &mut self.pps,
                _ => continue,
            };
            if cached.as_deref().is_some_and(|old| old != nal.bytes) {
                self.parameter_changes += 1;
                log::warn!(
                    "Timing: H.264 parameter set changed at local frame {}",
                    unit.frame_number
                );
            }
            *cached = Some(nal.bytes.to_vec());
        }
        if unit.keyframe || unit.discontinuity {
            log::info!(
                "Timing: local frame {}, keyframe={}, discontinuity={}",
                unit.frame_number,
                unit.keyframe,
                unit.discontinuity
            );
        }
        let actual = unit
            .received_at
            .saturating_duration_since(first.0)
            .as_secs_f64();
        let nominal = unit.frame_number.saturating_sub(first.1) as f64 * 1001.0 / 30000.0;
        self.drift_ms = (actual - nominal) * 1000.0;
        log::debug!(
            "Timing frame={}: arrival_ms={:.3}, gap_ms={:.3}, nominal_drift_ms={:.3}, queue_age_ms={:.3}, bytes={}",
            unit.frame_number,
            actual * 1000.0,
            gap.as_secs_f64() * 1000.0,
            self.drift_ms,
            unit.received_at.elapsed().as_secs_f64() * 1000.0,
            unit.bytes.len()
        );
        self.previous = Some((unit.received_at, unit.frame_number));
        self.frames += 1;
    }

    fn report(&self, elapsed: Duration) {
        log::info!(
            "Timing summary: elapsed={:.1}s, frames={}, average_fps={:.2}, gaps_100ms={}, max_gap_ms={:.3}, nominal_drift_ms={:.3}, parameter_changes={}",
            elapsed.as_secs_f64(),
            self.frames,
            self.frames as f64 / elapsed.as_secs_f64().max(f64::EPSILON),
            self.gaps,
            self.max_gap.as_secs_f64() * 1000.0,
            self.drift_ms,
            self.parameter_changes
        );
    }
}

pub(super) fn run(mut session: CameraSession, seconds: u64) -> Result<()> {
    log::info!(
        "Starting {seconds}s timing diagnostic on the initialized preview: no recording or automatic restart"
    );
    log::info!(
        "Flip the camera during the test. Local frame numbers are receiver counters, not camera counters; gaps/keyframes alone do not prove a stream restart."
    );
    let reader = session.reader();
    let start = Instant::now();
    let duration = Duration::from_secs(seconds);
    let mut timing = Timing::default();
    let mut report_at = start + Duration::from_secs(1);
    let mut silence_report_at = report_at;
    let result = (|| {
        while start.elapsed() < duration {
            match reader.receive_timeout(POLL) {
                Ok(Some(unit)) => {
                    timing.observe(&unit);
                    silence_report_at = Instant::now() + Duration::from_secs(1);
                }
                Ok(None) => return Err(Error::InvalidState("preview ended during timing test")),
                Err(Error::Timeout {
                    operation: "a live preview access unit",
                }) => {
                    if Instant::now() >= silence_report_at {
                        log::warn!("Timing: no access unit received for at least one second");
                        silence_report_at = Instant::now() + Duration::from_secs(1);
                    }
                }
                Err(error) => return Err(error),
            }
            if Instant::now() >= report_at {
                timing.report(start.elapsed());
                report_at = Instant::now() + Duration::from_secs(1);
            }
        }
        if timing.frames == 0 {
            return Err(Error::Timeout {
                operation: "video during the timing test",
            });
        }
        Ok(())
    })();
    timing.report(start.elapsed());
    let cleanup = session.close();
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(primary), Err(cleanup)) => Err(Error::cleanup(primary, cleanup)),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_arrival_gaps_independently_of_frame_numbers() {
        let at = Instant::now();
        let mut unit = AccessUnit {
            bytes: vec![],
            frame_number: 1,
            received_at: at,
            camera_metadata: 0,
            keyframe: true,
            discontinuity: true,
        };
        let mut timing = Timing::default();
        timing.observe(&unit);
        unit.frame_number = 2;
        unit.received_at = at + Duration::from_millis(33);
        timing.observe(&unit);
        unit.frame_number = 3;
        unit.received_at = at + Duration::from_millis(533);
        timing.observe(&unit);
        assert_eq!(timing.frames, 3);
        assert_eq!(timing.gaps, 1);
        assert_eq!(timing.max_gap, Duration::from_millis(500));
        assert!((timing.drift_ms - (533.0 - 2.0 * 1001.0 / 30.0)).abs() < 0.001);
    }

    #[test]
    fn detects_parameter_changes_but_not_repeated_headers() {
        let mut timing = Timing::default();
        let mut unit = AccessUnit {
            bytes: vec![0, 0, 1, 0x67, 1, 0, 0, 1, 0x68, 2],
            frame_number: 1,
            received_at: Instant::now(),
            camera_metadata: 0,
            keyframe: true,
            discontinuity: false,
        };
        timing.observe(&unit);
        unit.frame_number += 1;
        timing.observe(&unit);
        assert_eq!(timing.parameter_changes, 0);
        unit.bytes[4] = 3;
        unit.frame_number += 1;
        timing.observe(&unit);
        assert_eq!(timing.parameter_changes, 1);
    }
}
