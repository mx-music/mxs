//! The engine: synths and effects registered by name, samples loaded from files, and events stamped with seconds, played live or rendered offline.

use crate::mixer::{CAPACITY, Chain, Change, Command, Garbage, Mixer, Voice};
use crate::options::{Context, Control, Options};
use crate::sample::{Player, SampleData};
use crate::spec::{Spec, Value};
use crate::synth::{Effect, Note, Synth};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use fundsp::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

/// Seconds of silence a note's end fades over, so a note cut at its end does not click.
const END_FADE: f64 = 0.01;

/// What a change applies to.
#[derive(Clone, Debug)]
pub enum Target {
    /// The notes and samples started with these ids.
    Sounds(Vec<u64>),
    /// Every note and sample sounding in an effect chain.
    Part(u64),
    /// One effect of a chain, by its index.
    Effect(u64, usize),
}

/// Something to play at a time.
#[derive(Clone, Debug)]
pub enum Event {
    /// A note of a synth; `id` lets a later change reach it, and `chain` is the effect chain it plays into.
    Note { id: Option<u64>, synth: String, options: Vec<(String, Value)>, chain: Option<u64> },
    /// A sample `load_sample` returned, with the sample player's options.
    Sample { id: Option<u64>, sample: u64, options: Vec<(String, Value)>, chain: Option<u64> },
    /// New values for options of sounds already playing, each gliding over its `_slide` option when given.
    Set { target: Target, options: Vec<(String, f64)> },
}

impl Event {
    pub fn note(id: Option<u64>, synth: &str, options: &[(&str, f64)]) -> Event {
        let options = options.iter().map(|(k, v)| (k.to_string(), Value::Number(*v))).collect();
        Event::Note { id, synth: synth.to_string(), options, chain: None }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    UnknownSynth(String),
    UnknownEffect(String),
    UnknownSample(u64),
    UnknownChain(u64),
    UnknownOption { owner: String, option: String },
    BadValue { owner: String, option: String, reason: String },
    MissingOption { owner: String, option: String },
    BadSound { owner: String, outputs: usize },
    Sample(String),
    Device(String),
    Busy,
    Offline,
    Live,
    Failed,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownSynth(name) => write!(f, "no synth named {name}"),
            Error::UnknownEffect(name) => write!(f, "no effect named {name}"),
            Error::UnknownSample(id) => write!(f, "no sample loaded as {id}"),
            Error::UnknownChain(id) => write!(f, "no effect chain {id}"),
            Error::UnknownOption { owner, option } => write!(f, "{owner} has no option {option}"),
            Error::BadValue { owner, option, reason } => write!(f, "{owner}'s option {option} {reason}"),
            Error::MissingOption { owner, option } => write!(f, "{owner} needs the option {option}"),
            Error::BadSound { owner, outputs } => write!(f, "{owner} made a sound of {outputs} channels, not 1 or 2"),
            Error::Sample(reason) => write!(f, "{reason}"),
            Error::Device(reason) => write!(f, "{reason}"),
            Error::Busy => write!(f, "the engine has more events waiting than it can take"),
            Error::Offline => write!(f, "this engine renders offline and has no output"),
            Error::Live => write!(f, "this engine plays live and cannot render"),
            Error::Failed => write!(f, "the engine failed while filling audio, and has to be started again"),
        }
    }
}

impl std::error::Error for Error {}

struct Registered<T> {
    item: T,
    owner: Arc<str>,
    specs: Arc<[Spec]>,
}

struct Sound {
    chain: Option<u64>,
    start: f64,
    end: f64,
    controls: HashMap<String, Control>,
}

enum Output {
    Offline(Box<Mixer>),
    Live { stop: mpsc::Sender<()> },
}

/// The engine, live on the default output or offline.
pub struct Engine {
    rate: f64,
    synths: HashMap<String, Registered<Synth>>,
    effects: HashMap<String, Registered<Effect>>,
    sampler: (Arc<str>, Arc<[Spec]>),
    samples: HashMap<u64, Arc<SampleData>>,
    commands: rtrb::Producer<Command>,
    garbage: rtrb::Consumer<Garbage>,
    sounds: HashMap<u64, Sound>,
    ids: HashMap<u64, u64>,
    chains: HashMap<u64, Vec<HashMap<String, Control>>>,
    next: u64,
    warned: HashSet<(String, String)>,
    warnings: Vec<String>,
    clock: Arc<AtomicU64>,
    failed: Arc<AtomicBool>,
    output: Output,
}

