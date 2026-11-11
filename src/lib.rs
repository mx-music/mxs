//! mx's sound engine on FunDSP: synths and effects registered by name, played from events stamped with seconds, live on the default output or rendered offline.
//! mx's own synths are in `synths`, built only from what this crate makes public, as a synth of someone else's would be.

mod engine;
mod mixer;
pub mod nodes;
mod options;
mod sample;
mod spec;
mod synth;
pub mod synths;

pub use engine::{Engine, Error, Event, Source, Target};
pub use options::{Context, Options};
pub use spec::{Kind, Spec, Value};
pub use synth::{Effect, EffectDecl, Note, Synth, SynthDecl};

/// The rate mx plays and renders at.
pub const RATE: f64 = 48_000.0;
