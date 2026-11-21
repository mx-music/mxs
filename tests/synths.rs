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
    assert!(eighth > first + 2.0, "its eighth harmonic grew {eighth} dB");
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

fn deep_bass(options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(None, "deep_bass", options))])
        .unwrap();
    engine.render(seconds).unwrap().0
}

#[test]
fn the_deep_bass_plays_its_lowest_and_highest_notes_at_their_pitch() {
    for note in [22.0, 63.0] {
        let sound = deep_bass(&[("note", note)], 2.0);
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
fn a_harder_deep_bass_note_is_louder() {
    let rms = |sound: &[f32]| {
        (sound[..24_000]
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / 24_000.0)
            .sqrt()
    };
    let soft = deep_bass(&[("note", 33.0), ("vel", 0.3)], 1.0);
    let hard = deep_bass(&[("note", 33.0), ("vel", 1.0)], 1.0);
    let louder = 20.0 * (rms(&hard) / rms(&soft)).log10();
    assert!(louder > 3.0 && louder < 6.0, "it grew {louder} dB");
}

#[test]
fn a_hundred_deep_bass_notes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let notes: Vec<_> = (0..100)
        .map(|i| {
            let options = [("note", 28.0 + (i % 30) as f64), ("sustain", 0.3)];
            (i as f64 * 0.02, Event::note(None, "deep_bass", &options))
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

fn electric_piano(options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(None, "electric_piano", options))])
        .unwrap();
    engine.render(seconds).unwrap().0
}

#[test]
fn the_electric_piano_plays_its_lowest_and_highest_notes_at_their_pitch() {
    for note in [28.0, 88.0] {
        let sound = electric_piano(&[("note", note)], 2.0);
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
fn a_harder_electric_piano_note_is_louder_and_brighter() {
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
    let rms = |sound: &[f32]| {
        (sound[..24_000]
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / 24_000.0)
            .sqrt()
    };
    let soft = electric_piano(&[("note", 48.0), ("vel", 0.3)], 1.0);
    let hard = electric_piano(&[("note", 48.0), ("vel", 1.0)], 1.0);
    let louder = 20.0 * (rms(&hard) / rms(&soft)).log10();
    assert!(louder > 3.0 && louder < 6.0, "it grew {louder} dB");
    let tenth = |sound: &[f32]| level(sound, 1308.1) / level(sound, 130.81);
    assert!(
        tenth(&hard) > tenth(&soft),
        "its tenth harmonic stands no higher"
    );
}

#[test]
fn a_hundred_electric_piano_notes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let notes: Vec<_> = (0..100)
        .map(|i| {
            let options = [("note", 28.0 + (i % 60) as f64), ("sustain", 0.3)];
            (
                i as f64 * 0.02,
                Event::note(None, "electric_piano", &options),
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

fn electric_guitar(options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(None, "electric_guitar", options))])
        .unwrap();
    engine.render(seconds).unwrap().0
}

#[test]
fn the_electric_guitar_plays_its_lowest_and_highest_notes_at_their_pitch() {
    for note in [40.0, 86.0] {
        let sound = electric_guitar(&[("note", note)], 2.0);
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
fn a_harder_electric_guitar_note_is_louder() {
    let rms = |sound: &[f32]| {
        (sound[..24_000]
            .iter()
            .map(|x| (*x as f64).powi(2))
            .sum::<f64>()
            / 24_000.0)
            .sqrt()
    };
    for note in [40.0, 52.0, 64.0, 76.0] {
        let soft = electric_guitar(&[("note", note), ("vel", 0.3)], 1.0);
        let hard = electric_guitar(&[("note", note), ("vel", 1.0)], 1.0);
        let louder = 20.0 * (rms(&hard) / rms(&soft)).log10();
        assert!(louder > 3.0 && louder < 6.0, "note {note} grew {louder} dB");
    }
}

/// The alpha version's A2 at vel 1, in mxs 0.1.2, measured -35.4 dB in its left channel over 3.2 seconds.
#[test]
fn the_electric_guitar_plays_a2_at_vel_1_as_loud_as_its_alpha_version() {
    let sound = electric_guitar(&[("note", 45.0), ("vel", 1.0)], 3.2);
    let rms = (sound.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / sound.len() as f64).sqrt();
    let level = 20.0 * rms.log10();
    assert!((level + 35.4).abs() < 1.0, "A2 plays at {level} dB");
}

#[test]
fn a_hundred_electric_guitar_notes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let notes: Vec<_> = (0..100)
        .map(|i| {
            let options = [("note", 40.0 + (i % 46) as f64), ("sustain", 0.3)];
            (
                i as f64 * 0.02,
                Event::note(None, "electric_guitar", &options),
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

#[test]
fn a_hundred_marimba_notes_leave_nothing_sounding_once_they_end() {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let notes: Vec<_> = (0..100)
        .map(|i| {
            let options = [("note", 29.0 + (i % 67) as f64), ("sustain", 0.3)];
            (i as f64 * 0.02, Event::note(None, "marimba", &options))
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

#[test]
fn a_hundred_notes_of_each_struck_synth_played_from_figures_leave_nothing_sounding_once_they_end() {
    for (synth, lowest, span) in [
        ("harp", 36.0, 57),
        ("guitar", 35.0, 49),
        ("vibraphone", 48.0, 42),
        ("xylophone", 65.0, 44),
        ("glockenspiel", 72.0, 41),
        ("timpani", 40.0, 16),
    ] {
        let mut engine = Engine::offline(RATE);
        synths::all().into_iter().for_each(|s| engine.register(s));
        let notes: Vec<_> = (0..100)
            .map(|i| {
                let options = [
                    ("note", lowest + (i % span) as f64),
                    ("sustain", 0.3),
                    ("release", 0.2),
                ];
                (i as f64 * 0.02, Event::note(None, synth, &options))
            })
            .collect();
        engine.play(&notes).unwrap();
        engine.render(2.0).unwrap();
        assert!(engine.sounding(), "{synth} is silent");
        engine.render(1.0).unwrap();
        assert!(
            !engine.sounding(),
            "a {synth} note still sounds after its release ends"
        );
    }
}

fn cello(options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(None, "cello", options))])
        .unwrap();
    engine.render(seconds).unwrap().0
}

fn level(sound: &[f32], from: f64, to: f64) -> f64 {
    let part = &sound[(from * RATE) as usize..(to * RATE) as usize];
    let power = part.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / part.len() as f64;
    10.0 * power.max(1e-30).log10()
}

#[test]
fn a_cello_note_held_twice_as_long_as_its_recordings_holds_its_level_then_falls_once_let_go() {
    // The University of Iowa's arco notes hold for one to four seconds.
    let sound = cello(
        &[
            ("note", 48.0),
            ("vel", 0.6),
            ("sustain", 8.0),
            ("vibrato", 0.0),
        ],
        10.0,
    );
    let (early, late) = (level(&sound, 2.0, 3.0), level(&sound, 7.0, 8.0));
    assert!(
        (early - late).abs() < 1.0,
        "{early} dB early, {late} dB late"
    );
    let after = level(&sound, 9.5, 10.0);
    assert!(late - after > 20.0, "{late} dB held, {after} dB after");
}

#[test]
fn a_legato_cello_note_speaks_at_once() {
    let options = |legato: f64| [("note", 48.0), ("vel", 0.6), ("legato", legato)];
    let detached = level(&cello(&options(0.0), 1.0), 0.0, 0.03);
    let slurred = level(&cello(&options(1.0), 1.0), 0.0, 0.03);
    assert!(
        slurred - detached > 6.0,
        "{slurred} dB slurred, {detached} dB detached"
    );
}

#[test]
fn a_plucked_cello_note_rings_and_ends_by_itself() {
    let sound = cello(
        &[
            ("note", 48.0),
            ("vel", 0.6),
            ("pizz", 1.0),
            ("sustain", 30.0),
        ],
        12.0,
    );
    assert!(level(&sound, 0.0, 0.5) > -40.0);
    assert!(level(&sound, 11.0, 12.0) < -100.0);
}

#[test]
fn a_held_cello_note_fades_out_rather_than_being_cut_when_it_ends() {
    for (synth, note) in [("cello", 36.0), ("cello", 57.0)] {
        for vel in [0.3, 1.0] {
            let mut engine = Engine::offline(RATE);
            synths::all().into_iter().for_each(|s| engine.register(s));
            let options = [
                ("note", note),
                ("vel", vel),
                ("sustain", 1.0),
                ("vibrato", 0.0),
            ];
            engine
                .play(&[(0.0, Event::note(None, synth, &options))])
                .unwrap();
            let sound = engine.render(8.0).unwrap().0;
            let last = sound.iter().rposition(|x| *x != 0.0).unwrap() as f64 / RATE;
            let held = level(&sound, 0.5, 1.0);
            let end = level(&sound, last - 0.05, last);
            assert!(
                held - end > 60.0,
                "{synth} {note} at vel {vel}: {held} dB held, {end} dB as it ends at {last} s"
            );
        }
    }
}

#[test]
fn the_reed_and_brass_winds_take_their_bores_least_corner_by_name_and_refuse_cutoff() {
    for synth in [
        "clarinet",
        "oboe",
        "bassoon",
        "tenor_sax",
        "trumpet",
        "trombone",
        "horn",
        "tuba",
    ] {
        let render = |options: &[(&str, f64)]| {
            let mut engine = Engine::offline(RATE);
            synths::all().into_iter().for_each(|s| engine.register(s));
            engine.play(&[(0.0, Event::note(None, synth, options))])?;
            Ok::<_, mxs::Error>(engine.render(1.0).unwrap().0)
        };
        let fitted = render(&[]).unwrap();
        let given = render(&[("least_corner", 5000.0)]).unwrap();
        assert!(fitted != given, "{synth} ignores least_corner");
        let refused = render(&[("cutoff", 5000.0)]).unwrap_err();
        assert_eq!(refused.to_string(), format!("{synth} has no option cutoff"));
    }
}
