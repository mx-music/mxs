//! mx's effects, each a stereo sound in and out, with options that glide as the song changes them.
//! Times such as phase and decay come in seconds; mx turns its beats into seconds before it starts an effect.

use crate::nodes::hz;
use crate::options::Options;
use crate::spec::Spec;
use crate::synth::Effect;
use fundsp::prelude::*;
use std::f32::consts::{FRAC_PI_4, TAU};

/// Every effect mx has.
pub fn all() -> Vec<Effect> {
    vec![
        level(),
        reverb(),
        echo(),
        lpf(),
        hpf(),
        distortion(),
        pan(),
        slicer(),
        compressor(),
        chorus(),
        bitcrusher(),
        tremolo(),
        rlpf(),
        bpf(),
        flanger(),
        wobble(),
        ping_pong(),
    ]
}

fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

/// The same mono effect on each channel, built twice.
fn per_channel(build: impl Fn() -> Net) -> Net {
    build() | build()
}

/// A MIDI note option as Hz, held to what a filter can take.
fn cutoff(options: &Options, name: &str) -> Net {
    options.follow(name) >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64).clamp(20.0, 20_000.0) as f32))
}

/// Resonance from 0 to 1 as a filter's Q: the nearer 1, the more it rings.
fn q_of(options: &Options, name: &str) -> Net {
    options.follow(name) >> net(map(|x: &Frame<f32, U1>| 1.0 / (1.0 - x[0].clamp(0.0, 1.0)).max(0.01)))
}

fn level() -> Effect {
    Effect::new("level").tail(0.0).build(|_, _| net(multipass::<U2>()))
}

fn reverb() -> Effect {
    Effect::new("reverb")
        .option(Spec::float("room").default(0.6).range(0.0, 1.0).doc("How big the room is."))
        .option(Spec::float("damp").default(0.5).range(0.0, 1.0).doc("How much the room dulls the high frequencies."))
        .tail_from(|o| (o.number("room") * 10.0 + 1.0).min(11.0))
        .build(|o, _| (net(multipass::<U2>()) | o.follow("room") | o.follow("damp")) >> net(An(Freeverb::new())))
}

fn echo() -> Effect {
    Effect::new("echo")
        .option(Spec::beats("phase").default(0.25).range(0.0, f64::INFINITY).doc("The time between echoes."))
        .option(Spec::beats("decay").default(2.0).doc("How long the echoes take to die away."))
        .option(Spec::beats("max_phase").default(2.0).range(0.0, 60.0).doc("The longest phase it can change to."))
        .tail_from(|o| o.number("decay").abs())
        .build(|o, _| {
            let most = o.number("max_phase");
            per_channel(|| {
                let comb = (net(pass()) | o.follow("phase") | o.follow("decay")) >> net(An(Comb::new(most, false)));
                net(pass()) & comb
            })
        })
}

fn lpf() -> Effect {
    Effect::new("lpf")
        .option(Spec::note("cutoff").default(100.0).doc("The MIDI note above which it cuts."))
        .tail(0.1)
        .build(|o, _| per_channel(|| (net(pass()) | cutoff(o, "cutoff") | net(dc(FRAC_1_SQRT_2))) >> net(lowpass::<f32>())))
}

fn hpf() -> Effect {
    Effect::new("hpf")
        .option(Spec::note("cutoff").default(100.0).doc("The MIDI note below which it cuts."))
        .tail(0.1)
        .build(|o, _| per_channel(|| (net(pass()) | cutoff(o, "cutoff") | net(dc(FRAC_1_SQRT_2))) >> net(highpass::<f32>())))
}

const FRAC_1_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn distortion() -> Effect {
    Effect::new("distortion")
        .option(Spec::float("distort").default(0.5).range(0.0, 1.0).doc("How hard it drives the sound."))
        .tail(0.0)
        .build(|o, _| {
            per_channel(|| {
                (net(pass()) | o.follow("distort"))
                    >> net(map(|x: &Frame<f32, U2>| {
                        let drive = 1.0 + x[1].clamp(0.0, 1.0) * 30.0;
                        (x[0] * drive).tanh() / drive.tanh()
                    }))
            })
        })
}

