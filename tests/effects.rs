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
        Synth::new("steady").build(|_, _| Note::rings(Net::wrap(Box::new(dc(0.5))), 3.0)),
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

/// Both sides of `synth` through one effect for 1.5 seconds, with a change to its options half a second in, when given.
fn changed(
    effect: &str,
    given: &[(&str, f64)],
    change: &[(&str, f64)],
    synth: &str,
) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[(effect.to_string(), options(given))])
        .unwrap();
    let note = |synth: &str| Event::Note {
        id: None,
        synth: synth.to_string(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    let mut events = vec![(0.0, note(synth))];
    if synth == "click" {
        events.push((0.6, note(synth)));
    }
    if !change.is_empty() {
        let set = Event::Set {
            target: Target::Effect(chain, 0),
            options: change.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        };
        events.push((0.5, set));
    }
    engine.play(&events).unwrap();
    engine.render(1.5).unwrap()
}

fn loudest(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |a, x| a.max(x.abs()))
}

/// The largest difference between two renders over a stretch of seconds.
fn apart(a: &[f32], b: &[f32], from: f64, to: f64) -> f32 {
    let (from, to) = (frame(from), frame(to));
    a[from..to]
        .iter()
        .zip(&b[from..to])
        .fold(0.0f32, |m, (x, y)| m.max((x - y).abs()))
}

#[test]
fn a_change_to_the_slicers_smooth_ms_and_phase_offset_reaches_it_while_it_plays() {
    let step = |samples: &[f32], from: f64, to: f64| {
        samples[frame(from)..frame(to)]
            .windows(2)
            .fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()))
    };
    let (smoothed, _) = changed("slicer", &[], &[("smooth_ms", 100.0)], "steady");
    assert!(step(&smoothed, 0.05, 0.5) > 0.2);
    assert!(step(&smoothed, 0.6, 1.4) < 0.01);
    assert!(loudest(&smoothed[frame(0.6)..]) > 0.2);

    let (plain, _) = changed("slicer", &[], &[], "steady");
    let (moved, _) = changed("slicer", &[], &[("phase_offset", 0.5)], "steady");
    assert_eq!(plain[..frame(0.5)], moved[..frame(0.5)]);
    assert_eq!(plain[frame(0.7)], 0.0);
    assert!((moved[frame(0.7)] - loudest(&plain)).abs() < 1e-6);
}

#[test]
fn a_change_to_the_flangers_switches_and_the_wobbles_filter_reaches_them_without_a_click() {
    let cases: [(&str, &[(&str, f64)], (&str, f64)); 3] = [
        (
            "flanger",
            &[("feedback", 0.7), ("phase", 0.5)],
            ("stereo_invert_wave", 1.0),
        ),
        (
            "flanger",
            &[("feedback", 0.7), ("phase", 0.5)],
            ("invert_flange", 1.0),
        ),
        ("wobble", &[], ("filter", 1.0)),
    ];
    for (effect, given, change) in cases {
        let (plain, plain_right) = changed(effect, given, &[], "tone");
        let (left, right) = changed(effect, given, &[change], "tone");
        assert_eq!(
            plain[..frame(0.5)],
            left[..frame(0.5)],
            "{effect} {change:?}"
        );
        let peak = loudest(&plain);
        assert!(
            apart(&plain, &left, 0.6, 1.4) > peak / 10.0,
            "{effect} {change:?}"
        );
        let before = loudest(&left[frame(0.0)..frame(0.5)]);
        assert!(
            loudest(&left[frame(0.5)..frame(0.52)]) <= before * 1.05,
            "{effect} {change:?}"
        );
        if change.0 == "stereo_invert_wave" {
            assert_eq!(plain, plain_right);
            assert!(apart(&left, &right, 0.6, 1.4) > peak / 10.0);
        }
        let mut whole = given.to_vec();
        whole.push(change);
        let (from_start, _) = changed(effect, &whole, &[], "tone");
        assert!(
            apart(&from_start, &left, 1.3, 1.5) < peak,
            "{effect} {change:?}"
        );
    }
}