impl Engine {
    /// An engine that renders offline at `rate` frames a second.
    pub fn offline(rate: f64) -> Engine {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let clock = Arc::new(AtomicU64::new(0));
        let mixer = Mixer::new(receiving, returning, clock.clone());
        Engine::with(rate, commands, garbage, clock, Arc::new(AtomicBool::new(false)), Output::Offline(Box::new(mixer)))
    }

    /// An engine that plays through the default output device at its own rate.
    pub fn live() -> Result<Engine, Error> {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let clock = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let mixer = Mixer::new(receiving, returning, clock.clone());
        let (stop, stopped) = mpsc::channel::<()>();
        let (ready, started) = mpsc::channel::<Result<f64, String>>();
        let failing = failed.clone();
        // cpal's stream cannot move between threads on every platform, so one thread owns it until the engine is dropped.
        std::thread::spawn(move || match open(mixer, failing) {
            Ok((stream, rate)) => {
                let _ = ready.send(Ok(rate));
                let _ = stopped.recv();
                drop(stream);
            }
            Err(reason) => {
                let _ = ready.send(Err(reason));
            }
        });
        let rate = started.recv().map_err(|e| Error::Device(e.to_string()))?.map_err(Error::Device)?;
        Ok(Engine::with(rate, commands, garbage, clock, failed, Output::Live { stop }))
    }