fn pan() -> Effect {
    Effect::new("pan")
        .option(Spec::float("pan").default(0.0).range(-1.0, 1.0).doc("Where it turns the sound, from -1, left, to 1, right."))
        .tail(0.0)
        .build(|o, _| {
            (net(multipass::<U2>()) | o.follow("pan"))
                >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U2> {
                    let pan = x[2].clamp(-1.0, 1.0);
                    [x[0] * (1.0 - pan).min(1.0), x[1] * (1.0 + pan).min(1.0)].into()
                }))
        })
}

fn slicer() -> Effect {
    Effect::new("slicer")
        .option(Spec::beats("phase").default(0.25).range(0.0, f64::INFINITY).doc("The length of a cycle."))
        .option(Spec::float("phase_offset").default(0.0).range(0.0, 1.0).doc("Where in the cycle it starts."))
        .option(Spec::float("pulse_width").default(0.5).range(0.0, 1.0).doc("The part of the cycle that is on."))
        .option(Spec::float("amp_min").default(0.0).doc("The level while off."))
        .option(Spec::float("amp_max").default(1.0).doc("The level while on."))
        .option(Spec::seconds("smooth").default(0.0).range(0.0, f64::INFINITY).doc("How many seconds each change takes."))
        .tail(0.0)
        .build(|o, _| {
            let offset = o.number("phase_offset") as f32;
            let open = (rate_of(o, "phase") | o.follow("pulse_width") | net(dc(1.0)) | net(dc(0.0)))
                >> net(An(Lfo::new(offset, 1.0, true)));
            let low = o.follow("amp_min");
            let level = (open | low | o.follow("amp_max"))
                >> net(map(|x: &Frame<f32, U3>| x[1] + x[0] * (x[2] - x[1])))
                >> net(An(Lag::new(o.number("smooth"))));
            (net(multipass::<U2>()) | level)
                >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U2> { [x[0] * x[2], x[1] * x[2]].into() }))
        })
}

/// Cycles a second of an option measured as the length of one cycle.
fn rate_of(options: &Options, name: &str) -> Net {
    options.follow(name) >> net(map(|x: &Frame<f32, U1>| 1.0 / x[0].max(0.001)))
}

fn compressor() -> Effect {
    Effect::new("compressor")
        .option(Spec::float("threshold").default(0.2).range(0.0001, f64::INFINITY).doc("The level above which it lowers the sound."))
        .option(Spec::float("slope_below").default(1.0).doc("How much the level changes below the threshold."))
        .option(Spec::float("slope_above").default(0.5).doc("How much the level changes above the threshold."))
        .option(Spec::seconds("clamp_time").default(0.01).range(0.0, f64::INFINITY).doc("How many seconds it takes to lower the sound."))
        .option(Spec::seconds("relax_time").default(0.01).range(0.0, f64::INFINITY).doc("How many seconds it takes to let go."))
        .tail(0.1)
        .build(|o, _| {
            let controls = o.follow("threshold")
                | o.follow("slope_below")
                | o.follow("slope_above")
                | o.follow("clamp_time")
                | o.follow("relax_time");
            (net(multipass::<U2>()) | controls) >> net(An(Compander::new()))
        })
}

fn chorus() -> Effect {
    Effect::new("chorus")
        .option(Spec::beats("phase").default(0.25).range(0.0, f64::INFINITY).doc("How long the delay takes to sweep back and forth."))
        .option(Spec::beats("decay").default(0.00001).doc("How long the delayed sound feeds back."))
        .option(Spec::beats("max_phase").default(1.0).range(0.0, 60.0).doc("The longest phase it can change to."))
        .tail_from(|o| o.number("decay").abs())
        .build(|o, _| {
            let most = o.number("max_phase").max(0.02);
            let side = |start: f32| {
                let sweep = (rate_of(o, "phase") | net(dc(0.5)) | net(dc(3.0)) | net(dc(0.0)))
                    >> net(An(Lfo::new(start, 1.0, false)))
                    >> net(map(|x: &Frame<f32, U1>| 0.007 + 0.005 * (x[0] * 2.0 - 1.0)));
                let comb = (net(pass()) | sweep | o.follow("decay")) >> net(An(Comb::new(most, true)));
                (net(pass()) & comb) * 0.5
            };
            side(0.0) | side(0.5)
        })
}

