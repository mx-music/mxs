//! The engine: synths and effects registered by name, samples loaded from files, and events stamped with seconds, played live or rendered offline.

use crate::clock::Timing;
use crate::device::{self, Format, Range, Settings};
use crate::mixer::{
    CAPACITY, Chain, Change, Command, Garbage, INPUTS, Input, Mixer, Out, Recorder, Voice, Voices,
};
use crate::options::{Context, Control, Options};
use crate::sample::{Player, SampleData};
use crate::spec::{Spec, Value};
use crate::synth::{Effect, Note, Synth};
use cpal::traits::{DeviceTrait, StreamTrait};
use fundsp::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

/// Seconds of silence a note's end fades over, so a note cut at its end does not click.
const END_FADE: f64 = 0.01;
/// How many notes sound at once unless the caller sets another limit: what one core fills in time with the berimbau, mx's costliest struck instrument.
pub const VOICES: usize = 48;

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
    Note {
        /// What a later change reaches the note by.
        id: Option<u64>,
        /// The name of the synth.
        synth: String,
        /// The options the note is played with.
        options: Vec<(String, Value)>,
        /// The effect chain the note plays into, if any.
        chain: Option<u64>,
    },
    /// A sample `load_sample` returned, with the sample player's options.
    Sample {
        /// What a later change reaches the sample by.
        id: Option<u64>,
        /// The sample, as `load_sample` returned it.
        sample: u64,
        /// The options of the sample player.
        options: Vec<(String, Value)>,
        /// The effect chain the sample plays into, if any.
        chain: Option<u64>,
    },
    /// Channels of the sound card's input, from `input`, numbered from 0, with amp and pan, playing until stopped.
    Input {
        /// What a later change reaches the input by.
        id: Option<u64>,
        /// The first channel of the sound card's input, numbered from 0.
        input: usize,
        /// How many channels it reads from `input` up.
        channels: usize,
        /// The input's options, amp and pan.
        options: Vec<(String, Value)>,
        /// The effect chain the input plays into, if any.
        chain: Option<u64>,
    },
    /// New values for options of sounds already playing, each gliding over its `_slide` option when given.
    Set {
        /// The sounds the change applies to.
        target: Target,
        /// The new number of each option.
        options: Vec<(String, f64)>,
    },
    /// Ends the sounds started with these ids, fading them over 10 ms.
    Stop {
        /// The ids the sounds were started with.
        ids: Vec<u64>,
    },
    /// Lets the sounds started with these ids go, as keys let up: a note shaped by its envelope falls silent over its release, and any other sound fades over 10 ms.
    Release {
        /// The ids the sounds were started with.
        ids: Vec<u64>,
    },
}

/// What `record` writes to its file.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// One channel of the sound card's input, numbered from 0.
    Input(usize),
    /// A sample `load_sample` returned, played once in place of the input.
    Sample(u64),
}

impl Event {
    /// A note of `synth` with number options and no effect chain; `id` lets a later change reach it.
    pub fn note(id: Option<u64>, synth: &str, options: &[(&str, f64)]) -> Event {
        let options = options
            .iter()
            .map(|(k, v)| (k.to_string(), Value::Number(*v)))
            .collect();
        Event::Note {
            id,
            synth: synth.to_string(),
            options,
            chain: None,
        }
    }
}

/// What went wrong when the engine was asked to do something; its `Display` says it in a sentence.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// No synth is registered by this name.
    UnknownSynth(String),
    /// No effect is registered by this name.
    UnknownEffect(String),
    /// No sample was loaded under this id.
    UnknownSample(u64),
    /// No effect chain was started under this id.
    UnknownChain(u64),
    /// A sound was given an option its declaration does not list.
    UnknownOption {
        /// The synth, effect or player it was given to.
        owner: String,
        /// The option's name.
        option: String,
    },
    /// An option was given a value its declaration does not accept.
    BadValue {
        /// The synth, effect or player it was given to.
        owner: String,
        /// The option's name.
        option: String,
        /// Why the value is refused, worded to follow the option's name.
        reason: String,
    },
    /// A required option was left out.
    MissingOption {
        /// The synth, effect or player that needs it.
        owner: String,
        /// The option's name.
        option: String,
    },
    /// A synth or an effect made a sound of neither 1 nor 2 channels.
    BadSound {
        /// The synth or effect that made it.
        owner: String,
        /// How many channels the sound had.
        outputs: usize,
    },
    /// A sample could not be loaded or is not usable; the reason.
    Sample(String),
    /// A device could not be used; the reason.
    Device(String),
    /// The sound card has no input of this number, counted from 0.
    UnknownInput(usize),
    /// A recording could not start or finish; the reason.
    Record(String),
    /// More events are waiting than the engine can take.
    Busy,
    /// The call needs a live engine, and this one renders offline.
    Offline,
    /// The call needs an offline engine, and this one plays live.
    Live,
    /// The engine failed while filling audio, and has to be started again.
    Failed,
    /// The output device of this name went away, and the engine has to be started again.
    DeviceLost(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownSynth(name) => write!(f, "no synth named {name}"),
            Error::UnknownEffect(name) => write!(f, "no effect named {name}"),
            Error::UnknownSample(id) => write!(f, "no sample loaded as {id}"),
            Error::UnknownChain(id) => write!(f, "no effect chain {id}"),
            Error::UnknownOption { owner, option } => write!(f, "{owner} has no option {option}"),
            Error::BadValue {
                owner,
                option,
                reason,
            } => write!(f, "{owner}'s option {option} {reason}"),
            Error::MissingOption { owner, option } => {
                write!(f, "{owner} needs the option {option}")
            }
            Error::BadSound { owner, outputs } => {
                write!(f, "{owner} made a sound of {outputs} channels, not 1 or 2")
            }
            Error::Sample(reason) => write!(f, "{reason}"),
            Error::Device(reason) => write!(f, "{reason}"),
            Error::UnknownInput(input) => {
                let number = input + 1;
                write!(f, "the sound card has no input {number}")
            }
            Error::Record(reason) => write!(f, "{reason}"),
            Error::Busy => write!(f, "the engine has more events waiting than it can take"),
            Error::Offline => write!(f, "this engine renders offline and has no output"),
            Error::Live => write!(f, "this engine plays live and cannot render"),
            Error::Failed => write!(
                f,
                "the engine failed while filling audio, and has to be started again"
            ),
            Error::DeviceLost(name) => write!(
                f,
                "the output device {name} went away, and the engine has to be started again"
            ),
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
    key: u64,
    chain: Option<u64>,
    start: f64,
    end: f64,
    controls: HashMap<String, Control>,
    release: Option<(Shared, f64)>,
}

