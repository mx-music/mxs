//! Checks of held notes and of the expressions every note takes, on the synths that follow them.

use mxs::{Engine, Event, RATE, Target, synths};
use std::f64::consts::PI;

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
}

fn rms(samples: &[f32]) -> f64 {
    (samples.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / samples.len() as f64).sqrt()
}

/// The left channel's level over the last `seconds` of `samples`.
fn last(samples: &[f32], seconds: f64) -> f64 {
    rms(&samples[samples.len() - (seconds * RATE) as usize..])
}

fn peak(samples: &[f32]) -> f64 {
    samples.iter().fold(0.0, |most, s| most.max(s.abs() as f64))
}

/// The options of a middle C, with vel on a synth that takes it.
fn middle_c(synth: &str) -> Vec<(&'static str, f64)> {
    match synth {
        "saw" => vec![("note", 60.0)],
        _ => vec![("note", 60.0), ("vel", 0.6)],
    }
}

#[test]
fn a_held_note_of_the_piano_violin_and_saw_sounds_until_it_is_let_go_and_then_falls_silent() {
    for synth in ["piano", "violin", "saw"] {
        let mut engine = engine();
        let held = Event::held(1, synth, &middle_c(synth));
        engine.play(&[(0.0, held)]).unwrap();
        let (before, _) = engine.render(5.0).unwrap();
        // The piano's strings fade by themselves, to about 47 dB under its peak by then.
        let under = |level: f64| 20.0 * (level / peak(&before)).log10();
        let sounding = under(last(&before, 0.1));
        assert!(
            sounding > -60.0,
            "{synth} {sounding} dB just before it is let go"
        );
        engine
            .play(&[(5.0, Event::Release { ids: vec![1] })])
            .unwrap();
        // The violin falls silent over 5 seconds, as a plain note of it does after its sustain.
        let (after, _) = engine.render(7.0).unwrap();
        let falling = under(rms(&after[..(0.02 * RATE) as usize]));
        assert!(falling > -60.0, "{synth} {falling} dB as it is let go");
        assert_eq!(last(&after, 0.5), 0.0, "{synth} after its release");
    }
}

#[test]
fn a_plain_saw_note_given_no_sustain_is_over_in_a_second() {
    let mut engine = engine();
    let plain = Event::note(Some(1), "saw", &middle_c("saw"));
    engine.play(&[(0.0, plain)]).unwrap();
    let (sound, _) = engine.render(5.0).unwrap();
    assert_eq!(last(&sound, 3.5), 0.0);
}

/// The window's spectrum at `freq`, through a Hann window.
fn magnitude(samples: &[f32], freq: f64) -> f64 {
    let n = samples.len() as f64;
    let (mut re, mut im) = (0.0, 0.0);
    for (i, s) in samples.iter().enumerate() {
        let w = 0.5 - 0.5 * (2.0 * PI * i as f64 / n).cos();
        let phase = 2.0 * PI * freq * i as f64 / RATE;
        re += w * *s as f64 * phase.cos();
        im -= w * *s as f64 * phase.sin();
    }
    (re * re + im * im).sqrt()
}

/// The frequency of the strongest peak between `low` and `high`, to a tenth of a hertz.
fn peak_between(samples: &[f32], low: f64, high: f64) -> f64 {
    let steps = ((high - low) * 10.0) as usize;
    (0..=steps)
        .map(|k| low + k as f64 / 10.0)
        .max_by(|a, b| magnitude(samples, *a).total_cmp(&magnitude(samples, *b)))
        .unwrap()
}

/// The share of the window's energy above `freq`, from its spectrum up to 20 kHz at each step the window resolves, every 20 Hz for 50 ms.
fn share_above(samples: &[f32], freq: f64) -> f64 {
    let power = |f: f64| magnitude(samples, f).powi(2);
    let step = RATE / samples.len() as f64;
    let bins: Vec<f64> = (1..(20_000.0 / step) as usize)
        .map(|k| k as f64 * step)
        .collect();
    let above: f64 = bins.iter().filter(|f| **f > freq).map(|f| power(*f)).sum();
    above / bins.iter().map(|f| power(*f)).sum::<f64>()
}

/// The share above the fourth harmonic of a window whose pitch is `pitch`.
fn above_the_fourth(samples: &[f32], pitch: f64) -> f64 {
    share_above(samples, 4.5 * pitch)
}

