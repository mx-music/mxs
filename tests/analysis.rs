use fundsp::prelude::*;
use mxs::{Engine, Event, Note, Origin, RATE, Report, Synth, Topic};
use std::time::Duration;

/// A mono 32-bit float WAV file of `samples` at the engine's rate.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data: Vec<u8> = samples.iter().flat_map(|x| x.to_le_bytes()).collect();
    let rate = RATE as u32;
    let mut fmt = Vec::new();
    for field in [3u16, 1] {
        fmt.extend(field.to_le_bytes());
    }
    fmt.extend(rate.to_le_bytes());
    fmt.extend((rate * 4).to_le_bytes());
    fmt.extend(4u16.to_le_bytes());
    fmt.extend(32u16.to_le_bytes());
    let mut body = b"WAVEfmt ".to_vec();
    body.extend(16u32.to_le_bytes());
    body.extend(fmt);
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

/// The reports of an analysis of `samples` played in place of the input, at `interval` seconds, over `seconds`.
fn analysed(samples: &[f32], interval: f64, seconds: f64) -> Vec<Report> {
    let mut engine = Engine::offline(RATE);
    let reports = engine.subscribe(&[Topic::Analysis]);
    let sample = engine
        .load_sample_bytes("input.wav", &wav(samples))
        .unwrap();
    engine
        .analyse(Origin::Sample(sample), interval, 0.0)
        .unwrap();
    // A subscriber keeps 64 reports waiting, so they are read as the render goes.
    let mut heard = Vec::new();
    for _ in 0..(seconds * 10.0) as usize {
        engine.render(0.1).unwrap();
        heard.extend(std::iter::from_fn(|| reports.wait(Duration::ZERO)));
    }
    heard
}

/// Each report of the loudness and the pitch, as its seconds, loudness, pitch and confidence.
fn measured(reports: &[Report]) -> Vec<(f64, f64, Option<f64>, f64)> {
    reports
        .iter()
        .filter_map(|report| match report {
            Report::Analysis {
                seconds,
                loudness,
                pitch,
                confidence,
                ..
            } => Some((*seconds, *loudness, *pitch, *confidence)),
            _ => None,
        })
        .collect()
}

fn onsets(reports: &[Report]) -> Vec<f64> {
    reports
        .iter()
        .filter_map(|report| match report {
            Report::Onset { seconds, .. } => Some(*seconds),
            _ => None,
        })
        .collect()
}

fn cents(hz: f64, of: f64) -> f64 {
    1200.0 * (hz / of).log2()
}

/// The reports from 0.2 seconds in to 0.1 seconds before the sound ends, once the window is full.
fn steady(
    measured: &[(f64, f64, Option<f64>, f64)],
    seconds: f64,
) -> Vec<(f64, f64, Option<f64>, f64)> {
    measured
        .iter()
        .copied()
        .filter(|m| m.0 > 0.2 && m.0 < seconds - 0.1)
        .collect()
}

#[test]
fn a_220_hz_sine_at_minus_12_db_reports_its_pitch_surely_and_its_loudness_at_minus_15_db() {
    let peak = 10f32.powf(-12.0 / 20.0);
    let reports = steady(
        &measured(&analysed(&sine(220.0, peak, 1.0), 0.01, 1.0)),
        1.0,
    );
    assert!(reports.len() > 50);
    for (seconds, loudness, pitch, confidence) in reports {
        let pitch = pitch.unwrap_or_else(|| panic!("no pitch at {seconds}"));
        assert!(cents(pitch, 220.0).abs() < 5.0, "{pitch} Hz at {seconds}");
        assert!(confidence > 0.9, "confidence {confidence} at {seconds}");
        assert!((loudness + 15.0).abs() < 0.5, "{loudness} dB at {seconds}");
    }
}

#[test]
fn white_noise_reports_no_pitch_or_a_confidence_below_0_3() {
    let mut seed = 1u64;
    let noise: Vec<f32> = (0..48_000)
        .map(|_| {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((seed >> 33) as f64 / (1u64 << 31) as f64 * 2.0 - 1.0) as f32 * 0.25
        })
        .collect();
    for (seconds, _, pitch, confidence) in steady(&measured(&analysed(&noise, 0.01, 1.0)), 1.0) {
        assert!(
            pitch.is_none() || confidence < 0.3,
            "{pitch:?} at {confidence} at {seconds}"
        );
    }
}

