//! The tables of the instruments fitted to recordings: compiled in with the `tables` feature, which is on by default, or loaded at run time from their bytes.

use super::figures::Figures;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

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

/// The tables loaded from outside mxs, such as those mx's fitting writes, by their files' names; each stays loaded.
static OUTSIDE: Mutex<Option<HashMap<&'static str, &'static Figures>>> = Mutex::new(None);

/// The table from outside mxs whose file is named `name`, with or without its extension.
fn outside(name: &str) -> Option<(&'static str, &'static Figures)> {
    let outside = OUTSIDE.lock().unwrap_or_else(|e| e.into_inner());
    outside.as_ref()?.iter().find_map(|(file, figures)| {
        let stem = file.rsplit_once('.').map_or(*file, |(stem, _)| stem);
        (*file == name || stem == name).then_some((*file, *figures))
    })
}

fn index(file: &str) -> Option<usize> {
    FILES.iter().position(|f| *f == file)
}

/// The figures in the table `file`; the engine plays a note that reads them only once they are loaded.
pub(super) fn figures(file: &str) -> &'static Figures {
    let Some(at) = index(file) else {
        return outside(file).expect("a table loaded").1;
    };
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
        .or_else(|| outside(name).map(|(file, _)| file))
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
/// A table mxs does not carry, such as one fitted with mx, is kept under its file's name, which a graph's `figures` names it by; loaded again, it is replaced.
pub fn load_table(file: &str, bytes: &[u8]) -> Result<(), String> {
    let figures = Figures::try_read(bytes).map_err(|reason| format!("{file}: {reason}"))?;
    let Some(at) = index(file) else {
        let mut outside = OUTSIDE.lock().unwrap_or_else(|e| e.into_inner());
        let outside = outside.get_or_insert_with(HashMap::new);
        let name = outside
            .keys()
            .copied()
            .find(|f| *f == file)
            .unwrap_or_else(|| String::leak(file.to_string()));
        outside.insert(name, Box::leak(Box::new(figures)));
        return Ok(());
    };
    if !cfg!(feature = "tables") {
        let _ = LOADED[at].set(figures);
    }
    Ok(())
}
