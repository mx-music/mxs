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

const MANIFEST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/webassembly/Cargo.toml");
/// Where the check builds without the tables, apart from its build with them.
const WITHOUT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/webassembly/target/without-tables"
);

fn cargo(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
        .env_remove("RUSTFLAGS")
        .args(args)
        .args(["--manifest-path", MANIFEST]);
    command
}

#[test]
#[ignore]
fn the_phrase_renders_the_same_samples_with_its_tables_loaded_at_run_time_as_compiled_in() {
    let with = run(cargo(&["run", "--release", "--quiet"]).args(["--", "hash"]));
    let without = run(cargo(&[
        "run",
        "--release",
        "--quiet",
        "--no-default-features",
        "--target-dir",
        WITHOUT,
    ])
    .args(["--", "hash"]));
    assert_eq!(with, without);
}

#[test]
#[ignore]
fn without_the_tables_the_module_is_under_3_mb_refuses_the_piano_and_plays_the_tables_handed_in() {
    run(&mut cargo(&[
        "build",
        "--release",
        "--target",
        "wasm32-unknown-unknown",
        "--no-default-features",
        "--target-dir",
        WITHOUT,
    ]));
    let module = format!("{WITHOUT}/wasm32-unknown-unknown/release/mxs_webassembly_check.wasm");
    let size = std::fs::metadata(&module).unwrap().len();
    assert!(size < 3_000_000, "the module is {size} bytes");

    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/webassembly/check.mjs");
    let refused = Command::new("node")
        .args([script, &module])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&refused.stdout);
    assert!(
        !refused.status.success() && said.contains("TablesNotLoaded { synth: \"piano\""),
        "{said}"
    );

    let figures = concat!(env!("CARGO_MANIFEST_DIR"), "/src/synths/figures");
    let tables = ["piano.mxf", "piano_pedal.mxf", "marimba.mxf"];
    let wasm = levels(&run(Command::new("node")
        .args([script, &module, figures])
        .args(tables)));
    let native = levels(&run(&mut cargo(&["run", "--release", "--quiet"])));
    assert_eq!(wasm.len(), native.len());
    for (tenth, (w, n)) in wasm.iter().zip(&native).enumerate() {
        assert!(
            (w - n).abs() <= 0.01,
            "at {:.1} s WebAssembly gives {w:.3} dB and native {n:.3} dB",
            tenth as f64 / 10.0
        );
    }
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
