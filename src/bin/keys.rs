//! Plays one instrument from a MIDI keyboard through the default output: keys play notes, the sustain pedal holds them and the pitch bend wheel bends them.
//!
//! cargo run --release --features keys --bin keys -- [--list] [--input NAME] [--synth piano] [--buffer 256]

use midir::MidiInput;
use mxs::keys::{Keyboard, choose_input};
use mxs::{Engine, Error, Settings, synths};
use std::sync::mpsc;

fn main() {
    if let Err(reason) = run() {
        eprintln!("{reason}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    let midi = MidiInput::new("mxs keys").map_err(|e| e.to_string())?;
    let ports = midi.ports();
    let names: Vec<String> = ports.iter().map(|p| midi.port_name(p).unwrap_or_default()).collect();
    let instruments = synths::all();
    if args.iter().any(|a| a == "--list") {
        let known = if names.is_empty() { "none".to_string() } else { names.join(", ") };
        let mut playable: Vec<&str> = instruments.iter().map(|s| s.name()).collect();
        playable.sort();
        let playable = playable.join(", ");
        println!("MIDI inputs: {known}");
        println!("Instruments: {playable}");
        return Ok(());
    }
    let name = value("--synth").unwrap_or_else(|| "piano".to_string());
    let synth = instruments.iter().find(|s| s.name() == name).ok_or_else(|| format!("no instrument named {name}; give --list to see them"))?;
    let mut keyboard = Keyboard::new(synth);
    let index = choose_input(&names, value("--input").as_deref())?;
    let buffer = match value("--buffer") {
        Some(frames) => frames.parse().map_err(|_| format!("--buffer takes a number of frames, not {frames}"))?,
        None => 256,
    };
    let mut engine = Engine::live_with(&Settings { buffer: Some(buffer), ..Settings::default() }).map_err(|e| e.to_string())?;
    instruments.into_iter().for_each(|s| engine.register(s));
    let (send, messages) = mpsc::channel::<Vec<u8>>();
    let port = ports[index].clone();
    let _connection = midi
        .connect(&port, "mxs keys", move |_, bytes, _| drop(send.send(bytes.to_vec())), ())
        .map_err(|e| e.to_string())?;
    let input = &names[index];
    let rate = engine.rate();
    println!("Playing {name} from {input} at {rate} Hz, with buffers of {buffer} frames; Ctrl-C stops.");
    for bytes in messages {
        let events = keyboard.message(&bytes, engine.now());
        match engine.play(&events) {
            Ok(()) => {}
            Err(failed @ (Error::Failed | Error::DeviceLost(_))) => return Err(failed.to_string()),
            Err(reason) => eprintln!("{reason}"),
        }
    }
    Ok(())
}
