//! mx's piano, after its design in mx's priv/synthdefs/mx-instruments.scd: each harmonic a resonator struck once, with a thump of noise.

use crate::nodes::{db, hz, perc, shape, struck, white_noise};
use crate::{Note, Options, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;

pub fn piano() -> Synth {
    Synth::new("piano")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.05, 1.0)
                .doc("How hard the key is struck."),
        )
        .option(
            Spec::float("pedal")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How far the sustain pedal is down, letting the other strings ring."),
        )
        .options(Spec::envelope(1.0, 1.0))
        .build(|options, ctx| {
            let vel = options.number("vel");
            let bank = modes(options)
                .into_iter()
                .map(|(hz, level, fade)| struck(hz, level, fade))
                .reduce(|a, b| a & b)
                .expect("a piano note has harmonics");
            let strings = Net::wrap(Box::new(impulse::<U1>())) >> bank;
            let thump = (white_noise(ctx.seed()) >> lowpass_hz(400.0 + vel as f32 * 2000.0, 0.707))
                * shape(|t| perc(t, 0.001, 0.02));
            let sound = (strings * 0.019 + thump * (vel as f32 * 0.1 * 0.3))
                * ((vel / 0.6).powf(2.5)) as f32;
            Note::envelope(sound, options)
        })
}

/// Each mode of a note as `(hz, level, fade)`: the strings' harmonics, then their beating twins.
fn modes(options: &Options) -> Vec<(f64, f64, f64)> {
    let note = options.number("note");
    let freq = hz(note);
    let vel = options.number("vel");
    let pedal = options.number("pedal");
    // 0 from C2 up, rising to 1 at A#0.
    let low = ((36.0 - note) / 14.0).clamp(0.0, 1.0);
    let soft = (1.0 - vel) / 0.7;
    let stretch = (0.00016 * 2f64.powf((note - 52.0) / 7.4)).max(0.00014);
    let slope = 12.0 * (0.6 / vel).powf(0.585);
    let corner = 1000.0 * (vel / 0.6).min(1.0).powf(0.75);

    let mut strings = Vec::new();
    let mut beating = Vec::new();
    for n in 1..=48 {
        let n = n as f64;
        let hz = freq * n * ((1.0 + stretch * n * n) / (1.0 + stretch)).sqrt();
        if hz >= 16000.0 || (n > 24.0 && low <= 0.0) {
            continue;
        }
        let hammer = 20.0 * ((n * PI / 8.5).sin().abs() + 0.05).log10() * 0.45;
        let bass = (30.0 - 6.0 * low) * (70.0 / hz).log2().max(0.0);
        let tilt = low * (8.0 * soft * soft - 2.0) * n.log2();
        let treble = slope * (hz / freq.max(corner)).log2().max(0.0);
        let shape = -3.0 * n.log2() + hammer - bass - 0.4 * (note - 76.0).max(0.0);
        let level = db(shape - treble - tilt - 16.0 * (n / 24.0).log2().max(0.0));
        let fade = 10.0
            * 2f64.powf((note - 66.0).max(0.0) / -14.0)
            * 2f64.powf((hz / 1500.0).log2().max(0.0) * -0.9)
            * 2f64.powf((160.0 / hz).log2().max(0.0) * -0.5)
            * (1.0
                + low
                    * (0.5 - 0.75 * (-2.0 * (hz / 300.0).log2().powi(2)).exp()
                        + 1.2 * soft * soft * ((hz / 300.0).log2() / 2.0).clamp(0.0, 1.0)));
        // The undamped strings ring at the harmonics as brightly as at vel 1 and up to twice as long below 1 kHz, the note's own at half.
        let sympathy = if n <= 24.0 {
            db(shape - 8.6 * (hz / freq.max(1000.0)).log2().max(0.0)) * pedal * 0.65
        } else {
            0.0
        };
        let rung = level * (1.0 - pedal * 0.5) + sympathy;
        let lasting =
            fade + sympathy / (rung + 1e-9) * fade * ((1000.0 / hz).log2() / 2.0).clamp(0.0, 1.0);
        strings.push((hz, rung, lasting));
        // A note's strings are tuned a hair apart and beat against each other.
        if n <= 10.0 {
            beating.push((hz * 1.0001, level * 0.5, fade * 1.5));
        }
    }

    strings.into_iter().chain(beating).collect()
}
