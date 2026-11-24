use mxs::graph::{Clause, Graph};
use mxs::{Engine, Event, RATE};

fn n(x: f64) -> Graph {
    Graph::Number(x)
}

fn call(name: &str, args: Vec<Graph>, named: Vec<(&str, Graph)>) -> Graph {
    Graph::call(name, args, named)
}

fn sine(hz: f64) -> Graph {
    call("sine", vec![], vec![("hz", n(hz))])
}

/// The note `sound` makes for `seconds`, at its own level: the engine plays a note in the middle at 1 / √2 on each side.
fn sound(sound: Graph, seconds: f64) -> Vec<f32> {
    let graph = call("rings", vec![sound], vec![("seconds", n(seconds + 1.0))]);
    let synth = mxs::graph::synth(
        "shaped",
        vec![],
        vec![Clause {
            matches: vec![],
            graph,
        }],
    )
    .unwrap();
    let mut engine = Engine::offline(RATE);
    engine.register(synth);
    let note = Event::Note {
        id: None,
        synth: "shaped".to_string(),
        options: Default::default(),
        chain: None,
        held: false,
    };
    engine.play(&[(0.0, note)]).unwrap();
    let left = engine.render(seconds).unwrap().0;
    left.iter().map(|x| x * std::f32::consts::SQRT_2).collect()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, x| m.max(x.abs()))
}

/// The level of `samples` at `hz`, by a Goertzel filter over all of them.
fn level(samples: &[f32], hz: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * hz / RATE;
    let (mut s1, mut s2) = (0.0, 0.0);
    for &x in samples {
        let s = x as f64 + 2.0 * w.cos() * s1 - s2;
        s2 = s1;
        s1 = s;
    }
    let power = s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2;
    2.0 * power.sqrt() / samples.len() as f64
}

fn db(x: f64) -> f64 {
    20.0 * x.log10()
}

/// The level of `samples` around `hz`, averaged over the bins within 50 Hz, for noise.
fn band(samples: &[f32], hz: f64) -> f64 {
    let bins: Vec<f64> = (-10..=10)
        .map(|k| level(samples, hz + k as f64 * 5.0).powi(2))
        .collect();
    (bins.iter().sum::<f64>() / bins.len() as f64).sqrt()
}

/// The strongest level of `samples` at whole-Hz frequencies more than 30 Hz from every harmonic of `hz`.
fn strongest_off(samples: &[f32], hz: f64) -> f64 {
    (100..24_000)
        .step_by(10)
        .map(|f| f as f64)
        .filter(|f| {
            let k = (f / hz).round();
            (f - k * hz).abs() > 30.0
        })
        .map(|f| level(samples, f))
        .fold(0.0, f64::max)
}

#[test]
fn a_sine_saturated_with_a_drive_of_24_db_peaks_at_no_more_than_1_and_its_third_harmonic_stands_within_20_db()
 {
    let shaped = sound(
        call("saturate", vec![sine(220.0)], vec![("drive", n(24.0))]),
        1.0,
    );
    let steady = &shaped[4800..];
    assert!(peak(steady) <= 1.0, "peak {}", peak(steady));
    let (first, third) = (level(steady, 220.0), level(steady, 660.0));
    assert!(
        db(third / first) > -20.0,
        "third harmonic {} dB",
        db(third / first)
    );
}

#[test]
fn a_sine_clipped_hard_at_half_peaks_at_half() {
    let shaped = sound(
        call("clip", vec![sine(220.0)], vec![("level", n(0.5))]),
        1.0,
    );
    assert!(
        (peak(&shaped[4800..]) - 0.5).abs() < 1e-3,
        "peak {}",
        peak(&shaped[4800..])
    );
}

// The ticket's sine of level 1 folded at 0.5 only touches 0 at its peak, so the sine is driven 6 dB past it to fold through 0.
#[test]
fn a_sine_folded_at_half_peaks_at_no_more_than_half_and_crosses_zero_more_than_twice_a_period() {
    let folded = call(
        "fold",
        vec![sine(100.0)],
        vec![("level", n(0.5)), ("drive", n(6.0))],
    );
    let shaped = sound(folded, 1.0);
    let steady = &shaped[4800..4800 + 4800];
    assert!(peak(steady) <= 0.5 + 1e-6, "peak {}", peak(steady));
    let crossings = steady
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    assert!(crossings > 2 * 10, "{crossings} crossings over 10 periods");
}

