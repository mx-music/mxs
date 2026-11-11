//! The winds' physical models: an air jet for the flute, a reed for the clarinet, oboe, bassoon and tenor sax, and the lips for the brass.
//! Each is a digital waveguide after Julius O. Smith III, "Physical Audio Signal Processing", https://ccrma.stanford.edu/~jos/pasp/, and Perry R. Cook, "A meta-wind-instrument physical model", ICMC 1992.
//! The bore is a delay line as long as the note's period, so a change of note carries the same column of air to the new pitch, and the breath's pressure follows vel while the note holds.
//! After the model come a peak at each band mx measures, its gain fitted to each instrument's recordings at pp, mf and ff and following vel between them.

use crate::nodes::{db, hz, shape, white_noise};
use crate::synth::SynthDecl;
use crate::{Context, Note, Options, Spec, Synth};
use fundsp::prelude::*;
use std::f32::consts::TAU;

/// The centres of the bands mx measures, from 80 Hz to 4 kHz.
const BANDS: [f32; 8] = [113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0];

const EQ: [&str; 8] = ["eq0", "eq1", "eq2", "eq3", "eq4", "eq5", "eq6", "eq7"];

pub fn all() -> Vec<Synth> {
    vec![
        flute(),
        reed_wind("clarinet", 5.0, Bore { conical: false, radiate: 1, corner: 8.0, keep: 0.95, air: 1500.0 }, &CLARINET),
        reed_wind("oboe", 5.0, Bore { conical: true, radiate: 2, corner: 8.0, keep: 0.95, air: 1500.0 }, &OBOE),
        reed_wind("bassoon", 5.5, Bore { conical: true, radiate: 1, corner: 8.0, keep: 0.95, air: 800.0 }, &BASSOON),
        reed_wind("tenor_sax", 4.94, Bore { conical: true, radiate: 1, corner: 8.0, keep: 0.95, air: 1500.0 }, &TENOR_SAX),
        reed_wind("trumpet", 4.9, Bore { conical: true, radiate: 2, corner: 8.0, keep: 0.95, air: 1500.0 }, &TRUMPET),
        reed_wind("trombone", 4.95, Bore { conical: true, radiate: 2, corner: 8.0, keep: 0.95, air: 1000.0 }, &TROMBONE),
        reed_wind("horn", 5.0, Bore { conical: true, radiate: 1, corner: 8.0, keep: 0.95, air: 800.0 }, &HORN),
        reed_wind("tuba", 5.0, Bore { conical: true, radiate: 1, corner: 8.0, keep: 0.95, air: 300.0 }, &TUBA),
    ]
}

/// A delay line read at a fraction of a sample by cubic Lagrange interpolation.
#[derive(Clone)]
struct Line {
    samples: Vec<f32>,
    at: usize,
}

impl Line {
    fn new(length: usize) -> Line {
        Line { samples: vec![0.0; length.next_power_of_two()], at: 0 }
    }

    fn write(&mut self, x: f32) {
        self.at = (self.at + 1) & (self.samples.len() - 1);
        self.samples[self.at] = x;
    }

    /// The sample written `delay` samples before the last, which must lie between 1 and the line's length less 3.
    fn read(&self, delay: f32) -> f32 {
        let mask = self.samples.len() - 1;
        let whole = delay.floor();
        let d = delay - whole;
        let back = |k: f32| self.samples[(self.at + self.samples.len() - k as usize) & mask];
        let (a, b, c, e) = (back(whole - 1.0), back(whole), back(whole + 1.0), back(whole + 2.0));
        -a * d * (d - 1.0) * (d - 2.0) / 6.0 + b * (d + 1.0) * (d - 1.0) * (d - 2.0) / 2.0 - c * (d + 1.0) * d * (d - 2.0) / 2.0 + e * (d + 1.0) * d * (d - 1.0) / 6.0
    }

