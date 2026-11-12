//! Plays a note through BlackHole every 5 seconds for the minutes given, 120 by default, each stamped in system time half a second ahead, and records BlackHole's input to find when each note's first sample came back.
//! Prints each minute how far the notes landed from their stamps, against the latency the engine reports, so the clock can be checked over hours with no sound in the room.
//!
//! cargo run --release --example clock_check -- [MINUTES]

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mxs::{Engine, Event, Settings, synths};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const LOOPBACK: &str = "BlackHole 2ch";
const RATE: u32 = 48_000;
/// A note's first sample is the first above this after a second of silence.
const THRESHOLD: f32 = 0.001;

fn main() {
    let minutes: u64 = std::env::args()
        .nth(1)
        .and_then(|m| m.parse().ok())
        .unwrap_or(120);
    let mut engine = Engine::live_with(&Settings {
        device: Some(LOOPBACK.into()),
        buffer: Some(128),
        ..Settings::default()
    })
    .unwrap_or_else(|e| panic!("{e}"));
    synths::all().into_iter().for_each(|s| engine.register(s));

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
    let onsets = Arc::new(Mutex::new(Vec::<SystemTime>::new()));
    let found = onsets.clone();
    let mut quiet = 0u32;
    let stream = device
        .build_input_stream(
            config,
            move |data: &[f32], info: &cpal::InputCallbackInfo| {
                let stamp = info.timestamp();
                let captured = SystemTime::now() - stamp.callback.duration_since(stamp.capture);
                for (i, frame) in data.chunks(2).enumerate() {
                    if frame[0].abs() > THRESHOLD {
                        if quiet >= RATE {
                            found
                                .lock()
                                .unwrap()
                                .push(captured + Duration::from_secs_f64(i as f64 / RATE as f64));
                        }
                        quiet = 0;
                    } else {
                        quiet = quiet.saturating_add(1);
                    }
                }
            },
            |e| eprintln!("recording: {e}"),
            None,
        )
        .unwrap();
    stream.play().unwrap();
    std::thread::sleep(Duration::from_secs(2));

    let start = Instant::now();
    let mut stamps: Vec<SystemTime> = Vec::new();
    let mut offsets: Vec<f64> = Vec::new();
    for minute in 1..=minutes {
        for _ in 0..12 {
            let at = SystemTime::now() + Duration::from_millis(500);
            let note = Event::note(
                None,
                "piano",
                &[
                    ("note", 72.0),
                    ("vel", 1.0),
                    ("sustain", 0.1),
                    ("release", 0.05),
                ],
            );
            engine
                .play_at(&[(at, note)])
                .unwrap_or_else(|e| panic!("{e}"));
            stamps.push(at);
            std::thread::sleep(Duration::from_secs(5));
        }
        let heard = onsets.lock().unwrap().clone();
        let fresh: Vec<f64> = stamps
            .drain(..)
            .filter_map(|at| {
                heard
                    .iter()
                    .find(|h| {
                        **h >= at - Duration::from_millis(100)
                            && **h <= at + Duration::from_millis(400)
                    })
                    .map(|h| ms(*h, at))
            })
            .collect();
        offsets.extend(&fresh);
        let (lo, hi) = fresh
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
        let latency = engine.latency().unwrap_or(0.0) * 1000.0;
        let elapsed = start.elapsed().as_secs() / 60;
        println!(
            "minute {elapsed}: {} of 12 notes heard, {lo:.3} to {hi:.3} ms after their stamps; the engine reports {latency:.3} ms of latency",
            fresh.len()
        );
    }
    let (lo, hi) = offsets
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), x| (lo.min(*x), hi.max(*x)));
    println!(
        "over {minutes} minutes, {} notes landed {lo:.3} to {hi:.3} ms after their stamps",
        offsets.len()
    );
}

fn ms(a: SystemTime, b: SystemTime) -> f64 {
    match a.duration_since(b) {
        Ok(d) => d.as_secs_f64() * 1000.0,
        Err(e) => -e.duration().as_secs_f64() * 1000.0,
    }
}
