//! Synths that play figures fitted to recordings of struck notes partial by partial, as `mix mx.fit` writes them.
//! Each partial is a sine whose level in dB runs in straight lines between fixed times after the strike, and falls at a last slope after them.
//! The strike's noise is white noise through each band's filter, its power in straight lines between the same times.
//! Figures of held notes, bowed, blown or played on an organ, hold each level after the last time until the note is let go, then fall at its slope, with the recordings' vibrato.

use crate::nodes::hz;
use crate::{Note, Options};
use fundsp::prelude::*;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

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
}

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
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let taken = self.bytes[self.at..self.at + N].try_into().unwrap();
        self.at += N;
        taken
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
    pub fn read(bytes: &[u8]) -> Figures {
        let mut reader = Reader { bytes, at: 0 };
        let magic = reader.take::<4>();
        let held = &magic == b"MXH1";
        assert!(
            held || &magic == b"MXF1",
            "not figures of struck or held notes"
        );
        let count = reader.u16() as usize;
        let times: Vec<f64> = (0..count).map(|_| reader.f32()).collect();
        let bands = (0..reader.u16())
            .map(|_| (reader.f32(), reader.f32()))
            .collect::<Vec<_>>();
        let recordings = (0..reader.u16())
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
                let partials = (0..reader.u16())
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
        Figures {
            times,
            bands,
            recordings,
        }
    }

    /// The recordings a note at vel takes its figures from, with their weights: the nearest recorded velocities below and above vel, and at each the nearest notes recorded at that velocity below and above the note, in straight lines between them.
    /// Velocity comes first since it changes the sound more than a few semitones do, and few notes are recorded at more than one velocity.
    fn nearest(&self, note: f64, vel: f64) -> Vec<(&Recording, f64)> {
        let mut velocities: Vec<u8> = self.recordings.iter().map(|r| r.velocity).collect();
        velocities.sort();
        velocities.dedup();
        let around = |values: &[f64], at: f64| -> Vec<(usize, f64)> {
            let above = values.iter().position(|v| *v >= at);
            match above {
                None => vec![(values.len() - 1, 1.0)],
                Some(0) => vec![(0, 1.0)],
                Some(i) if values[i] == at => vec![(i, 1.0)],
                Some(i) => {
                    let share = (at - values[i - 1]) / (values[i] - values[i - 1]);
                    vec![(i - 1, 1.0 - share), (i, share)]
                }
            }
        };
        let layers: Vec<f64> = velocities.iter().map(|v| *v as f64).collect();
        around(&layers, vel * 127.0)
            .into_iter()
            .flat_map(|(j, across)| {
                let mut at: Vec<&Recording> = self
                    .recordings
                    .iter()
                    .filter(|r| r.velocity == velocities[j])
                    .collect();
                at.sort_by_key(|r| r.note);
                let keys: Vec<f64> = at.iter().map(|r| r.note as f64).collect();
                around(&keys, note)
                    .into_iter()
                    .map(move |(i, along)| (at[i], along * across))
                    .collect::<Vec<_>>()
            })
            .filter(|(_, weight)| *weight > 0.0)
            .collect()
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
    /// A partial takes each recording's level of it at its weight, in dB, quieter by the weight of the recordings that lack it.
    pub fn voice(&self, note: f64, vel: f64) -> Voice {
        let nearest = self.nearest(note, vel);
        let pitch = hz(note);
        // Each harmonic is one group across the recordings, and so is each run of partials between harmonics within 1% of one another, one from each recording, so two nearly equal sines do not beat.
        let mut groups: Vec<Vec<(&Partial, f64)>> = vec![];
        let mut between: Vec<(&Partial, f64, usize)> = nearest
            .iter()
            .enumerate()
            .flat_map(|(i, (r, weight))| r.partials.iter().map(move |p| (p, *weight, i)))
            .collect();
        between.sort_by(|a, b| {
            (a.0.harmonic, a.0.ratio)
                .partial_cmp(&(b.0.harmonic, b.0.ratio))
                .unwrap()
        });
        let mut members: Vec<usize> = vec![];
        for (partial, weight, from) in between {
            let joins = groups.last().is_some_and(|group| {
                let (first, _) = group[0];
                let close = if partial.harmonic > 0 {
                    first.harmonic == partial.harmonic
                } else {
                    first.harmonic == 0 && partial.ratio - first.ratio <= 0.01 * first.ratio
                };
                close && !members.contains(&from)
            });
            if joins {
                groups.last_mut().unwrap().push((partial, weight));
                members.push(from);
            } else {
                groups.push(vec![(partial, weight)]);
                members = vec![from];
            }
        }
        let swings = groups
            .iter()
            .map(|held| {
                let total: f64 = held.iter().map(|(_, w)| w).sum();
                held.iter().map(|(p, w)| p.swing * w).sum::<f64>() / total
            })
            .collect();
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
        let partials = groups
            .iter()
            .map(|held| {
                let total: f64 = held.iter().map(|(_, w)| w).sum();
                let ratio = held.iter().map(|(p, w)| p.ratio * w).sum::<f64>() / total;
                let quieter = 20.0 * total.log10();
                let levels = (0..self.times.len())
                    .map(|t| {
                        held.iter().map(|(p, w)| p.shape.levels[t] * w).sum::<f64>() / total
                            + quieter
                    })
                    .collect();
                let tail = held.iter().map(|(p, w)| p.shape.tail * w).sum::<f64>() / total;
                (pitch * ratio, Shape { levels, tail })
            })
            .collect();
        let noise = self
            .bands
            .iter()
            .enumerate()
            .map(|(k, band)| {
                let levels = (0..self.times.len())
                    .map(|t| nearest.iter().map(|(r, w)| r.noise[k].levels[t] * w).sum())
                    .collect();
                let tail = nearest.iter().map(|(r, w)| r.noise[k].tail * w).sum();
                (*band, Shape { levels, tail })
            })
            .collect();
        Voice {
            times: self.times.clone(),
            partials,
            noise,
            swings,
            vibrato,
        }
    }
}