    fn clear(&mut self) {
        self.samples.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// A one-pole low-pass, y = (1 - a) x + a y.
#[derive(Clone, Default)]
struct Loss {
    y: f32,
}

impl Loss {
    fn pole(corner: f32, rate: f32) -> f32 {
        (-TAU * corner / rate).exp()
    }

    fn tick(&mut self, x: f32, a: f32) -> f32 {
        self.y = (1.0 - a) * x + a * self.y;
        self.y
    }

    /// The samples it delays a sine at `w` radians per sample.
    fn lag(a: f32, w: f32) -> f32 {
        (a * w.sin()).atan2(1.0 - a * w.cos()) / w
    }
}

/// A one-pole, one-zero filter that takes out the steady part of a signal.
#[derive(Clone, Default)]
struct DcBlock {
    x: f32,
    y: f32,
}

impl DcBlock {
    fn tick(&mut self, x: f32) -> f32 {
        self.tick_at(x, 0.997)
    }

    fn tick_at(&mut self, x: f32, r: f32) -> f32 {
        self.y = x - self.x + r * self.y;
        self.x = x;
        self.y
    }
}

/// The flute's air jet: the breath crosses the embouchure and is swung across the edge by the bore's sound half a period before, flowing into the bore on one side or the other as a sigmoid of its swing.
/// Its inputs are the pitch in Hz, the breath's pressure, and white noise for the jet's turbulence.
/// A jet delayed half a period favours the odd harmonics, so the output takes a part of the square of the bore's sound for the even ones, as an edge the jet does not split evenly gives.
/// Its second output is the side of the edge the jet flows to, from 0 to 1, which its turbulence follows.
#[derive(Clone)]
pub(crate) struct Jet {
    bore: Line,
    jet: Line,
    loss: Loss,
    drive: DcBlock,
    out: DcBlock,
    /// The bore's corner as a multiple of the pitch.
    corner: f32,
    /// How much of the bore's sound comes back from the open end.
    reflection: f32,
    /// How much of the jet's swing is turbulence.
    turbulence: f32,
    /// The part of the square of the sound in the output.
    even: f32,
    /// How much the breath drifts as a player's does, 1 being about half a dB and two cents, and its slow noise, filtered twice.
    pub(crate) wander: f32,
    drift: [f32; 2],
    /// How far the breath falls while a slur moves the pitch, from 0 to 1, and the fall as it follows.
    pub(crate) catch: f32,
    fall: f32,
    last: f32,
    rate: f32,
}

impl Jet {
    pub(crate) fn new(corner: f32, turbulence: f32, even: f32) -> Jet {
        Jet { bore: Line::new(4096), jet: Line::new(4096), loss: Loss::default(), drive: DcBlock::default(), out: DcBlock::default(), corner, reflection: 0.5, turbulence, even, wander: 0.0, drift: [0.0; 2], catch: 0.0, fall: 0.0, last: 0.0, rate: DEFAULT_SR as f32 }
    }
}

impl AudioNode for Jet {
    const ID: u64 = 0x6d78_0060;
    type Inputs = U3;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.bore.clear();
        self.jet.clear();
        self.loss = Loss::default();
        self.drive = DcBlock::default();
        self.out = DcBlock::default();
        self.drift = [0.0; 2];
        self.fall = 0.0;
        self.last = 0.0;
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U2> {
        let noise = input[2];
        // The noise through two low-passes at 4 Hz, scaled so a wander of 1 sways the level about half a dB and the pitch about two cents, as the Philharmonia's flute does.
        let slow = Loss::pole(4.0, self.rate);
        self.drift[0] = (1.0 - slow) * noise + slow * self.drift[0];
        self.drift[1] = (1.0 - slow) * self.drift[0] + slow * self.drift[1];
        let drift = self.drift[1] * 250.0 * self.wander;
        // A slur moving faster than an octave a second, faster than any vibrato, lets the breath fall, and it comes back over about 20 ms.
        let octave = input[0].max(20.0).log2();
        let moving = if self.last == 0.0 { 0.0 } else { (((octave - self.last).abs() * self.rate - 1.0) / 2.0).clamp(0.0, 1.0) };
        self.last = octave;
        self.fall = if moving > self.fall { moving } else { Loss::pole(8.0, self.rate) * self.fall };
        let freq = (input[0] * 2f32.powf(drift * 6.0 / 1200.0)).clamp(20.0, 5000.0);
        let breath = input[1].max(0.0) * (1.0 + 0.035 * drift) * (1.0 - self.catch * self.fall);
        let period = self.rate / freq;
        let w = TAU * freq / self.rate;
        let a = Loss::pole(self.corner * freq, self.rate);
        let u = self.loss.y;
        self.jet.write(self.drive.tick(u) + self.turbulence * noise);
        let swing = self.jet.read(0.5 * period);
        let side = (1.5 * swing).tanh();
        let flow = -breath * side;
        self.bore.write(self.reflection * u + flow);
        // The loop delays by the line, the low-pass, and the sample before the low-pass is read again.
        let b = self.bore.read((period - 1.0 - Loss::lag(a, w)).max(2.0));
        self.loss.tick(b, a);
        [self.out.tick(b + self.even * b * b), 0.5 + 0.5 * side].into()
    }
}

/// A reed, or the lips, as a valve the pressure across it opens and closes, at the end of a bore.
/// The clarinet's bore is a cylinder, closed at the reed and open at the bell, whose sound comes back inverted after half a period, so it favours the odd harmonics.
/// A conical bore, as the oboe's, bassoon's and saxophone's, and the brass's, whose bell and mouthpiece bring their resonances to whole multiples, comes back upright after a whole period, with every harmonic.
/// The valve follows Julius O. Smith III's reed table, https://ccrma.stanford.edu/~jos/pasp/Single_Reed_Instruments.html: the pressure wave it lets into the bore is the half mouth pressure less the part of the difference across it that the valve reflects.
/// Its inputs are the pitch in Hz, the mouth's pressure, and white noise for the breath's turbulence.
#[derive(Clone)]
pub(crate) struct Reed {
    bore: Line,
    loss: Loss,
    steady: DcBlock,
    out: DcBlock,
    conical: bool,
    /// How much of the difference the valve reflects when there is none, and how fast that grows with the difference.
    offset: f32,
    slope: f32,
    /// The bore's corner as a multiple of the pitch, and its loss at each round trip.
    corner: f32,
    keep: f32,
    turbulence: f32,
    /// How many times the sound in the bore is differenced as it radiates from the bell, each rising 6 dB an octave.
    radiate: usize,
    radiated: [f32; 2],
    rate: f32,
}

impl Reed {
    pub(crate) fn new(conical: bool, corner: f32, keep: f32, radiate: usize) -> Reed {
        Reed { bore: Line::new(8192), loss: Loss::default(), steady: DcBlock::default(), out: DcBlock::default(), conical, offset: 0.7, slope: 0.3, corner, keep, turbulence: 0.005, radiate, radiated: [0.0; 2], rate: DEFAULT_SR as f32 }
    }
}

impl AudioNode for Reed {
    const ID: u64 = 0x6d78_0062;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.bore.clear();
        self.loss = Loss::default();
        self.steady = DcBlock::default();
        self.out = DcBlock::default();
        self.radiated = [0.0; 2];
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let freq = input[0].clamp(20.0, 5000.0);
        let mouth = input[1].max(0.0) * (1.0 + self.turbulence * input[2]);
        let period = self.rate / freq;
        let w = TAU * freq / self.rate;
        let a = Loss::pole(self.corner * freq, self.rate);
        let back = if self.conical { self.loss.y } else { -self.loss.y };
        let across = mouth / 2.0 - back;
        let reflects = (self.offset + self.slope * across).clamp(-1.0, 1.0);
        let into = mouth / 2.0 - reflects * across;
        self.bore.write(into);
        let b = if self.conical {
            // An upright return would hold the mouth's steady pressure, so it is taken out below an eighth of the pitch, and the line lengthened by the lead that gives.
            let r = Loss::pole(freq / 8.0, self.rate);
            let lead = ((std::f32::consts::PI - w) / 2.0 - (r * w.sin()).atan2(1.0 - r * w.cos())) / w;
            let b = self.bore.read((period - 1.0 - Loss::lag(a, w) + lead).max(2.0));
            self.steady.tick_at(b, r)
        } else {
            self.bore.read((period / 2.0 - 1.0 - Loss::lag(a, w)).max(2.0))
        };
        self.loss.tick(b * self.keep, a);
        let mut sound = self.out.tick(into + back);
        for last in self.radiated.iter_mut().take(self.radiate) {
            (sound, *last) = (sound - *last, sound);
        }
        [sound].into()
    }
}

/// Peaks at each band mx measures, their gains following vel between the ones fitted at pp, mf and ff, or held at those given for fitting, then the level for vel.
/// Its inputs are the sound and vel.
#[derive(Clone)]
struct Bands {
    gains: [[f32; 8]; 3],
    given: [Option<f32>; 8],
    levels: [f32; 3],
    vel: f32,
    level: f32,
    coefficients: [[f32; 5]; 8],
    state: [[f32; 4]; 8],
    rate: f32,
}

/// A value at `vel` between its values at pp, mf and ff, played at vel 0.3, 0.6 and 1.
fn by_vel(vel: f32, at: [f32; 3]) -> f32 {
    at[0] + (at[1] - at[0]) * ((vel - 0.3) / 0.3).clamp(0.0, 1.0) + (at[2] - at[1]) * ((vel - 0.6) / 0.4).clamp(0.0, 1.0)
}

impl Bands {
    fn new(gains: [[f32; 8]; 3], given: [Option<f32>; 8], levels: [f32; 3]) -> Bands {
        Bands { gains, given, levels, vel: -1.0, level: 0.0, coefficients: [[0.0; 5]; 8], state: [[0.0; 4]; 8], rate: DEFAULT_SR as f32 }
    }

