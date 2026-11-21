//! The measured tables of the instruments fitted to recordings, compiled in or loaded at run time.
//! The tests for a build without them run with `cargo test --no-default-features --test tables`.

use mxs::{Engine, Event, synths};

fn engine() -> Engine {
    let mut engine = Engine::offline(48_000.0);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine
}

fn table(file: &str) -> Vec<u8> {
    let path = format!("{}/src/synths/figures/{file}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).unwrap()
}

fn note(synth: &str) -> Vec<(f64, Event)> {
    vec![(0.0, Event::note(None, synth, &[("note", 60.0)]))]
}

fn loudest(engine: &mut Engine) -> f32 {
    let (left, right) = engine.render(0.5).unwrap();
    left.iter().chain(&right).fold(0.0, |m, x| m.max(x.abs()))
}

#[test]
fn the_piano_names_its_two_tables_the_pluck_none_and_fourteen_synths_need_some() {
    let all = synths::all();
    let tables = |name: &str| {
        all.iter()
            .find(|s| s.name() == name)
            .unwrap()
            .tables()
            .to_vec()
    };
    assert_eq!(tables("piano"), ["piano.mxf", "piano_pedal.mxf"]);
    assert!(tables("pluck").is_empty());
    assert_eq!(all.iter().filter(|s| !s.tables().is_empty()).count(), 14);
}

#[test]
fn bytes_that_are_not_a_whole_table_of_mxs_are_refused() {
    let piano = table("piano.mxf");
    assert_eq!(
        synths::load_table("organ.mxf", &piano),
        Err("mxs has no table organ.mxf".to_string())
    );
    assert_eq!(
        synths::load_table("piano.mxf", &piano[..piano.len() / 2]),
        Err("piano.mxf: figures cut short, or with bytes after them".to_string())
    );
    assert_eq!(
        synths::load_table("piano.mxf", b"RIFF and more"),
        Err("piano.mxf: not figures of struck or held notes".to_string())
    );
}

// A build without the tables keeps what it loads, so only the test below loads any there.
#[cfg(feature = "tables")]
#[test]
fn every_table_reads_whole_from_its_file() {
    for synth in synths::all() {
        for file in synth.tables() {
            assert_eq!(synths::load_table(file, &table(file)), Ok(()), "{file}");
        }
    }
}

#[cfg(feature = "tables")]
#[test]
fn with_the_tables_compiled_in_the_piano_plays_with_none_loaded() {
    let mut engine = engine();
    engine.play(&note("piano")).unwrap();
    assert!(loudest(&mut engine) > 0.01);
}

#[cfg(not(feature = "tables"))]
#[test]
fn a_piano_note_is_refused_until_both_its_tables_load_and_the_pluck_plays_with_none() {
    let mut engine = engine();
    engine.play(&note("pluck")).unwrap();
    assert!(loudest(&mut engine) > 0.01);

    let refused = engine.play(&note("piano")).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "the piano's tables are not loaded: load piano.mxf and piano_pedal.mxf first"
    );
    synths::load_table("piano.mxf", &table("piano.mxf")).unwrap();
    let refused = engine.play(&note("piano")).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "the piano's tables are not loaded: load piano_pedal.mxf first"
    );
    synths::load_table("piano_pedal.mxf", &table("piano_pedal.mxf")).unwrap();
    engine.play(&note("piano")).unwrap();
    assert!(loudest(&mut engine) > 0.01);
}
