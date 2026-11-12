use mxs::{Engine, Event, RATE, check_sample, synths};
use std::path::{Path, PathBuf};

const FRAMES: usize = 4800;

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mxs-samples-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A ramp from -0.5 to 0.5 over a tenth of a second, each frame as `encode` writes it.
fn ramp(encode: impl Fn(f64) -> Vec<u8>) -> Vec<u8> {
    (0..FRAMES).flat_map(|i| encode(i as f64 / FRAMES as f64 - 0.5)).collect()
}

fn chunk(id: &[u8; 4], body: &[u8], big_endian: bool) -> Vec<u8> {
    let size = body.len() as u32;
    let mut out = id.to_vec();
    out.extend(if big_endian { size.to_be_bytes() } else { size.to_le_bytes() });
    out.extend(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

fn riff(fmt: &[u8], data: &[u8]) -> Vec<u8> {
    let body = [b"WAVE".to_vec(), chunk(b"fmt ", fmt, false), chunk(b"data", data, false)].concat();
    [b"RIFF".to_vec(), (body.len() as u32).to_le_bytes().to_vec(), body].concat()
}

/// A WAV format chunk of `size` bytes: the format, one channel at 48 kHz, and samples of `bytes` each.
fn fmt(tag: u16, bytes: u16, size: usize) -> Vec<u8> {
    let mut out = [tag.to_le_bytes(), 1u16.to_le_bytes()].concat();
    out.extend(48_000u32.to_le_bytes());
    out.extend((48_000 * bytes as u32).to_le_bytes());
    out.extend(bytes.to_le_bytes());
    out.extend((8 * bytes).to_le_bytes());
    out.resize(size, 0);
    out
}

fn extended(rate: f64) -> [u8; 10] {
    let exponent = rate.log2().floor() as i32;
    let mantissa = (rate * 2f64.powi(63 - exponent)) as u64;
    let mut out = [0u8; 10];
    out[..2].copy_from_slice(&((exponent + 16383) as u16).to_be_bytes());
    out[2..].copy_from_slice(&mantissa.to_be_bytes());
    out
}

fn form(kind: &[u8; 4], bits: u16, compression: Option<&[u8; 4]>, data: &[u8]) -> Vec<u8> {
    let mut comm = [1u16.to_be_bytes().to_vec(), (FRAMES as u32).to_be_bytes().to_vec(), bits.to_be_bytes().to_vec(), extended(RATE).to_vec()].concat();
    if let Some(c) = compression {
        comm.extend(c);
        comm.extend([0, 0]);
    }
    let ssnd = [vec![0; 8], data.to_vec()].concat();
    let body = [kind.to_vec(), chunk(b"COMM", &comm, true), chunk(b"SSND", &ssnd, true)].concat();
    [b"FORM".to_vec(), (body.len() as u32).to_be_bytes().to_vec(), body].concat()
}

fn int(x: f64, bits: u32) -> i64 {
    (x * (1i64 << (bits - 1)) as f64).round() as i64
}

/// The left channel of a sample played at its own rate from its start.
fn played(path: &Path) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let sample = engine.load_sample(path).unwrap();
    engine.play(&[(0.0, Event::Sample { id: None, sample, options: vec![], chain: None })]).unwrap();
    engine.render(0.1).unwrap().0
}

fn written(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn wav_and_aiff_files_of_every_common_layout_play_as_the_same_samples_in_32_bit_float_do() {
    let dir = dir("layouts");
    let reference = played(&written(&dir, "float.wav", &riff(&fmt(3, 4, 16), &ramp(|x| (x as f32).to_le_bytes().to_vec()))));
    let files = [
        ("pcm24-fmt20.wav", riff(&fmt(1, 3, 20), &ramp(|x| int(x, 24).to_le_bytes()[..3].to_vec()))),
        ("pcm16-fmt18.wav", riff(&fmt(1, 2, 18), &ramp(|x| (int(x, 16) as i16).to_le_bytes().to_vec()))),
        ("pcm32.wav", riff(&fmt(1, 4, 16), &ramp(|x| (int(x, 32) as i32).to_le_bytes().to_vec()))),
        ("pcm8.wav", riff(&fmt(1, 1, 16), &ramp(|x| vec![(int(x, 8) + 128) as u8]))),
        ("float64.wav", riff(&fmt(3, 8, 16), &ramp(|x| x.to_le_bytes().to_vec()))),
        ("extensible.wav", {
            let mut f = fmt(0xfffe, 3, 40);
            f[24..26].copy_from_slice(&1u16.to_le_bytes());
            riff(&f, &ramp(|x| int(x, 24).to_le_bytes()[..3].to_vec()))
        }),
        ("pcm8.aiff", form(b"AIFF", 8, None, &ramp(|x| vec![int(x, 8) as i8 as u8]))),
        ("pcm16.aiff", form(b"AIFF", 16, None, &ramp(|x| (int(x, 16) as i16).to_be_bytes().to_vec()))),
        ("pcm24.aiff", form(b"AIFF", 24, None, &ramp(|x| int(x, 24).to_be_bytes()[5..].to_vec()))),
        ("sowt.aifc", form(b"AIFC", 16, Some(b"sowt"), &ramp(|x| (int(x, 16) as i16).to_le_bytes().to_vec()))),
        ("fl32.aifc", form(b"AIFC", 32, Some(b"fl32"), &ramp(|x| (x as f32).to_be_bytes().to_vec()))),
    ];
    for (name, bytes) in files {
        let path = written(&dir, name, &bytes);
        assert_eq!(check_sample(&path), Ok(()), "{name}");
        let tolerance = if name.starts_with("pcm8") { 1e-2 } else { 1e-4 };
        let worst = played(&path).iter().zip(&reference).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max);
        assert!(worst < tolerance, "{name} is {worst} off");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_file_that_is_not_audio_is_refused_by_its_contents_whatever_its_name() {
    let dir = dir("refused");
    let path = written(&dir, "kick.wav", b"not a sound at all, only text\n");
    let refused = check_sample(&path).unwrap_err();
    assert_eq!(refused, "it is not a FLAC, WAV or AIFF file");
    let mut engine = Engine::offline(RATE);
    assert!(engine.load_sample(&path).is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}
