//! The tables of the instruments fitted to recordings: compiled in with the `tables` feature, which is on by default, or loaded at run time from their bytes.

use super::figures::Figures;
use std::sync::OnceLock;

macro_rules! tables {
    ($($file:literal),* $(,)?) => {
        /// Every table, by the name of its file in mxs's `src/synths/figures`.
        const FILES: &[&str] = &[$($file),*];
        #[cfg(feature = "tables")]
        const COMPILED: &[&[u8]] = &[$(include_bytes!(concat!("figures/", $file))),*];
    };
}

tables![
    "cello.mxh",
    "deep_bass.mxf",
    "electric_bass.mxf",
    "electric_guitar.mxf",
    "electric_piano.mxf",
    "flute.mxh",
    "glockenspiel.mxf",
    "guitar.mxf",
    "harp.mxf",
    "marimba.mxf",
    "piano.mxf",
    "piano_pedal.mxf",
    "timpani.mxf",
    "vibraphone.mxf",
    "xylophone.mxf",
];

/// The tables each synth that needs any plays.
const SYNTHS: &[(&str, &[&str])] = &[
    ("cello", &["cello.mxh"]),
    ("deep_bass", &["deep_bass.mxf"]),
    ("electric_bass", &["electric_bass.mxf"]),
    ("electric_guitar", &["electric_guitar.mxf"]),
    ("electric_piano", &["electric_piano.mxf"]),
    ("flute", &["flute.mxh"]),
    ("glockenspiel", &["glockenspiel.mxf"]),
    ("guitar", &["guitar.mxf"]),
    ("harp", &["harp.mxf"]),
    ("marimba", &["marimba.mxf"]),
    ("piano", &["piano.mxf", "piano_pedal.mxf"]),
    ("timpani", &["timpani.mxf"]),
    ("vibraphone", &["vibraphone.mxf"]),
    ("xylophone", &["xylophone.mxf"]),
];

static LOADED: [OnceLock<Figures>; FILES.len()] = [const { OnceLock::new() }; FILES.len()];

fn index(file: &str) -> Option<usize> {
    FILES.iter().position(|f| *f == file)
}

/// The figures in the table `file`; the engine plays a note that reads them only once they are loaded.
pub(super) fn figures(file: &str) -> &'static Figures {
    let at = index(file).expect("a table mxs has");
    #[cfg(feature = "tables")]
    return LOADED[at].get_or_init(|| Figures::read(COMPILED[at]));
    #[cfg(not(feature = "tables"))]
    LOADED[at]
        .get()
        .unwrap_or_else(|| panic!("the table {file} is not loaded"))
}

/// The file of the table named `name`, its file's name with or without its extension, such as `marimba` or `marimba.mxf`.
pub(crate) fn file(name: &str) -> Option<&'static str> {
    FILES
        .iter()
        .copied()
        .find(|f| *f == name || f.rsplit_once('.').is_some_and(|(stem, _)| stem == name))
}

/// The tables the synth named plays, none for a synth that needs none.
pub(super) fn of(synth: &str) -> &'static [&'static str] {
    SYNTHS
        .iter()
        .find(|(name, _)| *name == synth)
        .map_or(&[], |(_, files)| files)
}

/// Those of `tables` not loaded yet; none with the tables compiled in.
pub(crate) fn missing(tables: &[&'static str]) -> Vec<&'static str> {
    tables
        .iter()
        .copied()
        .filter(|file| {
            !cfg!(feature = "tables") && index(file).is_some_and(|at| LOADED[at].get().is_none())
        })
        .collect()
}

/// Loads the table in the file named `file`, such as `piano.mxf`, from its bytes, for a build of mxs without the `tables` feature; [`Synth::tables`](crate::Synth::tables) names the tables a synth needs.
/// A table loaded once stays loaded, and with the tables compiled in the bytes are checked and the table compiled in is kept.
pub fn load_table(file: &str, bytes: &[u8]) -> Result<(), String> {
    let at = index(file).ok_or_else(|| format!("mxs has no table {file}"))?;
    let figures = Figures::try_read(bytes).map_err(|reason| format!("{file}: {reason}"))?;
    if !cfg!(feature = "tables") {
        let _ = LOADED[at].set(figures);
    }
    Ok(())
}
