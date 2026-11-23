//! A phrase rendered offline the same way natively and in WebAssembly, so `tests/webassembly.rs` can compare the two.
//! The phrase plays the piano, the marimba and a sample loaded from the bytes of a WAV file, through a reverb.
//! Built without the `tables` feature, mxs has no tables compiled in, and the phrase plays only once the tables of [`TABLES`] are loaded.

use mxs::{Engine, Event, Value, effects, synths};
use std::cell::RefCell;
use std::io::Cursor;

/// The phrase's length in seconds.
pub const SECONDS: usize = 4;

/// The tables the phrase's synths play.
pub const TABLES: [&str; 2] = ["piano.mxf", "piano_pedal.mxf"];

/// The bytes of a WAV file of a decaying 330 Hz tone, a quarter of a second long.
fn tone() -> Vec<u8> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = Cursor::new(Vec::new());
    let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
    for i in 0..12_000 {
        let t = i as f64 / 48_000.0;
        let x = (std::f64::consts::TAU * 330.0 * t).sin() * (-12.0 * t).exp();
        writer.write_sample((x * 16_000.0) as i16).unwrap();
    }
    writer.finalize().unwrap();
    bytes.into_inner()
}

/// The level of each tenth of a second of the phrase, in decibels.
pub fn levels() -> Vec<f64> {
    rendered()
        .iter()
        .map(|(left, right)| {
            let sum: f64 = left.iter().chain(right).map(|x| (*x as f64).powi(2)).sum();
            10.0 * (sum / (left.len() + right.len()) as f64 + 1e-20).log10()
        })
        .collect()
}

/// The FNV-1a hash of every sample of the phrase, left then right in each tenth of a second.
pub fn hash() -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for (left, right) in rendered() {
        for sample in left.iter().chain(&right) {
            for byte in sample.to_bits().to_le_bytes() {
                hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
            }
        }
    }
    hash
}

/// Each tenth of a second of the phrase, left and right.
fn rendered() -> Vec<(Vec<f32>, Vec<f32>)> {
    let mut engine = Engine::offline(48_000.0);
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    let reverb = (
        "reverb".to_string(),
        vec![("decay_seconds".to_string(), Value::Number(2.0))],
    );
    let chain = engine.start_effects(&[reverb]).unwrap();
    let sample = engine.load_sample_bytes("tone.wav", &tone()).unwrap();
    let into = |event: Event| match event {
        Event::Note {
            id,
            synth,
            options,
            held,
            ..
        } => Event::Note {
            id,
            synth,
            options,
            chain: Some(chain),
            held,
        },
        other => other,
    };
    let mut events = Vec::new();
    for (i, note) in [60.0, 64.0, 67.0, 72.0].into_iter().enumerate() {
        let at = i as f64 * 0.5;
        events.push((
            at,
            into(Event::note(
                None,
                "piano",
                &[("note", note), ("sustain", 0.4)],
            )),
        ));
        events.push((
            at + 0.25,
            into(Event::note(None, "marimba", &[("note", note + 12.0)])),
        ));
    }
    events.push((
        2.5,
        Event::Sample {
            id: None,
            sample,
            options: vec![],
            chain: Some(chain),
        },
    ));
    engine.play(&events).unwrap();
    (0..SECONDS * 10)
        .map(|_| engine.render(0.1).unwrap())
        .collect()
}

thread_local! {
    static OUT: RefCell<Vec<f64>> = const { RefCell::new(Vec::new()) };
    static PANIC: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static ROOM: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Room for `length` bytes, where the page writes a table's file name and then its bytes before calling [`load_table`].
#[unsafe(no_mangle)]
pub extern "C" fn room(length: usize) -> *mut u8 {
    ROOM.with(|room| {
        let mut room = room.borrow_mut();
        *room = vec![0; length];
        room.as_mut_ptr()
    })
}

/// Loads the table whose file is named by the first `name` bytes of the room, from the bytes after them; 0 when it loads.
#[unsafe(no_mangle)]
pub extern "C" fn load_table(name: usize) -> u32 {
    ROOM.with(|room| {
        let room = room.borrow();
        let file = std::str::from_utf8(&room[..name]).unwrap_or("");
        mxs::synths::load_table(file, &room[name..]).map_or(1, |()| 0)
    })
}

/// Renders the phrase and returns where its levels start; there are `SECONDS * 10` of them.
#[unsafe(no_mangle)]
pub extern "C" fn render() -> *const f64 {
    std::panic::set_hook(Box::new(|info| {
        PANIC.with(|message| *message.borrow_mut() = info.to_string().into_bytes())
    }));
    OUT.with(|out| {
        *out.borrow_mut() = levels();
        out.borrow().as_ptr()
    })
}

/// The message of a panic, which WebAssembly otherwise reports only as unreachable code: its length in four bytes, then the text.
#[unsafe(no_mangle)]
pub extern "C" fn panic_message() -> *const u8 {
    PANIC.with(|message| {
        let mut message = message.borrow_mut();
        let text = std::mem::take(&mut *message);
        message.extend_from_slice(&(text.len() as u32).to_le_bytes());
        message.extend_from_slice(&text);
        message.as_ptr()
    })
}
