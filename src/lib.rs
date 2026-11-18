#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod clock;
mod device;
pub mod effects;
mod engine;
pub mod graph;
pub mod keys;
mod load;
mod mixer;
pub mod nodes;
mod options;
mod sample;
mod spec;
mod synth;
pub mod synths;

pub use device::{Device, Settings, devices};
pub use engine::{Engine, Error, Event, Source, Target, VOICES};
pub use load::Load;
pub use options::{Context, Options};
pub use sample::check as check_sample;
pub use sample::check_bytes as check_sample_bytes;
pub use sample::peak as sample_peak;
pub use sample::peak_bytes as sample_peak_bytes;
pub use spec::{Kind, Spec, Value};
pub use synth::{Effect, EffectDecl, Note, Synth, SynthDecl};

/// The rate mx plays and renders at.
pub const RATE: f64 = 48_000.0;
