//! Plays Beethoven's "Ode to Joy" through the default output, or writes it to the WAV file given: ode_to_joy [out.wav]
//! Its three statements of the theme bring in the groups of synths as the symphony does: the cello and double bass alone, then the oboe and flute over a harp, then the violin, trumpet and glockenspiel over an organ, timpani and drums.

use mxs::{Engine, Event, RATE, Value, synths};

/// Beats a minute, a beat being a quarter note.
const BPM: f64 = 108.0;
/// Beats in a bar.
const BAR: f64 = 4.0;
/// Beats in one statement of the theme, which is sixteen bars.
const VERSE: f64 = 16.0 * BAR;

/// The first three bars of each phrase, in C major, as (MIDI note, beats); MIDI 60 is middle C.
const OPENING: [(f64, f64); 12] = [
    (64.0, 1.0),
    (64.0, 1.0),
    (65.0, 1.0),
    (67.0, 1.0),
    (67.0, 1.0),
    (65.0, 1.0),
    (64.0, 1.0),
    (62.0, 1.0),
    (60.0, 1.0),
    (60.0, 1.0),
    (62.0, 1.0),
    (64.0, 1.0),
];
/// The fourth bar of the first phrase, which rests on D.
const OPEN_END: [(f64, f64); 3] = [(64.0, 1.5), (62.0, 0.5), (62.0, 2.0)];
/// The fourth bar of the second and last phrases, which closes on C.
const CLOSED_END: [(f64, f64); 3] = [(62.0, 1.5), (60.0, 0.5), (60.0, 2.0)];
/// The four bars between the phrases: D D E C, D E F E C, D E F E D, and C D with a held G below.
const BRIDGE: [(f64, f64); 17] = [
    (62.0, 1.0),
    (62.0, 1.0),
    (64.0, 1.0),
    (60.0, 1.0),
    (62.0, 1.0),
    (64.0, 0.5),
    (65.0, 0.5),
    (64.0, 1.0),
    (60.0, 1.0),
    (62.0, 1.0),
    (64.0, 0.5),
    (65.0, 0.5),
    (64.0, 1.0),
    (62.0, 1.0),
    (60.0, 1.0),
    (62.0, 1.0),
    (55.0, 2.0),
];

/// C major, as MIDI notes with the root first.
const C: &[f64] = &[48.0, 52.0, 55.0];
/// G major.
const G: &[f64] = &[43.0, 47.0, 50.0];
/// G major with its seventh.
const G7: &[f64] = &[43.0, 47.0, 50.0, 53.0];
/// The chord under each bar of a statement.
const CHORDS: [&[f64]; 16] = [C, C, C, G, C, C, C, C, G, C, G7, G, C, C, C, C];

/// One note to play, as (beat from the start of the piece, synth, options, beats held).
type Hit = (f64, &'static str, Vec<(&'static str, f64)>, f64);

/// Builds the arrangement, then plays it live or renders it to the WAV file given.
fn main() {
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live().unwrap_or_else(|e| panic!("{e}")),
    };
    synths::all().into_iter().for_each(|s| engine.register(s));

    let seconds = |beats: f64| beats * 60.0 / BPM;
    // A short lead, so the first notes are queued before they sound.
    let lead = engine.now() + 0.2;
    let mut end: f64 = 0.0;
    let events: Vec<(f64, Event)> = arrangement()
        .into_iter()
        .map(|(beat, synth, options, held)| {
            // A note given an option its synth does not declare is an error, so only declared ones are passed.
            let takes = |option: &str| {
                engine
                    .synths()
                    .find(|s| s.name() == synth)
                    .is_some_and(|s| s.specs().iter().any(|spec| spec.name == option))
            };
            let mut options: Vec<(String, Value)> = options
                .into_iter()
                // amp belongs to every note, not to its synth.
                .filter(|(name, _)| *name == "amp" || takes(name))
                .map(|(name, value)| (name.to_string(), Value::Number(value)))
                .collect();
            // The engine takes times in seconds, so the beats held are converted here.
            if takes("sustain") {
                options.push(("sustain".to_string(), Value::Number(seconds(held))));
            }
            let start = lead + seconds(beat);
            // Three seconds past the last note let the releases and the crash ring out.
            end = end.max(start + seconds(held) + 3.0);
            (
                start,
                Event::Note {
                    id: None,
                    synth: synth.to_string(),
                    options,
                    chain: None,
                },
            )
        })
        .collect();
    engine.play(&events).unwrap_or_else(|e| panic!("{e}"));

    if let Some(path) = path {
        let (left, right) = engine.render(end).unwrap();
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: RATE as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut wav = hound::WavWriter::create(&path, spec).unwrap();
        for (l, r) in left.into_iter().zip(right) {
            wav.write_sample(l).unwrap();
            wav.write_sample(r).unwrap();
        }
        wav.finalize().unwrap();
        return;
    }
    println!("playing at {} Hz", engine.rate());
    std::thread::sleep(std::time::Duration::from_secs_f64(end));
}

