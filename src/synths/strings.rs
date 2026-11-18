//! mx's strings, plucked and struck: the guitar, the cavaquinho and bandolim, the harp, the sitar, the tanpura and the piano.

use super::common::{along, bpf, hpf, lpf, net, partials, silent_after, sine_from, struck};
use super::figures::{Figures, evened, fitted, kept};
use crate::nodes::{
    db, decay2, hit, hz, modes, perc, segment_at, shape, struck_gliding, white_noise,
};
use crate::{Note, Options, Spec, Synth, Value};
use fundsp::prelude::*;
use std::f64::consts::PI;
use std::sync::OnceLock;

/// Every synth of this family mx ships.
pub fn all() -> Vec<Synth> {
    vec![
        guitar(),
        electric_bass(),
        deep_bass(),
        plucked("cavaquinho", 0.1, 1.2, 1.05, &[420.0, 1100.0], &[1.0]),
        plucked("bandolim", 0.12, 1.6, 1.1, &[380.0, 900.0], &[1.0, 1.0025]),
        harp(),
        sitar(),
        tanpura(),
    ]
}

fn guitar() -> Synth {
    Synth::new("guitar")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .options(Spec::envelope(1000.0, 2.0))
        .build(|o, _ctx| kept(guitar_figures(), GUITAR_LEVEL, o))
}

/// The guitar's figures, fitted partial by partial to the University of Iowa's guitar.
fn guitar_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/guitar.mxf")))
}

/// The guitar's gain, as loud at E3 at vel 1 as the version measured band by band.
const GUITAR_LEVEL: f64 = 3.2;

fn electric_bass() -> Synth {
    Synth::new("electric_bass")
        .option(Spec::note("note").default(33.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .options(Spec::envelope(1000.0, 0.1))
        .build(|o, _ctx| fitted(ebass(), EBASS_LEVEL, BASS_GROWTH, o))
}

/// The electric bass's figures, fitted partial by partial to NSynth's bass_electronic_018.
fn ebass() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/electric_bass.mxf")))
}

/// The electric bass's gain at vel 1, as loud at A1 as the version measured band by band.
const EBASS_LEVEL: f64 = 0.544;

fn deep_bass() -> Synth {
    Synth::new("deep_bass")
        .option(Spec::note("note").default(33.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .options(Spec::envelope(1000.0, 0.1))
        .build(|o, _ctx| fitted(dbass(), DBASS_LEVEL, BASS_GROWTH, o))
}

/// The deep bass's figures, fitted partial by partial to NSynth's bass_electronic_025 and bass_electronic_027.
fn dbass() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/deep_bass.mxf")))
}

/// The deep bass's gain at vel 1, as loud at A1 as the version measured band by band.
const DBASS_LEVEL: f64 = 0.398;

/// How the basses grow louder with vel, as the versions measured band by band did: about 5 dB from vel 0.3 to 1.
const BASS_GROWTH: f64 = 0.5;

/// A plucked string: harmonics shaped by where it is plucked, the higher ones dying sooner, through the body's resonances; tremolo picks it again that many times a second.
fn plucked(
    name: &'static str,
    pick: f64,
    ring: f64,
    bright: f64,
    body: &'static [f64],
    courses: &'static [f64],
) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the string is plucked."),
        )
        .option(
            Spec::float("tremolo")
                .default(0.0)
                .range(0.0, f64::INFINITY)
                .doc("How many times a second a held note is picked; 0 picks it once."),
        )
        .options(Spec::envelope(0.0, 0.8))
        .build(move |o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let note = o.number("note");
            let freq = hz(note);
            let tremolo = o.number("tremolo");
            let since = move |t: f64| {
                if tremolo > 0.0 {
                    t % (1.0 / tremolo.max(0.01))
                } else {
                    t
                }
            };
            let length = ring * 2f64.powf((60.0 - note) / 24.0);
            let mut strings = Net::wrap(Box::new(zero()));
            for n in 1..=16 {
                let n = n as f64;
                if freq * n >= 14_000.0 {
                    continue;
                }
                let level = ((n * PI * pick).sin().abs() + 0.02)
                    * n.powf(-(bright + (1.0 - vel) * 0.8))
                    / courses.len() as f64;
                let fall = length / (1.0 + (n - 1.0) * 0.35);
                let fade = shape(move |t| {
                    segment_at(&[0.0, 1.0, 0.0], &[0.001, fall], &[-4.0, -4.0], since(t))
                });
                for detune in courses {
                    strings =
                        strings + sine_from(freq * n * detune, 0.0) * fade.clone() * level as f32;
                }
            }
            let damp = shape(move |t| {
                segment_at(&[1.0, 0.15, 1.0], &[0.004, 0.012], &[0.0, 0.0], since(t))
            });
            let attack_level = shape(move |t| {
                segment_at(&[0.0, 1.0, 0.0], &[0.0005, 0.008], &[-4.0, -4.0], since(t))
            });
            let attack = (white_noise(ctx.seed()) >> bpf(freq * 6.0, 1.0))
                * attack_level
                * (vel * 0.3) as f32;
            let sound = strings * damp + attack;
            let mut bodies = net(pass());
            for &f in body {
                bodies = bodies & (bpf(f, 0.4) * 0.5);
            }
            Note::envelope((sound >> bodies) * ((0.3 + vel * 0.7) * 0.3) as f32, o)
        })
}

