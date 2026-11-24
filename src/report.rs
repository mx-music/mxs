//! What the engine says of itself while it plays, to whoever subscribes: its clock, the levels of its output, how many notes sound, its output device and its warnings.
//! The audio thread only copies its output into a ring buffer while levels are wanted; a thread of the engine's own works the reports out, so nothing here makes the audio wait.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

/// How many points a levels report draws the wave with.
pub const WAVE: usize = 64;
/// How many bands a levels report splits the output into, from 50 Hz to 16 kHz.
pub const BANDS: usize = 25;
/// The lowest level a band is given, in dB, which stands for silence.
pub const FLOOR: f32 = -120.0;

/// How many reports wait for a subscriber before the oldest are dropped.
const WAITING: usize = 64;
/// How many warnings wait for [`Engine::warnings`](crate::Engine::warnings) before the oldest are dropped.
const SAID: usize = 1024;
/// How many levels reports a second.
const LEVELS_A_SECOND: f64 = 20.0;
/// The frames whose spectrum a levels report gives, the last of them the report's own.
const SPECTRUM: usize = 8192;

/// A kind of report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Topic {
    /// How the engine's seconds line up with the system clock; a live engine sends it at once, whenever the two move apart by more than 2 ms, and at least once a second, and an offline engine never.
    Clock,
    /// The level, the wave and the spectrum of the main output, twenty times a second.
    Levels,
    /// How many notes sound, about once a second.
    Voices,
    /// The output device, at once and whenever it changes.
    Output,
    /// What [`Engine::warnings`](crate::Engine::warnings) gives, as it is noticed: at once for a value held to its range, and within a second for the counts of late events, stolen notes and late buffers.
    Warnings,
    /// Where the beat is: a beat, its tempo and when it is heard, at once and whenever the tempo changes, every quarter beat while it moves.
    Beats,
    /// What becomes of the passes handed to the parts: that a part's last pass is about to end, and that a pass with a new label has started.
    Passes,
    /// Each key that goes down or comes up on a MIDI input the engine plays, and an input that goes away.
    Keys,
    /// What each analysis [`Engine::analyse`](crate::Engine::analyse) started hears: the loudness and the pitch at its interval, and each onset as it happens.
    Analysis,
}

