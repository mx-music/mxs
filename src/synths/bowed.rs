//! The bowed string of mx's violin, viola, cello and double bass: a waveguide after the bowed-string model of McIntyre, Schumacher and Woodhouse, "On the oscillations of musical instruments", JASA 74, 1983, and Julius O. Smith III's digital waveguides, https://ccrma.stanford.edu/~jos/pasp/Bowed_Strings.html.
//! After the string come a peak at each band mx measures, with its gain fitted to each instrument's recordings, then vibrato as a swinging delay, a low-pass, the bow's hiss and the swell in.

use crate::nodes::{db, hz, pink_noise, shape};
use crate::synth::SynthDecl;
use crate::{Context, Options, Spec};
use fundsp::prelude::*;
use std::f64::consts::PI;

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
}

/// The bow's options, and for fitting, each band's gain in place of the one fitted for the note's vel.
pub(crate) fn bow_options(mut synth: SynthDecl) -> SynthDecl {
    synth = synth
        .option(Spec::float("bow_speed").default(1.0).range(0.1, 4.0).doc("How fast the bow moves, against its speed at the note's vel."))
        .option(Spec::float("bow_force").default(1.0).range(0.1, 4.0).doc("How hard the bow presses, against its force at the note's vel: the harder, the wider the speeds at which it holds the string."))
        .option(Spec::float("legato").default(0.0).range(0.0, 1.0).doc("1 changes bow from a note still sounding: the string speaks at once instead of swelling in from silence."))
        .option(Spec::hz("body_hz").range(20.0, 2000.0).doc("The corner below which the body radiates little, in place of the one fitted; for fitting."))
        .option(Spec::float("bow_position").default(0.12).range(0.05, 0.5).doc("Where the bow meets the string, as a fraction of its length from the bridge."));
    for (name, hz) in EQ.iter().zip(BANDS) {
        let doc = format!("The gain in dB of the band at {hz} Hz, in place of the one fitted for the note's vel; for fitting.");
        synth = synth.option(Spec::db(name).range(-60.0, 60.0).doc(&doc));
    }
    synth
}

/// A bowed note, swelling in and holding until the envelope ends it.
pub(crate) fn held(options: &Options, ctx: &mut Context, fit: &Fit) -> Net {
    let vel = options.number("vel").clamp(0.0, 1.0);
    let sustain = options.number("sustain");
    let by_vel = |pp: f64, mf: f64, ff: f64| pp + (mf - pp) * ((vel - 0.3) / 0.3).clamp(0.0, 1.0) + (ff - mf) * ((vel - 0.6) / 0.4).clamp(0.0, 1.0);

    // The bow presses harder as it moves faster, as it must to hold the string in Helmholtz motion, by Schelleng's minimum bow force, and up to twice as hard below 110 Hz, where the string would otherwise slip twice a period.
    let speed = (0.2 + vel * 0.6) * options.number("bow_speed");
    let low = (110.0 / hz(options.number("note"))).clamp(1.0, 2.0);
    let legato = options.number("legato").clamp(0.0, 1.0);
    let bow = Bow { speed, force: 1.8 * low * speed * options.number("bow_force"), position: options.number("bow_position") as f32, takes: 0.03 - 0.01 * legato };
    let body = options.given("body_hz").unwrap_or(fit.body);
    let mut sound = string(options.follow("note"), bow) >> highpass_hz(body as f32, 0.541) >> highpass_hz(body as f32, 1.307);
    for (i, hz) in BANDS.into_iter().enumerate() {
        let gain = options.given(EQ[i]).unwrap_or_else(|| by_vel(fit.gains[0][i], fit.gains[1][i], fit.gains[2][i]));
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
    sound = (sound | swing) >> tap(0.0, 0.01) >> lowpass_hz(cutoff, 0.541) >> lowpass_hz(cutoff, 1.307);
    let hiss = (pink_noise(ctx.seed()) >> highpass_hz((1800.0 - vel * 400.0) as f32, 0.707)) * db(by_vel(-35.5, -35.5, -30.5)) as f32;
    sound = (sound + hiss) >> dcblock::<f32>();

    // The string swells in as the recordings do at each dynamic, though never over more than part of its sustain; on a bow change it speaks within 60 ms.
    let from_silence = by_vel(fit.swell[0], fit.swell[1], fit.swell[2]).min(sustain * 0.4);
    let swell = (from_silence + (0.06f64.min(from_silence) - from_silence) * legato).max(1e-3);
    let rising = shape(move |t| 1.0 - (1.0 - (t / swell).min(1.0)).powi(2));
    let level = (vel.powf(0.6) * db(by_vel(fit.levels[0], fit.levels[1], fit.levels[2]))) as f32;
    sound * rising * level
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
}

/// The bowed string: a waveguide whose two halves meet at the bow, where friction holds the string or lets it slip, following its note's pitch.
fn string(note: Net, bow: Bow) -> Net {
    let Bow { speed, force, position, takes } = bow;
    let (neck_loss, bridge_loss) = (0.995f32, 0.97f32);

    // The bow reaches its speed and force together and quickly, so the string takes up Helmholtz motion at once, after Guettler's attacks.
    let bow = envelope(move |t: f64| speed * (0.05 + 0.95 * (t / takes).min(1.0))) | envelope(move |t: f64| force * (0.05 + 0.95 * (t / takes).min(1.0)));
    // The string sticks to the bow while their speeds are close, and slips once they part; the pitch rides along to each half.
    let junction = map(move |i: &Frame<f32, U5>| -> Frame<f32, U4> {
        let (bow, width, freq, from_neck, from_bridge) = (i[0], i[1], i[2], i[3], i[4]);
        let slip = bow - (from_neck + from_bridge);
        let held = slip / (1.0 + (slip / width).powi(4));
        [from_bridge + held, freq, from_neck + held, freq].into()
    });
    // Each half delays its wave by its share of the period at the pitch, and loses its highs at a corner that follows the pitch.
    let neck = (pass() | (map(move |f: &Frame<f32, U1>| delays(f[0], position).0) ^ map(|f: &Frame<f32, U1>| cuts(f[0]).0)))
        >> (tap(0.0, 0.1) | pass())
        >> lowpole::<f32>() * -neck_loss;
    let bridge = (pass() | (map(move |f: &Frame<f32, U1>| delays(f[0], position).1) ^ map(|f: &Frame<f32, U1>| cuts(f[0]).1)))
        >> (tap(0.0, 0.1) | pass())
        >> lowpole::<f32>() * -bridge_loss;
    // The pitch's channel feeds back nothing, so it holds the pitch alone.
    let waveguide = feedback((bow | multipass::<U3>()) >> junction >> (dc(0.0) | neck | bridge));
    let string = (pass() | dc((0.0, 0.0))) >> waveguide >> (sink() | sink() | pass());
    let pitch = note >> map(|n: &Frame<f32, U1>| hz(n[0] as f64) as f32);
    pitch >> Net::wrap(Box::new(oversample(string)))
}