    fn with(
        rate: f64,
        commands: rtrb::Producer<Command>,
        garbage: rtrb::Consumer<Garbage>,
        clock: Arc<AtomicU64>,
        failed: Arc<AtomicBool>,
        output: Output,
    ) -> Engine {
        let sampler = Arc::from("sample");
        let sampler_specs: Vec<Spec> = sample_specs().into_iter().chain(placing_specs()).collect();
        Engine {
            rate,
            synths: HashMap::new(),
            effects: HashMap::new(),
            sampler: (sampler, sampler_specs.into()),
            samples: HashMap::new(),
            commands,
            garbage,
            sounds: HashMap::new(),
            ids: HashMap::new(),
            chains: HashMap::new(),
            next: 0,
            warned: HashSet::new(),
            warnings: Vec::new(),
            clock,
            failed,
            output,
        }
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The seconds the engine has played or rendered so far.
    pub fn now(&self) -> f64 {
        self.clock.load(Ordering::Acquire) as f64 / self.rate
    }

    /// Adds a synth, replacing one of the same name.
    pub fn register(&mut self, synth: Synth) {
        let owner: Arc<str> = Arc::from(synth.name.as_str());
        let specs: Vec<Spec> = synth.specs.iter().cloned().chain(placing_specs()).collect();
        self.synths.insert(synth.name.clone(), Registered { item: synth, owner, specs: specs.into() });
    }

    /// Adds an effect, replacing one of the same name.
    pub fn register_effect(&mut self, effect: Effect) {
        let owner: Arc<str> = Arc::from(effect.name.as_str());
        let specs: Vec<Spec> = effect.specs.iter().cloned().chain(blending_specs()).collect();
        self.effects.insert(effect.name.clone(), Registered { item: effect, owner, specs: specs.into() });
    }

    pub fn synths(&self) -> impl Iterator<Item = &Synth> {
        self.synths.values().map(|r| &r.item)
    }

    pub fn effects(&self) -> impl Iterator<Item = &Effect> {
        self.effects.values().map(|r| &r.item)
    }

    /// The options every note takes besides its synth's: amp and pan.
    pub fn note_specs(&self) -> Vec<Spec> {
        placing_specs()
    }

    /// The options every sample takes.
    pub fn sample_specs(&self) -> Vec<Spec> {
        self.sampler.1.to_vec()
    }

    /// The options every effect takes besides its own: mix, pre_amp and amp.
    pub fn effect_specs(&self) -> Vec<Spec> {
        blending_specs()
    }

    /// Loads a FLAC or WAV file, and returns the number events name it by.
    pub fn load_sample(&mut self, path: impl AsRef<Path>) -> Result<u64, Error> {
        let data = SampleData::load(path.as_ref()).map_err(Error::Sample)?;
        self.next += 1;
        self.samples.insert(self.next, Arc::new(data));
        Ok(self.next)
    }

    /// How many channels a loaded sample has.
    pub fn sample_channels(&self, sample: u64) -> Option<usize> {
        self.samples.get(&sample).map(|data| data.channels.len())
    }

        /// Values held to their ranges since the last call, once for each option.
    pub fn warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    /// Starts an effect chain of `effects`, each a name and its options, in order, and returns the chain notes name to play into.
    pub fn start_effects(&mut self, effects: &[(String, Vec<(String, Value)>)]) -> Result<u64, Error> {
        self.check_running()?;
        let mut unit: Net = Net::wrap(Box::new(multipass::<U2>()));
        let mut controls = Vec::new();
        let mut tail = 0.0;
        for (name, given) in effects {
            let registered = self.effects.get(name).ok_or_else(|| Error::UnknownEffect(name.clone()))?;
            let (owner, specs) = (registered.owner.clone(), registered.specs.clone());
            let (build, effect_tail) = (registered.item.build.clone(), registered.item.tail);
            let values = self.check(&owner, &specs, given)?;
            let options = Options::new(owner.clone(), specs, values);
            let wet = build(&options, &mut Context::new(owner.clone()));
            if wet.inputs() != 2 || wet.outputs() != 2 {
                return Err(Error::BadSound { owner: owner.to_string(), outputs: wet.outputs() });
            }
            let pre = options.follow("pre_amp") >> Net::wrap(Box::new(split::<U2>()));
            let wet = (Net::wrap(Box::new(multipass::<U2>())) * pre) >> wet;
            let blended =
                (Net::wrap(Box::new(multipass::<U2>())) ^ wet) | options.follow("mix") | options.follow("amp");
            unit = unit >> (blended >> Net::wrap(Box::new(An(Blend))));
            controls.push(options.take_controls());
            tail += effect_tail;
        }
        unit.set_sample_rate(self.rate);
        unit.allocate();
        self.next += 1;
        let key = self.next;
        let chain = Chain { key, unit: Box::new(unit), input: BufferVec::new(2), stop: None, tail: self.frames(tail) };
        self.send(Command::StartChain(Box::new(chain)))?;
        self.chains.insert(key, controls);
        Ok(key)
    }

    /// Removes an effect chain at `at` seconds, once its notes and its effects' tails have ended.
    pub fn stop_effects(&mut self, chain: u64, at: f64) -> Result<(), Error> {
        if !self.chains.contains_key(&chain) {
            return Err(Error::UnknownChain(chain));
        }
        self.send(Command::StopChain { key: chain, at: self.frames(at) })
    }

    /// Plays events, each at its seconds on the engine's clock; an event whose time has passed plays at once.
    /// Every event is checked before any is sent, so a batch with a mistake plays nothing.
    pub fn play(&mut self, events: &[(f64, Event)]) -> Result<(), Error> {
        self.check_running()?;
        self.sweep();
        let mut commands = Vec::with_capacity(events.len());
        let mut sounds = Vec::new();
        for (at, event) in events {
            match event {
                Event::Note { id, synth, options, chain } => {
                    let registered = self.synths.get(synth).ok_or_else(|| Error::UnknownSynth(synth.clone()))?;
                    let (owner, specs, build) =
                        (registered.owner.clone(), registered.specs.clone(), registered.item.build.clone());
                    let values = self.check(&owner, &specs, options)?;
                    let options = Options::new(owner.clone(), specs, values);
                    let note = build(&options, &mut Context::new(owner.clone()));
                    let (voice, sound) = self.voice(&owner, note, &options, *at, *chain)?;
                    commands.push(Command::Start(voice));
                    sounds.push((*id, sound));
                }
                Event::Sample { id, sample, options, chain } => {
                    let data = self.samples.get(sample).ok_or(Error::UnknownSample(*sample))?.clone();
                    let (owner, specs) = self.sampler.clone();
                    let values = self.check(&owner, &specs, options)?;
                    let options = Options::new(owner.clone(), specs, values);
                    let note = sample_note(data, &options);
                    let (voice, sound) = self.voice(&owner, note, &options, *at, *chain)?;
                    commands.push(Command::Start(voice));
                    sounds.push((*id, sound));
                }
                Event::Set { target, options } => {
                    for change in self.changes(*at, target, options) {
                        commands.push(Command::Change(Box::new(change)));
                    }
                }
            }
        }
        for command in commands {
            self.send(command)?;
        }
        for (id, sound) in sounds {
            self.next += 1;
            if let Some(id) = id {
                self.ids.insert(id, self.next);
            }
            self.sounds.insert(self.next, sound);
        }
        Ok(())
    }

    /// Renders the next `seconds` of an offline engine, as its left and right channels.
    pub fn render(&mut self, seconds: f64) -> Result<(Vec<f32>, Vec<f32>), Error> {
        self.check_running()?;
        let frames = self.frames(seconds) as usize;
        let Output::Offline(mixer) = &mut self.output else {
            return Err(Error::Live);
        };
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mixer.render(&mut left, &mut right)));
        self.drain();
        match rendered {
            Ok(()) => Ok((left, right)),
            Err(_) => {
                self.failed.store(true, Ordering::Release);
                Err(Error::Failed)
            }
        }
    }

    fn frames(&self, seconds: f64) -> u64 {
        (seconds.max(0.0) * self.rate).round() as u64
    }

    fn check_running(&self) -> Result<(), Error> {
        if self.failed.load(Ordering::Acquire) { Err(Error::Failed) } else { Ok(()) }
    }

    fn send(&mut self, mut command: Command) -> Result<(), Error> {
        for _ in 0..1000 {
            match self.commands.push(command) {
                Ok(()) => return Ok(()),
                Err(rtrb::PushError::Full(back)) => command = back,
            }
            self.drain();
            match &self.output {
                Output::Live { .. } => std::thread::sleep(Duration::from_millis(1)),
                Output::Offline(_) => return Err(Error::Busy),
            }
        }
        Err(Error::Busy)
    }

    fn drain(&mut self) {
        while self.garbage.pop().is_ok() {}
    }

    /// Forgets sounds that ended over a second ago, and frees what the mixer has finished with.
    fn sweep(&mut self) {
        self.drain();
        let now = self.now();
        self.sounds.retain(|_, sound| sound.end + 1.0 > now);
        let sounds = &self.sounds;
        self.ids.retain(|_, key| sounds.contains_key(key));
    }

    /// The values to play: each given value checked against its declaration and held to its range.
    fn check(&mut self, owner: &str, specs: &[Spec], given: &[(String, Value)]) -> Result<HashMap<String, Value>, Error> {
        let mut values = HashMap::new();
        for (name, value) in given {
            let spec = specs.iter().find(|s| &s.name == name);
            let slide = name.strip_suffix("_slide").filter(|base| specs.iter().any(|s| s.name == *base));
            let (value, held) = match (spec, slide) {
                (Some(spec), _) => spec.check(value),
                (None, Some(_)) => match value {
                    Value::Number(x) if *x >= 0.0 => Ok((value.clone(), false)),
                    _ => Err("takes seconds from 0 up".to_string()),
                },
                (None, None) => {
                    return Err(Error::UnknownOption { owner: owner.to_string(), option: name.clone() });
                }
            }
            .map_err(|reason| Error::BadValue { owner: owner.to_string(), option: name.clone(), reason })?;
            if held && self.warned.insert((owner.to_string(), name.clone())) {
                let range = spec.and_then(|s| s.range).unwrap_or((0.0, 0.0));
                let (low, high) = range;
                self.warnings.push(format!("{owner}'s option {name} was held to its range, {low} to {high}"));
            }
            values.insert(name.clone(), value);
        }
        if let Some(missing) = specs.iter().find(|s| s.required && !values.contains_key(&s.name)) {
            return Err(Error::MissingOption { owner: owner.to_string(), option: missing.name.clone() });
        }
        Ok(values)
    }

    /// The note placed in the stereo field by its pan and scaled by its amp, ready for the mixer.
    fn voice(
        &self,
        owner: &str,
        note: Note,
        options: &Options,
        at: f64,
        chain: Option<u64>,
    ) -> Result<(Box<Voice>, Sound), Error> {
        let placing = options.follow("pan") | options.follow("amp");
        let mut unit = match note.sound.outputs() {
            1 => (note.sound | placing) >> Net::wrap(Box::new(An(PanMono))),
            2 => (note.sound | placing) >> Net::wrap(Box::new(An(Balance))),
            outputs => return Err(Error::BadSound { owner: owner.to_string(), outputs }),
        };
        unit.set_sample_rate(self.rate);
        unit.allocate();
        let start = Ord::max(self.frames(at), self.clock.load(Ordering::Acquire));
        let end = start + Ord::max(self.frames(note.end), 1);
        let voice = Voice { chain, unit: Box::new(unit), start, end, fade: Ord::max(self.frames(END_FADE), 1) };
        let sound = Sound { chain, start: at, end: at + note.end, controls: options.take_controls() };
        Ok((Box::new(voice), sound))
    }

    fn changes(&self, at: f64, target: &Target, options: &[(String, f64)]) -> Vec<Change> {
        let controls: Vec<&HashMap<String, Control>> = match target {
            Target::Sounds(ids) => ids
                .iter()
                .filter_map(|id| self.ids.get(id))
                .filter_map(|key| self.sounds.get(key))
                .map(|s| &s.controls)
                .collect(),
            Target::Part(chain) => self
                .sounds
                .values()
                .filter(|s| s.chain == Some(*chain) && s.start <= at && at < s.end)
                .map(|s| &s.controls)
                .collect(),
            Target::Effect(chain, index) => self.chains.get(chain).and_then(|c| c.get(*index)).into_iter().collect(),
        };
        let mut changes = Vec::new();
        let mut by_option: HashMap<&str, (Option<f32>, Option<f32>)> = HashMap::new();
        for (name, value) in options {
            match name.strip_suffix("_slide") {
                Some(base) => by_option.entry(base).or_default().1 = Some(*value as f32),
                None => by_option.entry(name).or_default().0 = Some(*value as f32),
            }
        }
        for controls in controls {
            for (name, (value, slide)) in &by_option {
                if let Some(control) = controls.get(*name) {
                    changes.push(Change { at: self.frames(at), control: control.clone(), value: *value, slide: *slide });
                }
            }
        }
        changes
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Output::Live { stop, .. } = &self.output {
            let _ = stop.send(());
        }
    }
}

