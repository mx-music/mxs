//! Levels shaped by the sound itself: drive, limiting, compression, smoothing, lag and bit crushing.

use super::*;

/// How many taps the distortion's low-pass has, and how many frames its sound comes out late: the filter twice over, at a quarter of a frame a tap.
pub(super) const DRIVE_TAPS: usize = 249;

pub(super) const DRIVE_LATE: usize = (DRIVE_TAPS - 1) / 4;

/// The low-pass the distortion runs at four times the engine's rate, before and after its curve: a sinc under a Kaiser window, which passes up to 0.43 of the engine's rate and is 70 dB down from half of it, and delays every frequency alike.
pub(super) fn drive_taps() -> &'static [f64] {
    static TAPS: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    TAPS.get_or_init(|| {
        // The modified Bessel function of order 0, by its series.
        let bessel = |x: f64| {
            let (mut sum, mut term) = (1.0, 1.0);
            for k in 1..40 {
                term *= (x / 2.0 / k as f64).powi(2);
                sum += term;
            }
            sum
        };
        let (beta, edge, middle) = (6.76, 0.11625, (DRIVE_TAPS - 1) as f64 / 2.0);
        let taps: Vec<f64> = (0..DRIVE_TAPS)
            .map(|n| {
                let from = n as f64 - middle;
                let turn = std::f64::consts::TAU * edge * from;
                let sinc = if from == 0.0 { 1.0 } else { turn.sin() / turn };
                let window = bessel(beta * (1.0 - (from / middle).powi(2)).sqrt()) / bessel(beta);
                sinc * window
            })
            .collect();
        let sum: f64 = taps.iter().sum();
        taps.iter().map(|tap| tap / sum).collect()
    })
}

/// The distortion's curve worked out at four times the engine's rate, so the harmonics it adds above half the engine's rate are filtered out before they come back at other pitches.
/// Between two samples the curve is the mean of itself over the step from one to the other, from its antiderivative, after Parker, Zavalishin and Le Bivic, "Reducing the aliasing of nonlinear waveshaping using continuous-time convolution", 2016.
/// Its sound comes out `DRIVE_LATE` frames after its input.
/// Inputs: sound, distort from 0 to 1.
#[derive(Clone)]
pub(super) struct Drive {
    hard: bool,
    /// The last sounds in, and the last out of the curve at four times the rate, each written twice over so a run of them is one slice.
    heard: Vec<f64>,
    bent: Vec<f64>,
    at: (usize, usize),
    before: f64,
}

impl Drive {
    const HEARD: usize = DRIVE_TAPS / 4 + 1;

    pub(super) fn new(hard: bool) -> Self {
        Drive {
            hard,
            heard: vec![0.0; 2 * Self::HEARD],
            bent: vec![0.0; 2 * DRIVE_TAPS],
            at: (0, 0),
            before: 0.0,
        }
    }

    fn curve(&self, x: f64) -> f64 {
        if self.hard {
            x.clamp(-1.0, 1.0)
        } else {
            x.tanh()
        }
    }

    /// The curve's antiderivative.
    fn under(&self, x: f64) -> f64 {
        let size = x.abs();
        if self.hard {
            if size <= 1.0 { x * x / 2.0 } else { size - 0.5 }
        } else {
            size + (-2.0 * size).exp().ln_1p() - std::f64::consts::LN_2
        }
    }
}

impl AudioNode for Drive {
    const ID: u64 = 0x6d78_007f;
    type Inputs = U2;
    type Outputs = U1;

    fn reset(&mut self) {
        *self = Drive::new(self.hard);
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        let taps = drive_taps();
        let drive = 1.0 + f64::from(input[1].clamp(0.0, 1.0)) * 30.0;
        let level = if self.hard { 1.0 } else { drive.tanh() };
        let (heard, bent) = (Self::HEARD, DRIVE_TAPS);
        self.at.0 = (self.at.0 + heard - 1) % heard;
        self.heard[self.at.0] = f64::from(input[0]);
        self.heard[self.at.0 + heard] = f64::from(input[0]);
        let mut out = 0.0;
        for step in 0..4 {
            // The sound between two frames: every fourth tap from this step's, over the last sounds in, newest first.
            let recent = &self.heard[self.at.0..self.at.0 + heard];
            let between: f64 = taps[step..]
                .iter()
                .step_by(4)
                .zip(recent)
                .map(|(tap, sound)| tap * sound)
                .sum();
            let now = drive * 4.0 * between;
            let moved = now - self.before;
            let curved = if moved.abs() < 1e-6 {
                self.curve((now + self.before) / 2.0)
            } else {
                (self.under(now) - self.under(self.before)) / moved
            };
            self.before = now;
            self.at.1 = (self.at.1 + bent - 1) % bent;
            self.bent[self.at.1] = curved / level;
            self.bent[self.at.1 + bent] = curved / level;
            // The frame out is the one on this frame's own time, the first of its four.
            if step == 0 {
                let recent = &self.bent[self.at.1..self.at.1 + bent];
                out = taps
                    .iter()
                    .zip(recent)
                    .map(|(tap, sound)| tap * sound)
                    .sum();
            }
        }
        [out as f32].into()
    }
}

