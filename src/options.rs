//! A note's options as its synth's build function reads them, and the context it builds in.

use crate::spec::{Spec, Value};
use fundsp::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A note's options: the values the note was given, checked and held to their ranges, and the declared defaults for the rest.
pub struct Options {
    pub(crate) owner: Arc<str>,
    pub(crate) specs: Arc<[Spec]>,
    values: HashMap<String, Value>,
    controls: Mutex<HashMap<String, Control>>,
    held: bool,
    /// Where an effect reads the chain its chain is keyed by.
    key: Option<Arc<KeyTap>>,
}

/// The block a chain is keyed by: both sides of the output of the chain that keys it, which the mixer writes before the keyed chain plays.
pub(crate) struct KeyTap {
    on: std::sync::atomic::AtomicBool,
    block: std::sync::atomic::AtomicU64,
    samples: [[std::sync::atomic::AtomicU32; fundsp::MAX_BUFFER_SIZE]; 2],
}

impl KeyTap {
    pub(crate) fn new() -> KeyTap {
        KeyTap {
            on: std::sync::atomic::AtomicBool::new(false),
            block: std::sync::atomic::AtomicU64::new(0),
            samples: std::array::from_fn(|_| std::array::from_fn(|_| Default::default())),
        }
    }

    /// Says whether a chain keys this one, and starts a block of silence.
    pub(crate) fn start_block(&self, on: bool) {
        use std::sync::atomic::Ordering::Relaxed;
        self.on.store(on, Relaxed);
        self.block.fetch_add(1, Relaxed);
        for side in &self.samples {
            for sample in side {
                sample.store(0, Relaxed);
            }
        }
    }

    pub(crate) fn write(&self, frame: usize, left: f32, right: f32) {
        use std::sync::atomic::Ordering::Relaxed;
        self.samples[0][frame].store(left.to_bits(), Relaxed);
        self.samples[1][frame].store(right.to_bits(), Relaxed);
    }
}

/// Both sides of the chain that keys this one, and 1 while one does, 0 otherwise.
#[derive(Clone)]
pub(crate) struct Keyed {
    tap: Arc<KeyTap>,
    block: u64,
    frame: usize,
}

impl fundsp::prelude::AudioNode for Keyed {
    const ID: u64 = 0x6d78_007b;
    type Inputs = fundsp::prelude::U0;
    type Outputs = fundsp::prelude::U3;

    fn tick(
        &mut self,
        _input: &fundsp::prelude::Frame<f32, Self::Inputs>,
    ) -> fundsp::prelude::Frame<f32, Self::Outputs> {
        use std::sync::atomic::Ordering::Relaxed;
        let block = self.tap.block.load(Relaxed);
        if block != self.block {
            (self.block, self.frame) = (block, 0);
        }
        let frame = Ord::min(self.frame, fundsp::MAX_BUFFER_SIZE - 1);
        self.frame += 1;
        let side = |side: usize| f32::from_bits(self.tap.samples[side][frame].load(Relaxed));
        let on = if self.tap.on.load(Relaxed) { 1.0 } else { 0.0 };
        [side(0), side(1), on].into()
    }
}

/// The value an option glides to and the seconds it takes, which the engine sets while the note sounds.
#[derive(Clone)]
pub(crate) struct Control {
    pub target: Shared,
    pub slide: Shared,
}

impl Options {
    pub(crate) fn new(owner: Arc<str>, specs: Arc<[Spec]>, values: HashMap<String, Value>) -> Self {
        Options {
            owner,
            specs,
            values,
            controls: Mutex::new(HashMap::new()),
            held: false,
            key: None,
        }
    }

    /// The same options, for a note that sounds until it is let go when `held`.
    pub(crate) fn holding(mut self, held: bool) -> Self {
        self.held = held;
        self
    }

    /// The same options, for an effect whose chain is keyed through `tap`.
    pub(crate) fn keyed_by(mut self, tap: Arc<KeyTap>) -> Self {
        self.key = Some(tap);
        self
    }

