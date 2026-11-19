use fundsp::prelude::*;
use mxs::graph::{Clause, Graph};
use mxs::nodes::{hz, struck};
use mxs::{Effect, Engine, Event, Note, RATE, Spec, Synth, Value};

fn n(x: f64) -> Graph {
    Graph::Number(x)
}

fn opt(name: &str) -> Graph {
    Graph::Option(name.to_string())
}

fn call(name: &str, args: Vec<Graph>) -> Graph {
    Graph::call(name, args, vec![])
}

fn render(synth: Synth, options: &[(&str, Value)]) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    let name = synth.name().to_string();
    engine.register(synth);
    let options = options
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    engine
        .play(&[(
            0.0,
            Event::Note {
                id: None,
                synth: name,
                options,
                chain: None,
                held: false,
            },
        )])
        .unwrap();
    engine.render(1.0).unwrap().0
}

fn note_specs() -> Vec<Spec> {
    vec![
        Spec::note("note").default(60.0),
        Spec::float("vel").default(0.6).range(0.0, 1.0),
    ]
}

/// impulse |> mode(hz: hz(note) * ratio, level: level, fade: 0.5)
fn struck_mode(ratio: f64, level: Graph) -> Graph {
    let freq = call("mul", vec![call("hz", vec![opt("note")]), n(ratio)]);
    Graph::call(
        "mode",
        vec![call("impulse", vec![])],
        vec![("hz", freq), ("level", level), ("fade", n(0.5))],
    )
}

fn rings(sound: Graph, seconds: f64) -> Graph {
    Graph::call("rings", vec![sound], vec![("seconds", n(seconds))])
}

#[test]
fn a_traced_mode_sounds_as_the_same_mode_written_in_rust() {
    let level = call("mul", vec![opt("vel"), n(0.5)]);
    let traced = Synth::graph(
        "traced",
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph: rings(struck_mode(1.0, level), 0.8),
        }],
    )
    .unwrap();
    let written = Synth::new("written").options(note_specs()).build(|o, _| {
        let impulse = Net::wrap(Box::new(impulse::<U1>()));
        Note::rings(
            impulse >> struck(hz(o.number("note")), o.number("vel") * 0.5, 0.5),
            0.8,
        )
    });
    let options = [("note", Value::Number(64.0)), ("vel", Value::Number(0.8))];
    let (a, b) = (render(traced, &options), render(written, &options));
    assert!(a.iter().any(|x| x.abs() > 0.01));
    assert_eq!(a, b);
}

#[test]
fn a_mode_whose_level_selects_0_is_left_out() {
    let above = |ratio: f64| {
        let freq = call("mul", vec![call("hz", vec![opt("note")]), n(ratio)]);
        struck_mode(
            ratio,
            call(
                "select",
                vec![call("lt", vec![freq, n(15_000.0)]), n(0.3), n(0.0)],
            ),
        )
    };
    let synths = || {
        let both = call(
            "mix",
            vec![Graph::List(vec![struck_mode(1.0, n(0.5)), above(40.0)])],
        );
        let with_select = Synth::graph(
            "both",
            note_specs(),
            vec![Clause {
                matches: vec![],
                graph: rings(both, 0.8),
            }],
        )
        .unwrap();
        let alone = Synth::graph(
            "alone",
            note_specs(),
            vec![Clause {
                matches: vec![],
                graph: rings(struck_mode(1.0, n(0.5)), 0.8),
            }],
        )
        .unwrap();
        (with_select, alone)
    };
    let high = [("note", Value::Number(84.0))];
    let (with_select, alone) = synths();
    assert_eq!(render(with_select, &high), render(alone, &high));
    let low = [("note", Value::Number(36.0))];
    let (with_select, alone) = synths();
    assert_ne!(render(with_select, &low), render(alone, &low));
}