#[test]
fn sines_at_55_hz_and_1760_hz_report_their_pitch() {
    for hz in [55.0, 1760.0] {
        let reports = steady(&measured(&analysed(&sine(hz, 0.5, 1.0), 0.01, 1.0)), 1.0);
        assert!(!reports.is_empty());
        for (seconds, _, pitch, _) in reports {
            let pitch = pitch.unwrap_or_else(|| panic!("no pitch for {hz} Hz at {seconds}"));
            assert!(
                cents(pitch, hz).abs() < 5.0,
                "{pitch} Hz for {hz} Hz at {seconds}"
            );
        }
    }
}

#[test]
fn a_click_every_quarter_second_reports_one_onset_each_at_its_time() {
    let mut clicks = vec![0.0f32; 48_000 * 2];
    let at: Vec<usize> = (1..8).map(|k| k * 12_000).collect();
    for &frame in &at {
        for k in 0..48 {
            clicks[frame + k] = 0.5 * (1.0 - k as f32 / 48.0);
        }
    }
    let found = onsets(&analysed(&clicks, 0.01, 2.0));
    assert_eq!(found.len(), at.len(), "onsets at {found:?}");
    for (seconds, frame) in found.iter().zip(&at) {
        let off = seconds - *frame as f64 / RATE;
        assert!(off.abs() < 0.005, "onset {off} seconds off");
    }
}

#[test]
fn an_interval_of_20_ms_reports_every_20_ms_of_engine_time() {
    let times: Vec<f64> = measured(&analysed(&sine(220.0, 0.5, 1.0), 0.02, 1.0))
        .iter()
        .map(|m| m.0)
        .collect();
    assert!(times.len() >= 49, "{} reports", times.len());
    for pair in times.windows(2) {
        assert!((pair[1] - pair[0] - 0.02).abs() < 1e-9, "{pair:?}");
    }
}

fn tone() -> Synth {
    Synth::new("tone").build(|_, _| Note::rings(sine_hz::<f32>(330.0) * 0.3, 2.0))
}

#[test]
fn the_main_output_renders_the_same_samples_with_an_analysis_on_it() {
    let render = |analysing: bool| {
        let mut engine = Engine::offline(RATE);
        engine.register(tone());
        let _reports = engine.subscribe(&[Topic::Analysis]);
        if analysing {
            engine.analyse(Origin::Output, 0.01, 0.0).unwrap();
        }
        engine
            .play(&[(0.0, Event::note(None, "tone", &[]))])
            .unwrap();
        engine.render(1.0).unwrap()
    };
    assert_eq!(render(true), render(false));
}

#[test]
fn an_analysis_of_a_chain_hears_what_plays_into_it_and_stops_when_asked() {
    let mut engine = Engine::offline(RATE);
    engine.register(tone());
    mxs::effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    let reports = engine.subscribe(&[Topic::Analysis]);
    let chain = engine.start_effects(&[]).unwrap();
    let id = engine.analyse(Origin::Chain(chain), 0.01, 0.0).unwrap();
    let mut note = Event::note(None, "tone", &[]);
    if let Event::Note { chain: into, .. } = &mut note {
        *into = Some(chain);
    }
    engine.play(&[(0.0, note)]).unwrap();
    engine.render(0.5).unwrap();
    engine.stop_analysis(id, 0.5).unwrap();
    engine.render(0.5).unwrap();
    let heard = measured(&std::iter::from_fn(|| reports.wait(Duration::ZERO)).collect::<Vec<_>>());
    let late: Vec<_> = heard.iter().filter(|m| m.0 > 0.2).collect();
    assert!(!late.is_empty() && late.iter().all(|m| m.0 < 0.5));
    for m in late {
        assert!(cents(m.2.unwrap(), 330.0).abs() < 5.0);
    }
    assert_eq!(
        engine.stop_analysis(id, 1.0).unwrap_err().to_string(),
        format!("no analysis runs as {id}")
    );
}

#[test]
fn a_caller_that_takes_no_reports_for_10_seconds_is_told_how_many_were_dropped() {
    let mut engine = Engine::offline(RATE);
    let reports = engine.subscribe(&[Topic::Analysis]);
    let sample = engine
        .load_sample_bytes("input.wav", &wav(&sine(220.0, 0.5, 10.0)))
        .unwrap();
    engine.analyse(Origin::Sample(sample), 0.05, 0.0).unwrap();
    engine.render(10.0).unwrap();
    let Some(Report::Dropped {
        reports: dropped, ..
    }) = reports.wait(Duration::ZERO)
    else {
        panic!("no count of dropped reports");
    };
    let rest = std::iter::from_fn(|| reports.wait(Duration::ZERO)).count();
    // 200 reports of the loudness and the pitch, and the onset where the sine starts.
    assert_eq!(dropped as usize + rest, 201);
}
