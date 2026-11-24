//! A struck note's sound: its sines and noise bands, read from the voice's figures as it falls.

use super::*;

// The voice as a sound at `gain`, which fills itself again as it starts when given a refill, and follows the bend and brightness of `expressions` when given them.
pub(super) fn struck(
    voice: Voice,
    gain: f64,
    refill: Option<Refill>,
    expressions: Option<Net>,
) -> Net {
    let node = Struck::new(voice, gain, refill, DEFAULT_SR);
    let plain = || Net::wrap(Box::new(dc((0.0, 0.5))));
    expressions.unwrap_or_else(plain) >> Net::wrap(Box::new(An(node)))
}

#[derive(Clone)]
pub(super) struct Struck {
    voice: Voice,
    rate: f64,
    sines: Vec<Sine>,
    bands: Vec<Band>,
    /// Every band below half the rate, warmed, which a note takes the bands it sounds from.
    warm: Vec<Band>,
    sample: usize,
    gain: f32,
    refill: Option<Refill>,
    /// The bend and brightness at its input, and those its sines play at.
    wanted: (f64, f64),
    applied: (f64, f64),
    /// The lowest partial's frequency, which a brightness tilts the others' levels from.
    lowest: f64,
}

impl Struck {
    fn new(voice: Voice, gain: f64, refill: Option<Refill>, rate: f64) -> Struck {
        let mut node = Struck {
            voice,
            rate,
            sines: vec![],
            bands: vec![],
            warm: vec![],
            sample: 0,
            gain: gain as f32,
            refill,
            wanted: (0.0, 0.5),
            applied: (0.0, 0.5),
            lowest: 0.0,
        };
        node.set_sample_rate(rate);
        node
    }

    /// The sines and bands `voice` sounds with at its first sample, at `rate`.
    pub(super) fn opening(voice: Voice, rate: f64) -> (Vec<Sine>, Vec<Band>) {
        let node = Struck::new(voice, 1.0, None, rate);
        (node.sines, node.bands)
    }

    // Starts the note the voice holds from its first sample, within the room set aside.
    fn start(&mut self) {
        self.sample = 0;
        fill_sines(&self.voice, self.rate, &mut self.sines);
        fill_bands(&self.voice, &self.warm, &mut self.bands);
        self.applied.0 = 0.0;
        self.lowest = self.sines.iter().fold(f64::INFINITY, |low, sine| {
            low.min(self.voice.partials[sine.shape].0)
        });
        self.read();
    }

    // Turns each sine at its partial's frequency moved by the bend wanted, when it moved.
    fn bend(&mut self) {
        if self.wanted.0 != self.applied.0 {
            let ratio = 2f64.powf(self.wanted.0 / 12.0);
            for sine in &mut self.sines {
                let freq = self.voice.partials[sine.shape].0 * ratio;
                let omega = 2.0 * std::f64::consts::PI * freq / self.rate;
                sine.turn = (omega.cos(), omega.sin());
            }
            self.applied.0 = self.wanted.0;
        }
        self.applied.1 = self.wanted.1;
    }

    // Fills the voice with the note and vel the engine gave it, when it gave one.
    fn refill(&mut self) {
        let Some(refill) = &mut self.refill else {
            return;
        };
        let Some(given) = refill.hold.given() else {
            return;
        };
        let recipe = &refill.recipes[picked(given.pedal, refill.recipes.len())];
        self.gain = recipe.play(given.note, given.vel, &mut self.voice, &mut refill.working) as f32;
        self.start();
    }

    // The levels at the next reading, BLOCK samples on, for each sine and band to glide to.
    fn read(&mut self) {
        self.bend();
        let t = (self.sample + BLOCK) as f64 / self.rate;
        let last = *self.voice.times.last().unwrap();
        // A brightness away from 0.5 tilts the levels by up to 3 dB an octave either way.
        let tilt = (self.applied.1 != 0.5).then_some((self.applied.1 - 0.5) * 6.0);
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            let (freq, shape) = &self.voice.partials[sine.shape];
            let mut level = shape.at(&self.voice.times, t);
            if let Some(tilt) = tilt {
                level += tilt * (freq / self.lowest).log2();
            }
            let target = (t / RISE).min(1.0) * 10f64.powf(level / 20.0);
            sine.step = (target - sine.amplitude) / BLOCK as f64;
            if level < -140.0 && t > last {
                sine.done = true;
            }
            // Renormalised now and then, so rounding does not grow or shrink it.
            let size = (sine.phasor.0 * sine.phasor.0 + sine.phasor.1 * sine.phasor.1).sqrt();
            sine.phasor = (sine.phasor.0 / size, sine.phasor.1 / size);
        }
        for band in self.bands.iter_mut().filter(|b| !b.done) {
            let power = self.voice.noise[band.shape]
                .1
                .power_at(&self.voice.times, t);
            let rising = if t >= band.rise {
                1.0
            } else {
                0.5 - 0.5 * (std::f64::consts::PI * t / band.rise).cos()
            };
            let target = rising * power.sqrt() * band.scale;
            band.step = (target - band.amplitude) / BLOCK as f64;
            if power < 1e-14 && t > last {
                band.done = true;
            }
        }
    }
}

impl AudioNode for Struck {
    const ID: u64 = 0x6d78_0070;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.warm = warm_bands(&self.voice, sample_rate);
        self.sines = Vec::with_capacity(self.voice.partials.capacity());
        self.bands = Vec::with_capacity(self.warm.len());
        self.start();
    }

    fn reset(&mut self) {
        self.set_sample_rate(self.rate);
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        self.wanted = (input[0] as f64, input[1] as f64);
        if self.sample == 0 && self.refill.is_some() {
            self.refill();
        }
        if self.sample == 0 && self.wanted != self.applied {
            self.read();
        }
        if self.sample > 0 && self.sample.is_multiple_of(BLOCK) {
            self.read();
        }
        let mut sum = 0.0;
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            sum += sine.amplitude * sine.phasor.1;
            let (re, im) = sine.phasor;
            sine.phasor = (
                re * sine.turn.0 - im * sine.turn.1,
                re * sine.turn.1 + im * sine.turn.0,
            );
            sine.amplitude += sine.step;
        }
        for band in self.bands.iter_mut().filter(|b| !b.done) {
            sum += band.amplitude * band.filtered();
            band.amplitude += band.step;
        }
        self.sample += 1;
        [sum as f32 * self.gain].into()
    }
}
