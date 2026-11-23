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
        Synth::new("saw")
            .option(Spec::hz("hz").default(440.0))
            .build(|o, _| {
                Note::rings(
                    Net::wrap(Box::new(saw_hz(o.number("hz") as f32))) * 0.5,
                    2.0,
                )
            }),
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
    let left = through("reverb", &[("decay_seconds", 3.0)], "hiss", 3.0);
    assert!(rms(&left[frame(1.2)..frame(1.7)]) > 0.001);
    let small = through("reverb", &[("decay_seconds", 0.3)], "hiss", 3.0);
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
    let room = |seconds: f64| {
        (
            "reverb".to_string(),
            vec![("decay_seconds".to_string(), Value::Number(seconds))],
        )
    };
    let echo = (
        "echo".to_string(),
        vec![("decay".to_string(), Value::Number(2.0))],
    );
    let effects = [room(5.5), echo.clone(), ("lpf".to_string(), vec![])];
    assert_eq!(
        engine.effect_tails(&effects, None).unwrap(),
        vec![5.5, 2.0, 0.1]
    );
    assert_eq!(
        engine.effect_tails(&effects, Some(120.0)).unwrap(),
        vec![5.5, 1.0, 0.1]
    );
    assert_eq!(
        engine.effect_tails(&[room(11.0)], None).unwrap(),
        vec![11.0]
    );
    let unknown = engine
        .effect_tails(&[("no_such".to_string(), vec![])], None)
        .unwrap_err();
    assert_eq!(unknown.to_string(), "no effect named no_such");
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

/// Both sides of a click at 0.1 seconds through the reverb with the options given, wet only.
fn reverb_click(given: &[(&str, f64)], seconds: f64) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    let mut given = given.to_vec();
    given.push(("mix", 1.0));
    let chain = engine
        .start_effects(&[("reverb".to_string(), options(&given))])
        .unwrap();
    let click = Event::Note {
        id: None,
        synth: "click".to_string(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(0.1, click)]).unwrap();
    engine.render(seconds).unwrap()
}

/// The first frame louder than a thousandth of the loudest.
fn first_sound(samples: &[f32]) -> usize {
    let floor = loudest(samples) / 1000.0;
    samples.iter().position(|x| x.abs() > floor).unwrap()
}

#[test]
fn the_reverb_keeps_a_sound_louder_on_its_own_side() {
    let (left, right) = through_both("reverb", &[("tone", 0.0, -1.0, 1.0)], 2.0);
    let (wet_left, wet_right) = {
        let mut engine = engine();
        let chain = engine
            .start_effects(&[("reverb".to_string(), options(&[("mix", 1.0)]))])
            .unwrap();
        let note = Event::Note {
            id: None,
            synth: "hiss".to_string(),
            options: options(&[("pan", -1.0)]),
            chain: Some(chain),
            held: false,
        };
        engine.play(&[(0.0, note)]).unwrap();
        engine.render(3.0).unwrap()
    };
    assert!(rms(&left) > 0.0 && rms(&right) > 0.0);
    let apart = 20.0 * (rms(&wet_left) / rms(&wet_right)).log10();
    assert!(apart >= 3.0, "{apart}");
    assert!(rms(&wet_right) > rms(&wet_left) / 10.0);
}

#[test]
fn the_reverb_fades_60_db_in_its_decay_seconds_after_its_predelay_and_starts_later_in_a_larger_room()
 {
    let (left, _) = reverb_click(&[("decay_seconds", 2.0)], 3.2);
    let level = |at: f64| rms(&left[frame(0.1 + at)..frame(0.1 + at + 0.05)]);
    let start = level(0.05);
    let down = |at: f64| 20.0 * (level(at) / start).log10();
    assert!(down(1.6) > -60.0, "{}", down(1.6));
    assert!(down(2.4) < -60.0, "{}", down(2.4));

    let (long, _) = reverb_click(&[("decay_seconds", 6.0)], 3.2);
    let long_level = |at: f64| rms(&long[frame(0.1 + at)..frame(0.1 + at + 0.05)]);
    assert!(20.0 * (long_level(2.4) / long_level(0.05)).log10() > -40.0);

    let (plain, _) = reverb_click(&[], 0.5);
    let (late, _) = reverb_click(&[("predelay_ms", 50.0)], 0.5);
    assert!(first_sound(&plain) >= frame(0.1));
    assert_eq!(first_sound(&late), first_sound(&plain) + frame(0.05));
    assert!(late[..frame(0.15)].iter().all(|x| *x == 0.0));

    let (small, _) = reverb_click(&[("size", 15.0)], 0.5);
    let (large, _) = reverb_click(&[("size", 40.0)], 0.5);
    assert!(first_sound(&large) > first_sound(&small) + frame(0.01));
}

