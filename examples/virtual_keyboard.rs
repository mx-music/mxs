//! Plays a keyboard part into the keys program, on the piano and then the saw, from a virtual MIDI source and records what it plays through BlackHole, so it can be checked with no keyboard and no sound in the room.
//! Writes the recording to played.wav and, to sent.csv, the frame of the recording each message was sent at, with what it was.
//!
//! cargo build --release --features keys && cargo run --release --features keys --example virtual_keyboard -- target/release/keys OUT_DIR [BUFFER]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use midir::MidiOutput;
use midir::os::unix::VirtualOutput;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SOURCE: &str = "mxs virtual keyboard";
const LOOPBACK: &str = "BlackHole 2ch";
const RATE: u32 = 48_000;

/// What the input callback gathers: the left channel, and the moment each buffer's first frame was captured, with its frame.
#[derive(Default)]
struct Recorded {
    samples: Vec<f32>,
    anchors: Vec<(usize, Instant)>,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (keys, out) = (&args[0], std::path::Path::new(&args[1]));
    let buffer = args.get(2).map_or("128", |b| b.as_str());
    std::fs::create_dir_all(out).unwrap();

    let midi = MidiOutput::new(SOURCE).unwrap();
    let mut source = midi.create_virtual(SOURCE).unwrap();

    let refused = Command::new(keys)
        .args(["--input", "a keyboard that is not there"])
        .output()
        .unwrap();
    let refusal = String::from_utf8_lossy(&refused.stderr);
    println!("asked for an input that does not exist: {}", refusal.trim());

    let host = cpal::default_host();
    let device = host
        .input_devices()
        .unwrap()
        .find(|d| {
            d.description()
                .map(|d| d.name() == LOOPBACK)
                .unwrap_or(false)
        })
        .expect("BlackHole 2ch");
    let config = cpal::StreamConfig {
        channels: 2,
        sample_rate: RATE,
        buffer_size: cpal::BufferSize::Default,
    };
    let recorded = Arc::new(Mutex::new(Recorded::default()));
    let gathered = recorded.clone();
    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], info: &cpal::InputCallbackInfo| {
                let stamp = info.timestamp();
                let captured = Instant::now() - stamp.callback.duration_since(stamp.capture);
                let mut r = gathered.lock().unwrap();
                let frame = r.samples.len();
                r.anchors.push((frame, captured));
                r.samples.extend(data.chunks(2).map(|f| f[0]));
            },
            |e| eprintln!("recording: {e}"),
            None,
        )
        .unwrap();
    stream.play().unwrap();

    let play = |synth: &str| {
        Command::new(keys)
            .args([
                "--input", SOURCE, "--device", LOOPBACK, "--synth", synth, "--buffer", buffer,
            ])
            .stdout(Stdio::inherit())
            .spawn()
            .unwrap()
    };
    let mut player = play("piano");
    std::thread::sleep(Duration::from_secs(2));

    let start = Instant::now();
    let mut sent: Vec<(Instant, [u8; 3], &str)> = Vec::new();
    let mut at = |seconds: f64, bytes: [u8; 3], what: &'static str| {
        let when = start + Duration::from_secs_f64(seconds);
        std::thread::sleep(when.saturating_duration_since(Instant::now()));
        source.send(&bytes).unwrap();
        sent.push((Instant::now(), bytes, what));
    };

    // Twenty short notes from silence, for the delay from message to sound.
    for i in 0..20 {
        let t = i as f64 * 0.7;
        at(t, [0x90, 72, 100], "onset");
        at(t + 0.1, [0x80, 72, 0], "off");
    }
    // Three keys at three loudnesses.
    for (i, (key, velocity)) in [(48u8, 30u8), (60, 70), (72, 120)].into_iter().enumerate() {
        let t = 15.0 + i as f64 * 1.5;
        at(t, [0x90, key, velocity], "key");
        at(t + 1.0, [0x80, key, 0], "off");
    }
    // A key let up with the pedal down, held until the pedal comes up, then the same without the pedal.
    at(20.0, [0x90, 57, 90], "pedal key");
    at(20.1, [0xb0, 64, 127], "pedal down");
    at(20.3, [0x80, 57, 0], "key up under pedal");
    at(21.8, [0xb0, 64, 0], "pedal up");
    at(23.5, [0x90, 57, 90], "free key");
    at(23.8, [0x80, 57, 0], "key up");
    // A held A4 on the saw, as the piano's strings cannot bend, bent to each end of the wheel and back.
    at(24.5, [0xb0, 120, 0], "piano done");
    player.kill().unwrap();
    player = play("saw");
    at(25.5, [0x90, 69, 90], "bent key");
    at(26.0, [0xe0, 0x7f, 0x7f], "wheel up");
    at(26.6, [0xe0, 0x00, 0x00], "wheel down");
    at(27.2, [0xe0, 0x00, 0x40], "wheel centre");
    at(27.8, [0x80, 69, 0], "off");
    std::thread::sleep(Duration::from_secs(2));

    player.kill().unwrap();
    drop(stream);
    let r = recorded.lock().unwrap();
    let frame_of = |when: Instant| {
        let &(frame, captured) = r
            .anchors
            .iter()
            .rev()
            .find(|(_, c)| *c <= when)
            .unwrap_or(&r.anchors[0]);
        frame as f64 + when.duration_since(captured).as_secs_f64() * RATE as f64
    };
    let lines: Vec<String> = sent
        .iter()
        .map(|&(when, b, what)| format!("{:.1},{},{},{},{what}", frame_of(when), b[0], b[1], b[2]))
        .collect();
    std::fs::write(out.join("sent.csv"), lines.join("\n") + "\n").unwrap();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(out.join("played.wav"), spec).unwrap();
    r.samples.iter().for_each(|&x| wav.write_sample(x).unwrap());
    wav.finalize().unwrap();
    println!(
        "recorded {:.1} s and {} messages to {}",
        r.samples.len() as f64 / RATE as f64,
        sent.len(),
        out.display()
    );
}
