//! Renders one note of a synth to a stereo 48 kHz float WAV: `render <synth> <seconds> <out.wav> [name=value ...]`

use mxs::{Engine, Event, RATE, Value, synths};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [synth, seconds, out, rest @ ..] = args.as_slice() else {
        eprintln!("usage: render <synth> <seconds> <out.wav> [name=value ...]");
        std::process::exit(2);
    };
    let seconds: f64 = seconds
        .parse()
        .unwrap_or_else(|_| fail("seconds must be a number"));
    let options = rest
        .iter()
        .map(|arg| {
            let (name, value) = arg
                .split_once('=')
                .unwrap_or_else(|| fail(&format!("expected name=value, got {arg}")));
            let value = value
                .parse()
                .map(Value::Number)
                .unwrap_or_else(|_| Value::Name(value.to_string()));
            (name.to_string(), value)
        })
        .collect();
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let note = Event::Note {
        id: None,
        synth: synth.clone(),
        options,
        chain: None,
        held: false,
    };
    engine
        .play(&[(0.0, note)])
        .unwrap_or_else(|e| fail(&e.to_string()));
    let (left, right) = engine
        .render(seconds)
        .unwrap_or_else(|e| fail(&e.to_string()));
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(out, spec).unwrap_or_else(|e| fail(&e.to_string()));
    for (l, r) in left.into_iter().zip(right) {
        wav.write_sample(l).unwrap();
        wav.write_sample(r).unwrap();
    }
    wav.finalize().unwrap();
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
