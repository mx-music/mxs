//! Checks of held notes and of the expressions every note takes, on the synths that follow them.

use mxs::{Engine, Event, RATE, synths};

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
