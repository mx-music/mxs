//! Synths that play figures fitted to recordings of struck notes partial by partial, as `mix mx.fit` writes them.
//! Each partial is a sine whose level in dB runs in straight lines between fixed times after the strike, and falls at a last slope after them.
//! The strike's noise is white noise through each band's filter, its power in straight lines between the same times.
//! Figures of held notes, bowed, blown or played on an organ, hold each level after the last time until the note is let go, then fall at its slope, with the recordings' vibrato.

use super::common::{effort, followed_effort, pressed};
use crate::nodes::hz;
use crate::synth::{Given, Ready};
use crate::{Context, Note, Options};
use fundsp::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};

/// The seconds over which a sine rises at its start.
const RISE: f64 = 0.001;
/// The level no noise band goes below, where it is silent.
const SILENT: f64 = -160.0;
/// The seconds the noise runs through a band's filters before the strike, so the band starts with the strike instead of ringing up after it.
const WARM: f64 = 0.1;
/// The samples between two readings of the levels, which glide in straight lines in between.
const BLOCK: usize = 16;
/// The seconds over which a slur moves each partial to the new note's level.
const SLUR: f64 = 0.015;
/// The seconds a slur's catch lasts, as the breath settles on the new note.
const CATCH: f64 = 0.04;
/// How much faster a tongued note runs through its recorded attack than a note started from rest, as a played Syrinx's tongued notes stand 3 dB under for 30 ms at the median.
const TONGUE: f64 = 2.5;
/// A breath taken after a phrase, as a played Syrinx's ten breaths: 450 ms at the median, 46 dB under the loudest notes, which is 40 dB under the held note it follows, its power in octaves from 100 Hz to 16 kHz in dB against its whole.
const INHALE: f64 = 0.45;
const INHALE_UNDER: f64 = 40.0;
const INHALE_OCTAVES: [(f64, f64, f64); 7] = [
    (100.0, 250.0, -34.0),
    (250.0, 500.0, -15.0),
    (500.0, 1000.0, -6.0),
    (1000.0, 2000.0, -4.0),
    (2000.0, 4000.0, -10.0),
    (4000.0, 8000.0, -16.0),
    (8000.0, 16000.0, -22.0),
];
/// The seconds the pitch takes through a leap or a tongued note.
const LEAP: f64 = 0.008;
/// The seconds the pitch takes through a slur at the median, as a played Syrinx's moves from 20 cents off one note to within 20 cents of the next in 12 ms at the median and 8 to 19 between its quartiles.
const BEND: f64 = 0.022;
/// The fewest seconds between two readings of the figures as vel moves.
const SWELL: f64 = 0.01;

/// The figures of an instrument's recorded notes.
pub(super) struct Figures {
    times: Vec<f64>,
    bands: Vec<(f64, f64)>,
    recordings: Vec<Recording>,
    /// Each velocity recorded, softest first, with its recordings from the lowest note up.
    layers: Vec<(u8, Vec<usize>)>,
    /// The sum of the squares of each band's filters' response to an impulse, once asked for.
    passed: OnceLock<Vec<f64>>,
    /// The middle note recorded.
    middle: f64,
}

/// A partial no note plays, which stands for one not taken yet.
static NO_PARTIAL: Partial = Partial {
    ratio: 0.0,
    harmonic: 0,
    shape: Shape {
        levels: Vec::new(),
        tail: 0.0,
    },
    swing: 0.0,
};

struct Recording {
    note: u8,
    velocity: u8,
    partials: Vec<Partial>,
    noise: Vec<Shape>,
    vibrato: Option<Vibrato>,
}

/// A held note's vibrato: the seconds it holds from, where its vibrato is full, its rate in Hz and its depth in cents either side.
#[derive(Clone, Copy, Default)]
pub(super) struct Vibrato {
    pub settles: f64,
    pub rate: f64,
    pub depth: f64,
}

#[derive(Clone)]
struct Partial {
    ratio: f64,
    harmonic: u16,
    shape: Shape,
    /// How far its level swings with the vibrato, in dB at its full depth, rising as the pitch does.
    swing: f64,
}

/// A level in dB at each of the times, and its slope in dB a second after the last.
#[derive(Clone)]
struct Shape {
    levels: Vec<f64>,
    tail: f64,
}

/// What a note plays: each partial's frequency and shape, and each noise band's edges and shape.
#[derive(Clone)]
pub(super) struct Voice {
    times: Vec<f64>,
    partials: Vec<(f64, Shape)>,
    noise: Vec<((f64, f64), Shape)>,
    swings: Vec<f64>,
    vibrato: Option<Vibrato>,
    /// Shapes kept for the next note it is filled with, so filling it again needs no new room.
    spare: Vec<Shape>,
}

/// Reads figures; past the end it gives zeros and is short, and a count that more items than the bytes left could hold is 0 and short.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    short: bool,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let Some(taken) = self.bytes.get(self.at..self.at + N) else {
            self.short = true;
            return [0; N];
        };
        self.at += N;
        taken.try_into().unwrap()
    }

    /// A count of items each at least `each` bytes long.
    fn count(&mut self, each: usize) -> usize {
        let count = self.u16() as usize;
        if count * each > self.bytes.len() - self.at {
            self.short = true;
            return 0;
        }
        count
    }

    fn u8(&mut self) -> u8 {
        self.take::<1>()[0]
    }

    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }

    fn f32(&mut self) -> f64 {
        f32::from_le_bytes(self.take()) as f64
    }

    fn tenths(&mut self) -> f64 {
        i16::from_le_bytes(self.take()) as f64 / 10.0
    }

    fn shape(&mut self, times: usize) -> Shape {
        let tail = self.tenths();
        Shape {
            levels: (0..times).map(|_| self.tenths()).collect(),
            tail,
        }
    }
}

impl Figures {
    /// Reads figures as `mix mx.fit` writes them: "MXF1", the times, the noise bands, then each recording's note, velocity, partials and noise.
    /// Figures of held notes start "MXH1", and give each recording's vibrato before its partials, and each partial's swing with it after its slope.
    #[cfg(any(feature = "tables", test))]
    pub fn read(bytes: &[u8]) -> Figures {
        Figures::try_read(bytes).expect("figures compiled in are whole")
    }

    /// Reads figures as [`Figures::read`] does, or says why the bytes are not figures.
    pub fn try_read(bytes: &[u8]) -> Result<Figures, String> {
        let mut reader = Reader {
            bytes,
            at: 0,
            short: false,
        };
        let magic = reader.take::<4>();
        let held = &magic == b"MXH1";
        if !held && &magic != b"MXF1" {
            return Err("not figures of struck or held notes".to_string());
        }
        let count = reader.count(4);
        let times: Vec<f64> = (0..count).map(|_| reader.f32()).collect();
        let bands = (0..reader.count(8))
            .map(|_| (reader.f32(), reader.f32()))
            .collect::<Vec<_>>();
        let vibrato_bytes = if held { 16 } else { 0 };
        let noise_bytes = bands.len() * 2 * (1 + times.len());
        let partial_bytes = 8 + if held { 2 } else { 0 } + 2 * times.len();
        let recordings = (0..reader.count(4 + vibrato_bytes + noise_bytes))
            .map(|_| {
                let (note, velocity) = (reader.u8(), reader.u8());
                let vibrato = held.then(|| {
                    let (settles, _release, rate, depth) =
                        (reader.f32(), reader.f32(), reader.f32(), reader.f32());
                    Vibrato {
                        settles,
                        rate,
                        depth,
                    }
                });
                let partials = (0..reader.count(partial_bytes))
                    .map(|_| {
                        let (ratio, harmonic) = (reader.f32(), reader.u16());
                        let tail = reader.tenths();
                        let swing = if held { reader.tenths() } else { 0.0 };
                        Partial {
                            ratio,
                            harmonic,
                            shape: Shape {
                                levels: (0..times.len()).map(|_| reader.tenths()).collect(),
                                tail,
                            },
                            swing,
                        }
                    })
                    .collect();
                let noise = bands.iter().map(|_| reader.shape(times.len())).collect();
                Recording {
                    note,
                    velocity,
                    partials,
                    noise,
                    vibrato,
                }
            })
            .collect();
        if reader.short || reader.at != bytes.len() {
            return Err("figures cut short, or with bytes after them".to_string());
        }
        Ok(Figures::new(times, bands, recordings))
    }

    // Each recording's partials are kept in the order a note groups them in, so a note merges them
    // without sorting.
    fn new(times: Vec<f64>, bands: Vec<(f64, f64)>, mut recordings: Vec<Recording>) -> Figures {
        for recording in &mut recordings {
            recording.partials.sort_by(|a, b| {
                (a.harmonic, a.ratio)
                    .partial_cmp(&(b.harmonic, b.ratio))
                    .unwrap()
            });
        }
        let mut velocities: Vec<u8> = recordings.iter().map(|r| r.velocity).collect();
        velocities.sort();
        velocities.dedup();
        let layers = velocities
            .into_iter()
            .map(|velocity| {
                let mut at: Vec<usize> = (0..recordings.len())
                    .filter(|i| recordings[*i].velocity == velocity)
                    .collect();
                at.sort_by_key(|i| recordings[*i].note);
                (velocity, at)
            })
            .collect();
        let mut notes: Vec<u8> = recordings.iter().map(|r| r.note).collect();
        notes.sort();
        let middle = notes.get(notes.len() / 2).map_or(0.0, |n| *n as f64);
        Figures {
            times,
            bands,
            recordings,
            layers,
            passed: OnceLock::new(),
            middle,
        }
    }

    /// The energy of a voice's partials and noise over the first half second.
    pub fn whole_energy(&self, voice: &Voice) -> f64 {
        let passed = self.passed.get_or_init(|| {
            self.bands
                .iter()
                .map(|(low, high)| response_energy(band_filter(*low, *high, DEFAULT_SR)))
                .collect()
        });
        let steps = 100;
        // A sine's energy counts its amplitude squared, twice its power; white noise from -1 to 1 has a power of 1/3, and the filters pass the sum of their response squared of it.
        let noise = voice
            .noise
            .iter()
            .zip(passed)
            .map(|((_, shape), passed)| {
                (0..steps)
                    .map(|i| shape.power_at(&voice.times, (i as f64 + 0.5) * 0.5 / steps as f64))
                    .sum::<f64>()
                    * 2.0
                    / 3.0
                    * passed
            })
            .sum::<f64>()
            * 0.5
            / steps as f64;
        voice.energy() + noise
    }

