//! mx's own synths.

mod basic;
mod berimbau;
mod bowed;
mod common;
mod drums;
mod organs;
mod strings;
mod tuned;
mod winds;

pub use berimbau::berimbau;
pub(crate) use bowed::bowed_string;
pub(crate) use common::partials;
pub use strings::piano;
pub(crate) use winds::{air_jet, wind_bands};

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
