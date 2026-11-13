//! The drum kit: the kick, snare, clap, crash, hi-hats, toms and rimshot.

use super::{
    amps, bpf, high_shelf, hpf, impulse_net, low_shelf, lpf, net, noise, plate, scaled,
    strike_options, times, tune, vel,
};
use crate::nodes::{between, db, exp_segments, hit, hz, modes, segments};
use crate::{Context, Note, Spec, Synth};
use fundsp::prelude::*;

/// The synths of this file.
pub(super) fn all() -> Vec<Synth> {
    vec![
        kick(),
        snare(),
        clap(),
        crash(),
        ride(),
        hat("closed_hihat", &CLOSED_LEVELS, &CLOSED_RINGS, 0.8, false),
        hat("open_hihat", &OPEN_LEVELS, &OPEN_RINGS, 5.0, true),
        tom("low_tom", 41.85, &LOW_TOM),
        tom("mid_tom", 43.4, &HIGH_TOM),
        tom("high_tom", 45.0, &HIGH_TOM),
        rimshot(),
        fitted("cross_stick", "MXS_CROSS", &CROSS_STICK),
        fitted("pedal_hihat", "MXS_PEDAL", &PEDAL_HIHAT),
    ]
}

fn kick() -> Synth {
    Synth::new("kick")
        .option(Spec::note("note").default(34.0).doc("The drum's pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard it is struck."),
        )
        .option(
            Spec::float("beater")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("How hard the beater is; 0 is a felt beater, which leaves out the click."),
        )
        .build(|o, ctx| {
            let note = o.number("note");
            let vel = vel(o, 0.0);
            let hard = o.number("beater");
            let sweep = segments(&[26.0, 0.0], &[0.07], &[-6.0])
                >> net(map(move |x: &Frame<f32, U1>| hz(note + x[0] as f64) as f32));
            let body = (sweep >> net(An(Sine::<f32>::with_phase(0.25))))
                * hit(0.001 + 0.004 * (1.0 - hard), 0.55, -5.0);
            let beater = (noise(ctx) >> hpf(1800.0))
                * hit(0.0005, 0.012, -4.0)
                * (0.3 * hard * db((vel - 0.6) * 20.0)) as f32;
            let shell = modes(impulse_net(), &[hz(note) * 1.75], &[0.1], &[1.5]);
            let struck = (body + beater) >> net(map(|x: &Frame<f32, U1>| x[0].tanh()));
            Note::rings((struck + shell) * (vel / 0.6).powf(2.5) as f32, 1.5)
        })
}

