//! The effects that come with the crate, each a stereo sound in and out, with options that glide when they are changed while it runs.
//! Times such as phase and decay are in seconds, or in beats in a part started by [`Engine::start_part_in_beats`](crate::Engine::start_part_in_beats).
//!
//! Notes play into a chain of effects, which [`Engine::start_effects`](crate::Engine::start_effects) makes from names and options, in order:
//!
//! ```
//! use mxs::{Engine, Event, RATE, Value, effects, synths};
//!
//! fn main() -> Result<(), mxs::Error> {
//!     let mut engine = Engine::offline(RATE);
//!     synths::all().into_iter().for_each(|s| engine.register(s));
//!     effects::all().into_iter().for_each(|e| engine.register_effect(e));
//!
//!     let reverb = ("reverb".to_string(), vec![("decay_seconds".to_string(), Value::Number(3.0))]);
//!     let chain = engine.start_effects(&[reverb])?;
//!
//!     let note = Event::Note {
//!         id: None,
//!         synth: "piano".to_string(),
//!         options: vec![("note".to_string(), Value::Number(60.0))],
//!         chain: Some(chain),
//!         held: false,
//!     };
//!     engine.play(&[(0.0, note)])?;
//!     engine.render(3.0)?;
//!     Ok(())
//! }
//! ```
//!
#![doc = include_str!("../../docs/effects.md")]

use crate::nodes::hz;
use crate::options::Options;
use crate::spec::Spec;
use crate::synth::{Effect, EffectDecl};
use delay::{Comb, Echo, PingPong, PitchShift};
pub(crate) use dynamics::Lag;
use dynamics::{Compressor, Crusher, DRIVE_LATE, Drive, Following, Limiter, Smooth};
use fundsp::prelude::*;
use reverb::Reverb;
use std::f32::consts::{FRAC_PI_4, TAU};
pub(crate) use wave::Lfo;
use wave::{Ease, Phaser, Shifted, Sides};

mod delay;
mod dynamics;
mod reverb;
mod wave;

/// Every effect that comes with the crate.
pub fn all() -> Vec<Effect> {
    vec![
        level(),
        echo(),
        lpf(),
        hpf(),
        distortion(),
        pan(),
        slicer(),
        compressor(),
        chorus(),
        bitcrusher(),
        tremolo(),
        rlpf(),
        bpf(),
        flanger(),
        wobble(),
        ping_pong(),
        pitch_shift(),
        eq(),
        limiter(),
        reverb(),
        phaser(),
        width(),
        wah(),
    ]
}

fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

/// The same mono effect on each channel, built twice.
fn per_channel(build: impl Fn() -> Net) -> Net {
    build() | build()
}

/// A MIDI note option as Hz, held to what a filter can take.
fn cutoff(options: &Options, name: &str) -> Net {
    options.follow(name)
        >> net(map(|x: &Frame<f32, U1>| {
            hz(x[0] as f64).clamp(20.0, 20_000.0) as f32
        }))
}

/// Resonance from 0 to 1 as a filter's Q: the nearer 1, the more it rings.
fn q_of(options: &Options, name: &str) -> Net {
    options.follow(name)
        >> net(map(|x: &Frame<f32, U1>| {
            1.0 / (1.0 - x[0].clamp(0.0, 1.0)).max(0.01)
        }))
}

fn level() -> Effect {
    EffectDecl::new("level")
        .tail(0.0)
        .build(|_, _| net(multipass::<U2>()))
}

