//! mx's strings, plucked and struck: the guitar, the cavaquinho and bandolim, the harp, the sitar, the tanpura and the piano.

use super::common::{along, bpf, hpf, lpf, net, partials, silent_after, sine_from, struck};
use crate::nodes::{
    clicks, db, decay2, hit, hz, modes, perc, segment_at, segments, shape, struck_gliding,
    white_noise,
};
use crate::{Note, Options, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;

/// Every synth of this family mx ships.
pub fn all() -> Vec<Synth> {
    vec![
        guitar(),
        electric_bass(),
        plucked("cavaquinho", 0.1, 1.2, 1.05, &[420.0, 1100.0], &[1.0]),
        plucked("bandolim", 0.12, 1.6, 1.1, &[380.0, 900.0], &[1.0, 1.0025]),
        harp(),
        sitar(),
        tanpura(),
    ]
}

fn guitar() -> Synth {
    Synth::new("guitar")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .options(Spec::envelope(1000.0, 2.0))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let stretched: Vec<f64> = (1..=48)
                .map(|n| n as f64 * (1.0 + 0.00009 * (n * n) as f64).sqrt())
                .collect();
            let octave = (freq / 1000.0).log2();
            let line = |value: f64, at: f64, slope: f64| -> Vec<f64> {
                let base = value + slope * (octave - (at / 1000.0).log2());
                stretched.iter().map(|s| base + slope * s.log2()).collect()
            };
            let zip = |a: Vec<f64>, b: Vec<f64>, f: fn(f64, f64) -> f64| -> Vec<f64> {
                a.into_iter().zip(b).map(|(x, y)| f(x, y)).collect()
            };
            let mf = zip(
                zip(
                    line(-5.2, 120.0, 4.9),
                    zip(
                        line(-8.4, 320.0, -13.8),
                        line(-35.0, 2000.0, -9.0),
                        f64::max,
                    ),
                    f64::min,
                ),
                line(-43.0, 3200.0, -25.0),
                f64::min,
            );
            let spread: Vec<f64> = line(11.0, 120.0, 1.9)
                .into_iter()
                .map(|x| x.clamp(11.0, 21.0))
                .collect();
            let levels: Vec<f64> = (0..48)
                .map(|i| {
                    let pluck = ((i + 1) as f64 * PI * 0.17).sin().abs() + 0.03;
                    if freq * stretched[i] < 16_000.0 {
                        db(mf[i] + spread[i] * ((vel - 0.6) / 0.7)) * pluck
                    } else {
                        0.0
                    }
                })
                .collect();
            let fades = zip(
                zip(
                    zip(
                        line(4.3f64.log2(), 320.0, -0.72),
                        line(5.6f64.log2(), 320.0, -0.53),
                        f64::max,
                    ),
                    line(2.6f64.log2(), 800.0, -0.75),
                    f64::min,
                ),
                line(3.6f64.log2(), 800.0, -1.25),
                f64::min,
            );
            let near: Vec<f64> = line(0.0, 200.0, 1.0);
            let drained: Vec<f64> = fades
                .iter()
                .zip(&near)
                .map(|(f, n)| 2f64.powf(*f) * (1.0 - 0.55 * (-8.0 * n * n).exp()))
                .collect();
            let hz_: Vec<f64> = stretched.iter().map(|s| freq * s).collect();
            let fast = modes(net(impulse::<U1>()), &hz_, &levels, &drained);
            let slow_hz: Vec<f64> = hz_.iter().take(8).map(|f| f * 1.0004).collect();
            let slow_rings: Vec<f64> = drained.iter().take(8).map(|r| r * 1.6).collect();
            let slow_levels: Vec<f64> = levels.iter().take(8).map(|l| l * 0.25).collect();
            let slow = modes(net(impulse::<U1>()), &slow_hz, &slow_levels, &slow_rings);
            let pick = (white_noise(ctx.seed()) >> bpf(freq * 8.0, 1.2))
                * segments(&[0.0, 1.0, 0.0], &[0.0005, 0.006], &[-4.0, -4.0])
                * (vel * 0.2) as f32;
            // It falls silent where scsynth would free it, 80 dB down.
            let rings = levels
                .iter()
                .zip(&drained)
                .chain(slow_levels.iter().zip(&slow_rings))
                .filter(|(l, _)| **l * 0.075 > 1e-4)
                .map(|(l, r)| r * (20.0 * (l * 0.075 / 1e-4).log10()) / 60.0)
                .fold(0.1, f64::max);
            Note::envelope((fast + slow) * 0.075 + pick, o).within(rings)
        })
}

