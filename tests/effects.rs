use fundsp::prelude::*;
use mxs::nodes::white_noise;
use mxs::{Engine, Event, Note, RATE, Spec, Synth, Target, Value, effects};

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    engine.register(
        Synth::new("click").build(|_, _| Note::rings(Net::wrap(Box::new(impulse::<U1>())), 0.001)),
    );
    engine.register(
        Synth::new("hiss")
            .option(Spec::seconds("length").default(1.0))
            .build(|o, ctx| Note::rings(white_noise(ctx.seed()) * 0.25, o.number("length"))),
    );
    engine.register(
        Synth::new("tone")
            .option(Spec::hz("hz").default(440.0))
            .build(|o, _| {
                Note::rings(
                    Net::wrap(Box::new(sine_hz::<f32>(o.number("hz") as f32))) * 0.5,
                    2.0,
                )
            }),
    );
    engine
}

fn options(pairs: &[(&str, f64)]) -> Vec<(String, Value)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), Value::Number(*v)))
        .collect()
}

/// Renders `synth` through one effect for `seconds`, as the left channel.
fn through(effect: &str, given: &[(&str, f64)], synth: &str, seconds: f64) -> Vec<f32> {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[(effect.to_string(), options(given))])
        .unwrap();
    let note = Event::Note {
        id: None,
        synth: synth.to_string(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(0.0, note)]).unwrap();
    engine.render(seconds).unwrap().0
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

fn frame(seconds: f64) -> usize {
    (seconds * RATE) as usize
}

#[test]
fn every_effect_starts_and_keeps_its_output_finite() {
    for effect in effects::all() {
        let left = through(effect.name(), &[], "hiss", 1.5);
        assert!(
            left.iter().all(|x| x.is_finite()),
            "{} gave a sample that is not finite",
            effect.name()
        );
        assert!(
            rms(&left[..frame(1.0)]) > 0.001,
            "{} is silent",
            effect.name()
        );
    }
}

#[test]
fn level_passes_the_sound_unchanged() {
    let dry = through("level", &[], "hiss", 0.5);
    let mut engine = engine();
    engine
        .play(&[(0.0, Event::note(None, "hiss", &[]))])
        .unwrap();
    let alone = engine.render(0.5).unwrap().0;
    assert_eq!(dry, alone);
}

#[test]
fn lpf_cuts_above_its_cutoff_and_hpf_below() {
    let low = through("lpf", &[("cutoff", 40.0)], "tone", 1.0);
    let high = through("hpf", &[("cutoff", 40.0)], "tone", 1.0);
    let tone = rms(&through("level", &[], "tone", 1.0)[frame(0.5)..]);
    assert!(rms(&low[frame(0.5)..]) < tone * 0.2);
    assert!((rms(&high[frame(0.5)..]) - tone).abs() < tone * 0.05);
}

#[test]
fn echo_repeats_a_click_a_phase_apart_each_quieter() {
    let left = through("echo", &[("phase", 0.1), ("decay", 1.0)], "click", 0.5);
    let (dry, first, second) = (left[0], left[frame(0.1)], left[frame(0.2)]);
    assert_eq!(first, dry);
    let feedback = 0.001f32.powf(0.1 / 1.0);
    assert!(
        (second - first * feedback).abs() < 1e-4,
        "{first} then {second}"
    );
    assert_eq!(left[frame(0.15)], 0.0);
}

#[test]
fn reverb_rings_on_after_the_sound_stops() {
    let left = through("reverb", &[("room", 0.8)], "hiss", 3.0);
    assert!(rms(&left[frame(1.2)..frame(1.7)]) > 0.001);
    let small = through("reverb", &[("room", 0.1)], "hiss", 3.0);
    assert!(rms(&small[frame(1.2)..frame(1.7)]) < rms(&left[frame(1.2)..frame(1.7)]) / 4.0);
}

#[test]
fn tremolo_turns_the_sound_down_by_its_depth_once_a_phase() {
    let left = through(
        "tremolo",
        &[("phase", 0.5), ("depth", 1.0), ("wave", 3.0)],
        "tone",
        1.0,
    );
    let quietest = left[frame(0.2)..frame(0.8)]
        .chunks(480)
        .map(rms)
        .fold(f32::MAX, f32::min);
    let loudest = left[frame(0.2)..frame(0.8)]
        .chunks(480)
        .map(rms)
        .fold(0.0, f32::max);
    assert!(quietest < loudest * 0.1, "{quietest} against {loudest}");
}

#[test]
fn an_effects_tail_comes_from_its_options() {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[(
            "echo".to_string(),
            options(&[("phase", 0.05), ("decay", 0.3)]),
        )])
        .unwrap();
    let note = Event::Note {
        id: None,
        synth: "click".to_string(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(0.0, note)]).unwrap();
    engine.stop_effects(chain, 0.0).unwrap();
    let (left, _) = engine.render(1.0).unwrap();
    assert!(left[frame(0.1)].abs() > 0.0);
    assert!(left[frame(0.5)..].iter().all(|&x| x == 0.0));
}

