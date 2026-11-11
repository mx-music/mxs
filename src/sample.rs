//! Samples loaded from FLAC or WAV files, and the player that reads them at a rate.

use fundsp::prelude::*;
use std::path::Path;
use std::sync::Arc;

pub(crate) struct SampleData {
    pub channels: Vec<Vec<f32>>,
    pub rate: f64,
}

impl SampleData {
    pub fn load(path: &Path) -> Result<SampleData, String> {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        match extension.as_str() {
            "flac" => load_flac(path),
            "wav" => load_wav(path),
            _ => Err(format!("{} is not a FLAC or WAV file", path.display())),
        }
    }

    pub fn frames(&self) -> usize {
        self.channels[0].len()
    }
}

fn load_flac(path: &Path) -> Result<SampleData, String> {
    let mut reader = claxon::FlacReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let info = reader.streaminfo();
    let count = info.channels as usize;
    let scale = 1.0 / (1u64 << (info.bits_per_sample - 1)) as f32;
    let mut channels = vec![Vec::new(); count];
    for (i, sample) in reader.samples().enumerate() {
        let sample = sample.map_err(|e| format!("{}: {e}", path.display()))?;
        channels[i % count].push(sample as f32 * scale);
    }
    Ok(SampleData { channels, rate: info.sample_rate as f64 })
}

fn load_wav(path: &Path) -> Result<SampleData, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    let count = spec.channels as usize;
    let values: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            reader.samples::<i32>().map(|s| s.map(|s| s as f32 * scale)).collect::<Result<_, _>>()
        }
    }
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut channels = vec![Vec::with_capacity(values.len() / count); count];
    for (i, value) in values.into_iter().enumerate() {
        channels[i % count].push(value);
    }
    Ok(SampleData { channels, rate: spec.sample_rate as f64 })
}

/// Reads a sample from `from` toward `to`, fractions of its length, at the rate its input gives, backwards when the rate or the span is reversed, and silent outside the span.
/// It always has two outputs; a mono sample gives the same frames on both.
#[derive(Clone)]
pub(crate) struct Player {
    data: Arc<SampleData>,
    position: f64,
    low: f64,
    high: f64,
    direction: f64,
    step: f64,
}

impl Player {
    pub fn new(data: Arc<SampleData>, from: f64, to: f64, rate: f64) -> Self {
        let frames = data.frames() as f64;
        let (from, to) = (from.clamp(0.0, 1.0) * frames, to.clamp(0.0, 1.0) * frames);
        let (low, high) = (from.min(to), from.max(to));
        let direction = if to >= from { 1.0 } else { -1.0 };
        let position = if (to - from) * rate >= 0.0 { low } else { high };
        Player { step: data.rate / DEFAULT_SR, data, position, low, high, direction }
    }

    fn at(&self, channel: usize) -> f32 {
        let samples = &self.data.channels[Ord::min(channel, self.data.channels.len() - 1)];
        let i = self.position.floor();
        let fraction = (self.position - i) as f32;
        let i = i as usize;
        let a = samples.get(i).copied().unwrap_or(0.0);
        let b = samples.get(i + 1).copied().unwrap_or(0.0);
        a + (b - a) * fraction
    }
}

impl AudioNode for Player {
    const ID: u64 = 0x6d78_0004;
    type Inputs = U1;
    type Outputs = U2;

    fn set_sample_rate(&mut self, rate: f64) {
        self.step = self.data.rate / rate;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U2> {
        let inside = self.position >= self.low && self.position < self.high;
        let frame = if inside { [self.at(0), self.at(1)] } else { [0.0, 0.0] };
        self.position += input[0] as f64 * self.step * self.direction;
        frame.into()
    }
}
