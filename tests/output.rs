//! Checks that change the system's default output, which need BlackHole 2ch, SwitchAudioSource (`brew install switchaudio-osx`) and a release build; run them by hand with `cargo test --release --test output -- --ignored --test-threads 1`.
//! Each plays a quiet note on the default output for about a second before moving it to BlackHole 2ch, and sets the default output back when it ends.

use fundsp::prelude::*;
use mxs::{Engine, Event, Note, Report, Settings, Source, Synth, Topic, synths};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

const BLACKHOLE: &str = "BlackHole 2ch";

/// The system's default output, set back when this is dropped.
struct Default(String);

impl Default {
    fn kept() -> Default {
        let out = Command::new("SwitchAudioSource")
            .args(["-c", "-t", "output"])
            .output()
            .expect("SwitchAudioSource is installed");
        let current = String::from_utf8(out.stdout).unwrap().trim().to_string();
        assert_ne!(
            current, BLACKHOLE,
            "start from an output other than {BLACKHOLE}"
        );
        Default(current)
    }
}

impl Drop for Default {
    fn drop(&mut self) {
        choose(&self.0);
    }
}

fn choose(output: &str) {
    let status = Command::new("SwitchAudioSource")
        .args(["-t", "output", "-s", output])
        .output()
        .unwrap()
        .status;
    assert!(status.success(), "{output} could not be chosen");
}

fn engine(settings: &Settings) -> Engine {
    let mut engine = Engine::live_with(settings).unwrap();
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.register(click());
    engine
}

fn click() -> Synth {
    Synth::new("click").build(|_, _| Note::rings(dc(0.25), 0.01))
}

/// An engine that plays to BlackHole 2ch and records what reaches it.
fn listener(path: &Path) -> Engine {
    let mut listener = engine(&Settings {
        device: Some(BLACKHOLE.to_string()),
        rate: Some(48_000),
        with_input: true,
        input_device: Some(BLACKHOLE.to_string()),
        ..Settings::default()
    });
    listener
        .record(path, Source::Input(0), listener.now())
        .unwrap();
    listener
}

fn heard(listener: &mut Engine, path: &Path) -> Vec<f32> {
    listener.stop_recording().unwrap();
    let mut reader = hound::WavReader::open(path).unwrap();
    let full = (1i64 << (reader.spec().bits_per_sample - 1)) as f32;
    let samples: Vec<f32> = reader
        .samples::<i32>()
        .map(|s| s.unwrap() as f32 / full)
        .collect();
    let _ = std::fs::remove_file(path);
    samples.chunks(2).map(|frame| frame[0]).collect()
}

/// The system seconds at which `engine` plays its `seconds`, by its clock.
fn played_at(engine: &Engine, seconds: f64) -> f64 {
    let (now, at) = engine.clock();
    let at = at.unwrap().duration_since(SystemTime::UNIX_EPOCH).unwrap();
    at.as_secs_f64() + seconds - now + engine.latency().unwrap()
}

/// The frames at which a click starts after at least 10 ms under it.
fn clicks(samples: &[f32]) -> Vec<usize> {
    let mut found = Vec::new();
    let mut quiet = 480;
    for (frame, sample) in samples.iter().enumerate() {
        if sample.abs() > 0.1 {
            if quiet >= 480 {
                found.push(frame);
            }
            quiet = 0;
        } else {
            quiet += 1;
        }
    }
    found
}

/// The frequency of a steady wave, from where it first and last rises through zero.
fn frequency(samples: &[f32], rate: f64) -> f64 {
    let rises: Vec<f64> = samples
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
        .map(|(i, pair)| i as f64 + (-pair[0] / (pair[1] - pair[0])) as f64)
        .collect();
    let span = rises.last().unwrap() - rises[0];
    (rises.len() - 1) as f64 * rate / span
}

