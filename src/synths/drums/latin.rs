//! Latin hand percussion: the cowbell, clave, agogos, ganza, shaker, congas, timbales, guiro, triangle, tambourine, cabasa, cajon and bongos.

use super::{
    HAT_BANDS, SKIN_BANDS, bpf, high_shelf, hpf, impulse_net, low_shelf, lpf, net, noise, peak,
    plate, resonant_highpass, resonant_lowpass, scaled, strike_options, times, vel,
};
use crate::nodes::{Draws, between, db, decay2, dust, exp_segments, hit, hz, modes, segments};
use crate::{Context, Note, Spec, Synth};
use fundsp::prelude::*;

/// The synths of this file.
pub(super) fn all() -> Vec<Synth> {
    vec![
        cowbell(),
        clave(),
        agogo("low_agogo", &LOW_AGOGO),
        agogo("high_agogo", &HIGH_AGOGO),
        shake("ganza", &GANZA),
        shake("shaker", &SHAKER),
        conga("low_conga", &LOW_CONGA),
        conga("mid_conga", &MID_CONGA),
        conga("high_conga", &HIGH_CONGA),
        timbale("low_timbale", 62.0),
        timbale("high_timbale", 67.0),
        timbale_shell(),
        guiro(),
        triangle(),
        tambourine("tambourine", false),
        tambourine("tambourine_shake", true),
        cabasa(),
        cajon(),
        bongo("low_bongo", &LOW_BONGO),
        bongo("high_bongo", &HIGH_BONGO),
    ]
}

fn cowbell() -> Synth {
    Synth::new("cowbell")
        .options(strike_options(70.07))
        .option(
            Spec::float("muted")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 damps the bell with the hand."),
        )
        .build(|o, ctx| {
            let vel = vel(o, 0.05);
            let muted = o.number("muted").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let bright = vel - 0.6;
            let strike =
                impulse_net() + (noise(ctx) >> hpf(3000.0)) * hit(0.0002, 0.002, -4.0) * 0.3;
            let ratios = [
                0.925, 0.955, 1.0, 1.034, 1.07, 1.093, 2.09, 2.115, 3.016, 3.63, 3.88, 4.04, 5.43,
                7.41, 9.4, 10.5, 12.6, 14.7, 17.0, 21.0,
            ];
            let levels = [
                -26.0, -18.0, 0.0, -1.0, -7.0, -12.0, -18.0, -13.0, -27.0, -22.0, -14.0, -18.0,
                -7.0, -15.0, -20.0, -21.0, -29.0, -31.0, -33.0, -38.0,
            ];
            let tone = [
                0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -11.0, -11.0, -11.0, -18.0, -18.0,
                7.0, 7.0, 7.0, 7.0, 7.0, 7.0,
            ];
            let mute = [
                -8.0, -8.0, -8.0, -8.0, -8.0, -8.0, -12.0, -12.0, 7.0, 7.0, 7.0, 7.0, 4.0, 4.0,
                11.0, 11.0, 11.0, 19.0, 19.0, 19.0,
            ];
            let worn = 0.6 + 0.4 * vel;
            let rings = [
                0.8,
                0.8,
                0.8,
                0.8,
                0.8,
                0.8,
                0.65,
                0.65,
                1.35,
                1.05,
                1.05,
                1.05,
                0.9 * worn,
                0.9 * worn,
                0.9,
                0.9,
                0.9,
                0.75,
                0.75,
                0.6,
            ];
            let a: Vec<f64> = (0..20)
                .map(|i| db(levels[i] + tone[i] * bright + mute[i] * muted))
                .collect();
            let f = scaled(&ratios, freq);
            let bell = modes(strike.clone(), &f, &a, &times(&rings, 0.13))
                + modes(
                    strike,
                    &f,
                    &times(&a, 0.06),
                    &times(&rings, 1.0 - muted * 0.1),
                );
            Note::rings(bell * (db(-27.5) * (vel / 0.6).powf(2.3)) as f32, 1.5)
        })
}

fn clave() -> Synth {
    Synth::new("clave")
        .options(strike_options(95.2))
        .build(|o, ctx| {
            let vel = vel(o, 0.05);
            let freq = hz(o.number("note"));
            let bright = (vel - 0.6) * 30.0;
            let strike =
                impulse_net() + (noise(ctx) >> hpf(1000.0)) * hit(0.0002, 0.002, -4.0) * 0.3;
            let ratios = [0.65, 1.0, 1.08, 1.5, 2.72, 3.6, 6.2];
            let levels: Vec<f64> = [-24.0, 0.0, -6.0, -6.0, -9.5, -20.0, -22.0]
                .iter()
                .zip([0.0, 0.0, 0.0, 0.6, 0.6, 1.3, 1.0])
                .map(|(l, b)| db(l + b * bright))
                .collect();
            let rings = [0.75, 0.75, 0.75, 0.72, 0.65, 0.5, 0.4];
            let f = scaled(&ratios, freq);
            let stick = modes(strike.clone(), &f, &levels, &times(&rings, 0.08))
                + modes(strike, &f, &times(&levels, 0.1), &rings);
            Note::rings(stick * (db(-21.8) * (vel / 0.6).powf(1.72)) as f32, 1.0)
        })
}

