use fundsp::prelude::*;
use mxs::nodes::{hz, mode, white_noise};
use mxs::{Effect, Engine, Error, Event, Note, RATE, Source, Spec, Synth, Target, Value, synths};

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
}

/// A synth whose sound is its `level` option, held, for a second.
fn level() -> Synth {
    Synth::new("level")
        .option(Spec::float("level").default(1.0).range(0.0, 2.0))
        .build(|options, _| Note::rings(options.follow("level"), 1.0))
}

fn frame(seconds: f64) -> usize {
    (seconds * RATE) as usize
}

fn options(pairs: &[(&str, f64)]) -> Vec<(String, Value)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Value::Number(*v)))
        .collect()
}

#[test]
fn a_synth_of_ones_own_registers_and_renders_the_same_samples_on_every_run() {
    let render = || {
        let mut engine = engine();
        engine.register(
            Synth::new("plate")
                .option(Spec::note("note").default(60.0))
                .option(Spec::float("vel").default(0.6).range(0.0, 1.0))
                .build(|options, ctx| {
                    let freq = hz(options.number("note"));
                    let struck = Net::wrap(Box::new(impulse::<U1>()))
                        >> (mode(freq, 0.0, 1.0, 3.0) & mode(freq * 2.76, 0.0, 0.5, 1.2));
                    let tap = (white_noise(ctx.seed()) >> highpass_hz(4000.0, 0.7)) * 0.01;
                    Note::rings((struck + tap) * options.number("vel") as f32, 4.0)
                }),
        );
        let phrase: Vec<_> = [60.0, 64.0, 67.0, 72.0]
            .iter()
            .enumerate()
            .map(|(i, note)| {
                (
                    i as f64 * 0.25,
                    Event::note(None, "plate", &[("note", *note), ("vel", 0.8)]),
                )
            })
            .chain([
                (0.5, Event::note(None, "piano", &[("note", 48.0)])),
                (0.7, Event::note(None, "berimbau", &[])),
            ])
            .collect();
        engine.play(&phrase).unwrap();
        engine.render(3.0).unwrap()
    };
    let (first, second) = (render(), render());
    assert!(first.0.iter().any(|x| x.abs() > 0.01));
    assert_eq!(first, second);
}

#[test]
fn an_option_a_synth_does_not_declare_is_an_error_naming_it() {
    let mut engine = engine();
    let error = engine
        .play(&[(0.0, Event::note(None, "berimbau", &[("sustain", 1.0)]))])
        .unwrap_err();
    assert_eq!(
        error,
        Error::UnknownOption {
            owner: "berimbau".into(),
            option: "sustain".into()
        }
    );
    assert_eq!(error.to_string(), "berimbau has no option sustain");
    let error = engine
        .play(&[(0.0, Event::note(None, "harpsichord", &[]))])
        .unwrap_err();
    assert_eq!(error.to_string(), "no synth named harpsichord");
}

#[test]
fn a_batch_with_a_mistake_plays_nothing() {
    let mut engine = engine();
    engine.register(level());
    let batch = [
        (0.0, Event::note(None, "level", &[])),
        (0.0, Event::note(None, "level", &[("loudness", 1.0)])),
    ];
    assert!(engine.play(&batch).is_err());
    let (left, _) = engine.render(0.5).unwrap();
    assert!(left.iter().all(|&x| x == 0.0));
}

#[test]
fn a_value_outside_its_range_is_held_to_it_and_reported_once() {
    let mut engine = engine();
    engine.register(level());
    let loud = Event::note(None, "level", &[("level", 5.0)]);
    engine.play(&[(0.0, loud.clone()), (0.0, loud)]).unwrap();
    assert_eq!(
        engine.warnings(),
        vec!["level's option level was held to its range, 0 to 2".to_string()]
    );
    let (left, _) = engine.render(0.5).unwrap();
    // Two notes at level 2, panned to the centre at equal power.
    assert!((left[frame(0.1)] - 4.0 * 0.5f32.sqrt()).abs() < 1e-4);
    engine
        .play(&[(1.0, Event::note(None, "level", &[("level", 5.0)]))])
        .unwrap();
    assert!(engine.warnings().is_empty());
}

