//! Checks of voices kept ready, with an allocator that counts what the thread filling audio allocates.

use mxs::{Engine, Error, Event, RATE, synths};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = COUNT.try_with(|c| c.set(c.get() + 1));
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count();
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn allocations(f: impl FnOnce()) -> usize {
    COUNT.with(|c| c.set(0));
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    COUNT.with(Cell::get)
}

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
}

const BLOCK: usize = 128;

fn chord() -> Vec<Event> {
    [36.0, 43.0, 48.0, 52.0, 55.0, 60.0, 64.0, 67.0, 71.0, 76.0]
        .iter()
        .zip([0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 0.65, 0.45])
        .map(|(&note, vel)| Event::note(None, "piano", &[("note", note), ("vel", vel)]))
        .collect()
}

// The chord handed over at the engine's clock, then the block being filled and a second after it.
fn played(engine: &mut Engine) -> (usize, Vec<f32>) {
    let mut left = vec![0.0; RATE as usize];
    let mut right = vec![0.0; RATE as usize];
    let now = engine.now();
    let events: Vec<(f64, Event)> = chord().into_iter().map(|e| (now, e)).collect();
    engine.play(&events).unwrap();
    let (first, rest) = left.split_at_mut(BLOCK);
    let (first_right, rest_right) = right.split_at_mut(BLOCK);
    let made = allocations(|| engine.fill(first, first_right).unwrap());
    engine.fill(rest, rest_right).unwrap();
    (made, left)
}

#[test]
fn a_chord_of_ten_piano_notes_starts_from_ready_voices_in_the_block_being_filled() {
    let mut engine = engine();
    engine.keep_ready("piano", 10).unwrap();
    engine.render(0.1).unwrap();
    let (made, left) = played(&mut engine);
    assert_eq!(made, 0, "filling the block the chord starts in allocates");
    assert!(left[..BLOCK].iter().any(|s| s.abs() > 0.0));
    let mut built = engine_at_the_same_frame();
    let (_, expected) = played(&mut built);
    assert!(
        left == expected,
        "the ready chord plays otherwise than the built one"
    );
}

// An engine at the same frame with no voices kept ready, which builds the chord's notes.
fn engine_at_the_same_frame() -> Engine {
    let mut engine = engine();
    engine.render(0.1).unwrap();
    engine
}

#[test]
fn the_voices_kept_ready_are_replaced_as_a_chord_takes_them() {
    let mut engine = engine();
    engine.keep_ready("piano", 10).unwrap();
    played(&mut engine);
    assert_eq!(engine.kept_ready("piano"), 10);
}

#[test]
fn a_note_with_an_option_besides_note_and_vel_is_built_and_leaves_the_ready_voices() {
    let mut engine = engine();
    engine.keep_ready("piano", 2).unwrap();
    let louder = Event::note(None, "piano", &[("note", 60.0), ("amp", 0.5)]);
    let other = Event::note(None, "marimba", &[("note", 60.0)]);
    engine.play(&[(0.0, louder), (0.0, other)]).unwrap();
    assert_eq!(engine.kept_ready("piano"), 2);
    let (left, _) = engine.render(0.5).unwrap();
    assert!(left.iter().any(|s| s.abs() > 0.01));
}

#[test]
fn a_synth_that_builds_each_note_for_its_note_alone_is_refused() {
    let mut engine = engine();
    assert_eq!(
        engine.keep_ready("cello", 4),
        Err(Error::CannotKeepReady("cello".to_string()))
    );
    assert_eq!(
        engine.keep_ready("nothing", 4),
        Err(Error::UnknownSynth("nothing".to_string()))
    );
    assert_eq!(engine.kept_ready("cello"), 0);
}

#[test]
fn a_count_of_0_stops_keeping_voices_ready() {
    let mut engine = engine();
    engine.keep_ready("marimba", 3).unwrap();
    assert_eq!(engine.kept_ready("marimba"), 3);
    engine.keep_ready("marimba", 0).unwrap();
    assert_eq!(engine.kept_ready("marimba"), 0);
}
