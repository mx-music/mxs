//! mx's strings, plucked and struck: the guitar, the cavaquinho and bandolim, the harp, the sitar, the tanpura and the piano.

use super::common::{along, bpf, hpf, lpf, net, partials, silent_after, sine_from, struck};
use super::figures::{Figures, Recipe, Voice, evened, fitted, kept, strike_among};
use crate::nodes::{db, decay2, hit, hz, modes, segment_at, shape, struck_gliding, white_noise};
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
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| kept(guitar_figures(), GUITAR_LEVEL, o, ctx))
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
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| fitted(ebass(), EBASS_LEVEL, BASS_GROWTH, o, ctx))
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
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| fitted(dbass(), DBASS_LEVEL, BASS_GROWTH, o, ctx))
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
    struck("harp", 2.0)
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|o, ctx| evened(harp_figures(), HARP_LEVEL, o, ctx))
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

/// The piano: the Steinway B of the Versilian Community Sample Library played from figures fitted partial by partial to its recordings, with the pedal and without.
/// Its options are `note`, `vel` for how hard the key is struck, `pedal` for the sustain pedal, `aftersound` for the fade in two stages, `uneven` and `instrument` for the differences of the keys not recorded, and `knock` for the hammer's knock.
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
                .doc("How far each partial fades in two stages, fast after the strike and then slowly: 0.5 as the Steinway's do, 0 at one rate, and 1 with a deeper fast stage."),
        )
        .option(
            Spec::float("uneven")
                .default(1.0)
                .range(0.0, 1.0)
                .doc("How far each key that was not recorded differs from the recorded keys either side in its partials' levels."),
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
                .doc("How loud the hammer's knock under 600 Hz is: 1 as recorded, 0 left out."),
        )
        .options(Spec::envelope(1.0, 1.0))
        .options(Spec::expressions(&["bend", "brightness"]))
        .build(|options, ctx| {
            let pedal = options.number("pedal");
            // A note with the pedal all the way up or down plays one voice, which can be kept ready for either.
            if pedal == 0.0 || pedal == 1.0 {
                let choices = [0.0, 1.0].map(|down| (piano_at(down), piano_recipe(options, down)));
                return strike_among(choices.into(), pedal as usize, options, ctx);
            }
            let sound = [(0.0, 1.0 - pedal), (1.0, pedal)]
                .into_iter()
                .filter(|(_, share)| *share > 0.0)
                .map(|(down, share)| {
                    let (voice, gain) = piano_voice(options, down);
                    let end = voice.silent_after();
                    (voice.sound() * (gain * share) as f32, end)
                })
                .reduce(|(a, x), (b, y)| (a + b, x.max(y)))
                .expect("a piano note has a voice");
            Note::envelope(sound.0, options).within(sound.1)
        })
}

/// The piano's figures without the pedal, fitted partial by partial to the Steinway's recordings.
fn piano_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/piano.mxf")))
}

/// The piano's figures with the pedal down.
fn piano_pedal_figures() -> &'static Figures {
    static FIGURES: OnceLock<Figures> = OnceLock::new();
    FIGURES.get_or_init(|| Figures::read(include_bytes!("figures/piano_pedal.mxf")))
}

/// The piano's gain at vel 1, as loud at C4 as the version measured band by band.
const PIANO_LEVEL: f64 = 0.2034;
/// The gain of the figures with the pedal down against those without, so C4 at vel 1 with the pedal is as loud as the version measured band by band.
const PEDAL_LEVEL: f64 = 1.21;
/// How the piano grows louder with vel, as the version measured band by band did.
const PIANO_GROWTH: f64 = 2.62;
/// The most a partial of a key not recorded stands from the keys either side at `uneven: 1`, in dB, so the median is half of it, as the Steinway's keys stand from their neighbours.
const UNEVEN: f64 = 10.0;
/// How much quieter the keys from C3 down play, in dB, so a single strike peaks under full scale with the pedal down; the trim eases off up to C4.
const BASS_TRIM: f64 = -2.0;
/// How much quieter each key above C4 plays per semitone, in dB, to the most of TREBLE_MOST, so the treble stands under C4 at vel 0.6 as it did before the refit, within 4 dB.
const TREBLE_TRIM: f64 = -0.6;
const TREBLE_MOST: f64 = -20.0;
/// The frequency under which the strike's noise is the hammer's knock, in Hz.
const KNOCK_UNDER: f64 = 600.0;

