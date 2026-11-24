//! The engine: synths and effects registered by name, samples loaded from files, and events stamped with seconds, played live or rendered offline.

use crate::clock::Timing;
use crate::device::{self, Format, Range, Settings};
use crate::error::Error;
use crate::listen::Listening;
use crate::load::{Load, Meter};
use crate::mixer::{
    CAPACITY, Chain, Change, Command, Garbage, Heard, INPUTS, Input, Kept, Listened, Listener,
    Mixer, Out, PartChange, Recorder, Voice, Voices,
};
use crate::options::{Context, Control, Controls, Options};
use crate::ready::{Given, Ready};
use crate::report::{self, Analyser, Hub, Report, Reports, Tapped, Topic};
use crate::sample::{Grains, Player, SampleData, Samples, Stretcher};
use crate::spec::{Spec, Value, checked, held_warning};
use crate::synth::{BuildNote, Effect, Note, Synth};
use crate::time::{Tempo, TempoMap};
use crate::workers::{Pool, performance_cores};
use cpal::traits::{DeviceTrait, StreamTrait};
use fundsp::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

#[cfg(feature = "keys")]
mod midi;

/// How long before its time an event in beats is built and placed at its frame; a tempo change moves every event further off than this.
const LEAD: f64 = 0.15;
/// How many beats after its beat an event in beats still plays, at once.
const LATE: f64 = 0.25;
/// How often a live engine looks for the events in beats that have come near.
const TICK: Duration = Duration::from_millis(2);
/// Seconds of silence a note's end fades over, so a note cut at its end does not click.
const END_FADE: f64 = 0.01;
/// How many notes sound at once unless the caller sets another limit: what one core fills in time with the berimbau, the costliest of the crate's struck instruments.
pub const VOICES: usize = 48;

/// The default limit on notes at once for an engine filling voices on `workers` threads: [`VOICES`] on one, rising by three fifths of it for each more, so 192 on six.
pub fn voice_limit_for(workers: usize) -> usize {
    VOICES * (5 + 3 * workers.saturating_sub(1)) / 5
}

/// What a change applies to.
#[derive(Clone, Debug)]
pub enum Target {
    /// The notes and samples started with these ids.
    Sounds(Vec<u64>),
    /// Every note and sample sounding in a part or an effect chain.
    Part(u64),
    /// One effect of a chain, by its index.
    Effect(u64, usize),
    /// The send from one chain into another, as [`Engine::send`] made it; its one option is `level`.
    Send(u64, u64),
}

/// Something to play at a time.
/// [The guide to events](crate::guide::events) follows a note from the call that hands it over to the end of its sound.
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
        /// Whether the note sounds until it is let go, with its sustain left unread.
        held: bool,
    },
    /// A sample [`Engine::load_sample`] returned, with the sample player's options.
    Sample {
        /// What a later change reaches the sample by.
        id: Option<u64>,
        /// The sample, as [`Engine::load_sample`] returned it.
        sample: u64,
        /// The options of the sample player.
        options: Vec<(String, Value)>,
        /// The effect chain the sample plays into, if any.
        chain: Option<u64>,
    },
    /// Overlapping grains of a sample or a buffer, with the options [`Engine::grains_specs`] lists.
    Grains {
        /// What a later change reaches the grains by.
        id: Option<u64>,
        /// The sample or buffer, as [`Engine::load_sample`] or [`Engine::create_buffer`] returned it.
        sample: u64,
        /// The options of the grains.
        options: Vec<(String, Value)>,
        /// The effect chain the grains play into, if any.
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
    /// Lets the sounds started with these ids go, as keys let up: a note shaped by its envelope falls silent over its release, a sound that fades by itself rings on, and one that never ends fades over 10 ms.
    Release {
        /// The ids the sounds were started with.
        ids: Vec<u64>,
    },
}

/// A stretch of a part's music, with its events at their beats, which [`Engine::pass`] hands to the part.
/// [The guide to time](crate::guide::time) shows a program that hands a part one pass after another.
#[derive(Clone, Debug, Default)]
pub struct Pass {
    /// The beat it starts at.
    pub from: f64,
    /// The beat it ends at.
    pub to: f64,
    /// A number of the caller's choosing: a [`Report::PassStarted`] says when the first event of a pass starts whose label is another than that of the pass handed to the part before it.
    pub label: u64,
    /// How many beats before `to` a [`Report::PassEnding`] is sent, unless another pass was handed to the part by then; none for no report.
    pub notice: Option<f64>,
    /// Each event and its beat; the `chain` an event names is left aside, as the event plays into the pass's part.
    pub events: Vec<(f64, Event)>,
}

/// What [`Engine::record`] writes to its file.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// One channel of the sound card's input, numbered from 0.
    Input(usize),
    /// A sample [`Engine::load_sample`] returned, played once in place of the input.
    Sample(u64),
}

/// Where the sound comes from that an analysis hears, for [`Engine::analyse`], or that a recording into a buffer takes, for [`Engine::record_buffer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    /// One channel of the sound card's input, numbered from 0.
    Input(usize),
    /// A sample [`Engine::load_sample`] returned, played once in place of the input, to the analysis or the recording alone.
    Sample(u64),
    /// An effect chain's output, or a part's, after its gain, mute and solo.
    Chain(u64),
    /// The main output.
    Output,
}

impl Event {
    /// A note of `synth` with number options and no effect chain; `id` lets a later change reach it.
    ///
    /// ```
    /// use mxs::Event;
    ///
    /// let middle_c = Event::note(None, "piano", &[("note", 60.0), ("vel", 0.8)]);
    /// let held = Event::note(Some(1), "organ", &[("note", 48.0), ("sustain", 600.0)]);
    /// let let_go = Event::Release { ids: vec![1] };
    /// ```
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
            held: false,
        }
    }

    /// A note of `synth` that sounds until an [`Event::Release`] with its `id` lets it go, or until it fades by itself.
    ///
    /// ```
    /// use mxs::Event;
    ///
    /// let held = Event::held(1, "violin", &[("note", 67.0), ("vel", 0.5)]);
    /// let let_go = Event::Release { ids: vec![1] };
    /// ```
    pub fn held(id: u64, synth: &str, options: &[(&str, f64)]) -> Event {
        match Event::note(Some(id), synth, options) {
            Event::Note {
                id,
                synth,
                options,
                chain,
                ..
            } => Event::Note {
                id,
                synth,
                options,
                chain,
                held: true,
            },
            _ => unreachable!("a note is a note"),
        }
    }
}

/// Refuses a synth whose tables are not loaded.
pub(crate) fn tables_loaded(synth: &Synth) -> Result<(), Error> {
    let missing = crate::synths::missing_tables(synth.tables());
    if missing.is_empty() {
        return Ok(());
    }
    Err(Error::TablesNotLoaded {
        synth: synth.name().to_string(),
        tables: missing.into_iter().map(str::to_string).collect(),
    })
}

struct Registered<T> {
    item: T,
    owner: Arc<str>,
    specs: Arc<[Spec]>,
}

struct Sound {
    key: u64,
    chain: Option<u64>,
    /// The beat it was handed over at, when it came in a pass.
    beat: Option<f64>,
    start: f64,
    end: f64,
    controls: Controls,
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
///
/// An engine plays the synths and effects registered on it, by name, from events that each carry a time.
/// [`Engine::live`] plays through a sound card and [`Engine::offline`] renders into memory; every other call is the same for both.
///
/// ```
/// use mxs::{Engine, Event, RATE, synths};
///
/// fn main() -> Result<(), mxs::Error> {
///     let mut engine = Engine::offline(RATE);
///     synths::all().into_iter().for_each(|s| engine.register(s));
///
///     let note = Event::note(None, "cello", &[("note", 48.0), ("sustain", 1.0)]);
///     engine.play(&[(0.0, note)])?;
///     let (left, right) = engine.render(2.0)?;
///     assert_eq!(left.len(), right.len());
///     Ok(())
/// }
/// ```
///
/// # What an engine does
///
/// | To | Call | Guide |
/// | --- | --- | --- |
/// | Make one | [`live`](Engine::live), [`live_with`](Engine::live_with), [`live_with_input`](Engine::live_with_input), [`offline`](Engine::offline) | [Playing live](crate::guide::live), [How the engine is built](crate::guide::architecture) |
/// | Say what it can play | [`register`](Engine::register), [`register_effect`](Engine::register_effect), [`synths`](Engine::synths), [`effects`](Engine::effects), [`note_specs`](Engine::note_specs), [`sample_specs`](Engine::sample_specs), [`effect_specs`](Engine::effect_specs) | [A synth or an effect of your own](crate::guide::building) |
/// | Play | [`play`](Engine::play), [`play_at`](Engine::play_at), [`play_at_frames`](Engine::play_at_frames), [`sounding`](Engine::sounding), [`set_voice_limit`](Engine::set_voice_limit) | [Events and the life of a note](crate::guide::events) |
/// | Use effects and parts | [`start_effects`](Engine::start_effects), [`stop_effects`](Engine::stop_effects), [`start_main_effects`](Engine::start_main_effects), [`send`](Engine::send), [`route`](Engine::route), [`key`](Engine::key), [`start_part`](Engine::start_part), [`start_part_in_beats`](Engine::start_part_in_beats), [`part`](Engine::part), [`stop_part`](Engine::stop_part), [`set_gain`](Engine::set_gain), [`mute`](Engine::mute), [`solo`](Engine::solo) | [Effects, parts and the signal path](crate::guide::signal_path) |
/// | Read the clock | [`now`](Engine::now), [`clock`](Engine::clock), [`seconds_at`](Engine::seconds_at), [`latency`](Engine::latency), [`rate`](Engine::rate) | [Frames, seconds and beats](crate::guide::time) |
/// | Play in beats | [`place_beats`](Engine::place_beats), [`set_tempo`](Engine::set_tempo), [`pass`](Engine::pass), [`cut`](Engine::cut), [`beat`](Engine::beat), [`beat_at`](Engine::beat_at), [`beat_seconds`](Engine::beat_seconds) | [Frames, seconds and beats](crate::guide::time) |
/// | Render offline | [`render`](Engine::render), [`fill`](Engine::fill) | [How the engine is built](crate::guide::architecture) |
/// | Hear how it is doing | [`subscribe`](Engine::subscribe), [`warnings`](Engine::warnings), [`load`](Engine::load), [`behind`](Engine::behind) | [What the engine reports](crate::guide::reports), [Playing live](crate::guide::live) |
/// | Play a MIDI input on a part, with the `keys` feature | [`midi_inputs`](Engine::midi_inputs), [`play_midi`](Engine::play_midi), [`change_midi`](Engine::change_midi), [`stop_midi`](Engine::stop_midi) | [Playing live](crate::guide::live) |
/// | Play files and the input, and record | [`load_sample`](Engine::load_sample), [`load_sample_bytes`](Engine::load_sample_bytes), [`sample_channels`](Engine::sample_channels), [`inputs`](Engine::inputs), [`record_output`](Engine::record_output), [`stop_recording_output`](Engine::stop_recording_output), [`record`](Engine::record), [`stop_recording`](Engine::stop_recording) | [Samples, live input and recording](crate::guide::samples) |
pub struct Engine {
    rate: f64,
    synths: HashMap<String, Registered<Synth>>,
    effects: HashMap<String, Registered<Effect>>,
    sampler: (Arc<str>, Arc<[Spec]>),
    grainer: (Arc<str>, Arc<[Spec]>),
    input: (Arc<str>, Arc<[Spec]>),
    inputs: usize,
    recording: Option<Recording>,
    output_recording: Option<Recording>,
    samples: HashMap<u64, Arc<SampleData>>,
    book: Arc<Mutex<Book>>,
    builder: Option<Builder>,
    next: Arc<AtomicU64>,
    warned: HashSet<(String, String)>,
    hub: Arc<Hub>,
    analyser: Analyser,
    /// What an offline engine's mixer copies for the levels reports.
    tapped: Option<Tapped>,
    /// The whole second an offline engine last reported its voices in.
    voiced: Option<u64>,
    reporter: Option<JoinHandle<()>>,
    /// The reporting side of each analysis, which the reporting thread reads, or an offline engine as it renders.
    listening: Arc<Mutex<Vec<Listening>>>,
    /// The sound each analysis plays to itself alone, by the analysis.
    analyses: HashMap<u64, Option<u64>>,
    clock: Arc<AtomicU64>,
    voices: Arc<Voices>,
    meter: Arc<Meter>,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    device: String,
    timing: Option<Arc<Mutex<Timing>>>,
    output: Output,
    /// The MIDI inputs played on parts, by the input's name, or none for every input.
    #[cfg(feature = "keys")]
    players: HashMap<Option<String>, midi::Player>,
    /// How many threads fill voices, counting the one that renders.
    workers: usize,
}

impl Engine {
    /// An engine that renders offline at `rate` frames a second.
    pub fn offline(rate: f64) -> Engine {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let clock = Arc::new(AtomicU64::new(0));
        let voices = Voices::new(VOICES);
        let mut mixer = Mixer::new(receiving, returning, clock.clone(), voices.clone(), None);
        let hub = Hub::new();
        let (tap, tapped) = report::tap(hub.clone(), rate);
        mixer.report_to(tap);
        let mut engine = Engine::with(
            rate,
            commands,
            garbage,
            clock,
            voices,
            Meter::new(),
            Arc::new(AtomicBool::new(false)),
            hub,
            Output::Offline(Box::new(mixer)),
        );
        engine.tapped = Some(tapped);
        engine
    }

