use fundsp::prelude::*;
use mxs::{
    Engine, Error, Event, Note, RATE, Report, Spec, Synth, SynthDecl, Topic, Value, effects, synths,
};

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    engine.register(tone());
    engine
}

/// A sine at `hz` that peaks at 0.25, held for three seconds.
fn tone() -> Synth {
    SynthDecl::new("tone")
        .option(Spec::float("hz").default(440.0).range(20.0, 20_000.0))
        .build(|options, _| Note::rings(sine_hz::<f32>(options.number("hz") as f32) * 0.25, 3.0))
}

fn note(hz: f64, part: u64) -> Event {
    Event::Note {
        id: None,
        synth: "tone".to_string(),
        options: vec![("hz".to_string(), Value::Number(hz))],
        chain: Some(part),
        held: false,
    }
}

fn frame(seconds: f64) -> usize {
    (seconds * RATE).round() as usize
}

/// The level in dB of the partial at `hz` over a quarter second from `from`, which holds a whole number of cycles of 440, 660 and 880 Hz, so none of them shows at another's frequency.
fn partial(samples: &[f32], hz: f64, from: f64) -> f64 {
    let window = &samples[frame(from)..frame(from) + frame(0.25)];
    let (mut sin, mut cos) = (0.0, 0.0);
    for (i, x) in window.iter().enumerate() {
        let angle = std::f64::consts::TAU * hz * i as f64 / RATE;
        sin += f64::from(*x) * angle.sin();
        cos += f64::from(*x) * angle.cos();
    }
    20.0 * (2.0 * sin.hypot(cos) / window.len() as f64).log10()
}

/// An engine with a 440 Hz sine in the part first and a 660 Hz sine in the part second, both from 0, and a note at 880 Hz in first handed over at once to start at 1.5 seconds.
fn two_parts() -> Engine {
    let mut engine = engine();
    let first = engine.start_part("first", &[]).unwrap();
    let second = engine.start_part("second", &[]).unwrap();
    engine
        .play(&[
            (0.0, note(440.0, first)),
            (0.0, note(660.0, second)),
            (1.5, note(880.0, first)),
        ])
        .unwrap();
    engine
}

#[test]
fn a_muted_part_is_silent_within_10_ms_with_what_was_handed_over_before_and_comes_back_in_phase() {
    let (untouched, _) = two_parts().render(3.0).unwrap();
    let mut engine = two_parts();
    engine.mute("first", true, 1.0).unwrap();
    engine.mute("first", false, 2.0).unwrap();
    let (left, right) = engine.render(3.0).unwrap();
    assert_eq!(left, right);

    assert_eq!(left[..frame(1.0)], untouched[..frame(1.0)]);
    let before = partial(&left, 440.0, 0.5);
    assert!((before - partial(&untouched, 440.0, 0.5)).abs() < 1e-9 && before > -20.0);
    let muted = partial(&left, 440.0, 1.01);
    assert!(muted < -80.0, "{muted} dB at 440 Hz once muted");
    let other = partial(&left, 660.0, 1.01) - partial(&untouched, 660.0, 1.01);
    assert!(other.abs() < 1e-4, "the other part moved by {other} dB");

    let handed_over = partial(&left, 880.0, 1.5);
    assert!(handed_over < -80.0, "{handed_over} dB at 880 Hz");
    assert!(partial(&untouched, 880.0, 1.5) > -20.0);

    assert_eq!(left[frame(2.01)..], untouched[frame(2.01)..]);
    assert_ne!(
        left[frame(2.0)..frame(2.01)],
        untouched[frame(2.0)..frame(2.01)]
    );
}

#[test]
fn a_part_muted_or_left_out_of_a_solo_before_it_makes_a_sound_is_silent_from_its_first_frame() {
    let mut engine = two_parts();
    engine.mute("first", true, 0.0).unwrap();
    engine.solo("first", true, 0.0).unwrap();
    let (left, _) = engine.render(1.0).unwrap();
    let heard = left.iter().position(|x| *x != 0.0);
    assert_eq!(heard, None, "heard from frame {heard:?} of {}", left.len());
}

