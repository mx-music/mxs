//! mx's own synths.

mod basic;
mod berimbau;
mod bowed;
mod drums;
mod instruments;
mod piano;
mod winds;

pub use berimbau::berimbau;
pub(crate) use bowed::bowed_string;
pub(crate) use instruments::partials;
pub use piano::piano;
pub(crate) use winds::{air_jet, wind_bands};

/// Every synth mx ships.
pub fn all() -> Vec<crate::Synth> {
    let mut synths = vec![piano(), berimbau()];
    synths.extend(basic::all());
    synths.extend(drums::all());
    synths.extend(instruments::all());
    synths.extend(winds::all());
    synths
}
