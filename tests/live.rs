//! Checks of a live engine, which need BlackHole 2ch, a virtual sound card, so that nothing is heard, and a release build, as a debug one fills too slowly; run them with `cargo test --release --test live -- --ignored --test-threads 1`.

use fundsp::prelude::*;
use mxs::{
    Engine, Error, Event, Note, Origin, Pass, Report, Settings, Spec, Synth, Target, Tempo, Topic,
    synths,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime};

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

#[test]
#[ignore]
fn levels_reports_arrive_twenty_times_a_second_and_stop_once_unsubscribed() {
    let mut engine = live(512);
    let reports = engine.subscribe(&[Topic::Levels]);
    let start = engine.now() + 0.2;
    let notes: Vec<(f64, Event)> = (0..16)
        .map(|i| {
            let at = start + i as f64 * 0.125;
            (at, Event::note(None, "piano", &[("note", 60.0)]))
        })
        .collect();
    engine.play(&notes).unwrap();
    let began = Instant::now();
    let mut arrived = Vec::new();
    while began.elapsed() < Duration::from_secs(2) {
        if let Some(Report::Levels(levels)) = reports.wait(Duration::from_millis(100)) {
            arrived.push((engine.now() - levels.seconds, levels));
        }
    }
    let a_second = arrived.len() as f64 / 2.0;
    println!("{a_second} levels reports a second");
    assert!((18.0..=22.0).contains(&a_second), "{a_second}");
    assert!(arrived.iter().any(|(_, levels)| levels.peak[0] > 0.01));
    for pair in arrived.windows(2) {
        assert!((pair[1].1.seconds - pair[0].1.seconds - 0.05).abs() < 1e-9);
    }
    let latest = arrived.iter().map(|(after, _)| *after).fold(0.0, f64::max);
    println!(
        "a report arrived up to {:.1} ms after its last sample was filled",
        latest * 1000.0
    );
    assert!(latest < 0.1, "{latest}");
    reports.unsubscribe(Topic::Levels);
    assert_eq!(reports.wait(Duration::from_millis(300)), None);
    assert_eq!(engine.load().late, 0);
}

#[test]
#[ignore]
fn a_clock_report_comes_within_a_second_and_gives_when_the_sound_card_plays_a_sample() {
    let engine = live(512);
    let reports = engine.subscribe(&[Topic::Clock, Topic::Output]);
    let mut clocks = Vec::new();
    let mut output = None;
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(2500) {
        match reports.wait(Duration::from_millis(100)) {
            Some(Report::Clock {
                seconds,
                system,
                delay,
            }) => clocks.push((began.elapsed(), seconds, system, delay)),
            Some(report @ Report::Output { .. }) => output = Some(report),
            _ => {}
        }
    }
    assert!(clocks[0].0 < Duration::from_secs(1), "{:?}", clocks[0].0);
    assert!((3..=6).contains(&clocks.len()), "{}", clocks.len());
    let (now, at) = engine.clock();
    for (_, seconds, system, delay) in &clocks {
        assert_eq!(Some(*delay), engine.latency());
        let filled = at.unwrap() - Duration::from_secs_f64(now - seconds);
        let after = system.duration_since(filled).unwrap().as_secs_f64();
        assert!((after - delay).abs() < 0.002, "{after} against {delay}");
    }
    println!("the output's delay is {:.2} ms", clocks[0].3 * 1000.0);
    let Some(Report::Output {
        device,
        rate,
        delay_ms,
        ..
    }) = output
    else {
        panic!("no output report")
    };
    assert_eq!(device.as_deref(), Some("BlackHole 2ch"));
    assert_eq!(rate, 48_000.0);
    assert_eq!(delay_ms, engine.latency().map(|seconds| seconds * 1000.0));
}