/// Turns both sides down alike just before a peak on either would pass the ceiling, and lets them back up afterwards; its sound comes out `LIMITER_LOOK_AHEAD` late.
/// The gain each frame may have is the ceiling over its level; the lowest of those over the look-ahead, let back up over the relax time, is averaged over the look-ahead, so the gain glides down to each peak and no sample passes the ceiling.
/// Inputs: left, right, the ceiling as a level, the relax time in milliseconds.
#[derive(Clone)]
pub(super) struct Limiter {
    ahead: usize,
    sound: Vec<[f32; 2]>,
    /// The frames of the look-ahead whose gain is lower than every later one, oldest first, as (frame, gain).
    lowest: std::collections::VecDeque<(u64, f32)>,
    let_up: f32,
    gains: Vec<f32>,
    sum: f64,
    at: usize,
    frame: u64,
    rate: f64,
}

impl Limiter {
    pub(super) fn new() -> Self {
        let mut limiter = Limiter {
            ahead: 0,
            sound: Vec::new(),
            lowest: std::collections::VecDeque::new(),
            let_up: 1.0,
            gains: Vec::new(),
            sum: 0.0,
            at: 0,
            frame: 0,
            rate: DEFAULT_SR,
        };
        limiter.set_sample_rate(DEFAULT_SR);
        limiter
    }
}

impl AudioNode for Limiter {
    const ID: u64 = 0x6d78_007a;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.ahead = (LIMITER_LOOK_AHEAD * sample_rate).round().max(1.0) as usize;
        self.sound = vec![[0.0; 2]; self.ahead];
        self.gains = vec![1.0; self.ahead];
        self.lowest = std::collections::VecDeque::with_capacity(self.ahead + 2);
        self.reset();
    }

    fn reset(&mut self) {
        self.sound.fill([0.0; 2]);
        self.gains.fill(1.0);
        self.sum = self.ahead as f64;
        self.lowest.clear();
        self.let_up = 1.0;
        self.at = 0;
        self.frame = 0;
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let level = input[0].abs().max(input[1].abs());
        let ceiling = input[2].max(1e-6);
        let allowed = if level > ceiling {
            ceiling / level
        } else {
            1.0
        };
        while self.lowest.back().is_some_and(|(_, gain)| *gain >= allowed) {
            self.lowest.pop_back();
        }
        self.lowest.push_back((self.frame, allowed));
        while self
            .lowest
            .front()
            .is_some_and(|(frame, _)| frame + (self.ahead as u64) < self.frame)
        {
            self.lowest.pop_front();
        }
        let lowest = self.lowest.front().map_or(1.0, |(_, gain)| *gain);
        let frames = (f64::from(input[3]).max(1.0) / 1000.0 * self.rate) as f32;
        let risen = self.let_up + (1.0 - self.let_up) * (6.9 / frames).min(1.0);
        self.let_up = lowest.min(risen);
        self.sum += f64::from(self.let_up) - f64::from(self.gains[self.at]);
        self.gains[self.at] = self.let_up;
        let gain = (self.sum / self.ahead as f64) as f32;
        let delayed = std::mem::replace(&mut self.sound[self.at], [input[0], input[1]]);
        self.at = (self.at + 1) % self.ahead;
        self.frame += 1;
        [delayed[0] * gain, delayed[1] * gain].into()
    }
}

/// Glides to its first input, falling 60 dB short of each step over its second, in milliseconds.
/// Inputs: level, milliseconds.
#[derive(Clone)]
pub(super) struct Smooth {
    ms: f32,
    keep: f32,
    level: Option<f32>,
    rate: f64,
}

impl Smooth {
    pub(super) fn new() -> Self {
        Smooth {
            ms: 0.0,
            keep: 0.0,
            level: None,
            rate: DEFAULT_SR,
        }
    }
}

impl AudioNode for Smooth {
    const ID: u64 = 0x6d78_0079;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.ms = 0.0;
        self.keep = 0.0;
    }

    fn reset(&mut self) {
        self.level = None;
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        if input[1] != self.ms {
            self.ms = input[1];
            let seconds = self.ms as f64 / 1000.0;
            self.keep = if seconds > 0.0 {
                0.001f64.powf(1.0 / (seconds * self.rate)) as f32
            } else {
                0.0
            };
        }
        let level = match self.level {
            Some(level) => input[0] + (level - input[0]) * self.keep,
            None => input[0],
        };
        self.level = Some(level);
        [level].into()
    }
}

