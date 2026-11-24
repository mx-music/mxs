use mxs::keys::{Keyboard, choose_input};
use mxs::{Engine, Event, Note, RATE, Spec, Synth, SynthDecl, synths};

/// A synth whose sound is its note times its vel, which a key holds and lets go over a release of 0.1 seconds.
fn tone() -> Synth {
    SynthDecl::new("tone")
        .option(Spec::note("note").default(60.0))
        .option(Spec::float("vel").default(0.5).range(0.0, 1.0))
        .options(Spec::envelope(1.0, 0.1))
        .build(|o, _| {
            let vel = o.number("vel") as f32;
            Note::envelope(o.follow("note") * vel, o)
        })
}

fn frame(seconds: f64) -> usize {
    (seconds * RATE) as usize
}

/// Plays `messages`, each a time and its MIDI bytes, through a keyboard of `synth`, and renders a second.
fn played(synth: Synth, messages: &[(f64, [u8; 3])]) -> (Vec<f32>, Vec<(f64, Event)>) {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let mut keyboard = Keyboard::new(&synth);
    engine.register(synth);
    let events: Vec<(f64, Event)> = messages
        .iter()
        .flat_map(|(at, bytes)| keyboard.message(bytes, *at))
        .collect();
    engine.play(&events).unwrap();
    (engine.render(1.0).unwrap().0, events)
}

const CENTRE: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[test]
fn a_key_plays_at_its_pitch_and_loudness_until_it_comes_up() {
    let (left, _) = played(
        tone(),
        &[
            (0.0, [0x90, 60, 127]),
            (0.0, [0x90, 64, 64]),
            (0.2, [0x80, 60, 0]),
            (0.4, [0x90, 64, 0]),
        ],
    );
    let both = CENTRE * (60.0 + 64.0 * 64.0 / 127.0);
    assert!(
        (left[frame(0.1)] - both).abs() < 1e-3,
        "{} against {both}",
        left[frame(0.1)]
    );
    let one = CENTRE * 64.0 * 64.0 / 127.0;
    assert!(
        left[frame(0.25)] > one,
        "the first is falling: {}",
        left[frame(0.25)]
    );
    assert!(
        (left[frame(0.35)] - one).abs() < 1e-3,
        "{}",
        left[frame(0.35)]
    );
    assert!(
        left[frame(0.52)..].iter().all(|&x| x.abs() < 1e-6),
        "{}",
        left[frame(0.52)]
    );
}

#[test]
fn the_sustain_pedal_holds_notes_whose_keys_came_up_until_it_does() {
    let (left, _) = played(
        tone(),
        &[
            (0.0, [0x90, 60, 127]),
            (0.05, [0xb0, 64, 127]),
            (0.1, [0x80, 60, 0]),
            (0.4, [0xb0, 64, 0]),
        ],
    );
    assert!(
        (left[frame(0.35)] - CENTRE * 60.0).abs() < 1e-3,
        "{}",
        left[frame(0.35)]
    );
    assert!(
        left[frame(0.52)..].iter().all(|&x| x.abs() < 1e-6),
        "{}",
        left[frame(0.52)]
    );
}

#[test]
fn the_pitch_bend_wheel_bends_sounding_notes_two_semitones_at_its_ends() {
    let (left, _) = played(
        tone(),
        &[
            (0.0, [0x90, 60, 127]),
            (0.2, [0xe0, 0x7f, 0x7f]),
            (0.5, [0xe0, 0, 0]),
        ],
    );
    let up = 60.0 + 2.0 * 8191.0 / 8192.0;
    assert!(
        (left[frame(0.4)] - CENTRE * up).abs() < 1e-3,
        "{}",
        left[frame(0.4)]
    );
    assert!(
        (left[frame(0.7)] - CENTRE * 58.0).abs() < 1e-3,
        "{}",
        left[frame(0.7)]
    );
    assert!(
        left[frame(0.205)] > CENTRE * 60.0 && left[frame(0.205)] < CENTRE * up,
        "it glides: {}",
        left[frame(0.205)]
    );
}