fn harp() -> Synth {
    struck("harp", 2.0).build(|o, _ctx| evened(harp_figures(), HARP_LEVEL, o))
}

/// The harp's figures, fitted partial by partial to the folk harp of the Versilian Community Sample Library.
fn harp_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/harp.mxf")))
}

/// The harp's gain, as loud at C4 at vel 1 as the version measured band by band.
const HARP_LEVEL: f64 = 0.197;

fn sitar() -> Synth {
    struck("sitar", 2.0)
        .option(
            Spec::note("sa")
                .default(49.0)
                .doc("The note the sympathetic strings are tuned to, and an octave above it."),
        )
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let centres = [
                200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0, 12649.0,
            ];
            let level_table = [10.0, 19.7, 31.5, 30.3, 22.5, 11.9, 17.9, -0.3, -9.0, -15.0];
            let fade_table = [12.68, 9.83, 12.68, 9.79, 9.32, 5.55, 3.97, 3.77, 2.4, 2.5];
            let hz_: Vec<f64> = (1..=90).map(|n| freq * n as f64).collect();
            let levels: Vec<f64> = hz_
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    if *f < 16_000.0 {
                        db(along(&centres, &level_table, *f)
                            + (vel - 0.6) * 4.0 * ((i + 1) as f64).log2())
                    } else {
                        0.0
                    }
                })
                .collect();
            let fades: Vec<f64> = hz_
                .iter()
                .map(|f| along(&centres, &fade_table, *f))
                .collect();
            // The string glides with its note option, as meend bends it.
            let pitch = o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let bank = (1..=90)
                .zip(levels.iter().zip(&fades))
                .filter(|(_, (level, _))| **level != 0.0)
                .map(|(n, (&level, &fade))| {
                    net(map(move |x: &Frame<f32, U2>| -> Frame<f32, U2> {
                        [x[0], x[1] * n as f32].into()
                    })) >> struck_gliding(level, fade)
                });
            let string = (net(impulse::<U1>()) | pitch)
                >> bank.reduce(|a, b| a & b).expect("a string has harmonics");
            let sa = hz(o.number("sa"));
            let sympathetic_hz = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 2.0, 4.0, 6.0, 8.0].map(|r| sa * r);
            let sympathetic = modes(
                lpf(4000.0) * 0.0001,
                &sympathetic_hz,
                &[1.0, 0.5, 0.35, 0.25, 0.2, 0.15, 0.7, 0.35, 0.25, 0.15],
                &[6.0; 10],
            );
            let pick =
                (white_noise(ctx.seed()) >> bpf(freq * 12.0, 1.2)) * hit(0.0005, 0.008, -4.0) * 0.3;
            let gain = 0.0007 * (vel / 0.6).powf(1.5);
            let sound = (string >> (net(pass()) & sympathetic)) * gain as f32 + pick * vel as f32;
            Note::envelope(sound, o).within(silent_after(&levels, &fades, gain) + 6.0)
        })
}

