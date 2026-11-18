//! mx's tuned percussion: the marimba, the vibraphone, the xylophone, the glockenspiel and the timpani.

use super::common::struck;
use super::figures::{Figures, kept, kept_gain};
use crate::nodes::shape;
use crate::{Note, Spec, Synth};
use std::f64::consts::PI;
use std::sync::OnceLock;

/// Every synth of this family mx ships.
pub fn all() -> Vec<Synth> {
    vec![
        marimba(),
        vibraphone(),
        xylophone(),
        glockenspiel(),
        timpani(),
    ]
}

fn marimba() -> Synth {
    struck("marimba", 0.3).build(|o, ctx| kept(marimba_figures(), MARIMBA_LEVEL, o, ctx))
}

/// The marimba's figures, fitted partial by partial to the University of Iowa's marimba struck with yarn mallets.
fn marimba_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/marimba.mxf")))
}

/// The marimba's gain, as loud at C4 at vel 1 as the version measured band by band.
const MARIMBA_LEVEL: f64 = 1.53;

fn vibraphone() -> Synth {
    struck("vibraphone", 0.3)
        .option(Spec::float("motor").default(0.0).range(0.0, f64::INFINITY).doc("How many times a second the discs in the tubes pulse the note; 0 leaves the motor off."))
        .build(|o, ctx| {
            let motor = o.number("motor");
            // With the motor off the pulse is 1 throughout, so the note plays as kept, which can be kept ready.
            if motor == 0.0 {
                return kept(vibraphone_figures(), VIBRAPHONE_LEVEL, o, ctx);
            }
            let pulse = shape(move |t| if motor > 0.0 { 1.0 - 0.25 * (1.0 - (2.0 * PI * motor * t).cos()) } else { 1.0 });
            let note = o.number("note");
            let vel = o.number("vel").clamp(0.0, 1.0);
            let voice = vibraphone_figures().voice(note, vel);
            let gain = kept_gain(vibraphone_figures(), VIBRAPHONE_LEVEL, note, vel);
            let end = voice.silent_after();
            Note::envelope(voice.sound() * pulse * gain as f32, o).within(end)
        })
}

/// The vibraphone's figures, fitted partial by partial to the University of Iowa's vibraphone played with the motor off.
fn vibraphone_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/vibraphone.mxf")))
}

/// The vibraphone's gain, as loud at C4 at vel 1 as the version measured band by band.
const VIBRAPHONE_LEVEL: f64 = 14.7;

fn xylophone() -> Synth {
    struck("xylophone", 0.3).build(|o, ctx| kept(xylophone_figures(), XYLOPHONE_LEVEL, o, ctx))
}

/// The xylophone's figures, fitted partial by partial to the University of Iowa's xylophone struck with rosewood mallets.
fn xylophone_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/xylophone.mxf")))
}

/// The xylophone's gain, as loud at C5 at vel 1 as the version measured band by band.
const XYLOPHONE_LEVEL: f64 = 2.38;

fn glockenspiel() -> Synth {
    struck("glockenspiel", 0.3)
        .build(|o, ctx| kept(glockenspiel_figures(), GLOCKENSPIEL_LEVEL, o, ctx))
}

/// The glockenspiel's figures, fitted partial by partial to the University of Iowa's orchestral bells struck with plastic mallets.
fn glockenspiel_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/glockenspiel.mxf")))
}

/// The glockenspiel's gain, as loud at C6 at vel 1 as the version measured band by band.
const GLOCKENSPIEL_LEVEL: f64 = 6.95;

fn timpani() -> Synth {
    struck("timpani", 0.3).build(|o, ctx| kept(timpani_figures(), TIMPANI_LEVEL, o, ctx))
}

/// The timpani's figures, fitted partial by partial to the five timpani of VSCO 2.
fn timpani_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/timpani.mxf")))
}

/// The timpani's gain, as loud at drum 3's pitch at vel 1 as the version measured band by band.
const TIMPANI_LEVEL: f64 = 1.03;