/// The bands the electric bass's figures are given at, in Hz, a third of an octave apart.
const BASS_BANDS: [f64; 22] = [
    40.0, 50.0, 63.0, 80.0, 101.0, 127.0, 160.0, 202.0, 254.0, 320.0, 403.0, 508.0, 640.0, 806.0,
    1016.0, 1280.0, 1613.0, 2032.0, 2560.0, 3225.0, 4064.0, 5120.0,
];
/// A note's fundamental at each band, in dB against the lowest note's, which brings every note to the same peak, as the recordings are.
const BASS_OWN: [f64; 22] = [
    0.0, 0.4, 0.7, -0.1, -0.7, 1.7, 4.8, 7.8, 10.8, 12.2, 12.4, 12.4, 12.4, 12.4, 12.4, 12.4, 12.4,
    12.4, 12.4, 12.4, 12.4, 12.4,
];
/// A harmonic's level as the finger lets go, at each band, in dB against the note's fundamental, with a row for each of the harmonics 1 to 10, and for 11 to 12, 13 to 15, 16 to 20, 21 to 30, and 31 up.
const BASS_LEVELS: [[f64; 22]; 15] = [
    [
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        0.0, 0.0, 0.0, 0.0,
    ],
    [
        4.3, 4.3, 4.3, 4.3, 4.6, 4.6, 4.1, 5.2, 2.2, 3.8, 1.0, -3.0, -5.9, -14.9, -23.9, -32.9,
        -41.9, -50.9, -59.9, -68.9, -77.9, -86.9,
    ],
    [
        2.8, 2.8, 2.8, 2.8, 2.8, 2.8, 4.5, 4.5, 4.6, 6.7, 1.8, 0.6, -8.4, -20.3, -31.3, -40.3,
        -49.3, -58.3, -67.3, -76.3, -85.3, -94.3,
    ],
    [
        3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 1.2, 2.4, 0.5, 5.9, -1.0, -2.8, -18.9, -27.9, -21.8,
        -30.8, -39.8, -48.8, -57.8, -66.8, -75.8,
    ],
    [
        -1.2, -1.2, -1.2, -1.2, -1.2, -1.2, -1.2, -2.0, -3.2, -3.3, -1.0, 0.2, -4.9, -14.1, -21.7,
        -26.1, -31.3, -39.7, -48.7, -57.7, -66.7, -73.4,
    ],
    [
        -9.9, -9.9, -9.9, -9.9, -9.9, -9.9, -9.9, -9.9, -11.3, -12.9, -12.5, -9.7, -9.2, -14.9,
        -19.7, -23.1, -33.3, -48.6, -60.9, -69.9, -78.9, -85.6,
    ],
    [
        -18.0, -18.0, -18.0, -18.0, -18.0, -18.0, -18.0, -18.0, -18.0, -16.6, -15.6, -17.3, -16.0,
        -13.7, -15.0, -17.8, -28.4, -44.0, -55.5, -64.5, -73.5, -80.3,
    ],
    [
        -17.8, -17.8, -17.8, -17.8, -17.8, -17.8, -17.8, -17.8, -17.8, -16.0, -12.4, -12.2, -15.5,
        -17.2, -15.6, -16.8, -28.8, -40.6, -40.7, -43.1, -52.1, -58.8,
    ],
    [
        -11.5, -11.5, -11.5, -11.5, -11.5, -11.5, -11.5, -11.5, -11.5, -11.5, -10.5, -8.6, -8.6,
        -12.9, -17.0, -18.8, -27.0, -35.9, -38.1, -43.0, -52.0, -58.7,
    ],
    [
        -18.9, -18.9, -18.9, -18.9, -18.9, -18.9, -18.9, -18.9, -18.9, -18.9, -16.4, -10.3, -6.3,
        -8.2, -11.9, -14.4, -22.2, -34.9, -44.1, -51.7, -60.7, -67.4,
    ],
    [
        -17.9, -17.9, -17.9, -17.9, -17.9, -17.9, -17.9, -17.9, -17.9, -17.9, -17.9, -16.7, -14.5,
        -12.4, -10.4, -12.7, -23.0, -35.8, -42.1, -42.7, -47.4, -54.2,
    ],
    [
        -23.8, -23.8, -23.8, -23.8, -23.8, -23.8, -23.8, -23.8, -23.8, -23.8, -23.8, -24.7, -26.4,
        -28.5, -28.8, -25.8, -27.1, -34.7, -41.9, -49.6, -58.6, -65.3,
    ],
    [
        -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -30.6, -31.4,
        -32.8, -31.8, -27.9, -25.2, -27.8, -34.9, -42.0, -49.7, -56.5,
    ],
    [
        -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3, -27.3,
        -27.3, -29.6, -32.3, -31.8, -30.9, -34.6, -44.3, -55.1, -61.8,
    ],
    [
        -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9, -37.9,
        -37.9, -37.9, -37.9, -37.9, -34.8, -29.3, -29.9, -37.5, -44.2,
    ],
];
/// The seconds a harmonic at each band takes to fade 60 dB, with the same rows.
const BASS_FADES: [[f64; 22]; 15] = [
    [
        16.28, 16.57, 16.13, 9.89, 6.02, 1.29, 3.68, 3.38, 2.05, 1.67, 1.67, 1.67, 1.67, 1.67,
        1.67, 1.67, 1.67, 1.67, 1.67, 1.67, 1.67, 1.67,
    ],
    [
        12.33, 12.33, 12.33, 12.33, 6.94, 3.49, 8.63, 6.05, 3.54, 1.37, 1.26, 0.82, 0.59, 0.59,
        0.59, 0.59, 0.59, 0.59, 0.59, 0.59, 0.59, 0.59,
    ],
    [
        2.95, 2.95, 2.95, 2.95, 2.95, 2.95, 2.36, 4.63, 8.26, 2.33, 1.98, 1.03, 0.53, 0.53, 0.41,
        0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41,
    ],
    [
        5.75, 5.75, 5.75, 5.75, 5.75, 5.75, 5.75, 3.77, 3.41, 4.08, 2.50, 1.15, 0.90, 0.64, 0.38,
        0.32, 0.32, 0.32, 0.32, 0.32, 0.32, 0.32,
    ],
    [
        4.13, 4.13, 4.13, 4.13, 4.13, 4.13, 4.13, 3.68, 2.82, 2.51, 2.43, 1.71, 0.99, 0.68, 0.48,
        0.31, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25,
    ],
    [
        3.15, 3.15, 3.15, 3.15, 3.15, 3.15, 3.15, 3.15, 2.86, 2.22, 1.91, 1.81, 1.29, 0.72, 0.39,
        0.24, 0.23, 0.32, 0.40, 0.40, 0.40, 0.40,
    ],
    [
        2.28, 2.28, 2.28, 2.28, 2.28, 2.28, 2.28, 2.28, 2.28, 2.21, 1.74, 1.43, 1.41, 1.04, 0.59,
        0.32, 0.24, 0.28, 0.32, 0.32, 0.32, 0.32,
    ],
    [
        1.43, 1.43, 1.43, 1.43, 1.43, 1.43, 1.43, 1.43, 1.43, 1.38, 1.16, 1.05, 1.13, 0.89, 0.54,
        0.34, 0.28, 0.24, 0.12, 0.08, 0.08, 0.08,
    ],
    [
        0.83, 0.83, 0.83, 0.83, 0.83, 0.83, 0.83, 0.83, 0.83, 0.83, 0.78, 0.61, 0.54, 0.72, 0.74,
        0.39, 0.25, 0.19, 0.11, 0.08, 0.08, 0.08,
    ],
    [
        1.18, 1.18, 1.18, 1.18, 1.18, 1.18, 1.18, 1.18, 1.18, 1.18, 0.99, 0.62, 0.47, 0.56, 0.58,
        0.36, 0.24, 0.19, 0.13, 0.10, 0.10, 0.10,
    ],
    [
        0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.42, 0.40, 0.39, 0.44,
        0.37, 0.24, 0.25, 0.22, 0.11, 0.08, 0.08,
    ],
    [
        0.56, 0.56, 0.56, 0.56, 0.56, 0.56, 0.56, 0.56, 0.56, 0.56, 0.56, 0.50, 0.41, 0.41, 0.45,
        0.42, 0.39, 0.31, 0.20, 0.15, 0.15, 0.15,
    ],
    [
        0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.41, 0.36, 0.28, 0.26,
        0.27, 0.24, 0.18, 0.14, 0.12, 0.12, 0.12,
    ],
    [
        0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.16, 0.18,
        0.19, 0.16, 0.14, 0.14, 0.17, 0.20, 0.20,
    ],
    [
        0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25,
        0.25, 0.25, 0.20, 0.11, 0.07, 0.07, 0.07,
    ],
];

