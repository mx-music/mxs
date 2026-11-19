//! mxs renders offline in WebAssembly as it does natively.
//! It needs the wasm32-unknown-unknown target and Node: `cargo test --release --test webassembly -- --ignored`.

use std::process::Command;

fn run(command: &mut Command) -> String {
    let out = command.output().expect("the command starts");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "{}{}",
        text,
        String::from_utf8_lossy(&out.stderr)
    );
    text
}

fn levels(text: &str) -> Vec<f64> {
    text.lines().map(|line| line.parse().unwrap()).collect()
}

#[test]
#[ignore]
fn a_phrase_renders_in_webassembly_at_the_levels_it_renders_natively() {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/webassembly/Cargo.toml");
    let cargo = || {
        let mut command = Command::new(env!("CARGO"));
        command.env_remove("RUSTFLAGS");
        command
    };
    run(cargo().args([
        "build",
        "--release",
        "--target",
        "wasm32-unknown-unknown",
        "--manifest-path",
        manifest,
    ]));
    let native = levels(&run(cargo().args([
        "run",
        "--release",
        "--quiet",
        "--manifest-path",
        manifest,
    ])));
    let module = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/webassembly/target/wasm32-unknown-unknown/release/mxs_webassembly_check.wasm"
    );
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/webassembly/check.mjs");
    let wasm = levels(&run(Command::new("node").args([script, module])));
    assert_eq!(wasm.len(), native.len());
    // The sample starts at 2.5 s, after the notes have faded.
    assert!(wasm[26] > wasm[24] + 20.0, "{wasm:?}");
    for (tenth, (w, n)) in wasm.iter().zip(&native).enumerate() {
        assert!(
            (w - n).abs() <= 0.01,
            "at {:.1} s WebAssembly gives {w:.3} dB and native {n:.3} dB",
            tenth as f64 / 10.0
        );
    }
}