#[test]
fn the_reverbs_low_cut_cuts_the_room_below_its_note_and_its_damp_dulls_it() {
    let hiss = |given: &[(&str, f64)]| {
        let mut given = given.to_vec();
        given.push(("mix", 1.0));
        through("reverb", &given, "hiss", 2.0)
    };
    let (open, cut) = (hiss(&[]), hiss(&[("low_cut", 60.0)]));
    let lows = |samples: &[f32]| partial_db(samples, 65.0, 0.5);
    // One bin of noise wanders, so the level at 65 Hz is the mean over neighbouring frequencies.
    let around = |samples: &[f32]| {
        let hz = [59.0, 61.0, 63.0, 65.0, 67.0, 69.0, 71.0];
        hz.iter()
            .map(|hz| partial_db(samples, *hz, 0.5))
            .sum::<f64>()
            / hz.len() as f64
    };
    assert!(lows(&open).is_finite());
    assert!(
        around(&cut) <= around(&open) - 12.0,
        "{} {}",
        around(&cut),
        around(&open)
    );
    let highs = |samples: &[f32]| {
        let hz = [8000.0, 8100.0, 8200.0, 8300.0, 8400.0];
        hz.iter()
            .map(|hz| partial_db(samples, *hz, 0.5))
            .sum::<f64>()
            / hz.len() as f64
    };
    let (bright, dull) = (hiss(&[("damp", 0.0)]), hiss(&[("damp", 1.0)]));
    assert!(highs(&dull) < highs(&bright) - 12.0);

    let mut engine = engine();
    let tails = engine
        .effect_tails(
            &[("reverb".to_string(), options(&[("decay_seconds", 3.0)]))],
            None,
        )
        .unwrap();
    assert_eq!(tails, vec![3.0]);
}

/// Both sides of a tone at `hz` through an effect with the options given, for the seconds given.
fn tone_through(
    effect: &str,
    given: &[(&str, f64)],
    hz: f64,
    seconds: f64,
) -> (Vec<f32>, Vec<f32>) {
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
    engine.render(seconds).unwrap()
}

/// How far, in dB, the phaser with the options given passes a tone at each frequency above the same tone dry, on the left and on the right.
fn phaser_gains(given: &[(&str, f64)], hz: &[f64], from: f64) -> Vec<(f64, f64)> {
    // A sine is as loud dry at any frequency.
    let (dry, _) = tone_through("level", &[], 440.0, from + 0.5);
    hz.iter()
        .map(|hz| {
            let (left, right) = tone_through("phaser", given, *hz, from + 0.5);
            let level = |samples: &[f32]| {
                20.0 * f64::from(rms(&samples[frame(from)..frame(from + 0.5)])).log10()
            };
            (level(&left) - level(&dry), level(&right) - level(&dry))
        })
        .collect()
}

/// The notches of a phaser held at the note 72: the frequencies where its stages turn the sound half a cycle, an odd number of times.
fn notches(stages: usize) -> Vec<f64> {
    let corner = 440.0 * 2f64.powf((72.0 - 69.0) / 12.0);
    (0..stages / 2)
        .map(|k| corner * (std::f64::consts::PI * (2 * k + 1) as f64 / (2 * stages) as f64).tan())
        .collect()
}

#[test]
fn a_phaser_given_no_mix_is_half_dry_so_its_notches_are_cut() {
    let held = [("low", 72.0), ("high", 72.0)];
    for (left, right) in phaser_gains(&held, &notches(4), 0.5) {
        assert!(left <= -12.0 && right <= -12.0, "{left} {right}");
    }
    let own = |name: &str| {
        let engine = engine();
        let effect = engine
            .effects()
            .find(|effect| effect.name() == name)
            .unwrap();
        let mixes: Vec<Option<Value>> = effect
            .specs()
            .iter()
            .filter(|spec| spec.name == "mix")
            .map(|spec| spec.default.clone())
            .collect();
        mixes
    };
    assert_eq!(own("phaser"), vec![Some(Value::Number(0.5))]);
    assert_eq!(own("echo"), vec![]);
}

