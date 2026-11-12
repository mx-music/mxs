//! The drum kit: the kick, snare, clap, crash, hi-hats, toms and rimshot.

use super::{
    HAT_BANDS, SKIN_BANDS, amps, bpf, high_shelf, hpf, impulse_net, low_shelf, lpf, net, noise,
    peak, plate, scaled, strike_options, times, vel,
};
use crate::nodes::{between, db, exp_segments, hit, hz, modes, segments};
use crate::{Note, Spec, Synth};
use fundsp::prelude::*;

/// The synths of this file.
pub(super) fn all() -> Vec<Synth> {
    vec![
        kick(),
        snare(),
        clap(),
        crash(),
        hat("closed_hihat", &CLOSED_LEVELS, &CLOSED_RINGS, 0.8, false),
        hat("open_hihat", &OPEN_LEVELS, &OPEN_RINGS, 5.0, true),
        tom("low_tom", 41.85, &LOW_TOM),
        tom("mid_tom", 43.4, &HIGH_TOM),
        tom("high_tom", 45.0, &HIGH_TOM),
        rimshot(),
    ]
}

fn kick() -> Synth {
    Synth::new("kick")
        .option(Spec::note("note").default(34.0).doc("The drum's pitch."))
        .build(|o, ctx| {
            let note = o.number("note");
            let sweep = segments(&[26.0, 0.0], &[0.07], &[-6.0])
                >> net(map(move |x: &Frame<f32, U1>| hz(note + x[0] as f64) as f32));
            let body = (sweep >> net(An(Sine::<f32>::with_phase(0.25)))) * hit(0.001, 0.55, -5.0);
            let beater = (noise(ctx) >> hpf(1800.0)) * hit(0.0005, 0.012, -4.0) * 0.3;
            Note::rings(
                (body + beater) >> net(map(|x: &Frame<f32, U1>| x[0].tanh())),
                0.7,
            )
        })
}

fn snare() -> Synth {
    Synth::new("snare")
        .options(strike_options(55.0))
        .build(|o, ctx| {
            let vel = vel(o, 0.0);
            let freq = hz(o.number("note"));
            let stick =
                impulse_net() + (noise(ctx) >> lpf(4000.0)) * hit(0.0005, 0.004, -4.0) * 0.3;
            let hard = ((vel - 0.65) / 0.175).clamp(0.0, 1.0);
            let ratios = [1.0, 1.2, 1.46, 1.66, 2.18, 2.8];
            let levels: Vec<f64> = [-3.0, 0.0, -3.0, -3.0, -14.0, -14.0]
                .iter()
                .zip([0.0, 0.0, -11.0, -11.0, 4.0, 4.0])
                .map(|(l, h)| db(l + h * hard))
                .collect();
            let rings = [0.65, 0.65, 0.65, 0.6, 0.55, 0.55];
            let hz_ = scaled(&ratios, freq);
            let head = modes(stick.clone(), &hz_, &levels, &times(&rings, 0.07))
                + modes(stick, &hz_, &times(&levels, db(-30.0)), &rings);
            let shake = || exp_segments(&[0.0001, 1.0, 0.001], &[0.001, 0.5]);
            let wash =
                noise(ctx) >> hpf(500.0) >> low_shelf(1500.0, 6.0) >> high_shelf(6000.0, -6.0);
            let dark = wash >> high_shelf(2000.0, (vel - 0.65).min(0.0) * 20.0);
            let cutoff = shake()
                >> net(map(|x: &Frame<f32, U1>| {
                    (7500.0 * x[0].max(0.0).powf(0.15)).max(2000.0)
                }));
            let wires = ((dark | cutoff | net(dc(std::f32::consts::FRAC_1_SQRT_2)))
                >> net(lowpass::<f32>()))
                * shake()
                * db(-42.5 - (vel - 0.65).abs() * 14.0) as f32;
            let sound = (head * db(-32.0) as f32 + wires) * (vel / 0.6).powf(3.8) as f32;
            Note::rings(sound, 1.5)
        })
}

fn clap() -> Synth {
    Synth::new("clap")
        .option(Spec::note("note").default(60.0).doc("The clap's pitch."))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let hands = segments(
                &[0.0, 1.0, 0.2, 1.0, 0.2, 0.9, 0.1, 0.8, 0.0],
                &[0.001, 0.008, 0.001, 0.009, 0.001, 0.011, 0.001, 0.32],
                &[-6.0; 8],
            );
            Note::rings((noise(ctx) >> bpf(freq * 4.2, 0.9)) * hands * 1.8, 0.6)
        })
}

