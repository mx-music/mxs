//! mx's own synths, by family, each with the options it takes.
//!
//! An engine plays the synths registered on it, by name, and checks each note's options against what the synth declares:
//!
//! ```
//! use mxs::{Engine, Event, RATE, synths};
//!
//! fn main() -> Result<(), mxs::Error> {
//!     let mut engine = Engine::offline(RATE);
//!     synths::all().into_iter().for_each(|s| engine.register(s));
//!
//!     let note = Event::note(None, "marimba", &[("note", 60.0), ("vel", 0.8)]);
//!     engine.play(&[(0.0, note)])?;
//!     engine.render(2.0)?;
//!
//!     let wrong = Event::note(None, "marimba", &[("wobble", 1.0)]);
//!     assert!(engine.play(&[(0.0, wrong)]).is_err());
//!     Ok(())
//! }
//! ```
//!
//! A time is in seconds.
//! A song of mx writes the envelope's times and the slides in beats, and mx turns them into seconds before they reach the engine.
//! Almost every synth takes `note`, and most take `vel`, how hard the note is played.
//! A later change of an option glides over the seconds its partner ending in `_slide` gives, such as `note_slide`.
//!
#![doc = include_str!("../../docs/synths.md")]

mod basic;
mod bowed;
mod common;
mod drums;
mod organs;
mod strings;
mod tuned;
mod winds;

pub(crate) use bowed::bowed_string;
pub(crate) use common::partials;
pub use drums::berimbau;
pub use strings::piano;
pub(crate) use winds::{drifting_swing, played_jet, wind_bands};

/// mx's synths by family, each a name and its synths; together they are the synths of `all`.
pub fn families() -> Vec<(&'static str, Vec<crate::Synth>)> {
    let mut plucked = vec![piano()];
    plucked.extend(strings::all());
    let mut struck = vec![berimbau()];
    struck.extend(drums::all());
    vec![
        ("Basic waveforms", basic::all()),
        ("Piano and plucked strings", plucked),
        ("Bowed strings", bowed::all()),
        ("Winds", winds::all()),
        ("Organs and reeds", organs::all()),
        ("Tuned percussion", tuned::all()),
        ("Drums", struck),
    ]
}

/// Every synth mx ships.
pub fn all() -> Vec<crate::Synth> {
    let mut synths = vec![piano(), berimbau()];
    synths.extend(basic::all());
    synths.extend(drums::all());
    synths.extend(bowed::all());
    synths.extend(strings::all());
    synths.extend(tuned::all());
    synths.extend(organs::all());
    synths.extend(winds::all());
    synths
}
