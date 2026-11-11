use mxs::keys::{Keyboard, choose_input};
use mxs::{Engine, Event, Note, RATE, Spec, Synth, synths};

/// A synth whose sound is its note times its vel, which a key holds and lets go over a release of 0.1 seconds.
fn tone() -> Synth {
    Synth::new("tone")
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
    let events: Vec<(f64, Event)> = messages.iter().flat_map(|(at, bytes)| keyboard.message(bytes, *at)).collect();
    engine.play(&events).unwrap();
    (engine.render(1.0).unwrap().0, events)
}

const CENTRE: f32 = std::f32::consts::FRAC_1_SQRT_2;

#[test]
fn a_key_plays_at_its_pitch_and_loudness_until_it_comes_up() {
    let (left, _) = played(tone(), &[(0.0, [0x90, 60, 127]), (0.0, [0x90, 64, 64]), (0.2, [0x80, 60, 0]), (0.4, [0x90, 64, 0])]);
    let both = CENTRE * (60.0 + 64.0 * 64.0 / 127.0);
    assert!((left[frame(0.1)] - both).abs() < 1e-3, "{} against {both}", left[frame(0.1)]);
    let one = CENTRE * 64.0 * 64.0 / 127.0;
    assert!(left[frame(0.25)] > one, "the first is falling: {}", left[frame(0.25)]);
    assert!((left[frame(0.35)] - one).abs() < 1e-3, "{}", left[frame(0.35)]);
    assert!(left[frame(0.52)..].iter().all(|&x| x.abs() < 1e-6), "{}", left[frame(0.52)]);
}

#[test]
fn the_sustain_pedal_holds_notes_whose_keys_came_up_until_it_does() {
    let (left, _) = played(tone(), &[(0.0, [0x90, 60, 127]), (0.05, [0xb0, 64, 127]), (0.1, [0x80, 60, 0]), (0.4, [0xb0, 64, 0])]);
    assert!((left[frame(0.35)] - CENTRE * 60.0).abs() < 1e-3, "{}", left[frame(0.35)]);
    assert!(left[frame(0.52)..].iter().all(|&x| x.abs() < 1e-6), "{}", left[frame(0.52)]);
}

#[test]
fn the_pitch_bend_wheel_bends_sounding_notes_two_semitones_at_its_ends() {
    let (left, _) = played(tone(), &[(0.0, [0x90, 60, 127]), (0.2, [0xe0, 0x7f, 0x7f]), (0.5, [0xe0, 0, 0])]);
    let up = 60.0 + 2.0 * 8191.0 / 8192.0;
    assert!((left[frame(0.4)] - CENTRE * up).abs() < 1e-3, "{}", left[frame(0.4)]);
    assert!((left[frame(0.7)] - CENTRE * 58.0).abs() < 1e-3, "{}", left[frame(0.7)]);
    assert!(left[frame(0.205)] > CENTRE * 60.0 && left[frame(0.205)] < CENTRE * up, "it glides: {}", left[frame(0.205)]);
}

#[test]
fn the_piano_takes_the_velocity_as_vel_and_is_struck_with_its_pedal_down_while_the_pedal_is() {
    let (left, events) = played(synths::all().into_iter().find(|s| s.name() == "piano").unwrap(), &[(0.0, [0xb0, 64, 127]), (0.0, [0x90, 60, 100])]);
    assert!(left.iter().any(|x| x.abs() > 0.01));
    let Event::Note { options, .. } = &events.iter().find(|(_, e)| matches!(e, Event::Note { .. })).unwrap().1 else { unreachable!() };
    let value = |name: &str| options.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
    assert_eq!(value("vel"), Some(mxs::Value::Number(100.0 / 127.0)));
    assert_eq!(value("pedal"), Some(mxs::Value::Number(1.0)));
    assert_eq!(value("note"), Some(mxs::Value::Number(60.0)));
}

#[test]
fn an_input_that_does_not_exist_is_refused_listing_those_that_do() {
    let names = vec!["Keystation 49".to_string(), "IAC Bus 1".to_string()];
    assert_eq!(choose_input(&names, Some("Launchkey")), Err("no MIDI input named Launchkey; the inputs are Keystation 49, IAC Bus 1".to_string()));
    assert_eq!(choose_input(&names, Some("IAC Bus 1")), Ok(1));
    assert_eq!(choose_input(&[], None), Err("no MIDI input to play from; there are none".to_string()));
}
