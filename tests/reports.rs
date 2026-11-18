use fundsp::prelude::*;
use mxs::{Engine, Event, Levels, Note, RATE, Report, Reports, Synth, Topic, synths};

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.register(tone());
    engine
}

/// A sine at 440 Hz that peaks at -12 dB in each channel once it is placed in the centre, held for two seconds.
fn tone() -> Synth {
    let peak = 10f32.powf(-12.0 / 20.0) * std::f32::consts::SQRT_2;
    Synth::new("tone").build(move |_, _| Note::rings(sine_hz::<f32>(440.0) * peak, 2.0))
}

fn db(level: f32) -> f32 {
    20.0 * level.log10()
}

fn taken(reports: &Reports) -> Vec<Report> {
    std::iter::from_fn(|| reports.next()).collect()
}

fn levels(reports: &[Report]) -> Vec<&Levels> {
    reports
        .iter()
        .filter_map(|report| match report {
            Report::Levels(levels) => Some(&**levels),
            _ => None,
        })
        .collect()
}

/// The reports of the tone played for two seconds, taken every half second as it renders.
fn tone_reports(topics: &[Topic], amp: f64) -> Vec<Report> {
    let mut engine = engine();
    let reports = engine.subscribe(topics);
    engine
        .play(&[(0.0, Event::note(None, "tone", &[("amp", amp)]))])
        .unwrap();
    let mut all = Vec::new();
    for _ in 0..4 {
        engine.render(0.5).unwrap();
        all.extend(taken(&reports));
    }
    all
}

#[test]
fn a_sine_peaking_at_minus_12_db_reports_that_peak_and_an_rms_level_3_db_under_it() {
    let reports = tone_reports(&[Topic::Levels], 1.0);
    let levels = levels(&reports);
    assert_eq!(levels.len(), 40);
    for (index, report) in levels.iter().enumerate() {
        assert!((report.seconds - (index + 1) as f64 * 0.05).abs() < 1e-9);
    }
    for report in &levels[1..38] {
        for channel in 0..2 {
            let (peak, rms) = (db(report.peak[channel]), db(report.rms[channel]));
            assert!((peak + 12.0).abs() < 0.1, "peak {peak} dB");
            assert!((rms + 15.0).abs() < 0.1, "rms {rms} dB");
        }
        assert!(!report.full_scale);
        let (lowest, highest) = report
            .wave
            .iter()
            .fold((0f32, 0f32), |(low, high), x| (low.min(*x), high.max(*x)));
        assert!((db(highest) + 12.0).abs() < 0.2 && (db(-lowest) + 12.0).abs() < 0.2);
    }
}

#[test]
fn the_sine_is_loudest_in_the_band_that_holds_440_hz_and_30_db_lower_an_octave_away() {
    let reports = tone_reports(&[Topic::Levels], 1.0);
    let band = |hz: f64| {
        (0..mxs::BANDS)
            .find(|&i| Levels::band(i).0 <= hz && hz < Levels::band(i).1)
            .unwrap()
    };
    for report in &levels(&reports)[10..38] {
        let loudest = (0..mxs::BANDS)
            .max_by(|a, b| report.bands[*a].total_cmp(&report.bands[*b]))
            .unwrap();
        assert_eq!(loudest, band(440.0));
        let own = report.bands[loudest];
        assert!((own + 15.0).abs() < 0.2, "{own} dB");
        for octave in [220.0, 880.0] {
            let away = report.bands[band(octave)];
            assert!(away < own - 30.0, "{away} dB at {octave} Hz");
        }
    }
}

#[test]
fn a_sound_at_amp_4_reports_that_the_output_reached_full_scale() {
    let quiet = tone_reports(&[Topic::Levels], 1.0);
    assert!(levels(&quiet).iter().all(|report| !report.full_scale));
    let loud = tone_reports(&[Topic::Levels], 4.0);
    assert!(levels(&loud)[1..38].iter().all(|report| report.full_scale));
}