/// The window's energy above 1.5 times `pitch` over its energy from half to 1.5 times it, at each step the window resolves.
/// A bar's overtones stand near 3, 4 and 10 times its pitch and soon fade, and a bass's carry most of its energy, so they are weighed against the fundamental alone.
fn overtones_over_fundamental(samples: &[f32], pitch: f64) -> f64 {
    let step = RATE / samples.len() as f64;
    let energy = |low: f64, high: f64| {
        let first = (low / step).ceil() as usize;
        (first..(high / step) as usize)
            .map(|k| magnitude(samples, k as f64 * step).powi(2))
            .sum::<f64>()
    };
    energy(1.5 * pitch, 20_000.0) / energy(0.5 * pitch, 1.5 * pitch)
}

#[test]
fn a_bend_of_two_semitones_and_a_brighter_sound_reach_a_held_piano_violin_and_saw_note_within_10_ms()
 {
    let middle = 261.63;
    for synth in ["piano", "violin", "saw"] {
        let mut engine = engine();
        let mut options = middle_c(synth);
        match synth {
            "violin" => options.push(("vibrato", 0.0)),
            // Its default cutoff, 2.6 kHz, already passes most of what lies above its fourth harmonic.
            "saw" => options.push(("cutoff", 70.0)),
            _ => {}
        }
        engine
            .play(&[(0.0, Event::held(1, synth, &options))])
            .unwrap();
        let change = Event::Set {
            target: Target::Sounds(vec![1]),
            options: vec![("bend".into(), 2.0), ("brightness".into(), 1.0)],
        };
        engine.play(&[(1.0, change)]).unwrap();
        let (sound, _) = engine.render(1.5).unwrap();
        let window = |from: f64| &sound[(from * RATE) as usize..((from + 0.05) * RATE) as usize];
        let (before, after) = (window(0.95), window(1.01));
        let was = peak_between(before, 0.9 * middle, 1.05 * middle);
        let is = peak_between(after, 1.05 * middle, 1.2 * middle);
        let moved = 12.0 * (is / was).log2();
        assert!(
            (moved - 2.0).abs() < 0.1,
            "{synth} moved {moved} semitones, {was} to {is} Hz"
        );
        let (dull, bright) = (share_above(before, 4.5 * was), share_above(after, 4.5 * is));
        println!(
            "{synth}: {was:.1} to {is:.1} Hz, {moved:.3} semitones; share above the fourth harmonic {dull:.4} to {bright:.4}"
        );
        assert!(
            bright > 1.2 * dull,
            "{synth}'s share above its fourth harmonic went from {dull} to {bright}"
        );
    }
}

#[test]
fn samples_and_the_sound_cards_input_take_none_of_the_expressions_notes_take() {
    let engine = Engine::offline(RATE);
    let names = |specs: Vec<mxs::Spec>| specs.into_iter().map(|s| s.name).collect::<Vec<_>>();
    assert_eq!(
        names(engine.note_specs()),
        [
            "amp",
            "pan",
            "bend",
            "brightness",
            "pressure",
            "vibrato",
            "pedal"
        ]
    );
    assert_eq!(names(engine.input_specs()), ["amp", "pan"]);
    let sample = names(engine.sample_specs());
    assert!(sample.contains(&"amp".to_string()));
    assert!(!sample.contains(&"bend".to_string()), "{sample:?}");
    assert!(!sample.contains(&"vibrato".to_string()), "{sample:?}");
}

#[test]
fn letting_go_of_a_kick_berimbau_or_crash_leaves_it_ringing_while_stopping_one_cuts_it() {
    for synth in ["kick", "berimbau", "crash"] {
        let render = |end: Option<Event>| {
            let mut engine = engine();
            let note = Event::note(Some(1), synth, &[("note", 60.0)]);
            engine.play(&[(0.0, note)]).unwrap();
            if let Some(end) = end {
                engine.play(&[(0.2, end)]).unwrap();
            }
            engine.render(2.0).unwrap().0
        };
        let left_alone = render(None);
        assert!(
            render(Some(Event::Release { ids: vec![1] })) == left_alone,
            "{synth} let go plays otherwise than left alone"
        );
        let stopped = render(Some(Event::Stop { ids: vec![1] }));
        let after = (0.21 * RATE) as usize;
        assert!(
            stopped[after..].iter().all(|s| *s == 0.0),
            "{synth} stopped"
        );
    }
}

