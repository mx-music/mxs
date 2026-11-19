//! What the synths share: sines, band-passes, additive partials and the swell of a held note.

use crate::nodes::{db, hz, segments};
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

/// A unit that adds `by` of the option `name`, as it glides, to its input.
pub(crate) fn moved_by(options: &Options, name: &str, by: fn(f32) -> f32) -> Net {
    let moved =
        (net(pass()) | options.follow(name)) >> net(map(move |x: &Frame<f32, U2>| x[0] + by(x[1])));
    net(Unhashed(moved))
}

/// A unit that hands the hash it is pinged with on untouched, so the oscillators after it take the phase they took before it was there.
#[derive(Clone)]
struct Unhashed(Net);

impl AudioUnit for Unhashed {
    fn reset(&mut self) {
        self.0.reset();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.0.set_sample_rate(sample_rate);
    }

    fn tick(&mut self, input: &[f32], output: &mut [f32]) {
        self.0.tick(input, output);
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        self.0.process(size, input, output);
    }

    fn inputs(&self) -> usize {
        self.0.inputs()
    }

    fn outputs(&self) -> usize {
        self.0.outputs()
    }

    fn route(&mut self, input: &SignalFrame, frequency: f64) -> SignalFrame {
        self.0.route(input, frequency)
    }

    fn get_id(&self) -> u64 {
        self.0.get_id()
    }

    fn ping(&mut self, probe: bool, hash: AttoHash) -> AttoHash {
        self.0.ping(probe, hash.clone());
        hash
    }

    fn footprint(&self) -> usize {
        self.0.footprint()
    }

    fn allocate(&mut self) {
        self.0.allocate();
    }
}

/// `sound` tilted by brightness 3 dB an octave either way of 0.5, from the note's pitch up, as a brightness tilts a voice's partials; at 0.5 it is `sound` itself.
/// Four high shelves two octaves apart, each 6 dB at brightness 0 or 1, make the slope.
pub(crate) fn tilted(options: &Options, sound: Net) -> Net {
    let pitch = || {
        options.follow("note")
            >> moved_by(options, "bend", |x| x)
            >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32))
    };
    let gain = || {
        options.follow("brightness")
            >> net(map(|x: &Frame<f32, U1>| {
                db((x[0] as f64 - 0.5) * 12.0) as f32
            }))
    };
    let mut shelves = net(pass());
    for k in 0..4 {
        let above = 2.0 * 4f32.powi(k);
        let corner = pitch() >> net(map(move |x: &Frame<f32, U1>| (x[0] * above).min(18_000.0)));
        shelves = (shelves | corner | net(dc(0.707)) | gain()) >> net(highshelf::<f32>());
    }
    let chosen = map(|x: &Frame<f32, U3>| if x[2] == 0.5 { x[0] } else { x[1] });
    ((sound >> (net(pass()) ^ shelves)) | options.follow("brightness")) >> net(chosen)
}
