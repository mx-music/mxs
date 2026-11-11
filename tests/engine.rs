use fundsp::prelude::*;
use mxs::nodes::{hz, mode, white_noise};
use mxs::{Effect, Engine, Error, Event, Note, RATE, Spec, Synth, Target, Value, synths};

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
    pairs.iter().map(|(k, v)| (k.to_string(), Value::Number(*v))).collect()
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
                    let struck = Net::wrap(Box::new(impulse::<U1>())) >> (mode(freq, 0.0, 1.0, 3.0) & mode(freq * 2.76, 0.0, 0.5, 1.2));
                    let tap = (white_noise(ctx.seed()) >> highpass_hz(4000.0, 0.7)) * 0.01;
                    Note::rings((struck + tap) * options.number("vel") as f32, 4.0)
                }),
        );
        let phrase: Vec<_> = [60.0, 64.0, 67.0, 72.0]
            .iter()
            .enumerate()
            .map(|(i, note)| (i as f64 * 0.25, Event::note(None, "plate", &[("note", *note), ("vel", 0.8)])))
            .chain([(0.5, Event::note(None, "piano", &[("note", 48.0)])), (0.7, Event::note(None, "berimbau", &[]))])
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
    let error = engine.play(&[(0.0, Event::note(None, "berimbau", &[("sustain", 1.0)]))]).unwrap_err();
    assert_eq!(error, Error::UnknownOption { owner: "berimbau".into(), option: "sustain".into() });
    assert_eq!(error.to_string(), "berimbau has no option sustain");
    let error = engine.play(&[(0.0, Event::note(None, "harpsichord", &[]))]).unwrap_err();
    assert_eq!(error.to_string(), "no synth named harpsichord");
}

#[test]
fn a_batch_with_a_mistake_plays_nothing() {
    let mut engine = engine();
    engine.register(level());
    let batch = [(0.0, Event::note(None, "level", &[])), (0.0, Event::note(None, "level", &[("loudness", 1.0)]))];
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
    assert_eq!(engine.warnings(), vec!["level's option level was held to its range, 0 to 2".to_string()]);
    let (left, _) = engine.render(0.5).unwrap();
    // Two notes at level 2, panned to the centre at equal power.
    assert!((left[frame(0.1)] - 4.0 * 0.5f32.sqrt()).abs() < 1e-4);
    engine.play(&[(1.0, Event::note(None, "level", &[("level", 5.0)]))]).unwrap();
    assert!(engine.warnings().is_empty());
}

#[test]
fn a_named_option_takes_only_its_names() {
    let mut engine = engine();
    engine.register(
        Synth::new("bol")
            .option(Spec::name("bol", &["na", "tun"]).default("na"))
            .build(|options, _| Note::rings(dc(if options.name("bol") == "na" { 1.0 } else { 0.5 }), 1.0)),
    );
    let tun = Event::Note { id: None, synth: "bol".into(), options: vec![("bol".into(), Value::from("tun"))], chain: None };
    engine.play(&[(0.0, tun)]).unwrap();
    let dha = Event::Note { id: None, synth: "bol".into(), options: vec![("bol".into(), Value::from("dha"))], chain: None };
    assert_eq!(engine.play(&[(0.0, dha)]).unwrap_err().to_string(), "bol's option bol takes one of na, tun");
}

#[test]
fn every_note_of_a_batch_of_3000_sounds() {
    let mut engine = engine();
    engine.register(Synth::new("click").build(|_, _| Note::rings(dc(1.0), 0.001)));
    let clicks: Vec<_> = (0..3000).map(|i| (i as f64 * 0.002, Event::note(None, "click", &[]))).collect();
    engine.play(&clicks).unwrap();
    let (left, _) = engine.render(6.5).unwrap();
    let starts = left.windows(2).filter(|w| w[0] == 0.0 && w[1] > 0.0).count() + usize::from(left[0] > 0.0);
    assert_eq!(starts, 3000);
}

