//! The catalogs in `docs`, which the crate's documentation includes, are written from the synths and effects themselves.
//! `MXS_CATALOG=write cargo test --test catalog` writes them again after a synth or an effect changes.

use mxs::{Engine, Kind, RATE, Spec, Value, effects, synths};
use std::collections::BTreeSet;

const ENVELOPE: [&str; 7] = [
    "attack",
    "decay",
    "sustain",
    "release",
    "attack_level",
    "decay_level",
    "sustain_level",
];

fn number(x: f64) -> String {
    format!("{x}")
}

fn takes(spec: &Spec) -> String {
    let kind = match &spec.kind {
        Kind::Note => "a MIDI note".to_string(),
        Kind::Hz => "hertz".to_string(),
        Kind::Db => "decibels".to_string(),
        Kind::Seconds => "seconds".to_string(),
        Kind::Ms => "milliseconds".to_string(),
        Kind::Beats => "seconds".to_string(),
        Kind::Float => "a number".to_string(),
        Kind::Integer => "a whole number".to_string(),
        Kind::Boolean => return "0 or 1".to_string(),
        Kind::Name(values) => return format!("one of `{}`", values.join("`, `")),
    };
    match spec.range {
        Some((low, high)) if high.is_finite() => {
            format!("{kind}, {} to {}", number(low), number(high))
        }
        Some((low, _)) => format!("{kind}, {} or more", number(low)),
        None => kind,
    }
}

fn default(spec: &Spec) -> String {
    match &spec.default {
        Some(Value::Number(x)) => number(*x),
        Some(Value::Name(name)) => format!("`{name}`"),
        None if spec.required => "required".to_string(),
        None => String::new(),
    }
}

fn table(specs: &[&Spec]) -> String {
    let mut out = String::from("| Option | Takes | Default | What it does |\n|---|---|---|---|\n");
    for spec in specs {
        let doc = spec.doc.replace('|', "\\|");
        out.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            spec.name,
            takes(spec),
            default(spec),
            doc
        ));
    }
    out
}

fn find<'a>(specs: &'a [Spec], name: &str) -> Option<&'a Spec> {
    specs.iter().find(|s| s.name == name)
}

/// One synth's or effect's section: its own options in a table, and its envelope in a sentence when it has the whole of one.
fn section(name: &str, doc: &str, specs: &[Spec]) -> String {
    let enveloped = ENVELOPE.iter().all(|e| find(specs, e).is_some());
    let own: Vec<&Spec> = specs
        .iter()
        .filter(|s| !(enveloped && ENVELOPE.contains(&s.name.as_str())))
        .collect();
    let mut out = format!("### `{name}`\n\n");
    if !doc.is_empty() {
        out.push_str(&format!("{doc}\n\n"));
    }
    if !own.is_empty() {
        out.push_str(&table(&own));
        out.push('\n');
    }
    if enveloped {
        out.push_str(&format!(
            "It takes the envelope, with a `sustain` of {} and a `release` of {} unless given.\n\n",
            default(find(specs, "sustain").unwrap()),
            default(find(specs, "release").unwrap())
        ));
    }
    out
}

/// A MIDI note by its name, such as 59 as B3.
fn note_name(note: f64) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let note = note as i64;
    let (name, octave) = (NAMES[note.rem_euclid(12) as usize], note.div_euclid(12) - 1);
    format!("{name}{octave}")
}

/// The sentence that gives a synth's family and the notes it is meant to play.
fn meant(synth: &mxs::Synth) -> String {
    let family = synth.family();
    match synth.range() {
        Some((low, high)) => {
            let (low, high) = (note_name(low), note_name(high));
            format!("One of the {family}, meant to play from {low} to {high}.")
        }
        None => format!("One of the {family}, with no range of its own."),
    }
}