    /// An offline engine whose input is `channels` fed through the producer returned, for tests of live input.
    #[cfg(test)]
    pub(crate) fn offline_with_input(rate: f64, channels: usize) -> (Engine, rtrb::Producer<f32>) {
        let (commands, receiving) = rtrb::RingBuffer::new(CAPACITY);
        let (returning, garbage) = rtrb::RingBuffer::new(CAPACITY);
        let (feed, frames) = rtrb::RingBuffer::new(CAPACITY * channels);
        let clock = Arc::new(AtomicU64::new(0));
        let voices = Voices::new(VOICES);
        let mut mixer = Mixer::new(
            receiving,
            returning,
            clock.clone(),
            voices.clone(),
            Some(Input { frames, channels }),
        );
        let hub = Hub::new();
        let (tap, tapped) = report::tap(hub.clone(), rate);
        mixer.report_to(tap);
        let output = Output::Offline(Box::new(mixer));
        let mut engine = Engine::with(
            rate,
            commands,
            garbage,
            clock,
            voices,
            Meter::new(),
            Arc::new(AtomicBool::new(false)),
            hub,
            output,
        );
        engine.tapped = Some(tapped);
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
        let meter = Meter::new();
        let metering = meter.clone();
        let hub = Hub::new();
        let tapping = hub.clone();
        // cpal's streams cannot move between threads on every platform, so one thread owns them until the engine is dropped.
        std::thread::spawn(move || {
            match open(
                receiving, returning, mixing, limiting, metering, failing, losing, tapping,
                &settings,
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
            meter,
            failed,
            hub,
            Output::Live { stop },
        );
        engine.inputs = opened.inputs;
        engine.lost = lost;
        engine.device = opened.name;
        engine.timing = Some(opened.timing);
        engine.book().timing = engine.timing.clone();
        engine.builder = Some(Builder::start(engine.book.clone(), opened.rate));
        engine.hub.keep(Report::Output {
            seconds: engine.now(),
            device: Some(engine.device.clone()),
            rate: engine.rate,
            delay_ms: engine.latency().map(|seconds| seconds * 1000.0),
        });
        engine.reporter = Some(report(&engine, opened.tapped));
        Ok(engine)
    }

    fn with(
        rate: f64,
        commands: rtrb::Producer<Command>,
        garbage: rtrb::Consumer<Garbage>,
        clock: Arc<AtomicU64>,
        voices: Arc<Voices>,
        meter: Arc<Meter>,
        failed: Arc<AtomicBool>,
        hub: Arc<Hub>,
        output: Output,
    ) -> Engine {
        let sampler = Arc::from("sample");
        let next = Arc::new(AtomicU64::new(0));
        let sampler_specs: Vec<Spec> = sample_specs().into_iter().chain(placing_specs()).collect();
        hub.keep(Report::Output {
            seconds: 0.0,
            device: None,
            rate,
            delay_ms: None,
        });
        let mut engine = Engine {
            rate,
            synths: HashMap::new(),
            effects: HashMap::new(),
            sampler: (sampler, sampler_specs.into()),
            grainer: (
                Arc::from("grains"),
                grains_specs().into_iter().chain(placing_specs()).collect(),
            ),
            input: (Arc::from("input"), placing_specs().into()),
            inputs: 0,
            recording: None,
            output_recording: None,
            samples: HashMap::new(),
            book: Arc::new(Mutex::new(Book {
                rate,
                live: matches!(output, Output::Live { .. }),
                commands,
                garbage,
                sounds: HashMap::new(),
                ids: HashMap::new(),
                chains: HashMap::new(),
                sends: HashMap::new(),
                routes: HashMap::new(),
                keys: HashMap::new(),
                ranks: HashMap::new(),
                ended: std::collections::HashSet::new(),
                parts: HashMap::new(),
                map: TempoMap::new(),
                score: Vec::new(),
                endings: HashMap::new(),
                labels: HashMap::new(),
                in_beats: HashMap::new(),
                followers: Vec::new(),
                pointed: f64::NEG_INFINITY,
                cuts: (0, Vec::new()),
                notices: Vec::new(),
                timing: None,
                voices: voices.clone(),
                next: next.clone(),
                clock: clock.clone(),
                hub: hub.clone(),
                ready: HashMap::new(),
            })),
            builder: None,
            next,
            warned: HashSet::new(),
            hub,
            analyser: Analyser::new(rate),
            tapped: None,
            voiced: None,
            reporter: None,
            listening: Arc::new(Mutex::new(Vec::new())),
            analyses: HashMap::new(),
            clock,
            voices,
            meter,
            failed,
            lost: Arc::new(AtomicBool::new(false)),
            device: String::new(),
            timing: None,
            output,
            #[cfg(feature = "keys")]
            players: HashMap::new(),
            workers: 1,
        };
        engine.set_workers(performance_cores());
        engine
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
        let specs: Vec<Spec> = synth.specs.iter().cloned().chain(note_specs()).collect();
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
        let shared = blending_specs()
            .into_iter()
            .filter(|shared| effect.specs.iter().all(|own| own.name != shared.name));
        let specs: Vec<Spec> = effect.specs.iter().cloned().chain(shared).collect();
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

    /// How many seconds each of `effects`, given as [`Engine::start_effects`] takes them, sounds on after its input stops, such as a reverb's tail.
    /// With `bpm`, the options an effect declares in beats are in beats at that tempo, as [`Engine::start_part_in_beats`] takes them.
    pub fn effect_tails(
        &mut self,
        effects: &[(String, Vec<(String, Value)>)],
        bpm: Option<f64>,
    ) -> Result<Vec<f64>, Error> {
        effects
            .iter()
            .map(|(name, given)| {
                let registered = self
                    .effects
                    .get(name)
                    .ok_or_else(|| Error::UnknownEffect(name.clone()))?;
                let (owner, specs) = (registered.owner.clone(), registered.specs.clone());
                let tail = registered.item.tail.clone();
                let values = self.check(&owner, &specs, given)?;
                let mut options = Options::new(owner, specs, values);
                if let Some(bpm) = bpm {
                    options.in_beats(60.0 / bpm);
                }
                Ok(tail(&options))
            })
            .collect()
    }

    /// The options every note takes besides its synth's: amp and pan, the expressions bend, brightness and pressure, and vibrato and pedal.
    pub fn note_specs(&self) -> Vec<Spec> {
        note_specs()
    }

    /// The options every sample takes.
    pub fn sample_specs(&self) -> Vec<Spec> {
        self.sampler.1.to_vec()
    }

    /// The options grains take, from [`Event::Grains`].
    pub fn grains_specs(&self) -> Vec<Spec> {
        self.grainer.1.to_vec()
    }

    /// The options the sound card's input takes, from [`Event::Input`].
    pub fn input_specs(&self) -> Vec<Spec> {
        self.input.1.to_vec()
    }

    /// The options every effect takes besides its own: mix, pre_amp and amp.
    /// An effect that lists one of them among its own gives it its own default.
    pub fn effect_specs(&self) -> Vec<Spec> {
        blending_specs()
    }

    /// Loads a FLAC, WAV or AIFF file, and returns the number events name it by.
    pub fn load_sample(&mut self, path: impl AsRef<Path>) -> Result<u64, Error> {
        let data = SampleData::load(path.as_ref()).map_err(Error::Sample)?;
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.samples.insert(id, Arc::new(data));
        Ok(id)
    }

    /// Loads the bytes of a FLAC, WAV or AIFF file, which messages call `shown`, and returns the number events name it by.
    pub fn load_sample_bytes(&mut self, shown: &str, bytes: &[u8]) -> Result<u64, Error> {
        let data = SampleData::decode(shown, bytes).map_err(Error::Sample)?;
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.samples.insert(id, Arc::new(data));
        Ok(id)
    }

    /// How many channels a loaded sample has.
    pub fn sample_channels(&self, sample: u64) -> Option<usize> {
        self.samples.get(&sample).map(|data| data.channels.len())
    }

    /// How long filling audio took against how long the audio lasts, and how many buffers a live engine filled too late for the sound card, since the last call.
    /// An offline engine counts each stretch it renders as a buffer, a quarter second at most.
    pub fn load(&mut self) -> Load {
        self.meter.take()
    }

    /// Seconds the engine's clock has fallen behind the system's since it started: gaps in the sound, and the drift between the two clocks; a live engine knows it, an offline one does not.
    /// After a gap, [`Engine::seconds_at`] catches up by at most a millisecond a second, so until then an event given a system time plays late by what is left of the gap.
    pub fn behind(&self) -> Option<f64> {
        self.timing().and_then(|t| t.behind())
    }

    /// Values held to their ranges since the last call, once for each option, how many notes were stolen to keep to the voice limit, how many events arrived after their time, and how many buffers reached the sound card late.
    pub fn warnings(&mut self) -> Vec<String> {
        count_warnings(&self.hub, &self.meter, &self.voices, self.rate, self.now());
        self.hub.take_said()
    }

    /// Subscribes to kinds of report, which the engine then sends without being asked; the subscription ends when what is returned is dropped.
    /// A live engine works its reports out on a thread of its own, and an offline one as it renders, the same ones on every run.
    /// Subscribing changes no sample of the output, and a kind nobody subscribes to is not worked out.
    pub fn subscribe(&self, topics: &[Topic]) -> Reports {
        self.hub.subscribe(topics)
    }

    /// Sets how many threads fill the voices of a block, counting the one that renders: one fills them all there, as an engine of one thread does.
    /// An engine starts with as many as the machine has performance cores; workers take over once eight voices or more sound at once, the output is the same sample for sample whatever their number, and a limit on voices left at its default follows [`voice_limit_for`].
    pub fn set_workers(&mut self, workers: usize) {
        let workers = workers.clamp(1, 64);
        let limit = &self.voices.limit;
        let _ = limit.compare_exchange(
            voice_limit_for(self.workers),
            voice_limit_for(workers),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
        self.workers = workers;
        let _ = self
            .book()
            .send(Command::Workers(Box::new(Pool::new(workers))));
    }

    /// How many threads fill voices, counting the one that renders.
    pub fn workers(&self) -> usize {
        self.workers
    }

    /// Sets how many notes sound at once; a note past it takes the place of the quietest, which fades out over 10 ms.
    pub fn set_voice_limit(&mut self, limit: usize) {
        self.voices.limit.store(limit, Ordering::Relaxed);
    }

    /// Starts an effect chain of `effects`, each a name and its options, in order, and returns the chain notes name to play into.
    ///
    /// ```
    /// use mxs::{Engine, RATE, Value, effects};
    ///
    /// fn main() -> Result<(), mxs::Error> {
    ///     let mut engine = Engine::offline(RATE);
    ///     effects::all().into_iter().for_each(|e| engine.register_effect(e));
    ///
    ///     let echo = ("echo".to_string(), vec![("phase".to_string(), Value::Number(0.25))]);
    ///     let reverb = ("reverb".to_string(), vec![]);
    ///     let chain = engine.start_effects(&[echo, reverb])?;
    ///     engine.stop_effects(chain, 4.0)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn start_effects(
        &mut self,
        effects: &[(String, Vec<(String, Value)>)],
    ) -> Result<u64, Error> {
        self.start_chain(None, effects, false, false)
    }

    /// Starts the main effects, which everything the engine plays goes through after each part's own effects, gain, mute and solo, and returns what a change names as their chain.
    /// With `beats`, the options the effects declare in beats are given in beats, as [`Engine::start_part_in_beats`] takes them.
    /// Started again, they take the place of those before, and an empty list ends them.
    pub fn start_main_effects(
        &mut self,
        effects: &[(String, Vec<(String, Value)>)],
        beats: bool,
    ) -> Result<u64, Error> {
        self.start_chain(None, effects, beats, true)
    }

    /// Starts a part by a name, with `effects` as [`Engine::start_effects`] takes them, none for a part without effects, and returns what a note, a sample or an input names as its `chain` to play into the part.
    /// A part's gain, mute and solo apply after its effects, so an effect's tail is turned down with the part.
    pub fn start_part(
        &mut self,
        name: &str,
        effects: &[(String, Vec<(String, Value)>)],
    ) -> Result<u64, Error> {
        if self.book().parts.contains_key(name) {
            return Err(Error::PartExists(name.to_string()));
        }
        self.start_chain(Some(name), effects, false, false)
    }

    /// Starts a part as [`Engine::start_part`] does, with the options its effects declare in beats given in beats, the ones left out too.
    /// Each lasts those beats at the tempo of the beat the engine is playing, and those an effect lets change while it runs, such as the time between an echo's repeats, follow the tempo as it changes.
    pub fn start_part_in_beats(
        &mut self,
        name: &str,
        effects: &[(String, Vec<(String, Value)>)],
    ) -> Result<u64, Error> {
        if self.book().parts.contains_key(name) {
            return Err(Error::PartExists(name.to_string()));
        }
        self.start_chain(Some(name), effects, true, false)
    }

    /// What a note names as its `chain` to play into the part started by `name`.
    pub fn part(&self, name: &str) -> Result<u64, Error> {
        let part = self.book().parts.get(name).copied();
        part.ok_or_else(|| Error::UnknownPart(name.to_string()))
    }

    /// Removes a part at `at` seconds, once its sounds and its effects' tails have ended; its name is free for another part at once.
    pub fn stop_part(&mut self, name: &str, at: f64) -> Result<(), Error> {
        let part = self.part(name)?;
        self.stop_effects(part, at)?;
        self.book().parts.remove(name);
        Ok(())
    }

    /// Sets a part's gain from `at` seconds, reached over 10 ms; 1 leaves the part as it is, and 0 silences it.
    pub fn set_gain(&mut self, part: &str, gain: f64, at: f64) -> Result<(), Error> {
        self.change_part(part, at, PartChange::Gain(gain.max(0.0) as f32))
    }

    /// Mutes a part from `at` seconds, or lets it be heard again, over 10 ms; a muted part plays on unheard, the sounds handed over before the mute included.
    pub fn mute(&mut self, part: &str, muted: bool, at: f64) -> Result<(), Error> {
        self.change_part(part, at, PartChange::Mute(muted))
    }

    /// Solos a part from `at` seconds, or ends its solo: while any part is soloed, only the soloed parts are heard, a sound that plays into no part is heard as ever, and so is a chain that others send or route into, which then plays only what the soloed parts feed it.
    pub fn solo(&mut self, part: &str, soloed: bool, at: f64) -> Result<(), Error> {
        self.change_part(part, at, PartChange::Solo(soloed))
    }

    fn change_part(&mut self, part: &str, at: f64, change: PartChange) -> Result<(), Error> {
        self.check_running()?;
        let key = self.part(part)?;
        let at = self.frames(at);
        self.book().send(Command::Part { key, at, change })
    }

    fn start_chain(
        &mut self,
        name: Option<&str>,
        effects: &[(String, Vec<(String, Value)>)],
        beats: bool,
        main: bool,
    ) -> Result<u64, Error> {
        self.check_running()?;
        let mut unit: Net = Net::wrap(Box::new(multipass::<U2>()));
        let mut controls = Vec::new();
        let mut tail = 0.0;
        let bpm = {
            let book = self.book();
            book.map
                .bpm_at(book.pointed.max(book.map.beat_at(book.now())))
        };
        let mut in_beats = Vec::new();
        let mut followers = Vec::new();
        let tap = Arc::new(crate::options::KeyTap::new());
        for (name, given) in effects {
            let registered = self
                .effects
                .get(name)
                .ok_or_else(|| Error::UnknownEffect(name.clone()))?;
            let (owner, specs) = (registered.owner.clone(), registered.specs.clone());
            let (build, effect_tail) =
                (registered.item.build.clone(), registered.item.tail.clone());
            let late = registered.item.late;
            let values = self.check(&owner, &specs, given)?;
            let mut options =
                Options::new(owner.clone(), specs.clone(), values).keyed_by(tap.clone());
            let mut given = Vec::new();
            if beats {
                given = options.beats();
                options.in_beats(60.0 / bpm);
            }
            let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build(&options, &mut Context::new(owner.clone()))
            }));
            let wet = built.map_err(|panic| {
                let said = panic.downcast_ref::<String>().cloned();
                let said = said.or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()));
                let reason = said.unwrap_or_else(|| "it panicked".to_string());
                let reason = reason
                    .strip_prefix(&format!("{name}: "))
                    .unwrap_or(&reason)
                    .to_string();
                Error::CannotBuild {
                    owner: owner.to_string(),
                    reason,
                }
            })?;
            if wet.inputs() != 2 || wet.outputs() != 2 {
                return Err(Error::BadSound {
                    owner: owner.to_string(),
                    outputs: wet.outputs(),
                });
            }
            let pre = options.follow("pre_amp") >> Net::wrap(Box::new(split::<U2>()));
            let wet = (Net::wrap(Box::new(multipass::<U2>())) * pre) >> wet;
            let dry = if late == 0 {
                Net::wrap(Box::new(multipass::<U2>()))
            } else {
                Net::wrap(Box::new(An(Held::new(late)) | An(Held::new(late))))
            };
            let blended = (dry ^ wet) | options.follow("mix") | options.follow("amp");
            unit = unit >> (blended >> Net::wrap(Box::new(An(Blend))));
            let changing = options.take_controls();
            let follows = |name: &String| changing.follows(name);
            let index = controls.len();
            followers.extend(
                given
                    .into_iter()
                    .filter(|(name, _)| follows(name))
                    .map(|(name, beats)| (index, name, beats)),
            );
            in_beats.push(
                specs
                    .iter()
                    .filter(|spec| spec.counts_beats())
                    .map(|spec| spec.name.clone())
                    .filter(follows)
                    .collect(),
            );
            controls.push(changing);
            tail += effect_tail(&options);
        }
        unit.set_sample_rate(self.rate);
        unit.allocate();
        let key = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let chain = Chain {
            key,
            unit: Box::new(unit),
            input: BufferVec::new(2),
            stop: None,
            tail: self.frames(tail),
            heard: Heard::new(self.frames(END_FADE)),
            out: None,
            sends: Vec::with_capacity(crate::mixer::SENDS),
            key_from: None,
            tap,
            rank: 0,
            fed: 0,
            played: (0, [0.0; 2], [0.0; 2]),
        };
        let mut book = self.book();
        let chain = Box::new(chain);
        book.send(if main {
            Command::StartMain(chain)
        } else {
            Command::StartChain(chain)
        })?;
        book.chains.insert(key, controls);
        book.in_beats.insert(key, in_beats);
        let followers = followers
            .into_iter()
            .map(|(index, name, beats)| (key, index, name, beats));
        book.followers.extend(followers);
        if let Some(name) = name {
            book.parts.insert(name.to_string(), key);
            book.hub.name_part(key, name);
        }
        Ok(key)
    }

    /// Removes an effect chain at `at` seconds, once its notes and its effects' tails have ended.
    pub fn stop_effects(&mut self, chain: u64, at: f64) -> Result<(), Error> {
        self.settle();
        let at = self.frames(at);
        let mut book = self.book();
        if !book.chains.contains_key(&chain) {
            return Err(Error::UnknownChain(chain));
        }
        book.send(Command::StopChain { key: chain, at })
    }

    /// Sends a share of the sound of the chain `from` into the chain `to`, at `level`, besides where that sound already goes; a send already made between the two takes the level instead.
    /// The share is taken after the chain's effects and its part's gain, mute and solo, and reaches `to` at the same sample, with no delay.
    /// An [`Event::Set`] to [`Target::Send`] changes its `level` while the sound plays, gliding over `level_slide` seconds.
    /// A send that would make a chain feed itself, directly or through others, is refused with [`Error::FeedsItself`].
    ///
    /// ```
    /// use mxs::{Engine, RATE, effects};
    ///
    /// fn main() -> Result<(), mxs::Error> {
    ///     let mut engine = Engine::offline(RATE);
    ///     effects::all().into_iter().for_each(|e| engine.register_effect(e));
    ///
    ///     let reverb = engine.start_effects(&[("reverb".to_string(), vec![])])?;
    ///     let keys = engine.start_part("keys", &[])?;
    ///     let bass = engine.start_part("bass", &[])?;
    ///     engine.send(keys, reverb, 0.5)?;
    ///     engine.send(bass, reverb, 0.1)?;
    ///     Ok(())
    /// }
    /// ```
    pub fn send(&mut self, from: u64, to: u64, level: f64) -> Result<(), Error> {
        self.check_running()?;
        self.settle();
        let mut book = self.book();
        book.check_chains(&[from, to])?;
        if let Some(control) = book.sends.get(&(from, to)) {
            control.target.set(level as f32);
            control.slide.set(0.0);
            return Ok(());
        }
        if book
            .sends
            .keys()
            .filter(|(sender, _)| *sender == from)
            .count()
            >= crate::mixer::SENDS
        {
            return Err(Error::TooManySends {
                chain: from,
                most: crate::mixer::SENDS,
            });
        }
        book.feed(from, to)?;
        let spec = Spec::float("level").default(level);
        let options = Options::new("send".into(), vec![spec].into(), HashMap::new());
        let mut unit = options.follow("level");
        unit.set_sample_rate(self.rate);
        unit.allocate();
        let control = options
            .take_controls()
            .get("level")
            .expect("followed")
            .clone();
        let send = crate::mixer::Send {
            from,
            to,
            level: Box::new(unit),
        };
        book.send(Command::Send(Box::new(send)))?;
        book.sends.insert((from, to), control);
        Ok(())
    }

    /// Sends the whole sound of `chain` into the chain `into` in place of the main output, or back to the main output with `None`.
    /// It reaches `into` at the same sample, after the chain's effects and its part's gain, mute and solo, and a route that would make a chain feed itself is refused with [`Error::FeedsItself`].
    pub fn route(&mut self, chain: u64, into: Option<u64>) -> Result<(), Error> {
        self.check_running()?;
        self.settle();
        let mut book = self.book();
        book.check_chains(&[chain])?;
        if let Some(into) = into {
            book.check_chains(&[into])?;
            book.feed(chain, into)?;
        }
        book.send(Command::Route {
            key: chain,
            out: into,
        })?;
        match into {
            Some(into) => book.routes.insert(chain, into),
            None => book.routes.remove(&chain),
        };
        Ok(())
    }

    /// Has the effects of `chain` that read a key, such as the compressor, read the sound of the chain `by` in place of their own input, or their own input again with `None`.
    /// A compressor keyed so lowers its own sound by the level of `by`, which sounds as it would without it.
    /// A key that would make a chain feed itself is refused with [`Error::FeedsItself`], as the chain `by` has to play first.
    pub fn key(&mut self, chain: u64, by: Option<u64>) -> Result<(), Error> {
        self.check_running()?;
        self.settle();
        let mut book = self.book();
        book.check_chains(&[chain])?;
        if let Some(by) = by {
            book.check_chains(&[by])?;
            book.feed(by, chain)?;
        }
        book.send(Command::Key { key: chain, by })?;
        match by {
            Some(by) => book.keys.insert(chain, by),
            None => book.keys.remove(&chain),
        };
        Ok(())
    }

    /// Starts an analysis of `sound` from `at` seconds and returns its id: the subscribers to [`Topic::Analysis`] are sent its loudness and its pitch every `interval` seconds, and each onset as it happens, until [`Engine::stop_analysis`].
    /// It leaves the sound as it is, and several run at once.
    pub fn analyse(&mut self, sound: Origin, interval: f64, at: f64) -> Result<u64, Error> {
        self.check_running()?;
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let (listening, rate) = (self.listening.clone(), self.rate);
        let interval = Ord::max((interval * rate).round() as u64, 1);
        let key = self.listen(id, sound, at, None, |start| {
            let (samples, heard) = rtrb::RingBuffer::new(rate as usize);
            let (spans, spanned) = rtrb::RingBuffer::new(4096);
            let analysis = Listening::new(id, heard, spanned, rate, interval, start);
            listening
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(analysis);
            Kept::Rings { samples, spans }
        })?;
        self.analyses.insert(id, key);
        Ok(id)
    }

    /// Starts copying `sound` from `at` seconds, for `frames` or until stopped, into what `keep` makes from the frame it starts at, and returns the key of the voice that plays a sound to it alone.
    fn listen(
        &mut self,
        id: u64,
        sound: Origin,
        at: f64,
        frames: Option<u64>,
        keep: impl FnOnce(u64) -> Kept,
    ) -> Result<Option<u64>, Error> {
        self.settle();
        let (listened, draft) = match sound {
            Origin::Input(input) => (
                Listened::Own,
                Some(self.input_draft(None, input, 1, &[], at, Out::Listen(id))?),
            ),
            Origin::Sample(sample) => (
                Listened::Own,
                Some(self.sample_draft(None, sample, &[], at, Out::Listen(id))?),
            ),
            Origin::Chain(chain) if self.book().chains.contains_key(&chain) => {
                (Listened::Chain(chain), None)
            }
            Origin::Chain(chain) => return Err(Error::UnknownChain(chain)),
            Origin::Output => (Listened::Main, None),
        };
        let started = match draft {
            Some(draft) => {
                let built = build(draft, self.rate)?;
                let mut book = self.book();
                book.sweep();
                Some(book.start(built))
            }
            None => None,
        };
        let start = match &started {
            Some((voice, _)) => voice.start,
            None => Ord::max(self.frames(at), self.clock.load(Ordering::Acquire)),
        };
        let stop = frames.map(|frames| start + frames);
        let listener = Listener {
            id,
            listened,
            start,
            stop,
            sound: BufferVec::new(2),
            kept: keep(start),
        };
        let mut book = self.book();
        book.send(Command::Listen(Box::new(listener)))?;
        let Some((voice, sound)) = started else {
            return Ok(None);
        };
        let key = voice.key;
        book.sounds.insert(sound.key, sound);
        book.send(Command::Start(voice))?;
        if let Some(at) = stop {
            book.send(Command::Stop { key, at })?;
        }
        Ok(Some(key))
    }

    /// Creates a silent buffer of `seconds` at the engine's rate, with one channel or two, and returns the number events name it by, as [`Engine::load_sample`] does.
    /// It plays as a sample does, and [`Engine::record_buffer`] records into it.
    pub fn create_buffer(&mut self, seconds: f64, channels: usize) -> Result<u64, Error> {
        let frames = self.frames(seconds) as usize;
        let data = SampleData {
            channels: (0..channels.clamp(1, 2))
                .map(|_| Samples::silent(frames))
                .collect(),
            rate: self.rate,
            peak: 0.0,
        };
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        self.samples.insert(id, Arc::new(data));
        Ok(id)
    }

    /// Records `sound` into `buffer` from its first frame, from `at` seconds for `seconds`, keeping what was there scaled by `level`: 0 replaces it and 1 adds over it.
    /// A one-channel buffer takes the sound's two channels summed at the level of a sound in the middle; a sample played in place of an input plays to the recording alone.
    /// Several recordings into buffers run at once, and a sound that plays the buffer meanwhile plays what has been written by the time it reads each frame.
    pub fn record_buffer(
        &mut self,
        buffer: u64,
        sound: Origin,
        at: f64,
        seconds: f64,
        level: f64,
    ) -> Result<(), Error> {
        self.check_running()?;
        let data = self
            .samples
            .get(&buffer)
            .ok_or(Error::UnknownSample(buffer))?
            .clone();
        let frames = Ord::min(self.frames(seconds), data.frames() as u64);
        let id = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let level = level as f32;
        self.listen(id, sound, at, Some(frames), |_| Kept::Buffer {
            data,
            level,
        })?;
        Ok(())
    }

    /// Records into `buffer` as [`Engine::record_buffer`] does, from `beat` for `beats`, at the seconds those beats fall on by the tempo map as it is when called.
    pub fn record_buffer_in_beats(
        &mut self,
        buffer: u64,
        sound: Origin,
        beat: f64,
        beats: f64,
        level: f64,
    ) -> Result<(), Error> {
        let (from, to) = (self.beat_seconds(beat), self.beat_seconds(beat + beats));
        self.record_buffer(buffer, sound, from, to - from, level)
    }

    /// Writes a sample or a buffer, as it is now, to a 32-bit float WAV file at `path`, with its channels and its rate.
    pub fn save_sample(&self, sample: u64, path: impl AsRef<Path>) -> Result<(), Error> {
        let data = self
            .samples
            .get(&sample)
            .ok_or(Error::UnknownSample(sample))?;
        let shown = path.as_ref().display();
        let spec = hound::WavSpec {
            channels: data.channels.len() as u16,
            sample_rate: data.rate.round() as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let failed = |e: hound::Error| Error::Record(format!("cannot write {shown}: {e}"));
        let mut wav = hound::WavWriter::create(path.as_ref(), spec).map_err(failed)?;
        for frame in 0..data.frames() {
            for channel in &data.channels {
                wav.write_sample(channel.get(frame).unwrap_or(0.0))
                    .map_err(failed)?;
            }
        }
        wav.finalize().map_err(failed)
    }

    /// Lets go of a sample or a buffer: events can no longer name it, and it is freed once the sounds playing it and the recordings into it have ended.
    pub fn free_sample(&mut self, sample: u64) -> Result<(), Error> {
        self.samples
            .remove(&sample)
            .map(drop)
            .ok_or(Error::UnknownSample(sample))
    }

    /// Stops the analysis `id` at `at` seconds; the reports of what it heard up to then are still sent.
    pub fn stop_analysis(&mut self, id: u64, at: f64) -> Result<(), Error> {
        let key = self
            .analyses
            .remove(&id)
            .ok_or(Error::UnknownAnalysis(id))?;
        self.settle();
        let at = self.frames(at);
        let mut book = self.book();
        book.send(Command::StopListening { id, at })?;
        if let Some(key) = key {
            book.send(Command::Stop { key, at })?;
        }
        Ok(())
    }

    /// Records `source` to a stereo 24-bit WAV file at `path` from `at` seconds, apart from the main output, until [`Engine::stop_recording`].
    /// It replaces a recording already running.
    pub fn record(&mut self, path: impl AsRef<Path>, source: Source, at: f64) -> Result<(), Error> {
        self.check_running()?;
        self.stop_recording()?;
        self.settle();
        let draft = match source {
            Source::Input(input) => self.input_draft(None, input, 1, &[], at, Out::Tap)?,
            Source::Sample(sample) => self.sample_draft(None, sample, &[], at, Out::Tap)?,
        };
        let built = build(draft, self.rate)?;
        let (voice, sound) = {
            let mut book = self.book();
            book.sweep();
            book.start(built)
        };
        let (recorder, mut recording) = self.recorder(path.as_ref(), voice.start, false)?;
        recording.source = Some(voice.key);
        let mut book = self.book();
        book.sounds.insert(sound.key, sound);
        book.send(Command::Start(voice))?;
        book.send(Command::Record(Box::new(recorder)))?;
        drop(book);
        self.recording = Some(recording);
        Ok(())
    }

    /// Records the main output to a stereo 32-bit float WAV file at `path` from `at` seconds, until [`Engine::stop_recording_output`].
    /// It runs beside a recording of an input, and replaces a recording of the output already running.
    pub fn record_output(&mut self, path: impl AsRef<Path>, at: f64) -> Result<(), Error> {
        self.check_running()?;
        self.stop_recording_output()?;
        let start = Ord::max(self.frames(at), self.clock.load(Ordering::Acquire));
        let (recorder, recording) = self.recorder(path.as_ref(), start, true)?;
        self.book()
            .send(Command::RecordOutput(Box::new(recorder)))?;
        self.output_recording = Some(recording);
        Ok(())
    }

    /// Stops recording the output and closes its file, with what was recorded up to now.
    pub fn stop_recording_output(&mut self) -> Result<(), Error> {
        let Some(recording) = self.output_recording.take() else {
            return Ok(());
        };
        self.settle();
        let sent = self.book().send(Command::StopRecordingOutput);
        self.finish(recording, sent)
    }

    /// Whether any note or sample started is still sounding; live input does not count.
    pub fn sounding(&self) -> bool {
        let now = self.now();
        self.book()
            .sounds
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
        self.settle();
        let now = self.clock.load(Ordering::Acquire);
        let mut book = self.book();
        let mut sent = Ok(());
        if let Some(source) = recording.source {
            book.sounds.remove(&source);
            sent = book.send(Command::Stop {
                key: source,
                at: now,
            });
        }
        let sent = sent.and_then(|()| book.send(Command::StopRecording));
        drop(book);
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
            self.book().drain();
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
    /// A live engine builds the notes' voices on a thread of its own, in the order they were handed over, so the call returns once the events are checked; a note handed over with less time than its voice takes to build starts late, and [`Engine::warnings`] counts it.
    /// One mistake shows only as a voice is built, a synth whose sound has neither 1 nor 2 channels: an offline engine still refuses the batch, and a live one leaves that note out and says so in [`Engine::warnings`].
    pub fn play(&mut self, events: &[(f64, Event)]) -> Result<(), Error> {
        self.check_running()?;
        let mut drafts = Vec::with_capacity(events.len());
        let mut fixed = Vec::new();
        for (at, event) in events {
            if let Event::Set { target, options } = event
                && let Target::Effect(chain, index) = target
            {
                let names = options.iter().map(|(name, _)| name.as_str());
                fixed.extend(names.map(|name| (*chain, *index, name)));
            }
            if let Event::Set { target, .. } = event
                && let Target::Send(from, to) = target
                && !self.book().sends.contains_key(&(*from, *to))
            {
                return Err(Error::UnknownSend(*from, *to));
            }
            drafts.push(self.draft(*at, event, None)?);
        }
        if !fixed.is_empty() {
            // An effect's option set in seconds stops following the tempo.
            let followers = &mut self.book().followers;
            followers.retain(|f| !fixed.contains(&(f.0, f.1, f.2.as_str())));
        }
        let (drafts, taken, replace) = self.take_ready(events, drafts);
        if self.builder.is_some() {
            if !taken.is_empty() {
                let mut book = self.book();
                book.sweep();
                book.commit(taken)?;
            }
            self.replace_ready(&replace);
            if let Some(builder) = &self.builder {
                builder.hand(drafts);
            }
            return Ok(());
        }
        self.replace_ready(&replace);
        // A batch's changes are made once its sounds start, so the ready voices can go first.
        let mut items = taken;
        for draft in drafts {
            items.push(match draft {
                Draft::Sound(sound) => Item::Sound(Box::new(build(*sound, self.rate)?)),
                Draft::Change(at, event) => Item::Change(at, event),
            });
        }
        let mut book = self.book();
        book.sweep();
        book.commit(items)
    }

    /// Takes a voice kept ready for each note of `events` that one can play, in place of its draft, with the synth of each taken, to replace it; the rest of the drafts are left to build.
    fn take_ready(
        &mut self,
        events: &[(f64, Event)],
        drafts: Vec<Draft>,
    ) -> (Vec<Draft>, Vec<Item>, Vec<Arc<str>>) {
        let mut book = self.book();
        if book.ready.values().all(Vec::is_empty) {
            return (drafts, Vec::new(), Vec::new());
        }
        let mut left = Vec::with_capacity(drafts.len());
        let mut taken = Vec::new();
        let mut replace = Vec::new();
        for (draft, (_, event)) in drafts.into_iter().zip(events) {
            match draft {
                Draft::Sound(sound) => match book.take_ready(event, &sound) {
                    Some(built) => {
                        replace.push(sound.options.owner.clone());
                        taken.push(Item::Sound(Box::new(built)));
                    }
                    None => left.push(Draft::Sound(sound)),
                },
                change => left.push(change),
            }
        }
        (left, taken, replace)
    }

    /// Builds a voice to keep ready for each synth named, on the builder thread of a live engine and at once on an offline one.
    fn replace_ready(&mut self, synths: &[Arc<str>]) {
        let mut drafts = Vec::with_capacity(synths.len());
        for synth in synths {
            if let Ok(mut draft) = self.note_draft(None, synth, &[], false, 0.0, Out::Main) {
                draft.ready = true;
                drafts.push(Draft::Sound(Box::new(draft)));
            }
        }
        if let Some(builder) = &self.builder {
            builder.hand(drafts);
            return;
        }
        let rate = self.rate;
        let built: Vec<(Arc<str>, Built)> = drafts
            .into_iter()
            .filter_map(|draft| match draft {
                Draft::Sound(sound) => {
                    let owner = sound.options.owner.clone();
                    build(*sound, rate).ok().map(|built| (owner, built))
                }
                Draft::Change(..) => None,
            })
            .collect();
        self.book().keep(built);
    }

    /// Keeps `count` voices of `synth` ready, built before any note asks for one, so a note of it, held or not, handed over with no options but note, vel, bend, brightness, pressure and a pedal of 0 or 1, starts from one in the block being filled, with nothing built or allocated on the audio thread.
    /// Each voice taken is replaced off the audio thread: on a live engine's builder thread, and on an offline one as the note is handed over.
    /// The voices are built here, so the call takes as long as building them; a count of 0 stops keeping voices of `synth` ready.
    /// Only notes handed over by seconds or frames take a ready voice; a note in a pass is built as before.
    /// A synth fitted to recordings of struck notes can be kept ready; one that builds each note for its note and vel alone is refused with [`Error::CannotKeepReady`].
    pub fn keep_ready(&mut self, synth: &str, count: usize) -> Result<(), Error> {
        self.check_running()?;
        let mut kept = Vec::with_capacity(count);
        for _ in 0..Ord::max(count, 1) {
            let mut draft = self.note_draft(None, synth, &[], false, 0.0, Out::Main)?;
            draft.ready = true;
            let built = build(draft, self.rate)?;
            if built.ready.is_none() {
                return Err(Error::CannotKeepReady(synth.to_string()));
            }
            kept.push(built);
        }
        kept.truncate(count);
        self.book().ready.insert(synth.to_string(), kept);
        Ok(())
    }

    /// How many voices of `synth` are kept ready now.
    pub fn kept_ready(&self, synth: &str) -> usize {
        self.book().ready.get(synth).map_or(0, Vec::len)
    }

    /// An event checked, to play at `at` seconds into the output it names, or into `part` when one is given.
    fn draft(&mut self, at: f64, event: &Event, part: Option<u64>) -> Result<Draft, Error> {
        let out = |chain: &Option<u64>| part.or(*chain).map_or(Out::Main, Out::Chain);
        Ok(match event {
            Event::Note {
                id,
                synth,
                options,
                chain,
                held,
            } => Draft::Sound(Box::new(self.note_draft(
                *id,
                synth,
                options,
                *held,
                at,
                out(chain),
            )?)),
            Event::Sample {
                id,
                sample,
                options,
                chain,
            } => Draft::Sound(Box::new(self.sample_draft(
                *id,
                *sample,
                options,
                at,
                out(chain),
            )?)),
            Event::Grains {
                id,
                sample,
                options,
                chain,
            } => Draft::Sound(Box::new(self.grains_draft(
                *id,
                *sample,
                options,
                at,
                out(chain),
            )?)),
            Event::Input {
                id,
                input,
                channels,
                options,
                chain,
            } => Draft::Sound(Box::new(self.input_draft(
                *id,
                *input,
                *channels,
                options,
                at,
                out(chain),
            )?)),
            change => Draft::Change(at, change.clone()),
        })
    }

    /// Places beat 0 of the tempo map at `seconds` of the engine's clock; it is at 0 until placed.
    /// The events in beats that are more than 150 ms from sounding move with it.
    pub fn place_beats(&mut self, seconds: f64) -> Result<(), Error> {
        self.check_running()?;
        self.place(self.now() + LEAD, |book| {
            book.map.place(seconds);
            book.pointed = f64::NEG_INFINITY;
        })
    }

    /// Changes the tempo from `beat` on, in place of whatever the map held from that beat on; the tempo is 120 beats a minute until changed.
    /// Every event in beats that is more than 150 ms from sounding moves to where its beat now falls, and the effect options in beats follow.
    pub fn set_tempo(&mut self, beat: f64, tempo: Tempo) -> Result<(), Error> {
        self.check_running()?;
        self.place(self.now() + LEAD, |book| {
            book.map.set(beat, tempo);
            book.repoint(beat);
        })
    }

    /// The beat of the frame the engine fills next.
    pub fn beat(&self) -> f64 {
        self.beat_at(self.now())
    }

    /// The beat that falls at `seconds` of the engine's clock, by the tempo map as it is now.
    pub fn beat_at(&self, seconds: f64) -> f64 {
        self.book().map.beat_at(seconds)
    }

    /// The engine's seconds at which `beat` falls, by the tempo map as it is now.
    pub fn beat_seconds(&self, beat: f64) -> f64 {
        self.book().map.seconds_at(beat)
    }

    /// Hands a part a pass: events at their beats, which the engine places at the frames those beats fall on by the tempo map, 150 ms before each sounds.
    /// An option a synth or an effect declares in beats, and every `_slide`, is given in beats, and lasts as long as those beats last at the tempo of the event's beat.
    /// An event whose beat has passed plays at once when it is less than a quarter of a beat late, and is dropped otherwise; [`Engine::warnings`] counts both.
    /// Every event is checked before any is kept, so a pass with a mistake plays nothing.
    pub fn pass(&mut self, part: &str, pass: &Pass) -> Result<(), Error> {
        self.check_running()?;
        let key = self.part(part)?;
        let mut scored = Vec::with_capacity(pass.events.len());
        for (beat, event) in &pass.events {
            scored.push(Scored {
                beat: *beat,
                part: key,
                starts: None,
                draft: self.draft(0.0, event, Some(key))?,
            });
        }
        scored.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        let mut book = self.book();
        if let Some(first) = scored.first_mut()
            && book.labels.insert(key, pass.label) != Some(pass.label)
        {
            first.starts = Some(pass.label);
        }
        match pass.notice {
            Some(notice) => book.endings.insert(key, (pass.to, notice, false)),
            None => book.endings.remove(&key),
        };
        book.score.extend(scored);
        book.score.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        drop(book);
        if let Some(builder) = &self.builder {
            builder.hand(Vec::new());
        }
        Ok(())
    }

    /// Cuts a part from `beat`: every event in beats handed to it for that beat or after, which has not started, is dropped, so that another pass can be handed over in its place.
    /// A note that started before the cut rings on, and a release dropped with the rest no longer ends it.
    pub fn cut(&mut self, part: &str, beat: f64) -> Result<(), Error> {
        self.check_running()?;
        let key = self.part(part)?;
        self.book().cut(key, beat)
    }

    /// Takes what falls before `horizon` seconds out of the events in beats and sends it on, to the builder thread of a live engine and to the mixer of an offline one, then makes `change` to the tempo map and takes what falls before `horizon` by the map as changed.
    fn place(&self, horizon: f64, change: impl FnOnce(&mut Book)) -> Result<(), Error> {
        let mut book = self.book();
        let mut drafts = book.due(horizon);
        change(&mut book);
        drafts.extend(book.due(horizon));
        book.tell();
        if let Some(builder) = &self.builder {
            drop(book);
            builder.hand(drafts);
            return Ok(());
        }
        if drafts.is_empty() {
            return Ok(());
        }
        let mut items = Vec::with_capacity(drafts.len());
        for draft in drafts {
            items.push(match draft {
                Draft::Sound(sound) => Item::Sound(Box::new(build(*sound, self.rate)?)),
                Draft::Change(at, event) => Item::Change(at, event),
            });
        }
        book.sweep();
        book.commit(items)
    }

    fn book(&self) -> std::sync::MutexGuard<'_, Book> {
        self.book.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits until a live engine has built and sent every event handed over so far.
    fn settle(&self) {
        if let Some(builder) = &self.builder {
            builder.settle();
        }
    }

    /// Renders the next `seconds` of an offline engine, as its left and right channels.
    /// The samples are the same whatever the length of each stretch a render is asked for in.
    pub fn render(&mut self, seconds: f64) -> Result<(Vec<f32>, Vec<f32>), Error> {
        let frames = self.frames(seconds) as usize;
        let mut left = vec![0.0; frames];
        let mut right = vec![0.0; frames];
        self.fill_from(&mut left, &mut right, true)?;
        Ok((left, right))
    }

    /// Fills `left` and `right`, of one length, with the next frames of an offline engine.
    /// It is the layer under the tempo map and needs neither the map nor a sound card: it plays the events handed over at frames or at seconds, and of the events in beats only those [`Engine::render`] has already placed.
    pub fn fill(&mut self, left: &mut [f32], right: &mut [f32]) -> Result<(), Error> {
        let frames = Ord::min(left.len(), right.len());
        self.fill_from(&mut left[..frames], &mut right[..frames], false)
    }

    /// Plays events, each at a frame of the engine's clock, as [`Engine::play`] does at seconds.
    pub fn play_at_frames(&mut self, events: &[(u64, Event)]) -> Result<(), Error> {
        let rate = self.rate;
        let timed = |(frame, event): &(u64, Event)| (*frame as f64 / rate, event.clone());
        self.play(&events.iter().map(timed).collect::<Vec<_>>())
    }

    fn fill_from(&mut self, left: &mut [f32], right: &mut [f32], beats: bool) -> Result<(), Error> {
        self.check_running()?;
        if !matches!(self.output, Output::Offline(_)) {
            return Err(Error::Live);
        }
        // Rendering outruns a recording's writer, so it goes a quarter second at a time, once the recordings have room for it.
        let chunk = Ord::max((self.rate / 4.0) as usize, 1);
        let mut filled = Ok(());
        for (left, right) in left.chunks_mut(chunk).zip(right.chunks_mut(chunk)) {
            if beats {
                let horizon = self.now() + left.len() as f64 / self.rate + LEAD;
                filled = self.place(horizon, |_| {});
            }
            filled = filled.and_then(|()| self.fill_chunk(left, right));
            if filled.is_err() {
                break;
            }
        }
        if self.hub.wants(Topic::Warnings) {
            count_warnings(&self.hub, &self.meter, &self.voices, self.rate, self.now());
        }
        let mut book = self.book();
        book.drain();
        if beats {
            book.tell();
        }
        drop(book);
        filled
    }

    fn fill_chunk(&mut self, left: &mut [f32], right: &mut [f32]) -> Result<(), Error> {
        let (meter, rate) = (self.meter.clone(), self.rate);
        let (hub, clock, voices) = (self.hub.clone(), self.clock.clone(), self.voices.clone());
        let (analyser, voiced) = (&mut self.analyser, &mut self.voiced);
        let tapped = &mut self.tapped;
        let listening = self.listening.clone();
        let Output::Offline(mixer) = &mut self.output else {
            return Err(Error::Live);
        };
        let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            mixer.receive();
            while mixer.recording_room() < left.len() {
                std::thread::sleep(Duration::from_millis(1));
            }
            let first = clock.load(Ordering::Acquire);
            // WebAssembly without a host has no clock, so an offline render there records no load.
            let began = (!cfg!(target_family = "wasm")).then(Instant::now);
            mixer.render(left, right);
            let took = began.map_or(Duration::ZERO, |began| began.elapsed());
            meter.record(took, left.len(), rate, false);
            if let Some(tapped) = tapped {
                tapped.report(analyser, &hub);
            }
            hear(&listening, &hub);
            let seconds = (first + left.len() as u64) as f64 / rate;
            let whole = seconds.floor() as u64;
            if hub.wants(Topic::Voices) && *voiced != Some(whole) {
                *voiced = Some(whole);
                let sounding = voices.sounding.load(Ordering::Relaxed);
                hub.send(Report::Voices { seconds, sounding });
            }
        }));
        if rendered.is_err() {
            self.failed.store(true, Ordering::Release);
            return Err(Error::Failed);
        }
        Ok(())
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

    /// The values to play: each given value checked against its declaration and held to its range.
    fn check(
        &mut self,
        owner: &str,
        specs: &[Spec],
        given: &[(String, Value)],
    ) -> Result<HashMap<String, Value>, Error> {
        let (values, held) = checked(owner, specs, given)?;
        for name in held {
            if self.warned.insert((owner.to_string(), name.clone())) {
                self.hub.warn(self.now(), held_warning(owner, specs, &name));
            }
        }
        Ok(values)
    }

    fn note_draft(
        &mut self,
        id: Option<u64>,
        synth: &str,
        given: &[(String, Value)],
        held: bool,
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let registered = self
            .synths
            .get(synth)
            .ok_or_else(|| Error::UnknownSynth(synth.to_string()))?;
        tables_loaded(&registered.item)?;
        let (owner, specs, build) = (
            registered.owner.clone(),
            registered.specs.clone(),
            registered.item.build.clone(),
        );
        let values = self.check(&owner, &specs, given)?;
        let options = Options::new(owner, specs, values).holding(held);
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: None,
            options,
            make: Make::Note(build),
            ready: false,
        })
    }

    fn sample_draft(
        &mut self,
        id: Option<u64>,
        sample: u64,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let data = self
            .samples
            .get(&sample)
            .ok_or(Error::UnknownSample(sample))?
            .clone();
        let (owner, specs) = self.sampler.clone();
        let values = self.check(&owner, &specs, given)?;
        let stretched = given.iter().any(|(name, _)| {
            ["stretch", "pitch", "stretch_slide", "pitch_slide"].contains(&name.as_str())
        });
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: None,
            options: Options::new(owner, specs, values),
            make: Make::Sample(data, stretched),
            ready: false,
        })
    }

    fn grains_draft(
        &mut self,
        id: Option<u64>,
        sample: u64,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let data = self
            .samples
            .get(&sample)
            .ok_or(Error::UnknownSample(sample))?
            .clone();
        let (owner, specs) = self.grainer.clone();
        let values = self.check(&owner, &specs, given)?;
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: None,
            options: Options::new(owner, specs, values),
            make: Make::Grains(data),
            ready: false,
        })
    }

    fn input_draft(
        &mut self,
        id: Option<u64>,
        input: usize,
        channels: usize,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let channels = channels.clamp(1, 2);
        if input + channels > Ord::min(self.inputs, INPUTS) {
            return Err(Error::UnknownInput(input + channels - 1));
        }
        let (owner, specs) = self.input.clone();
        let values = self.check(&owner, &specs, given)?;
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: Some((input, channels)),
            options: Options::new(owner, specs, values),
            make: Make::Input(channels),
            ready: false,
        })
    }
}

