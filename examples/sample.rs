//! A sound file played at several rates, forwards and backwards: sample [out.wav] [file]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example sample
//! cargo run --release --example sample -- out.wav voice.flac
//!
//! Without a file it first renders a vibraphone note into one, and plays that.

use mxs::{Engine, Error, Event, RATE, Value, check_sample, synths};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Each time the file plays: how fast, and where it sits from left to right.
const TAKES: [(f64, f64); 4] = [(1.0, 0.0), (0.5, -0.6), (1.5, 0.6), (-1.0, 0.0)];

fn main() -> Result<(), Error> {
    let mut args = std::env::args().skip(1);
    let (path, file) = (args.next(), args.next());
    let file = match file {
        Some(file) => PathBuf::from(file),
        None => rendered_note()?,
    };
    let length = check_sample(&file).map_err(Error::Sample)?;

    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live()?,
    };
    let sample = engine.load_sample(&file)?;

    let mut at = engine.now() + 0.1;
    let mut events = Vec::new();
    for (rate, pan) in TAKES {
        let options = [
            ("rate", rate),
            ("pan", pan),
            ("normalize", 1.0),
            ("amp", 0.5),
        ]
        .map(|(option, value)| (option.to_string(), Value::Number(value)));
        let take = Event::Sample {
            id: None,
            sample,
            options: options.to_vec(),
            chain: None,
        };
        events.push((at, take));
        at += length / rate.abs() + 0.2;
    }
    engine.play(&events)?;

    finish(&mut engine, path, at)
}

/// Two seconds of a vibraphone note, rendered offline into a WAV file among the temporary files.
fn rendered_note() -> Result<PathBuf, Error> {
    let file = std::env::temp_dir().join("mxs-example-sample.wav");
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.record_output(Path::new(&file), 0.0)?;
    let options = [("note", 62.0), ("vel", 0.8), ("motor", 5.0)];
    engine.play(&[(0.0, Event::note(None, "vibraphone", &options))])?;
    engine.render(2.0)?;
    engine.stop_recording_output()?;
    Ok(file)
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