fn synths_catalog() -> String {
    let families = synths::families();
    let mut out = String::from("## The synths\n\n| Family | Synths |\n|---|---|\n");
    for (family, members) in &families {
        let names: Vec<String> = members
            .iter()
            .map(|s| format!("[`{0}`](#{0})", s.name()))
            .collect();
        let family = family.title();
        out.push_str(&format!("| {family} | {} |\n", names.join(", ")));
    }
    out.push_str("\n## What every note takes\n\nBesides its synth's own options, every note takes these.\n\n");
    let placing = Engine::offline(RATE).note_specs();
    out.push_str(&table(&placing.iter().collect::<Vec<_>>()));
    out.push_str("\n## The envelope\n\nA synth that takes the envelope takes these seven options, with its own defaults for `sustain` and `release`.\n\n");
    let envelope = Spec::envelope(0.0, 0.0);
    let rows: Vec<&Spec> = envelope.iter().collect();
    out.push_str(
        &table(&rows)
            .replace("| 0 | How long it then holds", "| | How long it then holds")
            .replace(
                "| 0 | How long it then takes to fall silent",
                "| | How long it then takes to fall silent",
            ),
    );
    out.push('\n');
    for (family, members) in &families {
        let family = family.title();
        out.push_str(&format!("## {family}\n\n"));
        for synth in members {
            let doc = format!("{} {}", synth.doc(), meant(synth))
                .trim()
                .to_string();
            out.push_str(&section(synth.name(), &doc, synth.specs()));
        }
    }
    out
}

fn effects_catalog() -> String {
    let all = effects::all();
    let names: Vec<String> = all
        .iter()
        .map(|e| format!("[`{0}`](#{0})", e.name()))
        .collect();
    let mut out = format!("## The effects\n\n{}.\n\n", names.join(", "));
    out.push_str(
        "## What every effect takes\n\nBesides its own options, every effect takes these.\nAn effect that lists one of them among its own, as `phaser` lists `mix`, gives it its own default.\n\n",
    );
    let common = Engine::offline(RATE).effect_specs();
    out.push_str(&table(&common.iter().collect::<Vec<_>>()));
    out.push('\n');
    for effect in &all {
        out.push_str(&section(effect.name(), "", effect.specs()));
    }
    out
}

fn check(path: &str, catalog: String) {
    if std::env::var("MXS_CATALOG").as_deref() == Ok("write") {
        std::fs::write(path, &catalog).unwrap();
    }
    let written = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        written == catalog,
        "{path} is out of date; run MXS_CATALOG=write cargo test --test catalog"
    );
}

#[test]
fn the_synths_catalog_lists_every_synth_with_its_options() {
    check("docs/synths.md", synths_catalog());
}

#[test]
fn the_effects_catalog_lists_every_effect_with_its_options() {
    check("docs/effects.md", effects_catalog());
}

#[test]
fn every_synth_says_what_it_is() {
    for synth in synths::all() {
        assert!(
            synth.doc().ends_with('.'),
            "{} has no description",
            synth.name()
        );
    }
    assert!(!synths::piano().doc().is_empty());
    assert!(!synths::berimbau().doc().is_empty());
}

#[test]
fn the_families_hold_every_synth_once() {
    let all: Vec<String> = synths::all().iter().map(|s| s.name().to_string()).collect();
    let in_families: Vec<String> = synths::families()
        .iter()
        .flat_map(|(_, members)| members.iter().map(|s| s.name().to_string()))
        .collect();
    assert_eq!(all.len(), in_families.len());
    assert_eq!(
        all.iter().collect::<BTreeSet<_>>(),
        in_families.iter().collect::<BTreeSet<_>>()
    );
}

#[test]
fn an_effect_time_not_in_beats_names_its_unit() {
    for effect in effects::all() {
        for spec in effect.specs() {
            let name = spec.name.as_str();
            let at = format!("{}'s {name}", effect.name());
            match spec.kind {
                Kind::Seconds => assert!(name.ends_with("_seconds"), "{at} is in seconds"),
                Kind::Ms => assert!(name.ends_with("_ms"), "{at} is in milliseconds"),
                _ => {
                    assert!(
                        !name.ends_with("_seconds") && !name.ends_with("_ms"),
                        "{at}"
                    );
                    assert!(!name.ends_with("_time"), "{at} names no unit");
                    assert!(!spec.doc.contains("seconds"), "{at} says seconds");
                }
            }
        }
    }
}
