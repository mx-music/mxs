//! Samples loaded from FLAC, WAV or AIFF files, and the player that reads them at a rate.

use fundsp::prelude::*;
use std::path::Path;
use std::sync::Arc;

pub(crate) struct SampleData {
    pub channels: Vec<Vec<f32>>,
    pub rate: f64,
    /// The loudest sample in any channel, as a level from 0.
    pub peak: f32,
}

impl SampleData {
    pub fn load(path: &Path) -> Result<SampleData, String> {
        let shown = path.display();
        let bytes = std::fs::read(path).map_err(|e| format!("{shown}: {e}"))?;
        let data = match Format::of(&bytes) {
            Some(Format::Flac) => load_flac(path),
            Some(Format::Wav) => wav(&bytes).and_then(|pcm| pcm.decode(&bytes)),
            Some(Format::Aiff) => aiff(&bytes).and_then(|pcm| pcm.decode(&bytes)),
            None => Err("it is not a FLAC, WAV or AIFF file".to_string()),
        };
        let data = data.map_err(|e| if e.starts_with(&shown.to_string()) { e } else { format!("{shown}: {e}") })?;
        let peak = data.channels.iter().flatten().fold(0.0f32, |peak, x| peak.max(x.abs()));
        Ok(SampleData { peak, ..data })
    }

    /// A WAV file's samples, from its bytes, for recordings built into mxs.
    pub fn wav(bytes: &[u8]) -> Result<SampleData, String> {
        let data = wav(bytes).and_then(|pcm| pcm.decode(bytes))?;
        let peak = data.channels.iter().flatten().fold(0.0f32, |peak, x| peak.max(x.abs()));
        Ok(SampleData { peak, ..data })
    }

    pub fn frames(&self) -> usize {
        self.channels[0].len()
    }
}

/// The length in seconds of the file at `path`, reading only its headers, or why mx cannot play it.
pub fn check(path: &Path) -> Result<f64, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match Format::of(&bytes) {
        Some(Format::Flac) => {
            let info = claxon::FlacReader::open(path).map_err(|e| e.to_string())?.streaminfo();
            Ok(info.samples.unwrap_or(0) as f64 / info.sample_rate as f64)
        }
        Some(Format::Wav) => wav(&bytes).map(|pcm| pcm.seconds()),
        Some(Format::Aiff) => aiff(&bytes).map(|pcm| pcm.seconds()),
        None => Err("it is not a FLAC, WAV or AIFF file".to_string()),
    }
}

/// The loudest sample of the file at `path`, as a level from 0, read from all of it.
pub fn peak(path: &Path) -> Result<f32, String> {
    SampleData::load(path).map(|data| data.peak)
}

enum Format {
    Flac,
    Wav,
    Aiff,
}

impl Format {
    fn of(bytes: &[u8]) -> Option<Format> {
        match (bytes.get(0..4), bytes.get(8..12)) {
            (Some(b"fLaC"), _) => Some(Format::Flac),
            (Some(b"RIFF"), Some(b"WAVE")) => Some(Format::Wav),
            (Some(b"FORM"), Some(b"AIFF" | b"AIFC")) => Some(Format::Aiff),
            _ => None,
        }
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
    Ok(SampleData { channels, rate: info.sample_rate as f64, peak: 0.0 })
}

#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    /// Signed integers, or unsigned ones for 8-bit WAV, in this many bytes each.
    Int { bytes: usize, unsigned: bool },
    Float { bytes: usize },
}

/// Where a file's sample frames lie and how they are written.
struct Pcm {
    channels: usize,
    rate: f64,
    encoding: Encoding,
    big_endian: bool,
    start: usize,
    length: usize,
}

impl Pcm {
    fn width(&self) -> usize {
        match self.encoding {
            Encoding::Int { bytes, .. } | Encoding::Float { bytes } => bytes,
        }
    }

    fn seconds(&self) -> f64 {
        (self.length / (self.width() * self.channels)) as f64 / self.rate
    }

    fn decode(&self, bytes: &[u8]) -> Result<SampleData, String> {
        let width = self.width();
        let frames = self.length / (width * self.channels);
        let mut channels = vec![Vec::with_capacity(frames); self.channels];
        for (i, chunk) in bytes[self.start..self.start + frames * width * self.channels].chunks_exact(width).enumerate() {
            channels[i % self.channels].push(self.value(chunk));
        }
        Ok(SampleData { channels, rate: self.rate, peak: 0.0 })
    }

    fn value(&self, chunk: &[u8]) -> f32 {
        let mut word = [0u8; 8];
        if self.big_endian {
            word[8 - chunk.len()..].copy_from_slice(chunk);
            word.reverse();
        } else {
            word[..chunk.len()].copy_from_slice(chunk);
        }
        let raw = u64::from_le_bytes(word);
        match self.encoding {
            Encoding::Float { bytes: 4 } => f32::from_bits(raw as u32),
            Encoding::Float { .. } => f64::from_bits(raw) as f32,
            Encoding::Int { bytes, unsigned } => {
                let bits = 8 * bytes as u32;
                // Shifted to the top of 64 bits and back, so the sign extends.
                let signed = if unsigned { raw as i64 - (1i64 << (bits - 1)) } else { ((raw << (64 - bits)) as i64) >> (64 - bits) };
                (signed as f64 / (1u64 << (bits - 1)) as f64) as f32
            }
        }
    }
}

