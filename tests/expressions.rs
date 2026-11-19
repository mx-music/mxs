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

/// The share of the window's energy above `freq`, from its spectrum every 20 Hz.
fn share_above(samples: &[f32], freq: f64) -> f64 {
    let power = |f: f64| magnitude(samples, f).powi(2);
    let bins: Vec<f64> = (1..1000).map(|k| k as f64 * 20.0).collect();
    let above: f64 = bins.iter().filter(|f| **f > freq).map(|f| power(*f)).sum();
    above / bins.iter().map(|f| power(*f)).sum::<f64>()
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
        ["amp", "pan", "bend", "brightness", "pressure"]
    );
    assert_eq!(names(engine.input_specs()), ["amp", "pan"]);
    let sample = names(engine.sample_specs());
    assert!(sample.contains(&"amp".to_string()));
    assert!(!sample.contains(&"bend".to_string()), "{sample:?}");
}
