use fundsp::prelude::*;
use mxs::nodes::{harmonic_tone, struck, struck_gliding};

#[test]
fn a_gliding_resonator_at_a_steady_frequency_rings_as_a_struck_one() {
    let mut fixed = struck(3000.0, 0.7, 0.3);
    let mut gliding =
        (Net::wrap(Box::new(pass())) | Net::wrap(Box::new(dc(3000.0)))) >> struck_gliding(0.7, 0.3);
    for n in 0..2000 {
        let x = if n == 0 { 1.0 } else { 0.0 };
        let (a, b) = (fixed.filter_mono(x), gliding.filter_mono(x));
        assert!((a - b).abs() < 1e-5, "sample {n}: {a} against {b}");
    }
}

#[test]
fn a_harmonic_tone_sums_sines_at_the_levels_asked_for_each_harmonic() {
    let rate = DEFAULT_SR;
    let mut tone = Net::wrap(Box::new(dc(300.0)))
        >> harmonic_tone(3, |k, f, t| {
            assert!((f - 300.0 * k as f64).abs() < 1e-6);
            [1.0, 0.0, 0.25 + t][k - 1]
        });
    for n in 0..2000 {
        let t = n as f64 / rate;
        let x = std::f64::consts::TAU * 300.0 * t;
        let expected = x.sin() + (0.25 + t) * (3.0 * x).sin();
        let got = tone.get_mono() as f64;
        assert!(
            (got - expected).abs() < 1e-4,
            "sample {n}: {got} against {expected}"
        );
    }
}

#[test]
fn a_harmonic_tone_leaves_out_harmonics_near_half_the_sample_rate() {
    let mut tone = Net::wrap(Box::new(dc(9000.0))) >> harmonic_tone(4, |_, _, _| 1.0);
    let rate = DEFAULT_SR;
    for n in 0..500 {
        let x = std::f64::consts::TAU * 9000.0 * n as f64 / rate;
        let expected = x.sin() + (2.0 * x).sin();
        let got = tone.get_mono() as f64;
        assert!(
            (got - expected).abs() < 1e-3,
            "sample {n}: {got} against {expected}"
        );
    }
}
