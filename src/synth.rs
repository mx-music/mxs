//! Synths and effects, each a name, the options it declares, and a function that builds its sound.

use crate::nodes::{gated, onset};
use crate::options::{Context, Options};
use crate::spec::Spec;
use fundsp::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub(crate) type BuildNote = dyn Fn(&Options, &mut Context) -> Note + Send + Sync;
type BuildEffect = dyn Fn(&Options, &mut Context) -> Net + Send + Sync;
type Tail = dyn Fn(&Options) -> f64 + Send + Sync;

/// A synth: a name, its options, and how it builds one note's sound.
/// [`Synth::new`] starts declaring one in Rust, and [`Synth::graph`] builds one from data; [the guide to building a synth](crate::guide::building) shows both.
#[derive(Clone)]
pub struct Synth {
    pub(crate) name: String,
    pub(crate) doc: String,
    pub(crate) specs: Vec<Spec>,
    pub(crate) build: Arc<BuildNote>,
}

/// A synth being declared, until `build` gives it its sound.
pub struct SynthDecl {
    name: String,
    doc: String,
    specs: Vec<Spec>,
}

impl Synth {
    /// Starts declaring a synth that notes play by `name`; declare its options, then `build` it.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(name: &str) -> SynthDecl {
        SynthDecl {
            name: name.to_string(),
            doc: String::new(),
            specs: Vec::new(),
        }
    }

    /// The name notes play the synth by.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// What the synth is, in a sentence or two, or nothing when its declaration gave none.
    pub fn doc(&self) -> &str {
        &self.doc
    }

    /// The options the synth takes.
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }
}

impl SynthDecl {
    /// What the synth is, for whoever chooses one.
    pub fn doc(mut self, doc: &str) -> Self {
        self.doc = doc.to_string();
        self
    }

    /// Declares one option.
    pub fn option(mut self, spec: Spec) -> Self {
        self.specs.push(spec);
        self
    }

    /// Declares several options.
    pub fn options(mut self, specs: impl IntoIterator<Item = Spec>) -> Self {
        self.specs.extend(specs);
        self
    }

    /// Gives the synth its sound: `build` returns the sound of a note from the options it was played with.
    pub fn build<F>(self, build: F) -> Synth
    where
        F: Fn(&Options, &mut Context) -> Note + Send + Sync + 'static,
    {
        Synth {
            name: self.name,
            doc: self.doc,
            specs: self.specs,
            build: Arc::new(build),
        }
    }
}

/// One note's sound, mono or stereo, and the seconds after which it is silent, which the engine needs when it starts the note.
pub struct Note {
    pub(crate) sound: Net,
    pub(crate) end: f64,
    /// The gate that lets the note go before its envelope ends, and the seconds it then takes to fall silent.
    pub(crate) release: Option<(Shared, f64)>,
    /// Above 0 while the envelope holds at the end of its decay until it is let go.
    pub(crate) held: Option<Shared>,
}

/// What the engine gives a voice built ready as its note starts: the note and vel it plays, whether it is held, and its pedal, 0 or 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Given {
    pub note: f64,
    pub vel: f64,
    pub held: bool,
    pub pedal: f64,
}

/// The hold the engine keeps on a voice built ready: what it is given, written before it starts, and how many seconds a note given that lasts and takes to fall silent once let go.
pub(crate) struct Ready {
    note: AtomicU64,
    vel: AtomicU64,
    held: AtomicBool,
    pedal: AtomicU64,
    lasts: Box<dyn Fn(Given) -> (f64, f64) + Send + Sync>,
}

impl Ready {
    pub(crate) fn new(lasts: impl Fn(Given) -> (f64, f64) + Send + Sync + 'static) -> Arc<Ready> {
        Arc::new(Ready {
            note: AtomicU64::new(f64::NAN.to_bits()),
            vel: AtomicU64::new(0),
            held: AtomicBool::new(false),
            pedal: AtomicU64::new(0),
            lasts: Box::new(lasts),
        })
    }

    /// Gives the voice what it plays, before the engine sends it on, and says how many seconds it lasts and takes to fall silent once let go.
    pub(crate) fn give(&self, given: Given) -> (f64, f64) {
        self.note.store(given.note.to_bits(), Ordering::Relaxed);
        self.vel.store(given.vel.to_bits(), Ordering::Relaxed);
        self.held.store(given.held, Ordering::Relaxed);
        self.pedal.store(given.pedal.to_bits(), Ordering::Relaxed);
        (self.lasts)(given)
    }

    /// What was given, or `None` before anything is.
    pub(crate) fn given(&self) -> Option<Given> {
        let note = f64::from_bits(self.note.load(Ordering::Relaxed));
        (!note.is_nan()).then(|| Given {
            note,
            vel: f64::from_bits(self.vel.load(Ordering::Relaxed)),
            held: self.held.load(Ordering::Relaxed),
            pedal: f64::from_bits(self.pedal.load(Ordering::Relaxed)),
        })
    }
}

