//! Two parts, each with its own effects, and changes to both while they play: parts [out.wav]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example parts
//!
//! The chords play into a low-pass filter that opens over the first bars, and the vibraphone into an echo and a reverb.
//! The chords are muted for the third bar and turned down for the last.

use mxs::{Engine, Error, Event, RATE, Target, Value, effects, synths};
use std::time::Duration;

const BAR: f64 = 2.0;
const CHORDS: [[f64; 3]; 4] = [
    [50.0, 57.0, 65.0],
    [48.0, 55.0, 64.0],
    [46.0, 53.0, 62.0],
    [45.0, 52.0, 61.0],
];
const MELODY: [f64; 4] = [77.0, 76.0, 74.0, 73.0];

fn main() -> Result<(), Error> {
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live()?,
    };
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));

    let chords = engine.start_part("chords", &[effect("lpf", &[("cutoff", 60.0)])])?;
    let echo = effect("echo", &[("phase", BAR / 8.0 * 3.0), ("mix", 0.4)]);
    let reverb = effect("reverb", &[("decay_seconds", 3.0), ("mix", 0.5)]);
    let lead = engine.start_part("lead", &[echo, reverb])?;

    let start = engine.now() + 0.1;
    let mut events = Vec::new();
    for (bar, (chord, melody)) in CHORDS.iter().zip(MELODY).enumerate() {
        let at = start + BAR * bar as f64;
        for &note in chord {
            let options = [("note", note), ("sustain", BAR * 0.9), ("amp", 0.2)];
            events.push((at, note_into(chords, "saw", &options)));
        }
        let options = [("note", melody), ("vel", 0.8)];
        events.push((at + BAR / 2.0, note_into(lead, "vibraphone", &options)));
    }
    // The filter is the first effect of the chords' chain, and opens over two bars.
    let open = Event::Set {
        target: Target::Effect(chords, 0),
        options: vec![
            ("cutoff".to_string(), 110.0),
            ("cutoff_slide".to_string(), BAR * 2.0),
        ],
    };
    events.push((start, open));
    engine.play(&events)?;

    engine.mute("chords", true, start + BAR * 2.0)?;
    engine.mute("chords", false, start + BAR * 3.0)?;
    engine.set_gain("chords", 0.4, start + BAR * 3.0)?;

    finish(&mut engine, path, start + BAR * 4.0 + 5.0)
}

/// An effect and its options, as `start_part` takes them.
fn effect(name: &str, options: &[(&str, f64)]) -> (String, Vec<(String, Value)>) {
    let options = options
        .iter()
        .map(|(option, value)| (option.to_string(), Value::Number(*value)));
    (name.to_string(), options.collect())
}

/// A note that plays into a part.
fn note_into(part: u64, synth: &str, options: &[(&str, f64)]) -> Event {
    let options = options
        .iter()
        .map(|(option, value)| (option.to_string(), Value::Number(*value)));
    Event::Note {
        id: None,
        synth: synth.to_string(),
        options: options.collect(),
        chain: Some(part),
        held: false,
    }
}

/// Lets a live engine play for `seconds`, or renders them into the WAV file given.
fn finish(engine: &mut Engine, path: Option<String>, seconds: f64) -> Result<(), Error> {
    match path {
        Some(path) => {
            engine.record_output(&path, 0.0)?;
            engine.render(seconds)?;
            engine.stop_recording_output()
        }
        None => {
            std::thread::sleep(Duration::from_secs_f64(seconds));
            Ok(())
        }
    }
}
