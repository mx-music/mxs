//! The nodes the crate's synths are made of, for a synth of your own to use.
//! Each is a FunDSP unit, so they combine with FunDSP's own through its operators.
//! [The guide to building a synth](crate::guide::building) groups them by what they are for.

use fundsp::prelude::*;

/// The frequency of a MIDI note.
pub fn hz(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

/// The gain of a level in decibels.
pub fn db(decibels: f64) -> f64 {
    10f64.powf(decibels / 20.0)
}

/// A resonator set ringing by its input: an impulse starts it at `level` from `phase`, and it fades 60 dB over `fade` seconds.
pub fn mode(hz: f64, phase: f64, level: f64, fade: f64) -> Net {
    Net::wrap(Box::new(An(Mode::new(hz, phase, level, fade))))
}

/// A resonator that answers as those of scsynth, SuperCollider's server, do, measured from their renders: an impulse rings it at 2 cos(w / 2) times `level`, half a sample late, w being its frequency in radians per sample, and it fades 60 dB over `fade` seconds.
pub fn struck(hz: f64, level: f64, fade: f64) -> Net {
    let mut resonator = Mode::new(hz, 0.0, level, fade);
    resonator.like_scsynth = true;
    resonator.set_sample_rate(DEFAULT_SR);
    Net::wrap(Box::new(An(resonator)))
}

/// [`struck`], at the frequency its second input gives at each sample, so it glides as the note does.
pub fn struck_gliding(level: f64, fade: f64) -> Net {
    Net::wrap(Box::new(An(Gliding {
        level,
        fade,
        rate: DEFAULT_SR,
        hz: -1.0,
        coefficients: (0.0, 0.0, 0.0, 0.0),
        x1: 0.0,
        y1: 0.0,
        y2: 0.0,
    })))
}

/// An impulse at each of `clicks`, given as seconds and level in the order they come.
pub fn clicks(clicks: Vec<(f64, f64)>) -> Net {
    Net::wrap(Box::new(An(Clicks {
        clicks,
        next: 0,
        t: 0.0,
        dt: 1.0 / DEFAULT_SR,
    })))
}

/// A level worked out from the seconds since the note started, at every sample.
pub fn shape<F>(f: F) -> Net
where
    F: Fn(f64) -> f64 + Clone + Send + Sync + 'static,
{
    Net::wrap(Box::new(An(Shape {
        f,
        t: 0.0,
        dt: 1.0 / DEFAULT_SR,
    })))
}

/// [`shape`](fn@shape) as a node rather than a `Net`, for inside a `feedback` loop; unlike FunDSP's `envelope`, it does not depend on where it sits in a graph.
pub fn shape_node<F>(f: F) -> An<impl AudioNode<Inputs = U0, Outputs = U1>>
where
    F: Fn(f64) -> f64 + Clone + Send + Sync + 'static,
{
    An(Shape {
        f,
        t: 0.0,
        dt: 1.0 / DEFAULT_SR,
    })
}

/// A tone of `count` harmonics at the frequency its input gives, whose levels move as it sounds.
/// `level` gives a harmonic's gain from its number, its frequency in Hz and the seconds since the note started; it is asked every 32 samples, and the gain moves in a straight line between.
pub fn harmonic_tone<F>(count: usize, level: F) -> Net
where
    F: Fn(usize, f64, f64) -> f64 + Clone + Send + Sync + 'static,
{
    Net::wrap(Box::new(An(HarmonicTone {
        level,
        gains: vec![(0.0, 0.0); count],
        phase: 0.0,
        at: 0,
        dt: 1.0 / DEFAULT_SR,
    })))
}

/// A percussive level at `t` seconds: a straight rise over `attack`, then a fall to silence over `release`.
pub fn perc(t: f64, attack: f64, release: f64) -> f64 {
    if t < attack {
        t / attack
    } else {
        (1.0 - (t - attack) / release).max(0.0).powi(4)
    }
}

/// A level moving from each of `levels` to the next over each of `times` seconds, holding the last.
/// Each segment's `curves` value bends it: 0 is a straight line, above 0 it starts slowly and below 0 quickly, as e^(curve x) does.
pub fn segments(levels: &[f64], times: &[f64], curves: &[f64]) -> Net {
    let (levels, times, curves) = (levels.to_vec(), times.to_vec(), curves.to_vec());
    shape(move |t| segment_at(&levels, &times, &curves, t))
}

/// A level moving through `levels` over `times` as [`segments`] does, until its input falls to 0: from then it falls from where it is to silence over the last of `times`, along the last of `curves`.
pub fn gated(levels: &[f64], times: &[f64], curves: &[f64]) -> Net {
    let (levels, times, curves) = (levels.to_vec(), times.to_vec(), curves.to_vec());
    Net::wrap(Box::new(An(Gated {
        levels,
        times,
        curves,
        t: 0.0,
        dt: 1.0 / DEFAULT_SR,
        let_go: None,
    })))
}

/// The level of [`segments`] at `t` seconds.
pub fn segment_at(levels: &[f64], times: &[f64], curves: &[f64], t: f64) -> f64 {
    let mut start = 0.0;
    for (k, &time) in times.iter().enumerate() {
        if t < start + time {
            let x = (t - start) / time;
            let curve = curves.get(k).copied().unwrap_or(0.0);
            let bent = if curve.abs() < 1e-3 {
                x
            } else {
                (1.0 - (curve * x).exp()) / (1.0 - curve.exp())
            };
            return levels[k] + (levels[k + 1] - levels[k]) * bent;
        }
        start += time;
    }
    levels[Ord::min(times.len(), levels.len() - 1)]
}

/// A level moving from each of `levels` to the next over each of `times` seconds along an exponential curve, holding the last; no level may be 0.
pub fn exp_segments(levels: &[f64], times: &[f64]) -> Net {
    let (levels, times) = (levels.to_vec(), times.to_vec());
    shape(move |t| {
        let mut start = 0.0;
        for (k, &time) in times.iter().enumerate() {
            if t < start + time {
                let x = (t - start) / time;
                return levels[k] * (levels[k + 1] / levels[k]).powf(x);
            }
            start += time;
        }
        levels[Ord::min(times.len(), levels.len() - 1)]
    })
}

/// A percussive level: a rise over `attack` and a fall over `decay` seconds, each bent by `curve` as in [`segments`].
pub fn hit(attack: f64, decay: f64, curve: f64) -> Net {
    segments(&[0.0, 1.0, 0.0], &[attack, decay], &[curve, curve])
}

/// Resonators set ringing by `exciter`, one at each of `hz`, with its level and the seconds it takes to fade 60 dB; those above 20 kHz are left out.
/// Each is [`struck`].
pub fn modes(exciter: Net, hz: &[f64], levels: &[f64], rings: &[f64]) -> Net {
    let mut bank: Option<Net> = None;
    for ((&f, &level), &ring) in hz.iter().zip(levels).zip(rings) {
        if f >= 20_000.0 || f <= 0.0 || ring <= 0.0 {
            continue;
        }
        let one = struck(f, level, ring);
        bank = Some(match bank {
            Some(bank) => bank & one,
            None => one,
        });
    }
    match bank {
        Some(bank) => exciter >> bank,
        None => exciter >> Net::wrap(Box::new(sink() | zero())),
    }
}

/// A level read from a table given at each of `bands`, in a straight line between the two bands around `x`, and held beyond the ends.
pub fn between(x: f64, bands: &[f64], table: &[f64]) -> f64 {
    let x = x.clamp(bands[0], bands[bands.len() - 1]);
    let k = bands
        .windows(2)
        .position(|w| x <= w[1])
        .unwrap_or(bands.len() - 2);
    let along = (x - bands[k]) / (bands[k + 1] - bands[k]);
    table[k] + (table[k + 1] - table[k]) * along
}

/// Numbers drawn from a seed, the same on every run: SplitMix64.
pub struct Draws(u64);

impl Draws {
    /// Draws that start from `seed`.
    pub fn new(seed: u64) -> Self {
        Draws(seed)
    }

    /// A number from 0 up to 1.
    pub fn draw(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as f64 / u64::MAX as f64
    }

    /// A number from `low` to `high`, as likely in each octave.
    pub fn exp(&mut self, low: f64, high: f64) -> f64 {
        low * (high / low).powf(self.draw())
    }
}

/// White noise drawn from `seed`.
pub fn white_noise(seed: u64) -> Net {
    Net::wrap(Box::new(white().seed(seed)))
}

/// Pink noise drawn from `seed`, as loud in each band as the pink noise of scsynth, SuperCollider's server, which the crate's synths were designed against; FunDSP's measures 4.3 dB louder from 100 Hz to 8 kHz.
pub fn pink_noise(seed: u64) -> Net {
    Net::wrap(Box::new(pink::<f64>().seed(seed))) * db(-4.3) as f32
}

#[derive(Clone)]
struct Mode {
    hz: f64,
    phase: f64,
    level: f64,
    fade: f64,
    like_scsynth: bool,
    coefficients: (f64, f64, f64, f64),
    x1: f64,
    y1: f64,
    y2: f64,
}

impl Mode {
    fn new(hz: f64, phase: f64, level: f64, fade: f64) -> Self {
        let mut mode = Mode {
            hz,
            phase,
            level,
            fade,
            like_scsynth: false,
            coefficients: (0.0, 0.0, 0.0, 0.0),
            x1: 0.0,
            y1: 0.0,
            y2: 0.0,
        };
        mode.set_sample_rate(DEFAULT_SR);
        mode
    }
}

impl AudioNode for Mode {
    const ID: u64 = 0x6d78_0002;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        (self.x1, self.y1, self.y2) = (0.0, 0.0, 0.0);
    }

    fn set_sample_rate(&mut self, rate: f64) {
        let w = 2.0 * std::f64::consts::PI * self.hz / rate;
        let r = 1000f64.powf(-1.0 / (self.fade * rate));
        let (level, phase) = if self.like_scsynth {
            (
                self.level * 2.0 * (w / 2.0).cos(),
                std::f64::consts::FRAC_PI_2 - w / 2.0,
            )
        } else {
            (self.level, self.phase)
        };
        // Its impulse response is level * r^n * sin((n + 1) * w + phase).
        self.coefficients = (
            -2.0 * r * w.cos(),
            r * r,
            level * (w + phase).sin(),
            -level * r * phase.sin(),
        );
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let (a1, a2, b0, b1) = self.coefficients;
        let x = input[0] as f64;
        let y = b0 * x + b1 * self.x1 - a1 * self.y1 - a2 * self.y2;
        (self.x1, self.y2, self.y1) = (x, self.y1, y);
        [y as f32].into()
    }
}