fn snare() -> Synth {
    Synth::new("snare")
        .options(strike_options(55.0))
        .option(
            Spec::float("snares")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("0 releases the wires from the head."),
        )
        .option(
            Spec::float("rim")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 strikes the rim with the head, a rimshot."),
        )
        .option(
            Spec::float("buzz")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 presses the stick into the head so it bounces."),
        )
        .build(|o, ctx| {
            let vel = vel(o, 0.0);
            let freq = hz(o.number("note"));
            let (snares, rim, buzz) = (o.number("snares"), o.number("rim"), o.number("buzz"));
            let one = impulse_net() + (noise(ctx) >> lpf(4000.0)) * hit(0.0005, 0.004, -4.0) * 0.3;
            let bounced = |one: Net| {
                let mut stick = one.clone();
                if buzz > 0.0 {
                    let (mut at, mut level) = (0.0, buzz);
                    for gap in [0.055, 0.05, 0.045, 0.065, 0.05, 0.05, 0.065, 0.05] {
                        at += gap;
                        level *= 0.67;
                        stick = stick + (one.clone() >> net(delay(at))) * level as f32;
                    }
                }
                stick
            };
            let stick = if buzz > 0.0 {
                bounced(one) >> lpf(1500.0)
            } else {
                bounced(one)
            };
            let burst = bounced(noise(ctx) * hit(0.0005, 0.004, -4.0));
            let hard = ((vel - 0.65) / 0.175).clamp(0.0, 1.0);
            let ratios = [1.0, 1.2, 1.46, 1.66, 2.18, 2.8, 3.6, 4.4];
            let levels: Vec<f64> = [-3.0, 0.0, -3.0, -3.0, -14.0, -14.0, -24.0, -24.0]
                .iter()
                .zip([0.0, 0.0, -11.0, -11.0, 4.0, 4.0, 0.0, 0.0])
                .map(|(l, h)| db(l + h * hard))
                .collect();
            let rings = [0.65, 0.65, 0.65, 0.6, 0.55, 0.55, 0.5, 0.5];
            let hz_ = scaled(&ratios, freq);
            let levels: Vec<f64> = levels.iter().zip(&hz_).map(|(l, f)| l * f / 20.0).collect();
            let stick = stick >> net(lowpole_hz(20.0));
            let head = modes(stick.clone(), &hz_, &levels, &times(&rings, 0.07))
                + modes(stick.clone(), &hz_, &times(&levels, db(-11.0)), &rings);
            let shake = || {
                exp_segments(&[0.0001, 1.0, 0.001], &[0.001, 0.5]) * (1.0 - buzz) as f32
                    + exp_segments(&[0.4, 1.0, 0.9, 0.001], &[0.015, 0.11, 1.15]) * buzz as f32
            };
            let soft = (1.0 - buzz) * (vel - 0.65);
            let wash =
                noise(ctx) >> hpf(500.0) >> low_shelf(1500.0, 6.0) >> high_shelf(6000.0, -6.0);
            let dark = wash >> high_shelf(2000.0, soft.min(0.0) * 20.0);
            let cutoff = shake()
                >> net(map(|x: &Frame<f32, U1>| {
                    (7500.0 * x[0].max(0.0).powf(0.15)).max(2000.0)
                }));
            let wires = ((dark | cutoff | net(dc(std::f32::consts::FRAC_1_SQRT_2)))
                >> net(lowpass::<f32>()))
                * shake()
                * (snares * db(-30.5 - buzz * 5.0 - soft.abs() * 14.0)) as f32;
            let mut sound = (head * db(-32.0) as f32 + wires)
                * ((snares * (1.0 - rim)).sqrt() * (vel / 0.6).powf(3.8)) as f32;
            let scale = freq / hz(SNARE_OFF.note);
            let bright = |stick: Net| stick >> high_shelf(2500.0, (vel - 0.6) * 12.0);
            if snares < 1.0 && rim < 1.0 {
                let weight = ((1.0 - snares) * (1.0 - rim)).sqrt() * (vel / 0.6).powf(2.0);
                sound = sound
                    + fitted_net(
                        ctx,
                        &SNARE_OFF,
                        "MXS_SNARE_OFF",
                        bright(burst.clone()),
                        scale,
                        &bounced,
                        &bright,
                    ) * weight as f32;
            }
            if rim > 0.0 {
                let weight = rim.sqrt() * (vel / 0.6).powf(2.0);
                sound = sound
                    + fitted_net(
                        ctx,
                        &RIMSHOT,
                        "MXS_RIMSHOT",
                        bright(burst),
                        scale,
                        &bounced,
                        &bright,
                    ) * weight as f32;
            }
            let plain = snares == 1.0 && rim == 0.0 && buzz == 0.0;
            Note::rings(sound, if plain { 1.5 } else { 2.5 })
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
                -19.4, -46.4, -46.5, -32.0, -32.8, -36.5, -27.1, -31.8, -19.3,
            ];
            let rings = [9.6, 9.6, 5.2, 5.28, 4.36, 3.38, 2.83, 1.93, 0.8];
            let mut fit = levels.to_vec();
            fit.extend([50.0, 6.0, -20.0, -14.0, 2.0]);
            let levels = &fit[0..9];
            let shorter = [0.75, 0.75, 0.68, 0.51, 0.4, 0.31, 0.18, 0.0, 0.0];
            let stick = (noise(ctx) >> hpf(1000.0) >> high_shelf(3000.0, (vel - 0.6) * 4.0))
                * hit(0.0005, 0.01, -4.0);
            let mut bank = Net::wrap(Box::new(zero()));
            for some in plate(160, 300.0, 15000.0) {
                let mean = some.iter().sum::<f64>() / some.len() as f64;
                let longer = (vel / 0.55).powf(-between(mean, &bands, &shorter));
                let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
                let l: Vec<f64> = some
                    .iter()
                    .map(|&f| {
                        db(between(f, &bands, levels) - fit[10]
                            + if f < 1000.0 { 12.0 } else { 0.0 })
                    })
                    .collect();
                let r: Vec<f64> = some
                    .iter()
                    .map(|&f| between(f, &bands, &rings) * longer)
                    .collect();
                bank = bank + modes(stick.clone(), &f, &l, &r);
            }
            let noise_rings: Vec<f64> = (0..9)
                .map(|i| rings[i] * (vel / 0.55).powf(-shorter[i]))
                .collect();
            let same = |level: Net| level;
            let early_rings: Vec<f64> = (0..9)
                .map(|i| {
                    if i < 3 {
                        noise_rings[i]
                    } else {
                        noise_rings[i].min(fit[13])
                    }
                })
                .collect();
            bank = bank * db(fit[12] + 7.0) as f32
                + ((band_noise(ctx, levels, &noise_rings, 1, scale, &same) * db(fit[12]) as f32
                    + band_noise(ctx, levels, &early_rings, 1, scale, &same)
                    + band_noise(ctx, &[-33.0], &noise_rings, 0, scale * 0.7, &same))
                    >> high_shelf(3000.0, (vel - 0.6) * 4.0))
                    * db(fit[9]) as f32;
            let crack = (noise(ctx) >> hpf(6000.0 * scale))
                * hit(0.001, 0.06, -4.0)
                * (2.0 * db(fit[11])) as f32;
            let thud = (noise(ctx)
                >> bpf(330.0 * scale, 1.0)
                >> hpf(110.0 * scale)
                >> lpf(400.0 * scale)
                >> lpf(400.0 * scale))
                * hit(0.001, 0.035, -4.0)
                * (31.6 * db(fit[11])) as f32;
            let (held, release) = (o.number("sustain"), o.number("release"));
            let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
            let sound = (bank * 0.1 + crack + thud) * 0.9 * choke * (vel / 0.6).powf(1.7) as f32;
            Note::rings(sound, held + release)
        })
}

