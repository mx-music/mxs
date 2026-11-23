//! Synths and effects described as data: each a graph of calls on the note's options, which the engine builds into a sound when a note starts, with no compiling.
//! An option is read once, when the note starts; arithmetic on options and numbers is worked out then, and a node whose level works out to 0 is left out.
//!
//! [`Synth::graph`] and [`Effect::graph`] build from a [`Graph`], a tree of calls to the functions listed here.
//! A program that lets its users write synths, in a language of its own or in a file, turns what they wrote into a `Graph`.
//! [The guide to building a synth](crate::guide::building) has a program that does.
//!
//! # The vocabulary
//!
//! A call takes arguments by position and by name.
//! Below, `sound` is an argument by position that is a sound or a number, and an argument written `name:` is given by name.
//!
//! ## Sources
//!
//! | Call | What it makes |
//! | --- | --- |
//! | `impulse(at:, period:, until:)` | One sample at 1 when the note starts, then silence; given any of the three, one at `at` seconds, 0 unless given, and again every `period` seconds before `until`, as the tanpura plucks. |
//! | `sine(hz:, phase:)` | A sine wave; `phase` is in radians, and 0 unless given. |
//! | `saw(hz:)`, `square(hz:)`, `triangle(hz:)` | The wave named. |
//! | `pulse(hz:, width:)` | A pulse wave; `width` is the part of each cycle that is up, and a half unless given. |
//! | `noise(colour, seed:, drawn:, mix:)` | Noise; `colour` is the name `white` or `pink`, and `seed`, if given, fixes its numbers instead of drawing a seed from the note; `drawn` seeds it with the note's seed from `draw`, taken with `mix`, plus `drawn`. |
//! | `clicks(rate:, seed:)` | Clicks at random times, `rate` a second on average, which may change as the note sounds, each of a random level from 0 to 1; `seed`, if given, fixes them instead of drawing a seed from the note. |
//! | `time(every:)` | The seconds since the note started, starting over every `every` seconds if given, worked out before the sound is rounded to a sample's precision. |
//! | `follow(option)` | An option as a signal, which glides to each new value it is given while the note sounds. |
//! | `input()` | In an effect, the channel coming in. |
//! | `left()`, `right()` | In an effect over both sides, the side named coming in. |
//! | `sweep(length:, wave:, invert:, offset:, width:)` | A level from 0 to 1 once every `length` seconds; `wave` is 0 saw, 1 pulse, 2 triangle, 3 sine or 4 cubic, and 3 unless given, `invert: 1` turns it upside down, `offset` is where in the cycle it starts, from 0 to 1, and `width` the part of a pulse that is up, a half unless given. All but `offset` may change while it sounds. |
//!
//! ## Filters
//!
//! | Call | What it does to `sound` |
//! | --- | --- |
//! | `lowpass(sound, hz:, q:, poles:)`, `highpass`, `bandpass` | Filters it; `q` is 0.707 and `poles` is 2 unless given, and `hz` is held from 20 to 20000; `hz` and `q` may change while the note sounds; `poles: 1` makes a lowpass or a highpass of one pole, which takes no `q`. |
//! | `ladder(sound, hz:, resonance:)` | Filters it through a four-pole resonant lowpass, the ladder of analog synths; `resonance` is from 0, unless given, to 1, where the filter rings on at `hz` once set ringing, and both may change while the note sounds. |
//! | `lowshelf(sound, hz:, q:, db:)`, `highshelf`, `peak` | Raises or lowers it by `db` below, above or around `hz`. |
//! | `mode(sound, hz:, level:, fade:)` | Sets a resonator ringing with it, which fades 60 dB over `fade` seconds; `level` and `fade` are known when the note starts. Given lists of the same length, it sets a resonator ringing for each, all struck by the one sound. A sound in `hz`, or in its list, glides the resonator's frequency as it changes. |
//! | `delay(sound, seconds:, max:)` | Delays it by `seconds`, which may change up to `max`. |
//! | `fed_back(sound, seconds:, feedback:, lowpass:, highpass:, let_in:)` | Delays it by `seconds` and feeds what comes back in again, `feedback` times, through a one-pole low-pass at `lowpass` Hz and high-pass at `highpass` Hz inside the loop, each left out unless given; it lets its input in for the first `let_in` seconds only, or always unless given, and sounds only what comes back, as a plucked string's period or an echo's repeats do. All are numbers known when the note starts. |
//! | `dc_block(sound, hz:)` | Takes its constant offset away, below `hz`, 10 unless given. |
//! | `allpass(sound, hz:, q:)` | Passes every frequency at the same level and shifts its phase around `hz`; `q` is 0.707 unless given, and both may change while it sounds. |
//! | `lag(sound, seconds:)` | Glides to it, falling 60 dB short of each step over `seconds`, known when the note starts. |
//! | `struck(sound, attack:, fall:)` | Rises over `attack` and falls over `fall` seconds, both 60 dB times known when the note starts, at each click or impulse of the sound, starting again at the next. |
//!
//! ## Distortion
//!
//! Each works at twice the engine's rate, so it makes fewer aliases, and its `drive:` in decibels, 0 unless given, and its `level:`, 1 unless given, may change while the note sounds.
//!
//! | Call | What it does to `sound` |
//! | --- | --- |
//! | `saturate(sound, drive:, level:)` | Rounds it off smoothly towards `level`, which it never passes. |
//! | `clip(sound, drive:, level:)` | Cuts it off at `level` either side of 0. |
//! | `fold(sound, drive:, level:)` | Folds it back each time it passes `level` either side of 0. |
//!
//! ## Levels
//!
//! | Call | What it makes |
//! | --- | --- |
//! | `envelope(points)` | A level that moves from silence through each of `points`, a list of `[seconds, level]`, of `[seconds, level, curve]` bent as [`segments`](crate::nodes::segments) bends, or of `[seconds, level, exp]`, exponential. |
//! | `perc(attack:, release:)` | A level that rises over `attack` seconds and falls over `release`. |
//! | `envelope(sound, points)`, `perc(sound, attack:, release:)` | The sound, scaled by that level. |
//! | `gain(sound, level)` | The sound, scaled. |
//! | `follower(sound, rise:, fall:)` | The level of the sound, which rises towards its size over `rise` seconds and falls over `fall`, both known when the note starts. |
//! | `mix(list)` | The sounds of a list, added up. |
//! | `share(sound, through:)` | The sound `through` makes, in which each `input` is `sound`, built once, so a noise in it plays the same numbers at each use. |
//!
//! ## How the note ends
//!
//! A synth's graph ends in one of these, as a synth written in Rust ends in a [`Note`].
//!
//! | Call | What it makes |
//! | --- | --- |
//! | `rings(sound, seconds:)` | A note that fades by itself and is silent after `seconds`. |
//! | `note_envelope(sound)` | A note shaped by its envelope options, which falls silent over its release when let go. |
//!
//! ## An effect's sides and its tail
//!
//! An effect's graph is one channel's sound, built once for each channel, unless it is one of these.
//!
//! | Call | What it makes |
//! | --- | --- |
//! | `sides(left, right)` | An effect over both sides: the sound of each, built from `left()` and `right()`. |
//! | `tail(graph, seconds:)` | The effect, sounding on `seconds` after its input stops; an effect without it sounds on for 1 second. |
//! | `tail(graph, option:)` | The effect, sounding on for as long as the option named says. |
//!
//! ## Arithmetic
//!
//! Each works on numbers when the note starts, and on sounds at every sample.
//!
//! | Calls | What they give |
//! | --- | --- |
//! | `add`, `sub`, `mul`, `div`, `pow`, `min`, `max` | Of two values. |
//! | `neg`, `abs`, `log2`, `exp`, `sin`, `cos` | Of one value. |
//! | `hz`, `db` | The frequency of a MIDI note, and the gain of a level in decibels. |
//! | `clamp(x, low, high)` | `x` held from `low` to `high`. |
//! | `draw(index, mix:)` | The note's `index`th number from 0 up to 1, from 0, drawn from the note's seed: the next seed the note draws, mixed with how many notes of the synth came before times `mix`, 0 unless given; `mix` is a whole number, or one too large for a float given as the name of its digits, and every draw and drawn noise of a note takes the same. |
//! | `lt`, `gt`, `le`, `ge`, `eq`, `ne` | 1 when the comparison of two values holds, and 0 otherwise; `eq` and `ne` also compare an option that takes names with one of its names, and an option that takes names goes nowhere else. |
//! | `select(condition, then, otherwise)` | `then` when `condition` is not 0, and `otherwise` when it is. |
//! | `or_else(option, fallback)` | An option's value when the note gave one, and `fallback` otherwise. |
//!
//! ## The instruments' models
//!
//! `partials`, `bowed_string`, `air_jet`, `reed`, `wind_bands`, `vibrato`, `fingers` and `sway` are the models the crate's own strings and winds are built from.
//! Their arguments are in this module's source.
//!
//! # Files
//!
//! [`Synth::from_json`] and [`Effect::from_json`] read a synth or an effect from a JSON file, and [`Synth::from_json_file`] and [`Effect::from_json_file`] from its path, so a program in any language can write one.
//!
//! ```json
//! {
//!   "format": "mxs-synth",
//!   "version": 1,
//!   "name": "plate",
//!   "options": [
//!     {"name": "note", "kind": "note", "default": 60, "doc": "The pitch."},
//!     {"name": "vel", "kind": "float", "default": 0.6, "range": [0, 1], "doc": "How hard it is struck."}
//!   ],
//!   "defs": {"freq": {"call": "hz", "args": [{"option": "note"}]}},
//!   "clauses": [
//!     {"matches": {}, "graph": {"call": "rings", "args": [
//!       {"call": "mode", "args": [{"call": "impulse"}], "named": {"hz": {"ref": "freq"}, "level": 0.2, "fade": 3}}
//!     ], "named": {"seconds": 4}}}
//!   ]
//! }
//! ```
//!
//! - `format` is `mxs-synth` or `mxs-effect`, and `version` is 1; a version newer than this mxs reads is refused, naming both.
//! - `options` declare the options as [`Spec`] does: `name`, `kind` (`note`, `hz`, `db`, `seconds`, `beats`, `float`, `integer`, `boolean`, or `name` with a list of `names`), `default`, `range` as a pair with `null` where unbounded, `required` and `doc`.
//! - A value is a number, a list, `{"option": name}`, `{"name": name}` for a name such as a noise's colour, `{"call": name, "args": [...], "named": {...}}` with either part left out when empty, or `{"ref": name}`.
//! - `defs` names values used more than once, and `{"ref": name}` stands for one; a ref to a name that `defs` lacks, or a def that refers to itself, is refused.
//! - A synth has `clauses`, each with the `matches` a note's options must meet and its `graph`; an effect has one `graph`.
//! - A call outside the vocabulary, or one missing an argument it needs by name, is refused with the path to it in the file, such as `clauses[0].graph.args[0]: lowpass needs hz`, and text that is not JSON with the line and column where it stops being JSON.

