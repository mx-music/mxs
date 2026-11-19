use fundsp::prelude::*;
use mxs::{Engine, Event, Note, Origin, RATE, Synth, Tempo};
use std::sync::atomic::{AtomicUsize, Ordering};

static SAVED: AtomicUsize = AtomicUsize::new(0);

/// A mono 32-bit float WAV file of `samples` at the engine's rate.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|x| x.to_le_bytes()).collect();
    let rate = RATE as u32;
    let mut body = b"WAVEfmt ".to_vec();
    body.extend(16u32.to_le_bytes());
    body.extend(3u16.to_le_bytes());
    body.extend(1u16.to_le_bytes());
    body.extend(rate.to_le_bytes());
    body.extend((rate * 4).to_le_bytes());
    body.extend(4u16.to_le_bytes());
    body.extend(32u16.to_le_bytes());
    body.extend(b"data");
    body.extend((data.len() as u32).to_le_bytes());
    body.extend(data);
    let mut file = b"RIFF".to_vec();
    file.extend((body.len() as u32).to_le_bytes());
    file.extend(body);
    file
}

fn sine(hz: f64, peak: f32, seconds: f64) -> Vec<f32> {
    (0..(seconds * RATE) as usize)
        .map(|i| peak * (2.0 * std::f64::consts::PI * hz * i as f64 / RATE).sin() as f32)
        .collect()
}

fn sample(engine: &mut Engine, samples: &[f32]) -> u64 {
    engine
        .load_sample_bytes("sound.wav", &wav(samples))
        .unwrap()
}

fn play(engine: &mut Engine, at: f64, sample: u64, options: &[(&str, f64)]) {
    let options = options
        .iter()
        .map(|(k, v)| (k.to_string(), (*v).into()))
        .collect();
    let event = Event::Sample {
        id: None,
        sample,
        options,
        chain: None,
    };
    engine.play(&[(at, event)]).unwrap();
}

/// The channels of a sample saved to a file.
fn saved(engine: &Engine, sample: u64) -> Vec<Vec<f32>> {
    let (process, count) = (std::process::id(), SAVED.fetch_add(1, Ordering::Relaxed));
    let path = std::env::temp_dir().join(format!("mxs-buffer-{process}-{count}.wav"));
    engine.save_sample(sample, &path).unwrap();
    let mut reader = hound::WavReader::open(&path).unwrap();
    let channels = reader.spec().channels as usize;
    let all: Vec<f32> = reader.samples::<f32>().map(|x| x.unwrap()).collect();
    std::fs::remove_file(&path).unwrap();
    (0..channels)
        .map(|c| all.iter().skip(c).step_by(channels).copied().collect())
        .collect()
}