impl Topic {
    fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// The level of a part over a twentieth of a second, after its effects, its gain, its mute and its solo.
#[derive(Clone, Debug, PartialEq)]
pub struct PartLevels {
    /// The part's name.
    pub name: String,
    /// The loudest sample of the left and the right channel, where 1 is full scale.
    pub peak: [f32; 2],
    /// The RMS level of the left and the right channel.
    pub rms: [f32; 2],
}

/// The level of the main output over a twentieth of a second.
#[derive(Clone, Debug, PartialEq)]
pub struct Levels {
    /// The engine's seconds at the end of the stretch.
    pub seconds: f64,
    /// The loudest sample of the left and the right channel, where 1 is full scale.
    pub peak: [f32; 2],
    /// The RMS level of the left and the right channel.
    pub rms: [f32; 2],
    /// Whether a sample of either channel reached full scale.
    pub full_scale: bool,
    /// The wave of both channels mixed to one, as the sample furthest from 0 in each of [`WAVE`] equal stretches, held to -1 to 1.
    pub wave: [f32; WAVE],
    /// The RMS level in dB of both channels mixed to one, in each of [`BANDS`] bands, over the last 8192 frames; [`Levels::band`] gives a band's edges, and a band with no sound reads [`FLOOR`].
    pub bands: [f32; BANDS],
    /// The level of each part that has a name, in the order of their names.
    pub parts: Vec<PartLevels>,
}

impl Levels {
    /// The lowest and the highest frequency of a band, in Hz: the bands run from 50 Hz to 16 kHz, each as wide as the last on a log scale, about a third of an octave.
    pub fn band(index: usize) -> (f64, f64) {
        let edge = |i: usize| 50.0 * 320f64.powf(i as f64 / BANDS as f64);
        (edge(index), edge(index + 1))
    }
}

/// What the engine says to a subscriber.
/// [The guide to reports](crate::guide::reports) says when each is sent.
#[derive(Clone, Debug, PartialEq)]
pub enum Report {
    /// The sound card plays the engine's `seconds` at the time `system`, which is `delay` seconds after the engine fills them.
    Clock {
        /// The engine's seconds.
        seconds: f64,
        /// The system time at which the sound card plays them.
        system: SystemTime,
        /// Seconds from filling a frame to the sound card playing it, 0 when the device does not say.
        delay: f64,
    },
    /// The levels of the main output.
    Levels(Box<Levels>),
    /// How many notes sound.
    Voices {
        /// The engine's seconds the count was taken at.
        seconds: f64,
        /// The notes that sound; live input is not a note.
        sounding: usize,
    },
    /// The output the engine plays through.
    Output {
        /// The engine's seconds the report was made at.
        seconds: f64,
        /// The device's name; an offline engine has none.
        device: Option<String>,
        /// The rate the engine plays or renders at, in Hz.
        rate: f64,
        /// Milliseconds from filling a frame to the device playing it, when the device says.
        delay_ms: Option<f64>,
    },
    /// A warning, as [`Engine::warnings`](crate::Engine::warnings) words it.
    Warning {
        /// The engine's seconds the warning was noticed at.
        seconds: f64,
        /// The warning.
        message: String,
    },
    /// The engine's `seconds` are the `beat`, played at `bpm` beats a minute from there; the last such report says where every later beat falls, until the next.
    Beat {
        /// The engine's seconds at which the beat falls.
        seconds: f64,
        /// The beat.
        beat: f64,
        /// The beats a minute from that beat on.
        bpm: f64,
        /// The system time at which the sound card plays the beat, which a live engine knows.
        system: Option<SystemTime>,
    },
    /// A part's last pass ends within the beats its caller asked to be told at, and no other pass was handed over to follow it.
    PassEnding {
        /// The engine's seconds at which those beats before its end fall.
        seconds: f64,
        /// The part's name.
        part: String,
        /// The beat the pass ends at.
        beat: f64,
    },
    /// The first event of a pass with another label than the pass before it starts.
    PassStarted {
        /// The engine's seconds at which the event starts.
        seconds: f64,
        /// The part's name.
        part: String,
        /// The pass's label.
        label: u64,
        /// The beat of the event.
        beat: f64,
    },
    /// A key went down or came up on a MIDI input the engine plays.
    Key {
        /// The engine's seconds at which the message arrived.
        seconds: f64,
        /// The input's name.
        input: String,
        /// The key, as MIDI numbers it, 60 for middle C.
        key: u8,
        /// How hard it was struck or let up, from 0 to 127.
        velocity: u8,
        /// Whether it went down.
        down: bool,
    },
    /// A MIDI input the engine played went away, and the notes it held were let go.
    InputGone {
        /// The engine's seconds at which it was missed.
        seconds: f64,
        /// The input's name.
        input: String,
    },
    /// The loudness and the pitch an analysis hears over the window that ends at `seconds`.
    Analysis {
        /// The analysis, as [`Engine::analyse`](crate::Engine::analyse) returned it.
        id: u64,
        /// The engine's seconds of the last sample heard.
        seconds: f64,
        /// The loudness, in decibels below full scale, of the sound summed to one channel at the level a sound in the middle is played at.
        loudness: f64,
        /// The pitch in Hz, from 50 to 2000, or none when the sound has no clear pitch.
        pitch: Option<f64>,
        /// How sure the pitch is, from 0 to 1.
        confidence: f64,
    },
    /// A new note or hit starts in what an analysis hears.
    Onset {
        /// The analysis, as [`Engine::analyse`](crate::Engine::analyse) returned it.
        id: u64,
        /// The engine's seconds of the sample it starts at.
        seconds: f64,
    },
    /// Reports were dropped, the oldest first, as the subscriber did not take them in time; this stands where they were.
    Dropped {
        /// The engine's seconds of the report that follows.
        seconds: f64,
        /// How many were dropped.
        reports: u64,
    },
}

impl Report {
    /// The engine's seconds of the sample the report describes.
    pub fn seconds(&self) -> f64 {
        match self {
            Report::Levels(levels) => levels.seconds,
            Report::Clock { seconds, .. }
            | Report::Voices { seconds, .. }
            | Report::Output { seconds, .. }
            | Report::Warning { seconds, .. }
            | Report::Beat { seconds, .. }
            | Report::PassEnding { seconds, .. }
            | Report::PassStarted { seconds, .. }
            | Report::Key { seconds, .. }
            | Report::InputGone { seconds, .. }
            | Report::Analysis { seconds, .. }
            | Report::Onset { seconds, .. }
            | Report::Dropped { seconds, .. } => *seconds,
        }
    }