mod file;

use crate::nodes::{self, db, hz};
use crate::{Context, Effect, Kind, Note, Options, Spec, Synth, Value};
use fundsp::prelude::*;

/// A value of a graph: a number, a name such as a noise's colour, one of the note's options, a list, or a call of the vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum Graph {
    /// A number.
    Number(f64),
    /// A name, such as a noise's colour.
    Name(String),
    /// One of the note's options, by its name.
    Option(String),
    /// A list of values.
    List(Vec<Graph>),
    /// A call of a function of the vocabulary.
    Call {
        /// The function's name.
        name: String,
        /// The arguments given by position.
        args: Vec<Graph>,
        /// The arguments given by name.
        named: Vec<(String, Graph)>,
    },
}

/// One clause of a synth: the values of options it matches, and the graph a note is then built from.
#[derive(Clone, Debug, PartialEq)]
pub struct Clause {
    /// The values the note's options must have for the clause to apply.
    pub matches: Vec<(String, Value)>,
    /// The graph a note that matches is built from.
    pub graph: Graph,
}

impl Graph {
    /// A call of the function `name` with its positional arguments and its named ones.
    pub fn call(name: &str, args: Vec<Graph>, named: Vec<(&str, Graph)>) -> Graph {
        let named = named.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Graph::Call {
            name: name.to_string(),
            args,
            named,
        }
    }
}

const ARITHMETIC: &[&str] = &[
    "add", "sub", "mul", "div", "mod", "pow", "neg", "min", "max", "clamp", "log2", "exp", "abs",
    "sin", "cos", "hz", "db", "draw", "lt", "gt", "le", "ge", "eq", "ne", "select", "or_else",
];
const NODES: &[&str] = &[
    "impulse",
    "follow",
    "time",
    "sine",
    "partials",
    "bowed_string",
    "air_jet",
    "wind_bands",
    "vibrato",
    "share",
    "saw",
    "square",
    "triangle",
    "pulse",
    "noise",
    "lowpass",
    "highpass",
    "bandpass",
    "lowshelf",
    "highshelf",
    "peak",
    "mode",
    "delay",
    "fed_back",
    "sway",
    "fingers",
    "reed",
    "dc_block",
    "saturate",
    "clip",
    "fold",
    "ladder",
    "envelope",
    "perc",
    "mix",
    "gain",
    "note_envelope",
    "rings",
    "input",
    "left",
    "right",
    "sweep",
    "follower",
    "allpass",
    "lag",
    "clicks",
    "struck",
    "sides",
    "tail",
];

/// The arguments by name each call cannot do without, which a file is checked for when it loads.
const NEEDS: &[(&str, &[&str])] = &[
    ("sine", &["hz"]),
    ("saw", &["hz"]),
    ("square", &["hz"]),
    ("triangle", &["hz"]),
    ("pulse", &["hz"]),
    ("lowpass", &["hz"]),
    ("highpass", &["hz"]),
    ("bandpass", &["hz"]),
    ("lowshelf", &["hz", "db"]),
    ("highshelf", &["hz", "db"]),
    ("peak", &["hz", "db"]),
    ("ladder", &["hz"]),
    ("mode", &["hz", "level", "fade"]),
    ("delay", &["seconds", "max"]),
    ("fed_back", &["seconds", "feedback"]),
    ("reed", &["hz", "breath", "corner", "keep", "bell"]),
    ("perc", &["attack", "release"]),
    ("rings", &["seconds"]),
    ("partials", &["ratios", "levels"]),
    ("vibrato", &["depth", "hz"]),
    ("wind_bands", &["vel", "gains", "levels"]),
    (
        "bowed_string",
        &["note", "speed", "force", "position", "takes", "rosin"],
    ),
    ("air_jet", &["hz", "breath", "even"]),
    ("share", &["through"]),
    ("sweep", &["length"]),
    ("follower", &["rise", "fall"]),
    ("allpass", &["hz"]),
    ("lag", &["seconds"]),
    ("clicks", &["rate"]),
    ("struck", &["attack", "fall"]),
];

/// The arguments by name each of these calls takes, any other being refused when the graph is checked.
const TAKES: &[(&str, &[&str])] = &[
    ("time", &["every"]),
    ("impulse", &["at", "period", "until"]),
    ("noise", &["seed", "drawn", "mix"]),
    ("draw", &["mix"]),
    (
        "fed_back",
        &["seconds", "feedback", "lowpass", "highpass", "let_in"],
    ),
    ("sway", &[]),
    (
        "reed",
        &[
            "hz",
            "breath",
            "steep",
            "through",
            "eased",
            "conical",
            "least_corner",
            "corner",
            "keep",
            "zeta",
            "apex",
            "relative",
            "tongue",
            "gasp",
            "mend",
            "bell",
            "bell_corner",
            "smooth",
            "lift",
            "catch",
        ],
    ),
    ("sweep", &["length", "wave", "invert", "offset", "width"]),
    ("follower", &["rise", "fall"]),
    ("allpass", &["hz", "q"]),
    ("lag", &["seconds"]),
    ("clicks", &["rate", "seed"]),
    ("struck", &["attack", "fall"]),
    ("share", &["through"]),
    ("tail", &["seconds", "option"]),
    ("sides", &[]),
    ("left", &[]),
    ("right", &[]),
];

