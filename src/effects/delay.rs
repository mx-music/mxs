//! Delays: a pitch shifter's grains, the comb an echo feeds back through, the wear of a tape echo, and a delay that bounces between the sides.

use super::*;

/// The longest the pitch shift delays its sound, in seconds.
pub(super) const SHIFT_DELAY: f64 = 0.04;

/// How far later than its nominal delay, in seconds, a grain of the pitch shift may start to continue the grain before.
pub(super) const SHIFT_REACH: f64 = 0.005;

/// The frames before a read point the pitch shift compares, every other one, to match a grain to the one before.
pub(super) const SHIFT_MATCH: usize = 256;

#[derive(Clone, Copy, Default)]
pub(super) struct ShiftGrain {
    delay: f32,
    age: usize,
    length: usize,
}

/// Shifts the pitch of its input by reading a delay line at another speed in Hann-windowed grains, half overlapping, each started at the delay where what it has just read best matches the grain before, so the waveform runs on unbroken; the sound is delayed by at most `SHIFT_DELAY`.
/// Inputs: sound, pitch in semitones.
#[derive(Clone)]
pub(super) struct PitchShift {
    buffer: Vec<f32>,
    at: usize,
    grains: [ShiftGrain; 3],
    until: usize,
    rate: f32,
}

impl PitchShift {
    pub(super) fn new() -> Self {
        let mut shift = PitchShift {
            buffer: Vec::new(),
            at: 0,
            grains: [ShiftGrain::default(); 3],
            until: 0,
            rate: 0.0,
        };
        shift.set_sample_rate(DEFAULT_SR);
        shift
    }

    fn read(&self, frames: f32) -> f32 {
        let n = self.buffer.len() as isize;
        let whole = frames.floor();
        let fraction = frames - whole;
        let back =
            |k: isize| self.buffer[(self.at as isize - whole as isize - k).rem_euclid(n) as usize];
        back(0) + (back(1) - back(0)) * fraction
    }

    /// The delay from `nominal` to `SHIFT_REACH` later whose last frames best match those before `from`, the delay of the grain to continue; the earliest of equal matches.
    fn matched(&self, from: f32, nominal: f32) -> f32 {
        let reach = (SHIFT_REACH as f32 * self.rate) as usize;
        let score = |delay: f32| {
            let (mut both, mut energy) = (0.0f32, 1e-9f32);
            for i in (0..SHIFT_MATCH).step_by(2) {
                let b = self.read(delay + i as f32);
                both += self.read(from + i as f32) * b;
                energy += b * b;
            }
            both / energy.sqrt()
        };
        let start = nominal.floor() + from.fract();
        let mut best = (start, score(start));
        for k in 1..=reach {
            let delay = start + k as f32;
            let found = score(delay);
            if found > best.1 + 1e-6 {
                best = (delay, found);
            }
        }
        best.0
    }
}

impl AudioNode for PitchShift {
    const ID: u64 = 0x6d78_0075;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.buffer = vec![0.0; (SHIFT_DELAY * sample_rate) as usize + SHIFT_MATCH + 16];
        self.at = 0;
        self.grains = [ShiftGrain::default(); 3];
        self.until = 0;
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.grains = [ShiftGrain::default(); 3];
        self.until = 0;
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        self.at = (self.at + 1) % self.buffer.len();
        self.buffer[self.at] = input[0];
        let speed = 2f32.powf(input[1] / 12.0);
        if self.until == 0 {
            let most = SHIFT_DELAY as f32 * self.rate;
            let reach = SHIFT_REACH as f32 * self.rate;
            // A grain read faster than the sound arrives starts its delay high enough to end it at the shortest.
            let length = if speed > 1.0 {
                (most * 0.95).min((most - reach - 4.0) / (speed - 1.0))
            } else {
                most * 0.95
            }
            .max(32.0) as usize;
            let nominal = 2.0 + (speed - 1.0).max(0.0) * length as f32;
            let youngest = self
                .grains
                .iter()
                .filter(|g| g.age < g.length)
                .min_by_key(|g| g.age)
                .map(|g| g.delay);
            let delay = match youngest {
                Some(from) => self.matched(from, nominal),
                None => nominal,
            };
            if let Some(free) = self.grains.iter_mut().find(|g| g.age >= g.length) {
                *free = ShiftGrain {
                    delay,
                    age: 0,
                    length,
                };
            }
            self.until = length / 2;
        }
        self.until -= 1;
        let mut out = 0.0;
        for k in 0..self.grains.len() {
            let grain = self.grains[k];
            if grain.age >= grain.length {
                continue;
            }
            let phase = grain.age as f32 / grain.length as f32;
            let window = 0.5 - 0.5 * (TAU * phase).cos();
            out += window * self.read(grain.delay);
            let grain = &mut self.grains[k];
            grain.delay += 1.0 - speed;
            grain.age += 1;
        }
        [out].into()
    }
}

/// A delay of the input that feeds back on itself, by as much as dies away 60 dB over its decay; a negative decay feeds it back inverted.
/// Inputs: sound, delay in seconds, decay in seconds.
#[derive(Clone)]
pub(super) struct Comb {
    most: f64,
    cubic: bool,
    buffer: Vec<f32>,
    at: usize,
    rate: f32,
}

impl Comb {
    pub(super) fn new(most: f64, cubic: bool) -> Self {
        let mut comb = Comb {
            most,
            cubic,
            buffer: Vec::new(),
            at: 0,
            rate: DEFAULT_SR as f32,
        };
        comb.set_sample_rate(DEFAULT_SR);
        comb
    }