    /// The kind of report this is; a count of dropped reports is of no kind.
    pub fn topic(&self) -> Option<Topic> {
        match self {
            Report::Clock { .. } => Some(Topic::Clock),
            Report::Levels(_) => Some(Topic::Levels),
            Report::Voices { .. } => Some(Topic::Voices),
            Report::Output { .. } => Some(Topic::Output),
            Report::Warning { .. } => Some(Topic::Warnings),
            Report::Beat { .. } => Some(Topic::Beats),
            Report::PassEnding { .. } | Report::PassStarted { .. } => Some(Topic::Passes),
            Report::Key { .. } | Report::InputGone { .. } => Some(Topic::Keys),
            Report::Analysis { .. } | Report::Onset { .. } => Some(Topic::Analysis),
            Report::Dropped { .. } => None,
        }
    }
}

struct Queue {
    topics: u16,
    reports: VecDeque<Report>,
    dropped: u64,
}

struct Sink {
    queue: Mutex<Queue>,
    ready: Condvar,
}

struct Inner {
    sinks: Vec<Arc<Sink>>,
    /// The last report of the output and of the beat, which a new subscriber to either is sent at once.
    kept: Vec<Report>,
    names: HashMap<u64, String>,
    said: VecDeque<String>,
    /// Counts the changes to who subscribes to what, so the engine's thread sends the clock and the voices to a new subscriber at once.
    changes: u64,
    stopped: bool,
}

/// Where reports go: each subscriber's queue, and the warnings kept for [`Engine::warnings`](crate::Engine::warnings).
pub(crate) struct Hub {
    wanted: AtomicU16,
    inner: Mutex<Inner>,
    woken: Condvar,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Hub {
    pub fn new() -> Arc<Hub> {
        Arc::new(Hub {
            wanted: AtomicU16::new(0),
            inner: Mutex::new(Inner {
                sinks: Vec::new(),
                kept: Vec::new(),
                names: HashMap::new(),
                said: VecDeque::new(),
                changes: 0,
                stopped: false,
            }),
            woken: Condvar::new(),
        })
    }

    /// Whether anyone subscribes to `topic`; one load, so the audio thread may ask.
    pub fn wants(&self, topic: Topic) -> bool {
        self.wanted.load(Ordering::Relaxed) & topic.bit() != 0
    }

    pub fn send(&self, report: Report) {
        let Some(topic) = report.topic() else { return };
        if !self.wants(topic) {
            return;
        }
        for sink in &lock(&self.inner).sinks {
            let mut queue = lock(&sink.queue);
            if queue.topics & topic.bit() != 0 {
                if queue.reports.len() == WAITING {
                    queue.reports.pop_front();
                    queue.dropped += 1;
                }
                queue.reports.push_back(report.clone());
                sink.ready.notify_all();
            }
        }
    }

    /// Keeps a warning for [`Engine::warnings`](crate::Engine::warnings), and sends it to whoever subscribes to warnings.
    pub fn warn(&self, seconds: f64, message: String) {
        let mut inner = lock(&self.inner);
        if inner.said.len() == SAID {
            inner.said.pop_front();
        }
        inner.said.push_back(message.clone());
        drop(inner);
        self.send(Report::Warning { seconds, message });
    }

    pub fn take_said(&self) -> Vec<String> {
        lock(&self.inner).said.drain(..).collect()
    }

    /// Names a part, for the levels reports.
    pub fn name_part(&self, key: u64, name: &str) {
        lock(&self.inner).names.insert(key, name.to_string());
    }

    pub fn forget_part(&self, key: u64) {
        lock(&self.inner).names.remove(&key);
    }

    pub fn part_name(&self, key: u64) -> Option<String> {
        lock(&self.inner).names.get(&key).cloned()
    }

    /// Sends a report of the output or of the beat, and keeps it: each new subscriber to its kind is sent the last one at once.
    pub fn keep(&self, report: Report) {
        let mut inner = lock(&self.inner);
        inner.kept.retain(|kept| kept.topic() != report.topic());
        inner.kept.push(report.clone());
        drop(inner);
        self.send(report);
    }

    pub fn subscribe(self: &Arc<Hub>, topics: &[Topic]) -> Reports {
        let sink = Arc::new(Sink {
            queue: Mutex::new(Queue {
                topics: 0,
                reports: VecDeque::new(),
                dropped: 0,
            }),
            ready: Condvar::new(),
        });
        lock(&self.inner).sinks.push(sink.clone());
        let reports = Reports {
            hub: self.clone(),
            sink,
        };
        topics.iter().for_each(|&topic| reports.subscribe(topic));
        reports
    }

    fn changed(&self, inner: &mut Inner) {
        let wanted = inner
            .sinks
            .iter()
            .fold(0, |all, sink| all | lock(&sink.queue).topics);
        self.wanted.store(wanted, Ordering::Relaxed);
        inner.changes += 1;
        self.woken.notify_all();
    }

    /// Waits until someone subscribes to something, and returns the count of changes to who subscribes, or None once the engine is gone.
    pub fn wait(&self) -> Option<u64> {
        let mut inner = lock(&self.inner);
        while !inner.stopped && self.wanted.load(Ordering::Relaxed) == 0 {
            inner = self.woken.wait(inner).unwrap_or_else(|e| e.into_inner());
        }
        (!inner.stopped).then_some(inner.changes)
    }

    pub fn stop(&self) {
        lock(&self.inner).stopped = true;
        self.woken.notify_all();
    }
}

/// A subscription to the engine's reports, from [`Engine::subscribe`](crate::Engine::subscribe), which ends when this is dropped.
/// Reports wait here until they are taken, 64 at most: past that the oldest are dropped, and a [`Report::Dropped`] says how many.
pub struct Reports {
    hub: Arc<Hub>,
    sink: Arc<Sink>,
}

impl Reports {
    /// Subscribes to another kind of report.
    pub fn subscribe(&self, topic: Topic) {
        let mut inner = lock(&self.hub.inner);
        let mut queue = lock(&self.sink.queue);
        let new = queue.topics & topic.bit() == 0;
        queue.topics |= topic.bit();
        let kept = inner.kept.iter().find(|kept| kept.topic() == Some(topic));
        if let (true, Some(kept)) = (new, kept) {
            queue.reports.push_back(kept.clone());
            self.sink.ready.notify_all();
        }
        drop(queue);
        self.hub.changed(&mut inner);
    }