/// An electric bass played with the fingers: a level and a fade for each harmonic, read from its number and its frequency, and the finger's drag on the string before it lets go, as measured from the notes of NSynth's bass_electronic_018.
fn electric_bass() -> Synth {
    Synth::new("electric_bass")
        .option(Spec::note("note").default(33.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .options(Spec::envelope(1000.0, 0.1))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let note = o.number("note");
            let freq = hz(note);
            let hard = ((vel - 0.6) / 0.4).clamp(-1.0, 0.5);
            let harmonics: Vec<(usize, f64)> = (1..=60)
                .map(|n| (n, freq * n as f64))
                .filter(|(_, f)| *f < 5000.0)
                .collect();
            let row = |n: usize| match n {
                1..=10 => n - 1,
                11..=12 => 10,
                13..=15 => 11,
                16..=20 => 12,
                21..=30 => 13,
                _ => 14,
            };
            let bright = |n: usize| if n >= 5 { 3.0 } else { 0.0 };
            // The longer-ringing resonator beside each harmonic slows its fall, so the first is shorter than the fade measured.
            let first = |n: usize| match n {
                1..=4 => 0.92,
                5..=6 => 0.85,
                _ => 0.8,
            };
            let soft = 2f64.powf(0.15 * hard.min(0.0));
            let own = along(&BASS_BANDS, &BASS_OWN, freq);
            let low = ((note - 40.0) / 7.0).clamp(-1.0, 0.0);
            let pitches: Vec<f64> = harmonics.iter().map(|(_, f)| *f).collect();
            let levels: Vec<f64> = harmonics
                .iter()
                .map(|(n, f)| {
                    db(own + along(&BASS_BANDS, &BASS_LEVELS[row(*n)], *f) + hard * bright(*n))
                })
                .collect();
            let fades: Vec<f64> = harmonics
                .iter()
                .map(|(n, f)| {
                    // The harmonics from the sixteenth up die sooner on the notes below E2 than the fades fitted, which could only be read on the notes above.
                    let sooner = if *n >= 16 { 2f64.powf(low) } else { 1.0 };
                    along(&BASS_BANDS, &BASS_FADES[row(*n)], *f) * soft * first(*n) * sooner
                })
                .collect();
            let gain = 0.04 * vel.sqrt();
            // Each recorded harmonic falls fast at first and slower later: a fainter resonator beside each rings longer.
            let late = |n: usize| {
                if n <= 6 {
                    (db(-21.0), 2.5)
                } else {
                    (db(-25.0), 4.0)
                }
            };
            let late_levels: Vec<f64> = harmonics
                .iter()
                .zip(&levels)
                .map(|((n, _), l)| l * late(*n).0)
                .collect();
            let late_fades: Vec<f64> = harmonics
                .iter()
                .zip(&fades)
                .map(|((n, _), f)| f * late(*n).1)
                .collect();
            let string = modes(clicks(vec![(0.03, 1.0)]), &pitches, &levels, &fades)
                + modes(
                    clicks(vec![(0.03, 1.0)]),
                    &pitches,
                    &late_levels,
                    &late_fades,
                );
            // The finger: it drags on the string for 0.03 seconds before it lets go, and the note starts then.
            let finger =
                (white_noise(ctx.seed()) >> lowpass_hz(500.0, 0.707) >> lowpole_hz(1200.0))
                    * segments(
                        &[0.0, 0.5, 1.0, 0.0],
                        &[0.003, 0.03 - 0.003, 0.004],
                        &[0.0, 0.0, 0.0],
                    )
                    * db(16.0 + own + low * -8.0) as f32;
            let rise = segments(&[0.0, 0.0, 1.0], &[0.03, 0.0007], &[0.0, 0.0]);
            Note::envelope((string * rise + finger) * gain as f32, o).within(
                0.03 + silent_after(&levels, &fades, gain).max(silent_after(
                    &late_levels,
                    &late_fades,
                    gain,
                )),
            )
        })
}

