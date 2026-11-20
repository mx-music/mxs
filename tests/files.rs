use mxs::graph::{Clause, Graph};
use mxs::{Effect, Engine, Event, RATE, Spec, Synth, Value};

const MARIMBA: &str = include_str!("../notes/instruments-as-data/marimba.json");

fn n(x: f64) -> Graph {
    Graph::Number(x)
}

fn opt(name: &str) -> Graph {
    Graph::Option(name.to_string())
}

fn call(name: &str, args: Vec<Graph>) -> Graph {
    Graph::call(name, args, vec![])
}

fn two(name: &str, a: Graph, b: Graph) -> Graph {
    call(name, vec![a, b])
}

fn mallet(hz: Graph, corner: f64, slope: Graph) -> Graph {
    let above = |over: f64| {
        two(
            "max",
            call("log2", vec![two("div", hz.clone(), n(over))]),
            n(0.0),
        )
    };
    two(
        "add",
        two("mul", slope, above(corner)),
        two("mul", n(24.0), above(3000.0)),
    )
}

/// mx's marimba sketch built in Rust, as marimba.py in the notes of mxs-00012 writes it to its file.
fn marimba() -> Synth {
    let (note, vel) = (opt("note"), opt("vel"));
    let freq = call("hz", vec![note.clone()]);
    let freq_times = |k: f64| two("mul", freq.clone(), n(k));
    let octaves = |from: f64, per: f64| {
        two(
            "pow",
            n(2.0),
            two("div", two("sub", note.clone(), n(from)), n(per)),
        )
    };
    let ring = two(
        "mul",
        two("mul", n(4.6), octaves(36.0, -17.0)),
        two(
            "pow",
            n(2.0),
            two(
                "div",
                two("max", two("sub", note.clone(), n(78.0)), n(0.0)),
                n(-16.0),
            ),
        ),
    );
    let swell = two("mul", n(0.45), octaves(36.0, -18.0));
    let fourth = two(
        "sub",
        two(
            "sub",
            two(
                "sub",
                n(-12.0),
                mallet(
                    freq_times(4.0),
                    600.0,
                    two("mul", n(57.0), two("sub", n(1.0), vel.clone())),
                ),
            ),
            two(
                "mul",
                n(10.0),
                two(
                    "max",
                    call("log2", vec![two("div", freq_times(4.0), n(900.0))]),
                    n(0.0),
                ),
            ),
        ),
        two(
            "mul",
            n(20.0),
            two(
                "max",
                call("log2", vec![two("div", n(370.0), freq_times(4.0))]),
                n(0.0),
            ),
        ),
    );
    let overtone = |k: f64| {
        two(
            "sub",
            two(
                "add",
                n(10.0),
                two("mul", n(14.5), call("log2", vec![vel.clone()])),
            ),
            mallet(
                freq_times(k),
                1100.0,
                two("sub", n(33.4), two("mul", n(11.4), vel.clone())),
            ),
        )
    };
    let bar = vec![
        (4.0, fourth, 0.3),
        (10.0, overtone(10.0), 0.1),
        (19.0, two("sub", overtone(19.0), n(8.0)), 0.08),
        (30.0, two("sub", overtone(30.0), n(12.0)), 0.06),
    ];
    let twelfth = call(
        "select",
        vec![
            two("lt", note.clone(), n(46.0)),
            n(-82.0),
            call(
                "select",
                vec![two("gt", note.clone(), n(62.0)), n(-82.0), n(-22.0)],
            ),
        ],
    );
    let tube_slope = two(
        "add",
        n(12.0),
        two("mul", n(45.0), two("sub", n(1.0), vel.clone())),
    );
    let tube = [
        (3.0, twelfth),
        (5.0, n(-26.0)),
        (7.0, n(-26.0)),
        (9.0, n(-26.0)),
    ]
    .into_iter()
    .map(|(k, level)| {
        (
            k,
            two(
                "sub",
                level,
                mallet(freq_times(k), 600.0, tube_slope.clone()),
            ),
            0.35,
        )
    });
    let strike = call("impulse", vec![]);
    let mode = |hz: Graph, level: Graph, fade: Graph| {
        Graph::call(
            "mode",
            vec![strike.clone()],
            vec![("hz", hz), ("level", level), ("fade", fade)],
        )
    };
    let mut modes = vec![
        mode(freq.clone(), n(1.25), ring.clone()),
        mode(freq.clone(), n(-1.0), swell),
    ];
    for (k, level, fade) in bar.into_iter().chain(tube) {
        let level = call(
            "select",
            vec![
                two("lt", freq_times(k), n(15_000.0)),
                call("db", vec![level]),
                n(0.0),
            ],
        );
        let fade = two(
            "mul",
            two("mul", ring.clone(), n(fade)),
            two(
                "pow",
                two("min", two("div", n(1500.0), freq_times(k)), n(1.0)),
                n(0.8),
            ),
        );
        modes.push(mode(freq_times(k), level, fade));
    }
    let struck = call("mix", vec![Graph::List(modes)]);
    let band = |low: f64, poles: f64, high: f64, release: f64, level: f64| {
        let noise = call("noise", vec![Graph::Name("white".to_string())]);
        let sound = Graph::call(
            "lowpass",
            vec![noise],
            vec![("hz", n(low)), ("poles", n(poles))],
        );
        let sound = Graph::call("highpass", vec![sound], vec![("hz", n(high))]);
        let struck = Graph::call(
            "perc",
            vec![sound],
            vec![("attack", n(0.0005)), ("release", n(release))],
        );
        two("gain", struck, n(level))
    };
    let tap = two(
        "add",
        band(2200.0, 6.0, 1000.0, 0.12, 0.55),
        band(4000.0, 4.0, 2500.0, 0.05, 0.3),
    );
    let by_vel = |power: f64| two("pow", two("div", vel.clone(), n(0.6)), n(power));
    let sound = two(
        "gain",
        two("add", struck, two("mul", tap, by_vel(0.5))),
        two("mul", n(0.1), by_vel(1.9)),
    );
    let graph = Graph::call(
        "rings",
        vec![call("note_envelope", vec![sound])],
        vec![("seconds", two("div", two("mul", ring, n(4.0)), n(3.0)))],
    );
    let unbounded = |spec: Spec| spec.range(0.0, f64::INFINITY);
    let specs = vec![
        Spec::note("note").default(52.0).doc("The pitch."),
        Spec::float("vel")
            .default(0.6)
            .range(0.05, 1.0)
            .doc("How hard the bar is struck."),
        unbounded(Spec::seconds("attack").default(0.0)),
        unbounded(Spec::seconds("decay").default(0.0)),
        unbounded(Spec::seconds("sustain").default(1000.0)),
        unbounded(Spec::seconds("release").default(0.3)),
        Spec::float("attack_level").default(1.0),
        Spec::float("decay_level").default(-1.0),
        Spec::float("sustain_level").default(1.0),
    ];
    Synth::graph(
        "marimba",
        specs,
        vec![Clause {
            matches: vec![],
            graph,
        }],
    )
    .unwrap()
}