/// The response of [`struck`], written out: level (1 + cos w)(1 - r z^-1) / (1 - 2 r cos w z^-1 + r^2 z^-2).
#[derive(Clone)]
struct Gliding {
    level: f64,
    fade: f64,
    rate: f64,
    hz: f64,
    coefficients: (f64, f64, f64, f64),
    x1: f64,
    y1: f64,
    y2: f64,
}

impl AudioNode for Gliding {
    const ID: u64 = 0x6d78_0004;
    type Inputs = U2;
    type Outputs = U1;

    fn reset(&mut self) {
        (self.x1, self.y1, self.y2, self.hz) = (0.0, 0.0, 0.0, -1.0);
    }

    fn set_sample_rate(&mut self, rate: f64) {
        (self.rate, self.hz) = (rate, -1.0);
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        let hz = (input[1] as f64).clamp(1.0, self.rate * 0.49);
        if hz != self.hz {
            let w = 2.0 * std::f64::consts::PI * hz / self.rate;
            let r = 1000f64.powf(-1.0 / (self.fade * self.rate));
            let gain = self.level * (1.0 + w.cos());
            self.coefficients = (-2.0 * r * w.cos(), r * r, gain, -gain * r);
            self.hz = hz;
        }
        let (a1, a2, b0, b1) = self.coefficients;
        let x = input[0] as f64;
        let y = b0 * x + b1 * self.x1 - a1 * self.y1 - a2 * self.y2;
        (self.x1, self.y2, self.y1) = (x, self.y1, y);
        [y as f32].into()
    }
}

