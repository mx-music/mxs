//! mx's berimbau, after its design in mx's priv/synthdefs/mx-drums.scd: the string's partials struck by the stick, swung by the gourd, over the wire's close modes and the stick's thud.

use crate::{db_amp, fast_envelope, midi_hz, perc, struck, Opts};
use fundsp::math::rnd1;
use fundsp::prelude::*;
use std::f64::consts::PI;

/// Scales the berimbau to the loudness of scsynth's render of the same strokes.
const GAIN: f64 = 1.4;

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
/// What the wire's levels take off at each band to sound as loud as scsynth's render: its modes are drawn from another generator, and FunDSP's resonators answer the strike's noise louder.
const WIRE_FIT: [f64; 7] = [-10.0, -15.8, -12.9, -14.3, -7.0, -6.6, -12.8];

pub fn berimbau(opts: &Opts, rate: f64) -> Box<dyn AudioUnit> {
    let vel = opts.get("vel", 0.6).clamp(0.0, 1.0);
    let buzz = opts.get("buzz", 0.0).clamp(0.0, 1.0);
    let coin = opts.get("coin", 0.0).clamp(0.0, 1.0).max(buzz);
    let mix = |a: f64, b: f64| a + (b - a) * coin;
    let hz = midi_hz(opts.get("note", 39.11)) * 1.0621f64.powf(coin);
    let damp = 1.0 - buzz * 0.96;
    let gourd = opts.get("gourd", 2.3);

    // The gourd starts open and swings some harmonics up and others down as it closes against the belly.
    let partials = (0..RATIOS.len()).map(|i| {
        let mode = struck(hz * RATIOS[i], 0.0, db_amp(mix(OPEN[i], PRESSED[i])), RINGS[i] * damp, rate);
        let swing = match RATIOS[i] as u32 {
            2 => -4.5,
            3 => -7.0,
            5 => 12.0,
            6 => 13.0,
            _ => 0.0,
        };
        if gourd > 0.0 && swing != 0.0 {
            mode * Net::wrap(Box::new(envelope(move |t: f64| db_amp((2.0 * PI * gourd * t).cos() * swing))))
        } else {
            mode
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
        let loud = db_amp(at(f, &wire_levels)) * if long { db_amp(at(f, &TAILS) + 10.0) } else { 1.0 };
        let ring = if long { 1.5 } else { (at(f, &WIRE_RINGS) * spread).min(2.5) };
        struck(f, 0.0, loud, ring * damp.sqrt(), rate)
    });
    let strike = Net::wrap(Box::new(white().seed(1) * fast_envelope(|t: f32| perc(t, 0.0002, 0.002)) * 0.6));
    let wire = strike >> wire_modes.reduce(|a, b| a & b).unwrap();

    // The stick's dull thud, in three bands of noise that rise at once and fall away exponentially.
    let thud = [(200.0, -20.0, 0.3), (316.0, -20.0, 0.3), (502.0, -10.0, 0.5)].into_iter().enumerate().map(|(i, (centre, level, fade))| {
        let fade = fade * damp.sqrt();
        let shape = envelope(move |t: f64| {
            if t < 0.001 {
                t / 0.001
            } else if t < 0.001 + fade {
                0.001f64.powf((t - 0.001) / fade)
            } else {
                0.001 * (1.0 - (t - 0.001 - fade) / 0.01).max(0.0)
            }
        });
        let q = 1.0 / 0.35;
        // FunDSP's bandpass passes its centre at a gain of q, taken off here.
        let band = white().seed(2 + i as u64) >> bandpass_hz(centre, q) >> bandpass_hz(centre, q) >> bandpass_hz(centre, q);
        Net::wrap(Box::new(band * shape * (db_amp(level) / (q as f64).powi(3)) as f32))
    });
    let thud = thud.reduce(|a, b| a + b).unwrap();
    let tick = Net::wrap(Box::new(impulse::<U1>() >> highpass_hz(200.0, 0.707) * 0.1));
    let mut sound = string * 0.05 + thud + wire * 0.05 + tick;

    // With buzz, the coin rests on the wire and rattles against it.
    if buzz > 0.0 {
        let rattle = (dc((hz as f32, 0.08)) >> pulse()) * 0.5 + 0.42 >> highpass_hz(1200.0, 0.707);
        let rattle = rattle * fast_envelope(|t: f32| perc(t, 0.002, 0.12)) * (buzz * 0.3) as f32;
        sound = sound + Net::wrap(Box::new(rattle));
    }

    let brighter = db_amp((vel - 0.6) * 5.0) as f32;
    let level = (0.2 * (vel / 0.6).powf(1.5) * GAIN) as f32;
    let length = Net::wrap(Box::new(envelope(|t: f64| if t < LENGTH { 1.0 } else { 0.0 })));
    Box::new((sound >> highshelf_hz(1500.0, 0.707, brighter)) * level * length)
}
