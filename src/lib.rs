use fundsp::prelude::*;
use std::collections::HashMap;

mod berimbau;
mod cello;
mod piano;

pub const RATE: f64 = 48_000.0;

/// A note's options by name, as mx sends them to scsynth.
#[derive(Clone, Default, Debug)]
pub struct Opts(pub HashMap<String, f64>);

impl Opts {
    pub fn get(&self, name: &str, default: f64) -> f64 {
        self.0.get(name).copied().unwrap_or(default)
    }

    /// Parses `name=value` pairs.
    pub fn parse<'a>(args: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut opts = HashMap::new();
        for arg in args {
            let (name, value) = arg.split_once('=').ok_or(format!("expected name=value, got {arg}"))?;
            let value = value.parse().map_err(|_| format!("not a number: {arg}"))?;
            opts.insert(name.to_string(), value);
        }
        Ok(Self(opts))
    }
}

/// A note of one of the engine's synths, mono, as the left channel at the centre.
pub fn synth(name: &str, opts: &Opts, rate: f64) -> Option<Box<dyn AudioUnit>> {
    match name {
        "piano" => Some(piano::piano(opts, rate)),
        "piano.static" => Some(piano::piano_static(opts, rate)),
        "cello.bowed" => Some(cello::cello(opts, rate)),
        "berimbau" => Some(berimbau::berimbau(opts, rate)),
        "berimbau.static" => Some(berimbau::berimbau_static(opts, rate)),
        _ => None,
    }
}

/// The note's envelope, all times in seconds: attack, decay to decay_level, hold at sustain_level for sustain, then release to silence.
/// Its fade shapes are fitted by measurement, not taken from scsynth.
pub fn note_envelope(opts: &Opts, sustain: f64, release: f64) -> An<impl AudioNode<Inputs = U0, Outputs = U1> + use<>> {
    let attack = opts.get("attack", 0.0);
    let decay = opts.get("decay", 0.0);
    let sustain = opts.get("sustain", sustain);
    let release = opts.get("release", release);
    let peak = opts.get("attack_level", 1.0);
    let held = opts.get("sustain_level", 1.0);
    let decayed = Some(opts.get("decay_level", -1.0)).filter(|&l| l >= 0.0).unwrap_or(held);
    envelope(move |t: f64| {
        let ease = |from: f64, to: f64, x: f64, power: i32| to + (from - to) * (1.0 - x.clamp(0.0, 1.0)).powi(power);
        let level = if t < attack {
            peak * t / attack
        } else if t < attack + decay {
            ease(peak, decayed, (t - attack) / decay, 2)
        } else if t < attack + decay + sustain {
            decayed + (held - decayed) * (t - attack - decay) / sustain
        } else {
            ease(held, 0.0, (t - attack - decay - sustain) / release, 3)
        };
        level
    })
}

/// Renders some seconds of a note offline.
pub fn render(name: &str, opts: &Opts, seconds: f64) -> Option<Vec<f32>> {
    let mut unit = synth(name, opts, RATE)?;
    unit.set_sample_rate(RATE);
    let frames = (seconds * RATE).round() as usize;
    Some((0..frames).map(|_| unit.get_mono()).collect())
}

/// A resonator struck by its input, which an impulse sets ringing from `phase` at `level`, fading 60 dB over `fade` seconds.
pub(crate) fn struck(hz: f64, phase: f64, level: f64, fade: f64, rate: f64) -> Net {
    let (a1, a2, b0, b1) = struck_coefficients(hz, phase, level, fade, rate);
    Net::wrap(Box::new(biquad::<f64>(a1, a2, b0, b1, 0.0)))
}

/// The biquad coefficients of `struck`, as `(a1, a2, b0, b1)`.
pub(crate) fn struck_coefficients(hz: f64, phase: f64, level: f64, fade: f64, rate: f64) -> (f64, f64, f64, f64) {
    let w = 2.0 * std::f64::consts::PI * hz / rate;
    let r = 1000f64.powf(-1.0 / (fade * rate));
    // Its impulse response is level * r^n * sin((n + 1) * w + phase).
    (-2.0 * r * w.cos(), r * r, level * (w + phase).sin(), -level * r * phase.sin())
}

/// An envelope from `f(t)`, sampled every 20 microseconds rather than every 2 milliseconds as FunDSP's envelope is, for shapes of a few milliseconds.
pub(crate) fn fast_envelope<E>(f: E) -> An<fundsp::envelope::Envelope<f32, E, f32>>
where
    E: FnMut(f32) -> f32 + Clone + Send + Sync,
{
    An(fundsp::envelope::Envelope::new(2e-5, f))
}

/// A percussive envelope: a linear rise over `attack`, then a fall to silence over `release`.
pub(crate) fn perc(t: f32, attack: f32, release: f32) -> f32 {
    if t < attack {
        t / attack
    } else {
        (1.0 - (t - attack) / release).max(0.0).powi(4)
    }
}

pub(crate) fn midi_hz(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

pub(crate) fn db_amp(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}
