use super::*;

/// What the voices of `synth` kept ready are given as they are taken.
fn kept(engine: &Engine, synth: &str) -> Vec<Arc<Ready>> {
    engine.book().ready[synth]
        .iter()
        .filter_map(|built| built.ready.clone())
        .collect()
}

#[test]
fn a_held_piano_note_with_the_pedal_down_takes_a_voice_kept_ready_and_one_half_down_does_not() {
    let mut engine = Engine::offline(48_000.0);
    crate::synths::all()
        .into_iter()
        .for_each(|s| engine.register(s));
    engine.keep_ready("piano", 2).unwrap();
    let ready = kept(&engine, "piano");
    let options = |pedal| {
        [
            ("note", 60.0),
            ("vel", 0.6),
            ("pedal", pedal),
            ("bend", 0.0),
        ]
    };
    engine
        .play(&[(0.0, Event::held(1, "piano", &options(0.5)))])
        .unwrap();
    assert!(ready.iter().all(|hold| hold.given().is_none()));
    engine
        .play(&[(0.0, Event::held(2, "piano", &options(1.0)))])
        .unwrap();
    let given = Given {
        note: 60.0,
        vel: 0.6,
        held: true,
        pedal: 1.0,
        pressure: 0.0,
    };
    assert!(ready.iter().any(|hold| hold.given() == Some(given)));
}

#[test]
fn once_the_output_device_goes_away_the_next_call_names_it() {
    let mut engine = Engine::offline(48_000.0);
    engine.device = "USB Interface".to_string();
    engine.lost.store(true, Ordering::Release);
    let error = engine
        .play(&[(0.0, Event::note(None, "saw", &[]))])
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "the output device USB Interface went away, and the engine has to be started again"
    );
}