// At 4.8 kHz, the ticket's frequency, every alias lands on a harmonic, since 48 kHz is 10 of its periods, so 5 kHz is measured instead.
#[test]
fn saturating_at_twice_the_rate_makes_weaker_aliases_than_saturating_at_the_engine_rate() {
    let hz = 5000.0;
    let shaped = sound(
        call("saturate", vec![sine(hz)], vec![("drive", n(30.0))]),
        1.0,
    );
    let gain = 10f64.powf(30.0 / 20.0);
    let plain: Vec<f32> = (0..shaped.len())
        .map(|i| ((2.0 * std::f64::consts::PI * hz * i as f64 / RATE).sin() * gain).tanh() as f32)
        .collect();
    let (ours, theirs) = (
        strongest_off(&shaped[4800..], hz),
        strongest_off(&plain[4800..], hz),
    );
    assert!(
        ours < theirs,
        "aliases at {} dB, against {} dB",
        db(ours),
        db(theirs)
    );
}

#[test]
fn white_noise_through_the_ladder_at_1_khz_stands_40_db_lower_at_4_khz_than_at_250_hz() {
    let noise = call("noise", vec![Graph::Name("white".to_string())], vec![]);
    let filtered = sound(
        call(
            "ladder",
            vec![noise],
            vec![("hz", n(1000.0)), ("resonance", n(0.0))],
        ),
        4.0,
    );
    let fall = db(band(&filtered, 4000.0) / band(&filtered, 250.0));
    assert!(fall <= -40.0, "{fall} dB");
}

#[test]
fn an_impulse_into_the_ladder_at_full_resonance_rings_on_at_its_cutoff_for_a_second() {
    let ringing = sound(
        call(
            "ladder",
            vec![call("impulse", vec![], vec![])],
            vec![("hz", n(500.0)), ("resonance", n(1.0))],
        ),
        2.0,
    );
    let late = &ringing[48_000..96_000];
    assert!(peak(late) > 1e-4, "peak {} after a second", peak(late));
    let crossings = late
        .windows(2)
        .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
        .count() as f64;
    let cents = 1200.0 * (crossings / 500.0).log2();
    assert!(cents.abs() < 20.0, "rings at {crossings} Hz");
}

#[test]
fn a_lowpass_whose_q_glides_from_0_7_to_10_rises_15_db_at_its_cutoff() {
    let q = call(
        "add",
        vec![
            n(0.7),
            call(
                "envelope",
                vec![Graph::List(vec![Graph::List(vec![n(1.0), n(9.3)])])],
                vec![],
            ),
        ],
        vec![],
    );
    let filtered = sound(
        call(
            "lowpass",
            vec![sine(1000.0)],
            vec![("hz", n(1000.0)), ("q", q)],
        ),
        1.0,
    );
    let (start, end) = (
        level(&filtered[480..4800], 1000.0),
        level(&filtered[43_200..48_000], 1000.0),
    );
    assert!(db(end / start) >= 15.0, "rose {} dB", db(end / start));
}

#[test]
fn removing_the_offset_at_10_hz_centres_the_sound_and_leaves_a_100_hz_sine_as_it_was() {
    let offset = call(
        "add",
        vec![n(0.5), call("gain", vec![sine(100.0), n(0.25)], vec![])],
        vec![],
    );
    let centred = sound(call("dc_block", vec![offset], vec![("hz", n(10.0))]), 2.0);
    let late = &centred[24_000..96_000];
    let mean = late.iter().map(|&x| x as f64).sum::<f64>() / late.len() as f64;
    assert!(mean.abs() < 1e-3, "mean {mean}");
    let change = db(level(late, 100.0) / 0.25);
    assert!(change.abs() < 0.1, "{change} dB");
}

#[test]
fn removing_the_offset_without_hz_sounds_as_it_did_at_10_hz() {
    let offset = || call("add", vec![n(0.5), sine(100.0)], vec![]);
    let given = sound(call("dc_block", vec![offset()], vec![("hz", n(10.0))]), 0.5);
    let plain = sound(call("dc_block", vec![offset()], vec![]), 0.5);
    assert_eq!(given, plain);
}

#[test]
fn drive_and_level_move_while_the_note_sounds() {
    let level = call(
        "add",
        vec![
            n(0.2),
            call(
                "envelope",
                vec![Graph::List(vec![Graph::List(vec![n(1.0), n(0.6)])])],
                vec![],
            ),
        ],
        vec![],
    );
    let shaped = sound(call("clip", vec![sine(220.0)], vec![("level", level)]), 1.0);
    let (early, late) = (peak(&shaped[480..4800]), peak(&shaped[43_200..48_000]));
    assert!(early < 0.35 && late > 0.7, "peaks {early} then {late}");
}