    /// Whether the note sounds until it is let go, its sustain left unread, as [`Event::held`](crate::Event::held) starts it.
    pub fn held(&self) -> bool {
        self.held
    }

    /// Each option declared in beats, with the beats it was given or its default.
    pub(crate) fn beats(&self) -> Vec<(String, f64)> {
        let mut beats = Vec::new();
        for spec in self.specs.iter().filter(|spec| spec.counts_beats()) {
            if let Some(Value::Number(given)) = spec.value_in(&self.values) {
                beats.push((spec.name.clone(), *given));
            }
        }
        beats
    }

    /// Turns the options given in beats into seconds, each beat lasting `per_beat` seconds: those declared in beats, their defaults too, and every `_slide`.
    pub(crate) fn in_beats(&mut self, per_beat: f64) {
        for spec in self.specs.iter().filter(|spec| spec.counts_beats()) {
            let value = spec.value_in(&self.values);
            if let Some(Value::Number(beats)) = value {
                let seconds = Value::Number(beats * per_beat);
                self.values.insert(spec.name.clone(), seconds);
            }
        }
        let specs = &self.specs;
        let declared = |name: &str| specs.iter().any(|s| s.name == name && s.counts_beats());
        for (name, value) in &mut self.values {
            if let (true, false, Value::Number(beats)) =
                (name.ends_with("_slide"), declared(name), value)
            {
                *beats *= per_beat;
            }
        }
    }

    pub(crate) fn value(&self, name: &str) -> &Value {
        let spec = self.specs.iter().find(|s| s.name == name);
        let spec = spec.unwrap_or_else(|| {
            panic!(
                "{} reads the option {name}, which it does not declare",
                self.owner
            )
        });
        spec.value_in(&self.values).unwrap_or_else(|| {
            panic!(
                "{} reads the option {name}, which has no default and was not given",
                self.owner
            )
        })
    }

    /// An option's value when the note starts.
    pub fn number(&self, name: &str) -> f64 {
        match self.value(name) {
            Value::Number(x) => *x,
            Value::Name(n) => panic!(
                "{} reads the option {name} as a number, but it is the name {n}",
                self.owner
            ),
        }
    }

    /// An option's value when the note starts, for an option that takes one of a list of names.
    pub fn name(&self, name: &str) -> &str {
        match self.value(name) {
            Value::Name(n) => n,
            Value::Number(x) => panic!(
                "{} reads the option {name} as a name, but it is the number {x}",
                self.owner
            ),
        }
    }

    /// An option's value when the note gave one, for an option declared without a default.
    pub fn given(&self, name: &str) -> Option<f64> {
        if !self.specs.iter().any(|s| s.name == name) {
            panic!(
                "{} reads the option {name}, which it does not declare",
                self.owner
            );
        }
        match self.values.get(name) {
            Some(Value::Number(x)) => Some(*x),
            _ => None,
        }
    }

    /// A signal of an option's value, which glides to each new value a change gives while the note sounds, over the option's `_slide` seconds.
    pub fn follow(&self, name: &str) -> Net {
        let start = self.number(name) as f32;
        let slide = self
            .values
            .get(&format!("{name}_slide"))
            .map_or(0.0, |v| match v {
                Value::Number(x) => *x as f32,
                Value::Name(_) => 0.0,
            });
        let mut controls = self.controls.lock().unwrap();
        let control = controls
            .entry(name.to_string())
            .or_insert_with(|| Control {
                target: Shared::new(start),
                slide: Shared::new(slide),
            })
            .clone();
        Net::wrap(Box::new(An(Glide::new(control, start))))
    }

