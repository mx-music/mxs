//! Plays a short phrase through the default output, scheduled on FunDSP's Sequencer, or writes it to the WAV file given: play [synth] [out.wav]
//! The piano plays it in chords, the berimbau plays four bars of the toque of Angola, and any other synth plays the phrase's top line an octave down.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample};
use fundsp::prelude::*;
use mx_engine::{synth, Opts};


// (beat, note, vel, beats held)
const PHRASE: &[(f64, f64, f64, f64)] = &[
    (0.0, 48.0, 0.5, 4.0), (0.0, 55.0, 0.4, 4.0), (0.0, 64.0, 0.6, 1.0),
    (1.0, 67.0, 0.6, 1.0), (2.0, 72.0, 0.7, 1.0), (3.0, 71.0, 0.6, 1.0),
    (4.0, 45.0, 0.5, 4.0), (4.0, 52.0, 0.4, 4.0), (4.0, 69.0, 0.6, 1.5),
    (5.5, 67.0, 0.5, 0.5), (6.0, 64.0, 0.6, 2.0),
    (8.0, 41.0, 0.5, 4.0), (8.0, 60.0, 0.45, 4.0), (8.0, 65.0, 0.5, 4.0), (8.0, 69.0, 0.55, 4.0), (8.0, 76.0, 0.7, 4.0),
    (12.0, 43.0, 0.5, 4.0), (12.0, 59.0, 0.45, 4.0), (12.0, 62.0, 0.5, 4.0), (12.0, 67.0, 0.55, 4.0), (12.0, 74.0, 0.65, 2.0),
    (14.0, 36.0, 0.6, 6.0), (14.0, 60.0, 0.5, 6.0), (14.0, 64.0, 0.5, 6.0), (14.0, 67.0, 0.55, 6.0), (14.0, 72.0, 0.7, 6.0),
];


fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (synth_name, path) = match args.as_slice() {
        [name, path] => (name.clone(), Some(path.clone())),
        [arg] if arg.ends_with(".wav") => ("piano".to_string(), Some(arg.clone())),
        [name] => (name.clone(), None),
        _ => ("piano".to_string(), None),
    };
    // Each note as its beat, its beats held and its options; mx sends every time option in beats, the synth's default release included.
    let (bpm, notes): (f64, Vec<(f64, f64, Vec<(&str, f64)>)>) = match synth_name.as_str() {
        "piano" => (84.0, PHRASE.iter().map(|&(beat, note, vel, held)| (beat, held, vec![("note", note), ("vel", vel), ("release", 1.0)])).collect()),
        "berimbau" => (119.0, angola()),
        _ => (84.0, top_line().into_iter().map(|(beat, note, vel, held)| (beat, held, vec![("note", note), ("vel", vel), ("release", 0.3)])).collect()),
    };
    let seconds = |beat: f64| beat * 60.0 / bpm;

    let device = cpal::default_host().default_output_device().expect("no output device");
    let config = device.default_output_config().expect("no output config");
    let rate = if path.is_some() { 48_000.0 } else { config.sample_rate() as f64 };
    let channels = config.channels() as usize;
    println!("playing on {} at {rate} Hz", device.description().map(|d| d.to_string()).unwrap_or_default());

    let mut sequencer = Sequencer::new(0, 1, ReplayMode::None);
    sequencer.set_sample_rate(rate);
    let backend = sequencer.backend();

    let lead = 0.2;
    let mut end: f64 = 0.0;
    for (beat, held, options) in notes {
        let mut opts = Opts::default();
        for (name, value) in options {
            let value = if name == "release" { seconds(value) } else { value };
            opts.0.insert(name.to_string(), value);
        }
        let sustain = seconds(held);
        opts.0.insert("sustain".to_string(), sustain);
        // A drum lasts as long as its longest part, 2.9 seconds for the berimbau.
        let length = opts.0.get("release").map_or(2.9, |release| sustain + release);
        let mut unit = synth(&synth_name, &opts, rate).expect("no such synth");
        unit.set_sample_rate(rate);
        let start = lead + seconds(beat);
        let stop = start + length;
        end = end.max(stop);
        sequencer.push(start, stop, Fade::Smooth, 0.0, 0.0, unit);
    }

    if let Some(path) = path {
        let spec = hound::WavSpec { channels: 2, sample_rate: rate as u32, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
        let mut wav = hound::WavWriter::create(&path, spec).unwrap();
        let mut backend = backend;
        for _ in 0..((end + 0.3) * rate) as usize {
            let s = backend.get_mono();
            wav.write_sample(s).unwrap();
            wav.write_sample(s).unwrap();
        }
        wav.finalize().unwrap();
        return;
    }

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => run::<f32>(&device, &config.into(), channels, backend),
        cpal::SampleFormat::I16 => run::<i16>(&device, &config.into(), channels, backend),
        other => panic!("unsupported sample format {other}"),
    };
    stream.play().unwrap();
    std::thread::sleep(std::time::Duration::from_secs_f64(end + 0.3));
}

/// Four bars of the toque of Angola, as mx's examples/berimbau/solo.exs writes it, "O C . xx": the open string, the coin pressed, a rest and two buzzes a sixteenth apart.
fn angola() -> Vec<(f64, f64, Vec<(&'static str, f64)>)> {
    (0..4)
        .flat_map(|bar| {
            let at = bar as f64 * 4.0;
            [
                (at, 1.0, vec![("vel", 0.85)]),
                (at + 1.0, 1.0, vec![("vel", 0.68), ("coin", 1.0)]),
                (at + 3.0, 0.25, vec![("vel", 0.38), ("buzz", 1.0)]),
                (at + 3.25, 0.75, vec![("vel", 0.5), ("buzz", 1.0)]),
            ]
        })
        .collect()
}

/// The phrase's highest note at each beat, an octave down.
fn top_line() -> Vec<(f64, f64, f64, f64)> {
    let mut line: Vec<(f64, f64, f64, f64)> = Vec::new();
    for &(beat, note, vel, held) in PHRASE {
        match line.last_mut() {
            Some(last) if last.0 == beat => {
                if note - 12.0 > last.1 {
                    *last = (beat, note - 12.0, vel, held);
                }
            }
            _ => line.push((beat, note - 12.0, vel, held)),
        }
    }
    line
}

fn run<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    mut backend: impl AudioUnit + 'static,
) -> cpal::Stream {
    device
        .build_output_stream(
            config.clone(),
            move |data: &mut [T], _| {
                for frame in data.chunks_mut(channels) {
                    let s = T::from_sample(backend.get_mono().clamp(-1.0, 1.0));
                    frame.iter_mut().for_each(|x| *x = s);
                }
            },
            |e| eprintln!("stream error: {e}"),
            None,
        )
        .unwrap()
}
