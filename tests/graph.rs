use fundsp::prelude::*;
use mxs::graph::{Clause, Graph};
use mxs::nodes::{decay2, dust, hz, modes, pulses, shape, struck, struck_gliding, white_noise};
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

#[test]
fn a_traced_saw_square_triangle_and_pulse_start_at_phase_0() {
    let wave = |name: &str| {
        let freq = call("hz", vec![opt("note")]);
        let sound = Graph::call(name, vec![], vec![("hz", freq)]);
        let traced = Synth::graph(
            name,
            note_specs(),
            vec![Clause {
                matches: vec![],
                graph: rings(sound, 0.8),
            }],
        )
        .unwrap();
        render(traced, &[("note", Value::Number(45.0))])
    };
    let written = |mut unit: Box<dyn AudioUnit>| {
        unit.set(Setting::phase(0.0));
        unit.reset();
        let synth = Synth::new("written")
            .options(note_specs())
            .build(move |o, _| {
                let mut unit = unit.clone();
                unit.reset();
                Note::rings(dc(hz(o.number("note")) as f32) >> Net::wrap(unit), 0.8)
            });
        render(synth, &[("note", Value::Number(45.0))])
    };
    for (name, unit) in [
        ("saw", Box::new(saw()) as Box<dyn AudioUnit>),
        ("square", Box::new(square())),
        ("triangle", Box::new(triangle())),
    ] {
        assert_eq!(wave(name), written(unit), "{name}");
    }
    let pulse = Synth::new("written").options(note_specs()).build(|o, _| {
        let mut wave = pulse();
        wave.set(Setting::phase(0.0));
        wave.reset();
        let freq = dc(hz(o.number("note")) as f32);
        Note::rings((freq | dc(0.5)) >> Net::wrap(Box::new(wave)), 0.8)
    });
    let pulse = render(pulse, &[("note", Value::Number(45.0))]);
    assert_eq!(wave("pulse"), pulse, "pulse");
}

/// The traced mode through a chain of effects, each with its options, as both sides.
fn through_chain(effects: Vec<Effect>, chain: &[(&str, Vec<(&str, f64)>)]) -> (Vec<f32>, Vec<f32>) {
    let mut engine = Engine::offline(RATE);
    engine.register(
        Synth::graph(
            "traced",
            note_specs(),
            vec![Clause {
                matches: vec![],
                graph: rings(struck_mode(1.0, n(0.3)), 0.8),
            }],
        )
        .unwrap(),
    );
    for effect in effects {
        engine.register_effect(effect);
    }
    let chain: Vec<(String, Vec<(String, Value)>)> = chain
        .iter()
        .map(|(name, options)| {
            let options = options
                .iter()
                .map(|(k, v)| (k.to_string(), Value::Number(*v)))
                .collect();
            (name.to_string(), options)
        })
        .collect();
    let chain = engine.start_effects(&chain).unwrap();
    engine
        .play(&[(
            0.0,
            Event::Note {
                id: None,
                synth: "traced".to_string(),
                options: vec![],
                chain: Some(chain),
                held: false,
            },
        )])
        .unwrap();
    engine.render(1.0).unwrap()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0f32, |m, x| m.max(x.abs()))
}

fn named(name: &str, args: Vec<Graph>, named: Vec<(&str, Graph)>) -> Graph {
    Graph::call(name, args, named)
}

/// An effect that keeps the left side and silences the right.
fn lefty() -> Effect {
    Effect::graph(
        "lefty",
        vec![],
        call("sides", vec![call("left", vec![]), n(0.0)]),
    )
    .unwrap()
}

#[test]
fn an_effect_over_both_sides_reaches_one_side_from_the_other() {
    let swap = Effect::graph(
        "swap",
        vec![],
        call("sides", vec![call("right", vec![]), call("left", vec![])]),
    )
    .unwrap();
    let (left, right) = through_chain(vec![lefty()], &[("lefty", vec![])]);
    assert!(peak(&left) > 0.01 && peak(&right) == 0.0);
    let (left, right) = through_chain(vec![lefty(), swap], &[("lefty", vec![]), ("swap", vec![])]);
    assert!(peak(&right) > 0.01 && peak(&left) == 0.0);
}