    /// The recordings a note at vel takes its figures from, with their weights: the nearest recorded velocities below and above vel, and at each the nearest notes recorded at that velocity below and above the note, in straight lines between them.
    /// Velocity comes first since it changes the sound more than a few semitones do, and few notes are recorded at more than one velocity.
    fn nearest(&self, note: f64, vel: f64) -> ([(&Recording, f64); 4], usize) {
        let mut nearest = [(&self.recordings[0], 0.0); 4];
        let mut count = 0;
        let (layers, n) = around(self.layers.len(), |j| self.layers[j].0 as f64, vel * 127.0);
        for &(j, across) in &layers[..n] {
            let at = &self.layers[j].1;
            let (keys, m) = around(at.len(), |i| self.recordings[at[i]].note as f64, note);
            for &(i, along) in &keys[..m] {
                let weight = along * across;
                if weight > 0.0 {
                    nearest[count] = (&self.recordings[at[i]], weight);
                    count += 1;
                }
            }
        }
        (nearest, count)
    }

    /// Whether a recording was made at this note.
    pub fn recorded(&self, note: f64) -> bool {
        note.fract() == 0.0 && self.recordings.iter().any(|r| r.note as f64 == note)
    }

    /// The softest velocity recorded, as vel from 0 to 1.
    pub fn softest(&self) -> f64 {
        self.recordings
            .iter()
            .map(|r| r.velocity)
            .min()
            .unwrap_or(127) as f64
            / 127.0
    }

    /// What a note at vel plays, from the recordings nearest it.
    pub fn voice(&self, note: f64, vel: f64) -> Voice {
        let mut voice = Voice::empty();
        self.fill(&mut voice, note, vel);
        voice
    }

    /// A voice with room for any note of these figures, which `fill` then fills with no new room.
    pub fn room(&self) -> Voice {
        Figures::room_among(&[self])
    }

    /// A voice with room for a note of any of `all`.
    fn room_among(all: &[&Figures]) -> Voice {
        let most = |of: fn(&Figures) -> usize| all.iter().map(|f| of(f)).max().unwrap_or(0);
        let partials = 4 * most(|f| {
            f.recordings
                .iter()
                .map(|r| r.partials.len())
                .max()
                .unwrap_or(0)
        });
        let (times, bands) = (most(|f| f.times.len()), most(|f| f.bands.len()));
        let shape = || Shape {
            levels: Vec::with_capacity(times),
            tail: 0.0,
        };
        Voice {
            times: Vec::with_capacity(times),
            partials: Vec::with_capacity(partials),
            noise: Vec::with_capacity(bands),
            swings: Vec::with_capacity(partials),
            vibrato: None,
            spare: (0..partials + bands).map(|_| shape()).collect(),
        }
    }

    /// Fills `voice` with what a note at vel plays, from the recordings nearest it.
    /// A partial takes each recording's level of it at its weight, in dB, quieter by the weight of the recordings that lack it.
    pub fn fill(&self, voice: &mut Voice, note: f64, vel: f64) {
        let (nearest, count) = self.nearest(note, vel);
        let nearest = &nearest[..count];
        let pitch = hz(note);
        voice.clear();
        voice.times.extend_from_slice(&self.times);
        // The vibrato's rate is the weighted rate of the recordings that have one, its depth the weighted depth of all, so a soft recording with none narrows it.
        let vibrato = nearest.iter().any(|(r, _)| r.vibrato.is_some()).then(|| {
            let total: f64 = nearest.iter().map(|(_, w)| w).sum();
            let each = |f: &dyn Fn(&Vibrato) -> f64| {
                nearest
                    .iter()
                    .map(|(r, w)| r.vibrato.as_ref().map_or(0.0, f) * w)
                    .sum::<f64>()
                    / total
            };
            let swinging: f64 = nearest
                .iter()
                .filter(|(r, _)| r.vibrato.is_some_and(|v| v.depth > 0.0))
                .map(|(_, w)| w)
                .sum();
            let rate = if swinging > 0.0 {
                nearest
                    .iter()
                    .map(|(r, w)| r.vibrato.filter(|v| v.depth > 0.0).map_or(0.0, |v| v.rate) * w)
                    .sum::<f64>()
                    / swinging
            } else {
                0.0
            };
            Vibrato {
                settles: each(&|v| v.settles),
                rate,
                depth: each(&|v| v.depth),
            }
        });
        voice.vibrato = vibrato;
        // Each harmonic is one group across the recordings, and so is each run of partials between harmonics within 1% of one another, one from each recording, so two nearly equal sines do not beat.
        // Each recording's partials are in order, so taking the least of the next of each, the first recording's on a tie, takes them all in order.
        let mut next = [0; 4];
        let mut group = [(&NO_PARTIAL, 0.0); 4];
        let mut members = [0; 4];
        let mut size = 0;
        loop {
            let taken = (0..count)
                .filter(|i| next[*i] < nearest[*i].0.partials.len())
                .min_by(|a, b| {
                    let (x, y) = (
                        &nearest[*a].0.partials[next[*a]],
                        &nearest[*b].0.partials[next[*b]],
                    );
                    (x.harmonic, x.ratio)
                        .partial_cmp(&(y.harmonic, y.ratio))
                        .unwrap()
                });
            let Some(from) = taken else {
                break;
            };
            let partial = &nearest[from].0.partials[next[from]];
            next[from] += 1;
            let weight = nearest[from].1;
            let joins = size > 0 && {
                let (first, _) = group[0];
                let close = if partial.harmonic > 0 {
                    first.harmonic == partial.harmonic
                } else {
                    first.harmonic == 0 && partial.ratio - first.ratio <= 0.01 * first.ratio
                };
                close && !members[..size].contains(&from)
            };
            if !joins && size > 0 {
                voice.held(&group[..size], pitch);
                size = 0;
            }
            group[size] = (partial, weight);
            members[size] = from;
            size += 1;
        }
        if size > 0 {
            voice.held(&group[..size], pitch);
        }
        for (k, band) in self.bands.iter().enumerate() {
            let mut shape = voice.spare_shape();
            shape.levels.extend((0..self.times.len()).map(|t| {
                nearest
                    .iter()
                    .map(|(r, w)| r.noise[k].levels[t] * w)
                    .sum::<f64>()
            }));
            shape.tail = nearest.iter().map(|(r, w)| r.noise[k].tail * w).sum();
            voice.noise.push((*band, shape));
        }
    }
}

/// The values either side of `at` among `count` values that rise, with their weights in a straight line between them: one value when `at` is past either end or on a value.
fn around(count: usize, value: impl Fn(usize) -> f64, at: f64) -> ([(usize, f64); 2], usize) {
    match (0..count).position(|i| value(i) >= at) {
        None => ([(count - 1, 1.0), (0, 0.0)], 1),
        Some(0) => ([(0, 1.0), (0, 0.0)], 1),
        Some(i) if value(i) == at => ([(i, 1.0), (0, 0.0)], 1),
        Some(i) => {
            let share = (at - value(i - 1)) / (value(i) - value(i - 1));
            ([(i - 1, 1.0 - share), (i, share)], 2)
        }
    }
}

type Fill = dyn Fn(f64, f64, &mut Voice) + Send + Sync;
type Gain = dyn Fn(f64, f64, &Voice, &mut Voice) -> f64 + Send + Sync;

/// How a struck instrument plays a note and vel: how it fills a voice, and the gain it plays the filled voice at, with a second voice to work in.
/// The two are apart so the engine can learn how long a note lasts from the fill alone.
#[derive(Clone)]
pub(super) struct Recipe {
    fill: Arc<Fill>,
    gain: Arc<Gain>,
}

impl Recipe {
    pub(super) fn new(
        fill: impl Fn(f64, f64, &mut Voice) + Send + Sync + 'static,
        gain: impl Fn(f64, f64, &Voice, &mut Voice) -> f64 + Send + Sync + 'static,
    ) -> Recipe {
        Recipe {
            fill: Arc::new(fill),
            gain: Arc::new(gain),
        }
    }

    /// Fills `voice` with a note at vel and gives the gain it plays at.
    pub(super) fn play(&self, note: f64, vel: f64, voice: &mut Voice, working: &mut Voice) -> f64 {
        (self.fill)(note, vel, voice);
        (self.gain)(note, vel, voice, working)
    }
}

/// A struck note from `figures`, played by `recipe` at the note's note and vel.
/// Built ready, it keeps room for any note of the figures, and fills itself again with the note and vel the engine gives it, as it starts.
pub(super) fn strike(
    figures: &'static Figures,
    recipe: Recipe,
    o: &Options,
    ctx: &mut Context,
) -> Note {
    strike_among(vec![(figures, recipe)], 0, o, ctx)
}

