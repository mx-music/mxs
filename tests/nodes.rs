use fundsp::prelude::*;
use mxs::nodes::{struck, struck_gliding};

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