struct Agogo {
    note: f64,
    gain: f64,
    strike: [f64; 3],
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    length: f64,
}

const LOW_AGOGO: Agogo = Agogo {
    note: 84.0,
    gain: 0.026,
    strike: [7.0, 7800.0, 0.0],
    ratios: &[
        0.953, 1.0, 2.197, 2.197, 2.85, 3.249, 3.763, 3.91, 4.021, 4.657, 5.848, 6.488, 7.3, 8.1,
        9.39,
    ],
    levels: &[
        0.3, 1.0, 0.5, 0.2, 0.65, 0.8, 0.3, 0.25, 0.45, 0.4, 0.5, 0.5, 0.6, 0.45, 0.45,
    ],
    rings: &[
        2.6, 2.0, 0.25, 1.0, 0.7, 1.0, 0.62, 0.6, 0.72, 0.52, 0.57, 0.45, 0.4, 0.35, 0.29,
    ],
    length: 2.6,
};

const HIGH_AGOGO: Agogo = Agogo {
    note: 90.07,
    gain: 0.035,
    strike: [4.0, 20000.0, 6.0],
    ratios: &[
        0.671, 0.704, 1.0, 1.47, 2.206, 3.226, 3.572, 4.013, 4.598, 5.24, 5.9, 6.6,
    ],
    levels: &[
        0.04, 0.04, 1.0, 0.1, 0.25, 0.4, 0.1, 0.35, 0.8, 0.6, 0.5, 0.45,
    ],
    rings: &[
        0.75, 1.6, 0.56, 0.75, 1.05, 0.85, 0.65, 0.62, 0.42, 0.5, 0.38, 0.34,
    ],
    length: 1.5,
};

fn agogo(name: &'static str, a: &'static Agogo) -> Synth {
    Synth::new(name)
        .options(strike_options(a.note))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let freq = hz(o.number("note"));
            let click =
                impulse_net() + (noise(ctx) >> hpf(3000.0)) * hit(0.0002, 0.002, -4.0) * 0.5;
            let stick = click >> lpf(4000.0 + vel * 12000.0);
            let bell = modes(stick, &scaled(a.ratios, freq), a.levels, a.rings);
            let hiss = (noise(ctx) >> hpf(1200.0) >> hpf(1200.0))
                * exp_segments(&[0.001, 1.0, 0.001], &[0.0005, 0.2])
                * a.strike[0] as f32;
            let thump = (noise(ctx) >> bpf(freq * 0.47, 0.8))
                * exp_segments(&[0.001, 1.0, 0.001], &[0.001, 0.15])
                * a.strike[2] as f32;
            let top = (a.strike[1] * (0.4 + vel)).clamp(1000.0, 20000.0);
            let sound = (bell + (hiss >> lpf(top)) + thump)
                * (a.gain * vel.powf(1.4) / 0.6f64.powf(1.4)) as f32;
            Note::rings(sound, a.length)
        })
}

struct Shake {
    note: f64,
    gain: f64,
    down: [f64; 11],
    upward: [f64; 11],
    stages: usize,
    dip: f64,
    darken: f64,
    length: f64,
}

const GANZA: Shake = Shake {
    note: 111.0,
    gain: 0.33,
    down: [
        0.045, 0.72, 2.6, 1.45, -8.0, 0.21, 0.72, 1.0, 1.0, -4.0, 5.0,
    ],
    upward: [0.025, 0.2, 2.4, 1.4, -12.0, 0.13, 0.2, 1.0, 1.0, -6.0, 5.0],
    stages: 1,
    dip: 0.75,
    darken: 1.0,
    length: 0.26,
};

const SHAKER: Shake = Shake {
    note: 103.0,
    gain: 0.3,
    down: [0.02, 0.85, 2.6, 0.8, -3.0, 1.1, 0.2, 0.35, 0.65, 3.0, 5.0],
    upward: [0.15, 0.85, 1.6, 0.6, -11.0, 1.1, 0.85, 1.0, 0.9, -6.0, 5.0],
    stages: 3,
    dip: 0.6,
    darken: 0.45,
    length: 1.2,
};

