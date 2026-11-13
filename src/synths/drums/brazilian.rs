//! Brazilian percussion: the pandeiro, surdo, cuica, tamborim, caxixi and berimbau.

use super::{
    amps, bpf, high_shelf, hpf, impulse_net, lpf, net, noise, peak, scaled, strike_options, vel,
};
use crate::nodes::{
    Draws, db, exp_segments, hit, hz, modes, perc, segments, shape, struck, white_noise,
};
use crate::{Note, Spec, Synth};
use fundsp::math::rnd1;
use fundsp::prelude::*;
use std::f64::consts::PI;

/// The synths of this file, each of the pandeiro's strokes alone besides it.
pub(super) fn all() -> Vec<Synth> {
    let mut synths = vec![
        pandeiro("pandeiro", None),
        surdo(),
        cuica(),
        tamborim(),
        caxixi(),
    ];
    for stroke in PANDEIRO.iter().map(|s| s.name) {
        synths.push(pandeiro(&format!("pandeiro_{stroke}"), Some(stroke)));
    }
    synths
}

struct Stroke {
    name: &'static str,
    gain: f64,
    head: [f64; 5],
    rings: [f64; 5],
    crack: [f64; 3],
    jingle: [f64; 2],
    length: f64,
}

const PANDEIRO: [Stroke; 4] = [
    Stroke {
        name: "thumb",
        gain: 0.0507,
        head: [1.0, 0.5, 0.18, 0.1, 0.05],
        rings: [0.55, 0.8, 0.6, 0.3, 0.2],
        crack: [3.0, 0.15, 8.0],
        jingle: [1.0, 0.5],
        length: 1.6,
    },
    Stroke {
        name: "heel",
        gain: 0.074,
        head: [0.25, 1.0, 0.45, 0.2, 0.12],
        rings: [0.12, 0.5, 0.3, 0.2, 0.15],
        crack: [10.0, 0.15, 8.0],
        jingle: [3.5, 0.4],
        length: 1.0,
    },
    Stroke {
        name: "slap",
        gain: 0.221,
        head: [0.01, 0.015, 0.015, 0.01, 0.01],
        rings: [0.6, 0.6, 0.5, 0.4, 0.3],
        crack: [6.0, 0.13, 5.0],
        jingle: [1.5, 0.3],
        length: 0.8,
    },
    Stroke {
        name: "jingles",
        gain: 0.582,
        head: [0.035, 0.025, 0.012, 0.006, 0.003],
        rings: [0.4, 0.4, 0.3, 0.2, 0.1],
        crack: [1.0, 0.08, 10.0],
        jingle: [1.0, 0.35],
        length: 1.2,
    },
];

/// What a fully muted head's modes are multiplied by: their pitch, a whole tone up, and the seconds they ring; chosen by ear, not measured.
const PANDEIRO_MUTED: [f64; 2] = [1.1225, 0.2];