/// Each chunk of a RIFF or IFF file after its 12-byte header: its id, where its body starts and its length, kept inside the file.
fn chunks(bytes: &[u8], big_endian: bool) -> Vec<(&[u8], usize, usize)> {
    let mut found = Vec::new();
    let mut at = 12;
    while at + 8 <= bytes.len() {
        let size = bytes[at + 4..at + 8].try_into().map(|b| if big_endian { u32::from_be_bytes(b) } else { u32::from_le_bytes(b) }).unwrap() as usize;
        let length = Ord::min(size, bytes.len() - at - 8);
        found.push((&bytes[at..at + 4], at + 8, length));
        at += 8 + size + (size & 1);
    }
    found
}

fn wav(bytes: &[u8]) -> Result<Pcm, String> {
    let chunks = chunks(bytes, false);
    let (_, fmt, size) = *chunks.iter().find(|c| c.0 == b"fmt ").ok_or("it has no format chunk")?;
    if size < 16 {
        return Err(format!("its format chunk is {size} bytes, under the 16 it needs"));
    }
    let u16_at = |i: usize| u16::from_le_bytes([bytes[fmt + i], bytes[fmt + i + 1]]);
    let mut tag = u16_at(0);
    // WAVE_FORMAT_EXTENSIBLE names the real format in the first two bytes of its subformat.
    if tag == 0xfffe && size >= 26 {
        tag = u16_at(24);
    }
    let channels = u16_at(2) as usize;
    let rate = u32::from_le_bytes(bytes[fmt + 4..fmt + 8].try_into().unwrap()) as f64;
    let align = u16_at(12) as usize;
    if channels == 0 || align == 0 || align % channels != 0 {
        return Err(format!("it has {channels} channels in frames of {align} bytes"));
    }
    let width = align / channels;
    let encoding = match (tag, width) {
        (1, 1) => Encoding::Int { bytes: 1, unsigned: true },
        (1, 2..=4) => Encoding::Int { bytes: width, unsigned: false },
        (3, 4 | 8) => Encoding::Float { bytes: width },
        _ => return Err(format!("it holds format {tag} in {width}-byte samples, where mx reads PCM in 1 to 4 bytes and float in 4 or 8")),
    };
    let (_, start, length) = *chunks.iter().find(|c| c.0 == b"data").ok_or("it has no data chunk")?;
    Ok(Pcm { channels, rate, encoding, big_endian: false, start, length })
}

fn aiff(bytes: &[u8]) -> Result<Pcm, String> {
    let compressed = &bytes[8..12] == b"AIFC";
    let chunks = chunks(bytes, true);
    let (_, comm, size) = *chunks.iter().find(|c| c.0 == b"COMM").ok_or("it has no COMM chunk")?;
    if size < 18 || (compressed && size < 22) {
        return Err(format!("its COMM chunk is {size} bytes, too short"));
    }
    let channels = u16::from_be_bytes([bytes[comm], bytes[comm + 1]]) as usize;
    let bits = u16::from_be_bytes([bytes[comm + 6], bytes[comm + 7]]) as usize;
    let rate = extended(&bytes[comm + 8..comm + 18]);
    let kind = if compressed { &bytes[comm + 18..comm + 22] } else { b"NONE" };
    let (encoding, big_endian) = match kind {
        b"NONE" | b"twos" if (1..=32).contains(&bits) => (Encoding::Int { bytes: bits.div_ceil(8), unsigned: false }, true),
        b"sowt" if (1..=32).contains(&bits) => (Encoding::Int { bytes: bits.div_ceil(8), unsigned: false }, false),
        b"fl32" | b"FL32" => (Encoding::Float { bytes: 4 }, true),
        b"fl64" | b"FL64" => (Encoding::Float { bytes: 8 }, true),
        _ => return Err(format!("it holds {}-bit samples compressed as {}, which mx does not read", bits, String::from_utf8_lossy(kind))),
    };
    if channels == 0 || !(1.0..=1.0e6).contains(&rate) {
        return Err(format!("it has {channels} channels at {rate} Hz"));
    }
    let (_, ssnd, length) = *chunks.iter().find(|c| c.0 == b"SSND").ok_or("it has no SSND chunk")?;
    let offset = if length >= 8 { u32::from_be_bytes(bytes[ssnd..ssnd + 4].try_into().unwrap()) as usize } else { 0 };
    let skip = Ord::min(8 + offset, length);
    Ok(Pcm { channels, rate, encoding, big_endian, start: ssnd + skip, length: length - skip })
}

/// An 80-bit IEEE 754 extended float, as AIFF gives its sample rate.
fn extended(b: &[u8]) -> f64 {
    let exponent = (u16::from_be_bytes([b[0], b[1]]) & 0x7fff) as i32;
    let mantissa = u64::from_be_bytes(b[2..10].try_into().unwrap());
    let sign = if b[0] & 0x80 != 0 { -1.0 } else { 1.0 };
    if exponent == 0 && mantissa == 0 { 0.0 } else { sign * mantissa as f64 * 2f64.powi(exponent - 16383 - 63) }
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