fn shake(name: &'static str, s: &'static Shake) -> Synth {
    Synth::new(name)
        .options(strike_options(s.note))
        .option(
            Spec::float("up")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 shakes upward instead of down."),
        )
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let up = o.number("up").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let shape: Vec<f64> = (0..11)
                .map(|i| s.down[i] + up * (s.upward[i] - s.down[i]))
                .collect();
            let burst = segments(&[0.0, 1.0, 0.0], &[shape[0], shape[6]], &[2.0, shape[9]]);
            let tail = segments(&[0.0, 1.0], &[shape[0]], &[2.0])
                * exp_segments(&[1.0, 1.0, 0.001], &[shape[0], shape[1]]);
            let ending = segments(&[1.0, 1.0, 0.0], &[shape[5], 0.04], &[0.0, 0.0]);
            let keep = shape[7] as f32;
            let mixed = (coloured(&shape, s, freq, vel, ctx) | burst | tail)
                >> net(map(move |x: &Frame<f32, U4>| {
                    x[0] * x[2] * (1.0 - keep) + x[1] * x[3] * keep
                }));
            Note::rings(mixed * ending * (s.gain * vel / 0.6) as f32, s.length)
        })
}

/// The shaker's beads coloured as its design gives: high-passed at a bottom that sinks, dipped, low-passed at a top that falls, brightened by vel, and the same darkened, as two outputs.
fn coloured(shape: &[f64], s: &Shake, freq: f64, vel: f64, ctx: &mut Context) -> Net {
    let beads = ((net(dc(4000.0)) >> dust(ctx.seed())) >> decay2(0.0001, 0.001)) * noise(ctx)
        + noise(ctx) * 0.1;
    let lowest = freq * shape[3];
    let bottom = exp_segments(&[lowest, lowest * shape[8]], &[shape[1]])
        >> net(map(|x: &Frame<f32, U1>| x[0].clamp(20.0, 20000.0)));
    let mut colour = beads;
    for _ in 0..s.stages {
        colour = (colour | bottom.clone() | net(dc(1.0))) >> net(highpass::<f32>());
    }
    let dipped = colour >> peak((freq * s.dip).clamp(20.0, 20000.0), 0.5, shape[4]);
    let highest = freq * shape[2];
    let top = exp_segments(&[highest, highest * s.darken], &[shape[1]])
        >> net(map(|x: &Frame<f32, U1>| x[0].clamp(20.0, 20000.0)));
    let bright = ((dipped | top | net(dc(1.0))) >> net(lowpass::<f32>()))
        >> high_shelf(freq * 1.2, (vel - 0.6) * 8.0);
    bright >> (net(pass()) ^ resonant_lowpass(freq * shape[10], 1.0))
}

struct Conga {
    note: f64,
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    skin: [f64; 7],
    skin_rings: [f64; 7],
    tap: [f64; 7],
    tap_rings: [f64; 7],
    head_tap: &'static [f64],
    tap_gain: f64,
    curve: f64,
    bright: [f64; 2],
    gain: f64,
}

const LOW_CONGA: Conga = Conga {
    note: 48.93,
    ratios: &[0.558, 0.78, 0.85, 1.0, 1.12, 1.525, 2.14, 2.24, 2.32, 2.42],
    levels: &[
        -7.0, -21.0, -23.0, 0.0, -27.0, -12.0, -20.0, -13.0, -14.0, -12.0,
    ],
    rings: &[0.5, 1.05, 1.05, 1.08, 1.08, 1.05, 0.8, 0.8, 0.8, 0.75],
    skin: [-23.0, -34.0, -53.5, -45.0, -37.0, -41.0, -51.0],
    skin_rings: [0.64, 0.5, 0.65, 0.7, 0.57, 0.53, 0.89],
    tap: [-14.0, -21.0, -40.0, -28.0, -38.0, -35.0, -43.0],
    tap_rings: [0.6, 0.6, 0.8, 0.8, 1.2, 1.0, 1.3],
    head_tap: &[
        -40.0, -38.0, -38.0, -41.0, -38.0, -33.0, -26.0, -26.0, -26.0, -26.0,
    ],
    tap_gain: -9.0,
    curve: 0.79,
    bright: [-6.0, 5.0],
    gain: -33.8,
};