/// The pandeiro, whose `stroke` option picks the stroke; `fixed` makes one of the old names that each played one stroke.
fn pandeiro(name: &str, fixed: Option<&'static str>) -> Synth {
    let mut decl = Synth::new(name).options(strike_options(49.0)).option(
        Spec::float("muted")
            .default(0.0)
            .range(0.0, 1.0)
            .doc("1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter."),
    );
    if fixed.is_none() {
        let names: Vec<&str> = PANDEIRO.iter().map(|s| s.name).collect();
        decl = decl.option(Spec::name("stroke", &names).default("thumb").doc(
            "The stroke: the thumb, the heel of the hand, a slap or the fingertips on the jingles.",
        ));
    }
    decl.build(move |o, ctx| {
        let picked = fixed.unwrap_or_else(|| o.name("stroke"));
        let s = PANDEIRO
            .iter()
            .find(|s| s.name == picked)
            .expect("a stroke the option allows");
        let vel = vel(o, 0.0);
        let freq = hz(o.number("note"));
        let mute = o.number("muted").clamp(0.0, 1.0);
        let pressed = freq * PANDEIRO_MUTED[0].powf(mute);
        let rings = s.rings.map(|ring| ring * PANDEIRO_MUTED[1].powf(mute));
        let stick =
            (impulse_net() + noise(ctx) * hit(0.0005, 0.004, -4.0)) >> lpf(800.0 + vel * 4000.0);
        let skin = modes(
            stick,
            &scaled(&[0.974, 1.652, 2.475, 3.25, 4.13], pressed),
            &s.head,
            &rings,
        );
        let body = (noise(ctx) >> bpf(freq * s.crack[2], 2.0))
            * (hit(0.001, s.crack[1], -4.0) + hit(0.001, s.crack[1] * 4.0, -4.0) * 0.06)
            * s.crack[0] as f32;
        let shaking = exp_segments(&[0.001, 1.0, 0.001], &[0.001, s.jingle[1]]);
        let rattle = noise(ctx) * shaking;
        let disc_ratios = [
            22.24, 24.01, 25.11, 29.26, 32.13, 35.68, 40.93, 83.71, 85.83, 87.68, 91.32, 92.08,
            102.31, 106.03,
        ];
        let disc_levels = [
            0.25, 0.15, 0.15, 0.2, 0.25, 0.15, 0.15, 0.85, 1.0, 0.75, 0.55, 0.6, 0.6, 0.8,
        ];
        let disc_rings = [
            0.04, 0.04, 0.04, 0.04, 0.04, 0.04, 0.04, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08,
        ];
        let rattling = rattle >> (net(pass()) ^ net(pass()));
        let discs = rattling
            >> (((net(pass()) * 0.02)
                >> modes(
                    net(pass()),
                    &scaled(&disc_ratios, freq),
                    &disc_levels,
                    &disc_rings,
                ))
                | (hpf(freq * 20.0) * 0.3));
        let jingles = (discs >> net(map(|x: &Frame<f32, U2>| x[0] + x[1]))) * s.jingle[0] as f32;
        let bright = (skin + body + jingles) >> high_shelf(2000.0, (vel - 0.6) * 5.0);
        Note::rings(bright * (s.gain * (vel / 0.6).powf(0.75)) as f32, s.length)
    })
}

/// The surdo de primeira's open stroke rings on its lowest mode, 66.6 Hz in plotzki's loops, with the mode at twice it 10 dB down; the muted stroke, the hand on the head, loses that mode and rings higher and shorter.
/// Each set is ratios to the open stroke's lowest mode, levels in dB and seconds to fade 60 dB; above them, modes in each band from 250 Hz to 4 kHz fill the felt mallet's thud.
const SURDO_OPEN: [[f64; 8]; 3] = [
    [1.0, 1.69, 2.0, 2.38, 2.61, 3.0, 3.32, 4.12],
    [0.0, -26.0, -15.5, -16.0, -12.5, -18.5, -21.5, -24.5],
    [1.45, 1.25, 1.25, 1.1, 1.1, 1.0, 1.0, 1.2],
];
const SURDO_MUTED: [[f64; 8]; 3] = [
    [1.82, 2.03, 2.36, 2.57, 2.76, 3.11, 5.01, 5.71],
    [0.0, -6.6, -7.2, -8.3, -0.4, -3.7, 0.0, -6.5],
    [0.52, 0.68, 0.64, 0.64, 0.67, 0.69, 0.44, 0.48],
];
/// The higher modes' ratios, their levels in dB as struck and the seconds they fade 60 dB in, at each band from 250 Hz to 4 kHz; each leaves a tail 20 dB down that fades as slowly as the recordings' bands.
const SURDO_HIGH: [[f64; 6]; 4] = [
    [4.9, 7.6, 12.0, 19.0, 30.0, 47.5],
    [-24.0, -12.3, -5.5, -23.0, -37.0, -34.0],
    [0.5, 0.25, 0.25, 0.2, 0.2, 0.2],
    [1.2, 2.4, 2.4, 2.4, 2.5, 3.4],
];

