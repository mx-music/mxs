//! Each synth's family, and the notes it is meant to play.

use mxs::{Family, synths};

fn synth(name: &str) -> mxs::Synth {
    synths::all()
        .into_iter()
        .find(|s| s.name() == name)
        .unwrap()
}

#[test]
fn the_flute_is_a_wind_meant_for_b3_to_d_flat_7() {
    let flute = synth("flute");
    assert_eq!(flute.family(), Family::Winds);
    assert_eq!(flute.range(), Some((59.0, 97.0)));
}

#[test]
fn the_clarinet_plays_the_notes_it_was_fitted_to_from_d3_to_c7() {
    let clarinet = synth("clarinet");
    assert_eq!(clarinet.family(), Family::Winds);
    assert_eq!(clarinet.range(), Some((50.0, 96.0)));
}

#[test]
fn the_piano_plays_further_than_it_was_fitted_from_a0_to_c8() {
    assert_eq!(synth("piano").range(), Some((21.0, 108.0)));
}

#[test]
fn saw_is_a_synth_with_no_range() {
    let saw = synth("saw");
    assert_eq!(saw.family(), Family::Synths);
    assert_eq!(saw.range(), None);
}

#[test]
fn the_families_are_the_sounds_panels_in_its_order() {
    let names: Vec<&str> = synths::families()
        .iter()
        .map(|(family, _)| family.title())
        .collect();
    assert_eq!(
        names,
        [
            "Keys",
            "Plucked strings",
            "Bowed strings",
            "Winds",
            "Mallets",
            "Synths",
            "Drums"
        ]
    );
    let of = |family: Family| -> Vec<String> {
        let mut names: Vec<String> = synths::all()
            .iter()
            .filter(|s| s.family() == family)
            .map(|s| s.name().to_string())
            .collect();
        names.sort();
        names
    };
    assert_eq!(
        of(Family::Keys),
        ["accordion", "electric_piano", "harmonium", "organ", "piano"]
    );
    assert_eq!(
        of(Family::Mallets),
        [
            "glockenspiel",
            "marimba",
            "timpani",
            "vibraphone",
            "xylophone"
        ]
    );
    assert_eq!(
        of(Family::Synths),
        [
            "acid",
            "deep_bass",
            "fm",
            "noise",
            "pulse",
            "saw",
            "square",
            "supersaw",
            "tri"
        ]
    );
    assert!(of(Family::PluckedStrings).contains(&"berimbau".to_string()));
    assert!(of(Family::PluckedStrings).contains(&"tanpura".to_string()));
    assert!(of(Family::Drums).contains(&"kick".to_string()));
}

#[test]
fn a_range_runs_low_to_high_on_a_pitched_synth_and_drums_have_none() {
    for synth in synths::all() {
        if let Some((low, high)) = synth.range() {
            assert!(low < high, "{}", synth.name());
        }
        if synth.family() == Family::Drums {
            assert_eq!(synth.range(), None, "{}", synth.name());
        }
    }
}

#[test]
fn a_family_reads_as_its_name_in_lower_case() {
    assert_eq!(Family::PluckedStrings.name(), "plucked strings");
    assert_eq!(Family::Winds.to_string(), "winds");
}