/// A note played from figures fitted to recordings.
/// The recordings are each brought to the same peak, so a velocity carries the energy vel 1 has over the first half second, and plays louder by vel alone, as vel to the power `growth`.
pub(super) fn fitted(figures: &Figures, level: f64, growth: f64, o: &Options) -> Note {
    let vel = o.number("vel").clamp(0.0, 1.0);
    let note = o.number("note");
    let voice = figures.voice(note, vel);
    let loudness = (figures.voice(note, 1.0).energy() / voice.energy()).sqrt();
    let gain = (level * vel.powf(growth) * loudness).max(1e-9);
    let end = voice.silent_after();
    Note::envelope(voice.sound() * gain as f32, o).within(end)
}

/// A note played from figures fitted to recordings that keep their levels from one velocity to the next, so the figures carry its loudness.
pub(super) fn kept(figures: &Figures, level: f64, o: &Options) -> Note {
    let note = o.number("note");
    let vel = o.number("vel").clamp(0.0, 1.0);
    played(figures, note, vel, kept_gain(figures, level, note, vel), o)
}

/// The gain of a note played as `kept` plays it: below the softest velocity recorded, a note is quieter in proportion to vel.
pub(super) fn kept_gain(figures: &Figures, level: f64, _note: f64, vel: f64) -> f64 {
    (level * (vel / figures.softest()).min(1.0)).max(1e-9)
}

/// A note played from figures fitted to recordings that keep their levels from one velocity to the next, but not from one note to the next.
pub(super) fn evened(figures: &Figures, level: f64, o: &Options) -> Note {
    let note = o.number("note");
    let vel = o.number("vel").clamp(0.0, 1.0);
    played(
        figures,
        note,
        vel,
        evened_gain(figures, level, note, vel),
        o,
    )
}

/// The gain of a note played as `evened` plays it: every note at vel 1 carries the same energy over the first half second, its noise's with its partials', and a softer velocity is as much quieter as the recordings at that note.
pub(super) fn evened_gain(figures: &Figures, level: f64, note: f64, vel: f64) -> f64 {
    let even = figures.voice(note, 1.0).whole_energy().sqrt();
    (level * (vel / figures.softest()).min(1.0) / even).max(1e-9)
}

