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
            let shapes = |m: usize| marimba_shapes(m, note, vel);
            let ring = shapes(0)
                .iter()
                .map(|(_, [_, fade, _, _])| MARIMBA_FALL * fade.exp())
                .fold(0.0, f64::max);
            let mut modes = vec![];
            for (m, (k, _)) in MARIMBA_MODES.iter().enumerate() {
                if freq * k >= 15_000.0 {
                    continue;
                }
                // Resonators at one frequency struck together add up, so the shapes either side of the note and vel sound in their shares.
                for (share, [level, fade, second, second_fade]) in shapes(m) {
                    if level < -100.0 || share == 0.0 {
                        continue;
                    }
                    let level = 1.25 * share * db(level);
                    modes.push((*k, level, MARIMBA_FALL * fade.exp()));
                    modes.push((*k, level * second, MARIMBA_FALL * second_fade.exp()));
                }
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

/// Each of the marimba's partials as a multiple of the note, and its shape at vel 40, 80 and 127 out of 127, at every sixth note from C2 to C7.
/// A shape is two resonators: the first's level in dB against the note's own, both their time constants' natural logarithms, and the second's level as a share of the first's, negative where the partial swells.
/// Read from the University of Iowa's 183 yarn recordings at pp, mf and ff as fitted by gradient descent, each the middle of the notes within three semitones.
#[allow(
    clippy::approx_constant,
    reason = "fitted logarithms of time constants, two of which round to -3.14"
)]
const MARIMBA_MODES: [(f64, [[[f64; 4]; 11]; 3]); 8] = [
    (
        1.0,
        [
            [
                [0.00, -0.56, -0.84, -2.14],
                [0.00, -0.77, -0.90, -2.84],
                [0.00, -1.25, -0.92, -2.26],
                [0.00, -1.13, -0.92, -3.01],
                [0.00, -1.52, -0.89, -3.37],
                [0.00, -1.55, -0.79, -4.04],
                [0.00, -1.93, -0.86, -4.74],
                [0.00, -2.20, 0.41, -4.60],
                [0.00, -2.84, -1.00, -5.80],
                [0.00, -2.98, 0.06, -3.90],
                [0.00, -3.49, 0.00, 2.00],
            ],
            [
                [0.00, -0.55, -0.89, -2.21],
                [0.00, -0.78, -0.91, -2.68],
                [0.00, -1.12, -0.93, -2.26],
                [0.00, -1.10, -0.94, -3.14],
                [0.00, -1.57, -0.91, -3.04],
                [0.00, -1.51, -0.84, -4.16],
                [0.00, -1.97, -0.95, -4.87],
                [0.00, -2.34, -0.81, -5.42],
                [0.00, -2.15, 30.00, -2.87],
                [0.00, -2.59, 27.78, -3.06],
                [0.00, -3.52, 0.11, -4.68],
            ],
            [
                [0.00, -0.51, -0.86, -2.26],
                [0.00, -0.81, -0.91, -2.52],
                [0.00, -1.08, -0.93, -2.20],
                [0.00, -1.08, -0.88, -2.92],
                [0.00, -1.53, -0.87, -3.16],
                [0.00, -1.45, -0.81, -4.28],
                [0.00, -1.91, -0.91, -4.90],
                [0.00, -2.31, -1.00, -5.77],
                [0.00, -2.71, 0.55, -4.89],
                [0.00, -3.00, 0.58, -3.54],
                [0.00, -3.22, -0.89, -3.20],
            ],
        ],
    ),
    (
        2.0,
        [
            [
                [-49.30, -1.53, -0.94, -2.26],
                [-18.59, -2.47, -0.98, -2.45],
                [-64.78, -1.59, 9.01, -4.88],
                [-68.21, -1.55, -0.88, -4.54],
                [-150.00, -5.00, 0.00, -5.00],
                [-38.48, -2.69, -1.00, -2.77],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-84.59, -2.40, 30.00, -3.90],
                [-49.30, -4.38, -0.02, -2.79],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-25.84, -2.18, -0.98, -2.17],
                [-35.33, -4.82, 0.04, -0.83],
                [-69.69, -0.55, 30.00, -4.79],
                [-66.11, -0.97, 2.86, -2.19],
                [-54.79, -2.08, -0.74, -4.23],
                [-50.31, -2.10, 22.59, -5.66],
                [-52.92, -2.48, 12.26, -5.22],
                [-45.37, -2.94, -0.60, -3.85],
                [-22.02, -3.14, 1.15, -5.06],
                [-5.64, -5.27, 0.02, -3.05],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-56.25, -2.77, 11.31, -4.66],
                [-38.87, -4.66, 0.06, -0.88],
                [-61.61, -0.75, 3.77, -4.24],
                [-53.03, -1.57, 1.82, -5.14],
                [-44.00, -1.94, -0.41, -3.36],
                [-45.28, -2.13, 30.00, -6.11],
                [-41.53, -2.65, 3.06, -4.58],
                [-42.02, -3.06, 7.13, -5.08],
                [-45.37, -3.35, 5.05, -5.84],
                [-29.08, -5.65, 0.15, -3.61],
                [-38.77, -6.00, 0.03, -3.68],
            ],
        ],
    ),
    (
        3.0,
        [
            [
                [-64.55, 0.60, -0.75, -1.16],
                [-150.00, -5.00, 0.00, -5.00],
                [-48.04, -1.60, 7.99, -3.95],
                [-28.65, -4.34, 0.07, -1.68],
                [-18.32, -2.30, -1.00, -2.31],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-33.60, -1.96, -0.97, -1.95],
                [-73.41, 0.38, 30.00, -4.76],
                [-40.72, -1.55, 5.67, -4.37],
                [-33.97, -1.86, 4.37, -4.91],
                [-1.68, -2.61, -1.00, -2.62],
                [-62.24, -1.96, 30.00, -6.06],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-59.90, -0.18, 8.50, -4.32],
                [-6.74, -1.10, -1.00, -1.10],
                [-46.15, -1.17, 16.15, -4.55],
                [8.37, -2.23, -1.00, -2.24],
                [8.68, -2.52, -1.00, -2.52],
                [-31.80, -2.94, -0.77, -3.29],
                [-8.76, -3.06, -0.98, -3.05],
                [-34.88, -5.26, 0.00, -2.12],
                [-31.06, -5.89, 0.01, -3.07],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        4.0,
        [
            [
                [-29.13, -1.62, -0.39, -4.39],
                [-24.02, -1.75, 1.58, -4.10],
                [-20.94, -2.12, -0.22, -2.63],
                [-42.65, -2.55, 15.15, -5.98],
                [-48.73, -2.79, 2.30, -5.21],
                [-51.10, -3.42, -0.01, -1.50],
                [-22.14, -3.54, -0.95, -3.53],
                [-49.19, -5.21, -0.00, -2.13],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-18.17, -1.85, -0.64, -2.42],
                [-17.54, -1.85, 0.41, -3.72],
                [-28.06, -1.93, 1.41, -4.59],
                [-33.43, -2.22, 2.38, -3.13],
                [-36.29, -2.80, 1.47, -4.07],
                [-39.74, -3.46, 0.00, -0.49],
                [-36.38, -3.84, 0.00, 2.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-52.23, -0.91, 7.99, -1.83],
                [-21.49, -1.84, 1.69, -4.50],
                [-20.95, -1.99, 0.25, -3.94],
                [-21.67, -2.39, 0.11, -3.17],
                [-15.08, -3.12, -0.68, -3.10],
                [-45.63, -3.06, 12.42, -3.44],
                [-25.91, -3.81, 0.00, 2.00],
                [-53.77, -4.18, -0.01, -1.75],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        5.0,
        [
            [
                [-34.10, -3.19, -0.05, -1.31],
                [-62.59, -1.92, 3.03, -4.41],
                [-38.51, -4.88, 0.04, -1.37],
                [-68.55, -1.19, 30.00, -5.76],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-150.00, -5.00, 0.00, -5.00],
                [-33.98, -4.20, 0.02, -1.03],
                [-51.51, -1.88, 10.13, -5.10],
                [-2.12, -2.36, -1.00, -2.37],
                [-35.24, -2.58, -0.98, -2.81],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-53.76, -1.29, 5.06, -3.02],
                [-44.63, -3.73, -0.11, -1.50],
                [-44.35, -1.81, 7.58, -4.63],
                [-40.83, -1.97, 3.91, -4.91],
                [-49.86, -2.06, 6.61, -4.93],
                [-17.70, -2.02, -0.98, -2.02],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        7.0,
        [
            [
                [-64.74, -2.59, 4.56, -4.29],
                [-35.17, -4.67, 0.01, -2.07],
                [-62.51, -4.33, 0.12, -1.30],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-42.57, -3.76, 0.10, -2.02],
                [-40.49, -4.62, 0.05, -1.68],
                [-56.70, -1.91, -0.67, -2.32],
                [-14.64, -2.55, -1.00, -2.56],
                [-20.28, -2.67, -1.00, -2.68],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-58.21, -1.93, 8.02, -4.58],
                [-63.60, -1.88, 8.97, -4.53],
                [-63.63, -1.51, 16.27, -4.73],
                [-61.22, -1.90, -0.76, -2.35],
                [-59.43, -2.15, -0.88, -2.90],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        10.0,
        [
            [
                [-9.47, -2.54, -0.83, -5.25],
                [7.23, -2.48, -0.97, -2.47],
                [-34.28, -3.04, -0.09, -2.24],
                [-38.55, -3.61, 0.59, -4.49],
                [-45.61, -4.60, 0.01, -3.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-30.48, -2.00, 25.16, -2.71],
                [-21.50, -2.76, 3.85, -3.11],
                [5.62, -2.91, -0.94, -2.89],
                [3.80, -3.23, -0.97, -3.23],
                [-33.47, -3.94, -0.08, -3.24],
                [-39.86, -5.98, 0.02, -4.11],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-8.86, -2.46, 4.00, -3.18],
                [0.33, -2.95, -0.89, -4.72],
                [23.03, -2.84, -0.98, -2.83],
                [-10.01, -3.78, 0.01, -2.48],
                [7.94, -3.67, -0.97, -3.67],
                [-47.80, -5.15, 0.01, -1.55],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        19.0,
        [
            [
                [-38.57, -4.75, 0.03, -2.80],
                [-40.51, -4.24, -0.21, -3.41],
                [-49.31, -5.52, -0.00, -2.56],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-35.82, -2.98, 3.38, -3.69],
                [-28.22, -3.44, 0.00, 2.00],
                [-31.60, -4.39, -0.97, -4.38],
                [-38.08, -5.39, -0.00, -2.88],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-49.97, -2.40, 30.00, -3.35],
                [4.36, -3.39, -0.96, -3.38],
                [-38.05, -5.10, 0.09, -3.59],
                [-8.72, -4.56, -0.96, -4.54],
                [-16.42, -5.90, -0.90, -5.88],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
];

/// The seconds a resonator takes to fall 60 dB for each second of its time constant.
const MARIMBA_FALL: f64 = 6.907_755_278_982_137;

/// The shapes of the marimba's `m`th partial at the notes and velocities of `MARIMBA_MODES` either side of `note` and `vel`, each with its share.
fn marimba_shapes(m: usize, note: f64, vel: f64) -> [(f64, [f64; 4]); 4] {
    let at = ((note - 36.0) / 6.0).clamp(0.0, 10.0);
    let low = Ord::min(at.floor() as usize, 9);
    let along = at - low as f64;
    let velocities = [40.0 / 127.0, 80.0 / 127.0, 1.0];
    let v = vel.clamp(velocities[0], 1.0);
    let (soft, loud) = if v <= velocities[1] { (0, 1) } else { (1, 2) };
    let up = (v / velocities[soft]).log2() / (velocities[loud] / velocities[soft]).log2();
    let shapes = &MARIMBA_MODES[m].1;
    [
        ((1.0 - along) * (1.0 - up), shapes[soft][low]),
        (along * (1.0 - up), shapes[soft][low + 1]),
        ((1.0 - along) * up, shapes[loud][low]),
        (along * up, shapes[loud][low + 1]),
    ]
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
            Note::envelope(super::figures::following(voice, o) * pulse * gain as f32, o).within(end)
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
