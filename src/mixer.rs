//! The audio side of the engine: it starts each note at its sample, sends it through its effect chain, applies changes when they are due, and fades each note out over its last 10 ms.
//! It allocates and frees nothing while filling audio: notes and chains come built over one ring buffer, and go back over another to be freed by the engine.

use crate::options::Control;
use fundsp::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const CAPACITY: usize = 16384;
const BLOCK: usize = MAX_BUFFER_SIZE;

pub(crate) struct Voice {
    pub chain: Option<u64>,
    pub unit: Box<dyn AudioUnit>,
    pub start: u64,
    pub end: u64,
    pub fade: u64,
}

pub(crate) struct Chain {
    pub key: u64,
    pub unit: Box<dyn AudioUnit>,
    pub input: BufferVec,
    pub stop: Option<u64>,
    pub tail: u64,
}

pub(crate) struct Change {
    pub at: u64,
    pub control: Control,
    pub value: Option<f32>,
    pub slide: Option<f32>,
}

pub(crate) enum Command {
    Start(Box<Voice>),
    Change(Box<Change>),
    StartChain(Box<Chain>),
    StopChain { key: u64, at: u64 },
}

// Held only so the engine drops them, off the audio thread.
#[allow(dead_code)]
pub(crate) enum Garbage {
    Voice(Box<Voice>),
    Change(Box<Change>),
    Chain(Box<Chain>),
}

pub(crate) struct Mixer {
    commands: rtrb::Consumer<Command>,
    garbage: rtrb::Producer<Garbage>,
    // Boxed so each goes back to the engine as the same allocation it came in.
    #[allow(clippy::vec_box)]
    voices: Vec<Box<Voice>>,
    #[allow(clippy::vec_box)]
    chains: Vec<Box<Chain>>,
    #[allow(clippy::vec_box)]
    changes: Vec<Box<Change>>,
    buffer: BufferVec,
    now: u64,
    clock: Arc<AtomicU64>,
}

impl Mixer {
    pub fn new(commands: rtrb::Consumer<Command>, garbage: rtrb::Producer<Garbage>, clock: Arc<AtomicU64>) -> Self {
        Mixer {
            commands,
            garbage,
            voices: Vec::with_capacity(CAPACITY),
            chains: Vec::with_capacity(256),
            changes: Vec::with_capacity(CAPACITY),
            buffer: BufferVec::new(2),
            now: clock.load(Ordering::Acquire),
            clock,
        }
    }

    /// Fills `left` and `right` with the next frames.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let mut done = 0;
        while done < left.len() {
            let n = Ord::min(left.len() - done, BLOCK);
            self.block(&mut left[done..done + n], &mut right[done..done + n]);
            done += n;
        }
    }

    fn receive(&mut self) {
        while let Ok(command) = self.commands.pop() {
            match command {
                Command::Start(voice) => self.voices.push(voice),
                Command::Change(change) => self.changes.push(change),
                Command::StartChain(chain) => self.chains.push(chain),
                Command::StopChain { key, at } => {
                    if let Some(chain) = self.chains.iter_mut().find(|c| c.key == key) {
                        chain.stop = Some(at);
                    }
                }
            }
        }
    }

    fn block(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.receive();
        let n = left.len();
        let end = self.now + n as u64;
        left.fill(0.0);
        right.fill(0.0);

        // Changes apply at the start of the block they fall in, within 1.3 ms of their time at 48 kHz.
        let mut i = 0;
        while i < self.changes.len() {
            if self.changes[i].at < end {
                let change = self.changes.swap_remove(i);
                if let Some(slide) = change.slide {
                    change.control.slide.set_value(slide);
                }
                if let Some(value) = change.value {
                    change.control.target.set_value(value);
                }
                let _ = self.garbage.push(Garbage::Change(change));
            } else {
                i += 1;
            }
        }

        for chain in &mut self.chains {
            chain.input.clear();
        }

        let mut i = 0;
        while i < self.voices.len() {
            let voice = &mut self.voices[i];
            if voice.start >= end {
                i += 1;
                continue;
            }
            let from = voice.start.saturating_sub(self.now) as usize;
            let to = (Ord::min(voice.end, end) - Ord::min(self.now, voice.end)) as usize;
            let to = Ord::max(to, from);
            if to > from {
                let length = to - from;
                voice.unit.process(length, &BufferRef::empty(), &mut self.buffer.buffer_mut());
                let target = voice.chain.and_then(|key| self.chains.iter_mut().position(|c| c.key == key));
                for k in 0..length {
                    let at = self.now + (from + k) as u64;
                    let left_to_end = voice.end - at;
                    let gain = if left_to_end < voice.fade { left_to_end as f32 / voice.fade as f32 } else { 1.0 };
                    let l = self.buffer.at_f32(0, k) * gain;
                    let r = self.buffer.at_f32(1, k) * gain;
                    match target {
                        Some(c) => {
                            let input = &mut self.chains[c].input;
                            input.set_f32(0, from + k, input.at_f32(0, from + k) + l);
                            input.set_f32(1, from + k, input.at_f32(1, from + k) + r);
                        }
                        None => {
                            left[from + k] += l;
                            right[from + k] += r;
                        }
                    }
                }
            }
            if voice.end <= end {
                let voice = self.voices.swap_remove(i);
                let _ = self.garbage.push(Garbage::Voice(voice));
            } else {
                i += 1;
            }
        }

        let mut i = 0;
        while i < self.chains.len() {
            let chain = &mut self.chains[i];
            chain.unit.process(n, &chain.input.buffer_ref(), &mut self.buffer.buffer_mut());
            for k in 0..n {
                left[k] += self.buffer.at_f32(0, k);
                right[k] += self.buffer.at_f32(1, k);
            }
            let key = chain.key;
            let finished = chain.stop.is_some_and(|stop| end >= stop + chain.tail)
                && !self.voices.iter().any(|v| v.chain == Some(key));
            if finished {
                let chain = self.chains.swap_remove(i);
                let _ = self.garbage.push(Garbage::Chain(chain));
            } else {
                i += 1;
            }
        }

        self.now = end;
        self.clock.store(end, Ordering::Release);
    }
}
