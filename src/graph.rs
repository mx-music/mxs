//! Synths described as data, as mx's defsynth traces them: each clause a graph of calls on the note's options, which the engine builds into a sound when a note starts.
//! An option is read once, when the note starts; arithmetic on options and numbers is worked out then, and a node whose level works out to 0 is left out.

use crate::nodes::{self, db, hz};
use crate::{Context, Note, Options, Spec, Synth, Value};
use fundsp::prelude::*;

/// A traced value: a number, a name such as a noise's colour, one of the note's options, a list, or a call of the vocabulary.
#[derive(Clone, Debug, PartialEq)]
pub enum Graph {
    Number(f64),
    Name(String),
    Option(String),
    List(Vec<Graph>),
    Call { name: String, args: Vec<Graph>, named: Vec<(String, Graph)> },
}

/// One clause of a synth: the values of options it matches, and the graph it traces to.
#[derive(Clone, Debug, PartialEq)]
pub struct Clause {
    pub matches: Vec<(String, Value)>,
    pub graph: Graph,
}

impl Graph {
    pub fn call(name: &str, args: Vec<Graph>, named: Vec<(&str, Graph)>) -> Graph {
        let named = named.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        Graph::Call { name: name.to_string(), args, named }
    }
}

const ARITHMETIC: &[&str] = &[
    "add", "sub", "mul", "div", "pow", "neg", "min", "max", "clamp", "log2", "exp", "abs", "sin", "cos", "hz", "db", "lt", "gt", "le", "ge", "eq", "ne", "select",
];
const NODES: &[&str] = &[
    "impulse", "sine", "saw", "square", "triangle", "pulse", "noise", "lowpass", "highpass", "bandpass", "lowshelf", "highshelf", "peak", "mode", "envelope", "perc",
    "mix", "gain", "note_envelope", "rings",
];

impl Synth {
    /// A synth whose note is built from the first of `clauses` whose matches its options meet, or is silent when none does.
    pub fn graph(name: &str, specs: Vec<Spec>, clauses: Vec<Clause>) -> Result<Synth, String> {
        for clause in &clauses {
            known(&clause.graph).map_err(|reason| format!("{name}: {reason}"))?;
        }
        let owner = name.to_string();
        Ok(Synth::new(name).options(specs).build(move |options, ctx| {
            let Some(clause) = clauses.iter().find(|c| c.matches.iter().all(|(k, v)| options.value(k) == v)) else {
                return Note::rings(dc(0.0f32), 0.0);
            };
            match (Build { options, ctx }).eval(&clause.graph) {
                Ok(V::Note(note)) => note,
                Ok(_) => panic!("{owner} ends with neither note_envelope nor rings"),
                Err(reason) => panic!("{owner}: {reason}"),
            }
        }))
    }
}