/// An event handed over in a pass, which waits for its beat to come near.
struct Scored {
    beat: f64,
    part: u64,
    /// The label of its pass, when it is the first event of a pass with another label than the one before.
    starts: Option<u64>,
    draft: Draft,
}

/// An event checked and not yet built.
enum Draft {
    Sound(Box<SoundDraft>),
    Change(f64, Event),
}

/// A note, sample or input whose options are checked, with what builds its sound.
struct SoundDraft {
    id: Option<u64>,
    at: f64,
    /// The beat it was handed over at, when it came in a pass.
    beat: Option<f64>,
    /// How many cuts had been made when it was taken to be built.
    stamp: u64,
    out: Out,
    /// The first input channel and how many the sound reads, for live input.
    input: Option<(usize, usize)>,
    options: Options,
    make: Make,
    /// Whether it is built to be kept ready rather than played.
    ready: bool,
}

enum Make {
    Note(Arc<BuildNote>),
    /// A sample, and whether its event named a stretch or a pitch.
    Sample(Arc<SampleData>, bool),
    Grains(Arc<SampleData>),
    /// Live input of this many channels.
    Input(usize),
}

/// An event ready for the mixer but for its place in time.
enum Item {
    Sound(Box<Built>),
    Change(f64, Event),
}

/// A sound built and allocated, placed in the stereo field by its pan and scaled by its amp.
struct Built {
    id: Option<u64>,
    at: f64,
    beat: Option<f64>,
    stamp: u64,
    out: Out,
    input: Option<(usize, usize)>,
    unit: Net,
    /// Seconds the sound lasts.
    lasts: f64,
    controls: Controls,
    release: Option<(Shared, f64)>,
    /// Above 0 while the note's envelope holds until it is let go.
    held: Option<Shared>,
    /// The hold on a voice built to be kept ready.
    ready: Option<Arc<Ready>>,
}

