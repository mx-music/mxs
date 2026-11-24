// The README links its guides and catalogs by file; the first definition of a link wins, so these point the same links at the modules here.
//! [architecture]: guide::architecture
//! [events]: guide::events
//! [signal_path]: guide::signal_path
//! [time]: guide::time
//! [reports]: guide::reports
//! [live]: guide::live
//! [building]: guide::building
//! [samples]: guide::samples
//! [synths]: synths
//! [effects]: effects
//!
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

#[cfg(test)]
mod allocations;
pub mod effects;
mod engine;
mod error;
pub mod graph;
pub mod guide;
pub mod keys;
pub mod nodes;
mod options;
mod ready;
mod spec;
mod synth;
pub mod synths;

pub use engine::device::{Device, Settings, devices};
pub use engine::load::Load;
pub use engine::report::{BANDS, FLOOR, Levels, PartLevels, Report, Reports, Topic, WAVE};
pub use engine::sample::check as check_sample;
pub use engine::sample::check_bytes as check_sample_bytes;
pub use engine::sample::peak as sample_peak;
pub use engine::sample::peak_bytes as sample_peak_bytes;
pub use engine::time::Tempo;
pub use engine::{Engine, Event, Origin, Pass, Source, Target, VOICES, voice_limit_for};
pub use error::Error;
pub use options::{Context, Options};
pub use spec::{Kind, Spec, Value};
pub use synth::{Effect, EffectDecl, Family, Note, Synth, SynthDecl};

/// The rate an offline engine is usually made with, in frames a second; a live engine runs at its device's.
pub const RATE: f64 = 48_000.0;

/// The vel a note plays at when it gives none, and the vel each synth's levels and brightness are fitted at.
pub const DEFAULT_VEL: f64 = 0.6;
