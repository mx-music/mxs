//! Playing one instrument from MIDI messages: a key starts a note at its pitch and loudness, held until the key comes up, which lets the note go, the sustain pedal holds what is sounding, and the pitch bend wheel bends it.
//!
//! A [`Keyboard`] takes the bytes of each message and returns the events to hand to [`Engine::play`](crate::Engine::play), so it works with whatever reads the MIDI port.
//! [The guide to playing live](crate::guide::live) has a program, and the `keys` program of the repository reads a port with the `midir` crate.

use crate::{Event, Synth, Target};
use std::collections::HashMap;

/// How far the pitch bend wheel bends a note at either end, in semitones.
pub const BEND: f64 = 2.0;
/// How long a note of a synth that does not follow bend glides to each new position of the wheel, smoothing the steps between its messages.
const BEND_GLIDE: f64 = 0.01;

/// One instrument played from MIDI, turning each message into the engine's events.
pub struct Keyboard {
    synth: String,
    options: Vec<String>,
    /// The id of each key's note, while the key is down.
    down: HashMap<u8, u64>,
    /// The notes whose keys came up while the pedal was down, with their keys.
    held: Vec<(u8, u64)>,
    pedal: bool,
    bend: f64,
    next: u64,
}

impl Keyboard {
    /// A keyboard that plays notes of `synth`.
    pub fn new(synth: &Synth) -> Keyboard {
        let options = synth.specs().iter().map(|s| s.name.clone()).collect();
        Keyboard {
            synth: synth.name().to_string(),
            options,
            down: HashMap::new(),
            held: Vec::new(),
            pedal: false,
            bend: 0.0,
            next: 0,
        }
    }

    /// The events one MIDI message makes, at `at` seconds; a message other than a key, the sustain pedal or the wheel makes none.
    pub fn message(&mut self, bytes: &[u8], at: f64) -> Vec<(f64, Event)> {
        match *bytes {
            [status, key, velocity] if status & 0xf0 == 0x90 && velocity > 0 => {
                self.key_down(key, velocity, at)
            }
            [status, key, _] if status & 0xf0 == 0x80 || status & 0xf0 == 0x90 => {
                self.key_up(key, at)
            }
            [status, 64, value] if status & 0xf0 == 0xb0 => self.set_pedal(value >= 64, at),
            [status, low, high] if status & 0xf0 == 0xe0 => {
                let position = ((high as u16) << 7 | low as u16) as f64;
                self.set_bend((position - 8192.0) / 8192.0 * BEND, at)
            }
            _ => Vec::new(),
        }
    }

    /// Numbers the notes from `first` on.
    #[cfg(feature = "keys")]
    pub(crate) fn numbered_from(&mut self, first: u64) {
        self.next = first;
    }

    /// Plays `synth` from the next key on, leaving the notes that sound to their keys and the pedal.
    #[cfg(feature = "keys")]
    pub(crate) fn play_as(&mut self, synth: &Synth) {
        self.synth = synth.name().to_string();
        self.options = synth.specs().iter().map(|s| s.name.clone()).collect();
    }

    /// Lets go every note that sounds, held by its key or by the pedal.
    #[cfg(feature = "keys")]
    pub(crate) fn let_go_all(&mut self, at: f64) -> Vec<(f64, Event)> {
        let ids: Vec<u64> = self
            .down
            .drain()
            .map(|(_, id)| id)
            .chain(self.held.drain(..).map(|(_, id)| id))
            .collect();
        if ids.is_empty() {
            return Vec::new();
        }
        vec![(at, Event::Release { ids })]
    }

    fn has(&self, option: &str) -> bool {
        self.options.iter().any(|o| o == option)
    }

    fn key_down(&mut self, key: u8, velocity: u8, at: f64) -> Vec<(f64, Event)> {
        let mut events = self.key_up(key, at);
        self.next += 1;
        let id = self.next;
        let vel = velocity as f64 / 127.0;
        let loudness = if self.has("vel") { "vel" } else { "amp" };
        let mut options = vec![(loudness, vel)];
        if self.has("bend") {
            options.push(("note", key as f64));
            if self.bend != 0.0 {
                options.push(("bend", self.bend));
            }
        } else if self.has("note") {
            options.push(("note", key as f64 + self.bend));
        }
        if self.has("pedal") {
            options.push(("pedal", if self.pedal { 1.0 } else { 0.0 }));
        }
        events.push((at, Event::held(id, &self.synth, &options)));
        self.down.insert(key, id);
        events
    }

    fn key_up(&mut self, key: u8, at: f64) -> Vec<(f64, Event)> {
        let Some(id) = self.down.remove(&key) else {
            return Vec::new();
        };
        if self.pedal {
            self.held.push((key, id));
            Vec::new()
        } else {
            vec![(at, Event::Release { ids: vec![id] })]
        }
    }

    fn set_pedal(&mut self, down: bool, at: f64) -> Vec<(f64, Event)> {
        self.pedal = down;
        let mut events = Vec::new();
        if self.has("pedal") {
            let ids = self
                .down
                .values()
                .chain(self.held.iter().map(|(_, id)| id))
                .copied()
                .collect();
            events.push((
                at,
                Event::Set {
                    target: Target::Sounds(ids),
                    options: vec![("pedal".into(), if down { 1.0 } else { 0.0 })],
                },
            ));
        }
        if !down && !self.held.is_empty() {
            let ids = self.held.drain(..).map(|(_, id)| id).collect();
            events.push((at, Event::Release { ids }));
        }
        events
    }

    fn set_bend(&mut self, bend: f64, at: f64) -> Vec<(f64, Event)> {
        self.bend = bend;
        let sounding: Vec<(u8, u64)> = self
            .down
            .iter()
            .map(|(&k, &id)| (k, id))
            .chain(self.held.iter().copied())
            .collect();
        if self.has("bend") {
            let ids = sounding.into_iter().map(|(_, id)| id).collect();
            let options = vec![("bend".into(), bend)];
            return vec![(
                at,
                Event::Set {
                    target: Target::Sounds(ids),
                    options,
                },
            )];
        }
        if !self.has("note") {
            return Vec::new();
        }
        sounding
            .into_iter()
            .map(|(key, id)| {
                let options = vec![
                    ("note".into(), key as f64 + bend),
                    ("note_slide".into(), BEND_GLIDE),
                ];
                (
                    at,
                    Event::Set {
                        target: Target::Sounds(vec![id]),
                        options,
                    },
                )
            })
            .collect()
    }
}

/// The input of `names` to play from: the one named, or the first; or, for a name none has, what the inputs are.
pub fn choose_input(names: &[String], wanted: Option<&str>) -> Result<usize, String> {
    let known = names.join(", ");
    let listed = if names.is_empty() {
        "there are none".to_string()
    } else {
        format!("the inputs are {known}")
    };
    match wanted {
        None if names.is_empty() => Err(format!("no MIDI input to play from; {listed}")),
        None => Ok(0),
        Some(name) => names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| format!("no MIDI input named {name}; {listed}")),
    }
}