fn tanpura() -> Synth {
    Synth::new("tanpura")
        .option(
            Spec::note("note")
                .default(52.0)
                .doc("Sa, the note the drone is tuned to."),
        )
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the strings are plucked."),
        )
        .option(
            Spec::beats("gap")
                .default(0.8)
                .range(0.05, f64::INFINITY)
                .doc("The time between plucks; a cycle of the four strings takes six gaps."),
        )
        .options(Spec::envelope(16.0, 4.0))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let gap = o.number("gap").max(0.05);
            let until = o.number("sustain");
            let body = [-7.0, 10.0, 1.0, 13.0, 12.0, 9.0];
            let slopes: Vec<f64> = body.windows(2).map(|w| w[1] - w[0]).chain([7.0]).collect();
            let curve = |f: f64| {
                let octaves = (f / 100.0).log2().clamp(0.0, 6.0);
                body[0]
                    + octaves * slopes[0]
                    + slopes
                        .windows(2)
                        .enumerate()
                        .map(|(i, w)| (w[1] - w[0]) * (octaves - (i + 1) as f64).max(0.0))
                        .sum::<f64>()
                    + 40.0 * (f / 100.0).min(1.0).log10()
            };
            let plucks = |beat: f64| {
                net(An(Pulses {
                    start: beat * gap,
                    period: 6.0 * gap,
                    until,
                    next: beat * gap,
                    t: 0.0,
                    dt: 1.0 / DEFAULT_SR,
                }))
            };
            let mut sound = net(zero());
            for (ratio, beat, offset) in [
                (0.75, 0.0, -11.0),
                (1.0, 2.0, 0.0),
                (1.0017, 3.0, 0.0),
                (0.4994, 4.0, -3.0),
            ] {
                let string_hz = freq * ratio;
                for n in 1..=24 {
                    let n = n as f64;
                    if string_hz * n >= 12_000.0 {
                        continue;
                    }
                    let level = db(curve(138.6 * ratio * n) + offset);
                    let bloom = 0.005 + 1.722 * (n - 1.0) / 47.0;
                    let fade = 25.872 * n.powf(-0.35);
                    sound = sound
                        + sine_from(string_hz * n, n * 2.4 + beat)
                            * (plucks(beat) >> decay2(bloom, fade))
                            * level as f32;
                }
                let count = (16_000.0 / string_hz).floor() as usize;
                if count > 24 {
                    let ratios: Vec<f64> = (25..=count).map(|k| k as f64).collect();
                    let levels = vec![1.0 / count as f64; ratios.len()];
                    let train = partials(
                        net(dc(string_hz as f32)),
                        &ratios,
                        &levels,
                        &vec![0.0; ratios.len()],
                    );
                    let high =
                        train >> hpf(string_hz * 24.5) >> hpf(string_hz * 24.5) >> lpf(7000.0);
                    sound = sound
                        + high * (plucks(beat) >> decay2(1.7, 7.0)) * db(45.0 + offset) as f32;
                }
            }
            let all_plucks = plucks(0.0) + plucks(2.0) + plucks(3.0) + plucks(4.0);
            let mut buzz = net(zero());
            for (centre, level) in [
                200.0, 320.0, 500.0, 800.0, 1260.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0,
            ]
            .iter()
            .zip([-12.0, 9.0, 26.0, -7.0, 16.0, 4.0, -4.0, 0.0, -6.0, -11.0])
            {
                let at = freq / 138.6 * centre;
                buzz = buzz
                    + (white_noise(ctx.seed()) >> bpf(at, 0.3) >> bpf(at, 0.3)) * db(level) as f32;
            }
            let buzz = buzz * (all_plucks >> decay2(0.05, 6.0));
            Note::envelope(
                (sound + buzz * 18.75) * (0.004 * (0.3 + vel * 0.7)) as f32,
                o,
            )
        })
}

/// Impulses every `period` seconds from `start`, until `until` seconds.
#[derive(Clone)]
struct Pulses {
    start: f64,
    period: f64,
    until: f64,
    next: f64,
    t: f64,
    dt: f64,
}

impl AudioNode for Pulses {
    const ID: u64 = 0x6d78_0041;
    type Inputs = U0;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.dt = 1.0 / sample_rate;
    }

    fn reset(&mut self) {
        (self.next, self.t) = (self.start, 0.0);
    }

    fn tick(&mut self, _input: &Frame<f32, U0>) -> Frame<f32, U1> {
        let fire = self.t + self.dt / 2.0 >= self.next && self.next < self.until;
        if fire {
            self.next += self.period;
        }
        self.t += self.dt;
        [if fire { 1.0 } else { 0.0 }].into()
    }
}