enum Output {
    Offline(Box<Mixer>),
    Live { stop: mpsc::Sender<()> },
}

struct Recording {
    source: Option<u64>,
    giving_up: Arc<AtomicBool>,
    writer: JoinHandle<Result<(), String>>,
}

type Wav = hound::WavWriter<std::io::BufWriter<std::fs::File>>;

/// The engine, live on an output device or offline.
pub struct Engine {
    rate: f64,
    synths: HashMap<String, Registered<Synth>>,
    effects: HashMap<String, Registered<Effect>>,
    sampler: (Arc<str>, Arc<[Spec]>),
    input: (Arc<str>, Arc<[Spec]>),
    inputs: usize,
    recording: Option<Recording>,
    output_recording: Option<Recording>,
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
    voices: Arc<Voices>,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    device: String,
    timing: Option<Arc<Mutex<Timing>>>,
    output: Output,
}

impl Engine {
    /// An engine that renders offline at `rate` frames a second.
    pub fn offline(rate: f64) -> Engine {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let clock = Arc::new(AtomicU64::new(0));
        let voices = Voices::new(VOICES);
        let mixer = Mixer::new(receiving, returning, clock.clone(), voices.clone(), None);
        Engine::with(
            rate,
            commands,
            garbage,
            clock,
            voices,
            Arc::new(AtomicBool::new(false)),
            Output::Offline(Box::new(mixer)),
        )
    }

    /// An offline engine whose input is `channels` fed through the producer returned, for tests of live input.
    #[cfg(test)]
    pub(crate) fn offline_with_input(rate: f64, channels: usize) -> (Engine, rtrb::Producer<f32>) {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let (feed, frames) = rtrb::RingBuffer::new(CAPACITY * channels);
        let clock = Arc::new(AtomicU64::new(0));
        let voices = Voices::new(VOICES);
        let mixer = Mixer::new(
            receiving,
            returning,
            clock.clone(),
            voices.clone(),
            Some(Input { frames, channels }),
        );
        let output = Output::Offline(Box::new(mixer));
        let mut engine = Engine::with(
            rate,
            commands,
            garbage,
            clock,
            voices,
            Arc::new(AtomicBool::new(false)),
            output,
        );
        engine.inputs = channels;
        (engine, feed)
    }

    /// An engine that plays through the default output device at its own rate.
    pub fn live() -> Result<Engine, Error> {
        Engine::live_with(&Settings::default())
    }

    /// An engine that plays through the default output device, and reads the default input device at the same rate.
    pub fn live_with_input() -> Result<Engine, Error> {
        Engine::live_with(&Settings {
            with_input: true,
            ..Settings::default()
        })
    }

    /// An engine that plays as `settings` ask, on the default output device at its own rate for what they leave out.
    pub fn live_with(settings: &Settings) -> Result<Engine, Error> {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let clock = Arc::new(AtomicU64::new(0));
        let failed = Arc::new(AtomicBool::new(false));
        let (stop, stopped) = mpsc::channel::<()>();
        let (ready, started) = mpsc::channel::<Result<Opened, String>>();
        let failing = failed.clone();
        let lost = Arc::new(AtomicBool::new(false));
        let losing = lost.clone();
        let settings = settings.clone();
        let mixing = clock.clone();
        let voices = Voices::new(VOICES);
        let limiting = voices.clone();
        // cpal's streams cannot move between threads on every platform, so one thread owns them until the engine is dropped.
        std::thread::spawn(move || {
            match open(
                receiving, returning, mixing, limiting, failing, losing, &settings,
            ) {
                Ok((streams, opened)) => {
                    let _ = ready.send(Ok(opened));
                    let _ = stopped.recv();
                    drop(streams);
                }
                Err(reason) => {
                    let _ = ready.send(Err(reason));
                }
            }
        });
        let opened = started
            .recv()
            .map_err(|e| Error::Device(e.to_string()))?
            .map_err(Error::Device)?;
        let mut engine = Engine::with(
            opened.rate,
            commands,
            garbage,
            clock,
            voices,
            failed,
            Output::Live { stop },
        );
        engine.inputs = opened.inputs;
        engine.lost = lost;
        engine.device = opened.name;
        engine.timing = Some(opened.timing);
        Ok(engine)
    }