    fn read(&self, frames: f32) -> f32 {
        let n = self.buffer.len();
        let whole = frames.floor();
        let fraction = frames - whole;
        let back = |k: isize| {
            self.buffer[(self.at as isize - whole as isize - k).rem_euclid(n as isize) as usize]
        };
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

    /// Moves on a frame: takes `sound` in, delayed by `delay` seconds and fed back by as much as dies away 60 dB over `decay` seconds, and gives what comes back round after `shape` works on it, at the comb's rate.
    fn step(
        &mut self,
        sound: f32,
        delay: f32,
        decay: f32,
        shape: impl FnOnce(f32, f32) -> f32,
    ) -> f32 {
        let limit = (self.buffer.len() - 3) as f32;
        let frames = (delay * self.rate).clamp(1.0, limit);
        self.at = (self.at + 1) % self.buffer.len();
        let delayed = shape(self.read(frames), self.rate);
        let feedback = if decay == 0.0 {
            0.0
        } else {
            0.001f32.powf(delay.max(0.0) / decay.abs()).copysign(decay)
        };
        self.buffer[self.at] = sound + feedback * delayed;
        delayed
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
        [self.step(input[0], input[1], input[2], |delayed, _| delayed)].into()
    }
}

/// What each pass round an echo's loop does to the sound: a low-pass and a high-pass of one pole, and a curve that rounds it off.
/// A cut that is off, or a drive of 0, leaves the samples as they are.
#[derive(Clone, Default)]
pub(super) struct Wear {
    below: f32,
    under: f32,
    cuts: (f32, f32),
    keeps: (f32, f32),
}

impl Wear {
    fn pass(&mut self, sound: f32, high_cut: f32, low_cut: f32, drive: f32, rate: f32) -> f32 {
        if (high_cut, low_cut) != self.cuts {
            self.cuts = (high_cut, low_cut);
            let keep = |note: f32| 1.0 - (-TAU * hz(note as f64) as f32 / rate).exp();
            self.keeps = (keep(high_cut).min(1.0), keep(low_cut).min(1.0));
        }
        let mut sound = sound;
        if high_cut < 136.0 {
            self.below += self.keeps.0 * (sound - self.below);
            sound = self.below;
        } else {
            self.below = sound;
        }
        if low_cut > 0.0 {
            self.under += self.keeps.1 * (sound - self.under);
            sound -= self.under;
        } else {
            self.under = 0.0;
        }
        if drive > 0.0 {
            let gain = 4.0 * drive.min(1.0);
            sound = (gain * sound).tanh() / gain;
        }
        sound
    }
}

/// The comb of the echo: a delay that feeds back on itself by as much as dies away 60 dB over its decay, each pass worn by its cuts and its drive, the first echo once.
/// Inputs: sound, delay in seconds, decay in seconds, high_cut and low_cut as MIDI notes, drive.
#[derive(Clone)]
pub(super) struct Echo {
    comb: Comb,
    wear: Wear,
}

impl Echo {
    pub(super) fn new(most: f64) -> Self {
        Echo {
            comb: Comb::new(most, false),
            wear: Wear::default(),
        }
    }
}

impl AudioNode for Echo {
    const ID: u64 = 0x6d78_007e;
    type Inputs = U6;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.comb.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.comb.reset();
        self.wear = Wear::default();
    }

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U1> {
        let wear = &mut self.wear;
        let delayed = self
            .comb
            .step(input[0], input[1], input[2], |delayed, rate| {
                wear.pass(delayed, input[3], input[4], input[5], rate)
            });
        [delayed].into()
    }
}

/// Echoes a phase apart, alternating between one side and the other, each `feedback` times the one before and worn by the cuts and the drive once more.
/// Inputs: left, right, phase in seconds, feedback, the side of the first echo from -1 to 1, high_cut and low_cut as MIDI notes, drive.
#[derive(Clone)]
pub(super) struct PingPong {
    most: f64,
    first: Vec<f32>,
    second: Vec<f32>,
    at: usize,
    rate: f32,
    wear: (Wear, Wear),
}

impl PingPong {
    pub(super) fn new(most: f64) -> Self {
        let mut ping = PingPong {
            most,
            first: Vec::new(),
            second: Vec::new(),
            at: 0,
            rate: DEFAULT_SR as f32,
            wear: (Wear::default(), Wear::default()),
        };
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
    type Inputs = U8;
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
        self.wear = (Wear::default(), Wear::default());
    }

    fn tick(&mut self, input: &Frame<f32, U8>) -> Frame<f32, U2> {
        let limit = (self.first.len() - 3) as f32;
        let frames = (input[2] * self.rate).clamp(1.0, limit);
        let feedback = input[3].clamp(0.0, 0.999);
        self.at = (self.at + 1) % self.first.len();
        let (high_cut, low_cut, drive) = (input[5], input[6], input[7]);
        let first = Self::read(&self.first, self.at, frames);
        let first = self.wear.0.pass(first, high_cut, low_cut, drive, self.rate) * feedback;
        let second = Self::read(&self.second, self.at, frames);
        let second = self
            .wear
            .1
            .pass(second, high_cut, low_cut, drive, self.rate)
            * feedback;
        self.first[self.at] = input[0] + input[1] + second;
        self.second[self.at] = first;
        let place = |side: f32| {
            let angle = (side + 1.0) * FRAC_PI_4;
            (angle.cos(), angle.sin())
        };
        let (fl, fr) = place(input[4]);
        let (sl, sr) = place(-input[4]);
        [
            input[0] + first * fl + second * sl,
            input[1] + first * fr + second * sr,
        ]
        .into()
    }
}