/// The piano: each harmonic a resonator struck once, with a thump of noise.
/// Its options are `note`, `vel` for how hard the key is struck, `pedal` for the sustain pedal, `aftersound` for the fade in two stages, `uneven` and `instrument` for the keys' differences, and `knock` for the hammer's knock.
pub fn piano() -> Synth {
    Synth::new("piano")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.05, 1.0)
                .doc("How hard the key is struck."),
        )
        .option(
            Spec::float("pedal")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How far the sustain pedal is down, letting the other strings ring."),
        )
        .option(
            Spec::float("aftersound")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How far each harmonic fades in two stages, fast after the strike and then slowly, as the Steinway's do."),
        )
        .option(
            Spec::float("uneven")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("How far each key differs from its neighbours in its harmonics' levels, fades and beating."),
        )
        .option(
            Spec::integer("instrument")
                .default(0.0)
                .doc("Which piano's keys `uneven` draws, each number another instrument."),
        )
        .option(
            Spec::float("knock")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("How loud the hammer's knock through the soundboard is."),
        )
        .options(Spec::envelope(1.0, 1.0))
        .build(|options, ctx| {
            let parts = piano_parts(options);
            let bank = parts
                .harmonics
                .iter()
                .map(|h| (h.hz, h.level, h.fade))
                .chain(parts.partners.iter().map(|p| (p.hz, p.level, p.fade)))
                .map(|(hz, level, fade)| crate::nodes::struck(hz, level, fade))
                .reduce(|a, b| a & b)
                .expect("a piano note has harmonics");
            let strings = Net::wrap(Box::new(impulse::<U1>())) >> bank;
            let thump = (white_noise(ctx.seed())
                >> lowpass_hz(parts.thump.below_hz as f32, 0.707))
                * shape(|t| perc(t, 0.001, 0.02));
            let sound = (strings * STRINGS + thump * parts.thump.level as f32)
                * parts.loudness as f32;
            let Some(knock) = parts.knock else {
                return Note::envelope(sound, options);
            };
            let board = knock
                .resonances
                .iter()
                .map(|&hz| crate::nodes::struck(hz, 1.0, 0.5))
                .reduce(|a, b| a & b)
                .expect("the soundboard has resonances");
            let blow = (white_noise(ctx.seed()) >> lowpass_hz(700.0, 0.707))
                * shape(|t| perc(t, 0.001, 0.012));
            let knocked = (blow >> board) * knock.level as f32;
            Note::envelope(sound + knocked, options)
        })
}

const KNOCK: f64 = 0.0007;
const STRINGS: f32 = 0.019;
const THUMP_SECONDS: f64 = 0.021;
// The soundboard's resonances stay where they are whatever the key.
const BOARD: [f64; 8] = [80.0, 105.0, 160.0, 230.0, 320.0, 350.0, 410.0, 520.0];

/// What a piano note is made of, worked out from its options before any sound exists: the piano plays a note from these and nothing else.
#[derive(Debug, Clone, PartialEq)]
pub struct PianoParts {
    /// The strings' harmonics, from the first up.
    pub harmonics: Vec<Harmonic>,
    /// The resonators that ring beside the harmonics: the slow stage of a harmonic's fade, or a twin tuned a hair apart that beats against it.
    pub partners: Vec<Partner>,
    /// The noise of the hammer's strike.
    pub thump: Thump,
    /// The hammer's knock through the soundboard, or `None` when `knock` is 0.
    pub knock: Option<Knock>,
    pub(crate) loudness: f64,
}

/// One harmonic of a piano note's strings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Harmonic {
    /// Which harmonic it is, 1 for the fundamental.
    pub number: u32,
    /// Its frequency in Hz, stretched as a piano string's are.
    pub hz: f64,
    /// The amplitude it is struck at.
    pub level: f64,
    /// The seconds it takes to fall 60 dB.
    pub fade: f64,
}

