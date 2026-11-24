//! Analyses of a sound while the engine plays: its loudness and its pitch at an interval, and each onset as it happens.
//! The audio thread only copies the sound into a ring buffer; the reporting side works the analysis out, so nothing here makes the audio wait.

use crate::engine::report::{FLOOR, Hub, Report, Topic};
use std::collections::VecDeque;

/// The lowest pitch found, in Hz.
const LOWEST: f64 = 50.0;
/// The highest pitch found, in Hz.
const HIGHEST: f64 = 2000.0;
/// How low the normalised difference must dip at a lag for the sound to have a pitch there.
const DIP: f64 = 0.2;
/// The frames an onset is looked for in at a time.
const STEP: usize = 64;
/// How many steps before one its energy is compared with.
const BEFORE: usize = 32;
/// How many times the energy of the steps before it a step must have to start an onset, 10 dB.
const RISE: f64 = 10.0;
/// The least energy a step must have to start an onset, -60 dB.
const QUIET: f64 = 1e-6;
/// The fewest seconds between two onsets.
const GAP: f64 = 0.05;

/// The reporting side of an analysis: it reads what the audio thread copies, and sends the reports.
pub(crate) struct Listening {
    pub id: u64,
    samples: rtrb::Consumer<f32>,
    spans: rtrb::Consumer<(u64, u32)>,
    rate: f64,
    /// The frames between two reports of the loudness and the pitch, counted from `start`.
    interval: u64,
    start: u64,
    /// The frame after the last one heard.
    next: Option<u64>,
    /// The last frames heard, oldest first once `at` wraps.
    history: Vec<f32>,
    at: usize,
    heard: usize,
    window: Vec<f32>,
    differences: Vec<f64>,
    step: Vec<f32>,
    step_start: u64,
    energies: VecDeque<f64>,
    last_onset: Option<u64>,
}

impl Listening {
    pub fn new(
        id: u64,
        samples: rtrb::Consumer<f32>,
        spans: rtrb::Consumer<(u64, u32)>,
        rate: f64,
        interval: u64,
        start: u64,
    ) -> Listening {
        let longest = (rate / LOWEST).ceil() as usize;
        Listening {
            id,
            samples,
            spans,
            rate,
            interval: Ord::max(interval, 1),
            start,
            next: None,
            history: vec![0.0; 2 * longest + 1],
            at: 0,
            heard: 0,
            window: Vec::with_capacity(2 * longest + 1),
            differences: Vec::with_capacity(longest + 2),
            step: Vec::with_capacity(STEP),
            step_start: 0,
            energies: VecDeque::with_capacity(BEFORE),
            last_onset: None,
        }
    }

    /// Reads what was copied so far and sends the reports it completes; false once the audio side has let the analysis go and all of it is read.
    pub fn hear(&mut self, hub: &Hub) -> bool {
        let wanted = hub.wants(Topic::Analysis);
        while let Ok((first, count)) = self.spans.pop() {
            if self.next != Some(first) {
                self.restart(first);
            }
            for k in 0..count as u64 {
                let x = self.samples.pop().unwrap_or(0.0);
                self.take(first + k, x, wanted, hub);
            }
            self.next = Some(first + count as u64);
        }
        !(self.spans.is_abandoned() && self.spans.is_empty())
    }

    /// Forgets what was heard before a gap, so no report spans it.
    fn restart(&mut self, first: u64) {
        self.history.fill(0.0);
        self.at = 0;
        self.heard = 0;
        self.step.clear();
        self.step_start = first;
        self.energies.clear();
    }

    fn take(&mut self, frame: u64, x: f32, wanted: bool, hub: &Hub) {
        self.history[self.at] = x;
        self.at = (self.at + 1) % self.history.len();
        self.heard += 1;
        if self.step.is_empty() {
            self.step_start = frame;
        }
        self.step.push(x);
        if self.step.len() == STEP {
            if let Some(onset) = self.onset()
                && wanted
            {
                hub.send(Report::Onset {
                    id: self.id,
                    seconds: onset as f64 / self.rate,
                });
            }
            self.step.clear();
        }
        if (frame + 1 - self.start).is_multiple_of(self.interval) && wanted {
            let (loudness, pitch, confidence) = self.measure();
            hub.send(Report::Analysis {
                id: self.id,
                seconds: frame as f64 / self.rate,
                loudness,
                pitch,
                confidence,
            });
        }
    }

