use mxs::synths::{PianoParts, piano_parts};
use mxs::{Engine, Error, Event, RATE, synths};

fn piano(options: &[(&str, f64)], seconds: f64) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
        .play(&[(0.0, Event::note(None, "piano", options))])
        .unwrap();
    engine.render(seconds).unwrap().0
}

fn parts(options: &[(&str, f64)]) -> PianoParts {
    piano_parts(options).unwrap().0
}

/// The energy of each half second of a render, which a change to the sound moves and the last bit of a platform's maths does not.
fn energies(sound: &[f32]) -> Vec<f64> {
    sound
        .chunks(sound.len() / 6)
        .take(6)
        .map(|slice| slice.iter().map(|x| (*x as f64).powi(2)).sum())
        .collect()
}

#[test]
#[ignore]
fn print_energies() {
    for note in [21.0, 60.0, 108.0] {
        for vel in [0.2, 0.6, 1.0] {
            for pedal in [0.0, 1.0] {
                let sound = piano(&[("note", note), ("vel", vel), ("pedal", pedal)], 3.0);
                let energies: Vec<String> =
                    energies(&sound).iter().map(|e| format!("{e:e}")).collect();
                println!("({note:?}, {vel:?}, {pedal:?}, [{}]),", energies.join(", "));
            }
        }
    }
}

