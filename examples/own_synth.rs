//! Two synths written here and registered next to the crate's own: own_synth [out.wav]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example own_synth
//!
//! `plate` rings by itself, as a struck thing does.
//! `lead` has an envelope and follows its pitch and its filter, so the phrase slides from note to note under one held note.

use fundsp::prelude::*;
use mxs::nodes::{db, hz, modes};
use mxs::{Engine, Error, Event, Note, RATE, Spec, Synth, SynthDecl, Target};
use std::time::Duration;

/// A metal plate: four resonators set ringing by one impulse, the higher ones fading sooner.
fn plate() -> Synth {
    SynthDecl::new("plate")
        .doc("A metal plate struck with a hard mallet.")
        .option(Spec::note("note").default(60.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the plate is struck."),
        )
        .build(|note, _ctx| {
            let freq = hz(note.number("note"));
            let strike = Net::wrap(Box::new(impulse::<U1>()));
            let struck = modes(
                strike,
                &[freq, freq * 2.76, freq * 5.4, freq * 8.93],
                &[1.0, db(-6.0), db(-12.0), db(-18.0)],
                &[3.0, 1.2, 0.6, 0.3],
            );
            Note::rings(struck * (0.2 * note.number("vel")) as f32, 4.0)
        })
}

/// A sawtooth under a low-pass filter, whose pitch and cutoff a caller can change while a note sounds.
fn lead() -> Synth {
    SynthDecl::new("lead")
        .doc("A sawtooth lead that slides.")
        .option(Spec::note("note").default(57.0).doc("The pitch."))
        .option(
            Spec::hz("cutoff")
                .default(1200.0)
                .range(20.0, 20_000.0)
                .doc("Where the low-pass cuts."),
        )
        .options(Spec::envelope(1.0, 0.2))
        .build(|note, _ctx| {
            let pitch = note.follow("note") >> map(|n: &Frame<f32, U1>| midi_hz(n[0]));
            let filtered = ((pitch >> saw()) | note.follow("cutoff") | dc(1.0)) >> lowpass::<f32>();
            Note::envelope(filtered * 0.15, note)
        })
}

fn main() -> Result<(), Error> {
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live()?,
    };
    engine.register(plate());
    engine.register(lead());

    let start = engine.now() + 0.1;
    let mut events = Vec::new();
    for (beat, note) in [62.0, 69.0, 74.0, 69.0, 65.0, 72.0, 77.0, 72.0]
        .into_iter()
        .enumerate()
    {
        let options = [("note", note), ("vel", 0.8)];
        events.push((
            start + 0.5 * beat as f64,
            Event::note(None, "plate", &options),
        ));
    }

    // One note held for the whole phrase, which each change slides to another pitch and opens a little further.
    let held = [("note", 50.0), ("sustain", 4.0), ("attack", 0.05)];
    events.push((start, Event::note(Some(1), "lead", &held)));
    for (step, (note, cutoff)) in [(53.0, 1800.0), (57.0, 2600.0), (62.0, 4000.0)]
        .into_iter()
        .enumerate()
    {
        let slide = Event::Set {
            target: Target::Sounds(vec![1]),
            options: vec![
                ("note".to_string(), note),
                ("note_slide".to_string(), 0.15),
                ("cutoff".to_string(), cutoff),
                ("cutoff_slide".to_string(), 0.5),
            ],
        };
        events.push((start + 1.0 + step as f64, slide));
    }
    engine.play(&events)?;

    finish(&mut engine, path, start + 6.0)
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