/// Builds a draft's sound, which is the costly part of starting a note and needs nothing of the engine.
fn build(draft: SoundDraft, rate: f64) -> Result<Built, Error> {
    let options = &draft.options;
    let owner = options.owner.clone();
    let mut ctx = match draft.ready {
        true => Context::ready(owner.clone()),
        false => Context::new(owner.clone()),
    };
    let note = match draft.make {
        Make::Note(build) => build(options, &mut ctx),
        Make::Sample(data, stretched) => sample_note(data, stretched, options),
        Make::Grains(data) => grains_note(data, options),
        Make::Input(1) => Note::rings(Net::wrap(Box::new(pass())), f64::INFINITY),
        Make::Input(_) => Note::rings(Net::wrap(Box::new(multipass::<U2>())), f64::INFINITY),
    };
    let placing = options.follow("pan") | options.follow("amp");
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
    unit.set_sample_rate(rate);
    unit.allocate();
    Ok(Built {
        id: draft.id,
        at: draft.at,
        beat: draft.beat,
        stamp: draft.stamp,
        out: draft.out,
        input: draft.input,
        unit,
        lasts: note.end,
        controls: options.take_controls(),
        release: note.release,
        held: note.held,
        ready: ctx.take_ready(),
    })
}

/// The sounds started and the way to the mixer, which the engine's caller and a live engine's builder thread both reach.
struct Book {
    rate: f64,
    live: bool,
    commands: rtrb::Producer<Command>,
    garbage: rtrb::Consumer<Garbage>,
    sounds: HashMap<u64, Sound>,
    ids: HashMap<u64, u64>,
    chains: HashMap<u64, Vec<Controls>>,
    /// The level of each send, by the chain it is from and the chain it goes into.
    sends: HashMap<(u64, u64), Control>,
    /// The chain each routed chain plays into in place of the main output.
    routes: HashMap<u64, u64>,
    /// The chain that keys each keyed chain.
    keys: HashMap<u64, u64>,
    /// The rank of each chain that has one above 0: higher than that of every chain that feeds or keys it.
    ranks: HashMap<u64, u32>,
    /// The chains the mixer has ended.
    ended: std::collections::HashSet<u64>,
    /// The chain of each part started by a name and not stopped.
    parts: HashMap<String, u64>,
    map: TempoMap,
    /// The events handed over in beats and not yet placed at their frames, in the order of their beats.
    score: Vec<Scored>,
    /// For each part, the end of the last pass handed to it, the beats before it to say so at, and whether that was said.
    endings: HashMap<u64, (f64, f64, bool)>,
    /// The label of the last pass handed to each part.
    labels: HashMap<u64, u64>,
    /// For each chain and each of its effects, the options declared in beats that change while it runs.
    in_beats: HashMap<u64, Vec<Vec<String>>>,
    /// The effect options given in beats, which follow the tempo: the chain, the effect's index, the option and its beats.
    followers: Vec<(u64, usize, String, f64)>,
    /// The last beat at which the tempo was read for the followers and the beat reports.
    pointed: f64,
    /// The cuts made, the last ones kept: how many came before each, its part and its beat.
    cuts: (u64, Vec<(u64, u64, f64)>),
    /// The reports of passes, each held until the clock reaches its seconds.
    notices: Vec<(f64, Report)>,
    /// A live engine's clock against the system's, for the beat reports.
    timing: Option<Arc<Mutex<Timing>>>,
    voices: Arc<Voices>,
    next: Arc<AtomicU64>,
    clock: Arc<AtomicU64>,
    /// Where what the builder thread has to say goes, until [`Engine::warnings`] is asked.
    hub: Arc<Hub>,
    /// The voices kept ready of each synth asked for, each built and waiting for its note and vel.
    ready: HashMap<String, Vec<Built>>,
}