#[test]
fn the_piano_sounds_as_its_figures_were_fitted() {
    let before: [(f64, f64, f64, [f64; 6]); 18] = [
        (
            21.0,
            0.2,
            0.0,
            [
                8.175109688580266e-2,
                3.8170791431930216e-2,
                7.727879576581801e-3,
                1.4599833368636594e-4,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.2,
            1.0,
            [
                1.1407666075761008e-1,
                6.725184294409298e-2,
                1.7220126488776712e-2,
                2.8547842528884454e-4,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.6,
            0.0,
            [
                2.7230471436041743e1,
                1.2233419195840979e1,
                2.527951117372006e0,
                4.560169017378049e-2,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.6,
            1.0,
            [
                3.2969578651643374e1,
                1.7104792566654925e1,
                4.342280259896723e0,
                7.648882903216526e-2,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            1.0,
            0.0,
            [
                3.784632692353139e2,
                1.394960119961121e2,
                2.7796100996651585e1,
                4.6712485171070384e-1,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            1.0,
            1.0,
            [
                4.8919092652245115e2,
                2.180648492704398e2,
                5.2191435705409006e1,
                1.0230542098895046e0,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.2,
            0.0,
            [
                1.0664047875056525e-1,
                1.226846741886721e-2,
                1.5434602843245504e-3,
                1.7535097145063194e-5,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.2,
            1.0,
            [
                1.5621678195904884e-1,
                2.9066747798697987e-2,
                3.0464775883513096e-3,
                3.516898985166956e-5,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.6,
            0.0,
            [
                3.368837110815106e1,
                2.985177646110462e0,
                3.6185223754707174e-1,
                4.84813612944349e-3,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.6,
            1.0,
            [
                4.968330755625918e1,
                8.729910791493278e0,
                8.865828860726985e-1,
                1.0597125612140866e-2,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            1.0,
            0.0,
            [
                4.9198598760423056e2,
                3.498021190611004e1,
                3.333874890758761e0,
                3.912454998382551e-2,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            1.0,
            1.0,
            [
                7.27268148150127e2,
                1.0518654545520178e2,
                9.7201298493685e0,
                9.63434658368236e-2,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.2,
            0.0,
            [
                1.4687281816921959e-3,
                3.5573668026413146e-7,
                6.837264647170705e-10,
                1.7035892089899346e-12,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.2,
            1.0,
            [
                1.7544203518481414e-3,
                7.4533603375028175e-6,
                9.638140378366668e-8,
                2.353634660078096e-10,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.6,
            0.0,
            [
                3.9798776442374456e-1,
                2.5604253412323624e-5,
                8.026551668796153e-8,
                1.6269882206016322e-10,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.6,
            1.0,
            [
                4.892541028658641e-1,
                1.8154038607793113e-3,
                2.0296600153582135e-5,
                4.892665470859242e-8,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            1.0,
            0.0,
            [
                6.717621795486939e0,
                1.9222216684264624e-3,
                9.46073204381189e-8,
                0e0,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            1.0,
            1.0,
            [
                7.347844049837823e0,
                1.4709039692257439e-2,
                2.0246517972766066e-4,
                2.774975609302622e-7,
                0e0,
                0e0,
            ],
        ),
    ];
    for (note, vel, pedal, energies_before) in before {
        let sound = piano(&[("note", note), ("vel", vel), ("pedal", pedal)], 3.0);
        for (now, then) in energies(&sound).iter().zip(energies_before) {
            assert!(
                (now - then).abs() <= 1e-6 * then.abs() + 1e-15,
                "note {note} at vel {vel} and pedal {pedal} changed: {now} against {then}"
            );
        }
    }
}

#[test]
fn the_parts_of_middle_c_begin_with_its_fundamental() {
    let parts = parts(&[("note", 60.0), ("vel", 0.6)]);
    let first = parts.harmonics[0];
    assert_eq!(first.number, 1);
    assert!(
        (first.hz - 261.63).abs() < 0.5,
        "harmonic 1 is at {} Hz",
        first.hz
    );
    for h in &parts.harmonics {
        assert!(h.hz > 0.0 && h.level > 0.0 && h.fade > 0.0, "{h:?}");
    }
}

#[test]
fn a_note_below_c2_has_more_harmonics() {
    let low = parts(&[("note", 24.0)]).harmonics.len();
    let middle = parts(&[("note", 60.0)]).harmonics.len();
    assert!(
        low > middle,
        "note 24 has {low} harmonics and note 60 {middle}"
    );
}

#[test]
fn no_harmonic_reaches_16_khz() {
    let parts = parts(&[("note", 100.0)]);
    assert!(parts.harmonics.iter().all(|h| h.hz < 16_000.0));
}

#[test]
fn a_harder_strike_is_brighter() {
    let tenth_against_first = |vel: f64| {
        let parts = parts(&[("note", 60.0), ("vel", vel)]);
        let level = |n| {
            parts
                .harmonics
                .iter()
                .find(|h| h.number == n)
                .unwrap()
                .level
        };
        level(10) / level(1)
    };
    assert!(tenth_against_first(1.0) > tenth_against_first(0.3));
}

#[test]
fn the_pedal_lets_the_fundamental_ring_longer() {
    let fade = |pedal: f64| parts(&[("note", 60.0), ("pedal", pedal)]).harmonics[0].fade;
    assert!(fade(1.0) > fade(0.0));
}

/// The level in dB of each 50 ms of a render, from its start.
fn levels(sound: &[f32]) -> Vec<f64> {
    sound
        .chunks(2_400)
        .map(|slice| {
            10.0 * (slice.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / slice.len() as f64)
                .log10()
        })
        .collect()
}

/// The Steinway's C4 at vl4 falls 18.8 dB from its 50 ms after the strike to its 50 ms a second later.
#[test]
fn by_default_middle_c_falls_over_its_first_second_as_far_as_the_steinways() {
    let sound = piano(&[("note", 60.0), ("vel", 1.0), ("sustain", 4.0)], 1.2);
    let levels = levels(&sound);
    let fallen = levels[1] - levels[21];
    assert!((fallen - 18.8).abs() <= 2.0, "it fell {fallen} dB");
}

#[test]
fn aftersound_0_fades_each_partial_at_one_rate() {
    let parts =
        |aftersound: f64| parts(&[("note", 60.0), ("vel", 1.0), ("aftersound", aftersound)]);
    let sound = piano(
        &[
            ("note", 60.0),
            ("vel", 1.0),
            ("aftersound", 0.0),
            ("knock", 0.0),
            ("sustain", 8.0),
        ],
        4.0,
    );
    let hz = parts(0.0).harmonics[0].hz;
    let level = |t: f64| {
        let middle = (t * RATE) as usize;
        let (re, im) = at(&sound, hz, middle - 4_800, middle + 4_800);
        20.0 * re.hypot(im).log10()
    };
    let (early, late) = (level(0.5) - level(1.5), level(2.5) - level(3.5));
    assert!(
        (early - late).abs() < 1.5,
        "the fundamental fell {early} dB in its second second and {late} dB in its fourth"
    );
}

#[test]
fn the_knock_rings_under_600_hz_unless_it_is_off() {
    assert_eq!(parts(&[("note", 60.0), ("knock", 0.0)]).knock, None);
    let knock = parts(&[("note", 60.0), ("knock", 1.0)]).knock.unwrap();
    assert!(knock.level > 0.0);
    assert!(!knock.resonances.is_empty() && knock.resonances.iter().all(|hz| *hz < 600.0));
    let low = |knock: f64| {
        let sound = piano(&[("note", 60.0), ("vel", 1.0), ("knock", knock)], 0.05);
        let (re, im) = at(&sound, 300.0, 0, 2_400);
        re.hypot(im)
    };
    assert!(low(0.0) < low(1.0));
}

#[test]
fn uneven_varies_only_the_keys_not_recorded() {
    let levels = |note: f64, uneven: f64| -> Vec<f64> {
        parts(&[("note", note), ("uneven", uneven)])
            .harmonics
            .iter()
            .map(|h| h.level)
            .collect()
    };
    assert_eq!(levels(60.0, 1.0), levels(60.0, 0.0));
    assert_ne!(levels(61.0, 1.0), levels(61.0, 0.0));
}

#[test]
fn a_harder_strike_thumps_brighter() {
    let thump = |vel: f64| parts(&[("note", 60.0), ("vel", vel)]).thump;
    assert!(thump(1.0).below_hz > thump(0.3).below_hz);
    assert!(thump(1.0).seconds > 0.0);
}

#[test]
fn the_same_options_give_the_same_parts() {
    let options = [("note", 60.0), ("vel", 0.6), ("instrument", 3.0)];
    assert_eq!(parts(&options), parts(&options));
}

#[test]
fn a_value_out_of_range_is_held_with_a_warning() {
    let (held, warnings) = piano_parts(&[("note", 60.0), ("vel", 2.0)]).unwrap();
    assert_eq!(held, parts(&[("note", 60.0), ("vel", 1.0)]));
    assert_eq!(
        warnings,
        ["piano's option vel was held to its range, 0.05 to 1"]
    );
}

#[test]
fn an_option_the_piano_does_not_have_is_refused() {
    let refused = piano_parts(&[("cutoff", 100.0)]).unwrap_err();
    assert_eq!(
        refused,
        Error::UnknownOption {
            owner: "piano".to_string(),
            option: "cutoff".to_string()
        }
    );
    assert_eq!(refused.to_string(), "piano has no option cutoff");
}

/// The amplitude of `sound` at `hz` over a window of samples, as a complex number.
fn at(sound: &[f32], hz: f64, from: usize, to: usize) -> (f64, f64) {
    let (re, im) = sound[from..to]
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(re, im), (i, x)| {
            let turn = std::f64::consts::TAU * hz * (from + i) as f64 / RATE;
            (re + *x as f64 * turn.cos(), im + *x as f64 * turn.sin())
        });
    let n = (to - from) as f64 / 2.0;
    (re / n, im / n)
}

#[test]
fn a_rendered_note_rings_at_the_frequencies_its_parts_give() {
    let options = [
        ("note", 60.0),
        ("vel", 0.6),
        ("knock", 0.0),
        ("sustain", 8.0),
    ];
    let parts = parts(&options);
    let sound = piano(&options, 4.0);
    for h in &parts.harmonics[..8] {
        let peak = (-60..=60)
            .map(|step| h.hz + step as f64 * 0.025)
            .max_by(|a, b| {
                let size = |hz: f64| {
                    let (re, im) = at(&sound, hz, 4_800, 100_800);
                    re.hypot(im)
                };
                size(*a).total_cmp(&size(*b))
            })
            .unwrap();
        assert!(
            (peak - h.hz).abs() < 1.0,
            "harmonic {} rings at {peak} Hz, not {} Hz",
            h.number,
            h.hz
        );
    }
}

/// The level in dB of a note's first half second.
fn opening(options: &[(&str, f64)]) -> f64 {
    let sound = piano(options, 0.5);
    10.0 * (sound.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / sound.len() as f64).log10()
}

#[test]
fn every_key_up_to_the_highest_recorded_is_louder_at_each_harder_velocity() {
    for note in 21..=104 {
        let levels: Vec<f64> = [0.2, 0.4, 0.6, 0.8, 1.0]
            .iter()
            .map(|vel| opening(&[("note", note as f64), ("vel", *vel)]))
            .collect();
        assert!(
            levels.windows(2).all(|pair| pair[1] > pair[0]),
            "note {note} at vel 0.2 to 1: {levels:?}"
        );
    }
}

#[test]
fn no_key_stands_more_than_6_db_from_the_keys_either_side() {
    let levels: Vec<f64> = (21..=104)
        .map(|note| opening(&[("note", note as f64), ("vel", 0.6), ("uneven", 0.0)]))
        .collect();
    for i in 1..levels.len() - 1 {
        let around = (levels[i - 1] + levels[i + 1]) / 2.0;
        assert!(
            (levels[i] - around).abs() <= 6.0,
            "note {} is {:.1} dB from the keys either side",
            21 + i,
            levels[i] - around
        );
    }
}