/// The theme's sixteen bars as (beat from the start of the statement, note, beats): the opening with its first ending, the opening with its closing one, the bridge, and the closing phrase again.
fn theme() -> Vec<(f64, f64, f64)> {
    let phrase =
        |end: &[(f64, f64)]| -> Vec<(f64, f64)> { OPENING.iter().chain(end).copied().collect() };
    let notes: Vec<(f64, f64)> = [
        phrase(&OPEN_END),
        phrase(&CLOSED_END),
        BRIDGE.to_vec(),
        phrase(&CLOSED_END),
    ]
    .concat();
    let mut beat = 0.0;
    notes
        .into_iter()
        .map(|(note, beats)| {
            let at = beat;
            beat += beats;
            (at, note, beats)
        })
        .collect()
}

/// The notes of the arrangement, added one at a time.
struct Score(Vec<Hit>);

impl Score {
    /// Adds a note of `synth` with the options given, held for `held` beats.
    fn hit(&mut self, beat: f64, synth: &'static str, options: &[(&'static str, f64)], held: f64) {
        self.0.push((beat, synth, options.to_vec(), held));
    }

    /// Adds a pitched note, `vel` being how hard it is played from 0 to 1.
    fn note(&mut self, beat: f64, synth: &'static str, note: f64, vel: f64, held: f64) {
        self.hit(beat, synth, &[("note", note), ("vel", vel)], held);
    }
}

/// Every note of the piece: the theme on a different group of synths in each of its three statements, the chords and drums under it, and a closing chord.
fn arrangement() -> Vec<Hit> {
    let mut score = Score(Vec::new());
    let theme = theme();
    for verse in 0..3 {
        let start = verse as f64 * VERSE;
        // Each statement is played harder than the last.
        let vel = [0.4, 0.55, 0.62][verse];
        for &(beat, note, beats) in &theme {
            let at = start + beat;
            match verse {
                // The cello alone, an octave down.
                0 => score.note(at, "cello", note - 12.0, vel, beats),
                // The oboe, and the flute an octave above it.
                1 => {
                    score.note(at, "oboe", note, vel, beats);
                    score.note(at, "flute", note + 12.0, vel * 0.7, beats);
                }
                // The violin an octave up, the trumpet at pitch and the glockenspiel two octaves up.
                _ => {
                    score.note(at, "violin", note + 12.0, vel, beats);
                    score.note(at, "trumpet", note, vel * 0.4, beats);
                    score.note(at, "glockenspiel", note + 24.0, 0.9, beats);
                }
            }
        }
        for (bar, chord) in CHORDS.iter().enumerate() {
            let at = start + bar as f64 * BAR;
            // The double bass holds each bar's root an octave below the chord.
            score.note(at, "double_bass", chord[0] - 12.0, vel, BAR);
            if verse > 0 {
                score.note(at, "cello", chord[0], vel * 0.8, BAR);
            }
            // The harp plays the chord as eighth notes: root, third, fifth, third, and again.
            if verse == 1 {
                for (step, &tone) in [0, 1, 2, 1, 0, 1, 2, 1].iter().enumerate() {
                    score.note(at + step as f64 * 0.5, "harp", chord[tone], vel * 0.8, 0.5);
                }
            }
            // The organ's loud set holds the chord, and the drums keep time under it.
            if verse == 2 {
                for &note in *chord {
                    score.hit(at, "organ", &[("note", note), ("loud", 1.0)], BAR);
                }
                for beat in 0..4 {
                    let at = at + beat as f64;
                    // Kick and timpani on beats 1 and 3, snare and tambourine on 2 and 4, hi-hat on every eighth.
                    if beat % 2 == 0 {
                        // The kick has no vel, so amp keeps it from covering the rest.
                        score.hit(at, "kick", &[("amp", 0.6)], 0.5);
                        score.note(at, "timpani", chord[0], 0.9, 1.0);
                    } else {
                        score.hit(at, "snare", &[("vel", 0.9)], 0.5);
                        score.hit(at, "tambourine", &[("vel", 0.8)], 0.5);
                    }
                    score.hit(at, "closed_hihat", &[("vel", 0.7)], 0.25);
                    score.hit(at + 0.5, "closed_hihat", &[("vel", 0.5)], 0.25);
                }
            }
        }
        if verse == 2 {
            score.hit(start, "crash", &[("vel", 0.5)], BAR);
        }
    }
    // A last C chord in the bar after the third statement.
    let finale = 3.0 * VERSE;
    for (synth, note) in [
        ("violin", 72.0),
        ("trumpet", 60.0),
        ("cello", 48.0),
        ("double_bass", 36.0),
        ("timpani", 48.0),
    ] {
        score.note(finale, synth, note, 0.62, BAR);
    }
    for &note in C {
        score.hit(finale, "organ", &[("note", note), ("loud", 1.0)], BAR);
    }
    score.hit(finale, "kick", &[("amp", 0.6)], 0.5);
    score.hit(finale, "crash", &[("vel", 0.5)], BAR);
    score.0
}
