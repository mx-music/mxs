//! mx's own synths.

mod basic;
mod berimbau;
mod drums;
mod cello;
mod piano;

pub use berimbau::berimbau;
pub use cello::cello;
pub use piano::piano;

/// Every synth mx ships.
pub fn all() -> Vec<crate::Synth> {
    let mut synths = vec![piano(), berimbau(), cello()];
    synths.extend(basic::all());
    synths.extend(drums::all());
    synths
}