#[test]
fn the_phaser_cuts_a_notch_for_every_two_stages_and_none_dry() {
    let grid: Vec<f64> = (0..36).map(|i| 80.0 * 2f64.powf(i as f64 / 6.0)).collect();
    let held = |stages: f64| {
        [
            ("stages", stages),
            ("low", 72.0),
            ("high", 72.0),
            ("mix", 0.5),
        ]
    };
    for stages in [2usize, 4, 6, 8] {
        let at = notches(stages);
        for (left, right) in phaser_gains(&held(stages as f64), &at, 0.5) {
            assert!(left <= -12.0 && right <= -12.0, "{stages} {left} {right}");
        }
        let curve: Vec<f64> = phaser_gains(&held(stages as f64), &grid, 0.5)
            .iter()
            .map(|(left, _)| *left)
            .collect();
        // Each stretch of the curve under -12 dB is one notch.
        let dips = (1..curve.len())
            .filter(|i| curve[*i] <= -12.0 && curve[i - 1] > -12.0)
            .count();
        assert_eq!(dips, stages / 2, "{stages} {curve:?}");
    }

    let dry = through("level", &[], "tone", 1.0);
    let off = through("phaser", &[("mix", 0.0)], "tone", 1.0);
    let apart = dry
        .iter()
        .zip(&off)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(apart <= loudest(&dry) * 1e-5, "{apart}");

    let mut engine = engine();
    let odd = engine
        .start_effects(&[("phaser".to_string(), options(&[("stages", 3.0)]))])
        .unwrap_err();
    assert!(
        odd.to_string().contains("phaser") && odd.to_string().contains("stages"),
        "{odd}"
    );
}

#[test]
fn the_phasers_feedback_raises_the_sound_between_its_notches_and_keeps_them() {
    let corner = 440.0 * 2f64.powf(3.0 / 12.0);
    let at = [notches(4)[0], corner, notches(4)[1]];
    let depth = |feedback: f64| {
        let given = [
            ("stages", 4.0),
            ("low", 72.0),
            ("high", 72.0),
            ("mix", 0.5),
            ("feedback", feedback),
        ];
        let gains = phaser_gains(&given, &at, 0.5);
        (gains[1].0 - gains[0].0, gains[1].0 - gains[2].0, gains[1].0)
    };
    let (plain, fed) = (depth(0.0), depth(0.7));
    assert!(plain.2.abs() < 0.5, "{plain:?}");
    assert!(
        fed.0 >= plain.0 + 6.0 && fed.1 >= plain.1 + 6.0,
        "{plain:?} {fed:?}"
    );
}

#[test]
fn the_phaser_sweeps_its_notches_once_every_phase_and_the_left_the_other_way_when_asked() {
    // A tone at 1000 Hz dips each time a notch passes it; the level is taken every 10 ms from 0.5 seconds.
    let levels = |flipped: f64| {
        let given = [
            ("phase", 0.5),
            ("low", 60.0),
            ("high", 84.0),
            ("mix", 0.5),
            ("stereo_invert_wave", flipped),
        ];
        let (left, right) = tone_through("phaser", &given, 1000.0, 2.0);
        let every = |samples: &[f32]| -> Vec<f32> {
            (0..120)
                .map(|i| {
                    let from = 0.5 + 0.01 * i as f64;
                    rms(&samples[frame(from)..frame(from + 0.01)])
                })
                .collect()
        };
        (every(&left), every(&right))
    };
    // How far apart two runs of levels are, as a share of how far the first moves.
    let apart = |a: &[f32], b: &[f32]| {
        let mean = a.iter().sum::<f32>() / a.len() as f32;
        let moves = a.iter().map(|x| (x - mean).abs()).sum::<f32>();
        a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / moves
    };
    let (left, right) = levels(0.0);
    let deepest = left.iter().fold(f32::MAX, |a, b| a.min(*b));
    let highest = left.iter().fold(0.0f32, |a, b| a.max(*b));
    assert!(deepest < highest * 0.25, "{deepest} {highest}");
    let (again, half) = (
        apart(&left[..50], &left[50..100]),
        apart(&left[..50], &left[25..75]),
    );
    assert!(again < 0.1 && half > 0.5, "{again} {half}");
    assert!(apart(&left[..100], &right[..100]) < 0.05);

    let (left, right) = levels(1.0);
    let (together, half) = (
        apart(&left[..50], &right[..50]),
        apart(&left[..50], &right[25..75]),
    );
    assert!(together > 0.5 && half < 0.1, "{together} {half}");
}