    /// Unsubscribes from a kind of report; none of that kind is given after this returns.
    pub fn unsubscribe(&self, topic: Topic) {
        let mut inner = lock(&self.hub.inner);
        let mut queue = lock(&self.sink.queue);
        queue.topics &= !topic.bit();
        queue.reports.retain(|report| report.topic() != Some(topic));
        drop(queue);
        self.hub.changed(&mut inner);
    }

    /// The oldest report that waits, if any.
    pub fn next(&self) -> Option<Report> {
        Reports::take(&mut lock(&self.sink.queue))
    }

    /// The oldest report that waits, or the first to arrive within `timeout`.
    pub fn wait(&self, timeout: Duration) -> Option<Report> {
        let queue = lock(&self.sink.queue);
        let (mut queue, _) = self
            .sink
            .ready
            .wait_timeout_while(queue, timeout, |queue| queue.reports.is_empty())
            .unwrap_or_else(|e| e.into_inner());
        Reports::take(&mut queue)
    }

    fn take(queue: &mut Queue) -> Option<Report> {
        let seconds = queue.reports.front()?.seconds();
        if queue.dropped > 0 {
            let reports = std::mem::take(&mut queue.dropped);
            return Some(Report::Dropped { seconds, reports });
        }
        queue.reports.pop_front()
    }
}

impl Drop for Reports {
    fn drop(&mut self) {
        let mut inner = lock(&self.hub.inner);
        inner.sinks.retain(|sink| !Arc::ptr_eq(sink, &self.sink));
        self.hub.changed(&mut inner);
    }
}

/// What a part played over a stretch of the levels reports: the frames of it, the loudest sample and the squares summed, of each channel.
#[derive(Clone, Copy)]
pub(crate) struct PartSpan {
    /// The frame after the stretch's last.
    pub end: u64,
    pub key: u64,
    pub frames: u32,
    pub peak: [f32; 2],
    pub squares: [f64; 2],
}

/// The audio thread's end of the rings its output and its parts' levels are copied into while levels are wanted.
pub(crate) struct Tap {
    hub: Arc<Hub>,
    every: u64,
    frames: rtrb::Producer<[f32; 2]>,
    spans: rtrb::Producer<(u64, u32)>,
    parts: rtrb::Producer<PartSpan>,
}

/// The other end, which a live engine's reporting thread reads, and an offline engine as it renders.
pub(crate) struct Tapped {
    frames: rtrb::Consumer<[f32; 2]>,
    spans: rtrb::Consumer<(u64, u32)>,
    parts: rtrb::Consumer<PartSpan>,
    read: Vec<(u64, usize)>,
    left: Vec<f32>,
    right: Vec<f32>,
}

/// Rings that hold a second of output at `rate`.
pub(crate) fn tap(hub: Arc<Hub>, rate: f64) -> (Tap, Tapped) {
    let (frames, tapped) = rtrb::RingBuffer::new(rate as usize);
    let (spans, spanned) = rtrb::RingBuffer::new(4096);
    let (parts, parted) = rtrb::RingBuffer::new(4096);
    let tap = Tap {
        hub,
        every: every(rate),
        frames,
        spans,
        parts,
    };
    let tapped = Tapped {
        frames: tapped,
        spans: spanned,
        parts: parted,
        read: Vec::new(),
        left: Vec::new(),
        right: Vec::new(),
    };
    (tap, tapped)
}

/// How many frames a levels report covers.
fn every(rate: f64) -> u64 {
    Ord::max((rate / LEVELS_A_SECOND).round() as u64, 1)
}

impl Tap {
    /// How many frames a levels report covers, counted from the engine's start, while anyone wants levels.
    pub fn every(&self) -> Option<u64> {
        self.hub.wants(Topic::Levels).then_some(self.every)
    }