/// A struck note played by the `chosen` of `choices`, each figures with their recipe, following bend and brightness while it sounds.
/// Built ready, it keeps room for any note of any of them, and fills itself again from the one the pedal it is given picks, the first at 0 and the last at 1.
pub(super) fn strike_among(
    choices: Vec<(&'static Figures, Recipe)>,
    chosen: usize,
    o: &Options,
    ctx: &mut Context,
) -> Note {
    let (note, vel) = (o.number("note"), o.number("vel"));
    let recipe = &choices[chosen].1;
    let expressions = Some(o.follow("bend") | o.follow("brightness"));
    if !ctx.asks_ready() {
        let (mut voice, mut working) = (Voice::empty(), Voice::empty());
        let gain = recipe.play(note, vel, &mut voice, &mut working);
        let end = voice.silent_after();
        let sound = voice.struck(1.0, None, expressions) * gain as f32;
        return Note::envelope(sound, o).within(end);
    }
    let all: Vec<&Figures> = choices.iter().map(|(figures, _)| *figures).collect();
    let (mut voice, mut working) = (Figures::room_among(&all), Figures::room_among(&all));
    let gain = recipe.play(note, vel, &mut voice, &mut working);
    let end = voice.silent_after();
    let (envelope, release) = (Note::envelope_seconds(o), o.number("release"));
    let fills: Vec<Arc<Fill>> = choices.iter().map(|(_, r)| r.fill.clone()).collect();
    let room = Mutex::new(Figures::room_among(&all));
    let hold = Ready::new(move |given: Given| {
        let mut voice = room.lock().unwrap_or_else(|e| e.into_inner());
        fills[picked(given.pedal, fills.len())](given.note, given.vel, &mut voice);
        let fades = voice.silent_after();
        let lasts = if given.held {
            fades
        } else {
            envelope.min(fades)
        };
        (lasts, release)
    });
    ctx.built_ready(hold.clone());
    let refill = Refill {
        hold,
        recipes: choices.into_iter().map(|(_, recipe)| recipe).collect(),
        working,
    };
    Note::envelope(voice.struck(gain, Some(refill), expressions), o).within(end)
}

/// Which of `count` choices a pedal of 0 or 1 picks.
fn picked(pedal: f64, count: usize) -> usize {
    Ord::min(pedal as usize, count - 1)
}

/// A note played from figures fitted to recordings.
/// The recordings are each brought to the same peak, so a velocity carries the energy vel 1 has over the first half second, and plays louder by vel alone, as vel to the power `growth`.
pub(super) fn fitted(
    figures: &'static Figures,
    level: f64,
    growth: f64,
    o: &Options,
    ctx: &mut Context,
) -> Note {
    let recipe = Recipe::new(
        move |note, vel, voice| figures.fill(voice, note, vel.clamp(0.0, 1.0)),
        move |note, vel, voice, loudest| {
            figures.fill(loudest, note, 1.0);
            let loudness = (loudest.energy() / voice.energy()).sqrt();
            (level * vel.clamp(0.0, 1.0).powf(growth) * loudness).max(1e-9)
        },
    );
    strike(figures, recipe, o, ctx)
}

/// A note played from figures fitted to recordings that keep their levels from one velocity to the next, so the figures carry its loudness.
pub(super) fn kept(figures: &'static Figures, level: f64, o: &Options, ctx: &mut Context) -> Note {
    let recipe = Recipe::new(
        move |note, vel, voice| figures.fill(voice, note, vel.clamp(0.0, 1.0)),
        move |note, vel, _, _| kept_gain(figures, level, note, vel.clamp(0.0, 1.0)),
    );
    strike(figures, recipe, o, ctx)
}

/// The gain of a note played as `kept` plays it: below the softest velocity recorded, a note is quieter in proportion to vel.
pub(super) fn kept_gain(figures: &Figures, level: f64, _note: f64, vel: f64) -> f64 {
    (level * (vel / figures.softest()).min(1.0)).max(1e-9)
}

/// A note played from figures fitted to recordings that keep their levels from one velocity to the next, but not from one note to the next.
/// Every note at vel 1 carries the same energy over the first half second, its noise's with its partials', and a softer velocity is as much quieter as the recordings at that note.
pub(super) fn evened(
    figures: &'static Figures,
    level: f64,
    o: &Options,
    ctx: &mut Context,
) -> Note {
    let recipe = Recipe::new(
        move |note, vel, voice| figures.fill(voice, note, vel.clamp(0.0, 1.0)),
        move |note, vel, _, loudest| {
            figures.fill(loudest, note, 1.0);
            let even = figures.whole_energy(loudest).sqrt();
            (level * (vel.clamp(0.0, 1.0) / figures.softest()).min(1.0) / even).max(1e-9)
        },
    );
    strike(figures, recipe, o, ctx)
}

impl Shape {
    fn at(&self, times: &[f64], t: f64) -> f64 {
        let last = times.len() - 1;
        if t >= times[last] {
            return self.levels[last] + self.tail * (t - times[last]);
        }
        let at = Ord::max(times.partition_point(|time| *time <= t), 1) - 1;
        let share = (t - times[at]) / (times[at + 1] - times[at]);
        self.levels[at] + share * (self.levels[at + 1] - self.levels[at])
    }

    // The power in straight lines between the times, as the noise's level runs.
    fn power_at(&self, times: &[f64], t: f64) -> f64 {
        let last = times.len() - 1;
        if t >= times[last] {
            return 10f64.powf((self.levels[last] + self.tail * (t - times[last])) / 10.0);
        }
        let at = Ord::max(times.partition_point(|time| *time <= t), 1) - 1;
        let share = (t - times[at]) / (times[at + 1] - times[at]);
        let power = |level: f64| 10f64.powf(level / 10.0);
        (1.0 - share) * power(self.levels[at]) + share * power(self.levels[at + 1])
    }
}

impl Voice {
    /// A voice with no partials or bands and no room kept, which `Figures::fill` grows as it fills it.
    pub fn empty() -> Voice {
        Voice {
            times: vec![],
            partials: vec![],
            noise: vec![],
            swings: vec![],
            vibrato: None,
            spare: vec![],
        }
    }

    // Empties it for another note, keeping its shapes for that note's partials and bands.
    fn clear(&mut self) {
        self.times.clear();
        self.spare
            .extend(self.partials.drain(..).map(|(_, shape)| shape));
        self.spare
            .extend(self.noise.drain(..).map(|(_, shape)| shape));
        self.swings.clear();
    }

    // A kept shape emptied, or a new one when none is kept.
    fn spare_shape(&mut self) -> Shape {
        let mut shape = self.spare.pop().unwrap_or_else(|| Shape {
            levels: Vec::with_capacity(self.times.len()),
            tail: 0.0,
        });
        shape.levels.clear();
        shape
    }

    // Adds the partial a group of partials, one from each recording with its weight, plays.
    fn held(&mut self, held: &[(&Partial, f64)], pitch: f64) {
        let total: f64 = held.iter().map(|(_, w)| w).sum();
        self.swings
            .push(held.iter().map(|(p, w)| p.swing * w).sum::<f64>() / total);
        let ratio = held.iter().map(|(p, w)| p.ratio * w).sum::<f64>() / total;
        let quieter = 20.0 * total.log10();
        let mut shape = self.spare_shape();
        shape.levels.extend((0..self.times.len()).map(|t| {
            held.iter().map(|(p, w)| p.shape.levels[t] * w).sum::<f64>() / total + quieter
        }));
        shape.tail = held.iter().map(|(p, w)| p.shape.tail * w).sum::<f64>() / total;
        self.partials.push((pitch * ratio, shape));
    }

    /// Each partial's fade bent from the straight line between its first and last level: 0 makes it one straight line, 1 keeps it, and more deepens the bend.
    pub fn bend(&mut self, by: f64) {
        let (first, last) = (self.times[0], *self.times.last().unwrap());
        for (_, shape) in &mut self.partials {
            let (start, end) = (shape.levels[0], *shape.levels.last().unwrap());
            for (level, t) in shape.levels.iter_mut().zip(&self.times) {
                let line = start + (end - start) * (t - first) / (last - first);
                *level = line + by * (*level - line);
            }
        }
    }

    /// The noise in the bands whose centre is under `hz` made `by` times as loud; 0 leaves it out.
    pub fn noise_under(&mut self, hz: f64, by: f64) {
        for ((low, high), shape) in &mut self.noise {
            if (*low * *high).sqrt() <= hz {
                shape.levels.iter_mut().for_each(|level| {
                    *level = if by > 0.0 {
                        (*level + 20.0 * by.log10()).max(SILENT)
                    } else {
                        SILENT
                    }
                });
            }
        }
    }

    /// Each partial louder or quieter by `by` of its frequency, in dB.
    pub fn vary(&mut self, by: impl Fn(f64) -> f64) {
        for (hz, shape) in &mut self.partials {
            let change = by(*hz);
            shape.levels.iter_mut().for_each(|level| *level += change);
        }
    }

    /// Each partial as its frequency, its loudest level in dB, and the seconds it takes to fall 60 dB from it.
    pub fn partials(&self) -> Vec<(f64, f64, f64)> {
        self.partials
            .iter()
            .map(|(hz, shape)| {
                let (loudest, fade) = self.fall(shape);
                (*hz, loudest, fade)
            })
            .collect()
    }

    // A shape's loudest level, and the seconds it takes to fall 60 dB from it.
    fn fall(&self, shape: &Shape) -> (f64, f64) {
        let (at, loudest) =
            shape
                .levels
                .iter()
                .enumerate()
                .fold(
                    (0, f64::MIN),
                    |(i, m), (j, l)| if *l > m { (j, *l) } else { (i, m) },
                );
        let last = self.times.len() - 1;
        let fallen = (at..=last)
            .find(|j| shape.levels[*j] <= loudest - 60.0)
            .map(|j| self.times[j])
            .unwrap_or_else(|| {
                let left = 60.0 - (loudest - shape.levels[last]);
                self.times[last]
                    + if shape.tail < 0.0 {
                        left / -shape.tail
                    } else {
                        60.0
                    }
            });
        (loudest, fallen - self.times[at])
    }

    /// Each band of noise as its edges in Hz, its loudest power in dB, and the seconds it takes to fall 60 dB from it.
    pub fn noise(&self) -> Vec<((f64, f64), f64, f64)> {
        self.noise
            .iter()
            .map(|(band, shape)| {
                let (loudest, fade) = self.fall(shape);
                (*band, loudest, fade)
            })
            .collect()
    }

    /// The energy of the partials over the first half second, which sets how loud a velocity plays.
    pub fn energy(&self) -> f64 {
        let steps = 100;
        self.partials
            .iter()
            .map(|(_, shape)| {
                (0..steps)
                    .map(|i| {
                        10f64.powf(
                            shape.at(&self.times, (i as f64 + 0.5) * 0.5 / steps as f64) / 10.0,
                        )
                    })
                    .sum::<f64>()
            })
            .sum::<f64>()
            * 0.5
            / steps as f64
    }

    /// The seconds until every partial falls 100 dB under the loudest, where the recordings reach their noise floor.
    pub fn silent_after(&self) -> f64 {
        let loudest = self
            .partials
            .iter()
            .flat_map(|(_, shape)| shape.levels.iter())
            .cloned()
            .fold(f64::MIN, f64::max);
        let under = loudest - 100.0;
        let last = self.times.len() - 1;
        self.partials
            .iter()
            .map(|(_, shape)| {
                let end = shape.levels[last];
                if end >= under {
                    return if shape.tail < 0.0 {
                        self.times[last] + (end - under) / -shape.tail
                    } else {
                        600.0
                    };
                }
                let loud = shape.levels.iter().rposition(|l| *l >= under);
                loud.map_or(0.0, |i| self.times[Ord::min(i + 1, last)])
            })
            .fold(0.1, f64::max)
    }

    /// The voice as a sound, each partial a sine and each band of noise its filters' output.
    pub fn sound(self) -> Net {
        self.struck(1.0, None, None)
    }

    /// The voice as a sound that follows the bend and brightness of the note's options while it sounds.
    pub(super) fn following(self, o: &Options) -> Net {
        self.struck(1.0, None, Some(o.follow("bend") | o.follow("brightness")))
    }

    // The voice as a sound at `gain`, which fills itself again as it starts when given a refill, and follows the bend and brightness of `expressions` when given them.
    fn struck(self, gain: f64, refill: Option<Refill>, expressions: Option<Net>) -> Net {
        let mut node = Struck {
            voice: self,
            rate: DEFAULT_SR,
            sines: vec![],
            bands: vec![],
            warm: vec![],
            sample: 0,
            gain: gain as f32,
            refill,
            wanted: (0.0, 0.5),
            applied: (0.0, 0.5),
            lowest: 0.0,
        };
        node.set_sample_rate(DEFAULT_SR);
        let plain = || Net::wrap(Box::new(dc((0.0, 0.5))));
        expressions.unwrap_or_else(plain) >> Net::wrap(Box::new(An(node)))
    }
}

/// What a voice built ready fills itself again with: the hold the engine gives its note through, its instrument's recipes by pedal, and a voice for the recipe to work in.
#[derive(Clone)]
struct Refill {
    hold: Arc<Ready>,
    recipes: Vec<Recipe>,
    working: Voice,
}

// A partial as a phasor turned a step every sample, with its amplitude gliding to the next reading.
#[derive(Clone)]
struct Sine {
    shape: usize,
    phasor: (f64, f64),
    turn: (f64, f64),
    amplitude: f64,
    step: f64,
    done: bool,
    drift: f64,
}

#[derive(Clone)]
struct Band {
    shape: usize,
    filter: [[f64; 5]; 4],
    state: [[f64; 4]; 4],
    noise: u64,
    rise: f64,
    /// The gain that moves the figures' noise from `NOISE_RATE` to the rate it plays at.
    scale: f64,
    amplitude: f64,
    step: f64,
    done: bool,
}

impl Band {
    fn white(&mut self) -> f64 {
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 7;
        self.noise ^= self.noise << 17;
        (self.noise >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }

    fn filtered(&mut self) -> f64 {
        let mut x = self.white();
        for (s, [b0, b1, b2, a1, a2]) in self.state.iter_mut().zip(&self.filter) {
            let y = b0 * x + b1 * s[0] + b2 * s[1] - a1 * s[2] - a2 * s[3];
            *s = [x, s[0], y, s[2]];
            x = y;
        }
        x
    }
}

/// The sample rate the figures' noise gains are written for, as mx writes them.
const NOISE_RATE: f64 = 48_000.0;

/// The gain that gives a band's noise at `rate` the power its written gain gives at `NOISE_RATE`, as white noise through the same filters carries less power the higher the rate.
fn noise_scale(low: f64, high: f64, rate: f64) -> f64 {
    if high >= rate / 2.0 || high >= NOISE_RATE / 2.0 {
        return 1.0;
    }
    (response_energy(band_filter(low, high, NOISE_RATE))
        / response_energy(band_filter(low, high, rate)))
    .sqrt()
}

/// A band's noise gain and its filters once warmed, by its edges, its place among the bands and the rate, which every note of every instrument with that band shares.
static WARMED: LazyLock<Mutex<HashMap<(u64, u64, usize, u64), Band>>> =
    LazyLock::new(Default::default);

// The band `k` of a voice, its filters warmed on noise so it starts as a recording's would; the
// first voice at a rate works it out and every later one takes it.
fn warmed(k: usize, low: f64, high: f64, rate: f64) -> Band {
    let key = (low.to_bits(), high.to_bits(), k, rate.to_bits());
    if let Some(band) = WARMED.lock().unwrap().get(&key) {
        return band.clone();
    }
    let mut band = Band {
        shape: k,
        filter: band_filter(low, high, rate),
        state: [[0.0; 4]; 4],
        noise: 0x9e37_79b9_7f4a_7c15 ^ (k as u64 + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9),
        rise: (1.0 / (high - low)).clamp(0.002, 0.02),
        scale: noise_scale(low, high, rate),
        amplitude: 0.0,
        step: 0.0,
        done: false,
    };
    for _ in 0..(WARM * rate) as usize {
        band.filtered();
    }
    WARMED.lock().unwrap().insert(key, band.clone());
    band
}

/// The sum of the squares of the filters' response to an impulse.
fn response_energy(filter: [[f64; 5]; 4]) -> f64 {
    let mut state = [[0.0; 4]; 4];
    (0..8192)
        .map(|i| {
            let mut x = if i == 0 { 1.0 } else { 0.0 };
            for (s, [b0, b1, b2, a1, a2]) in state.iter_mut().zip(&filter) {
                let y = b0 * x + b1 * s[0] + b2 * s[1] - a1 * s[2] - a2 * s[3];
                *s = [x, s[0], y, s[2]];
                x = y;
            }
            x * x
        })
        .sum()
}

/// Four band-pass sections with a peak gain of 1 at the band's centre and its width as their bandwidth, as `mix mx.fit` fits the noise through.
fn band_filter(low: f64, high: f64, rate: f64) -> [[f64; 5]; 4] {
    let centre = (low * high).sqrt();
    let omega = 2.0 * std::f64::consts::PI * centre / rate;
    let alpha = omega.sin() * (high - low) / (2.0 * centre);
    let a0 = 1.0 + alpha;
    [[
        alpha / a0,
        0.0,
        -alpha / a0,
        -2.0 * omega.cos() / a0,
        (1.0 - alpha) / a0,
    ]; 4]
}

/// Every band of a voice below half the rate, warmed.
fn warm_bands(voice: &Voice, rate: f64) -> Vec<Band> {
    voice
        .noise
        .iter()
        .enumerate()
        .filter(|(_, ((_, high), _))| *high < rate / 2.0)
        .map(|(k, ((low, high), _))| warmed(k, *low, *high, rate))
        .collect()
}

/// Fills `sines` with a sine at rest for each partial of the voice below half the rate that sounds, within the room they have.
fn fill_sines(voice: &Voice, rate: f64, sines: &mut Vec<Sine>) {
    sines.clear();
    sines.extend(
        voice
            .partials
            .iter()
            .enumerate()
            .filter(|(_, (freq, shape))| {
                *freq < rate / 2.0 && shape.levels.iter().any(|l| *l > -140.0)
            })
            .map(|(i, (freq, _))| {
                let omega = 2.0 * std::f64::consts::PI * freq / rate;
                Sine {
                    shape: i,
                    phasor: (1.0, 0.0),
                    turn: (omega.cos(), omega.sin()),
                    amplitude: 0.0,
                    step: 0.0,
                    done: false,
                    drift: 0.0,
                }
            }),
    );
}

/// Fills `bands` with the warmed bands where the voice has noise, within the room they have.
fn fill_bands(voice: &Voice, warm: &[Band], bands: &mut Vec<Band>) {
    bands.clear();
    bands.extend(
        warm.iter()
            .filter(|band| voice.noise[band.shape].1.levels.iter().any(|l| *l > SILENT))
            .cloned(),
    );
}

#[derive(Clone)]
struct Struck {
    voice: Voice,
    rate: f64,
    sines: Vec<Sine>,
    bands: Vec<Band>,
    /// Every band below half the rate, warmed, which a note takes the bands it sounds from.
    warm: Vec<Band>,
    sample: usize,
    gain: f32,
    refill: Option<Refill>,
    /// The bend and brightness at its input, and those its sines play at.
    wanted: (f64, f64),
    applied: (f64, f64),
    /// The lowest partial's frequency, which a brightness tilts the others' levels from.
    lowest: f64,
}

impl Struck {
    // Starts the note the voice holds from its first sample, within the room set aside.
    fn start(&mut self) {
        self.sample = 0;
        fill_sines(&self.voice, self.rate, &mut self.sines);
        fill_bands(&self.voice, &self.warm, &mut self.bands);
        self.applied.0 = 0.0;
        self.lowest = self.sines.iter().fold(f64::INFINITY, |low, sine| {
            low.min(self.voice.partials[sine.shape].0)
        });
        self.read();
    }

    // Turns each sine at its partial's frequency moved by the bend wanted, when it moved.
    fn bend(&mut self) {
        if self.wanted.0 != self.applied.0 {
            let ratio = 2f64.powf(self.wanted.0 / 12.0);
            for sine in &mut self.sines {
                let freq = self.voice.partials[sine.shape].0 * ratio;
                let omega = 2.0 * std::f64::consts::PI * freq / self.rate;
                sine.turn = (omega.cos(), omega.sin());
            }
            self.applied.0 = self.wanted.0;
        }
        self.applied.1 = self.wanted.1;
    }

    // Fills the voice with the note and vel the engine gave it, when it gave one.
    fn refill(&mut self) {
        let Some(refill) = &mut self.refill else {
            return;
        };
        let Some(given) = refill.hold.given() else {
            return;
        };
        let recipe = &refill.recipes[picked(given.pedal, refill.recipes.len())];
        self.gain = recipe.play(given.note, given.vel, &mut self.voice, &mut refill.working) as f32;
        self.start();
    }

    // The levels at the next reading, BLOCK samples on, for each sine and band to glide to.
    fn read(&mut self) {
        self.bend();
        let t = (self.sample + BLOCK) as f64 / self.rate;
        let last = *self.voice.times.last().unwrap();
        // A brightness away from 0.5 tilts the levels by up to 3 dB an octave either way.
        let tilt = (self.applied.1 != 0.5).then_some((self.applied.1 - 0.5) * 6.0);
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            let (freq, shape) = &self.voice.partials[sine.shape];
            let mut level = shape.at(&self.voice.times, t);
            if let Some(tilt) = tilt {
                level += tilt * (freq / self.lowest).log2();
            }
            let target = (t / RISE).min(1.0) * 10f64.powf(level / 20.0);
            sine.step = (target - sine.amplitude) / BLOCK as f64;
            if level < -140.0 && t > last {
                sine.done = true;
            }
            // Renormalised now and then, so rounding does not grow or shrink it.
            let size = (sine.phasor.0 * sine.phasor.0 + sine.phasor.1 * sine.phasor.1).sqrt();
            sine.phasor = (sine.phasor.0 / size, sine.phasor.1 / size);
        }
        for band in self.bands.iter_mut().filter(|b| !b.done) {
            let power = self.voice.noise[band.shape]
                .1
                .power_at(&self.voice.times, t);
            let rising = if t >= band.rise {
                1.0
            } else {
                0.5 - 0.5 * (std::f64::consts::PI * t / band.rise).cos()
            };
            let target = rising * power.sqrt() * band.scale;
            band.step = (target - band.amplitude) / BLOCK as f64;
            if power < 1e-14 && t > last {
                band.done = true;
            }
        }
    }
}

impl AudioNode for Struck {
    const ID: u64 = 0x6d78_0070;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.warm = warm_bands(&self.voice, sample_rate);
        self.sines = Vec::with_capacity(self.voice.partials.capacity());
        self.bands = Vec::with_capacity(self.warm.len());
        self.start();
    }

    fn reset(&mut self) {
        self.set_sample_rate(self.rate);
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        self.wanted = (input[0] as f64, input[1] as f64);
        if self.sample == 0 && self.refill.is_some() {
            self.refill();
        }
        if self.sample == 0 && self.wanted != self.applied {
            self.read();
        }
        if self.sample > 0 && self.sample.is_multiple_of(BLOCK) {
            self.read();
        }
        let mut sum = 0.0;
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            sum += sine.amplitude * sine.phasor.1;
            let (re, im) = sine.phasor;
            sine.phasor = (
                re * sine.turn.0 - im * sine.turn.1,
                re * sine.turn.1 + im * sine.turn.0,
            );
            sine.amplitude += sine.step;
        }
        for band in self.bands.iter_mut().filter(|b| !b.done) {
            sum += band.amplitude * band.filtered();
            band.amplitude += band.step;
        }
        self.sample += 1;
        [sum as f32 * self.gain].into()
    }
}

