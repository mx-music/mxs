//! Checks of MIDI inputs played on parts inside a live engine, from virtual MIDI sources, which need BlackHole 2ch, so that nothing is heard, and a release build; run them with `cargo test --release --features keys --test midi -- --ignored --test-threads 1`.
#![cfg(feature = "keys")]

use fundsp::prelude::*;
use midir::os::unix::VirtualOutput;
use midir::{MidiOutput, MidiOutputConnection};
use mxs::{Engine, Error, Note, RATE, Report, Reports, Settings, Spec, Synth, Topic, synths};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

const BUFFER: u32 = 128;

fn live() -> Engine {
    let settings = Settings {
        device: Some("BlackHole 2ch".to_string()),
        rate: Some(48_000),
        buffer: Some(BUFFER),
        ..Settings::default()
    };
    let mut engine = Engine::live_with(&settings).unwrap();
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.register(tone("one", 1.0));
    engine.register(tone("two", 3.0));
    engine
}

/// A sine `times` the note's pitch, at its vel, held while its key is down and let go over 0.1 seconds.
fn tone(name: &str, times: f32) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(60.0))
        .option(Spec::float("vel").default(0.5).range(0.0, 1.0))
        .options(Spec::envelope(1.0, 0.1))
        .build(move |o, _| {
            let vel = o.number("vel") as f32;
            let pitch = o.follow("note")
                >> Net::wrap(Box::new(
                    map(move |x: &Frame<f32, U1>| midi_hz(x[0]) * times) >> sine::<f32>(),
                ));
            Note::envelope(pitch * vel, o)
        })
}

/// A virtual MIDI source of a name no other test uses, once the engine can see it.
fn source(name: &str) -> (String, MidiOutputConnection) {
    let name = format!("mxs {name} {}", std::process::id());
    let connection = MidiOutput::new("mxs test")
        .unwrap()
        .create_virtual(&name)
        .unwrap();
    let began = Instant::now();
    while !Engine::midi_inputs().unwrap().contains(&name) {
        assert!(
            began.elapsed() < Duration::from_secs(2),
            "{name} never showed"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    (name, connection)
}

fn hz(note: f64) -> f64 {
    440.0 * 2f64.powf((note - 69.0) / 12.0)
}

/// Records the output from `from`, a time still to come.
fn record(engine: &mut Engine, from: f64, name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("mxs-midi-{name}-{}.wav", std::process::id()));
    engine.record_output(&path, from).unwrap();
    while engine.now() < from + 0.05 {
        std::thread::sleep(Duration::from_millis(5));
    }
    path
}

/// Ends the recording at `path` and gives its left channel.
fn recorded(engine: &mut Engine, path: &PathBuf) -> Vec<f32> {
    engine.stop_recording_output().unwrap();
    let samples: Vec<f32> = hound::WavReader::open(path)
        .unwrap()
        .samples::<f32>()
        .map(|s| s.unwrap())
        .collect();
    let _ = std::fs::remove_file(path);
    samples.chunks(2).map(|frame| frame[0]).collect()
}

/// The magnitude of `samples` at `hz`, as the amplitude of a sine there.
fn magnitude(samples: &[f32], hz: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * hz / RATE;
    let (re, im) = samples
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(re, im), (i, x)| {
            let x = *x as f64;
            (re + x * (w * i as f64).cos(), im - x * (w * i as f64).sin())
        });
    2.0 * (re * re + im * im).sqrt() / samples.len() as f64
}

/// The frequency of a sine, from where it first and last rises through zero.
fn frequency(samples: &[f32]) -> f64 {
    let rises: Vec<f64> = samples
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
        .map(|(i, pair)| i as f64 + (-pair[0] / (pair[1] - pair[0])) as f64)
        .collect();
    (rises.len() - 1) as f64 * RATE / (rises.last().unwrap() - rises[0])
}

fn seconds(from: f64, to: f64) -> std::ops::Range<usize> {
    (from * RATE) as usize..(to * RATE) as usize
}

fn keys(reports: &Reports) -> Vec<Report> {
    std::iter::from_fn(|| reports.next())
        .filter(|report| matches!(report, Report::Key { .. } | Report::InputGone { .. }))
        .collect()
}

