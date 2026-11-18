//! A short jazzy salsa on a piano, a plucked bass and Latin percussion, as a primer on using mxs: salsa [out.wav]
//! It plays through the default output, or renders offline into the WAV file given.
//!
//! cargo run --release --example salsa
//!
//! Using mxs takes three steps, which `main` follows in order:
//! 1. Make an engine, live on the sound card or offline, and register the synths on it.
//! 2. Describe the music as notes: a time in seconds, a synth's name and its options.
//! 3. Hand the notes to the engine, then let it play or render them.

use mxs::{Engine, Event, RATE, synths};

/// Beats a minute; a beat is a quarter note, and the music is written in eighth notes, two to a beat.
const BPM: f64 = 180.0;
/// Bars of the song, which is a four-bar turnaround played over and over: Dm7, G7, Cmaj7 and A7.
const BARS: usize = 28;

/// The piano's chord for each bar of the turnaround, as MIDI notes; 60 is middle C.
const CHORDS: [[f64; 4]; 4] = [
    [65.0, 69.0, 72.0, 74.0], // Dm7: F A C D
    [65.0, 67.0, 71.0, 74.0], // G7: F G B D
    [64.0, 67.0, 71.0, 72.0], // Cmaj7: E G B C
    [64.0, 67.0, 69.0, 73.0], // A7: E G A C#
];
/// The bass's root for each bar of the turnaround.
const ROOTS: [f64; 4] = [38.0, 43.0, 36.0, 45.0];
/// The piano's montuno, a syncopated pattern of two bars: (eighth note in the bar, which of the chord's four notes) for each.
const MONTUNO: [&[(usize, usize)]; 2] = [
    &[(0, 1), (1, 2), (3, 1), (4, 3), (5, 2), (7, 1)],
    &[(0, 2), (2, 1), (3, 3), (5, 1), (6, 2), (7, 0)],
];
/// The 3-2 son clave over two bars of sixteen eighth notes, which the other parts are played against.
const CLAVE: [usize; 5] = [0, 3, 6, 10, 12];

fn main() {
    // 1. An engine plays live on the default output, or renders offline when it is given a file.
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live().unwrap_or_else(|e| panic!("{e}")),
    };
    // The engine plays the synths it knows by name, so give it all that come with the crate.
    synths::all().into_iter().for_each(|s| engine.register(s));

    // 2. The notes, each stamped in seconds on the engine's clock; a short lead lets them queue before they sound.
    let lead = engine.now() + 0.2;
    let mut end = 0.0;
    let notes: Vec<(f64, Event)> = song()
        .into_iter()
        .map(|(beat, synth, options)| {
            let at = lead + seconds(beat);
            end = f64::max(end, at + 3.0);
            (at, Event::note(None, synth, &options))
        })
        .collect();

    // 3. The engine takes all of them at once and sounds each at its time.
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

/// A repeatable wobble between -0.5 and 0.5 for an eighth note of a bar, so the parts are played less evenly than a machine would.
fn feel(bar: usize, step: usize) -> f64 {
    ((bar * 31 + step * 17) % 11) as f64 / 10.0 - 0.5
}

/// Every note of the song as (beat, synth, options); options are numbers by name, and a synth keeps its defaults for those left out.
fn song() -> Vec<(f64, &'static str, Vec<(&'static str, f64)>)> {
    let mut notes = Vec::new();
    for bar in 0..BARS {
        let chord = CHORDS[bar % 4];
        let root = ROOTS[bar % 4];
        let next_root = ROOTS[(bar + 1) % 4];
        // The beat an eighth note of this bar falls on, and the same a little early or late for the parts that play loosely.
        let at = |step: usize| bar as f64 * 4.0 + step as f64 * 0.5;
        let loose = |step: usize| at(step) + 0.03 * feel(bar, step);
        // A velocity pushed a little up or down, never past 1.
        let touch = |vel: f64, step: usize| (vel * (1.0 + 0.3 * feel(bar, step))).clamp(0.0, 1.0);

        for step in 0..8 {
            // The clave runs through the whole song, over two bars at a time, and keeps exact time.
            if CLAVE.contains(&((bar % 2) * 8 + step)) {
                notes.push((at(step), "clave", vec![("vel", 0.7)]));
            }
            // The shaker plays every eighth note, alternating its stroke up and down.
            let up = (step % 2) as f64;
            let vel = touch(0.3 + 0.1 * up, step);
            notes.push((loose(step), "shaker", vec![("vel", vel), ("up", up)]));
        }

        // A simple conga tumbao: muted and open tones on the low and high drums, and a slap on the last eighth.
        for (step, synth, vel, mute, slap) in [
            (0, "low_conga", 0.45, 1.0, 0.0),
            (2, "high_conga", 0.4, 1.0, 0.0),
            (3, "low_conga", 0.8, 0.0, 0.0),
            (4, "high_conga", 0.6, 0.0, 0.0),
            (6, "low_conga", 0.7, 0.0, 0.0),
            (7, "high_conga", 0.7, 0.0, 1.0),
        ] {
            let options = vec![("vel", touch(vel, step)), ("mute", mute), ("slap", slap)];
            notes.push((loose(step), synth, options));
        }

        // The bass comes in after two bars of percussion, and keeps exact time.
        if bar >= 2 {
            // It skips the downbeat: its root on the "and" of 2, the fifth above it on beat 4,
            // and the next bar's root on the "and" of 4, early, as bass players push into a new chord.
            // Its amp lifts it, as the plucked string is quiet.
            for (step, note, beats) in [(3, root, 1.2), (6, root + 7.0, 0.4), (7, next_root, 0.9)] {
                let options = vec![
                    ("note", note),
                    ("vel", touch(0.9, step)),
                    ("pizz", 1.0),
                    ("sustain", seconds(beats)),
                    ("amp", 3.0),
                ];
                notes.push((at(step), "double_bass", options));
            }
        }

        // The piano joins at bar 4 and plays the montuno low, each note doubled an octave down as salsa pianists do.
        if bar >= 4 {
            for &(step, tone) in MONTUNO[bar % 2] {
                for octave in [12.0, 24.0] {
                    let options = vec![
                        ("note", chord[tone] - octave),
                        ("vel", touch(0.85, step)),
                        ("sustain", seconds(0.45)),
                        ("amp", 0.4),
                    ];
                    notes.push((loose(step), "piano", options));
                }
            }
        }

        // The cowbell keeps the beat from bar 12, and each phrase ends with a timbale fill.
        if bar >= 12 {
            for step in [0, 2, 4, 6] {
                notes.push((loose(step), "cowbell", vec![("vel", touch(0.5, step))]));
            }
            if bar % 4 == 3 {
                for step in 4..8 {
                    let synth = if step % 2 == 0 {
                        "low_timbale"
                    } else {
                        "high_timbale"
                    };
                    notes.push((loose(step), synth, Vec::new()));
                }
            }
        }
    }

    // A last chord on the downbeat after the final bar.
    let finale = BARS as f64 * 4.0;
    for note in CHORDS[2] {
        notes.push((
            finale,
            "piano",
            vec![("note", note), ("vel", 0.6), ("sustain", seconds(3.0))],
        ));
    }
    notes.push((
        finale,
        "double_bass",
        vec![
            ("note", 36.0),
            ("vel", 1.0),
            ("pizz", 1.0),
            ("sustain", seconds(2.0)),
            ("amp", 3.0),
        ],
    ));
    notes.push((finale, "low_conga", vec![("vel", 0.8)]));
    notes
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