fn surdo() -> Synth {
    Synth::new("surdo")
        .options(strike_options(36.31))
        .option(
            Spec::float("muted")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 mutes the head with the hand, so it rings higher and shorter."),
        )
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let mute = o.number("muted").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mallet = impulse_net()
                + (noise(ctx) >> lpf(1500.0 + vel * 3000.0)) * hit(0.001, 0.006, -4.0) * 0.2;
            let set = |modes: &[[f64; 8]; 3], by: f64| -> (Vec<f64>, Vec<f64>, Vec<f64>) {
                (
                    scaled(&modes[0], freq),
                    modes[1].iter().map(|l| db(*l) * by).collect(),
                    modes[2].to_vec(),
                )
            };
            let (open_hz, open_levels, open_rings) = set(&SURDO_OPEN, 1.0 - mute);
            let (muted_hz, muted_levels, muted_rings) =
                set(&SURDO_MUTED, mute * db(SURDO_MUTED_LEVEL));
            let high_hz = scaled(&SURDO_HIGH[0], freq);
            let high_tails: Vec<f64> = SURDO_HIGH[1].iter().map(|l| db(l - 20.0)).collect();
            let tail_rings: Vec<f64> = SURDO_HIGH[3]
                .iter()
                .map(|r| r * (1.0 - 0.6 * mute))
                .collect();
            let head = modes(mallet.clone(), &open_hz, &open_levels, &open_rings)
                + modes(mallet.clone(), &muted_hz, &muted_levels, &muted_rings)
                + modes(
                    mallet.clone(),
                    &high_hz,
                    &amps(&SURDO_HIGH[1]),
                    &SURDO_HIGH[2],
                )
                + modes(mallet, &high_hz, &high_tails, &tail_rings);
            let level = 0.05 * (vel / 0.6).powf(1.6);
            Note::rings(
                (head >> high_shelf(800.0, (vel - 0.6) * 6.0)) * level as f32,
                3.6,
            )
        })
}

const SURDO_MUTED_LEVEL: f64 = -6.0;

