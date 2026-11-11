//! mx's berimbau, after its design in mx's priv/synthdefs/mx-drums.scd: the string's partials struck by the stick, swung by the gourd, over the wire's close modes and the stick's thud.

use crate::nodes::{db, hz, perc, shape, struck, white_noise};
use crate::{Note, Spec, Synth};
use fundsp::math::rnd1;
use fundsp::prelude::*;
use std::f64::consts::PI;

/// The seconds a stroke lasts.
const LENGTH: f64 = 2.9;

const RATIOS: [f64; 19] = [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 9.0, 10.0, 12.0, 14.0, 16.0, 19.35, 20.38, 21.0, 21.47, 35.0, 37.0, 39.0, 44.0];
const OPEN: [f64; 19] = [-10.0, 0.0, -15.5, 0.0, -8.0, -10.0, -10.0, -8.0, -14.6, -13.5, -11.6, -12.0, -12.0, -9.9, -12.0, -15.4, -10.8, -12.7, -11.6];
const PRESSED: [f64; 19] = [-30.0, 0.0, -5.4, -7.6, -7.9, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -30.0, -5.0, -3.1, -7.1, -6.0];
const RINGS: [f64; 19] = [2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.5, 2.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0, 0.8];

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

pub fn berimbau() -> Synth {
    Synth::new("berimbau")
        .option(Spec::note("note").default(39.11).doc("The open string's pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the stick strikes."))
        .option(Spec::float("coin").default(0.0).range(0.0, 1.0).doc("How hard the coin presses the wire, raising it a whole tone."))
        .option(Spec::float("buzz").default(0.0).range(0.0, 1.0).doc("How loosely the coin rests on the wire, rattling against it."))
        .option(Spec::float("gourd").default(2.3).range(0.0, 20.0).doc("How many times a second the gourd swings against the belly."))
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
                let partial = struck(freq * RATIOS[i], db(mix(OPEN[i], PRESSED[i])), RINGS[i] * damp);
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
            let wire_levels: [f64; 7] = std::array::from_fn(|i| mix(WIRE_OPEN[i], WIRE_PRESSED[i]) + WIRE_FIT[i]);
            let wire_modes = (0..96).map(|i| {
                let f = 700.0 * (16000.0f64 / 700.0).powf((i as f64 + rnd1(2 * i as u64 + SEED)) / 96.0);
                let spread = 0.2 * 30f64.powf(rnd1(2 * i as u64 + SEED + 1));
                let long = i % 4 == 0;
                let loud = db(at(f, &wire_levels)) * if long { db(at(f, &TAILS) + 10.0) } else { 1.0 };
                let ring = if long { 1.5 } else { (at(f, &WIRE_RINGS) * spread).min(2.5) };
                struck(f, loud, ring * damp.sqrt())
            });
            let strike = white_noise(1) * shape(|t| perc(t, 0.0002, 0.002)) * 0.6;
            let wire = strike >> wire_modes.reduce(|a, b| a & b).unwrap();

            // The stick's dull thud, in three bands of noise that rise at once and fall away exponentially.
            let thud = [(200.0, -20.0, 0.3), (316.0, -20.0, 0.3), (502.0, -10.0, 0.5)].into_iter().enumerate().map(
                |(i, (centre, level, fade))| {
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
                },
            );
            let thud = thud.reduce(|a, b| a + b).unwrap();
            let tick = Net::wrap(Box::new(impulse::<U1>() >> (highpass_hz(200.0, 0.707) * 0.1)));
            let mut sound = string * 0.05 + thud + wire * 0.05 + tick;

            // With buzz, the coin rests on the wire and rattles against it.
            if buzz > 0.0 {
                let rattle = ((dc((freq as f32, 0.08)) >> pulse()) * 0.5 + 0.42) >> highpass_hz(1200.0, 0.707);
                let rattle = Net::wrap(Box::new(rattle)) * shape(|t| perc(t, 0.002, 0.12)) * (buzz * 0.3) as f32;
                sound = sound + rattle;
            }

            let brighter = db((vel - 0.6) * 5.0) as f32;
            let level = (0.2 * (vel / 0.6).powf(1.5)) as f32;
            Note::rings((sound >> highshelf_hz(1500.0, 0.707, brighter)) * level, LENGTH)
        })
}