#[test]
fn an_effect_over_both_sides_adds_them_in_the_order_written() {
    let side = Effect::graph(
        "side",
        vec![],
        call(
            "sides",
            vec![
                call("sub", vec![call("left", vec![]), call("right", vec![])]),
                call("sub", vec![call("right", vec![]), call("left", vec![])]),
            ],
        ),
    )
    .unwrap();
    let (dry, _) = through_chain(vec![lefty()], &[("lefty", vec![])]);
    let (left, right) = through_chain(vec![lefty(), side], &[("lefty", vec![]), ("side", vec![])]);
    for ((dry, left), right) in dry.iter().zip(&left).zip(&right) {
        assert!((left - dry).abs() < 1e-6 && (right + dry).abs() < 1e-6);
    }
}

#[test]
fn sides_and_inputs_in_the_wrong_place_are_refused_when_the_effect_is_registered() {
    let refused = |graph: Graph| Effect::graph("wrong", vec![], graph).err().unwrap();
    assert!(
        refused(call(
            "sides",
            vec![call("input", vec![]), call("left", vec![])]
        ))
        .contains("takes left() and right(), not input()")
    );
    assert!(refused(call("left", vec![])).contains("inside sides"));
    assert!(refused(call("sides", vec![call("left", vec![])])).contains("sides takes the left"));
    let synth = Synth::graph(
        "wrong",
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph: rings(call("left", vec![]), 0.5),
        }],
    );
    assert!(synth.err().unwrap().contains("a synth has no input"));
}

#[test]
fn an_effect_says_how_long_it_sounds_on_with_tail() {
    let pass = || call("input", vec![]);
    let decay = Spec::beats("decay").default(4.0);
    let effects = vec![
        Effect::graph("plain", vec![], pass()).unwrap(),
        Effect::graph(
            "long",
            vec![],
            named("tail", vec![pass()], vec![("seconds", n(3.0))]),
        )
        .unwrap(),
        Effect::graph(
            "ringing",
            vec![decay],
            named("tail", vec![pass()], vec![("option", name("decay"))]),
        )
        .unwrap(),
    ];
    let mut engine = Engine::offline(RATE);
    for effect in effects {
        engine.register_effect(effect);
    }
    let chain: Vec<(String, Vec<(String, Value)>)> = ["plain", "long", "ringing"]
        .iter()
        .map(|name| (name.to_string(), vec![]))
        .collect();
    assert_eq!(
        engine.effect_tails(&chain, Some(120.0)).unwrap(),
        vec![1.0, 3.0, 2.0]
    );

    let refused = |tail: Vec<(&str, Graph)>| {
        Effect::graph("wrong", vec![], named("tail", vec![pass()], tail))
            .err()
            .unwrap()
    };
    assert!(
        refused(vec![("option", name("decay"))])
            .contains("tail names decay, which is not an option")
    );
    assert!(refused(vec![("beats", n(1.0))]).contains("tail does not take beats:"));
}

#[test]
fn a_node_given_an_argument_it_does_not_take_is_refused_when_registered() {
    let allpass = named(
        "allpass",
        vec![call("input", vec![])],
        vec![("seconds", n(1.0))],
    );
    assert_eq!(
        Effect::graph("wrong", vec![], allpass).err().unwrap(),
        "wrong: allpass does not take seconds:"
    );
    let sweep = named("sweep", vec![], vec![("wave", n(1.0))]);
    assert_eq!(
        Effect::graph("wrong", vec![], sweep).err().unwrap(),
        "wrong: sweep needs length:"
    );
}

#[test]
fn a_sweep_starts_where_its_offset_says_on_each_side() {
    let sweep = |offset: f64| {
        named(
            "sweep",
            vec![],
            vec![("length", n(0.5)), ("wave", n(3.0)), ("offset", n(offset))],
        )
    };
    let effect = Effect::graph(
        "sweeps",
        vec![],
        call("sides", vec![sweep(0.0), sweep(0.5)]),
    )
    .unwrap();
    let (left, right) = through_chain(vec![effect], &[("sweeps", vec![])]);
    for (left, right) in left.iter().zip(&right) {
        assert!((left + right - 1.0).abs() < 1e-4);
    }
    let quarter = (0.125 * RATE) as usize;
    assert!(left[quarter] > 0.999 && right[quarter] < 0.001);
}