#[derive(Clone)]
struct Clicks {
    clicks: Vec<(f64, f64)>,
    next: usize,
    t: f64,
    dt: f64,
}

impl AudioNode for Clicks {
    const ID: u64 = 0x6d78_0005;
    type Inputs = U0;
    type Outputs = U1;

    fn reset(&mut self) {
        (self.next, self.t) = (0, 0.0);
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.dt = 1.0 / rate;
    }

    fn tick(&mut self, _input: &Frame<f32, U0>) -> Frame<f32, U1> {
        let mut level = 0.0;
        // A click sounds at the sample nearest its time.
        while let Some(&(at, click)) = self.clicks.get(self.next) {
            if at >= self.t + self.dt / 2.0 {
                break;
            }
            level += click;
            self.next += 1;
        }
        self.t += self.dt;
        [level as f32].into()
    }
}

#[derive(Clone)]
struct Shape<F> {
    f: F,
    t: f64,
    dt: f64,
}

impl<F> AudioNode for Shape<F>
where
    F: Fn(f64) -> f64 + Clone + Send + Sync + 'static,
{
    const ID: u64 = 0x6d78_0003;
    type Inputs = U0;
    type Outputs = U1;

    fn reset(&mut self) {
        self.t = 0.0;
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.dt = 1.0 / rate;
    }

    fn tick(&mut self, _input: &Frame<f32, U0>) -> Frame<f32, U1> {
        let level = (self.f)(self.t);
        self.t += self.dt;
        [level as f32].into()
    }
}

#[derive(Clone)]
struct HarmonicTone<F> {
    level: F,
    /// Each harmonic's gain and what it moves by at each sample.
    gains: Vec<(f64, f64)>,
    phase: f64,
    at: u64,
    dt: f64,
}

