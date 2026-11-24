//! Voices filled on several threads; the timing checks need a release build: `cargo test --release --test playing workers:: -- --ignored --nocapture --test-threads 1`.

use mxs::{Engine, Event, RATE, Value, effects, synths};
use std::time::Instant;

fn engine(workers: usize) -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    engine.set_workers(workers);
    engine
}

/// Two bars of piano chords, berimbau strokes and a pluck through a reverb, sixty notes sounding at the busiest.
fn song(engine: &mut Engine) -> (Vec<f32>, Vec<f32>) {
    let reverb = (
        "reverb".to_string(),
        vec![("decay_seconds".to_string(), Value::Number(2.0))],
    );
    let chain = engine.start_effects(&[reverb]).unwrap();
    let mut events = Vec::new();
    for beat in 0..8 {
        let at = beat as f64 * 0.25;
        for k in 0..6 {
            let note = 48.0 + [0.0, 4.0, 7.0, 12.0, 16.0, 19.0][k] + (beat % 4) as f64;
            events.push((
                at,
                Event::note(None, "piano", &[("note", note), ("sustain", 1.5)]),
            ));
        }
        events.push((at + 0.125, Event::note(None, "berimbau", &[])));
        let pluck = Event::Note {
            id: None,
            synth: "pluck".to_string(),
            options: vec![("note".to_string(), Value::Number(60.0 + beat as f64))],
            chain: Some(chain),
            held: false,
        };
        events.push((at, pluck));
    }
    engine.play(&events).unwrap();
    engine.render(2.0).unwrap()
}

/// An engine on `workers` with the one limit on voices, as the default limit grows with the workers and a song past it would steal other notes.
fn limited(workers: usize) -> Engine {
    let mut engine = engine(workers);
    engine.set_voice_limit(mxs::VOICES);
    engine
}

#[test]
fn a_song_renders_the_same_samples_on_1_2_4_and_6_workers() {
    let one = song(&mut limited(1));
    assert!(one.0.iter().any(|x| x.abs() > 0.01));
    for workers in [2, 4, 6] {
        assert!(song(&mut limited(workers)) == one, "{workers} workers");
    }
}

#[test]
fn an_engine_takes_the_number_of_workers_it_is_given_and_raises_its_default_limit_with_them() {
    let mut engine = engine(1);
    assert_eq!(engine.workers(), 1);
    engine.set_workers(6);
    assert_eq!(engine.workers(), 6);
    assert_eq!(mxs::voice_limit_for(1), mxs::VOICES);
    assert_eq!(mxs::voice_limit_for(6), 4 * mxs::VOICES);
}

/// The seconds `count` held piano notes take to render for two seconds on `workers`.
fn held_piano(count: usize, workers: usize, seconds: f64) -> f64 {
    let mut engine = engine(workers);
    engine.set_voice_limit(count + 1);
    let notes: Vec<_> = (0..count)
        .map(|i| {
            let note = 48.0 + (i % 12) as f64;
            (
                0.0,
                Event::note(None, "piano", &[("note", note), ("sustain", 60.0)]),
            )
        })
        .collect();
    for batch in notes.chunks(256) {
        engine.play(batch).unwrap();
    }
    engine.render(0.01).unwrap();
    let started = Instant::now();
    engine.render(seconds).unwrap();
    started.elapsed().as_secs_f64()
}

#[test]
#[ignore]
fn on_six_workers_240_held_piano_notes_play_at_least_4_times_as_many_at_once_as_on_one() {
    let at_once = |workers: usize| 240.0 * 2.0 / held_piano(240, workers, 2.0);
    let (one, six) = (at_once(1), at_once(6));
    println!(
        "{one:.0} at once on 1 worker, {six:.0} on 6, {:.2} times",
        six / one
    );
    assert!(six >= 4.0 * one);
}

#[test]
#[ignore]
fn one_piano_note_takes_no_more_than_10_percent_longer_a_block_on_six_workers() {
    let best = |workers: usize| {
        (0..5)
            .map(|_| held_piano(1, workers, 10.0))
            .fold(f64::INFINITY, f64::min)
    };
    let (one, six) = (best(1), best(6));
    println!(
        "one note: {one:.4} s on 1 worker, {six:.4} s on 6, {:.1} percent",
        (six / one - 1.0) * 100.0
    );
    assert!(six <= one * 1.1);
}
