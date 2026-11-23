//! The synths that come with the crate, by family, each with the options it takes.
//!
//! An engine plays the synths registered on it, by name, and checks each note's options against what the synth declares:
//!
//! ```
//! use mxs::{Engine, Event, RATE, synths};
//!
//! fn main() -> Result<(), mxs::Error> {
//!     let mut engine = Engine::offline(RATE);
//!     synths::all().into_iter().for_each(|s| engine.register(s));
//!
//!     let note = Event::note(None, "marimba", &[("note", 60.0), ("vel", 0.8)]);
//!     engine.play(&[(0.0, note)])?;
//!     engine.render(2.0)?;
//!
//!     let wrong = Event::note(None, "marimba", &[("wobble", 1.0)]);
//!     assert!(engine.play(&[(0.0, wrong)]).is_err());
//!     Ok(())
//! }
//! ```
//!
//! A time is in seconds when a note is handed to [`Engine::play`](crate::Engine::play).
//! In a pass, handed to [`Engine::pass`](crate::Engine::pass), the envelope's times and the slides are in beats.
//! Almost every synth takes `note`, and most take `vel`, how hard the note is played.
//! A later change of an option glides over the seconds its partner ending in `_slide` gives, such as `note_slide`.
//!
#![doc = include_str!("../../docs/synths.md")]

mod basic;
mod bowed;
mod common;
mod drums;
mod electric;
mod figures;
mod organs;
mod strings;
mod tables;
mod tuned;
mod winds;

use crate::Family;
pub(crate) use bowed::bowed_string;
pub(crate) use common::partials;
pub(crate) use tables::file as table_file;

/// The files a graph naming the table `name` plays: those of mxs's synth of that name, as the piano plays two, or the one the name gives.
pub(crate) fn table_files(name: &str) -> Vec<&'static str> {
    match tables::of(name) {
        [] => tables::file(name).into_iter().collect(),
        files => files.to_vec(),
    }
}
pub use tables::load_table;
pub(crate) use tables::missing as missing_tables;
pub use winds::sway;
pub(crate) use winds::{
    Reeded, drifting_swing, fingers, played_jet, reed_through_bell, wind_bands,
};

/// A struck note from the table in `file`, played as mxs's voices play theirs: `kept`, whose figures carry its loudness, `fitted`, louder by vel to the power `growth`, or `evened`, every note at vel 1 as loud, each at `level`.
pub(crate) fn struck_from(
    file: &str,
    play: &str,
    level: f64,
    growth: f64,
    o: &crate::Options,
    ctx: &mut crate::Context,
) -> Result<crate::Note, String> {
    let figures = tables::figures(file);
    match play {
        "kept" => Ok(figures::kept(figures, level, o, ctx)),
        "fitted" => Ok(figures::fitted(figures, level, growth, o, ctx)),
        "evened" => Ok(figures::evened(figures, level, o, ctx)),
        other => Err(format!(
            "a table plays kept, fitted, evened, following, held, blown or piano, not {other}"
        )),
    }
}

/// A struck note from the table in `file` at `level`, as `kept` gives it, the voice following bend and brightness and multiplied by `times`, as the vibraphone's motor pulses it.
pub(crate) fn following_from(
    file: &str,
    level: f64,
    times: fundsp::prelude::Net,
    o: &crate::Options,
) -> crate::Note {
    let figures = tables::figures(file);
    let note = o.number("note");
    let vel = o.number("vel").clamp(0.0, 1.0);
    let voice = figures.voice(note, vel);
    let gain = figures::kept_gain(figures, level, note, vel);
    let end = voice.silent_after();
    crate::Note::envelope(voice.following(o) * times * gain as f32, o).within(end)
}

/// How a wind player plays a held note from a table: the figures of mxs's flute_figured's breath, its seed drawn last.
#[derive(Clone, Copy)]
pub(crate) struct Blown {
    pub wander: f64,
    pub wander_above: f64,
    pub catch: f64,
    pub vary: f64,
    pub inhale: bool,
    pub even: f64,
}