    fn tune(&mut self, vel: f32) {
        self.vel = vel;
        // Below pp the breath fades the sound towards silence.
        self.level = db(by_vel(vel, self.levels) as f64) as f32 * (vel / 0.3).min(1.0);
        for (band, centre) in BANDS.into_iter().enumerate() {
            let gain = self.given[band].unwrap_or_else(|| by_vel(vel, [self.gains[0][band], self.gains[1][band], self.gains[2][band]]));
            // A peaking filter after Robert Bristow-Johnson's "Cookbook formulae for audio EQ biquad filter coefficients".
            let a = 10f32.powf(gain / 40.0);
            let w = TAU * centre / self.rate;
            let alpha = w.sin() / (2.0 * 1.4);
            let a0 = 1.0 + alpha / a;
            self.coefficients[band] = [(1.0 + alpha * a) / a0, -2.0 * w.cos() / a0, (1.0 - alpha * a) / a0, -2.0 * w.cos() / a0, (1.0 - alpha / a) / a0];
        }
    }
}

impl AudioNode for Bands {
    const ID: u64 = 0x6d78_0061;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.vel = -1.0;
    }

    fn reset(&mut self) {
        self.state = [[0.0; 4]; 8];
        self.vel = -1.0;
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        let vel = input[1].clamp(0.0, 1.0);
        if (vel - self.vel).abs() > 1e-4 {
            self.tune(vel);
        }
        let mut x = input[0];
        for ([b0, b1, b2, a1, a2], [x1, x2, y1, y2]) in self.coefficients.iter().zip(self.state.iter_mut()) {
            let y = *b0 * x + *b1 * *x1 + *b2 * *x2 - *a1 * *y1 - *a2 * *y2;
            (*x2, *x1, *y2, *y1) = (*x1, x, *y1, y);
            x = y;
        }
        [x * self.level].into()
    }
}