impl<F> AudioNode for HarmonicTone<F>
where
    F: Fn(usize, f64, f64) -> f64 + Clone + Send + Sync + 'static,
{
    const ID: u64 = 0x6d78_0008;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        (self.phase, self.at) = (0.0, 0);
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.dt = 1.0 / rate;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let hz = input[0] as f64;
        if self.at.is_multiple_of(32) {
            let (t, top) = (self.at as f64 * self.dt, 0.45 / self.dt);
            for (k, (gain, step)) in self.gains.iter_mut().enumerate() {
                let f = (k + 1) as f64 * hz;
                let level = |t| {
                    if f < top {
                        (self.level)(k + 1, f, t)
                    } else {
                        0.0
                    }
                };
                if self.at == 0 {
                    *gain = level(t);
                }
                *step = (level(t + 32.0 * self.dt) - *gain) / 32.0;
            }
        }
        self.at += 1;
        // sin(k x) from the two before it: sin(k x) = 2 cos(x) sin((k - 1) x) - sin((k - 2) x).
        let (sin, cos) = (std::f64::consts::TAU * self.phase).sin_cos();
        let (mut before, mut last, mut sum) = (0.0, sin, 0.0);
        for (gain, step) in self.gains.iter_mut() {
            sum += *gain * last;
            *gain += *step;
            (before, last) = (last, 2.0 * cos * last - before);
        }
        self.phase = (self.phase + hz * self.dt).fract();
        [sum as f32].into()
    }
}

#[derive(Clone)]
struct Gated {
    levels: Vec<f64>,
    times: Vec<f64>,
    curves: Vec<f64>,
    t: f64,
    dt: f64,
    /// The seconds at which the gate fell, and the level then.
    let_go: Option<(f64, f64)>,
}

impl AudioNode for Gated {
    const ID: u64 = 0x6d78_0030;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        (self.t, self.let_go) = (0.0, None);
    }

    fn set_sample_rate(&mut self, rate: f64) {
        self.dt = 1.0 / rate;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        if self.let_go.is_none() && input[0] <= 0.0 {
            self.let_go = Some((
                self.t,
                segment_at(&self.levels, &self.times, &self.curves, self.t),
            ));
        }
        let level = match self.let_go {
            None => segment_at(&self.levels, &self.times, &self.curves, self.t),
            Some((at, from)) => {
                let release = self.times.last().copied().unwrap_or(0.0);
                let curve = self.curves.last().copied().unwrap_or(0.0);
                segment_at(&[from, 0.0], &[release], &[curve], self.t - at)
            }
        };
        self.t += self.dt;
        [level as f32].into()
    }
}

/// Its input silenced for 64 samples and then let in over the next 64, as scsynth starts a note under its control-rate envelope.
pub fn onset() -> Net {
    Net::wrap(Box::new(An(Onset { at: 0 })))
}

#[derive(Clone)]
struct Onset {
    at: usize,
}

impl AudioNode for Onset {
    const ID: u64 = 0x6d78_0032;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        self.at = 0;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let level = (self.at.saturating_sub(64) as f32 / 64.0).min(1.0);
        self.at += 1;
        [input[0] * level].into()
    }
}

/// Impulses at random times, as many a second on average as its input, each of a random level from 0 to 1, drawn from `seed`.
pub fn dust(seed: u64) -> Net {
    Net::wrap(Box::new(An(Dust {
        seed,
        state: seed,
        rate: DEFAULT_SR,
    })))
}

/// Each impulse of its input rising over `attack` and falling over `decay` seconds, both 60 dB times.
pub fn decay2(attack: f64, decay: f64) -> Net {
    Net::wrap(Box::new(An(Decay2 {
        attack,
        decay,
        keep: (0.0, 0.0),
        levels: (0.0, 0.0),
    })))
}

#[derive(Clone)]
struct Dust {
    seed: u64,
    state: u64,
    rate: f64,
}

impl AudioNode for Dust {
    const ID: u64 = 0x6d78_0030;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, rate: f64) {
        self.rate = rate;
    }

    fn reset(&mut self) {
        self.state = self.seed;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let mut draws = Draws(self.state);
        let chance = draws.draw();
        let level = draws.draw();
        self.state = draws.0;
        let p = input[0].max(0.0) as f64 / self.rate;
        [if chance < p { level as f32 } else { 0.0 }].into()
    }
}

#[derive(Clone)]
struct Decay2 {
    attack: f64,
    decay: f64,
    keep: (f64, f64),
    levels: (f64, f64),
}

impl AudioNode for Decay2 {
    const ID: u64 = 0x6d78_0031;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, rate: f64) {
        let keep = |seconds: f64| {
            if seconds > 0.0 {
                0.001f64.powf(1.0 / (seconds * rate))
            } else {
                0.0
            }
        };
        self.keep = (keep(self.attack), keep(self.decay));
    }

    fn reset(&mut self) {
        self.levels = (0.0, 0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let x = input[0] as f64;
        self.levels = (
            x + self.levels.0 * self.keep.0,
            x + self.levels.1 * self.keep.1,
        );
        [(self.levels.1 - self.levels.0) as f32].into()
    }
}
