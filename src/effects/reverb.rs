//! A room as a feedback delay network.

use super::*;

/// A room as a feedback delay network, after Jot and Chaigne, "Digital delay networks for designing artificial reverberators", 1991: sixteen delay lines whose lengths follow the room's size, each fed back through a gain that fades it 60 dB in the decay time and a low-pass that fades its highest frequencies sooner, by the damping.
/// Eight lines take the left side and eight the right; each eight is mixed among itself by a Hadamard matrix every time round, and a small rotation between the two eights lets a sound cross to the other side a little at a time, so a room starts on the side of its sound and spreads.
/// Three all-passes a side smear what comes in, and the longest lines are read at a slowly moving point, which keeps a long tail from ringing at fixed pitches.
/// Inputs: left, right, the decay in seconds, damp; it gives the wet sound only.
#[derive(Clone)]
pub(super) struct Reverb {
    size: f32,
    smear: f32,
    moving: f32,
    rate: f32,
    lines: Vec<Vec<f32>>,
    at: [usize; REVERB_LINES],
    dulled: [f32; REVERB_LINES],
    gains: [f32; REVERB_LINES],
    smears: Vec<Vec<f32>>,
    smear_at: [usize; 6],
    /// The decay and damp the gains and the low-pass were last worked out for.
    set: (f32, f32),
    /// How much of its last value each line's low-pass keeps.
    keeps: [f32; REVERB_LINES],
    turns: [f32; 8],
}

pub(super) const REVERB_LINES: usize = 16;

/// The length of each line as a share of the time sound takes to cross the room, left and right in turn.
pub(super) const REVERB_LENGTHS: [f32; REVERB_LINES] = [
    0.350, 0.375, 0.403, 0.432, 0.463, 0.497, 0.533, 0.571, 0.613, 0.657, 0.705, 0.756, 0.811,
    0.870, 0.933, 1.000,
];
/// The length of each all-pass on the way in as a share of that time, three for the left and three for the right.
pub(super) const REVERB_SMEARS: [f32; 6] = [0.079, 0.083, 0.121, 0.127, 0.167, 0.173];
/// The share of each side's sound that crosses to the other each time round, as the sine and cosine of the rotation.
pub(super) const REVERB_CROSS: (f32, f32) = (0.3873, 0.9220);
pub(super) const SPEED_OF_SOUND: f32 = 343.0;

impl Reverb {
    pub(super) fn new(size: f32) -> Self {
        Reverb::shaped(size, 0.6, 6.0)
    }

    /// A room whose all-passes feed back by `smear` and whose long lines' read points move by `moving` frames either way.
    fn shaped(size: f32, smear: f32, moving: f32) -> Self {
        let mut reverb = Reverb {
            size,
            smear,
            moving,
            rate: DEFAULT_SR as f32,
            lines: Vec::new(),
            at: [0; REVERB_LINES],
            dulled: [0.0; REVERB_LINES],
            gains: [0.0; REVERB_LINES],
            smears: Vec::new(),
            smear_at: [0; 6],
            set: (f32::NAN, f32::NAN),
            keeps: [0.0; REVERB_LINES],
            turns: [0.0; 8],
        };
        reverb.set_sample_rate(DEFAULT_SR);
        reverb
    }

    /// The frames of a share of the room's crossing time, moved up to a prime so no two lines repeat together.
    fn frames(&self, share: f32) -> usize {
        let mut frames = (share * self.size / SPEED_OF_SOUND * self.rate)
            .round()
            .max(2.0) as usize;
        let prime = |n: usize| {
            (2..)
                .take_while(|d| d * d <= n)
                .all(|d| !n.is_multiple_of(d))
        };
        while !prime(frames) {
            frames += 1;
        }
        frames
    }
}