/// A held note played from figures fitted to recordings of held notes: it holds until `hold` seconds or until its gate lets it go, then each partial and band falls at its slope.
/// Its pitch follows `note` as it slides, and its vibrato, `vibrato` when given in place of the recordings', fades in until the note settles.
/// With `legato` at 1 it speaks at once, from where the recordings settle.
/// With a `breath`, the note is played as a wind player plays it, see `Breath`.
/// Its gain at a vel is `gain`.
/// Built ready, it keeps room for any note of the figures, and starts again from the note and vel the engine gives it.
pub(super) fn held_note(
    figures: &'static Figures,
    gain: impl Fn(f64) -> f64 + Send + Sync + 'static,
    vibrato: Option<f64>,
    legato: f64,
    breath: Option<Breath>,
    expressed: bool,
    o: &Options,
    ctx: &mut Context,
) -> Note {
    let note = o.number("note");
    let vel = effort(o).clamp(0.0, 1.0);
    let ready = ctx.asks_ready();
    let mut voice = if ready {
        figures.room()
    } else {
        Voice::empty()
    };
    figures.fill(&mut voice, note, vel);
    vibrate(&mut voice, vibrato);
    let skip = legato.clamp(0.0, 1.0) * voice.vibrato.map_or(0.0, |v| v.settles);
    let hold = if o.held() {
        f64::INFINITY
    } else {
        o.number("attack") + o.number("decay") + o.number("sustain")
    };
    let falls = if breath.is_some() { 5.0 } else { voice.falls() };
    let under = voice.loudest() - 100.0;
    let gate = shared(1.0);
    let level = gain(vel);
    let node = Sustained {
        voice,
        falls,
        under,
        base: note,
        hold,
        gate: gate.clone(),
        rate: DEFAULT_SR,
        sines: vec![],
        bands: vec![],
        sample: 0,
        let_go: None,
        phase: 0.0,
        skip,
        figures,
        vel,
        vibrato,
        breath,
        origin: 0.0,
        gliding: 0.0,
        caught: f64::NEG_INFINITY,
        swelled: 0.0,
        blown: vel,
        dipped: false,
        last_vel: vel,
        deepen: 1.0,
        slur: 0.0,
        slurred: f64::NEG_INFINITY,
        slurring: BEND,
        expressed: if expressed {
            (o.number("bend"), o.number("brightness"))
        } else {
            (0.0, 0.5)
        },
        loud: 1.0,
        sounding: 1.0,
        register: breath.map_or(1.0, |b| {
            register(figures, note, b.even, &mut Voice::empty())
        }),
        pace: 1.0,
        inhale: vec![],
        inhaled: None,
        fading: vec![],
        warm: vec![],
        gain: 1.0,
        refill: None,
        rates: ([0.0; 8], 0),
    };
    let mut node = node;
    node.deepen = node.vary();
    let expressions = if expressed {
        o.follow("bend") | o.follow("brightness")
    } else {
        Net::wrap(Box::new(dc((0.0, 0.5))))
    };
    let notes = o.follow("note") | followed_effort(o) | expressions;
    if !ready {
        node.set_sample_rate(DEFAULT_SR);
        return Note {
            sound: notes >> (Net::wrap(Box::new(An(node))) * level as f32),
            end: hold + falls,
            release: Some((gate, falls)),
            held: None,
        };
    }
    let room = Mutex::new(figures.room());
    let hold_for = Ready::new(move |given: Given| {
        let mut voice = room.lock().unwrap_or_else(|e| e.into_inner());
        let vel = pressed(given.vel, given.pressure).clamp(0.0, 1.0);
        figures.fill(&mut voice, given.note, vel);
        let falls = if breath.is_some() { 5.0 } else { voice.falls() };
        let hold = if given.held { f64::INFINITY } else { hold };
        (hold + falls, falls)
    });
    ctx.built_ready(hold_for.clone());
    node.gain = level as f32;
    node.refill = Some(HeldRefill {
        hold: hold_for,
        plain: hold,
        gain: Arc::new(gain),
        legato,
        breath,
        working: figures.room(),
    });
    node.set_sample_rate(DEFAULT_SR);
    Note {
        sound: notes >> Net::wrap(Box::new(An(node))),
        end: hold + falls,
        release: Some((gate, falls)),
        held: None,
    }
}