/// A plucked string: harmonics shaped by where it is plucked, the higher ones dying sooner, through the body's resonances; tremolo picks it again that many times a second.
fn plucked(
    name: &'static str,
    pick: f64,
    ring: f64,
    bright: f64,
    body: &'static [f64],
    courses: &'static [f64],
) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .option(
            Spec::float("tremolo")
                .default(0.0)
                .range(0.0, f64::INFINITY)
                .doc("How many times a second a held note is picked; 0 picks it once."),
        )
        .options(Spec::envelope(0.0, 0.8))
        .build(move |o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let note = o.number("note");
            let freq = hz(note);
            let tremolo = o.number("tremolo");
            let since = move |t: f64| {
                if tremolo > 0.0 {
                    t % (1.0 / tremolo.max(0.01))
                } else {
                    t
                }
            };
            let length = ring * 2f64.powf((60.0 - note) / 24.0);
            let mut strings = Net::wrap(Box::new(zero()));
            for n in 1..=16 {
                let n = n as f64;
                if freq * n >= 14_000.0 {
                    continue;
                }
                let level = ((n * PI * pick).sin().abs() + 0.02)
                    * n.powf(-(bright + (1.0 - vel) * 0.8))
                    / courses.len() as f64;
                let fall = length / (1.0 + (n - 1.0) * 0.35);
                let fade = shape(move |t| {
                    segment_at(&[0.0, 1.0, 0.0], &[0.001, fall], &[-4.0, -4.0], since(t))
                });
                for detune in courses {
                    strings =
                        strings + sine_from(freq * n * detune, 0.0) * fade.clone() * level as f32;
                }
            }
            let damp = shape(move |t| {
                segment_at(&[1.0, 0.15, 1.0], &[0.004, 0.012], &[0.0, 0.0], since(t))
            });
            let attack_level = shape(move |t| {
                segment_at(&[0.0, 1.0, 0.0], &[0.0005, 0.008], &[-4.0, -4.0], since(t))
            });
            let attack = (white_noise(ctx.seed()) >> bpf(freq * 6.0, 1.0))
                * attack_level
                * (vel * 0.3) as f32;
            let sound = strings * damp + attack;
            let mut bodies = net(pass());
            for &f in body {
                bodies = bodies & (bpf(f, 0.4) * 0.5);
            }
            Note::envelope((sound >> bodies) * ((0.3 + vel * 0.7) * 0.3) as f32, o)
        })
}

