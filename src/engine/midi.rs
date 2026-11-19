//! MIDI inputs played on parts inside the engine: each message becomes notes in the block being filled, with no caller in between.

use super::{
    Book, Draft, Engine, Error, Event, Hand, Item, Make, Out, SoundDraft, checked, held_warning,
};
use crate::keys::Keyboard;
use crate::options::Options;
use crate::report::{Hub, Report};
use crate::spec::{Spec, Value};
use crate::synth::Synth;
use midir::{MidiInput, MidiInputConnection};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

/// The first id of the notes the MIDI inputs play, above any a caller is expected to give.
pub(crate) const FIRST_ID: u64 = 1 << 62;
/// How many notes each input's instrument keeps ready, enough for a chord of both hands.
const READY: usize = 10;
/// How often the inputs played are looked for, to notice one that went away.
const LOOK: Duration = Duration::from_millis(100);
/// How many players were started, which numbers each one's notes apart from the others'.
static PLAYERS: AtomicU64 = AtomicU64::new(0);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// The instrument an input plays, which the caller can change while keys are played.
struct Instrument {
    synth: Synth,
    owner: Arc<str>,
    specs: Arc<[Spec]>,
    given: Vec<(String, Value)>,
}

/// One input played: its name, the state of its keys and pedal, and whether it went away.
struct Port {
    name: String,
    keyboard: Keyboard,
    gone: bool,
}

/// What every input of a player shares: the way to the mixer and the builder, and the instrument.
struct Shared {
    book: Arc<Mutex<Book>>,
    hand: Hand,
    hub: Arc<Hub>,
    clock: Arc<AtomicU64>,
    rate: f64,
    out: Out,
    instrument: Mutex<Instrument>,
    /// The options already said to be held to their ranges.
    warned: Mutex<HashSet<String>>,
}

impl Shared {
    fn now(&self) -> f64 {
        self.clock.load(Ordering::Acquire) as f64 / self.rate
    }

    /// Plays one message of `port`, then reports its key, if it is one.
    fn message(&self, port: &Mutex<Port>, bytes: &[u8]) {
        let at = self.now();
        let mut port = lock(port);
        let events = port.keyboard.message(bytes, at);
        self.play(events);
        if let Some((key, velocity, down)) = key_of(bytes) {
            let input = port.name.clone();
            self.hub.send(Report::Key {
                seconds: at,
                input,
                key,
                velocity,
                down,
            });
        }
    }

    /// Starts each note from a voice kept ready when one is, and hands the rest to the builder in their order.
    fn play(&self, events: Vec<(f64, Event)>) {
        if events.is_empty() {
            return;
        }
        let instrument = lock(&self.instrument);
        let mut ready = Vec::new();
        let mut handed = Vec::new();
        for (at, event) in events {
            let Event::Note {
                id, options, held, ..
            } = event
            else {
                handed.push(Draft::Change(at, event));
                continue;
            };
            let mut given = instrument.given.clone();
            given.extend(options);
            let Some(draft) = self.draft(&instrument, id, &given, held, at) else {
                continue;
            };
            let note = Event::Note {
                id,
                synth: instrument.owner.to_string(),
                options: given,
                chain: None,
                held,
            };
            match lock(&self.book).take_ready(&note, &draft) {
                Some(built) => ready.push(Item::Sound(Box::new(built))),
                None => handed.push(Draft::Sound(Box::new(draft))),
            }
        }
        if !ready.is_empty() {
            let taken = ready.len();
            let mut book = lock(&self.book);
            book.sweep();
            if let Err(error) = book.commit(ready) {
                let now = book.now();
                book.hub
                    .warn(now, format!("{error}, so a key did not play"));
            }
            drop(book);
            for _ in 0..taken {
                if let Some(mut draft) = self.draft(&instrument, None, &[], false, 0.0) {
                    draft.ready = true;
                    draft.out = Out::Main;
                    handed.push(Draft::Sound(Box::new(draft)));
                }
            }
        }
        if !handed.is_empty() {
            self.hand.hand(handed);
        }
    }

