//! The audio side of the engine: it starts each note at its sample, sends it through its effect chain, applies changes when they are due, and fades each note out over its last 10 ms.
//! It allocates and frees nothing while filling audio: notes and chains come built over one ring buffer, and go back over another to be freed by the engine.

use crate::options::Control;
use crate::report::{PartSpan, Tap};
use crate::sample::SampleData;
use crate::workers::{FEWEST, Pool};
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
    /// Only to the analysis of this id, apart from the main output.
    Listen(u64),
}

/// Where a voice's sound is added in the block being filled.
#[derive(Clone, Copy)]
enum Target {
    Chain(usize),
    Tap,
    Listener(usize),
    /// An analysis that has ended, whose sound goes nowhere.
    Nowhere,
}

/// What an analysis hears.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Listened {
    /// The sounds played to it alone, as an input or a sample is.
    Own,
    /// A chain's output, after its gain, mute and solo.
    Chain(u64),
    Main,
}

/// Where a listener puts what it hears.
pub(crate) enum Kept {
    /// Summed to one channel, into rings an analysis reads.
    Rings {
        samples: rtrb::Producer<f32>,
        spans: rtrb::Producer<(u64, u32)>,
    },
    /// Into a buffer from its first frame, over what was there scaled by `level`.
    Buffer { data: Arc<SampleData>, level: f32 },
}

/// Copies what an analysis or a recording into a buffer hears from `start` until `stop`; it never waits.
pub(crate) struct Listener {
    pub id: u64,
    pub listened: Listened,
    pub start: u64,
    pub stop: Option<u64>,
    pub sound: BufferVec,
    pub kept: Kept,
}

/// A sound's two channels summed to one, at the level of a sound in the middle.
fn mono(left: f32, right: f32) -> f32 {
    (left + right) * std::f32::consts::FRAC_1_SQRT_2
}

impl Listener {
    /// Copies the frames of its sound in the block starting at `now` that fall from its start to its stop; an analysis copies only when its rings have room for all of them.
    fn copy(&mut self, now: u64, n: usize) {
        let from = self.start.saturating_sub(now) as usize;
        let to = match self.stop {
            Some(stop) => Ord::min(stop.saturating_sub(now) as usize, n),
            None => n,
        };
        if from >= to {
            return;
        }
        match &mut self.kept {
            Kept::Rings { samples, spans } => {
                if samples.slots() < to - from || spans.slots() == 0 {
                    return;
                }
                for k in from..to {
                    let _ = samples.push(mono(self.sound.at_f32(0, k), self.sound.at_f32(1, k)));
                }
                let _ = spans.push((now + from as u64, (to - from) as u32));
            }
            Kept::Buffer { data, level } => {
                let first = (now + from as u64 - self.start) as usize;
                for k in from..to {
                    let (left, right) = (self.sound.at_f32(0, k), self.sound.at_f32(1, k));
                    let frame = first + k - from;
                    if let [only] = &data.channels[..] {
                        let old = only.get(frame).unwrap_or(0.0);
                        only.set(frame, old * *level + mono(left, right));
                    } else {
                        for (channel, x) in data.channels.iter().zip([left, right]) {
                            let old = channel.get(frame).unwrap_or(0.0);
                            channel.set(frame, old * *level + x);
                        }
                    }
                }
            }
        }
    }
}

/// How many notes may sound at once, how many sound in the block being filled, how many the mixer has stolen to keep to the limit, and how many events reached it after their time, by up to how many frames.
pub(crate) struct Voices {
    pub limit: AtomicUsize,
    pub sounding: AtomicUsize,
    pub stolen: AtomicU64,
    pub late: AtomicU64,
    pub most_late: AtomicU64,
    /// Events in beats dropped for coming a quarter of a beat or more after their beat.
    pub dropped: AtomicU64,
}