const RIDE_BANDS: [f64; 10] = [
    200.0, 300.0, 500.0, 800.0, 1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0,
];
const RIDE_LEVELS: [f64; 10] = [0.0, -7.4, -5.6, -20.4, 2.4, -13.5, 6.5, 8.0, 2.1, 11.3];
const RIDE_RINGS: [f64; 10] = [20.0, 12.2, 8.7, 7.65, 6.37, 5.78, 4.47, 3.67, 2.76, 2.79];

const RIDE_BELL_HZ: [f64; 13] = [
    271.2, 284.9, 1193.8, 2340.4, 3435.8, 4295.8, 4834.8, 4900.5, 4974.0, 5941.0, 6084.5, 7322.7,
    8958.5,
];
const RIDE_BELL_LEVELS: [f64; 13] = [
    -3.0, -4.5, -6.1, -4.5, -7.7, -5.2, 0.0, -0.2, -1.1, -5.2, -3.9, -5.1, -4.5,
];
const RIDE_BELL_RINGS: [f64; 13] = [
    6.38, 6.38, 3.26, 2.98, 2.83, 2.2, 1.44, 1.4, 1.94, 1.44, 1.12, 1.26, 0.97,
];

fn ride() -> Synth {
    Synth::new("ride")
        .options(strike_options(60.0))
        .option(
            Spec::beats("sustain")
                .default(8.0)
                .range(0.0, f64::INFINITY)
                .doc("How long it rings before it is damped."),
        )
        .option(
            Spec::beats("release")
                .default(3.0)
                .range(0.0, f64::INFINITY)
                .doc("How long the damping takes."),
        )
        .option(
            Spec::float("bell")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 strikes the bell instead of the bow."),
        )
        .build(|o, ctx| {
            let t = [-14.0, -8.0, 0.0, 22.0, 0.0];
            let mut fit = RIDE_LEVELS.to_vec();
            fit.extend([30.0, 22.0, 26.0, -14.0]);
            let bell = o.number("bell");
            let vel = vel(o, 0.0);
            let scale = 2f64.powf((o.number("note") - 60.0) / 12.0);
            let stick = (noise(ctx) >> hpf(1000.0) >> high_shelf(3000.0, (vel - 0.6) * 4.0))
                * hit(0.0005, 0.01, -4.0);
            let mut wash = Net::wrap(Box::new(zero()));
            let mut ping = Net::wrap(Box::new(zero()));
            for some in plate(160, 150.0, 16000.0) {
                let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
                let l: Vec<f64> = some
                    .iter()
                    .map(|&f| {
                        let quieter = match f {
                            f if f < 1000.0 => 0.0,
                            f if f < 3000.0 => fit[11],
                            f if f < 6000.0 => fit[11] + 6.0,
                            _ => fit[11] + 14.0,
                        };
                        db(between(f, &RIDE_BANDS, &fit[0..10]) - quieter)
                    })
                    .collect();
                let r: Vec<f64> = some
                    .iter()
                    .map(|&f| between(f, &RIDE_BANDS, &RIDE_RINGS))
                    .collect();
                wash = wash + modes(stick.clone(), &f, &l, &r);
                let f3: Vec<f64> = f.iter().step_by(3).copied().collect();
                let l3: Vec<f64> = l.iter().step_by(3).copied().collect();
                let r3 = vec![0.35; f3.len()];
                ping = ping + modes(stick.clone(), &f3, &l3, &r3);
            }
            let tilt = |sound: Net| sound >> high_shelf(3000.0, (vel - 0.6) * 4.0);
            let same = |level: Net| level;
            let high = db(fit[10]) as f32;
            wash = wash
                + tilt(band_noise(
                    ctx,
                    &fit[3..10],
                    &RIDE_RINGS[3..],
                    3,
                    scale,
                    &same,
                )) * high;
            ping = ping + tilt(band_noise(ctx, &fit[3..10], &[0.35; 7], 3, scale, &same)) * high;
            let tick =
                (noise(ctx) >> hpf(4000.0 * scale)) * hit(0.0005, 0.015, -4.0) * db(fit[12]) as f32;
            let (held, release) = (o.number("sustain"), o.number("release"));
            let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
            let mut cup_levels = amps(&RIDE_BELL_LEVELS);
            cup_levels[0] *= db(t[3]);
            cup_levels[1] *= db(t[3]);
            let cup = modes(
                stick.clone(),
                &scaled(&RIDE_BELL_HZ, scale),
                &cup_levels,
                &RIDE_BELL_RINGS,
            ) * (bell * db(t[0])) as f32;
            let wash_db = -13.0 + bell * t[1];
            let ping_db = fit[13] + bell * (t[2] - fit[13]);
            let tick = tick * db(bell * t[4]) as f32;
            let sound = (wash * db(wash_db) as f32 + ping * db(ping_db) as f32 + tick + cup)
                * 0.007
                * db(bell * -4.0) as f32
                * choke
                * (vel / 0.6).powf(1.7) as f32;
            Note::rings(sound, held + release)
        })
}

