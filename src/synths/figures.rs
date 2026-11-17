//! Synths that play figures fitted to recordings of struck notes partial by partial, as `mix mx.fit` writes them.
//! Each partial is a sine whose level in dB runs in straight lines between fixed times after the strike, and falls at a last slope after them.
//! The strike's noise is white noise through each band's filter, its power in straight lines between the same times.

use crate::nodes::hz;
use fundsp::prelude::*;

/// The seconds over which a sine rises at its start.
const RISE: f64 = 0.001;
/// The level no noise band goes below, where it is silent.
const SILENT: f64 = -160.0;
/// The seconds the noise runs through a band's filters before the strike, so the band starts with the strike instead of ringing up after it.
const WARM: f64 = 0.1;
/// The samples between two readings of the levels, which glide in straight lines in between.
const BLOCK: usize = 16;

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
}

#[derive(Clone)]
struct Partial {
    ratio: f64,
    harmonic: u16,
    shape: Shape,
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
    pub fn read(bytes: &[u8]) -> Figures {
        let mut reader = Reader { bytes, at: 0 };
        assert_eq!(&reader.take::<4>(), b"MXF1", "not figures of struck notes");
        let count = reader.u16() as usize;
        let times: Vec<f64> = (0..count).map(|_| reader.f32()).collect();
        let bands = (0..reader.u16())
            .map(|_| (reader.f32(), reader.f32()))
            .collect::<Vec<_>>();
        let recordings = (0..reader.u16())
            .map(|_| {
                let (note, velocity) = (reader.u8(), reader.u8());
                let partials = (0..reader.u16())
                    .map(|_| {
                        let (ratio, harmonic) = (reader.f32(), reader.u16());
                        Partial {
                            ratio,
                            harmonic,
                            shape: reader.shape(times.len()),
                        }
                    })
                    .collect();
                let noise = bands.iter().map(|_| reader.shape(times.len())).collect();
                Recording {
                    note,
                    velocity,
                    partials,
                    noise,
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
        }
    }
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
}

#[derive(Clone)]
struct Band {
    shape: usize,
    filter: [[f64; 5]; 4],
    state: [[f64; 4]; 4],
    noise: u64,
    rise: f64,
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
            let target = rising * power.sqrt();
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
                }
            })
            .collect();
        self.bands = self
            .voice
            .noise
            .iter()
            .enumerate()
            .filter(|(_, ((_, high), shape))| {
                *high < sample_rate / 2.0 && shape.levels.iter().any(|l| *l > SILENT)
            })
            .map(|(k, ((low, high), _))| {
                let mut band = Band {
                    shape: k,
                    filter: band_filter(*low, *high, sample_rate),
                    state: [[0.0; 4]; 4],
                    noise: 0x9e37_79b9_7f4a_7c15
                        ^ (k as u64 + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9),
                    rise: (1.0 / (high - low)).clamp(0.002, 0.02),
                    amplitude: 0.0,
                    step: 0.0,
                    done: false,
                };
                for _ in 0..(WARM * sample_rate) as usize {
                    band.filtered();
                }
                band
            })
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
            })
            .collect();
        Recording {
            note,
            velocity: 127,
            partials,
            noise: vec![],
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