impl Book {
    fn check_chains(&self, chains: &[u64]) -> Result<(), Error> {
        match chains
            .iter()
            .find(|chain| !self.chains.contains_key(chain) || self.ended.contains(chain))
        {
            Some(chain) => Err(Error::UnknownChain(*chain)),
            None => Ok(()),
        }
    }

    /// The chains `chain` plays into or keys, each of which has to play after it.
    fn fed_by(&self, chain: u64) -> Vec<u64> {
        let sends = self.sends.keys().filter(|(from, _)| *from == chain);
        let mut fed: Vec<u64> = sends.map(|(_, to)| *to).collect();
        fed.extend(self.routes.get(&chain));
        let keyed = self.keys.iter().filter(|(_, by)| **by == chain);
        fed.extend(keyed.map(|(keyed, _)| *keyed));
        fed.sort_unstable();
        fed.dedup();
        fed
    }

    /// The chains on the way from `from` to `to` through what each feeds, both included, if there is one.
    fn way(&self, from: u64, to: u64, seen: &mut Vec<u64>) -> Option<Vec<u64>> {
        if from == to {
            return Some(vec![to]);
        }
        if seen.contains(&from) {
            return None;
        }
        seen.push(from);
        self.fed_by(from).into_iter().find_map(|next| {
            let mut way = self.way(next, to, seen)?;
            way.insert(0, from);
            Some(way)
        })
    }

