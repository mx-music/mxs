//! mx's electric instruments, fitted to recordings: the electric piano and the electric guitar.

use super::figures::Figures;
use crate::{Note, Options, Spec, Synth};
use std::sync::OnceLock;

/// Every synth of this family mx ships.
pub fn all() -> Vec<Synth> {
    vec![electric_piano(), electric_guitar()]
}

fn electric_piano() -> Synth {
    Synth::new("electric_piano")
        .option(Spec::note("note").default(60.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the key is struck."),
        )
        .options(Spec::envelope(1000.0, 0.15))
        .build(|o, _ctx| fitted(epiano(), EPIANO_LEVEL, o))
}

/// The electric piano's figures, fitted partial by partial to NSynth's keyboard_electronic_098.
fn epiano() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/electric_piano.mxf")))
}

/// The electric piano's gain at vel 1, as loud at E3 as its alpha version.
const EPIANO_LEVEL: f64 = 0.168;

fn electric_guitar() -> Synth {
    Synth::new("electric_guitar")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is picked."),
        )
        .options(Spec::envelope(1000.0, 0.08))
        .build(|o, _ctx| fitted(eguitar(), EGUITAR_LEVEL, o))
}

/// The electric guitar's figures, fitted partial by partial to NSynth's guitar_electronic_028.
fn eguitar() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/electric_guitar.mxf")))
}

/// The electric guitar's gain at vel 1, as loud at A2 as its alpha version.
const EGUITAR_LEVEL: f64 = 0.269;

/// A note played from figures fitted to recordings.
/// The recordings are each brought to the same peak, so a velocity carries the energy vel 1 has over the first half second, and plays louder by vel alone.
fn fitted(figures: &Figures, level: f64, o: &Options) -> Note {
    let vel = o.number("vel").clamp(0.0, 1.0);
    let note = o.number("note");
    let voice = figures.voice(note, vel);
    let loudness = (figures.voice(note, 1.0).energy() / voice.energy()).sqrt();
    let gain = (level * vel.powf(0.4) * loudness).max(1e-9);
    let end = voice.silent_after();
    Note::envelope(voice.sound() * gain as f32, o).within(end)
}
