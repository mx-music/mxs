//! A note's options as its synth's build function reads them, and the context it builds in.

use crate::spec::{Kind, Spec, Value};
use fundsp::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A note's options: the values the note was given, checked and held to their ranges, and the declared defaults for the rest.
pub struct Options {
    pub(crate) owner: Arc<str>,
    pub(crate) specs: Arc<[Spec]>,
    pub(crate) values: HashMap<String, Value>,
    pub(crate) controls: Mutex<HashMap<String, Control>>,
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
        }
    }

    /// Turns the options given in beats into seconds, each beat lasting `per_beat` seconds: those declared in beats, their defaults too, and every `_slide`.
    pub(crate) fn in_beats(&mut self, per_beat: f64) {
        for spec in self.specs.iter().filter(|spec| spec.kind == Kind::Beats) {
            let value = self.values.get(&spec.name).or(spec.default.as_ref());
            if let Some(Value::Number(beats)) = value {
                let seconds = Value::Number(beats * per_beat);
                self.values.insert(spec.name.clone(), seconds);
            }
        }
        let specs = &self.specs;
        let declared = |name: &str| {
            specs
                .iter()
                .any(|s| s.name == name && s.kind == Kind::Beats)
        };
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
        self.values
            .get(name)
            .or(spec.default.as_ref())
            .unwrap_or_else(|| {
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

    pub(crate) fn take_controls(&self) -> HashMap<String, Control> {
        std::mem::take(&mut self.controls.lock().unwrap())
    }
}

/// What a build function needs besides the note's options.
pub struct Context {
    owner: Arc<str>,
    count: u64,
}

impl Context {
    pub(crate) fn new(owner: Arc<str>) -> Self {
        Context { owner, count: 0 }
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
