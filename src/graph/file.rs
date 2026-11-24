//! Synths and effects read from JSON files, in the format the module's documentation describes.

use super::{Clause, Graph, in_vocabulary, needs};
use crate::{Effect, Kind, Spec, Synth, Value};
use serde_json::{Map, Value as Json};
use std::path::Path;

/// The newest version of the format this mxs reads.
const VERSION: u64 = 1;

/// A synth read from the text of an `mxs-synth` file, or why it cannot be read, naming the path in the file to what is wrong.
pub fn synth_from_json(text: &str) -> Result<Synth, String> {
    let file = File::parse(text, "mxs-synth")?;
    let clauses = file
        .list("clauses")?
        .iter()
        .enumerate()
        .map(|(i, clause)| file.clause(clause, &format!("clauses[{i}]")))
        .collect::<Result<_, _>>()?;
    crate::graph::synth(&file.name, file.specs()?, clauses)
}

/// A synth read from the `mxs-synth` file at `path`.
pub fn synth_from_json_file(path: impl AsRef<Path>) -> Result<Synth, String> {
    read(path.as_ref(), synth_from_json)
}

/// An effect read from the text of an `mxs-effect` file, or why it cannot be read, naming the path in the file to what is wrong.
pub fn effect_from_json(text: &str) -> Result<Effect, String> {
    let file = File::parse(text, "mxs-effect")?;
    let graph = file
        .top
        .get("graph")
        .ok_or("an mxs-effect file needs graph")?;
    let graph = file.value(graph, "graph", &mut Vec::new())?;
    crate::graph::effect(&file.name, file.specs()?, graph)
}

/// An effect read from the `mxs-effect` file at `path`.
pub fn effect_from_json_file(path: impl AsRef<Path>) -> Result<Effect, String> {
    read(path.as_ref(), effect_from_json)
}

fn read<T>(path: &Path, from: fn(&str) -> Result<T, String>) -> Result<T, String> {
    let shown = path.display();
    let text = std::fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))?;
    from(&text).map_err(|reason| format!("{shown}: {reason}"))
}

struct File {
    top: Map<String, Json>,
    name: String,
    defs: Map<String, Json>,
}

impl File {
    fn parse(text: &str, format: &str) -> Result<File, String> {
        let json: Json = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
        let Json::Object(top) = json else {
            return Err("a file is a JSON object".to_string());
        };
        let given = top.get("format").and_then(Json::as_str);
        match given {
            Some(f) if f == format => {}
            Some(f @ ("mxs-synth" | "mxs-effect")) => {
                return Err(format!("this is an {f} file, not an {format} one"));
            }
            Some(f) => return Err(format!("format {f} is neither mxs-synth nor mxs-effect")),
            None => return Err(format!("format is missing; this reads {format}")),
        }
        let version = top
            .get("version")
            .and_then(Json::as_u64)
            .filter(|v| *v >= 1)
            .ok_or("version is missing, or is not a whole number from 1")?;
        if version > VERSION {
            return Err(format!(
                "version {version} of {format} is newer than this mxs reads, which is version {VERSION}"
            ));
        }
        let name = top
            .get("name")
            .and_then(Json::as_str)
            .ok_or("name is missing, or is not text")?
            .to_string();
        let defs = match top.get("defs") {
            None => Map::new(),
            Some(Json::Object(defs)) => defs.clone(),
            Some(_) => return Err("defs: an object of named values".to_string()),
        };
        Ok(File { top, name, defs })
    }

    fn list(&self, key: &str) -> Result<&Vec<Json>, String> {
        match self.top.get(key) {
            Some(Json::Array(items)) => Ok(items),
            Some(_) => Err(format!("{key}: a list")),
            None => Err(format!("{key} is missing")),
        }
    }

    fn specs(&self) -> Result<Vec<Spec>, String> {
        match self.top.get("options") {
            None => Ok(Vec::new()),
            Some(_) => self
                .list("options")?
                .iter()
                .enumerate()
                .map(|(i, option)| spec(option, &format!("options[{i}]")))
                .collect(),
        }
    }

    fn clause(&self, clause: &Json, at: &str) -> Result<Clause, String> {
        let clause = object(clause, at, &["matches", "graph"])?;
        let matches = match clause.get("matches") {
            None => Vec::new(),
            Some(Json::Object(matches)) => matches
                .iter()
                .map(|(option, value)| {
                    let value = match value {
                        Json::String(name) => Value::Name(name.clone()),
                        other => Value::Number(number(other, &format!("{at}.matches.{option}"))?),
                    };
                    Ok((option.clone(), value))
                })
                .collect::<Result<_, String>>()?,
            Some(_) => return Err(format!("{at}.matches: an object of options and values")),
        };
        let graph = clause
            .get("graph")
            .ok_or(format!("{at}: graph is missing"))?;
        let graph = self.value(graph, &format!("{at}.graph"), &mut Vec::new())?;
        Ok(Clause { matches, graph })
    }