#[test]
#[ignore]
fn a_note_held_on_the_default_output_moves_to_the_new_default_and_the_engine_reports_it() {
    let _default = Default::kept();
    let path = std::env::temp_dir().join(format!("mxs-output-{}.wav", std::process::id()));
    let mut listener = listener(&path);
    let mut engine = engine(&Settings::default());
    let reports = engine.subscribe(&[Topic::Output, Topic::Clock, Topic::Warnings]);
    let held = Event::note(
        None,
        "tri",
        &[("note", 69.0), ("sustain", 4.0), ("amp", 0.005)],
    );
    engine.play(&[(engine.now() + 0.1, held)]).unwrap();
    std::thread::sleep(Duration::from_millis(1000));
    let moved = engine.now();
    let reference = listener.now() + 1.5;
    engine
        .play(&[(moved + 2.0, Event::note(None, "click", &[]))])
        .unwrap();
    listener
        .play(&[(reference, Event::note(None, "click", &[]))])
        .unwrap();
    let switched = SystemTime::now();
    let choosing = Instant::now();
    let chooser = std::thread::spawn(|| choose(BLACKHOLE));
    let (mut output, mut clock, mut warnings) = (None, None, vec![]);
    let mut last = 0.0;
    while choosing.elapsed() < Duration::from_millis(3500) {
        let now = engine.now();
        assert!(
            now >= last,
            "the engine's seconds went back from {last} to {now}"
        );
        last = now;
        while let Some(report) = reports.next() {
            match report {
                Report::Output {
                    device: Some(device),
                    rate,
                    delay_ms,
                    ..
                } if device == BLACKHOLE => output = Some((choosing.elapsed(), rate, delay_ms)),
                Report::Output { device, .. } => {
                    assert!(output.is_none(), "{device:?} reported after {BLACKHOLE}")
                }
                Report::Clock { seconds, .. } if output.is_some() && clock.is_none() => {
                    clock = Some((choosing.elapsed(), seconds))
                }
                Report::Warning { message, .. } => warnings.push(message),
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    chooser.join().unwrap();
    let rate = engine.rate();
    let reference_at = played_at(&listener, reference);
    let predicted = played_at(&engine, moved + 2.0) - reference_at;
    let sound = heard(&mut listener, &path);

    let (reported, reported_rate, delay_ms) = output.expect("a report of the new output");
    assert!(reported < Duration::from_millis(500), "{reported:?}");
    assert_eq!(reported_rate, rate);
    assert_eq!(delay_ms, engine.latency().map(|seconds| seconds * 1000.0));
    let (clocked, _) = clock.expect("a clock report after the move");
    assert!(clocked < Duration::from_secs(1), "{clocked:?}");
    if rate != 48_000.0 {
        let said = format!(
            "{BLACKHOLE} plays at 48000 Hz and the engine at {rate} Hz, so the system converts between them"
        );
        assert!(warnings.contains(&said), "{warnings:?}");
    }

    let found = clicks(&sound);
    assert_eq!(found.len(), 2, "{found:?}");
    let first = sound.iter().position(|s| s.abs() > 0.001).unwrap();
    // The listener's click is heard when its clock says, which gives every frame recorded its system time.
    let arrived = reference_at + (first as f64 - found[0] as f64) / 48_000.0;
    let chosen = switched
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    println!(
        "the note reached {BLACKHOLE} {:.0} ms after it was chosen",
        (arrived - chosen) * 1000.0
    );
    assert!(arrived - chosen < 0.5, "{} s", arrived - chosen);
    let steady = &sound[first + 4_800..found[0] - 2_400];
    let pitch = frequency(steady, 48_000.0);
    assert!((pitch - 440.0).abs() < 1.0, "{pitch} Hz");

    let apart = (found[1] as f64 - found[0] as f64) / 48_000.0;
    println!(
        "the click handed over before the move came {:.1} ms from where the clocks put it",
        (apart - predicted) * 1000.0
    );
    assert!(
        (apart - predicted).abs() < 0.02,
        "{apart} against {predicted}"
    );
}

#[test]
#[ignore]
fn an_engine_started_on_a_named_output_stays_on_it_when_the_default_changes() {
    let _default = Default::kept();
    let path = std::env::temp_dir().join(format!("mxs-named-{}.wav", std::process::id()));
    let mut engine = listener(&path);
    let reports = engine.subscribe(&[Topic::Output]);
    std::thread::sleep(Duration::from_millis(500));
    choose(BLACKHOLE);
    std::thread::sleep(Duration::from_millis(500));
    let at = engine.now() + 0.2;
    engine
        .play(&[(at, Event::note(None, "click", &[]))])
        .unwrap();
    std::thread::sleep(Duration::from_millis(1000));
    let mut outputs = vec![];
    while let Some(report) = reports.next() {
        if let Report::Output { device, .. } = report {
            outputs.push(device);
        }
    }
    assert_eq!(outputs, vec![Some(BLACKHOLE.to_string())]);
    assert_eq!(clicks(&heard(&mut engine, &path)).len(), 1);
}
