//! Counts the memory FunDSP's Sequencer frees on the thread that renders its backend, as an audio callback would, and how many events sound: sequencer_frees <events> [batches]
//! Each batch of events is pushed after the backend renders a block, and every event lasts 10 ms; the frontend pushes nothing afterwards.

use fundsp::prelude::*;
use fundsp::realseq::SequencerBackend;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static FREES: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static AUDIO: Cell<bool> = const { Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if AUDIO.with(|a| a.get()) {
            FREES.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|n| n.parse().ok())
        .collect();
    let (events, batches) = (
        args.first().copied().unwrap_or(100),
        args.get(1).copied().unwrap_or(1),
    );
    let mut sequencer = Sequencer::new(0, 1, ReplayMode::None);
    sequencer.set_sample_rate(48_000.0);
    let backend = std::sync::Arc::new(std::sync::Mutex::new(sequencer.backend()));
    let mut sounding = 0.0;
    for _ in 0..batches {
        for _ in 0..events {
            sequencer.push(0.0, 0.01, Fade::Smooth, 0.0, 0.0, Box::new(dc(1.0)));
        }
        let backend = backend.clone();
        sounding = std::thread::spawn(move || render(&mut backend.lock().unwrap(), 1))
            .join()
            .unwrap();
    }
    let backend = backend.clone();
    std::thread::spawn(move || render(&mut backend.lock().unwrap(), 100))
        .join()
        .unwrap();
    println!(
        "{batches} x {events} events: {sounding} sounding at the last push, {} frees on the audio thread",
        FREES.load(Ordering::Relaxed)
    );
}

/// Renders blocks of 64 samples as the audio thread, and returns the level at the end of the first.
fn render(backend: &mut SequencerBackend, blocks: usize) -> f32 {
    AUDIO.with(|a| a.set(true));
    let mut buffer = BufferVec::new(1);
    let mut level = 0.0;
    for block in 0..blocks {
        backend.process(64, &BufferRef::empty(), &mut buffer.buffer_mut());
        if block == 0 {
            level = buffer.channel_f32(0)[63];
        }
    }
    AUDIO.with(|a| a.set(false));
    level
}
