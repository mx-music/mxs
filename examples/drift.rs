//! Plays silence live for the minutes given, 120 by default, printing each minute how far the sound card's clock has drifted from the system's.

use mxs::Engine;
use std::time::{Duration, SystemTime};

fn main() {
    let minutes: u64 = std::env::args()
        .nth(1)
        .and_then(|m| m.parse().ok())
        .unwrap_or(120);
    let engine = Engine::live().unwrap_or_else(|e| panic!("{e}"));
    let offset = || {
        let (seconds, system) = engine.clock();
        system
            .unwrap()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            - seconds
    };
    std::thread::sleep(Duration::from_secs(5));
    let first = offset();
    let latency = engine.latency().unwrap_or(0.0) * 1000.0;
    println!("{} Hz, {latency:.2} ms of latency", engine.rate());
    for minute in 1..=minutes {
        std::thread::sleep(Duration::from_secs(60));
        let drift = (offset() - first) * 1000.0;
        let ppm = drift / 1000.0 / (minute as f64 * 60.0) * 1e6;
        println!(
            "minute {minute}: the system clock has gained {drift:+.3} ms on the sound card's since the start, {ppm:+.1} parts per million"
        );
    }
}