#[test]
fn a_change_to_the_ping_pongs_pan_start_moves_its_first_echo_to_the_other_side() {
    let first_echo = |change: &[(&str, f64)]| {
        let (left, right) = changed("ping_pong", &[], change, "click");
        let at = frame(0.6 + 0.25);
        (
            loudest(&left[at - 48..at + 48]),
            loudest(&right[at - 48..at + 48]),
        )
    };
    let (left, right) = first_echo(&[]);
    assert!(right > 0.01 && left < right / 100.0, "{left} {right}");
    let (left, right) = first_echo(&[("pan_start", -1.0)]);
    assert!(left > 0.01 && right < left / 100.0, "{left} {right}");
}

/// Both sides of notes through a limiter: each note is a synth, when it starts and the side it is panned to.
fn limited(
    given: &[(&str, f64)],
    notes: &[(&str, f64, f64, f64)],
    seconds: f64,
) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    engine.register(
        Synth::new("burst")
            .build(|_, _| Note::rings(Net::wrap(Box::new(sine_hz::<f32>(1000.0))) * 4.0, 0.05)),
    );
    let chain = engine
        .start_effects(&[("limiter".to_string(), options(given))])
        .unwrap();
    let events: Vec<_> = notes
        .iter()
        .map(|(synth, at, pan, amp)| {
            let note = Event::Note {
                id: None,
                synth: synth.to_string(),
                options: options(&[("pan", *pan), ("amp", *amp)]),
                chain: Some(chain),
                held: false,
            };
            (*at, note)
        })
        .collect();
    engine.play(&events).unwrap();
    engine.render(seconds).unwrap()
}

const LOOK_AHEAD: f64 = 0.005;

#[test]
fn the_limiter_keeps_every_peak_under_its_ceiling_and_leaves_a_quiet_sound_as_it_was() {
    let ceiling = 10f32.powf(-1.0 / 20.0);
    let (left, right) = limited(&[], &[("tone", 0.0, 0.0, 8.0)], 1.0);
    assert!(loudest(&left) <= ceiling + 1e-6 && loudest(&right) <= ceiling + 1e-6);
    assert!(loudest(&left[frame(0.5)..]) > ceiling * 0.95);

    let (left, _) = limited(&[("ceiling", -6.0)], &[("tone", 0.0, 0.0, 8.0)], 1.0);
    assert!(loudest(&left) <= 10f32.powf(-6.0 / 20.0) + 1e-6);

    let (dry, _) = through_both("level", &[("tone", 0.0, 0.0, 0.2)], 1.0);
    let (quiet, _) = limited(&[], &[("tone", 0.0, 0.0, 0.2)], 1.0);
    let late = frame(LOOK_AHEAD);
    assert_eq!(dry[..frame(0.9)], quiet[late..late + frame(0.9)]);
}

/// Both sides of notes through one effect as it comes.
fn through_both(
    effect: &str,
    notes: &[(&str, f64, f64, f64)],
    seconds: f64,
) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[(effect.to_string(), vec![])])
        .unwrap();
    let events: Vec<_> = notes
        .iter()
        .map(|(synth, at, pan, amp)| {
            let note = Event::Note {
                id: None,
                synth: synth.to_string(),
                options: options(&[("pan", *pan), ("amp", *amp)]),
                chain: Some(chain),
                held: false,
            };
            (*at, note)
        })
        .collect();
    engine.play(&events).unwrap();
    engine.render(seconds).unwrap()
}

#[test]
fn the_limiter_lowers_both_sides_alike_and_lets_go_over_its_relax_ms() {
    let notes = [("tone", 0.0, 1.0, 0.2), ("burst", 0.5, -1.0, 1.0)];
    let (_, right) = limited(&[], &notes, 1.2);
    let (left, _) = limited(&[], &notes, 1.2);
    let (_, dry_right) = through_both("level", &notes[..1], 1.2);
    let level = |samples: &[f32], at: f64| rms(&samples[frame(at)..frame(at + 0.02)]);
    let quiet = level(&dry_right, 0.3);
    assert!((level(&right, 0.3 + LOOK_AHEAD) - quiet).abs() < quiet * 0.01);

    // During the burst the left is held at the ceiling, about 12 dB down, and the right is down by the same.
    let ceiling = 10f32.powf(-1.0 / 20.0);
    assert!(loudest(&left[frame(0.5)..frame(0.56)]) <= ceiling + 1e-6);
    let down = level(&right, 0.52 + LOOK_AHEAD) / quiet;
    let held = ceiling / (4.0 * std::f32::consts::FRAC_1_SQRT_2 * 2.0f32.sqrt());
    assert!((20.0 * (down / held).log10()).abs() < 1.5, "{down} {held}");

    // Within 1 dB of dry 0.3 seconds after the burst ends.
    let after = level(&right, 0.55 + 0.3 + LOOK_AHEAD) / quiet;
    assert!(20.0 * after.log10() > -1.0, "{after}");

    let (_, slow) = limited(&[("relax_ms", 1000.0)], &notes, 1.2);
    assert!(20.0 * (level(&slow, 0.6 + LOOK_AHEAD) / quiet).log10() < -3.0);
    assert!(20.0 * (level(&right, 0.6 + LOOK_AHEAD) / quiet).log10() > -3.0);
}

