//! mxs as WebAssembly for the mxs npm package: one engine an instance, driven by requests in JSON, with blocks of samples rendered for an audio worklet or an offline render.
//! It takes plain numbers and bytes in its own memory, so it needs no bindings generator and no imports, and has no tables compiled in, so a page fetches only those it plays.

use engine::{Effect, Engine, Event, Kind, Spec, Synth, Target, Value, effects, synths};
use serde_json::{Map, Value as Json, json};
use std::cell::RefCell;

struct State {
    engine: Engine,
    room: Vec<u8>,
    answer: Vec<u8>,
    block: Vec<f32>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
    static PANIC: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with<T>(f: impl FnOnce(&mut State) -> T) -> T {
    STATE.with(|state| f(state.borrow_mut().as_mut().expect("start is called first")))
}

/// Starts an engine at `rate` with mxs's synths and effects, and a limit of `voices` sounding at once, or mxs's own limit for 0.
#[unsafe(no_mangle)]
pub extern "C" fn start(rate: f64, voices: usize) {
    std::panic::set_hook(Box::new(|info| {
        PANIC.with(|message| *message.borrow_mut() = info.to_string().into_bytes())
    }));
    let mut engine = Engine::offline(rate);
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    if voices > 0 {
        engine.set_voice_limit(voices);
    }
    STATE.with(|state| {
        *state.borrow_mut() = Some(State {
            engine,
            room: Vec::new(),
            answer: Vec::new(),
            block: Vec::new(),
        })
    });
}

/// The message of the last panic, which WebAssembly otherwise reports only as unreachable code: its length in four bytes, then the text.
#[unsafe(no_mangle)]
pub extern "C" fn panic_message() -> *const u8 {
    PANIC.with(|message| {
        let mut message = message.borrow_mut();
        let text = std::mem::take(&mut *message);
        message.extend_from_slice(&(text.len() as u32).to_le_bytes());
        message.extend_from_slice(&text);
        message.as_ptr()
    })
}

/// Room for `length` bytes, where the page writes a request, or a file's name and then its bytes.
#[unsafe(no_mangle)]
pub extern "C" fn room(length: usize) -> *mut u8 {
    with(|state| {
        state.room.clear();
        state.room.resize(length, 0);
        state.room.as_mut_ptr()
    })
}

/// Answers the request in JSON in the first `length` bytes of the room, with `{"ok": value}` or `{"error": message}` in JSON: its length in four bytes, then the text.
#[unsafe(no_mangle)]
pub extern "C" fn call(length: usize) -> *const u8 {
    with(|state| {
        let answer = serde_json::from_slice::<Json>(&state.room[..length])
            .map_err(|e| format!("a request is JSON: {e}"))
            .and_then(|request| answer(&mut state.engine, &request));
        answered(state, answer)
    })
}

/// Loads the table whose file is named by the first `name` bytes of the room, from the bytes after them, and answers as [`call`] does.
#[unsafe(no_mangle)]
pub extern "C" fn load_table(name: usize) -> *const u8 {
    with(|state| {
        let answer = std::str::from_utf8(&state.room[..name])
            .map_err(|e| e.to_string())
            .and_then(|file| synths::load_table(file, &state.room[name..]))
            .map(|()| Json::Null);
        answered(state, answer)
    })
}

/// Loads the sound file whose name is the first `name` bytes of the room, from the bytes after them, and answers with the number a sample plays it by.
#[unsafe(no_mangle)]
pub extern "C" fn load_sample(name: usize) -> *const u8 {
    with(|state| {
        let shown = String::from_utf8_lossy(&state.room[..name]).into_owned();
        let bytes = state.room[name..].to_vec();
        let answer = state
            .engine
            .load_sample_bytes(&shown, &bytes)
            .map(Json::from)
            .map_err(|e| e.to_string());
        answered(state, answer)
    })
}

/// Renders the next `frames` frames, the left channel and then the right.
#[unsafe(no_mangle)]
pub extern "C" fn render(frames: usize) -> *const f32 {
    with(|state| {
        let rate = state.engine.rate();
        state.block.clear();
        state.block.resize(frames * 2, 0.0);
        if let Ok((left, right)) = state.engine.render(frames as f64 / rate) {
            let count = left.len().min(frames);
            state.block[..count].copy_from_slice(&left[..count]);
            state.block[frames..frames + count].copy_from_slice(&right[..count]);
        }
        state.block.as_ptr()
    })
}

fn answered(state: &mut State, answer: Result<Json, String>) -> *const u8 {
    let text = match answer {
        Ok(value) => json!({ "ok": value }),
        Err(message) => json!({ "error": message }),
    }
    .to_string();
    state.answer.clear();
    state
        .answer
        .extend_from_slice(&(text.len() as u32).to_le_bytes());
    state.answer.extend_from_slice(text.as_bytes());
    state.answer.as_ptr()
}

fn answer(engine: &mut Engine, request: &Json) -> Result<Json, String> {
    let field = |name: &str| request.get(name).ok_or(format!("the request needs {name}"));
    let text = |name: &str| {
        field(name)?
            .as_str()
            .map(str::to_string)
            .ok_or(format!("{name} is text"))
    };
    match text("op")?.as_str() {
        "synths" => Ok(engine.synths().map(synth).collect()),
        "effects" => Ok(engine.effects().map(effect).collect()),
        "now" => Ok(engine.now().into()),
        "play" => {
            let now = engine.now();
            let events = field("events")?
                .as_array()
                .ok_or("events is a list")?
                .iter()
                .map(|e| event(e).map(|(after, event)| (now + after, event)))
                .collect::<Result<Vec<_>, String>>()?;
            engine.play(&events).map_err(|e| e.to_string())?;
            Ok(Json::Null)
        }
        "start_effects" => {
            let chain = field("effects")?
                .as_array()
                .ok_or("effects is a list of [name, options]")?
                .iter()
                .map(|e| match e.as_array().map(Vec::as_slice) {
                    Some([Json::String(name), given]) => Ok((name.clone(), options(given)?)),
                    Some([Json::String(name)]) => Ok((name.clone(), Vec::new())),
                    _ => Err("each effect is [name, options]".to_string()),
                })
                .collect::<Result<Vec<_>, String>>()?;
            let chain = engine.start_effects(&chain).map_err(|e| e.to_string())?;
            Ok(chain.into())
        }
        "load_synth" => {
            let synth = Synth::from_json(&text("text")?)?;
            let name = synth.name().to_string();
            engine.register(synth);
            Ok(name.into())
        }
        "load_effect" => {
            let effect = Effect::from_json(&text("text")?)?;
            let name = effect.name().to_string();
            engine.register_effect(effect);
            Ok(name.into())
        }
        other => Err(format!("no request {other}")),
    }
}

/// An event and the seconds after now it plays at.
fn event(json: &Json) -> Result<(f64, Event), String> {
    let field = |name: &str| json.get(name);
    let after = field("after").and_then(Json::as_f64).unwrap_or(0.0);
    let id = field("id").and_then(Json::as_u64);
    let chain = field("chain").and_then(Json::as_u64);
    let given = field("options").map_or(Ok(Vec::new()), options)?;
    let ids = || {
        field("ids")
            .and_then(Json::as_array)
            .and_then(|ids| ids.iter().map(Json::as_u64).collect::<Option<Vec<_>>>())
            .ok_or("ids is a list of numbers".to_string())
    };
    let event = match field("kind").and_then(Json::as_str) {
        Some("note") => Event::Note {
            id,
            synth: field("synth")
                .and_then(Json::as_str)
                .ok_or("a note needs synth")?
                .to_string(),
            options: given,
            chain,
            held: field("held").and_then(Json::as_bool).unwrap_or(false),
        },
        Some("sample") => Event::Sample {
            id,
            sample: field("sample")
                .and_then(Json::as_u64)
                .ok_or("a sample needs the number load_sample gave")?,
            options: given,
            chain,
        },
        Some("set") => Event::Set {
            target: Target::Sounds(ids()?),
            options: given
                .into_iter()
                .map(|(name, value)| match value {
                    Value::Number(n) => Ok((name, n)),
                    Value::Name(_) => Err(format!("set takes numbers, and {name} is not one")),
                })
                .collect::<Result<_, String>>()?,
        },
        Some("release") => Event::Release { ids: ids()? },
        Some("stop") => Event::Stop { ids: ids()? },
        _ => return Err("an event's kind is note, sample, set, release or stop".to_string()),
    };
    Ok((after, event))
}

fn options(json: &Json) -> Result<Vec<(String, Value)>, String> {
    let Json::Object(given) = json else {
        return Err("options are an object of names and values".to_string());
    };
    given
        .iter()
        .map(|(name, value)| match value {
            Json::Number(n) => Ok((name.clone(), Value::Number(n.as_f64().unwrap_or(0.0)))),
            Json::String(s) => Ok((name.clone(), Value::Name(s.clone()))),
            _ => Err(format!("the option {name} is a number or text")),
        })
        .collect()
}

fn synth(synth: &Synth) -> Json {
    json!({
        "name": synth.name(),
        "doc": synth.doc(),
        "tables": synth.tables(),
        "options": synth.specs().iter().map(spec).collect::<Vec<_>>(),
    })
}

fn effect(effect: &Effect) -> Json {
    json!({
        "name": effect.name(),
        "options": effect.specs().iter().map(spec).collect::<Vec<_>>(),
    })
}

fn spec(spec: &Spec) -> Json {
    let mut fields = Map::new();
    fields.insert("name".into(), spec.name.clone().into());
    let kind = match &spec.kind {
        Kind::Note => "note",
        Kind::Hz => "hz",
        Kind::Db => "db",
        Kind::Seconds => "seconds",
        Kind::Ms => "ms",
        Kind::Beats => "beats",
        Kind::Float => "float",
        Kind::Integer => "integer",
        Kind::Boolean => "boolean",
        Kind::Name(names) => {
            fields.insert("names".into(), names.clone().into());
            "name"
        }
    };
    fields.insert("kind".into(), kind.into());
    let default = match &spec.default {
        Some(Value::Number(n)) => Json::from(*n),
        Some(Value::Name(name)) => Json::from(name.clone()),
        None => Json::Null,
    };
    fields.insert("default".into(), default);
    let bound = |b: f64| {
        if b.is_finite() {
            Json::from(b)
        } else {
            Json::Null
        }
    };
    let range = spec
        .range
        .map_or(Json::Null, |(low, high)| json!([bound(low), bound(high)]));
    fields.insert("range".into(), range);
    fields.insert("required".into(), spec.required.into());
    fields.insert("doc".into(), spec.doc.clone().into());
    Json::Object(fields)
}