fn in_vocabulary(name: &str) -> bool {
    ARITHMETIC.contains(&name) || NODES.contains(&name)
}

fn needs(name: &str) -> &'static [&'static str] {
    NEEDS
        .iter()
        .find(|(call, _)| *call == name)
        .map_or(&[], |(_, keys)| keys)
}

impl Synth {
    /// A synth whose note is built from the first of `clauses` whose matches its options meet, or is silent when none does.
    pub fn graph(name: &str, specs: Vec<Spec>, clauses: Vec<Clause>) -> Result<Synth, String> {
        for clause in &clauses {
            known(&clause.graph).map_err(|reason| format!("{name}: {reason}"))?;
            named_options(&clause.graph, &specs).map_err(|reason| format!("{name}: {reason}"))?;
            let mut mixes = Vec::new();
            mixes_in(&clause.graph, &mut mixes);
            if mixes.windows(2).any(|pair| pair[0] != pair[1]) {
                return Err(format!(
                    "{name}: each draw and drawn noise of a note takes the same mix"
                ));
            }
            if ["input", "left", "right", "sides", "tail"]
                .iter()
                .any(|node| calls(&clause.graph, node))
            {
                return Err(format!("{name}: a synth has no input; an effect takes one"));
            }
        }
        let owner = name.to_string();
        let notes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        Ok(Synth::new(name).options(specs).build(move |options, ctx| {
            let count = notes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let Some(clause) = clauses
                .iter()
                .find(|c| c.matches.iter().all(|(k, v)| options.value(k) == v))
            else {
                return Note::rings(dc(0.0f32), 0.0);
            };
            match (Build {
                options,
                ctx,
                count,
                drawn: None,
                inputs: None,
                sides: 0,
            })
            .eval(&clause.graph)
            {
                Ok(V::Note(note)) => note,
                Ok(_) => panic!("{owner} ends with neither note_envelope nor rings"),
                Err(reason) => panic!("{owner}: {reason}"),
            }
        }))
    }
}

impl Effect {
    /// An effect built from a graph: `graph` is one channel's sound, its `input` that channel coming in, and it is built once for each channel.
    /// A graph that is `sides(left, right)` is built once, each side from `left()` and `right()`, and one wrapped in `tail` sounds on for that long after its input stops.
    pub fn graph(name: &str, specs: Vec<Spec>, graph: Graph) -> Result<Effect, String> {
        known(&graph).map_err(|reason| format!("{name}: {reason}"))?;
        named_options(&graph, &specs).map_err(|reason| format!("{name}: {reason}"))?;
        let refuse = |reason: String| format!("{name}: {reason}");
        let (graph, tail) = match graph {
            Graph::Call { name, args, named } if name == "tail" => {
                let [inner] = <[Graph; 1]>::try_from(args)
                    .map_err(|_| refuse("tail takes the effect's graph".to_string()))?;
                let tail = match named.as_slice() {
                    [(key, Graph::Number(seconds))] if key == "seconds" && *seconds >= 0.0 => {
                        Tail::Seconds(*seconds)
                    }
                    [(key, Graph::Name(option))] if key == "option" => {
                        if !specs.iter().any(|s| &s.name == option) {
                            return Err(refuse(format!(
                                "tail names {option}, which is not an option"
                            )));
                        }
                        Tail::Option(option.clone())
                    }
                    _ => {
                        return Err(refuse(
                            "tail takes seconds:, a number from 0 up, or option:, an option's name"
                                .to_string(),
                        ));
                    }
                };
                (inner, Some(tail))
            }
            graph => (graph, None),
        };
        let sides = match graph {
            Graph::Call { name, args, .. } if name == "sides" => {
                let pair = <[Graph; 2]>::try_from(args)
                    .map_err(|_| refuse("sides takes the left sound and the right".to_string()))?;
                pair.to_vec()
            }
            graph => vec![graph],
        };
        for side in &sides {
            if calls(side, "sides") || calls(side, "tail") {
                return Err(refuse(
                    "sides and tail go around the whole graph, tail outermost".to_string(),
                ));
            }
            let other: &[&str] = if sides.len() == 2 {
                &["input"]
            } else {
                &["left", "right"]
            };
            if other.iter().any(|node| calls(side, node)) {
                return Err(refuse(if sides.len() == 2 {
                    "an effect over both sides takes left() and right(), not input()".to_string()
                } else {
                    "left() and right() go in an effect over both sides, inside sides".to_string()
                }));
            }
        }
        let owner = name.to_string();
        let decl = specs
            .into_iter()
            .fold(Effect::new(name), |decl, spec| decl.option(spec));
        let decl = match tail {
            Some(Tail::Seconds(seconds)) => decl.tail(seconds),
            Some(Tail::Option(option)) => decl.tail_from(move |o| o.number(&option).abs()),
            None => decl,
        };
        Ok(decl.build(move |options, ctx| {
            let width = sides.len();
            let mut side = |graph: &Graph| -> Net {
                let mut build = Build {
                    options,
                    ctx,
                    count: 0,
                    drawn: None,
                    inputs: Some(Vec::new()),
                    sides: width,
                };
                let sound = build
                    .eval(graph)
                    .and_then(signal)
                    .unwrap_or_else(|reason| panic!("{owner}: {reason}"));
                let uses = build.inputs.unwrap_or_default();
                if sound.inputs() != uses.len() {
                    panic!("{owner}: its input is taken where it is left out or taken twice, such as in a sound multiplied by 0");
                }
                fed(&uses, width, sound)
            };
            match sides.as_slice() {
                [left, right] => side(left) ^ side(right),
                _ => side(&sides[0]) | side(&sides[0]),
            }
        }))
    }
}

// A graph nests as deep as the music written for it, and the BEAM runs these on a scheduler's small stack, so each level grows the stack when it runs low.
// A level of eval and call takes over 128 KiB in a debug build.
const RED_ZONE: usize = 256 * 1024;
const GROWTH: usize = 4 * 1024 * 1024;

/// How long an effect's graph says it sounds on after its input stops.
enum Tail {
    Seconds(f64),
    Option(String),
}

/// A sound fed the channels it takes, in the order it takes them, out of `width` coming in.
fn fed(uses: &[usize], width: usize, sound: Net) -> Net {
    let channel = |index: usize| -> Net {
        (0..width)
            .map(|side| {
                if side == index {
                    net(pass())
                } else {
                    net(sink())
                }
            })
            .reduce(|taken, side| taken | side)
            .unwrap_or_else(|| net(pass()))
    };
    match uses.split_first() {
        None => channel(width) | sound,
        Some((first, rest)) => {
            rest.iter()
                .fold(channel(*first), |inputs, index| inputs ^ channel(*index))
                >> sound
        }
    }
}

fn calls(graph: &Graph, node: &str) -> bool {
    stacker::maybe_grow(RED_ZONE, GROWTH, || match graph {
        Graph::List(items) => items.iter().any(|g| calls(g, node)),
        Graph::Call { name, args, named } => {
            name == node
                || args
                    .iter()
                    .chain(
                        named
                            .iter()
                            // An air jet's sound and side take the jet's outputs as their input, and a share's through takes the shared sound.
                            .filter(|(k, _)| match name.as_str() {
                                "air_jet" => !matches!(k.as_str(), "sound" | "side"),
                                "share" => k != "through",
                                _ => true,
                            })
                            .map(|(_, v)| v),
                    )
                    .any(|g| calls(g, node))
        }
        _ => false,
    })
}