    /// Copies what a part played up to the end of a levels report's stretch; it never waits.
    pub fn part(&mut self, span: PartSpan) {
        let _ = self.parts.push(span);
    }

    /// Copies the frames from `first` on, unless nobody wants levels or the ring has no room for all of them; it never waits.
    pub fn push(&mut self, first: u64, left: &[f32], right: &[f32]) {
        if !self.hub.wants(Topic::Levels)
            || self.frames.slots() < left.len()
            || self.spans.slots() == 0
        {
            return;
        }
        for (l, r) in left.iter().zip(right) {
            let _ = self.frames.push([*l, *r]);
        }
        let _ = self.spans.push((first, left.len() as u32));
    }
}

impl Tapped {
    fn collect(&mut self) {
        self.read.clear();
        self.left.clear();
        self.right.clear();
        while let Ok((first, count)) = self.spans.pop() {
            for _ in 0..count {
                let [l, r] = self.frames.pop().unwrap_or([0.0; 2]);
                self.left.push(l);
                self.right.push(r);
            }
            self.read.push((first, count as usize));
        }
    }

    /// Hands each stretch of output copied so far to `each`, with the frame it starts at.
    fn each(&self, mut each: impl FnMut(u64, &[f32], &[f32])) {
        let mut from = 0;
        for &(first, count) in &self.read {
            let to = from + count;
            each(first, &self.left[from..to], &self.right[from..to]);
            from = to;
        }
    }