/// How far, in dB, the part of `samples` at `high` hertz is above the part at `low` in the tenth of a second from `from`.
fn tilt(samples: &[f32], high: f64, low: f64, from: f64) -> f64 {
    let part = |hz: f64| {
        let turn = std::f64::consts::TAU * hz / RATE;
        let (re, im) = samples[frame(from)..frame(from + 0.09)]
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(re, im), (n, x)| {
                let angle = turn * n as f64;
                (
                    re + f64::from(*x) * angle.cos(),
                    im + f64::from(*x) * angle.sin(),
                )
            });
        10.0 * (re * re + im * im).log10()
    };
    part(high) - part(low)
}

/// A click through `effect` with echoes 0.1 seconds apart, both sides added, since ping_pong puts each echo on one.
fn echoed_click(effect: &str, given: &[(&str, f64)]) -> Vec<f32> {
    let mut engine = engine();
    let mut all = vec![("phase", 0.1)];
    all.extend_from_slice(given);
    let chain = engine
        .start_effects(&[(effect.to_string(), options(&all))])
        .unwrap();
    let note = Event::Note {
        id: None,
        synth: "click".to_string(),
        options: vec![],
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(0.0, note)]).unwrap();
    let (left, right) = engine.render(0.5).unwrap();
    left.iter().zip(&right).map(|(l, r)| l + r).collect()
}

#[test]
fn an_echo_cuts_each_repeat_above_its_high_cut_and_below_its_low_cut_once_more_than_the_one_before()
{
    for (effect, long) in [("echo", ("decay", 1.0)), ("ping_pong", ("feedback", 0.7))] {
        for (cut, high, low) in [
            (("high_cut", 84.0), 4000.0, 500.0),
            (("low_cut", 60.0), 100.0, 1000.0),
        ] {
            let sound = echoed_click(effect, &[long, cut]);
            let tilts: Vec<f64> = [0.0, 0.1, 0.2, 0.3]
                .iter()
                .map(|at| tilt(&sound, high, low, at - 0.001))
                .collect();
            for pair in tilts.windows(2) {
                assert!(pair[1] <= pair[0] - 3.0, "{effect} {cut:?} {tilts:?}");
            }
        }
        let plain = echoed_click(effect, &[long]);
        let flat = tilt(&plain, 4000.0, 500.0, 0.299) - tilt(&plain, 4000.0, 500.0, 0.0);
        assert!(flat.abs() < 0.01, "{effect} {flat}");
    }
}

#[test]
fn an_echos_drive_rounds_each_repeat_off_more_than_the_one_before() {
    for (effect, long) in [
        ("echo", ("decay", 150.0)),
        ("ping_pong", ("feedback", 0.97)),
    ] {
        let mut engine = engine();
        // A tone a quarter of full scale, as a part in a mix is.
        let given = [
            ("phase", 2.5),
            ("max_phase", 3.0),
            long,
            ("drive", 0.5),
            ("pre_amp", 0.5),
        ];
        let chain = engine
            .start_effects(&[(effect.to_string(), options(&given))])
            .unwrap();
        let note = Event::Note {
            id: None,
            synth: "tone".to_string(),
            options: vec![],
            chain: Some(chain),
            held: false,
        };
        engine.play(&[(0.0, note)]).unwrap();
        let (left, right) = engine.render(9.5).unwrap();
        let sound: Vec<f32> = left.iter().zip(&right).map(|(l, r)| l + r).collect();
        let third = |from: f64| tilt(&sound, 1320.0, 440.0, from);
        let (dry, first, last) = (third(1.0), third(3.5), third(8.5));
        assert!(dry < -40.0, "{effect} {dry}");
        assert!(last >= first + 6.0, "{effect} {first} {last}");
    }
}

/// A synth of the test engine at `hz` through the distortion with the options given, for two seconds, as the left channel.
fn distorted(synth: &str, hz: f64, given: &[(&str, Value)]) -> Vec<f32> {
    let mut engine = engine();
    let given: Vec<(String, Value)> = given
        .iter()
        .map(|(name, value)| (name.to_string(), value.clone()))
        .collect();
    let chain = engine
        .start_effects(&[("distortion".to_string(), given)])
        .unwrap();
    let note = Event::Note {
        id: None,
        synth: synth.to_string(),
        options: options(&[("hz", hz)]),
        chain: Some(chain),
        held: false,
    };
    engine.play(&[(0.0, note)]).unwrap();
    engine.render(2.0).unwrap().0
}