fn reverb() -> Effect {
    EffectDecl::new("reverb")
        .option(
            Spec::float("size")
                .default(20.0)
                .range(5.0, 60.0)
                .doc("The size of the room in metres, read as the effect starts."),
        )
        .option(
            Spec::seconds("decay_seconds")
                .default(2.0)
                .range(0.1, 60.0)
                .doc("How many seconds its sound takes to fade 60 dB."),
        )
        .option(
            Spec::ms("predelay_ms")
                .default(0.0)
                .range(0.0, 500.0)
                .doc("The gap in milliseconds before the room starts, read as the effect starts."),
        )
        .option(
            Spec::note("low_cut")
                .default(0.0)
                .doc("The MIDI note below which the room is cut, 0 for none."),
        )
        .option(
            Spec::float("damp")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How much the room dulls the high frequencies."),
        )
        .tail_from(|o| o.number("decay_seconds") + o.number("predelay_ms") / 1000.0)
        .build(|o, _| {
            let gap = o.number("predelay_ms") / 1000.0;
            let late = per_channel(|| {
                if gap > 0.0 {
                    net(delay(gap))
                } else {
                    net(pass())
                }
            });
            let room = (late | o.follow("decay_seconds") | o.follow("damp"))
                >> net(An(Reverb::new(o.number("size") as f32)));
            room >> per_channel(|| {
                (net(pass()) | cutoff(o, "low_cut") | net(dc(FRAC_1_SQRT_2)))
                    >> net(highpass::<f32>())
            })
        })
}

/// The options of an echo whose repeats wear as a tape's do.
fn worn(effect: crate::synth::EffectDecl) -> crate::synth::EffectDecl {
    effect
        .option(Spec::note("high_cut").default(136.0).doc(
            "A MIDI note above which each echo is cut once more than the one before; off from 136.",
        ))
        .option(Spec::note("low_cut").default(0.0).doc(
            "A MIDI note below which each echo is cut once more than the one before; off at 0.",
        ))
        .option(
            Spec::float("drive")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How much each echo is rounded off, once more than the one before."),
        )
}

fn echo() -> Effect {
    worn(EffectDecl::new("echo"))
        .option(
            Spec::beats("phase")
                .default(0.25)
                .range(0.0, f64::INFINITY)
                .doc("The time between echoes."),
        )
        .option(
            Spec::beats("decay")
                .default(2.0)
                .doc("How long the echoes take to die away."),
        )
        .option(
            Spec::beats("max_phase")
                .default(2.0)
                .range(0.0, 60.0)
                .doc("The longest phase it can change to."),
        )
        .tail_from(|o| o.number("decay").abs())
        .build(|o, _| {
            let most = o.number("max_phase");
            per_channel(|| {
                let comb = (net(pass())
                    | o.follow("phase")
                    | o.follow("decay")
                    | o.follow("high_cut")
                    | o.follow("low_cut")
                    | o.follow("drive"))
                    >> net(An(Echo::new(most)));
                net(pass()) & comb
            })
        })
}

fn lpf() -> Effect {
    EffectDecl::new("lpf")
        .option(
            Spec::note("cutoff")
                .default(100.0)
                .doc("The MIDI note above which it cuts."),
        )
        .tail(0.1)
        .build(|o, _| {
            per_channel(|| {
                (net(pass()) | cutoff(o, "cutoff") | net(dc(FRAC_1_SQRT_2)))
                    >> net(lowpass::<f32>())
            })
        })
}

fn hpf() -> Effect {
    EffectDecl::new("hpf")
        .option(
            Spec::note("cutoff")
                .default(100.0)
                .doc("The MIDI note below which it cuts."),
        )
        .tail(0.1)
        .build(|o, _| {
            per_channel(|| {
                (net(pass()) | cutoff(o, "cutoff") | net(dc(FRAC_1_SQRT_2)))
                    >> net(highpass::<f32>())
            })
        })
}

const FRAC_1_SQRT_2: f32 = std::f32::consts::FRAC_1_SQRT_2;

fn distortion() -> Effect {
    EffectDecl::new("distortion")
        .option(
            Spec::float("distort")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How hard it drives the sound."),
        )
        .option(
            Spec::name("curve", &["soft", "hard"])
                .default("soft")
                .doc("How it bends the sound: soft rounds it off and hard cuts it off flat; read as the effect starts."),
        )
        .option(
            Spec::note("tone")
                .default(0.0)
                .doc("A MIDI note for a low-pass after it, off at 0."),
        )
        .tail(0.1)
        .late(DRIVE_LATE)
        .build(|o, _| {
            let hard = o.name("curve") == "hard";
            per_channel(|| {
                let driven = (net(pass()) | o.follow("distort")) >> net(An(Drive::new(hard)));
                let off = o.follow("tone")
                    >> net(map(|x: &Frame<f32, U1>| if x[0] > 0.0 { 1.0 } else { 0.0 }));
                let filtered = net(pass())
                    ^ ((net(pass()) | cutoff(o, "tone") | net(dc(FRAC_1_SQRT_2)))
                        >> net(lowpass::<f32>()));
                ((driven >> filtered) | off)
                    >> net(map(|x: &Frame<f32, U3>| x[0] + (x[1] - x[0]) * x[2]))
            })
        })
}