/// One stroke of the cuíca, as reinsamba's pack "Cuica" on Freesound has them: the low strokes, with the head free, at note 65, and the high ones, with it pressed, at note 79, between which every figure moves with the note.
/// The levels of the 2nd to the 16th harmonics against the fundamental, in dB, every 20 ms from 25 ms before the stroke's start to 175 ms after, held after, for the low strokes and the high ones.
/// Measured over 25 ms windows of the recordings' strokes from where the pitch tracker finds them starting, a frame's length before they sound, and taken as the median of each moment: a low stroke is bright as it scoops up and darkens some 20 dB as it settles, where a high one barely changes.
const CUICA_NOTES: [f64; 2] = [65.0, 79.0];
const CUICA_TRACKS: [[[f64; 11]; 15]; 2] = [
    [
        [
            4.0, -2.0, 1.0, -1.0, -5.0, -12.0, -12.0, -14.0, -21.0, -21.0, -16.0,
        ],
        [
            -12.0, -23.0, -15.0, -19.0, -22.0, -24.0, -28.0, -30.0, -33.0, -33.0, -31.0,
        ],
        [
            -10.0, -28.0, -19.0, -19.0, -18.0, -26.0, -31.0, -32.0, -37.0, -40.0, -38.0,
        ],
        [
            -23.0, -40.0, -31.0, -34.0, -37.0, -42.0, -46.0, -47.0, -50.0, -52.0, -48.0,
        ],
        [
            -28.0, -38.0, -33.0, -30.0, -34.0, -44.0, -46.0, -47.0, -50.0, -49.0, -47.0,
        ],
        [
            -25.0, -45.0, -37.0, -44.0, -46.0, -50.0, -53.0, -54.0, -56.0, -57.0, -55.0,
        ],
        [
            -30.0, -49.0, -41.0, -40.0, -43.0, -48.0, -52.0, -53.0, -55.0, -58.0, -56.0,
        ],
        [
            -28.0, -52.0, -46.0, -45.0, -50.0, -54.0, -57.0, -60.0, -63.0, -65.0, -61.0,
        ],
        [
            -34.0, -57.0, -45.0, -47.0, -50.0, -52.0, -56.0, -60.0, -62.0, -63.0, -59.0,
        ],
        [
            -35.0, -56.0, -44.0, -42.0, -49.0, -60.0, -63.0, -67.0, -69.0, -68.0, -65.0,
        ],
        [
            -37.0, -59.0, -50.0, -50.0, -54.0, -58.0, -60.0, -65.0, -68.0, -70.0, -64.0,
        ],
        [
            -36.0, -62.0, -55.0, -54.0, -56.0, -65.0, -72.0, -75.0, -75.0, -74.0, -68.0,
        ],
        [
            -38.0, -62.0, -57.0, -58.0, -63.0, -73.0, -76.0, -76.0, -75.0, -74.0, -67.0,
        ],
        [
            -40.0, -66.0, -64.0, -64.0, -67.0, -71.0, -77.0, -77.0, -76.0, -72.0, -67.0,
        ],
        [
            -39.0, -66.0, -64.0, -63.0, -69.0, -73.0, -76.0, -77.0, -75.0, -73.0, -69.0,
        ],
    ],
    [
        [
            -13.0, -24.0, -23.0, -23.0, -25.0, -22.0, -22.0, -22.0, -21.0, -23.0, -21.0,
        ],
        [
            -19.0, -29.0, -29.0, -23.0, -19.0, -21.0, -21.0, -24.0, -22.0, -21.0, -21.0,
        ],
        [
            -32.0, -43.0, -39.0, -37.0, -34.0, -32.0, -32.0, -34.0, -36.0, -36.0, -36.0,
        ],
        [
            -39.0, -46.0, -44.0, -40.0, -40.0, -36.0, -34.0, -36.0, -36.0, -38.0, -39.0,
        ],
        [
            -44.0, -53.0, -52.0, -49.0, -45.0, -48.0, -49.0, -50.0, -50.0, -49.0, -50.0,
        ],
        [
            -46.0, -60.0, -60.0, -55.0, -52.0, -52.0, -52.0, -52.0, -55.0, -55.0, -56.0,
        ],
        [
            -48.0, -62.0, -62.0, -60.0, -58.0, -58.0, -58.0, -59.0, -59.0, -58.0, -59.0,
        ],
        [
            -45.0, -64.0, -64.0, -64.0, -60.0, -57.0, -57.0, -57.0, -58.0, -60.0, -61.0,
        ],
        [
            -45.0, -65.0, -67.0, -67.0, -65.0, -64.0, -64.0, -63.0, -62.0, -60.0, -60.0,
        ],
        [
            -44.0, -64.0, -65.0, -64.0, -58.0, -52.0, -51.0, -50.0, -54.0, -54.0, -56.0,
        ],
        [
            -34.0, -59.0, -63.0, -67.0, -66.0, -66.0, -67.0, -64.0, -64.0, -60.0, -61.0,
        ],
        [
            -43.0, -63.0, -71.0, -71.0, -67.0, -65.0, -65.0, -64.0, -63.0, -62.0, -61.0,
        ],
        [
            -44.0, -64.0, -67.0, -68.0, -67.0, -65.0, -66.0, -65.0, -63.0, -62.0, -61.0,
        ],
        [
            -43.0, -65.0, -69.0, -72.0, -69.0, -68.0, -67.0, -67.0, -64.0, -64.0, -61.0,
        ],
        [
            -43.0, -66.0, -70.0, -74.0, -70.0, -69.0, -67.0, -66.0, -65.0, -64.0, -62.0,
        ],
    ],
];
const CUICA_TRACK_STEP: f64 = 0.02;

/// The cents under the note the stroke starts at, the seconds it holds there as the rub catches, and the seconds it then scoops up into the note.
const CUICA_SCOOPS: [[f64; 3]; 2] = [[198.0, 0.03, 0.08], [140.0, 0.02, 0.06]];
/// The cents the pitch sags as the stroke ends, and the seconds into the stroke it starts to, sagging over 50 ms.
const CUICA_SAGS: [[f64; 2]; 2] = [[30.0, 0.17], [40.0, 0.15]];
/// The stroke's level and the seconds between, a 30 ms rise and a hold that the low strokes keep longer than the high ones, then the head ringing on from 25 dB down and fading about 30 dB in 100 ms as the stick turns.
const CUICA_LEVELS: [[f64; 7]; 2] = [
    [0.14, 0.9, 1.0, 0.56, 0.18, 0.056, 0.0],
    [0.16, 0.8, 1.0, 0.4, 0.25, 0.056, 0.0],
];
const CUICA_TIMES: [[f64; 6]; 2] = [
    [0.03, 0.06, 0.06, 0.05, 0.02, 0.15],
    [0.03, 0.02, 0.06, 0.055, 0.02, 0.15],
];
/// No two of a player's strokes match: across the recordings' strokes the scoop is up to two thirds shallower than its median but seldom more than a quarter deeper, the length by a third, the pitch it settles at by 77 cents, of which a stroke here keeps 30, and the level by 2 dB.
const CUICA_SPREADS: [f64; 4] = [0.65, 0.3, 30.0, 2.0];
const CUICA_DEEPER: f64 = 0.4;
/// The pitch wobbles about 13 cents as the stick grips and slips, and the rub's noise sits about 60 dB under the fundamental between its harmonics.
const CUICA_WOBBLE: f32 = 1600.0;
const CUICA_RUB: f64 = -32.0;