const CLOSED_LEVELS: [f64; 10] = [-30.0, -6.4, -9.2, -10.6, -4.7, -1.4, -5.2, 10.4, 10.7, 14.7];
const CLOSED_RINGS: [f64; 10] = [0.79, 0.55, 0.58, 0.74, 0.55, 0.43, 0.71, 0.5, 0.31, 0.22];
const OPEN_LEVELS: [f64; 10] = [-21.0, -14.9, 7.9, -22.7, 5.6, -2.4, 8.4, 11.1, 11.0, 15.0];
const OPEN_RINGS: [f64; 10] = [2.0, 3.5, 2.37, 2.49, 2.32, 2.96, 2.63, 2.49, 2.71, 2.6];

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
        let same = |n: Net| n;
        let dark = band_noise(ctx, levels, rings, 0, scale, &same)
            >> high_shelf(4000.0, (0.333 - soft) * 10.0);
        let sizzle = (noise(ctx) >> hpf(7000.0 * scale)) * hit(0.001, 0.05, -4.0) * 0.035;
        let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
        let thud = ((noise(ctx) * exp_segments(&[0.001, 1.0, 0.001], &[0.0005, rings[1]]))
            >> bpf(320.0 * scale, 0.2)
            >> hpf(285.0 * scale)
            >> hpf(285.0 * scale)
            >> lpf(400.0 * scale))
            * (soft.max(0.0).sqrt() * if pedal { 0.0 } else { 0.25 }) as f32;
        let gain = if pedal { 3.3 } else { 2.2 };
        let sound = (dark * 0.1 + sizzle + thud) * gain * choke * (vel / 0.6).powf(2.8) as f32;
        Note::rings(sound, held + release)
    })
}

struct Tom {
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    skin: [f64; 8],
    skin_rings: [f64; 8],
    gain: f64,
    curve: f64,
    bright: [f64; 4],
}