/// Refuses an option that takes names anywhere but in eq or ne with a name it takes, as a number is read from every other option.
fn named_options(graph: &Graph, specs: &[Spec]) -> Result<(), String> {
    let takes = |option: &str| {
        specs
            .iter()
            .find(|s| s.name == option)
            .and_then(|s| match &s.kind {
                Kind::Name(values) => Some(values.clone()),
                _ => None,
            })
    };
    stacker::maybe_grow(RED_ZONE, GROWTH, || match graph {
        Graph::Call { name, args, .. }
            if let ("eq" | "ne", [a, b]) = (name.as_str(), args.as_slice())
                && let Some((option, given)) = compared(a, b)
                && let Some(values) = takes(option) =>
        {
            if values.iter().any(|v| v == given) {
                Ok(())
            } else {
                let names = match values.split_last() {
                    Some((last, [])) => last.clone(),
                    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
                    None => String::new(),
                };
                Err(format!("{option} takes {names}, not {given}"))
            }
        }
        Graph::Call { args, named, .. } => args
            .iter()
            .chain(named.iter().map(|(_, v)| v))
            .try_for_each(|g| named_options(g, specs)),
        Graph::List(items) => items.iter().try_for_each(|g| named_options(g, specs)),
        Graph::Option(option) if takes(option).is_some() => Err(format!(
            "{option} takes names, which only eq and ne compare with a name"
        )),
        _ => Ok(()),
    })
}

/// The option and the name of a comparison of one with the other, either way round.
fn compared<'g>(a: &'g Graph, b: &'g Graph) -> Option<(&'g str, &'g str)> {
    match (a, b) {
        (Graph::Option(option), Graph::Name(name)) | (Graph::Name(name), Graph::Option(option)) => {
            Some((option, name))
        }
        _ => None,
    }
}

/// The mixes given to `draw` and to a drawn `noise` in `graph`, in order.
fn mixes_in<'a>(graph: &'a Graph, mixes: &mut Vec<&'a Graph>) {
    stacker::maybe_grow(RED_ZONE, GROWTH, || match graph {
        Graph::List(items) => items.iter().for_each(|item| mixes_in(item, mixes)),
        Graph::Call { name, args, named } => {
            if matches!(name.as_str(), "draw" | "noise") {
                mixes.extend(named.iter().filter(|(k, _)| k == "mix").map(|(_, v)| v));
            }
            args.iter()
                .chain(named.iter().map(|(_, v)| v))
                .for_each(|g| mixes_in(g, mixes));
        }
        _ => {}
    })
}

fn known(graph: &Graph) -> Result<(), String> {
    stacker::maybe_grow(RED_ZONE, GROWTH, || match graph {
        Graph::List(items) => items.iter().try_for_each(known),
        Graph::Call { name, args, named } => {
            if !in_vocabulary(name) {
                return Err(format!("{name} is not in the vocabulary"));
            }
            if let Some((_, takes)) = TAKES.iter().find(|(call, _)| call == name)
                && let Some((key, _)) = named.iter().find(|(k, _)| !takes.contains(&k.as_str()))
            {
                return Err(format!("{name} does not take {key}:"));
            }
            if let Some(key) = needs(name).iter().find(|key| {
                TAKES.iter().any(|(call, _)| call == name) && !named.iter().any(|(k, _)| k == *key)
            }) {
                return Err(format!("{name} needs {key}:"));
            }
            if matches!(name.as_str(), "lowpass" | "highpass")
                && named
                    .iter()
                    .any(|(k, v)| k == "poles" && *v == Graph::Number(1.0))
                && named.iter().any(|(k, _)| k == "q")
            {
                return Err(format!("{name} of one pole takes no q:"));
            }
            if name == "mode" {
                let lengths: Vec<Option<usize>> = ["hz", "level", "fade"]
                    .iter()
                    .filter_map(|key| named.iter().find(|(k, _)| k == key))
                    .map(|(_, v)| match v {
                        Graph::List(items) => Some(items.len()),
                        _ => None,
                    })
                    .collect();
                if lengths.iter().any(Option::is_some) && lengths.windows(2).any(|w| w[0] != w[1]) {
                    return Err("mode's hz:, level: and fade: lists differ in length".to_string());
                }
            }
            args.iter()
                .chain(named.iter().map(|(_, v)| v))
                .try_for_each(known)
        }
        _ => Ok(()),
    })
}

/// What a graph works out to: a number known when the note starts, a signal, a list, a name, or the note itself.
enum V {
    Num(f64),
    Sig(Net),
    List(Vec<V>),
    Name(String),
    Note(Note),
}

fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

fn signal(v: V) -> Result<Net, String> {
    match v {
        V::Num(x) => Ok(net(dc(x as f32))),
        V::Sig(n) => Ok(n),
        V::List(_) => Err("a list where a sound or a number goes".to_string()),
        V::Name(name) => Err(format!("the name {name} where a sound or a number goes")),
        V::Note(_) => Err("a finished note where a sound goes".to_string()),
    }
}

fn number(v: V, what: &str) -> Result<f64, String> {
    match v {
        V::Num(x) => Ok(x),
        _ => Err(format!("{what} must be known when the note starts")),
    }
}

fn numbers(v: V, what: &str) -> Result<Vec<f64>, String> {
    match v {
        V::List(items) => items.into_iter().map(|item| number(item, what)).collect(),
        _ => Err(format!("{what}s must be a list")),
    }
}

fn binary(a: V, b: V, f: fn(f64, f64) -> f64) -> Result<V, String> {
    match (a, b) {
        (V::Num(a), V::Num(b)) => Ok(V::Num(f(a, b))),
        (a, b) => Ok(V::Sig(
            (signal(a)? | signal(b)?)
                >> net(map(move |x: &Frame<f32, U2>| {
                    f(x[0] as f64, x[1] as f64) as f32
                })),
        )),
    }
}

fn unary(a: V, f: fn(f64) -> f64) -> Result<V, String> {
    match a {
        V::Num(a) => Ok(V::Num(f(a))),
        a => Ok(V::Sig(
            signal(a)? >> net(map(move |x: &Frame<f32, U1>| f(x[0] as f64) as f32)),
        )),
    }
}

fn add(a: V, b: V) -> Result<V, String> {
    match (a, b) {
        (V::Num(x), other) | (other, V::Num(x)) if x == 0.0 => Ok(other),
        (V::Sig(a), V::Sig(b)) => Ok(V::Sig(a + b)),
        (a, b) => binary(a, b, |a, b| a + b),
    }
}

fn mul(a: V, b: V) -> Result<V, String> {
    match (a, b) {
        (V::Num(x), _) | (_, V::Num(x)) if x == 0.0 => Ok(V::Num(0.0)),
        (V::Sig(a), V::Sig(b)) => Ok(V::Sig(a * b)),
        (V::Sig(a), V::Num(x)) | (V::Num(x), V::Sig(a)) => Ok(V::Sig(a * x as f32)),
        (a, b) => binary(a, b, |a, b| a * b),
    }
}

struct Build<'a> {
    options: &'a Options,
    ctx: &'a mut Context,
    /// How many notes the synth started before this one, which `draw` mixes into the note's seed.
    count: u64,
    /// The mix and the seed `draw` and a drawn noise take from, once the first of them asks.
    drawn: Option<(u64, u64)>,
    /// The channel an effect's graph takes each time it takes its input so far, in order; None in a synth, which has none.
    inputs: Option<Vec<usize>>,
    /// How many channels an effect's graph is built over: 1 for each channel on its own, 2 for both sides, and 0 in a synth.
    sides: usize,
}