fn open(mut mixer: Mixer, failed: Arc<AtomicBool>) -> Result<(cpal::Stream, f64), String> {
    let device = cpal::default_host().default_output_device().ok_or("no output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!("the output takes {} samples, and the engine writes f32", config.sample_format()));
    }
    let rate = config.sample_rate() as f64;
    let channels = config.channels() as usize;
    let mut left = vec![0.0f32; 16384];
    let mut right = vec![0.0f32; 16384];
    let stream = device
        .build_output_stream(
            config.into(),
            move |data: &mut [f32], _| fill(&mut mixer, &failed, data, channels, &mut left, &mut right),
            |e| eprintln!("mxs output: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((stream, rate))
}

/// Fills an interleaved buffer of `channels` from the mixer, or silence once it has panicked, which marks the engine failed instead of aborting the process.
fn fill(mixer: &mut Mixer, failed: &AtomicBool, data: &mut [f32], channels: usize, left: &mut [f32], right: &mut [f32]) {
    if failed.load(Ordering::Relaxed) {
        data.fill(0.0);
        return;
    }
    let filled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for frames in data.chunks_mut(channels * left.len()) {
            let n = frames.len() / channels;
            mixer.render(&mut left[..n], &mut right[..n]);
            for (i, frame) in frames.chunks_mut(channels).enumerate() {
                frame[0] = left[i];
                if channels > 1 {
                    frame[1] = right[i];
                }
                frame.iter_mut().skip(2).for_each(|x| *x = 0.0);
            }
        }
    }));
    if filled.is_err() {
        failed.store(true, Ordering::Release);
        data.fill(0.0);
    }
}