/// What a held voice built ready starts again with: the hold the engine gives its note and vel through, its gain at a vel, its legato, its breath as it was built, and a voice to work in.
#[derive(Clone)]
struct HeldRefill {
    hold: Arc<Ready>,
    /// The seconds a note not held is held for before it falls.
    plain: f64,
    gain: Arc<dyn Fn(f64) -> f64 + Send + Sync>,
    legato: f64,
    breath: Option<Breath>,
    working: Voice,
}

/// The voice with `vibrato` in semitones in place of the recordings'.
fn vibrated(mut voice: Voice, vibrato: Option<f64>) -> Voice {
    vibrate(&mut voice, vibrato);
    voice
}

/// Gives the voice `vibrato` in semitones in place of the recordings'.
fn vibrate(voice: &mut Voice, vibrato: Option<f64>) {
    if let Some(depth) = vibrato {
        // Recordings played without vibrato give it a player's: 5.4 Hz, full a second in.
        let recorded = voice.vibrato.filter(|v| v.rate > 0.0).unwrap_or(Vibrato {
            settles: 1.0,
            rate: 5.4,
            depth: 0.0,
        });
        voice.vibrato = Some(Vibrato {
            depth: depth * 100.0,
            ..recorded
        });
    }
}

impl Voice {
    // The time in its attack at which the fundamental first stands within 10 dB of its last level.
    fn tongued(&self) -> f64 {
        let Some((_, shape)) = self.partials.first() else {
            return 0.0;
        };
        let held = shape.levels[shape.levels.len() - 1];
        let at = shape
            .levels
            .iter()
            .position(|l| *l >= held - 10.0)
            .unwrap_or(0);
        self.times[at]
    }

    pub(super) fn loudest(&self) -> f64 {
        self.partials
            .iter()
            .flat_map(|(_, s)| s.levels.iter())
            .cloned()
            .fold(f64::MIN, f64::max)
    }

    /// The seconds every partial and band takes, once let go from its last level, to fall 100 dB under the loudest partial, at most 5.
    fn falls(&self) -> f64 {
        let under = self.loudest() - 100.0;
        self.partials
            .iter()
            .map(|(_, s)| s)
            .chain(self.noise.iter().map(|(_, s)| s))
            .map(|s| {
                let from = s.levels.iter().cloned().fold(f64::MIN, f64::max);
                if from <= under || s.tail >= 0.0 {
                    0.0
                } else {
                    (from - under) / -s.tail
                }
            })
            .fold(0.05, f64::max)
            .min(5.0)
    }
}

/// How a wind player plays held figures: a change of note slurs to the new note's figures from where they settle, with a catch as the breath settles, a note reached through a dip in vel is tongued and plays its recorded attack, the figures follow vel as it swells or eases, and each partial's level wanders slowly as a breath does.
#[derive(Clone, Copy)]
pub(super) struct Breath {
    /// How far the fundamental's level wanders, in dB.
    pub wander: f64,
    /// How far the other partials' levels wander, in dB.
    pub wander_above: f64,
    /// How far the partials dip in a slur's catch, in dB.
    pub catch: f64,
    /// How far the vibrato's depth differs from note to note, as the spread of the log of its factor.
    pub vary: f64,
    /// Whether a breath is taken once the note is let go.
    pub inhale: bool,
    /// How much of the recordings' rise in loudness up the range a player evens out, from 0 to 1.
    pub even: f64,
    pub seed: u64,
}

/// The gain that takes `even` of the way from a note's held loudness at vel 1 to that of the middle recorded note.
/// It works the loudness out in `working`.
fn register(figures: &Figures, note: f64, even: f64, working: &mut Voice) -> f64 {
    let mut held = |note: f64| {
        figures.fill(working, note, 1.0);
        let power: f64 = working
            .partials
            .iter()
            .map(|(_, s)| 10f64.powf(s.levels[s.levels.len() - 1] / 10.0))
            .sum();
        10.0 * power.max(1e-30).log10()
    };
    10f64.powf(-even * (held(note) - held(figures.middle)) / 20.0)
}

/// Where a note's figures start again from.
#[derive(Clone, Copy, PartialEq)]
enum From {
    /// Where the recordings settle, for a slur.
    Settled,
    /// Where their attack stands 10 dB under the held level, for a note tongued on a breath that keeps flowing, as a played Syrinx's tongued notes dip 8 dB at the median.
    Attack,
    /// The same time, for a change of vel.
    Same,
}

#[derive(Clone)]
struct Sustained {
    voice: Voice,
    falls: f64,
    under: f64,
    base: f64,
    hold: f64,
    gate: Shared,
    rate: f64,
    sines: Vec<Sine>,
    bands: Vec<Band>,
    sample: usize,
    let_go: Option<f64>,
    phase: f64,
    skip: f64,
    figures: &'static Figures,
    vel: f64,
    vibrato: Option<f64>,
    breath: Option<Breath>,
    origin: f64,
    gliding: f64,
    caught: f64,
    swelled: f64,
    blown: f64,
    dipped: bool,
    last_vel: f64,
    deepen: f64,
    slur: f64,
    slurred: f64,
    slurring: f64,
    /// The bend and brightness at its input.
    expressed: (f64, f64),
    loud: f64,
    sounding: f64,
    register: f64,
    pace: f64,
    inhale: Vec<(Band, f64)>,
    inhaled: Option<f64>,
    fading: Vec<Sine>,
    /// Every band below half the rate, warmed, which a note takes the bands it sounds from.
    warm: Vec<Band>,
    gain: f32,
    refill: Option<HeldRefill>,
    /// The first rates it was set to, and how many times it was set.
    rates: ([f64; 8], usize),
}