fn harp() -> Synth {
    struck("harp", 2.0).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let stiffness = 0.00024 * 2f64.powf((note - 60.0) / 17.7);
        let hz_: Vec<f64> = (1..=48)
            .map(|n| freq * n as f64 * (1.0 + stiffness * (n * n) as f64).sqrt())
            .collect();
        let loud = (vel - 1.0) / 0.7;
        let levels: Vec<f64> = hz_
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let octaves = (f / 250.0).log2();
                let level = 20.0 + octaves.min(0.0) * 13.0 - octaves.max(0.0) * 9.0
                    + loud * (4.5 + 3.5 * octaves.max(0.0));
                let pluck = ((i + 1) as f64 * PI * 0.48).sin().abs() + 0.05;
                if *f < 16_000.0 {
                    db(level) * pluck
                } else {
                    0.0
                }
            })
            .collect();
        let fades: Vec<f64> = hz_
            .iter()
            .map(|f| {
                let octaves = (f / 250.0).log2();
                15.6 * 2f64.powf(octaves.max(0.0) * -1.15 + octaves.min(0.0) * 0.3)
            })
            .collect();
        let string = modes(net(impulse::<U1>()), &hz_, &levels, &fades);
        let noise = (white_noise(ctx.seed()) >> bpf(freq * 6.0, 1.2))
            * hit(0.0005, 0.006, -4.0)
            * (vel * 0.2) as f32;
        let gain = vel.powf(0.6);
        Note::envelope((string * 0.01 + noise) * gain as f32, o).within(silent_after(
            &levels,
            &fades,
            0.01 * gain,
        ))
    })
}

