//! A voice built ready: the engine builds it before its note is played, and gives it the note, vel, hold and pedal as the note starts.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// What the engine gives a voice built ready as its note starts: the note and vel it plays, whether it is held, and its pedal, 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Given {
    pub note: f64,
    pub vel: f64,
    pub held: bool,
    pub pedal: f64,
    pub pressure: f64,
}

/// The hold the engine keeps on a voice built ready: what it is given, written before it starts, and how many seconds a note given that lasts and takes to fall silent once let go.
pub(crate) struct Ready {
    note: AtomicU64,
    vel: AtomicU64,
    held: AtomicBool,
    pedal: AtomicU64,
    pressure: AtomicU64,
    lasts: Box<dyn Fn(Given) -> (f64, f64) + Send + Sync>,
}

impl Ready {
    pub(crate) fn new(lasts: impl Fn(Given) -> (f64, f64) + Send + Sync + 'static) -> Arc<Ready> {
        Arc::new(Ready {
            note: AtomicU64::new(f64::NAN.to_bits()),
            vel: AtomicU64::new(0),
            held: AtomicBool::new(false),
            pedal: AtomicU64::new(0),
            pressure: AtomicU64::new(0),
            lasts: Box::new(lasts),
        })
    }

    /// Gives the voice what it plays, before the engine sends it on, and says how many seconds it lasts and takes to fall silent once let go.
    pub(crate) fn give(&self, given: Given) -> (f64, f64) {
        self.note.store(given.note.to_bits(), Ordering::Relaxed);
        self.vel.store(given.vel.to_bits(), Ordering::Relaxed);
        self.held.store(given.held, Ordering::Relaxed);
        self.pedal.store(given.pedal.to_bits(), Ordering::Relaxed);
        self.pressure
            .store(given.pressure.to_bits(), Ordering::Relaxed);
        (self.lasts)(given)
    }

    /// What was given, or `None` before anything is.
    pub(crate) fn given(&self) -> Option<Given> {
        let note = f64::from_bits(self.note.load(Ordering::Relaxed));
        (!note.is_nan()).then(|| Given {
            note,
            vel: f64::from_bits(self.vel.load(Ordering::Relaxed)),
            held: self.held.load(Ordering::Relaxed),
            pedal: f64::from_bits(self.pedal.load(Ordering::Relaxed)),
            pressure: f64::from_bits(self.pressure.load(Ordering::Relaxed)),
        })
    }
}
