//! The audio side of the engine: it starts each note at its sample, sends it through its effect chain, applies changes when they are due, and fades each note out over its last 10 ms.
//! It allocates and frees nothing while filling audio: notes and chains come built over one ring buffer, and go back over another to be freed by the engine.

use crate::options::Control;
use fundsp::prelude::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

pub(crate) const CAPACITY: usize = 16384;
/// The most input channels the mixer reads.
pub(crate) const INPUTS: usize = 8;
const BLOCK: usize = MAX_BUFFER_SIZE;
/// Input frames waiting beyond this are skipped, so live input stays within about 40 ms at 48 kHz.
const INPUT_LAG: usize = 2048;

/// Where a voice's sound goes.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Out {
    Main,
    Chain(u64),
    /// Only to the recorder, apart from the main output.
    Tap,
}

/// How many notes may sound at once, how many the mixer has stolen to keep to it, and how many events reached it after their time, by up to how many frames.
pub(crate) struct Voices {
    pub limit: AtomicUsize,
    pub stolen: AtomicU64,
    pub late: AtomicU64,
    pub most_late: AtomicU64,
}

impl Voices {
    pub fn new(limit: usize) -> Arc<Voices> {
        Arc::new(Voices {
            limit: AtomicUsize::new(limit),
            stolen: AtomicU64::new(0),
            late: AtomicU64::new(0),
            most_late: AtomicU64::new(0),
        })
    }
}

pub(crate) struct Voice {
    pub key: u64,
    pub out: Out,
    /// The first input channel and how many the voice reads, for live input.
    pub input: Option<(usize, usize)>,
    pub unit: Box<dyn AudioUnit>,
    pub start: u64,
    /// How many frames its time had passed when it was sent, which moved its start to then.
    pub late: u64,
    pub end: u64,
    pub fade: u64,
    /// The loudest sample of its last block, which picks the quietest note to steal.
    pub level: f32,
    pub stolen: bool,
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

/// Writes what it records from `start`, the tap or the main output, as interleaved stereo, until it is stopped.
pub(crate) struct Recorder {
    pub start: u64,
    pub out: rtrb::Producer<f32>,
}

/// The sound card's input, as interleaved frames of `channels`.
pub(crate) struct Input {
    pub frames: rtrb::Consumer<f32>,
    pub channels: usize,
}

pub(crate) enum Command {
    Start(Box<Voice>),
    Change(Box<Change>),
    StartChain(Box<Chain>),
    StopChain { key: u64, at: u64 },
    Stop { key: u64, at: u64 },
    Record(Box<Recorder>),
    StopRecording,
    RecordOutput(Box<Recorder>),
    StopRecordingOutput,
}

// Held only so the engine drops them, off the audio thread.
#[allow(dead_code)]
pub(crate) enum Garbage {
    Voice(Box<Voice>),
    Change(Box<Change>),
    Chain(Box<Chain>),
    Recorder(Box<Recorder>),
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
    input: Option<Input>,
    inputs: BufferVec,
    tap: BufferVec,
    recorder: Option<Box<Recorder>>,
    output_recorder: Option<Box<Recorder>>,
    now: u64,
    clock: Arc<AtomicU64>,
    limit: Arc<Voices>,
}

impl Mixer {
    pub fn new(
        commands: rtrb::Consumer<Command>,
        garbage: rtrb::Producer<Garbage>,
        clock: Arc<AtomicU64>,
        limit: Arc<Voices>,
        input: Option<Input>,
    ) -> Self {
        Mixer {
            commands,
            garbage,
            voices: Vec::with_capacity(CAPACITY),
            chains: Vec::with_capacity(256),
            changes: Vec::with_capacity(CAPACITY),
            buffer: BufferVec::new(2),
            input,
            inputs: BufferVec::new(INPUTS),
            tap: BufferVec::new(2),
            recorder: None,
            output_recorder: None,
            now: clock.load(Ordering::Acquire),
            clock,
            limit,
        }
    }

    /// Fills `left` and `right` with the next frames.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let mut done = 0;
        while done < left.len() {
            self.receive();
            let mut n = Ord::min(left.len() - done, BLOCK);
            if let Some(next) = self
                .changes
                .iter()
                .map(|c| c.at)
                .filter(|&at| at > self.now)
                .min()
            {
                n = Ord::min(n, (next - self.now) as usize);
            }
            self.block(&mut left[done..done + n], &mut right[done..done + n]);
            done += n;
        }
    }