#[test]
fn a_follower_rises_and_falls_within_its_times() {
    let tone = named(
        "mul",
        vec![
            named("sine", vec![], vec![("hz", n(200.0))]),
            call("lt", vec![call("time", vec![]), n(0.5)]),
        ],
        vec![],
    );
    let level = named(
        "follower",
        vec![tone],
        vec![("rise", n(0.02)), ("fall", n(0.1))],
    );
    let effect = Effect::graph("level", vec![], level).unwrap();
    let (left, _) = through_chain(vec![effect], &[("level", vec![])]);
    let at = |seconds: f64| left[(seconds * RATE) as usize];
    assert!(at(0.02) >= 0.9, "{}", at(0.02));
    assert!(at(0.4) > 0.9 && at(0.4) <= 1.0);
    assert!(at(0.6) < 0.1, "{}", at(0.6));
    assert!(at(0.55) > 0.2, "{}", at(0.55));
}

fn list(xs: &[f64]) -> Graph {
    Graph::List(xs.iter().map(|&x| n(x)).collect())
}

fn noise(seed: Option<f64>) -> Graph {
    let named = seed.map(|s| ("seed", n(s))).into_iter().collect();
    Graph::call("noise", vec![Graph::Name("white".to_string())], named)
}

/// noise(:white) |> mode(hz: [...], level: [...], fade: [...])
fn bank(fades: &[f64]) -> Graph {
    Graph::call(
        "mode",
        vec![noise(None)],
        vec![
            ("hz", list(&[440.0, 1210.0, 2630.0])),
            ("level", list(&[0.3, 0.2, 0.1])),
            ("fade", list(fades)),
        ],
    )
}

fn traced(name: &str, graph: Graph) -> Result<Synth, String> {
    Synth::graph(
        name,
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph: rings(graph, 0.8),
        }],
    )
}

#[test]
fn a_traced_mode_given_lists_strikes_every_mode_with_one_noise_as_modes_in_rust_does() {
    let traced = traced("bank", bank(&[0.4, 0.3, 0.2])).unwrap();
    let written = Synth::new("bank").options(note_specs()).build(|_, ctx| {
        let noise = white_noise(ctx.seed());
        let bank = modes(
            noise,
            &[440.0, 1210.0, 2630.0],
            &[0.3, 0.2, 0.1],
            &[0.4, 0.3, 0.2],
        );
        Note::rings(bank, 0.8)
    });
    let (a, b) = (render(traced, &[]), render(written, &[]));
    assert!(a.iter().any(|x| x.abs() > 0.01));
    assert_eq!(a, b);
}

#[test]
fn a_mode_given_lists_of_different_lengths_is_refused_when_the_synth_is_registered() {
    assert_eq!(
        traced("bank", bank(&[0.4, 0.3])).err(),
        Some("bank: mode's hz:, level: and fade: lists differ in length".to_string())
    );
}

#[test]
fn a_noise_given_a_seed_plays_that_seed_and_leaves_the_note_s_seeds_to_the_rest() {
    let both = Graph::call("add", vec![noise(Some(7.0)), noise(None)], vec![]);
    let traced = traced("seeded", both).unwrap();
    let written = Synth::new("seeded")
        .options(note_specs())
        .build(|_, ctx| Note::rings(white_noise(7) + white_noise(ctx.seed()), 0.8));
    let (a, b) = (render(traced, &[]), render(written, &[]));
    assert!(a.iter().any(|x| x.abs() > 0.01));
    assert_eq!(a, b);
}

/// One side of the saw through an effect of `graph`, with its options given.
fn through_graph(specs: Vec<Spec>, graph: Graph, given: Vec<(&str, f64)>) -> (Vec<f32>, Vec<f32>) {
    let effect = Effect::graph("probe", specs, graph).unwrap();
    through_chain(vec![effect], &[("probe", given)])
}

