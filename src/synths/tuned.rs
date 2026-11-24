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
/// A shape is two resonators: the first's level in dB against the peak of the note's own partial over the first 20 ms, both their time constants' natural logarithms, and the second's level as a share of the first's, negative where the partial swells.
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
                [8.90, -0.56, -0.84, -2.14],
                [8.62, -0.77, -0.90, -2.84],
                [13.98, -1.25, -0.92, -2.26],
                [7.41, -1.13, -0.92, -3.01],
                [5.53, -1.52, -0.89, -3.37],
                [1.34, -1.55, -0.79, -4.04],
                [-0.67, -1.93, -0.86, -4.74],
                [-4.24, -2.20, 0.41, -4.60],
                [-1.50, -2.84, -1.00, -5.80],
                [-1.45, -2.98, 0.06, -3.90],
                [0.63, -3.49, 0.00, 2.00],
            ],
            [
                [10.69, -0.55, -0.89, -2.21],
                [8.78, -0.78, -0.91, -2.68],
                [12.44, -1.12, -0.93, -2.26],
                [5.94, -1.10, -0.94, -3.14],
                [6.98, -1.57, -0.91, -3.04],
                [-0.29, -1.51, -0.84, -4.16],
                [-1.02, -1.97, -0.95, -4.87],
                [-2.00, -2.34, -0.81, -5.42],
                [-30.70, -2.15, 30.00, -2.87],
                [-29.81, -2.59, 27.78, -3.06],
                [-0.13, -3.52, 0.11, -4.68],
            ],
            [
                [5.59, -0.51, -0.86, -2.26],
                [8.77, -0.81, -0.91, -2.52],
                [13.03, -1.08, -0.93, -2.20],
                [7.33, -1.08, -0.88, -2.92],
                [6.52, -1.53, -0.87, -3.16],
                [-0.24, -1.45, -0.81, -4.28],
                [-1.78, -1.91, -0.91, -4.90],
                [-4.60, -2.31, -1.00, -5.77],
                [-2.97, -2.71, 0.55, -4.89],
                [-4.27, -3.00, 0.58, -3.54],
                [18.24, -3.22, -0.89, -3.20],
            ],
        ],
    ),
    (
        2.0,
        [
            [
                [-40.40, -1.53, -0.94, -2.26],
                [-9.97, -2.47, -0.98, -2.45],
                [-50.80, -1.59, 9.01, -4.88],
                [-60.80, -1.55, -0.88, -4.54],
                [-150.00, -5.00, 0.00, -5.00],
                [-37.14, -2.69, -1.00, -2.77],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-86.09, -2.40, 30.00, -3.90],
                [-50.75, -4.38, -0.02, -2.79],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-15.14, -2.18, -0.98, -2.17],
                [-26.55, -4.82, 0.04, -0.83],
                [-57.25, -0.55, 30.00, -4.79],
                [-60.18, -0.97, 2.86, -2.19],
                [-47.80, -2.08, -0.74, -4.23],
                [-50.60, -2.10, 22.59, -5.66],
                [-53.93, -2.48, 12.26, -5.22],
                [-47.37, -2.94, -0.60, -3.85],
                [-52.72, -3.14, 1.15, -5.06],
                [-35.45, -5.27, 0.02, -3.05],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-50.66, -2.77, 11.31, -4.66],
                [-30.10, -4.66, 0.06, -0.88],
                [-48.58, -0.75, 3.77, -4.24],
                [-45.70, -1.57, 1.82, -5.14],
                [-37.48, -1.94, -0.41, -3.36],
                [-45.52, -2.13, 30.00, -6.11],
                [-43.31, -2.65, 3.06, -4.58],
                [-46.62, -3.06, 7.13, -5.08],
                [-48.34, -3.35, 5.05, -5.84],
                [-33.35, -5.65, 0.15, -3.61],
                [-20.53, -6.00, 0.03, -3.68],
            ],
        ],
    ),
    (
        3.0,
        [
            [
                [-55.65, 0.60, -0.75, -1.16],
                [-150.00, -5.00, 0.00, -5.00],
                [-34.06, -1.60, 7.99, -3.95],
                [-21.24, -4.34, 0.07, -1.68],
                [-12.79, -2.30, -1.00, -2.31],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-22.91, -1.96, -0.97, -1.95],
                [-64.62, 0.38, 30.00, -4.76],
                [-28.28, -1.55, 5.67, -4.37],
                [-28.03, -1.86, 4.37, -4.91],
                [5.30, -2.61, -1.00, -2.62],
                [-62.53, -1.96, 30.00, -6.06],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-54.31, -0.18, 8.50, -4.32],
                [2.03, -1.10, -1.00, -1.10],
                [-33.12, -1.17, 16.15, -4.55],
                [15.70, -2.23, -1.00, -2.24],
                [15.20, -2.52, -1.00, -2.52],
                [-32.04, -2.94, -0.77, -3.29],
                [-10.54, -3.06, -0.98, -3.05],
                [-39.49, -5.26, 0.00, -2.12],
                [-34.03, -5.89, 0.01, -3.07],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        4.0,
        [
            [
                [-20.22, -1.62, -0.39, -4.39],
                [-15.40, -1.75, 1.58, -4.10],
                [-6.96, -2.12, -0.22, -2.63],
                [-35.24, -2.55, 15.15, -5.98],
                [-43.20, -2.79, 2.30, -5.21],
                [-49.75, -3.42, -0.01, -1.50],
                [-22.81, -3.54, -0.95, -3.53],
                [-53.43, -5.21, -0.00, -2.13],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-7.48, -1.85, -0.64, -2.42],
                [-8.75, -1.85, 0.41, -3.72],
                [-15.62, -1.93, 1.41, -4.59],
                [-27.49, -2.22, 2.38, -3.13],
                [-29.31, -2.80, 1.47, -4.07],
                [-40.03, -3.46, 0.00, -0.49],
                [-37.39, -3.84, 0.00, 2.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-46.64, -0.91, 7.99, -1.83],
                [-12.72, -1.84, 1.69, -4.50],
                [-7.92, -1.99, 0.25, -3.94],
                [-14.34, -2.39, 0.11, -3.17],
                [-8.56, -3.12, -0.68, -3.10],
                [-45.88, -3.06, 12.42, -3.44],
                [-27.69, -3.81, 0.00, 2.00],
                [-58.38, -4.18, -0.01, -1.75],
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
                [-25.20, -3.19, -0.05, -1.31],
                [-53.97, -1.92, 3.03, -4.41],
                [-24.53, -4.88, 0.04, -1.37],
                [-61.14, -1.19, 30.00, -5.76],
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
                [-25.19, -4.20, 0.02, -1.03],
                [-39.07, -1.88, 10.13, -5.10],
                [3.82, -2.36, -1.00, -2.37],
                [-28.25, -2.58, -0.98, -2.81],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-48.18, -1.29, 5.06, -3.02],
                [-35.86, -3.73, -0.11, -1.50],
                [-31.32, -1.81, 7.58, -4.63],
                [-33.49, -1.97, 3.91, -4.91],
                [-43.34, -2.06, 6.61, -4.93],
                [-17.94, -2.02, -0.98, -2.02],
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
                [-55.84, -2.59, 4.56, -4.29],
                [-26.55, -4.67, 0.01, -2.07],
                [-48.53, -4.33, 0.12, -1.30],
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
                [-31.87, -3.76, 0.10, -2.02],
                [-31.71, -4.62, 0.05, -1.68],
                [-44.27, -1.91, -0.67, -2.32],
                [-8.70, -2.55, -1.00, -2.56],
                [-13.30, -2.67, -1.00, -2.68],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-52.63, -1.93, 8.02, -4.58],
                [-54.83, -1.88, 8.97, -4.53],
                [-50.61, -1.51, 16.27, -4.73],
                [-53.89, -1.90, -0.76, -2.35],
                [-52.91, -2.15, -0.88, -2.90],
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
                [-0.57, -2.54, -0.83, -5.25],
                [15.85, -2.48, -0.97, -2.47],
                [-20.30, -3.04, -0.09, -2.24],
                [-31.13, -3.61, 0.59, -4.49],
                [-40.07, -4.60, 0.01, -3.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-19.78, -2.00, 25.16, -2.71],
                [-12.71, -2.76, 3.85, -3.11],
                [18.05, -2.91, -0.94, -2.89],
                [9.74, -3.23, -0.97, -3.23],
                [-26.49, -3.94, -0.08, -3.24],
                [-40.15, -5.98, 0.02, -4.11],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-3.27, -2.46, 4.00, -3.18],
                [9.10, -2.95, -0.89, -4.72],
                [36.06, -2.84, -0.98, -2.83],
                [-2.68, -3.78, 0.01, -2.48],
                [14.46, -3.67, -0.97, -3.67],
                [-48.04, -5.15, 0.01, -1.55],
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
                [-29.67, -4.75, 0.03, -2.80],
                [-31.88, -4.24, -0.21, -3.41],
                [-35.34, -5.52, -0.00, -2.56],
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
                [-25.13, -2.98, 3.38, -3.69],
                [-19.44, -3.44, 0.00, 2.00],
                [-19.16, -4.39, -0.97, -4.38],
                [-32.14, -5.39, -0.00, -2.88],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-44.39, -2.40, 30.00, -3.35],
                [13.13, -3.39, -0.96, -3.38],
                [-25.02, -5.10, 0.09, -3.59],
                [-1.39, -4.56, -0.96, -4.54],
                [-9.90, -5.90, -0.90, -5.88],
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