fn pan() -> Effect {
    EffectDecl::new("pan")
        .option(
            Spec::float("pan")
                .default(0.0)
                .range(-1.0, 1.0)
                .doc("Where it turns the sound, from -1, left, to 1, right."),
        )
        .tail(0.0)
        .build(|o, _| {
            (net(multipass::<U2>()) | o.follow("pan"))
                >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U2> {
                    let pan = x[2].clamp(-1.0, 1.0);
                    [x[0] * (1.0 - pan).min(1.0), x[1] * (1.0 + pan).min(1.0)].into()
                }))
        })
}

fn slicer() -> Effect {
    EffectDecl::new("slicer")
        .option(
            Spec::beats("phase")
                .default(0.25)
                .range(0.0, f64::INFINITY)
                .doc("The length of a cycle."),
        )
        .option(
            Spec::float("phase_offset")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("Where in the cycle it starts."),
        )
        .option(
            Spec::float("pulse_width")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("The part of the cycle that is on."),
        )
        .option(
            Spec::float("amp_min")
                .default(0.0)
                .doc("The level while off."),
        )
        .option(
            Spec::float("amp_max")
                .default(1.0)
                .doc("The level while on."),
        )
        .option(
            Spec::ms("smooth_ms")
                .default(0.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds each change takes."),
        )
        .option(spread())
        .tail(0.0)
        .build(|o, _| {
            let open = (rate_of(o, "phase")
                | o.follow("pulse_width")
                | net(dc(1.0))
                | net(dc(0.0))
                | eased(o, "phase_offset")
                | eased(o, "spread"))
                >> net(An(Sides::new(o.number("phase_offset") as f32, true)));
            let level = || {
                let level = (net(pass()) | o.follow("amp_min") | o.follow("amp_max"))
                    >> net(map(|x: &Frame<f32, U3>| x[1] + x[0] * (x[2] - x[1])));
                (level | o.follow("smooth_ms")) >> net(An(Smooth::new()))
            };
            (net(multipass::<U2>()) | (open >> (level() | level()))) >> net(map(each_side))
        })
}

/// An option that moves to each new value a change gives it over 10 ms for a whole step, so the change does not click.
fn eased(options: &Options, name: &str) -> Net {
    options.follow(name) >> net(An(Ease::default()))
}

/// An option that is off below 0.5 and on from it, as a level that goes from 0 to 1 and back over 10 ms.
fn switched(options: &Options, name: &str) -> Net {
    options.follow(name)
        >> net(map(
            |x: &Frame<f32, U1>| if x[0] >= 0.5 { 1.0 } else { 0.0 },
        ))
        >> net(An(Ease::default()))
}

/// `off` at a level of 0, `on` at 1, and between the two as the level goes from one to the other.
fn between(off: f32, on: f32, level: f32) -> f32 {
    if level <= 0.0 {
        off
    } else if level >= 1.0 {
        on
    } else {
        off + (on - off) * level
    }
}

/// Cycles a second of an option measured as the length of one cycle.
fn rate_of(options: &Options, name: &str) -> Net {
    options.follow(name) >> net(map(|x: &Frame<f32, U1>| 1.0 / x[0].max(0.001)))
}

fn compressor() -> Effect {
    EffectDecl::new("compressor")
        .option(
            Spec::float("threshold")
                .default(-18.0)
                .range(-80.0, 0.0)
                .doc("The level above which it lowers the sound, in dB below full scale."),
        )
        .option(
            Spec::float("ratio").default(4.0).range(1.0, 100.0).doc(
                "How many dB the sound has to rise above the threshold to come out 1 dB louder.",
            ),
        )
        .option(
            Spec::float("makeup")
                .default(0.0)
                .range(-40.0, 40.0)
                .doc("How many dB it raises the sound afterwards."),
        )
        .option(
            Spec::ms("clamp_ms")
                .default(10.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds it takes to lower the sound."),
        )
        .option(
            Spec::ms("relax_ms")
                .default(150.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds it takes to let go."),
        )
        .tail(0.1)
        .build(|o, _| {
            let controls = o.follow("threshold")
                | o.follow("ratio")
                | o.follow("makeup")
                | (o.follow("clamp_ms") * 0.001)
                | (o.follow("relax_ms") * 0.001);
            (net(multipass::<U2>()) | controls | o.key()) >> net(An(Compressor::new()))
        })
}

fn chorus() -> Effect {
    EffectDecl::new("chorus")
        .option(
            Spec::beats("phase")
                .default(0.25)
                .range(0.0, f64::INFINITY)
                .doc("How long the delay takes to sweep back and forth."),
        )
        .option(
            Spec::beats("decay")
                .default(0.00001)
                .doc("How long the delayed sound feeds back."),
        )
        .option(
            Spec::beats("max_phase")
                .default(1.0)
                .range(0.0, 60.0)
                .doc("The longest phase it can change to."),
        )
        .tail_from(|o| o.number("decay").abs())
        .build(|o, _| {
            let most = o.number("max_phase").max(0.02);
            let side = |start: f32| {
                let sweep = (rate_of(o, "phase") | net(dc(0.5)) | net(dc(3.0)) | net(dc(0.0)))
                    >> net(An(Lfo::new(start, 1.0, false)))
                    >> net(map(|x: &Frame<f32, U1>| 0.007 + 0.005 * (x[0] * 2.0 - 1.0)));
                let comb =
                    (net(pass()) | sweep | o.follow("decay")) >> net(An(Comb::new(most, true)));
                (net(pass()) & comb) * 0.5
            };
            side(0.0) | side(0.5)
        })
}

fn bitcrusher() -> Effect {
    EffectDecl::new("bitcrusher")
        .option(
            Spec::hz("sample_rate")
                .default(10_000.0)
                .range(1.0, f64::INFINITY)
                .doc("The rate in Hz it holds samples at."),
        )
        .option(
            Spec::float("bits")
                .default(8.0)
                .range(1.0, 24.0)
                .doc("How many bits it rounds each sample to."),
        )
        .option(
            Spec::note("cutoff")
                .default(0.0)
                .doc("A MIDI note for a low-pass after it, off at 0."),
        )
        .tail(0.1)
        .build(|o, _| {
            per_channel(|| {
                let crushed = (net(pass()) | o.follow("sample_rate") | o.follow("bits"))
                    >> net(An(Crusher::new()));
                let off = o.follow("cutoff")
                    >> net(map(|x: &Frame<f32, U1>| if x[0] > 0.0 { 1.0 } else { 0.0 }));
                let filtered = net(pass())
                    ^ ((net(pass()) | cutoff(o, "cutoff") | net(dc(FRAC_1_SQRT_2)))
                        >> net(lowpass::<f32>()));
                ((crushed >> filtered) | off)
                    >> net(map(|x: &Frame<f32, U3>| x[0] + (x[1] - x[0]) * x[2]))
            })
        })
}

/// The wave options every swept effect takes, with its own default wave.
fn wave_options(
    effect: crate::synth::EffectDecl,
    wave: f64,
    phase: f64,
) -> crate::synth::EffectDecl {
    effect
        .option(
            Spec::beats("phase")
                .default(phase)
                .range(0.0, f64::INFINITY)
                .doc("How long one sweep takes."),
        )
        .option(
            Spec::float("wave")
                .default(wave)
                .range(0.0, 4.0)
                .doc("The sweep's shape: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic."),
        )
        .option(
            Spec::float("invert_wave")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 flips the sweep."),
        )
        .option(
            Spec::float("phase_offset")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("Where in the sweep it starts."),
        )
}

/// The sweep from 0 to 1, of the shape and length the wave options give.
fn sweep(options: &Options, width: Net) -> Net {
    (rate_of(options, "phase")
        | width
        | options.follow("wave")
        | options.follow("invert_wave")
        | eased(options, "phase_offset"))
        >> net(An(Shifted::new(
            options.number("phase_offset") as f32,
            false,
        )))
}

/// The sweep from 0 to 1 for each side, the right the share `spread` of half a cycle after the left.
fn sweeps(options: &Options, width: Net) -> Net {
    (rate_of(options, "phase")
        | width
        | options.follow("wave")
        | options.follow("invert_wave")
        | eased(options, "phase_offset")
        | eased(options, "spread"))
        >> net(An(Sides::new(options.number("phase_offset") as f32, false)))
}

fn spread() -> Spec {
    Spec::float("spread")
        .default(0.0)
        .range(0.0, 1.0)
        .doc("How far the right side's cycle runs after the left's: 1 is half a cycle, so the sound moves from side to side.")
}

/// Each side at a level of its own.
/// Inputs: left, right, the left's level, the right's level.
fn each_side(x: &Frame<f32, U4>) -> Frame<f32, U2> {
    [x[0] * x[2], x[1] * x[3]].into()
}

fn tremolo() -> Effect {
    wave_options(EffectDecl::new("tremolo"), 2.0, 4.0)
        .option(
            Spec::float("depth")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How far it turns the sound down."),
        )
        .option(spread())
        .tail(0.0)
        .build(|o, _| {
            let level = || {
                (net(pass()) | o.follow("depth"))
                    >> net(map(|x: &Frame<f32, U2>| 1.0 - x[1] * x[0]))
            };
            let levels = sweeps(o, net(dc(0.5))) >> (level() | level());
            (net(multipass::<U2>()) | levels) >> net(map(each_side))
        })
}

fn width() -> Effect {
    EffectDecl::new("width")
        .option(
            Spec::float("side")
                .default(1.0)
                .range(0.0, 2.0)
                .doc("How wide the sound is: 0 is mono, 1 is as it came and 2 is twice the difference between its sides."),
        )
        .option(
            Spec::note("mono_below")
                .default(0.0)
                .doc("A MIDI note below which everything is put in the centre, off at 0."),
        )
        .tail(0.1)
        .build(|o, _| {
            // Two low-passes in a row and two high-passes in a row add back up to the whole sound, so the centre keeps its level around the note.
            let twice = |filter: fn() -> Net| {
                let once = |sound: Net| {
                    (sound | cutoff(o, "mono_below") | net(dc(FRAC_1_SQRT_2))) >> filter()
                };
                once(once(net(pass())))
            };
            let low: fn() -> Net = || net(lowpass::<f32>());
            let high: fn() -> Net = || net(highpass::<f32>());
            let centre = net(pass()) ^ (twice(low) & twice(high));
            let apart = net(pass()) ^ twice(high);
            let halves = net(map(|x: &Frame<f32, U2>| -> Frame<f32, U2> {
                [(x[0] + x[1]) * 0.5, (x[0] - x[1]) * 0.5].into()
            }));
            (halves >> (centre | apart) | o.follow("side") | o.follow("mono_below"))
                >> net(map(|x: &Frame<f32, U6>| -> Frame<f32, U2> {
                    let (centre, apart) = if x[5] > 0.0 { (x[1], x[3]) } else { (x[0], x[2]) };
                    [centre + x[4] * apart, centre - x[4] * apart].into()
                }))
        })
}

/// An option in dB as the factor a filter raises its band by.
fn gain_of(options: &Options, name: &str) -> Net {
    options.follow(name) >> net(map(|x: &Frame<f32, U1>| db_amp(x[0])))
}

fn eq() -> Effect {
    EffectDecl::new("eq")
        .option(
            Spec::note("low_at")
                .default(36.0)
                .doc("The MIDI note below which the low band acts."),
        )
        .option(
            Spec::float("low_gain")
                .default(0.0)
                .doc("How far the low band is raised or lowered, in dB."),
        )
        .option(
            Spec::note("mid_at")
                .default(60.0)
                .doc("The MIDI note the mid band is around."),
        )
        .option(
            Spec::float("mid_gain")
                .default(0.0)
                .doc("How far the mid band is raised or lowered, in dB."),
        )
        .option(
            Spec::float("mid_q")
                .default(1.0)
                .range(0.1, 20.0)
                .doc("How narrow the mid band is: the higher, the narrower."),
        )
        .option(
            Spec::note("high_at")
                .default(96.0)
                .doc("The MIDI note above which the high band acts."),
        )
        .option(
            Spec::float("high_gain")
                .default(0.0)
                .doc("How far the high band is raised or lowered, in dB."),
        )
        .tail(0.2)
        .build(|o, _| {
            per_channel(|| {
                let shelf = || net(dc(FRAC_1_SQRT_2));
                let low = (net(pass()) | cutoff(o, "low_at") | shelf() | gain_of(o, "low_gain"))
                    >> net(lowshelf::<f32>());
                let mid = (low | cutoff(o, "mid_at") | o.follow("mid_q") | gain_of(o, "mid_gain"))
                    >> net(bell::<f32>());
                (mid | cutoff(o, "high_at") | shelf() | gain_of(o, "high_gain"))
                    >> net(highshelf::<f32>())
            })
        })
}

/// How far ahead of its output the limiter reads, in seconds, which is how late its sound comes out.
pub const LIMITER_LOOK_AHEAD: f64 = 0.005;

fn limiter() -> Effect {
    EffectDecl::new("limiter")
        .option(
            Spec::float("ceiling")
                .default(-1.0)
                .range(-60.0, 0.0)
                .doc("The level no peak passes, in dB below full scale."),
        )
        .option(
            Spec::ms("relax_ms")
                .default(100.0)
                .range(1.0, 10_000.0)
                .doc("How many milliseconds it takes to let the sound back up."),
        )
        .tail(LIMITER_LOOK_AHEAD)
        .build(|o, _| {
            (net(multipass::<U2>()) | gain_of(o, "ceiling") | o.follow("relax_ms"))
                >> net(An(Limiter::new()))
        })
}

fn rlpf() -> Effect {
    EffectDecl::new("rlpf")
        .option(
            Spec::note("cutoff")
                .default(100.0)
                .doc("The MIDI note above which it cuts."),
        )
        .option(
            Spec::float("res")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How much it rings at its cutoff."),
        )
        .tail(0.2)
        .build(|o, _| {
            per_channel(|| {
                (net(pass()) | cutoff(o, "cutoff") | q_of(o, "res")) >> net(lowpass::<f32>())
            })
        })
}

/// How far under full scale, in dB, a sound leaves the wah's filter at `cutoff_min`.
const WAH_FLOOR: f32 = 40.0;

fn wah() -> Effect {
    EffectDecl::new("wah")
        .option(
            Spec::note("cutoff_min")
                .default(60.0)
                .doc("The MIDI note the filter is at with silence coming in."),
        )
        .option(
            Spec::note("cutoff_max")
                .default(100.0)
                .doc("The MIDI note the filter is at with a sound at full scale coming in."),
        )
        .option(
            Spec::float("res")
                .default(0.7)
                .range(0.0, 1.0)
                .doc("How much it rings at its cutoff."),
        )
        .option(
            Spec::float("sensitivity")
                .default(0.0)
                .doc("How far the level it follows is raised or lowered, in dB."),
        )
        .option(
            Spec::ms("clamp_ms")
                .default(10.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds it takes to follow a rising sound."),
        )
        .option(
            Spec::ms("relax_ms")
                .default(150.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds it takes to follow a falling sound."),
        )
        .tail(0.2)
        .build(|o, _| {
            let controls = (o.follow("clamp_ms") * 0.001)
                | (o.follow("relax_ms") * 0.001)
                | o.follow("sensitivity")
                | o.follow("cutoff_min")
                | o.follow("cutoff_max");
            let cut = (net(multipass::<U2>()) | controls) >> net(An(Following::new()));
            ((net(multipass::<U2>()) ^ cut) | q_of(o, "res"))
                >> net(map(|x: &Frame<f32, U4>| -> Frame<f32, U6> {
                    [x[0], x[2], x[3], x[1], x[2], x[3]].into()
                }))
                >> (net(lowpass::<f32>()) | net(lowpass::<f32>()))
        })
}

fn bpf() -> Effect {
    EffectDecl::new("bpf")
        .option(
            Spec::note("centre")
                .default(100.0)
                .doc("The MIDI note of the band it keeps."),
        )
        .option(
            Spec::float("res")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How narrow the band is."),
        )
        .tail(0.2)
        .build(|o, _| {
            per_channel(|| {
                // FunDSP's band-pass peaks at its Q, so it is scaled back to a peak of 1.
                let band =
                    (net(pass()) | cutoff(o, "centre") | q_of(o, "res")) >> net(bandpass::<f32>());
                (band | q_of(o, "res")) >> net(map(|x: &Frame<f32, U2>| x[0] / x[1]))
            })
        })
}

fn flanger() -> Effect {
    wave_options(EffectDecl::new("flanger"), 4.0, 4.0)
        .option(
            Spec::float("stereo_invert_wave")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 sweeps the left channel the other way."),
        )
        .option(
            Spec::ms("delay_ms")
                .default(5.0)
                .doc("The delay it sweeps up from."),
        )
        .option(Spec::ms("depth_ms").default(5.0).doc("How far it sweeps."))
        .option(
            Spec::ms("max_delay_ms")
                .default(20.0)
                .range(1.0, 1000.0)
                .doc("The longest delay."),
        )
        .option(
            Spec::float("feedback")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How strongly the delayed sound feeds back."),
        )
        .option(
            Spec::beats("decay")
                .default(2.0)
                .doc("How long the delayed sound feeds back."),
        )
        .option(
            Spec::float("invert_flange")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 subtracts the delayed sound instead."),
        )
        .tail(1.0)
        .build(|o, _| {
            let most = o.number("max_delay_ms") / 1000.0;
            let side = |flipped: Net| {
                let level = (sweep(o, net(dc(0.5))) | flipped)
                    >> net(map(|x: &Frame<f32, U2>| between(x[0], 1.0 - x[0], x[1])));
                let limit = most as f32;
                let delay = (o.follow("delay_ms") | o.follow("depth_ms") | level)
                    >> net(map(move |x: &Frame<f32, U3>| {
                        ((x[0] + x[1] * x[2]) / 1000.0).clamp(0.0001, limit)
                    }));
                let decay = (o.follow("decay") | o.follow("feedback"))
                    >> net(map(|x: &Frame<f32, U2>| x[0] * x[1].clamp(0.0, 1.0)));
                let comb = (net(pass()) | delay | decay) >> net(An(Comb::new(most, true)));
                (net(pass()) | switched(o, "invert_flange"))
                    >> net(map(|x: &Frame<f32, U2>| -> Frame<f32, U3> {
                        [x[0], x[0], x[1]].into()
                    }))
                    >> (net(pass()) | comb | net(pass()))
                    >> net(map(|x: &Frame<f32, U3>| x[0] + x[1] * (1.0 - 2.0 * x[2])))
            };
            side(switched(o, "stereo_invert_wave")) | side(net(dc(0.0)))
        })
}

fn phaser() -> Effect {
    wave_options(EffectDecl::new("phaser"), 3.0, 4.0)
        .option(
            Spec::float("stereo_invert_wave")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("1 sweeps the left channel the other way."),
        )
        .option(
            Spec::integer("stages")
                .default(4.0)
                .range(2.0, 8.0)
                .doc("How many stages it has, 2, 4, 6 or 8, one notch for every two, read as the effect starts."),
        )
        .option(
            Spec::note("low")
                .default(60.0)
                .doc("The MIDI note its sweep starts from."),
        )
        .option(
            Spec::note("high")
                .default(108.0)
                .doc("The MIDI note its sweep reaches."),
        )
        .option(
            Spec::float("feedback")
                .default(0.0)
                .range(0.0, 0.95)
                .doc("How much of its sound feeds back, which raises the sound between the notches."),
        )
        .option(
            Spec::float("mix")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How much of the turned sound replaces the dry one; the notches are deepest at 0.5."),
        )
        .tail(0.1)
        .build(|o, _| {
            let stages = o.number("stages");
            if ![2.0, 4.0, 6.0, 8.0].contains(&stages) {
                panic!("phaser: stages is 2, 4, 6 or 8, not {stages}");
            }
            let side = |flipped: Net| {
                let level = (sweep(o, net(dc(0.5))) | flipped)
                    >> net(map(|x: &Frame<f32, U2>| between(x[0], 1.0 - x[0], x[1])));
                let corner = (level | o.follow("low") | o.follow("high"))
                    >> net(map(|x: &Frame<f32, U3>| {
                        hz((x[1] + x[0] * (x[2] - x[1])) as f64).clamp(20.0, 20_000.0) as f32
                    }));
                (net(pass()) | corner | o.follow("feedback"))
                    >> net(An(Phaser::new(stages as usize)))
            };
            side(switched(o, "stereo_invert_wave")) | side(net(dc(0.0)))
        })
}

fn wobble() -> Effect {
    wave_options(EffectDecl::new("wobble"), 0.0, 0.5)
        .option(
            Spec::float("pulse_width")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("The part of a cycle that is up, for wave 1."),
        )
        .option(
            Spec::note("cutoff_min")
                .default(60.0)
                .doc("The MIDI note the filter sweeps down to."),
        )
        .option(
            Spec::note("cutoff_max")
                .default(120.0)
                .doc("The MIDI note the filter sweeps up to."),
        )
        .option(
            Spec::float("res")
                .default(0.8)
                .range(0.0, 1.0)
                .doc("How much the filter rings at its cutoff."),
        )
        .option(
            Spec::float("filter")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("0 for a low-pass and 1 for a high-pass."),
        )
        .tail(0.2)
        .build(|o, _| {
            per_channel(|| {
                let level = sweep(o, o.follow("pulse_width"));
                let cut = (level | o.follow("cutoff_min") | o.follow("cutoff_max"))
                    >> net(map(|x: &Frame<f32, U3>| {
                        hz((x[1] + x[0] * (x[2] - x[1])) as f64).clamp(20.0, 20_000.0) as f32
                    }));
                let input = net(pass()) | cut | q_of(o, "res");
                let both = input
                    >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U6> {
                        [x[0], x[1], x[2], x[0], x[1], x[2]].into()
                    }))
                    >> (net(lowpass::<f32>()) | net(highpass::<f32>()));
                (both | switched(o, "filter"))
                    >> net(map(|x: &Frame<f32, U3>| between(x[0], x[1], x[2])))
            })
        })
}

fn ping_pong() -> Effect {
    worn(EffectDecl::new("ping_pong"))
        .option(
            Spec::beats("phase")
                .default(0.25)
                .range(0.001, f64::INFINITY)
                .doc("The time between echoes."),
        )
        .option(
            Spec::float("feedback")
                .default(0.5)
                .range(0.0, 0.999)
                .doc("How much of each echo the next keeps."),
        )
        .option(
            Spec::beats("max_phase")
                .default(1.0)
                .range(0.01, 60.0)
                .doc("The longest phase it can change to."),
        )
        .option(
            Spec::float("pan_start")
                .default(1.0)
                .range(-1.0, 1.0)
                .doc("The side of the first echo, from -1, left, to 1, right."),
        )
        .tail_from(|o| {
            let (phase, feedback) = (o.number("phase"), o.number("feedback").max(0.001));
            (2.0 * phase * (-60.0 / (20.0 * feedback.log10())).max(1.0)).min(60.0)
        })
        .build(|o, _| {
            (net(multipass::<U2>())
                | o.follow("phase")
                | o.follow("feedback")
                | eased(o, "pan_start")
                | o.follow("high_cut")
                | o.follow("low_cut")
                | o.follow("drive"))
                >> net(An(PingPong::new(o.number("max_phase"))))
        })
}

fn pitch_shift() -> Effect {
    EffectDecl::new("pitch_shift")
        .option(
            Spec::float("pitch")
                .default(0.0)
                .range(-24.0, 24.0)
                .doc("How many semitones the sound is shifted by; the effect delays the sound by up to 40 ms."),
        )
        .tail(0.04)
        .build(|o, _| {
            per_channel(|| (net(pass()) | o.follow("pitch")) >> net(An(PitchShift::new())))
        })
}