#[test]
fn a_solo_leaves_only_the_soloed_parts_heard_until_it_is_cleared() {
    let (untouched, _) = two_parts().render(3.0).unwrap();
    let mut engine = two_parts();
    engine.solo("second", true, 1.0).unwrap();
    engine.solo("second", false, 2.0).unwrap();
    let (left, _) = engine.render(3.0).unwrap();
    assert!(partial(&left, 440.0, 1.01) < -80.0);
    assert!(partial(&left, 880.0, 1.5) < -80.0);
    assert!((partial(&left, 660.0, 1.01) - partial(&untouched, 660.0, 1.01)).abs() < 1e-4);
    assert_eq!(left[frame(2.01)..], untouched[frame(2.01)..]);
}

#[test]
fn a_part_muted_and_soloed_stays_silent_and_a_sound_in_no_part_is_heard_through_a_solo() {
    let mut engine = two_parts();
    engine
        .play(&[(0.0, Event::note(None, "tone", &[("hz", 1320.0)]))])
        .unwrap();
    engine.solo("first", true, 1.0).unwrap();
    engine.mute("first", true, 1.0).unwrap();
    let (left, _) = engine.render(2.0).unwrap();
    assert!(partial(&left, 440.0, 1.01) < -80.0);
    assert!(partial(&left, 660.0, 1.01) < -80.0);
    assert!(partial(&left, 1320.0, 1.01) > -20.0);
}

#[test]
fn a_gain_of_a_half_lowers_the_part_by_6_db_from_10_ms_on() {
    let mut engine = two_parts();
    engine.set_gain("first", 0.5, 1.0).unwrap();
    let (left, _) = engine.render(2.0).unwrap();
    let fell = partial(&left, 440.0, 0.5) - partial(&left, 440.0, 1.01);
    assert!((fell - 6.02).abs() < 0.1, "{fell} dB");
    let other = partial(&left, 660.0, 0.5) - partial(&left, 660.0, 1.01);
    assert!(other.abs() < 1e-4, "{other} dB");
}

#[test]
fn muting_a_part_with_a_reverb_silences_the_tail_too() {
    let reverb = [("reverb".to_string(), vec![])];
    let render = |mute: bool| {
        let mut engine = engine();
        let part = engine.start_part("hall", &reverb).unwrap();
        engine
            .play(&[(
                0.0,
                Event::Note {
                    id: None,
                    synth: "piano".to_string(),
                    options: vec![("sustain".to_string(), Value::Number(0.3))],
                    chain: Some(part),
                    held: false,
                },
            )])
            .unwrap();
        if mute {
            engine.mute("hall", true, 1.0).unwrap();
        }
        engine.render(2.0).unwrap().0
    };
    let (ringing, muted) = (render(false), render(true));
    let loudest = |samples: &[f32]| samples.iter().fold(0f32, |most, x| most.max(x.abs()));
    assert!(
        loudest(&ringing[frame(1.01)..]) > 0.001,
        "the reverb has no tail to mute"
    );
    assert_eq!(loudest(&muted[frame(1.01)..]), 0.0);
    assert_eq!(muted[..frame(1.0)], ringing[..frame(1.0)]);
}

