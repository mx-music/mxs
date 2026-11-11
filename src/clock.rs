//! How the engine's clock, counted in frames by the sound card, lines up with the system clock, which runs at a slightly different speed.

/// How fast the offset between the clocks may rise between readings, in seconds a second: well above any sound card's drift, as crystals keep within about 100 parts per million.
const RISE: f64 = 1e-3;

/// The offset between the engine's seconds and the system's, read at each output callback.
#[derive(Clone, Debug)]
pub(crate) struct Timing {
    rate: f64,
    /// System seconds since the Unix epoch, less the engine's seconds, at the same instant.
    offset: Option<f64>,
    last: u64,
    /// Seconds from rendering a frame to the device playing it, as the device gives it.
    pub latency: Option<f64>,
}

impl Timing {
    pub fn new(rate: f64) -> Timing {
        Timing { rate, offset: None, last: 0, latency: None }
    }

    /// Records that `frame` was rendered at `system` seconds since the Unix epoch, to be played `latency` seconds later.
    // A callback reads the system clock late by however long it took to be called, never early, so the lowest offsets are the truest, and the offset follows them.
    pub fn observe(&mut self, frame: u64, system: f64, latency: Option<f64>) {
        let offset = system - frame as f64 / self.rate;
        self.offset = Some(match self.offset {
            None => offset,
            Some(old) => offset.min(old + RISE * frame.saturating_sub(self.last) as f64 / self.rate),
        });
        self.last = frame;
        self.latency = latency;
    }

    /// The system seconds at which the engine renders its `seconds`, once a callback has been seen.
    pub fn system(&self, seconds: f64) -> Option<f64> {
        self.offset.map(|offset| seconds + offset)
    }

    /// The engine's seconds at which it renders `system` seconds since the Unix epoch, once a callback has been seen.
    pub fn engine(&self, system: f64) -> Option<f64> {
        self.offset.map(|offset| system - offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sound card at `rate` whose frames run `drift` faster than the system clock, its callbacks seen with up to `jitter` seconds of delay.
    fn drifting(rate: f64, drift: f64, jitter: f64, hours: f64, mut each: impl FnMut(&Timing, f64, u64)) {
        let mut timing = Timing::new(rate);
        let started = 1_800_000_000.0;
        let buffer = 512;
        let mut seed = 1u64;
        let mut frame = 0;
        while (frame as f64) < hours * 3600.0 * rate {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
            let delay = (seed >> 11) as f64 / (1u64 << 53) as f64 * jitter;
            let truly = started + frame as f64 / (rate * (1.0 + drift));
            timing.observe(frame, truly + delay, Some(0.01));
            each(&timing, truly, frame);
            frame += buffer;
        }
    }

    #[test]
    fn over_two_hours_of_drift_either_way_a_system_time_maps_to_the_frame_rendered_then_within_1_ms() {
        let rate = 48_000.0;
        for drift in [100e-6, -100e-6] {
            let mut worst: f64 = 0.0;
            drifting(rate, drift, 0.002, 2.0, |timing, truly, frame| {
                if frame as f64 > rate * 5.0 {
                    let seconds = timing.engine(truly).unwrap();
                    worst = worst.max((seconds - frame as f64 / rate).abs());
                }
            });
            assert!(worst < 0.001, "{worst} s off at a drift of {drift}");
        }
    }

    #[test]
    fn mapping_once_at_the_start_would_be_off_by_the_drift() {
        let rate = 48_000.0;
        let mut first = None;
        let mut last = 0.0;
        drifting(rate, 50e-6, 0.0, 2.0, |timing, truly, frame| {
            let offset = *first.get_or_insert(timing.system(0.0).unwrap());
            last = (truly - offset - frame as f64 / rate).abs();
        });
        assert!(last > 0.3, "{last}");
    }

    #[test]
    fn the_system_time_of_a_moment_and_its_engine_seconds_are_one_mapping() {
        let mut timing = Timing::new(48_000.0);
        assert_eq!(timing.engine(1_800_000_000.0), None);
        timing.observe(48_000, 1_800_000_001.0, Some(0.012));
        assert_eq!(timing.system(1.0), Some(1_800_000_001.0));
        assert_eq!(timing.engine(1_800_000_003.5), Some(3.5));
        assert_eq!(timing.latency, Some(0.012));
    }
}
