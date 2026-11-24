//! The engine: synths and effects registered by name, samples loaded from files, and events stamped with seconds, played live or rendered offline.

use crate::engine::clock::Timing;
use crate::engine::listen::Listening;
use crate::engine::live::Settings;
use crate::engine::load::{Load, Meter};
use crate::engine::mixer::{
    CAPACITY, Chain, Change, Command, Garbage, Heard, INPUTS, Kept, Listened, Listener, Mixer, Out,
    PartChange, Recorder, Voice, Voices,
};
use crate::engine::report::{Analyser, Hub, Report, Reports, Tapped, Topic};
use crate::engine::sample::{Grains, Player, SampleData, Samples, Stretcher};
use crate::engine::time::{Tempo, TempoMap};
use crate::engine::workers::{Pool, performance_cores};
use crate::error::Error;
use crate::options::{Context, Control, Controls, Options};
use crate::ready::{Given, Ready};
use crate::spec::{Spec, Value, checked, held_warning};
use crate::synth::{BuildNote, Effect, Note, Synth};
use build::{
    Blend, Builder, Held, blending_specs, build, grains_specs, note_specs, placing_specs,
    sample_specs,
};
pub use events::{Event, Origin, Pass, Source, Target};
use fundsp::prelude::*;
use live::{Opened, open};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

mod book;
mod build;
pub(crate) mod clock;
mod events;
pub(crate) mod listen;
pub(crate) mod live;
pub(crate) mod load;
#[cfg(feature = "keys")]
mod midi;
pub(crate) mod mixer;
pub(crate) mod report;
pub(crate) mod sample;
pub(crate) mod time;
pub(crate) mod workers;

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
        let hub = Hub::new();
        let (tap, tapped) = report::tap(hub.clone(), rate);
        let mixer = Mixer::new(
            receiving,
            returning,
            clock.clone(),
            voices.clone(),
            None,
            tap,
        );
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
        let hub = Hub::new();
        let (tap, tapped) = report::tap(hub.clone(), rate);
        let mixer = Mixer::new(
            receiving,
            returning,
            clock.clone(),
            voices.clone(),
            Some(crate::engine::mixer::Input { frames, channels }),
            tap,
        );
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
        self.timing().and_then(|t| t.latency())
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
            book.map.bpm_read(book.now())
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
            sends: Vec::with_capacity(crate::engine::mixer::SENDS),
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
            >= crate::engine::mixer::SENDS
        {
            return Err(Error::TooManySends {
                chain: from,
                most: crate::engine::mixer::SENDS,
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
        let send = crate::engine::mixer::Send {
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

    /// Places beat 0 of the tempo map at `seconds` of the engine's clock; it is at 0 until placed.
    /// The events in beats that are more than 150 ms from sounding move with it.
    pub fn place_beats(&mut self, seconds: f64) -> Result<(), Error> {
        self.check_running()?;
        self.place(self.now() + LEAD, |book| {
            book.map.place(seconds);
        })
    }

    /// Changes the tempo from `beat` on, in place of whatever the map held from that beat on; the tempo is 120 beats a minute until changed.
    /// Every event in beats that is more than 150 ms from sounding moves to where its beat now falls, and the effect options in beats follow.
    pub fn set_tempo(&mut self, beat: f64, tempo: Tempo) -> Result<(), Error> {
        self.check_running()?;
        self.place(self.now() + LEAD, |book| {
            book.map.set(beat, tempo);
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
                        Some((timing.heard(now)?, timing.latency().unwrap_or(0.0)))
                    });
                if let Some((heard, delay)) = read {
                    let offset = heard - now;
                    let moved = mapped.is_none_or(|(sent, at)| {
                        (offset - sent).abs() > 0.002 || at.elapsed() >= Duration::from_secs(1)
                    });
                    if fresh || moved {
                        mapped = Some((offset, Instant::now()));
                        hub.send(Report::Clock {
                            seconds: now,
                            system: SystemTime::UNIX_EPOCH + Duration::from_secs_f64(heard),
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

#[cfg(test)]
mod tests;