    pub fn receive(&mut self) {
        while let Ok(command) = self.commands.pop() {
            match command {
                Command::Start(voice) => {
                    self.count_late(voice.late + self.now.saturating_sub(voice.start));
                    self.voices.push(voice);
                }
                Command::Change(change) => {
                    self.count_late(self.now.saturating_sub(change.at));
                    self.changes.push(change);
                }
                Command::StartChain(chain) => self.chains.push(chain),
                Command::StopChain { key, at } => {
                    if let Some(chain) = self.chains.iter_mut().find(|c| c.key == key) {
                        chain.stop = Some(at);
                    }
                }
                Command::Stop { key, at } => {
                    if let Some(voice) = self.voices.iter_mut().find(|v| v.key == key) {
                        let at = Ord::max(Ord::max(at, voice.start), self.now);
                        voice.end = Ord::min(voice.end, at + voice.fade);
                    }
                }
                Command::Record(recorder) => {
                    if let Some(old) = self.recorder.replace(recorder) {
                        let _ = self.garbage.push(Garbage::Recorder(old));
                    }
                }
                Command::StopRecording => {
                    if let Some(old) = self.recorder.take() {
                        let _ = self.garbage.push(Garbage::Recorder(old));
                    }
                }
                Command::RecordOutput(recorder) => {
                    if let Some(old) = self.output_recorder.replace(recorder) {
                        let _ = self.garbage.push(Garbage::Recorder(old));
                    }
                }
                Command::StopRecordingOutput => {
                    if let Some(old) = self.output_recorder.take() {
                        let _ = self.garbage.push(Garbage::Recorder(old));
                    }
                }
            }
        }
    }

    fn count_late(&self, frames: u64) {
        if frames > 0 {
            self.limit.late.fetch_add(1, Ordering::Relaxed);
            self.limit.most_late.fetch_max(frames, Ordering::Relaxed);
        }
    }

    fn block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len();
        let end = self.now + n as u64;
        left.fill(0.0);
        right.fill(0.0);