const HIGH_TOM: Tom = Tom {
    ratios: &[1.0, 1.16, 1.32, 1.95, 2.55, 2.71, 2.99, 3.08, 3.59, 6.78],
    levels: &[
        0.0, -8.0, -11.0, -10.0, -7.7, -11.0, -4.6, -7.8, -10.7, -19.8,
    ],
    rings: &[1.47, 1.47, 1.5, 2.1, 1.0, 0.79, 0.79, 0.78, 0.92, 0.62],
    skin: [34.0, -51.6, 28.7, -2.7, 16.0, 8.1, -3.4, -32.5],
    skin_rings: [0.75, 0.47, 0.54, 0.38, 0.65, 0.47, 0.6, 0.1],
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
    skin: [35.0, 35.5, 19.4, 26.0, 20.0, 9.7, 6.5, -58.9],
    skin_rings: [0.9, 0.73, 0.33, 0.5, 0.6, 0.62, 0.55, 0.06],
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
            let tilt = |n: Net| {
                n >> high_shelf(t.bright[0], below * t.bright[1])
                    >> high_shelf(2500.0, below * t.bright[2] + above * t.bright[3])
            };
            let stick = tilt(struck >> net(lowpole_hz(20.0)) >> lpf(2000.0));
            let hz_ = scaled(t.ratios, freq);
            let levels: Vec<f64> = amps(t.levels)
                .iter()
                .zip(&hz_)
                .map(|(l, f)| l * f / 20.0)
                .collect();
            let head = modes(stick.clone(), &hz_, &levels, &times(t.rings, 0.08))
                + modes(stick.clone(), &hz_, &levels, t.rings);
            let same = |n: Net| n;
            let click = (noise(ctx) * hit(0.0005, 0.004, -4.0)) >> hpf(1500.0) >> lpf(4000.0);
            let dense = tilt(
                band_noise(ctx, &t.skin, &t.skin_rings, 2, freq / f0, &same)
                    + click * db(16.0) as f32,
            );
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

/// A stroke fitted to recordings: its strongest partials as measured, over many close modes with a level and a ring for each band, sounding once more with a short ring for the stroke's first moments.
struct Fitted {
    note: f64,
    partials: &'static [(f64, f64, f64)],
    levels: [f64; 10],
    rings: [f64; 10],
    partial_db: f64,
    early_db: f64,
    early_ring: f64,
    dense_db: f64,
    gain: f64,
}

const CROSS_STICK: Fitted = Fitted {
    note: 53.6,
    partials: &[
        (181.2, 0.0, 0.65),
        (199.6, -12.1, 0.65),
        (289.0, -8.2, 1.71),
        (308.6, -10.7, 1.68),
        (1067.9, -19.0, 0.44),
        (1116.4, -14.4, 0.47),
        (1204.4, -16.1, 0.48),
        (1226.1, -17.4, 0.48),
        (1345.6, -18.8, 0.42),
        (1840.0, -17.6, 0.53),
        (1891.9, -19.3, 0.57),
        (1940.2, -19.4, 0.50),
    ],
    levels: [
        -19.1, -4.9, -13.9, -5.2, -2.0, -3.9, 0.0, -9.6, -15.6, -13.5,
    ],
    rings: [0.66, 1.20, 0.62, 0.53, 0.46, 0.52, 0.47, 0.51, 0.45, 0.33],
    partial_db: -6.0,
    early_db: 10.0,
    early_ring: 0.08,
    dense_db: 30.0,
    gain: -32.0,
};

const SNARE_OFF: Fitted = Fitted {
    note: 53.6,
    partials: &[
        (180.3, -7.3, 0.51),
        (198.9, -18.5, 0.51),
        (212.1, -17.0, 0.43),
        (227.0, 0.0, 0.40),
        (237.8, -9.6, 0.40),
        (259.6, -21.7, 0.40),
        (335.7, -18.2, 1.06),
        (350.2, -20.1, 0.62),
        (544.8, -20.0, 1.02),
        (874.1, -16.5, 1.42),
        (885.6, -17.2, 1.42),
        (1361.5, -19.8, 0.52),
    ],
    levels: [
        0.0, -50.2, -26.7, -17.4, -39.8, -25.7, -30.7, -51.1, -61.0, -57.1,
    ],
    rings: [0.51, 0.60, 0.98, 1.26, 0.74, 0.56, 0.43, 0.51, 0.48, 2.00],
    partial_db: -6.0,
    early_db: 0.0,
    early_ring: 0.1,
    dense_db: 40.0,
    gain: -32.0,
};

const RIMSHOT: Fitted = Fitted {
    note: 53.6,
    partials: &[
        (181.2, -11.8, 0.59),
        (228.6, -12.2, 0.46),
        (338.8, -8.9, 0.66),
        (486.8, 0.0, 1.44),
        (628.7, -7.8, 2.49),
        (836.8, -13.4, 1.56),
        (885.1, -3.9, 1.68),
        (993.5, -8.0, 1.01),
        (1133.6, -11.0, 1.42),
        (1704.8, -11.7, 0.35),
        (1781.3, -12.6, 0.84),
        (1930.3, -15.0, 1.05),
    ],
    levels: [
        0.0, -6.9, -18.8, -5.6, -22.0, -1.5, -26.0, -22.5, -23.5, -25.9,
    ],
    rings: [0.49, 0.66, 1.64, 1.73, 1.21, 0.66, 1.36, 1.40, 1.26, 1.21],
    partial_db: -6.0,
    early_db: 3.0,
    early_ring: 0.1,
    dense_db: 40.0,
    gain: -26.0,
};

const PEDAL_HIHAT: Fitted = Fitted {
    note: 60.0,
    partials: &[],
    levels: [-4.9, 0.0, -17.8, -7.3, -8.2, -11.7, -10.6, -1.0, -0.7, -0.1],
    rings: [0.60, 0.97, 0.50, 0.39, 0.41, 0.45, 0.50, 0.50, 0.43, 0.30],
    partial_db: 0.0,
    early_db: -40.0,
    early_ring: 0.2,
    dense_db: 30.0,
    gain: -30.0,
};

/// Noise in each of the ride's bands from `first` on, at its level in dB, fading 60 dB over its ring; `shape` is applied to each band's level.
pub(super) fn band_noise(
    ctx: &mut Context,
    levels: &[f64],
    rings: &[f64],
    first: usize,
    scale: f64,
    shape: &dyn Fn(Net) -> Net,
) -> Net {
    let mut sound = Net::wrap(Box::new(zero()));
    for ((&f, &l), &r) in RIDE_BANDS[first..].iter().zip(levels).zip(rings) {
        let f = f * scale;
        let level = shape(exp_segments(&[0.001, 1.0, 0.001], &[0.0005, r]));
        let band =
            (noise(ctx) * level) >> bpf(f, 0.5) >> hpf(f * 0.6) >> lpf(f * 1.7) >> lpf(f * 1.7);
        sound = sound + band * (db(l) * (200.0 / f).sqrt()) as f32;
    }
    sound
}

fn fitted_net(
    ctx: &mut Context,
    d: &Fitted,
    env: &str,
    stick: Net,
    scale: f64,
    shape: &dyn Fn(Net) -> Net,
    tilt: &dyn Fn(Net) -> Net,
) -> Net {
    let mut defaults = d.levels.to_vec();
    defaults.extend(d.rings);
    defaults.extend([d.partial_db, d.early_db, d.early_ring, d.dense_db]);
    let t = tune(env, &defaults);
    let (levels, rings) = (&t[0..10], &t[10..20]);
    let hz_: Vec<f64> = d.partials.iter().map(|p| p.0 * scale).collect();
    let l: Vec<f64> = d.partials.iter().map(|p| db(p.1 + t[20])).collect();
    let r: Vec<f64> = d.partials.iter().map(|p| p.2).collect();
    let mut sound = modes(stick.clone(), &hz_, &l, &r);
    let dense = band_noise(ctx, levels, rings, 0, scale, shape)
        + band_noise(ctx, levels, &[t[22]; 10], 0, scale, shape) * db(t[21]) as f32;
    sound = sound + tilt(dense) * db(t[23]) as f32;
    sound * db(d.gain) as f32
}

fn fitted(name: &'static str, env: &'static str, d: &'static Fitted) -> Synth {
    Synth::new(name)
        .options(strike_options(d.note))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let scale = 2f64.powf((o.number("note") - d.note) / 12.0);
            let stick =
                (noise(ctx) * hit(0.0005, 0.004, -4.0)) >> high_shelf(2500.0, (vel - 0.6) * 12.0);
            let ring = d.rings.iter().cloned().fold(0.1, f64::max);
            let tilt = |sound: Net| sound >> high_shelf(2500.0, (vel - 0.6) * 12.0);
            let same = |level: Net| level;
            Note::rings(
                fitted_net(ctx, d, env, stick, scale, &same, &tilt) * (vel / 0.6).powf(2.0) as f32,
                ring.min(4.0),
            )
        })
}