#[test]
fn the_piano_takes_the_velocity_as_vel_and_is_struck_with_its_pedal_down_while_the_pedal_is() {
    let (left, events) = played(
        synths::all()
            .into_iter()
            .find(|s| s.name() == "piano")
            .unwrap(),
        &[(0.0, [0xb0, 64, 127]), (0.0, [0x90, 60, 100])],
    );
    assert!(left.iter().any(|x| x.abs() > 0.01));
    let Event::Note { options, .. } = &events
        .iter()
        .find(|(_, e)| matches!(e, Event::Note { .. }))
        .unwrap()
        .1
    else {
        unreachable!()
    };
    let value = |name: &str| {
        options
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    assert_eq!(value("vel"), Some(mxs::Value::Number(100.0 / 127.0)));
    assert_eq!(value("pedal"), Some(mxs::Value::Number(1.0)));
    assert_eq!(value("note"), Some(mxs::Value::Number(60.0)));
}

#[test]
fn an_input_that_does_not_exist_is_refused_listing_those_that_do() {
    let names = vec!["Keystation 49".to_string(), "IAC Bus 1".to_string()];
    assert_eq!(
        choose_input(&names, Some("Launchkey")),
        Err("no MIDI input named Launchkey; the inputs are Keystation 49, IAC Bus 1".to_string())
    );
    assert_eq!(choose_input(&names, Some("IAC Bus 1")), Ok(1));
    assert_eq!(
        choose_input(&[], None),
        Err("no MIDI input to play from; there are none".to_string())
    );
}

fn synth(name: &str) -> Synth {
    synths::all()
        .into_iter()
        .find(|s| s.name() == name)
        .unwrap()
}

/// The magnitude of `samples` at `hz`, as the amplitude of a sine there.
fn magnitude(samples: &[f32], hz: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * hz / RATE;
    let (re, im) = samples
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(re, im), (i, x)| {
            let x = *x as f64;
            (re + x * (w * i as f64).cos(), im - x * (w * i as f64).sin())
        });
    2.0 * (re * re + im * im).sqrt() / samples.len() as f64
}

/// The frequency between `low` and `high` where `samples` are loudest, to a tenth of a hertz.
fn peak_between(samples: &[f32], low: f64, high: f64) -> f64 {
    let steps = ((high - low) * 10.0) as usize;
    (0..=steps)
        .map(|k| low + k as f64 / 10.0)
        .max_by(|a, b| magnitude(samples, *a).total_cmp(&magnitude(samples, *b)))
        .unwrap()
}

#[test]
fn the_wheel_at_its_top_bends_a_held_piano_and_violin_note_two_semitones_up_within_10_ms() {
    let middle = 261.63;
    for name in ["piano", "violin"] {
        let (left, events) = played(
            synth(name),
            &[(0.0, [0x90, 60, 76]), (0.5, [0xe0, 0x7f, 0x7f])],
        );
        let window = |from: f64| &left[frame(from)..frame(from + 0.05)];
        let was = peak_between(window(0.45), 0.9 * middle, 1.05 * middle);
        let is = peak_between(window(0.51), 1.05 * middle, 1.2 * middle);
        let moved = 12.0 * (is / was).log2();
        println!("{name}: {was:.1} to {is:.1} Hz, {moved:.3} semitones");
        assert!((moved - 2.0).abs() < 0.1, "{name} moved {moved} semitones");
        let bent = events.iter().any(|(_, event)| match event {
            Event::Set { options, .. } => options.iter().any(|(n, _)| n == "bend"),
            _ => false,
        });
        assert!(bent, "{name}'s wheel sets no bend: {events:?}");
    }
}

/// The loudness of each 50 ms of `samples`.
fn levels(samples: &[f32]) -> Vec<f64> {
    samples
        .chunks(frame(0.05))
        .map(|chunk| {
            (chunk.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / chunk.len() as f64).sqrt()
        })
        .collect()
}

#[test]
fn a_viola_key_held_for_5_seconds_swells_as_a_note_of_its_default_sustain_and_sounds_until_let_up()
{
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let mut keyboard = Keyboard::new(&synth("viola"));
    let mut events = keyboard.message(&[0x90, 60, 76], 0.0);
    events.extend(keyboard.message(&[0x80, 60, 0], 5.0));
    engine.play(&events).unwrap();
    let (held, _) = engine.render(6.0).unwrap();
    let mut plain = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| plain.register(s));
    let vel = 76.0 / 127.0;
    let note = Event::note(None, "viola", &[("note", 60.0), ("vel", vel)]);
    plain.play(&[(0.0, note)]).unwrap();
    let (expected, _) = plain.render(0.8).unwrap();
    let (swell, swelled) = (levels(&held[..frame(0.8)]), levels(&expected));
    println!("held {swell:.3?}\nplain {swelled:.3?}");
    for (window, (h, p)) in swell.iter().zip(&swelled).enumerate() {
        let apart = 20.0 * (h / p).log10();
        assert!(
            apart.abs() < 1.0,
            "window {window} is {apart} dB from the plain note's"
        );
    }
    let late = levels(&held[frame(4.8)..frame(4.95)]);
    assert!(
        late.iter().all(|l| *l > 0.01),
        "it fades before its key is up: {late:?}"
    );
    let after = &held[frame(5.6)..];
    assert!(
        after.iter().all(|s| s.abs() < 1e-4),
        "it sounds once let up"
    );
}