/// A held note from the table in `file`, its pitch following note as it slides, with `vibrato` in place of the recordings' when given, speaking at once from where they settle at `legato` 1.
/// Bowed, without `blown`, it is `level` at vel and quieter in proportion below the softest recorded; blown, it is `level` throughout, breathed as `blown` sets.
pub(crate) fn held_from(
    file: &str,
    level: f64,
    vibrato: Option<f64>,
    legato: f64,
    blown: Option<Blown>,
    o: &crate::Options,
    ctx: &mut crate::Context,
) -> crate::Note {
    let figures = tables::figures(file);
    let breath = blown.map(|b| figures::Breath {
        wander: b.wander,
        wander_above: b.wander_above,
        catch: b.catch,
        vary: b.vary,
        inhale: b.inhale,
        even: b.even,
        seed: ctx.seed(),
    });
    if breath.is_some() {
        figures::held_note(
            figures,
            move |_| level,
            vibrato,
            legato,
            breath,
            true,
            o,
            ctx,
        )
    } else {
        let gain = move |vel: f64| level * (vel / figures.softest()).min(1.0);
        figures::held_note(figures, gain, vibrato, legato, None, true, o, ctx)
    }
}

/// A piano note, as mxs's piano plays it from its tables with the note's options.
pub(crate) fn piano_from(o: &crate::Options, ctx: &mut crate::Context) -> crate::Note {
    strings::piano_note(o, ctx)
}

/// The piano: each harmonic a resonator struck once, with a thump of noise.
pub fn piano() -> crate::Synth {
    described(strings::piano())
}

pub use strings::{Harmonic, Knock, Partner, PartnerKind, PianoParts, Thump, parts as piano_parts};

/// The berimbau: the string's partials struck by the stick, swung by the gourd, over the wire's close modes and the stick's thud.
pub fn berimbau() -> crate::Synth {
    described(drums::berimbau())
}

fn described(mut synth: crate::Synth) -> crate::Synth {
    synth.doc = about(&synth.name).to_string();
    synth.tables = tables::of(&synth.name);
    synth.family = family(&synth.name);
    synth.range = range(&synth.name);
    takes_vel(synth)
}

/// Each synth's family, as mx Studio's Sounds panel groups them.
fn family(name: &str) -> Family {
    match name {
        "piano" | "electric_piano" | "organ" | "harmonium" | "accordion" => Family::Keys,
        "guitar" | "electric_guitar" | "electric_bass" | "bandolim" | "cavaquinho" | "harp"
        | "sitar" | "pluck" | "tanpura" | "berimbau" => Family::PluckedStrings,
        "violin" | "viola" | "cello" | "double_bass" => Family::BowedStrings,
        "flute" | "flute_figured" | "oboe" | "clarinet" | "bassoon" | "tenor_sax" | "trumpet"
        | "trombone" | "horn" | "tuba" | "brass" => Family::Winds,
        "marimba" | "vibraphone" | "xylophone" | "glockenspiel" | "timpani" => Family::Mallets,
        "tri" | "saw" | "square" | "pulse" | "supersaw" | "fm" | "acid" | "deep_bass" | "noise" => {
            Family::Synths
        }
        _ => Family::Drums,
    }
}

