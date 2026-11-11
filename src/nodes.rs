//! The engine's vocabulary of nodes, which mx's synths are made of and a synth written in Rust can use.
//! Each is a FunDSP unit, so they combine with FunDSP's own through its operators.

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

/// A level worked out from the seconds since the note started, at every sample.
pub fn shape<F>(f: F) -> Net
where
    F: Fn(f64) -> f64 + Clone + Send + Sync + 'static,
{
    Net::wrap(Box::new(An(Shape { f, t: 0.0, dt: 1.0 / DEFAULT_SR })))
}

/// A percussive level at `t` seconds: a straight rise over `attack`, then a fall to silence over `release`.
pub fn perc(t: f64, attack: f64, release: f64) -> f64 {
    if t < attack { t / attack } else { (1.0 - (t - attack) / release).max(0.0).powi(4) }
}

/// White noise drawn from `seed`.
pub fn white_noise(seed: u64) -> Net {
    Net::wrap(Box::new(white().seed(seed)))
}

/// Pink noise drawn from `seed`.
pub fn pink_noise(seed: u64) -> Net {
    Net::wrap(Box::new(pink::<f64>().seed(seed)))
}

#[derive(Clone)]
struct Mode {
    hz: f64,
    phase: f64,
    level: f64,
    fade: f64,
    coefficients: (f64, f64, f64, f64),
    x1: f64,
    y1: f64,
    y2: f64,
}

impl Mode {
    fn new(hz: f64, phase: f64, level: f64, fade: f64) -> Self {
        let mut mode = Mode { hz, phase, level, fade, coefficients: (0.0, 0.0, 0.0, 0.0), x1: 0.0, y1: 0.0, y2: 0.0 };
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
        // Its impulse response is level * r^n * sin((n + 1) * w + phase).
        self.coefficients =
            (-2.0 * r * w.cos(), r * r, self.level * (w + self.phase).sin(), -self.level * r * self.phase.sin());
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let (a1, a2, b0, b1) = self.coefficients;
        let x = input[0] as f64;
        let y = b0 * x + b1 * self.x1 - a1 * self.y1 - a2 * self.y2;
        (self.x1, self.y2, self.y1) = (x, self.y1, y);
        [y as f32].into()
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