impl Note {
    /// A sound that rings until it fades by itself, after `seconds`.
    pub fn rings(sound: impl AudioUnit + 'static, seconds: f64) -> Note {
        Note {
            sound: net(sound),
            end: seconds,
            release: None,
            held: None,
        }
    }

    /// A sound shaped by the note's envelope, from its options attack, decay, sustain, release, attack_level, decay_level and sustain_level, which ends when the envelope does, or falls silent over its release once let go.
    /// A held note stays at decay_level after its decay, leaving its sustain unread, until it is let go.
    pub fn envelope(sound: impl AudioUnit + 'static, options: &Options) -> Note {
        let attack = options.number("attack");
        let decay = options.number("decay");
        let sustain = options.number("sustain");
        let release = options.number("release");
        let peak = options.number("attack_level");
        let held = options.number("sustain_level");
        let decayed = Some(options.number("decay_level"))
            .filter(|&l| l >= 0.0)
            .unwrap_or(held);
        // The same curves as mx's envelope on scsynth: a slow attack, a quick decay, a straight sustain and a quick release.
        let levels = [0.0, peak, decayed, held, 0.0];
        let times = [attack, decay, sustain, release];
        let gate = shared(1.0);
        let holding = shared(if options.held() { 1.0 } else { 0.0 });
        let level =
            net(var(&gate)) >> gated(&levels, &times, &[1.0, -2.0, 0.0, -3.0], &holding) >> onset();
        let sound = net(sound);
        let sound = if sound.outputs() == 2 {
            sound * (level >> split::<U2>())
        } else {
            sound * level
        };
        let end = if options.held() {
            f64::INFINITY
        } else {
            Note::envelope_seconds(options)
        };
        Note {
            sound,
            end,
            release: Some((gate, release)),
            held: Some(holding),
        }
    }

    /// The seconds the envelope of a note with `options` lasts, let go at its end.
    pub(crate) fn envelope_seconds(options: &Options) -> f64 {
        options.number("attack")
            + options.number("decay")
            + options.number("sustain")
            + options.number("release")
    }

    /// Ends the note after `seconds`, if it would end later.
    pub fn within(mut self, seconds: f64) -> Note {
        self.end = self.end.min(seconds);
        self
    }
}

/// An effect: a name, its options, how long it sounds on after its input stops, and how it builds a stereo sound from a stereo input.
/// [`Effect::new`] starts declaring one, as [the guide to building a synth](crate::guide::building) shows.
#[derive(Clone)]
pub struct Effect {
    pub(crate) name: String,
    pub(crate) specs: Vec<Spec>,
    pub(crate) tail: Arc<Tail>,
    pub(crate) build: Arc<BuildEffect>,
}

/// An effect being declared, until `build` gives it its sound.
pub struct EffectDecl {
    name: String,
    specs: Vec<Spec>,
    tail: Arc<Tail>,
}

impl Effect {
    /// Starts declaring an effect that chains use by `name`; declare its options, then `build` it.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(name: &str) -> EffectDecl {
        EffectDecl {
            name: name.to_string(),
            specs: Vec::new(),
            tail: Arc::new(|_| 1.0),
        }
    }

    /// The name chains use the effect by.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The options the effect takes.
    pub fn specs(&self) -> &[Spec] {
        &self.specs
    }
}

impl EffectDecl {
    /// Declares one option.
    pub fn option(mut self, spec: Spec) -> Self {
        self.specs.push(spec);
        self
    }

    /// How many seconds the effect sounds on after its input stops, such as a reverb's tail.
    pub fn tail(mut self, seconds: f64) -> Self {
        self.tail = Arc::new(move |_| seconds);
        self
    }

    /// How many seconds the effect sounds on after its input stops, worked out from its options, such as an echo's decay.
    pub fn tail_from<F>(mut self, tail: F) -> Self
    where
        F: Fn(&Options) -> f64 + Send + Sync + 'static,
    {
        self.tail = Arc::new(tail);
        self
    }

    /// Gives the effect its sound: `build` returns the net that processes the sound, from the options the effect was started with.
    pub fn build<F>(self, build: F) -> Effect
    where
        F: Fn(&Options, &mut Context) -> Net + Send + Sync + 'static,
    {
        Effect {
            name: self.name,
            specs: self.specs,
            tail: self.tail,
            build: Arc::new(build),
        }
    }
}

fn net(sound: impl AudioUnit + 'static) -> Net {
    let boxed: Box<dyn AudioUnit> = Box::new(sound);
    Net::wrap(boxed)
}