        // Blocks are cut where the next change falls, so each change applies at its own sample.
        let mut i = 0;
        while i < self.changes.len() {
            if self.changes[i].at <= self.now {
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

        self.keep_to_the_limit(end);

        for chain in &mut self.chains {
            chain.input.clear();
        }
        self.tap.clear();
        self.read_input(n);

        let mut i = 0;
        while i < self.voices.len() {
            let voice = &mut self.voices[i];
            if voice.start >= end {
                i += 1;
                continue;
            }
            // Live input starts at the block it falls in, so it reads the input from the block's first frame.
            let from = if voice.input.is_some() {
                0
            } else {
                voice.start.saturating_sub(self.now) as usize
            };
            let to = (Ord::min(voice.end, end) - Ord::min(self.now, voice.end)) as usize;
            let to = Ord::max(to, from);
            if to > from {
                let length = to - from;
                let inputs = self.inputs.buffer_ref();
                let input = match voice.input {
                    Some((first, count)) => inputs.subset(first, count),
                    None => BufferRef::empty(),
                };
                voice
                    .unit
                    .process(length, &input, &mut self.buffer.buffer_mut());
                let target = match voice.out {
                    Out::Chain(key) => self.chains.iter_mut().position(|c| c.key == key).map(Some),
                    Out::Tap => Some(None),
                    Out::Main => None,
                };
                let mut loudest = 0.0f32;
                for k in 0..length {
                    let at = self.now + (from + k) as u64;
                    let left_to_end = voice.end - at;
                    let gain = if left_to_end < voice.fade {
                        left_to_end as f32 / voice.fade as f32
                    } else {
                        1.0
                    };
                    let l = self.buffer.at_f32(0, k) * gain;
                    let r = self.buffer.at_f32(1, k) * gain;
                    loudest = loudest.max(l.abs()).max(r.abs());
                    match target {
                        Some(c) => {
                            let input = match c {
                                Some(c) => &mut self.chains[c].input,
                                None => &mut self.tap,
                            };
                            input.set_f32(0, from + k, input.at_f32(0, from + k) + l);
                            input.set_f32(1, from + k, input.at_f32(1, from + k) + r);
                        }
                        None => {
                            left[from + k] += l;
                            right[from + k] += r;
                        }
                    }
                }
                voice.level = loudest;
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
            chain
                .unit
                .process(n, &chain.input.buffer_ref(), &mut self.buffer.buffer_mut());
            for k in 0..n {
                left[k] += self.buffer.at_f32(0, k);
                right[k] += self.buffer.at_f32(1, k);
            }
            let key = chain.key;
            let finished = chain.stop.is_some_and(|stop| end >= stop + chain.tail)
                && !self.voices.iter().any(|v| v.out == Out::Chain(key));
            if finished {
                let chain = self.chains.swap_remove(i);
                let _ = self.garbage.push(Garbage::Chain(chain));
            } else {
                i += 1;
            }
        }

        if let Some(recorder) = &mut self.recorder {
            let from = recorder.start.saturating_sub(self.now) as usize;
            for k in from..n {
                let _ = recorder.out.push(self.tap.at_f32(0, k));
                let _ = recorder.out.push(self.tap.at_f32(1, k));
            }
        }
        if let Some(recorder) = &mut self.output_recorder {
            let from = recorder.start.saturating_sub(self.now) as usize;
            for k in from..n {
                let _ = recorder.out.push(left[k]);
                let _ = recorder.out.push(right[k]);
            }
        }

        self.now = end;
        self.clock.store(end, Ordering::Release);
    }

    /// Steals notes until no more than the limit sound by the block's `end`: first the quietest of those already playing, which fade out over their last 10 ms, then the earliest of those starting in this block, which never start.
    /// Live input is not a note, and a note always starts, so a new note past the limit takes the place of an old one.
    fn keep_to_the_limit(&mut self, end: u64) {
        let limit = self.limit.limit.load(Ordering::Relaxed);
        let now = self.now;
        let sounding = |v: &Voice| v.input.is_none() && !v.stolen && v.start < end && v.end > now;
        let mut count = self.voices.iter().filter(|v| sounding(v)).count();
        while count > limit {
            let playing = self
                .voices
                .iter()
                .enumerate()
                .filter(|(_, v)| sounding(v) && v.start < now)
                .min_by(|a, b| a.1.level.total_cmp(&b.1.level));
            let starting = || {
                self.voices
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| sounding(v))
                    .min_by_key(|(_, v)| v.start)
            };
            let Some((i, _)) = playing.or_else(starting) else {
                break;
            };
            let voice = &mut self.voices[i];
            voice.stolen = true;
            voice.end = if voice.start >= now {
                voice.start
            } else {
                Ord::min(voice.end, now + voice.fade)
            };
            self.limit.stolen.fetch_add(1, Ordering::Relaxed);
            count -= 1;
        }
    }

    /// The voices sent so far, taken out of the mixer, for measuring them alone.
    #[cfg(test)]
    pub(crate) fn take_voices(&mut self) -> Vec<Box<Voice>> {
        self.receive();
        std::mem::take(&mut self.voices)
    }

    /// The frames every running recording has room for before its ring is full.
    pub fn recording_room(&self) -> usize {
        let room = |r: &Option<Box<Recorder>>| r.as_ref().map_or(usize::MAX, |r| r.out.slots() / 2);
        Ord::min(room(&self.recorder), room(&self.output_recorder))
    }

    /// Reads the next `n` frames of input, or silence for what has not arrived.
    fn read_input(&mut self, n: usize) {
        self.inputs.clear();
        let Some(input) = &mut self.input else { return };
        let channels = input.channels;
        let waiting = input.frames.slots() / channels;
        if waiting > INPUT_LAG {
            for _ in 0..(waiting - BLOCK) * channels {
                let _ = input.frames.pop();
            }
        }
        let frames = Ord::min(input.frames.slots() / channels, n);
        for k in 0..frames {
            for c in 0..channels {
                let x = input.frames.pop().unwrap_or(0.0);
                if c < INPUTS {
                    self.inputs.set_f32(c, k, x);
                }
            }
        }
    }
}
