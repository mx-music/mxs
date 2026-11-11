//! mx's own synths.

mod basic;
mod berimbau;
mod drums;
mod instruments;
mod bowed;
mod piano;

pub use berimbau::berimbau;
pub use piano::piano;
pub(crate) use bowed::bowed_string;
pub(crate) use instruments::partials;

/// Every synth mx ships.
pub fn all() -> Vec<crate::Synth> {
    let mut synths = vec![piano(), berimbau()];
    synths.extend(basic::all());
    synths.extend(drums::all());
    synths.extend(instruments::all());
    synths
}
