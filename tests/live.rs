//! Checks of a live engine, which need BlackHole 2ch, a virtual sound card, so that nothing is heard, and a release build, as a debug one fills too slowly; run them with `cargo test --release --test live -- --ignored --test-threads 1`.

use fundsp::prelude::*;
use mxs::{Engine, Error, Event, Note, Settings, Spec, Synth, Target, synths};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

fn live(buffer: u32) -> Engine {
    let settings = Settings {
        device: Some("BlackHole 2ch".to_string()),
        rate: Some(48_000),
        buffer: Some(buffer),
        ..Settings::default()
    };
    let mut engine = Engine::live_with(&settings).unwrap();
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
}

/// A silent synth that holds the audio thread for 20 ms once, 50 ms into its note.
fn stall() -> Synth {
    Synth::new("stall").build(|_, _| {
        let stalled = Arc::new(AtomicBool::new(false));
        let sound = envelope(move |t: f32| {
            if t > 0.05 && !stalled.swap(true, Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(20));
            }
            0.0
        });
        Note::rings(sound, 0.5)
    })
}

#[test]
#[ignore]
fn a_buffer_held_for_20_ms_is_counted_late_and_puts_the_clock_behind() {
    let mut engine = live(512);
    engine.register(stall());
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(engine.load().late, 0);
    let before = engine.behind().unwrap();
    engine
        .play(&[(engine.now() + 0.1, Event::note(None, "stall", &[]))])
        .unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    let load = engine.load();
    assert_eq!(load.late, 1, "{load:?}");
    assert!(load.most_over > 0.008 && load.most_over < 0.012, "{load:?}");
    assert!(load.worst > 1.8, "{load:?}");
    let fell = engine.behind().unwrap() - before;
    println!("{load:?}, the clock fell {:.1} ms behind", fell * 1000.0);
    let warnings = engine.warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("1 buffer reached the sound card late, by up to "),
        "{warnings:?}"
    );
    assert!(engine.warnings().is_empty());
}

