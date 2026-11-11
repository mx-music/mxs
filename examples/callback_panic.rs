//! Panics in cpal's audio callback on purpose and reports whether the process survives it: callback_panic [caught]
//! With caught, the callback's body runs inside catch_unwind and the stream goes silent once it panics.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let device = cpal::default_host().default_output_device().expect("no output device");
    let config: cpal::StreamConfig = device.default_output_config().expect("no output config").into();
    let caught = std::env::args().nth(1).as_deref() == Some("caught");
    let mut calls = 0;
    let mut dead = false;
    let stream = device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| {
                let mut fill = |data: &mut [f32]| {
                    data.fill(0.0);
                    calls += 1;
                    if calls == 10 {
                        panic!("on purpose in the audio callback");
                    }
                };
                if !caught {
                    fill(data);
                } else if dead || std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fill(data))).is_err() {
                    dead = true;
                    data.fill(0.0);
                }
            },
            |e| eprintln!("stream error: {e}"),
            None,
        )
        .unwrap();
    stream.play().unwrap();
    for tenth in 1..=10 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        println!("main thread alive at {:.1} s", tenth as f64 / 10.0);
    }
    println!("process survived");
}