    /// A note of the instrument given `given`, into the part, warning once of each option held to its range.
    fn draft(
        &self,
        instrument: &Instrument,
        id: Option<u64>,
        given: &[(String, Value)],
        held: bool,
        at: f64,
    ) -> Option<SoundDraft> {
        let (owner, specs) = (&instrument.owner, &instrument.specs);
        let (values, held_names) = match checked(owner, specs, given) {
            Ok(checked) => checked,
            Err(error) => {
                let message = format!("{error}, so a key did not play");
                self.hub.warn(self.now(), message);
                return None;
            }
        };
        for name in held_names {
            if lock(&self.warned).insert(format!("{owner} {name}")) {
                self.hub.warn(self.now(), held_warning(owner, specs, &name));
            }
        }
        let mut options = Options::new(owner.clone(), specs.clone(), values);
        options.held = held;
        Some(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out: self.out,
            input: None,
            options,
            make: Make::Note(instrument.synth.build.clone()),
            ready: false,
        })
    }

    /// Lets go every note `port` holds.
    fn let_go(&self, port: &Mutex<Port>) {
        let at = self.now();
        let events = lock(port).keyboard.let_go_all(at);
        self.play(events);
    }
}

/// The key, its velocity and whether it went down, of a message that is a key's.
fn key_of(bytes: &[u8]) -> Option<(u8, u8, bool)> {
    match *bytes {
        [status, key, velocity] if status & 0xf0 == 0x90 => Some((key, velocity, velocity > 0)),
        [status, key, velocity] if status & 0xf0 == 0x80 => Some((key, velocity, false)),
        _ => None,
    }
}

/// MIDI inputs played on one part: their connections, and the thread that notices one going away.
pub(crate) struct Player {
    shared: Arc<Shared>,
    ports: Vec<Arc<Mutex<Port>>>,
    connections: Vec<MidiInputConnection<()>>,
    stop: Arc<AtomicBool>,
    watcher: Option<JoinHandle<()>>,
}