#[test]
#[ignore]
fn eight_bars_of_piano_reach_the_sound_card_with_no_buffer_late() {
    let mut engine = live(512);
    std::thread::sleep(Duration::from_millis(500));
    engine.load();
    let start = engine.now() + 0.2;
    let notes: Vec<(f64, Event)> = (0..64)
        .map(|i| {
            let note = 48.0 + [0.0, 4.0, 7.0, 12.0][i % 4] + (i / 16) as f64 * 2.0;
            (
                start + i as f64 * 0.125,
                Event::note(None, "piano", &[("note", note)]),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    std::thread::sleep(Duration::from_secs(10));
    let load = engine.load();
    println!(
        "{load:?}, the clock is {:.2} ms behind",
        engine.behind().unwrap() * 1000.0
    );
    assert!(load.buffers > 800, "{load:?}");
    assert_eq!(load.late, 0, "{load:?}");
    assert!(load.worst < 1.0, "{load:?}");
    assert!(engine.warnings().iter().all(|w| !w.contains("buffer")));
}

/// Ten minutes; run it alone with `cargo test --release --test live ten_minutes -- --ignored --nocapture`.
#[test]
#[ignore]
fn ten_minutes_of_piano_reach_the_sound_card_with_no_buffer_late() {
    let mut engine = live(512);
    std::thread::sleep(Duration::from_millis(500));
    engine.load();
    let (mut late, mut worst, mut buffers) = (0, 0.0f64, 0);
    for bar in 0..300 {
        let start = engine.now() + 0.2;
        let notes: Vec<(f64, Event)> = (0..16)
            .map(|i| {
                let note = 48.0 + [0.0, 4.0, 7.0, 12.0][i % 4] + (bar % 4) as f64 * 2.0;
                (
                    start + i as f64 * 0.125,
                    Event::note(None, "piano", &[("note", note)]),
                )
            })
            .collect();
        engine.play(&notes).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        let load = engine.load();
        late += load.late;
        worst = worst.max(load.worst);
        buffers += load.buffers;
    }
    println!(
        "{buffers} buffers, {late} late, worst share {worst:.3}, the clock is {:.2} ms behind, warnings {:?}",
        engine.behind().unwrap() * 1000.0,
        engine.warnings()
    );
    assert_eq!(late, 0);
}

/// A synth whose sound is its `level` option, held, for a second.
fn level() -> Synth {
    Synth::new("level")
        .option(Spec::float("level").default(1.0).range(0.0, 2.0))
        .build(|options, _| Note::rings(options.follow("level"), 1.0))
}

/// The left channel of what a live engine plays for `seconds` from `from`, a time still to come.
fn recorded(engine: &mut Engine, from: f64, seconds: f64, name: &str) -> Vec<f32> {
    let path = std::env::temp_dir().join(format!("mxs-live-{name}-{}.wav", std::process::id()));
    engine.record_output(&path, from).unwrap();
    while engine.now() < from + seconds + 0.1 {
        std::thread::sleep(Duration::from_millis(10));
    }
    engine.stop_recording_output().unwrap();
    let samples: Vec<f32> = hound::WavReader::open(&path)
        .unwrap()
        .samples::<f32>()
        .map(|s| s.unwrap())
        .collect();
    let _ = std::fs::remove_file(&path);
    samples.chunks(2).map(|frame| frame[0]).collect()
}

#[test]
#[ignore]
fn handing_over_fifty_low_piano_notes_returns_in_under_a_millisecond() {
    let mut engine = live(512);
    let start = engine.now() + 0.5;
    let notes: Vec<(f64, Event)> = (0..50)
        .map(|i| {
            let note = 21.0 + (i % 12) as f64;
            (
                start + i as f64 * 0.02,
                Event::note(None, "piano", &[("note", note)]),
            )
        })
        .collect();
    let began = Instant::now();
    engine.play(&notes).unwrap();
    let took = began.elapsed();
    println!("handing over 50 piano notes took {took:?}");
    assert!(took < Duration::from_millis(1), "{took:?}");
    std::thread::sleep(Duration::from_millis(1500));
    assert!(engine.sounding());
    let warnings = engine.warnings();
    assert!(
        warnings.iter().all(|w| !w.contains("after their time")),
        "{warnings:?}"
    );
}

#[test]
#[ignore]
fn a_note_handed_over_200_ms_ahead_starts_at_its_exact_frame() {
    let mut engine = live(512);
    engine.register(level());
    let from = engine.now() + 0.05;
    let at = from + 0.15;
    engine
        .play(&[(at, Event::note(None, "level", &[]))])
        .unwrap();
    let left = recorded(&mut engine, from, 0.4, "exact");
    let first = left.iter().position(|&x| x != 0.0).unwrap();
    let expected = ((at * 48_000.0).round() - (from * 48_000.0).round()) as usize;
    assert_eq!(first, expected);
    assert!(engine.warnings().is_empty());
}

#[test]
#[ignore]
fn a_batch_with_a_mistake_is_refused_by_the_call_and_plays_nothing() {
    let mut engine = live(512);
    engine.register(level());
    let at = engine.now() + 0.1;
    let mut batch: Vec<(f64, Event)> = (0..9)
        .map(|_| (at, Event::note(None, "level", &[])))
        .collect();
    batch.push((at, Event::note(None, "nothing", &[])));
    assert_eq!(
        engine.play(&batch),
        Err(Error::UnknownSynth("nothing".to_string()))
    );
    std::thread::sleep(Duration::from_millis(300));
    assert!(!engine.sounding());
}

#[test]
#[ignore]
fn a_change_handed_over_right_after_a_note_reaches_it() {
    let mut engine = live(512);
    engine.register(level());
    let from = engine.now() + 0.05;
    let at = from + 0.15;
    engine
        .play(&[(at, Event::note(Some(7), "level", &[]))])
        .unwrap();
    let quieter = Event::Set {
        target: Target::Sounds(vec![7]),
        options: vec![("level".to_string(), 0.5)],
    };
    engine.play(&[(at + 0.1, quieter)]).unwrap();
    let left = recorded(&mut engine, from, 0.4, "change");
    let frame = |seconds: f64| (seconds * 48_000.0) as usize;
    let (before, after) = (left[frame(0.2)], left[frame(0.3)]);
    assert!(
        before > 0.1 && (after / before - 0.5).abs() < 1e-3,
        "{before} then {after}"
    );
}

#[test]
#[ignore]
fn a_note_whose_voice_takes_longer_to_build_than_it_was_given_starts_late_and_is_counted() {
    let mut engine = live(512);
    engine.register(Synth::new("slow").build(|_, _| {
        std::thread::sleep(Duration::from_millis(50));
        Note::rings(dc(0.5), 1.0)
    }));
    std::thread::sleep(Duration::from_millis(200));
    let began = Instant::now();
    engine
        .play(&[(engine.now() + 0.02, Event::note(None, "slow", &[]))])
        .unwrap();
    assert!(began.elapsed() < Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(300));
    assert!(engine.sounding());
    let warnings = engine.warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].starts_with("1 event arrived after their time, by up to "),
        "{warnings:?}"
    );
}

#[test]
#[ignore]
fn a_sound_of_three_channels_is_left_out_and_named_while_the_rest_of_its_batch_plays() {
    let mut engine = live(512);
    engine.register(level());
    engine.register(Synth::new("three").build(|_, _| Note::rings(dc((0.0, 0.0, 0.0)), 1.0)));
    let at = engine.now() + 0.1;
    let batch = [
        (at, Event::note(None, "three", &[])),
        (at, Event::note(None, "level", &[])),
    ];
    engine.play(&batch).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(engine.sounding());
    assert_eq!(
        engine.warnings(),
        vec!["three made a sound of 3 channels, not 1 or 2, so its note did not play".to_string()]
    );
}