impl Sustained {
    // Starts the note the voice holds again from its first sample, within the room set aside.
    fn restart(&mut self) {
        fill_sines(&self.voice, self.rate, &mut self.sines);
        fill_bands(&self.voice, &self.warm, &mut self.bands);
        self.sample = 0;
        self.let_go = None;
        self.phase = 0.0;
        self.origin = 0.0;
        self.gliding = 0.0;
        self.fading.clear();
        self.read(self.base, self.vel);
    }

    // Starts again from the note and vel the engine gave it, when it gave one, as `held_note` builds a note.
    fn refill(&mut self) {
        let Some(refill) = &mut self.refill else {
            return;
        };
        let Some(given) = refill.hold.given() else {
            return;
        };
        let (note, vel) = (
            given.note,
            pressed(given.vel, given.pressure).clamp(0.0, 1.0),
        );
        self.hold = if given.held {
            f64::INFINITY
        } else {
            refill.plain
        };
        let breath = refill.breath;
        self.register = breath.map_or(1.0, |b| {
            register(self.figures, note, b.even, &mut refill.working)
        });
        self.gain = (refill.gain)(vel) as f32;
        let legato = refill.legato;
        self.figures.fill(&mut self.voice, note, vel);
        vibrate(&mut self.voice, self.vibrato);
        self.skip = legato.clamp(0.0, 1.0) * self.voice.vibrato.map_or(0.0, |v| v.settles);
        self.falls = if breath.is_some() {
            5.0
        } else {
            self.voice.falls()
        };
        self.under = self.voice.loudest() - 100.0;
        self.base = note;
        self.vel = vel;
        self.blown = vel;
        self.last_vel = vel;
        self.breath = breath;
        self.deepen = self.vary();
        // A note built takes a draw for each of its sines each time its rate is set, so this one takes those of the rates set before the last again.
        let (rates, set) = self.rates;
        for rate in &rates[..Ord::min(set, rates.len()).saturating_sub(1)] {
            let sines = self
                .voice
                .partials
                .iter()
                .filter(|(freq, shape)| {
                    *freq < rate / 2.0 && shape.levels.iter().any(|l| *l > -140.0)
                })
                .count();
            for _ in 0..sines {
                self.uniform();
            }
        }
        self.restart();
    }

    // A held level at `t`: the shape's level until the note is let go, its last level after the last time, and falling at its slope after the letting go.
    // A band's level runs in straight lines of power between the times, as the fit reads it.
    fn level(&self, shape: &Shape, t: f64, power: bool) -> f64 {
        let times = &self.voice.times;
        let at = |t: f64| {
            let last = times.len() - 1;
            if t >= times[last] {
                shape.levels[last]
            } else if power {
                10.0 * shape.power_at(times, t).max(1e-30).log10()
            } else {
                shape.at(times, t)
            }
        };
        match self.let_go {
            Some(go) if t > go => {
                // A slope too slow to reach the floor before the note ends, as `falls` is at most 5 seconds, is steepened to reach it then.
                let from = at((go - self.origin) * self.pace + self.skip);
                let tail = shape.tail.min(-(from - self.under).max(0.0) / self.falls);
                from + tail * (t - go)
            }
            _ => at((t - self.origin) * self.pace + self.skip),
        }
    }

    // Moves to the figures of `note` at `vel`: each sine goes on as the nearest harmonic of the new note, a band as the same band, and the rest fade out, all over the slur.
    fn revoice(&mut self, note: f64, vel: f64, t: f64, from: From) {
        if let Some(breath) = self.breath.filter(|_| from != From::Same) {
            self.register = register(self.figures, note, breath.even, &mut Voice::empty());
        }
        let voice = vibrated(self.figures.voice(note, vel), self.vibrato);
        let (old, new) = (hz(self.base), hz(note));
        let mut sines = std::mem::take(&mut self.sines);
        let mut bands = std::mem::take(&mut self.bands);
        let mut struck = Struck {
            voice: voice.clone(),
            rate: self.rate,
            sines: vec![],
            bands: vec![],
            warm: vec![],
            sample: 0,
            gain: 1.0,
            refill: None,
            wanted: (0.0, 0.5),
            applied: (0.0, 0.5),
            lowest: 0.0,
        };
        struck.set_sample_rate(self.rate);
        for mut sine in struck.sines {
            let harmonic = voice.partials[sine.shape].0 / new;
            let nearest = sines.iter().position(|s| {
                !s.done && (self.voice.partials[s.shape].0 / old / harmonic - 1.0).abs() < 0.03
            });
            if let Some(k) = nearest {
                let was = sines.swap_remove(k);
                sine.phasor = was.phasor;
                sine.amplitude = was.amplitude;
                sine.drift = was.drift;
            } else {
                sine.amplitude = 0.0;
            }
            self.sines.push(sine);
        }
        for mut band in struck.bands {
            let edges = voice.noise[band.shape].0;
            band.rise = 0.0;
            match bands
                .iter()
                .position(|b| !b.done && self.voice.noise[b.shape].0 == edges)
            {
                Some(k) => {
                    let was = bands.swap_remove(k);
                    band.state = was.state;
                    band.noise = was.noise;
                    band.amplitude = was.amplitude;
                }
                None => band.amplitude = 0.0,
            }
            self.bands.push(band);
        }
        let samples = SLUR * self.rate;
        for mut sine in sines.into_iter().filter(|s| !s.done) {
            sine.step = -sine.amplitude / samples;
            self.fading.push(sine);
        }
        // A slurred step of a semitone or two glides, as a played Syrinx's do, while a leap moves within 8 ms, as its leaps take 0 to 7 between the quartiles, since a jump at once clicks; a tongue stops the air between the two.
        if from != From::Same {
            let step = (self.base - note).abs() <= 2.5;
            self.slur = if from == From::Settled || !step {
                self.base - note
            } else {
                0.0
            };
            self.slurred = t;
            let (u, v) = (self.uniform(), self.uniform());
            let drawn = (BEND
                * (0.5
                    * (-2.0 * u.max(1e-12).ln()).sqrt()
                    * (2.0 * std::f64::consts::PI * v).cos())
                .exp())
            .clamp(0.015, 0.04);
            self.slurring = if from == From::Settled && step {
                drawn
            } else {
                LEAP
            };
        }
        match from {
            From::Settled => {
                self.skip = voice.vibrato.map_or(0.0, |v| v.settles);
                self.origin = t;
                self.caught = t;
                self.pace = 1.0;
                self.deepen = self.vary();
            }
            From::Attack => {
                self.skip = voice.tongued();
                self.origin = t;
                self.pace = TONGUE;
                self.deepen = self.vary();
            }
            From::Same => {}
        }
        self.under = voice.loudest() - 100.0;
        self.voice = voice;
        self.base = note;
        self.vel = vel;
        // A tongue stops the air at once, so a tongued note drops to its attack faster than a slur moves.
        self.gliding = t + if from == From::Attack {
            SLUR / 3.0
        } else {
            SLUR
        };
    }

    // Sets up the breath taken after the note, 45 dB under its held level, in its octaves.
    fn take_breath(&mut self, t: f64) {
        let held: f64 = self
            .voice
            .partials
            .iter()
            .map(|(_, s)| 10f64.powf(s.levels[s.levels.len() - 1] / 10.0) / 2.0)
            .sum::<f64>()
            * self.loud
            * self.loud;
        let total = held * 10f64.powf(-INHALE_UNDER / 10.0);
        let whole: f64 = INHALE_OCTAVES
            .iter()
            .map(|(_, _, w)| 10f64.powf(w / 10.0))
            .sum();
        let seed = self.breath.map_or(1, |b| b.seed);
        self.inhale = INHALE_OCTAVES
            .iter()
            .enumerate()
            .filter(|(_, (_, high, _))| *high < self.rate / 2.0)
            .map(|(k, (low, high, w))| {
                let filter = band_filter(*low, *high, self.rate);
                // Uniform noise from -1 to 1 has a power of a third, which the filter scales by its response's energy.
                let power = total * 10f64.powf(w / 10.0) / whole;
                let band = Band {
                    shape: k,
                    filter,
                    state: [[0.0; 4]; 4],
                    noise: seed ^ (k as u64 + 7).wrapping_mul(0xbf58_476d_1ce4_e5b9),
                    rise: 0.0,
                    scale: 1.0,
                    amplitude: 0.0,
                    step: 0.0,
                    done: false,
                };
                (band, (power / (response_energy(filter) / 3.0)).sqrt())
            })
            .collect();
        self.inhaled = Some(t + 0.08);
    }

    // The breath's level at `t`: in over 40 ms, falling 12 dB as the lungs fill, out over the last 40 ms.
    fn inhaling(&mut self, t: f64) {
        let Some(from) = self.inhaled else {
            return;
        };
        let x = t - from;
        let shape = if !(0.0..INHALE).contains(&x) {
            0.0
        } else {
            let rise = (x / 0.04).min(1.0);
            let fall = ((INHALE - x) / 0.04).min(1.0);
            (0.5 - 0.5 * (std::f64::consts::PI * rise).cos())
                * (0.5 - 0.5 * (std::f64::consts::PI * fall).cos())
                * 10f64.powf(-12.0 * x / INHALE / 20.0)
        };
        for (band, amplitude) in self.inhale.iter_mut() {
            band.step = (shape * *amplitude - band.amplitude) / BLOCK as f64;
        }
    }

    // A uniform draw from 0 to 1 from the breath's seed, mixed first so that seeds alike give draws apart.
    fn uniform(&mut self) -> f64 {
        let Some(breath) = self.breath.as_mut() else {
            return 0.5;
        };
        breath.seed = breath.seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = breath.seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as f64 / u64::MAX as f64
    }