fn render(synth: Synth, note: f64, vel: f64) -> (Vec<f32>, Vec<f32>) {
    let mut engine = Engine::offline(RATE);
    let name = synth.name().to_string();
    engine.register(synth);
    let options = [("note", note), ("vel", vel)];
    engine
        .play(&[(0.0, Event::note(None, &name, &options))])
        .unwrap();
    engine.render(3.0).unwrap()
}

#[test]
fn the_marimba_file_renders_the_samples_of_the_same_graph_built_in_rust() {
    for (note, vel) in [(36.0, 0.3), (52.0, 0.55), (68.0, 0.8), (84.0, 1.0)] {
        let read = render(Synth::from_json(MARIMBA).unwrap(), note, vel);
        let built = render(marimba(), note, vel);
        assert!(
            read.0.iter().any(|s| s.abs() > 0.001),
            "note {note} is silent"
        );
        assert!(
            read == built,
            "note {note} at vel {vel} renders other samples"
        );
    }
}

#[test]
fn the_marimba_file_loads_from_its_path_with_the_options_it_declares() {
    let synth = Synth::from_json_file("notes/instruments-as-data/marimba.json").unwrap();
    assert_eq!(synth.name(), "marimba");
    let vel = synth.specs().iter().find(|s| s.name == "vel").unwrap();
    assert_eq!(vel.range, Some((0.05, 1.0)));
    assert_eq!(vel.doc, "How hard the bar is struck.");
    let release = synth.specs().iter().find(|s| s.name == "release").unwrap();
    assert_eq!(release.range, Some((0.0, f64::INFINITY)));
}

fn file(body: &str) -> String {
    format!(
        r#"{{"format": "mxs-synth", "version": 1, "name": "plate", "options": [{{"name": "note", "kind": "note", "default": 60}}], {body}}}"#
    )
}

fn refused(text: &str) -> String {
    match Synth::from_json(text) {
        Ok(_) => panic!("the file was read: {text}"),
        Err(reason) => reason,
    }
}

const RINGS: &str = r#"{"call": "rings", "args": [{"call": "mode", "args": [{"call": "impulse"}], "named": {"hz": {"ref": "freq"}, "level": 0.2, "fade": 1}}], "named": {"seconds": 1}}"#;

