//! Control waves and what moves with them: the LFO, its shifted and two-sided readings, easing, and the phaser they sweep.

use super::*;

/// A chain of first-order all-passes that share a corner, with the chain's output fed back to its input.
/// Each stage turns the sound from nothing at the lowest frequencies to half a cycle at the highest, a quarter at the corner, so mixed half and half with the dry sound the chain cuts a notch wherever it has turned the sound an odd number of half cycles, one for every two stages.
/// The feedback is solved within the sample, and the output is raised by it, so the notches stay where they are and as deep, and the sound between them rises.
/// Inputs: the sound, the corner in Hz, the feedback; it gives the wet sound only.
#[derive(Clone)]
pub(super) struct Phaser {
    stages: usize,
    rate: f32,
    /// Each stage's last input less its coefficient times its last output.
    held: [f32; 8],
}

impl Phaser {
    pub(super) fn new(stages: usize) -> Self {
        Phaser {
            stages,
            rate: DEFAULT_SR as f32,
            held: [0.0; 8],
        }
    }
}

impl AudioNode for Phaser {
    const ID: u64 = 0x6d78_007d;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.held = [0.0; 8];
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let turn =
            (std::f32::consts::PI * input[1].clamp(20.0, self.rate * 0.45) / self.rate).tan();
        let a = (turn - 1.0) / (turn + 1.0);
        let feedback = input[2].clamp(0.0, 0.95);
        // Each stage gives a * its input + what it holds, so the chain gives gain * its input + rest.
        let (mut gain, mut rest) = (1.0f32, 0.0f32);
        for held in &self.held[..self.stages] {
            gain *= a;
            rest = rest * a + held;
        }
        let out = (gain * input[0] + rest) / (1.0 - feedback * gain);
        let mut through = input[0] + feedback * out;
        for held in &mut self.held[..self.stages] {
            let passed = a * through + *held;
            *held = through - a * passed;
            through = passed;
        }
        [out * (1.0 + feedback)].into()
    }
}

/// A control wave from 0 to 1: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic, starting `offset` of a cycle in.
/// Inputs: cycles a second, the pulse's width, the shape, and 1 to flip it.
#[derive(Clone)]
pub(crate) struct Lfo {
    offset: f32,
    /// Its place in its cycle, kept in f64: a step is a few millionths of a cycle, and in f32 its rounding adds up to milliseconds within a minute.
    phase: f64,
    level: f32,
    pulse_only: bool,
    rate: f64,
}

impl Lfo {
    pub(crate) fn new(offset: f32, level: f32, pulse_only: bool) -> Self {
        Lfo {
            offset,
            phase: f64::from(offset).rem_euclid(1.0),
            level,
            pulse_only,
            rate: DEFAULT_SR,
        }
    }

    /// Starts its cycle `offset` in from now on, moving its place in the cycle by as much as the offset moved.
    fn shift_to(&mut self, offset: f32) {
        if offset != self.offset {
            let moved = f64::from(offset) - f64::from(self.offset);
            self.phase = (self.phase + moved).rem_euclid(1.0);
            self.offset = offset;
        }
    }
}

impl AudioNode for Lfo {
    const ID: u64 = 0x6d78_0012;
    type Inputs = U4;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
    }

    fn reset(&mut self) {
        self.phase = f64::from(self.offset).rem_euclid(1.0);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U1> {
        let level = self.at(self.phase as f32, input);
        self.phase = (self.phase + f64::from(input[0].max(0.0)) / self.rate).fract();
        [level].into()
    }
}

impl Lfo {
    /// Its level at a place in its cycle.
    fn at(&self, p: f32, input: &Frame<f32, U4>) -> f32 {
        let shape = if self.pulse_only {
            1
        } else {
            input[2].clamp(0.0, 4.0).round() as u32
        };
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
        level * self.level
    }
}

/// An [`Lfo`] whose cycle moves on by as much as its last input, where in the cycle it starts, changes.
/// Inputs: the Lfo's four, then the offset from 0 to 1.
#[derive(Clone)]
pub(super) struct Shifted {
    lfo: Lfo,
}

impl Shifted {
    pub(super) fn new(offset: f32, pulse_only: bool) -> Self {
        Shifted {
            lfo: Lfo::new(offset, 1.0, pulse_only),
        }
    }
}

impl AudioNode for Shifted {
    const ID: u64 = 0x6d78_0077;
    type Inputs = U5;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.lfo.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.lfo.reset();
    }

    fn tick(&mut self, input: &Frame<f32, U5>) -> Frame<f32, U1> {
        self.lfo.shift_to(input[4]);
        self.lfo
            .tick(&[input[0], input[1], input[2], input[3]].into())
    }
}

/// A [`Shifted`] read for each side: the left where its cycle is, the right the share its last input gives of half a cycle behind.
/// Inputs: the Shifted's five, then the share from 0 to 1.
#[derive(Clone)]
pub(super) struct Sides {
    shifted: Shifted,
}

impl Sides {
    pub(super) fn new(offset: f32, pulse_only: bool) -> Self {
        Sides {
            shifted: Shifted::new(offset, pulse_only),
        }
    }
}

impl AudioNode for Sides {
    const ID: u64 = 0x6d78_0081;
    type Inputs = U6;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.shifted.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.shifted.reset();
    }

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U2> {
        let lfo: Frame<f32, U4> = [input[0], input[1], input[2], input[3]].into();
        let left = self
            .shifted
            .tick(&[input[0], input[1], input[2], input[3], input[4]].into())[0];
        if input[5] <= 0.0 {
            return [left, left].into();
        }
        // The Shifted has moved on a frame, so the place the left was read at is a frame back.
        let step = f64::from(input[0].max(0.0)) / self.shifted.lfo.rate;
        let behind = step + 0.5 * f64::from(input[5].min(1.0));
        let place = (self.shifted.lfo.phase - behind).rem_euclid(1.0);
        [left, self.shifted.lfo.at(place as f32, &lfo)].into()
    }
}

/// Moves to its input at no more than 1 every 10 ms, starting at its first input.
#[derive(Clone)]
pub(super) struct Ease {
    level: Option<f32>,
    step: f32,
}

impl Default for Ease {
    fn default() -> Self {
        Ease {
            level: None,
            step: (100.0 / DEFAULT_SR) as f32,
        }
    }
}

impl AudioNode for Ease {
    const ID: u64 = 0x6d78_0078;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.step = (100.0 / sample_rate) as f32;
    }

    fn reset(&mut self) {
        self.level = None;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let level = match self.level {
            Some(level) if (input[0] - level).abs() > self.step => {
                level + self.step.copysign(input[0] - level)
            }
            _ => input[0],
        };
        self.level = Some(level);
        [level].into()
    }
}