const MID_CONGA: Conga = Conga {
    note: 51.88,
    ratios: &[
        0.487, 0.6, 0.66, 0.785, 0.944, 1.0, 1.13, 1.19, 1.627, 2.126, 2.226, 2.437, 3.5,
    ],
    levels: &[
        -5.5, -29.0, -30.0, -26.0, -19.0, 0.0, -24.0, -30.0, -20.0, -32.0, -22.0, -23.0, -16.0,
    ],
    rings: &[
        0.78, 0.9, 0.9, 1.05, 1.05, 1.05, 1.05, 1.05, 0.85, 0.62, 0.62, 0.62, 0.7,
    ],
    skin: [-22.0, -35.0, -55.0, -52.0, -45.0, -48.0, -58.0],
    skin_rings: [0.7, 0.6, 0.8, 0.8, 0.6, 0.6, 0.6],
    tap: [-12.0, -17.6, -18.2, -24.5, -14.0, -4.0, -15.0],
    tap_rings: [0.6, 0.6, 0.6, 0.7, 0.7, 0.5, 0.6],
    head_tap: &[
        -18.0, -18.0, -18.0, -18.0, -18.0, -28.0, -28.0, -28.0, -30.0, -30.0, -30.0, -30.0, -28.0,
    ],
    tap_gain: -23.1,
    curve: 1.17,
    bright: [-5.5, 3.0],
    gain: -23.3,
};

const HIGH_CONGA: Conga = Conga {
    note: 56.62,
    ratios: &[0.372, 0.856, 0.943, 1.0, 1.12, 1.618, 1.664, 2.247, 2.415],
    levels: &[-12.0, -33.0, -26.5, 0.0, -33.6, -14.5, -17.5, -25.0, -39.5],
    rings: &[0.9, 0.72, 0.71, 0.7, 0.7, 0.8, 0.8, 0.47, 0.47],
    skin: [-19.0, -34.0, -53.5, -50.0, -44.0, -46.0, -52.0],
    skin_rings: [0.48, 0.62, 0.72, 1.0, 0.85, 1.15, 1.4],
    tap: [-20.0, -28.6, -22.4, -16.0, -16.8, -2.0, -17.2],
    tap_rings: [1.3, 1.3, 1.1, 1.1, 1.2, 0.35, 1.2],
    head_tap: &[
        -11.0, -19.0, -19.0, -19.0, -19.0, -33.0, -33.0, -30.0, -30.0,
    ],
    tap_gain: -32.4,
    curve: 1.42,
    bright: [-1.5, 3.0],
    gain: -22.8,
};

fn conga(name: &'static str, c: &'static Conga) -> Synth {
    Synth::new(name)
        .options(strike_options(c.note))
        .option(
            Spec::float("mute")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 taps the head with the fingers."),
        )
        .option(
            Spec::float("slap")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 slaps the head."),
        )
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let mute = o.number("mute").clamp(0.0, 1.0);
            let slap = o.number("slap").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mix = |a: f64, b: f64| a + (b - a) * mute;
            let hand = impulse_net()
                + (noise(ctx) >> lpf(2000.0 + vel * 6000.0 + slap * 6000.0))
                    * hit(0.0005, 0.003 + slap * 0.01, -4.0)
                    * 0.3;
            let levels: Vec<f64> = c
                .levels
                .iter()
                .zip(c.head_tap)
                .map(|(l, t)| db(l + mute * t - slap * 6.0))
                .collect();
            let rings = times(c.rings, mix(1.0, 0.7) * (1.0 - slap * 0.75));
            let head = modes(hand.clone(), &scaled(c.ratios, freq), &levels, &rings);
            let skin_levels: Vec<f64> = (0..7)
                .map(|i| mix(c.skin[i], c.tap[i]) + slap * 12.0)
                .collect();
            let skin_ring: Vec<f64> = (0..7)
                .map(|i| mix(c.skin_rings[i], c.tap_rings[i]) * (1.0 - slap * 0.5))
                .collect();
            let mut skin = Net::wrap(Box::new(zero()));
            for some in plate(40, 400.0, 10000.0) {
                let l: Vec<f64> = some
                    .iter()
                    .map(|&f| db(between(f, &SKIN_BANDS, &skin_levels)))
                    .collect();
                let r: Vec<f64> = some
                    .iter()
                    .map(|&f| between(f, &SKIN_BANDS, &skin_ring))
                    .collect();
                skin = skin + modes(hand.clone(), &some, &l, &r);
            }
            let crack =
                (noise(ctx) >> bpf(3000.0, 0.7)) * hit(0.0005, 0.04, -4.0) * (slap * 4.0) as f32;
            let tone = (vel - 0.6).min(0.0) / -0.3 * c.bright[0]
                + (vel - 0.6).max(0.0) / 0.4 * c.bright[1];
            let sound = (head + skin + crack) >> peak(900.0, 2.0, tone);
            Note::rings(
                sound * (db(c.gain + mute * c.tap_gain) * (vel / 0.6).powf(c.curve)) as f32,
                1.31,
            )
        })
}