/// An instrument's figures, fitted to its recordings at pp, mf and ff.
pub(crate) struct Fit {
    /// The gain in dB at each band.
    pub gains: [[f32; 8]; 3],
    /// The whole sound's level in dB.
    pub levels: [f32; 3],
    /// The breath's pressure.
    pub breath: [f32; 3],
    /// The seconds the breath takes to reach its pressure from silence.
    pub rise: [f32; 3],
    /// The breath's noise in dB, against the sound of the model.
    pub air: [f32; 3],
}

/// A wind's options: those every wind takes, then for fitting each band's gain, the level, the breath and its rise, in place of those fitted for the note's vel.
fn wind_options(name: &str, vibrato: f64) -> SynthDecl {
    let mut synth = Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch; a change while the note holds slurs to the new one."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is blown; a change while the note holds swells or fades it."))
        .option(Spec::float("vibrato").default(vibrato).range(0.0, 2.0).doc("The vibrato's depth in semitones, reached as the note holds."))
        .options(Spec::envelope(0.5, 0.2))
        .option(Spec::float("breath").range(0.0, 3.0).doc("The breath's pressure, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::seconds("rise").range(0.001, 2.0).doc("The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::db("air").range(-90.0, 0.0).doc("The breath's noise in dB, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::db("level").range(-60.0, 60.0).doc("The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting."));
    for (name, hz) in EQ.iter().zip(BANDS) {
        let doc = format!("The gain in dB of the band at {hz} Hz, in place of the one fitted for the note's vel; for fitting.");
        synth = synth.option(Spec::db(name).range(-60.0, 60.0).doc(&doc));
    }
    synth
}

/// The note's frequency, gliding with its note option, swung by a vibrato `depth` semitones deep at `rate` that waits a quarter second, then grows in over 1.2.
fn pitch(o: &Options, depth: f64, rate: f64) -> Net {
    let swing = shape(move |t| {
        let grown = ((t - 0.25) / 1.2).clamp(0.0, 1.0);
        (std::f64::consts::TAU * rate * t).sin() * depth * grown * grown
    });
    (o.follow("note") | swing) >> Net::wrap(Box::new(map(|x: &Frame<f32, U2>| hz(x[0] as f64 + x[1] as f64) as f32)))
}

/// The breath's pressure following vel, rising from silence over its fitted rise at the vel the note starts at.
fn breath(o: &Options, fit: &Fit) -> Net {
    let vel = o.number("vel").clamp(0.0, 1.0) as f32;
    let rise = o.given("rise").unwrap_or_else(|| by_vel(vel, fit.rise) as f64).max(1e-3);
    let at = fit.breath;
    let given = o.given("breath").map(|b| b as f32);
    let pressure = o.follow("vel") >> Net::wrap(Box::new(map(move |v: &Frame<f32, U1>| given.unwrap_or_else(|| by_vel(v[0].clamp(0.0, 1.0), at)))));
    pressure * shape(move |t| 1.0 - (1.0 - (t / rise).min(1.0)).powi(2))
}

/// The model's sound with the breath's noise around `centre` Hz, through the fitted bands, at the fitted level for vel.
fn voiced(o: &Options, ctx: &mut Context, model: Net, fit: &Fit, centre: f32) -> Net {
    let given = EQ.map(|name| o.given(name).map(|g| g as f32));
    let levels = o.given("level").map_or(fit.levels, |l| [l as f32; 3]);
    let airs = o.given("air").map_or(fit.air, |a| [a as f32; 3]);
    let air = o.follow("vel") >> Net::wrap(Box::new(map(move |v: &Frame<f32, U1>| db(by_vel(v[0].clamp(0.0, 1.0), airs) as f64) as f32)));
    let noise = (white_noise(ctx.seed()) >> Net::wrap(Box::new(bandpass_hz(centre, 1.0)))) * air;
    (model + noise | o.follow("vel")) >> Net::wrap(Box::new(An(Bands::new(fit.gains, given, levels))))
}

/// The flute's air jet as a defsynth plays it, its turbulence drawn from `seed`.
pub(crate) fn air_jet(pitch: Net, breath: Net, seed: u64, even: f64) -> Net {
    played_jet(pitch, breath, seed, even, 0.0, 0.0, 6.0) >> Net::wrap(Box::new(pass() | sink()))
}

/// The air jet with the breath's `wander` and its `catch` at each slur, its sound and the side the jet flows to.
fn played_jet(pitch: Net, breath: Net, seed: u64, even: f64, wander: f64, catch: f64, corner: f64) -> Net {
    let mut jet = Jet::new(corner as f32, 0.02, even as f32);
    (jet.wander, jet.catch) = (wander as f32, catch as f32);
    (pitch | breath | white_noise(seed)) >> Net::wrap(Box::new(An(jet)))
}

/// The bands and level as a defsynth plays them, following `vel`: `gains` at pp, mf and ff, eight to each, and `levels` at each.
pub(crate) fn wind_bands(sound: Net, vel: Net, gains: &[f64], levels: &[f64]) -> Net {
    let gain = |d: usize| std::array::from_fn(|band| gains[d * 8 + band] as f32);
    let bands = Bands::new([gain(0), gain(1), gain(2)], [None; 8], [levels[0] as f32, levels[1] as f32, levels[2] as f32]);
    (sound | vel) >> Net::wrap(Box::new(An(bands)))
}

/// The part of the square of the jet's sound in the flute's output, at pp, mf and ff.
const FLUTE_EVEN: [f32; 3] = [1.12, 0.8, 0.571];

const FLUTE: Fit = Fit { gains: [[0.0; 8]; 3], levels: [-12.0, -12.0, -12.0], breath: [0.38, 0.45, 0.56], rise: [0.05, 0.05, 0.05], air: [-90.0, -90.0, -90.0] };

fn flute() -> Synth {
    wind_options("flute", 0.25)
        .option(Spec::float("wander").default(1.0).range(0.0, 4.0).doc("How much the breath drifts while a note holds, as a player's does; 0 holds it still."))
        .option(Spec::float("catch").default(0.25).range(0.0, 1.0).doc("How far the breath falls while a slur moves the pitch; 0 glides smoothly."))
        .option(Spec::float("corner").range(2.0, 40.0).doc("The bore's corner as a multiple of the pitch, in place of the one fitted; for fitting."))
        .option(Spec::db("shelf").range(-30.0, 30.0).doc("The gain in dB above 4 kHz, in place of the one fitted; for fitting."))
        .option(Spec::db("hiss").range(-90.0, 0.0).doc("The breath's hiss above 3 kHz in dB, in place of the one fitted; for fitting."))
        .option(Spec::float("pulse").range(0.0, 1.0).doc("How much the hiss follows the jet across the edge, from 0 steady to 1 only on one side, in place of the one fitted; for fitting."))
        .option(Spec::float("even").range(0.0, 4.0).doc("The part of the square of the jet's sound in the output, in place of the one fitted; for fitting."))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let even = o.given("even").unwrap_or_else(|| by_vel(vel as f32, FLUTE_EVEN) as f64);
            let jet = played_jet(pitch(o, o.number("vibrato") * vel.powf(0.65), 5.0), breath(o, &FLUTE), ctx.seed(), even, o.number("wander"), o.number("catch"), o.given("corner").unwrap_or(6.0));
            // The breath hisses above 3 kHz where the jet strikes the edge, pulsing as it crosses, after the bands, so they do not colour it.
            let pulse = o.given("pulse").unwrap_or(1.0) as f32;
            let side = Net::wrap(Box::new(map(move |s: &Frame<f32, U1>| 1.0 - pulse + 2.0 * pulse * s[0])));
            let hiss = side * (white_noise(ctx.seed()) >> Net::wrap(Box::new(highpass_hz(3000.0, 0.707) >> lowpass_hz(10_000.0, 0.707)))) * breath(o, &FLUTE) * db(o.given("hiss").unwrap_or(-14.0)) as f32;
            // mx's bands stop at 4 kHz, and above them the jet falls away faster than Iowa's flute, 12 dB from 5 to 10 kHz at C5 and C6, which a shelf gives back.
            let bright = Net::wrap(Box::new(highshelf_hz(4000.0, 0.707, db(o.given("shelf").unwrap_or(12.0)) as f32)));
            let tone = voiced(o, ctx, Net::wrap(Box::new(pass())), &FLUTE, 2000.0) >> bright;
            Note::envelope(jet >> (tone | hiss) >> Net::wrap(Box::new(join::<U2>())), o)
        })
}