fn crash() -> Synth {
    Synth::new("crash")
        .options(strike_options(60.0))
        .option(
            Spec::beats("sustain")
                .default(9.0)
                .range(0.0, f64::INFINITY)
                .doc("How long it rings before it is choked."),
        )
        .option(
            Spec::beats("release")
                .default(1.0)
                .range(0.0, f64::INFINITY)
                .doc("How long the choke takes."),
        )
        .build(|o, ctx| {
            let vel = vel(o, 0.0);
            let scale = 2f64.powf((o.number("note") - 60.0) / 12.0);
            let bands = [
                300.0, 500.0, 800.0, 1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0,
            ];
            let levels = [
                -30.0, -14.0, -43.0, -30.0, -23.0, -24.0, -18.0, -34.0, -24.0,
            ];
            let rings = [11.0, 9.6, 7.06, 5.28, 4.36, 3.38, 2.83, 1.93, 0.8];
            let shorter = [0.0, 0.75, 0.68, 0.51, 0.4, 0.31, 0.18, 0.0, 0.0];
            let stick = (noise(ctx) >> hpf(1000.0) >> high_shelf(3000.0, (vel - 0.6) * 4.0))
                * hit(0.0005, 0.01, -4.0);
            let mut bank = Net::wrap(Box::new(zero()));
            for some in plate(160, 300.0, 15000.0) {
                let mean = some.iter().sum::<f64>() / some.len() as f64;
                let longer = (vel / 0.55).powf(-between(mean, &bands, &shorter));
                let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
                let l: Vec<f64> = some
                    .iter()
                    .map(|&f| db(between(f, &bands, &levels)))
                    .collect();
                let r: Vec<f64> = some
                    .iter()
                    .map(|&f| between(f, &bands, &rings) * longer)
                    .collect();
                bank = bank + modes(stick.clone(), &f, &l, &r);
            }
            let crack = (noise(ctx) >> hpf(6000.0 * scale)) * hit(0.001, 0.06, -4.0) * 2.0;
            let thud = (noise(ctx)
                >> bpf(330.0 * scale, 1.0)
                >> hpf(200.0 * scale)
                >> lpf(400.0 * scale)
                >> lpf(400.0 * scale))
                * hit(0.001, 0.035, -4.0)
                * 31.6;
            let (held, release) = (o.number("sustain"), o.number("release"));
            let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
            let sound = (bank * 0.1 + crack + thud) * 0.126 * choke * (vel / 0.6).powf(1.7) as f32;
            Note::rings(sound, held + release)
        })
}

const CLOSED_LEVELS: [f64; 10] = [
    -15.0, -12.0, -11.0, -31.0, -20.0, -18.0, -21.0, -5.0, -8.0, -4.0,
];
const CLOSED_RINGS: [f64; 10] = [0.6, 0.64, 0.66, 0.67, 0.57, 0.55, 0.53, 0.49, 0.41, 0.28];
const OPEN_LEVELS: [f64; 10] = [
    -16.0, -12.0, -1.0, -20.0, -13.0, -15.0, -9.0, -3.0, -8.0, -4.0,
];
const OPEN_RINGS: [f64; 10] = [2.4, 2.5, 2.5, 2.5, 2.5, 2.6, 2.6, 2.5, 2.6, 2.7];

fn hat(
    name: &'static str,
    levels: &'static [f64; 10],
    rings: &'static [f64; 10],
    length: f64,
    pedal: bool,
) -> Synth {
    let mut decl = Synth::new(name).options(strike_options(92.0)).option(
        Spec::beats("release")
            .default(0.05)
            .range(0.0, f64::INFINITY)
            .doc("How long the hat takes to close."),
    );
    if pedal {
        decl = decl.option(
            Spec::beats("sustain")
                .default(length)
                .range(0.0, f64::INFINITY)
                .doc("How long it rings open."),
        );
    }
    decl.build(move |o, ctx| {
        let vel = vel(o, 0.0);
        let scale = 2f64.powf((o.number("note") - 92.0) / 12.0);
        let held = if pedal { o.number("sustain") } else { length };
        let release = o.number("release");
        let soft = (1.0 - vel) / 0.7;
        let stick = (noise(ctx) >> hpf(1000.0) >> high_shelf(3000.0, (vel - 0.6) * 4.0))
            * hit(0.0005, 0.005, -4.0);
        let mut bank = Net::wrap(Box::new(zero()));
        for some in plate(120, 150.0, 16000.0) {
            let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
            let l: Vec<f64> = some
                .iter()
                .map(|&f| db(between(f, &HAT_BANDS, levels)))
                .collect();
            let r: Vec<f64> = some
                .iter()
                .map(|&f| between(f, &HAT_BANDS, rings))
                .collect();
            bank = bank + modes(stick.clone(), &f, &l, &r);
        }
        let dark = bank >> peak(1300.0, 1.5, soft * 6.0);
        let sizzle = (noise(ctx) >> hpf(7000.0 * scale)) * hit(0.001, 0.05, -4.0) * 0.1;
        let thud = (noise(ctx)
            >> bpf(320.0 * scale, 0.2)
            >> hpf(250.0 * scale)
            >> hpf(250.0 * scale)
            >> lpf(400.0 * scale))
            * hit(0.001, 0.06, -4.0)
            * db(soft.sqrt() * 50.0 - 20.0) as f32;
        let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
        let sound = (dark * 0.1 + sizzle + thud) * 0.0525 * choke * (vel / 0.6).powf(2.8) as f32;
        Note::rings(sound, held + release)
    })
}