fn played(figures: &Figures, note: f64, vel: f64, gain: f64, o: &Options) -> Note {
    let voice = figures.voice(note, vel);
    let end = voice.silent_after();
    Note::envelope(voice.sound() * gain as f32, o).within(end)
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

    /// The noise in the bands under `hz` made `by` times as loud; 0 leaves it out.
    pub fn noise_under(&mut self, hz: f64, by: f64) {
        for ((_, high), shape) in &mut self.noise {
            if *high <= hz {
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

    /// The energy of the partials and the noise over the first half second.
    pub fn whole_energy(&self) -> f64 {
        let steps = 100;
        // A sine's energy counts its amplitude squared, twice its power; white noise from -1 to 1 has a power of 1/3, and the filters pass the sum of their response squared of it.
        let noise = self
            .noise
            .iter()
            .map(|((low, high), shape)| {
                let passed = response_energy(band_filter(*low, *high, DEFAULT_SR));
                (0..steps)
                    .map(|i| shape.power_at(&self.times, (i as f64 + 0.5) * 0.5 / steps as f64))
                    .sum::<f64>()
                    * 2.0
                    / 3.0
                    * passed
            })
            .sum::<f64>()
            * 0.5
            / steps as f64;
        self.energy() + noise
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
        let mut node = Struck {
            voice: self,
            rate: DEFAULT_SR,
            sines: vec![],
            bands: vec![],
            sample: 0,
        };
        node.set_sample_rate(DEFAULT_SR);
        Net::wrap(Box::new(An(node)))
    }
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
    (response_energy(band_filter(low, high, NOISE_RATE)) / response_energy(band_filter(low, high, rate))).sqrt()
}

/// A band's noise gain and its filters once warmed, by its edges, its place among the bands and the rate, which every note of an instrument shares.
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

#[derive(Clone)]
struct Struck {
    voice: Voice,
    rate: f64,
    sines: Vec<Sine>,
    bands: Vec<Band>,
    sample: usize,
}

impl Struck {
    // The levels at the next reading, BLOCK samples on, for each sine and band to glide to.
    fn read(&mut self) {
        let t = (self.sample + BLOCK) as f64 / self.rate;
        let last = *self.voice.times.last().unwrap();
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            let level = self.voice.partials[sine.shape].1.at(&self.voice.times, t);
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
    type Inputs = U0;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.sample = 0;
        self.sines = self
            .voice
            .partials
            .iter()
            .enumerate()
            .filter(|(_, (freq, shape))| {
                *freq < sample_rate / 2.0 && shape.levels.iter().any(|l| *l > -140.0)
            })
            .map(|(i, (freq, _))| {
                let omega = 2.0 * std::f64::consts::PI * freq / sample_rate;
                Sine {
                    shape: i,
                    phasor: (1.0, 0.0),
                    turn: (omega.cos(), omega.sin()),
                    amplitude: 0.0,
                    step: 0.0,
                    done: false,
                    drift: 0.0,
                }
            })
            .collect();
        // Every band is warmed, silent or not, so the instrument's first note readies the bands
        // its other notes sound.
        self.bands = self
            .voice
            .noise
            .iter()
            .enumerate()
            .filter(|(_, ((_, high), _))| *high < sample_rate / 2.0)
            .map(|(k, ((low, high), shape))| (warmed(k, *low, *high, sample_rate), shape))
            .filter(|(_, shape)| shape.levels.iter().any(|l| *l > SILENT))
            .map(|(band, _)| band)
            .collect();
        self.read();
    }

    fn reset(&mut self) {
        self.set_sample_rate(self.rate);
    }

    fn tick(&mut self, _input: &Frame<f32, U0>) -> Frame<f32, U1> {
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
        [sum as f32].into()
    }
}

/// A held note played from figures fitted to recordings of held notes: it holds until `hold` seconds or until its gate lets it go, then each partial and band falls at its slope.
/// Its pitch follows `note` as it slides, and its vibrato, `vibrato` when given in place of the recordings', fades in until the note settles.
/// With `legato` at 1 it speaks at once, from where the recordings settle.
/// With a `breath`, the note is played as a wind player plays it, see `Breath`.
pub(super) fn held_note(
    figures: &'static Figures,
    gain: f64,
    vibrato: Option<f64>,
    legato: f64,
    breath: Option<Breath>,
    o: &Options,
) -> Note {
    let note = o.number("note");
    let vel = o.number("vel").clamp(0.0, 1.0);
    let voice = vibrated(figures.voice(note, vel), vibrato);
    let skip = legato.clamp(0.0, 1.0) * voice.vibrato.map_or(0.0, |v| v.settles);
    let hold = o.number("attack") + o.number("decay") + o.number("sustain");
    let falls = if breath.is_some() { 5.0 } else { voice.falls() };
    let under = voice.loudest() - 100.0;
    let gate = shared(1.0);
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
        bend: 0.0,
        bent: f64::NEG_INFINITY,
        bending: BEND,
        loud: 1.0,
        sounding: 1.0,
        register: breath.map_or(1.0, |b| register(figures, note, b.even)),
        pace: 1.0,
        inhale: vec![],
        inhaled: None,
        fading: vec![],
    };
    let mut node = node;
    node.deepen = node.vary();
    node.set_sample_rate(DEFAULT_SR);
    let sound =
        (o.follow("note") | o.follow("vel")) >> (Net::wrap(Box::new(An(node))) * gain as f32);
    Note {
        sound,
        end: hold + falls,
        release: Some((gate, falls)),
    }
}

/// The voice with `vibrato` in semitones in place of the recordings'.
fn vibrated(mut voice: Voice, vibrato: Option<f64>) -> Voice {
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
    voice
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
fn register(figures: &Figures, note: f64, even: f64) -> f64 {
    let held = |note: f64| {
        let power: f64 = figures
            .voice(note, 1.0)
            .partials
            .iter()
            .map(|(_, s)| 10f64.powf(s.levels[s.levels.len() - 1] / 10.0))
            .sum();
        10.0 * power.max(1e-30).log10()
    };
    let mut notes: Vec<u8> = figures.recordings.iter().map(|r| r.note).collect();
    notes.sort();
    let middle = notes[notes.len() / 2] as f64;
    10f64.powf(-even * (held(note) - held(middle)) / 20.0)
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
    bend: f64,
    bent: f64,
    bending: f64,
    loud: f64,
    sounding: f64,
    register: f64,
    pace: f64,
    inhale: Vec<(Band, f64)>,
    inhaled: Option<f64>,
    fading: Vec<Sine>,
}

impl Sustained {
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
            self.register = register(self.figures, note, breath.even);
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
            sample: 0,
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
            self.bend = if from == From::Settled || !step {
                self.base - note
            } else {
                0.0
            };
            self.bent = t;
            let (u, v) = (self.uniform(), self.uniform());
            let drawn = (BEND
                * (0.5
                    * (-2.0 * u.max(1e-12).ln()).sqrt()
                    * (2.0 * std::f64::consts::PI * v).cos())
                .exp())
            .clamp(0.015, 0.04);
            self.bending = if from == From::Settled && step {
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
        if self.breath.is_none() || self.let_go.is_some() {
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
        let x = ((t - self.bent) / self.bending).clamp(0.0, 1.0);
        let bend = self.bend * (0.5 + 0.5 * (std::f64::consts::PI * x).cos());
        let ratio = 2f64
            .powf((note - self.base + bend) / 12.0 + self.deepen * vibrato.depth * swing / 1200.0);
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
            let level = self.level(shape, t, false)
                + self.voice.swings[self.sines[k].shape] * swing
                + wandering * depth * drift
                - catch;
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
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let mut struck = Struck {
            voice: self.voice.clone(),
            rate: sample_rate,
            sines: vec![],
            bands: vec![],
            sample: 0,
        };
        struck.set_sample_rate(sample_rate);
        self.rate = sample_rate;
        self.sample = 0;
        self.let_go = None;
        self.phase = 0.0;
        self.origin = 0.0;
        self.gliding = 0.0;
        self.fading.clear();
        self.sines = struck.sines;
        self.bands = struck.bands;
        self.sines.iter_mut().for_each(|s| s.amplitude = 0.0);
        self.bands.iter_mut().for_each(|b| b.amplitude = 0.0);
        self.read(self.base, self.vel);
    }

    fn reset(&mut self) {
        self.set_sample_rate(self.rate);
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
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
        [(sum * self.sounding + breath) as f32].into()
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
        let figures = Figures {
            times: vec![0.0, 1.0],
            bands: vec![],
            recordings: vec![
                recording(60, &[(1.0, 1), (7.35, 0), (8.35, 0)]),
                recording(62, &[(1.0, 1), (7.37, 0), (9.1, 0)]),
            ],
        };
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
}

#[cfg(test)]
mod noise_rate {
    use super::{band_filter, noise_scale, Band, NOISE_RATE};

    fn power(low: f64, high: f64, rate: f64) -> f64 {
        let mut band = Band { shape: 0, filter: band_filter(low, high, rate), state: [[0.0; 4]; 4], noise: 7, rise: 0.0, scale: noise_scale(low, high, rate), amplitude: 0.0, step: 0.0, done: false };
        let samples = (20.0 * rate) as usize;
        let sum: f64 = (0..samples).map(|_| (band.filtered() * band.scale).powi(2)).skip((0.5 * rate) as usize).sum();
        10.0 * (sum / (samples as f64 - 0.5 * rate)).log10()
    }

    #[test]
    fn a_noise_band_carries_the_same_power_at_any_rate_it_plays_at() {
        for (low, high) in [(1057.0, 1205.0), (2851.0, 3207.0), (5088.0, 5702.0)] {
            let at = power(low, high, NOISE_RATE);
            for rate in [16_000.0, 44_100.0, 96_000.0] {
                let other = power(low, high, rate);
                assert!((other - at).abs() < 0.3, "{low}-{high} Hz at {rate}: {other:.2} dB against {at:.2}");
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

    #[test]
    fn a_fitted_note_after_the_first_builds_in_under_0_3_ms() {
        for name in ["piano", "cello", "marimba", "electric_piano"] {
            let synth = synth(name);
            build(&synth, 60.0, 48_000.0);
            let notes = [48.0, 55.0, 62.0, 69.0, 76.0];
            let start = Instant::now();
            for note in notes {
                build(&synth, note, 48_000.0);
            }
            let each = start.elapsed().as_secs_f64() * 1000.0 / notes.len() as f64;
            assert!(each < 0.3, "a {name} note builds in {each:.3} ms");
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
