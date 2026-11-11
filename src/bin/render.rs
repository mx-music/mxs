//! Renders one note to a mono 48 kHz float WAV: render <synth> <seconds> <out.wav> [name=value ...]

use mx_engine::{render, Opts, RATE};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [synth, seconds, out, rest @ ..] = args.as_slice() else {
        eprintln!("usage: render <synth> <seconds> <out.wav> [name=value ...]");
        std::process::exit(2);
    };
    let opts = Opts::parse(rest.iter().map(String::as_str)).unwrap_or_else(|e| fail(&e));
    let seconds: f64 = seconds.parse().unwrap_or_else(|_| fail("seconds must be a number"));
    let samples = render(synth, &opts, seconds).unwrap_or_else(|| fail(&format!("no synth named {synth}")));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(out, spec).unwrap_or_else(|e| fail(&e.to_string()));
    for s in samples {
        wav.write_sample(s).unwrap();
    }
    wav.finalize().unwrap();
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