/// A tone in a part of its own, another in a second part and a third in no part, for 1.5 seconds, with a setup run on the engine first.
fn three_tones(setup: impl Fn(&mut Engine) -> Vec<(f64, Event)>) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    let mut events = setup(&mut engine);
    let first = engine.start_part("first", &[]).unwrap();
    let second = engine
        .start_part("second", &[("level".to_string(), options(&[("amp", 0.5)]))])
        .unwrap();
    for (chain, hz) in [(Some(first), 220.0), (Some(second), 330.0), (None, 550.0)] {
        let note = Event::Note {
            id: None,
            synth: "tone".to_string(),
            options: options(&[("hz", hz)]),
            chain,
            held: false,
        };
        events.push((0.0, note));
    }
    engine.play(&events).unwrap();
    engine.render(1.5).unwrap()
}

#[test]
fn the_main_effects_take_everything_the_engine_plays_after_each_parts_own() {
    let (dry, dry_right) = three_tones(|_| vec![]);
    let (halved, halved_right) = three_tones(|engine| {
        let main = [("level".to_string(), options(&[("amp", 0.5)]))];
        engine.start_main_effects(&main, false).unwrap();
        vec![]
    });
    for (dry, halved) in [(&dry, &halved), (&dry_right, &halved_right)] {
        for (x, y) in dry.iter().zip(halved.iter()) {
            assert!((x * 0.5 - y).abs() < 1e-6);
        }
    }

    let (limited, _) = three_tones(|engine| {
        let main = [("limiter".to_string(), options(&[("ceiling", -12.0)]))];
        engine.start_main_effects(&main, false).unwrap();
        vec![]
    });
    assert!(loudest(&dry) > 0.5);
    assert!(loudest(&limited) <= 10f32.powf(-12.0 / 20.0) + 1e-6);

    let (changed, _) = three_tones(|engine| {
        let main = engine
            .start_main_effects(&[("level".to_string(), vec![])], false)
            .unwrap();
        let set = Event::Set {
            target: Target::Effect(main, 0),
            options: vec![("amp".to_string(), 0.0)],
        };
        vec![(0.5, set)]
    });
    assert_eq!(changed[..frame(0.5)], dry[..frame(0.5)]);
    assert_eq!(loudest(&changed[frame(0.6)..]), 0.0);

    let (replaced, _) = three_tones(|engine| {
        engine
            .start_main_effects(&[("level".to_string(), options(&[("amp", 0.0)]))], false)
            .unwrap();
        engine.start_main_effects(&[], false).unwrap();
        vec![]
    });
    assert_eq!(replaced, dry);
}

fn peak_db(samples: &[f32]) -> f32 {
    20.0 * loudest(samples).log10()
}

/// The level in dB of the partial at `hz` over a whole number of its cycles from `from` seconds.
fn partial_db(samples: &[f32], hz: f64, from: f64) -> f64 {
    let cycles = (0.5 * hz).floor();
    let window = &samples[frame(from)..frame(from) + (cycles / hz * RATE).round() as usize];
    let (mut sin, mut cos) = (0.0, 0.0);
    for (i, x) in window.iter().enumerate() {
        let angle = std::f64::consts::TAU * hz * i as f64 / RATE;
        sin += f64::from(*x) * angle.sin();
        cos += f64::from(*x) * angle.cos();
    }
    20.0 * (2.0 * (sin * sin + cos * cos).sqrt() / window.len() as f64).log10()
}

