//! mx's plain synths: waves under a resonant low-pass, FM, a plucked string, a supersaw, an acid bass and a brass.

use super::common::moved_by;
use crate::nodes::{db, hz, segments, white_noise};
use crate::{Note, Options, Spec, Synth};
use fundsp::prelude::*;

pub fn all() -> Vec<Synth> {
    vec![
        saw(),
        square(),
        tri(),
        pulse(),
        noise(),
        fm(),
        pluck(),
        supersaw(),
        acid(),
        brass(),
    ]
}

fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

// FunDSP's saw and pulse waves measure 4.5 dB louder than scsynth's, and its square 3.8 dB, so they are brought down to the level mx's designs were made at.
// They start at phase 0 rather than FunDSP's hashed phase, so a saw and a pulse at one pitch add up the same in every note.
fn saw_wave() -> Net {
    from_zero(fundsp::prelude::saw()) * db(-4.5) as f32
}

fn pulse_wave() -> Net {
    from_zero(fundsp::prelude::pulse()) * db(-4.5) as f32
}

fn from_zero(mut wave: impl AudioUnit + 'static) -> Net {
    wave.set(Setting::phase(0.0));
    wave.reset();
    net(wave)
}

fn to_hz(note: Net) -> Net {
    note >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32))
}

/// A resonant low-pass whose cutoff is a MIDI note, with res from 0 to 1.
fn filtered(options: &Options, sound: Net) -> Net {
    filtered_at(options, options.follow("cutoff"), sound)
}

/// A resonant low-pass whose cutoff is the MIDI note `cutoff` gives, with res from 0 to 1.
fn filtered_at(options: &Options, cutoff: Net, sound: Net) -> Net {
    let cutoff = cutoff
        >> net(map(|x: &Frame<f32, U1>| {
            hz(x[0] as f64).clamp(20.0, 18_000.0) as f32
        }));
    let q = options.follow("res")
        >> net(map(|x: &Frame<f32, U1>| {
            1.0 / (1.0 - x[0].clamp(0.0, 1.0) * 0.93)
        }));
    (sound | cutoff | q) >> net(lowpass::<f32>())
}

fn filter_options(cutoff: f64) -> [Spec; 2] {
    [
        Spec::note("cutoff")
            .default(cutoff)
            .doc("The MIDI note above which the low-pass cuts."),
        Spec::float("res")
            .default(0.2)
            .range(0.0, 1.0)
            .doc("How much the low-pass rings at its cutoff."),
    ]
}

fn wave(name: &str, cutoff: f64, release: f64) -> crate::SynthDecl {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .options(filter_options(cutoff))
        .options(Spec::envelope(0.0, release))
}

fn saw() -> Synth {
    wave("saw", 100.0, 1.0)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, _| Note::envelope(brightened(o, bent(o) >> saw_wave()) * 0.5, o))
}

/// The note's frequency, gliding with its note and bend options.
fn bent(options: &Options) -> Net {
    to_hz(options.follow("note") >> moved_by(options, "bend", |x| x))
}

/// The low-pass of [`filtered`], its cutoff moved by brightness.
fn brightened(options: &Options, sound: Net) -> Net {
    let cutoff = options.follow("cutoff") >> moved_by(options, "brightness", brighter);
    filtered_at(options, cutoff, sound)
}

/// The semitones a brightness moves a low-pass's cutoff by: two octaves either way of 0.5.
fn brighter(brightness: f32) -> f32 {
    (brightness - 0.5) * 48.0
}

fn square() -> Synth {
    wave("square", 100.0, 1.0)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, _| {
            Note::envelope(
                brightened(o, bent(o) >> net(fundsp::prelude::square())) * (0.5 * db(-3.8)) as f32,
                o,
            )
        })
}

fn tri() -> Synth {
    wave("tri", 100.0, 1.0)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, _| Note::envelope(brightened(o, bent(o) >> net(triangle())) * 0.6, o))
}

fn pulse() -> Synth {
    wave("pulse", 100.0, 1.0)
        .option(
            Spec::float("pulse_width")
                .default(0.3)
                .range(0.01, 0.99)
                .doc("The part of each cycle that is up."),
        )
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, _| {
            let width =
                o.follow("pulse_width") >> net(map(|x: &Frame<f32, U1>| x[0].clamp(0.01, 0.99)));
            Note::envelope(brightened(o, (bent(o) | width) >> pulse_wave()) * 0.5, o)
        })
}