/// Renders `seconds` 1024 frames at a time, feeding each frame's input before it, as a sound card would.
fn render_fed(
    engine: &mut Engine,
    feed: &mut rtrb::Producer<f32>,
    seconds: f64,
    frame: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let frames = engine.frames(seconds) as usize;
    while left.len() < frames {
        let n = Ord::min(1024, frames - left.len());
        for _ in 0..n {
            frame.iter().for_each(|&x| feed.push(x).unwrap());
        }
        let (l, r) = engine.render(n as f64 / engine.rate).unwrap();
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

#[test]
fn live_input_plays_its_channels_placed_by_pan_until_it_stops() {
    let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 2);
    let mono = Event::Input {
        id: Some(1),
        input: 0,
        channels: 1,
        options: vec![],
        chain: None,
    };
    engine.play(&[(0.0, mono)]).unwrap();
    let (left, right) = render_fed(&mut engine, &mut feed, 0.1, &[0.5, 0.25]);
    let centre = std::f32::consts::FRAC_1_SQRT_2;
    assert!((left[1000] - 0.5 * centre).abs() < 1e-6);
    assert!((right[1000] - 0.5 * centre).abs() < 1e-6);
    engine.play(&[(0.1, Event::Stop { ids: vec![1] })]).unwrap();
    let (left, _) = render_fed(&mut engine, &mut feed, 0.1, &[0.5, 0.25]);
    assert_eq!(left[1000], 0.0);

    let options = vec![("pan".to_string(), Value::Number(-1.0))];
    let stereo = Event::Input {
        id: None,
        input: 0,
        channels: 2,
        options,
        chain: None,
    };
    engine.play(&[(0.2, stereo)]).unwrap();
    let (left, right) = render_fed(&mut engine, &mut feed, 0.1, &[0.5, 0.25]);
    assert_eq!(left[1000], 0.5);
    assert_eq!(right[1000], 0.0);
    let beyond = Event::Input {
        id: None,
        input: 1,
        channels: 2,
        options: vec![],
        chain: None,
    };
    assert_eq!(engine.play(&[(0.3, beyond)]), Err(Error::UnknownInput(2)));
}

#[test]
fn live_input_let_go_ends_in_10_ms_since_it_never_ends_by_itself() {
    let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
    let input = Event::Input {
        id: Some(1),
        input: 0,
        channels: 1,
        options: vec![],
        chain: None,
    };
    engine.play(&[(0.0, input)]).unwrap();
    engine
        .play(&[(0.1, Event::Release { ids: vec![1] })])
        .unwrap();
    let (left, _) = render_fed(&mut engine, &mut feed, 0.2, &[0.5]);
    assert!(left[4000] > 0.0);
    assert!(left[4800 + 480..].iter().all(|s| *s == 0.0));
}

#[test]
fn live_input_plays_through_the_main_effects() {
    let played = |main: Option<f64>| {
        let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
        crate::effects::all()
            .into_iter()
            .for_each(|e| engine.register_effect(e));
        if let Some(amp) = main {
            let level = (
                "level".to_string(),
                vec![("amp".to_string(), Value::Number(amp))],
            );
            engine.start_main_effects(&[level], false).unwrap();
        }
        let input = Event::Input {
            id: Some(1),
            input: 0,
            channels: 1,
            options: vec![],
            chain: None,
        };
        engine.play(&[(0.0, input)]).unwrap();
        render_fed(&mut engine, &mut feed, 0.2, &[0.5]).0
    };
    let (straight, halved) = (played(None), played(Some(0.5)));
    assert!(straight[4000] > 0.0);
    assert!((straight[4000] * 0.5 - halved[4000]).abs() < 1e-6);
}

#[test]
fn a_recording_of_an_input_writes_it_centred() {
    let dir = std::env::temp_dir().join(format!("mxs-input-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("input.wav");
    let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
    engine.record(&path, Source::Input(0), 0.0).unwrap();
    let (left, _) = render_fed(&mut engine, &mut feed, 0.1, &[0.5]);
    assert_eq!(left[100], 0.0);
    engine.stop_recording().unwrap();
    let mut reader = hound::WavReader::open(&path).unwrap();
    let samples: Vec<i32> = reader.samples::<i32>().map(|x| x.unwrap()).collect();
    assert_eq!(samples.len(), 2 * 4800);
    let centre = std::f32::consts::FRAC_1_SQRT_2;
    assert!((samples[200] as f32 / 8_388_607.0 - 0.5 * centre).abs() < 1e-4);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn an_analysis_of_an_input_hears_it_alone_and_reports_its_pitch() {
    let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
    let reports = engine.subscribe(&[Topic::Analysis]);
    engine.analyse(Origin::Input(0), 0.01, 0.0).unwrap();
    let mut pitches = Vec::new();
    for block in 0..48 {
        for i in 0..1000 {
            let t = (block * 1000 + i) as f64 / 48_000.0;
            feed.push(0.5 * (2.0 * std::f64::consts::PI * 220.0 * t).sin() as f32)
                .unwrap();
        }
        let (left, right) = engine.render(1000.0 / 48_000.0).unwrap();
        assert!(left.iter().chain(&right).all(|x| *x == 0.0));
        while let Some(report) = reports.wait(Duration::ZERO) {
            if let Report::Analysis {
                pitch: Some(hz), ..
            } = report
            {
                pitches.push(hz);
            }
        }
    }
    assert!(pitches.len() > 40);
    assert!(
        pitches.iter().all(|hz| (hz - 220.0).abs() < 0.5),
        "{pitches:?}"
    );
}

/// Renders `seconds` of an engine whose one input is fed `signal` from frame 0.
fn render_signal(
    engine: &mut Engine,
    feed: &mut rtrb::Producer<f32>,
    seconds: f64,
    signal: impl Fn(usize) -> f32,
) -> (Vec<f32>, Vec<f32>) {
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let frames = engine.frames(seconds) as usize;
    while left.len() < frames {
        let n = Ord::min(1000, frames - left.len());
        (left.len()..left.len() + n).for_each(|i| feed.push(signal(i)).unwrap());
        let (l, r) = engine.render(n as f64 / engine.rate).unwrap();
        left.extend(l);
        right.extend(r);
    }
    (left, right)
}

#[test]
fn an_input_recorded_into_a_buffer_from_1_second_plays_back_from_3_seconds_exactly() {
    let signal = |i: usize| 0.5 * ((i as f32 * 0.0137).sin() + (i as f32 * 0.091).sin()) / 2.0;
    let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
    let buffer = engine.create_buffer(2.0, 2).unwrap();
    engine
        .record_buffer(buffer, Origin::Input(0), 1.0, 2.0, 0.0)
        .unwrap();
    let sample = Event::Sample {
        id: None,
        sample: buffer,
        options: vec![],
        chain: None,
    };
    engine.play(&[(3.0, sample)]).unwrap();
    let (left, right) = render_signal(&mut engine, &mut feed, 5.0, signal);
    assert!(left[..144_000].iter().all(|x| *x == 0.0));

    let (mut straight, mut feed) = Engine::offline_with_input(48_000.0, 1);
    let input = Event::Input {
        id: None,
        input: 0,
        channels: 1,
        options: vec![],
        chain: None,
    };
    straight.play(&[(1.0, input)]).unwrap();
    let (expected, _) = render_signal(&mut straight, &mut feed, 3.0, signal);
    // The buffer fades its last 10 ms as it plays, as every sound does at its end.
    let played = 144_000..240_000 - 480;
    assert_eq!(left[played.clone()], expected[48_000..144_000 - 480]);
    assert_eq!(left[played.clone()], right[played]);
}

#[test]
fn a_live_input_through_the_pitch_shift_at_5_semitones_rises_to_587_hz_within_50_ms() {
    let (mut engine, mut feed) = Engine::offline_with_input(48_000.0, 1);
    crate::effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    let shift = (
        "pitch_shift".to_string(),
        vec![("pitch".to_string(), Value::Number(5.0))],
    );
    let chain = engine.start_effects(&[shift]).unwrap();
    let input = Event::Input {
        id: None,
        input: 0,
        channels: 1,
        options: vec![],
        chain: Some(chain),
    };
    engine.play(&[(0.0, input)]).unwrap();
    let tone = |i: usize| {
        let t = i as f64 / 48_000.0;
        if t < 0.5 {
            0.0
        } else {
            0.5 * (2.0 * std::f64::consts::PI * 440.0 * t).sin() as f32
        }
    };
    let (left, _) = render_signal(&mut engine, &mut feed, 2.5, tone);
    let heard = left.iter().position(|x| x.abs() > 0.01).unwrap();
    let delay = heard as f64 / 48_000.0 - 0.5;
    assert!(delay <= 0.05, "delayed {delay} seconds");
    let steady = &left[72_000..120_000];
    let level = |hz: f64| {
        let w = 2.0 * std::f64::consts::PI * hz / 48_000.0;
        let n = steady.len() as f64;
        let (mut s1, mut s2) = (0.0, 0.0);
        for (i, &x) in steady.iter().enumerate() {
            let window = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n).cos();
            let s = x as f64 * window + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s;
        }
        s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2
    };
    let found = (0..2000)
        .map(|k| 577.0 + k as f64 * 0.01)
        .max_by(|a, b| level(*a).total_cmp(&level(*b)))
        .unwrap();
    let expected = 440.0 * 2f64.powf(5.0 / 12.0);
    let cents = 1200.0 * (found / expected).log2();
    assert!(cents.abs() < 2.0, "{found} Hz");
}
