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

fn checksum(sound: &[f32]) -> u64 {
    sound.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, x| {
        x.to_bits().to_le_bytes().iter().fold(hash, |h, b| {
            (h ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3)
        })
    })
}

#[test]
fn the_piano_sounds_sample_for_sample_as_before_its_parts_were_given() {
    let before = [
        (21.0, 0.2, 0.0, 0xca6b346b1fdf55d7),
        (21.0, 0.2, 1.0, 0x268d03e5278d30b1),
        (21.0, 0.6, 0.0, 0xb8305742fa139db1),
        (21.0, 0.6, 1.0, 0x8d9195d8fb451790),
        (21.0, 1.0, 0.0, 0x3d5a35d9ad7b06f9),
        (21.0, 1.0, 1.0, 0x34e885eec7422371),
        (60.0, 0.2, 0.0, 0x6408783c2e616f0c),
        (60.0, 0.2, 1.0, 0x7d50fb8dfd1e9686),
        (60.0, 0.6, 0.0, 0x194550ad43ceefc2),
        (60.0, 0.6, 1.0, 0x979ce758ae93b85c),
        (60.0, 1.0, 0.0, 0xf8f607796f3f9528),
        (60.0, 1.0, 1.0, 0xa9a1e013fe4a1102),
        (108.0, 0.2, 0.0, 0x7f2be649e3e96bd3),
        (108.0, 0.2, 1.0, 0xb11ce5e0d83a14ef),
        (108.0, 0.6, 0.0, 0xd429602864b9a3c6),
        (108.0, 0.6, 1.0, 0x7187397c27bb7427),
        (108.0, 1.0, 0.0, 0xb0693a6540eff256),
        (108.0, 1.0, 1.0, 0xcd4c5719de3c7808),
    ];
    for (note, vel, pedal, sum) in before {
        let sound = piano(&[("note", note), ("vel", vel), ("pedal", pedal)], 3.0);
        assert_eq!(
            checksum(&sound),
            sum,
            "note {note} at vel {vel} and pedal {pedal} changed"
        );
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
