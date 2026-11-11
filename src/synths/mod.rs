//! mx's own synths.

mod berimbau;
mod cello;
mod piano;

pub use berimbau::berimbau;
pub use cello::cello;
pub use piano::piano;

/// Every synth mx ships.
pub fn all() -> Vec<crate::Synth> {
    vec![piano(), berimbau(), cello()]
}