#[test]
fn a_named_option_takes_only_its_names() {
    let mut engine = engine();
    engine.register(
        Synth::new("bol")
            .option(Spec::name("bol", &["na", "tun"]).default("na"))
            .build(|options, _| {
                Note::rings(
                    dc(if options.name("bol") == "na" {
                        1.0
                    } else {
                        0.5
                    }),
                    1.0,
                )
            }),
    );
    let tun = Event::Note {
        id: None,
        synth: "bol".into(),
        options: vec![("bol".into(), Value::from("tun"))],
        chain: None,
        held: false,
    };
    engine.play(&[(0.0, tun)]).unwrap();
    let dha = Event::Note {
        id: None,
        synth: "bol".into(),
        options: vec![("bol".into(), Value::from("dha"))],
        chain: None,
        held: false,
    };
    assert_eq!(
        engine.play(&[(0.0, dha)]).unwrap_err().to_string(),
        "bol's option bol takes one of na, tun"
    );
}

#[test]
fn every_note_of_a_batch_of_3000_sounds() {
    let mut engine = engine();
    engine.register(Synth::new("click").build(|_, _| Note::rings(dc(1.0), 0.001)));
    let clicks: Vec<_> = (0..3000)
        .map(|i| (i as f64 * 0.002, Event::note(None, "click", &[])))
        .collect();
    engine.play(&clicks).unwrap();
    let (left, _) = engine.render(6.5).unwrap();
    let starts = left
        .windows(2)
        .filter(|w| w[0] == 0.0 && w[1] > 0.0)
        .count()
        + usize::from(left[0] > 0.0);
    assert_eq!(starts, 3000);
}

#[test]
fn a_note_starts_at_its_sample_and_fades_out_over_its_last_10_ms() {
    let mut engine = engine();
    engine.register(level());
    engine
        .play(&[(0.5, Event::note(None, "level", &[]))])
        .unwrap();
    let (left, _) = engine.render(2.0).unwrap();
    assert_eq!(left[frame(0.5) - 1], 0.0);
    assert!(left[frame(0.5)] > 0.7);
    assert!(left[frame(1.495)] < left[frame(1.4)] * 0.6);
    assert_eq!(left[frame(1.5)], 0.0);
}

#[test]
fn a_change_glides_a_playing_notes_option_over_its_slide() {
    let mut engine = engine();
    engine.register(level());
    engine
        .play(&[(0.0, Event::note(Some(7), "level", &[("level", 1.0)]))])
        .unwrap();
    let change = Event::Set {
        target: Target::Sounds(vec![7]),
        options: vec![("level".into(), 0.0), ("level_slide".into(), 0.4)],
    };
    engine.play(&[(0.2, change)]).unwrap();
    let (left, _) = engine.render(0.9).unwrap();
    let centre = 0.5f32.sqrt();
    assert!((left[frame(0.1)] - centre).abs() < 1e-4);
    assert!((left[frame(0.4)] - centre * 0.5).abs() < 0.01);
    assert!(left[frame(0.7)].abs() < 1e-4);
}

#[test]
fn a_change_reaches_a_note_of_the_same_batch() {
    let mut engine = engine();
    engine.register(level());
    let note = Event::note(Some(3), "level", &[]);
    let change = Event::Set {
        target: Target::Sounds(vec![3]),
        options: vec![("amp".into(), 0.0)],
    };
    engine.play(&[(0.0, note), (0.2, change)]).unwrap();
    let (left, _) = engine.render(0.5).unwrap();
    assert!(left[frame(0.1)] > 0.5);
    assert_eq!(left[frame(0.3)], 0.0);
}

