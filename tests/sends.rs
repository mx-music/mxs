use fundsp::prelude::*;
use mxs::{Engine, Error, Event, Note, RATE, Spec, Synth, Target, Value, effects};

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    engine.register(
        Synth::new("steady").build(|_, _| Note::rings(Net::wrap(Box::new(dc(0.5))), 3.0)),
    );
    engine.register(
        Synth::new("tone")
            .option(Spec::hz("hz").default(440.0))
            .option(Spec::seconds("length").default(2.0))
            .build(|o, _| {
                let tone = Net::wrap(Box::new(sine_hz::<f32>(o.number("hz") as f32))) * 0.5;
                Note::rings(tone, o.number("length"))
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

fn effect(name: &str, given: &[(&str, f64)]) -> (String, Vec<(String, Value)>) {
    (name.to_string(), options(given))
}

fn note(synth: &str, given: &[(&str, f64)], chain: Option<u64>) -> Event {
    Event::Note {
        id: None,
        synth: synth.to_string(),
        options: options(given),
        chain,
        held: false,
    }
}

fn frame(seconds: f64) -> usize {
    (seconds * RATE).round() as usize
}

fn rms(samples: &[f32]) -> f32 {
    (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt()
}

fn loudest(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |peak, x| peak.max(x.abs()))
}

fn db(ratio: f32) -> f32 {
    20.0 * ratio.log10()
}

const REVERB: &[(&str, f64)] = &[("mix", 1.0), ("room", 0.3)];

/// Two tones a fifth apart for a second, each in a chain of its own made by `chain`, as the left channel of 2 seconds.
fn two_tones(chain: impl Fn(&mut Engine) -> u64) -> Vec<f32> {
    let mut engine = engine();
    for hz in [440.0, 660.0] {
        let part = chain(&mut engine);
        let tone = note("tone", &[("hz", hz), ("length", 1.0)], Some(part));
        engine.play(&[(0.0, tone)]).unwrap();
    }
    engine.render(2.0).unwrap().0
}

#[test]
fn two_parts_sending_into_one_reverb_sound_as_each_through_a_reverb_of_its_own() {
    let own = two_tones(|engine| engine.start_effects(&[effect("reverb", REVERB)]).unwrap());
    let dry = two_tones(|engine| engine.start_effects(&[]).unwrap());
    let shared = {
        let mut engine = engine();
        let reverb = engine.start_effects(&[effect("reverb", REVERB)]).unwrap();
        for hz in [440.0, 660.0] {
            let part = engine.start_effects(&[]).unwrap();
            engine.send(part, reverb, 1.0).unwrap();
            let tone = note("tone", &[("hz", hz), ("length", 1.0)], Some(part));
            engine.play(&[(0.0, tone)]).unwrap();
        }
        engine.render(2.0).unwrap().0
    };
    let peak = loudest(&own);
    assert!(peak > 0.05);
    let apart = (0..own.len())
        .map(|i| (shared[i] - dry[i] - own[i]).abs())
        .fold(0.0, f32::max);
    assert!(db(apart / peak) < -90.0, "{}", db(apart / peak));
}

#[test]
fn sixteen_parts_sending_into_one_reverb_render_in_half_the_time_of_sixteen_reverbs() {
    let timed = |shared: bool| {
        let mut engine = engine();
        let reverb = engine.start_effects(&[effect("reverb", REVERB)]).unwrap();
        for part in 0..16 {
            let chain = if shared {
                let chain = engine.start_effects(&[]).unwrap();
                engine.send(chain, reverb, 1.0).unwrap();
                chain
            } else {
                engine.start_effects(&[effect("reverb", REVERB)]).unwrap()
            };
            let hz = 220.0 + 20.0 * f64::from(part);
            let tone = note("tone", &[("hz", hz)], Some(chain));
            engine.play(&[(0.0, tone)]).unwrap();
        }
        let start = std::time::Instant::now();
        engine.render(2.0).unwrap();
        start.elapsed()
    };
    let (shared, own) = (timed(true), timed(false));
    assert!(shared * 2 <= own, "{shared:?} {own:?}");
}

#[test]
fn a_sound_through_a_chain_that_feeds_another_comes_out_at_the_sample_it_would_alone() {
    let straight = {
        let mut engine = engine();
        engine.play(&[(0.1, note("steady", &[], None))]).unwrap();
        engine.render(0.3).unwrap().0
    };
    let through = {
        let mut engine = engine();
        let second = engine.start_effects(&[effect("level", &[])]).unwrap();
        let first = engine.start_effects(&[effect("level", &[])]).unwrap();
        engine.route(first, Some(second)).unwrap();
        engine
            .play(&[(0.1, note("steady", &[], Some(first)))])
            .unwrap();
        engine.render(0.3).unwrap().0
    };
    assert_eq!(loudest(&straight[..frame(0.1)]), 0.0);
    assert!(straight[frame(0.1) + 1] > 0.0);
    assert_eq!(straight, through);
}

#[test]
fn a_chain_that_would_feed_itself_is_refused_naming_the_chains_in_the_loop() {
    let mut engine = engine();
    let a = engine.start_part("verse", &[]).unwrap();
    let b = engine.start_effects(&[]).unwrap();
    let c = engine.start_effects(&[]).unwrap();

    let error = engine.send(a, a, 1.0).unwrap_err();
    assert!(matches!(error, Error::FeedsItself(_)));
    assert_eq!(error.to_string(), "chain verse would feed itself");
    assert_eq!(
        engine.route(b, Some(b)).unwrap_err().to_string(),
        format!("chain {b} would feed itself")
    );

    engine.route(a, Some(b)).unwrap();
    assert_eq!(
        engine.send(b, a, 0.5).unwrap_err().to_string(),
        format!("chain {b} would feed itself, through verse")
    );
    engine.send(b, c, 1.0).unwrap();
    assert_eq!(
        engine.route(c, Some(a)).unwrap_err().to_string(),
        format!("chain {c} would feed itself, through verse and {b}")
    );
    assert_eq!(
        engine.key(a, Some(c)).unwrap_err().to_string(),
        format!("chain {c} would feed itself, through verse and {b}")
    );
    assert!(matches!(
        engine.send(a, 999, 1.0),
        Err(Error::UnknownChain(999))
    ));
    assert!(matches!(
        engine.route(999, None),
        Err(Error::UnknownChain(999))
    ));

    // What was refused changed nothing: a sound into the part still comes out once.
    engine.play(&[(0.0, note("steady", &[], Some(a)))]).unwrap();
    let (left, _) = engine.render(0.2).unwrap();
    let alone = {
        let mut engine = self::engine();
        engine.play(&[(0.0, note("steady", &[], None))]).unwrap();
        engine.render(0.2).unwrap().0
    };
    // Through b it goes on to the main output and, by b's send, into c: twice.
    assert!((left[frame(0.1)] - 2.0 * alone[frame(0.1)]).abs() < 1e-6);
}

#[test]
fn a_sends_level_glides_while_the_sound_plays_and_leaves_the_dry_sound_as_it_was() {
    let mut engine = engine();
    let wet = engine.start_effects(&[effect("level", &[])]).unwrap();
    let part = engine.start_effects(&[]).unwrap();
    engine.send(part, wet, 1.0).unwrap();
    let down = Event::Set {
        target: Target::Send(part, wet),
        options: vec![("level".to_string(), 0.0), ("level_slide".to_string(), 0.1)],
    };
    let steady = note("steady", &[], Some(part));
    engine.play(&[(0.0, steady), (1.0, down)]).unwrap();
    let (left, _) = engine.render(1.5).unwrap();
    let dry = left[frame(1.4)];
    assert!(dry > 0.1);
    assert!((left[frame(0.9)] - 2.0 * dry).abs() < 1e-6);
    let halfway = left[frame(1.05)] / dry;
    assert!(halfway > 1.2 && halfway < 1.8, "{halfway}");
    assert!((left[frame(1.11)] - dry).abs() < dry * 0.01);
    assert!(
        left[frame(1.0)..frame(1.2)]
            .windows(2)
            .all(|w| w[1] <= w[0] + 1e-6)
    );

    assert!(matches!(
        engine.play(&[(0.0, Event::Set {
            target: Target::Send(wet, part),
            options: vec![("level".to_string(), 0.0)],
        })]),
        Err(Error::UnknownSend(from, to)) if from == wet && to == part
    ));
}

#[test]
fn a_shared_chain_stopped_plays_until_its_last_feeder_has_ended_and_its_tail_has_rung_out() {
    let tail = engine()
        .effect_tails(&[effect("reverb", REVERB)], None)
        .unwrap()[0];
    let rendered = |stop_part: bool| {
        let mut engine = engine();
        let reverb = engine.start_effects(&[effect("reverb", REVERB)]).unwrap();
        let part = engine.start_effects(&[]).unwrap();
        engine.route(part, Some(reverb)).unwrap();
        let tone = note("tone", &[("length", 1.9)], Some(part));
        engine.play(&[(0.0, tone)]).unwrap();
        engine.stop_effects(reverb, 0.5).unwrap();
        if stop_part {
            engine.stop_effects(part, 2.0).unwrap();
        }
        engine.render(2.5 + tail).unwrap().0
    };
    let stopped = rendered(true);
    // Stopped at half a second, the reverb still plays its feeder to the end, and rings on after it.
    assert!(rms(&stopped[frame(1.5)..frame(1.9)]) > 0.01);
    assert!(rms(&stopped[frame(2.05)..frame(2.3)]) > 0.0005);
    assert!(loudest(&stopped[frame(1.9 + tail)..frame(1.95 + tail)]) > 0.0);
    // Its tail over, it has ended, and plays nothing more, not the last of its ringing.
    assert_eq!(loudest(&stopped[frame(2.15 + tail)..]), 0.0);
    // While its feeder is not stopped it plays on.
    let kept = rendered(false);
    assert!(loudest(&kept[frame(2.15 + tail)..]) > 0.0);
}

/// A pad under the compressor's threshold and a kick every half second in a chain of its own, as the left channel of 2.5 seconds; the pad's compressor is keyed by the kick's chain when `keyed`, and `pad` is the pad's level.
fn pumped(keyed: bool, pad: f64, kick: bool) -> Vec<f32> {
    let mut engine = engine();
    let kicks = engine.start_effects(&[]).unwrap();
    let compressor = effect(
        "compressor",
        &[
            ("threshold", 0.2),
            ("slope_above", 0.25),
            ("relax_ms", 50.0),
        ],
    );
    let pads = engine.start_effects(&[compressor]).unwrap();
    if keyed {
        engine.key(pads, Some(kicks)).unwrap();
    }
    let mut events = vec![(
        0.0,
        note(
            "tone",
            &[("amp", pad), ("hz", 1000.0), ("length", 2.5)],
            Some(pads),
        ),
    )];
    for beat in 0..4 {
        let thump = note(
            "tone",
            &[("amp", 3.0), ("hz", 100.0), ("length", 0.1)],
            Some(kicks),
        );
        if kick {
            events.push((0.25 + 0.5 * f64::from(beat), thump));
        }
    }
    engine.play(&events).unwrap();
    engine.render(2.5).unwrap().0
}

#[test]
fn a_compressor_keyed_by_another_chain_lowers_its_sound_by_that_chains_level() {
    let kicks = pumped(false, 0.0, true);
    assert!(loudest(&kicks) >= 1.0);
    let dry = pumped(false, 0.3, false);
    let level = |samples: &[f32], from: f64| rms(&samples[frame(from)..frame(from + 0.01)]);
    let quiet = level(&dry, 0.2);
    assert!(quiet > 0.05);

    // Unkeyed, the pad stays under the threshold and the kicks do not move it.
    let unkeyed: Vec<f32> = pumped(false, 0.3, true)
        .iter()
        .zip(&kicks)
        .map(|(both, kick)| both - kick)
        .collect();
    assert!(db(level(&unkeyed, 0.27) / quiet).abs() < 0.1);

    let keyed: Vec<f32> = pumped(true, 0.3, true)
        .iter()
        .zip(&kicks)
        .map(|(both, kick)| both - kick)
        .collect();
    for beat in 0..4 {
        let at = 0.25 + 0.5 * f64::from(beat);
        assert!(db(level(&keyed, at - 0.02) / quiet).abs() < 1.0);
        let down = db(level(&keyed, at + 0.01) / quiet);
        assert!(down <= -6.0, "{down}");
        let back = db(level(&keyed, at + 0.1 + 0.2) / quiet);
        assert!(back.abs() < 1.0, "{back}");
    }

    // The chain that sets the compressor off sounds the same with it and without.
    assert_eq!(pumped(true, 0.0, true), kicks);

    // A key taken away leaves the compressor to its own input again.
    let mut engine = engine();
    let pads = engine.start_effects(&[effect("compressor", &[])]).unwrap();
    let other = engine.start_effects(&[]).unwrap();
    engine.key(pads, Some(other)).unwrap();
    engine.key(pads, None).unwrap();
    engine
        .play(&[(0.0, note("steady", &[], Some(pads)))])
        .unwrap();
    let (left, _) = engine.render(0.5).unwrap();
    let mut plain = self::engine();
    let pads = plain.start_effects(&[effect("compressor", &[])]).unwrap();
    plain
        .play(&[(0.0, note("steady", &[], Some(pads)))])
        .unwrap();
    assert_eq!(left, plain.render(0.5).unwrap().0);
}

#[test]
fn a_soloed_part_is_still_heard_through_the_chains_it_feeds() {
    let rendered = |solo: bool, feed: bool| {
        let mut engine = engine();
        let wet = engine.start_effects(&[effect("level", &[])]).unwrap();
        let keys = engine.start_part("keys", &[]).unwrap();
        let bass = engine.start_part("bass", &[]).unwrap();
        if feed {
            engine.send(keys, wet, 1.0).unwrap();
            engine.send(bass, wet, 1.0).unwrap();
        }
        if solo {
            engine.solo("keys", true, 0.0).unwrap();
        }
        let tones = [
            (0.0, note("tone", &[("hz", 440.0)], Some(keys))),
            (0.0, note("tone", &[("hz", 660.0)], Some(bass))),
        ];
        engine.play(&tones).unwrap();
        engine.render(0.5).unwrap().0
    };
    let keys_alone = rendered(true, false);
    let soloed = rendered(true, true);
    // The keys come out dry and through the chain they send into; the bass, not soloed, through neither.
    for i in frame(0.1)..frame(0.4) {
        assert!((soloed[i] - 2.0 * keys_alone[i]).abs() < 1e-6);
    }
    assert!(loudest(&rendered(false, true)) > loudest(&soloed));
}
