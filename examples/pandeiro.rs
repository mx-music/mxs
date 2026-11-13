//! A pandeiro solo in the language of choro and samba, with a visit to the northeast, as a primer on phrasing one drum: pandeiro [out.wav]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example pandeiro
//!
//! A pandeiro player's two hands share the work.
//! The striking hand alternates its lower part, the thumb and the wrist, with its upper part, the fingertips.
//! The holding hand rocks the drum to meet each stroke, which shakes the jingles even where nothing strikes, and a finger of it presses the head from behind to mute the bass.
//! So the solo is written as that rocking motion, one cell to a sixteenth note, and `solo` turns the motion into strokes with their loudness and timing.

use mxs::{Engine, Event, RATE, synths};

/// Beats a minute; a beat is a quarter note of 2/4, rocked as four sixteenths.
const BPM: f64 = 104.0;

/// How the holding hand rocks the drum through a beat.
#[derive(Clone, Copy)]
enum Feel {
    /// As in choro and samba: wider on the second sixteenth and on the way back, with the third and fourth sixteenths early.
    Rocked,
    /// As Marcos Suzano plays the northeastern and funk grooves: every sixteenth alike.
    Even,
}

/// The solo as phrases played in order: (cells, times through, feel, loudness at its start, loudness at its end).
/// A cell is a sixteenth, and two bars are sixteen cells.
/// The thumb plays T, a bass muted by the holding hand, and O, an open bass; the fingertips play the same two on the head as t and o.
/// On the rim, where only the jingles sound, h is the wrist and f the fingertips.
/// S is a slap, R a roll of the thumb rubbed along the head, and `.` a ghost note: the hand rocks the drum and strikes nothing.
/// `_` is silence, a `>` before a cell accents it, and several strokes in one cell split it evenly.
const SOLO: [(&str, usize, Feel, f64, f64); 27] = [
    // The rocking alone, then the fingertips where the hand swings widest, then a pickup.
    (
        ". . . .  . . . .  . . . .  . . . .",
        1,
        Feel::Rocked,
        0.6,
        0.9,
    ),
    (
        ". f . .  . f . .  . f . f  . f h o",
        1,
        Feel::Rocked,
        0.7,
        0.9,
    ),
    // The choro groove: the bass copies the surdo, muted on the first beat and open on the second.
    (
        "T f h f  O f h f  T f h f  O f h o",
        2,
        Feel::Rocked,
        0.85,
        0.85,
    ),
    // A short roll of two fingers on the second sixteenth, as Jorginho do Pandeiro ornaments it.
    (
        "T f h f  O ff h f  T f h f  O f h o",
        1,
        Feel::Rocked,
        0.9,
        0.9,
    ),
    // Samba: a slap before the second beat, and the fingertips' bass pushing into each beat.
    (
        "T f h S  O f h o  T f h o  O f h o",
        2,
        Feel::Rocked,
        0.95,
        0.95,
    ),
    // A call of bass, slap and bass over three, three and two sixteenths, which the groove answers.
    (
        ">O f h >S  f h >O f  T f h f  O f h o",
        1,
        Feel::Rocked,
        1.0,
        0.9,
    ),
    // The call again, answered by a slap and a breath.
    (
        ">O f h >S  f h >O f  >S _ . f  O f h o",
        1,
        Feel::Rocked,
        1.0,
        0.9,
    ),
    // The tamborim's line of 2+2+3+2+2+2+3 sixteenths in the accents, over the surdo's two basses.
    (
        "T f >h f  O f h o  T o h S  O o h f",
        2,
        Feel::Rocked,
        0.95,
        0.95,
    ),
    (
        "T f >h f  O f h o  T o h S  O o >S o",
        1,
        Feel::Rocked,
        1.0,
        1.0,
    ),
    // Partido alto: slaps and open basses with space between, the drum still rocking.
    (
        ". S . .  O . O .  O . S .  . O . O",
        2,
        Feel::Rocked,
        0.9,
        0.9,
    ),
    (
        ". S . .  O . O .  O . SS .  . O h O",
        1,
        Feel::Rocked,
        0.95,
        0.95,
    ),
    // It breaks into groups of three sixteenths against the beat,
    (
        ". S . .  O . O .  S o _ S  o _ S o",
        1,
        Feel::Rocked,
        0.95,
        1.0,
    ),
    // which land on a slap and leave the rocking alone again.
    (
        "_ S o _  >S _ . .  . . . .  . . . .",
        1,
        Feel::Rocked,
        1.0,
        0.45,
    ),
    // The northeast, played evenly and led by the fingertips: the baião's bass arrives first, alone.
    (
        ". . . O  . . . .  . . . O  . . S .",
        1,
        Feel::Even,
        0.6,
        0.7,
    ),
    // Baião: the thumb falls off the beat, where the zabumba plays, and the slap is its rim.
    (
        "t h f O  f h S h  t h f O  f h S h",
        2,
        Feel::Even,
        0.8,
        0.8,
    ),
    // The zabumba's own two-bar phrase.
    (
        "t h f O  f h S h  t O f O  S h S h",
        2,
        Feel::Even,
        0.9,
        0.9,
    ),
    // Maracatu turned to funk: slaps on the beat, basses around them.
    (
        "o h f O  S O f h  f O f h  S O f h",
        2,
        Feel::Even,
        1.0,
        1.0,
    ),
    // A run of thirty-second notes from quiet to loud, and three slaps to turn around on.
    (
        "hf hf hf hf  hf hf hf hf  >S _ >S _  >S _ . o",
        1,
        Feel::Even,
        0.45,
        1.0,
    ),
    // Samba again, at full strength, then the tamborim's line.
    (
        "T f h S  O f h o  T f h o  O f h o",
        1,
        Feel::Rocked,
        1.0,
        1.0,
    ),
    (
        "T f >h f  O f h o  T o h S  O o >S o",
        1,
        Feel::Rocked,
        1.0,
        1.0,
    ),
    // Open basses every three sixteenths, across both bars.
    (
        "O f h O  f h O f  h O f h  >S _ >S o",
        1,
        Feel::Rocked,
        1.0,
        1.0,
    ),
    // The groove settles and fades.
    (
        "T f h S  O f h o  T f h o  O f h o",
        1,
        Feel::Rocked,
        1.0,
        0.8,
    ),
    (
        "T f h f  O f h f  T f h f  O f h f",
        1,
        Feel::Rocked,
        0.8,
        0.45,
    ),
    // The call once more, unanswered.
    (">O f h >S  f h >O _", 1, Feel::Rocked, 0.9, 0.9),
    // Two rolls of a beat each, as Jorginho ends a solo, and the last bass.
    ("R R R R", 2, Feel::Even, 0.5, 1.0),
    (">O", 1, Feel::Even, 1.0, 1.0),
    ("_", 1, Feel::Even, 1.0, 1.0),
];