/// How far, in dB, the energy that is on no multiple of `hz`, 0 among them, is below the whole sound, over a second from half a second in.
fn away_from_harmonics(samples: &[f32], hz: f64) -> f64 {
    let count = RATE as usize;
    let window: Vec<f64> = (0..count)
        .map(|n| 0.5 - 0.5 * (std::f64::consts::TAU * n as f64 / count as f64).cos())
        .collect();
    let sound: Vec<f64> = samples[frame(0.5)..frame(0.5) + count]
        .iter()
        .zip(&window)
        .map(|(x, w)| f64::from(*x) * w)
        .collect();
    let whole: f64 = sound.iter().map(|x| x * x).sum();
    let (sum, squares): (f64, f64) = window
        .iter()
        .fold((0.0, 0.0), |(sum, squares), w| (sum + w, squares + w * w));
    let on: f64 = (0..=(RATE / 2.0 / hz) as usize)
        .map(|k| {
            let turn = std::f64::consts::TAU * k as f64 * hz / RATE;
            let (re, im) = sound
                .iter()
                .enumerate()
                .fold((0.0, 0.0), |(re, im), (n, x)| {
                    let angle = turn * n as f64;
                    (re + x * angle.cos(), im + x * angle.sin())
                });
            let both = if k == 0 { 1.0 } else { 2.0 };
            both * (re * re + im * im) * squares / (sum * sum)
        })
        .sum();
    10.0 * ((whole - on).max(whole * 1e-12) / whole).log10()
}

#[test]
fn the_distortion_adds_nothing_away_from_the_notes_harmonics_on_a_high_saw() {
    let c7 = 2093.0;
    let dry = distorted("saw", c7, &[("mix", Value::Number(0.0))]);
    let clean = away_from_harmonics(&dry, c7);
    assert!(clean < -45.0, "{clean}");
    for curve in ["soft", "hard"] {
        for distort in [0.5, 1.0] {
            let given = [
                ("distort", Value::Number(distort)),
                ("curve", Value::from(curve)),
            ];
            let away = away_from_harmonics(&distorted("saw", c7, &given), c7);
            assert!(away <= -40.0, "{curve} {distort} {away}");
        }
    }
}

#[test]
fn the_soft_distortion_of_a_low_sine_has_the_level_and_harmonics_of_a_tanh_of_it() {
    let dry = distorted("tone", 110.0, &[("mix", Value::Number(0.0))]);
    for distort in [0.0, 0.5, 1.0] {
        let drive = 1.0 + distort as f32 * 30.0;
        let plain: Vec<f32> = dry
            .iter()
            .map(|x| (x * drive).tanh() / drive.tanh())
            .collect();
        let wet = distorted("tone", 110.0, &[("distort", Value::Number(distort))]);
        for harmonic in [1.0, 3.0, 5.0] {
            let (was, is) = (
                partial_db(&plain, 110.0 * harmonic, 0.5),
                partial_db(&wet, 110.0 * harmonic, 0.5),
            );
            assert!(
                (was - is).abs() < 1.0 || was < -80.0,
                "{distort} {harmonic} {was} {is}"
            );
        }
    }
}

#[test]
fn the_distortions_tone_cuts_above_its_note_and_is_off_at_0() {
    let driven = [("distort", Value::Number(0.5))];
    let toned = [
        ("distort", Value::Number(0.5)),
        ("tone", Value::Number(84.0)),
    ];
    let open = partial_db(&distorted("saw", 220.0, &driven), 3960.0, 0.5);
    let cut = partial_db(&distorted("saw", 220.0, &toned), 3960.0, 0.5);
    assert!(cut <= open - 12.0, "{open} {cut}");
    let low = (
        partial_db(&distorted("saw", 220.0, &driven), 220.0, 0.5),
        partial_db(&distorted("saw", 220.0, &toned), 220.0, 0.5),
    );
    assert!((low.0 - low.1).abs() < 1.0, "{low:?}");
}