/// Mixes eight values by a Hadamard matrix that keeps their energy.
pub(super) fn hadamard(values: &mut [f32; 8]) {
    for width in [1, 2, 4] {
        for start in (0..8).step_by(width * 2) {
            for i in start..start + width {
                let (a, b) = (values[i], values[i + width]);
                (values[i], values[i + width]) = (a + b, a - b);
            }
        }
    }
    for value in values.iter_mut() {
        *value *= 0.353_553_4;
    }
}

impl AudioNode for Reverb {
    const ID: u64 = 0x6d78_007c;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.lines = REVERB_LENGTHS
            .iter()
            .map(|share| vec![0.0; self.frames(*share)])
            .collect();
        self.smears = REVERB_SMEARS
            .iter()
            .map(|share| vec![0.0; self.frames(*share)])
            .collect();
        self.reset();
    }

    fn reset(&mut self) {
        self.lines.iter_mut().for_each(|line| line.fill(0.0));
        self.smears.iter_mut().for_each(|line| line.fill(0.0));
        (self.at, self.smear_at) = ([0; REVERB_LINES], [0; 6]);
        self.dulled = [0.0; REVERB_LINES];
        self.set = (f32::NAN, f32::NAN);
        self.turns = std::array::from_fn(|i| i as f32 / 8.0);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let (decay, damp) = (input[2].max(0.05), input[3].clamp(0.0, 1.0));
        if (decay, damp) != self.set {
            self.set = (decay, damp);
            // The highest frequencies fade in this share of the decay time: all of it with no damping, a twentieth with all.
            let share = (1.0 - damp).powf(1.5).max(0.05);
            for i in 0..REVERB_LINES {
                let gain = 0.001f32.powf(self.lines[i].len() as f32 / (self.rate * decay));
                self.gains[i] = gain;
                self.keeps[i] = 1.0 - 2.0 / (1.0 + gain.powf(1.0 - 1.0 / share));
            }
        }

        let mut sides = [input[0], input[1]];
        for (i, smear) in self.smears.iter_mut().enumerate() {
            let at = &mut self.smear_at[i];
            let held = smear[*at];
            let into = sides[i % 2] + held * self.smear;
            smear[*at] = into;
            sides[i % 2] = held - into * self.smear;
            *at = (*at + 1) % smear.len();
        }

        // The four longest lines of each side are read up to 12 frames short of their length, each moving at its own slow rate.
        let mut heard = [[0.0f32; 8]; 2];
        let mut out = [0.0f32; 2];
        for i in 0..REVERB_LINES {
            let line = &self.lines[i];
            let read = if i >= 8 {
                let turn = &mut self.turns[i - 8];
                *turn = (*turn + (0.31 + 0.17 * (i - 8) as f32) / self.rate).fract();
                let short = self.moving * (1.0 + (std::f32::consts::TAU * *turn).sin());
                let (whole, part) = (short as usize, short.fract());
                let first = line[(self.at[i] + whole) % line.len()];
                let second = line[(self.at[i] + whole + 1) % line.len()];
                first + (second - first) * part
            } else {
                line[self.at[i]]
            };
            let lowered = read * self.gains[i];
            self.dulled[i] = lowered + (self.dulled[i] - lowered) * self.keeps[i];
            heard[i % 2][i / 2] = self.dulled[i];
            let sign = if (i / 2).is_multiple_of(2) { 1.0 } else { -1.0 };
            out[i % 2] += self.dulled[i] * sign;
        }

        hadamard(&mut heard[0]);
        hadamard(&mut heard[1]);
        let (cross, keep) = REVERB_CROSS;
        for i in 0..REVERB_LINES {
            let (own, other) = (heard[i % 2][i / 2], heard[1 - i % 2][i / 2]);
            let turned = if i % 2 == 0 {
                own * keep + other * cross
            } else {
                own * keep - other * cross
            };
            let at = self.at[i];
            self.lines[i][at] = turned + sides[i % 2];
            self.at[i] = (at + 1) % self.lines[i].len();
        }
        [out[0] * 0.2, out[1] * 0.2].into()
    }
}