#[test]
fn each_effect_says_how_long_it_rings_after_its_input_stops_from_its_options_and_the_tempo() {
    let mut engine = engine();
    let room = |room: f64| {
        (
            "reverb".to_string(),
            vec![("room".to_string(), Value::Number(room))],
        )
    };
    let echo = (
        "echo".to_string(),
        vec![("decay".to_string(), Value::Number(2.0))],
    );
    let effects = [room(0.45), echo.clone(), ("lpf".to_string(), vec![])];
    assert_eq!(
        engine.effect_tails(&effects, None).unwrap(),
        vec![5.5, 2.0, 0.1]
    );
    assert_eq!(
        engine.effect_tails(&effects, Some(120.0)).unwrap(),
        vec![5.5, 1.0, 0.1]
    );
    assert_eq!(engine.effect_tails(&[room(1.0)], None).unwrap(), vec![11.0]);
    let unknown = engine
        .effect_tails(&[("hall".to_string(), vec![])], None)
        .unwrap_err();
    assert_eq!(unknown.to_string(), "no effect named hall");
}

/// How far, in dB, a tone at `hz` comes out of eq above the same tone dry.
fn eq_gain(given: &[(&str, f64)], hz: f64) -> f32 {
    let level = |effect: &str, given: &[(&str, f64)]| {
        let mut engine = engine();
        let chain = engine
            .start_effects(&[(effect.to_string(), options(given))])
            .unwrap();
        let note = Event::Note {
            id: None,
            synth: "tone".to_string(),
            options: options(&[("hz", hz)]),
            chain: Some(chain),
            held: false,
        };
        engine.play(&[(0.0, note)]).unwrap();
        rms(&engine.render(1.0).unwrap().0[frame(0.5)..])
    };
    20.0 * (level("eq", given) / level("level", &[])).log10()
}

#[test]
fn eq_raises_or_lowers_each_of_its_three_bands_and_leaves_the_rest() {
    let mid = [("mid_at", 69.0), ("mid_gain", 6.0)];
    assert!((eq_gain(&mid, 440.0) - 6.0).abs() < 0.1);
    assert!(eq_gain(&mid, 55.0).abs() < 0.5);
    assert!(eq_gain(&mid, 7040.0).abs() < 0.5);
    let narrow = [("mid_at", 69.0), ("mid_gain", 6.0), ("mid_q", 4.0)];
    assert!(eq_gain(&narrow, 660.0) < eq_gain(&mid, 660.0) - 1.0);

    let low = [("low_at", 48.0), ("low_gain", -12.0)];
    assert!((eq_gain(&low, 40.0) + 12.0).abs() < 1.0);
    assert!(eq_gain(&low, 2000.0).abs() < 0.5);

    let high = [("high_at", 96.0), ("high_gain", 6.0)];
    assert!((eq_gain(&high, 8000.0) - 6.0).abs() < 1.0);
    assert!(eq_gain(&high, 200.0).abs() < 0.5);
}

#[test]
fn eq_with_no_band_given_is_the_dry_sound() {
    let dry = through("level", &[], "hiss", 0.5);
    let flat = through("eq", &[], "hiss", 0.5);
    let peak = dry.iter().fold(0.0f32, |a, x| a.max(x.abs()));
    let apart = dry
        .iter()
        .zip(&flat)
        .fold(0.0f32, |a, (x, y)| a.max((x - y).abs()));
    assert!(apart <= peak * 1.0e-5, "{apart} against {peak}");
}

#[test]
fn eq_glides_a_band_to_a_new_gain_over_its_slide() {
    let mut engine = engine();
    let chain = engine.start_effects(&[("eq".to_string(), vec![])]).unwrap();
    let note = Event::Note {
        id: None,
        synth: "tone".to_string(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    let change = Event::Set {
        target: Target::Effect(chain, 0),
        options: vec![
            ("mid_at".to_string(), 69.0),
            ("mid_gain".to_string(), -6.0),
            ("mid_gain_slide".to_string(), 0.5),
        ],
    };
    engine.play(&[(0.0, note), (0.5, change)]).unwrap();
    let left = engine.render(1.5).unwrap().0;
    let at = |seconds: f64| rms(&left[frame(seconds)..frame(seconds + 0.05)]);
    let db = |seconds: f64| 20.0 * (at(seconds) / at(0.4)).log10();
    assert!(db(0.75) < -1.5 && db(0.75) > -4.5, "{}", db(0.75));
    assert!((db(1.2) + 6.0).abs() < 0.2, "{}", db(1.2));
}