/// How much louder or softer the jingles sound on each sixteenth of a rocked beat.
/// The hand swings widest into the second, which makes it the accent, and wide again on the way back.
const LEAN: [f64; 4] = [0.85, 1.25, 1.0, 0.8];
/// How late each sixteenth of a rocked beat is played, in beats.
/// Studies of samba recordings find the third and fourth sixteenths between 0.025 and 0.05 beats early.
const PUSH: [f64; 4] = [0.0, 0.0, -0.03, -0.04];
/// How late a bass on the beat is played, in beats; the same studies find it a little behind.
const BASS_LATE: f64 = 0.01;

fn main() {
    // An engine plays live on the default output, or renders offline when it is given a file.
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live().unwrap_or_else(|e| panic!("{e}")),
    };
    synths::all().into_iter().for_each(|s| engine.register(s));

    // The strokes, each stamped in seconds on the engine's clock; a short lead lets them queue before they sound.
    let lead = engine.now() + 0.2;
    let mut end = 0.0;
    let notes: Vec<(f64, Event)> = solo()
        .into_iter()
        .map(|(beat, synth, options)| {
            let at = lead + seconds(beat);
            end = f64::max(end, at + 2.5);
            (at, Event::note(None, synth, &options))
        })
        .collect();

    engine.play(&notes).unwrap_or_else(|e| panic!("{e}"));
    match path {
        Some(path) => write_wav(&path, &mut engine, end),
        None => std::thread::sleep(std::time::Duration::from_secs_f64(end)),
    }
}