fn sitar() -> Synth {
    struck("sitar", 2.0)
        .option(
            Spec::note("sa")
                .default(49.0)
                .doc("The note the sympathetic strings are tuned to, and an octave above it."),
        )
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let centres = [
                200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0, 12649.0,
            ];
            let level_table = [10.0, 19.7, 31.5, 30.3, 22.5, 11.9, 17.9, -0.3, -9.0, -15.0];
            let fade_table = [12.68, 9.83, 12.68, 9.79, 9.32, 5.55, 3.97, 3.77, 2.4, 2.5];
            let hz_: Vec<f64> = (1..=90).map(|n| freq * n as f64).collect();
            let levels: Vec<f64> = hz_
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    if *f < 16_000.0 {
                        db(along(&centres, &level_table, *f)
                            + (vel - 0.6) * 4.0 * ((i + 1) as f64).log2())
                    } else {
                        0.0
                    }
                })
                .collect();
            let fades: Vec<f64> = hz_
                .iter()
                .map(|f| along(&centres, &fade_table, *f))
                .collect();
            // The string glides with its note option, as meend bends it.
            let pitch = o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let bank = (1..=90)
                .zip(levels.iter().zip(&fades))
                .filter(|(_, (level, _))| **level != 0.0)
                .map(|(n, (&level, &fade))| {
                    net(map(move |x: &Frame<f32, U2>| -> Frame<f32, U2> {
                        [x[0], x[1] * n as f32].into()
                    })) >> struck_gliding(level, fade)
                });
            let string = (net(impulse::<U1>()) | pitch)
                >> bank.reduce(|a, b| a & b).expect("a string has harmonics");
            let sa = hz(o.number("sa"));
            let sympathetic_hz = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 2.0, 4.0, 6.0, 8.0].map(|r| sa * r);
            let sympathetic = modes(
                lpf(4000.0) * 0.0001,
                &sympathetic_hz,
                &[1.0, 0.5, 0.35, 0.25, 0.2, 0.15, 0.7, 0.35, 0.25, 0.15],
                &[6.0; 10],
            );
            let pick =
                (white_noise(ctx.seed()) >> bpf(freq * 12.0, 1.2)) * hit(0.0005, 0.008, -4.0) * 0.3;
            let gain = 0.0007 * (vel / 0.6).powf(1.5);
            let sound = (string >> (net(pass()) & sympathetic)) * gain as f32 + pick * vel as f32;
            Note::envelope(sound, o).within(silent_after(&levels, &fades, gain) + 6.0)
        })
}