/// The lowest and highest MIDI notes each pitched synth is meant to play: the notes of the recordings it was fitted to, as [`about`] names them, or further where it says so.
/// Timpani take the range of the usual set of four, "roughly D2 to A3", from https://en.wikipedia.org/wiki/Timpani.
/// The cavaquinho, bandolim, sitar, tanpura, organ, harmonium and accordion were fitted to no run of notes, and no open reference found gives their range, so they have none for now, as do the generic pluck and brass.
fn range(name: &str) -> Option<(f64, f64)> {
    let (low, high) = match name {
        "piano" => (21, 108),
        "electric_piano" => (28, 88),
        "guitar" => (35, 83),
        "electric_guitar" => (40, 86),
        "electric_bass" | "deep_bass" => (28, 63),
        "harp" => (36, 92),
        "violin" => (55, 107),
        "viola" => (48, 94),
        "cello" => (36, 82),
        "double_bass" => (28, 67),
        "flute" | "flute_figured" => (59, 97),
        "clarinet" => (50, 96),
        "oboe" => (58, 92),
        "bassoon" => (34, 74),
        "tenor_sax" => (44, 76),
        "trumpet" => (52, 87),
        "trombone" => (40, 72),
        "horn" => (36, 77),
        "tuba" => (34, 58),
        "marimba" => (29, 96),
        "vibraphone" => (48, 89),
        "xylophone" => (65, 108),
        "glockenspiel" => (72, 112),
        "timpani" => (38, 57),
        _ => return None,
    };
    Some((f64::from(low), f64::from(high)))
}

/// The synth, given vel after its note when it declares none, scaling its level by vel / DEFAULT_VEL, so a note given no vel plays as before.
fn takes_vel(mut synth: crate::Synth) -> crate::Synth {
    if synth.specs.iter().any(|spec| spec.name == "vel") {
        return synth;
    }
    let vel = crate::Spec::float("vel")
        .default(crate::DEFAULT_VEL)
        .range(0.0, 1.0)
        .doc("How hard it is played, which scales its level by vel / 0.6.");
    let after_note = synth
        .specs
        .iter()
        .position(|s| s.name == "note")
        .map_or(0, |at| at + 1);
    synth.specs.insert(after_note, vel);
    let build = synth.build.clone();
    synth.build = std::sync::Arc::new(move |o, ctx| {
        let mut note = build(o, ctx);
        let scale = o.number("vel") / crate::DEFAULT_VEL;
        if scale != 1.0 {
            note.sound = note.sound * scale as f32;
        }
        note
    });
    synth
}