#[test]
fn the_hard_distortion_cuts_a_sine_off_flat_and_an_unknown_curve_is_refused() {
    let given = [
        ("distort", Value::Number(1.0)),
        ("curve", Value::from("hard")),
    ];
    let wet = distorted("tone", 440.0, &given);
    let mut levels: Vec<f32> = wet[frame(0.5)..frame(1.5)]
        .iter()
        .map(|x| x.abs())
        .collect();
    levels.sort_by(f32::total_cmp);
    let top = levels[levels.len() / 2];
    let flat = levels
        .iter()
        .filter(|x| (*x - top).abs() < 0.03 * top)
        .count();
    assert!(flat as f64 > 0.5 * levels.len() as f64, "{flat}");
    let soft = distorted("tone", 440.0, &[("distort", Value::Number(1.0))]);
    assert!(loudest(&soft[frame(0.5)..frame(1.5)]) <= 1.01);

    let mut engine = engine();
    let warm = vec![("curve".to_string(), Value::from("warm"))];
    assert!(
        engine
            .start_effects(&[("distortion".to_string(), warm)])
            .is_err()
    );
}

#[test]
fn the_distortion_mixed_half_with_the_dry_sound_cancels_none_of_it() {
    for hz in [1000.0, 6000.0, 12000.0, 18000.0] {
        let level = |mix: f64| {
            let given = [("distort", Value::Number(0.0)), ("mix", Value::Number(mix))];
            f64::from(rms(&distorted("tone", hz, &given)[frame(0.5)..frame(1.5)]))
        };
        let (dry, half, wet) = (level(0.0), level(0.5), level(1.0));
        let together = 20.0 * (half / ((dry + wet) / 2.0)).log10();
        assert!(together > -0.5, "{hz} {together}");
    }
}

/// Both sides of tones, each at a frequency and a pan, through an effect with the options given, for 1.5 seconds.
fn tones_through(
    effect: &str,
    given: &[(&str, f64)],
    tones: &[(f64, f64)],
) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[(effect.to_string(), options(given))])
        .unwrap();
    let events: Vec<_> = tones
        .iter()
        .map(|(hz, pan)| {
            let note = Event::Note {
                id: None,
                synth: "tone".to_string(),
                options: options(&[("hz", *hz), ("pan", *pan), ("amp", 0.5)]),
                chain: Some(chain),
                held: false,
            };
            (0.0, note)
        })
        .collect();
    engine.play(&events).unwrap();
    engine.render(1.5).unwrap()
}

fn db_between(a: &[f32], b: &[f32]) -> f64 {
    20.0 * f64::from(rms(a) / rms(b)).log10()
}

#[test]
fn width_narrows_the_sound_to_mono_at_side_0_leaves_it_at_1_and_doubles_the_difference_at_2() {
    let tones = [(440.0, -1.0), (660.0, 0.5)];
    let (dry_left, dry_right) = tones_through("level", &[], &tones);
    let apart = |left: &[f32], right: &[f32]| -> Vec<f32> {
        left.iter().zip(right).map(|(l, r)| l - r).collect()
    };
    let added = |left: &[f32], right: &[f32]| -> Vec<f32> {
        left.iter().zip(right).map(|(l, r)| l + r).collect()
    };
    assert!(rms(&apart(&dry_left, &dry_right)) > 0.05);

    let (left, right) = tones_through("width", &[("side", 0.0)], &tones);
    assert_eq!(left, right);
    assert!(db_between(&added(&left, &right), &added(&dry_left, &dry_right)).abs() < 0.1);

    for given in [vec![], vec![("side", 1.0)]] {
        let (left, right) = tones_through("width", &given, &tones);
        let floor = loudest(&dry_left) * 1e-5;
        assert!(loudest(&apart(&left, &dry_left)) <= floor);
        assert!(loudest(&apart(&right, &dry_right)) <= floor);
    }

    let (left, right) = tones_through("width", &[("side", 2.0)], &tones);
    let wider = db_between(&apart(&left, &right), &apart(&dry_left, &dry_right));
    assert!((wider - 6.02).abs() < 0.5, "{wider}");
    assert!(db_between(&added(&left, &right), &added(&dry_left, &dry_right)).abs() < 0.1);
}

#[test]
fn widths_mono_below_puts_what_is_under_its_note_in_the_centre_and_leaves_the_rest() {
    let tones = [(60.0, -1.0), (2000.0, -1.0)];
    let (dry_left, dry_right) = tones_through("level", &[], &tones);
    assert!(partial_db(&dry_right, 60.0, 0.5) < partial_db(&dry_left, 60.0, 0.5) - 30.0);

    let (left, right) = tones_through("width", &[("mono_below", 48.0)], &tones);
    let low = partial_db(&left, 60.0, 0.5) - partial_db(&right, 60.0, 0.5);
    assert!(low.abs() < 1.0, "{low}");
    let high = partial_db(&left, 2000.0, 0.5) - partial_db(&right, 2000.0, 0.5);
    assert!(high >= 30.0, "{high}");
    let kept = partial_db(&left, 2000.0, 0.5) - partial_db(&dry_left, 2000.0, 0.5);
    assert!(kept.abs() < 0.1, "{kept}");
}