    /// Works the levels reports out of what was copied so far, and sends them.
    // The parts are read after the output: a part's levels are copied before the output they belong to, so none is missed.
    pub fn report(&mut self, analyser: &mut Analyser, hub: &Hub) {
        self.collect();
        while let Ok(span) = self.parts.pop() {
            analyser.played(span);
        }
        if !hub.wants(Topic::Levels) {
            analyser.forget_parts();
            return;
        }
        self.each(|first, left, right| {
            analyser.feed(first, left, right, &|key| hub.part_name(key), |levels| {
                hub.send(Report::Levels(Box::new(levels)))
            })
        });
    }
}

/// Works the levels reports out of the output, a stretch at a time: one report for each twentieth of a second, counted in frames from the engine's start, so the same output gives the same reports however it is cut into stretches.
pub(crate) struct Analyser {
    rate: f64,
    every: u64,
    next: u64,
    /// Whether every frame of the stretch being measured has been seen.
    whole: bool,
    peak: [f32; 2],
    squares: [f64; 2],
    full_scale: bool,
    wave: [f32; WAVE],
    history: Vec<f32>,
    at: usize,
    spectrum: Spectrum,
    /// What each part played, up to the reports not made yet.
    pending: Vec<PartSpan>,
}

impl Analyser {
    /// Keeps what a part played until the report it belongs to is made.
    pub fn played(&mut self, span: PartSpan) {
        self.pending.push(span);
    }

    /// Forgets what the parts played, while nobody wants levels.
    pub fn forget_parts(&mut self) {
        self.pending.clear();
    }

    pub fn new(rate: f64) -> Analyser {
        Analyser {
            rate,
            every: every(rate),
            next: 0,
            whole: true,
            peak: [0.0; 2],
            squares: [0.0; 2],
            full_scale: false,
            wave: [0.0; WAVE],
            history: vec![0.0; SPECTRUM],
            at: 0,
            spectrum: Spectrum::new(),
            pending: Vec::new(),
        }
    }

    /// Takes the output from the frame `first` on, and hands `send` each report it completes, its parts named by `name`.
    pub fn feed(
        &mut self,
        first: u64,
        left: &[f32],
        right: &[f32],
        name: &dyn Fn(u64) -> Option<String>,
        mut send: impl FnMut(Levels),
    ) {
        if first != self.next {
            self.start();
            self.whole = first.is_multiple_of(self.every);
            self.history.fill(0.0);
        }
        for (k, (&l, &r)) in left.iter().zip(right).enumerate() {
            let frame = first + k as u64;
            let within = frame % self.every;
            let mono = (l + r) / 2.0;
            self.history[self.at] = mono;
            self.at = (self.at + 1) % SPECTRUM;
            self.peak = [self.peak[0].max(l.abs()), self.peak[1].max(r.abs())];
            self.squares[0] += f64::from(l) * f64::from(l);
            self.squares[1] += f64::from(r) * f64::from(r);
            self.full_scale |= l.abs() >= 1.0 || r.abs() >= 1.0;
            let point = &mut self.wave[(within as usize * WAVE) / self.every as usize];
            if mono.abs() > point.abs() {
                *point = mono.clamp(-1.0, 1.0);
            }
            if within + 1 == self.every {
                let parts = self.parts(frame + 1, name);
                if self.whole {
                    send(self.levels((frame + 1) as f64 / self.rate, parts));
                }
                self.start();
            }
        }
        self.next = first + left.len() as u64;
    }

    fn start(&mut self) {
        self.whole = true;
        self.peak = [0.0; 2];
        self.squares = [0.0; 2];
        self.full_scale = false;
        self.wave = [0.0; WAVE];
    }

    /// The levels of each named part over the stretch that ends before the frame `end`.
    fn parts(&mut self, end: u64, name: &dyn Fn(u64) -> Option<String>) -> Vec<PartLevels> {
        let rms = |squares: f64, frames: u32| (squares / f64::from(frames.max(1))).sqrt() as f32;
        let mut parts: Vec<PartLevels> = self
            .pending
            .iter()
            .filter(|span| span.end == end)
            .filter_map(|span| {
                Some(PartLevels {
                    name: name(span.key)?,
                    peak: span.peak,
                    rms: [
                        rms(span.squares[0], span.frames),
                        rms(span.squares[1], span.frames),
                    ],
                })
            })
            .collect();
        parts.sort_by(|a, b| a.name.cmp(&b.name));
        self.pending.retain(|span| span.end > end);
        parts
    }

