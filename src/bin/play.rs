//! Plays a short phrase through the default output through the engine, or writes it to the WAV file given: play [synth] [out.wav]
//! The piano plays it in chords, the berimbau plays four bars of the toque of Angola, and any other synth plays the phrase's top line an octave down.

use mx_engine::{Engine, Event, RATE, Value, synths};

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
    // Each note as its beat, its beats held and its options; mx sends every time option in seconds, the synth's default release included.
    let (bpm, notes): (f64, Vec<(f64, f64, Vec<(&str, f64)>)>) = match synth_name.as_str() {
        "piano" => (84.0, PHRASE.iter().map(|&(beat, note, vel, held)| (beat, held, vec![("note", note), ("vel", vel), ("release", 1.0)])).collect()),
        "berimbau" => (119.0, angola()),
        _ => (84.0, top_line().into_iter().map(|(beat, note, vel, held)| (beat, held, vec![("note", note), ("vel", vel), ("release", 0.3)])).collect()),
    };
    let seconds = |beat: f64| beat * 60.0 / bpm;
    let drum = synth_name == "berimbau";

    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live().unwrap_or_else(|e| panic!("{e}")),
    };
    synths::all().into_iter().for_each(|s| engine.register(s));
    let lead = engine.now() + 0.2;
    let mut end: f64 = 0.0;
    let events: Vec<(f64, Event)> = notes
        .into_iter()
        .map(|(beat, held, options)| {
            let mut options: Vec<(String, Value)> = options
                .into_iter()
                .map(|(name, value)| {
                    let value = if name == "release" { seconds(value) } else { value };
                    (name.to_string(), Value::Number(value))
                })
                .collect();
            if !drum {
                options.push(("sustain".to_string(), Value::Number(seconds(held))));
            }
            let start = lead + seconds(beat);
            end = end.max(start + seconds(held) + 3.0);
            (start, Event::Note { id: None, synth: synth_name.clone(), options, chain: None })
        })
        .collect();
    engine.play(&events).unwrap_or_else(|e| panic!("{e}"));

    if let Some(path) = path {
        let (left, right) = engine.render(end).unwrap();
        let spec = hound::WavSpec { channels: 2, sample_rate: RATE as u32, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
        let mut wav = hound::WavWriter::create(&path, spec).unwrap();
        for (l, r) in left.into_iter().zip(right) {
            wav.write_sample(l).unwrap();
            wav.write_sample(r).unwrap();
        }
        wav.finalize().unwrap();
        return;
    }
    println!("playing at {} Hz", engine.rate());
    std::thread::sleep(std::time::Duration::from_secs_f64(end));
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