fn placing_specs() -> Vec<Spec> {
    vec![
        Spec::float("amp").default(1.0).range(0.0, 16.0).doc("The level the sound is scaled by."),
        Spec::float("pan").default(0.0).range(-1.0, 1.0).doc("Where the sound sits, from -1, left, to 1, right."),
    ]
}

fn blending_specs() -> Vec<Spec> {
    vec![
        Spec::float("mix").default(1.0).range(0.0, 1.0).doc("How much of the processed sound replaces the dry one."),
        Spec::float("pre_amp").default(1.0).range(0.0, 16.0).doc("The level the sound is scaled by before the effect."),
        Spec::float("amp").default(1.0).range(0.0, 16.0).doc("The level the blend is scaled by."),
    ]
}

fn sample_specs() -> Vec<Spec> {
    vec![
        Spec::float("rate").default(1.0).doc("How fast the sample plays, backwards when below 0."),
        Spec::float("start").default(0.0).range(0.0, 1.0).doc("Where it starts, as a fraction of its length."),
        Spec::float("finish").default(1.0).range(0.0, 1.0).doc("Where it finishes, as a fraction of its length."),
        Spec::beats("attack").default(0.0).range(0.0, f64::INFINITY).doc("How long it takes to rise to attack_level."),
        Spec::beats("decay").default(0.0).range(0.0, f64::INFINITY).doc("How long it then takes to fall to decay_level."),
        Spec::beats("sustain").default(-1.0).doc("How long it then holds; the rest of the sample when below 0."),
        Spec::beats("release").default(0.0).range(0.0, f64::INFINITY).doc("How long it then takes to fall silent."),
        Spec::float("attack_level").default(1.0).range(0.0, f64::INFINITY).doc("The level the attack rises to."),
        Spec::float("decay_level").default(-1.0).doc("The level the decay falls to; sustain_level when below 0."),
        Spec::float("sustain_level").default(1.0).range(0.0, f64::INFINITY).doc("The level it holds at."),
    ]
}