#[test]
fn the_compressor_lowers_what_is_over_its_threshold_by_its_ratio_and_raises_it_by_its_makeup() {
    let dry = through("level", &[], "tone", 1.0);
    let level = f64::from(peak_db(&dry[frame(0.5)..]));

    // 12 dB over the threshold at a ratio of 4 leaves 3 dB over: 9 dB down.
    let over = [("threshold", level - 12.0), ("ratio", 4.0)];
    let squeezed = through("compressor", &over, "tone", 1.0);
    let down = f64::from(peak_db(&squeezed[frame(0.5)..])) - level;
    assert!((down + 9.0).abs() < 0.5, "{down}");

    let made_up = [("threshold", level - 12.0), ("ratio", 4.0), ("makeup", 6.0)];
    let raised = through("compressor", &made_up, "tone", 1.0);
    let down = f64::from(peak_db(&raised[frame(0.5)..])) - level;
    assert!((down + 3.0).abs() < 0.5, "{down}");

    // 12 dB under the threshold it is left as it was.
    let under = [("threshold", level + 12.0), ("ratio", 4.0)];
    let left = through("compressor", &under, "tone", 1.0);
    let apart = f64::from(peak_db(&left[frame(0.5)..])) - level;
    assert!(apart.abs() < 0.2, "{apart}");

    let mut engine = engine();
    let effect = [("compressor".to_string(), options(&[("slope_above", 0.5)]))];
    assert!(engine.start_effects(&effect).is_err());
}

#[test]
fn the_compressor_as_it_comes_keeps_the_shape_of_a_low_notes_cycle() {
    let mut engine = engine();
    engine.register(Synth::new("low").build(|_, _| {
        let first = Net::wrap(Box::new(sine_hz::<f32>(41.0))) * 0.8;
        let third = Net::wrap(Box::new(sine_hz::<f32>(123.0))) * 0.2;
        Note::rings(first + third, 3.0)
    }));
    let mut played = |effect: &str| {
        let chain = engine
            .start_effects(&[(effect.to_string(), vec![])])
            .unwrap();
        let note = Event::Note {
            id: None,
            synth: "low".to_string(),
            options: vec![],
            chain: Some(chain),
            held: false,
        };
        let at = engine.now();
        engine.play(&[(at, note)]).unwrap();
        engine.render(3.5).unwrap().0
    };
    let (dry, squeezed) = (played("level"), played("compressor"));
    // The note is over the threshold, so the compressor does turn it down.
    assert!(
        peak_db(&squeezed[frame(1.0)..frame(2.0)]) < peak_db(&dry[frame(1.0)..frame(2.0)]) - 3.0
    );
    let shape = |samples: &[f32]| partial_db(samples, 123.0, 1.0) - partial_db(samples, 41.0, 1.0);
    let apart = shape(&squeezed) - shape(&dry);
    assert!(apart.abs() < 0.5, "{apart}");
}

#[test]
fn the_compressor_turns_both_sides_down_by_what_the_louder_side_asks() {
    let notes = [("tone", 0.0, 1.0, 0.1), ("tone", 0.5, -1.0, 2.0)];
    let mut engine = engine();
    let chain = engine
        .start_effects(&[("compressor".to_string(), options(&[("relax_ms", 20.0)]))])
        .unwrap();
    let events: Vec<_> = notes
        .iter()
        .map(|(synth, at, pan, amp)| {
            let note = Event::Note {
                id: None,
                synth: synth.to_string(),
                options: options(&[("pan", *pan), ("amp", *amp), ("hz", 440.0 + 220.0 * pan)]),
                chain: Some(chain),
                held: false,
            };
            (*at, note)
        })
        .collect();
    engine.play(&events).unwrap();
    let (left, right) = engine.render(1.5).unwrap();
    let (_, dry_right) = through_both("level", &[("tone", 0.0, 1.0, 0.1)], 1.5);
    let (dry_left, _) = through_both("level", &[("tone", 0.5, -1.0, 2.0)], 1.5);
    let level = |samples: &[f32], at: f64| rms(&samples[frame(at)..frame(at + 0.1)]);
    // Alone the quiet right is under the threshold and left as it was.
    assert!((20.0 * (level(&right, 0.3) / level(&dry_right, 0.3)).log10()).abs() < 0.2);
    // Once the loud left plays, both come down by the same amount.
    let left_down = 20.0 * (level(&left, 1.0) / level(&dry_left, 1.0)).log10();
    let right_down = 20.0 * (level(&right, 1.0) / level(&dry_right, 1.0)).log10();
    assert!(left_down < -6.0, "{left_down}");
    assert!(
        (left_down - right_down).abs() < 0.2,
        "{left_down} {right_down}"
    );
}