    /// Both sides of the chain that keys the effect's chain, as [`Engine::key`](crate::Engine::key) sets it, and a third signal that is 1 while one does and 0 otherwise.
    /// It is silence in a note.
    pub fn key(&self) -> Net {
        match &self.key {
            Some(tap) => Net::wrap(Box::new(fundsp::prelude::An(Keyed {
                tap: tap.clone(),
                block: 0,
                frame: 0,
            }))),
            None => Net::wrap(Box::new(fundsp::prelude::dc((0.0, 0.0, 0.0)))),
        }
    }

    /// The options the built sound follows, which the engine changes while it plays; taking them leaves none.
    pub(crate) fn take_controls(&self) -> Controls {
        Controls(std::mem::take(&mut self.controls.lock().unwrap()))
    }
}

/// The options a sound follows while it plays, each by the control the engine changes it through.
#[derive(Default)]
pub(crate) struct Controls(HashMap<String, Control>);

impl Controls {
    /// Whether the sound follows `name`.
    pub(crate) fn follows(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }

    /// The control the sound follows `name` through.
    pub(crate) fn get(&self, name: &str) -> Option<&Control> {
        self.0.get(name)
    }

    /// Sets `name` to `value` at once, when the sound follows it.
    pub(crate) fn set(&self, name: &str, value: f32) {
        if let Some(control) = self.0.get(name) {
            control.target.set(value);
        }
    }
}

/// What a build function needs besides the note's options.
pub struct Context {
    owner: Arc<str>,
    count: u64,
    /// Whether the note is built to be kept ready, and the hold on it once built so.
    ready: (bool, Option<Arc<crate::ready::Ready>>),
}

impl Context {
    pub(crate) fn new(owner: Arc<str>) -> Self {
        Context {
            owner,
            count: 0,
            ready: (false, None),
        }
    }

    /// A context that asks for the note to be built so the engine can keep it ready and give it another note and vel before it starts.
    pub(crate) fn ready(owner: Arc<str>) -> Self {
        Context {
            ready: (true, None),
            ..Context::new(owner)
        }
    }

    pub(crate) fn asks_ready(&self) -> bool {
        self.ready.0
    }

    pub(crate) fn built_ready(&mut self, hold: Arc<crate::ready::Ready>) {
        self.ready.1 = Some(hold);
    }

    /// The hold on the note built, when it was built ready.
    pub(crate) fn take_ready(&mut self) -> Option<Arc<crate::ready::Ready>> {
        self.ready.1.take()
    }

    /// A seed for a random node, from the synth's name and how many seeds it asked for before, so a note sounds the same on every run.
    pub fn seed(&mut self) -> u64 {
        self.count += 1;
        // FNV-1a, whose output does not change between Rust releases as std's hasher may.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in self
            .owner
            .as_bytes()
            .iter()
            .chain(&self.count.to_le_bytes())
        {
            hash = (hash ^ *byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
        hash
    }
}

/// Moves in a straight line to each new target, over the slide time it finds when the target changes.
#[derive(Clone)]
pub(crate) struct Glide {
    control: Control,
    aim: f32,
    current: f32,
    step: f32,
    left: u64,
    rate: f64,
}

impl Glide {
    fn new(control: Control, start: f32) -> Self {
        Glide {
            control,
            aim: start,
            current: start,
            step: 0.0,
            left: 0,
            rate: DEFAULT_SR,
        }
    }
}

impl AudioNode for Glide {
    const ID: u64 = 0x6d78_0001;
    type Inputs = U0;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
    }

    fn tick(&mut self, _input: &Frame<f32, U0>) -> Frame<f32, U1> {
        let target = self.control.target.value();
        if target != self.aim {
            self.aim = target;
            let frames = (self.control.slide.value() as f64 * self.rate)
                .round()
                .max(0.0) as u64;
            if frames == 0 {
                self.current = target;
                self.left = 0;
            } else {
                self.step = (target - self.current) / frames as f32;
                self.left = frames;
            }
        }
        if self.left > 0 {
            self.current += self.step;
            self.left -= 1;
            if self.left == 0 {
                self.current = self.aim;
            }
        }
        [self.current].into()
    }
}
