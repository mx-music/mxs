//! Music handed over in beats, a bar at a time, while the tempo slows under it: beats [out.wav]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example beats
//!
//! The program hands the engine one bar, and each next bar when the engine reports that the last is about to end.
//! The tempo falls from 132 to 84 beats a minute over the middle bars, and the echo, whose time is given in beats, follows it.

use mxs::{Engine, Error, Event, Pass, RATE, Report, Tempo, Topic, Value, effects, synths};
use std::time::Duration;

const BARS: u64 = 8;
/// The bass's note for each bar, over and over.
const ROOTS: [f64; 4] = [38.0, 41.0, 36.0, 43.0];
/// The son clave in eighth notes, three strokes in one bar and two in the next.
const CLAVE: [&[f64]; 2] = [&[0.0, 1.5, 3.0], &[1.0, 2.0]];

fn main() -> Result<(), Error> {
    let path = std::env::args().nth(1);
    let offline = path.is_some();
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live()?,
    };
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    if let Some(path) = &path {
        engine.record_output(path, 0.0)?;
    }

    // Three quarters of a beat between the echo's repeats, whatever the tempo.
    let echo = [("phase", 0.75), ("mix", 0.3)].map(|(k, v)| (k.to_string(), Value::Number(v)));
    engine.start_part_in_beats("band", &[("echo".to_string(), echo.to_vec())])?;

    engine.place_beats(engine.now() + 0.2)?;
    engine.set_tempo(0.0, Tempo::Hold(132.0))?;
    let slowing = Tempo::Move {
        from: 132.0,
        to: 84.0,
        over: 12.0,
    };
    engine.set_tempo(8.0, slowing)?;

    let reports = engine.subscribe(&[Topic::Passes]);
    engine.pass("band", &bar(0))?;
    let end = (BARS * 4) as f64 + 2.0;
    while engine.beat() < end {
        let report = if offline {
            engine.render(0.02)?;
            reports.next()
        } else {
            reports.wait(Duration::from_millis(20))
        };
        match report {
            Some(Report::PassEnding { beat, .. }) if beat < (BARS * 4) as f64 => {
                engine.pass("band", &bar(beat as u64 / 4))?;
            }
            Some(Report::PassStarted { label, seconds, .. }) => {
                println!("bar {label} starts at {seconds:.2} s");
            }
            _ => {}
        }
    }
    engine.stop_recording_output()
}

/// One bar of four beats, as a pass that asks to be told a beat before it ends.
fn bar(index: u64) -> Pass {
    let from = (index * 4) as f64;
    let mut events = Vec::new();
    for &beat in CLAVE[index as usize % 2] {
        events.push((from + beat, Event::note(None, "clave", &[("vel", 0.7)])));
    }
    let root = ROOTS[index as usize % 4];
    for (beat, note) in [(0.0, root), (1.5, root + 7.0), (2.5, root + 12.0)] {
        // The sustain is in beats, as the bar is, so it shortens and lengthens with the tempo.
        let options = [("note", note), ("vel", 0.8), ("sustain", 0.75)];
        events.push((from + beat, Event::note(None, "electric_bass", &options)));
    }
    Pass {
        from,
        to: from + 4.0,
        label: index,
        notice: Some(1.0),
        events,
    }
}