fn tanpura() -> Synth {
    Synth::new("tanpura")
        .option(
            Spec::note("note")
                .default(52.0)
                .doc("Sa, the note the drone is tuned to."),
        )
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the strings are plucked."),
        )
        .option(
            Spec::beats("gap")
                .default(0.8)
                .range(0.05, f64::INFINITY)
                .doc("The time between plucks; a cycle of the four strings takes six gaps."),
        )
        .options(Spec::envelope(16.0, 4.0))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let gap = o.number("gap").max(0.05);
            let until = o.number("sustain");
            let body = [-7.0, 10.0, 1.0, 13.0, 12.0, 9.0];
            let slopes: Vec<f64> = body.windows(2).map(|w| w[1] - w[0]).chain([7.0]).collect();
            let curve = |f: f64| {
                let octaves = (f / 100.0).log2().clamp(0.0, 6.0);
                body[0]
                    + octaves * slopes[0]
                    + slopes
                        .windows(2)
                        .enumerate()
                        .map(|(i, w)| (w[1] - w[0]) * (octaves - (i + 1) as f64).max(0.0))
                        .sum::<f64>()
                    + 40.0 * (f / 100.0).min(1.0).log10()
            };
            let plucks = |beat: f64| {
                net(An(Pulses {
                    start: beat * gap,
                    period: 6.0 * gap,
                    until,
                    next: beat * gap,
                    t: 0.0,
                    dt: 1.0 / DEFAULT_SR,
                }))
            };
            let mut sound = net(zero());
            for (ratio, beat, offset) in [
                (0.75, 0.0, -11.0),
                (1.0, 2.0, 0.0),
                (1.0017, 3.0, 0.0),
                (0.4994, 4.0, -3.0),
            ] {
                let string_hz = freq * ratio;
                for n in 1..=24 {
                    let n = n as f64;
                    if string_hz * n >= 12_000.0 {
                        continue;
                    }
                    let level = db(curve(138.6 * ratio * n) + offset);
                    let bloom = 0.005 + 1.722 * (n - 1.0) / 47.0;
                    let fade = 25.872 * n.powf(-0.35);
                    sound = sound
                        + sine_from(string_hz * n, n * 2.4 + beat)
                            * (plucks(beat) >> decay2(bloom, fade))
                            * level as f32;
                }
                let count = (16_000.0 / string_hz).floor() as usize;
                if count > 24 {
                    let ratios: Vec<f64> = (25..=count).map(|k| k as f64).collect();
                    let levels = vec![1.0 / count as f64; ratios.len()];
                    let train = partials(
                        net(dc(string_hz as f32)),
                        &ratios,
                        &levels,
                        &vec![0.0; ratios.len()],
                    );
                    let high =
                        train >> hpf(string_hz * 24.5) >> hpf(string_hz * 24.5) >> lpf(7000.0);
                    sound = sound
                        + high * (plucks(beat) >> decay2(1.7, 7.0)) * db(45.0 + offset) as f32;
                }
            }
            let all_plucks = plucks(0.0) + plucks(2.0) + plucks(3.0) + plucks(4.0);
            let mut buzz = net(zero());
            for (centre, level) in [
                200.0, 320.0, 500.0, 800.0, 1260.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0,
            ]
            .iter()
            .zip([-12.0, 9.0, 26.0, -7.0, 16.0, 4.0, -4.0, 0.0, -6.0, -11.0])
            {
                let at = freq / 138.6 * centre;
                buzz = buzz
                    + (white_noise(ctx.seed()) >> bpf(at, 0.3) >> bpf(at, 0.3)) * db(level) as f32;
            }
            let buzz = buzz * (all_plucks >> decay2(0.05, 6.0));
            Note::envelope(
                (sound + buzz * 18.75) * (0.004 * (0.3 + vel * 0.7)) as f32,
                o,
            )
        })
}

/// Impulses every `period` seconds from `start`, until `until` seconds.
#[derive(Clone)]
struct Pulses {
    start: f64,
    period: f64,
    until: f64,
    next: f64,
    t: f64,
    dt: f64,
}

impl AudioNode for Pulses {
    const ID: u64 = 0x6d78_0041;
    type Inputs = U0;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.dt = 1.0 / sample_rate;
    }

    fn reset(&mut self) {
        (self.next, self.t) = (self.start, 0.0);
    }

    fn tick(&mut self, _input: &Frame<f32, U0>) -> Frame<f32, U1> {
        let fire = self.t + self.dt / 2.0 >= self.next && self.next < self.until;
        if fire {
            self.next += self.period;
        }
        self.t += self.dt;
        [if fire { 1.0 } else { 0.0 }].into()
    }
}