impl Build<'_> {
    fn eval(&mut self, graph: &Graph) -> Result<V, String> {
        stacker::maybe_grow(RED_ZONE, GROWTH, || match graph {
            Graph::Number(x) => Ok(V::Num(*x)),
            Graph::Name(name) => Ok(V::Name(name.clone())),
            Graph::Option(name) => Ok(V::Num(self.options.number(name))),
            Graph::List(items) => Ok(V::List(
                items
                    .iter()
                    .map(|g| self.eval(g))
                    .collect::<Result<_, _>>()?,
            )),
            Graph::Call { name, args, named } => {
                let before = self.inputs.as_ref().map_or(0, Vec::len);
                let value = self.call(name, args, named)?;
                // A sound worked out to a number, as one multiplied by 0, takes no input any more.
                if let (false, Some(uses)) = (matches!(value, V::Sig(_)), self.inputs.as_mut()) {
                    uses.truncate(before);
                }
                Ok(value)
            }
        })
    }

    fn call(&mut self, name: &str, args: &[Graph], named: &[(String, Graph)]) -> Result<V, String> {
        // An option the note may leave out, such as a drum's pitch: its value when given, and the fallback otherwise.
        if let ("or_else", [Graph::Option(option), fallback]) = (name, args) {
            return match self.options.given(option) {
                Some(x) => Ok(V::Num(x)),
                None => self.eval(fallback),
            };
        }
        // An option as a signal that glides to each new value the song gives while the note sounds.
        if let ("follow", [Graph::Option(option)]) = (name, args) {
            return Ok(V::Sig(self.options.follow(option)));
        }
        // An option that takes names, compared with a name when the note starts.
        if let ("eq" | "ne", [a, b]) = (name, args)
            && let Some((option, given)) = compared(a, b)
        {
            let same = matches!(self.options.value(option), Value::Name(n) if n == given);
            return Ok(V::Num(((name == "eq") == same) as u8 as f64));
        }
        if name == "air_jet" {
            return self.air_jet(named);
        }
        if name == "share" {
            return self.share(args, named);
        }
        let before = self.inputs.as_ref().map_or(0, Vec::len);
        let mut ends = Vec::new();
        let mut args: Vec<V> = args
            .iter()
            .map(|g| {
                let v = self.eval(g);
                ends.push(self.inputs.as_ref().map_or(0, Vec::len));
                v
            })
            .collect::<Result<_, _>>()?;
        let count = args.len();
        let mut taking = 0;
        let mut named: Vec<(String, V)> = named
            .iter()
            .map(|(k, g)| {
                let start = self.inputs.as_ref().map_or(0, Vec::len);
                let v = self.eval(g)?;
                taking += (self.inputs.as_ref().map_or(0, Vec::len) > start) as usize;
                Ok((k.clone(), v))
            })
            .collect::<Result<_, String>>()?;
        // Arguments by name are joined in the node's own order, so two that take different sides could be crossed.
        if self.sides == 2
            && taking > 1
            && let Some(uses) = &self.inputs
            && uses[before..].iter().any(|side| *side != uses[before])
        {
            return Err(format!(
                "{name} takes the left side in one argument by name and the right in another; give it one side there"
            ));
        }
        let mut take = |key: &str| {
            named
                .iter()
                .position(|(k, _)| k == key)
                .map(|i| named.swap_remove(i).1)
        };
        let mut arg = |i: usize| -> Result<V, String> {
            if i < args.len() {
                Ok(std::mem::replace(&mut args[i], V::Num(0.0)))
            } else {
                Err(format!("{name} takes {} arguments", i + 1))
            }
        };
        match name {
            "add" => add(arg(0)?, arg(1)?),
            "sub" => add(arg(0)?, mul(arg(1)?, V::Num(-1.0))?),
            "mul" => mul(arg(0)?, arg(1)?),
            "div" => binary(arg(0)?, arg(1)?, |a, b| a / b),
            "mod" => binary(arg(0)?, arg(1)?, f64::rem_euclid),
            "pow" => binary(arg(0)?, arg(1)?, f64::powf),
            "min" => binary(arg(0)?, arg(1)?, f64::min),
            "max" => binary(arg(0)?, arg(1)?, f64::max),
            "lt" => binary(arg(0)?, arg(1)?, |a, b| (a < b) as u8 as f64),
            "gt" => binary(arg(0)?, arg(1)?, |a, b| (a > b) as u8 as f64),
            "le" => binary(arg(0)?, arg(1)?, |a, b| (a <= b) as u8 as f64),
            "ge" => binary(arg(0)?, arg(1)?, |a, b| (a >= b) as u8 as f64),
            "eq" => binary(arg(0)?, arg(1)?, |a, b| (a == b) as u8 as f64),
            "ne" => binary(arg(0)?, arg(1)?, |a, b| (a != b) as u8 as f64),
            "neg" => unary(arg(0)?, |a| -a),
            "log2" => unary(arg(0)?, f64::log2),
            "exp" => unary(arg(0)?, f64::exp),
            "abs" => unary(arg(0)?, f64::abs),
            "sin" => unary(arg(0)?, f64::sin),
            "cos" => unary(arg(0)?, f64::cos),
            "hz" => unary(arg(0)?, hz),
            "db" => unary(arg(0)?, db),
            "clamp" => {
                let (x, low, high) = (arg(0)?, arg(1)?, arg(2)?);
                binary(binary(x, low, f64::max)?, high, f64::min)
            }
            "select" => match arg(0)? {
                V::Num(c) => {
                    let (then, otherwise) = (arg(1)?, arg(2)?);
                    // The side left out no longer takes the input it took.
                    if let Some(uses) = self.inputs.as_mut() {
                        let left_out = if c != 0.0 {
                            ends[1]..ends[2]
                        } else {
                            ends[0]..ends[1]
                        };
                        uses.drain(left_out);
                    }
                    Ok(if c != 0.0 { then } else { otherwise })
                }
                c => {
                    let picked = (signal(c)? | signal(arg(1)?)? | signal(arg(2)?)?)
                        >> net(map(
                            |x: &Frame<f32, U3>| if x[0] != 0.0 { x[1] } else { x[2] },
                        ));
                    Ok(V::Sig(picked))
                }
            },
            "impulse" => {
                let at = take("at")
                    .map(|v| number(v, "an impulse's at"))
                    .transpose()?;
                let period = take("period")
                    .map(|v| number(v, "an impulse's period"))
                    .transpose()?;
                let until = take("until")
                    .map(|v| number(v, "an impulse's until"))
                    .transpose()?;
                Ok(V::Sig(match (at, period, until) {
                    (None, None, None) => net(impulse::<U1>()),
                    (at, period, until) => nodes::pulses(
                        at.unwrap_or(0.0),
                        period.unwrap_or(f64::INFINITY),
                        until.unwrap_or(f64::INFINITY),
                    ),
                }))
            }
            "sine" => {
                let freq = signal(take("hz").ok_or("sine needs hz:")?)?;
                let phase = take("phase")
                    .map(|p| number(p, "a sine's phase"))
                    .transpose()?
                    .unwrap_or(0.0);
                Ok(V::Sig(
                    freq >> net(An(Sine::<f32>::with_phase(
                        (phase / std::f64::consts::TAU).rem_euclid(1.0) as f32,
                    ))),
                ))
            }
            "input" | "left" | "right" => match self.inputs.as_mut() {
                Some(uses) => {
                    uses.push((name == "right") as usize);
                    Ok(V::Sig(net(pass())))
                }
                None => Err("a synth has no input; an effect takes one".to_string()),
            },
            "sweep" => {
                let length = signal(take("length").ok_or("sweep needs length:")?)?;
                let wave = signal(take("wave").unwrap_or(V::Num(3.0)))?;
                let invert = signal(take("invert").unwrap_or(V::Num(0.0)))?;
                let width = signal(take("width").unwrap_or(V::Num(0.5)))?;
                let offset = take("offset")
                    .map(|o| number(o, "a sweep's offset"))
                    .transpose()?
                    .unwrap_or(0.0);
                let rate = length >> net(map(|x: &Frame<f32, U1>| 1.0 / x[0].max(0.001)));
                Ok(V::Sig(
                    (rate | width | wave | invert)
                        >> net(An(crate::effects::Lfo::new(offset as f32, 1.0, false))),
                ))
            }
            "follower" => {
                let sound = signal(arg(0)?)?;
                let rise = number(
                    take("rise").ok_or("follower needs rise:")?,
                    "a follower's rise",
                )?;
                let fall = number(
                    take("fall").ok_or("follower needs fall:")?,
                    "a follower's fall",
                )?;
                Ok(V::Sig(sound >> net(An(nodes::Follower::new(rise, fall)))))
            }
            "allpass" => {
                let sound = signal(arg(0)?)?;
                let freq = take("hz").ok_or("allpass needs hz:")?;
                let freq = binary(
                    binary(freq, V::Num(20.0), f64::max)?,
                    V::Num(20_000.0),
                    f64::min,
                )?;
                let q = signal(take("q").unwrap_or(V::Num(std::f64::consts::FRAC_1_SQRT_2)))?;
                Ok(V::Sig((sound | signal(freq)? | q) >> net(allpass::<f32>())))
            }
            "lag" => {
                let sound = signal(arg(0)?)?;
                let seconds = number(
                    take("seconds").ok_or("lag needs seconds:")?,
                    "a lag's seconds",
                )?;
                Ok(V::Sig(sound >> net(An(crate::effects::Lag::new(seconds)))))
            }
            "clicks" => {
                let rate = signal(take("rate").ok_or("clicks needs rate:")?)?;
                let seed = match take("seed") {
                    Some(seed) => number(seed, "a click's seed")? as u64,
                    None => self.ctx.seed(),
                };
                Ok(V::Sig(rate >> nodes::dust(seed)))
            }
            "struck" => {
                let sound = signal(arg(0)?)?;
                let attack = number(
                    take("attack").ok_or("struck needs attack:")?,
                    "a strike's attack",
                )?;
                let fall = number(take("fall").ok_or("struck needs fall:")?, "a strike's fall")?;
                Ok(V::Sig(sound >> nodes::decay2(attack, fall)))
            }
            "draw" => {
                let index = number(arg(0)?, "a draw's index")? as usize;
                let mut draws = nodes::Draws::new(self.note_seed(take("mix"))?);
                Ok(V::Num(
                    (0..=index).map(|_| draws.draw()).last().unwrap_or(0.0),
                ))
            }
            "time" => match take("every") {
                Some(every) => {
                    let every = number(every, "a time's every")?;
                    Ok(V::Sig(nodes::shape(move |t| t % every)))
                }
                None => Ok(V::Sig(nodes::shape(|t| t))),
            },
            "bowed_string" => {
                let note = signal(take("note").ok_or("bowed_string needs note:")?)?;
                let mut figure = |key: &str| {
                    number(
                        take(key).ok_or(format!("bowed_string needs {key}:"))?,
                        "a bow's figure",
                    )
                };
                let (speed, force, position, takes, rosin) = (
                    figure("speed")?,
                    figure("force")?,
                    figure("position")?,
                    figure("takes")?,
                    figure("rosin")?,
                );
                Ok(V::Sig(crate::synths::bowed_string(
                    note,
                    speed,
                    force,
                    position,
                    takes,
                    rosin,
                    self.ctx.seed(),
                )))
            }
            "vibrato" => {
                let depth = number(
                    take("depth").ok_or("vibrato needs depth:")?,
                    "a vibrato's depth",
                )?;
                let rate = number(take("hz").ok_or("vibrato needs hz:")?, "a vibrato's hz")?;
                Ok(V::Sig(crate::synths::drifting_swing(
                    depth,
                    rate,
                    self.ctx.seed(),
                )))
            }
            "wind_bands" => {
                let sound = signal(arg(0)?)?;
                let vel = signal(take("vel").ok_or("wind_bands needs vel:")?)?;
                let gains = numbers(
                    take("gains").ok_or("wind_bands needs gains:")?,
                    "a band's gain",
                )?;
                let levels = numbers(take("levels").ok_or("wind_bands needs levels:")?, "a level")?;
                if !matches!(gains.len(), 24 | 33) || levels.len() != 3 {
                    return Err(
                        "wind_bands takes 24 gains, eight at each of pp, mf and ff, or 33, eleven at each, and 3 levels"
                            .to_string(),
                    );
                }
                Ok(V::Sig(crate::synths::wind_bands(
                    sound, vel, &gains, &levels,
                )))
            }
            "sway" => Ok(V::Sig(crate::synths::sway(self.ctx.seed()))),
            "fingers" => Ok(V::Sig(signal(arg(0)?)? >> crate::synths::fingers())),
            "reed" => reed(&mut take, self.ctx),
            "partials" => {
                let pitch = signal(arg(0)?)?;
                let ratios = numbers(
                    take("ratios").ok_or("partials needs ratios:")?,
                    "a partial's ratio",
                )?;
                let levels = numbers(
                    take("levels").ok_or("partials needs levels:")?,
                    "a partial's level",
                )?;
                let phases = take("phases")
                    .map(|p| numbers(p, "a partial's phase"))
                    .transpose()?
                    .unwrap_or_else(|| vec![0.0; ratios.len()]);
                if levels.len() != ratios.len() || phases.len() != ratios.len() {
                    return Err("partials takes as many levels and phases as ratios".to_string());
                }
                Ok(V::Sig(crate::synths::partials(
                    pitch, &ratios, &levels, &phases,
                )))
            }
            "saw" | "square" | "triangle" => {
                let freq = signal(take("hz").ok_or(format!("{name} needs hz:"))?)?;
                // A Net ignores a setting with no node address, so the phase is set on the unit itself.
                let mut wave: Box<dyn AudioUnit> = match name {
                    "saw" => Box::new(saw()),
                    "square" => Box::new(square()),
                    _ => Box::new(triangle()),
                };
                wave.set(Setting::phase(0.0));
                wave.reset();
                Ok(V::Sig(freq >> Net::wrap(wave)))
            }
            "pulse" => {
                let freq = signal(take("hz").ok_or("pulse needs hz:")?)?;
                let width = signal(take("width").unwrap_or(V::Num(0.5)))?;
                let mut wave = pulse();
                wave.set(Setting::phase(0.0));
                wave.reset();
                Ok(V::Sig((freq | width) >> net(wave)))
            }
            "noise" => {
                let colour = arg(0)?;
                let seed = match (take("seed"), take("drawn")) {
                    (Some(seed), _) => number(seed, "a noise's seed")? as u64,
                    (None, Some(offset)) => {
                        let offset = number(offset, "a noise's drawn")? as u64;
                        self.note_seed(take("mix"))?.wrapping_add(offset)
                    }
                    (None, None) => self.ctx.seed(),
                };
                match colour {
                    V::Name(colour) if colour == "white" => Ok(V::Sig(nodes::white_noise(seed))),
                    V::Name(colour) if colour == "pink" => Ok(V::Sig(nodes::pink_noise(seed))),
                    _ => Err("noise takes :white or :pink".to_string()),
                }
            }
            "lowpass" | "highpass" | "bandpass" => {
                let mut sound = signal(arg(0)?)?;
                let freq = take("hz").ok_or(format!("{name} needs hz:"))?;
                let q = match take("q").unwrap_or(V::Num(std::f64::consts::FRAC_1_SQRT_2)) {
                    V::Num(q) => Ok(q),
                    moving => Err(signal(moving)?),
                };
                let poles = take("poles")
                    .map(|p| number(p, "a filter's poles"))
                    .transpose()?
                    .unwrap_or(2.0);
                let freq = binary(
                    binary(freq, V::Num(20.0), f64::max)?,
                    V::Num(20_000.0),
                    f64::min,
                )?;
                let freq = signal(freq)?;
                if poles == 1.0 && name != "bandpass" {
                    let pole = match name {
                        "lowpass" => net(lowpole::<f32>()),
                        _ => net(highpole::<f32>()),
                    };
                    return Ok(V::Sig((sound | freq) >> pole));
                }
                for _ in 0..((poles / 2.0).round().max(1.0) as usize) {
                    let filter: Net = match name {
                        "lowpass" => net(lowpass::<f32>()),
                        "highpass" => net(highpass::<f32>()),
                        _ => net(bandpass::<f32>()),
                    };
                    // FunDSP's band-pass peaks at its q, so it is scaled back to a peak of 1.
                    sound = match &q {
                        Ok(q) if name == "bandpass" => {
                            (sound | freq.clone() | net(dc(*q as f32)))
                                >> (filter * (1.0 / q) as f32)
                        }
                        Ok(q) => (sound | freq.clone() | net(dc(*q as f32))) >> filter,
                        Err(q) if name == "bandpass" => {
                            ((sound | freq.clone() | q.clone()) >> filter | q.clone())
                                >> net(map(|x: &Frame<f32, U2>| x[0] / x[1]))
                        }
                        Err(q) => (sound | freq.clone() | q.clone()) >> filter,
                    };
                }
                Ok(V::Sig(sound))
            }
            "lowshelf" | "highshelf" | "peak" => {
                let sound = signal(arg(0)?)?;
                let freq = signal(take("hz").ok_or(format!("{name} needs hz:"))?)?;
                let q = signal(take("q").unwrap_or(V::Num(std::f64::consts::FRAC_1_SQRT_2)))?;
                let gain = signal(unary(take("db").ok_or(format!("{name} needs db:"))?, db)?)?;
                let filter: Net = match name {
                    "lowshelf" => net(lowshelf::<f32>()),
                    "highshelf" => net(highshelf::<f32>()),
                    _ => net(bell::<f32>()),
                };
                Ok(V::Sig((sound | freq | q | gain) >> filter))
            }
            "mode" => {
                let sound = arg(0)?;
                let freq = take("hz").ok_or("mode needs hz:")?;
                let gliding = match &freq {
                    V::Sig(_) => true,
                    V::List(items) => items.iter().any(|item| matches!(item, V::Sig(_))),
                    _ => false,
                };
                if gliding {
                    let level = take("level").ok_or("mode needs level:")?;
                    let fade = take("fade").ok_or("mode needs fade:")?;
                    return gliding_modes(signal(sound)?, freq, level, fade);
                }
                if let V::List(_) = freq {
                    let freqs = numbers(freq, "a mode's hz")?;
                    let levels =
                        numbers(take("level").ok_or("mode needs level:")?, "a mode's level")?;
                    let fades = numbers(take("fade").ok_or("mode needs fade:")?, "a mode's fade")?;
                    return Ok(V::Sig(nodes::modes(
                        signal(sound)?,
                        &freqs,
                        &levels,
                        &fades,
                    )));
                }
                let freq = number(freq, "a mode's hz")?;
                let level = number(take("level").ok_or("mode needs level:")?, "a mode's level")?;
                let fade = number(take("fade").ok_or("mode needs fade:")?, "a mode's fade")?;
                if level == 0.0 || freq >= 20_000.0 {
                    return Ok(V::Num(0.0));
                }
                Ok(V::Sig(signal(sound)? >> nodes::struck(freq, level, fade)))
            }
            "delay" => {
                let sound = signal(arg(0)?)?;
                let seconds = signal(take("seconds").ok_or("delay needs seconds:")?)?;
                let longest = number(take("max").ok_or("delay needs max:")?, "a delay's max")?;
                Ok(V::Sig((sound | seconds) >> net(tap(0.0, longest as f32))))
            }
            "fed_back" => {
                let sound = signal(arg(0)?)?;
                let mut known = |key: &str, default: Option<f64>| -> Result<f64, String> {
                    match (take(key), default) {
                        (Some(value), _) => number(value, &format!("a fed_back's {key}")),
                        (None, Some(default)) => Ok(default),
                        (None, None) => Err(format!("fed_back needs {key}:")),
                    }
                };
                let seconds = known("seconds", None)?;
                let feedback = known("feedback", None)?;
                let lowpass = known("lowpass", Some(0.0))?;
                let highpass = known("highpass", Some(0.0))?;
                let let_in = known("let_in", Some(f64::INFINITY))?;
                Ok(V::Sig(
                    sound >> nodes::fed_back(seconds, feedback, lowpass, highpass, let_in),
                ))
            }
            "dc_block" => {
                let sound = signal(arg(0)?)?;
                let cutoff = take("hz")
                    .map(|hz| number(hz, "a dc_block's hz"))
                    .transpose()?
                    .unwrap_or(10.0);
                Ok(V::Sig(sound >> net(dcblock_hz(cutoff as f32))))
            }
            "saturate" | "clip" | "fold" => {
                let sound = signal(arg(0)?)?;
                let gain = signal(unary(take("drive").unwrap_or(V::Num(0.0)), db)?)?;
                let level = signal(take("level").unwrap_or(V::Num(1.0)))?;
                let shaper = match name {
                    "saturate" => net(oversample(map(|x: &Frame<f32, U3>| {
                        nodes::saturate(x[0] * x[1], x[2])
                    }))),
                    "clip" => net(oversample(map(|x: &Frame<f32, U3>| {
                        nodes::clip(x[0] * x[1], x[2])
                    }))),
                    _ => net(oversample(map(|x: &Frame<f32, U3>| {
                        nodes::fold(x[0] * x[1], x[2])
                    }))),
                };
                // The filters of the higher rate ring past the level, so the shaped sound is held to it again.
                let shaped = (sound | gain | level.clone()) >> shaper;
                Ok(V::Sig(
                    (shaped | level) >> net(map(|x: &Frame<f32, U2>| nodes::clip(x[0], x[1]))),
                ))
            }
            "ladder" => {
                let sound = signal(arg(0)?)?;
                let freq = take("hz").ok_or("ladder needs hz:")?;
                let freq = binary(
                    binary(freq, V::Num(20.0), f64::max)?,
                    V::Num(20_000.0),
                    f64::min,
                )?;
                let resonance = take("resonance").unwrap_or(V::Num(0.0));
                let resonance = binary(
                    binary(resonance, V::Num(0.0), f64::max)?,
                    V::Num(1.0),
                    f64::min,
                )?;
                Ok(V::Sig(
                    (sound | signal(freq)? | signal(resonance)?) >> nodes::ladder(),
                ))
            }
            "envelope" => {
                let (sound, points) = if count == 2 {
                    (Some(arg(0)?), arg(1)?)
                } else {
                    (None, arg(0)?)
                };
                let shape = envelope(points)?;
                match sound {
                    Some(sound) => mul(sound, V::Sig(shape)),
                    None => Ok(V::Sig(shape)),
                }
            }
            "perc" => {
                let attack = number(
                    take("attack").ok_or("perc needs attack:")?,
                    "a perc's attack",
                )?;
                let release = number(
                    take("release").ok_or("perc needs release:")?,
                    "a perc's release",
                )?;
                let shape = V::Sig(nodes::shape(move |t| nodes::perc(t, attack, release)));
                if count == 0 {
                    Ok(shape)
                } else {
                    mul(arg(0)?, shape)
                }
            }
            "mix" => match arg(0)? {
                V::List(items) => items.into_iter().try_fold(V::Num(0.0), add),
                _ => Err("mix takes a list".to_string()),
            },
            "gain" => mul(arg(0)?, arg(1)?),
            "note_envelope" => Ok(V::Note(Note::envelope(signal(arg(0)?)?, self.options))),
            "rings" => {
                let seconds = number(
                    take("seconds").ok_or("rings needs seconds:")?,
                    "how long a note rings",
                )?;
                match arg(0)? {
                    V::Note(note) => Ok(V::Note(note.within(seconds))),
                    sound => Ok(V::Note(Note::rings(signal(sound)?, seconds))),
                }
            }
            _ => Err(format!("{name} is not in the vocabulary")),
        }
    }

    /// The flute's air jet, its figures 0 unless given, with its `sound:` and its `side:`, from 0 to 1, each built on that output as its input.
    /// It draws its seed before its side and sound draw theirs, as the flute does.
    fn air_jet(&mut self, named: &[(String, Graph)]) -> Result<V, String> {
        let given = |key: &str| named.iter().find(|(k, _)| k == key).map(|(_, g)| g);
        let pitch = signal(self.eval(given("hz").ok_or("air_jet needs hz:")?)?)?;
        let breath = signal(self.eval(given("breath").ok_or("air_jet needs breath:")?)?)?;
        let mut figures = [0.0; 6];
        for (figure, key) in figures
            .iter_mut()
            .zip(["even", "wander", "catch", "corner", "tuned", "lift"])
        {
            match given(key) {
                Some(g) => *figure = number(self.eval(g)?, "an air jet's figure")?,
                None if key == "even" => return Err("air_jet needs even:".to_string()),
                None => {}
            }
        }
        let [even, wander, catch, corner, tuned, lift] = figures;
        let jet = crate::synths::played_jet(
            pitch,
            breath,
            self.ctx.seed(),
            even,
            wander,
            catch,
            corner,
            tuned,
            lift,
        );
        let side = self.through(given("side"))?;
        let sound = self.through(given("sound"))?.unwrap_or_else(|| net(pass()));
        Ok(V::Sig(match side {
            Some(side) => jet >> (sound | side) >> net(join::<U2>()),
            None => jet >> (sound | net(sink())),
        }))
    }

    /// The note's seed for `draw` and a drawn noise: the context's next seed mixed with how many notes came before, `mix` times; drawn once, at the first that asks.
    fn note_seed(&mut self, mix: Option<V>) -> Result<u64, String> {
        let mix = match mix {
            None => 0,
            Some(V::Num(x)) if x >= 0.0 && x.fract() == 0.0 && x < 2f64.powi(53) => x as u64,
            Some(V::Name(digits)) => digits
                .parse::<u64>()
                .map_err(|_| format!("a draw's mix is a whole number, not {digits}"))?,
            Some(_) => {
                return Err("a draw's mix is a whole number known when the note starts".to_string());
            }
        };
        match self.drawn {
            Some((taken, seed)) if taken == mix => Ok(seed),
            Some(_) => Err("each draw and drawn noise of a note takes the same mix".to_string()),
            None => {
                let seed = self.ctx.seed() ^ self.count.wrapping_mul(mix);
                self.drawn = Some((mix, seed));
                Ok(seed)
            }
        }
    }

    /// The sound a share's `through` makes of its one sound, built once and fed to each of its uses.
    fn share(&mut self, args: &[Graph], named: &[(String, Graph)]) -> Result<V, String> {
        let [sound] = args else {
            return Err("share takes the sound it shares".to_string());
        };
        let sound = signal(self.eval(sound)?)?;
        let through = named.iter().find(|(k, _)| k == "through").map(|(_, g)| g);
        let body = self.through(through)?.ok_or("share needs through:")?;
        Ok(V::Sig(sound >> body))
    }

    /// The sound `graph` makes of its input, when given.
    fn through(&mut self, graph: Option<&Graph>) -> Result<Option<Net>, String> {
        let Some(graph) = graph else {
            return Ok(None);
        };
        let outer = self.inputs.replace(Vec::new());
        let sound = self.eval(graph).and_then(signal);
        let uses = std::mem::replace(&mut self.inputs, outer).unwrap_or_default();
        Ok(Some(fed(&uses, 1, sound?)))
    }
}

