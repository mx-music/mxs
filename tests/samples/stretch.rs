use mxs::{Engine, Event, RATE, Target, Value, effects};

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

fn options(given: &[(&str, f64)]) -> Vec<(String, Value)> {
    given
        .iter()
        .map(|(k, v)| (k.to_string(), Value::Number(*v)))
        .collect()
}

/// The left channel of `samples` played at 0 with `given` options, over `seconds`.
fn played(samples: &[f32], given: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    let sample = engine
        .load_sample_bytes("sound.wav", &wav(samples))
        .unwrap();
    let event = Event::Sample {
        id: Some(1),
        sample,
        options: options(given),
        chain: None,
    };
    engine.play(&[(0.0, event)]).unwrap();
    engine.render(seconds).unwrap().0
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

/// The frequency of the strongest partial within 5 percent of `near`, found every 0.5 Hz and then every 0.01 Hz around the strongest.
fn strongest(samples: &[f32], near: f64) -> f64 {
    let best = |from: f64, step: f64, count: usize| {
        (0..count)
            .map(|k| from + k as f64 * step)
            .max_by(|a, b| level(samples, *a).total_cmp(&level(samples, *b)))
            .unwrap()
    };
    let span = near * 0.05;
    let coarse = best(near - span, 0.5, (4.0 * span) as usize + 1);
    best(coarse - 0.5, 0.01, 101)
}

fn cents(hz: f64, of: f64) -> f64 {
    1200.0 * (hz / of).log2()
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// The seconds from the start to the last sample louder than -80 dB.
fn sounding(samples: &[f32]) -> f64 {
    let last = samples.iter().rposition(|x| x.abs() > 1e-4).unwrap_or(0);
    (last + 1) as f64 / RATE
}

fn frames(seconds: f64) -> usize {
    (seconds * RATE) as usize
}

#[test]
fn a_440_hz_sine_stretched_by_2_plays_for_4_seconds_at_440_hz() {
    let out = played(&sine(440.0, 2.0), &[("stretch", 2.0)], 5.0);
    let length = sounding(&out);
    assert!((length - 4.0).abs() < 0.01, "{length} seconds");
    let found = strongest(&out[frames(0.5)..frames(3.5)], 440.0);
    assert!(cents(found, 440.0).abs() < 1.0, "{found} Hz");
}

#[test]
fn a_440_hz_sine_at_pitch_12_plays_at_880_hz_for_2_seconds() {
    let out = played(&sine(440.0, 2.0), &[("pitch", 12.0)], 3.0);
    let length = sounding(&out);
    assert!((length - 2.0).abs() < 0.01, "{length} seconds");
    let found = strongest(&out[frames(0.25)..frames(1.75)], 880.0);
    assert!(cents(found, 880.0).abs() < 1.0, "{found} Hz");
}

#[test]
fn a_440_hz_sine_stretched_by_one_half_at_pitch_minus_7_plays_at_293_66_hz_for_1_second() {
    let out = played(&sine(440.0, 2.0), &[("stretch", 0.5), ("pitch", -7.0)], 2.0);
    let length = sounding(&out);
    assert!((length - 1.0).abs() < 0.01, "{length} seconds");
    let found = strongest(&out[frames(0.1)..frames(0.9)], 293.66);
    let expected = 440.0 * 2f64.powf(-7.0 / 12.0);
    assert!(cents(found, expected).abs() < 1.0, "{found} Hz");
}

#[test]
fn a_sample_with_stretch_1_and_pitch_0_renders_the_same_samples_as_one_with_neither() {
    let mut noise = Vec::new();
    let mut x = 1u32;
    for _ in 0..48_000 {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        noise.push((x as f32 / u32::MAX as f32 - 0.5) * 0.5);
    }
    let plain = played(&noise, &[("rate", 0.9)], 1.5);
    let given = played(
        &noise,
        &[("rate", 0.9), ("stretch", 1.0), ("pitch", 0.0)],
        1.5,
    );
    assert_eq!(plain, given);
}

#[test]
fn hits_every_half_second_stretched_by_1_2_start_every_0_6_seconds_as_loud() {
    let mut hits = vec![0.0f32; frames(4.0)];
    for k in 0..7 {
        let at = frames(0.25 + 0.5 * k as f64);
        for i in 0..240 {
            let fall = (-(i as f32) / 40.0).exp();
            hits[at + i] = 0.8 * fall * (i as f32 * 0.3).sin();
        }
    }
    let original = played(&hits, &[], 4.5);
    let out = played(&hits, &[("stretch", 1.2)], 5.5);
    let peak = |x: &[f32]| x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    for k in 0..7 {
        let was = frames(0.25 + 0.5 * k as f64);
        let at = frames(0.3 + 0.6 * k as f64);
        let before = peak(&original[was - 2400..was + 2400]);
        let window = &out[at - 2400..at + 2400];
        let now = peak(window);
        let onset = window.iter().position(|x| x.abs() > 0.1 * now).unwrap();
        let off = (onset as f64 - 2400.0) / RATE;
        assert!(off.abs() < 0.005, "hit {k} starts {off} seconds off");
        let louder = db(f64::from(now / before));
        assert!(
            louder.abs() < 3.0,
            "hit {k} peaks {louder} dB from the original"
        );
    }
}

#[test]
fn a_pitch_gliding_from_0_to_12_over_a_second_passes_622_hz_midway_and_ends_at_880_hz() {
    let mut engine = Engine::offline(RATE);
    let sample = engine
        .load_sample_bytes("sound.wav", &wav(&sine(440.0, 4.0)))
        .unwrap();
    let event = Event::Sample {
        id: Some(1),
        sample,
        options: options(&[("pitch", 0.0)]),
        chain: None,
    };
    let glide = Event::Set {
        target: Target::Sounds(vec![1]),
        options: vec![
            ("pitch_slide".to_string(), 1.0),
            ("pitch".to_string(), 12.0),
        ],
    };
    engine.play(&[(0.0, event), (1.0, glide)]).unwrap();
    let (out, _) = engine.render(4.0).unwrap();
    let crossings = |from: f64, to: f64| {
        let x = &out[frames(from)..frames(to)];
        let ups: Vec<usize> = (1..x.len())
            .filter(|&i| x[i - 1] < 0.0 && x[i] >= 0.0)
            .collect();
        let periods = (ups.len() - 1) as f64;
        periods * RATE / (ups[ups.len() - 1] - ups[0]) as f64
    };
    let middle = crossings(1.49, 1.51);
    let six = 440.0 * 2f64.powf(0.5);
    assert!(cents(middle, six).abs() < 50.0, "{middle} Hz midway");
    let found = strongest(&out[frames(2.25)..frames(3.75)], 880.0);
    assert!(cents(found, 880.0).abs() < 1.0, "{found} Hz at the end");
}

#[test]
fn the_pitch_shift_effect_raises_a_440_hz_sine_by_5_semitones_and_delays_it_by_at_most_50_ms() {
    let mut engine = Engine::offline(RATE);
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    let sample = engine
        .load_sample_bytes("sound.wav", &wav(&sine(440.0, 2.0)))
        .unwrap();
    let shift = ("pitch_shift".to_string(), options(&[("pitch", 5.0)]));
    let chain = engine.start_effects(&[shift]).unwrap();
    let event = Event::Sample {
        id: None,
        sample,
        options: vec![],
        chain: Some(chain),
    };
    engine.play(&[(0.5, event)]).unwrap();
    let (out, _) = engine.render(3.0).unwrap();
    let found = strongest(&out[frames(1.0)..frames(2.0)], 587.33);
    let expected = 440.0 * 2f64.powf(5.0 / 12.0);
    assert!(cents(found, expected).abs() < 2.0, "{found} Hz");
    let heard = out.iter().position(|x| x.abs() > 0.01).unwrap();
    let delay = heard as f64 / RATE - 0.5;
    assert!(delay <= 0.05, "delayed {delay} seconds");
}
