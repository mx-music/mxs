//! Times building one note of each instrument, which happens off the audio thread, and rendering a second of it, which happens on it.

use mx_engine::{synth, Opts, RATE};
use std::time::Instant;

fn main() {
    for (name, args) in [("piano", vec!["note=36"]), ("piano", vec!["note=60"]), ("cello.bowed", vec!["note=48"]), ("berimbau", vec![])] {
        let opts = Opts::parse(args.iter().copied().chain(["sustain=60"])).unwrap();
        let count = 50;
        let start = Instant::now();
        let mut units: Vec<_> = (0..count).map(|_| synth(name, &opts, RATE).unwrap()).collect();
        let build = start.elapsed().as_secs_f64() / count as f64;
        let unit = &mut units[0];
        unit.set_sample_rate(RATE);
        let mut buffer = fundsp::prelude::BufferVec::new(unit.outputs());
        let start = Instant::now();
        for _ in 0..(RATE as usize / 64) {
            unit.process(64, &fundsp::prelude::BufferRef::empty(), &mut buffer.buffer_mut());
        }
        let render = start.elapsed().as_secs_f64();
        println!("{name} {}: builds in {:.2} ms, renders a second in {:.1} ms, {:.0} notes at once on one core", args.join(" "), build * 1e3, render * 1e3, 1.0 / render);
    }
}
