//! Plays Beethoven's "Ode to Joy" through the default output, or writes it to the WAV file given: ode_to_joy [out.wav]
//! Its three statements of the theme bring in the groups of synths as the symphony does: the cello and double bass alone, then the oboe and flute over a harp, then the violin, trumpet and glockenspiel over an organ, timpani and drums.

use mxs::{Engine, Event, RATE, Value, synths};

const BPM: f64 = 108.0;
const BAR: f64 = 4.0;
const VERSE: f64 = 16.0 * BAR;

// (note, beats)
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
const OPEN_END: [(f64, f64); 3] = [(64.0, 1.5), (62.0, 0.5), (62.0, 2.0)];
const CLOSED_END: [(f64, f64); 3] = [(62.0, 1.5), (60.0, 0.5), (60.0, 2.0)];
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

const C: &[f64] = &[48.0, 52.0, 55.0];
const G: &[f64] = &[43.0, 47.0, 50.0];
const G7: &[f64] = &[43.0, 47.0, 50.0, 53.0];
// The chord under each bar of a statement.
const CHORDS: [&[f64]; 16] = [C, C, C, G, C, C, C, C, G, C, G7, G, C, C, C, C];

// (beat, synth, options, beats held)
type Hit = (f64, &'static str, Vec<(&'static str, f64)>, f64);

fn main() {
    let path = std::env::args().nth(1);
    let mut engine = match path {
        Some(_) => Engine::offline(RATE),
        None => Engine::live().unwrap_or_else(|e| panic!("{e}")),
    };
    synths::all().into_iter().for_each(|s| engine.register(s));

    let seconds = |beats: f64| beats * 60.0 / BPM;
    let lead = engine.now() + 0.2;
    let mut end: f64 = 0.0;
    let events: Vec<(f64, Event)> = arrangement()
        .into_iter()
        .map(|(beat, synth, options, held)| {
            let takes = |option: &str| {
                engine
                    .synths()
                    .find(|s| s.name() == synth)
                    .is_some_and(|s| s.specs().iter().any(|spec| spec.name == option))
            };
            let mut options: Vec<(String, Value)> = options
                .into_iter()
                .filter(|(name, _)| *name == "amp" || takes(name))
                .map(|(name, value)| (name.to_string(), Value::Number(value)))
                .collect();
            if takes("sustain") {
                options.push(("sustain".to_string(), Value::Number(seconds(held))));
            }
            let start = lead + seconds(beat);
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

/// The theme's sixteen bars as (beat, note, beats): the opening twice, its two endings, and the bridge between.
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

struct Score(Vec<Hit>);

impl Score {
    fn hit(&mut self, beat: f64, synth: &'static str, options: &[(&'static str, f64)], held: f64) {
        self.0.push((beat, synth, options.to_vec(), held));
    }

    fn note(&mut self, beat: f64, synth: &'static str, note: f64, vel: f64, held: f64) {
        self.hit(beat, synth, &[("note", note), ("vel", vel)], held);
    }
}

fn arrangement() -> Vec<Hit> {
    let mut score = Score(Vec::new());
    let theme = theme();
    for verse in 0..3 {
        let start = verse as f64 * VERSE;
        let vel = [0.4, 0.55, 0.62][verse];
        for &(beat, note, beats) in &theme {
            let at = start + beat;
            match verse {
                0 => score.note(at, "cello", note - 12.0, vel, beats),
                1 => {
                    score.note(at, "oboe", note, vel, beats);
                    score.note(at, "flute", note + 12.0, vel * 0.7, beats);
                }
                _ => {
                    score.note(at, "violin", note + 12.0, vel, beats);
                    score.note(at, "trumpet", note, vel * 0.4, beats);
                    score.note(at, "glockenspiel", note + 24.0, 0.9, beats);
                }
            }
        }
        for (bar, chord) in CHORDS.iter().enumerate() {
            let at = start + bar as f64 * BAR;
            score.note(at, "double_bass", chord[0] - 12.0, vel, BAR);
            if verse > 0 {
                score.note(at, "cello", chord[0], vel * 0.8, BAR);
            }
            if verse == 1 {
                for (step, &tone) in [0, 1, 2, 1, 0, 1, 2, 1].iter().enumerate() {
                    score.note(at + step as f64 * 0.5, "harp", chord[tone], vel * 0.8, 0.5);
                }
            }
            if verse == 2 {
                for &note in *chord {
                    score.hit(at, "organ", &[("note", note), ("loud", 1.0)], BAR);
                }
                for beat in 0..4 {
                    let at = at + beat as f64;
                    if beat % 2 == 0 {
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