    fn levels(&mut self, seconds: f64, parts: Vec<PartLevels>) -> Levels {
        let rms = |squares: f64| (squares / self.every as f64).sqrt() as f32;
        Levels {
            seconds,
            peak: self.peak,
            rms: [rms(self.squares[0]), rms(self.squares[1])],
            full_scale: self.full_scale,
            wave: self.wave,
            bands: self.spectrum.bands(&self.history, self.at, self.rate),
            parts,
        }
    }
}

/// A Fourier transform of `SPECTRUM` frames under a Hann window, with its tables worked out once.
struct Spectrum {
    window: Vec<f32>,
    /// The square of the window, summed, which the power of each band is taken against.
    power: f64,
    turns: Vec<(f32, f32)>,
    swaps: Vec<usize>,
    real: Vec<f32>,
    imaginary: Vec<f32>,
}

impl Spectrum {
    fn new() -> Spectrum {
        let n = SPECTRUM;
        let around = |i: usize| std::f64::consts::TAU * i as f64 / n as f64;
        let window: Vec<f32> = (0..n)
            .map(|i| (0.5 - 0.5 * around(i).cos()) as f32)
            .collect();
        let bits = n.trailing_zeros();
        Spectrum {
            power: window.iter().map(|w| f64::from(*w) * f64::from(*w)).sum(),
            window,
            turns: (0..n / 2)
                .map(|i| (around(i).cos() as f32, -around(i).sin() as f32))
                .collect(),
            swaps: (0..n)
                .map(|i| i.reverse_bits() >> (usize::BITS - bits))
                .collect(),
            real: vec![0.0; n],
            imaginary: vec![0.0; n],
        }
    }

    /// The level of each band over the frames in `history`, a ring whose oldest frame is at `oldest`.
    fn bands(&mut self, history: &[f32], oldest: usize, rate: f64) -> [f32; BANDS] {
        let n = SPECTRUM;
        for i in 0..n {
            self.real[self.swaps[i]] = history[(oldest + i) % n] * self.window[i];
        }
        self.imaginary.fill(0.0);
        let mut size = 2;
        while size <= n {
            let (half, step) = (size / 2, n / size);
            for start in (0..n).step_by(size) {
                for k in 0..half {
                    let (cos, sin) = self.turns[k * step];
                    let (a, b) = (start + k, start + k + half);
                    let real = self.real[b] * cos - self.imaginary[b] * sin;
                    let imaginary = self.real[b] * sin + self.imaginary[b] * cos;
                    self.real[b] = self.real[a] - real;
                    self.imaginary[b] = self.imaginary[a] - imaginary;
                    self.real[a] += real;
                    self.imaginary[a] += imaginary;
                }
            }
            size *= 2;
        }
        let mut bands = [FLOOR; BANDS];
        for (index, band) in bands.iter_mut().enumerate() {
            let (low, high) = Levels::band(index);
            let bin = |hz: f64| ((hz * n as f64 / rate).round() as usize).min(n / 2);
            let (from, to) = (bin(low), bin(high).max(bin(low) + 1).min(n / 2 + 1));
            let power: f64 = (from..to)
                .map(|k| {
                    f64::from(self.real[k]) * f64::from(self.real[k])
                        + f64::from(self.imaginary[k]) * f64::from(self.imaginary[k])
                })
                .sum();
            // Twice, for the half of the spectrum that mirrors this one.
            let mean_square = 2.0 * power / (n as f64 * self.power);
            *band = (10.0 * mean_square.log10()).max(f64::from(FLOOR)) as f32;
        }
        bands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, peak: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| (peak * (std::f64::consts::TAU * hz * i as f64 / 48_000.0).sin()) as f32)
            .collect()
    }

    fn levels_of(sound: &[f32], cut: usize) -> Vec<Levels> {
        let mut analyser = Analyser::new(48_000.0);
        let mut reports = Vec::new();
        let mut first = 0;
        for stretch in sound.chunks(cut) {
            analyser.feed(first, stretch, stretch, &|_| None, |levels| {
                reports.push(levels)
            });
            first += stretch.len() as u64;
        }
        reports
    }

    #[test]
    fn the_same_output_gives_the_same_reports_however_it_is_cut() {
        let sound = sine(440.0, 0.25, 48_000);
        let whole = levels_of(&sound, 48_000);
        assert_eq!(whole.len(), 20);
        assert_eq!(whole, levels_of(&sound, 64));
        assert_eq!(whole, levels_of(&sound, 777));
        assert_eq!(whole[3].seconds, 0.2);
    }

