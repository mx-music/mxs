//! mxs-00011's spike: how many notes play at once when the voices are filled on several cores.
//! cargo test --release --lib spike_parallel -- --ignored --nocapture --test-threads 1

use crate::mixer::Voice;
use crate::{Engine, Event, RATE, synths};
use fundsp::prelude::*;
use std::sync::{Barrier, Mutex};
use std::time::Instant;

const BLOCK: usize = 64;

fn voices(name: &str, note: f64, count: usize) -> Vec<Box<Voice>> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let held: &[(&str, f64)] = if name == "berimbau" { &[] } else { &[("sustain", 60.0)] };
    let notes: Vec<_> = (0..count)
        .map(|i| {
            let options: Vec<(&str, f64)> = [("note", note + (i % 12) as f64)].iter().chain(held).copied().collect();
            (0.0, Event::note(None, name, &options))
        })
        .collect();
    for batch in notes.chunks(256) {
        engine.play(batch).unwrap();
    }
    engine.take_voices()
}

/// Fills `blocks` blocks of every voice into its own buffer and sums them in voice order, on `threads` threads, returning the seconds taken and the left channel.
fn fill(voices: &mut [Box<Voice>], threads: usize, blocks: usize) -> (f64, Vec<f32>) {
    let per = voices.len().div_ceil(threads);
    let outputs: Vec<Mutex<Vec<[f32; 2 * BLOCK]>>> = voices.chunks(per).map(|c| Mutex::new(vec![[0.0; 2 * BLOCK]; c.len()])).collect();
    let mut left = Vec::with_capacity(blocks * BLOCK);
    let started = Instant::now();
    if threads == 1 {
        let mut buffer = BufferVec::new(2);
        for _ in 0..blocks {
            let mut mix = [0.0f32; BLOCK];
            for voice in voices.iter_mut() {
                voice.unit.process(BLOCK, &BufferRef::empty(), &mut buffer.buffer_mut());
                for (k, x) in mix.iter_mut().enumerate() {
                    *x += buffer.at_f32(0, k);
                }
            }
            left.extend_from_slice(&mix);
        }
    } else {
        let start = Barrier::new(outputs.len() + 1);
        let done = Barrier::new(outputs.len() + 1);
        std::thread::scope(|scope| {
            for (chunk, output) in voices.chunks_mut(per).zip(&outputs) {
                let (start, done) = (&start, &done);
                scope.spawn(move || {
                    let mut buffer = BufferVec::new(2);
                    for _ in 0..blocks {
                        start.wait();
                        let mut output = output.lock().unwrap();
                        for (voice, out) in chunk.iter_mut().zip(output.iter_mut()) {
                            voice.unit.process(BLOCK, &BufferRef::empty(), &mut buffer.buffer_mut());
                            for k in 0..BLOCK {
                                out[k] = buffer.at_f32(0, k);
                                out[BLOCK + k] = buffer.at_f32(1, k);
                            }
                        }
                        drop(output);
                        done.wait();
                    }
                });
            }
            for _ in 0..blocks {
                start.wait();
                done.wait();
                let mut mix = [0.0f32; BLOCK];
                for output in &outputs {
                    for out in output.lock().unwrap().iter() {
                        for (k, x) in mix.iter_mut().enumerate() {
                            *x += out[k];
                        }
                    }
                }
                left.extend_from_slice(&mix);
            }
        });
    }
    (started.elapsed().as_secs_f64(), left)
}

#[test]
#[ignore]
fn notes_at_once_on_several_cores() {
    let seconds = 2.0;
    let blocks = (seconds * RATE) as usize / BLOCK;
    for (name, note, count) in [("piano", 48.0, 240), ("berimbau", 39.11, 240)] {
        let mut reference = None;
        for threads in [1, 2, 4, 6, 8] {
            let mut voices = voices(name, note, count);
            let (took, left) = fill(&mut voices, threads, blocks);
            let at_once = count as f64 * seconds / took;
            let same = reference.get_or_insert_with(|| left.clone()) == &left;
            println!("{name}: {count} notes on {threads} threads render {seconds} s in {took:.3} s, so {at_once:.0} at once; the same samples as on 1: {same}");
        }
    }
    for count in [1, 4, 16] {
        for threads in [1, 2, 4] {
            let mut voices = voices("piano", 48.0, count);
            let (took, _) = fill(&mut voices, threads, blocks);
            let per_block = took / blocks as f64 * 1e6;
            let budget = per_block / (BLOCK as f64 / RATE * 1e6) * 100.0;
            println!("{count} piano notes on {threads} threads: {per_block:.1} µs a block of 64, {budget:.1} percent of its time");
        }
    }
}