struct Tom {
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    skin: [f64; 7],
    gain: f64,
    curve: f64,
    bright: [f64; 4],
}

const HIGH_TOM: Tom = Tom {
    ratios: &[1.0, 1.16, 1.32, 1.95, 2.55, 2.71, 2.99, 3.08, 3.59, 6.78],
    levels: &[
        0.0, -8.0, -11.0, -10.0, -7.7, -11.0, -4.6, -7.8, -10.7, -10.8,
    ],
    rings: &[1.47, 1.47, 1.5, 2.1, 1.0, 0.79, 0.79, 0.78, 0.92, 0.62],
    skin: [-10.0, -12.0, -24.0, -25.0, -24.0, -35.0, -39.0],
    gain: -35.1,
    curve: 1.32,
    bright: [1500.0, 0.0, 12.0, 16.0],
};

const LOW_TOM: Tom = Tom {
    ratios: &[
        1.0, 1.18, 1.39, 1.59, 1.78, 2.88, 3.0, 3.21, 3.73, 4.1, 4.22,
    ],
    levels: &[
        0.0, -9.4, -12.1, -12.9, -15.2, -9.3, -9.8, -14.7, -14.6, -6.9, -13.2,
    ],
    rings: &[1.6, 1.6, 1.56, 1.5, 1.4, 1.5, 1.5, 1.5, 0.95, 0.93, 1.06],
    skin: [-6.0, -10.0, -26.0, -23.0, -18.0, -30.0, -32.0],
    gain: -32.0,
    curve: 0.79,
    bright: [250.0, 24.0, 20.0, 10.0],
};

fn tom(name: &'static str, note: f64, t: &'static Tom) -> Synth {
    Synth::new(name)
        .options(strike_options(note))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let freq = hz(o.number("note"));
            let f0 = hz(note);
            let below = (vel - 0.65).min(0.0);
            let above = (vel - 0.65).max(0.0);
            let struck = impulse_net() + noise(ctx) * hit(0.0005, 0.003, -4.0) * 0.3;
            let stick = struck
                >> high_shelf(t.bright[0], below * t.bright[1])
                >> high_shelf(2500.0, below * t.bright[2] + above * t.bright[3]);
            let hz_ = scaled(t.ratios, freq);
            let levels = amps(t.levels);
            let head = modes(stick.clone(), &hz_, &levels, &times(t.rings, 0.08))
                + modes(stick.clone(), &hz_, &times(&levels, db(-10.0)), t.rings);
            let mut dense = Net::wrap(Box::new(zero()));
            for some in plate(60, 400.0, 7000.0) {
                let f: Vec<f64> = some.iter().map(|f| f / f0 * freq).collect();
                let l: Vec<f64> = some
                    .iter()
                    .map(|&f| db(between(f, &SKIN_BANDS, &t.skin)))
                    .collect();
                let r: Vec<f64> = some
                    .iter()
                    .map(|&f| between(f, &SKIN_BANDS, &[0.9, 0.6, 0.55, 0.6, 0.6, 0.6, 0.5]))
                    .collect();
                dense = dense + modes(stick.clone(), &f, &l, &r);
            }
            let sound = (head + dense * db(-11.0) as f32)
                * (db(t.gain) * (vel / 0.65).powf(t.curve)) as f32;
            Note::rings(sound, 2.5)
        })
}

fn rimshot() -> Synth {
    Synth::new("rimshot")
        .option(Spec::note("note").default(74.0).doc("The rim's pitch."))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let rim = modes(
                impulse_net(),
                &scaled(&[1.0, 2.72, 4.3], freq),
                &[1.0, 0.5, 0.3],
                &[0.06, 0.04, 0.02],
            ) * 0.25;
            let head = (noise(ctx) >> bpf(2400.0, 1.0)) * hit(0.0005, 0.05, -4.0) * 0.6;
            Note::rings(rim + head, 0.3)
        })
}