fn timbale(name: &'static str, note: f64) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(note).doc("The drum's pitch."))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let stick = (noise(ctx) >> hpf(1500.0)) * hit(0.0004, 0.008, -4.0);
            let head = modes(
                stick.clone() + impulse_net(),
                &scaled(&[1.0, 1.59, 2.14, 2.65], freq),
                &[1.0, 0.5, 0.35, 0.2],
                &[0.35, 0.22, 0.15, 0.1],
            ) * 0.09;
            let shell = modes(
                stick,
                &scaled(&[1.0, 1.47, 2.09, 2.56], freq * 4.1),
                &[0.4, 0.3, 0.2, 0.15],
                &[0.9, 0.7, 0.5, 0.4],
            ) * 0.05;
            Note::rings(head + shell, 1.2)
        })
}

fn timbale_shell() -> Synth {
    Synth::new("timbale_shell")
        .option(Spec::note("note").default(93.0).doc("The shell's pitch."))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let stick = (noise(ctx) >> hpf(3000.0)) * hit(0.0002, 0.003, -4.0);
            let shell = modes(
                stick.clone() + impulse_net(),
                &scaled(&[1.0, 1.38, 1.83, 2.41], freq),
                &[1.0, 0.6, 0.4, 0.25],
                &[0.05, 0.04, 0.03, 0.02],
            ) * 0.2;
            Note::rings(shell + stick * 0.4, 0.25)
        })
}

fn guiro() -> Synth {
    Synth::new("guiro")
        .options(strike_options(93.29))
        .option(
            Spec::beats("sustain")
                .default(0.25)
                .range(0.02, f64::INFINITY)
                .doc("How long the scrape lasts."),
        )
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let vel = vel(o, 0.05);
            let length = o.number("sustain").max(0.02);
            let bright = vel - 0.6;
            let rate = (55.0 / length).clamp(40.0, 125.0);
            // The ridges come at a rate that wanders by 15 percent, and each clicks at a level from 0.6 to 1.
            let mut draws = Draws::new(ctx.seed());
            let mut at = 0.0;
            let mut clicks: Vec<(f64, f64)> = Vec::new();
            while at < length + 0.02 {
                clicks.push((at, 0.6 + 0.4 * draws.draw()));
                at += 1.0 / (rate * (0.85 + 0.3 * draws.draw()));
            }
            let ridges = crate::nodes::clicks(clicks);
            let stroke = segments(
                &[0.0, 1.0, 1.0, 0.0],
                &[0.005, length, 0.01],
                &[0.0, 0.0, 0.0],
            );
            let ticks =
                ((ridges.clone() * 4.0) + (ridges >> decay2(0.0, 0.0015)) * noise(ctx)) * stroke;
            let ratios = [0.29, 0.37, 0.52, 0.67, 1.0, 1.53, 1.94, 3.0, 3.7, 4.4];
            let levels: Vec<f64> = [
                -22.0, -18.0, -9.0, -4.0, 0.0, -12.0, -10.0, -22.0, -32.0, -34.0,
            ]
            .iter()
            .zip([0.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0, 20.0, 20.0, 20.0])
            .map(|(l, b)| db(l + b * bright))
            .collect();
            let rings = [0.16, 0.11, 0.12, 0.16, 0.16, 0.3, 0.2, 0.25, 0.5, 0.5];
            let f = scaled(&ratios, freq);
            let gourd = (modes(ticks.clone(), &f, &levels, &times(&rings, 0.2))
                + modes(ticks, &f, &times(&levels, 0.05), &times(&rings, 4.0)))
                * 0.03;
            Note::rings(
                gourd * (db(-6.0) * (vel / 0.6).powf(1.15)) as f32,
                length + 0.5,
            )
        })
}