/// The largest lateness the warnings give, in milliseconds.
fn most_late(warnings: &[String]) -> f64 {
    warnings
        .iter()
        .filter_map(|w| w.split("after their time, by up to ").nth(1))
        .filter_map(|rest| rest.split(" ms").next()?.parse().ok())
        .fold(0.0, f64::max)
}

fn settle(engine: &mut Engine) {
    std::thread::sleep(Duration::from_millis(300));
    engine.warnings();
}

#[test]
#[ignore]
fn a_key_starts_its_piano_note_within_a_buffer_and_2_ms_of_the_message_and_is_reported() {
    let (name, mut keyboard) = source("key");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(Some(&name), "keys", "piano", &[]).unwrap();
    let reports = engine.subscribe(&[Topic::Keys]);
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "key");
    let sent = SystemTime::now();
    keyboard.send(&[0x90, 60, 100]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    keyboard.send(&[0x80, 60, 40]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let sound = recorded(&mut engine, &path);
    let message = engine.seconds_at(sent).unwrap();
    let onset = sound.iter().position(|s| s.abs() > 1e-4).unwrap();
    let started = from + onset as f64 / RATE;
    let after = (started - message) * 1000.0;
    println!("the note started {after:.2} ms after the message");
    let buffer = BUFFER as f64 / RATE;
    assert!(
        after < (buffer + 0.002) * 1000.0 && after > -0.5,
        "{after} ms"
    );
    match keys(&reports).as_slice() {
        [
            Report::Key {
                seconds: down_at,
                input,
                key: 60,
                velocity: 100,
                down: true,
            },
            Report::Key {
                key: 60,
                velocity: 40,
                down: false,
                ..
            },
        ] => {
            assert_eq!(input, &name);
            let arrived = (down_at - message) * 1000.0;
            println!("its report says it arrived {arrived:.2} ms after it was sent");
            assert!(arrived.abs() <= buffer * 1000.0 + 2.0, "{arrived} ms");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
#[ignore]
fn a_chord_of_ten_piano_keys_starts_all_ten_notes_within_a_buffer_and_2_ms_while_the_builder_is_busy()
 {
    let (name, mut keyboard) = source("chord");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(Some(&name), "keys", "piano", &[]).unwrap();
    let voices = engine.subscribe(&[Topic::Voices]);
    let reports = engine.subscribe(&[Topic::Keys]);
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "chord");
    // Forty notes no ready voice can play keep the builder busy, so a key that waited for it would start late.
    let later = engine.now() + 5.0;
    let busy: Vec<(f64, mxs::Event)> = (0..40)
        .map(|i| {
            let options = [("note", 30.0 + i as f64), ("amp", 0.0)];
            (later, mxs::Event::note(None, "piano", &options))
        })
        .collect();
    engine.play(&busy).unwrap();
    let sent = SystemTime::now();
    for key in [36, 43, 48, 52, 55, 60, 64, 67, 71, 76] {
        keyboard.send(&[0x90, key, 90]).unwrap();
    }
    std::thread::sleep(Duration::from_millis(1500));
    let sound = recorded(&mut engine, &path);
    let message = engine.seconds_at(sent).unwrap();
    let onset = sound.iter().position(|s| s.abs() > 1e-4).unwrap();
    let after = (from + onset as f64 / RATE - message) * 1000.0;
    println!("the chord started {after:.2} ms after its first message");
    let buffer = BUFFER as f64 / RATE * 1000.0;
    assert!(after < buffer + 2.0, "{after} ms");
    // Each note starts as late after its key's arrival as the engine counts it, so none starts later after the message than the last to arrive and the latest.
    let late = most_late(&engine.warnings());
    let arrived = keys(&reports)
        .iter()
        .filter_map(|report| match report {
            Report::Key { seconds, .. } => Some((seconds - message) * 1000.0),
            _ => None,
        })
        .fold(f64::MIN, f64::max);
    println!(
        "its last key arrived {arrived:.2} ms after the first was sent, and its notes were late by up to {late} ms"
    );
    assert!(arrived + late < buffer + 2.0, "{arrived} + {late} ms");
    let most = std::iter::from_fn(|| voices.next())
        .filter_map(|report| match report {
            Report::Voices { sounding, .. } => Some(sounding),
            _ => None,
        })
        .max();
    assert_eq!(most, Some(10));
}

#[test]
#[ignore]
fn the_sustain_pedal_holds_a_note_whose_key_came_up_and_the_wheel_bends_a_held_note() {
    let (name, mut keyboard) = source("pedal");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(Some(&name), "keys", "one", &[]).unwrap();
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "pedal");
    let start = engine.now();
    let at = |seconds: f64| {
        while engine.now() < start + seconds {
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    keyboard.send(&[0x90, 69, 127]).unwrap();
    keyboard.send(&[0xb0, 64, 127]).unwrap();
    at(0.1);
    keyboard.send(&[0x80, 69, 0]).unwrap();
    at(0.4);
    keyboard.send(&[0xe0, 0x7f, 0x7f]).unwrap();
    at(0.7);
    keyboard.send(&[0xb0, 64, 0]).unwrap();
    at(1.0);
    let sound = recorded(&mut engine, &path);
    let offset = start - from;
    let held = frequency(&sound[seconds(offset + 0.2, offset + 0.35)]);
    let bent = frequency(&sound[seconds(offset + 0.5, offset + 0.65)]);
    println!("{held:.2} Hz held by the pedal, {bent:.2} Hz with the wheel at its top");
    assert!((held - 440.0).abs() < 1.0, "{held} Hz");
    let up = hz(69.0 + 2.0 * 8191.0 / 8192.0);
    assert!((bent - up).abs() < 1.0, "{bent} Hz against {up}");
    let after = &sound[seconds(offset + 0.9, offset + 0.95)];
    assert!(
        after.iter().all(|s| *s == 0.0),
        "it sounds once the pedal is up"
    );
}

#[test]
#[ignore]
fn a_muted_part_silences_the_keys_played_on_it() {
    let (name, mut keyboard) = source("mute");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(Some(&name), "keys", "one", &[]).unwrap();
    engine.mute("keys", true, engine.now()).unwrap();
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "mute");
    keyboard.send(&[0x90, 69, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    engine.mute("keys", false, engine.now()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let sound = recorded(&mut engine, &path);
    let muted = &sound[seconds(0.0, 0.15)];
    assert!(muted.iter().all(|s| *s == 0.0), "the muted key sounds");
    let unmuted = magnitude(&sound[seconds(0.3, 0.4)], 440.0);
    assert!(unmuted > 0.1, "{unmuted} once unmuted");
}

#[test]
#[ignore]
fn changing_the_instrument_plays_the_next_key_with_it_and_leaves_a_held_note_ringing() {
    let (name, mut keyboard) = source("change");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(Some(&name), "keys", "one", &[]).unwrap();
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "change");
    keyboard.send(&[0x90, 57, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    engine.change_midi(Some(&name), "two", &[]).unwrap();
    keyboard.send(&[0x90, 69, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    keyboard.send(&[0x80, 57, 0]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let sound = recorded(&mut engine, &path);
    let both = &sound[seconds(0.25, 0.35)];
    let (first, second) = (magnitude(both, hz(57.0)), magnitude(both, 3.0 * hz(69.0)));
    let one = magnitude(both, hz(69.0));
    println!(
        "{first:.3} at the held note's pitch, {second:.3} at three times the next's, {one:.3} at its own"
    );
    assert!(first > 0.1 && second > 0.1 && one < 0.01);
    let after = magnitude(&sound[seconds(0.6, 0.7)], hz(57.0));
    assert!(after < 0.01, "{after} once its key came up");
    engine.change_midi(Some(&name), "piano", &[]).unwrap();
    engine.change_midi(Some(&name), "vibraphone", &[]).unwrap();
    assert_eq!(engine.kept_ready("vibraphone"), 10);
}

#[test]
#[ignore]
fn two_sources_play_two_parts_each_with_its_own_instrument() {
    let (first, mut low) = source("first");
    let (second, mut high) = source("second");
    let mut engine = live();
    engine.start_part("low", &[]).unwrap();
    engine.start_part("high", &[]).unwrap();
    engine.play_midi(Some(&first), "low", "one", &[]).unwrap();
    engine.play_midi(Some(&second), "high", "two", &[]).unwrap();
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "two");
    low.send(&[0x90, 60, 127]).unwrap();
    high.send(&[0x90, 60, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    engine.mute("high", true, engine.now()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let sound = recorded(&mut engine, &path);
    let both = &sound[seconds(0.1, 0.2)];
    let (one, two) = (magnitude(both, hz(60.0)), magnitude(both, 3.0 * hz(60.0)));
    assert!(one > 0.1 && two > 0.1, "{one} and {two}");
    let alone = magnitude(&sound[seconds(0.3, 0.4)], 3.0 * hz(60.0));
    assert!(alone < 0.01, "{alone} once the high part is muted");
}

#[test]
#[ignore]
fn an_input_that_is_not_connected_is_refused_naming_it() {
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    match engine.play_midi(Some("No Such Keyboard"), "keys", "piano", &[]) {
        Err(Error::Midi(message)) => {
            assert!(
                message.starts_with("no MIDI input named No Such Keyboard; "),
                "{message}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
#[ignore]
fn closing_a_source_while_a_key_is_down_lets_its_note_go_reports_it_and_the_engine_plays_on() {
    let (gone, mut leaving) = source("leaving");
    let (staying, mut other) = source("staying");
    let mut engine = live();
    engine.start_part("leaving", &[]).unwrap();
    engine.start_part("staying", &[]).unwrap();
    engine
        .play_midi(Some(&gone), "leaving", "one", &[])
        .unwrap();
    engine
        .play_midi(Some(&staying), "staying", "two", &[])
        .unwrap();
    let reports = engine.subscribe(&[Topic::Keys]);
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "gone");
    leaving.send(&[0x90, 57, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    drop(leaving);
    std::thread::sleep(Duration::from_millis(500));
    other.send(&[0x90, 60, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let sound = recorded(&mut engine, &path);
    let ringing = magnitude(&sound[seconds(0.0, 0.08)], hz(57.0));
    let after = magnitude(&sound[seconds(0.45, 0.55)], hz(57.0));
    let played = magnitude(&sound[seconds(0.65, 0.75)], 3.0 * hz(60.0));
    println!("{ringing:.3} before the source closed, {after:.4} after, {played:.3} from the other");
    assert!(ringing > 0.1 && after < 0.001 && played > 0.1);
    let gone_at = keys(&reports).into_iter().find_map(|report| match report {
        Report::InputGone { input, seconds } => Some((input, seconds)),
        _ => None,
    });
    assert_eq!(gone_at.map(|(input, _)| input), Some(gone));
}

#[test]
fn an_offline_engine_plays_no_midi_input() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.start_part("keys", &[]).unwrap();
    assert_eq!(
        engine.play_midi(None, "keys", "piano", &[]),
        Err(Error::Offline)
    );
}

#[test]
#[ignore]
fn every_input_connected_plays_when_none_is_named_and_stopping_lets_go_of_its_notes() {
    let (_name, mut keyboard) = source("every");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(None, "keys", "one", &[]).unwrap();
    settle(&mut engine);
    let from = engine.now() + 0.05;
    let path = record(&mut engine, from, "every");
    keyboard.send(&[0x90, 69, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    engine.stop_midi(None).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    keyboard.send(&[0x90, 72, 127]).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    let sound = recorded(&mut engine, &path);
    let held = magnitude(&sound[seconds(0.05, 0.15)], 440.0);
    let after = &sound[seconds(0.4, 0.55)];
    println!("{held:.3} while the key is down");
    assert!(held > 0.1, "{held}");
    assert!(
        after.iter().all(|s| *s == 0.0),
        "a note sounds once the input is stopped"
    );
    assert!(matches!(engine.stop_midi(None), Err(Error::Midi(_))));
}

#[test]
#[ignore]
fn a_piano_key_played_with_the_pedal_down_starts_from_a_ready_voice_while_the_builder_is_busy() {
    let (name, mut keyboard) = source("pedal down");
    let mut engine = live();
    engine.start_part("keys", &[]).unwrap();
    engine.play_midi(Some(&name), "keys", "piano", &[]).unwrap();
    keyboard.send(&[0xb0, 64, 127]).unwrap();
    settle(&mut engine);
    let later = engine.now() + 5.0;
    let busy: Vec<(f64, mxs::Event)> = (0..40)
        .map(|i| {
            let options = [("note", 30.0 + i as f64), ("amp", 0.0)];
            (later, mxs::Event::note(None, "piano", &options))
        })
        .collect();
    engine.play(&busy).unwrap();
    keyboard.send(&[0x90, 55, 80]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let late = most_late(&engine.warnings());
    println!("the key's note was late by up to {late} ms");
    assert_eq!(late, 0.0);
    assert_eq!(engine.kept_ready("piano"), 10);
}