/// A held level note with changes to its options at `changes`, each a frame and its options, rendered for half a second.
fn changed(changes: &[(usize, Vec<(String, f64)>)]) -> Vec<f32> {
    let mut engine = engine();
    engine.register(level());
    engine
        .play(&[(0.0, Event::note(Some(5), "level", &[]))])
        .unwrap();
    let events: Vec<_> = changes
        .iter()
        .map(|(at, options)| {
            (
                *at as f64 / RATE,
                Event::Set {
                    target: Target::Sounds(vec![5]),
                    options: options.clone(),
                },
            )
        })
        .collect();
    engine.play(&events).unwrap();
    engine.render(0.5).unwrap().0
}

#[test]
fn a_change_applies_at_its_exact_sample() {
    let at = frame(0.20025);
    let left = changed(&[(at, vec![("amp".into(), 0.0)])]);
    let centre = 0.5f32.sqrt();
    assert!((left[at - 1] - centre).abs() < 1e-6, "{}", left[at - 1]);
    assert!(left[at..].iter().all(|&x| x == 0.0), "{}", left[at]);
}

#[test]
fn two_changes_in_one_block_each_apply_at_their_own_sample() {
    let left = changed(&[
        (9620, vec![("level".into(), 0.5)]),
        (9630, vec![("level".into(), 0.25)]),
    ]);
    let centre = 0.5f32.sqrt();
    assert_eq!(9620 / 64, 9630 / 64, "both fall in one block");
    assert!((left[9619] - centre).abs() < 1e-6);
    assert!((left[9620] - centre * 0.5).abs() < 1e-6 && (left[9629] - centre * 0.5).abs() < 1e-6);
    assert!((left[9630] - centre * 0.25).abs() < 1e-6);
}

#[test]
fn a_slide_starts_gliding_at_its_changes_sample() {
    let at = frame(0.20025);
    let left = changed(&[(at, vec![("level".into(), 0.0), ("level_slide".into(), 0.1)])]);
    let centre = 0.5f32.sqrt();
    assert!((left[at - 1] - centre).abs() < 1e-6);
    assert!(
        (left[at + frame(0.05)] - centre * 0.5).abs() < 1e-3,
        "{}",
        left[at + frame(0.05)]
    );
    assert!(
        left[at + frame(0.1) + 1].abs() < 1e-4,
        "{}",
        left[at + frame(0.1) + 1]
    );
}

#[test]
fn amp_and_pan_place_every_note() {
    let mut engine = engine();
    engine.register(level());
    engine
        .play(&[(
            0.0,
            Event::note(None, "level", &[("amp", 0.5), ("pan", -1.0)]),
        )])
        .unwrap();
    let (left, right) = engine.render(0.5).unwrap();
    assert!((left[frame(0.1)] - 0.5).abs() < 1e-4);
    assert!(right[frame(0.1)].abs() < 1e-4);
}

#[test]
fn notes_play_through_their_effect_chain_until_it_stops() {
    let mut engine = engine();
    engine.register(level());
    engine.register_effect(
        Effect::new("double")
            .tail(0.5)
            .build(|_, _| Net::wrap(Box::new(multipass::<U2>() * 2.0))),
    );
    let chain = engine
        .start_effects(&[("double".to_string(), options(&[("mix", 1.0)]))])
        .unwrap();
    let note = Event::Note {
        id: None,
        synth: "level".into(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(0.0, note)]).unwrap();
    engine.stop_effects(chain, 0.2).unwrap();
    let (left, _) = engine.render(1.0).unwrap();
    assert!((left[frame(0.5)] - 2.0 * 0.5f32.sqrt()).abs() < 1e-4);
    let half = Event::Set {
        target: Target::Effect(chain, 0),
        options: vec![("mix".into(), 0.5)],
    };
    assert!(engine.play(&[(1.1, half)]).is_ok());
    // The chain is gone once its note and tail have ended, so a note sent into it plays dry.
    engine.render(1.0).unwrap();
    let late = Event::Note {
        id: None,
        synth: "level".into(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(2.0, late)]).unwrap();
    let (left, _) = engine.render(0.5).unwrap();
    assert!((left[frame(0.1)] - 0.5f32.sqrt()).abs() < 1e-4);
}