#[test]
fn a_note_plays_the_clause_its_options_match() {
    let specs = vec![
        Spec::note("note").default(60.0),
        Spec::name("bol", &["na", "tun"]).default("na"),
    ];
    let clause = |bol: &str, ratio: f64| Clause {
        matches: vec![("bol".to_string(), Value::Name(bol.to_string()))],
        graph: rings(struck_mode(ratio, n(0.5)), 0.8),
    };
    let tabla = || {
        Synth::graph(
            "tabla",
            specs.clone(),
            vec![clause("na", 1.0), clause("tun", 2.0)],
        )
        .unwrap()
    };
    let na = render(tabla(), &[("bol", Value::Name("na".to_string()))]);
    let tun = render(tabla(), &[("bol", Value::Name("tun".to_string()))]);
    let by_default = render(tabla(), &[]);
    assert_ne!(na, tun);
    assert_eq!(na, by_default);
}

#[test]
fn a_note_envelope_shapes_a_traced_sound_as_it_shapes_one_written_in_rust() {
    let specs = || {
        let mut specs = note_specs();
        specs.extend(Spec::envelope(0.3, 0.2));
        specs
    };
    let sine = Graph::call("sine", vec![], vec![("hz", call("hz", vec![opt("note")]))]);
    let graph = call("note_envelope", vec![call("gain", vec![sine, opt("vel")])]);
    let traced = Synth::graph(
        "traced",
        specs(),
        vec![Clause {
            matches: vec![],
            graph,
        }],
    )
    .unwrap();
    let written = Synth::new("written").options(specs()).build(|o, _| {
        let freq = Net::wrap(Box::new(dc(hz(o.number("note")) as f32)));
        let mut sine = Net::wrap(Box::new(An(Sine::<f32>::with_phase(0.0))));
        sine.reset();
        Note::envelope((freq >> sine) * o.number("vel") as f32, o)
    });
    let options = [
        ("note", Value::Number(69.0)),
        ("vel", Value::Number(0.5)),
        ("attack", Value::Number(0.05)),
    ];
    let (a, b) = (render(traced, &options), render(written, &options));
    assert!(a.iter().any(|x| x.abs() > 0.1));
    for (k, (x, y)) in a.iter().zip(&b).enumerate() {
        assert!((x - y).abs() < 1e-5, "frame {k}: {x} against {y}");
    }
}

#[test]
fn a_call_outside_the_vocabulary_is_refused_when_the_synth_is_registered() {
    let graph = rings(call("wobble", vec![n(1.0)]), 1.0);
    let refused = Synth::graph(
        "odd",
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph,
        }],
    );
    assert_eq!(
        refused.err(),
        Some("odd: wobble is not in the vocabulary".to_string())
    );
}

#[test]
fn an_option_the_note_leaves_out_takes_its_fallback() {
    let specs = vec![Spec::note("note")];
    let level = call("or_else", vec![opt("note"), n(63.0)]);
    let synth = || {
        Synth::graph(
            "drum",
            specs.clone(),
            vec![Clause {
                matches: vec![],
                graph: rings(
                    call("mul", vec![call("impulse", vec![]), level.clone()]),
                    0.1,
                ),
            }],
        )
        .unwrap()
    };
    assert_eq!(
        render(synth(), &[])[0],
        63.0 * std::f32::consts::FRAC_1_SQRT_2
    );
    assert_eq!(
        render(synth(), &[("note", Value::Number(40.0))])[0],
        40.0 * std::f32::consts::FRAC_1_SQRT_2
    );
}

fn through(effect: Option<Effect>) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    let level = call("mul", vec![opt("vel"), n(0.5)]);
    engine.register(
        Synth::graph(
            "traced",
            note_specs(),
            vec![Clause {
                matches: vec![],
                graph: rings(struck_mode(1.0, level), 0.8),
            }],
        )
        .unwrap(),
    );
    let chain = effect.map(|effect| {
        let name = effect.name().to_string();
        engine.register_effect(effect);
        engine.start_effects(&[(name, vec![])]).unwrap()
    });
    engine
        .play(&[(
            0.0,
            Event::Note {
                id: None,
                synth: "traced".to_string(),
                options: vec![],
                chain,
                held: false,
            },
        )])
        .unwrap();
    engine.render(1.0).unwrap().0
}