#[test]
fn an_effect_whose_input_is_multiplied_by_0_or_left_out_of_a_select_still_plays_the_rest() {
    let input = || call("input", vec![]);
    let (dry, _) = through_graph(vec![], input(), vec![]);
    assert!(peak(&dry) > 0.01);

    let silenced = call("add", vec![call("mul", vec![input(), n(0.0)]), input()]);
    assert_eq!(through_graph(vec![], silenced, vec![]).0, dry);

    let wet = || vec![Spec::float("wet").default(0.0)];
    let mixed = || call("add", vec![call("mul", vec![input(), opt("wet")]), input()]);
    assert_eq!(through_graph(wet(), mixed(), vec![]).0, dry);
    let doubled = through_graph(wet(), mixed(), vec![("wet", 1.0)]).0;
    assert!((peak(&doubled) - 2.0 * peak(&dry)).abs() < 1e-5);

    let on = || vec![Spec::float("on").default(1.0)];
    let picked = || {
        call(
            "add",
            vec![
                call(
                    "select",
                    vec![
                        opt("on"),
                        input(),
                        call("mul", vec![call("add", vec![input(), input()]), n(0.25)]),
                    ],
                ),
                input(),
            ],
        )
    };
    let whole = through_graph(on(), picked(), vec![]).0;
    assert!((peak(&whole) - 2.0 * peak(&dry)).abs() < 1e-5);
    let half = through_graph(on(), picked(), vec![("on", 0.0)]).0;
    assert!((peak(&half) - 1.5 * peak(&dry)).abs() < 1e-5);
}

#[test]
fn an_effect_over_both_sides_whose_side_is_multiplied_by_0_plays_the_other() {
    let crossed = call(
        "sides",
        vec![
            call(
                "add",
                vec![
                    call("mul", vec![call("left", vec![]), n(0.0)]),
                    call("right", vec![]),
                ],
            ),
            call("right", vec![]),
        ],
    );
    let swap = call("sides", vec![call("right", vec![]), call("left", vec![])]);
    let swap = Effect::graph("swap", vec![], swap).unwrap();
    let crossed = Effect::graph("crossed", vec![], crossed).unwrap();
    let chain = [("lefty", vec![]), ("swap", vec![]), ("crossed", vec![])];
    let (left, right) = through_chain(vec![lefty(), swap, crossed], &chain);
    assert!(peak(&right) > 0.01);
    assert_eq!(left, right);
}

#[test]
fn an_effect_that_cannot_be_built_is_an_error_naming_it_when_its_chain_starts() {
    let broken = named(
        "delay",
        vec![call("input", vec![])],
        vec![("seconds", n(0.1)), ("max", call("input", vec![]))],
    );
    let effect = Effect::graph("broken", vec![], broken).unwrap();
    let mut engine = Engine::offline(RATE);
    engine.register_effect(effect);
    let error = engine
        .start_effects(&[("broken".to_string(), vec![])])
        .unwrap_err()
        .to_string();
    assert_eq!(
        error,
        "effect broken could not be built: a delay's max must be known when the note starts"
    );
    assert!(engine.start_effects(&[]).is_ok());
}

#[test]
fn a_shared_sound_is_built_once_and_plays_the_same_noise_at_each_use_as_a_clone_in_rust_does() {
    let input = || call("input", vec![]);
    let body = Graph::call(
        "add",
        vec![
            Graph::call(
                "mode",
                vec![input()],
                vec![
                    ("hz", list(&[440.0, 1210.0])),
                    ("level", list(&[0.3, 0.2])),
                    ("fade", list(&[0.4, 0.3])),
                ],
            ),
            Graph::call("highpass", vec![input()], vec![("hz", n(3000.0))]),
        ],
        vec![],
    );
    let shared = Graph::call("share", vec![noise(None)], vec![("through", body)]);
    let traced = traced("shared", shared).unwrap();
    let written = Synth::new("shared").options(note_specs()).build(|_, ctx| {
        let noise = white_noise(ctx.seed());
        let rung = modes(noise.clone(), &[440.0, 1210.0], &[0.3, 0.2], &[0.4, 0.3]);
        let high = noise
            >> Net::wrap(Box::new(highpass_hz(
                3000.0f32,
                std::f32::consts::FRAC_1_SQRT_2,
            )));
        Note::rings(rung + high, 0.8)
    });
    let (a, b) = (render(traced, &[]), render(written, &[]));
    assert!(a.iter().any(|x| x.abs() > 0.01));
    for (k, (x, y)) in a.iter().zip(&b).enumerate() {
        assert!((x - y).abs() < 1e-5, "frame {k}: {x} against {y}");
    }
}