    // The vibrato's depth for a note, a player's being deeper on one note than the next.
    fn vary(&mut self) -> f64 {
        let Some(vary) = self.breath.map(|b| b.vary) else {
            return 1.0;
        };
        let (u, v) = (self.uniform(), self.uniform());
        (vary * (-2.0 * u.max(1e-12).ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()).exp()
    }

    // A slow wander from -1 to 1, as a breath's: white noise through a one-pole lowpass at 1 Hz, kept at unit spread.
    fn wander(&mut self, drift: f64) -> f64 {
        if self.breath.is_none() {
            return 0.0;
        }
        let white = self.uniform() * 2.0 - 1.0;
        let pole = 1.0 - (-2.0 * std::f64::consts::PI * BLOCK as f64 / self.rate).exp();
        drift + pole * (white * (3.0 * (2.0 - pole) / pole).sqrt() - drift)
    }

    // Plays a change of note or vel as a wind player does.
    fn breathe(&mut self, note: f64, vel: f64, t: f64) {
        if self.let_go.is_some() {
            return;
        }
        if self.breath.is_none() {
            // A bowed string has no breath, but follows how hard it is played as vel or pressure moves.
            if (vel - self.vel).abs() > 0.03 && t >= self.swelled + SWELL {
                self.swelled = t;
                self.revoice(self.base, vel, t, From::Same);
            }
            return;
        }
        // The breath's level is the loudest vel lately, easing over a fifth of a second, so a quick dip under half of it is the tongue.
        let ease = (-(BLOCK as f64) / self.rate / 0.2).exp();
        self.blown = vel.max(self.blown * ease);
        if vel < 0.5 * self.blown {
            self.dipped = true;
        }
        let moved = (note - self.base).abs() >= 0.5;
        let rising = vel > self.last_vel + 1e-4;
        self.last_vel = vel;
        // The figures and loudness follow the breath, not the tongue's dip, which is played as the attack.
        let blown = self.blown;
        self.loud = (blown / self.figures.softest()).min(1.0) * self.register;
        if self.dipped && (moved || rising) {
            self.dipped = false;
            self.revoice(note, blown, t, From::Attack);
        } else if moved {
            self.revoice(note, blown, t, From::Settled);
        } else if (blown - self.vel).abs() > 0.03 && t >= self.swelled + SWELL {
            self.swelled = t;
            self.revoice(self.base, blown, t, From::Same);
        }
    }

    // The levels and frequencies at the next reading, BLOCK samples on, for each sine and band to glide to, with the note at `note`.
    fn read(&mut self, note: f64, vel: f64) {
        let t = (self.sample + BLOCK) as f64 / self.rate;
        if self.let_go.is_none() && (t >= self.hold || self.gate.value() <= 0.0) {
            self.let_go = Some(t.min(self.hold.max(0.0)));
            if self.breath.is_some_and(|b| b.inhale) {
                self.take_breath(t);
            }
        }
        self.breathe(note, vel, t);
        self.inhaling(t);
        let vibrato = self.voice.vibrato.unwrap_or_default();
        let swing = if vibrato.depth > 0.0 && vibrato.rate > 0.0 {
            self.phase += 2.0 * std::f64::consts::PI * vibrato.rate * BLOCK as f64 / self.rate;
            (t / vibrato.settles.max(0.05)).min(1.0) * self.phase.sin()
        } else {
            0.0
        };
        let x = ((t - self.slurred) / self.slurring).clamp(0.0, 1.0);
        let slur = self.slur * (0.5 + 0.5 * (std::f64::consts::PI * x).cos());
        let (bend, brightness) = self.expressed;
        let ratio = 2f64.powf(
            (note - self.base + slur + bend) / 12.0 + self.deepen * vibrato.depth * swing / 1200.0,
        );
        // A brightness away from 0.5 tilts the levels by up to 3 dB an octave either way.
        let tilt = (brightness != 0.5).then_some((brightness - 0.5) * 6.0);
        let gone = self.let_go.is_some_and(|go| t > go);
        let gliding = ((self.gliding - t) * self.rate).max(BLOCK as f64);
        self.fading.retain(|s| s.amplitude > 0.0);
        // A slur's catch: the partials dip as the breath settles on the new note.
        let catch = self.breath.map_or(0.0, |b| {
            let x = (t - self.caught) / CATCH;
            if (0.0..1.0).contains(&x) {
                b.catch * (std::f64::consts::PI * x).sin()
            } else {
                0.0
            }
        });
        // The wander comes in over the second after the figures start to hold.
        let local = t - self.origin + self.skip;
        let wandering = ((local - 0.5) / 1.0).clamp(0.0, 1.0);
        for k in 0..self.sines.len() {
            if self.sines[k].done {
                continue;
            }
            let drift = self.wander(self.sines[k].drift);
            self.sines[k].drift = drift;
            let (freq, shape) = &self.voice.partials[self.sines[k].shape];
            let depth = self.breath.map_or(0.0, |b| {
                if (freq / hz(self.base) - 1.0).abs() < 0.03 {
                    b.wander
                } else {
                    b.wander_above
                }
            });
            let mut level = self.level(shape, t, false)
                + self.voice.swings[self.sines[k].shape] * swing
                + wandering * depth * drift
                - catch;
            if let Some(tilt) = tilt {
                level += tilt * (freq / hz(self.base)).log2();
            }
            let omega =
                2.0 * std::f64::consts::PI * (freq * ratio).min(0.49 * self.rate) / self.rate;
            let sine = &mut self.sines[k];
            sine.turn = (omega.cos(), omega.sin());
            let target = (t / RISE).min(1.0) * 10f64.powf(level / 20.0);
            sine.step = (target - sine.amplitude) / gliding;
            if level < -140.0 && gone {
                sine.done = true;
            }
            let size = (sine.phasor.0 * sine.phasor.0 + sine.phasor.1 * sine.phasor.1).sqrt();
            sine.phasor = (sine.phasor.0 / size, sine.phasor.1 / size);
        }
        for k in 0..self.bands.len() {
            if self.bands[k].done {
                continue;
            }
            let shape = &self.voice.noise[self.bands[k].shape].1;
            let level = self.level(shape, t, true);
            let band = &mut self.bands[k];
            let rising = if t >= band.rise {
                1.0
            } else {
                0.5 - 0.5 * (std::f64::consts::PI * t / band.rise).cos()
            };
            let target = rising * 10f64.powf(level / 20.0);
            band.step = (target - band.amplitude) / gliding;
            if level < -140.0 && gone {
                band.done = true;
            }
        }
    }
}

impl AudioNode for Sustained {
    const ID: u64 = 0x6d78_0071;
    type Inputs = U4;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let (rates, set) = &mut self.rates;
        if let Some(rate) = rates.get_mut(*set) {
            *rate = sample_rate;
        }
        *set += 1;
        self.rate = sample_rate;
        self.warm = warm_bands(&self.voice, sample_rate);
        self.sines = Vec::with_capacity(self.voice.partials.capacity());
        self.bands = Vec::with_capacity(self.warm.len());
        self.restart();
    }