    /// The frame an onset starts at in the step just heard, if one does.
    fn onset(&mut self) -> Option<u64> {
        let energy = self
            .step
            .iter()
            .map(|&x| f64::from(x) * f64::from(x))
            .sum::<f64>()
            / STEP as f64;
        let before = if self.energies.is_empty() {
            0.0
        } else {
            self.energies.iter().sum::<f64>() / self.energies.len() as f64
        };
        if self.energies.len() == BEFORE {
            self.energies.pop_front();
        }
        self.energies.push_back(energy);
        let apart = self
            .last_onset
            .is_none_or(|last| (self.step_start - last) as f64 >= GAP * self.rate);
        if energy < QUIET || energy < RISE * before || !apart {
            return None;
        }
        let peak = self.step.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        let within = self
            .step
            .iter()
            .position(|x| x.abs() >= peak / 2.0)
            .unwrap_or(0);
        let onset = self.step_start + within as u64;
        self.last_onset = Some(onset);
        Some(onset)
    }

    /// The loudness in dB, the pitch and how sure it is, of the frames last heard.
    fn measure(&mut self) -> (f64, Option<f64>, f64) {
        self.window.clear();
        let length = self.history.len();
        self.window.extend(
            self.history[self.at..]
                .iter()
                .chain(&self.history[..self.at]),
        );
        let filled = Ord::min(self.heard, length);
        let squares: f64 = self.window[length - filled..]
            .iter()
            .map(|&x| f64::from(x) * f64::from(x))
            .sum();
        let rms = (squares / Ord::max(filled, 1) as f64).sqrt();
        let loudness = if rms > 0.0 {
            (20.0 * rms.log10()).max(f64::from(FLOOR))
        } else {
            f64::from(FLOOR)
        };
        if filled < length || rms < 1e-5 {
            return (loudness, None, 0.0);
        }
        let (pitch, confidence) = pitch(&self.window, self.rate, &mut self.differences);
        (loudness, pitch, confidence)
    }
}

/// The pitch of `x` and how sure it is, by YIN, de Cheveigné and Kawahara's method (2002): the first lag at which the sound's difference from itself, over the mean difference up to that lag, dips below a threshold, refined between samples by a parabola.
/// `x` holds twice the longest lag and one frame more.
fn pitch(x: &[f32], rate: f64, d: &mut Vec<f64>) -> (Option<f64>, f64) {
    let longest = (x.len() - 1) / 2;
    let shortest = Ord::max((rate / HIGHEST).floor() as usize, 2);
    let width = x.len() - longest - 1;
    d.clear();
    d.push(0.0);
    for lag in 1..=longest + 1 {
        let difference: f64 = (0..width)
            .map(|j| {
                let step = f64::from(x[j]) - f64::from(x[j + lag]);
                step * step
            })
            .sum();
        d.push(difference);
    }
    let mut sum = 0.0;
    d[0] = 1.0;
    for (lag, difference) in d.iter_mut().enumerate().skip(1) {
        sum += *difference;
        *difference = if sum > 0.0 {
            *difference * lag as f64 / sum
        } else {
            1.0
        };
    }
    let dipped = (shortest..longest)
        .find(|&lag| d[lag] < DIP)
        .map(|mut lag| {
            while lag + 1 < longest && d[lag + 1] < d[lag] {
                lag += 1;
            }
            lag
        });
    let lowest = (shortest..longest)
        .min_by(|&a, &b| d[a].total_cmp(&d[b]))
        .unwrap_or(shortest);
    let confidence = (1.0 - d[dipped.unwrap_or(lowest)]).clamp(0.0, 1.0);
    let Some(lag) = dipped else {
        return (None, confidence);
    };
    let (before, at, after) = (d[lag - 1], d[lag], d[lag + 1]);
    let bend = before - 2.0 * at + after;
    let shift = if bend.abs() > 1e-12 {
        0.5 * (before - after) / bend
    } else {
        0.0
    };
    let hz = rate / (lag as f64 + shift);
    ((LOWEST..=HIGHEST).contains(&hz).then_some(hz), confidence)
}