fn bitcrusher() -> Effect {
    Effect::new("bitcrusher")
        .option(Spec::hz("sample_rate").default(10_000.0).range(1.0, f64::INFINITY).doc("The rate in Hz it holds samples at."))
        .option(Spec::float("bits").default(8.0).range(1.0, 24.0).doc("How many bits it rounds each sample to."))
        .option(Spec::note("cutoff").default(0.0).doc("A MIDI note for a low-pass after it, off at 0."))
        .tail(0.1)
        .build(|o, _| {
            per_channel(|| {
                let crushed = (net(pass()) | o.follow("sample_rate") | o.follow("bits")) >> net(An(Crusher::new()));
                let off = o.follow("cutoff") >> net(map(|x: &Frame<f32, U1>| if x[0] > 0.0 { 1.0 } else { 0.0 }));
                let filtered = net(pass()) ^ ((net(pass()) | cutoff(o, "cutoff") | net(dc(FRAC_1_SQRT_2))) >> net(lowpass::<f32>()));
                ((crushed >> filtered) | off) >> net(map(|x: &Frame<f32, U3>| x[0] + (x[1] - x[0]) * x[2]))
            })
        })
}

/// The wave options every swept effect takes, with its own default wave.
fn wave_options(effect: crate::synth::EffectDecl, wave: f64, phase: f64) -> crate::synth::EffectDecl {
    effect
        .option(Spec::beats("phase").default(phase).range(0.0, f64::INFINITY).doc("How long one sweep takes."))
        .option(Spec::float("wave").default(wave).range(0.0, 4.0).doc("The sweep's shape: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic."))
        .option(Spec::float("invert_wave").default(0.0).range(0.0, 1.0).doc("1 flips the sweep."))
        .option(Spec::float("phase_offset").default(0.0).range(0.0, 1.0).doc("Where in the sweep it starts."))
}

/// The sweep from 0 to 1, of the shape and length the wave options give.
fn sweep(options: &Options, width: Net) -> Net {
    let offset = options.number("phase_offset") as f32;
    (rate_of(options, "phase") | width | options.follow("wave") | options.follow("invert_wave"))
        >> net(An(Lfo::new(offset, 1.0, false)))
}

fn tremolo() -> Effect {
    wave_options(Effect::new("tremolo"), 2.0, 4.0)
        .option(Spec::float("depth").default(0.5).range(0.0, 1.0).doc("How far it turns the sound down."))
        .tail(0.0)
        .build(|o, _| {
            let level = (sweep(o, net(dc(0.5))) | o.follow("depth"))
                >> net(map(|x: &Frame<f32, U2>| 1.0 - x[1] * x[0]));
            (net(multipass::<U2>()) | level) >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U2> { [x[0] * x[2], x[1] * x[2]].into() }))
        })
}

fn rlpf() -> Effect {
    Effect::new("rlpf")
        .option(Spec::note("cutoff").default(100.0).doc("The MIDI note above which it cuts."))
        .option(Spec::float("res").default(0.5).range(0.0, 1.0).doc("How much it rings at its cutoff."))
        .tail(0.2)
        .build(|o, _| per_channel(|| (net(pass()) | cutoff(o, "cutoff") | q_of(o, "res")) >> net(lowpass::<f32>())))
}

fn bpf() -> Effect {
    Effect::new("bpf")
        .option(Spec::note("centre").default(100.0).doc("The MIDI note of the band it keeps."))
        .option(Spec::float("res").default(0.6).range(0.0, 1.0).doc("How narrow the band is."))
        .tail(0.2)
        .build(|o, _| {
            per_channel(|| {
                // FunDSP's band-pass peaks at its Q, so it is scaled back to a peak of 1.
                let band = (net(pass()) | cutoff(o, "centre") | q_of(o, "res")) >> net(bandpass::<f32>());
                (band | q_of(o, "res")) >> net(map(|x: &Frame<f32, U2>| x[0] / x[1]))
            })
        })
}