/// A reed or brass wind's bore: whether it is conical, how many times its sound is differenced as it radiates, its corner as a multiple of the pitch, how much it keeps at each round trip, and the centre in Hz of the breath's noise.
struct Bore {
    conical: bool,
    radiate: usize,
    corner: f32,
    keep: f32,
    air: f32,
}

const UNFITTED: Fit = Fit { gains: [[0.0; 8]; 3], levels: [-12.0; 3], breath: [1.4, 1.7, 2.0], rise: [0.05; 3], air: [-40.0; 3] };
const CLARINET: Fit = UNFITTED;
const OBOE: Fit = UNFITTED;
const BASSOON: Fit = UNFITTED;
const TENOR_SAX: Fit = UNFITTED;
const TRUMPET: Fit = UNFITTED;
const TROMBONE: Fit = UNFITTED;
const HORN: Fit = UNFITTED;
const TUBA: Fit = UNFITTED;

fn reed_wind(name: &'static str, rate: f64, bore: Bore, fit: &'static Fit) -> Synth {
    wind_options(name, 0.0).build(move |o, ctx| {
        let reed = Reed::new(bore.conical, bore.corner, bore.keep, bore.radiate);
        let model = (pitch(o, o.number("vibrato"), rate) | breath(o, fit) | white_noise(ctx.seed())) >> Net::wrap(Box::new(An(reed)));
        Note::envelope(voiced(o, ctx, model, fit, bore.air), o)
    })
}