fn triangle() -> Synth {
    Synth::new("triangle")
        .options(strike_options(69.0))
        .option(
            Spec::beats("sustain")
                .default(30.0)
                .range(0.0, f64::INFINITY)
                .doc("How long the bar rings before the hand damps it."),
        )
        .option(
            Spec::beats("release")
                .default(1.0)
                .range(0.0, f64::INFINITY)
                .doc("How long the hand takes to damp it."),
        )
        .build(|o, ctx| {
            let vel = vel(o, 0.05);
            let scale = 2f64.powf((o.number("note") - 69.0) / 12.0);
            let (held, release) = (o.number("sustain"), o.number("release"));
            let beater = impulse_net()
                + (noise(ctx) >> hpf(3000.0 + vel * 5000.0))
                    * hit(0.0002, 0.002, -4.0)
                    * vel as f32;
            let bands = [1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0];
            let levels = [-40.0, -44.5, -21.5, -15.0, -2.0, -0.5];
            let rings = [10.0, 10.0, 9.0, 5.0, 3.0, 1.5];
            let groups = plate(120, 1000.0, 18000.0);
            let lowest = groups[0].iter().sum::<f64>() / groups[0].len() as f64;
            let mut bar = Net::wrap(Box::new(zero()));
            let mut longest: f64 = 0.0;
            for some in &groups {
                let mean = some.iter().sum::<f64>() / some.len() as f64;
                let damp = release * (mean / lowest).powf(-0.35);
                longest = longest.max(held + damp);
                let hand = exp_segments(&[1.0, 1.0, 0.001], &[held, damp]);
                let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
                let l: Vec<f64> = some
                    .iter()
                    .map(|&f| db(between(f, &bands, &levels)))
                    .collect();
                let r: Vec<f64> = some.iter().map(|&f| between(f, &bands, &rings)).collect();
                bar = bar + modes(beater.clone(), &f, &l, &r) * hand;
            }
            let hard = (vel - 0.6).max(0.0) / 0.4;
            let sound = bar >> peak(1300.0, 0.8, hard * 8.0) >> low_shelf(3500.0, hard * 5.0);
            let level = db(-46.0) * (vel / 0.6).powf(2.6);
            // The bar falls below scsynth's silence threshold well before its 10 s rings run out.
            Note::rings(sound * level as f32, longest.min(held + 12.0))
        })
}

fn tambourine(name: &'static str, shaken: bool) -> Synth {
    let mut decl = Synth::new(name).options(strike_options(69.0));
    if shaken {
        decl = decl.option(
            Spec::beats("sustain")
                .default(0.25)
                .range(0.02, f64::INFINITY)
                .doc("How long it is shaken."),
        );
    }
    decl.build(move |o, ctx| {
        let vel = vel(o, 0.05);
        let scale = 2f64.powf((o.number("note") - 69.0) / 12.0);
        let rattle = 0.15 + vel * 0.6;
        let held = if shaken {
            o.number("sustain").max(0.02)
        } else {
            0.0
        };
        let shaking = || {
            segments(
                &[0.0, 1.0, 1.0, 0.0],
                &[0.005, held, rattle],
                &[0.0, 0.0, -4.0],
            )
        };
        let clicks = ((shaking() * 400.0) >> dust(ctx.seed())) * shaking();
        let strike = if shaken {
            Net::wrap(Box::new(zero()))
        } else {
            impulse_net() + (noise(ctx) >> hpf(2000.0)) * hit(0.0005, 0.004, -4.0)
        };
        let bands = HAT_BANDS;
        let levels = [
            -29.0, -24.0, -24.8, -17.5, -13.3, -17.0, -21.4, 0.0, -5.0, 0.0,
        ];
        let rings = [0.55, 0.6, 0.7, 0.5, 0.45, 0.2, 0.15, 0.15, 0.15, 0.15];
        let exciter = strike * 0.3 + (clicks >> hpf(2500.0) >> hpf(2500.0));
        let mut jingles = Net::wrap(Box::new(zero()));
        for some in plate(120, 170.0, 16000.0) {
            let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
            let l: Vec<f64> = some
                .iter()
                .map(|&f| db(between(f, &bands, &levels)))
                .collect();
            let r: Vec<f64> = some.iter().map(|&f| between(f, &bands, &rings)).collect();
            jingles = jingles + modes(exciter.clone(), &f, &l, &r);
        }
        let sound = (jingles >> low_shelf(1000.0, (vel - 0.6).max(0.0) * 16.0))
            * (db(-36.0) * (vel / 0.6).powf(1.9)) as f32;
        Note::rings(sound, 0.005 + held + rattle + 0.7)
    })
}