fn known(graph: &Graph) -> Result<(), String> {
    match graph {
        Graph::List(items) => items.iter().try_for_each(known),
        Graph::Call { name, args, named } => {
            if !ARITHMETIC.contains(&name.as_str()) && !NODES.contains(&name.as_str()) {
                return Err(format!("{name} is not in the vocabulary"));
            }
            args.iter().chain(named.iter().map(|(_, v)| v)).try_for_each(known)
        }
        _ => Ok(()),
    }
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

fn binary(a: V, b: V, f: fn(f64, f64) -> f64) -> Result<V, String> {
    match (a, b) {
        (V::Num(a), V::Num(b)) => Ok(V::Num(f(a, b))),
        (a, b) => Ok(V::Sig((signal(a)? | signal(b)?) >> net(map(move |x: &Frame<f32, U2>| f(x[0] as f64, x[1] as f64) as f32)))),
    }
}

fn unary(a: V, f: fn(f64) -> f64) -> Result<V, String> {
    match a {
        V::Num(a) => Ok(V::Num(f(a))),
        a => Ok(V::Sig(signal(a)? >> net(map(move |x: &Frame<f32, U1>| f(x[0] as f64) as f32)))),
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
}

impl Build<'_> {
    fn eval(&mut self, graph: &Graph) -> Result<V, String> {
        match graph {
            Graph::Number(x) => Ok(V::Num(*x)),
            Graph::Name(name) => Ok(V::Name(name.clone())),
            Graph::Option(name) => Ok(V::Num(self.options.number(name))),
            Graph::List(items) => Ok(V::List(items.iter().map(|g| self.eval(g)).collect::<Result<_, _>>()?)),
            Graph::Call { name, args, named } => self.call(name, args, named),
        }
    }

    fn call(&mut self, name: &str, args: &[Graph], named: &[(String, Graph)]) -> Result<V, String> {
        let mut args: Vec<V> = args.iter().map(|g| self.eval(g)).collect::<Result<_, _>>()?;
        let count = args.len();
        let mut named: Vec<(String, V)> = named.iter().map(|(k, g)| Ok((k.clone(), self.eval(g)?))).collect::<Result<_, String>>()?;
        let mut take = |key: &str| named.iter().position(|(k, _)| k == key).map(|i| named.swap_remove(i).1);
        let mut arg = |i: usize| -> Result<V, String> {
            if i < args.len() { Ok(std::mem::replace(&mut args[i], V::Num(0.0))) } else { Err(format!("{name} takes {} arguments", i + 1)) }
        };
        match name {
            "add" => add(arg(0)?, arg(1)?),
            "sub" => add(arg(0)?, mul(arg(1)?, V::Num(-1.0))?),
            "mul" => mul(arg(0)?, arg(1)?),
            "div" => binary(arg(0)?, arg(1)?, |a, b| a / b),
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
                    Ok(if c != 0.0 { then } else { otherwise })
                }
                c => {
                    let picked = (signal(c)? | signal(arg(1)?)? | signal(arg(2)?)?)
                        >> net(map(|x: &Frame<f32, U3>| if x[0] != 0.0 { x[1] } else { x[2] }));
                    Ok(V::Sig(picked))
                }
            },
            "impulse" => Ok(V::Sig(net(impulse::<U1>()))),
            "sine" => {
                let freq = signal(take("hz").ok_or("sine needs hz:")?)?;
                let phase = take("phase").map(|p| number(p, "a sine's phase")).transpose()?.unwrap_or(0.0);
                Ok(V::Sig(freq >> net(An(Sine::<f32>::with_phase((phase / std::f64::consts::TAU).rem_euclid(1.0) as f32)))))
            }
            "saw" | "square" | "triangle" => {
                let freq = signal(take("hz").ok_or(format!("{name} needs hz:"))?)?;
                let mut wave: Net = match name {
                    "saw" => net(saw()),
                    "square" => net(square()),
                    _ => net(triangle()),
                };
                wave.set(Setting::phase(0.0));
                wave.reset();
                Ok(V::Sig(freq >> wave))
            }
            "pulse" => {
                let freq = signal(take("hz").ok_or("pulse needs hz:")?)?;
                let width = signal(take("width").unwrap_or(V::Num(0.5)))?;
                let mut wave = net(pulse());
                wave.set(Setting::phase(0.0));
                wave.reset();
                Ok(V::Sig((freq | width) >> wave))
            }
            "noise" => match arg(0)? {
                V::Name(colour) if colour == "white" => Ok(V::Sig(nodes::white_noise(self.ctx.seed()))),
                V::Name(colour) if colour == "pink" => Ok(V::Sig(nodes::pink_noise(self.ctx.seed()))),
                _ => Err("noise takes :white or :pink".to_string()),
            },
            "lowpass" | "highpass" | "bandpass" => {
                let mut sound = signal(arg(0)?)?;
                let freq = take("hz").ok_or(format!("{name} needs hz:"))?;
                let q = take("q").map(|q| number(q, "a filter's q")).transpose()?.unwrap_or(std::f64::consts::FRAC_1_SQRT_2);
                let poles = take("poles").map(|p| number(p, "a filter's poles")).transpose()?.unwrap_or(2.0);
                let freq = binary(binary(freq, V::Num(20.0), f64::max)?, V::Num(20_000.0), f64::min)?;
                let freq = signal(freq)?;
                for _ in 0..((poles / 2.0).round().max(1.0) as usize) {
                    let filter: Net = match name {
                        "lowpass" => net(lowpass::<f32>()),
                        "highpass" => net(highpass::<f32>()),
                        // FunDSP's band-pass peaks at its q, so it is scaled back to a peak of 1.
                        _ => net(bandpass::<f32>()) * (1.0 / q) as f32,
                    };
                    sound = (sound | freq.clone() | net(dc(q as f32))) >> filter;
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
                let freq = number(take("hz").ok_or("mode needs hz:")?, "a mode's hz")?;
                let level = number(take("level").ok_or("mode needs level:")?, "a mode's level")?;
                let fade = number(take("fade").ok_or("mode needs fade:")?, "a mode's fade")?;
                if level == 0.0 || freq >= 20_000.0 {
                    return Ok(V::Num(0.0));
                }
                Ok(V::Sig(signal(sound)? >> nodes::struck(freq, level, fade)))
            }
            "envelope" => {
                let (sound, points) = if count == 2 { (Some(arg(0)?), arg(1)?) } else { (None, arg(0)?) };
                let shape = envelope(points)?;
                match sound {
                    Some(sound) => mul(sound, V::Sig(shape)),
                    None => Ok(V::Sig(shape)),
                }
            }
            "perc" => {
                let attack = number(take("attack").ok_or("perc needs attack:")?, "a perc's attack")?;
                let release = number(take("release").ok_or("perc needs release:")?, "a perc's release")?;
                let shape = V::Sig(nodes::shape(move |t| nodes::perc(t, attack, release)));
                if count == 0 { Ok(shape) } else { mul(arg(0)?, shape) }
            }
            "mix" => match arg(0)? {
                V::List(items) => items.into_iter().try_fold(V::Num(0.0), add),
                _ => Err("mix takes a list".to_string()),
            },
            "gain" => mul(arg(0)?, arg(1)?),
            "note_envelope" => Ok(V::Note(Note::envelope(signal(arg(0)?)?, self.options))),
            "rings" => {
                let seconds = number(take("seconds").ok_or("rings needs seconds:")?, "how long a note rings")?;
                match arg(0)? {
                    V::Note(note) => Ok(V::Note(note.within(seconds))),
                    sound => Ok(V::Note(Note::rings(signal(sound)?, seconds))),
                }
            }
            _ => Err(format!("{name} is not in the vocabulary")),
        }
    }
}

/// Segments from silence through each of `points`, each `[seconds, level]` straight, `[seconds, level, curve]` bent as `segments` bends them, or `[seconds, level, exp]` exponential.
fn envelope(points: V) -> Result<Net, String> {
    let V::List(points) = points else { return Err("an envelope takes a list of points".to_string()) };
    let mut levels = vec![0.0];
    let mut times = Vec::new();
    let mut curves = Vec::new();
    for point in points {
        let V::List(parts) = point else { return Err("each point of an envelope is seconds and a level".to_string()) };
        let mut parts = parts.into_iter();
        times.push(number(parts.next().ok_or("an envelope's point needs seconds")?, "an envelope's seconds")?);
        levels.push(number(parts.next().ok_or("an envelope's point needs a level")?, "an envelope's level")?);
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
