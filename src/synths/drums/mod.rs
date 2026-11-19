//! mx's drums and hand percussion.
//! Their seeded modes are drawn from mxs's own generator, so they sit at other frequencies than scsynth's draws, with the same levels and rings at each band.

use crate::DEFAULT_VEL;
use crate::nodes::{Draws, db, white_noise};
use crate::{Context, Note, Options, Spec, Synth};
use fundsp::prelude::*;

mod brazilian;
mod cuica;
mod kit;
mod latin;
mod tabla;

pub use brazilian::berimbau;

/// Every drum and hand percussion instrument mx ships, but the berimbau, which `synths::all` adds itself.
pub fn all() -> Vec<Synth> {
    let mut drums = kit::all();
    drums.extend(latin::all());
    drums.extend(brazilian::all());
    drums.extend(tabla::all());
    drums.into_iter().map(trimmed).collect()
}

/// How many dB each drum's default stroke measured louder than scsynth's render of it, which it is turned down by.
/// Their strikes' noise comes from another generator than scsynth's, and in a short burst it sets the modes ringing a few dB louder or softer.
/// The timbales' noise is not seeded on scsynth either, so theirs are the mean of four renders, which spread over 3 dB.
const TRIMS: [(&str, f64); 17] = [
    ("snare", 3.3),
    ("crash", 4.9),
    ("closed_hihat", 3.2),
    ("open_hihat", 5.4),
    ("mid_tom", 5.0),
    ("clave", 4.4),
    ("ganza", 2.3),
    ("low_conga", 3.4),
    ("mid_conga", -3.2),
    ("low_timbale", 5.2),
    ("high_timbale", 2.5),
    ("guiro", 5.5),
    ("triangle", 3.5),
    ("tambourine", 7.2),
    ("tambourine_shake", 6.6),
    ("cajon", -3.5),
    ("low_bongo", -5.2),
];

fn trimmed(synth: Synth) -> Synth {
    let Some(&(_, louder)) = TRIMS.iter().find(|(name, _)| *name == synth.name) else {
        return synth;
    };
    let gain = db(-louder) as f32;
    let build = synth.build.clone();
    Synth {
        build: std::sync::Arc::new(move |o, ctx| {
            let note = build(o, ctx);
            Note {
                sound: note.sound * gain,
                ..note
            }
        }),
        ..synth
    }
}

/// SPIKE: values for a design being fitted, read from the environment variable `name`, with `defaults` where it gives none.
fn tune(name: &str, defaults: &[f64]) -> Vec<f64> {
    let given: Vec<f64> = std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .filter_map(|x| x.trim().parse().ok())
        .collect();
    defaults
        .iter()
        .enumerate()
        .map(|(i, &d)| given.get(i).copied().unwrap_or(d))
        .collect()
}

fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

fn noise(ctx: &mut Context) -> Net {
    white_noise(ctx.seed())
}

fn impulse_net() -> Net {
    net(impulse::<U1>())
}

fn hpf(f: f64) -> Net {
    net(highpass_hz(
        f.clamp(20.0, 20_000.0) as f32,
        std::f32::consts::FRAC_1_SQRT_2,
    ))
}

fn lpf(f: f64) -> Net {
    net(lowpass_hz(
        f.clamp(20.0, 20_000.0) as f32,
        std::f32::consts::FRAC_1_SQRT_2,
    ))
}

/// A band-pass with a peak of 1, whose width is `rq` times its centre.
fn bpf(f: f64, rq: f64) -> Net {
    net(bandpass_hz(
        f.clamp(20.0, 20_000.0) as f32,
        (1.0 / rq) as f32,
    )) * rq as f32
}

/// A shelf above `f`, raised by `gain` dB.
fn high_shelf(f: f64, gain: f64) -> Net {
    net(highshelf_hz(
        f.clamp(20.0, 20_000.0) as f32,
        std::f32::consts::FRAC_1_SQRT_2,
        db(gain) as f32,
    ))
}

/// A shelf below `f`, raised by `gain` dB.
fn low_shelf(f: f64, gain: f64) -> Net {
    net(lowshelf_hz(
        f.clamp(20.0, 20_000.0) as f32,
        std::f32::consts::FRAC_1_SQRT_2,
        db(gain) as f32,
    ))
}

/// A bell at `f` whose width is `rq` times it, raised by `gain` dB.
fn peak(f: f64, rq: f64, gain: f64) -> Net {
    net(bell_hz(
        f.clamp(20.0, 20_000.0) as f32,
        (1.0 / rq) as f32,
        db(gain) as f32,
    ))
}

fn resonant_lowpass(f: f64, rq: f64) -> Net {
    net(lowpass_hz(
        f.clamp(20.0, 20_000.0) as f32,
        (1.0 / rq) as f32,
    ))
}

fn resonant_highpass(f: f64, rq: f64) -> Net {
    net(highpass_hz(
        f.clamp(20.0, 20_000.0) as f32,
        (1.0 / rq) as f32,
    ))
}

fn amps(levels: &[f64]) -> Vec<f64> {
    levels.iter().map(|&l| db(l)).collect()
}

fn scaled(ratios: &[f64], freq: f64) -> Vec<f64> {
    ratios.iter().map(|r| r * freq).collect()
}

fn times(values: &[f64], by: f64) -> Vec<f64> {
    values.iter().map(|v| v * by).collect()
}

fn vel(o: &Options, low: f64) -> f64 {
    o.number("vel").clamp(low, 1.0)
}

fn strike_options(note: f64) -> [Spec; 2] {
    [
        Spec::note("note").default(note).doc("The drum's pitch."),
        Spec::float("vel")
            .default(DEFAULT_VEL)
            .range(0.0, 1.0)
            .doc("How hard it is struck."),
    ]
}

/// Many close modes drawn from a seed between `low` and `high` Hz, sorted, each with the level in dB and ring at its band, in groups of 20 that `group` may rescale.
fn plate(seed_count: usize, low: f64, high: f64) -> Vec<Vec<f64>> {
    let mut draws = Draws::new(1956);
    let mut hz: Vec<f64> = (0..seed_count).map(|_| draws.exp(low, high)).collect();
    hz.sort_by(f64::total_cmp);
    hz.chunks(20).map(|c| c.to_vec()).collect()
}

const HAT_BANDS: [f64; 10] = [
    200.0, 300.0, 500.0, 800.0, 1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0,
];