fn flanger() -> Effect {
    wave_options(Effect::new("flanger"), 4.0, 4.0)
        .option(Spec::float("stereo_invert_wave").default(0.0).range(0.0, 1.0).doc("1 sweeps the left channel the other way."))
        .option(Spec::float("delay").default(5.0).doc("The delay in milliseconds it sweeps up from."))
        .option(Spec::float("depth").default(5.0).doc("How far in milliseconds it sweeps."))
        .option(Spec::float("max_delay").default(20.0).range(1.0, 1000.0).doc("The longest delay in milliseconds."))
        .option(Spec::float("feedback").default(0.0).range(0.0, 1.0).doc("How strongly the delayed sound feeds back."))
        .option(Spec::beats("decay").default(2.0).doc("How long the delayed sound feeds back."))
        .option(Spec::float("invert_flange").default(0.0).range(0.0, 1.0).doc("1 subtracts the delayed sound instead."))
        .tail(1.0)
        .build(|o, _| {
            let most = o.number("max_delay") / 1000.0;
            let flip = if o.number("stereo_invert_wave") >= 0.5 { 1.0 } else { 0.0 };
            let sign = if o.number("invert_flange") >= 0.5 { -1.0 } else { 1.0 };
            let side = |flipped: f32| {
                let level = sweep(o, net(dc(0.5)))
                    >> net(map(move |x: &Frame<f32, U1>| if flipped > 0.0 { 1.0 - x[0] } else { x[0] }));
                let limit = most as f32;
                let delay = (o.follow("delay") | o.follow("depth") | level)
                    >> net(map(move |x: &Frame<f32, U3>| ((x[0] + x[1] * x[2]) / 1000.0).clamp(0.0001, limit)));
                let decay = (o.follow("decay") | o.follow("feedback"))
                    >> net(map(|x: &Frame<f32, U2>| x[0] * x[1].clamp(0.0, 1.0)));
                let comb = (net(pass()) | delay | decay) >> net(An(Comb::new(most, true)));
                net(pass()) & (comb * sign)
            };
            side(flip) | side(0.0)
        })
}

fn wobble() -> Effect {
    wave_options(Effect::new("wobble"), 0.0, 0.5)
        .option(Spec::float("pulse_width").default(0.5).range(0.0, 1.0).doc("The part of a cycle that is up, for wave 1."))
        .option(Spec::note("cutoff_min").default(60.0).doc("The MIDI note the filter sweeps down to."))
        .option(Spec::note("cutoff_max").default(120.0).doc("The MIDI note the filter sweeps up to."))
        .option(Spec::float("res").default(0.8).range(0.0, 1.0).doc("How much the filter rings at its cutoff."))
        .option(Spec::float("filter").default(0.0).range(0.0, 1.0).doc("0 for a low-pass and 1 for a high-pass."))
        .tail(0.2)
        .build(|o, _| {
            let high = o.number("filter") >= 0.5;
            per_channel(|| {
                let level = sweep(o, o.follow("pulse_width"));
                let cut = (level | o.follow("cutoff_min") | o.follow("cutoff_max"))
                    >> net(map(|x: &Frame<f32, U3>| hz((x[1] + x[0] * (x[2] - x[1])) as f64).clamp(20.0, 20_000.0) as f32));
                let input = net(pass()) | cut | q_of(o, "res");
                if high { input >> net(highpass::<f32>()) } else { input >> net(lowpass::<f32>()) }
            })
        })
}

fn ping_pong() -> Effect {
    Effect::new("ping_pong")
        .option(Spec::beats("phase").default(0.25).range(0.001, f64::INFINITY).doc("The time between echoes."))
        .option(Spec::float("feedback").default(0.5).range(0.0, 0.999).doc("How much of each echo the next keeps."))
        .option(Spec::beats("max_phase").default(1.0).range(0.01, 60.0).doc("The longest phase it can change to."))
        .option(Spec::float("pan_start").default(1.0).range(-1.0, 1.0).doc("The side of the first echo, from -1, left, to 1, right."))
        .tail_from(|o| {
            let (phase, feedback) = (o.number("phase"), o.number("feedback").max(0.001));
            (2.0 * phase * (-60.0 / (20.0 * feedback.log10())).max(1.0)).min(60.0)
        })
        .build(|o, _| {
            let side = o.number("pan_start") as f32;
            (net(multipass::<U2>()) | o.follow("phase") | o.follow("feedback"))
                >> net(An(PingPong::new(o.number("max_phase"), side)))
        })
}