#[test]
fn letting_go_of_a_marimba_guitar_or_piano_fades_it_over_its_release() {
    for synth in ["marimba", "guitar", "piano"] {
        let render = |let_go: bool| {
            let mut engine = engine();
            let note = Event::note(Some(1), synth, &[("note", 60.0)]);
            engine.play(&[(0.0, note)]).unwrap();
            if let_go {
                engine
                    .play(&[(0.2, Event::Release { ids: vec![1] })])
                    .unwrap();
            }
            engine.render(0.35).unwrap().0
        };
        let (left_alone, let_go) = (render(false), render(true));
        let window = |s: &[f32]| rms(&s[(0.25 * RATE) as usize..]);
        let fading = window(&let_go);
        assert!(
            fading > 0.0 && fading < 0.9 * window(&left_alone),
            "{synth}: {fading} against {}",
            window(&left_alone)
        );
    }
}

#[test]
fn a_synth_that_had_no_vel_plays_at_half_its_level_at_vel_0_3_and_as_before_at_0_6() {
    for synth in ["saw", "organ", "clap"] {
        let render = |vel: Option<f64>| {
            let mut engine = engine();
            let mut options = vec![("note", 60.0)];
            options.extend(vel.map(|v| ("vel", v)));
            engine
                .play(&[(0.0, Event::note(None, synth, &options))])
                .unwrap();
            engine.render(0.5).unwrap().0
        };
        let (plain, given, soft) = (render(None), render(Some(0.6)), render(Some(0.3)));
        assert!(
            plain == given,
            "{synth} at vel 0.6 plays otherwise than given none"
        );
        assert!(plain.iter().any(|s| s.abs() > 0.01));
        for (soft, plain) in soft.iter().zip(&plain) {
            assert!(
                (soft - 0.5 * plain).abs() <= 1e-6 * plain.abs(),
                "{synth}: {soft} against {plain}"
            );
        }
    }
}

/// The pitch a held note of `synth` measures before and 10 ms after `change` reaches it at `at` seconds, and how bright `measure` finds it then.
fn changed(
    synth: &str,
    options: &[(&str, f64)],
    change: (&str, f64),
    at: f64,
    measure: fn(&[f32], f64) -> f64,
) -> ((f64, f64), (f64, f64)) {
    changed_after(synth, options, change, at, 0.01, measure)
}

/// What [`changed`] measures, `delay` seconds after the change rather than 10 ms.
/// Each window lasts 50 ms, or 12 periods of a low note, so that its pitch is resolved to a tenth of a semitone.
fn changed_after(
    synth: &str,
    options: &[(&str, f64)],
    change: (&str, f64),
    at: f64,
    delay: f64,
    measure: fn(&[f32], f64) -> f64,
) -> ((f64, f64), (f64, f64)) {
    let note = options.iter().find(|(name, _)| *name == "note").unwrap().1;
    let middle = 440.0 * 2f64.powf((note - 69.0) / 12.0);
    let mut engine = engine();
    engine
        .play(&[(0.0, Event::held(1, synth, options))])
        .unwrap();
    let set = Event::Set {
        target: Target::Sounds(vec![1]),
        options: vec![(change.0.into(), change.1)],
    };
    engine.play(&[(at, set)]).unwrap();
    let (sound, _) = engine.render(at + 0.5).unwrap();
    let length = f64::max(0.05, 12.0 / middle);
    let window = |from: f64| &sound[(from * RATE) as usize..((from + length) * RATE) as usize];
    let (before, after) = (window(at - length), window(at + delay));
    let was = peak_between(before, 0.9 * middle, 1.05 * middle);
    let (low, high) = if change.0 == "bend" {
        (1.05, 1.2)
    } else {
        (0.9, 1.05)
    };
    let is = peak_between(after, low * middle, high * middle);
    let shares = (measure(before, was), measure(after, is));
    ((was, is), shares)
}