fn noise() -> Synth {
    wave("noise", 110.0, 1.0)
        .build(|o, ctx| Note::envelope(filtered(o, white_noise(ctx.seed())) * 0.4, o))
}

fn fm() -> Synth {
    Synth::new("fm")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("divisor")
                .default(2.0)
                .range(0.01, f64::INFINITY)
                .doc("The note's frequency over the modulator's."),
        )
        .option(
            Spec::float("depth")
                .default(1.0)
                .doc("How far the modulator swings the frequency, in times the note's."),
        )
        .options(Spec::envelope(0.0, 1.0))
        .options(Spec::expressions(&["bend"]))
        .build(|o, _| {
            let modulator = (bent(o) | o.follow("divisor"))
                >> net(map(|x: &Frame<f32, U2>| x[0] / x[1].max(0.01)))
                >> net(sine::<f32>());
            let carrier = (bent(o) | modulator | o.follow("depth"))
                >> net(map(|x: &Frame<f32, U3>| x[0] + x[1] * x[0] * x[2]))
                >> net(sine::<f32>());
            Note::envelope(carrier * 0.6, o)
        })
}

fn pluck() -> Synth {
    Synth::new("pluck")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::seconds("ring")
                .default(3.0)
                .range(0.0, f64::INFINITY)
                .doc("How many seconds the string takes to fall 60 dB."),
        )
        .option(
            Spec::float("coef")
                .default(0.3)
                .range(-0.99, 0.99)
                .doc("How fast the string loses its brightness."),
        )
        .options(Spec::envelope(0.0, 2.0))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let burst =
                white_noise(ctx.seed()) >> net(butterpass_hz((freq * 12.0).min(20_000.0) as f32));
            let string = burst
                >> net(An(KarplusStrong::new(
                    freq,
                    o.number("ring"),
                    o.number("coef"),
                )));
            Note::envelope((string >> net(dcblock_hz(30.0))) * 0.8, o)
        })
}

fn supersaw() -> Synth {
    wave("supersaw", 110.0, 1.0)
        .option(
            Spec::float("detune")
                .default(0.2)
                .doc("How far in semitones the saws spread around the note."),
        )
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, _| {
            let mut saws = Net::wrap(Box::new(zero()));
            for offset in [-1.0f32, -0.62, -0.3, 0.0, 0.3, 0.62, 1.0] {
                let note = o.follow("note") >> moved_by(o, "bend", |x| x);
                let freq = (note | o.follow("detune"))
                    >> net(map(move |x: &Frame<f32, U2>| {
                        hz((x[0] + offset * x[1]) as f64) as f32
                    }));
                saws = saws + (freq >> saw_wave());
            }
            Note::envelope(brightened(o, saws) * 0.18, o)
        })
}

fn acid() -> Synth {
    Synth::new("acid")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::note("cutoff")
                .default(70.0)
                .doc("The MIDI note the filter closes to."),
        )
        .option(
            Spec::float("res")
                .default(0.7)
                .range(0.0, 1.0)
                .doc("How much the filter rings at its cutoff."),
        )
        .options(Spec::envelope(0.0, 0.5))
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, _| {
            let spring = segments(&[30.0, 0.0], &[0.18], &[-4.0]);
            let cutoff = (o.follow("cutoff") | spring | o.follow("brightness"))
                >> net(map(|x: &Frame<f32, U3>| {
                    let moved = x[0] + x[1] + brighter(x[2]);
                    hz(moved as f64).clamp(20.0, 18_000.0) as f32
                }));
            let q = o.follow("res")
                >> net(map(|x: &Frame<f32, U1>| {
                    1.0 / (1.0 - x[0].clamp(0.0, 1.0) * 0.93)
                }));
            let half = bent(o) >> net(map(|x: &Frame<f32, U1>| x[0] * 0.5));
            let sound = (bent(o) >> saw_wave()) + ((half | net(dc(0.5))) >> pulse_wave()) * 0.25;
            let filtered = (sound | cutoff | q) >> net(lowpass::<f32>());
            let driven = filtered >> net(map(|x: &Frame<f32, U1>| (x[0] * 1.6).tanh() * 0.45));
            Note::envelope(driven, o)
        })
}