fn cuica() -> Synth {
    let strokes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    Synth::new("cuica")
        .options(strike_options(CUICA_NOTES[0]))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let note = o.number("note");
            let stroke = strokes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let seed = ctx.seed() ^ stroke.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let mut draws = Draws::new(seed);
            let mut normal = || {
                (-2.0 * draws.draw().max(1.0e-12).ln()).sqrt()
                    * (std::f64::consts::TAU * draws.draw()).cos()
            };
            let spread: Vec<f64> = CUICA_SPREADS.iter().map(|sd| sd * normal()).collect();
            let high =
                ((note - CUICA_NOTES[0]) / (CUICA_NOTES[1] - CUICA_NOTES[0])).clamp(0.0, 1.0);
            let mix = |pair: [f64; 2]| pair[0] + (pair[1] - pair[0]) * high;
            let under = mix([CUICA_SCOOPS[0][0], CUICA_SCOOPS[1][0]])
                * (1.0 + spread[0] * if spread[0] > 0.0 { CUICA_DEEPER } else { 1.0 }).max(0.15);
            let (hold, scoop) = (
                mix([CUICA_SCOOPS[0][1], CUICA_SCOOPS[1][1]]),
                mix([CUICA_SCOOPS[0][2], CUICA_SCOOPS[1][2]]),
            );
            let longer = spread[1].exp().clamp(0.7, 1.5);
            let (sag, sags) = (
                mix([CUICA_SAGS[0][0], CUICA_SAGS[1][0]]),
                mix([CUICA_SAGS[0][1], CUICA_SAGS[1][1]]) * longer,
            );
            let smooth = |x: f64| x * x * (3.0 - 2.0 * x);
            let settled = hz(note + spread[2] / 100.0);
            let glide = shape(move |t| {
                let risen = smooth(((t - hold) / scoop).clamp(0.0, 1.0));
                let sagged = smooth(((t - sags) / 0.05).clamp(0.0, 1.0));
                settled * 2f64.powf(-(under * (1.0 - risen) + sag * sagged) / 1200.0)
            });
            let wobble = white_noise(seed ^ 0x77)
                >> lpf(6.0)
                >> Net::wrap(Box::new(map(|x: &Frame<f32, U1>| {
                    2f32.powf(CUICA_WOBBLE * x[0] / 1200.0)
                })));
            let pitch = glide * wobble;
            let bright = (vel - 0.6) * 4.0;
            let mut tone = pitch.clone() >> Net::wrap(Box::new(sine::<f32>()));
            for k in (2..=16).filter(|k| *k as f64 * settled < 16_000.0) {
                let track: Vec<f64> = (0..11)
                    .map(|i| {
                        mix([CUICA_TRACKS[0][k - 2][i], CUICA_TRACKS[1][k - 2][i]])
                            + bright * (k as f64).log2().min(2.0)
                    })
                    .collect();
                let level = shape(move |t| {
                    let at = ((t + 0.025) / CUICA_TRACK_STEP).clamp(0.0, 10.0);
                    let i = Ord::min(at as usize, 9);
                    db(track[i] + (track[i + 1] - track[i]) * (at - i as f64))
                });
                tone = tone
                    + ((pitch.clone() * k as f32) >> Net::wrap(Box::new(sine::<f32>()))) * level;
            }
            let rub = (white_noise(seed ^ 0x55) >> lpf(4000.0)) * db(CUICA_RUB) as f32;
            let stroke: Vec<f64> = (0..7)
                .map(|i| mix([CUICA_LEVELS[0][i], CUICA_LEVELS[1][i]]))
                .collect();
            let times: Vec<f64> = (0..6)
                .map(|i| {
                    mix([CUICA_TIMES[0][i], CUICA_TIMES[1][i]])
                        * if i == 0 || i == 5 { 1.0 } else { longer }
                })
                .collect();
            let length: f64 = times.iter().sum();
            let envelope = segments(&stroke, &times, &[0.0, 0.0, 0.0, 0.0, 0.0, -4.0]);
            let level = CUICA_LEVEL * (vel / 0.6).powf(1.5) * db(spread[3]);
            Note::rings((tone + rub) * envelope * level as f32, length + 0.01)
        })
}

