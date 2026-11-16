use mxs::synths::{PartnerKind, PianoParts, piano_parts};
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
fn the_piano_sounds_as_before_its_parts_were_given() {
    let before: [(f64, f64, f64, [f64; 6]); 18] = [
        (
            21.0,
            0.2,
            0.0,
            [
                8.746297898655027e-3,
                1.0197114938325018e-3,
                1.679163414458695e-4,
                2.0572609672855904e-6,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.2,
            1.0,
            [
                3.800656562275634e-2,
                1.9302793436874806e-2,
                3.847052329862799e-3,
                5.8838543260475533e-5,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.6,
            0.0,
            [
                1.3827761678899273e1,
                6.193487863345389e0,
                9.857094851600292e-1,
                1.1754926783022139e-2,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            0.6,
            1.0,
            [
                2.1711517144861183e1,
                1.2251400439690325e1,
                2.2898295585826207e0,
                3.2166860894754026e-2,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            1.0,
            0.0,
            [
                9.803752695394728e2,
                4.0623247242402795e2,
                5.930101393604735e1,
                6.580844506982716e-1,
                0e0,
                0e0,
            ],
        ),
        (
            21.0,
            1.0,
            1.0,
            [
                6.963212721638998e2,
                3.5430527266979186e2,
                6.11573333912983e1,
                7.967234919558469e-1,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.2,
            0.0,
            [
                9.111630383528883e-2,
                2.3005491966380805e-2,
                2.6254911730939664e-3,
                2.4066948571875215e-5,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.2,
            1.0,
            [
                1.4026804832672546e-1,
                5.2476241009793825e-2,
                7.450554926032945e-3,
                7.966030626705106e-5,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.6,
            0.0,
            [
                3.1044064830158295e1,
                8.714147773410732e0,
                9.093176024636075e-1,
                6.541964207905424e-3,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            0.6,
            1.0,
            [
                4.299620685693665e1,
                1.596705827032582e1,
                2.089435081150042e0,
                1.981085667310677e-2,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            1.0,
            0.0,
            [
                4.05675149807036e2,
                1.1293377320433412e2,
                1.179813162674646e1,
                8.5234183178967e-2,
                0e0,
                0e0,
            ],
        ),
        (
            60.0,
            1.0,
            1.0,
            [
                5.577136882049632e2,
                2.0598339896526753e2,
                2.6954847128954675e1,
                2.557408144581984e-1,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.2,
            0.0,
            [
                1.3089465898500055e-3,
                2.205567449599075e-9,
                7.918092444200107e-14,
                6.375214629132397e-19,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.2,
            1.0,
            [
                1.3217275442417627e-3,
                2.2829712231375925e-9,
                9.256268171963393e-14,
                7.639640109402904e-19,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.6,
            0.0,
            [
                5.033550565901991e-2,
                2.0292492419057052e-7,
                1.892966623274356e-11,
                1.549101905575671e-16,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            0.6,
            1.0,
            [
                5.34867869626891e-2,
                2.217226211562421e-7,
                2.2181400310877853e-11,
                1.8563583945405274e-16,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            1.0,
            0.0,
            [
                3.844023923395057e-1,
                2.266677698340949e-6,
                2.431161902086179e-10,
                1.9921519457023673e-15,
                0e0,
                0e0,
            ],
        ),
        (
            108.0,
            1.0,
            1.0,
            [
                4.2406028404839524e-1,
                2.5083865772937607e-6,
                2.8493367540400907e-10,
                2.387286597937207e-15,
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
    assert!(middle <= 24 && low <= 48);
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

#[test]
fn without_aftersound_or_unevenness_only_the_first_ten_harmonics_have_twins() {
    let parts = parts(&[("note", 60.0), ("aftersound", 0.0), ("uneven", 0.0)]);
    assert!(parts.partners.iter().all(|p| p.kind == PartnerKind::Twin));
    let twins: Vec<u32> = parts.partners.iter().map(|p| p.harmonic).collect();
    assert_eq!(twins, (1..=10).collect::<Vec<_>>());
}

#[test]
fn by_default_harmonics_have_a_slow_stage() {
    let parts = parts(&[("note", 60.0)]);
    assert!(parts.partners.iter().any(|p| p.kind == PartnerKind::Slow));
}

#[test]
fn the_knock_rings_the_soundboard_unless_it_is_off() {
    assert_eq!(parts(&[("note", 60.0), ("knock", 0.0)]).knock, None);
    let knock = parts(&[("note", 60.0), ("knock", 1.0)]).knock.unwrap();
    assert!(knock.level > 0.0);
    assert_eq!(
        knock.resonances,
        [80.0, 105.0, 160.0, 230.0, 320.0, 350.0, 410.0, 520.0]
    );
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
fn a_rendered_note_rings_at_the_frequencies_and_fades_its_parts_give() {
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
        // Each partner rings within a hair of its harmonic, so the two are heard, and measured, together.
        let given = |t: f64| {
            let (re, im) = std::iter::once((h.hz, h.level, h.fade))
                .chain(
                    parts
                        .partners
                        .iter()
                        .filter(|p| p.harmonic == h.number)
                        .map(|p| (p.hz, p.level, p.fade)),
                )
                .fold((0.0, 0.0), |(re, im), (hz, level, fade)| {
                    let size = level * 10f64.powf(-3.0 * t / fade);
                    let turn = std::f64::consts::TAU * (hz - h.hz) * t;
                    (re + size * turn.cos(), im + size * turn.sin())
                });
            re.hypot(im)
        };
        let heard = |t: f64| {
            let middle = (t * RATE) as usize;
            let (re, im) = at(&sound, h.hz, middle - 4_800, middle + 4_800);
            re.hypot(im)
        };
        let fall = |level: &dyn Fn(f64) -> f64| 20.0 * (level(0.5) / level(3.0)).log10();
        let (expected, measured) = (fall(&given), fall(&heard));
        assert!(
            (measured / expected - 1.0).abs() < 0.1,
            "harmonic {} falls {measured} dB from 0.5 to 3 s, not {expected} dB",
            h.number
        );
    }
}