/// The peak level of the partial at `hz` in `x`, which holds a whole number of its periods.
fn level(x: &[f32], hz: f64) -> f64 {
    let (mut re, mut im) = (0.0, 0.0);
    for (i, &s) in x.iter().enumerate() {
        let phase = 2.0 * std::f64::consts::PI * hz * i as f64 / RATE;
        re += f64::from(s) * phase.cos();
        im += f64::from(s) * phase.sin();
    }
    2.0 * (re * re + im * im).sqrt() / x.len() as f64
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

#[test]
fn a_sine_recorded_over_another_at_level_1_keeps_both_at_their_recorded_levels() {
    let mut engine = Engine::offline(RATE);
    let buffer = engine.create_buffer(1.0, 1).unwrap();
    // Longer than the recordings, so the 10 ms in which every sound fades out at its end is not recorded.
    let high = sample(&mut engine, &sine(660.0, 0.5, 2.0));
    let low = sample(&mut engine, &sine(440.0, 0.25, 2.0));
    engine
        .record_buffer(buffer, Origin::Sample(high), 0.0, 1.0, 0.0)
        .unwrap();
    engine
        .record_buffer(buffer, Origin::Sample(low), 1.0, 1.0, 1.0)
        .unwrap();
    engine.render(2.5).unwrap();
    let held = &saved(&engine, buffer)[0];
    assert!(
        (level(held, 660.0) - 0.5).abs() < 1e-3,
        "{}",
        level(held, 660.0)
    );
    assert!(
        (level(held, 440.0) - 0.25).abs() < 1e-3,
        "{}",
        level(held, 440.0)
    );
}

#[test]
fn a_sine_recorded_over_another_at_level_one_half_leaves_it_6_db_lower() {
    let mut engine = Engine::offline(RATE);
    let buffer = engine.create_buffer(1.0, 1).unwrap();
    let high = sample(&mut engine, &sine(660.0, 0.5, 2.0));
    let low = sample(&mut engine, &sine(440.0, 0.25, 2.0));
    engine
        .record_buffer(buffer, Origin::Sample(high), 0.0, 1.0, 0.0)
        .unwrap();
    engine
        .record_buffer(buffer, Origin::Sample(low), 1.0, 1.0, 0.5)
        .unwrap();
    engine.render(2.5).unwrap();
    let held = &saved(&engine, buffer)[0];
    let lower = db(level(held, 660.0)) - db(0.5);
    assert!((lower + 6.02).abs() < 0.05, "{lower} dB");
    assert!((level(held, 440.0) - 0.25).abs() < 1e-3);
}

#[test]
fn a_buffer_read_faster_than_it_is_written_plays_what_was_written_by_then_and_then_silence() {
    let mut engine = Engine::offline(RATE);
    let buffer = engine.create_buffer(2.0, 2).unwrap();
    let tone = sample(&mut engine, &sine(440.0, 0.5, 2.0));
    engine
        .record_buffer(buffer, Origin::Sample(tone), 1.0, 2.0, 0.0)
        .unwrap();
    // Read at twice the rate from 1.5 seconds, it catches up with the recording at 2 seconds, one second into the buffer.
    play(&mut engine, 1.5, buffer, &[("rate", 2.0)]);
    let (left, _) = engine.render(3.0).unwrap();
    let frame = |seconds: f64| (seconds * RATE) as usize;
    let before = &left[frame(1.51)..frame(1.99)];
    let rms = (before.iter().map(|x| x * x).sum::<f32>() / before.len() as f32).sqrt();
    assert!((rms - 0.25).abs() < 0.01, "{rms}");
    assert!(left[frame(2.01)..].iter().all(|x| *x == 0.0));
}

#[test]
fn a_buffer_trailing_its_recording_plays_all_of_it() {
    let mut engine = Engine::offline(RATE);
    let buffer = engine.create_buffer(2.0, 2).unwrap();
    let tone = sample(&mut engine, &sine(440.0, 0.5, 3.0));
    engine
        .record_buffer(buffer, Origin::Sample(tone), 1.0, 2.0, 0.0)
        .unwrap();
    play(&mut engine, 1.5, buffer, &[]);
    let (left, right) = engine.render(4.0).unwrap();
    let mut straight = Engine::offline(RATE);
    let source = sample(&mut straight, &sine(440.0, 0.5, 2.0));
    play(&mut straight, 1.5, source, &[]);
    let (expected, _) = straight.render(4.0).unwrap();
    for (i, (x, e)) in left.iter().zip(&expected).enumerate() {
        assert!((x - e).abs() < 1e-6, "frame {i}: {x} against {e}");
    }
    assert_eq!(left, right);
}

fn tone() -> Synth {
    Synth::new("tone").build(|_, _| Note::rings(sine_hz::<f32>(440.0) * 0.3, 3.0))
}

#[test]
fn a_recording_of_a_sample_and_one_of_the_main_output_run_at_once_each_holding_its_own() {
    let mut engine = Engine::offline(RATE);
    engine.register(tone());
    let first = engine.create_buffer(1.0, 1).unwrap();
    let second = engine.create_buffer(1.0, 1).unwrap();
    let high = sample(&mut engine, &sine(660.0, 0.5, 2.0));
    engine
        .play(&[(0.0, Event::note(None, "tone", &[]))])
        .unwrap();
    engine
        .record_buffer(first, Origin::Sample(high), 1.0, 1.0, 0.0)
        .unwrap();
    engine
        .record_buffer(second, Origin::Output, 1.0, 1.0, 0.0)
        .unwrap();
    engine.render(2.5).unwrap();
    let (first, second) = (&saved(&engine, first)[0], &saved(&engine, second)[0]);
    assert!((level(first, 660.0) - 0.5).abs() < 1e-3);
    assert!(level(first, 440.0) < 1e-4);
    assert!(level(second, 440.0) > 0.1);
    assert!(level(second, 660.0) < 1e-4);
}

#[test]
fn a_buffer_saved_to_a_file_holds_the_samples_it_plays() {
    let mut engine = Engine::offline(RATE);
    let buffer = engine.create_buffer(1.0, 2).unwrap();
    let tone = sample(&mut engine, &sine(330.0, 0.5, 2.0));
    engine
        .record_buffer(buffer, Origin::Sample(tone), 0.0, 1.0, 0.0)
        .unwrap();
    engine.render(1.0).unwrap();
    play(&mut engine, 1.0, buffer, &[]);
    let (left, right) = engine.render(1.0).unwrap();
    let held = saved(&engine, buffer);
    // Playing it fades its last 10 ms, as every sound fades at its end.
    let faded = left.len() - 480;
    assert_eq!(held[0][..faded], left[..faded]);
    assert_eq!(held[1][..faded], right[..faded]);
    assert!(left.iter().any(|x| *x != 0.0));
}

#[test]
fn a_freed_buffer_plays_on_until_its_sound_ends_and_events_can_no_longer_name_it() {
    let mut engine = Engine::offline(RATE);
    let buffer = engine.create_buffer(1.0, 2).unwrap();
    let tone = sample(&mut engine, &sine(330.0, 0.5, 1.0));
    engine
        .record_buffer(buffer, Origin::Sample(tone), 0.0, 1.0, 0.0)
        .unwrap();
    play(&mut engine, 0.5, buffer, &[]);
    engine.render(0.25).unwrap();
    engine.free_sample(buffer).unwrap();
    let (left, _) = engine.render(1.5).unwrap();
    let rms = (left.iter().map(|x| x * x).sum::<f32>() / left.len() as f32).sqrt();
    assert!(rms > 0.1);
    let refused = Event::Sample {
        id: None,
        sample: buffer,
        options: vec![],
        chain: None,
    };
    let message = engine.play(&[(2.0, refused)]).unwrap_err().to_string();
    assert_eq!(message, format!("no sample loaded as {buffer}"));
}

#[test]
fn at_120_beats_a_minute_a_recording_from_beat_4_for_4_beats_starts_at_2_seconds_and_holds_2() {
    let mut engine = Engine::offline(RATE);
    engine.set_tempo(0.0, Tempo::Hold(120.0)).unwrap();
    let buffer = engine.create_buffer(3.0, 1).unwrap();
    let ones = sample(&mut engine, &vec![0.5; 5 * RATE as usize]);
    engine
        .record_buffer_in_beats(buffer, Origin::Sample(ones), 4.0, 4.0, 0.0)
        .unwrap();
    engine.render(5.0).unwrap();
    let held = &saved(&engine, buffer)[0];
    let written = held.iter().take_while(|x| **x != 0.0).count();
    assert_eq!(written, 2 * RATE as usize);
    assert!(held[written..].iter().all(|x| *x == 0.0));
    // The sample started at 2 seconds, so the buffer's first frame is the sample's first.
    assert!((held[0] - 0.5).abs() < 1e-6, "{}", held[0]);
}
