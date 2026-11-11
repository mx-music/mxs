//! mx's piano, after its design in mx's priv/synthdefs/mx-instruments.scd: each harmonic a resonator struck once, with a thump of noise.

use crate::{db_amp, fast_envelope, midi_hz, perc, struck, Opts};
use fundsp::prelude::*;
use std::f64::consts::PI;

/// Scales the piano to the loudness of scsynth's render of the same notes.
const GAIN: f64 = 1.4;

pub fn piano(opts: &Opts, rate: f64) -> Box<dyn AudioUnit> {
    let note = opts.get("note", 52.0);
    let freq = midi_hz(note);
    let vel = opts.get("vel", 0.6).clamp(0.05, 1.0);
    let pedal = opts.get("pedal", 0.0).clamp(0.0, 1.0);
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
        let level = db_amp(shape - treble - tilt - 16.0 * (n / 24.0).log2().max(0.0));
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
            db_amp(shape - 8.6 * (hz / freq.max(1000.0)).log2().max(0.0)) * pedal * 0.65
        } else {
            0.0
        };
        let rung = level * (1.0 - pedal * 0.5) + sympathy;
        let lasting = fade + sympathy / (rung + 1e-9) * fade * ((1000.0 / hz).log2() / 2.0).clamp(0.0, 1.0);
        strings.push((hz, n, rung, lasting));
        // A note's strings are tuned a hair apart and beat against each other.
        if n <= 10.0 {
            beating.push((hz * 1.0001, n, level * 0.5, fade * 1.5));
        }
    }

    // Harmonics that all start rising together add up to a click at the strike, louder than scsynth's, so their phases are spread by the golden angle.
    // Beating strings start together, or they would cancel for seconds.
    let modes = (strings.into_iter().chain(beating))
        .map(|(hz, n, level, fade)| struck(hz, n * 2.399_963, level, fade, rate));
    let bank = modes.reduce(|a, b| a & b).expect("a piano note has harmonics");
    let strings = Net::wrap(Box::new(impulse::<U1>())) >> bank;

    let thump = Net::wrap(Box::new(
        white() >> lowpass_hz(400.0 + vel as f32 * 2000.0, 0.707) * fast_envelope(|t: f32| perc(t, 0.001, 0.02)),
    ));
    let sound = (strings * 0.019 + thump * (vel as f32 * 0.1 * 0.3)) * ((vel / 0.6).powf(2.5) * GAIN) as f32;
    let note_envelope = Net::wrap(Box::new(crate::note_envelope(opts, 1.0, 1.0)));
    Box::new(sound * note_envelope)
}
