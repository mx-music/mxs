//! mx's sound engine on FunDSP: synths and effects registered by name, played from events stamped with seconds, live on the default output or rendered offline.
//! mx's own synths are in `synths`, built only from what this crate makes public, as a synth of someone else's would be.

mod clock;
mod device;
mod engine;
pub mod graph;
mod mixer;
pub mod effects;
pub mod keys;
pub mod nodes;
mod options;
mod sample;
mod spec;
#[cfg(test)]
mod spike_parallel;
mod synth;
pub mod synths;

pub use device::{Device, Settings, devices};
pub use sample::check as check_sample;
pub use sample::peak as sample_peak;
pub use engine::{Engine, Error, Event, Source, Target, VOICES};
pub use options::{Context, Options};
pub use spec::{Kind, Spec, Value};
pub use synth::{Effect, EffectDecl, Note, Synth, SynthDecl};

/// The rate mx plays and renders at.
pub const RATE: f64 = 48_000.0;