/// A delay of the input that feeds back on itself, by as much as dies away 60 dB over its decay; a negative decay feeds it back inverted.
/// Inputs: sound, delay in seconds, decay in seconds.
#[derive(Clone)]
struct Comb {
    most: f64,
    cubic: bool,
    buffer: Vec<f32>,
    at: usize,
    rate: f32,
}

impl Comb {
    fn new(most: f64, cubic: bool) -> Self {
        let mut comb = Comb { most, cubic, buffer: Vec::new(), at: 0, rate: DEFAULT_SR as f32 };
        comb.set_sample_rate(DEFAULT_SR);
        comb
    }

    fn read(&self, frames: f32) -> f32 {
        let n = self.buffer.len();
        let whole = frames.floor();
        let fraction = frames - whole;
        let back = |k: isize| self.buffer[(self.at as isize - whole as isize - k).rem_euclid(n as isize) as usize];
        if self.cubic {
            let (y0, y1, y2, y3) = (back(-1), back(0), back(1), back(2));
            let c1 = 0.5 * (y2 - y0);
            let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
            ((c3 * fraction + c2) * fraction + c1) * fraction + y1
        } else {
            back(0) + (back(1) - back(0)) * fraction
        }
    }
}

impl AudioNode for Comb {
    const ID: u64 = 0x6d78_0010;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.buffer = vec![0.0; (self.most * sample_rate).ceil() as usize + 4];
        self.at = 0;
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let limit = (self.buffer.len() - 3) as f32;
        let frames = (input[1] * self.rate).clamp(1.0, limit);
        self.at = (self.at + 1) % self.buffer.len();
        let delayed = self.read(frames);
        let decay = input[2];
        let feedback = if decay == 0.0 {
            0.0
        } else {
            0.001f32.powf(input[1].max(0.0) / decay.abs()).copysign(decay)
        };
        self.buffer[self.at] = input[0] + feedback * delayed;
        [delayed].into()
    }
}

/// Jezar's Freeverb, in the public domain: eight damped combs and four all-passes per channel, the right channel's tuned 23 samples longer.
/// Inputs: left, right, room, damp; it gives the wet sound only.
#[derive(Clone)]
struct Freeverb {
    combs: [Vec<Vec<f32>>; 2],
    comb_at: [[usize; 8]; 2],
    stores: [[f32; 8]; 2],
    allpasses: [Vec<Vec<f32>>; 2],
    allpass_at: [[usize; 4]; 2],
}

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

impl Freeverb {
    fn new() -> Self {
        let mut verb = Freeverb {
            combs: [Vec::new(), Vec::new()],
            comb_at: [[0; 8]; 2],
            stores: [[0.0; 8]; 2],
            allpasses: [Vec::new(), Vec::new()],
            allpass_at: [[0; 4]; 2],
        };
        verb.set_sample_rate(DEFAULT_SR);
        verb
    }
}

impl AudioNode for Freeverb {
    const ID: u64 = 0x6d78_0011;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let scale = sample_rate / 44_100.0;
        let length = |n: usize| Ord::max((n as f64 * scale).round() as usize, 1);
        for side in 0..2 {
            self.combs[side] = COMBS.iter().map(|&n| vec![0.0; length(n + side * SPREAD)]).collect();
            self.allpasses[side] = ALLPASSES.iter().map(|&n| vec![0.0; length(n + side * SPREAD)]).collect();
        }
        self.comb_at = [[0; 8]; 2];
        self.allpass_at = [[0; 4]; 2];
        self.stores = [[0.0; 8]; 2];
    }

    fn reset(&mut self) {
        self.set_sample_rate(DEFAULT_SR);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let feedback = input[2].clamp(0.0, 1.0) * 0.28 + 0.7;
        let damp = input[3].clamp(0.0, 1.0) * 0.4;
        let sound = (input[0] + input[1]) * 0.015;
        let mut out = [0.0f32; 2];
        for (side, out) in out.iter_mut().enumerate() {
            let mut sum = 0.0;
            for k in 0..8 {
                let buffer = &mut self.combs[side][k];
                let at = self.comb_at[side][k];
                let y = buffer[at];
                self.stores[side][k] = y * (1.0 - damp) + self.stores[side][k] * damp;
                buffer[at] = sound + self.stores[side][k] * feedback;
                self.comb_at[side][k] = (at + 1) % buffer.len();
                sum += y;
            }
            for k in 0..4 {
                let buffer = &mut self.allpasses[side][k];
                let at = self.allpass_at[side][k];
                let held = buffer[at];
                buffer[at] = sum + held * 0.5;
                sum = held - sum;
                self.allpass_at[side][k] = (at + 1) % buffer.len();
            }
            *out = sum;
        }
        out.into()
    }
}