    /// Makes room for `from` to feed or key `to`: refuses a loop, naming its chains, and has `to`, and what it feeds, play after `from`.
    fn feed(&mut self, from: u64, to: u64) -> Result<(), Error> {
        if let Some(way) = self.way(to, from, &mut Vec::new()) {
            let name = |chain: &u64| {
                let part = self.parts.iter().find(|(_, key)| *key == chain);
                part.map_or_else(|| chain.to_string(), |(name, _)| name.clone())
            };
            let mut chains = vec![name(&from)];
            chains.extend(way.iter().filter(|chain| **chain != from).map(name));
            return Err(Error::FeedsItself(chains));
        }
        let mut raise = vec![(to, self.ranks.get(&from).copied().unwrap_or(0) + 1)];
        while let Some((chain, least)) = raise.pop() {
            if self.ranks.get(&chain).copied().unwrap_or(0) < least {
                self.ranks.insert(chain, least);
                self.send(Command::Rank {
                    key: chain,
                    rank: least,
                })?;
                raise.extend(self.fed_by(chain).into_iter().map(|next| (next, least + 1)));
            }
        }
        Ok(())
    }

    /// Adds voices built ready to those kept for their synths.
    fn keep(&mut self, kept: Vec<(Arc<str>, Built)>) {
        for (synth, built) in kept {
            self.ready.entry(synth.to_string()).or_default().push(built);
        }
    }

    /// A voice kept ready for the note `sound` drafts, given its note and vel and placed as the draft is, when `event` gives no other option and one is kept.
    fn take_ready(&mut self, event: &Event, sound: &SoundDraft) -> Option<Built> {
        let Event::Note {
            synth,
            options,
            held,
            ..
        } = event
        else {
            return None;
        };
        let takes = |(name, value): &(String, Value)| match name.as_str() {
            "pedal" => matches!(value, Value::Number(x) if *x == 0.0 || *x == 1.0),
            name => READY_OPTIONS.contains(&name),
        };
        if !options.iter().all(takes) {
            return None;
        }
        let mut built = self.ready.get_mut(synth)?.pop()?;
        let hold = built.ready.as_ref()?;
        let given = Given {
            note: sound.options.number("note"),
            vel: sound.options.number("vel"),
            held: *held,
            pedal: if options.iter().any(|(name, _)| name == "pedal") {
                sound.options.number("pedal")
            } else {
                0.0
            },
            pressure: sound.options.number("pressure"),
        };
        let (lasts, release) = hold.give(given);
        built.lasts = lasts;
        if let Some((_, seconds)) = &mut built.release {
            *seconds = release;
        }
        if let Some(flag) = &built.held {
            flag.set(if *held { 1.0 } else { 0.0 });
        }
        // A note that follows these reads them from its first sample.
        for name in READY_OPTIONS {
            built.controls.set(name, sound.options.number(name) as f32);
        }
        built.id = sound.id;
        built.at = sound.at;
        built.beat = sound.beat;
        built.stamp = sound.stamp;
        built.out = sound.out;
        Some(built)
    }

    fn frames(&self, seconds: f64) -> u64 {
        (seconds.max(0.0) * self.rate).round() as u64
    }