fn brass() -> Synth {
    Synth::new("brass")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard it is blown."),
        )
        .options(Spec::envelope(0.0, 0.25))
        .build(|o, ctx| {
            let vel = o.number("vel") as f32;
            let breath = || segments(&[0.0, 1.0, 0.8], &[0.05, 0.25], &[2.0, -2.0]);
            let scoop = segments(&[-0.35, 0.0], &[0.06], &[3.0]);
            let vibrato = (net(sine_hz::<f32>(5.3)) * 0.12)
                * segments(&[0.0, 0.0, 1.0], &[0.3, 0.5], &[0.0, 0.0]);
            let freq = || {
                (o.follow("note") | scoop.clone() | vibrato.clone())
                    >> net(map(|x: &Frame<f32, U3>| {
                        hz((x[0] + x[1] + x[2]) as f64) as f32
                    }))
            };
            let reed = (freq() >> saw_wave()) + ((freq() | net(dc(0.38))) >> pulse_wave()) * 0.5;
            let cutoff = (freq() | breath())
                >> net(map(move |x: &Frame<f32, U2>| {
                    (x[0] * (1.5 + x[1] * (2.0 + vel * 9.0))).clamp(20.0, 18_000.0)
                }));
            let horn = (reed | cutoff | net(dc(1.0 / 0.7))) >> net(lowpass::<f32>());
            let centre = freq() >> net(map(|x: &Frame<f32, U1>| (x[0] * 4.0).min(20_000.0)));
            // FunDSP's band-pass peaks at its Q of 2, so it is halved back to a peak of 1.
            let air = (white_noise(ctx.seed()) | centre | net(dc(2.0))) >> net(bandpass::<f32>());
            let air = air * 0.5 * 0.04 * breath();
            Note::envelope((horn * 0.4 + air) * (0.4 + vel * 0.6), o)
        })
}

/// A burst fed into a delay one period long for its first period, then circulating through a one-pole low-pass, falling 60 dB over `ring` seconds.
#[derive(Clone)]
struct KarplusStrong {
    freq: f64,
    ring: f64,
    coef: f32,
    buffer: Vec<f32>,
    at: usize,
    period: f32,
    feedback: f32,
    written: usize,
    smoothed: f32,
}

impl KarplusStrong {
    fn new(freq: f64, ring: f64, coef: f64) -> Self {
        let mut string = KarplusStrong {
            freq,
            ring,
            coef: coef.clamp(-0.99, 0.99) as f32,
            buffer: Vec::new(),
            at: 0,
            period: 0.0,
            feedback: 0.0,
            written: 0,
            smoothed: 0.0,
        };
        string.set_sample_rate(DEFAULT_SR);
        string
    }
}

impl AudioNode for KarplusStrong {
    const ID: u64 = 0x6d78_0020;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let period = sample_rate / self.freq.max(1.0);
        self.period = period as f32;
        self.buffer = vec![0.0; period.ceil() as usize + 4];
        self.feedback = if self.ring > 0.0 {
            0.001f64.powf(1.0 / (self.freq * self.ring)) as f32
        } else {
            0.0
        };
        (self.at, self.written, self.smoothed) = (0, 0, 0.0);
    }

    fn reset(&mut self) {
        self.set_sample_rate(DEFAULT_SR);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let n = self.buffer.len();
        self.at = (self.at + 1) % n;
        let whole = self.period.floor() as usize;
        let fraction = self.period - self.period.floor();
        let a = self.buffer[(self.at + n - whole) % n];
        let b = self.buffer[(self.at + n - whole - 1) % n];
        let delayed = a + (b - a) * fraction;
        self.smoothed = (1.0 - self.coef.abs()) * delayed + self.coef * self.smoothed;
        let excite = if (self.written as f32) < self.period {
            input[0]
        } else {
            0.0
        };
        self.written += 1;
        self.buffer[self.at] = excite + self.feedback * self.smoothed;
        [delayed].into()
    }
}