#[test]
fn a_bend_of_two_semitones_reaches_a_held_note_of_each_basic_wave_within_10_ms() {
    for synth in ["square", "tri", "pulse", "supersaw", "acid", "fm"] {
        let ((was, is), _) = changed(
            synth,
            &[("note", 60.0)],
            ("bend", 2.0),
            1.0,
            above_the_fourth,
        );
        let moved = 12.0 * (is / was).log2();
        assert!(
            (moved - 2.0).abs() < 0.1,
            "{synth} moved {moved} semitones, {was} to {is} Hz"
        );
    }
}

#[test]
fn brightness_1_brightens_a_held_note_of_each_basic_wave_under_a_low_pass_within_10_ms() {
    for synth in ["square", "tri", "pulse", "supersaw", "acid"] {
        let (_, (dull, bright)) = changed(
            synth,
            &[("note", 60.0), ("cutoff", 70.0)],
            ("brightness", 1.0),
            1.0,
            above_the_fourth,
        );
        println!("{synth}: share above the fourth harmonic {dull:.4} to {bright:.4}");
        assert!(
            bright > 1.2 * dull,
            "{synth}'s share above its fourth harmonic went from {dull} to {bright}"
        );
    }
}

/// The struck and plucked synths fitted to recordings besides the piano, each with a note in its range.
const FITTED: [(&str, f64); 11] = [
    ("guitar", 52.0),
    ("electric_bass", 33.0),
    ("deep_bass", 33.0),
    ("harp", 60.0),
    ("electric_piano", 60.0),
    ("electric_guitar", 52.0),
    ("marimba", 60.0),
    ("vibraphone", 60.0),
    ("xylophone", 72.0),
    ("glockenspiel", 84.0),
    ("timpani", 48.0),
];

#[test]
fn a_bend_of_two_semitones_reaches_a_held_note_of_each_fitted_struck_or_plucked_synth_within_10_ms()
{
    for (synth, note) in FITTED {
        let options = [("note", note), ("vel", 0.6)];
        let ((was, is), _) = changed(synth, &options, ("bend", 2.0), 0.3, above_the_fourth);
        let moved = 12.0 * (is / was).log2();
        println!("{synth}: {was:.1} to {is:.1} Hz, {moved:.3} semitones");
        assert!(
            (moved - 2.0).abs() < 0.1,
            "{synth} moved {moved} semitones, {was} to {is} Hz"
        );
    }
    let motor = [("note", 60.0), ("vel", 0.6), ("motor", 5.0)];
    let ((was, is), _) = changed("vibraphone", &motor, ("bend", 2.0), 0.3, above_the_fourth);
    let moved = 12.0 * (is / was).log2();
    assert!(
        (moved - 2.0).abs() < 0.1,
        "the vibraphone with its motor on moved {moved} semitones"
    );
}

#[test]
fn brightness_1_raises_the_overtones_of_a_held_note_of_each_fitted_struck_or_plucked_synth_within_10_ms()
 {
    for (synth, note) in FITTED {
        let options = [("note", note), ("vel", 0.6)];
        // These notes fade, their overtones soonest, so the window after brightness 1 is weighed against the same window after brightness 0.5.
        let after = |brightness: f64| {
            let change = ("brightness", brightness);
            changed(synth, &options, change, 0.3, overtones_over_fundamental)
                .1
                .1
        };
        let (dull, bright) = (after(0.5), after(1.0));
        println!("{synth}: overtones over the fundamental {dull:.3e} at 0.5, {bright:.3e} at 1");
        assert!(
            bright > 1.2 * dull,
            "{synth}'s overtones over its fundamental went from {dull} to {bright}"
        );
    }
}

#[test]
fn a_bend_and_brightness_reach_a_held_viola_and_double_bass_note_within_10_ms() {
    for (synth, note) in [("viola", 60.0), ("double_bass", 40.0)] {
        let options = [("note", note), ("vel", 0.6), ("vibrato", 0.0)];
        let ((was, is), _) = changed(synth, &options, ("bend", 2.0), 1.0, above_the_fourth);
        let moved = 12.0 * (is / was).log2();
        let after = |brightness: f64| {
            let change = ("brightness", brightness);
            changed(synth, &options, change, 1.0, above_the_fourth).1.1
        };
        let (dull, bright) = (after(0.5), after(1.0));
        println!(
            "{synth}: {was:.1} to {is:.1} Hz, {moved:.3} semitones; share above the fourth harmonic {dull:.4} at 0.5, {bright:.4} at 1"
        );
        assert!(
            (moved - 2.0).abs() < 0.1,
            "{synth} moved {moved} semitones, {was} to {is} Hz"
        );
        assert!(
            bright > 1.2 * dull,
            "{synth}'s share above its fourth harmonic went from {dull} to {bright}"
        );
    }
}