/// A resonator that rings beside one harmonic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Partner {
    /// The number of the harmonic it belongs to.
    pub harmonic: u32,
    /// Whether it is that harmonic's slow stage or a twin that beats against it.
    pub kind: PartnerKind,
    /// Its frequency in Hz.
    pub hz: f64,
    /// The amplitude it is struck at.
    pub level: f64,
    /// The seconds it takes to fall 60 dB.
    pub fade: f64,
}

/// What a partner is to its harmonic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerKind {
    /// The slow part of the harmonic's fade, left ringing after the strike's sound has gone.
    Slow,
    /// A second string of the note, tuned a hair apart, beating against the first.
    Twin,
}

/// The noise of a piano hammer's strike.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thump {
    /// The amplitude of its noise.
    pub level: f64,
    /// The frequency its noise is filtered under, in Hz.
    pub below_hz: f64,
    /// The seconds it lasts.
    pub seconds: f64,
}

/// The knock of a piano hammer through the soundboard.
#[derive(Debug, Clone, PartialEq)]
pub struct Knock {
    /// The amplitude it sounds at.
    pub level: f64,
    /// The frequencies of the soundboard's resonances it rings, in Hz.
    pub resonances: Vec<f64>,
}

/// The parts of a piano note played with `given` options, without playing it, and a warning for each option held to its range.
/// Each option not given takes its default, as when the note is played, and every level includes the scaling the note applies for its vel.
pub fn parts(given: &[(&str, f64)]) -> Result<(PianoParts, Vec<String>), crate::Error> {
    let specs = piano().specs;
    let given: Vec<(String, Value)> = given
        .iter()
        .map(|(name, x)| (name.to_string(), Value::Number(*x)))
        .collect();
    let (values, held) = crate::engine::checked("piano", &specs, &given)?;
    let warnings = held
        .iter()
        .map(|name| crate::engine::held_warning("piano", &specs, name))
        .collect();
    let options = Options::new("piano".into(), specs.into(), values);
    let mut parts = piano_parts(&options);
    let strings = STRINGS as f64 * parts.loudness;
    parts.harmonics.iter_mut().for_each(|h| h.level *= strings);
    parts.partners.iter_mut().for_each(|p| p.level *= strings);
    parts.thump.level *= parts.loudness;
    Ok((parts, warnings))
}

/// The parts of a piano note played with `options`, with the levels as the note sounds them.
pub(crate) fn piano_parts(options: &Options) -> PianoParts {
    let vel = options.number("vel");
    let knock = options.number("knock");
    let (harmonics, partners) = piano_modes(options);
    let high = db(-(options.number("note") - 72.0).clamp(0.0, 7.0));
    PianoParts {
        harmonics,
        partners,
        thump: Thump {
            level: (vel as f32 * 0.1 * 0.3) as f64,
            below_hz: (400.0 + vel as f32 * 2000.0) as f64,
            seconds: THUMP_SECONDS,
        },
        knock: (knock > 0.0).then(|| Knock {
            level: (KNOCK * knock * high * (vel / 0.6).powf(1.5)) as f32 as f64,
            resonances: BOARD.to_vec(),
        }),
        loudness: (vel / 0.6).powf(2.5),
    }
}

/// A number from -1 to 1 for one harmonic of one key of one piano, the same at every strike.
fn vary(instrument: f64, note: f64, harmonic: f64, which: u64) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for part in [
        instrument as u64,
        note.round() as u64,
        harmonic as u64,
        which,
    ] {
        for byte in part.to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash ^= hash >> 33;
    hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
    hash ^= hash >> 33;
    (hash >> 11) as f64 / (1u64 << 52) as f64 - 1.0
}