    fn send(&mut self, mut command: Command) -> Result<(), Error> {
        for _ in 0..1000 {
            match self.commands.push(command) {
                Ok(()) => return Ok(()),
                Err(rtrb::PushError::Full(back)) => command = back,
            }
            self.drain();
            if !self.live {
                return Err(Error::Busy);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(Error::Busy)
    }

    fn drain(&mut self) {
        while let Ok(garbage) = self.garbage.pop() {
            if let Garbage::Chain(chain) = &garbage {
                let key = chain.key;
                self.ended.insert(key);
                self.sends
                    .retain(|(from, to), _| *from != key && *to != key);
                self.routes.retain(|from, to| *from != key && *to != key);
                self.keys.retain(|chain, by| *chain != key && *by != key);
                self.ranks.remove(&key);
                self.hub.forget_part(key);
                self.followers.retain(|follower| follower.0 != key);
                self.score.retain(|scored| scored.part != key);
                self.in_beats.remove(&key);
                self.endings.remove(&key);
                self.labels.remove(&key);
            }
        }
    }

    /// Forgets sounds that ended over a second ago, and frees what the mixer has finished with.
    fn sweep(&mut self) {
        self.drain();
        let now = self.clock.load(Ordering::Acquire) as f64 / self.rate;
        self.sounds.retain(|_, sound| sound.end + 1.0 > now);
        let sounds = &self.sounds;
        self.ids.retain(|_, key| sounds.contains_key(key));
    }

    /// The sound's voice, starting at its time or now if that has passed, and what the engine keeps of it.
    fn start(&mut self, built: Built) -> (Box<Voice>, Sound) {
        let key = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let now = self.clock.load(Ordering::Acquire);
        let start = Ord::max(self.frames(built.at), now);
        let late = now.saturating_sub(self.frames(built.at));
        let end = start.saturating_add(Ord::max(self.frames(built.lasts), 1));
        let fade = Ord::max(self.frames(END_FADE), 1);
        let voice = Voice {
            key,
            out: built.out,
            input: built.input,
            unit: Box::new(built.unit),
            start,
            late,
            end,
            fade,
            level: 0.0,
            stolen: false,
            slot: None,
        };
        let chain = match built.out {
            Out::Chain(key) => Some(key),
            _ => None,
        };
        let sound = Sound {
            key,
            chain,
            beat: built.beat,
            start: built.at,
            end: built.at + built.lasts,
            controls: built.controls,
            release: built.release,
        };
        (Box::new(voice), sound)
    }

    fn now(&self) -> f64 {
        self.clock.load(Ordering::Acquire) as f64 / self.rate
    }

    /// Whether anything waits for its time: an event in beats, a report of a pass, or a beat at which the tempo is another.
    fn pending(&self) -> bool {
        !self.score.is_empty()
            || !self.notices.is_empty()
            || self.endings.values().any(|(_, _, told)| !told)
            || self.map.next_point(self.pointed).is_some()
    }

    /// Takes what falls before `horizon` seconds out of what waits: the events in beats, placed at their seconds with their options in beats turned into seconds, and for each beat at which the tempo is another, a change to the effect options that follow it.
    /// It sends the reports of those beats, and holds those of the passes that start and of those about to end.
    fn due(&mut self, horizon: f64) -> Vec<Draft> {
        let now = self.now();
        let playing = self.map.beat_at(now);
        let until = self.map.beat_at(horizon);
        let count = self.score.partition_point(|scored| scored.beat < until);
        let taken: Vec<Scored> = self.score.drain(..count).collect();
        let mut due = Vec::with_capacity(taken.len());
        let mut dropped = 0;
        for scored in taken {
            // A release or a stop is kept however late, so that no note is left ringing.
            let ends = matches!(
                scored.draft,
                Draft::Change(_, Event::Stop { .. } | Event::Release { .. })
            );
            if playing - scored.beat >= LATE && !ends {
                dropped += 1;
                continue;
            }
            let (beat, part) = (scored.beat, scored.part);
            let at = self.map.seconds_at(beat);
            let per_beat = 60.0 / self.map.bpm_at(beat);
            if let (Some(label), Some(part)) = (scored.starts, self.hub.part_name(part)) {
                let seconds = at.max(now);
                let started = Report::PassStarted {
                    seconds,
                    part,
                    label,
                    beat,
                };
                self.notices.push((seconds, started));
            }
            due.push(match scored.draft {
                Draft::Sound(mut sound) => {
                    sound.at = at;
                    sound.beat = Some(beat);
                    sound.stamp = self.cuts.0;
                    sound.options.in_beats(per_beat);
                    Draft::Sound(sound)
                }
                Draft::Change(_, Event::Set { target, options }) => {
                    let options = self.in_seconds(&target, options, per_beat);
                    Draft::Change(at, Event::Set { target, options })
                }
                Draft::Change(_, event) => Draft::Change(at, event),
            });
        }
        self.voices.dropped.fetch_add(dropped, Ordering::Relaxed);
        for (part, (to, notice, told)) in &mut self.endings {
            let seconds = self.map.seconds_at(*to - *notice);
            if !*told && seconds < horizon {
                *told = true;
                if let Some(part) = self.hub.part_name(*part) {
                    let ending = Report::PassEnding {
                        seconds,
                        part,
                        beat: *to,
                    };
                    self.notices.push((seconds, ending));
                }
            }
        }
        // Of the beats already past, only the last still says what the tempo is.
        let mut points = Vec::new();
        while let Some(point) = self.map.next_point(self.pointed) {
            let at = self.map.seconds_at(point);
            if at >= horizon {
                break;
            }
            self.pointed = point;
            if at <= now {
                points.clear();
            }
            points.push((point, at));
        }
        for (beat, seconds) in points {
            let bpm = self.map.bpm_at(beat);
            let system = self.timing.as_ref().and_then(|timing| {
                let timing = timing.lock().unwrap_or_else(|e| e.into_inner());
                let heard = timing.system(seconds)? + timing.latency.unwrap_or(0.0);
                Some(SystemTime::UNIX_EPOCH + Duration::from_secs_f64(heard.max(0.0)))
            });
            self.hub.keep(Report::Beat {
                seconds,
                beat,
                bpm,
                system,
            });
            for (chain, index, name, beats) in &self.followers {
                let target = Target::Effect(*chain, *index);
                let options = vec![(name.clone(), beats * 60.0 / bpm)];
                let set = Event::Set { target, options };
                due.push(Draft::Change(seconds.max(now), set));
            }
        }
        due
    }

    /// A change's options in seconds, given in beats in a pass: each `_slide`, and an effect's options declared in beats, which follow the tempo from then on.
    fn in_seconds(
        &mut self,
        target: &Target,
        options: Vec<(String, f64)>,
        per_beat: f64,
    ) -> Vec<(String, f64)> {
        let mut seconds = Vec::with_capacity(options.len());
        for (name, value) in options {
            let effect = match target {
                Target::Effect(chain, index) => Some((*chain, *index)),
                _ => None,
            };
            let declared = effect.filter(|(chain, index)| {
                let names = self.in_beats.get(chain).and_then(|names| names.get(*index));
                names.is_some_and(|names| names.contains(&name))
            });
            if let Some((chain, index)) = declared {
                self.followers
                    .retain(|f| (f.0, f.1, f.2.as_str()) != (chain, index, name.as_str()));
                self.followers.push((chain, index, name.clone(), value));
            }
            let timed = declared.is_some() || name.ends_with("_slide");
            seconds.push((name, if timed { value * per_beat } else { value }));
        }
        seconds
    }

    /// Sends the reports of passes whose seconds the clock has reached.
    fn tell(&mut self) {
        let now = self.now();
        let mut reached = Vec::new();
        self.notices.retain(|(seconds, report)| {
            let keep = *seconds > now;
            if !keep {
                reached.push(report.clone());
            }
            keep
        });
        reached.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
        reached.into_iter().for_each(|report| self.hub.send(report));
    }

    /// Reads the tempo again from `beat` on, after the map changed there.
    fn repoint(&mut self, beat: f64) {
        self.pointed = self.pointed.min(beat.next_down());
    }

    /// Drops what a part was handed for `beat` and after: the events not yet placed, and the sounds placed that have not started.
    fn cut(&mut self, part: u64, beat: f64) -> Result<(), Error> {
        self.score
            .retain(|scored| scored.part != part || scored.beat < beat);
        self.endings.remove(&part);
        self.notices.retain(|(_, report)| match report {
            Report::PassStarted { beat: starts, .. } => *starts < beat,
            _ => true,
        });
        let (count, cuts) = &mut self.cuts;
        cuts.push((*count, part, beat));
        *count += 1;
        let kept = cuts.len().saturating_sub(32);
        cuts.drain(..kept);
        let now = self.now();
        let later = |sound: &Sound| {
            sound.chain == Some(part) && sound.start > now && sound.beat.is_some_and(|b| b >= beat)
        };
        let keys: Vec<u64> = self
            .sounds
            .values()
            .filter(|sound| later(sound))
            .map(|sound| sound.key)
            .collect();
        for key in keys {
            self.sounds.remove(&key);
            self.send(Command::Drop { key })?;
        }
        Ok(())
    }

    /// Whether a sound of a pass was cut while it was being built.
    fn was_cut(&self, built: &Built) -> bool {
        let (Some(beat), Out::Chain(part)) = (built.beat, built.out) else {
            return false;
        };
        let cut = |(before, of, from): &(u64, u64, f64)| {
            *before >= built.stamp && *of == part && beat >= *from
        };
        self.cuts.1.iter().any(cut)
    }

    /// Sends a batch to the mixer: its sounds first, so that a change can reach a note of the same batch.
    fn commit(&mut self, items: Vec<Item>) -> Result<(), Error> {
        let mut commands = Vec::with_capacity(items.len());
        let mut changes = Vec::new();
        for item in items {
            match item {
                Item::Sound(built) if self.was_cut(&built) => {}
                Item::Sound(built) => {
                    let id = built.id;
                    let (voice, sound) = self.start(*built);
                    if let Some(id) = id {
                        self.ids.insert(id, sound.key);
                    }
                    self.sounds.insert(sound.key, sound);
                    commands.push(Command::Start(voice));
                }
                Item::Change(at, event) => changes.push((at, event)),
            }
        }
        for (at, event) in changes {
            match event {
                Event::Set { target, options } => {
                    for change in self.changes(at, &target, &options) {
                        commands.push(Command::Change(Box::new(change)));
                    }
                }
                Event::Stop { ids } => {
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
                            // A sound with no release of its own rings until it fades, as a drum does once the hand leaves it.
                            None if sound.end.is_finite() => continue,
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

    fn send_changes(&self, at: f64, from: u64, to: u64, options: &[(String, f64)]) -> Vec<Change> {
        let value = |name: &str| options.iter().find(|(option, _)| option == name);
        let (level, slide) = (value("level"), value("level_slide"));
        let control = self
            .sends
            .get(&(from, to))
            .filter(|_| level.or(slide).is_some());
        let change = control.map(|control| Change {
            at: self.frames(at),
            control: control.clone(),
            value: level.map(|(_, level)| *level as f32),
            slide: slide.map(|(_, slide)| *slide as f32),
        });
        change.into_iter().collect()
    }

    fn changes(&self, at: f64, target: &Target, options: &[(String, f64)]) -> Vec<Change> {
        let controls: Vec<&Controls> = match target {
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
            Target::Send(from, to) => {
                return self.send_changes(at, *from, *to, options);
            }
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
                if let Some(control) = controls.get(name) {
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

/// Words what the audio thread counted since this was last called as warnings: late buffers, late events and stolen notes.
fn count_warnings(hub: &Hub, meter: &Meter, voices: &Voices, rate: f64, seconds: f64) {
    let (buffers, over) = meter.take_late();
    if buffers > 0 {
        let reached = if buffers == 1 {
            "buffer reached"
        } else {
            "buffers reached"
        };
        let ms = over * 1000.0;
        hub.warn(
            seconds,
            format!("{buffers} {reached} the sound card late, by up to {ms:.1} ms"),
        );
    }
    let late = voices.late.swap(0, Ordering::Relaxed);
    let most = voices.most_late.swap(0, Ordering::Relaxed);
    if late > 0 {
        let events = if late == 1 {
            "event arrived"
        } else {
            "events arrived"
        };
        let ms = most as f64 / rate * 1000.0;
        hub.warn(
            seconds,
            format!("{late} {events} after their time, by up to {ms:.1} ms, and played at once"),
        );
    }
    let dropped = voices.dropped.swap(0, Ordering::Relaxed);
    if dropped > 0 {
        let events = if dropped == 1 {
            "event was"
        } else {
            "events were"
        };
        hub.warn(
            seconds,
            format!(
                "{dropped} {events} dropped, handed over a quarter of a beat or more after the beat"
            ),
        );
    }
    let stolen = voices.stolen.swap(0, Ordering::Relaxed);
    if stolen > 0 {
        let limit = voices.limit.load(Ordering::Relaxed);
        let notes = if stolen == 1 {
            "note was"
        } else {
            "notes were"
        };
        hub.warn(
            seconds,
            format!("{stolen} {notes} stolen to keep to {limit} at once"),
        );
    }
}

/// Starts a live engine's thread that works its reports out and sends them, asleep while nobody subscribes.
/// Hands each analysis what was copied for it so far, and lets go of those the audio side has let go of.
fn hear(listening: &Mutex<Vec<Listening>>, hub: &Hub) {
    let mut listening = listening.lock().unwrap_or_else(|e| e.into_inner());
    listening.retain_mut(|analysis| analysis.hear(hub));
}

fn report(engine: &Engine, mut tapped: Tapped) -> JoinHandle<()> {
    let (hub, rate) = (engine.hub.clone(), engine.rate);
    let listening = engine.listening.clone();
    let (clock, voices, meter) = (
        engine.clock.clone(),
        engine.voices.clone(),
        engine.meter.clone(),
    );
    let timing = engine.timing.clone();
    std::thread::Builder::new()
        .name("mxs reports".to_string())
        .spawn(move || {
            let mut analyser = Analyser::new(rate);
            let mut seen = None;
            let mut mapped: Option<(f64, Instant)> = None;
            let mut second = None;
            while let Some(changes) = hub.wait() {
                let fresh = seen.replace(changes) != Some(changes);
                let now = clock.load(Ordering::Acquire) as f64 / rate;
                tapped.report(&mut analyser, &hub);
                hear(&listening, &hub);
                let read = timing
                    .as_ref()
                    .filter(|_| hub.wants(Topic::Clock))
                    .and_then(|timing| {
                        let timing = timing.lock().unwrap_or_else(|e| e.into_inner());
                        Some((timing.system(now)?, timing.latency.unwrap_or(0.0)))
                    });
                if let Some((filled, delay)) = read {
                    let offset = filled + delay - now;
                    let moved = mapped.is_none_or(|(sent, at)| {
                        (offset - sent).abs() > 0.002 || at.elapsed() >= Duration::from_secs(1)
                    });
                    if fresh || moved {
                        mapped = Some((offset, Instant::now()));
                        hub.send(Report::Clock {
                            seconds: now,
                            system: SystemTime::UNIX_EPOCH
                                + Duration::from_secs_f64(filled + delay),
                            delay,
                        });
                    }
                }
                let whole = now.floor() as u64;
                if fresh || second != Some(whole) {
                    second = Some(whole);
                    if hub.wants(Topic::Voices) {
                        let sounding = voices.sounding.load(Ordering::Relaxed);
                        hub.send(Report::Voices {
                            seconds: now,
                            sounding,
                        });
                    }
                    if hub.wants(Topic::Warnings) {
                        count_warnings(&hub, &meter, &voices, rate, now);
                    }
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        })
        .expect("the reports thread starts")
}

/// A live engine's thread that builds voices, so that neither the caller nor the audio thread waits for them.
struct Builder {
    drafts: Option<mpsc::Sender<Vec<Draft>>>,
    /// How many batches are handed over and not yet sent to the mixer.
    waiting: Arc<(Mutex<usize>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Builder {
    fn start(book: Arc<Mutex<Book>>, rate: f64) -> Builder {
        let (drafts, handed) = mpsc::channel::<Vec<Draft>>();
        let waiting = Arc::new((Mutex::new(0usize), Condvar::new()));
        let counting = waiting.clone();
        let thread = std::thread::Builder::new()
            .name("mxs builder".to_string())
            .spawn(move || {
                // Asleep until a batch is handed over, unless something in beats waits for its time.
                let mut pending = true;
                loop {
                    let handed_over = match pending {
                        true => handed.recv_timeout(TICK).map_err(|error| {
                            matches!(error, mpsc::RecvTimeoutError::Disconnected)
                        }),
                        false => handed.recv().map_err(|_| true),
                    };
                    let (mut batch, counted) = match handed_over {
                        Ok(batch) => (batch, true),
                        Err(false) => (Vec::new(), false),
                        Err(true) => break,
                    };
                    {
                        let mut book = book.lock().unwrap_or_else(|e| e.into_inner());
                        let horizon = book.now() + LEAD;
                        batch.extend(book.due(horizon));
                        book.tell();
                        pending = book.pending();
                    }
                    let mut said: Vec<String> = Vec::new();
                    let mut items = Vec::with_capacity(batch.len());
                    let mut kept = Vec::new();
                    for draft in batch {
                        match draft {
                            Draft::Sound(sound) => {
                                let owner = sound.options.owner.clone();
                                let ready = sound.ready;
                                let built = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(|| build(*sound, rate)),
                                );
                                match built {
                                    Ok(Ok(built)) if ready => kept.push((owner, built)),
                                    Ok(Ok(built)) => items.push(Item::Sound(Box::new(built))),
                                    Ok(Err(error)) => {
                                        said.push(format!("{error}, so its note did not play"))
                                    }
                                    Err(_) => said.push(format!(
                                        "{owner} panicked as its note was built, so the note did not play"
                                    )),
                                }
                            }
                            Draft::Change(at, event) => items.push(Item::Change(at, event)),
                        }
                    }
                    if counted || !items.is_empty() || !said.is_empty() {
                        let mut book = book.lock().unwrap_or_else(|e| e.into_inner());
                        book.keep(kept);
                        book.sweep();
                        if let Err(error) = book.commit(items) {
                            said.push(format!("{error}, so some events did not play"));
                        }
                        let now = book.now();
                        said.into_iter()
                            .for_each(|message| book.hub.warn(now, message));
                    }
                    if counted {
                        let (count, settled) = &*counting;
                        *count.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
                        settled.notify_all();
                    }
                }
            })
            .expect("the builder thread starts");
        Builder {
            drafts: Some(drafts),
            waiting,
            thread: Some(thread),
        }
    }

    fn hand(&self, batch: Vec<Draft>) {
        if let Some(hand) = self.handle() {
            hand.hand(batch);
        }
    }

    /// A way to hand the builder drafts from another thread.
    #[cfg_attr(not(feature = "keys"), allow(dead_code))]
    fn handle(&self) -> Option<Hand> {
        let drafts = self.drafts.clone()?;
        let waiting = self.waiting.clone();
        Some(Hand { drafts, waiting })
    }

    fn settle(&self) {
        let (count, settled) = &*self.waiting;
        let mut count = count.lock().unwrap_or_else(|e| e.into_inner());
        while *count > 0 {
            count = settled.wait(count).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// Hands drafts to the builder thread, which builds and plays them in the order they were handed over.
struct Hand {
    drafts: mpsc::Sender<Vec<Draft>>,
    waiting: Arc<(Mutex<usize>, Condvar)>,
}

impl Hand {
    fn hand(&self, batch: Vec<Draft>) {
        let (count, _) = &*self.waiting;
        *count.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        let _ = self.drafts.send(batch);
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        self.drafts.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // The players hand drafts to the builder, which stops only once nothing can.
        #[cfg(feature = "keys")]
        self.players.clear();
        let _ = self.stop_recording();
        let _ = self.stop_recording_output();
        if let Output::Live { stop, .. } = &self.output {
            let _ = stop.send(());
        }
        self.hub.stop();
        if let Some(reporter) = self.reporter.take() {
            let _ = reporter.join();
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
    meter: Arc<Meter>,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    hub: Arc<Hub>,
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
    let mut mixer = Mixer::new(commands, garbage, clock, voices, input);
    let following = hub.clone();
    let (tap, tapped) = report::tap(hub, chosen.rate as f64);
    mixer.report_to(tap);
    let timing = Arc::new(Mutex::new(Timing::new(chosen.rate as f64)));
    let live = Live {
        mixer,
        meter,
        rate: chosen.rate as f64,
        failed,
        lost,
        frames,
        timing: timing.clone(),
        hub: following,
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
            tapped,
        },
    ))
}

/// What a live engine learns once its device is playing.
struct Opened {
    rate: f64,
    inputs: usize,
    name: String,
    timing: Arc<Mutex<Timing>>,
    tapped: Tapped,
}

/// What the output callback owns.
struct Live {
    mixer: Mixer,
    meter: Arc<Meter>,
    rate: f64,
    failed: Arc<AtomicBool>,
    lost: Arc<AtomicBool>,
    frames: Arc<AtomicU64>,
    timing: Arc<Mutex<Timing>>,
    hub: Arc<Hub>,
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
        meter,
        rate,
        failed,
        lost,
        frames,
        timing,
        hub,
    } = live;
    let channels = config.channels as usize;
    let (moving, moved_frames) = (timing.clone(), frames.clone());
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
                let began = Instant::now();
                fill(&mut mixer, &failed, data, channels, &mut left, &mut right);
                meter.record(began.elapsed(), data.len() / channels, rate, true);
            },
            move |e| match e.kind() {
                cpal::ErrorKind::DeviceNotAvailable => lost.store(true, Ordering::Release),
                cpal::ErrorKind::DeviceChanged => {
                    follow(hub.clone(), moving.clone(), moved_frames.clone(), rate)
                }
                _ => eprintln!("mxs output: {e}"),
            },
            None,
        )
        .map_err(|e| e.to_string())
}

/// Once the engine has played a tenth of a second on the system's new default output, lines its clock up with the system's again and reports the new output.
// The move holds the callbacks up for about 100 ms, and the clock otherwise rises to meet that by a millisecond a second.
fn follow(hub: Arc<Hub>, timing: Arc<Mutex<Timing>>, frames: Arc<AtomicU64>, rate: f64) {
    std::thread::spawn(move || {
        let from = frames.load(Ordering::Acquire) + (rate / 10.0) as u64;
        let waiting = Instant::now();
        while frames.load(Ordering::Acquire) < from {
            if waiting.elapsed() > Duration::from_secs(2) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut held = timing.lock().unwrap_or_else(|e| e.into_inner());
        held.restart();
        let delay_ms = held.latency.map(|seconds| seconds * 1000.0);
        drop(held);
        let seconds = frames.load(Ordering::Acquire) as f64 / rate;
        let device = device::output(&cpal::default_host(), None).ok();
        let name = device.as_ref().map(device::name_of);
        hub.keep(Report::Output {
            seconds,
            device: name.clone(),
            rate,
            delay_ms,
        });
        let theirs = device
            .and_then(|device| device.default_output_config().ok())
            .map(|config| config.sample_rate() as f64)
            .filter(|theirs| *theirs != rate);
        if let (Some(name), Some(theirs)) = (name, theirs) {
            hub.warn(
                seconds,
                format!("{name} plays at {theirs} Hz and the engine at {rate} Hz, so the system converts between them"),
            );
        }
    });
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

/// The options a note may be given and still take a voice kept ready, besides a pedal of 0 or 1.
const READY_OPTIONS: [&str; 5] = ["note", "vel", "bend", "brightness", "pressure"];

fn note_specs() -> Vec<Spec> {
    let mut specs = placing_specs();
    specs.extend(Spec::expressions(&["bend", "brightness", "pressure"]));
    specs.push(
        Spec::float("vibrato")
            .default(0.0)
            .range(0.0, 2.0)
            .doc("The vibrato's depth in semitones, on a synth that lists it among its own; the others ignore it."),
    );
    specs.push(
        Spec::float("pedal")
            .default(0.0)
            .range(0.0, 1.0)
            .doc("How far the sustain pedal is down, on a synth that lists it among its own; the others ignore it."),
    );
    specs
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
        Spec::float("stretch").default(1.0).range(0.01, 100.0).doc("What its length is multiplied by, its pitch kept."),
        Spec::float("pitch").default(0.0).range(-48.0, 48.0).doc("How many semitones it is shifted by, its length kept."),
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

fn grains_specs() -> Vec<Spec> {
    vec![
        Spec::float("position").default(0.5).range(0.0, 1.0).doc("Where in the sample the grains are taken from, as a fraction of its length."),
        Spec::float("duration").default(0.1).range(0.001, 10.0).doc("How long each grain lasts, in seconds."),
        Spec::float("density").default(20.0).range(0.01, 1000.0).doc("How many grains start a second."),
        Spec::float("pitch").default(0.0).range(-48.0, 48.0).doc("The grains' pitch, in semitones from the sample's own."),
        Spec::float("spread").default(0.0).range(0.0, 1.0).doc("How far each grain's position strays at random either way, as a fraction of the sample's length."),
        Spec::float("width").default(0.0).range(0.0, 1.0).doc("How far each grain strays at random across the stereo field, from 0, in the middle, to 1, anywhere from left to right."),
        Spec::float("seed").default(0.0).doc("The seed of the random positions and places; grains with the same seed play the same."),
        Spec::beats("attack").default(0.0).range(0.0, f64::INFINITY).doc("How long the grains take to rise to full level."),
        Spec::beats("sustain").default(1.0).range(0.0, f64::INFINITY).doc("How long they then hold."),
        Spec::beats("release").default(0.0).range(0.0, f64::INFINITY).doc("How long they then take to fall silent."),
    ]
}

fn grains_note(data: Arc<SampleData>, options: &Options) -> Note {
    let moving = [
        "position", "duration", "density", "pitch", "spread", "width",
    ];
    let inputs = moving
        .iter()
        .map(|name| options.follow(name))
        .reduce(|all, next| all | next)
        .expect("grains have options");
    let seed = options.number("seed").to_bits();
    let grains = inputs >> Net::wrap(Box::new(An(Grains::new(data, seed))));
    let (attack, sustain, release) = (
        options.number("attack"),
        options.number("sustain"),
        options.number("release"),
    );
    let level = crate::nodes::shape(move |t| {
        if t < attack {
            t / attack
        } else if t < attack + sustain {
            1.0
        } else {
            (1.0 - (t - attack - sustain) / release).clamp(0.0, 1.0)
        }
    });
    let level = level >> Net::wrap(Box::new(split::<U2>()));
    Note::rings(grains * level, attack + sustain + release)
}

/// A sample's note; one whose event named a stretch or a pitch reads through a [`Stretcher`], and one that named neither through a [`Player`].
fn sample_note(data: Arc<SampleData>, stretched: bool, options: &Options) -> Note {
    let rate = options.number("rate");
    let (from, to) = (options.number("start"), options.number("finish"));
    let span = (to - from).abs() * data.frames() as f64 / data.rate;
    let stretch = if stretched {
        options.number("stretch")
    } else {
        1.0
    };
    let length = span / rate.abs().max(0.001) * stretch;
    let mono = data.channels.len() == 1;
    // A mono sample is placed at equal power, so it is raised by as much again as its pan lowers it at the centre.
    let centre = if mono { std::f32::consts::SQRT_2 } else { 1.0 };
    let gain = if options.number("normalize") > 0.5 && data.peak > 0.0 {
        centre / data.peak
    } else {
        1.0
    };
    let player = if stretched {
        (options.follow("rate") | options.follow("stretch") | options.follow("pitch"))
            >> Net::wrap(Box::new(An(Stretcher::new(data, from, to, rate))))
    } else {
        options.follow("rate") >> Net::wrap(Box::new(An(Player::new(data, from, to, rate))))
    } * gain;
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

/// A sound held back by a number of frames.
#[derive(Clone)]
struct Held {
    frames: Vec<f32>,
    at: usize,
}

impl Held {
    fn new(frames: usize) -> Self {
        Held {
            frames: vec![0.0; frames],
            at: 0,
        }
    }
}

impl AudioNode for Held {
    const ID: u64 = 0x6d78_0080;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        self.frames.fill(0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let out = self.frames[self.at];
        self.frames[self.at] = input[0];
        self.at = (self.at + 1) % self.frames.len();
        [out].into()
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
    use crate::SynthDecl;

    /// What the voices of `synth` kept ready are given as they are taken.
    fn kept(engine: &Engine, synth: &str) -> Vec<Arc<Ready>> {
        engine.book().ready[synth]
            .iter()
            .filter_map(|built| built.ready.clone())
            .collect()
    }

    #[test]
    fn a_held_piano_note_with_the_pedal_down_takes_a_voice_kept_ready_and_one_half_down_does_not() {
        let mut engine = Engine::offline(48_000.0);
        crate::synths::all()
            .into_iter()
            .for_each(|s| engine.register(s));
        engine.keep_ready("piano", 2).unwrap();
        let ready = kept(&engine, "piano");
        let options = |pedal| {
            [
                ("note", 60.0),
                ("vel", 0.6),
                ("pedal", pedal),
                ("bend", 0.0),
            ]
        };
        engine
            .play(&[(0.0, Event::held(1, "piano", &options(0.5)))])
            .unwrap();
        assert!(ready.iter().all(|hold| hold.given().is_none()));
        engine
            .play(&[(0.0, Event::held(2, "piano", &options(1.0)))])
            .unwrap();
        let given = Given {
            note: 60.0,
            vel: 0.6,
            held: true,
            pedal: 1.0,
            pressure: 0.0,
        };
        assert!(ready.iter().any(|hold| hold.given() == Some(given)));
    }

    #[test]
    fn a_panic_while_filling_audio_silences_the_engine_and_is_reported() {
        let mut engine = Engine::offline(48_000.0);
        engine.register(SynthDecl::new("broken").build(|_, _| {
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
    fn live_input_let_go_ends_in_10_ms_since_it_never_ends_by_itself() {
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
        let input = Event::Input {
            id: Some(1),
            input: 0,
            channels: 1,
            options: vec![],
            chain: None,
        };
        engine.play(&[(0.0, input)]).unwrap();
        engine
            .play(&[(0.1, Event::Release { ids: vec![1] })])
            .unwrap();
        let (left, _) = render_fed(&mut engine, &mut feed, 0.2, &[0.5]);
        assert!(left[4000] > 0.0);
        assert!(left[4800 + 480..].iter().all(|s| *s == 0.0));
    }

    #[test]
    fn live_input_plays_through_the_main_effects() {
        let played = |main: Option<f64>| {
            let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
            crate::effects::all()
                .into_iter()
                .for_each(|e| engine.register_effect(e));
            if let Some(amp) = main {
                let level = (
                    "level".to_string(),
                    vec![("amp".to_string(), Value::Number(amp))],
                );
                engine.start_main_effects(&[level], false).unwrap();
            }
            let input = Event::Input {
                id: Some(1),
                input: 0,
                channels: 1,
                options: vec![],
                chain: None,
            };
            engine.play(&[(0.0, input)]).unwrap();
            render_fed(&mut engine, &mut feed, 0.2, &[0.5]).0
        };
        let (straight, halved) = (played(None), played(Some(0.5)));
        assert!(straight[4000] > 0.0);
        assert!((straight[4000] * 0.5 - halved[4000]).abs() < 1e-6);
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

    #[test]
    fn an_analysis_of_an_input_hears_it_alone_and_reports_its_pitch() {
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
        let reports = engine.subscribe(&[Topic::Analysis]);
        engine.analyse(Origin::Input(0), 0.01, 0.0).unwrap();
        let mut pitches = Vec::new();
        for block in 0..48 {
            for i in 0..1000 {
                let t = (block * 1000 + i) as f64 / 48_000.0;
                feed.push(0.5 * (2.0 * std::f64::consts::PI * 220.0 * t).sin() as f32)
                    .unwrap();
            }
            let (left, right) = engine.render(1000.0 / 48_000.0).unwrap();
            assert!(left.iter().chain(&right).all(|x| *x == 0.0));
            while let Some(report) = reports.wait(Duration::ZERO) {
                if let Report::Analysis {
                    pitch: Some(hz), ..
                } = report
                {
                    pitches.push(hz);
                }
            }
        }
        assert!(pitches.len() > 40);
        assert!(
            pitches.iter().all(|hz| (hz - 220.0).abs() < 0.5),
            "{pitches:?}"
        );
    }

    /// Renders `seconds` of an engine whose one input is fed `signal` from frame 0.
    fn render_signal(
        engine: &mut Engine,
        feed: &mut rtrb::Producer<f32>,
        seconds: f64,
        signal: impl Fn(usize) -> f32,
    ) -> (Vec<f32>, Vec<f32>) {
        let (mut left, mut right) = (Vec::new(), Vec::new());
        let frames = engine.frames(seconds) as usize;
        while left.len() < frames {
            let n = Ord::min(1000, frames - left.len());
            (left.len()..left.len() + n).for_each(|i| feed.push(signal(i)).unwrap());
            let (l, r) = engine.render(n as f64 / engine.rate).unwrap();
            left.extend(l);
            right.extend(r);
        }
        (left, right)
    }

    #[test]
    fn an_input_recorded_into_a_buffer_from_1_second_plays_back_from_3_seconds_exactly() {
        let signal = |i: usize| 0.5 * ((i as f32 * 0.0137).sin() + (i as f32 * 0.091).sin()) / 2.0;
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
        let buffer = engine.create_buffer(2.0, 2).unwrap();
        engine
            .record_buffer(buffer, Origin::Input(0), 1.0, 2.0, 0.0)
            .unwrap();
        let sample = Event::Sample {
            id: None,
            sample: buffer,
            options: vec![],
            chain: None,
        };
        engine.play(&[(3.0, sample)]).unwrap();
        let (left, right) = render_signal(&mut engine, &mut feed, 5.0, signal);
        assert!(left[..144_000].iter().all(|x| *x == 0.0));

        let (mut straight, mut feed) = Engine::offline_with_input(48_000.0, 1);
        let input = Event::Input {
            id: None,
            input: 0,
            channels: 1,
            options: vec![],
            chain: None,
        };
        straight.play(&[(1.0, input)]).unwrap();
        let (expected, _) = render_signal(&mut straight, &mut feed, 3.0, signal);
        // The buffer fades its last 10 ms as it plays, as every sound does at its end.
        let played = 144_000..240_000 - 480;
        assert_eq!(left[played.clone()], expected[48_000..144_000 - 480]);
        assert_eq!(left[played.clone()], right[played]);
    }

    #[test]
    fn a_live_input_through_the_pitch_shift_at_5_semitones_rises_to_587_hz_within_50_ms() {
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
        crate::effects::all()
            .into_iter()
            .for_each(|e| engine.register_effect(e));
        let shift = (
            "pitch_shift".to_string(),
            vec![("pitch".to_string(), Value::Number(5.0))],
        );
        let chain = engine.start_effects(&[shift]).unwrap();
        let input = Event::Input {
            id: None,
            input: 0,
            channels: 1,
            options: vec![],
            chain: Some(chain),
        };
        engine.play(&[(0.0, input)]).unwrap();
        let tone = |i: usize| {
            let t = i as f64 / 48_000.0;
            if t < 0.5 {
                0.0
            } else {
                0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32
            }
        };
        let (left, _) = render_signal(&mut engine, &mut feed, 2.5, tone);
        let heard = left.iter().position(|x| x.abs() > 0.01).unwrap();
        let delay = heard as f64 / 48_000.0 - 0.5;
        assert!(delay <= 0.05, "delayed {delay} seconds");
        let steady = &left[72_000..120_000];
        let level = |hz: f64| {
            let w = 2.0 * std::f64::consts::PI * hz / 48_000.0;
            let n = steady.len() as f64;
            let (mut s1, mut s2) = (0.0, 0.0);
            for (i, &x) in steady.iter().enumerate() {
                let window = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n).cos();
                let s = x as f64 * window + 2.0 * w.cos() * s1 - s2;
                s2 = s1;
                s1 = s;
            }
            s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2
        };
        let found = (0..2000)
            .map(|k| 577.0 + k as f64 * 0.01)
            .max_by(|a, b| level(*a).total_cmp(&level(*b)))
            .unwrap();
        let expected = 440.0 * 2f64.powf(5.0 / 12.0);
        let cents = 1200.0 * (found / expected).log2();
        assert!(cents.abs() < 2.0, "{found} Hz");
    }
}
