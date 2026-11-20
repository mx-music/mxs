//! A note of every synth at its defaults renders the samples it rendered before, kept as a hash of each in `tests/defaults.txt`; a change meant to alter one rewrites it with `MXS_DEFAULTS=write cargo test --test defaults`.
//! Floats can round differently on another processor, and the winds differently in a release build, so the hashes are of a debug build, as plain `cargo test` makes, on Apple silicon alone.

#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use mxs::{Engine, Event, RATE, synths};

/// The 64-bit FNV-1a hash of `samples`' bits, which stays the same from one Rust release to the next.
fn fnv(samples: impl Iterator<Item = f32>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for sample in samples {
        for byte in sample.to_bits().to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

fn hashes() -> String {
    let mut out = String::new();
    for synth in synths::all() {
        let name = synth.name().to_string();
        let mut engine = Engine::offline(RATE);
        synths::all().into_iter().for_each(|s| engine.register(s));
        engine
            .play(&[(0.0, Event::note(None, &name, &[]))])
            .unwrap();
        let (left, right) = engine.render(2.0).unwrap();
        let hash = fnv(left.into_iter().chain(right));
        out.push_str(&format!("{name} {hash:016x}\n"));
    }
    out
}

#[test]
fn a_note_of_every_synth_at_its_defaults_renders_the_samples_it_rendered_before() {
    let path = "tests/defaults.txt";
    let rendered = hashes();
    if std::env::var("MXS_DEFAULTS").as_deref() == Ok("write") {
        std::fs::write(path, &rendered).unwrap();
    }
    let kept = std::fs::read_to_string(path).unwrap_or_default();
    let changed: Vec<&str> = rendered
        .lines()
        .filter(|line| !kept.lines().any(|k| k == *line))
        .collect();
    assert!(
        changed.is_empty(),
        "these render other samples than before: {changed:?}; if that is meant, run MXS_DEFAULTS=write cargo test --test defaults"
    );
}