fn one_pole(name: &str, extra: Vec<(&str, Graph)>) -> Graph {
    let mut named = vec![("hz", n(1000.0)), ("poles", n(1.0))];
    named.extend(extra);
    Graph::call(name, vec![noise(None)], named)
}

#[test]
fn a_lowpass_and_a_highpass_of_one_pole_sound_as_fundsp_s_lowpole_and_highpole() {
    for (name, filter) in [
        ("lowpass", Net::wrap(Box::new(lowpole_hz(1000.0f32)))),
        ("highpass", Net::wrap(Box::new(highpole_hz(1000.0f32)))),
    ] {
        let traced = traced("pole", one_pole(name, vec![])).unwrap();
        let written = Synth::new("pole")
            .options(note_specs())
            .build(move |_, ctx| Note::rings(white_noise(ctx.seed()) >> filter.clone(), 0.8));
        let (a, b) = (render(traced, &[]), render(written, &[]));
        assert!(a.iter().any(|x| x.abs() > 0.01), "{name}");
        for (k, (x, y)) in a.iter().zip(&b).enumerate() {
            assert!((x - y).abs() < 1e-5, "{name}, frame {k}: {x} against {y}");
        }
    }
}

#[test]
fn a_one_pole_filter_given_a_q_is_refused_when_the_synth_is_registered() {
    assert_eq!(
        traced("pole", one_pole("lowpass", vec![("q", n(2.0))])).err(),
        Some("pole: lowpass of one pole takes no q:".to_string())
    );
}

#[test]
fn mod_wraps_a_number_and_a_sound_into_zero_up_to_its_divisor() {
    let synth = |name: &str, sound: Graph| {
        Synth::graph(
            name,
            note_specs(),
            vec![Clause {
                matches: vec![],
                graph: rings(sound, 0.5),
            }],
        )
        .unwrap()
    };
    let impulse = || call("impulse", vec![]);
    let wrapped = call("mul", vec![impulse(), call("mod", vec![n(-0.25), n(1.0)])]);
    let plain = call("mul", vec![impulse(), n(0.75)]);
    assert_eq!(
        render(synth("wrapped", wrapped), &[]),
        render(synth("plain", plain), &[])
    );

    let ramp = render(
        synth("ramp", call("mod", vec![call("time", vec![]), n(0.1)])),
        &[],
    );
    let period = (0.1 * RATE) as usize;
    let at = |seconds: f64| ramp[(seconds * RATE) as usize];
    assert!((at(0.05) / at(0.09) - 0.05 / 0.09).abs() < 1e-3);
    assert!(at(0.2 + 0.5 / RATE).abs() < at(0.09).abs() * 1e-2);
    assert!((ramp[period + 100] - ramp[2 * period + 100]).abs() < 1e-4);
}

#[test]
fn time_given_every_starts_over_at_that_period_on_the_same_sample_as_a_voice_in_rust() {
    let graph = Graph::call("time", vec![], vec![("every", n(0.125))]);
    let traced = Synth::graph(
        "every",
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph: rings(graph, 0.5),
        }],
    )
    .unwrap();
    let rust = Synth::new("every").build(|_, _| Note::rings(shape(|t| t % 0.125), 0.5));
    assert_eq!(render(traced, &[]), render(rust, &[]));

    let refused = Synth::graph(
        "often",
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph: rings(Graph::call("time", vec![], vec![("often", n(0.1))]), 0.5),
        }],
    );
    assert_eq!(refused.err().unwrap(), "often: time does not take often:");
}