#[test]
fn a_loaded_sample_plays_at_its_rate_within_its_span() {
    let dir = std::env::temp_dir().join(format!("mxs-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ramp.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..48_000 {
        wav.write_sample(i as f32 / 48_000.0).unwrap();
    }
    wav.finalize().unwrap();

    let mut engine = engine();
    let sample = engine.load_sample(&path).unwrap();
    let play = |pairs: &[(&str, f64)]| Event::Sample {
        id: None,
        sample,
        options: options(pairs),
        chain: None,
    };
    engine
        .play(&[
            (0.0, play(&[("rate", 2.0), ("pan", -1.0)])),
            (0.0, play(&[("start", 0.5), ("pan", 1.0)])),
        ])
        .unwrap();
    let (left, right) = engine.render(1.0).unwrap();
    assert!((left[frame(0.25)] - 0.5).abs() < 1e-3);
    assert_eq!(left[frame(0.6)], 0.0);
    assert!((right[frame(0.25)] - 0.75).abs() < 1e-3);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_synth_that_panics_while_filling_audio_fails_the_engine_until_it_is_started_again() {
    let mut engine = engine();
    engine.register(Synth::new("broken").build(|_, _| {
        let breaks = map(|_: &Frame<f32, U0>| -> f32 { panic!("on purpose while filling audio") });
        Note::rings(breaks, 1.0)
    }));
    engine
        .play(&[(0.0, Event::note(None, "broken", &[]))])
        .unwrap();
    assert_eq!(engine.render(0.1).unwrap_err(), Error::Failed);
    assert_eq!(
        engine
            .play(&[(0.0, Event::note(None, "piano", &[]))])
            .unwrap_err(),
        Error::Failed
    );
}

#[test]
fn every_synth_declares_what_its_build_reads() {
    let mut engine = engine();
    let names: Vec<String> = engine.synths().map(|s| s.name().to_string()).collect();
    for name in names {
        engine
            .play(&[(0.0, Event::note(None, &name, &[]))])
            .unwrap();
    }
    engine.render(0.1).unwrap();
}

#[test]
fn a_stopped_sound_fades_out_over_10_ms_from_its_stop() {
    let mut engine = engine();
    engine.register(level());
    engine
        .play(&[
            (0.0, Event::note(Some(1), "level", &[])),
            (0.5, Event::Stop { ids: vec![1] }),
        ])
        .unwrap();
    let (left, _) = engine.render(1.0).unwrap();
    assert!(left[frame(0.49)] > 0.7);
    assert!(left[frame(0.505)] > 0.0 && left[frame(0.505)] < left[frame(0.49)]);
    assert_eq!(left[frame(0.511)], 0.0);
}

#[test]
fn an_engine_started_without_input_has_none_to_play() {
    let mut engine = engine();
    assert_eq!(engine.inputs(), 0);
    let input = Event::Input {
        id: None,
        input: 0,
        channels: 1,
        options: vec![],
        chain: None,
    };
    let error = engine.play(&[(0.0, input)]).unwrap_err();
    assert_eq!(error, Error::UnknownInput(0));
    assert_eq!(error.to_string(), "the sound card has no input 1");
}

#[test]
fn a_recording_writes_its_source_apart_from_the_main_output_until_it_stops() {
    let dir = std::env::temp_dir().join(format!("mxs-record-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ramp = dir.join("ramp.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(&ramp, spec).unwrap();
    for i in 0..48_000 {
        wav.write_sample(i as f32 / 48_000.0).unwrap();
    }
    wav.finalize().unwrap();

    let mut engine = engine();
    let sample = engine.load_sample(&ramp).unwrap();
    let recorded = dir.join("recorded.wav");
    engine
        .record(&recorded, Source::Sample(sample), 0.25)
        .unwrap();
    let (left, right) = engine.render(0.75).unwrap();
    assert!(left.iter().chain(&right).all(|&x| x == 0.0));
    engine.stop_recording().unwrap();

    let mut reader = hound::WavReader::open(&recorded).unwrap();
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().bits_per_sample, 24);
    assert_eq!(reader.spec().sample_rate, 48_000);
    let samples: Vec<f32> = reader
        .samples::<i32>()
        .map(|x| x.unwrap() as f32 / 8_388_607.0)
        .collect();
    assert_eq!(samples.len(), 2 * frame(0.5));
    let centre = std::f32::consts::FRAC_1_SQRT_2;
    let at = |seconds: f64| samples[2 * frame(seconds)];
    assert!((at(0.25) - 0.25 * centre).abs() < 1e-3);
    assert!((samples[2 * frame(0.25) + 1] - 0.25 * centre).abs() < 1e-3);

    let nowhere = dir.join("no-such-folder").join("take.wav");
    let error = engine
        .record(&nowhere, Source::Sample(sample), 0.0)
        .unwrap_err();
    assert!(matches!(error, Error::Record(_)));
    assert!(error.to_string().starts_with("cannot write "));
    assert_eq!(
        engine.record(&recorded, Source::Sample(99), 0.0),
        Err(Error::UnknownSample(99))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A synth of white noise, so a gap or a repeated block in a recording of it shows.
fn noise() -> Synth {
    Synth::new("noise").build(|_, ctx| Note::rings(white_noise(ctx.seed()) * 0.5, 3600.0))
}

#[test]
fn a_recording_of_the_output_matches_the_render_sample_for_sample_beside_a_recording_of_an_input() {
    let dir = std::env::temp_dir().join(format!("mxs-output-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ramp = dir.join("ramp.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(&ramp, spec).unwrap();
    (0..4800).for_each(|i| wav.write_sample(i as f32 / 4800.0).unwrap());
    wav.finalize().unwrap();

    let mut engine = engine();
    engine.register(noise());
    let sample = engine.load_sample(&ramp).unwrap();
    let (output, input) = (dir.join("output.wav"), dir.join("input.wav"));
    engine
        .play(&[(0.0, Event::note(None, "noise", &[("pan", -0.5)]))])
        .unwrap();
    engine.record_output(&output, 0.5).unwrap();
    engine.record(&input, Source::Sample(sample), 0.0).unwrap();
    let mut rendered = (Vec::new(), Vec::new());
    for _ in 0..3 {
        let (left, right) = engine.render(10.0).unwrap();
        rendered.0.extend(left);
        rendered.1.extend(right);
    }
    engine.stop_recording_output().unwrap();
    engine.stop_recording().unwrap();

    let mut reader = hound::WavReader::open(&output).unwrap();
    assert_eq!(reader.spec().sample_format, hound::SampleFormat::Float);
    assert_eq!(
        (reader.spec().channels, reader.spec().bits_per_sample),
        (2, 32)
    );
    let samples: Vec<f32> = reader.samples::<f32>().map(Result::unwrap).collect();
    let expected: Vec<f32> = rendered.0[frame(0.5)..]
        .iter()
        .zip(&rendered.1[frame(0.5)..])
        .flat_map(|(l, r)| [*l, *r])
        .collect();
    assert_eq!(samples.len(), expected.len());
    assert!(samples == expected);

    let input = hound::WavReader::open(&input).unwrap();
    assert_eq!(input.len() as usize, 2 * frame(30.0));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_recording_left_open_plays_up_to_its_last_flush() {
    let dir = std::env::temp_dir().join(format!("mxs-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let output = dir.join("open.wav");
    let mut engine = engine();
    engine.register(noise());
    engine
        .play(&[(0.0, Event::note(None, "noise", &[]))])
        .unwrap();
    engine.record_output(&output, 0.0).unwrap();
    engine.render(3.0).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    let reader = hound::WavReader::open(&output).unwrap();
    let seconds = reader.duration() as f64 / RATE;
    assert!(seconds >= 2.0, "the open file plays {seconds} seconds of 3");
    engine.stop_recording_output().unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sounding_counts_notes_until_they_end_and_not_live_input() {
    let mut engine = engine();
    engine.register(level());
    assert!(!engine.sounding());
    engine
        .play(&[(0.0, Event::note(None, "level", &[]))])
        .unwrap();
    engine.render(0.5).unwrap();
    assert!(engine.sounding());
    engine.render(0.6).unwrap();
    assert!(!engine.sounding());
}

/// A struck resonator that dies away over 5 seconds, so each note is quieter than the one after it.
fn ping() -> Synth {
    Synth::new("ping").build(|_, _| {
        Note::rings(
            Net::wrap(Box::new(impulse::<U1>())) >> mode(440.0, 0.0, 1.0, 5.0),
            5.0,
        )
    })
}

#[test]
fn past_the_voice_limit_the_quietest_notes_are_stolen_leaving_the_latest() {
    let render = |from: usize, limit: usize| {
        let mut engine = engine();
        engine.register(ping());
        engine.set_voice_limit(limit);
        let notes: Vec<_> = (from..20)
            .map(|i| (i as f64 * 0.05, Event::note(None, "ping", &[])))
            .collect();
        engine.play(&notes).unwrap();
        let (left, _) = engine.render(1.5).unwrap();
        (left, engine.warnings())
    };
    let (limited, warnings) = render(0, 8);
    let (latest, _) = render(12, 100);
    for k in frame(0.97)..frame(1.5) {
        assert!(
            (limited[k] - latest[k]).abs() < 1e-5,
            "frame {k}: {} against {}",
            limited[k],
            latest[k]
        );
    }
    assert_eq!(warnings, ["12 notes were stolen to keep to 8 at once"]);
}

#[test]
fn a_stolen_note_fades_out_over_10_ms() {
    let render = |notes: &[f64], limit: usize| {
        let mut engine = engine();
        engine.register(level());
        engine.set_voice_limit(limit);
        let notes: Vec<_> = notes
            .iter()
            .map(|&at| (at, Event::note(None, "level", &[])))
            .collect();
        engine.play(&notes).unwrap();
        engine.render(1.0).unwrap().0
    };
    let both = render(&[0.0, 0.5], 1);
    let second = render(&[0.5], 1);
    let first: Vec<f32> = both.iter().zip(&second).map(|(a, b)| a - b).collect();
    let centre = 0.5f32.sqrt();
    let from = (frame(0.5) / 64) * 64;
    let fade = frame(0.01);
    assert!((first[from - 1] - centre).abs() < 1e-6);
    for k in 0..fade {
        let expected = centre * (fade - k) as f32 / fade as f32;
        assert!(
            (first[from + k] - expected).abs() < 1e-4,
            "{k} into the fade: {} against {expected}",
            first[from + k]
        );
    }
    assert!(first[from + fade..].iter().all(|x| x.abs() < 1e-6));
}

#[test]
fn two_hundred_berimbau_strokes_at_once_are_cut_to_the_default_limit() {
    let mut engine = engine();
    let strokes: Vec<_> = (0..200)
        .map(|_| (0.0, Event::note(None, "berimbau", &[])))
        .collect();
    engine.play(&strokes).unwrap();
    engine.render(0.1).unwrap();
    let stolen = 200 - mxs::VOICES;
    assert_eq!(
        engine.warnings(),
        [format!(
            "{stolen} notes were stolen to keep to {} at once",
            mxs::VOICES
        )]
    );
}

#[test]
fn events_sent_5_ms_late_play_at_once_and_are_counted_late() {
    let mut engine = engine();
    engine.register(level());
    engine
        .play(&[(0.0, Event::note(Some(1), "level", &[("level", 0.5)]))])
        .unwrap();
    engine.render(0.1).unwrap();
    let late = engine.now() - 0.005;
    let change = Event::Set {
        target: Target::Sounds(vec![1]),
        options: vec![("level".into(), 0.0)],
    };
    engine
        .play(&[(late, Event::note(None, "level", &[])), (late, change)])
        .unwrap();
    let (left, _) = engine.render(0.1).unwrap();
    let centre = 0.5f32.sqrt();
    assert!((left[0] - centre).abs() < 1e-6, "{}", left[0]);
    assert_eq!(
        engine.warnings(),
        ["2 events arrived after their time, by up to 5.0 ms, and played at once"]
    );
    assert!(engine.warnings().is_empty());
}

#[test]
fn an_offline_engine_has_no_system_clock_to_play_at() {
    let mut engine = engine();
    assert_eq!(engine.clock().1, None);
    let at = std::time::SystemTime::now();
    assert_eq!(
        engine.play_at(&[(at, Event::note(None, "saw", &[]))]),
        Err(Error::Offline)
    );
}

#[test]
fn a_released_note_falls_silent_over_its_release_from_where_it_is() {
    let mut engine = engine();
    engine.register(
        Synth::new("held")
            .options(Spec::envelope(60.0, 0.1))
            .build(|o, _| Note::envelope(dc(1.0f32), o)),
    );
    let at = frame(0.20025);
    engine
        .play(&[
            (0.0, Event::note(Some(2), "held", &[])),
            (at as f64 / RATE, Event::Release { ids: vec![2] }),
        ])
        .unwrap();
    let (left, _) = engine.render(0.5).unwrap();
    let centre = 0.5f32.sqrt();
    assert!((left[at - 1] - centre).abs() < 1e-6, "{}", left[at - 1]);
    assert!(
        left[at + 1] < centre && left[at + frame(0.05)] > 0.0,
        "{} {}",
        left[at + 1],
        left[at + frame(0.05)]
    );
    assert!(
        left[at + frame(0.1) + 1..].iter().all(|&x| x.abs() < 1e-6),
        "{}",
        left[at + frame(0.1) + 1]
    );
    assert!(engine.warnings().is_empty());
}

#[test]
fn an_offline_engine_says_the_share_of_time_its_buffers_took_and_counts_none_late() {
    let mut engine = engine();
    assert_eq!(engine.load(), mxs::Load::default());
    assert_eq!(engine.behind(), None);
    let notes: Vec<(f64, Event)> = (0..8)
        .map(|i| {
            (
                i as f64 * 0.25,
                Event::note(None, "piano", &[("note", 48.0 + i as f64)]),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    engine.render(2.0).unwrap();
    let load = engine.load();
    assert_eq!(load.buffers, 8);
    assert!(load.mean > 0.0 && load.worst >= load.mean, "{load:?}");
    // A debug build fills slower than the sound lasts.
    if !cfg!(debug_assertions) {
        assert!(load.worst < 1.0, "{load:?}");
    }
    assert_eq!((load.late, load.most_over), (0, 0.0));
    assert_eq!(engine.load(), mxs::Load::default());
    assert!(engine.warnings().is_empty());
}

#[test]
fn an_offline_engine_refuses_a_batch_with_a_sound_of_three_channels_and_plays_none_of_it() {
    let mut engine = engine();
    engine.register(level());
    engine.register(Synth::new("three").build(|_, _| Note::rings(dc((0.0, 0.0, 0.0)), 1.0)));
    let batch = [
        (0.0, Event::note(None, "level", &[])),
        (0.0, Event::note(None, "three", &[])),
    ];
    assert_eq!(
        engine.play(&batch),
        Err(Error::BadSound {
            owner: "three".to_string(),
            outputs: 3
        })
    );
    let (left, _) = engine.render(0.5).unwrap();
    assert!(left.iter().all(|&x| x == 0.0));
}
