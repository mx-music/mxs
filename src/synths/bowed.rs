//! The bowed string of mx's violin, viola, cello and double bass: a waveguide after the bowed-string model of McIntyre, Schumacher and Woodhouse, "On the oscillations of musical instruments", JASA 74, 1983, and Julius O. Smith III's digital waveguides, <https://ccrma.stanford.edu/~jos/pasp/Bowed_Strings.html>.
//! After the string come a peak at each band mx measures, with its gain fitted to each instrument's recordings, then vibrato as a swinging delay, a low-pass, the bow's hiss, and the swell in.

use super::common::{harmonics, moved_by, net, silent_after, tilted};
use super::figures::{Figures, held_note};
use crate::nodes::modes;
use crate::nodes::{db, hz, pink_noise, shape, shape_node};
use crate::synth::SynthDecl;
use crate::{Context, Note, Options, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;
use std::sync::OnceLock;

/// The centres of the bands mx measures, from 80 Hz to 4 kHz.
const BANDS: [f64; 8] = [113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0];

const EQ: [&str; 8] = ["eq0", "eq1", "eq2", "eq3", "eq4", "eq5", "eq6", "eq7"];

/// The string runs at twice mx's rate, so the bow's side of the violin's top notes still spans a few samples.
const OVERSAMPLED: f64 = crate::RATE * 2.0;

/// An instrument's figures, fitted to its recordings.
pub(crate) struct Fit {
    /// The corner in Hz below which the body radiates little.
    pub body: f64,
    /// The gain in dB at each band, at pp, mf and ff.
    pub gains: [[f64; 8]; 3],
    /// The whole sound's level in dB at pp, mf and ff.
    pub levels: [f64; 3],
    /// The seconds the sound takes to swell in at pp, mf and ff.
    pub swell: [f64; 3],
    /// How much the rosin's noise sways the bow's speed at pp, mf and ff.
    pub rosin: [f64; 3],
    /// The bow's hiss in dB at pp, mf and ff.
    pub hiss: [f64; 3],
}

/// The bow's options, and for fitting, each band's gain in place of the one fitted for the note's vel.
pub(crate) fn bow_options(mut synth: SynthDecl) -> SynthDecl {
    synth = synth
        .option(Spec::float("bow_speed").default(1.0).range(0.1, 4.0).doc("How fast the bow moves, against its speed at the note's vel."))
        .option(Spec::float("bow_force").default(1.0).range(0.1, 4.0).doc("How hard the bow presses, against its force at the note's vel: the harder, the wider the speeds at which it holds the string."))
        .option(Spec::float("legato").default(0.0).range(0.0, 1.0).doc("1 changes bow from a note still sounding: the string speaks at once instead of swelling in from silence."))
        .option(Spec::hz("body_hz").range(20.0, 2000.0).doc("The corner below which the body radiates little, in place of the one fitted; for fitting."))
        .option(Spec::float("rosin").range(0.0, 2.0).doc("How much the rosin's noise sways the bow's speed, in place of the one fitted; for fitting."))
        .option(Spec::db("hiss").range(-90.0, 0.0).doc("The bow's hiss in dB, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::float("bow_position").default(0.12).range(0.05, 0.5).doc("Where the bow meets the string, as a fraction of its length from the bridge."));
    for (name, hz) in EQ.iter().zip(BANDS) {
        let doc = format!(
            "The gain in dB of the band at {hz} Hz, in place of the one fitted for the note's vel; for fitting."
        );
        synth = synth.option(Spec::db(name).range(-60.0, 60.0).doc(&doc));
    }
    synth
}

/// A bowed note, swelling in and holding until the envelope ends it.
pub(crate) fn held(options: &Options, ctx: &mut Context, fit: &Fit) -> Net {
    let vel = options.number("vel").clamp(0.0, 1.0);
    let sustain = options.number("sustain");
    let by_vel = |pp: f64, mf: f64, ff: f64| {
        pp + (mf - pp) * ((vel - 0.3) / 0.3).clamp(0.0, 1.0)
            + (ff - mf) * ((vel - 0.6) / 0.4).clamp(0.0, 1.0)
    };

    // The bow presses harder as it moves faster, as it must to hold the string in Helmholtz motion, by Schelleng's minimum bow force, and up to twice as hard below 110 Hz, where the string would otherwise slip twice a period.
    let speed = (0.2 + vel * 0.6) * options.number("bow_speed");
    let low = (110.0 / hz(options.number("note"))).clamp(1.0, 2.0);
    let legato = options.number("legato").clamp(0.0, 1.0);
    let bow = Bow {
        speed,
        force: 1.8 * low * speed * options.number("bow_force"),
        position: options.number("bow_position") as f32,
        takes: 0.03 - 0.01 * legato,
        rosin: options
            .given("rosin")
            .unwrap_or_else(|| by_vel(fit.rosin[0], fit.rosin[1], fit.rosin[2]))
            as f32,
        seed: ctx.seed(),
    };
    let body = options.given("body_hz").unwrap_or(fit.body);
    let note = options.follow("note") >> moved_by(options, "bend", |x| x);
    let mut sound =
        string(note, bow) >> highpass_hz(body as f32, 0.541) >> highpass_hz(body as f32, 1.307);
    for (i, hz) in BANDS.into_iter().enumerate() {
        let gain = options
            .given(EQ[i])
            .unwrap_or_else(|| by_vel(fit.gains[0][i], fit.gains[1][i], fit.gains[2][i]));
        sound = sound >> bell_hz(hz as f32, 1.4, db(gain) as f32);
    }
    // A delay that swings after the string gives the vibrato, which grows in after 0.3 seconds.
    let rate_hz = 5.4;
    let depth = options.number("vibrato") * 2f64.ln() / 12.0 / (2.0 * PI * rate_hz);
    let swing = shape(move |t| {
        let grown = ((t - 0.3).clamp(0.0, 1.0)).powi(2);
        0.002 + (2.0 * PI * rate_hz * t + PI / 2.0).sin() * depth * grown
    });
    let cutoff = (2000.0 + vel * vel * 7000.0) as f32;
    sound =
        (sound | swing) >> tap(0.0, 0.01) >> lowpass_hz(cutoff, 0.541) >> lowpass_hz(cutoff, 1.307);
    let hiss_level = db(options
        .given("hiss")
        .unwrap_or_else(|| by_vel(fit.hiss[0], fit.hiss[1], fit.hiss[2])))
        as f32;
    let hiss =
        (pink_noise(ctx.seed()) >> highpass_hz((1800.0 - vel * 400.0) as f32, 0.707)) * hiss_level;
    sound = (sound + hiss) >> dcblock::<f32>();

    // The string swells in as the recordings do at each dynamic, though never over more than part of its sustain; on a bow change it speaks within 60 ms.
    let from_silence = by_vel(fit.swell[0], fit.swell[1], fit.swell[2]).min(sustain * 0.4);
    let swell = (from_silence + (0.06f64.min(from_silence) - from_silence) * legato).max(1e-3);
    let rising = shape(move |t| 1.0 - (1.0 - (t / swell).min(1.0)).powi(2));
    let level = (vel.powf(0.6) * db(by_vel(fit.levels[0], fit.levels[1], fit.levels[2]))) as f32;
    tilted(options, sound) * rising * level
}

/// The seconds each half of the string delays its wave, at a pitch: its share of the period, less the feedback's sample and its loss filter's delay.
fn delays(freq: f32, position: f32) -> (f32, f32) {
    let period = OVERSAMPLED as f32 / freq;
    let (neck_cut, bridge_cut) = cuts(freq);
    // FunDSP's one-pole low-pass, y = (1 - a) x + a y, delays a sine at angular step w by atan2(a sin w, 1 - a cos w) / w samples.
    let w = std::f32::consts::TAU * freq / OVERSAMPLED as f32;
    let lag = |cut: f32| {
        let a = (-std::f32::consts::TAU * cut / OVERSAMPLED as f32).exp();
        (a * w.sin()).atan2(1.0 - a * w.cos()) / w
    };
    let neck = ((1.0 - position) * period - 1.0 - lag(neck_cut)) / OVERSAMPLED as f32;
    let bridge = (position * period - 1.0 - lag(bridge_cut)) / OVERSAMPLED as f32;
    (neck, bridge)
}

/// The loss filters' corners at a pitch: each end of the string loses its highs, the bridge more, and a high note's corners rise with it.
fn cuts(freq: f32) -> (f32, f32) {
    (6000f32.max(freq * 16.0), 3000f32.max(freq * 8.0))
}

/// How the bow plays the string: its speed, its force, which sets the widest gap in speed at which it holds the string, and where it meets the string, as a fraction of its length from the bridge.
struct Bow {
    speed: f64,
    force: f64,
    position: f32,
    /// The seconds the bow takes to reach its speed.
    takes: f64,
    /// How much the rosin's noise sways the bow's speed, and its seed.
    rosin: f32,
    seed: u64,
}

/// The bowed string as a defsynth plays it: `speed`, `force`, `position` and `takes` as `Bow` has them, and its rosin's noise drawn from `seed`.
pub(crate) fn bowed_string(
    note: Net,
    speed: f64,
    force: f64,
    position: f64,
    takes: f64,
    rosin: f64,
    seed: u64,
) -> Net {
    string(
        note,
        Bow {
            speed,
            force,
            position: position as f32,
            takes,
            rosin: rosin as f32,
            seed,
        },
    )
}

/// The bowed string: a waveguide whose two halves meet at the bow, where friction holds the string or lets it slip, following its note's pitch.
fn string(note: Net, bow: Bow) -> Net {
    let Bow {
        speed,
        force,
        position,
        takes,
        rosin,
        seed,
    } = bow;
    let (neck_loss, bridge_loss) = (0.995f32, 0.97f32);

    // The bow reaches its speed and force together and quickly, so the string takes up Helmholtz motion at once, after Guettler's attacks; the rosin's noise sways its speed, which the string shapes around each harmonic.
    let bow = (shape_node(move |t| speed * (0.05 + 0.95 * (t / takes).min(1.0)))
        * (1.0 + pink::<f32>().seed(seed) * rosin))
        | shape_node(move |t| force * (0.05 + 0.95 * (t / takes).min(1.0)));
    // The string sticks to the bow while their speeds are close, and slips once they part; the pitch rides along to each half.
    let junction = map(move |i: &Frame<f32, U5>| -> Frame<f32, U4> {
        let (bow, width, freq, from_neck, from_bridge) = (i[0], i[1], i[2], i[3], i[4]);
        let slip = bow - (from_neck + from_bridge);
        let held = slip / (1.0 + (slip / width).powi(4));
        [from_bridge + held, freq, from_neck + held, freq].into()
    });
    // Each half delays its wave by its share of the period at the pitch, and loses its highs at a corner that follows the pitch.
    let neck = (pass()
        | (map(move |f: &Frame<f32, U1>| delays(f[0], position).0)
            ^ map(|f: &Frame<f32, U1>| cuts(f[0]).0)))
        >> (tap(0.0, 0.1) | pass())
        >> (lowpole::<f32>() * -neck_loss);
    let bridge = (pass()
        | (map(move |f: &Frame<f32, U1>| delays(f[0], position).1)
            ^ map(|f: &Frame<f32, U1>| cuts(f[0]).1)))
        >> (tap(0.0, 0.1) | pass())
        >> (lowpole::<f32>() * -bridge_loss);
    // The pitch's channel feeds back nothing, so it holds the pitch alone.
    let waveguide = feedback((bow | multipass::<U3>()) >> junction >> (dc(0.0) | neck | bridge));
    let string = (pass() | dc((0.0, 0.0))) >> waveguide >> (sink() | sink() | pass());
    let pitch = note >> map(|n: &Frame<f32, U1>| hz(n[0] as f64) as f32);
    pitch >> Net::wrap(Box::new(oversample(string)))
}

/// Every bowed instrument mx ships.
pub fn all() -> Vec<Synth> {
    vec![
        figured(
            "violin",
            Bowed {
                peak: 333.0,
                rise: 18.0,
                soft_rise: 6.0,
                fall: 12.0,
                soft_fall: 3.0,
                fade: 2.1,
                fade_slope: -0.55,
                pluck: 0.13,
                gain: 0.6,
                count: 32,
                level: 0.0,
            },
            violin_figures,
            VIOLIN_LEVEL,
        ),
        bowed(
            "viola",
            Bowed {
                peak: 350.0,
                rise: 17.0,
                soft_rise: 3.0,
                fall: 14.0,
                soft_fall: 4.5,
                fade: 3.7,
                fade_slope: -0.7,
                pluck: 0.13,
                gain: 0.3,
                count: 32,
                level: -15.0,
            },
            Fit {
                body: 367.1,
                gains: [
                    [0.0, 9.3, 8.8, -6.2, 7.6, -0.8, 10.4, 3.7],
                    [0.0, 13.2, 6.4, -4.6, 10.4, -0.7, 15.3, 0.5],
                    [0.0, 12.7, 11.2, -6.2, 8.0, 2.4, 17.0, 1.7],
                ],
                levels: [-21.45, -29.20, -35.05],
                swell: [0.815, 0.68, 0.5],
                rosin: [0.1, 0.1, 0.1],
                hiss: [-35.5, -35.5, -30.5],
            },
        ),
        figured(
            "cello",
            Bowed {
                peak: 144.0,
                rise: 14.0,
                soft_rise: 3.0,
                fall: 10.0,
                soft_fall: 6.5,
                fade: 4.0,
                fade_slope: -0.15,
                pluck: 0.13,
                gain: 0.6,
                count: 32,
                level: 0.0,
            },
            cello_figures,
            CELLO_LEVEL,
        ),
        bowed(
            "double_bass",
            Bowed {
                peak: 84.0,
                rise: 24.0,
                soft_rise: 5.0,
                fall: 19.5,
                soft_fall: 2.0,
                fade: 3.9,
                fade_slope: 0.0,
                pluck: 0.13,
                gain: 0.6,
                count: 64,
                level: 0.0,
            },
            Fit {
                body: 67.5,
                gains: [
                    [8.4, 4.1, -5.9, -6.8, -8.7, -3.0, -8.3, -3.0],
                    [11.2, 5.1, -2.6, -3.4, -7.2, -0.0, -3.7, -9.4],
                    [17.3, 8.1, -0.1, 1.2, -1.4, 3.5, 0.9, -0.7],
                ],
                levels: [-19.05, -24.20, -31.55],
                swell: [0.396, 0.342, 0.22],
                rosin: [0.3, 0.53, 0.1],
                hiss: [-35.5, -35.5, -30.5],
            },
        ),
    ]
}

/// The figures a plucked string is measured by: its harmonics' peak, slopes and fades.
struct Bowed {
    peak: f64,
    rise: f64,
    soft_rise: f64,
    fall: f64,
    soft_fall: f64,
    fade: f64,
    fade_slope: f64,
    pluck: f64,
    gain: f64,
    count: usize,
    level: f64,
}

/// A bowed string, which pizz: 1 plucks instead.
fn bowed(name: &'static str, s: Bowed, fit: Fit) -> Synth {
    let synth = Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is bowed or plucked."),
        )
        .option(
            Spec::float("pizz")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 plucks the string instead of bowing it."),
        )
        .option(Spec::float("vibrato").default(0.25).range(0.0, 2.0).doc(
            "The vibrato's depth in semitones, reached as the note holds, read as the note starts.",
        ))
        .options(Spec::envelope(1.0, 0.3))
        .options(Spec::expressions(&["bend", "brightness"]));
    bow_options(synth).build(move |o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let pizz = o.number("pizz").clamp(0.0, 1.0);
        let freq = hz(o.number("note"));
        let soft = ((1.0 - vel) / 0.7).max(0.0);
        let mut sound = net(zero());
        if pizz < 1.0 {
            sound = sound + held(o, ctx, &fit) * (1.0 - pizz) as f32;
        }
        let mut end = f64::INFINITY;
        if pizz > 0.0 {
            let (pluck, silent) = plucked(&s, freq, vel, soft, pizz);
            sound = sound + pluck;
            if pizz >= 1.0 {
                end = silent;
            }
        }
        Note::envelope(sound, o).within(end)
    })
}

/// A plucked string at `freq`, as loud as `pizz` of a whole pluck, and the seconds after which it is silent.
fn plucked(s: &Bowed, freq: f64, vel: f64, soft: f64, pizz: f64) -> (Net, f64) {
    let hz_: Vec<f64> = harmonics(s.count).iter().map(|n| freq * n).collect();
    let octaves: Vec<f64> = hz_.iter().map(|f| (f / s.peak).log2()).collect();
    let levels: Vec<f64> = (0..s.count)
        .map(|i| {
            let p = octaves[i];
            let level = p.min(0.0) * (s.rise + s.soft_rise * soft.powi(3))
                - p.max(0.0) * (s.fall + s.soft_fall * soft);
            if hz_[i] < 16_000.0 { db(level) } else { 0.0 }
        })
        .collect();
    let fades: Vec<f64> = octaves
        .iter()
        .map(|p| s.fade * 2f64.powf(p * s.fade_slope))
        .collect();
    let gain = vel.powf(s.gain) * db(s.level);
    let sound = modes(net(impulse::<U1>()), &hz_, &levels, &fades) * (s.pluck * pizz * gain) as f32;
    (sound, silent_after(&levels, &fades, s.pluck))
}

/// A bowed string played from figures fitted to its recordings partial by partial, which pizz: 1 plucks instead.
/// Its loudness follows the recordings from one velocity to the next, `level` at vel 1, and a note softer than the softest recorded is quieter in proportion.
fn figured(name: &'static str, s: Bowed, figures: fn() -> &'static Figures, level: f64) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch; a change while the note holds slides to the new one."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is bowed or plucked."),
        )
        .option(
            Spec::float("pizz")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 plucks the string instead of bowing it."),
        )
        .option(
            Spec::float("vibrato")
                .default(0.25)
                .range(0.0, 2.0)
                .doc("The vibrato's depth in semitones, reached as the note holds, read as the note starts; the recordings are played without one."),
        )
        .option(
            Spec::float("legato")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 changes bow from a note still sounding: the string speaks at once, from where the recordings settle, instead of from silence."),
        )
        .options(Spec::envelope(1.0, 0.3))
        .options(Spec::expressions(&["bend", "brightness", "pressure"]))
        .build(move |o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let pizz = o.number("pizz").clamp(0.0, 1.0);
            let figures = figures();
            let gain = move |vel: f64| level * (vel / figures.softest()).min(1.0);
            // A plucked note mixes in a pluck built for its note, so only a bowed one can be kept ready.
            let built = &mut Context::new(o.owner.clone());
            let ctx = if pizz <= 0.0 { ctx } else { built };
            let bowed = held_note(figures, gain, Some(o.number("vibrato")), o.number("legato"), None, true, o, ctx);
            if pizz <= 0.0 {
                return bowed;
            }
            let soft = ((1.0 - vel) / 0.7).max(0.0);
            let (pluck, silent) = plucked(&s, hz(o.number("note")), vel, soft, pizz);
            let end = if pizz >= 1.0 { silent } else { f64::INFINITY };
            Note::envelope(bowed.sound * (1.0 - pizz) as f32 + pluck, o).within(end)
        })
}

/// The violin's figures, fitted partial by partial to the University of Iowa's arco violin at pp, mf and ff.
fn violin_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/violin.mxh")))
}

/// The violin's gain at vel 1, as loud at vel 1 as its bowed model on average over G3 to C7, since its recordings' high notes stand up to 9 dB quieter.
const VIOLIN_LEVEL: f64 = 1.39;

/// The cello's figures, fitted partial by partial to the University of Iowa's arco cello at pp, mf and ff.
fn cello_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/cello.mxh")))
}

/// The cello's gain at vel 1, as loud at C3 at vel 1 as its bowed model, within 0.1 dB.
const CELLO_LEVEL: f64 = 1.0;