#[test]
fn a_bend_reaches_a_held_note_of_each_wind_once_its_bore_settles_and_brightness_within_10_ms() {
    let winds = [
        ("flute", 72.0),
        ("flute_figured", 72.0),
        ("clarinet", 60.0),
        ("oboe", 67.0),
        ("bassoon", 48.0),
        ("tenor_sax", 55.0),
        ("trumpet", 64.0),
        ("trombone", 50.0),
        ("horn", 55.0),
        ("tuba", 36.0),
    ];
    for (synth, note) in winds {
        let options = [("note", note), ("vel", 0.6), ("vibrato", 0.0)];
        // The wave already in a reed's or a brass's bore takes a few round trips to settle on a new pitch.
        let delay = if synth.starts_with("flute") {
            0.01
        } else {
            0.1
        };
        let bend = ("bend", 2.0);
        let ((was, is), _) = changed_after(synth, &options, bend, 1.0, delay, above_the_fourth);
        let moved = 12.0 * (is / was).log2();
        let after = |brightness: f64| {
            let change = ("brightness", brightness);
            changed(synth, &options, change, 1.0, above_the_fourth).1.1
        };
        let (dull, bright) = (after(0.5), after(1.0));
        println!(
            "{synth}: {was:.1} to {is:.1} Hz, {moved:.3} semitones; share above the fourth harmonic {dull:.4} at 0.5, {bright:.4} at 1"
        );
        assert!(
            (moved - 2.0).abs() < 0.1,
            "{synth} moved {moved} semitones, {was} to {is} Hz"
        );
        assert!(
            bright > 1.2 * dull,
            "{synth}'s share above its fourth harmonic went from {dull} to {bright}"
        );
    }
}

#[test]
fn a_bend_and_brightness_reach_a_held_note_of_the_brass_sitar_organs_and_the_tabla_ghe_within_10_ms()
 {
    // The struck ones fade, so they are changed sooner.
    let synths = [
        ("brass", 60.0, 1.0, true),
        ("sitar", 60.0, 0.3, true),
        ("organ", 60.0, 1.0, true),
        ("harmonium", 60.0, 1.0, true),
        ("accordion", 60.0, 1.0, true),
        ("tabla_ghe", 45.0, 0.1, false),
    ];
    for (synth, note, at, brightens) in synths {
        let options = [("note", note)];
        let ((was, is), _) = changed(synth, &options, ("bend", 2.0), at, above_the_fourth);
        let moved = 12.0 * (is / was).log2();
        println!("{synth}: {was:.1} to {is:.1} Hz, {moved:.3} semitones");
        assert!(
            (moved - 2.0).abs() < 0.1,
            "{synth} moved {moved} semitones, {was} to {is} Hz"
        );
        if brightens {
            let after = |brightness: f64| {
                let change = ("brightness", brightness);
                changed(synth, &options, change, at, above_the_fourth).1.1
            };
            let (dull, bright) = (after(0.5), after(1.0));
            println!("{synth}: share above the fourth harmonic {dull:.4} at 0.5, {bright:.4} at 1");
            assert!(
                bright > 1.2 * dull,
                "{synth}'s share above its fourth harmonic went from {dull} to {bright}"
            );
        }
    }
}

/// The left channel of a held note of `synth` given `options`, with `change` handed over at 1 second when given, over 1.5 seconds.
fn held_render(synth: &str, options: &[(&str, f64)], change: Option<(&str, f64)>) -> Vec<f32> {
    held_for(1.5, synth, options, change)
}

#[test]
fn a_held_violin_note_at_vel_0_4_given_pressure_1_plays_as_one_at_vel_1() {
    let options = |vel: f64, pressure: f64| {
        [
            ("note", 60.0),
            ("vel", vel),
            ("vibrato", 0.0),
            ("pressure", pressure),
        ]
    };
    let pressed = held_render("violin", &options(0.4, 1.0), None);
    let loud = held_render("violin", &options(1.0, 0.0), None);
    let soft = held_render("violin", &options(0.4, 0.0), None);
    assert!(pressed == loud);
    assert!(pressed != soft);
}

