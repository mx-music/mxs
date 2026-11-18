//! How long the engine takes to fill each buffer, against how long the buffer lasts.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// How filling audio went since the engine was last asked.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    /// How many buffers were filled.
    pub buffers: u64,
    /// The time filling took as a share of the time the buffers last, over all of them; 0.25 is a quarter.
    pub mean: f64,
    /// The largest such share of any one buffer.
    pub worst: f64,
    /// How many buffers of a live engine took longer to fill than they last, each heard as a gap; an offline engine has no deadline and counts none.
    pub late: u64,
    /// By how many seconds the latest of them overran.
    pub most_over: f64,
}

/// The counts behind `Load`, written by whoever fills audio and read by the engine, without a lock or an allocation.
#[derive(Default)]
pub(crate) struct Meter {
    buffers: AtomicU64,
    took: AtomicU64,
    lasted: AtomicU64,
    worst: AtomicU64,
    late: AtomicU64,
    most_over: AtomicU64,
    warn_late: AtomicU64,
    warn_most_over: AtomicU64,
}

impl Meter {
    pub fn new() -> Arc<Meter> {
        Arc::new(Meter::default())
    }

    /// Records that a buffer of `frames` at `rate` took `took` to fill; `deadline` says a sound card was waiting for it.
    pub fn record(&self, took: Duration, frames: usize, rate: f64, deadline: bool) {
        if frames == 0 {
            return;
        }
        let took = took.as_nanos() as u64;
        let lasted = (frames as f64 / rate * 1e9) as u64;
        self.buffers.fetch_add(1, Ordering::Relaxed);
        self.took.fetch_add(took, Ordering::Relaxed);
        self.lasted.fetch_add(lasted, Ordering::Relaxed);
        let share = (took as f64 / lasted.max(1) as f64 * 1e6) as u64;
        self.worst.fetch_max(share, Ordering::Relaxed);
        if deadline && took > lasted {
            let over = took - lasted;
            self.late.fetch_add(1, Ordering::Relaxed);
            self.most_over.fetch_max(over, Ordering::Relaxed);
            self.warn_late.fetch_add(1, Ordering::Relaxed);
            self.warn_most_over.fetch_max(over, Ordering::Relaxed);
        }
    }

    /// The load since this was last called.
    pub fn take(&self) -> Load {
        let took = self.took.swap(0, Ordering::Relaxed);
        let lasted = self.lasted.swap(0, Ordering::Relaxed);
        Load {
            buffers: self.buffers.swap(0, Ordering::Relaxed),
            mean: if lasted == 0 {
                0.0
            } else {
                took as f64 / lasted as f64
            },
            worst: self.worst.swap(0, Ordering::Relaxed) as f64 / 1e6,
            late: self.late.swap(0, Ordering::Relaxed),
            most_over: self.most_over.swap(0, Ordering::Relaxed) as f64 / 1e9,
        }
    }

    /// How many buffers were late since this was last called, and by up to how many seconds, for the warnings.
    pub fn take_late(&self) -> (u64, f64) {
        (
            self.warn_late.swap(0, Ordering::Relaxed),
            self.warn_most_over.swap(0, Ordering::Relaxed) as f64 / 1e9,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn a_buffer_that_took_longer_than_it_lasts_is_late_by_the_difference() {
        let meter = Meter::new();
        // 512 frames at 48 kHz last 10.67 ms.
        meter.record(2 * MS, 512, 48_000.0, true);
        meter.record(20 * MS, 512, 48_000.0, true);
        meter.record(MS, 512, 48_000.0, true);
        let load = meter.take();
        assert_eq!((load.buffers, load.late), (3, 1));
        assert!((load.most_over - 0.009333).abs() < 1e-5);
        assert!((load.mean - 23.0 / 32.0).abs() < 1e-3);
        assert!((load.worst - 1.875).abs() < 1e-3);
        let (late, over) = meter.take_late();
        assert_eq!(late, 1);
        assert!((over - 0.009333).abs() < 1e-5);
    }

    #[test]
    fn asking_starts_the_counts_again_and_the_warnings_keep_their_own() {
        let meter = Meter::new();
        meter.record(20 * MS, 512, 48_000.0, true);
        assert_eq!(meter.take().late, 1);
        assert_eq!(meter.take(), Load::default());
        assert_eq!(meter.take_late().0, 1);
        assert_eq!(meter.take_late(), (0, 0.0));
    }

    #[test]
    fn a_buffer_with_no_deadline_is_never_late() {
        let meter = Meter::new();
        meter.record(20 * MS, 512, 48_000.0, false);
        let load = meter.take();
        assert_eq!(load.late, 0);
        assert!((load.worst - 1.875).abs() < 1e-3);
        assert_eq!(meter.take_late(), (0, 0.0));
    }
}
