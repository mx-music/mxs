//! mx's tuned percussion: the marimba, the vibraphone, the xylophone, the glockenspiel and the timpani.

use super::common::{moved_by, struck, tilted};
use super::figures::{Figures, kept, kept_gain};
use crate::nodes::{db, hz, perc, shape, struck_gliding, white_noise};
use crate::{Context, Note, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::{FRAC_1_SQRT_2, PI};

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

/// A marimba: the bar's note and its overtones two octaves, a little over three octaves and higher above it, and the odd harmonics of the tube under it, each a resonator struck once.
/// The harder the bar is struck, the louder and higher its overtones sound, and a short tap of the mallet sounds above them.
fn marimba() -> Synth {
    struck("marimba", 0.3)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| {
            let note = o.number("note");
            let vel = o.number("vel").clamp(0.05, 1.0);
            let freq = hz(note);
            // The seconds a note fades 60 dB in: 4.6 at C2, halving every 17 semitones, and every 16 more above F#5.
            let ring =
                4.6 * 2f64.powf((note - 36.0) / -17.0) * 2f64.powf((note - 78.0).max(0.0) / -16.0);
            // The tube takes a moment to answer the bar, so the note starts at a quarter of its level.
            let swell = 0.45 * 2f64.powf((note - 36.0) / -18.0);
            let fourth = -12.0
                - mallet(freq * 4.0, 600.0, 57.0 * (1.0 - vel))
                - 10.0 * (freq * 4.0 / 900.0).log2().max(0.0)
                - 20.0 * (370.0 / (freq * 4.0)).log2().max(0.0);
            let overtone =
                |k: f64| 10.0 + 14.5 * vel.log2() - mallet(freq * k, 1100.0, 33.4 - 11.4 * vel);
            let bar = [
                (4.0, fourth, 0.3),
                (10.0, overtone(10.0), 0.1),
                (19.0, overtone(19.0) - 8.0, 0.08),
                (30.0, overtone(30.0) - 12.0, 0.06),
            ];
            let twelfth = if (46.0..=62.0).contains(&note) {
                -22.0
            } else {
                -82.0
            };
            let tube_slope = 12.0 + 45.0 * (1.0 - vel);
            let tube = [(3.0, twelfth), (5.0, -26.0), (7.0, -26.0), (9.0, -26.0)]
                .map(|(k, level)| (k, level - mallet(freq * k, 600.0, tube_slope), 0.35));

            let mut modes = vec![(1.0, 1.25, ring), (1.0, -1.0, swell)];
            for (k, level, fade) in bar.into_iter().chain(tube) {
                let level = if freq * k < 15_000.0 { db(level) } else { 0.0 };
                // Overtones above 1.5 kHz die sooner the higher they are.
                modes.push((
                    k,
                    level,
                    ring * fade * (1500.0 / (freq * k)).min(1.0).powf(0.8),
                ));
            }
            let pitch = o.follow("note")
                >> moved_by(o, "bend", |x| x)
                >> Net::wrap(Box::new(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32)));
            let bank = modes
                .into_iter()
                .filter(|(_, level, _)| *level != 0.0)
                .map(|(k, level, fade)| {
                    let at = pitch.clone()
                        >> Net::wrap(Box::new(map(move |x: &Frame<f32, U1>| x[0] * k as f32)));
                    (Net::wrap(Box::new(pass())) | at) >> struck_gliding(level, fade)
                })
                .reduce(|bank, one| bank & one)
                .unwrap();
            let struck = Net::wrap(Box::new(impulse::<U1>())) >> bank;

            let tap = mallet_tap(ctx, 2200.0, 3, 1000.0, 0.12, 0.55)
                + mallet_tap(ctx, 4000.0, 2, 2500.0, 0.05, 0.3);
            let by_vel = |power: f64| (vel / 0.6).powf(power) as f32;
            let sound = (struck + tap * by_vel(0.5)) * (0.1 * by_vel(1.9));
            Note::envelope(tilted(o, sound), o).within(ring * 4.0 / 3.0)
        })
}

/// How many dB a mallet takes off a resonator at `hz`: `slope` an octave above `corner`, and 24 more an octave above 3 kHz.
fn mallet(hz: f64, corner: f64, slope: f64) -> f64 {
    slope * (hz / corner).log2().max(0.0) + 24.0 * (hz / 3000.0).log2().max(0.0)
}

/// A band of the mallet's tap: white noise through `stages` two-pole low-passes at `low` and a high-pass at `high`, struck once and gone over `release` seconds, at `level`.
fn mallet_tap(
    ctx: &mut Context,
    low: f64,
    stages: usize,
    high: f64,
    release: f64,
    level: f64,
) -> Net {
    let q = || Net::wrap(Box::new(dc(FRAC_1_SQRT_2 as f32)));
    let at = |hz: f64| Net::wrap(Box::new(dc(hz as f32)));
    let mut sound = white_noise(ctx.seed());
    for _ in 0..stages {
        sound = (sound | at(low) | q()) >> Net::wrap(Box::new(lowpass::<f32>()));
    }
    sound = (sound | at(high) | q()) >> Net::wrap(Box::new(highpass::<f32>()));
    sound * shape(move |t| perc(t, 0.0005, release)) * level as f32
}

fn vibraphone() -> Synth {
    struck("vibraphone", 0.3)
        .options(Spec::expressions(&["bend", "brightness"]))
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
            Note::envelope(voice.following(o) * pulse * gain as f32, o).within(end)
        })
}

/// The vibraphone's figures, fitted partial by partial to the University of Iowa's vibraphone played with the motor off.
fn vibraphone_figures() -> &'static Figures {
    super::tables::figures("vibraphone.mxf")
}

/// The vibraphone's gain, as loud at C4 at vel 1 as the version measured band by band.
const VIBRAPHONE_LEVEL: f64 = 14.7;

fn xylophone() -> Synth {
    struck("xylophone", 0.3)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| kept(xylophone_figures(), XYLOPHONE_LEVEL, o, ctx))
}

/// The xylophone's figures, fitted partial by partial to the University of Iowa's xylophone struck with rosewood mallets.
fn xylophone_figures() -> &'static Figures {
    super::tables::figures("xylophone.mxf")
}

/// The xylophone's gain, as loud at C5 at vel 1 as the version measured band by band.
const XYLOPHONE_LEVEL: f64 = 2.38;

fn glockenspiel() -> Synth {
    struck("glockenspiel", 0.3)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| kept(glockenspiel_figures(), GLOCKENSPIEL_LEVEL, o, ctx))
}

/// The glockenspiel's figures, fitted partial by partial to the University of Iowa's orchestral bells struck with plastic mallets.
fn glockenspiel_figures() -> &'static Figures {
    super::tables::figures("glockenspiel.mxf")
}

/// The glockenspiel's gain, as loud at C6 at vel 1 as the version measured band by band.
const GLOCKENSPIEL_LEVEL: f64 = 6.95;

fn timpani() -> Synth {
    struck("timpani", 0.3)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| kept(timpani_figures(), TIMPANI_LEVEL, o, ctx))
}

/// The timpani's figures, fitted partial by partial to the five timpani of VSCO 2.
fn timpani_figures() -> &'static Figures {
    super::tables::figures("timpani.mxf")
}

/// The timpani's gain, as loud at drum 3's pitch at vel 1 as the version measured band by band.
const TIMPANI_LEVEL: f64 = 1.03;