/// The strings' harmonics of a note, then their partners, with the levels the resonators are struck at.
fn piano_modes(options: &Options) -> (Vec<Harmonic>, Vec<Partner>) {
    let aftersound = options.number("aftersound");
    let uneven = options.number("uneven");
    let instrument = options.number("instrument");
    let note = options.number("note");
    let freq = hz(note);
    let vel = options.number("vel");
    let pedal = options.number("pedal");
    // 0 from C2 up, rising to 1 at A#0.
    let low = ((36.0 - note) / 14.0).clamp(0.0, 1.0);
    let soft = (1.0 - vel) / 0.7;
    let stretch = (0.00016 * 2f64.powf((note - 52.0) / 7.4)).max(0.00014);
    let slope = 12.0 * (0.6 / vel).powf(0.585);
    let corner = 1000.0 * (vel / 0.6).min(1.0).powf(0.75);

    let mut strings = Vec::new();
    let mut beating = Vec::new();
    for n in 1..=48 {
        let number = n as u32;
        let n = n as f64;
        let hz = freq * n * ((1.0 + stretch * n * n) / (1.0 + stretch)).sqrt();
        if hz >= 16000.0 || (n > 24.0 && low <= 0.0) {
            continue;
        }
        let hammer = 20.0 * ((n * PI / 8.5).sin().abs() + 0.05).log10() * 0.45;
        let bass = (30.0 - 6.0 * low) * (70.0 / hz).log2().max(0.0);
        let tilt = low * (8.0 * soft * soft - 2.0) * n.log2();
        let treble = slope * (hz / freq.max(corner)).log2().max(0.0);
        let shape = -3.0 * n.log2() + hammer - bass - 0.4 * (note - 76.0).max(0.0);
        let level = db(shape - treble - tilt - 16.0 * (n / 24.0).log2().max(0.0));
        let fade = 10.0
            * 2f64.powf((note - 66.0).max(0.0) / -14.0)
            * 2f64.powf((hz / 1500.0).log2().max(0.0) * -0.9)
            * 2f64.powf((160.0 / hz).log2().max(0.0) * -0.5)
            * (1.0
                + low
                    * (0.5 - 0.75 * (-2.0 * (hz / 300.0).log2().powi(2)).exp()
                        + 1.2 * soft * soft * ((hz / 300.0).log2() / 2.0).clamp(0.0, 1.0)));
        // The undamped strings ring at the harmonics as brightly as at vel 1 and up to twice as long below 1 kHz, the note's own at half.
        let sympathy = if n <= 24.0 {
            db(shape - 8.6 * (hz / freq.max(1000.0)).log2().max(0.0)) * pedal * 0.65
        } else {
            0.0
        };
        let rung = level * (1.0 - pedal * 0.5) + sympathy;
        let lasting =
            fade + sympathy / (rung + 1e-9) * fade * ((1000.0 / hz).log2() / 2.0).clamp(0.0, 1.0);
        if aftersound <= 0.0 && uneven <= 0.0 {
            strings.push(Harmonic {
                number,
                hz,
                level: rung,
                fade: lasting,
            });
            // A note's strings are tuned a hair apart and beat against each other.
            if n <= 10.0 {
                beating.push(Partner {
                    harmonic: number,
                    kind: PartnerKind::Twin,
                    hz: hz * 1.0001,
                    level: level * 0.5,
                    fade: fade * 1.5,
                });
            }
            continue;
        }
        let key = |which| uneven * vary(instrument, note, n, which);
        let gain = db(5.5 * (key(0) + key(1)));
        let longer = 2f64.powf(0.5 * key(2));
        let apart = 1.0
            + 0.0001
                * (1.0 + 5.0 * uneven * vary(instrument, note, 0.0, 3).abs())
                * (1.0 + 0.5 * key(4));
        let twin = if n <= 10.0 { level * 0.5 } else { 0.0 };
        // The strike's sound fades fast and leaves the slow part, 20 dB under it, to ring on.
        let slow = (rung + twin) * 0.1 * aftersound;
        let fast = rung + twin * aftersound - slow;
        // 0 up to G#3, rising to 1 at E4: the strike fades faster up the keyboard.
        let upper = ((note - 56.0) / 8.0).clamp(0.0, 1.0);
        let quick = (0.38 - 0.16 * upper).powf(aftersound);
        strings.push(Harmonic {
            number,
            hz,
            level: fast * gain,
            fade: lasting * longer * quick,
        });
        if slow > 0.0 {
            beating.push(Partner {
                harmonic: number,
                kind: PartnerKind::Slow,
                hz: hz * apart,
                level: slow * gain,
                fade: lasting * longer * (2.5 - 0.5 * upper),
            });
        }
        if twin * (1.0 - aftersound) > 0.0 {
            beating.push(Partner {
                harmonic: number,
                kind: PartnerKind::Twin,
                hz: hz * apart,
                level: twin * (1.0 - aftersound) * gain,
                fade: fade * longer * 1.5,
            });
        }
    }

    (strings, beating)
}