    fn with(
        rate: f64,
        commands: rtrb::Producer<Command>,
        garbage: rtrb::Consumer<Garbage>,
        clock: Arc<AtomicU64>,
        voices: Arc<Voices>,
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
            input: (Arc::from("input"), placing_specs().into()),
            inputs: 0,
            recording: None,
            output_recording: None,
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
            voices,
            failed,
            lost: Arc::new(AtomicBool::new(false)),
            device: String::new(),
            timing: None,
            output,
        }
    }

    /// The rate the engine plays and renders at, in Hz.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// How many channels of the sound card's input the engine reads; 0 unless it was started with input.
    pub fn inputs(&self) -> usize {
        self.inputs
    }

    /// The seconds the engine has played or rendered so far.
    pub fn now(&self) -> f64 {
        self.clock.load(Ordering::Acquire) as f64 / self.rate
    }

    /// The engine's seconds now, and the system time at which it renders them; a live engine knows it from its first buffer on, an offline one never.
    pub fn clock(&self) -> (f64, Option<SystemTime>) {
        let now = self.now();
        let system = self
            .timing()
            .and_then(|t| t.system(now))
            .map(|s| SystemTime::UNIX_EPOCH + Duration::from_secs_f64(s));
        (now, system)
    }

    /// Seconds from rendering a frame to the device playing it, as the device gives it.
    pub fn latency(&self) -> Option<f64> {
        self.timing().and_then(|t| t.latency)
    }

    /// The engine's seconds at which it renders the system time `at`, however far the two clocks have drifted apart; a live engine knows it, an offline one does not.
    pub fn seconds_at(&self, at: SystemTime) -> Option<f64> {
        let system = at
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        self.timing().and_then(|t| t.engine(system))
    }

    /// Plays events, each at a system time.
    pub fn play_at(&mut self, events: &[(SystemTime, Event)]) -> Result<(), Error> {
        let mut timed = Vec::with_capacity(events.len());
        for (at, event) in events {
            timed.push((self.seconds_at(*at).ok_or(Error::Offline)?, event.clone()));
        }
        self.play(&timed)
    }

    fn timing(&self) -> Option<Timing> {
        self.timing
            .as_ref()
            .map(|t| t.lock().unwrap_or_else(|e| e.into_inner()).clone())
    }

    /// Adds a synth, replacing one of the same name.
    pub fn register(&mut self, synth: Synth) {
        let owner: Arc<str> = Arc::from(synth.name.as_str());
        let specs: Vec<Spec> = synth.specs.iter().cloned().chain(placing_specs()).collect();
        self.synths.insert(
            synth.name.clone(),
            Registered {
                item: synth,
                owner,
                specs: specs.into(),
            },
        );
    }

    /// Adds an effect, replacing one of the same name.
    pub fn register_effect(&mut self, effect: Effect) {
        let owner: Arc<str> = Arc::from(effect.name.as_str());
        let specs: Vec<Spec> = effect
            .specs
            .iter()
            .cloned()
            .chain(blending_specs())
            .collect();
        self.effects.insert(
            effect.name.clone(),
            Registered {
                item: effect,
                owner,
                specs: specs.into(),
            },
        );
    }

    /// The synths registered, in no set order.
    pub fn synths(&self) -> impl Iterator<Item = &Synth> {
        self.synths.values().map(|r| &r.item)
    }

    /// The effects registered, in no set order.
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

    /// Loads a FLAC, WAV or AIFF file, and returns the number events name it by.
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

    /// Values held to their ranges since the last call, once for each option, and how many notes were stolen to keep to the voice limit.
    pub fn warnings(&mut self) -> Vec<String> {
        let late = self.voices.late.swap(0, Ordering::Relaxed);
        let most = self.voices.most_late.swap(0, Ordering::Relaxed);
        if late > 0 {
            let events = if late == 1 {
                "event arrived"
            } else {
                "events arrived"
            };
            let ms = most as f64 / self.rate * 1000.0;
            self.warnings.push(format!(
                "{late} {events} after their time, by up to {ms:.1} ms, and played at once"
            ));
        }
        let stolen = self.voices.stolen.swap(0, Ordering::Relaxed);
        if stolen > 0 {
            let limit = self.voices.limit.load(Ordering::Relaxed);
            let notes = if stolen == 1 {
                "note was"
            } else {
                "notes were"
            };
            self.warnings.push(format!(
                "{stolen} {notes} stolen to keep to {limit} at once"
            ));
        }
        std::mem::take(&mut self.warnings)
    }

    /// Sets how many notes sound at once; a note past it takes the place of the quietest, which fades out over 10 ms.
    pub fn set_voice_limit(&mut self, limit: usize) {
        self.voices.limit.store(limit, Ordering::Relaxed);
    }

    /// Starts an effect chain of `effects`, each a name and its options, in order, and returns the chain notes name to play into.
    pub fn start_effects(
        &mut self,
        effects: &[(String, Vec<(String, Value)>)],
    ) -> Result<u64, Error> {
        self.check_running()?;
        let mut unit: Net = Net::wrap(Box::new(multipass::<U2>()));
        let mut controls = Vec::new();
        let mut tail = 0.0;
        for (name, given) in effects {
            let registered = self
                .effects
                .get(name)
                .ok_or_else(|| Error::UnknownEffect(name.clone()))?;
            let (owner, specs) = (registered.owner.clone(), registered.specs.clone());
            let (build, effect_tail) =
                (registered.item.build.clone(), registered.item.tail.clone());
            let values = self.check(&owner, &specs, given)?;
            let options = Options::new(owner.clone(), specs, values);
            let wet = build(&options, &mut Context::new(owner.clone()));
            if wet.inputs() != 2 || wet.outputs() != 2 {
                return Err(Error::BadSound {
                    owner: owner.to_string(),
                    outputs: wet.outputs(),
                });
            }
            let pre = options.follow("pre_amp") >> Net::wrap(Box::new(split::<U2>()));
            let wet = (Net::wrap(Box::new(multipass::<U2>())) * pre) >> wet;
            let blended = (Net::wrap(Box::new(multipass::<U2>())) ^ wet)
                | options.follow("mix")
                | options.follow("amp");
            unit = unit >> (blended >> Net::wrap(Box::new(An(Blend))));
            controls.push(options.take_controls());
            tail += effect_tail(&options);
        }
        unit.set_sample_rate(self.rate);
        unit.allocate();
        self.next += 1;
        let key = self.next;
        let chain = Chain {
            key,
            unit: Box::new(unit),
            input: BufferVec::new(2),
            stop: None,
            tail: self.frames(tail),
        };
        self.send(Command::StartChain(Box::new(chain)))?;
        self.chains.insert(key, controls);
        Ok(key)
    }

    /// Removes an effect chain at `at` seconds, once its notes and its effects' tails have ended.
    pub fn stop_effects(&mut self, chain: u64, at: f64) -> Result<(), Error> {
        if !self.chains.contains_key(&chain) {
            return Err(Error::UnknownChain(chain));
        }
        self.send(Command::StopChain {
            key: chain,
            at: self.frames(at),
        })
    }

    /// Records `source` to a stereo 24-bit WAV file at `path` from `at` seconds, apart from the main output, until `stop_recording`.
    /// It replaces a recording already running.
    pub fn record(&mut self, path: impl AsRef<Path>, source: Source, at: f64) -> Result<(), Error> {
        self.check_running()?;
        self.stop_recording()?;
        self.sweep();
        let (voice, sound) = match source {
            Source::Input(input) => self.input_voice(input, 1, &[], at, Out::Tap)?,
            Source::Sample(sample) => self.sample_voice(sample, &[], at, Out::Tap)?,
        };
        let (recorder, mut recording) = self.recorder(path.as_ref(), voice.start, false)?;
        recording.source = Some(voice.key);
        self.sounds.insert(sound.key, sound);
        self.send(Command::Start(voice))?;
        self.send(Command::Record(Box::new(recorder)))?;
        self.recording = Some(recording);
        Ok(())
    }

    /// Records the main output to a stereo 32-bit float WAV file at `path` from `at` seconds, until `stop_recording_output`.
    /// It runs beside a recording of an input, and replaces a recording of the output already running.
    pub fn record_output(&mut self, path: impl AsRef<Path>, at: f64) -> Result<(), Error> {
        self.check_running()?;
        self.stop_recording_output()?;
        let start = Ord::max(self.frames(at), self.clock.load(Ordering::Acquire));
        let (recorder, recording) = self.recorder(path.as_ref(), start, true)?;
        self.send(Command::RecordOutput(Box::new(recorder)))?;
        self.output_recording = Some(recording);
        Ok(())
    }

    /// Stops recording the output and closes its file, with what was recorded up to now.
    pub fn stop_recording_output(&mut self) -> Result<(), Error> {
        let Some(recording) = self.output_recording.take() else {
            return Ok(());
        };
        let sent = self.send(Command::StopRecordingOutput);
        self.finish(recording, sent)
    }

    /// Whether any note or sample started is still sounding; live input does not count.
    pub fn sounding(&self) -> bool {
        let now = self.now();
        self.sounds
            .values()
            .any(|sound| sound.start <= now && now < sound.end && sound.end.is_finite())
    }

    /// A WAV file at `path` and the thread that writes it, fed by the recorder returned.
    fn recorder(
        &self,
        path: &Path,
        start: u64,
        float: bool,
    ) -> Result<(Recorder, Recording), Error> {
        let (bits, format) = if float {
            (32, hound::SampleFormat::Float)
        } else {
            (24, hound::SampleFormat::Int)
        };
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: self.rate as u32,
            bits_per_sample: bits,
            sample_format: format,
        };
        let shown = path.display();
        let wav = hound::WavWriter::create(path, spec)
            .map_err(|e| Error::Record(format!("cannot write {shown}: {e}")))?;
        // Two seconds of stereo frames, which the writer empties every 10 ms.
        let (out, frames) = rtrb::RingBuffer::new(4 * self.rate as usize);
        let giving_up = Arc::new(AtomicBool::new(false));
        let giving = giving_up.clone();
        let rate = self.rate;
        let writer = std::thread::spawn(move || write_wav(wav, frames, giving, float, rate));
        Ok((
            Recorder { start, out },
            Recording {
                source: None,
                giving_up,
                writer,
            },
        ))
    }

    /// Stops the recording of an input and closes its file, with what was recorded up to now.
    pub fn stop_recording(&mut self) -> Result<(), Error> {
        let Some(recording) = self.recording.take() else {
            return Ok(());
        };
        let now = self.clock.load(Ordering::Acquire);
        let mut sent = Ok(());
        if let Some(source) = recording.source {
            self.sounds.remove(&source);
            sent = self.send(Command::Stop {
                key: source,
                at: now,
            });
        }
        let sent = sent.and_then(|()| self.send(Command::StopRecording));
        self.finish(recording, sent)
    }

    /// Waits for the writer to close the file once the mixer hands its recorder back.
    fn finish(&mut self, recording: Recording, sent: Result<(), Error>) -> Result<(), Error> {
        if let Output::Offline(mixer) = &mut self.output {
            mixer.receive();
        }
        // The recorder's ring closes once the mixer hands it back and it is dropped here.
        let deadline = Instant::now() + Duration::from_secs(2);
        while !recording.writer.is_finished() {
            self.drain();
            if Instant::now() > deadline {
                recording.giving_up.store(true, Ordering::Release);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        sent?;
        match recording.writer.join() {
            Ok(Ok(())) => Ok(()),
            Ok(Err(reason)) => Err(Error::Record(reason)),
            Err(_) => Err(Error::Record("the recording's writer panicked".to_string())),
        }
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
                Event::Note {
                    id,
                    synth,
                    options,
                    chain,
                } => {
                    let registered = self
                        .synths
                        .get(synth)
                        .ok_or_else(|| Error::UnknownSynth(synth.clone()))?;
                    let (owner, specs, build) = (
                        registered.owner.clone(),
                        registered.specs.clone(),
                        registered.item.build.clone(),
                    );
                    let values = self.check(&owner, &specs, options)?;
                    let options = Options::new(owner.clone(), specs, values);
                    let note = build(&options, &mut Context::new(owner.clone()));
                    let out = chain.map_or(Out::Main, Out::Chain);
                    let (voice, sound) = self.voice(&owner, note, &options, *at, out, None)?;
                    commands.push(Command::Start(voice));
                    sounds.push((*id, sound));
                }
                Event::Sample {
                    id,
                    sample,
                    options,
                    chain,
                } => {
                    let out = chain.map_or(Out::Main, Out::Chain);
                    let (voice, sound) = self.sample_voice(*sample, options, *at, out)?;
                    commands.push(Command::Start(voice));
                    sounds.push((*id, sound));
                }
                Event::Input {
                    id,
                    input,
                    channels,
                    options,
                    chain,
                } => {
                    let out = chain.map_or(Out::Main, Out::Chain);
                    let (voice, sound) = self.input_voice(*input, *channels, options, *at, out)?;
                    commands.push(Command::Start(voice));
                    sounds.push((*id, sound));
                }
                Event::Set { .. } | Event::Stop { .. } | Event::Release { .. } => {}
            }
        }
        // The batch's sounds are known before its changes are resolved, so a change can reach a note of the same batch.
        for (id, sound) in sounds {
            if let Some(id) = id {
                self.ids.insert(id, sound.key);
            }
            self.sounds.insert(sound.key, sound);
        }
        for (at, event) in events {
            match event {
                Event::Set { target, options } => {
                    for change in self.changes(*at, target, options) {
                        commands.push(Command::Change(Box::new(change)));
                    }
                }
                Event::Stop { ids } => {
                    let at = *at;
                    for key in ids.iter().filter_map(|id| self.ids.get(id)) {
                        if let Some(sound) = self.sounds.get_mut(key) {
                            sound.end = sound.end.min(at.max(sound.start) + END_FADE);
                        }
                        commands.push(Command::Stop {
                            key: *key,
                            at: self.frames(at),
                        });
                    }
                }
                Event::Release { ids } => {
                    for key in ids
                        .iter()
                        .filter_map(|id| self.ids.get(id))
                        .copied()
                        .collect::<Vec<_>>()
                    {
                        let Some(sound) = self.sounds.get_mut(&key) else {
                            continue;
                        };
                        let at = at.max(sound.start);
                        let (stop, gate) = match &sound.release {
                            Some((gate, seconds)) => (at + seconds, Some(gate.clone())),
                            None => (at, None),
                        };
                        sound.end = sound.end.min(stop + END_FADE);
                        if let Some(gate) = gate {
                            let control = Control {
                                target: gate,
                                slide: shared(0.0),
                            };
                            commands.push(Command::Change(Box::new(Change {
                                at: self.frames(at),
                                control,
                                value: Some(0.0),
                                slide: None,
                            })));
                        }
                        commands.push(Command::Stop {
                            key,
                            at: self.frames(stop),
                        });
                    }
                }
                _ => {}
            }
        }
        for command in commands {
            self.send(command)?;
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
        // Rendering outruns a recording's writer, so it goes a quarter second at a time, once the recordings have room for it.
        let chunk = Ord::max((self.rate / 4.0) as usize, 1);
        let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for (left, right) in left.chunks_mut(chunk).zip(right.chunks_mut(chunk)) {
                mixer.receive();
                while mixer.recording_room() < left.len() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                mixer.render(left, right);
            }
        }));
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
        if self.lost.load(Ordering::Acquire) {
            Err(Error::DeviceLost(self.device.clone()))
        } else if self.failed.load(Ordering::Acquire) {
            Err(Error::Failed)
        } else {
            Ok(())
        }
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
    fn check(
        &mut self,
        owner: &str,
        specs: &[Spec],
        given: &[(String, Value)],
    ) -> Result<HashMap<String, Value>, Error> {
        let mut values = HashMap::new();
        for (name, value) in given {
            let spec = specs.iter().find(|s| &s.name == name);
            let slide = name
                .strip_suffix("_slide")
                .filter(|base| specs.iter().any(|s| s.name == *base));
            let (value, held) = match (spec, slide) {
                (Some(spec), _) => spec.check(value),
                (None, Some(_)) => match value {
                    Value::Number(x) if *x >= 0.0 => Ok((value.clone(), false)),
                    _ => Err("takes seconds from 0 up".to_string()),
                },
                (None, None) => {
                    return Err(Error::UnknownOption {
                        owner: owner.to_string(),
                        option: name.clone(),
                    });
                }
            }
            .map_err(|reason| Error::BadValue {
                owner: owner.to_string(),
                option: name.clone(),
                reason,
            })?;
            if held && self.warned.insert((owner.to_string(), name.clone())) {
                let range = spec.and_then(|s| s.range).unwrap_or((0.0, 0.0));
                let (low, high) = range;
                self.warnings.push(format!(
                    "{owner}'s option {name} was held to its range, {low} to {high}"
                ));
            }
            values.insert(name.clone(), value);
        }
        if let Some(missing) = specs
            .iter()
            .find(|s| s.required && !values.contains_key(&s.name))
        {
            return Err(Error::MissingOption {
                owner: owner.to_string(),
                option: missing.name.clone(),
            });
        }
        Ok(values)
    }

    fn sample_voice(
        &mut self,
        sample: u64,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<(Box<Voice>, Sound), Error> {
        let data = self
            .samples
            .get(&sample)
            .ok_or(Error::UnknownSample(sample))?
            .clone();
        let (owner, specs) = self.sampler.clone();
        let values = self.check(&owner, &specs, given)?;
        let options = Options::new(owner.clone(), specs, values);
        let note = sample_note(data, &options);
        self.voice(&owner, note, &options, at, out, None)
    }

    fn input_voice(
        &mut self,
        input: usize,
        channels: usize,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<(Box<Voice>, Sound), Error> {
        let channels = channels.clamp(1, 2);
        if input + channels > Ord::min(self.inputs, INPUTS) {
            return Err(Error::UnknownInput(input + channels - 1));
        }
        let (owner, specs) = self.input.clone();
        let values = self.check(&owner, &specs, given)?;
        let options = Options::new(owner.clone(), specs, values);
        let sound = match channels {
            1 => Net::wrap(Box::new(pass())),
            _ => Net::wrap(Box::new(multipass::<U2>())),
        };
        let note = Note::rings(sound, f64::INFINITY);
        self.voice(&owner, note, &options, at, out, Some((input, channels)))
    }

    /// The note placed in the stereo field by its pan and scaled by its amp, ready for the mixer.
    fn voice(
        &mut self,
        owner: &str,
        note: Note,
        options: &Options,
        at: f64,
        out: Out,
        input: Option<(usize, usize)>,
    ) -> Result<(Box<Voice>, Sound), Error> {
        let placing = options.follow("pan") | options.follow("amp");
        let release = note.release;
        let mut unit = match note.sound.outputs() {
            1 => (note.sound | placing) >> Net::wrap(Box::new(An(PanMono))),
            2 => (note.sound | placing) >> Net::wrap(Box::new(An(Balance))),
            outputs => {
                return Err(Error::BadSound {
                    owner: owner.to_string(),
                    outputs,
                });
            }
        };
        unit.set_sample_rate(self.rate);
        unit.allocate();
        self.next += 1;
        let key = self.next;
        let now = self.clock.load(Ordering::Acquire);
        let start = Ord::max(self.frames(at), now);
        let late = now.saturating_sub(self.frames(at));
        let end = start.saturating_add(Ord::max(self.frames(note.end), 1));
        let fade = Ord::max(self.frames(END_FADE), 1);
        let voice = Voice {
            key,
            out,
            input,
            unit: Box::new(unit),
            start,
            late,
            end,
            fade,
            level: 0.0,
            stolen: false,
        };
        let chain = match out {
            Out::Chain(key) => Some(key),
            _ => None,
        };
        let sound = Sound {
            key,
            chain,
            start: at,
            end: at + note.end,
            controls: options.take_controls(),
            release,
        };
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
            Target::Effect(chain, index) => self
                .chains
                .get(chain)
                .and_then(|c| c.get(*index))
                .into_iter()
                .collect(),
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
                    changes.push(Change {
                        at: self.frames(at),
                        control: control.clone(),
                        value: *value,
                        slide: *slide,
                    });
                }
            }
        }
        changes
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.stop_recording();
        let _ = self.stop_recording_output();
        if let Output::Live { stop, .. } = &self.output {
            let _ = stop.send(());
        }
    }
}