/// A control wave from 0 to 1: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic, starting `offset` of a cycle in.
/// Inputs: cycles a second, the pulse's width, the shape, and 1 to flip it.
#[derive(Clone)]
struct Lfo {
    offset: f32,
    phase: f32,
    level: f32,
    pulse_only: bool,
    rate: f32,
}

impl Lfo {
    fn new(offset: f32, level: f32, pulse_only: bool) -> Self {
        Lfo { offset, phase: offset.rem_euclid(1.0), level, pulse_only, rate: DEFAULT_SR as f32 }
    }
}

impl AudioNode for Lfo {
    const ID: u64 = 0x6d78_0012;
    type Inputs = U4;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.phase = self.offset.rem_euclid(1.0);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U1> {
        let p = self.phase;
        let shape = if self.pulse_only { 1 } else { input[2].clamp(0.0, 4.0).round() as u32 };
        let wave = match shape {
            0 => 2.0 * p - 1.0,
            1 => {
                if p < input[1] {
                    1.0
                } else {
                    -1.0
                }
            }
            2 => 4.0 * ((p - 0.25).rem_euclid(1.0) - 0.5).abs() - 1.0,
            3 => (TAU * p).sin(),
            _ => {
                // Two cubic halves shaped like a sine.
                let half = (p * 2.0).fract();
                let bump = 4.0 * half * (1.0 - half);
                let smooth = bump * (1.5 - 0.5 * bump);
                if p < 0.5 { smooth } else { -smooth }
            }
        };
        let level = (wave + 1.0) / 2.0;
        let level = if input[3] >= 0.5 { 1.0 - level } else { level };
        self.phase = (self.phase + input[0].max(0.0) / self.rate).fract();
        [level * self.level].into()
    }
}

/// Glides to its input, falling 60 dB short of each step over `seconds`.
#[derive(Clone)]
struct Lag {
    seconds: f64,
    keep: f32,
    level: Option<f32>,
}

impl Lag {
    fn new(seconds: f64) -> Self {
        let mut lag = Lag { seconds, keep: 0.0, level: None };
        lag.set_sample_rate(DEFAULT_SR);
        lag
    }
}

impl AudioNode for Lag {
    const ID: u64 = 0x6d78_0013;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.keep = if self.seconds > 0.0 { 0.001f64.powf(1.0 / (self.seconds * sample_rate)) as f32 } else { 0.0 };
    }

    fn reset(&mut self) {
        self.level = None;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let level = match self.level {
            Some(level) => input[0] + (level - input[0]) * self.keep,
            None => input[0],
        };
        self.level = Some(level);
        [level].into()
    }
}

/// Follows the level of each channel and turns it down above the threshold, or up below it, by the slopes.
/// Inputs: left, right, threshold, slope below, slope above, clamp time, relax time.
#[derive(Clone)]
struct Compander {
    level: [f32; 2],
    rate: f32,
}

impl Compander {
    fn new() -> Self {
        Compander { level: [0.0; 2], rate: DEFAULT_SR as f32 }
    }
}