    #[test]
    fn a_stretch_that_starts_part_way_reports_from_the_next_whole_twentieth() {
        let sound = sine(440.0, 0.25, 9600);
        let mut analyser = Analyser::new(48_000.0);
        let mut seconds = Vec::new();
        analyser.feed(1000, &sound, &sound, &|_| None, |levels| {
            seconds.push(levels.seconds)
        });
        assert_eq!(seconds, [0.1, 0.15, 0.2]);
    }

    #[test]
    fn a_band_is_about_a_third_of_an_octave_and_the_bands_run_from_50_hz_to_16_khz() {
        assert_eq!(Levels::band(0).0, 50.0);
        assert!((Levels::band(BANDS - 1).1 - 16_000.0).abs() < 1e-6);
        let (low, high) = Levels::band(9);
        assert!(low < 440.0 && 440.0 < high, "{low} to {high}");
        assert!((high / low - 2f64.powf(1.0 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn a_full_queue_drops_its_oldest_reports_and_says_how_many() {
        let hub = Hub::new();
        let reports = hub.subscribe(&[Topic::Voices]);
        for sounding in 0..WAITING + 6 {
            let seconds = sounding as f64;
            hub.send(Report::Voices { seconds, sounding });
        }
        assert_eq!(
            reports.next(),
            Some(Report::Dropped {
                seconds: 6.0,
                reports: 6
            })
        );
        let rest: Vec<Report> = std::iter::from_fn(|| reports.next()).collect();
        assert_eq!(rest.len(), WAITING);
        assert_eq!(rest[0].seconds(), 6.0);
    }

    #[test]
    fn a_report_goes_only_to_those_subscribed_to_its_kind_and_none_after_unsubscribing() {
        let hub = Hub::new();
        let voices = hub.subscribe(&[Topic::Voices]);
        let warnings = hub.subscribe(&[Topic::Warnings]);
        assert!(hub.wants(Topic::Voices) && !hub.wants(Topic::Levels));
        let report = Report::Voices {
            seconds: 1.0,
            sounding: 2,
        };
        hub.send(report.clone());
        hub.warn(1.5, "held".to_string());
        assert_eq!(voices.next(), Some(report.clone()));
        assert_eq!(voices.next(), None);
        assert!(
            matches!(warnings.next(), Some(Report::Warning { seconds, message }) if seconds == 1.5 && message == "held")
        );
        hub.send(report);
        voices.unsubscribe(Topic::Voices);
        assert_eq!(voices.next(), None);
        assert!(!hub.wants(Topic::Voices));
        drop(warnings);
        assert!(!hub.wants(Topic::Warnings));
        assert_eq!(hub.take_said(), ["held"]);
    }

    #[test]
    fn a_new_subscriber_to_the_output_is_sent_it_at_once() {
        let hub = Hub::new();
        let output = Report::Output {
            seconds: 0.0,
            device: None,
            rate: 48_000.0,
            delay_ms: None,
        };
        hub.keep(output.clone());
        let reports = hub.subscribe(&[Topic::Output]);
        assert_eq!(reports.next(), Some(output));
    }

    #[test]
    fn the_tap_copies_nothing_until_levels_are_wanted_and_then_each_stretch_with_its_frame() {
        let hub = Hub::new();
        let (mut tap, mut tapped) = tap(hub.clone(), 48_000.0);
        tap.push(0, &[0.1; 8], &[0.2; 8]);
        let mut seen = Vec::new();
        tapped.collect();
        tapped.each(|first, left, _| seen.push((first, left.len())));
        assert!(seen.is_empty());
        let _reports = hub.subscribe(&[Topic::Levels]);
        tap.push(8, &[0.1; 8], &[0.2; 8]);
        tap.push(16, &[0.1; 4], &[0.2; 4]);
        tapped.collect();
        tapped.each(|first, left, right| {
            assert!(left.iter().all(|x| *x == 0.1) && right.iter().all(|x| *x == 0.2));
            seen.push((first, left.len()))
        });
        assert_eq!(seen, [(8, 8), (16, 4)]);
    }
}