fn cabasa() -> Synth {
    Synth::new("cabasa")
        .options(strike_options(96.0))
        .option(
            Spec::float("rub")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 turns the beads against the cylinder for the sustain."),
        )
        .option(
            Spec::beats("sustain")
                .default(0.1)
                .range(0.01, f64::INFINITY)
                .doc("How long a rub lasts."),
        )
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let vel = vel(o, 0.0);
            let rub = o.number("rub").clamp(0.0, 1.0);
            let hold = o.number("sustain").max(0.01) * rub;
            let burst = segments(&[0.0, 1.0, 1.0], &[0.002, hold], &[0.0, 0.0])
                * exp_segments(&[1.0, 1.0, 1.0, 0.001], &[0.002, hold, 0.22]);
            let tail = segments(&[0.0, 1.0, 1.0], &[0.002, hold], &[0.0, 0.0])
                * exp_segments(&[1.0, 1.0, 1.0, 0.001], &[0.002, hold, 0.9]);
            let beads = ((net(dc(6000.0)) >> dust(ctx.seed())) >> decay2(0.0001, 0.0008))
                * noise(ctx)
                + noise(ctx) * 0.2;
            let bottom = freq * (0.6 + rub * 0.4);
            let colour =
                beads >> resonant_highpass(bottom, 1.41) >> resonant_highpass(bottom, 1.41);
            let shell = (noise(ctx) >> bpf(freq * 0.25, 1.0)) * (0.15 * (1.0 - rub)) as f32;
            let bright = ((colour >> high_shelf(freq * 3.0, 4.0) >> peak(freq * 2.4, 1.0, -3.0))
                + shell)
                >> high_shelf(freq * 2.0, (vel - 0.6) * 6.0);
            let both = bright >> (net(pass()) ^ resonant_lowpass(freq * 3.5, 1.0));
            let level = (0.5 * (1.0 - rub * 0.52) * (vel / 0.6).powf(1.1)) as f32;
            let sound = (both | burst | tail)
                >> net(map(|x: &Frame<f32, U4>| {
                    x[0] * x[2] * 0.9 + x[1] * x[3] * 0.1
                }));
            Note::rings(sound * level, 0.002 + hold + 0.9)
        })
}

fn cajon() -> Synth {
    Synth::new("cajon")
        .options(strike_options(40.0))
        .option(
            Spec::float("slap")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 slaps the front plate instead of the bass tone."),
        )
        .build(|o, ctx| {
            let vel = vel(o, 0.0);
            let slap = o.number("slap").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mix = |bass: f64, slapped: f64| bass + (slapped - bass) * slap;
            let ringing =
                |exciter: Net, ratios: &[f64], levels: &[f64], rings: &[f64], tail: &[f64]| {
                    let f = scaled(ratios, freq);
                    let tails: Vec<f64> = levels.iter().zip(tail).map(|(l, t)| l * t).collect();
                    modes(exciter.clone(), &f, levels, &times(rings, 0.25))
                        + modes(exciter, &f, &tails, rings)
                };
            let hand = (noise(ctx) >> lpf(2000.0 + slap * 8000.0 + vel * 4000.0))
                * hit(0.0005, 0.004 + slap * 0.004, -4.0);
            let click = (noise(ctx) >> hpf(1000.0)) * hit(0.0003, 0.003, -4.0);
            let box_ = ringing(
                impulse_net() + hand.clone() * 0.3,
                &[0.983, 1.61, 2.2, 3.7],
                &[1.0, 0.45, mix(0.05, 1.1), mix(0.033, 0.23)],
                &[mix(0.46, 0.9), 0.3, mix(0.33, 0.44), mix(0.7, 0.46)],
                &[0.3, 0.3, mix(0.3, 0.15), 0.3],
            );
            let plate_ = ringing(
                hand,
                &[6.3, 6.5, 6.7, 6.9, 9.8],
                &[1.0, 1.0, 1.0, 1.0, mix(0.4, 0.55)],
                &[
                    mix(0.45, 0.26),
                    mix(0.45, 0.26),
                    mix(0.45, 0.26),
                    mix(0.45, 0.26),
                    mix(0.6, 0.32),
                ],
                &[0.15; 5],
            ) * mix(0.0014, 0.0225) as f32;
            let bass_levels = [0.3, 0.4, 0.5, 0.6, 0.25, 0.29, 0.49, 0.47, 0.47];
            let slap_levels = [2.3, 1.55, 1.6, 1.9, 0.7, 0.8, 1.16, 1.19, 1.19];
            let bass_rings = [1.0, 0.97, 0.94, 0.9, 0.85, 0.8, 0.72, 0.66, 0.6];
            let slap_rings = [0.255, 0.25, 0.28, 0.27, 0.255, 0.24, 0.216, 0.198, 0.18];
            let levels: Vec<f64> = (0..9)
                .map(|i| mix(bass_levels[i], slap_levels[i]))
                .collect();
            let rings: Vec<f64> = (0..9)
                .map(|i| mix(0.5 * bass_rings[i], slap_rings[i]))
                .collect();
            let shimmer = modes(
                click,
                &scaled(
                    &[16.0, 23.0, 31.0, 44.0, 58.0, 75.0, 97.0, 128.0, 160.0],
                    freq,
                ),
                &levels,
                &rings,
            ) * mix(0.00175, 0.0078) as f32;
            let sound = (box_ * 0.05 + plate_ + shimmer) >> high_shelf(1500.0, (vel - 0.6) * 5.0);
            Note::rings(
                sound * (mix(1.0, 0.106) * (vel / 0.6).powf(1.6)) as f32,
                1.72,
            )
        })
}