/// What each synth is, and the recordings it is fitted to.
fn about(name: &str) -> &'static str {
    match name {
        "saw" => "A sawtooth wave under a resonant low-pass filter.",
        "square" => "A square wave under a resonant low-pass filter.",
        "tri" => "A triangle wave under a resonant low-pass filter.",
        "pulse" => "A pulse wave of any width under a resonant low-pass filter.",
        "noise" => "Noise under a resonant low-pass filter.",
        "fm" => "A sine wave whose frequency another sine wave swings.",
        "pluck" => {
            "A plain plucked string, which rings for as long as asked and dulls as it rings."
        }
        "supersaw" => "Sawtooth waves spread around the note, under a resonant low-pass filter.",
        "acid" => "An acid bass, a wave under a resonant filter that closes as the note sounds.",
        "brass" => "A plain brass sound, brighter the harder it is blown.",
        "piano" => {
            "A grand piano fitted partial by partial to the Steinway B of the Versilian Community Sample Library from A#0 to G#7, with the sustain pedal and without, and played from A0 to C8. What a note is made of can be had without playing it, from the same options: its harmonics, each with a frequency, a level and the seconds it takes to fall 60 dB, the partials between them as twins of the nearest harmonic, the noise of the strike and the knock under 600 Hz."
        }
        "guitar" => {
            "A nylon-string guitar fitted partial by partial to the University of Iowa's guitar from E2 to B5, and played down to B1, the low string of a 7-string guitar. A note rings until it fades by itself."
        }
        "electric_bass" => {
            "An electric bass played with the fingers, fitted partial by partial to the notes of the bass_electronic_018 instrument of Google's NSynth dataset from E1 to D#4. A note rings until it fades by itself."
        }
        "electric_guitar" => {
            "A solid-body electric guitar picked clean, fitted to the notes of the guitar_electronic_028 instrument of Google's NSynth dataset from E2 to D6. A note rings until it fades by itself."
        }
        "electric_piano" => {
            "An electric piano whose hammers strike tines, fitted to the notes of the keyboard_electronic_098 instrument of Google's NSynth dataset from E1 to E6. A note rings while the key is held until it fades by itself."
        }
        "deep_bass" => {
            "A dark bass played with the fingers, fitted partial by partial to the notes of the bass_electronic_025 and bass_electronic_027 instruments of Google's NSynth dataset from E1 to D#4. A note rings long until it fades by itself."
        }
        "cavaquinho" => "A cavaquinho, with steel strings, brighter and shorter than the guitar.",
        "bandolim" => "A bandolim, the Brazilian mandolin, with steel strings and a tremolo.",
        "harp" => {
            "A folk harp, the kind the Paraguayan and llanera harps are, fitted partial by partial to the folk harp of the Versilian Community Sample Library from C2 to G#6."
        }
        "sitar" => {
            "A plucked sitar note with its sympathetic strings, fitted to \"sitar01.flac\" on Freesound."
        }
        "tanpura" => {
            "A tanpura's drone, four strings plucked in turn with the note as Sa, fitted to \"Tanpura 10\" by sankalp on Freesound."
        }
        "violin" => {
            "A violin, bowed or plucked: a bowed string whose body, hiss, scratch and slow drift are fitted to the University of Iowa's arco violin at pp, mf and ff from G3 to C6, played there without vibrato, so its vibrato is a player's; a bowed note follows its note as it slides, so notes slurred in one bow are one note whose note changes."
        }
        "viola" => {
            "A viola, bowed or plucked as the violin is, fitted to the University of Iowa's viola from C3 to Bb6."
        }
        "cello" => {
            "A cello, bowed or plucked, its bowed notes fitted partial by partial to the University of Iowa's arco cello at pp, mf and ff from C2 to Bb5, played there without vibrato, so its vibrato is a player's."
        }
        "double_bass" => {
            "A double bass at the pitch it sounds, bowed or plucked as the violin is, fitted to the University of Iowa's double bass from E1 to G4."
        }
        "flute" => {
            "A flute, a model of an air jet at the end of a bore, so a change of note is a slur on the same breath. Fitted to the University of Iowa's flute from B3 to Db7 and VSCO 2's flute."
        }
        "flute_figured" => {
            "A flute played from figures fitted partial by partial to the University of Iowa's flute; temporary, to compare with the flute."
        }
        "clarinet" => {
            "A Bb clarinet at the pitch it sounds, a model of a reed on a bore, fitted to the University of Iowa's Bb clarinet from D3 to C7."
        }
        "oboe" => {
            "An oboe, a model of a reed on a bore, fitted to the University of Iowa's oboe from Bb3 to Ab6."
        }
        "bassoon" => {
            "A bassoon, a model of a reed on a bore, fitted to the University of Iowa's bassoon from Bb1 to D5."
        }
        "tenor_sax" => {
            "A tenor saxophone at the pitch it sounds, a model of a reed on a bore, fitted to the tenor saxophone of the Versilian Community Sample Library from Ab2 to E5."
        }
        "trumpet" => {
            "A Bb trumpet at the pitch it sounds, a model of lips on a bore, fitted to the University of Iowa's Bb trumpet from E3 to Eb6."
        }
        "trombone" => {
            "A tenor trombone, a model of lips on a bore, fitted to the University of Iowa's tenor trombone from E2 to C5."
        }
        "horn" => {
            "A French horn, a model of lips on a bore, fitted to VSCO 2's F horn from C2 to F5."
        }
        "tuba" => "A tuba, a model of lips on a bore, fitted to VSCO 2's tuba from Bb1 to Bb3.",
        "organ" => {
            "A pipe organ fitted to VSCO 2's organ, with a quiet set, a stopped flute, and a loud one, a principal chorus."
        }
        "harmonium" => {
            "The reed organ pumped by hand that accompanies Hindustani singing, fitted to the \"Harmonium\" pack by cabled_mess on Freesound."
        }
        "accordion" => {
            "An accordion's right hand: two reeds a few Hz apart, beating, fitted to \"Accordeon master\" by hammondman on Freesound."
        }
        "marimba" => {
            "A marimba, fitted partial by partial to the University of Iowa's marimba struck with yarn mallets, from C2 to C7, and played down to F1."
        }
        "vibraphone" => {
            "A vibraphone with its motor, fitted partial by partial to the University of Iowa's vibraphone played with the motor off, from C3 to F6."
        }
        "xylophone" => {
            "A xylophone, fitted partial by partial to the University of Iowa's xylophone struck with rosewood mallets, from F4 to C8."
        }
        "glockenspiel" => {
            "A glockenspiel, fitted partial by partial to the University of Iowa's orchestral bells struck with plastic mallets, from C5 to E8."
        }
        "timpani" => {
            "A timpani, whose partials are not whole multiples of its note, fitted partial by partial to the five timpani of VSCO 2, from 92 to 153 Hz."
        }
        "berimbau" => {
            "The berimbau of capoeira: the string struck by the stick, open, with the coin pressed or buzzed, and swung by the gourd. Fitted to \"Berimbau jam without caxixi\" by EliasArtista on Freesound."
        }
        "kick" => "A kick drum.",
        "snare" => "A snare drum, fitted to the snare of VSCO 2 with its snares on.",
        "clap" => "A hand clap.",
        "crash" => "A crash cymbal, fitted to the crash cymbal of VSCO 2.",
        "ride" => "A ride cymbal.",
        "closed_hihat" => {
            "A closed hi-hat, fitted to the hi-hat of the Versilian Community Sample Library."
        }
        "open_hihat" => {
            "An open hi-hat, fitted to the hi-hat of the Versilian Community Sample Library, which a shorter sustain closes as the pedal does."
        }
        "low_tom" => "A low tom, fitted to the low tom of the Versilian Community Sample Library.",
        "mid_tom" => "A middle tom, the high tom tuned between the high and the low one.",
        "high_tom" => {
            "A high tom, fitted to the high tom of the Versilian Community Sample Library."
        }
        "rimshot" => "A rimshot.",
        "cross_stick" => "A cross stick, the stick laid on the snare's head and struck on its rim.",
        "pedal_hihat" => "A hi-hat closed with the pedal.",
        "cowbell" => {
            "A cowbell, open or muted by the hand, fitted to the cowbells of VSCO 2 and the Versilian Community Sample Library."
        }
        "clave" => "Claves, fitted to those of the Versilian Community Sample Library and VSCO 2.",
        "low_agogo" => {
            "The low bell of an agogô, fitted to the agogô of the Versilian Community Sample Library."
        }
        "high_agogo" => {
            "The high bell of an agogô, fitted to the agogô of the Versilian Community Sample Library."
        }
        "ganza" => {
            "A ganzá, a small shaker, with a down and an up stroke, fitted to the small shaker of the Versilian Community Sample Library."
        }
        "shaker" => {
            "A large shaker, with a down and an up stroke, fitted to the large shaker of the Versilian Community Sample Library."
        }
        "low_conga" => {
            "The tumba, the low conga, with open, muted and slapped strokes, fitted to VSCO 2's congas."
        }
        "mid_conga" => {
            "The conga, the middle drum of the three, with open, muted and slapped strokes, fitted to VSCO 2's congas."
        }
        "high_conga" => {
            "The quinto, the high conga, with open, muted and slapped strokes, fitted to VSCO 2's congas."
        }
        "low_timbale" => "The low timbale.",
        "high_timbale" => "The high timbale.",
        "timbale_shell" => "A stroke on the timbale's shell, as cáscara is played.",
        "guiro" => {
            "A güiro, scraped for as long as its sustain, fitted to the University of Iowa's güiro and those of VSCO 2 and the Versilian Community Sample Library."
        }
        "triangle" => {
            "A triangle, which a hand damps, fitted to the University of Iowa's 6- and 8-inch triangles."
        }
        "tambourine" => {
            "A tambourine struck, fitted to the University of Iowa's three tambourines."
        }
        "tambourine_shake" => "A tambourine shaken for as long as its sustain.",
        "cabasa" => {
            "A cabasa, struck or turned, fitted to the cabasa of the Versilian Community Sample Library."
        }
        "cajon" => {
            "A cajón, from its bass tone to its slap, fitted to the cajón of the Versilian Community Sample Library."
        }
        "low_bongo" => {
            "The low bongo, open or muted, fitted to the bongos of the Versilian Community Sample Library."
        }
        "high_bongo" => {
            "The high bongo, open or muted, fitted to the bongos of the Versilian Community Sample Library."
        }
        "pandeiro" => {
            "The pandeiro of choro and samba, whose `stroke` option picks the thumb, the heel, the slap or the jingles, fitted to pandeiro recordings on Freesound."
        }
        "surdo" => {
            "The large drum that holds samba's beat, struck with a felt mallet, open or muted, fitted to three loops by plotzki on Freesound."
        }
        "cuica" => {
            "The friction drum of samba: each stroke is played back from the nearest of 28 strokes measured in reinsamba's pack \"Cuica\" on Freesound."
        }
        "tamborim" => {
            "The small high drum of samba struck with a stick, fitted to \"Tamborim Sound.ogg\" by Samba-x on Wikimedia Commons."
        }
        "caxixi" => {
            "The woven basket shaker a berimbau player shakes, fitted to \"Caxixi.ogg\" by Freddythehat on Wikimedia Commons."
        }
        "pandeiro_thumb" => "The pandeiro's thumb stroke, its low tone.",
        "pandeiro_heel" => "The pandeiro's heel stroke.",
        "pandeiro_slap" => "The pandeiro's slap.",
        "pandeiro_jingles" => "The pandeiro's jingles.",
        "tabla" => {
            "The tabla, whose `bol` option picks the stroke of the dayan or the bayan, fitted to the \"tabla bols\" pack by mmiron on Freesound."
        }
        "tabla_na" => "Na, a stroke of the dayan, the tabla's higher drum.",
        "tabla_tun" => "Tun, a stroke of the dayan, the tabla's higher drum.",
        "tabla_te" => "Te, a stroke of the dayan, the tabla's higher drum.",
        "tabla_tas" => "Tas, a stroke of the dayan, the tabla's higher drum.",
        "tabla_ghe" => {
            "Ghe, a stroke of the bayan, the tabla's lower drum, which a change of note bends as the palm does."
        }
        "tabla_ke" => "Ke, a stroke of the bayan, the tabla's lower drum.",
        _ => "",
    }
}

/// The crate's synths by family, in the order of [`Family::ALL`]; together they are the synths of [`all`].
pub fn families() -> Vec<(Family, Vec<crate::Synth>)> {
    let all = all();
    Family::ALL
        .into_iter()
        .map(|family| {
            let members = all.iter().filter(|s| s.family == family).cloned().collect();
            (family, members)
        })
        .collect()
}

/// Every synth that comes with the crate.
pub fn all() -> Vec<crate::Synth> {
    let mut synths = vec![strings::piano(), drums::berimbau()];
    synths.extend(basic::all());
    synths.extend(drums::all());
    synths.extend(bowed::all());
    synths.extend(strings::all());
    synths.extend(electric::all());
    synths.extend(tuned::all());
    synths.extend(organs::all());
    synths.extend(winds::all());
    synths.into_iter().map(described).collect()
}
