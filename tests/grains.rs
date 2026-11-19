use mxs::{Engine, Event, RATE, Target, Value};

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

fn sine(hz: f64, seconds: f64) -> Vec<f32> {
    (0..(seconds * RATE) as usize)
        .map(|i| 0.5 * (2.0 * std::f64::consts::PI * hz * i as f64 / RATE).sin() as f32)
        .collect()
}

/// The left channel of `seconds` of grains of `samples`, with `options`.
fn grains(samples: &[f32], options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    let sample = engine
        .load_sample_bytes("sound.wav", &wav(samples))
        .unwrap();
    engine.play(&[(0.0, event(sample, options))]).unwrap();
    engine.render(seconds).unwrap().0
}

fn event(sample: u64, options: &[(&str, f64)]) -> Event {
    let options = options
        .iter()
        .map(|(k, v)| (k.to_string(), Value::Number(*v)))
        .collect();
    Event::Grains {
        id: Some(1),
        sample,
        options,
        chain: None,
    }
}

/// The level of `samples` at `hz`, by a Goertzel filter over all of them under a Hann window.
fn level(samples: &[f32], hz: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * hz / RATE;
    let n = samples.len() as f64;
    let (mut s1, mut s2) = (0.0, 0.0);
    for (i, &x) in samples.iter().enumerate() {
        let window = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n).cos();
        let s = x as f64 * window + 2.0 * w.cos() * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let power = s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2;
    4.0 * power.sqrt() / n
}

/// The frequency of the strongest partial from 50 Hz to 4 kHz, found every 2 Hz and then every 0.05 Hz around the strongest.
fn strongest(samples: &[f32]) -> f64 {
    let best = |from: f64, step: f64, count: usize| {
        (0..count)
            .map(|k| from + k as f64 * step)
            .max_by(|a, b| level(samples, *a).total_cmp(&level(samples, *b)))
            .unwrap()
    };
    let coarse = best(50.0, 2.0, 1976);
    best(coarse - 3.0, 0.05, 121)
}

fn cents(hz: f64, of: f64) -> f64 {
    1200.0 * (hz / of).log2()
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// From 0.25 to 1.25 seconds, once the grains have started.
fn steady(samples: &[f32]) -> &[f32] {
    &samples[12_000..60_000]
}

#[test]
fn grains_of_a_440_hz_sine_sound_at_440_hz_and_an_octave_up_at_pitch_12() {
    let tone = sine(440.0, 2.0);
    for (pitch, hz) in [(0.0, 440.0), (12.0, 880.0)] {
        let played = grains(
            &tone,
            &[("duration", 0.05), ("density", 20.0), ("pitch", pitch)],
            1.5,
        );
        let found = strongest(steady(&played));
        assert!(cents(found, hz).abs() < 5.0, "{found} Hz at pitch {pitch}");
    }
}

#[test]
fn grains_with_no_spread_sound_the_part_of_the_sample_their_position_names() {
    let mut halves = sine(220.0, 1.0);
    halves.extend(sine(440.0, 1.0));
    for (position, hz) in [(0.25, 220.0), (0.75, 440.0)] {
        let played = grains(&halves, &[("duration", 0.05), ("position", position)], 1.5);
        let found = strongest(steady(&played));
        assert!(cents(found, hz).abs() < 5.0, "{found} Hz from {position}");
    }
}

#[test]
fn grains_of_a_440_hz_sine_have_no_partial_above_2_khz_within_60_db_of_their_strongest() {
    let options = [
        ("duration", 0.05),
        ("density", 20.0),
        ("spread", 0.1),
        ("width", 0.5),
    ];
    let played = grains(&sine(440.0, 2.0), &options, 1.5);
    let steady = steady(&played);
    let top = level(steady, strongest(steady));
    let above = (2_000..24_000)
        .step_by(10)
        .map(|hz| level(steady, hz as f64))
        .fold(0.0, f64::max);
    assert!(db(above / top) < -60.0, "{} dB", db(above / top));
}

#[test]
fn grains_with_the_same_seed_render_the_same_samples_and_another_seed_others() {
    let mut noise = Vec::new();
    let mut x = 1u32;
    for _ in 0..96_000 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        noise.push((x as f32 / u32::MAX as f32 - 0.5) * 0.5);
    }
    let options = |seed: f64| {
        [
            ("spread", 0.4),
            ("width", 1.0),
            ("density", 50.0),
            ("seed", seed),
        ]
    };
    let first = grains(&noise, &options(7.0), 1.0);
    assert!(first.iter().any(|x| *x != 0.0));
    assert_eq!(first, grains(&noise, &options(7.0), 1.0));
    assert_ne!(first, grains(&noise, &options(8.0), 1.0));
}

#[test]
fn grains_follow_a_position_that_moves_while_they_play() {
    let mut halves = sine(220.0, 1.0);
    halves.extend(sine(440.0, 1.0));
    let mut engine = Engine::offline(RATE);
    let sample = engine
        .load_sample_bytes("sound.wav", &wav(&halves))
        .unwrap();
    let options = [("duration", 0.05), ("position", 0.25), ("sustain", 3.0)];
    let set = Event::Set {
        target: Target::Sounds(vec![1]),
        options: vec![("position".to_string(), 0.75)],
    };
    engine
        .play(&[(0.0, event(sample, &options)), (1.5, set)])
        .unwrap();
    let (left, _) = engine.render(3.0).unwrap();
    let (before, after) = (strongest(steady(&left)), strongest(&left[96_000..144_000]));
    assert!(cents(before, 220.0).abs() < 5.0, "{before} Hz before");
    assert!(cents(after, 440.0).abs() < 5.0, "{after} Hz after");
}