#[test]
fn three_notes_held_report_three_voices_and_silence_none() {
    let mut engine = engine();
    let reports = engine.subscribe(&[Topic::Voices]);
    let chord: Vec<_> = (0..3)
        .map(|_| (0.0, Event::note(None, "tone", &[])))
        .collect();
    engine.play(&chord).unwrap();
    engine.render(4.0).unwrap();
    let voices: Vec<(f64, usize)> = taken(&reports)
        .iter()
        .map(|report| match report {
            Report::Voices { seconds, sounding } => (*seconds, *sounding),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(voices, [(0.25, 3), (1.0, 3), (2.0, 3), (3.0, 0), (4.0, 0)]);
}

#[test]
fn a_render_with_every_kind_subscribed_gives_the_same_samples_as_one_with_none() {
    let render = |topics: &[Topic]| {
        let mut engine = engine();
        let _reports = engine.subscribe(topics);
        let phrase = [
            (0.0, Event::note(None, "piano", &[("note", 60.0)])),
            (0.25, Event::note(None, "berimbau", &[])),
            (0.5, Event::note(None, "tone", &[])),
        ];
        engine.play(&phrase).unwrap();
        engine.render(2.0).unwrap()
    };
    let every = [
        Topic::Clock,
        Topic::Levels,
        Topic::Voices,
        Topic::Output,
        Topic::Warnings,
    ];
    assert_eq!(render(&every), render(&[]));
}

#[test]
fn two_renders_of_the_same_events_give_the_same_reports() {
    let render = || {
        let mut engine = engine();
        let reports = engine.subscribe(&[
            Topic::Clock,
            Topic::Levels,
            Topic::Voices,
            Topic::Output,
            Topic::Warnings,
        ]);
        engine.set_voice_limit(1);
        let phrase = [
            (0.0, Event::note(None, "piano", &[("note", 60.0)])),
            (0.25, Event::note(None, "berimbau", &[])),
            (0.5, Event::note(None, "tone", &[("amp", 99.0)])),
        ];
        engine.play(&phrase).unwrap();
        let mut all = Vec::new();
        for _ in 0..8 {
            engine.render(0.25).unwrap();
            all.extend(taken(&reports));
        }
        all
    };
    let (first, second) = (render(), render());
    assert_eq!(first, second);
    assert_eq!(levels(&first).len(), 40);
    assert!(matches!(
        &first[0],
        Report::Output { device: None, rate, delay_ms: None, .. } if *rate == RATE
    ));
    let warnings: Vec<&str> = first
        .iter()
        .filter_map(|report| match report {
            Report::Warning { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        warnings,
        [
            "tone's option amp was held to its range, 0 to 16",
            "1 note was stolen to keep to 1 at once",
            "1 note was stolen to keep to 1 at once"
        ]
    );
    assert!(!first.iter().any(|r| matches!(r, Report::Clock { .. })));
}

#[test]
fn the_warnings_are_still_given_when_asked_and_also_sent_as_reports() {
    let mut engine = engine();
    let reports = engine.subscribe(&[Topic::Warnings]);
    engine
        .play(&[(0.0, Event::note(None, "tone", &[("amp", 99.0)]))])
        .unwrap();
    let held = "tone's option amp was held to its range, 0 to 16";
    assert!(
        matches!(reports.next(), Some(Report::Warning { seconds, message }) if seconds == 0.0 && message == held)
    );
    assert_eq!(engine.warnings(), [held]);
    assert!(engine.warnings().is_empty());
}

#[test]
fn reports_not_taken_are_dropped_oldest_first_and_the_next_says_how_many() {
    let mut engine = engine();
    let reports = engine.subscribe(&[Topic::Levels]);
    engine.render(10.0).unwrap();
    let all = taken(&reports);
    assert_eq!(
        all[0],
        Report::Dropped {
            seconds: 6.85,
            reports: 136
        }
    );
    assert_eq!(levels(&all).len(), 64);
    assert_eq!(all.last().unwrap().seconds(), 10.0);
}

#[test]
fn no_report_of_a_kind_is_given_once_it_is_unsubscribed() {
    let mut engine = engine();
    let reports = engine.subscribe(&[Topic::Levels, Topic::Voices]);
    engine.render(1.0).unwrap();
    reports.unsubscribe(Topic::Levels);
    engine.render(1.0).unwrap();
    let all = taken(&reports);
    assert!(levels(&all).is_empty());
    assert_eq!(all.len(), 3);
    reports.subscribe(Topic::Levels);
    engine.render(0.5).unwrap();
    assert_eq!(levels(&taken(&reports)).len(), 10);
}