/// Glides to its input, falling 60 dB short of each step over `seconds`.
#[derive(Clone)]
pub(crate) struct Lag {
    seconds: f64,
    keep: f32,
    level: Option<f32>,
}

impl Lag {
    pub(crate) fn new(seconds: f64) -> Self {
        let mut lag = Lag {
            seconds,
            keep: 0.0,
            level: None,
        };
        lag.set_sample_rate(DEFAULT_SR);
        lag
    }
}

impl AudioNode for Lag {
    const ID: u64 = 0x6d78_0013;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.keep = if self.seconds > 0.0 {
            0.001f64.powf(1.0 / (self.seconds * sample_rate)) as f32
        } else {
            0.0
        };
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

/// Follows the level of the louder side, or of the louder side of its key while it has one, and above the threshold turns both sides down alike, so the level comes out 1 dB higher for each ratio dB it rises.
/// The level is the last peak let go over the relax time, which the level then rises to over the clamp time and falls to over the relax time again, so it holds still through the cycle of a low note.
/// Inputs: left, right, threshold in dB, ratio, makeup in dB, clamp time, relax time, the key's left and right, and whether it has a key.
#[derive(Clone)]
pub(super) struct Compressor {
    peak: f32,
    level: f32,
    rate: f32,
}

impl Compressor {
    pub(super) fn new() -> Self {
        Compressor {
            peak: 0.0,
            level: 0.0,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Compressor {
    const ID: u64 = 0x6d78_0014;
    type Inputs = U10;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        (self.peak, self.level) = (0.0, 0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U10>) -> Frame<f32, U2> {
        let keep = |seconds: f32| {
            if seconds > 0.0 {
                0.001f32.powf(1.0 / (seconds * self.rate))
            } else {
                0.0
            }
        };
        let (clamp, relax) = (keep(input[5]), keep(input[6]));
        let size = if input[9] > 0.5 {
            input[7].abs().max(input[8].abs())
        } else {
            input[0].abs().max(input[1].abs())
        };
        self.peak = size.max(size + (self.peak - size) * relax);
        let towards = if self.peak > self.level { clamp } else { relax };
        self.level = self.peak + (self.level - self.peak) * towards;
        let over = 20.0 * self.level.max(1e-9).log10() - input[2];
        let lowered = if over > 0.0 {
            over * (1.0 / input[3].max(1.0) - 1.0)
        } else {
            0.0
        };
        let gain = db_amp(lowered + input[4]);
        [input[0] * gain, input[1] * gain].into()
    }
}

/// A cutoff in Hz that follows the level of the louder side: at the first note with silence, at the second with a sound at full scale, and between them by the level in dB over the `WAH_FLOOR` under full scale.
/// The level is followed as the [`Compressor`] follows it.
/// Inputs: left, right, clamp time, relax time, dB added to the level, the two MIDI notes.
#[derive(Clone)]
pub(super) struct Following {
    peak: f32,
    level: f32,
    rate: f32,
}

impl Following {
    pub(super) fn new() -> Self {
        Following {
            peak: 0.0,
            level: 0.0,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Following {
    const ID: u64 = 0x6d78_0082;
    type Inputs = U7;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        (self.peak, self.level) = (0.0, 0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U7>) -> Frame<f32, U1> {
        let keep = |seconds: f32| {
            if seconds > 0.0 {
                0.001f32.powf(1.0 / (seconds * self.rate))
            } else {
                0.0
            }
        };
        let (clamp, relax) = (keep(input[2]), keep(input[3]));
        let size = input[0].abs().max(input[1].abs());
        self.peak = size.max(size + (self.peak - size) * relax);
        let towards = if self.peak > self.level { clamp } else { relax };
        self.level = self.peak + (self.level - self.peak) * towards;
        let db = 20.0 * self.level.max(1e-9).log10() + input[4];
        let open = (1.0 + db / WAH_FLOOR).clamp(0.0, 1.0);
        let note = input[5] + open * (input[6] - input[5]);
        [hz(note as f64).clamp(20.0, 20_000.0) as f32].into()
    }
}

/// Airwindows' DeRez, as mx's design describes it: each sample held at the sample rate, its step softened by blending in the sample held before, then rounded away from zero to a step of bits.
/// Inputs: sound, sample rate in Hz, bits.
#[derive(Clone)]
pub(super) struct Crusher {
    since: f32,
    held: f32,
    before: f32,
    rate: f32,
}

impl Crusher {
    pub(super) fn new() -> Self {
        Crusher {
            since: f32::INFINITY,
            held: 0.0,
            before: 0.0,
            rate: DEFAULT_SR as f32,
        }
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