/// The piano: each harmonic a resonator struck once, with a thump of noise.
/// Its options are `note`, `vel` for how hard the key is struck, `pedal` for the sustain pedal, `aftersound` for the fade in two stages, `uneven` and `instrument` for the keys' differences, and `knock` for the hammer's knock.
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
        .option(
            Spec::float("aftersound")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How far each harmonic fades in two stages, fast after the strike and then slowly, as the Steinway's do."),
        )
        .option(
            Spec::float("uneven")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("How far each key differs from its neighbours in its harmonics' levels, fades and beating."),
        )
        .option(
            Spec::integer("instrument")
                .default(0.0)
                .doc("Which piano's keys `uneven` draws, each number another instrument."),
        )
        .option(
            Spec::float("knock")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("How loud the hammer's knock through the soundboard is."),
        )
        .options(Spec::envelope(1.0, 1.0))
        .build(|options, ctx| {
            let vel = options.number("vel");
            let knock = options.number("knock");
            let bank = piano_modes(options)
                .into_iter()
                .map(|(hz, level, fade)| crate::nodes::struck(hz, level, fade))
                .reduce(|a, b| a & b)
                .expect("a piano note has harmonics");
            let strings = Net::wrap(Box::new(impulse::<U1>())) >> bank;
            let thump = (white_noise(ctx.seed()) >> lowpass_hz(400.0 + vel as f32 * 2000.0, 0.707))
                * shape(|t| perc(t, 0.001, 0.02));
            let sound = (strings * 0.019 + thump * (vel as f32 * 0.1 * 0.3))
                * ((vel / 0.6).powf(2.5)) as f32;
            if knock <= 0.0 {
                return Note::envelope(sound, options);
            }
            // The soundboard's resonances stay where they are whatever the key.
            let board = [80.0, 105.0, 160.0, 230.0, 320.0, 350.0, 410.0, 520.0]
                .into_iter()
                .map(|hz| crate::nodes::struck(hz, 1.0, 0.5))
                .reduce(|a, b| a & b)
                .expect("the soundboard has resonances");
            let blow = (white_noise(ctx.seed()) >> lowpass_hz(700.0, 0.707))
                * shape(|t| perc(t, 0.001, 0.012));
            let high = db(-(options.number("note") - 72.0).clamp(0.0, 7.0));
            let knocked = (blow >> board) * (KNOCK * knock * high * (vel / 0.6).powf(1.5)) as f32;
            Note::envelope(sound + knocked, options)
        })
}

const KNOCK: f64 = 0.0007;

/// A number from -1 to 1 for one harmonic of one key of one piano, the same at every strike.
fn vary(instrument: f64, note: f64, harmonic: f64, which: u64) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in [
        instrument as u64,
        note.round() as u64,
        harmonic as u64,
        which,
    ] {
        for byte in part.to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 33;
    (hash >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// Each mode of a note as `(hz, level, fade)`: the strings' harmonics, then their beating twins.
fn piano_modes(options: &Options) -> Vec<(f64, f64, f64)> {
    let aftersound = options.number("aftersound");
    let uneven = options.number("uneven");
    let instrument = options.number("instrument");
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
        if aftersound <= 0.0 && uneven <= 0.0 {
            strings.push((hz, rung, lasting));
            // A note's strings are tuned a hair apart and beat against each other.
            if n <= 10.0 {
                beating.push((hz * 1.0001, level * 0.5, fade * 1.5));
            }
            continue;
        }
        let key = |which| uneven * vary(instrument, note, n, which);
        let gain = db(5.5 * (key(0) + key(1)));
        let longer = 2f64.powf(0.5 * key(2));
        let apart = 1.0
            + 0.0001
                * (1.0 + 5.0 * uneven * vary(instrument, note, 0.0, 3).abs())
                * (1.0 + 0.5 * key(4));
        let twin = if n <= 10.0 { level * 0.5 } else { 0.0 };
        // The strike's sound fades fast and leaves the slow part, 20 dB under it, to ring on.
        let slow = (rung + twin) * 0.1 * aftersound;
        let fast = rung + twin * aftersound - slow;
        // 0 up to G#3, rising to 1 at E4: the strike fades faster up the keyboard.
        let upper = ((note - 56.0) / 8.0).clamp(0.0, 1.0);
        let quick = (0.38 - 0.16 * upper).powf(aftersound);
        strings.push((hz, fast * gain, lasting * longer * quick));
        if slow > 0.0 {
            beating.push((
                hz * apart,
                slow * gain,
                lasting * longer * (2.5 - 0.5 * upper),
            ));
        }
        if twin * (1.0 - aftersound) > 0.0 {
            beating.push((
                hz * apart,
                twin * (1.0 - aftersound) * gain,
                fade * longer * 1.5,
            ));
        }
    }

    strings.into_iter().chain(beating).collect()
}