#[test]
fn an_effect_traced_as_a_graph_shapes_each_channel_coming_in() {
    let dry = through(None);
    let halved = through(Some(
        Effect::graph(
            "halve",
            vec![],
            call("mul", vec![call("input", vec![]), n(0.5)]),
        )
        .unwrap(),
    ));
    let doubled = through(Some(
        Effect::graph(
            "double",
            vec![],
            call("add", vec![call("input", vec![]), call("input", vec![])]),
        )
        .unwrap(),
    ));
    let peak = |s: &[f32]| s.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(peak(&dry) > 0.01);
    assert!((peak(&halved) / peak(&dry) - 0.5).abs() < 1e-3);
    assert!((peak(&doubled) / peak(&dry) - 2.0).abs() < 1e-3);
}

#[test]
fn a_synth_that_takes_an_input_is_refused_when_registered() {
    let graph = rings(call("input", vec![]), 0.5);
    let refused = Synth::graph(
        "listens",
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph,
        }],
    );
    assert!(matches!(refused, Err(reason) if reason.contains("a synth has no input")));
}

#[test]
fn a_deeply_nested_graph_plays_on_a_thread_with_a_small_stack() {
    let level = (0..2000).fold(n(0.5), |inner, _| call("max", vec![inner, n(0.0)]));
    let graph = rings(struck_mode(1.0, level), 0.8);
    let sound = std::thread::Builder::new()
        .stack_size(1024 * 1024)
        .spawn(move || {
            let clauses = vec![Clause {
                matches: vec![],
                graph,
            }];
            render(Synth::graph("deep", note_specs(), clauses).unwrap(), &[])
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(sound.iter().any(|x| x.abs() > 0.01));
}

fn shape_specs() -> Vec<Spec> {
    vec![Spec::name("shape", &["saw", "sine"]).default("sine")]
}

fn picking(by: Graph) -> Vec<Clause> {
    let graph = rings(
        call(
            "select",
            vec![
                by,
                Graph::call("sine", vec![], vec![("hz", n(100.0))]),
                n(0.0),
            ],
        ),
        1.0,
    );
    vec![Clause {
        matches: vec![],
        graph,
    }]
}

fn name(n: &str) -> Graph {
    Graph::Name(n.to_string())
}

#[test]
fn eq_and_ne_compare_an_option_that_takes_names_with_a_name() {
    let picked = |by: fn() -> Graph, shape: &str| {
        let synth = Synth::graph("picky", shape_specs(), picking(by())).unwrap();
        let samples = render(synth, &[("shape", Value::Name(shape.to_string()))]);
        samples.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    };
    let is_saw = || call("eq", vec![opt("shape"), name("saw")]);
    let not_saw = || call("ne", vec![name("saw"), opt("shape")]);
    assert!(picked(is_saw, "saw") > 0.1);
    assert_eq!(picked(is_saw, "sine"), 0.0);
    assert_eq!(picked(not_saw, "saw"), 0.0);
    assert!(picked(not_saw, "sine") > 0.1);
}

#[test]
fn a_name_an_option_does_not_take_is_refused_when_the_synth_is_registered() {
    let refused = Synth::graph(
        "picky",
        shape_specs(),
        picking(call("eq", vec![opt("shape"), name("square")])),
    );
    assert_eq!(
        refused.err(),
        Some("picky: shape takes saw or sine, not square".to_string())
    );
}

#[test]
fn other_arithmetic_on_an_option_that_takes_names_is_refused_when_the_synth_is_registered() {
    let refused = Synth::graph(
        "picky",
        shape_specs(),
        picking(call("add", vec![opt("shape"), n(1.0)])),
    );
    assert_eq!(
        refused.err(),
        Some("picky: shape takes names, which only eq and ne compare with a name".to_string())
    );
}
