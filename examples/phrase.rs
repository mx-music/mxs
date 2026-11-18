//! A phrase on the marimba over a bowed bass note, the shortest program that plays something: phrase [out.wav]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example phrase

use mxs::{Engine, Error, Event, RATE, synths};
use std::time::Duration;

const PHRASE: [f64; 8] = [60.0, 64.0, 67.0, 72.0, 71.0, 67.0, 64.0, 60.0];
const STEP: f64 = 0.3;

fn main() -> Result<(), Error> {
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live()?,
    };
    synths::all().into_iter().for_each(|s| engine.register(s));

    // A tenth of a second ahead, so that every voice is built before its time.
    let start = engine.now() + 0.1;
    let mut events: Vec<(f64, Event)> = PHRASE
        .iter()
        .enumerate()
        .map(|(step, &note)| {
            let options = [("note", note), ("vel", 0.7)];
            (
                start + STEP * step as f64,
                Event::note(None, "marimba", &options),
            )
        })
        .collect();
    let length = STEP * PHRASE.len() as f64;
    let bass = [("note", 36.0), ("vel", 0.5), ("sustain", length)];
    events.push((start, Event::note(None, "cello", &bass)));
    engine.play(&events)?;

    finish(&mut engine, path, start + length + 2.0)
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