impl Voices {
    pub fn new(limit: usize) -> Arc<Voices> {
        Arc::new(Voices {
            limit: AtomicUsize::new(limit),
            sounding: AtomicUsize::new(0),
            stolen: AtomicU64::new(0),
            late: AtomicU64::new(0),
            most_late: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
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
    /// Where the workers filled its sound in the block being filled, if they did.
    pub slot: Option<usize>,
}

/// The frames of the block from `now` to `end` that `voice` fills, from and to.
fn span(voice: &Voice, now: u64, end: u64) -> (usize, usize) {
    // Live input starts at the block it falls in, so it reads the input from the block's first frame.
    let from = if voice.input.is_some() {
        0
    } else {
        voice.start.saturating_sub(now) as usize
    };
    let to = (Ord::min(voice.end, end) - Ord::min(now, voice.end)) as usize;
    (from, Ord::max(to, from))
}

/// A part: the sounds that play into it, its effects, and after them its gain, mute and solo.
pub(crate) struct Chain {
    pub key: u64,
    pub unit: Box<dyn AudioUnit>,
    pub input: BufferVec,
    pub stop: Option<u64>,
    pub tail: u64,
    pub heard: Heard,
    /// What it has played since the last levels report: the frames, the loudest sample and the squares summed, of each channel.
    pub played: (u32, [f32; 2], [f64; 2]),
}

/// How loud a part is heard: its gain, unless it is muted or another part is soloed, reached over `glide` frames so that nothing clicks.
pub(crate) struct Heard {
    pub gain: f32,
    pub muted: bool,
    pub soloed: bool,
    glide: u64,
    level: f32,
    toward: f32,
    step: f32,
    left: u64,
}

impl Heard {
    pub fn new(glide: u64) -> Heard {
        Heard {
            gain: 1.0,
            muted: false,
            soloed: false,
            glide: Ord::max(glide, 1),
            level: 1.0,
            toward: 1.0,
            step: 0.0,
            left: 0,
        }
    }

    fn wanted(&self, solo: bool) -> f32 {
        if self.muted || (solo && !self.soloed) {
            0.0
        } else {
            self.gain
        }
    }

    /// Starts gliding to the level the part should have, given whether any part is soloed.
    fn aim(&mut self, solo: bool) {
        let wanted = self.wanted(solo);
        if wanted != self.toward {
            self.toward = wanted;
            self.left = self.glide;
            self.step = (wanted - self.level) / self.glide as f32;
        }
    }

    /// The level of the next frame.
    fn next(&mut self) -> f32 {
        if self.left > 0 {
            self.left -= 1;
            self.level = if self.left == 0 {
                self.toward
            } else {
                self.level + self.step
            };
        }
        self.level
    }
}

/// A change to how a part is heard.
#[derive(Clone, Copy)]
pub(crate) enum PartChange {
    Gain(f32),
    Mute(bool),
    Solo(bool),
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
    StopChain {
        key: u64,
        at: u64,
    },
    Part {
        key: u64,
        at: u64,
        change: PartChange,
    },
    Stop {
        key: u64,
        at: u64,
    },
    /// Takes back a sound that has not started.
    Drop {
        key: u64,
    },
    Record(Box<Recorder>),
    StopRecording,
    RecordOutput(Box<Recorder>),
    StopRecordingOutput,
    Listen(Box<Listener>),
    StopListening {
        id: u64,
        at: u64,
    },
    Workers(Box<Pool>),
}

// Held only so the engine drops them, off the audio thread.
#[allow(dead_code)]
pub(crate) enum Garbage {
    Voice(Box<Voice>),
    Change(Box<Change>),
    Chain(Box<Chain>),
    Recorder(Box<Recorder>),
    Listener(Box<Listener>),
    Pool(Box<Pool>),
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
    part_changes: Vec<(u64, u64, PartChange)>,
    buffer: BufferVec,
    input: Option<Input>,
    inputs: BufferVec,
    tap: BufferVec,
    recorder: Option<Box<Recorder>>,
    output_recorder: Option<Box<Recorder>>,
    #[allow(clippy::vec_box)]
    listeners: Vec<Box<Listener>>,
    pool: Option<Box<Pool>>,
    now: u64,
    clock: Arc<AtomicU64>,
    limit: Arc<Voices>,
    reports: Option<Tap>,
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
            part_changes: Vec::with_capacity(CAPACITY),
            buffer: BufferVec::new(2),
            input,
            inputs: BufferVec::new(INPUTS),
            tap: BufferVec::new(2),
            recorder: None,
            output_recorder: None,
            listeners: Vec::with_capacity(64),
            pool: None,
            now: clock.load(Ordering::Acquire),
            clock,
            limit,
            reports: None,
        }
    }

    /// Fills voices on `pool`'s workers beside the thread that renders.
    pub fn work_on(&mut self, pool: Box<Pool>) {
        if let Some(old) = self.pool.replace(pool) {
            let _ = self.garbage.push(Garbage::Pool(old));
        }
    }

    /// Copies what `render` fills into `tap`, for the levels reports of a live engine.
    pub fn report_to(&mut self, tap: Tap) {
        self.reports = Some(tap);
    }

    /// Fills `left` and `right` with the next frames.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let first = self.now;
        let mut done = 0;
        while done < left.len() {
            self.receive();
            let mut n = Ord::min(left.len() - done, BLOCK);
            let parts = self.part_changes.iter().map(|change| change.0);
            if let Some(next) = self
                .changes
                .iter()
                .map(|c| c.at)
                .chain(parts)
                .filter(|&at| at > self.now)
                .min()
            {
                n = Ord::min(n, (next - self.now) as usize);
            }
            self.block(&mut left[done..done + n], &mut right[done..done + n]);
            done += n;
        }
        if let Some(tap) = &mut self.reports {
            tap.push(first, left, right);
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
                Command::StartChain(mut chain) => {
                    // A part created while another is soloed starts unheard, with no glide.
                    let solo = self.chains.iter().any(|c| c.heard.soloed);
                    chain.heard.aim(solo);
                    chain.heard.left = Ord::min(chain.heard.left, 1);
                    chain.heard.next();
                    self.chains.push(chain);
                }
                Command::Part { key, at, change } => {
                    if self.part_changes.len() < self.part_changes.capacity() {
                        self.part_changes.push((at, key, change));
                    }
                }
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
                Command::Drop { key } => {
                    let now = self.now;
                    if let Some(voice) = self.voices.iter_mut().find(|v| v.key == key)
                        && voice.start >= now
                    {
                        voice.stolen = true;
                        voice.end = now;
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
                Command::Listen(listener) => {
                    if self.listeners.len() < self.listeners.capacity() {
                        self.listeners.push(listener);
                    } else {
                        let _ = self.garbage.push(Garbage::Listener(listener));
                    }
                }
                Command::Workers(pool) => self.work_on(pool),
                Command::StopListening { id, at } => {
                    if let Some(listener) = self.listeners.iter_mut().find(|l| l.id == id) {
                        listener.stop = Some(Ord::max(at, self.now));
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

        let mut i = 0;
        while i < self.part_changes.len() {
            let (at, key, change) = self.part_changes[i];
            if at <= self.now {
                self.part_changes.swap_remove(i);
                if let Some(chain) = self.chains.iter_mut().find(|c| c.key == key) {
                    match change {
                        PartChange::Gain(gain) => chain.heard.gain = gain,
                        PartChange::Mute(muted) => chain.heard.muted = muted,
                        PartChange::Solo(soloed) => chain.heard.soloed = soloed,
                    }
                }
            } else {
                i += 1;
            }
        }
        let solo = self.chains.iter().any(|c| c.heard.soloed);
        let levels = self.reports.as_ref().map(|tap| tap.every());

        self.keep_to_the_limit(end);

        for chain in &mut self.chains {
            chain.input.clear();
            chain.heard.aim(solo);
        }
        for listener in &mut self.listeners {
            listener.sound.clear();
        }
        self.tap.clear();
        self.read_input(n);

        // Workers fill the voices that play no live input, each into a slot of its own; they are added below in the order of the voices, as on one thread.
        let parallel = match &mut self.pool {
            Some(pool) if pool.workers() > 1 => {
                let now = self.now;
                let filled = self
                    .voices
                    .iter()
                    .filter(|v| v.input.is_none() && v.start < end)
                    .count();
                if filled >= FEWEST {
                    pool.begin();
                    for voice in self.voices.iter_mut() {
                        let (from, to) = span(voice, now, end);
                        voice.slot = None;
                        if to > from && voice.input.is_none() {
                            voice.slot = pool.add(voice, to - from);
                        }
                    }
                    pool.fill();
                }
                filled >= FEWEST
            }
            _ => false,
        };

        let mut i = 0;
        while i < self.voices.len() {
            let voice = &mut self.voices[i];
            if voice.start >= end {
                i += 1;
                continue;
            }
            let (from, to) = span(voice, self.now, end);
            if to > from {
                let length = to - from;
                let slot = voice.slot.filter(|_| parallel);
                if slot.is_none() {
                    let inputs = self.inputs.buffer_ref();
                    let input = match voice.input {
                        Some((first, count)) => inputs.subset(first, count),
                        None => BufferRef::empty(),
                    };
                    voice
                        .unit
                        .process(length, &input, &mut self.buffer.buffer_mut());
                }
                let buffer = match (slot, &self.pool) {
                    (Some(slot), Some(pool)) => pool.output(slot),
                    _ => &self.buffer,
                };
                let target = match voice.out {
                    Out::Chain(key) => self
                        .chains
                        .iter()
                        .position(|c| c.key == key)
                        .map(Target::Chain),
                    Out::Tap => Some(Target::Tap),
                    Out::Listen(id) => Some(
                        self.listeners
                            .iter()
                            .position(|l| l.id == id)
                            .map_or(Target::Nowhere, Target::Listener),
                    ),
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
                    let l = buffer.at_f32(0, k) * gain;
                    let r = buffer.at_f32(1, k) * gain;
                    loudest = loudest.max(l.abs()).max(r.abs());
                    match target {
                        Some(Target::Nowhere) => {}
                        Some(target) => {
                            let input = match target {
                                Target::Chain(c) => &mut self.chains[c].input,
                                Target::Listener(l) => &mut self.listeners[l].sound,
                                _ => &mut self.tap,
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
                // In order, so that voices are added up in the order they were handed over, whenever each was.
                let voice = self.voices.remove(i);
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
                let level = chain.heard.next();
                let (l, r) = (
                    self.buffer.at_f32(0, k) * level,
                    self.buffer.at_f32(1, k) * level,
                );
                left[k] += l;
                right[k] += r;
                for listener in &mut self.listeners {
                    if listener.listened == Listened::Chain(chain.key) {
                        listener.sound.set_f32(0, k, l);
                        listener.sound.set_f32(1, k, r);
                    }
                }
                if let Some(Some(every)) = levels {
                    let (frames, peak, squares) = &mut chain.played;
                    *frames += 1;
                    *peak = [peak[0].max(l.abs()), peak[1].max(r.abs())];
                    squares[0] += f64::from(l) * f64::from(l);
                    squares[1] += f64::from(r) * f64::from(r);
                    let frame = self.now + k as u64 + 1;
                    if frame.is_multiple_of(every) {
                        let span = PartSpan {
                            end: frame,
                            key: chain.key,
                            frames: *frames,
                            peak: *peak,
                            squares: *squares,
                        };
                        chain.played = (0, [0.0; 2], [0.0; 2]);
                        if let Some(tap) = &mut self.reports {
                            tap.part(span);
                        }
                    }
                }
            }
            if levels == Some(None) {
                chain.played = (0, [0.0; 2], [0.0; 2]);
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

        let mut i = 0;
        while i < self.listeners.len() {
            let listener = &mut self.listeners[i];
            if listener.listened == Listened::Main {
                for k in 0..n {
                    listener.sound.set_f32(0, k, left[k]);
                    listener.sound.set_f32(1, k, right[k]);
                }
            }
            listener.copy(self.now, n);
            if listener.stop.is_some_and(|stop| stop <= end) {
                let listener = self.listeners.swap_remove(i);
                let _ = self.garbage.push(Garbage::Listener(listener));
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
        self.limit.sounding.store(count, Ordering::Relaxed);
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