#[test]
fn a_file_of_a_newer_version_is_refused_naming_its_version_and_the_newest_this_reads() {
    let newer = file(&format!(r#""clauses": [{{"graph": {RINGS}}}]"#))
        .replace(r#""version": 1"#, r#""version": 2"#);
    let reason = refused(&newer);
    assert!(
        reason.contains("version 2") && reason.contains("version 1"),
        "{reason}"
    );
}

#[test]
fn a_ref_to_a_name_the_file_does_not_define_is_refused_naming_it() {
    let reason = refused(&file(&format!(r#""clauses": [{{"graph": {RINGS}}}]"#)));
    assert!(reason.contains("ref freq names no def"), "{reason}");
    assert!(
        reason.contains("clauses[0].graph.args[0].named.hz"),
        "{reason}"
    );
}

#[test]
fn a_def_that_refers_to_itself_is_refused() {
    let looped = file(&format!(
        r#""defs": {{"freq": {{"call": "add", "args": [{{"ref": "freq"}}, 1]}}}}, "clauses": [{{"graph": {RINGS}}}]"#
    ));
    assert!(refused(&looped).contains("defs.freq refers to itself"));
}

#[test]
fn a_lowpass_without_hz_is_refused_with_the_path_to_the_call() {
    let graph = r#"{"call": "rings", "args": [{"call": "lowpass", "args": [{"call": "noise", "args": [{"name": "white"}]}]}], "named": {"seconds": 1}}"#;
    let reason = refused(&file(&format!(r#""clauses": [{{"graph": {graph}}}]"#)));
    assert_eq!(reason, "clauses[0].graph.args[0]: lowpass needs hz");
}

#[test]
fn a_file_missing_a_comma_is_refused_with_its_line_and_column() {
    let text = "{\n  \"format\": \"mxs-synth\"\n  \"version\": 1\n}";
    let reason = refused(text);
    assert!(
        reason.starts_with("not JSON") && reason.contains("line 3 column 3"),
        "{reason}"
    );
}

#[test]
fn a_call_outside_the_vocabulary_is_refused_with_its_path() {
    let graph = r#"{"call": "rings", "args": [{"call": "wobble"}], "named": {"seconds": 1}}"#;
    let reason = refused(&file(&format!(r#""clauses": [{{"graph": {graph}}}]"#)));
    assert_eq!(
        reason,
        "clauses[0].graph.args[0]: wobble is not in the vocabulary"
    );
}

#[test]
fn an_effect_file_is_not_read_as_a_synth() {
    let reason = refused(r#"{"format": "mxs-effect", "version": 1, "name": "dark", "graph": 0}"#);
    assert!(reason.contains("mxs-effect"), "{reason}");
}

fn through(effect: Effect) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    engine.register(Synth::from_json(MARIMBA).unwrap());
    let name = effect.name().to_string();
    engine.register_effect(effect);
    let chain = engine.start_effects(&[(name, vec![])]).unwrap();
    engine
        .play(&[(
            0.0,
            Event::Note {
                id: None,
                synth: "marimba".to_string(),
                options: vec![("note".to_string(), Value::Number(72.0))],
                chain: Some(chain),
                held: false,
            },
        )])
        .unwrap();
    engine.render(1.0).unwrap().0
}

#[test]
fn an_effect_file_of_a_lowpass_filters_a_chain_as_the_same_effect_built_in_rust() {
    let text = r#"{"format": "mxs-effect", "version": 1, "name": "dark",
        "options": [{"name": "hz", "kind": "hz", "default": 400, "range": [20, 20000]}],
        "graph": {"call": "lowpass", "args": [{"call": "input"}], "named": {"hz": {"option": "hz"}}}}"#;
    let built = Effect::graph(
        "dark",
        vec![Spec::hz("hz").default(400.0).range(20.0, 20000.0)],
        Graph::call(
            "lowpass",
            vec![call("input", vec![])],
            vec![("hz", opt("hz"))],
        ),
    )
    .unwrap();
    let read = through(Effect::from_json(text).unwrap());
    assert!(read.iter().any(|s| s.abs() > 0.001));
    assert!(read == through(built));
    let dry = render(Synth::from_json(MARIMBA).unwrap(), 72.0, 0.6).0;
    let energy = |s: &[f32]| s.iter().map(|x| x * x).sum::<f32>();
    assert!(energy(&read) < energy(&dry[..read.len()]) * 0.9);
}

/// The 64-bit FNV-1a hash of `samples`' bits, as tests/defaults.rs and the Python package's tests take it.
#[cfg(all(target_os = "macos", target_arch = "aarch64", debug_assertions))]
fn fnv(samples: impl Iterator<Item = f32>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for sample in samples {
        for byte in sample.to_bits().to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

// The Python package's tests check its render of the marimba file against this hash, of a debug build on Apple silicon, as tests/defaults.rs's are.
#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64", debug_assertions))]
fn the_marimba_file_renders_middle_c_to_the_samples_the_python_package_checks() {
    let (left, right) = render(Synth::from_json(MARIMBA).unwrap(), 60.0, 0.6);
    let second = RATE as usize;
    let hash = fnv(left[..second].iter().chain(&right[..second]).copied());
    assert_eq!(format!("{hash:016x}"), "3406ffda23b0932d");
}