#[test]
#[ignore]
fn three_notes_held_live_report_three_voices_and_a_late_buffer_is_sent_as_a_warning() {
    let mut engine = live(512);
    engine.register(stall());
    engine.register(level());
    let reports = engine.subscribe(&[Topic::Voices, Topic::Warnings]);
    std::thread::sleep(Duration::from_millis(300));
    let at = engine.now() + 0.1;
    let mut events: Vec<(f64, Event)> = (0..3)
        .map(|_| (at, Event::note(None, "level", &[("level", 0.0)])))
        .collect();
    events.push((at + 1.2, Event::note(None, "stall", &[])));
    engine.play(&events).unwrap();
    let mut sounding = Vec::new();
    let mut warnings = Vec::new();
    let began = Instant::now();
    while began.elapsed() < Duration::from_millis(3200) {
        match reports.wait(Duration::from_millis(100)) {
            Some(Report::Voices {
                sounding: count, ..
            }) => sounding.push(count),
            Some(Report::Warning { message, .. }) => warnings.push(message),
            _ => {}
        }
    }
    assert!(sounding.contains(&3), "{sounding:?}");
    assert_eq!(sounding.last(), Some(&0), "{sounding:?}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].starts_with("1 buffer reached the sound card late, by up to "));
    assert_eq!(engine.warnings(), warnings);
}

#[test]
#[ignore]
fn reports_left_untaken_for_ten_seconds_cost_no_buffer_and_the_next_says_how_many_were_dropped() {
    let mut engine = live(512);
    let reports = engine.subscribe(&[Topic::Levels, Topic::Voices]);
    std::thread::sleep(Duration::from_millis(500));
    engine.load();
    let start = engine.now() + 0.2;
    let notes: Vec<(f64, Event)> = (0..64)
        .map(|i| {
            let note = 48.0 + [0.0, 4.0, 7.0, 12.0][i % 4];
            let at = start + i as f64 * 0.125;
            (at, Event::note(None, "piano", &[("note", note)]))
        })
        .collect();
    engine.play(&notes).unwrap();
    std::thread::sleep(Duration::from_secs(10));
    let load = engine.load();
    assert_eq!(load.late, 0, "{load:?}");
    let Some(Report::Dropped {
        reports: dropped, ..
    }) = reports.next()
    else {
        panic!("no count of dropped reports")
    };
    println!("{dropped} reports dropped, {load:?}");
    assert!((130..=170).contains(&dropped), "{dropped}");
    let kept = std::iter::from_fn(|| reports.next()).count();
    assert!((64..=66).contains(&kept), "{kept}");
}

/// A level of 0.25 from the note's first frame, for 10 ms.
fn click() -> Synth {
    Synth::new("click").build(|_, _| Note::rings(dc(0.25), 0.01))
}

/// The frames at which sound starts after at least 1 ms of silence.
fn onsets(samples: &[f32]) -> Vec<usize> {
    let mut onsets = Vec::new();
    let mut silent = 48;
    for (frame, sample) in samples.iter().enumerate() {
        if sample.abs() > 0.01 {
            if silent >= 48 {
                onsets.push(frame);
            }
            silent = 0;
        } else {
            silent += 1;
        }
    }
    onsets
}

/// Ends the recording of the output at `path` and gives its left channel.
fn left_of(engine: &mut Engine, path: &std::path::Path) -> Vec<f32> {
    engine.stop_recording_output().unwrap();
    let samples: Vec<f32> = hound::WavReader::open(path)
        .unwrap()
        .samples::<f32>()
        .map(|s| s.unwrap())
        .collect();
    let _ = std::fs::remove_file(path);
    samples.chunks(2).map(|frame| frame[0]).collect()
}

#[test]
#[ignore]
fn events_in_beats_start_at_their_frames_live_and_a_tempo_change_and_a_cut_reach_those_to_come() {
    let mut engine = live(512);
    engine.register(click());
    engine.start_part("lead", &[]).unwrap();
    let reports = engine.subscribe(&[Topic::Beats, Topic::Passes]);
    let from = engine.now() + 0.1;
    let origin = from + 0.2;
    let path = std::env::temp_dir().join(format!("mxs-live-beats-{}.wav", std::process::id()));
    engine.record_output(&path, from).unwrap();
    engine.place_beats(origin).unwrap();
    let beats = (0..6).map(|beat| (f64::from(beat), Event::note(None, "click", &[])));
    let pass = Pass {
        from: 0.0,
        to: 6.0,
        label: 3,
        notice: Some(2.0),
        events: beats.collect(),
    };
    let began = Instant::now();
    engine.pass("lead", &pass).unwrap();
    let took = began.elapsed();
    assert!(took < Duration::from_millis(1), "{took:?}");

    // Beats 0 to 2 fall at 0, 0.5 and 1 s; from beat 2 on a beat lasts a quarter second, and beat 5 is cut.
    while engine.now() < origin + 0.6 {
        std::thread::sleep(Duration::from_millis(5));
    }
    engine.set_tempo(2.0, Tempo::Hold(240.0)).unwrap();
    engine.cut("lead", 5.0).unwrap();
    while engine.now() < from + 2.2 {
        std::thread::sleep(Duration::from_millis(10));
    }
    let left = left_of(&mut engine, &path);
    let frame = |seconds: f64| (seconds * 48_000.0).round() as usize;
    let expected: Vec<usize> = [0.0, 0.5, 1.0, 1.25, 1.5]
        .iter()
        .map(|after| frame(origin + after) - frame(from))
        .collect();
    assert_eq!(onsets(&left), expected);
    assert_eq!(engine.warnings(), Vec::<String>::new());

    let mut said = Vec::new();
    while let Some(report) = reports.next() {
        said.push(report);
    }
    let beats: Vec<(f64, f64, f64)> = said
        .iter()
        .filter_map(|report| match report {
            Report::Beat {
                seconds,
                beat,
                bpm,
                system,
            } => {
                // The system time of a beat is as far from now as the beat is on the engine's clock, give or take the output's delay and the wait above.
                let heard = system.expect("a live engine knows when a beat is heard");
                let ahead = heard
                    .duration_since(SystemTime::now())
                    .map_or_else(|e| -e.duration().as_secs_f64(), |d| d.as_secs_f64());
                let on_the_clock = seconds - engine.now();
                assert!((ahead - on_the_clock).abs() < 0.2, "{ahead} {on_the_clock}");
                Some((*seconds - origin, *beat, *bpm))
            }
            _ => None,
        })
        .collect();
    // The first is of the engine's start, with beat 0 at second 0, then one for the map as placed and one for the tempo change.
    assert_eq!(beats.len(), 3, "{said:?}");
    assert_eq!(beats[0], (-origin, 0.0, 120.0));
    assert!(beats[1].0.abs() < 1e-9 && beats[1].1 == 0.0 && beats[1].2 == 120.0);
    assert!((beats[2].0 - 1.0).abs() < 1e-9 && beats[2].1 == 2.0 && beats[2].2 == 240.0);
    let started = said.iter().find_map(|report| match report {
        Report::PassStarted { label, beat, .. } => Some((*label, *beat)),
        _ => None,
    });
    assert_eq!(started, Some((3, 0.0)));
}

#[test]
#[ignore]
fn a_pass_handed_over_for_the_beat_being_played_sounds_within_30_ms() {
    let mut engine = live(512);
    engine.register(click());
    engine.start_part("lead", &[]).unwrap();
    let from = engine.now() + 0.1;
    let path = std::env::temp_dir().join(format!("mxs-live-now-{}.wav", std::process::id()));
    engine.record_output(&path, from).unwrap();
    while engine.now() < from + 0.2 {
        std::thread::sleep(Duration::from_millis(5));
    }
    let (beat, handed) = (engine.beat(), engine.now());
    let pass = Pass {
        events: vec![(beat, Event::note(None, "click", &[]))],
        ..Pass::default()
    };
    engine.pass("lead", &pass).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let left = left_of(&mut engine, &path);
    let started = from + onsets(&left)[0] as f64 / 48_000.0;
    let after = (started - handed) * 1000.0;
    println!("the click started {after:.1} ms after it was handed over");
    assert!((0.0..30.0).contains(&after), "{after} ms");
}

/// The most an event arrived after its time, in ms, from the engine's warnings.
fn most_late(warnings: &[String]) -> Option<f64> {
    warnings.iter().find_map(|w| {
        let (_, after) = w.split_once("after their time, by up to ")?;
        after.split_once(" ms")?.0.parse().ok()
    })
}

#[test]
#[ignore]
fn a_chord_from_ready_voices_handed_over_now_starts_within_a_buffer_and_the_voices_are_replaced_within_100_ms()
 {
    let mut engine = live(128);
    engine.keep_ready("piano", 10).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    engine.warnings();
    let at = engine.now();
    let chord: Vec<(f64, Event)> = [36.0, 43.0, 48.0, 52.0, 55.0, 60.0, 64.0, 67.0, 71.0, 76.0]
        .iter()
        .map(|&note| {
            (
                at,
                Event::note(None, "piano", &[("note", note), ("vel", 0.7)]),
            )
        })
        .collect();
    let began = Instant::now();
    engine.play(&chord).unwrap();
    while engine.kept_ready("piano") < 10 {
        assert!(
            began.elapsed() < Duration::from_millis(100),
            "{} of 10 kept ready after 100 ms",
            engine.kept_ready("piano")
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    println!("the ready voices were replaced in {:?}", began.elapsed());
    std::thread::sleep(Duration::from_millis(300));
    assert!(engine.sounding());
    let warnings = engine.warnings();
    let late = most_late(&warnings).unwrap_or(0.0);
    println!("the chord was late by up to {late} ms: {warnings:?}");
    assert!(late <= 128.0 / 48.0 + 1.0, "{warnings:?}");
}

#[test]
#[ignore]
fn a_note_no_ready_voice_can_play_handed_over_after_its_time_starts_within_5_ms_of_it_and_is_counted_late()
 {
    let mut engine = live(128);
    engine.keep_ready("piano", 2).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    engine.warnings();
    let louder = Event::note(None, "piano", &[("note", 62.0), ("amp", 0.5)]);
    engine.play(&[(engine.now() - 0.001, louder)]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(engine.sounding());
    let warnings = engine.warnings();
    let late = most_late(&warnings).expect("the note is counted late");
    println!("the built note was late by up to {late} ms");
    assert!(
        warnings[0].starts_with("1 event arrived after their time"),
        "{warnings:?}"
    );
    assert!(late < 5.0, "{warnings:?}");
}

#[test]
#[ignore]
fn each_report_of_an_analysis_reaches_the_caller_within_30_ms_of_its_time() {
    let mut engine = live(512);
    let reports = engine.subscribe(&[Topic::Analysis]);
    std::thread::sleep(Duration::from_millis(500));
    let start = engine.now() + 0.1;
    engine.analyse(Origin::Output, 0.01, start).unwrap();
    let notes: Vec<(f64, Event)> = (0..8)
        .map(|k| {
            (
                start + 0.25 * k as f64,
                Event::note(None, "piano", &[("note", 57.0 + k as f64)]),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    let mut lags = Vec::new();
    let until = Instant::now() + Duration::from_secs(2);
    while Instant::now() < until {
        if let Some(Report::Analysis { seconds, .. }) = reports.wait(Duration::from_millis(50)) {
            lags.push(engine.now() - seconds);
        }
    }
    let worst = lags.iter().cloned().fold(f64::MIN, f64::max);
    println!(
        "{} reports, the latest {:.1} ms after its time",
        lags.len(),
        worst * 1000.0
    );
    assert!(lags.len() > 150);
    assert!(worst < 0.03);
}

#[test]
#[ignore]
fn an_analysis_whose_reports_go_untaken_for_10_seconds_leaves_no_buffer_late() {
    let mut engine = live(512);
    let reports = engine.subscribe(&[Topic::Analysis]);
    std::thread::sleep(Duration::from_millis(500));
    engine.load();
    let start = engine.now() + 0.1;
    engine.analyse(Origin::Output, 0.01, start).unwrap();
    let notes: Vec<(f64, Event)> = (0..40)
        .map(|k| {
            (
                start + 0.25 * k as f64,
                Event::note(None, "piano", &[("note", 57.0 + (k % 12) as f64)]),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    std::thread::sleep(Duration::from_secs(10));
    let load = engine.load();
    assert_eq!(load.late, 0, "{load:?}");
    let Some(Report::Dropped {
        reports: dropped, ..
    }) = reports.wait(Duration::ZERO)
    else {
        panic!("no count of dropped reports");
    };
    println!("{dropped} reports dropped, {load:?}");
    assert!(dropped > 900);
}

#[test]
#[ignore]
fn creating_a_60_second_stereo_buffer_while_piano_plays_makes_no_event_late() {
    let mut engine = live(512);
    std::thread::sleep(Duration::from_millis(500));
    engine.load();
    engine.warnings();
    let start = engine.now() + 0.2;
    let notes: Vec<(f64, Event)> = (0..32)
        .map(|i| {
            let note = 48.0 + [0.0, 4.0, 7.0, 12.0][i % 4] + (i / 16) as f64 * 2.0;
            (
                start + i as f64 * 0.125,
                Event::note(None, "piano", &[("note", note)]),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    std::thread::sleep(Duration::from_secs(1));
    let created = Instant::now();
    engine.create_buffer(60.0, 2).unwrap();
    let took = created.elapsed();
    let later: Vec<(f64, Event)> = (0..16)
        .map(|i| {
            (
                engine.now() + 0.1 + i as f64 * 0.125,
                Event::note(None, "piano", &[("note", 60.0)]),
            )
        })
        .collect();
    engine.play(&later).unwrap();
    std::thread::sleep(Duration::from_secs(4));
    let load = engine.load();
    let warnings = engine.warnings();
    println!("created in {took:?}, {load:?}, {warnings:?}");
    assert_eq!(load.late, 0, "{load:?}");
    assert!(
        warnings.iter().all(|w| !w.contains("after their time")),
        "{warnings:?}"
    );
}

#[test]
#[ignore]
fn a_texture_of_440_held_piano_notes_plays_for_10_minutes_with_no_buffer_late() {
    let mut engine = live(512);
    engine.set_voice_limit(500);
    std::thread::sleep(Duration::from_millis(500));
    let start = engine.now() + 0.2;
    let notes: Vec<(f64, Event)> = (0..440)
        .map(|i| {
            let note = 36.0 + (i % 48) as f64;
            (
                start,
                Event::note(None, "piano", &[("note", note), ("sustain", 620.0)]),
            )
        })
        .collect();
    for batch in notes.chunks(200) {
        engine.play(batch).unwrap();
    }
    std::thread::sleep(Duration::from_secs(5));
    engine.load();
    engine.warnings();
    let mut late = 0;
    let mut worst: f64 = 0.0;
    for _ in 0..60 {
        std::thread::sleep(Duration::from_secs(10));
        let load = engine.load();
        late += load.late;
        worst = worst.max(load.worst);
    }
    println!(
        "{late} buffers late in 10 minutes, the worst {worst:.2} of a buffer's time on {} workers",
        engine.workers()
    );
    assert_eq!(late, 0);
}
