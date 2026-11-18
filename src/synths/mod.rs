//! mx's own synths, by family, each with the options it takes.
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
//! A time is in seconds.
//! A song of mx writes the envelope's times and the slides in beats, and mx turns them into seconds before they reach the engine.
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
mod tuned;
mod winds;

pub(crate) use bowed::bowed_string;
pub(crate) use common::partials;
pub(crate) use winds::{drifting_swing, played_jet, wind_bands};

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
            "A violin, bowed or plucked, its bowed notes fitted partial by partial to the University of Iowa's arco violin at pp, mf and ff from G3 to B7, played there without vibrato, so its vibrato is a player's; a bowed note follows its note as it slides, so notes slurred in one bow are one note whose note changes."
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

/// mx's synths by family, each a name and its synths; together they are the synths of `all`.
pub fn families() -> Vec<(&'static str, Vec<crate::Synth>)> {
    let mut plucked = vec![strings::piano()];
    plucked.extend(strings::all());
    plucked.extend(electric::all());
    let mut struck = vec![drums::berimbau()];
    struck.extend(drums::all());
    let families = vec![
        ("Basic waveforms", basic::all()),
        ("Piano and plucked strings", plucked),
        ("Bowed strings", bowed::all()),
        ("Winds", winds::all()),
        ("Organs and reeds", organs::all()),
        ("Tuned percussion", tuned::all()),
        ("Drums", struck),
    ];
    families
        .into_iter()
        .map(|(family, synths)| (family, synths.into_iter().map(described).collect()))
        .collect()
}

/// Every synth mx ships.
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