    /// The graph `json` stands for; `resolving` holds the defs being read, so a def that refers to itself is refused.
    fn value(&self, json: &Json, at: &str, resolving: &mut Vec<String>) -> Result<Graph, String> {
        stacker::maybe_grow(super::RED_ZONE, super::GROWTH, || match json {
            Json::Number(_) => Ok(Graph::Number(number(json, at)?)),
            Json::Array(items) => Ok(Graph::List(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| self.value(item, &format!("{at}[{i}]"), resolving))
                    .collect::<Result<_, _>>()?,
            )),
            Json::Object(fields) if fields.contains_key("call") => self.call(fields, at, resolving),
            Json::Object(fields) if fields.len() == 1 => {
                let (key, given) = fields.iter().next().unwrap();
                let text = || {
                    given
                        .as_str()
                        .map(str::to_string)
                        .ok_or(format!("{at}.{key}: text"))
                };
                match key.as_str() {
                    "option" => Ok(Graph::Option(text()?)),
                    "name" => Ok(Graph::Name(text()?)),
                    "ref" => self.reference(&text()?, at, resolving),
                    _ => Err(not_a_value(at)),
                }
            }
            _ => Err(not_a_value(at)),
        })
    }

    fn call(
        &self,
        fields: &Map<String, Json>,
        at: &str,
        resolving: &mut Vec<String>,
    ) -> Result<Graph, String> {
        object(
            &Json::Object(fields.clone()),
            at,
            &["call", "args", "named"],
        )?;
        let name = fields["call"].as_str().ok_or(format!("{at}.call: text"))?;
        if !in_vocabulary(name) {
            return Err(format!("{at}: {name} is not in the vocabulary"));
        }
        let args = match fields.get("args") {
            None => Vec::new(),
            Some(Json::Array(args)) => args
                .iter()
                .enumerate()
                .map(|(i, arg)| self.value(arg, &format!("{at}.args[{i}]"), resolving))
                .collect::<Result<_, _>>()?,
            Some(_) => return Err(format!("{at}.args: a list")),
        };
        let named: Vec<(String, Graph)> = match fields.get("named") {
            None => Vec::new(),
            Some(Json::Object(named)) => named
                .iter()
                .map(|(key, arg)| {
                    Ok((
                        key.clone(),
                        self.value(arg, &format!("{at}.named.{key}"), resolving)?,
                    ))
                })
                .collect::<Result<_, String>>()?,
            Some(_) => return Err(format!("{at}.named: an object of arguments by name")),
        };
        if let Some(missing) = needs(name)
            .iter()
            .find(|key| !named.iter().any(|(k, _)| k == *key))
        {
            return Err(format!("{at}: {name} needs {missing}"));
        }
        Ok(Graph::Call {
            name: name.to_string(),
            args,
            named,
        })
    }

    fn reference(
        &self,
        name: &str,
        at: &str,
        resolving: &mut Vec<String>,
    ) -> Result<Graph, String> {
        let def = self
            .defs
            .get(name)
            .ok_or(format!("{at}: ref {name} names no def"))?;
        if resolving.iter().any(|r| r == name) {
            return Err(format!("defs.{name} refers to itself"));
        }
        resolving.push(name.to_string());
        let graph = self.value(def, &format!("defs.{name}"), resolving);
        resolving.pop();
        graph
    }
}

fn not_a_value(at: &str) -> String {
    format!("{at}: a value is a number, a list, or an object of option, name, ref or call")
}

fn object<'a>(json: &'a Json, at: &str, keys: &[&str]) -> Result<&'a Map<String, Json>, String> {
    let Json::Object(fields) = json else {
        return Err(format!("{at}: an object"));
    };
    match fields.keys().find(|k| !keys.contains(&k.as_str())) {
        Some(key) => Err(format!("{at}: {key} is not one of {}", keys.join(", "))),
        None => Ok(fields),
    }
}

fn number(json: &Json, at: &str) -> Result<f64, String> {
    json.as_f64().ok_or(format!("{at}: a number"))
}

fn spec(json: &Json, at: &str) -> Result<Spec, String> {
    let fields = object(
        json,
        at,
        &[
            "name", "kind", "names", "default", "range", "required", "doc",
        ],
    )?;
    let text = |key: &str| fields.get(key).and_then(Json::as_str);
    let name = text("name").ok_or(format!("{at}: name is missing, or is not text"))?;
    let at = format!("{at} ({name})");
    let kind = match text("kind").ok_or(format!("{at}: kind is missing, or is not text"))? {
        "note" => Kind::Note,
        "hz" => Kind::Hz,
        "db" => Kind::Db,
        "seconds" => Kind::Seconds,
        "ms" => Kind::Ms,
        "beats" => Kind::Beats,
        "float" => Kind::Float,
        "integer" => Kind::Integer,
        "boolean" => Kind::Boolean,
        "name" => Kind::Name(
            fields
                .get("names")
                .and_then(Json::as_array)
                .and_then(|names| {
                    names
                        .iter()
                        .map(|n| n.as_str().map(str::to_string))
                        .collect()
                })
                .ok_or(format!(
                    "{at}: an option of kind name needs names, a list of text"
                ))?,
        ),
        other => {
            return Err(format!(
                "{at}: kind {other} is none of note, hz, db, seconds, beats, float, integer, boolean or name"
            ));
        }
    };
    let mut spec = Spec::new(name, kind);
    if spec.kind == Kind::Boolean {
        spec = spec.range(0.0, 1.0);
    }
    spec.default = match fields.get("default") {
        None | Some(Json::Null) => None,
        Some(Json::String(name)) => Some(Value::Name(name.clone())),
        Some(other) => Some(Value::Number(number(other, &format!("{at}.default"))?)),
    };
    if let Some(range) = fields.get("range") {
        let bound = |i: usize, unbounded: f64| match range.get(i) {
            Some(Json::Null) => Ok(unbounded),
            Some(n) => number(n, &format!("{at}.range[{i}]")),
            None => Err(format!(
                "{at}.range: a pair of numbers, null where unbounded"
            )),
        };
        spec = spec.range(bound(0, f64::NEG_INFINITY)?, bound(1, f64::INFINITY)?);
    }
    if fields.get("required").and_then(Json::as_bool) == Some(true) {
        spec = spec.required();
    }
    if let Some(doc) = text("doc") {
        spec = spec.doc(doc);
    }
    Ok(spec)
}