#[test]
fn widths_side_and_mono_below_glide_to_a_new_value_over_their_slide() {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[("width".to_string(), vec![])])
        .unwrap();
    let note = Event::Note {
        id: None,
        synth: "steady".to_string(),
        options: options(&[("pan", -1.0)]),
        chain: Some(chain),
        held: false,
    };
    let set = Event::Set {
        target: Target::Effect(chain, 0),
        options: vec![("side".to_string(), 0.0), ("side_slide".to_string(), 1.0)],
    };
    engine.play(&[(0.0, note), (0.5, set)]).unwrap();
    let (left, right) = engine.render(2.0).unwrap();
    let share = |at: f64| right[frame(at)] / left[frame(at)];
    assert!(share(0.4) < 0.01);
    assert!(share(1.0) > 0.2 && share(1.0) < 0.5, "{}", share(1.0));
    assert!((share(1.7) - 1.0).abs() < 0.01);
}

#[test]
fn a_tremolos_spread_runs_the_right_sides_sweep_that_share_of_half_a_cycle_after_the_lefts() {
    let levels = |given: &[(&str, f64)]| {
        let (left, right) = tones_through("tremolo", given, &[(440.0, 0.0)]);
        let chunks =
            |side: &[f32]| -> Vec<f32> { side[..frame(1.5)].chunks(480).map(rms).collect() };
        (chunks(&left), chunks(&right))
    };
    let (left, right) = levels(&[("phase", 1.0), ("depth", 1.0), ("spread", 1.0)]);
    let peak = left.iter().cloned().fold(0.0, f32::max);
    // The triangle is loudest three quarters into each cycle: the right is there half a second after the left.
    for (at, loud, quiet) in [
        (25, &right, &left),
        (75, &left, &right),
        (125, &right, &left),
    ] {
        assert!(loud[at] > 0.9 * peak, "{at} {}", loud[at]);
        assert!(quiet[at] < 0.1 * peak, "{at} {}", quiet[at]);
    }

    let (left, right) = levels(&[("phase", 1.0), ("depth", 1.0), ("spread", 0.5)]);
    assert!(left[75] > 0.9 * peak && right[100] > 0.9 * peak);

    for given in [vec![], vec![("spread", 0.0)]] {
        let (left, right) = tones_through("tremolo", &given, &[(440.0, 0.0)]);
        assert_eq!(left, right);
        let (plain, _) = tones_through("tremolo", &[], &[(440.0, 0.0)]);
        assert_eq!(left, plain);
    }
}

#[test]
fn a_slicers_spread_of_1_sounds_one_side_at_a_time() {
    let both = |given: &[(&str, f64)]| {
        let mut engine = engine();
        let chain = engine
            .start_effects(&[("slicer".to_string(), options(given))])
            .unwrap();
        let note = Event::Note {
            id: None,
            synth: "steady".to_string(),
            options: vec![],
            chain: Some(chain),
            held: false,
        };
        engine.play(&[(0.0, note)]).unwrap();
        engine.render(2.0).unwrap()
    };
    let (left, right) = both(&[("phase", 1.0), ("spread", 1.0)]);
    let (mut lefts, mut rights, mut both_on) = (0, 0, 0);
    for (l, r) in left.iter().zip(&right) {
        both_on += usize::from(*l != 0.0 && *r != 0.0);
        lefts += usize::from(*l != 0.0);
        rights += usize::from(*r != 0.0);
    }
    assert_eq!(both_on, 0);
    assert!(
        lefts > frame(0.9) && rights > frame(0.9),
        "{lefts} {rights}"
    );

    let (left, right) = both(&[("phase", 1.0)]);
    assert_eq!(left, right);
}

/// Both sides of notes, each a synth at a time, an amp and a length or frequency, through a wah with the options given, for 2 seconds.
fn wah(given: &[(&str, f64)], notes: &[(&str, f64, f64, (&str, f64))]) -> (Vec<f32>, Vec<f32>) {
    notes_through("wah", given, notes)
}