/// The stroke's level, as loud as the first design's.
const CUICA_LEVEL: f64 = 0.05;

/// One stroke of the stick on the tamborim: its head rings on its lowest mode, 524 Hz in the Commons recording, with strong modes near 1.8 and 2.5 times it, the second outlasting the first; two quiet modes below it and higher modes with the stick's crack fill the bands from 160 Hz to 10 kHz.
/// Each mode is a ratio to the lowest, a level in dB and the seconds it fades 60 dB in.
const TAMBORIM: [[f64; 13]; 3] = [
    [
        0.38, 0.55, 1.0, 1.8, 2.5, 3.2, 4.0, 5.0, 6.3, 8.0, 10.0, 13.0, 17.0,
    ],
    [
        -26.0, -17.0, 0.0, -2.0, -3.5, -8.0, -9.0, -10.0, -11.0, -15.0, -18.0, -18.0, -22.0,
    ],
    [
        0.68, 0.58, 0.45, 0.85, 0.9, 0.85, 0.75, 0.7, 0.7, 0.6, 0.5, 0.45, 0.4,
    ],
];
const TAMBORIM_CRACK: f64 = -12.0;

fn tamborim() -> Synth {
    Synth::new("tamborim")
        .options(strike_options(72.03))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let freq = hz(o.number("note"));
            let stick =
                impulse_net() + (noise(ctx) >> hpf(2000.0)) * hit(0.0003, 0.004, -4.0) * 0.3;
            let head = modes(
                stick,
                &scaled(&TAMBORIM[0], freq),
                &amps(&TAMBORIM[1]),
                &TAMBORIM[2],
            );
            let crack = (noise(ctx) >> hpf(3000.0) >> lpf(12000.0))
                * hit(0.0005, 0.03, -4.0)
                * db(TAMBORIM_CRACK) as f32;
            let level = 0.02 * (vel / 0.6).powf(1.4);
            Note::rings(
                ((head + crack) >> high_shelf(2000.0, (vel - 0.6) * 6.0)) * level as f32,
                1.4,
            )
        })
}

/// The caxixi's bands from 400 Hz: their lowest edges, levels in dB, and each shake's envelope, a fast fall over its first tens of milliseconds plus a quieter tail, as gain in dB and the two time constants.
const CAXIXI: [[f64; 8]; 5] = [
    [
        400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0, 10000.0,
    ],
    [-36.0, -28.0, -14.5, -5.8, -5.4, -3.6, 1.5, -4.0],
    [0.027, 0.027, 0.027, 0.026, 0.036, 0.034, 0.022, 0.019],
    [-15.0, -15.0, -15.0, -16.7, -23.5, -28.0, -26.2, -28.5],
    [0.9, 0.9, 0.9, 0.42, 0.44, 0.41, 0.21, 0.33],
];
const CAXIXI_PEAK: f64 = 8.0;

fn caxixi() -> Synth {
    let shakes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    Synth::new("caxixi")
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard it is shaken."),
        )
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            // Each shake throws the seeds differently, so its strongest peak moves, as the recording's does from 1.7 to 9.6 kHz.
            let shake = shakes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let seed = ctx.seed() ^ shake.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let mut draws = Draws::new(seed);
            let top = 1734.0 * (9556.0f64 / 1734.0).powf(draws.draw().powf(1.6));
            let mut seeds = Net::wrap(Box::new(zero()));
            for band in 0..8 {
                let low = CAXIXI[0][band];
                let high = if band == 7 {
                    16000.0
                } else {
                    CAXIXI[0][band + 1]
                };
                let centre = (low * high).sqrt();
                let (fast, tail, slow) = (CAXIXI[2][band], db(CAXIXI[3][band]), CAXIXI[4][band]);
                let envelope = shape(move |t| {
                    (t / 0.002).min(1.0) * ((-t / fast).exp() + tail * (-t / slow).exp())
                });
                seeds = seeds
                    + (white_noise(seed.wrapping_add(band as u64))
                        >> bpf(centre, (high - low) / centre))
                        * envelope
                        * db(CAXIXI[1][band]) as f32;
            }
            let level = 0.3 * (vel / 0.6).powf(1.2);
            Note::rings(
                (seeds >> hpf(700.0) >> hpf(700.0) >> peak(top, 0.25, CAXIXI_PEAK)) * level as f32,
                3.0,
            )
        })
}