impl Player {
    /// Stops listening, then lets go every note still held.
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
        self.connections.clear();
        for port in &self.ports {
            self.shared.let_go(port);
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The names of the MIDI inputs connected now.
fn names(midi: &MidiInput) -> Vec<String> {
    midi.ports()
        .iter()
        .map(|port| midi.port_name(port).unwrap_or_default())
        .collect()
}

fn client(name: &str) -> Result<MidiInput, Error> {
    MidiInput::new(name).map_err(|error| Error::Midi(format!("MIDI is not available: {error}")))
}

impl Engine {
    /// The names of the MIDI inputs connected now, which [`Engine::play_midi`] takes.
    pub fn midi_inputs() -> Result<Vec<String>, Error> {
        Ok(names(&client("mxs")?))
    }

    /// Plays the MIDI input named, or every input connected when `input` is none, on the part named, with notes of `synth` given `options` besides the note, the loudness and the pedal each key gives.
    /// A key starts a note held until it comes up, in the block being filled when the message arrives, from one of the voices kept ready of `synth`; the sustain pedal and the pitch bend wheel play as [`Keyboard`] has them.
    /// Each key that goes down or comes up is reported under [`Topic::Keys`](crate::Topic::Keys), and so is an input that goes away, whose notes are let go.
    /// Playing an input already played starts again with the new part and instrument, letting go of the notes it held.
    /// It needs a live engine, and the `keys` feature.
    pub fn play_midi(
        &mut self,
        input: Option<&str>,
        part: &str,
        synth: &str,
        options: &[(&str, f64)],
    ) -> Result<(), Error> {
        self.check_running()?;
        let hand = self
            .builder
            .as_ref()
            .and_then(|builder| builder.handle())
            .ok_or(Error::Offline)?;
        let chain = self.part(part)?;
        let instrument = self.instrument(synth, options)?;
        let midi = client("mxs")?;
        let connected = names(&midi);
        let listed = if connected.is_empty() {
            "there are none".to_string()
        } else {
            let known = connected.join(", ");
            format!("the inputs are {known}")
        };
        let chosen: Vec<String> = match input {
            Some(name) if connected.iter().any(|c| c == name) => vec![name.to_string()],
            Some(name) => {
                return Err(Error::Midi(format!("no MIDI input named {name}; {listed}")));
            }
            None if connected.is_empty() => {
                return Err(Error::Midi(format!("no MIDI input to play; {listed}")));
            }
            None => connected,
        };
        let key = input.map(str::to_string);
        self.players.remove(&key);
        self.keep_instrument_ready(synth)?;
        let shared = Arc::new(Shared {
            book: self.book.clone(),
            hand,
            hub: self.hub.clone(),
            clock: self.clock.clone(),
            rate: self.rate,
            out: Out::Chain(chain),
            instrument: Mutex::new(instrument),
            warned: Mutex::new(HashSet::new()),
        });
        let first = FIRST_ID + (PLAYERS.fetch_add(1, Ordering::Relaxed) << 32);
        let mut ports = Vec::with_capacity(chosen.len());
        let mut connections = Vec::with_capacity(chosen.len());
        for (index, name) in chosen.into_iter().enumerate() {
            let midi = client("mxs")?;
            let found = midi
                .ports()
                .into_iter()
                .find(|p| midi.port_name(p).ok().as_deref() == Some(name.as_str()))
                .ok_or_else(|| Error::Midi(format!("the MIDI input {name} went away")))?;
            let mut keyboard = Keyboard::new(&lock(&shared.instrument).synth);
            keyboard.numbered_from(first + ((index as u64) << 24));
            let port = Arc::new(Mutex::new(Port {
                name: name.clone(),
                keyboard,
                gone: false,
            }));
            let (playing, heard) = (shared.clone(), port.clone());
            let connection = midi
                .connect(
                    &found,
                    "mxs",
                    move |_, bytes, _| playing.message(&heard, bytes),
                    (),
                )
                .map_err(|error| {
                    Error::Midi(format!(
                        "the MIDI input {name} could not be opened: {error}"
                    ))
                })?;
            ports.push(port);
            connections.push(connection);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let watcher = watch(
            client("mxs watch")?,
            shared.clone(),
            ports.clone(),
            stop.clone(),
        );
        self.players.insert(
            key,
            Player {
                shared,
                ports,
                connections,
                stop,
                watcher: Some(watcher),
            },
        );
        Ok(())
    }

    /// Plays notes of `synth` given `options` from the next key on the MIDI input named, or on every input when `input` is none, as [`Engine::play_midi`] was asked; the notes that sound ring on until their keys and the pedal let them go.
    pub fn change_midi(
        &mut self,
        input: Option<&str>,
        synth: &str,
        options: &[(&str, f64)],
    ) -> Result<(), Error> {
        self.check_running()?;
        let instrument = self.instrument(synth, options)?;
        let key = input.map(str::to_string);
        if !self.players.contains_key(&key) {
            return Err(not_played(input));
        }
        self.keep_instrument_ready(synth)?;
        let player = &self.players[&key];
        for port in &player.ports {
            lock(port).keyboard.play_as(&instrument.synth);
        }
        *lock(&player.shared.instrument) = instrument;
        Ok(())
    }

    /// Stops playing the MIDI input named, or every input when `input` is none, as [`Engine::play_midi`] was asked, and lets go every note it held.
    pub fn stop_midi(&mut self, input: Option<&str>) -> Result<(), Error> {
        let key = input.map(str::to_string);
        match self.players.remove(&key) {
            Some(_) => Ok(()),
            None => Err(not_played(input)),
        }
    }

    /// The instrument `synth` given `options`, checked.
    fn instrument(&mut self, synth: &str, options: &[(&str, f64)]) -> Result<Instrument, Error> {
        let registered = self
            .synths
            .get(synth)
            .ok_or_else(|| Error::UnknownSynth(synth.to_string()))?;
        let (item, owner, specs) = (
            registered.item.clone(),
            registered.owner.clone(),
            registered.specs.clone(),
        );
        let given: Vec<(String, Value)> = options
            .iter()
            .map(|(name, value)| (name.to_string(), Value::Number(*value)))
            .collect();
        self.check(&owner, &specs, &given)?;
        Ok(Instrument {
            synth: item,
            owner,
            specs,
            given,
        })
    }

    /// Keeps voices of `synth` ready for the keys, unless it builds each note for its note and vel alone.
    fn keep_instrument_ready(&mut self, synth: &str) -> Result<(), Error> {
        if self.kept_ready(synth) >= READY {
            return Ok(());
        }
        match self.keep_ready(synth, READY) {
            Ok(()) | Err(Error::CannotKeepReady(_)) => Ok(()),
            Err(error) => Err(error),
        }
    }
}

fn not_played(input: Option<&str>) -> Error {
    Error::Midi(match input {
        Some(name) => format!("the MIDI input {name} is not played"),
        None => "every MIDI input is not played".to_string(),
    })
}

/// Looks for the inputs of `ports` until `stop`, letting go of the notes of one that went away and reporting it.
fn watch(
    midi: MidiInput,
    shared: Arc<Shared>,
    ports: Vec<Arc<Mutex<Port>>>,
    stop: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("mxs midi".to_string())
        .spawn(move || {
            while !stop.load(Ordering::Acquire) {
                std::thread::sleep(LOOK);
                let connected = names(&midi);
                for port in &ports {
                    let mut held = lock(port);
                    if held.gone || connected.contains(&held.name) {
                        continue;
                    }
                    held.gone = true;
                    let input = held.name.clone();
                    drop(held);
                    shared.let_go(port);
                    let seconds = shared.now();
                    shared.hub.send(Report::InputGone { seconds, input });
                }
            }
        })
        .expect("the MIDI thread starts")
}
