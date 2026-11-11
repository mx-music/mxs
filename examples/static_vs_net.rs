//! Compares the piano built at run time as a Net with the same piano as one static graph: whether they render the same samples, and how fast each builds and renders.

use mx_engine::{render, synth, Opts, RATE};
use std::time::Instant;

fn main() {
    let cases = [
        ("piano", vec!["note=21", "vel=1"]),
        ("piano", vec!["note=36", "vel=0.3"]),
        ("piano", vec!["note=60"]),
        ("piano", vec!["note=96", "vel=1"]),
        ("berimbau", vec![]),
        ("berimbau", vec!["coin=1"]),
        ("berimbau", vec!["buzz=1"]),
    ];
    for (name, args) in cases {
        let opts = Opts::parse(args.iter().copied()).unwrap();
        let net = render(name, &opts, 3.0).unwrap();
        let fixed = render(&format!("{name}.static"), &opts, 3.0).unwrap();
        print!("{name} ");
        let peak = net.iter().fold(0f32, |m, x| m.max(x.abs()));
        let diff = net.iter().zip(&fixed).fold(0f32, |m, (a, b)| m.max((a - b).abs()));
        println!("{}: peak {peak:.4}, largest difference {diff:.2e} ({:.0} dB under the peak)", args.join(" "), 20.0 * (diff / peak).log10());
        // The thump's noise is over 21 ms after the strike; from 50 ms on only the strings sound.
        let after = (0.05 * RATE) as usize;
        let strings = net[after..].iter().zip(&fixed[after..]).fold(0f32, |m, (a, b)| m.max((a - b).abs()));
        println!("  from 50 ms on: largest difference {strings:.2e} ({:.0} dB under the peak)", 20.0 * (strings / peak).log10());
    }

    for (name, note) in [
        ("piano", "note=36"),
        ("piano", "note=60"),
        ("piano", "note=96"),
        ("piano.static", "note=36"),
        ("piano.static", "note=60"),
        ("piano.static", "note=96"),
        ("berimbau", "buzz=0"),
        ("berimbau.static", "buzz=0"),
    ] {
        {
            let opts = Opts::parse([note, "sustain=60"]).unwrap();
            let count = 200;
            let start = Instant::now();
            let mut units: Vec<_> = (0..count).map(|_| synth(name, &opts, RATE).unwrap()).collect();
            let build = start.elapsed().as_secs_f64() / count as f64;
            let unit = &mut units[0];
            unit.set_sample_rate(RATE);
            let mut buffer = fundsp::prelude::BufferVec::new(unit.outputs());
            let seconds = 5;
            let start = Instant::now();
            for _ in 0..(seconds * RATE as usize / 64) {
                unit.process(64, &fundsp::prelude::BufferRef::empty(), &mut buffer.buffer_mut());
            }
            let render = start.elapsed().as_secs_f64() / seconds as f64;
            println!("{name} {note}: builds in {:.3} ms, renders a second in {:.2} ms, {:.0} notes at once on one core", build * 1e3, render * 1e3, 1.0 / render);
        }
    }
}
