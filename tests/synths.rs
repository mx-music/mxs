use mxs::{Engine, Event, RATE, Target, synths};

#[test]
fn every_synth_renders_a_finite_audible_note_with_its_defaults() {
    for synth in synths::all() {
        let mut engine = Engine::offline(RATE);
        synths::all().into_iter().for_each(|s| engine.register(s));
        engine
            .play(&[(0.0, Event::note(None, synth.name(), &[]))])
            .unwrap();
        let (left, right) = engine.render(1.0).unwrap();
        let name = synth.name();
        assert!(
            left.iter().chain(&right).all(|x| x.is_finite()),
            "{name} gave a sample that is not finite"
        );
        let peak = left.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 1e-4, "{name} is silent");
        assert!(peak < 4.0, "{name} peaks at {peak}");
    }
}

#[test]
fn the_tabla_ghe_glides_to_a_new_note_over_its_slide() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(Some(1), "tabla_ghe", &[]))])
        .unwrap();
    let up = Event::Set {
        target: Target::Sounds(vec![1]),
        options: vec![("note".into(), 54.25), ("note_slide".into(), 0.1)],
    };
    engine.play(&[(0.2, up)]).unwrap();
    let (left, _) = engine.render(0.7).unwrap();
    let crossings = |from: f64, to: f64| {
        let part = &left[(from * RATE) as usize..(to * RATE) as usize];
        part.windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count() as f64
    };
    let ratio = crossings(0.45, 0.65) / crossings(0.0, 0.2);
    assert!(
        (ratio - 2.0).abs() < 0.15,
        "the pitch rose by a ratio of {ratio}"
    );
}

#[test]
fn a_hundred_accordion_notes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let notes: Vec<_> = (0..100)
        .map(|i| {
            (
                i as f64 * 0.02,
                Event::note(
                    None,
                    "accordion",
                    &[("note", 48.0 + (i % 27) as f64), ("sustain", 0.3)],
                ),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    engine.render(2.0).unwrap();
    assert!(engine.sounding());
    engine.render(1.0).unwrap();
    assert!(
        !engine.sounding(),
        "a note still sounds half a second after the last one's release ends"
    );
}

#[test]
fn a_hundred_surdo_strokes_open_and_muted_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let strokes: Vec<_> = (0..100)
        .map(|i| {
            (
                i as f64 * 0.02,
                Event::note(None, "surdo", &[("muted", (i % 2) as f64)]),
            )
        })
        .collect();
    engine.play(&strokes).unwrap();
    engine.render(3.0).unwrap();
    assert!(engine.sounding());
    engine.render(3.0).unwrap();
    assert!(
        !engine.sounding(),
        "a stroke still sounds after the last one's ring ends"
    );
}

#[test]
fn a_hundred_cuica_strokes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let strokes: Vec<_> = (0..100)
        .map(|i| {
            (
                i as f64 * 0.02,
                Event::note(None, "cuica", &[("vel", 0.3 + 0.7 * (i % 3) as f64 / 2.0)]),
            )
        })
        .collect();
    engine.play(&strokes).unwrap();
    engine.render(2.0).unwrap();
    assert!(engine.sounding());
    engine.render(1.0).unwrap();
    assert!(
        !engine.sounding(),
        "a stroke still sounds after the last one's arch ends"
    );
}

#[test]
fn a_hundred_tamborim_strokes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let strokes: Vec<_> = (0..100)
        .map(|i| (i as f64 * 0.02, Event::note(None, "tamborim", &[])))
        .collect();
    engine.play(&strokes).unwrap();
    engine.render(2.5).unwrap();
    assert!(engine.sounding());
    engine.render(1.5).unwrap();
    assert!(
        !engine.sounding(),
        "a stroke still sounds after the last one's ring ends"
    );
}

#[test]
fn a_hundred_caxixi_shakes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let shakes: Vec<_> = (0..100)
        .map(|i| (i as f64 * 0.02, Event::note(None, "caxixi", &[])))
        .collect();
    engine.play(&shakes).unwrap();
    engine.render(4.0).unwrap();
    assert!(engine.sounding());
    engine.render(1.5).unwrap();
    assert!(
        !engine.sounding(),
        "a shake still sounds after the last one's ring ends"
    );
}

fn electric_bass(options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(None, "electric_bass", options))])
        .unwrap();
    engine.render(seconds).unwrap().0
}

#[test]
fn the_electric_bass_plays_its_lowest_and_highest_notes_at_their_pitch() {
    for note in [22.0, 63.0] {
        let sound = electric_bass(&[("note", note)], 2.0);
        let period = RATE / (440.0 * 2f64.powf((note - 69.0) / 12.0));
        let window = &sound[4800..52_800];
        let alike = |lag: usize| -> f64 {
            window[lag..]
                .iter()
                .zip(window)
                .map(|(a, b)| (a * b) as f64)
                .sum::<f64>()
                / (window.len() - lag) as f64
        };
        let lags = (period / 2.5) as usize..(period * 2.5) as usize;
        let best = lags.max_by(|a, b| alike(*a).total_cmp(&alike(*b))).unwrap();
        let (before, at, after) = (alike(best - 1), alike(best), alike(best + 1));
        let lag = best as f64 + 0.5 * (before - after) / (before - 2.0 * at + after);
        let hz = RATE / lag;
        let cents = 1200.0 * (hz / (440.0 * 2f64.powf((note - 69.0) / 12.0))).log2();
        assert!(cents.abs() < 5.0, "note {note} is {cents} cents off");
    }
}

#[test]
fn a_harder_electric_bass_note_is_louder_and_brighter() {
    let level = |sound: &[f32], hz: f64| {
        let turn = |i: usize| std::f64::consts::TAU * hz * i as f64 / RATE;
        let (re, im) = sound[..24_000]
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(re, im), (i, x)| {
                (
                    re + *x as f64 * turn(i).cos(),
                    im + *x as f64 * turn(i).sin(),
                )
            });
        re.hypot(im)
    };
    let soft = electric_bass(&[("note", 33.0), ("vel", 0.3)], 1.0);
    let hard = electric_bass(&[("note", 33.0), ("vel", 1.0)], 1.0);
    let grew = |hz: f64| 20.0 * (level(&hard, hz) / level(&soft, hz)).log10();
    let (first, eighth) = (grew(55.0), grew(440.0));
    assert!(
        first > 3.0 && first < 6.0,
        "its fundamental grew {first} dB"
    );
    assert!(eighth > first + 5.0, "its eighth harmonic grew {eighth} dB");
}

#[test]
fn a_hundred_electric_bass_notes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let notes: Vec<_> = (0..100)
        .map(|i| {
            let options = [("note", 28.0 + (i % 30) as f64), ("sustain", 0.3)];
            (
                i as f64 * 0.02,
                Event::note(None, "electric_bass", &options),
            )
        })
        .collect();
    engine.play(&notes).unwrap();
    engine.render(2.0).unwrap();
    assert!(engine.sounding());
    engine.render(1.0).unwrap();
    assert!(
        !engine.sounding(),
        "a note still sounds after its release ends"
    );
}