/// Seconds in `beats` beats.
fn seconds(beats: f64) -> f64 {
    beats * 60.0 / BPM
}

/// Every stroke of the solo as (beat, synth, options).
fn solo() -> Vec<(f64, &'static str, Vec<(&'static str, f64)>)> {
    let mut strokes = Vec::new();
    let mut cell = 0;
    for (cells, times, feel, from, to) in SOLO {
        let written: Vec<&str> = cells.split_whitespace().collect();
        let last = f64::max(written.len() as f64 - 1.0, 1.0);
        for _ in 0..times {
            for (i, written) in written.iter().enumerate() {
                // The phrase swells or fades from its first cell to its last.
                let loudness = from + (to - from) * i as f64 / last;
                let accent = if written.starts_with('>') { 0.25 } else { 0.0 };
                let letters = written.trim_start_matches('>');
                // A roll is the thumb bouncing along the head, four times to a sixteenth.
                let letters = if letters == "R" { "rrrr" } else { letters };
                let count = letters.len() as f64;
                let place = cell % 4;
                let (lean, push) = match feel {
                    Feel::Rocked => (LEAN[place], PUSH[place]),
                    // Half the samba's push here is chosen by ear, as the studies measured samba.
                    Feel::Even => (1.0, PUSH[place] / 2.0),
                };
                for (j, letter) in letters.chars().enumerate() {
                    let Some((synth, muted, vel)) = stroke(letter) else {
                        continue;
                    };
                    let bass = synth == "pandeiro_thumb";
                    // Only the jingles take the lean of the rocking; basses and slaps are struck as written.
                    let lean = if bass || letter == 'S' { 1.0 } else { lean };
                    // A stroke split from another in its cell is lighter, but a roll is even.
                    let split = if j > 0 && letter != 'r' { 0.85 } else { 1.0 };
                    let touch = 1.0 + 0.15 * wobble(cell * 4 + j);
                    let vel = ((vel * lean + accent) * split * loudness * touch).clamp(0.02, 1.0);
                    let late = if bass && place == 0 { BASS_LATE } else { 0.0 };
                    let loose = 0.008 * wobble(cell * 4 + j + 1);
                    let beat = (cell as f64 + j as f64 / count) * 0.25 + push + late + loose;
                    let options = vec![("vel", vel), ("muted", muted)];
                    strokes.push((f64::max(beat, 0.0), synth, options));
                }
                cell += 1;
            }
        }
    }
    strokes
}

/// The pandeiro synth of a letter's stroke, how far the head is muted and its velocity, or none for silence.
/// The synth has no stroke for a rocked or rubbed jingle, so a ghost note and a roll are its fingertips on the jingles, very soft, with the head muted.
fn stroke(letter: char) -> Option<(&'static str, f64, f64)> {
    match letter {
        'T' => Some(("pandeiro_thumb", 1.0, 0.6)),
        'O' => Some(("pandeiro_thumb", 0.0, 0.75)),
        't' => Some(("pandeiro_thumb", 1.0, 0.5)),
        'o' => Some(("pandeiro_thumb", 0.0, 0.6)),
        'h' => Some(("pandeiro_heel", 0.0, 0.32)),
        'f' => Some(("pandeiro_jingles", 0.0, 0.38)),
        'S' => Some(("pandeiro_slap", 1.0, 0.7)),
        '.' => Some(("pandeiro_jingles", 1.0, 0.1)),
        'r' => Some(("pandeiro_jingles", 1.0, 0.2)),
        _ => None,
    }
}

/// A repeatable wobble between -0.5 and 0.5, so no two strokes are struck quite alike.
fn wobble(n: usize) -> f64 {
    ((n * 31) % 11) as f64 / 10.0 - 0.5
}

/// Renders `seconds` of what the engine was given and writes it as a stereo WAV file.
fn write_wav(path: &str, engine: &mut Engine, seconds: f64) {
    let (left, right) = engine.render(seconds).unwrap_or_else(|e| panic!("{e}"));
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: RATE as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut wav = hound::WavWriter::create(path, spec).unwrap();
    for (l, r) in left.into_iter().zip(right) {
        wav.write_sample(l).unwrap();
        wav.write_sample(r).unwrap();
    }
    wav.finalize().unwrap();
}