fn sample_note(data: Arc<SampleData>, options: &Options) -> Note {
    let rate = options.number("rate");
    let (from, to) = (options.number("start"), options.number("finish"));
    let span = (to - from).abs() * data.frames() as f64 / data.rate;
    let length = span / rate.abs().max(0.001);
    let mono = data.channels.len() == 1;
    let player = options.follow("rate") >> Net::wrap(Box::new(An(Player::new(data, from, to, rate))));
    let player = if mono { player >> Net::wrap(Box::new(pass() | sink())) } else { player };
    let sustain = options.number("sustain");
    let (attack, decay, release) = (options.number("attack"), options.number("decay"), options.number("release"));
    let hold = if sustain < 0.0 { (length - attack - decay - release).max(0.0) } else { sustain };
    let peak = options.number("attack_level");
    let held = options.number("sustain_level");
    let decayed = Some(options.number("decay_level")).filter(|&l| l >= 0.0).unwrap_or(held);
    let level = crate::nodes::shape(move |t| {
        let along = |from: f64, to: f64, x: f64| from + (to - from) * x.clamp(0.0, 1.0);
        if t < attack {
            peak * t / attack
        } else if t < attack + decay {
            along(peak, decayed, (t - attack) / decay)
        } else if t < attack + decay + hold {
            along(decayed, held, (t - attack - decay) / hold)
        } else {
            along(held, 0.0, (t - attack - decay - hold) / release)
        }
    });
    let level = if mono { level } else { level >> Net::wrap(Box::new(split::<U2>())) };
    Note::rings(player * level, attack + decay + hold + release)
}

/// A mono sound placed by its pan at equal power, and scaled by its amp: inputs sound, pan, amp.
#[derive(Clone)]
struct PanMono;

impl AudioNode for PanMono {
    const ID: u64 = 0x6d78_0005;
    type Inputs = U3;
    type Outputs = U2;

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U2> {
        let angle = (input[1].clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        let x = input[0] * input[2];
        [x * angle.cos(), x * angle.sin()].into()
    }
}

/// A stereo sound turned toward one side by its pan, and scaled by its amp: inputs left, right, pan, amp.
#[derive(Clone)]
struct Balance;

impl AudioNode for Balance {
    const ID: u64 = 0x6d78_0006;
    type Inputs = U4;
    type Outputs = U2;

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let pan = input[2].clamp(-1.0, 1.0);
        [input[0] * (1.0 - pan).min(1.0) * input[3], input[1] * (1.0 + pan).min(1.0) * input[3]].into()
    }
}

/// The dry and processed sound blended by mix and scaled by amp: inputs dry left and right, wet left and right, mix, amp.
#[derive(Clone)]
struct Blend;

impl AudioNode for Blend {
    const ID: u64 = 0x6d78_0007;
    type Inputs = U6;
    type Outputs = U2;

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U2> {
        let mix = input[4].clamp(0.0, 1.0);
        let amp = input[5];
        [(input[0] * (1.0 - mix) + input[2] * mix) * amp, (input[1] * (1.0 - mix) + input[3] * mix) * amp].into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Synth;

    #[test]
    fn a_panic_while_filling_audio_silences_the_engine_and_is_reported() {
        let mut engine = Engine::offline(48_000.0);
        engine.register(Synth::new("broken").build(|_, _| {
            let breaks = map(|_: &Frame<f32, U0>| -> f32 { panic!("on purpose while filling audio") });
            Note::rings(breaks, 1.0)
        }));
        engine.play(&[(0.0, Event::note(None, "broken", &[]))]).unwrap();
        let Output::Offline(mixer) = &mut engine.output else { unreachable!() };
        let failed = AtomicBool::new(false);
        let mut data = vec![1.0f32; 256];
        let (mut left, mut right) = (vec![0.0; 64], vec![0.0; 64]);
        fill(mixer, &failed, &mut data, 2, &mut left, &mut right);
        assert!(failed.load(Ordering::Acquire));
        assert!(data.iter().all(|&x| x == 0.0));
    }
}