/// Resonators struck by `sound`, each at a frequency that may change as the note sounds, as mxs's sitar's strings glide; a list of `freqs` gives one for each, with its level and fade.
fn gliding_modes(sound: Net, freqs: V, levels: V, fades: V) -> Result<V, String> {
    let (freqs, levels, fades) = match freqs {
        V::List(freqs) => (
            freqs,
            numbers(levels, "a mode's level")?,
            numbers(fades, "a mode's fade")?,
        ),
        freq => (
            vec![freq],
            vec![number(levels, "a mode's level")?],
            vec![number(fades, "a mode's fade")?],
        ),
    };
    let mut bank: Option<Net> = None;
    for ((freq, level), fade) in freqs.into_iter().zip(levels).zip(fades) {
        if level == 0.0 {
            continue;
        }
        let one = (net(pass()) | signal(freq)?) >> nodes::struck_gliding(level, fade);
        bank = Some(match bank {
            Some(bank) => bank & one,
            None => one,
        });
    }
    Ok(match bank {
        Some(bank) => V::Sig(sound >> bank),
        None => V::Num(0.0),
    })
}

/// A reed through its bell, from the arguments `take` hands over; kept out of [`Build::call`], whose frame it would otherwise grow.
#[inline(never)]
fn reed(take: &mut impl FnMut(&str) -> Option<V>, ctx: &mut Context) -> Result<V, String> {
    let mut sound = |key: &str, default: Option<f64>| -> Result<Net, String> {
        match (take(key), default) {
            (Some(value), _) => signal(value),
            (None, Some(default)) => signal(V::Num(default)),
            (None, None) => Err(format!("reed needs {key}:")),
        }
    };
    let pitch = sound("hz", None)?;
    let breath = sound("breath", None)?;
    let steep = sound("steep", Some(0.0))?;
    let through = sound("through", Some(1.0))?;
    let eased = sound("eased", Some(1.0))?;
    let mut known = |key: &str, default: Option<f64>| -> Result<f32, String> {
        match (take(key), default) {
            (Some(value), _) => Ok(number(value, &format!("a reed's {key}"))? as f32),
            (None, Some(default)) => Ok(default as f32),
            (None, None) => Err(format!("reed needs {key}:")),
        }
    };
    let set = crate::synths::Reeded {
        conical: known("conical", Some(0.0))? != 0.0,
        cutoff: known("least_corner", Some(0.0))?,
        corner: known("corner", None)?,
        keep: known("keep", None)?,
        zeta: known("zeta", Some(0.4))?,
        apex: known("apex", Some(0.3))?,
        relative: known("relative", Some(0.0))? != 0.0,
        tongue: known("tongue", Some(0.0))?,
        gasp: known("gasp", Some(0.0))?,
        mend: known("mend", Some(0.02))?,
        bell: known("bell", None)?,
        bell_corner: known("bell_corner", Some(0.0))?,
        smooth: known("smooth", Some(0.0))?,
        lift: known("lift", Some(0.0))?,
        catch: known("catch", Some(0.0))?,
    };
    // The reed draws its turbulence's seed after its arguments have drawn theirs.
    let seed = ctx.seed();
    Ok(V::Sig(crate::synths::reed_through_bell(
        pitch, breath, seed, steep, through, eased, set,
    )))
}

