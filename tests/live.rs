//! Checks of a live engine, which need BlackHole 2ch, a virtual sound card, so that nothing is heard, and a release build, as a debug one fills too slowly; run them with `cargo test --release --test live -- --ignored --test-threads 1`.

use fundsp::prelude::*;
use mxs::{Engine, Event, Note, Settings, Synth, synths};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