/// What a piano note with `options` plays with its pedal up at 0 or down at 1, and its gain: its recordings are brought to the same peak, which leaves a key with a sharp strike weaker than the next, so every key at every velocity carries the same energy in its partials over their first half second, its differences from `uneven` included, and plays louder by vel alone.
fn piano_voice(options: &Options, down: f64) -> (Voice, f64) {
    let mut voice = Voice::empty();
    let gain = piano_recipe(options, down).play(
        options.number("note"),
        options.number("vel"),
        &mut voice,
        &mut Voice::empty(),
    );
    (voice, gain)
}

/// The piano's figures with its pedal up at 0 or down at 1.
fn piano_at(down: f64) -> &'static Figures {
    if down > 0.0 {
        piano_pedal_figures()
    } else {
        piano_figures()
    }
}

/// How `piano_voice` fills a voice for a note and vel, with the rest of `options`, and its gain.
fn piano_recipe(options: &Options, down: f64) -> Recipe {
    let figures = piano_at(down);
    let aftersound = options.number("aftersound");
    let knock = options.number("knock");
    let uneven = options.number("uneven");
    let instrument = options.number("instrument");
    let fill = move |note: f64, vel: f64, voice: &mut Voice| {
        figures.fill(voice, note, vel);
        voice.bend(aftersound / 0.5);
        voice.noise_under(KNOCK_UNDER, knock);
        if !figures.recorded(note) {
            let pitch = hz(note);
            voice.vary(|partial_hz| {
                uneven * UNEVEN * vary(instrument, note, (4.0 * partial_hz / pitch).round(), 0)
            });
        }
    };
    let gain = move |note: f64, vel: f64, voice: &Voice, _: &mut Voice| {
        // The partials alone: the noise's estimated energy runs up to 4 dB over what the bands play.
        let evened = voice.energy().sqrt().recip();
        let trim = db(BASS_TRIM * ((60.0 - note) / 12.0).clamp(0.0, 1.0)
            + (TREBLE_TRIM * (note - 60.0).max(0.0)).max(TREBLE_MOST));
        let level = if down > 0.0 {
            PIANO_LEVEL * PEDAL_LEVEL
        } else {
            PIANO_LEVEL
        };
        level * vel.powf(PIANO_GROWTH) * evened * trim
    };
    Recipe::new(fill, gain)
}

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
    Ok((piano_parts(&options), warnings))
}

/// The parts of a piano note played with `options`, from the figures its pedal mostly plays, with the levels as the note sounds them.
/// The loudest partial within 3% of a whole multiple of the note is that harmonic; any other is a twin of the harmonic nearest it.
pub(crate) fn piano_parts(options: &Options) -> PianoParts {
    let (voice, gain) = piano_voice(options, (options.number("pedal") >= 0.5) as u8 as f64);
    let freq = hz(options.number("note"));
    let mut harmonics = vec![];
    let mut partners = vec![];
    let mut partials = voice.partials();
    partials.sort_by(|a, b| b.1.total_cmp(&a.1));
    for (hz, level, fade) in partials {
        let ratio = hz / freq;
        let number = ratio.round().max(1.0) as u32;
        let level = db(level) * gain;
        if (ratio - number as f64).abs() <= 0.03 * number as f64
            && !harmonics.iter().any(|h: &Harmonic| h.number == number)
        {
            harmonics.push(Harmonic {
                number,
                hz,
                level,
                fade,
            });
        } else {
            partners.push(Partner {
                harmonic: number,
                kind: PartnerKind::Twin,
                hz,
                level,
                fade,
            });
        }
    }
    harmonics.sort_by_key(|h| h.number);
    let noise = voice.noise();
    let (_, loudest, seconds) =
        noise.iter().copied().fold(
            ((0.0, 0.0), -160.0, 0.0),
            |a, b| if b.1 > a.1 { b } else { a },
        );
    let amplitude = |bands: &[&((f64, f64), f64, f64)]| {
        bands
            .iter()
            .map(|(_, power, _)| 10f64.powf(power / 10.0))
            .sum::<f64>()
            .sqrt()
            * gain
    };
    let all: Vec<_> = noise.iter().collect();
    let knocked: Vec<_> = noise
        .iter()
        .filter(|((_, high), power, _)| *high <= KNOCK_UNDER && *power > -150.0)
        .collect();
    let below_hz = noise
        .iter()
        .filter(|(_, power, _)| *power >= loudest - 20.0)
        .map(|((_, high), _, _)| *high)
        .fold(0.0, f64::max);
    PianoParts {
        harmonics,
        partners,
        thump: Thump {
            level: amplitude(&all),
            below_hz,
            seconds,
        },
        knock: (!knocked.is_empty()).then(|| Knock {
            level: amplitude(&knocked),
            resonances: knocked
                .iter()
                .map(|((low, high), _, _)| (low * high).sqrt())
                .collect(),
        }),
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
