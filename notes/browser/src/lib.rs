//! mxs as WebAssembly for the prototype page: one engine, notes of any synth at seconds, and blocks rendered for an audio worklet to play.
//! It takes plain numbers and returns a pointer into its own memory, so it needs no bindings generator and no imports.

use mxs::{Engine, Event, synths};
use std::cell::RefCell;

struct State {
    engine: Engine,
    block: Vec<f32>,
    name: Vec<u8>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static PANIC: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// The message of the last panic, which WebAssembly otherwise reports only as unreachable code, as a length in four bytes and then the text.
#[unsafe(no_mangle)]
pub extern "C" fn panic_message() -> *const u8 {
    PANIC.with(|message| {
        let mut message = message.borrow_mut();
        let text = String::from_utf8_lossy(&message).into_owned();
        message.clear();
        message.extend_from_slice(&(text.len() as u32).to_le_bytes());
        message.extend_from_slice(text.as_bytes());
        message.as_ptr()
    })
}

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|state| f(state.borrow_mut().as_mut().expect("start is called first")))
}

/// Starts an engine at `rate` with mxs's synths, and a limit of `voices` sounding at once.
#[unsafe(no_mangle)]
pub extern "C" fn start(rate: f64, voices: usize) {
    std::panic::set_hook(Box::new(|info| PANIC.with(|message| *message.borrow_mut() = info.to_string().into_bytes())));
    let mut engine = Engine::offline(rate);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.set_voice_limit(voices);
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            engine,
            block: Vec::new(),
            name: Vec::new(),
        })
    });
}

/// Room for a synth's name of `len` bytes, which the next note plays.
#[unsafe(no_mangle)]
pub extern "C" fn synth_name(len: usize) -> *mut u8 {
    with(|state| {
        state.name.resize(len, 0);
        state.name.as_mut_ptr()
    })
}

/// Plays a note of the named synth at `seconds`, with its sustain in seconds; 0 when it plays, and 1 when the engine refuses it.
#[unsafe(no_mangle)]
pub extern "C" fn note(seconds: f64, note: f64, vel: f64, sustain: f64) -> u32 {
    with(|state| {
        let synth = String::from_utf8_lossy(&state.name).into_owned();
        let event = Event::note(
            None,
            &synth,
            &[("note", note), ("vel", vel), ("sustain", sustain)],
        );
        state.engine.play(&[(seconds, event)]).map_or(1, |_| 0)
    })
}

/// Renders the next `frames` frames, the left channel and then the right.
#[unsafe(no_mangle)]
pub extern "C" fn render(frames: usize) -> *const f32 {
    with(|state| {
        let rate = state.engine.rate();
        state.block.clear();
        state.block.resize(frames * 2, 0.0);
        if let Ok((left, right)) = state.engine.render(frames as f64 / rate) {
            let count = left.len().min(frames);
            state.block[..count].copy_from_slice(&left[..count]);
            state.block[frames..frames + count].copy_from_slice(&right[..count]);
        }
        state.block.as_ptr()
    })
}