#[test]
fn a_held_clarinet_note_given_pressure_1_after_a_second_plays_as_one_given_vel_1_then() {
    let options = [("note", 60.0), ("vel", 0.4)];
    let pressed = held_render("clarinet", &options, Some(("pressure", 1.0)));
    let firmer = held_render("clarinet", &options, Some(("vel", 1.0)));
    let soft = held_render("clarinet", &options, None);
    let window = |s: &[f32]| s[(1.01 * RATE) as usize..(1.06 * RATE) as usize].to_vec();
    let level = |s: &[f32]| 20.0 * rms(&window(s)).log10();
    println!(
        "pressed {:.3} dB, vel 1 {:.3} dB, vel 0.4 {:.3} dB",
        level(&pressed),
        level(&firmer),
        level(&soft)
    );
    assert!((level(&pressed) - level(&firmer)).abs() < 0.1);
    assert!(level(&firmer) - level(&soft) > 1.0);
}

#[test]
fn a_held_violin_note_at_vel_0_4_given_pressure_1_after_a_second_plays_from_10_ms_later_as_one_at_vel_1()
 {
    let options = |vel: f64| [("note", 60.0), ("vel", vel), ("vibrato", 0.0)];
    let pressed = held_render("violin", &options(0.4), Some(("pressure", 1.0)));
    let loud = held_render("violin", &options(1.0), None);
    let soft = held_render("violin", &options(0.4), None);
    let window = |s: &[f32]| s[(1.01 * RATE) as usize..(1.06 * RATE) as usize].to_vec();
    let level = |s: &[f32]| 20.0 * rms(&window(s)).log10();
    let bright = |s: &[f32]| above_the_fourth(&window(s), 261.63);
    println!(
        "pressed {:.3} dB {:.4}, vel 1 {:.3} dB {:.4}, vel 0.4 {:.3} dB {:.4}",
        level(&pressed),
        bright(&pressed),
        level(&loud),
        bright(&loud),
        level(&soft),
        bright(&soft)
    );
    assert!((level(&pressed) - level(&loud)).abs() < 0.2);
    assert!((bright(&pressed) / bright(&loud) - 1.0).abs() < 0.1);
    assert!(level(&loud) - level(&soft) > 1.0);
}

#[test]
fn a_held_clarinet_note_at_vel_0_4_given_pressure_0_5_plays_as_one_at_vel_0_7() {
    let pressed = held_render(
        "clarinet",
        &[("note", 60.0), ("vel", 0.4), ("pressure", 0.5)],
        None,
    );
    let firmer = held_render("clarinet", &[("note", 60.0), ("vel", 0.7)], None);
    let soft = held_render("clarinet", &[("note", 60.0), ("vel", 0.4)], None);
    let level = |s: &[f32]| 20.0 * rms(&s[(0.5 * RATE) as usize..]).log10();
    println!(
        "pressed {:.3} dB, vel 0.7 {:.3} dB, vel 0.4 {:.3} dB",
        level(&pressed),
        level(&firmer),
        level(&soft)
    );
    assert!((level(&pressed) - level(&firmer)).abs() < 0.1);
    assert!(level(&firmer) - level(&soft) > 1.0);
}

#[test]
fn a_note_of_each_synth_that_follows_pressure_given_pressure_0_plays_as_one_given_none() {
    for synth in [
        "violin",
        "cello",
        "flute",
        "flute_figured",
        "clarinet",
        "oboe",
        "bassoon",
        "tenor_sax",
        "trumpet",
        "trombone",
        "horn",
        "tuba",
    ] {
        let options = [("note", 60.0), ("vel", 0.4)];
        let given = held_render(
            synth,
            &[("note", 60.0), ("vel", 0.4), ("pressure", 0.0)],
            None,
        );
        assert!(given.iter().any(|s| s.abs() > 0.001), "{synth} is silent");
        assert!(given == held_render(synth, &options, None), "{synth}");
    }
}

#[test]
fn a_piano_note_given_pressure_1_plays_as_one_given_none() {
    let pressed = held_render("piano", &[("note", 60.0), ("pressure", 1.0)], None);
    assert!(pressed == held_render("piano", &[("note", 60.0)], None));
}