fn plain(name: &str, graph: Graph) -> Result<Synth, String> {
    Synth::graph(
        name,
        note_specs(),
        vec![Clause {
            matches: vec![],
            graph: rings(graph, 0.5),
        }],
    )
}

#[test]
fn clicks_and_struck_play_as_dust_and_decay2_do_in_rust() {
    let clicks = || Graph::call("clicks", vec![], vec![("rate", n(400.0)), ("seed", n(7.0))]);
    let rust = Synth::new("clicks").build(|_, _| Note::rings(dc(400.0) >> dust(7), 0.5));
    let traced = render(plain("clicks", clicks()).unwrap(), &[]);
    assert!(traced.iter().filter(|x| **x != 0.0).count() > 100);
    assert_eq!(traced, render(rust, &[]));

    let struck_clicks = Graph::call(
        "struck",
        vec![clicks()],
        vec![("attack", n(0.0001)), ("fall", n(0.0008))],
    );
    let rust = Synth::new("struck")
        .build(|_, _| Note::rings((dc(400.0) >> dust(7)) >> decay2(0.0001, 0.0008), 0.5));
    assert_eq!(
        render(plain("struck", struck_clicks).unwrap(), &[]),
        render(rust, &[])
    );

    let refused = plain(
        "unfallen",
        Graph::call("struck", vec![clicks()], vec![("attack", n(0.001))]),
    );
    assert_eq!(refused.err().unwrap(), "unfallen: struck needs fall:");
}

#[test]
fn mode_given_a_sound_in_hz_glides_as_struck_gliding_does_in_rust() {
    let pitch = || {
        call(
            "add",
            vec![call("mul", vec![call("time", vec![]), n(100.0)]), n(220.0)],
        )
    };
    let gliding = Graph::call(
        "mode",
        vec![call("impulse", vec![])],
        vec![
            (
                "hz",
                Graph::List(
                    [1.0, 2.0, 3.0]
                        .iter()
                        .map(|&k| call("mul", vec![pitch(), n(k)]))
                        .collect(),
                ),
            ),
            ("level", Graph::List(vec![n(1.0), n(0.0), n(0.3)])),
            ("fade", Graph::List(vec![n(0.4), n(0.3), n(0.2)])),
        ],
    );
    let rust = Synth::new("gliding").build(|_, _| {
        let pitch = || shape(|t| t) * 100.0 + 220.0;
        let one =
            |k: f32, level: f64, fade: f64| (pass() | pitch() * k) >> struck_gliding(level, fade);
        Note::rings(
            impulse::<U1>() >> (one(1.0, 1.0, 0.4) & one(3.0, 0.3, 0.2)),
            0.5,
        )
    });
    let traced = render(plain("gliding", gliding).unwrap(), &[]);
    assert!(traced.iter().any(|x| x.abs() > 0.01));
    assert_eq!(traced, render(rust, &[]));

    let alone = Graph::call(
        "mode",
        vec![call("impulse", vec![])],
        vec![("hz", pitch()), ("level", n(1.0)), ("fade", n(0.4))],
    );
    let rust = Synth::new("alone").build(|_, _| {
        Note::rings(
            impulse::<U1>()
                >> ((pass() | shape(|t| t) * 100.0 + 220.0) >> struck_gliding(1.0, 0.4)),
            0.5,
        )
    });
    assert_eq!(
        render(plain("alone", alone).unwrap(), &[]),
        render(rust, &[])
    );
}

#[test]
fn impulse_given_at_period_and_until_plucks_as_the_tanpuras_pulses_do() {
    let train = Graph::call(
        "impulse",
        vec![],
        vec![("at", n(0.1)), ("period", n(0.15)), ("until", n(0.45))],
    );
    let traced = render(plain("train", train).unwrap(), &[]);
    let fired: Vec<usize> = (0..traced.len()).filter(|&k| traced[k] != 0.0).collect();
    assert_eq!(fired.len(), 3);
    assert_eq!(fired[0], (0.1 * RATE) as usize);
    let rust = Synth::new("train").build(|_, _| Note::rings(pulses(0.1, 0.15, 0.45), 0.5));
    assert_eq!(traced, render(rust, &[]));
}