impl AudioNode for Compander {
    const ID: u64 = 0x6d78_0014;
    type Inputs = U7;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.level = [0.0; 2];
    }

    fn tick(&mut self, input: &Frame<f32, U7>) -> Frame<f32, U2> {
        let threshold = input[2].max(0.0001);
        let keep = |seconds: f32| if seconds > 0.0 { 0.001f32.powf(1.0 / (seconds * self.rate)) } else { 0.0 };
        let (clamp, relax) = (keep(input[5]), keep(input[6]));
        let mut out = [0.0f32; 2];
        for side in 0..2 {
            let x = input[side];
            let size = x.abs();
            let keep = if size > self.level[side] { clamp } else { relax };
            self.level[side] = size + (self.level[side] - size) * keep;
            let level = self.level[side];
            let gain = if level > threshold {
                (level / threshold).powf(input[4] - 1.0)
            } else if level > 0.0 {
                (level / threshold).powf(input[3] - 1.0)
            } else {
                1.0
            };
            out[side] = x * gain.min(1000.0);
        }
        out.into()
    }
}

/// Airwindows' DeRez, as mx's design describes it: each sample held at the sample rate, its step softened by blending in the sample held before, then rounded away from zero to a step of bits.
/// Inputs: sound, sample rate in Hz, bits.
#[derive(Clone)]
struct Crusher {
    since: f32,
    held: f32,
    before: f32,
    rate: f32,
}

impl Crusher {
    fn new() -> Self {
        Crusher { since: f32::INFINITY, held: 0.0, before: 0.0, rate: DEFAULT_SR as f32 }
    }
}

impl AudioNode for Crusher {
    const ID: u64 = 0x6d78_0015;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        (self.since, self.held, self.before) = (f32::INFINITY, 0.0, 0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let rate = input[1].clamp(1.0, self.rate);
        let period = self.rate / rate;
        if self.since >= period {
            self.before = self.held;
            self.held = input[0];
            self.since -= period;
            if self.since >= period {
                self.since = 0.0;
            }
        }
        self.since += 1.0;
        let soften = (1.0 + rate / self.rate) / 2.0;
        let softened = self.held * soften + self.before * (1.0 - soften);
        let step = 2.0 / 2f32.powf(input[2].clamp(1.0, 24.0));
        [softened.signum() * (softened.abs() / step).ceil() * step].into()
    }
}

/// Echoes a phase apart, alternating between one side and the other, each `feedback` times the one before.
/// Inputs: left, right, phase in seconds, feedback.
#[derive(Clone)]
struct PingPong {
    most: f64,
    side: f32,
    first: Vec<f32>,
    second: Vec<f32>,
    at: usize,
    rate: f32,
}

impl PingPong {
    fn new(most: f64, side: f32) -> Self {
        let mut ping = PingPong { most, side, first: Vec::new(), second: Vec::new(), at: 0, rate: DEFAULT_SR as f32 };
        ping.set_sample_rate(DEFAULT_SR);
        ping
    }

    fn read(buffer: &[f32], at: usize, frames: f32) -> f32 {
        let n = buffer.len();
        let whole = frames.floor() as usize;
        let fraction = frames - frames.floor();
        let a = buffer[(at + n - whole) % n];
        let b = buffer[(at + n - whole - 1) % n];
        a + (b - a) * fraction
    }
}

impl AudioNode for PingPong {
    const ID: u64 = 0x6d78_0016;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        let n = (self.most * sample_rate).ceil() as usize + 4;
        self.first = vec![0.0; n];
        self.second = vec![0.0; n];
        self.at = 0;
    }

    fn reset(&mut self) {
        self.first.fill(0.0);
        self.second.fill(0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let limit = (self.first.len() - 3) as f32;
        let frames = (input[2] * self.rate).clamp(1.0, limit);
        let feedback = input[3].clamp(0.0, 0.999);
        self.at = (self.at + 1) % self.first.len();
        let first = Self::read(&self.first, self.at, frames) * feedback;
        let second = Self::read(&self.second, self.at, frames) * feedback;
        self.first[self.at] = input[0] + input[1] + second;
        self.second[self.at] = first;
        let place = |side: f32| {
            let angle = (side + 1.0) * FRAC_PI_4;
            (angle.cos(), angle.sin())
        };
        let (fl, fr) = place(self.side);
        let (sl, sr) = place(-self.side);
        [input[0] + first * fl + second * sl, input[1] + first * fr + second * sr].into()
    }
}
