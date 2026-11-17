//! What the synths share: sines, band-passes, additive partials and the swell of a held note.

use crate::nodes::{db, segments};
use crate::{Options, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;

/// A sine at `hz`, starting `phase` radians in.
pub(crate) fn sine_from(hz: f64, phase: f64) -> Net {
    net(dc(hz as f32))
        >> net(An(Sine::<f32>::with_phase(
            (phase / (2.0 * PI)).rem_euclid(1.0) as f32,
        )))
}

/// The seconds until resonators of `levels`, fading 60 dB over `rings`, all fall below 80 dB under full scale once scaled by `gain`, as scsynth would free them.
pub(crate) fn silent_after(levels: &[f64], rings: &[f64], gain: f64) -> f64 {
    levels
        .iter()
        .zip(rings)
        .filter(|(l, _)| l.abs() * gain > 1e-4)
        .map(|(l, r)| r * 20.0 * (l.abs() * gain / 1e-4).log10() / 60.0)
        .fold(0.1, f64::max)
}

pub(crate) fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

/// A band-pass with a peak of 1, whose width is `rq` times its centre.
pub(crate) fn bpf(f: f64, rq: f64) -> Net {
    net(bandpass_hz(
        f.clamp(20.0, 20_000.0) as f32,
        (1.0 / rq) as f32,
    )) * rq as f32
}

/// Sines at `ratios` times the pitch its input gives, each at its level and starting phase in radians, as one sound.
pub(crate) fn partials(pitch: Net, ratios: &[f64], levels: &[f64], phases: &[f64]) -> Net {
    let kept: Vec<(f32, f32, f32)> = ratios
        .iter()
        .zip(levels)
        .zip(phases)
        .filter(|((_, l), _)| **l != 0.0)
        .map(|((r, l), p)| {
            (
                *r as f32,
                *l as f32,
                (*p / (2.0 * PI)).rem_euclid(1.0) as f32,
            )
        })
        .collect();
    pitch
        >> net(An(Partials {
            start: kept.iter().map(|k| k.2).collect(),
            ratios: kept.iter().map(|k| (k.0, k.1)).collect(),
            phases: kept.iter().map(|k| k.2).collect(),
            rate: DEFAULT_SR as f32,
        }))
}

/// The phases mx's additive designs start their harmonics at, so they do not all peak together.
pub(crate) fn spread_phases(count: usize) -> Vec<f64> {
    (1..=count)
        .map(|n| PI * n as f64 * (n as f64 - 1.0) / 24.0)
        .collect()
}

pub(super) fn harmonics(count: usize) -> Vec<f64> {
    (1..=count).map(|n| n as f64).collect()
}

/// A rise to full over `seconds`, though never over more than part of the note's sustain.
pub(super) fn swell(o: &Options, seconds: f64) -> Net {
    segments(
        &[0.0, 1.0],
        &[seconds.min(o.number("sustain") * 0.4).max(1e-4)],
        &[-2.0],
    )
}

/// Sines summed, each at a ratio of the pitch its input gives.
#[derive(Clone)]
struct Partials {
    start: Vec<f32>,
    ratios: Vec<(f32, f32)>,
    phases: Vec<f32>,
    rate: f32,
}

impl AudioNode for Partials {
    const ID: u64 = 0x6d78_0040;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.phases.clone_from(&self.start);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let step = input[0] / self.rate;
        let mut sum = 0.0;
        for ((ratio, level), phase) in self.ratios.iter().zip(self.phases.iter_mut()) {
            sum += level * (std::f32::consts::TAU * *phase).sin();
            *phase = (*phase + step * ratio).fract();
        }
        [sum].into()
    }
}

pub(super) fn struck(name: &str, release: f64) -> crate::SynthDecl {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard it is struck."),
        )
        .options(Spec::envelope(1000.0, release))
}

pub(super) fn lpf(f: f64) -> Net {
    net(butterpass_hz(f as f32))
}

pub(super) fn hpf(f: f64) -> Net {
    net(highpass_hz(f as f32, std::f32::consts::FRAC_1_SQRT_2))
}

pub(super) fn above(f: f64, corner: f64) -> f64 {
    (f / corner).log2().max(0.0)
}

/// A value read at `f` from `values` at the bands' `centres`, along straight lines in octaves, carried on below the lowest for up to two bands' widths.
pub(super) fn along(centres: &[f64], values: &[f64], f: f64) -> f64 {
    let x = f.log2();
    values[0]
        + values
            .windows(2)
            .enumerate()
            .map(|(i, w)| {
                let (low, high) = (centres[i].log2(), centres[i + 1].log2());
                (w[1] - w[0])
                    * ((x - low) / (high - low)).clamp(if i == 0 { -2.0 } else { 0.0 }, 1.0)
            })
            .sum::<f64>()
}

/// An instrument fitted to recordings at five velocities: each note's fundamental by band against the lowest note's, and each harmonic's level against the fundamental and fade, by harmonic and band.
pub(super) struct Fitted {
    pub bands: &'static [f64; 22],
    pub vels: [f64; 5],
    pub own: &'static [[f64; 22]; 5],
    pub levels: &'static [[[f64; 22]; 15]; 5],
    pub fades: &'static [[[f64; 22]; 15]; 5],
}

impl Fitted {
    /// A note's fundamental level in dB, and its harmonics below 5 kHz as their pitches, levels and fades, read between the two recorded velocities around vel.
    pub fn harmonics(&self, freq: f64, vel: f64) -> (f64, Vec<f64>, Vec<f64>, Vec<f64>) {
        let upper = Ord::max(self.vels.iter().position(|v| *v >= vel).unwrap_or(4), 1);
        let lower = upper - 1;
        let mix =
            ((vel - self.vels[lower]) / (self.vels[upper] - self.vels[lower])).clamp(0.0, 1.0);
        let row = |n: usize| match n {
            1..=10 => n - 1,
            11..=12 => 10,
            13..=15 => 11,
            16..=20 => 12,
            21..=30 => 13,
            _ => 14,
        };
        let at = |table: &[[[f64; 22]; 15]; 5], n: usize, f: f64| {
            along(self.bands, &table[lower][row(n)], f) * (1.0 - mix)
                + along(self.bands, &table[upper][row(n)], f) * mix
        };
        let own = along(self.bands, &self.own[lower], freq) * (1.0 - mix)
            + along(self.bands, &self.own[upper], freq) * mix;
        let harmonics: Vec<(usize, f64)> = (1..=60)
            .map(|n| (n, freq * n as f64))
            .filter(|(_, f)| *f < 5000.0)
            .collect();
        let pitches = harmonics.iter().map(|(_, f)| *f).collect();
        let levels = harmonics
            .iter()
            .map(|(n, f)| db(own + at(self.levels, *n, *f)))
            .collect();
        let fades = harmonics
            .iter()
            // A resonator reads longer than the fade it is given once the measure takes its harmonics' medians by band, so each starts shorter.
            .map(|(n, f)| (at(self.fades, *n, *f) * 0.75).max(0.05))
            .collect();
        (own, pitches, levels, fades)
    }
}