    fn reset(&mut self) {
        self.set_sample_rate(self.rate);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U1> {
        self.expressed = (input[2] as f64, input[3] as f64);
        if self.sample == 0 && self.refill.is_some() {
            self.refill();
        }
        if self.sample > 0 && self.sample.is_multiple_of(BLOCK) {
            self.read(input[0] as f64, input[1] as f64);
        }
        let mut sum = 0.0;
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            sum += sine.amplitude * sine.phasor.1;
            let (re, im) = sine.phasor;
            sine.phasor = (
                re * sine.turn.0 - im * sine.turn.1,
                re * sine.turn.1 + im * sine.turn.0,
            );
            sine.amplitude += sine.step;
        }
        for sine in self.fading.iter_mut() {
            sum += sine.amplitude.max(0.0) * sine.phasor.1;
            let (re, im) = sine.phasor;
            sine.phasor = (
                re * sine.turn.0 - im * sine.turn.1,
                re * sine.turn.1 + im * sine.turn.0,
            );
            sine.amplitude += sine.step;
        }
        for band in self.bands.iter_mut().filter(|b| !b.done) {
            sum += band.amplitude * band.filtered();
            band.amplitude += band.step;
        }
        // Over about 10 ms, as a step in loudness from one note to the next is otherwise a click.
        self.sounding += (self.loud - self.sounding) / (0.01 * self.rate);
        let mut breath = 0.0;
        for (band, _) in self.inhale.iter_mut() {
            breath += band.amplitude * band.filtered();
            band.amplitude += band.step;
        }
        self.sample += 1;
        [(sum * self.sounding + breath) as f32 * self.gain].into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recording(note: u8, partials: &[(f64, u16)]) -> Recording {
        let shape = || Shape {
            levels: vec![-20.0, -30.0],
            tail: -10.0,
        };
        let partials = partials
            .iter()
            .map(|(ratio, harmonic)| Partial {
                ratio: *ratio,
                harmonic: *harmonic,
                shape: shape(),
                swing: 0.0,
            })
            .collect();
        Recording {
            note,
            velocity: 127,
            partials,
            noise: vec![],
            vibrato: None,
        }
    }

    #[test]
    fn a_note_between_recordings_plays_their_nearly_equal_partials_once() {
        let figures = Figures::new(
            vec![0.0, 1.0],
            vec![],
            vec![
                recording(60, &[(1.0, 1), (7.35, 0), (8.35, 0)]),
                recording(62, &[(1.0, 1), (7.37, 0), (9.1, 0)]),
            ],
        );
        let mut ratios: Vec<f64> = figures
            .voice(61.0, 1.0)
            .partials
            .iter()
            .map(|(freq, _)| freq / hz(61.0))
            .collect();
        ratios.sort_by(f64::total_cmp);
        let rounded: Vec<f64> = ratios.iter().map(|r| (r * 100.0).round() / 100.0).collect();
        assert_eq!(rounded, [1.0, 7.36, 8.35, 9.1]);
        let voice = figures.voice(61.0, 1.0);
        let (_, merged) = voice
            .partials
            .iter()
            .find(|(freq, _)| (freq / hz(61.0) - 7.36).abs() < 0.005)
            .unwrap();
        let level = merged.levels[0];
        assert!(
            (level + 20.0).abs() < 1e-9,
            "the merged partial plays at {level} dB"
        );
    }

    fn same(a: &Voice, b: &Voice) -> bool {
        let shape = |x: &Shape, y: &Shape| x.levels == y.levels && x.tail == y.tail;
        let vibrato = |v: &Option<Vibrato>| v.map(|v| (v.settles, v.rate, v.depth));
        a.times == b.times
            && a.partials.len() == b.partials.len()
            && a.partials
                .iter()
                .zip(&b.partials)
                .all(|((f, x), (g, y))| f == g && shape(x, y))
            && a.noise.len() == b.noise.len()
            && a.noise
                .iter()
                .zip(&b.noise)
                .all(|((f, x), (g, y))| f == g && shape(x, y))
            && a.swings == b.swings
            && vibrato(&a.vibrato) == vibrato(&b.vibrato)
    }

    #[test]
    fn a_voice_with_room_fills_with_any_note_as_a_new_voice_would_and_allocates_nothing() {
        let all: [(&str, &[u8]); 16] = [
            ("cello", include_bytes!("figures/cello.mxh")),
            ("deep_bass", include_bytes!("figures/deep_bass.mxf")),
            ("electric_bass", include_bytes!("figures/electric_bass.mxf")),
            (
                "electric_guitar",
                include_bytes!("figures/electric_guitar.mxf"),
            ),
            (
                "electric_piano",
                include_bytes!("figures/electric_piano.mxf"),
            ),
            ("flute", include_bytes!("figures/flute.mxh")),
            ("glockenspiel", include_bytes!("figures/glockenspiel.mxf")),
            ("guitar", include_bytes!("figures/guitar.mxf")),
            ("harp", include_bytes!("figures/harp.mxf")),
            ("marimba", include_bytes!("figures/marimba.mxf")),
            ("piano_pedal", include_bytes!("figures/piano_pedal.mxf")),
            ("piano", include_bytes!("figures/piano.mxf")),
            ("timpani", include_bytes!("figures/timpani.mxf")),
            ("vibraphone", include_bytes!("figures/vibraphone.mxf")),
            ("violin", include_bytes!("figures/violin.mxh")),
            ("xylophone", include_bytes!("figures/xylophone.mxf")),
        ];
        for (name, bytes) in all {
            let figures = Figures::read(bytes);
            let mut voice = figures.room();
            for note in (0..=127).map(|n| n as f64).chain([60.5, 33.25]) {
                for vel in [0.0, 0.05, 0.3, 0.5, 0.7, 0.9, 1.0] {
                    let made =
                        crate::allocations::allocations(|| figures.fill(&mut voice, note, vel));
                    assert_eq!(made, 0, "{name} at note {note}, vel {vel} allocates");
                    assert!(
                        same(&voice, &figures.voice(note, vel)),
                        "{name} at note {note}, vel {vel} fills otherwise than a new voice"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod noise_rate {
    use super::{Band, NOISE_RATE, band_filter, noise_scale};

    fn power(low: f64, high: f64, rate: f64) -> f64 {
        let mut band = Band {
            shape: 0,
            filter: band_filter(low, high, rate),
            state: [[0.0; 4]; 4],
            noise: 7,
            rise: 0.0,
            scale: noise_scale(low, high, rate),
            amplitude: 0.0,
            step: 0.0,
            done: false,
        };
        let samples = (20.0 * rate) as usize;
        let sum: f64 = (0..samples)
            .map(|_| (band.filtered() * band.scale).powi(2))
            .skip((0.5 * rate) as usize)
            .sum();
        10.0 * (sum / (samples as f64 - 0.5 * rate)).log10()
    }

    #[test]
    fn a_noise_band_carries_the_same_power_at_any_rate_it_plays_at() {
        for (low, high) in [(1057.0, 1205.0), (2851.0, 3207.0), (5088.0, 5702.0)] {
            let at = power(low, high, NOISE_RATE);
            for rate in [16_000.0, 44_100.0, 96_000.0] {
                let other = power(low, high, rate);
                assert!(
                    (other - at).abs() < 0.3,
                    "{low}-{high} Hz at {rate}: {other:.2} dB against {at:.2}"
                );
            }
        }
    }
}

#[cfg(test)]
mod build_time {
    use crate::options::{Context, Options};
    use crate::spec::Value;
    use fundsp::prelude::*;
    use std::time::Instant;

    fn build(synth: &crate::Synth, note: f64, rate: f64) -> crate::Note {
        let given = [
            ("note".to_string(), Value::Number(note)),
            ("vel".to_string(), Value::Number(0.7)),
        ];
        let (values, _) = crate::engine::checked(&synth.name, &synth.specs, &given).unwrap();
        let options = Options::new(
            synth.name.as_str().into(),
            synth.specs.clone().into(),
            values,
        );
        let mut note = (synth.build)(&options, &mut Context::new(synth.name.as_str().into()));
        note.sound.set_sample_rate(rate);
        note.sound.allocate();
        note
    }

    fn synth(name: &str) -> crate::Synth {
        crate::synths::all()
            .into_iter()
            .find(|s| s.name == name)
            .unwrap()
    }

    // Instruments share their noise bands, so each plays at a rate no other test plays, and its
    // first note works the bands out.
    #[test]
    fn a_fitted_note_after_the_first_builds_ten_times_as_fast() {
        for (name, rate) in [
            ("piano", 41_000.0),
            ("cello", 41_001.0),
            ("marimba", 41_002.0),
            ("electric_piano", 41_003.0),
        ] {
            let synth = synth(name);
            let start = Instant::now();
            build(&synth, 60.0, rate);
            let first = start.elapsed().as_secs_f64();
            let mut each: Vec<f64> = [48.0, 55.0, 62.0, 69.0, 76.0]
                .into_iter()
                .map(|note| {
                    let start = Instant::now();
                    build(&synth, note, rate);
                    start.elapsed().as_secs_f64()
                })
                .collect();
            each.sort_by(f64::total_cmp);
            let median = each[each.len() / 2];
            assert!(
                median * 10.0 < first,
                "a {name} note builds in {median:.6} s after a first note of {first:.6} s"
            );
        }
    }

    #[test]
    fn a_fitted_note_built_from_the_bands_kept_sounds_as_the_first_did() {
        // A rate no other test plays, so the first note works its bands out.
        let rate = 37_000.0;
        let render = || {
            let mut note = build(&synth("marimba"), 48.0, rate);
            let mut frame = vec![0.0; note.sound.outputs()];
            (0..rate as usize)
                .flat_map(|_| {
                    note.sound.tick(&[], &mut frame);
                    frame.clone()
                })
                .collect::<Vec<f32>>()
        };
        let first = render();
        assert!(first.iter().any(|s| s.abs() > 1e-4));
        assert_eq!(render(), first);
    }
}

#[cfg(test)]
mod ready {
    use crate::options::{Context, Options};
    use crate::spec::Value;
    use crate::synth::Given;
    use fundsp::prelude::*;

    fn options(synth: &crate::Synth, note: f64, vel: f64) -> Options {
        let given = [
            ("note".to_string(), Value::Number(note)),
            ("vel".to_string(), Value::Number(vel)),
        ];
        let (values, _) = crate::engine::checked(&synth.name, &synth.specs, &given).unwrap();
        Options::new(
            synth.name.as_str().into(),
            synth.specs.clone().into(),
            values,
        )
    }

    // Ticks into room made beforehand, so the ticks are all it allocates.
    fn samples_into(note: &mut crate::Note, frames: usize, out: &mut Vec<f32>) {
        let mut frame = [0.0; 2];
        let outputs = note.sound.outputs();
        for _ in 0..frames {
            note.sound.tick(&[], &mut frame[..outputs]);
            out.extend_from_slice(&frame[..outputs]);
        }
    }

    fn samples(note: &mut crate::Note, frames: usize) -> Vec<f32> {
        let mut out = Vec::with_capacity(2 * frames);
        samples_into(note, frames, &mut out);
        out
    }

    #[test]
    fn a_fitted_note_built_ready_and_given_another_note_plays_as_one_built_for_it() {
        let fitted = [
            "piano",
            "guitar",
            "electric_bass",
            "deep_bass",
            "harp",
            "electric_piano",
            "electric_guitar",
            "marimba",
            "vibraphone",
            "xylophone",
            "glockenspiel",
            "timpani",
            "violin",
            "cello",
            "flute_figured",
        ];
        for name in fitted {
            let synth = crate::synths::all()
                .into_iter()
                .find(|s| s.name == name)
                .unwrap();
            for (note, vel) in [(48.0, 0.3), (73.0, 1.0)] {
                let mut ctx = Context::ready(name.into());
                let plain = options(&synth, 60.0, 0.7);
                let mut ready = (synth.build)(&plain, &mut ctx);
                let hold = ctx.take_ready().expect("a fitted note builds ready");
                ready.sound.set_sample_rate(48_000.0);
                ready.sound.allocate();
                let mut built =
                    (synth.build)(&options(&synth, note, vel), &mut Context::new(name.into()));
                built.sound.set_sample_rate(48_000.0);
                built.sound.allocate();
                let release = built.release.as_ref().map(|(_, seconds)| *seconds);
                let given = Given {
                    note,
                    vel,
                    held: false,
                    pedal: 0.0,
                    pressure: 0.0,
                };
                assert_eq!(
                    hold.give(given),
                    (built.end, release.unwrap()),
                    "{name} lasts otherwise"
                );
                // As the engine does, a held note's note and vel are set where it follows them.
                let controls = plain.take_controls();
                for (option, value) in [("note", note), ("vel", vel)] {
                    if let Some(control) = controls.get(option) {
                        control.target.set(value as f32);
                    }
                }
                // A second and a half, past where a breath's wander comes in.
                let frames = 72_000;
                let mut first = Vec::with_capacity(2 * frames);
                let made =
                    crate::allocations::allocations(|| samples_into(&mut ready, 64, &mut first));
                assert_eq!(made, 0, "{name} allocates as it starts note {note}");
                first.extend(samples(&mut ready, frames - 64));
                assert!(
                    first == samples(&mut built, frames),
                    "{name} plays note {note} at vel {vel} otherwise"
                );
            }
        }
    }
}

#[cfg(test)]
mod noise_under {
    use super::{Figures, SILENT};

    #[test]
    fn leaves_out_the_band_that_crosses_the_frequency_by_its_centre_and_keeps_the_next() {
        let figures = Figures::read(include_bytes!("figures/piano.mxf"));
        let before = figures.voice(84.0, 0.6);
        let mut after = figures.voice(84.0, 0.6);
        after.noise_under(600.0, 0.0);
        let band = |voice: &super::Voice, low: f64| {
            voice
                .noise
                .iter()
                .find(|((l, _), _)| (*l - low).abs() < 1.0)
                .unwrap()
                .1
                .levels
                .clone()
        };
        assert!(band(&after, 516.0).iter().all(|level| *level <= SILENT));
        assert!(band(&before, 516.0).iter().any(|level| *level > SILENT));
        assert_eq!(band(&after, 602.0), band(&before, 602.0));
    }
}
