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
                1.5527814217954264e-1,
                6.395648638074065e-2,
                1.0261875279923635e-2,
                1.547240248198291e-4,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.2,
            1.0,
            [
                5.875870774093055e-2,
                4.100133724068381e-2,
                1.1326897248175698e-2,
                2.0915501949691131e-4,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.6,
            0.0,
            [
                3.608860885824939e1,
                1.5939449664300374e1,
                3.1494157482567715e0,
                5.537712022919852e-2,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.6,
            1.0,
            [
                1.618768487910817e1,
                8.67904621740563e0,
                2.2567242651934136e0,
                4.5504708587068024e-2,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            1.0,
            0.0,
            [
                4.8045895859913526e2,
                1.4480056664618937e2,
                2.5709786844209738e1,
                4.4711648723316477e-1,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            1.0,
            1.0,
            [
                3.876203639317774e2,
                1.5902106687851835e2,
                3.636809663643523e1,
                5.822914140092469e-1,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.2,
            0.0,
            [
                1.0671777196410585e-1,
                1.2277359633612622e-2,
                1.544578982358018e-3,
                1.754780660713087e-5,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.2,
            1.0,
            [
                1.5608627520546123e-1,
                2.9042464768499716e-2,
                3.043932492983409e-3,
                3.5139609021937946e-5,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.6,
            0.0,
            [
                3.371278794940031e1,
                2.987341261997548e0,
                3.6211450317770016e-1,
                4.851649987758955e-3,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.6,
            1.0,
            [
                4.9641803344291915e1,
                8.722618047169126e0,
                8.858422571329762e-1,
                1.058827301750262e-2,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            1.0,
            0.0,
            [
                4.923425450288728e2,
                3.500556317403001e1,
                3.336291046711611e0,
                3.915290469347238e-2,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            1.0,
            1.0,
            [
                7.266605506419872e2,
                1.0509866725176835e2,
                9.712009116592368e0,
                9.626297567026613e-2,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.2,
            0.0,
            [
                3.06869679885579e-4,
                8.328035673833413e-8,
                1.7246200361174643e-10,
                4.2908529716793377e-13,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.2,
            1.0,
            [
                1.2409963668570483e-3,
                3.359664111861324e-6,
                7.511914434728845e-8,
                1.8567664608389745e-10,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.6,
            0.0,
            [
                3.839831378772616e-2,
                2.706942465712587e-6,
                7.416466057340772e-9,
                1.503301678687283e-11,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.6,
            1.0,
            [
                5.508020596721789e-2,
                5.660924237736898e-4,
                1.3700884493365924e-6,
                3.40723962915926e-9,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            1.0,
            0.0,
            [
                3.972085523023889e-1,
                1.9436555342731442e-4,
                6.067332138392033e-9,
                2.6952342533195183e-12,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            1.0,
            1.0,
            [
                2.2950871263593577e0,
                5.114133793958978e-3,
                6.075801595263439e-5,
                8.776655661927419e-8,
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
    let refused = piano_parts(&[("brightness", 1.0)]).unwrap_err();
    assert_eq!(
        refused,
        Error::UnknownOption {
            owner: "piano".to_string(),
            option: "brightness".to_string()
        }
    );
    assert_eq!(refused.to_string(), "piano has no option brightness");
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