struct Bongo {
    note: f64,
    ratios: &'static [f64],
    open: &'static [f64],
    open_rings: &'static [f64],
    muted: &'static [f64],
    muted_rings: &'static [f64],
    fast: &'static [f64],
    tail: &'static [f64],
    curve: [f64; 2],
    gain: f64,
}

const LOW_BONGO: Bongo = Bongo {
    note: 60.0,
    ratios: &[
        0.46, 0.906, 1.005, 1.05, 1.548, 1.976, 2.9, 4.8, 7.6, 12.4, 19.5, 30.0,
    ],
    open: &[
        -31.8, -12.0, 0.0, -7.0, -11.5, -15.5, -21.0, -33.7, -34.0, -44.0, -45.0, -50.0,
    ],
    open_rings: &[
        1.97, 0.88, 0.91, 0.91, 0.79, 0.79, 0.68, 0.65, 0.62, 1.15, 1.26, 2.05,
    ],
    muted: &[
        -16.3, -7.0, 0.0, -7.0, -5.2, -9.2, -14.3, -35.4, -34.0, -38.0, -40.0, -45.0,
    ],
    muted_rings: &[
        0.79, 0.76, 0.79, 0.79, 0.75, 0.75, 0.7, 0.65, 0.69, 0.69, 0.63, 1.54,
    ],
    fast: &[
        0.25, 0.25, 0.18, 0.18, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25,
    ],
    tail: &[0.2; 12],
    curve: [1.95, 2.45],
    gain: 0.02,
};

const HIGH_BONGO: Bongo = Bongo {
    note: 70.0,
    ratios: &[
        0.26, 0.45, 0.63, 0.665, 0.97, 0.987, 1.7, 2.7, 4.3, 7.0, 11.0, 17.0, 27.0,
    ],
    open: &[
        -35.2, -16.0, -0.9, -0.9, -6.0, 0.0, -18.2, -20.6, -17.8, -31.8, -36.6, -40.6, -50.6,
    ],
    open_rings: &[
        6.5, 0.95, 0.98, 0.98, 0.64, 0.64, 0.59, 0.64, 0.65, 1.22, 1.8, 2.61, 3.53,
    ],
    muted: &[
        -29.0, -14.5, -1.0, -1.0, -6.0, 0.0, -15.2, -22.2, -19.3, -30.6, -30.9, -32.1, -45.7,
    ],
    muted_rings: &[
        1.59, 0.79, 0.9, 0.9, 0.68, 0.68, 0.52, 0.65, 0.71, 0.69, 0.63, 0.85, 2.49,
    ],
    fast: &[
        0.25, 0.25, 0.12, 0.12, 0.7, 0.7, 1.0, 0.25, 0.15, 0.25, 0.25, 0.25, 0.25,
    ],
    tail: &[
        0.2, 0.2, 0.14, 0.14, 0.0, 0.0, 0.0, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2,
    ],
    curve: [1.95, 1.0],
    gain: 0.017,
};

fn bongo(name: &'static str, b: &'static Bongo) -> Synth {
    Synth::new(name)
        .options(strike_options(b.note))
        .option(
            Spec::float("muted")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 mutes the head with the hand."),
        )
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let mute = o.number("muted").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mix = |a: f64, b: f64| a + (b - a) * mute;
            let hand = impulse_net()
                + (noise(ctx) >> lpf(3000.0 + vel * 6000.0)) * hit(0.0005, 0.003, -4.0) * 0.3;
            let levels: Vec<f64> = b
                .open
                .iter()
                .zip(b.muted)
                .map(|(o, m)| db(mix(*o, *m)))
                .collect();
            let rings: Vec<f64> = b
                .open_rings
                .iter()
                .zip(b.muted_rings)
                .map(|(o, m)| mix(*o, *m) * 1.28)
                .collect();
            let f = scaled(b.ratios, freq);
            let fast: Vec<f64> = rings.iter().zip(b.fast).map(|(r, f)| r * f).collect();
            let tails: Vec<f64> = levels.iter().zip(b.tail).map(|(l, t)| l * t).collect();
            let head = modes(hand.clone(), &f, &levels, &fast) + modes(hand, &f, &tails, &rings);
            let level = b.gain * (vel / 0.6).powf(mix(b.curve[0], b.curve[1]));
            Note::rings(
                (head >> high_shelf(1500.0, (vel - 0.6) * 5.0)) * level as f32,
                1.18,
            )
        })
}