/// The seconds a stroke lasts.
const LENGTH: f64 = 2.9;

const RATIOS: [f64; 19] = [
    2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 9.0, 10.0, 12.0, 14.0, 16.0, 19.35, 20.38, 21.0, 21.47, 35.0,
    37.0, 39.0, 44.0,
];
const OPEN: [f64; 19] = [
    -10.0, 0.0, -15.5, 0.0, -8.0, -10.0, -10.0, -8.0, -14.6, -13.5, -11.6, -12.0, -12.0, -9.9,
    -12.0, -15.4, -10.8, -12.7, -11.6,
];
const PRESSED: [f64; 19] = [
    -30.0, 0.0, -5.4, -7.6, -7.9, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0,
    -30.0, -5.0, -3.1, -7.1, -6.0,
];
const RINGS: [f64; 19] = [
    2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 0.8,
];

/// The wire's bands, and its modes' level in dB, ring in seconds and tail in dB at each.
const BANDS: [f64; 7] = [794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0, 12649.0];
const WIRE_OPEN: [f64; 7] = [-14.0, -8.0, -6.0, 17.0, 8.0, 6.0, 3.0];
const WIRE_PRESSED: [f64; 7] = [-14.0, -8.0, -3.0, 15.0, 10.0, 6.0, 5.0];
const WIRE_RINGS: [f64; 7] = [0.6, 0.6, 0.5, 0.25, 0.08, 0.07, 0.06];
const TAILS: [f64; 7] = [-60.0, -60.0, -60.0, -40.0, -22.0, -28.0, -24.0];
/// Seeds the wire's modes; of the seeds tried, 7000 keeps them under the string's 3rd and 5th harmonics, as scsynth's draws from 1956 are.
const SEED: u64 = 7000;
/// What the wire's levels take off at each band to sound as loud as scsynth's render: its modes are drawn from another generator than scsynth's.
const WIRE_FIT: [f64; 7] = [-10.0, -15.8, -12.9, -14.3, -7.0, -6.6, -12.8];

