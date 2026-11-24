//! Synths and effects, each a name, the options it declares, and a function that builds its sound.

use crate::nodes::{gated, onset};
use crate::options::{Context, Options};
use crate::spec::Spec;
use fundsp::prelude::*;
use std::sync::Arc;

pub(crate) type BuildNote = dyn Fn(&Options, &mut Context) -> Note + Send + Sync;
type BuildEffect = dyn Fn(&Options, &mut Context) -> Net + Send + Sync;
type Tail = dyn Fn(&Options) -> f64 + Send + Sync;

/// A synth: a name, its options, and how it builds one note's sound.
/// [`SynthDecl::new`] starts declaring one in Rust, and [`graph::synth`](crate::graph::synth) builds one from data; [the guide to building a synth](crate::guide::building) shows both.
#[derive(Clone)]
pub struct Synth {
    pub(crate) name: String,
    pub(crate) doc: String,
    pub(crate) specs: Vec<Spec>,
    pub(crate) build: Arc<BuildNote>,
    /// The files of the measured tables the synth plays, which a build without the `tables` feature loads first.
    pub(crate) tables: &'static [&'static str],
    pub(crate) family: Family,
    pub(crate) range: Option<(f64, f64)>,
}

/// The family a synth belongs to, as a musician would look for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// The piano, the electric piano, the organs and the accordion.
    Keys,
    /// The guitars, basses, harp and the plucked strings of Brazil and India.
    PluckedStrings,
    /// The violin, viola, cello and double bass.
    BowedStrings,
    /// The woodwinds and the brass.
    Winds,
    /// The marimba, vibraphone, xylophone, glockenspiel and timpani.
    Mallets,
    /// The waveforms and the synthesizer sounds.
    Synths,
    /// The drum kit and the percussion.
    Drums,
}

impl Family {
    /// Every family, in the order a list of synths shows them.
    pub const ALL: [Family; 7] = [
        Family::Keys,
        Family::PluckedStrings,
        Family::BowedStrings,
        Family::Winds,
        Family::Mallets,
        Family::Synths,
        Family::Drums,
    ];

    /// The family's name in lower case, such as `plucked strings`.
    pub fn name(self) -> &'static str {
        match self {
            Family::Keys => "keys",
            Family::PluckedStrings => "plucked strings",
            Family::BowedStrings => "bowed strings",
            Family::Winds => "winds",
            Family::Mallets => "mallets",
            Family::Synths => "synths",
            Family::Drums => "drums",
        }
    }

    /// The family's name as a heading, such as `Plucked strings`.
    pub fn title(self) -> &'static str {
        match self {
            Family::Keys => "Keys",
            Family::PluckedStrings => "Plucked strings",
            Family::BowedStrings => "Bowed strings",
            Family::Winds => "Winds",
            Family::Mallets => "Mallets",
            Family::Synths => "Synths",
            Family::Drums => "Drums",
        }
    }
}

impl std::fmt::Display for Family {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A synth being declared, until `build` gives it its sound.
pub struct SynthDecl {
    name: String,
    doc: String,
    specs: Vec<Spec>,
}

impl Synth {
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

    /// The files of the measured tables the synth plays, such as `piano.mxf`, none for most synths.
    /// A build of mxs without the `tables` feature loads each with [`load_table`](crate::synths::load_table) before the synth plays.
    pub fn tables(&self) -> &[&'static str] {
        self.tables
    }

    /// The synth's family; a synth declared outside the crate is one of the synths.
    pub fn family(&self) -> Family {
        self.family
    }

    /// The lowest and highest MIDI notes the synth is meant to play, or none for a synth with no natural range, such as `saw` or a drum.
    /// A note outside the range still plays.
    pub fn range(&self) -> Option<(f64, f64)> {
        self.range
    }
}

impl SynthDecl {
    /// Starts declaring a synth that notes play by `name`; declare its options, then `build` it.
    pub fn new(name: &str) -> SynthDecl {
        SynthDecl {
            name: name.to_string(),
            doc: String::new(),
            specs: Vec::new(),
        }
    }

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
            tables: &[],
            family: Family::Synths,
            range: None,
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
/// [`EffectDecl::new`] starts declaring one, as [the guide to building a synth](crate::guide::building) shows.
#[derive(Clone)]
pub struct Effect {
    pub(crate) name: String,
    pub(crate) specs: Vec<Spec>,
    pub(crate) tail: Arc<Tail>,
    pub(crate) late: usize,
    pub(crate) build: Arc<BuildEffect>,
}

/// An effect being declared, until `build` gives it its sound.
pub struct EffectDecl {
    name: String,
    specs: Vec<Spec>,
    tail: Arc<Tail>,
    late: usize,
}

impl Effect {
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
    /// Starts declaring an effect that chains use by `name`; declare its options, then `build` it.
    pub fn new(name: &str) -> EffectDecl {
        EffectDecl {
            name: name.to_string(),
            specs: Vec::new(),
            tail: Arc::new(|_| 1.0),
            late: 0,
        }
    }

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

    /// How many frames after its input the effect's sound comes out, 0 unless given; the dry sound it is mixed with is held back by as many, so the two line up.
    pub fn late(mut self, frames: usize) -> Self {
        self.late = frames;
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
            late: self.late,
            build: Arc::new(build),
        }
    }
}

fn net(sound: impl AudioUnit + 'static) -> Net {
    let boxed: Box<dyn AudioUnit> = Box::new(sound);
    Net::wrap(boxed)
}