#[test]
fn a_saw_marimba_and_kick_note_given_vibrato_0_5_and_pedal_1_plays_as_one_given_neither() {
    for synth in ["saw", "marimba", "kick"] {
        let given = held_render(synth, &[("vibrato", 0.5), ("pedal", 1.0)], None);
        assert!(given.iter().any(|s| s.abs() > 0.01), "{synth} is silent");
        assert!(given == held_render(synth, &[], None), "{synth}");
    }
}

/// The pitch at each period, in semitones from `pitch`, between rising zero crossings of the sound band-passed around it.
fn pitch_track(samples: &[f32], pitch: f64) -> Vec<f64> {
    let w = 2.0 * PI * pitch / RATE;
    let alpha = w.sin() / 6.0;
    let (b0, a0, a1, a2) = (alpha, 1.0 + alpha, -2.0 * w.cos(), 1.0 - alpha);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
    let band: Vec<f64> = samples
        .iter()
        .map(|x| {
            let x = *x as f64;
            let y = (b0 * (x - x2) - a1 * y1 - a2 * y2) / a0;
            (x2, x1, y2, y1) = (x1, x, y1, y);
            y
        })
        .collect();
    let rises: Vec<f64> = band
        .windows(2)
        .enumerate()
        .filter(|(_, pair)| pair[0] < 0.0 && pair[1] >= 0.0)
        .map(|(i, pair)| i as f64 + -pair[0] / (pair[1] - pair[0]))
        .collect();
    rises
        .windows(2)
        .map(|w| 12.0 * (RATE / (w[1] - w[0]) / pitch).log2())
        .collect()
}

/// How far a held note's pitch swings either way from 2 seconds in, in semitones: the swing of a sine with the pitch track's power between 4 and 7 Hz.
fn vibrato_depth(samples: &[f32], pitch: f64) -> f64 {
    let track = pitch_track(&samples[(2.0 * RATE) as usize..], pitch);
    let n = track.len() as f64;
    let mean = track.iter().sum::<f64>() / n;
    let step = pitch / n;
    let mut power = 0.0;
    let mut freq = (4.0 / step).ceil() * step;
    while freq <= 7.0 {
        let (re, im) = track
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(re, im), (i, x)| {
                let phase = 2.0 * PI * freq * i as f64 / pitch;
                (re + (x - mean) * phase.cos(), im - (x - mean) * phase.sin())
            });
        power += 2.0 * (re * re + im * im) / (n * n);
        freq += step;
    }
    (2.0 * power).sqrt()
}

fn held_for(
    seconds: f64,
    synth: &str,
    options: &[(&str, f64)],
    change: Option<(&str, f64)>,
) -> Vec<f32> {
    let mut engine = engine();
    engine
        .play(&[(0.0, Event::held(1, synth, options))])
        .unwrap();
    if let Some((name, value)) = change {
        let set = Event::Set {
            target: Target::Sounds(vec![1]),
            options: vec![(name.into(), value)],
        };
        engine.play(&[(1.0, set)]).unwrap();
    }
    engine.render(seconds).unwrap().0
}

#[test]
fn a_held_flute_and_tenor_sax_note_given_vibrato_0_25_swings_a_quarter_semitone_either_way() {
    for (synth, note) in [("flute", 72.0), ("flute", 79.0), ("tenor_sax", 55.0)] {
        let sound = held_for(8.0, synth, &[("note", note), ("vibrato", 0.25)], None);
        let depth = vibrato_depth(&sound, 440.0 * 2f64.powf((note - 69.0) / 12.0));
        println!("{synth} {note}: {depth:.4} semitones");
        assert!((depth - 0.25).abs() < 0.05, "{synth} {note}: {depth}");
    }
}

#[test]
fn vibrato_given_to_a_held_clarinet_note_while_it_sounds_swings_it() {
    let options = [("note", 60.0), ("vibrato", 0.0)];
    let sound = held_for(8.0, "clarinet", &options, Some(("vibrato", 0.25)));
    let depth = vibrato_depth(&sound, 440.0 * 2f64.powf(-9.0 / 12.0));
    let still = vibrato_depth(
        &held_for(8.0, "clarinet", &options, None),
        440.0 * 2f64.powf(-9.0 / 12.0),
    );
    println!("{depth:.4} semitones, {still:.4} left alone");
    assert!((depth - 0.25).abs() < 0.05, "{depth}");
    assert!(still < 0.05, "{still}");
}