#[test]
fn a_note_starts_at_its_sample_and_fades_out_over_its_last_10_ms() {
    let mut engine = engine();
    engine.register(level());
    engine.play(&[(0.5, Event::note(None, "level", &[]))]).unwrap();
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
    engine.play(&[(0.0, Event::note(Some(7), "level", &[("level", 1.0)]))]).unwrap();
    let change = Event::Set { target: Target::Sounds(vec![7]), options: vec![("level".into(), 0.0), ("level_slide".into(), 0.4)] };
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
    let change = Event::Set { target: Target::Sounds(vec![3]), options: vec![("amp".into(), 0.0)] };
    engine.play(&[(0.0, note), (0.2, change)]).unwrap();
    let (left, _) = engine.render(0.5).unwrap();
    assert!(left[frame(0.1)] > 0.5);
    assert_eq!(left[frame(0.3)], 0.0);
}

#[test]
fn amp_and_pan_place_every_note() {
    let mut engine = engine();
    engine.register(level());
    engine.play(&[(0.0, Event::note(None, "level", &[("amp", 0.5), ("pan", -1.0)]))]).unwrap();
    let (left, right) = engine.render(0.5).unwrap();
    assert!((left[frame(0.1)] - 0.5).abs() < 1e-4);
    assert!(right[frame(0.1)].abs() < 1e-4);
}

#[test]
fn notes_play_through_their_effect_chain_until_it_stops() {
    let mut engine = engine();
    engine.register(level());
    engine.register_effect(
        Effect::new("double").tail(0.5).build(|_, _| Net::wrap(Box::new(multipass::<U2>() * 2.0))),
    );
    let chain = engine.start_effects(&[("double".to_string(), options(&[("mix", 1.0)]))]).unwrap();
    let note = Event::Note { id: None, synth: "level".into(), options: vec![], chain: Some(chain) };
    engine.play(&[(0.0, note)]).unwrap();
    engine.stop_effects(chain, 0.2).unwrap();
    let (left, _) = engine.render(1.0).unwrap();
    assert!((left[frame(0.5)] - 2.0 * 0.5f32.sqrt()).abs() < 1e-4);
    let half = Event::Set { target: Target::Effect(chain, 0), options: vec![("mix".into(), 0.5)] };
    assert!(engine.play(&[(1.1, half)]).is_ok());
    // The chain is gone once its note and tail have ended, so a note sent into it plays dry.
    engine.render(1.0).unwrap();
    let late = Event::Note { id: None, synth: "level".into(), options: vec![], chain: Some(chain) };
    engine.play(&[(2.0, late)]).unwrap();
    let (left, _) = engine.render(0.5).unwrap();
    assert!((left[frame(0.1)] - 0.5f32.sqrt()).abs() < 1e-4);
}

#[test]
fn a_loaded_sample_plays_at_its_rate_within_its_span() {
    let dir = std::env::temp_dir().join(format!("mxs-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ramp.wav");
    let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 32, sample_format: hound::SampleFormat::Float };
    let mut wav = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..48_000 {
        wav.write_sample(i as f32 / 48_000.0).unwrap();
    }
    wav.finalize().unwrap();

    let mut engine = engine();
    let sample = engine.load_sample(&path).unwrap();
    let play = |pairs: &[(&str, f64)]| Event::Sample { id: None, sample, options: options(pairs), chain: None };
    engine.play(&[(0.0, play(&[("rate", 2.0), ("pan", -1.0)])), (0.0, play(&[("start", 0.5), ("pan", 1.0)]))]).unwrap();
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
    engine.play(&[(0.0, Event::note(None, "broken", &[]))]).unwrap();
    assert_eq!(engine.render(0.1).unwrap_err(), Error::Failed);
    assert_eq!(engine.play(&[(0.0, Event::note(None, "piano", &[]))]).unwrap_err(), Error::Failed);
}

#[test]
fn every_synth_declares_what_its_build_reads() {
    let mut engine = engine();
    let names: Vec<String> = engine.synths().map(|s| s.name().to_string()).collect();
    for name in names {
        engine.play(&[(0.0, Event::note(None, &name, &[]))]).unwrap();
    }
    engine.render(0.1).unwrap();
}