/// Segments from silence through each of `points`, each `[seconds, level]` straight, `[seconds, level, curve]` bent as `segments` bends them, or `[seconds, level, exp]` exponential.
fn envelope(points: V) -> Result<Net, String> {
    let V::List(points) = points else {
        return Err("an envelope takes a list of points".to_string());
    };
    let mut levels = vec![0.0];
    let mut times = Vec::new();
    let mut curves = Vec::new();
    for point in points {
        let V::List(parts) = point else {
            return Err("each point of an envelope is seconds and a level".to_string());
        };
        let mut parts = parts.into_iter();
        times.push(number(
            parts.next().ok_or("an envelope's point needs seconds")?,
            "an envelope's seconds",
        )?);
        levels.push(number(
            parts.next().ok_or("an envelope's point needs a level")?,
            "an envelope's level",
        )?);
        curves.push(match parts.next() {
            None => Some(0.0),
            Some(V::Num(curve)) => Some(curve),
            Some(V::Name(name)) if name == "exp" => None,
            Some(_) => return Err("an envelope's curve is a number or :exp".to_string()),
        });
    }
    Ok(nodes::shape(move |t| {
        let mut start = 0.0;
        for (k, &time) in times.iter().enumerate() {
            if t < start + time {
                let x = (t - start) / time;
                let (from, to) = (levels[k], levels[k + 1]);
                return match curves[k] {
                    None if from > 0.0 && to > 0.0 => from * (to / from).powf(x),
                    None => from + (to - from) * x,
                    Some(curve) => nodes::segment_at(&[from, to], &[1.0], &[curve], x),
                };
            }
            start += time;
        }
        *levels.last().unwrap_or(&0.0)
    }))
}