#[test]
fn a_recording_of_the_output_holds_what_is_heard_without_the_muted_part() {
    let dir = std::env::temp_dir().join(format!("mxs-parts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("output.wav");
    let mut engine = two_parts();
    engine.mute("first", true, 0.5).unwrap();
    engine.record_output(&path, 1.0).unwrap();
    engine.render(2.0).unwrap();
    engine.stop_recording_output().unwrap();
    let mut reader = hound::WavReader::open(&path).unwrap();
    let left: Vec<f32> = reader
        .samples::<f32>()
        .map(Result::unwrap)
        .step_by(2)
        .collect();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(partial(&left, 440.0, 0.0) < -80.0);
    assert!(partial(&left, 660.0, 0.0) > -20.0);
}

#[test]
fn the_levels_report_gives_each_part_its_level_by_name_and_0_while_muted() {
    let mut engine = two_parts();
    let reports = engine.subscribe(&[Topic::Levels]);
    engine.mute("first", true, 0.5).unwrap();
    let mut levels = Vec::new();
    for _ in 0..4 {
        engine.render(0.25).unwrap();
        levels.extend(
            std::iter::from_fn(|| reports.next()).map(|report| match report {
                Report::Levels(levels) => *levels,
                other => panic!("{other:?}"),
            }),
        );
    }
    assert_eq!(levels.len(), 20);
    for report in &levels {
        let names: Vec<&str> = report.parts.iter().map(|part| part.name.as_str()).collect();
        assert_eq!(names, ["first", "second"]);
        let (first, second) = (&report.parts[0], &report.parts[1]);
        let db = |level: f32| 20.0 * level.log10();
        assert!((db(second.peak[0]) - db(0.25 * 0.5f32.sqrt())).abs() < 0.01);
        assert!((db(second.rms[0]) - db(0.25 * 0.5)).abs() < 0.01);
        if report.seconds <= 0.5 {
            assert!((db(first.peak[1]) - db(0.25 * 0.5f32.sqrt())).abs() < 0.01);
        } else if report.seconds > 0.55 {
            assert_eq!((first.peak, first.rms), ([0.0; 2], [0.0; 2]));
        }
    }
}

#[test]
fn a_part_that_was_not_started_is_refused_by_its_name() {
    let mut engine = two_parts();
    assert_eq!(
        engine.part("nowhere").unwrap_err(),
        Error::UnknownPart("nowhere".to_string())
    );
    assert_eq!(
        engine.part("nowhere").unwrap_err().to_string(),
        "no part named nowhere"
    );
    for refused in [
        engine.mute("nowhere", true, 0.0),
        engine.solo("nowhere", true, 0.0),
        engine.set_gain("nowhere", 0.5, 0.0),
        engine.stop_part("nowhere", 0.0),
    ] {
        assert_eq!(refused, Err(Error::UnknownPart("nowhere".to_string())));
    }
    assert_eq!(
        engine.start_part("first", &[]).unwrap_err().to_string(),
        "a part named first is already started"
    );
}

#[test]
fn a_part_removed_plays_its_sounds_out_and_its_name_is_free_at_once() {
    let (untouched, _) = two_parts().render(3.0).unwrap();
    let mut engine = two_parts();
    let first = engine.part("first").unwrap();
    engine.stop_part("first", 0.5).unwrap();
    assert!(engine.part("first").is_err());
    let again = engine.start_part("first", &[]).unwrap();
    assert_ne!(again, first);
    let (left, _) = engine.render(3.0).unwrap();
    assert_eq!(left, untouched);
}

#[test]
fn piano_notes_in_a_part_at_gain_1_render_the_same_samples_as_without_parts() {
    let render = |part: bool| {
        let mut engine = engine();
        let chain = part.then(|| engine.start_part("piano", &[]).unwrap());
        let phrase: Vec<(f64, Event)> = [60.0, 64.0, 67.0, 72.0, 48.0, 55.0, 64.0, 76.0]
            .iter()
            .enumerate()
            .map(|(i, key)| {
                let options = vec![
                    ("note".to_string(), Value::Number(*key)),
                    ("vel".to_string(), Value::Number(0.4 + 0.07 * i as f64)),
                ];
                let synth = "piano".to_string();
                (
                    i as f64 * 0.2,
                    Event::Note {
                        id: None,
                        synth,
                        options,
                        chain,
                        held: false,
                    },
                )
            })
            .collect();
        engine.play(&phrase).unwrap();
        engine.render(3.0).unwrap()
    };
    let (without, with) = (render(false), render(true));
    assert!(without.0.iter().any(|x| x.abs() > 0.01));
    assert_eq!(without, with);
}