/// The berimbau: the string's partials struck by the stick, swung by the gourd, over the wire's close modes and the stick's thud.
/// Its `note` is the open string's pitch and its `vel` how hard the stick strikes; `coin` presses the wire, `buzz` loosens it and `gourd` swings the gourd.
pub fn berimbau() -> Synth {
    Synth::new("berimbau")
        .option(
            Spec::note("note")
                .default(39.11)
                .doc("The open string's pitch."),
        )
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the stick strikes."),
        )
        .option(
            Spec::float("coin")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How hard the coin presses the wire, raising it a whole tone."),
        )
        .option(
            Spec::float("buzz")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How loosely the coin rests on the wire, rattling against it."),
        )
        .option(
            Spec::float("gourd")
                .default(2.3)
                .range(0.0, 20.0)
                .doc("How many times a second the gourd swings against the belly."),
        )
        .build(|options, _ctx| {
            let vel = options.number("vel");
            let buzz = options.number("buzz");
            let coin = options.number("coin").max(buzz);
            let mix = |a: f64, b: f64| a + (b - a) * coin;
            let freq = hz(options.number("note")) * 1.0621f64.powf(coin);
            let damp = 1.0 - buzz * 0.96;
            let gourd = options.number("gourd");

            // The gourd starts open and swings some harmonics up and others down as it closes against the belly.
            let partials = (0..RATIOS.len()).map(|i| {
                let partial = struck(
                    freq * RATIOS[i],
                    db(mix(OPEN[i], PRESSED[i])),
                    RINGS[i] * damp,
                );
                let swing = match RATIOS[i] as u32 {
                    2 => -4.5,
                    3 => -7.0,
                    5 => 12.0,
                    6 => 13.0,
                    _ => 0.0,
                };
                if gourd > 0.0 && swing != 0.0 {
                    partial * shape(move |t| db((2.0 * PI * gourd * t).cos() * swing))
                } else {
                    partial
                }
            });
            let stick = Net::wrap(Box::new(impulse::<U1>() >> lowpass_hz(5000.0, 0.707)));
            let string = stick >> partials.reduce(|a, b| a & b).unwrap();

            // 96 close modes of the wire from 700 Hz up, seeded so every stroke is the same; every fourth rings on quietly.
            let at = |f: f64, table: &[f64; 7]| {
                let f = f.clamp(BANDS[0], BANDS[6]);
                let i = Ord::min(BANDS.iter().rposition(|&b| b <= f).unwrap(), 5);
                table[i] + (table[i + 1] - table[i]) * (f - BANDS[i]) / (BANDS[i + 1] - BANDS[i])
            };
            let wire_levels: [f64; 7] =
                std::array::from_fn(|i| mix(WIRE_OPEN[i], WIRE_PRESSED[i]) + WIRE_FIT[i]);
            let wire_modes = (0..96).map(|i| {
                let f = 700.0
                    * (16000.0f64 / 700.0).powf((i as f64 + rnd1(2 * i as u64 + SEED)) / 96.0);
                let spread = 0.2 * 30f64.powf(rnd1(2 * i as u64 + SEED + 1));
                let long = i % 4 == 0;
                let loud =
                    db(at(f, &wire_levels)) * if long { db(at(f, &TAILS) + 10.0) } else { 1.0 };
                let ring = if long {
                    1.5
                } else {
                    (at(f, &WIRE_RINGS) * spread).min(2.5)
                };
                struck(f, loud, ring * damp.sqrt())
            });
            let strike = white_noise(1) * shape(|t| perc(t, 0.0002, 0.002)) * 0.6;
            let wire = strike >> wire_modes.reduce(|a, b| a & b).unwrap();

            // The stick's dull thud, in three bands of noise that rise at once and fall away exponentially.
            let thud = [
                (200.0, -20.0, 0.3),
                (316.0, -20.0, 0.3),
                (502.0, -10.0, 0.5),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (centre, level, fade))| {
                let fade = fade * damp.sqrt();
                let fall = shape(move |t| {
                    if t < 0.001 {
                        t / 0.001
                    } else if t < 0.001 + fade {
                        0.001f64.powf((t - 0.001) / fade)
                    } else {
                        0.001 * (1.0 - (t - 0.001 - fade) / 0.01).max(0.0)
                    }
                });
                let q: f64 = 1.0 / 0.35;
                // FunDSP's bandpass passes its centre at a gain of q, taken off here.
                let band = white_noise(2 + i as u64)
                    >> bandpass_hz(centre, q)
                    >> bandpass_hz(centre, q)
                    >> bandpass_hz(centre, q);
                band * fall * (db(level) / q.powi(3)) as f32
            });
            let thud = thud.reduce(|a, b| a + b).unwrap();
            let tick = Net::wrap(Box::new(
                impulse::<U1>() >> (highpass_hz(200.0, 0.707) * 0.1),
            ));
            let mut sound = string * 0.05 + thud + wire * 0.05 + tick;

            // With buzz, the coin rests on the wire and rattles against it.
            if buzz > 0.0 {
                let rattle = ((dc((freq as f32, 0.08)) >> pulse()) * 0.5 + 0.42)
                    >> highpass_hz(1200.0, 0.707);
                let rattle = Net::wrap(Box::new(rattle))
                    * shape(|t| perc(t, 0.002, 0.12))
                    * (buzz * 0.3) as f32;
                sound = sound + rattle;
            }

            let brighter = db((vel - 0.6) * 5.0) as f32;
            let level = (0.2 * (vel / 0.6).powf(1.5)) as f32;
            Note::rings(
                (sound >> highshelf_hz(1500.0, 0.707, brighter)) * level,
                LENGTH,
            )
        })
}