/// Writes interleaved stereo frames to `wav` until the mixer lets go of the ring, or the engine gives up waiting for it.
/// It rewrites the file's header every half second, so a process killed while recording leaves a file that plays up to then.
fn write_wav(
    mut wav: Wav,
    mut frames: rtrb::Consumer<f32>,
    giving_up: Arc<AtomicBool>,
    float: bool,
    rate: f64,
) -> Result<(), String> {
    let mut since_flush = 0;
    loop {
        let done = frames.is_abandoned() || giving_up.load(Ordering::Acquire);
        // Whole frames only, so a flushed header never ends between a left and a right sample.
        let ready = frames.slots() / 2 * 2;
        for _ in 0..ready {
            let x = frames.pop().unwrap_or(0.0);
            let written = if float {
                wav.write_sample(x)
            } else {
                wav.write_sample((x.clamp(-1.0, 1.0) * 8_388_607.0).round() as i32)
            };
            written.map_err(|e| e.to_string())?;
        }
        since_flush += ready / 2;
        if done {
            return wav.finalize().map_err(|e| e.to_string());
        }
        if since_flush as f64 >= rate / 2.0 {
            wav.flush().map_err(|e| e.to_string())?;
            since_flush = 0;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

type Streams = (Option<cpal::Stream>, cpal::Stream);

fn open(
    commands: rtrb::Consumer<Command>,
    garbage: rtrb::Producer<Garbage>,
    clock: Arc<AtomicU64>,
    voices: Arc<Voices>,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    settings: &Settings,
) -> Result<(Streams, Opened), String> {
    let host = cpal::default_host();
    let device = device::output(&host, settings.device.as_deref())?;
    let name = device::name_of(&device);
    let rate = match settings.rate {
        Some(rate) => rate,
        None => device
            .default_output_config()
            .map_err(|e| e.to_string())?
            .sample_rate(),
    };
    let ranges: Vec<Range> = device
        .supported_output_configs()
        .map_err(|e| e.to_string())?
        .map(Range::from)
        .collect();
    let chosen = device::choose(&name, &ranges, rate, settings.buffer)?;
    let (input_stream, input) = if settings.with_input {
        open_input(&host, settings.input_device.as_deref(), chosen.rate)?
    } else {
        (None, None)
    };
    let inputs = input.as_ref().map_or(0, |i| i.channels);
    let frames = clock.clone();
    let mixer = Mixer::new(commands, garbage, clock, voices, input);
    let timing = Arc::new(Mutex::new(Timing::new(chosen.rate as f64)));
    let live = Live {
        mixer,
        failed,
        lost,
        frames,
        timing: timing.clone(),
    };
    let config = cpal::StreamConfig {
        channels: chosen.channels,
        sample_rate: chosen.rate,
        buffer_size: chosen
            .buffer
            .map_or(cpal::BufferSize::Default, cpal::BufferSize::Fixed),
    };
    let stream = match chosen.format {
        Format::F32 => output_stream::<f32>(&device, config, live),
        Format::I24 => output_stream::<cpal::I24>(&device, config, live),
        Format::I16 => output_stream::<i16>(&device, config, live),
    }?;
    stream.play().map_err(|e| e.to_string())?;
    let waiting = Instant::now();
    while timing
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .system(0.0)
        .is_none()
    {
        if waiting.elapsed() > Duration::from_secs(2) {
            return Err(format!(
                "{name} started but asked for no audio within 2 seconds"
            ));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok((
        (input_stream, stream),
        Opened {
            rate: chosen.rate as f64,
            inputs,
            name,
            timing,
        },
    ))
}

/// What a live engine learns once its device is playing.
struct Opened {
    rate: f64,
    inputs: usize,
    name: String,
    timing: Arc<Mutex<Timing>>,
}

/// What the output callback owns.
struct Live {
    mixer: Mixer,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
    timing: Arc<Mutex<Timing>>,
}

fn output_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    live: Live,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let Live {
        mut mixer,
        failed,
        lost,
        frames,
        timing,
    } = live;
    let channels = config.channels as usize;
    let mut left = vec![0.0f32; 16384];
    let mut right = vec![0.0f32; 16384];
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let stamp = info.timestamp();
                let latency = stamp.playback.duration_since(stamp.callback).as_secs_f64();
                let system = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                // The callback never waits on the engine's thread; a reading missed while it holds the lock is one of hundreds a second.
                if let Ok(mut timing) = timing.try_lock() {
                    timing.observe(frames.load(Ordering::Acquire), system, Some(latency));
                }
                fill(&mut mixer, &failed, data, channels, &mut left, &mut right)
            },
            move |e| match e.kind() {
                cpal::ErrorKind::DeviceNotAvailable => lost.store(true, Ordering::Release),
                _ => eprintln!("mxs output: {e}"),
            },
            None,
        )
        .map_err(|e| e.to_string())
}

fn open_input(
    host: &cpal::Host,
    name: Option<&str>,
    rate: cpal::SampleRate,
) -> Result<(Option<cpal::Stream>, Option<Input>), String> {
    let device = device::input(host, name)?;
    let ranges = device
        .supported_input_configs()
        .map_err(|e| e.to_string())?;
    let config = ranges
        .filter(|range| range.sample_format() == cpal::SampleFormat::F32)
        .find_map(|range| range.try_with_sample_rate(rate));
    let Some(config) = config else {
        let found = device
            .default_input_config()
            .map_err(|e| e.to_string())?
            .sample_rate();
        return Err(format!(
            "the input runs at {found} Hz and the output at {rate} Hz"
        ));
    };
    let channels = config.channels() as usize;
    let (mut feed, frames) = rtrb::RingBuffer::new(CAPACITY * channels);
    let stream = device
        .build_input_stream(
            config.into(),
            move |data: &[f32], _| {
                for &x in data {
                    let _ = feed.push(x);
                }
            },
            |e| eprintln!("mxs input: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok((Some(stream), Some(Input { frames, channels })))
}

/// Fills an interleaved buffer of `channels` from the mixer, or silence once it has panicked, which marks the engine failed instead of aborting the process.
fn fill<T: cpal::Sample + cpal::FromSample<f32>>(
    mixer: &mut Mixer,
    failed: &AtomicBool,
    data: &mut [T],
    channels: usize,
    left: &mut [f32],
    right: &mut [f32],
) {
    if failed.load(Ordering::Relaxed) {
        data.fill(T::EQUILIBRIUM);
        return;
    }
    let filled = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for frames in data.chunks_mut(channels * left.len()) {
            let n = frames.len() / channels;
            mixer.render(&mut left[..n], &mut right[..n]);
            for (i, frame) in frames.chunks_mut(channels).enumerate() {
                frame[0] = T::from_sample(left[i]);
                if channels > 1 {
                    frame[1] = T::from_sample(right[i]);
                }
                frame.iter_mut().skip(2).for_each(|x| *x = T::EQUILIBRIUM);
            }
        }
    }));
    if filled.is_err() {
        failed.store(true, Ordering::Release);
        data.fill(T::EQUILIBRIUM);
    }
}

fn placing_specs() -> Vec<Spec> {
    vec![
        Spec::float("amp")
            .default(1.0)
            .range(0.0, 16.0)
            .doc("The level the sound is scaled by."),
        Spec::float("pan")
            .default(0.0)
            .range(-1.0, 1.0)
            .doc("Where the sound sits, from -1, left, to 1, right."),
    ]
}

fn blending_specs() -> Vec<Spec> {
    vec![
        Spec::float("mix")
            .default(1.0)
            .range(0.0, 1.0)
            .doc("How much of the processed sound replaces the dry one."),
        Spec::float("pre_amp")
            .default(1.0)
            .range(0.0, 16.0)
            .doc("The level the sound is scaled by before the effect."),
        Spec::float("amp")
            .default(1.0)
            .range(0.0, 16.0)
            .doc("The level the blend is scaled by."),
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
        Spec::boolean("normalize").default(0.0).doc("Whether it is scaled so its file's loudest moment reaches full scale, before amp, in both channels at the centre for a mono file."),
    ]
}

fn sample_note(data: Arc<SampleData>, options: &Options) -> Note {
    let rate = options.number("rate");
    let (from, to) = (options.number("start"), options.number("finish"));
    let span = (to - from).abs() * data.frames() as f64 / data.rate;
    let length = span / rate.abs().max(0.001);
    let mono = data.channels.len() == 1;
    // A mono sample is placed at equal power, so it is raised by as much again as its pan lowers it at the centre.
    let centre = if mono { std::f32::consts::SQRT_2 } else { 1.0 };
    let gain = if options.number("normalize") > 0.5 && data.peak > 0.0 {
        centre / data.peak
    } else {
        1.0
    };
    let player = (options.follow("rate")
        >> Net::wrap(Box::new(An(Player::new(data, from, to, rate)))))
        * gain;
    let player = if mono {
        player >> Net::wrap(Box::new(pass() | sink()))
    } else {
        player
    };
    let sustain = options.number("sustain");
    let (attack, decay, release) = (
        options.number("attack"),
        options.number("decay"),
        options.number("release"),
    );
    let hold = if sustain < 0.0 {
        (length - attack - decay - release).max(0.0)
    } else {
        sustain
    };
    let peak = options.number("attack_level");
    let held = options.number("sustain_level");
    let decayed = Some(options.number("decay_level"))
        .filter(|&l| l >= 0.0)
        .unwrap_or(held);
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
    let level = if mono {
        level
    } else {
        level >> Net::wrap(Box::new(split::<U2>()))
    };
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
        [
            input[0] * (1.0 - pan).min(1.0) * input[3],
            input[1] * (1.0 + pan).min(1.0) * input[3],
        ]
        .into()
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
        [
            (input[0] * (1.0 - mix) + input[2] * mix) * amp,
            (input[1] * (1.0 - mix) + input[3] * mix) * amp,
        ]
        .into()
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
            let breaks =
                map(|_: &Frame<f32, U0>| -> f32 { panic!("on purpose while filling audio") });
            Note::rings(breaks, 1.0)
        }));
        engine
            .play(&[(0.0, Event::note(None, "broken", &[]))])
            .unwrap();
        let Output::Offline(mixer) = &mut engine.output else {
            unreachable!()
        };
        let failed = AtomicBool::new(false);
        let mut data = vec![1.0f32; 256];
        let (mut left, mut right) = (vec![0.0; 64], vec![0.0; 64]);
        fill(mixer, &failed, &mut data, 2, &mut left, &mut right);
        assert!(failed.load(Ordering::Acquire));
        assert!(data.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn a_device_of_16_bit_samples_gets_the_float_samples_rounded_to_them() {
        let render = || {
            let mut engine = Engine::offline(48_000.0);
            crate::synths::all()
                .into_iter()
                .for_each(|s| engine.register(s));
            engine
                .play(&[(0.0, Event::note(None, "saw", &[("amp", 2.0)]))])
                .unwrap();
            engine
        };
        let (mut floats, mut ints) = (render(), render());
        let (Output::Offline(a), Output::Offline(b)) = (&mut floats.output, &mut ints.output)
        else {
            unreachable!()
        };
        let failed = AtomicBool::new(false);
        let (mut left, mut right) = (vec![0.0; 64], vec![0.0; 64]);
        let mut float_data = vec![0.0f32; 2 * 4800];
        let mut int_data = vec![0i16; 2 * 4800];
        fill(a, &failed, &mut float_data, 2, &mut left, &mut right);
        fill(b, &failed, &mut int_data, 2, &mut left, &mut right);
        assert!(float_data.iter().any(|x| x.abs() > 0.1));
        for (x, n) in float_data.iter().zip(&int_data) {
            assert!((x * 32768.0 - *n as f32).abs() <= 1.0, "{x} as {n}");
        }
    }

    #[test]
    fn once_the_output_device_goes_away_the_next_call_names_it() {
        let mut engine = Engine::offline(48_000.0);
        engine.device = "USB Interface".to_string();
        engine.lost.store(true, Ordering::Release);
        let error = engine
            .play(&[(0.0, Event::note(None, "saw", &[]))])
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the output device USB Interface went away, and the engine has to be started again"
        );
    }

    /// Renders `seconds` 1024 frames at a time, feeding each frame's input before it, as a sound card would.
    fn render_fed(
        engine: &mut Engine,
        feed: &mut rtrb::Producer<f32>,
        seconds: f64,
        frame: &[f32],
    ) -> (Vec<f32>, Vec<f32>) {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let frames = engine.frames(seconds) as usize;
        while left.len() < frames {
            let n = Ord::min(1024, frames - left.len());
            for _ in 0..n {
                frame.iter().for_each(|&x| feed.push(x).unwrap());
            }
            let (l, r) = engine.render(n as f64 / engine.rate).unwrap();
            left.extend(l);
            right.extend(r);
        }
        (left, right)
    }

    #[test]
    fn live_input_plays_its_channels_placed_by_pan_until_it_stops() {
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 2);
        let mono = Event::Input {
            id: Some(1),
            input: 0,
            channels: 1,
            options: vec![],
            chain: None,
        };
        engine.play(&[(0.0, mono)]).unwrap();
        let (left, right) = render_fed(&mut engine, &mut feed, 0.1, &[0.5, 0.25]);
        let centre = std::f32::consts::FRAC_1_SQRT_2;
        assert!((left[1000] - 0.5 * centre).abs() < 1e-6);
        assert!((right[1000] - 0.5 * centre).abs() < 1e-6);
        engine.play(&[(0.1, Event::Stop { ids: vec![1] })]).unwrap();
        let (left, _) = render_fed(&mut engine, &mut feed, 0.1, &[0.5, 0.25]);
        assert_eq!(left[1000], 0.0);

        let options = vec![("pan".to_string(), Value::Number(-1.0))];
        let stereo = Event::Input {
            id: None,
            input: 0,
            channels: 2,
            options,
            chain: None,
        };
        engine.play(&[(0.2, stereo)]).unwrap();
        let (left, right) = render_fed(&mut engine, &mut feed, 0.1, &[0.5, 0.25]);
        assert_eq!(left[1000], 0.5);
        assert_eq!(right[1000], 0.0);
        let beyond = Event::Input {
            id: None,
            input: 1,
            channels: 2,
            options: vec![],
            chain: None,
        };
        assert_eq!(engine.play(&[(0.3, beyond)]), Err(Error::UnknownInput(2)));
    }

    #[test]
    fn a_recording_of_an_input_writes_it_centred() {
        let dir = std::env::temp_dir().join(format!("mxs-input-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("input.wav");
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
        engine.record(&path, Source::Input(0), 0.0).unwrap();
        let (left, _) = render_fed(&mut engine, &mut feed, 0.1, &[0.5]);
        assert_eq!(left[100], 0.0);
        engine.stop_recording().unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        let samples: Vec<i32> = reader.samples::<i32>().map(|x| x.unwrap()).collect();
        assert_eq!(samples.len(), 2 * 4800);
        let centre = std::f32::consts::FRAC_1_SQRT_2;
        assert!((samples[200] as f32 / 8_388_607.0 - 0.5 * centre).abs() < 1e-4);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
