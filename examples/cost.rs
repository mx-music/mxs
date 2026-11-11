//! Times building one note of each instrument, which happens off the audio thread, and rendering a second of it, which happens on it.

use mx_engine::{Engine, Event, RATE, synths};
use std::time::Instant;

fn main() {
    for (name, note) in [("piano", 36.0), ("piano", 60.0), ("cello.bowed", 48.0), ("berimbau", 39.11)] {
        let mut engine = Engine::offline(RATE);
        synths::all().into_iter().for_each(|s| engine.register(s));
        let count = 50;
        let notes: Vec<_> = (0..count).map(|_| (0.0, Event::note(None, name, &[("note", note)]))).collect();
        let start = Instant::now();
        engine.play(&notes).unwrap();
        let build = start.elapsed().as_secs_f64() / count as f64;

        let mut engine = Engine::offline(RATE);
        synths::all().into_iter().for_each(|s| engine.register(s));
        let held: &[(&str, f64)] = if name == "berimbau" { &[] } else { &[("sustain", 60.0)] };
        let options: Vec<(&str, f64)> = [("note", note)].iter().chain(held).copied().collect();
        engine.play(&[(0.0, Event::note(None, name, &options))]).unwrap();
        let start = Instant::now();
        engine.render(1.0).unwrap();
        let render = start.elapsed().as_secs_f64();
        println!("{name} note {note}: builds in {:.3} ms, renders a second in {:.1} ms, {:.0} notes at once on one core", build * 1e3, render * 1e3, 1.0 / render);
    }
}