fn notes_through(
    effect: &str,
    given: &[(&str, f64)],
    notes: &[(&str, f64, f64, (&str, f64))],
) -> (Vec<f32>, Vec<f32>) {
    let mut engine = engine();
    let chain = engine
        .start_effects(&[(effect.to_string(), options(given))])
        .unwrap();
    let events: Vec<_> = notes
        .iter()
        .map(|(synth, at, amp, (name, value))| {
            let note = Event::Note {
                id: None,
                synth: synth.to_string(),
                options: options(&[("amp", *amp), (name, *value)]),
                chain: Some(chain),
                held: false,
            };
            (*at, note)
        })
        .collect();
    engine.play(&events).unwrap();
    engine.render(2.0).unwrap()
}

/// How far, in dB, a wah with the options given raises a saw's partial at 1980 Hz, the saw at 220 Hz and the amp given.
fn wah_gain(given: &[(&str, f64)], amp: f64) -> f64 {
    let saw = [("saw", 0.0, amp, ("hz", 220.0))];
    let (wet, _) = wah(given, &saw);
    let dry: Vec<(&str, f64)> = given.iter().cloned().chain([("mix", 0.0)]).collect();
    let (dry, _) = wah(&dry, &saw);
    partial_db(&wet, 1980.0, 0.5) - partial_db(&dry, 1980.0, 0.5)
}

#[test]
fn a_wah_opens_its_filter_further_the_louder_the_sound_coming_in() {
    let open = [("cutoff_min", 60.0), ("cutoff_max", 100.0)];
    let (loud, quiet) = (wah_gain(&open, 1.0), wah_gain(&open, 0.1));
    assert!(loud > quiet + 12.0, "{loud} {quiet}");
    assert!(quiet < -12.0, "{quiet}");

    let closing = [("cutoff_min", 100.0), ("cutoff_max", 60.0)];
    let (loud, quiet) = (wah_gain(&closing, 1.0), wah_gain(&closing, 0.1));
    assert!(loud < quiet - 12.0, "{loud} {quiet}");

    let raised = [
        ("cutoff_min", 60.0),
        ("cutoff_max", 100.0),
        ("sensitivity", 12.0),
    ];
    let level = 10f64.powf(-12.0 / 20.0);
    let same = wah_gain(&raised, level) - wah_gain(&open, 1.0);
    assert!(same.abs() < 0.5, "{same}");
}

#[test]
fn a_wah_given_mix_0_is_the_dry_sound_and_moves_both_sides_alike() {
    let saw = [("saw", 0.0, 1.0, ("hz", 220.0))];
    let (dry, _) = notes_through("level", &[], &saw);
    let (wet, _) = wah(&[("mix", 0.0)], &saw);
    assert!(loudest(&dry) > 0.1);
    let floor = loudest(&dry) * 1e-5;
    for (x, y) in dry.iter().zip(&wet) {
        assert!((x - y).abs() <= floor);
    }
    let (left, right) = wah(&[], &saw);
    assert_eq!(left, right);
    assert!(apart(&left, &dry, 0.5, 1.5) > 0.01);
}

#[test]
fn a_wah_closes_over_its_relax_ms_after_the_sound_stops_and_opens_over_its_clamp_ms() {
    // A loud burst of noise for half a second over a quiet saw whose partial at 2000 Hz shows where the filter is.
    let notes = [
        ("hiss", 0.0, 2.0, ("length", 0.5)),
        ("saw", 0.0, 0.004, ("hz", 500.0)),
    ];
    let level = |samples: &[f32], at: f64| {
        let window = &samples[frame(at)..frame(at + 0.02)];
        let (mut sin, mut cos) = (0.0, 0.0);
        for (i, x) in window.iter().enumerate() {
            let angle = std::f64::consts::TAU * 2000.0 * i as f64 / RATE;
            sin += f64::from(*x) * angle.sin();
            cos += f64::from(*x) * angle.cos();
        }
        10.0 * (sin * sin + cos * cos).log10()
    };
    // The seconds after the burst ends at which the partial is 12 dB under where it was 10 ms after it.
    let falls = |relax: f64| {
        let (left, _) = wah(&[("relax_ms", relax)], &notes);
        let start = level(&left, 0.51);
        (0..140)
            .map(|step| 0.51 + f64::from(step) * 0.01)
            .find(|at| level(&left, *at) <= start - 12.0)
            .unwrap()
            - 0.5
    };
    let (fast, slow) = (falls(100.0), falls(500.0));
    assert!(slow >= 2.0 * fast, "{fast} {slow}");
    assert!(fast < 0.2, "{fast}");
}
