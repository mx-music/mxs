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
#![doc = include_str!("../docs/effects.md")]

use crate::nodes::hz;
use crate::options::Options;
use crate::spec::Spec;
use crate::synth::Effect;
use fundsp::prelude::*;
use std::f32::consts::{FRAC_PI_4, TAU};

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
    Effect::new("level")
        .tail(0.0)
        .build(|_, _| net(multipass::<U2>()))
}

fn reverb() -> Effect {
    Effect::new("reverb")
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
    worn(Effect::new("echo"))
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
    Effect::new("lpf")
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
    Effect::new("hpf")
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
    Effect::new("distortion")
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
    Effect::new("pan")
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
    Effect::new("slicer")
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
    Effect::new("compressor")
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
    Effect::new("chorus")
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
    Effect::new("bitcrusher")
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
    wave_options(Effect::new("tremolo"), 2.0, 4.0)
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
    Effect::new("width")
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
    Effect::new("eq")
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
    Effect::new("limiter")
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
    Effect::new("rlpf")
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

fn bpf() -> Effect {
    Effect::new("bpf")
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
    wave_options(Effect::new("flanger"), 4.0, 4.0)
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
    wave_options(Effect::new("phaser"), 3.0, 4.0)
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

/// A chain of first-order all-passes that share a corner, with the chain's output fed back to its input.
/// Each stage turns the sound from nothing at the lowest frequencies to half a cycle at the highest, a quarter at the corner, so mixed half and half with the dry sound the chain cuts a notch wherever it has turned the sound an odd number of half cycles, one for every two stages.
/// The feedback is solved within the sample, and the output is raised by it, so the notches stay where they are and as deep, and the sound between them rises.
/// Inputs: the sound, the corner in Hz, the feedback; it gives the wet sound only.
#[derive(Clone)]
struct Phaser {
    stages: usize,
    rate: f32,
    /// Each stage's last input less its coefficient times its last output.
    held: [f32; 8],
}

impl Phaser {
    fn new(stages: usize) -> Self {
        Phaser {
            stages,
            rate: DEFAULT_SR as f32,
            held: [0.0; 8],
        }
    }
}

impl AudioNode for Phaser {
    const ID: u64 = 0x6d78_007d;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.held = [0.0; 8];
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let turn =
            (std::f32::consts::PI * input[1].clamp(20.0, self.rate * 0.45) / self.rate).tan();
        let a = (turn - 1.0) / (turn + 1.0);
        let feedback = input[2].clamp(0.0, 0.95);
        // Each stage gives a * its input + what it holds, so the chain gives gain * its input + rest.
        let (mut gain, mut rest) = (1.0f32, 0.0f32);
        for held in &self.held[..self.stages] {
            gain *= a;
            rest = rest * a + held;
        }
        let out = (gain * input[0] + rest) / (1.0 - feedback * gain);
        let mut through = input[0] + feedback * out;
        for held in &mut self.held[..self.stages] {
            let passed = a * through + *held;
            *held = through - a * passed;
            through = passed;
        }
        [out * (1.0 + feedback)].into()
    }
}

fn wobble() -> Effect {
    wave_options(Effect::new("wobble"), 0.0, 0.5)
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
    worn(Effect::new("ping_pong"))
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
    Effect::new("pitch_shift")
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

/// The longest the pitch shift delays its sound, in seconds.
const SHIFT_DELAY: f64 = 0.04;
/// How far later than its nominal delay, in seconds, a grain of the pitch shift may start to continue the grain before.
const SHIFT_REACH: f64 = 0.005;
/// The frames before a read point the pitch shift compares, every other one, to match a grain to the one before.
const SHIFT_MATCH: usize = 256;

#[derive(Clone, Copy, Default)]
struct ShiftGrain {
    delay: f32,
    age: usize,
    length: usize,
}

/// Shifts the pitch of its input by reading a delay line at another speed in Hann-windowed grains, half overlapping, each started at the delay where what it has just read best matches the grain before, so the waveform runs on unbroken; the sound is delayed by at most `SHIFT_DELAY`.
/// Inputs: sound, pitch in semitones.
#[derive(Clone)]
struct PitchShift {
    buffer: Vec<f32>,
    at: usize,
    grains: [ShiftGrain; 3],
    until: usize,
    rate: f32,
}

impl PitchShift {
    fn new() -> Self {
        let mut shift = PitchShift {
            buffer: Vec::new(),
            at: 0,
            grains: [ShiftGrain::default(); 3],
            until: 0,
            rate: 0.0,
        };
        shift.set_sample_rate(DEFAULT_SR);
        shift
    }

    fn read(&self, frames: f32) -> f32 {
        let n = self.buffer.len() as isize;
        let whole = frames.floor();
        let fraction = frames - whole;
        let back =
            |k: isize| self.buffer[(self.at as isize - whole as isize - k).rem_euclid(n) as usize];
        back(0) + (back(1) - back(0)) * fraction
    }

    /// The delay from `nominal` to `SHIFT_REACH` later whose last frames best match those before `from`, the delay of the grain to continue; the earliest of equal matches.
    fn matched(&self, from: f32, nominal: f32) -> f32 {
        let reach = (SHIFT_REACH as f32 * self.rate) as usize;
        let score = |delay: f32| {
            let (mut both, mut energy) = (0.0f32, 1e-9f32);
            for i in (0..SHIFT_MATCH).step_by(2) {
                let b = self.read(delay + i as f32);
                both += self.read(from + i as f32) * b;
                energy += b * b;
            }
            both / energy.sqrt()
        };
        let start = nominal.floor() + from.fract();
        let mut best = (start, score(start));
        for k in 1..=reach {
            let delay = start + k as f32;
            let found = score(delay);
            if found > best.1 + 1e-6 {
                best = (delay, found);
            }
        }
        best.0
    }
}

impl AudioNode for PitchShift {
    const ID: u64 = 0x6d78_0075;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.buffer = vec![0.0; (SHIFT_DELAY * sample_rate) as usize + SHIFT_MATCH + 16];
        self.at = 0;
        self.grains = [ShiftGrain::default(); 3];
        self.until = 0;
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.grains = [ShiftGrain::default(); 3];
        self.until = 0;
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        self.at = (self.at + 1) % self.buffer.len();
        self.buffer[self.at] = input[0];
        let speed = 2f32.powf(input[1] / 12.0);
        if self.until == 0 {
            let most = SHIFT_DELAY as f32 * self.rate;
            let reach = SHIFT_REACH as f32 * self.rate;
            // A grain read faster than the sound arrives starts its delay high enough to end it at the shortest.
            let length = if speed > 1.0 {
                (most * 0.95).min((most - reach - 4.0) / (speed - 1.0))
            } else {
                most * 0.95
            }
            .max(32.0) as usize;
            let nominal = 2.0 + (speed - 1.0).max(0.0) * length as f32;
            let youngest = self
                .grains
                .iter()
                .filter(|g| g.age < g.length)
                .min_by_key(|g| g.age)
                .map(|g| g.delay);
            let delay = match youngest {
                Some(from) => self.matched(from, nominal),
                None => nominal,
            };
            if let Some(free) = self.grains.iter_mut().find(|g| g.age >= g.length) {
                *free = ShiftGrain {
                    delay,
                    age: 0,
                    length,
                };
            }
            self.until = length / 2;
        }
        self.until -= 1;
        let mut out = 0.0;
        for k in 0..self.grains.len() {
            let grain = self.grains[k];
            if grain.age >= grain.length {
                continue;
            }
            let phase = grain.age as f32 / grain.length as f32;
            let window = 0.5 - 0.5 * (TAU * phase).cos();
            out += window * self.read(grain.delay);
            let grain = &mut self.grains[k];
            grain.delay += 1.0 - speed;
            grain.age += 1;
        }
        [out].into()
    }
}

/// A delay of the input that feeds back on itself, by as much as dies away 60 dB over its decay; a negative decay feeds it back inverted.
/// Inputs: sound, delay in seconds, decay in seconds.
#[derive(Clone)]
struct Comb {
    most: f64,
    cubic: bool,
    buffer: Vec<f32>,
    at: usize,
    rate: f32,
}

impl Comb {
    fn new(most: f64, cubic: bool) -> Self {
        let mut comb = Comb {
            most,
            cubic,
            buffer: Vec::new(),
            at: 0,
            rate: DEFAULT_SR as f32,
        };
        comb.set_sample_rate(DEFAULT_SR);
        comb
    }

    fn read(&self, frames: f32) -> f32 {
        let n = self.buffer.len();
        let whole = frames.floor();
        let fraction = frames - whole;
        let back = |k: isize| {
            self.buffer[(self.at as isize - whole as isize - k).rem_euclid(n as isize) as usize]
        };
        if self.cubic {
            let (y0, y1, y2, y3) = (back(-1), back(0), back(1), back(2));
            let c1 = 0.5 * (y2 - y0);
            let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
            let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
            ((c3 * fraction + c2) * fraction + c1) * fraction + y1
        } else {
            back(0) + (back(1) - back(0)) * fraction
        }
    }
}

impl AudioNode for Comb {
    const ID: u64 = 0x6d78_0010;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.buffer = vec![0.0; (self.most * sample_rate).ceil() as usize + 4];
        self.at = 0;
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let limit = (self.buffer.len() - 3) as f32;
        let frames = (input[1] * self.rate).clamp(1.0, limit);
        self.at = (self.at + 1) % self.buffer.len();
        let delayed = self.read(frames);
        let decay = input[2];
        let feedback = if decay == 0.0 {
            0.0
        } else {
            0.001f32
                .powf(input[1].max(0.0) / decay.abs())
                .copysign(decay)
        };
        self.buffer[self.at] = input[0] + feedback * delayed;
        [delayed].into()
    }
}

/// How many taps the distortion's low-pass has, and how many frames its sound comes out late: the filter twice over, at a quarter of a frame a tap.
const DRIVE_TAPS: usize = 249;
const DRIVE_LATE: usize = (DRIVE_TAPS - 1) / 4;

/// The low-pass the distortion runs at four times the engine's rate, before and after its curve: a sinc under a Kaiser window, which passes up to 0.43 of the engine's rate and is 70 dB down from half of it, and delays every frequency alike.
fn drive_taps() -> &'static [f64] {
    static TAPS: std::sync::OnceLock<Vec<f64>> = std::sync::OnceLock::new();
    TAPS.get_or_init(|| {
        // The modified Bessel function of order 0, by its series.
        let bessel = |x: f64| {
            let (mut sum, mut term) = (1.0, 1.0);
            for k in 1..40 {
                term *= (x / 2.0 / k as f64).powi(2);
                sum += term;
            }
            sum
        };
        let (beta, edge, middle) = (6.76, 0.11625, (DRIVE_TAPS - 1) as f64 / 2.0);
        let taps: Vec<f64> = (0..DRIVE_TAPS)
            .map(|n| {
                let from = n as f64 - middle;
                let turn = std::f64::consts::TAU * edge * from;
                let sinc = if from == 0.0 { 1.0 } else { turn.sin() / turn };
                let window = bessel(beta * (1.0 - (from / middle).powi(2)).sqrt()) / bessel(beta);
                sinc * window
            })
            .collect();
        let sum: f64 = taps.iter().sum();
        taps.iter().map(|tap| tap / sum).collect()
    })
}

/// The distortion's curve worked out at four times the engine's rate, so the harmonics it adds above half the engine's rate are filtered out before they come back at other pitches.
/// Between two samples the curve is the mean of itself over the step from one to the other, from its antiderivative, after Parker, Zavalishin and Le Bivic, "Reducing the aliasing of nonlinear waveshaping using continuous-time convolution", 2016.
/// Its sound comes out `DRIVE_LATE` frames after its input.
/// Inputs: sound, distort from 0 to 1.
#[derive(Clone)]
struct Drive {
    hard: bool,
    /// The last sounds in, and the last out of the curve at four times the rate, each written twice over so a run of them is one slice.
    heard: Vec<f64>,
    bent: Vec<f64>,
    at: (usize, usize),
    before: f64,
}

impl Drive {
    const HEARD: usize = DRIVE_TAPS / 4 + 1;

    fn new(hard: bool) -> Self {
        Drive {
            hard,
            heard: vec![0.0; 2 * Self::HEARD],
            bent: vec![0.0; 2 * DRIVE_TAPS],
            at: (0, 0),
            before: 0.0,
        }
    }

    fn curve(&self, x: f64) -> f64 {
        if self.hard {
            x.clamp(-1.0, 1.0)
        } else {
            x.tanh()
        }
    }

    /// The curve's antiderivative.
    fn under(&self, x: f64) -> f64 {
        let size = x.abs();
        if self.hard {
            if size <= 1.0 { x * x / 2.0 } else { size - 0.5 }
        } else {
            size + (-2.0 * size).exp().ln_1p() - std::f64::consts::LN_2
        }
    }
}

impl AudioNode for Drive {
    const ID: u64 = 0x6d78_007f;
    type Inputs = U2;
    type Outputs = U1;

    fn reset(&mut self) {
        *self = Drive::new(self.hard);
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        let taps = drive_taps();
        let drive = 1.0 + f64::from(input[1].clamp(0.0, 1.0)) * 30.0;
        let level = if self.hard { 1.0 } else { drive.tanh() };
        let (heard, bent) = (Self::HEARD, DRIVE_TAPS);
        self.at.0 = (self.at.0 + heard - 1) % heard;
        self.heard[self.at.0] = f64::from(input[0]);
        self.heard[self.at.0 + heard] = f64::from(input[0]);
        let mut out = 0.0;
        for step in 0..4 {
            // The sound between two frames: every fourth tap from this step's, over the last sounds in, newest first.
            let recent = &self.heard[self.at.0..self.at.0 + heard];
            let between: f64 = taps[step..]
                .iter()
                .step_by(4)
                .zip(recent)
                .map(|(tap, sound)| tap * sound)
                .sum();
            let now = drive * 4.0 * between;
            let moved = now - self.before;
            let curved = if moved.abs() < 1e-6 {
                self.curve((now + self.before) / 2.0)
            } else {
                (self.under(now) - self.under(self.before)) / moved
            };
            self.before = now;
            self.at.1 = (self.at.1 + bent - 1) % bent;
            self.bent[self.at.1] = curved / level;
            self.bent[self.at.1 + bent] = curved / level;
            // The frame out is the one on this frame's own time, the first of its four.
            if step == 0 {
                let recent = &self.bent[self.at.1..self.at.1 + bent];
                out = taps
                    .iter()
                    .zip(recent)
                    .map(|(tap, sound)| tap * sound)
                    .sum();
            }
        }
        [out as f32].into()
    }
}

/// What each pass round an echo's loop does to the sound: a low-pass and a high-pass of one pole, and a curve that rounds it off.
/// A cut that is off, or a drive of 0, leaves the samples as they are.
#[derive(Clone, Default)]
struct Wear {
    below: f32,
    under: f32,
    cuts: (f32, f32),
    keeps: (f32, f32),
}

impl Wear {
    fn pass(&mut self, sound: f32, high_cut: f32, low_cut: f32, drive: f32, rate: f32) -> f32 {
        if (high_cut, low_cut) != self.cuts {
            self.cuts = (high_cut, low_cut);
            let keep = |note: f32| 1.0 - (-TAU * hz(note as f64) as f32 / rate).exp();
            self.keeps = (keep(high_cut).min(1.0), keep(low_cut).min(1.0));
        }
        let mut sound = sound;
        if high_cut < 136.0 {
            self.below += self.keeps.0 * (sound - self.below);
            sound = self.below;
        } else {
            self.below = sound;
        }
        if low_cut > 0.0 {
            self.under += self.keeps.1 * (sound - self.under);
            sound -= self.under;
        } else {
            self.under = 0.0;
        }
        if drive > 0.0 {
            let gain = 4.0 * drive.min(1.0);
            sound = (gain * sound).tanh() / gain;
        }
        sound
    }
}

/// The comb of the echo: a delay that feeds back on itself by as much as dies away 60 dB over its decay, each pass worn by its cuts and its drive, the first echo once.
/// Inputs: sound, delay in seconds, decay in seconds, high_cut and low_cut as MIDI notes, drive.
#[derive(Clone)]
struct Echo {
    comb: Comb,
    wear: Wear,
}

impl Echo {
    fn new(most: f64) -> Self {
        Echo {
            comb: Comb::new(most, false),
            wear: Wear::default(),
        }
    }
}

impl AudioNode for Echo {
    const ID: u64 = 0x6d78_007e;
    type Inputs = U6;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.comb.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.comb.reset();
        self.wear = Wear::default();
    }

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U1> {
        let comb = &mut self.comb;
        let limit = (comb.buffer.len() - 3) as f32;
        let frames = (input[1] * comb.rate).clamp(1.0, limit);
        comb.at = (comb.at + 1) % comb.buffer.len();
        let delayed = comb.read(frames);
        let delayed = self
            .wear
            .pass(delayed, input[3], input[4], input[5], comb.rate);
        let decay = input[2];
        let feedback = if decay == 0.0 {
            0.0
        } else {
            0.001f32
                .powf(input[1].max(0.0) / decay.abs())
                .copysign(decay)
        };
        comb.buffer[comb.at] = input[0] + feedback * delayed;
        [delayed].into()
    }
}

/// A room as a feedback delay network, after Jot and Chaigne, "Digital delay networks for designing artificial reverberators", 1991: sixteen delay lines whose lengths follow the room's size, each fed back through a gain that fades it 60 dB in the decay time and a low-pass that fades its highest frequencies sooner, by the damping.
/// Eight lines take the left side and eight the right; each eight is mixed among itself by a Hadamard matrix every time round, and a small rotation between the two eights lets a sound cross to the other side a little at a time, so a room starts on the side of its sound and spreads.
/// Three all-passes a side smear what comes in, and the longest lines are read at a slowly moving point, which keeps a long tail from ringing at fixed pitches.
/// Inputs: left, right, the decay in seconds, damp; it gives the wet sound only.
#[derive(Clone)]
struct Reverb {
    size: f32,
    smear: f32,
    moving: f32,
    rate: f32,
    lines: Vec<Vec<f32>>,
    at: [usize; REVERB_LINES],
    dulled: [f32; REVERB_LINES],
    gains: [f32; REVERB_LINES],
    smears: Vec<Vec<f32>>,
    smear_at: [usize; 6],
    /// The decay and damp the gains and the low-pass were last worked out for.
    set: (f32, f32),
    /// How much of its last value each line's low-pass keeps.
    keeps: [f32; REVERB_LINES],
    turns: [f32; 8],
}

const REVERB_LINES: usize = 16;
/// The length of each line as a share of the time sound takes to cross the room, left and right in turn.
const REVERB_LENGTHS: [f32; REVERB_LINES] = [
    0.350, 0.375, 0.403, 0.432, 0.463, 0.497, 0.533, 0.571, 0.613, 0.657, 0.705, 0.756, 0.811,
    0.870, 0.933, 1.000,
];
/// The length of each all-pass on the way in as a share of that time, three for the left and three for the right.
const REVERB_SMEARS: [f32; 6] = [0.079, 0.083, 0.121, 0.127, 0.167, 0.173];
/// The share of each side's sound that crosses to the other each time round, as the sine and cosine of the rotation.
const REVERB_CROSS: (f32, f32) = (0.3873, 0.9220);
const SPEED_OF_SOUND: f32 = 343.0;

impl Reverb {
    fn new(size: f32) -> Self {
        Reverb::shaped(size, 0.6, 6.0)
    }

    /// A room whose all-passes feed back by `smear` and whose long lines' read points move by `moving` frames either way.
    fn shaped(size: f32, smear: f32, moving: f32) -> Self {
        let mut reverb = Reverb {
            size,
            smear,
            moving,
            rate: DEFAULT_SR as f32,
            lines: Vec::new(),
            at: [0; REVERB_LINES],
            dulled: [0.0; REVERB_LINES],
            gains: [0.0; REVERB_LINES],
            smears: Vec::new(),
            smear_at: [0; 6],
            set: (f32::NAN, f32::NAN),
            keeps: [0.0; REVERB_LINES],
            turns: [0.0; 8],
        };
        reverb.set_sample_rate(DEFAULT_SR);
        reverb
    }

    /// The frames of a share of the room's crossing time, moved up to a prime so no two lines repeat together.
    fn frames(&self, share: f32) -> usize {
        let mut frames = (share * self.size / SPEED_OF_SOUND * self.rate)
            .round()
            .max(2.0) as usize;
        let prime = |n: usize| {
            (2..)
                .take_while(|d| d * d <= n)
                .all(|d| !n.is_multiple_of(d))
        };
        while !prime(frames) {
            frames += 1;
        }
        frames
    }
}

/// Mixes eight values by a Hadamard matrix that keeps their energy.
fn hadamard(values: &mut [f32; 8]) {
    for width in [1, 2, 4] {
        for start in (0..8).step_by(width * 2) {
            for i in start..start + width {
                let (a, b) = (values[i], values[i + width]);
                (values[i], values[i + width]) = (a + b, a - b);
            }
        }
    }
    for value in values.iter_mut() {
        *value *= 0.353_553_4;
    }
}

impl AudioNode for Reverb {
    const ID: u64 = 0x6d78_007c;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.lines = REVERB_LENGTHS
            .iter()
            .map(|share| vec![0.0; self.frames(*share)])
            .collect();
        self.smears = REVERB_SMEARS
            .iter()
            .map(|share| vec![0.0; self.frames(*share)])
            .collect();
        self.reset();
    }

    fn reset(&mut self) {
        self.lines.iter_mut().for_each(|line| line.fill(0.0));
        self.smears.iter_mut().for_each(|line| line.fill(0.0));
        (self.at, self.smear_at) = ([0; REVERB_LINES], [0; 6]);
        self.dulled = [0.0; REVERB_LINES];
        self.set = (f32::NAN, f32::NAN);
        self.turns = std::array::from_fn(|i| i as f32 / 8.0);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let (decay, damp) = (input[2].max(0.05), input[3].clamp(0.0, 1.0));
        if (decay, damp) != self.set {
            self.set = (decay, damp);
            // The highest frequencies fade in this share of the decay time: all of it with no damping, a twentieth with all.
            let share = (1.0 - damp).powf(1.5).max(0.05);
            for i in 0..REVERB_LINES {
                let gain = 0.001f32.powf(self.lines[i].len() as f32 / (self.rate * decay));
                self.gains[i] = gain;
                self.keeps[i] = 1.0 - 2.0 / (1.0 + gain.powf(1.0 - 1.0 / share));
            }
        }

        let mut sides = [input[0], input[1]];
        for (i, smear) in self.smears.iter_mut().enumerate() {
            let at = &mut self.smear_at[i];
            let held = smear[*at];
            let into = sides[i % 2] + held * self.smear;
            smear[*at] = into;
            sides[i % 2] = held - into * self.smear;
            *at = (*at + 1) % smear.len();
        }

        // The four longest lines of each side are read up to 12 frames short of their length, each moving at its own slow rate.
        let mut heard = [[0.0f32; 8]; 2];
        let mut out = [0.0f32; 2];
        for i in 0..REVERB_LINES {
            let line = &self.lines[i];
            let read = if i >= 8 {
                let turn = &mut self.turns[i - 8];
                *turn = (*turn + (0.31 + 0.17 * (i - 8) as f32) / self.rate).fract();
                let short = self.moving * (1.0 + (std::f32::consts::TAU * *turn).sin());
                let (whole, part) = (short as usize, short.fract());
                let first = line[(self.at[i] + whole) % line.len()];
                let second = line[(self.at[i] + whole + 1) % line.len()];
                first + (second - first) * part
            } else {
                line[self.at[i]]
            };
            let lowered = read * self.gains[i];
            self.dulled[i] = lowered + (self.dulled[i] - lowered) * self.keeps[i];
            heard[i % 2][i / 2] = self.dulled[i];
            let sign = if (i / 2).is_multiple_of(2) { 1.0 } else { -1.0 };
            out[i % 2] += self.dulled[i] * sign;
        }

        hadamard(&mut heard[0]);
        hadamard(&mut heard[1]);
        let (cross, keep) = REVERB_CROSS;
        for i in 0..REVERB_LINES {
            let (own, other) = (heard[i % 2][i / 2], heard[1 - i % 2][i / 2]);
            let turned = if i % 2 == 0 {
                own * keep + other * cross
            } else {
                own * keep - other * cross
            };
            let at = self.at[i];
            self.lines[i][at] = turned + sides[i % 2];
            self.at[i] = (at + 1) % self.lines[i].len();
        }
        [out[0] * 0.2, out[1] * 0.2].into()
    }
}

/// A control wave from 0 to 1: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic, starting `offset` of a cycle in.
/// Inputs: cycles a second, the pulse's width, the shape, and 1 to flip it.
#[derive(Clone)]
pub(crate) struct Lfo {
    offset: f32,
    phase: f32,
    level: f32,
    pulse_only: bool,
    rate: f32,
}

impl Lfo {
    pub(crate) fn new(offset: f32, level: f32, pulse_only: bool) -> Self {
        Lfo {
            offset,
            phase: offset.rem_euclid(1.0),
            level,
            pulse_only,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Lfo {
    const ID: u64 = 0x6d78_0012;
    type Inputs = U4;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.phase = self.offset.rem_euclid(1.0);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U1> {
        let level = self.at(self.phase, input);
        self.phase = (self.phase + input[0].max(0.0) / self.rate).fract();
        [level].into()
    }
}

impl Lfo {
    /// Its level at a place in its cycle.
    fn at(&self, p: f32, input: &Frame<f32, U4>) -> f32 {
        let shape = if self.pulse_only {
            1
        } else {
            input[2].clamp(0.0, 4.0).round() as u32
        };
        let wave = match shape {
            0 => 2.0 * p - 1.0,
            1 => {
                if p < input[1] {
                    1.0
                } else {
                    -1.0
                }
            }
            2 => 4.0 * ((p - 0.25).rem_euclid(1.0) - 0.5).abs() - 1.0,
            3 => (TAU * p).sin(),
            _ => {
                // Two cubic halves shaped like a sine.
                let half = (p * 2.0).fract();
                let bump = 4.0 * half * (1.0 - half);
                let smooth = bump * (1.5 - 0.5 * bump);
                if p < 0.5 { smooth } else { -smooth }
            }
        };
        let level = (wave + 1.0) / 2.0;
        let level = if input[3] >= 0.5 { 1.0 - level } else { level };
        level * self.level
    }
}

/// An [`Lfo`] whose cycle moves on by as much as its last input, where in the cycle it starts, changes.
/// Inputs: the Lfo's four, then the offset from 0 to 1.
#[derive(Clone)]
struct Shifted {
    lfo: Lfo,
}

impl Shifted {
    fn new(offset: f32, pulse_only: bool) -> Self {
        Shifted {
            lfo: Lfo::new(offset, 1.0, pulse_only),
        }
    }
}

impl AudioNode for Shifted {
    const ID: u64 = 0x6d78_0077;
    type Inputs = U5;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.lfo.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.lfo.reset();
    }

    fn tick(&mut self, input: &Frame<f32, U5>) -> Frame<f32, U1> {
        if input[4] != self.lfo.offset {
            self.lfo.phase = (self.lfo.phase + input[4] - self.lfo.offset).rem_euclid(1.0);
            self.lfo.offset = input[4];
        }
        self.lfo
            .tick(&[input[0], input[1], input[2], input[3]].into())
    }
}

/// A [`Shifted`] read for each side: the left where its cycle is, the right the share its last input gives of half a cycle behind.
/// Inputs: the Shifted's five, then the share from 0 to 1.
#[derive(Clone)]
struct Sides {
    shifted: Shifted,
}

impl Sides {
    fn new(offset: f32, pulse_only: bool) -> Self {
        Sides {
            shifted: Shifted::new(offset, pulse_only),
        }
    }
}

impl AudioNode for Sides {
    const ID: u64 = 0x6d78_0081;
    type Inputs = U6;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.shifted.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.shifted.reset();
    }

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U2> {
        let lfo: Frame<f32, U4> = [input[0], input[1], input[2], input[3]].into();
        let left = self
            .shifted
            .tick(&[input[0], input[1], input[2], input[3], input[4]].into())[0];
        if input[5] <= 0.0 {
            return [left, left].into();
        }
        // The Shifted has moved on a frame, so the place the left was read at is a frame back.
        let step = input[0].max(0.0) / self.shifted.lfo.rate;
        let behind = step + 0.5 * input[5].min(1.0);
        let place = (self.shifted.lfo.phase - behind).rem_euclid(1.0);
        [left, self.shifted.lfo.at(place, &lfo)].into()
    }
}

/// Moves to its input at no more than 1 every 10 ms, starting at its first input.
#[derive(Clone)]
struct Ease {
    level: Option<f32>,
    step: f32,
}

impl Default for Ease {
    fn default() -> Self {
        Ease {
            level: None,
            step: (100.0 / DEFAULT_SR) as f32,
        }
    }
}

impl AudioNode for Ease {
    const ID: u64 = 0x6d78_0078;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.step = (100.0 / sample_rate) as f32;
    }

    fn reset(&mut self) {
        self.level = None;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let level = match self.level {
            Some(level) if (input[0] - level).abs() > self.step => {
                level + self.step.copysign(input[0] - level)
            }
            _ => input[0],
        };
        self.level = Some(level);
        [level].into()
    }
}

/// Turns both sides down alike just before a peak on either would pass the ceiling, and lets them back up afterwards; its sound comes out `LIMITER_LOOK_AHEAD` late.
/// The gain each frame may have is the ceiling over its level; the lowest of those over the look-ahead, let back up over the relax time, is averaged over the look-ahead, so the gain glides down to each peak and no sample passes the ceiling.
/// Inputs: left, right, the ceiling as a level, the relax time in milliseconds.
#[derive(Clone)]
struct Limiter {
    ahead: usize,
    sound: Vec<[f32; 2]>,
    /// The frames of the look-ahead whose gain is lower than every later one, oldest first, as (frame, gain).
    lowest: std::collections::VecDeque<(u64, f32)>,
    let_up: f32,
    gains: Vec<f32>,
    sum: f64,
    at: usize,
    frame: u64,
    rate: f64,
}

impl Limiter {
    fn new() -> Self {
        let mut limiter = Limiter {
            ahead: 0,
            sound: Vec::new(),
            lowest: std::collections::VecDeque::new(),
            let_up: 1.0,
            gains: Vec::new(),
            sum: 0.0,
            at: 0,
            frame: 0,
            rate: DEFAULT_SR,
        };
        limiter.set_sample_rate(DEFAULT_SR);
        limiter
    }
}

impl AudioNode for Limiter {
    const ID: u64 = 0x6d78_007a;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.ahead = (LIMITER_LOOK_AHEAD * sample_rate).round().max(1.0) as usize;
        self.sound = vec![[0.0; 2]; self.ahead];
        self.gains = vec![1.0; self.ahead];
        self.lowest = std::collections::VecDeque::with_capacity(self.ahead + 2);
        self.reset();
    }

    fn reset(&mut self) {
        self.sound.fill([0.0; 2]);
        self.gains.fill(1.0);
        self.sum = self.ahead as f64;
        self.lowest.clear();
        self.let_up = 1.0;
        self.at = 0;
        self.frame = 0;
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let level = input[0].abs().max(input[1].abs());
        let ceiling = input[2].max(1e-6);
        let allowed = if level > ceiling {
            ceiling / level
        } else {
            1.0
        };
        while self.lowest.back().is_some_and(|(_, gain)| *gain >= allowed) {
            self.lowest.pop_back();
        }
        self.lowest.push_back((self.frame, allowed));
        while self
            .lowest
            .front()
            .is_some_and(|(frame, _)| frame + (self.ahead as u64) < self.frame)
        {
            self.lowest.pop_front();
        }
        let lowest = self.lowest.front().map_or(1.0, |(_, gain)| *gain);
        let frames = (f64::from(input[3]).max(1.0) / 1000.0 * self.rate) as f32;
        let risen = self.let_up + (1.0 - self.let_up) * (6.9 / frames).min(1.0);
        self.let_up = lowest.min(risen);
        self.sum += f64::from(self.let_up) - f64::from(self.gains[self.at]);
        self.gains[self.at] = self.let_up;
        let gain = (self.sum / self.ahead as f64) as f32;
        let delayed = std::mem::replace(&mut self.sound[self.at], [input[0], input[1]]);
        self.at = (self.at + 1) % self.ahead;
        self.frame += 1;
        [delayed[0] * gain, delayed[1] * gain].into()
    }
}

/// Glides to its first input, falling 60 dB short of each step over its second, in milliseconds.
/// Inputs: level, milliseconds.
#[derive(Clone)]
struct Smooth {
    ms: f32,
    keep: f32,
    level: Option<f32>,
    rate: f64,
}

impl Smooth {
    fn new() -> Self {
        Smooth {
            ms: 0.0,
            keep: 0.0,
            level: None,
            rate: DEFAULT_SR,
        }
    }
}

impl AudioNode for Smooth {
    const ID: u64 = 0x6d78_0079;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate;
        self.ms = 0.0;
        self.keep = 0.0;
    }

    fn reset(&mut self) {
        self.level = None;
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        if input[1] != self.ms {
            self.ms = input[1];
            let seconds = self.ms as f64 / 1000.0;
            self.keep = if seconds > 0.0 {
                0.001f64.powf(1.0 / (seconds * self.rate)) as f32
            } else {
                0.0
            };
        }
        let level = match self.level {
            Some(level) => input[0] + (level - input[0]) * self.keep,
            None => input[0],
        };
        self.level = Some(level);
        [level].into()
    }
}

/// Glides to its input, falling 60 dB short of each step over `seconds`.
#[derive(Clone)]
pub(crate) struct Lag {
    seconds: f64,
    keep: f32,
    level: Option<f32>,
}

impl Lag {
    pub(crate) fn new(seconds: f64) -> Self {
        let mut lag = Lag {
            seconds,
            keep: 0.0,
            level: None,
        };
        lag.set_sample_rate(DEFAULT_SR);
        lag
    }
}

impl AudioNode for Lag {
    const ID: u64 = 0x6d78_0013;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.keep = if self.seconds > 0.0 {
            0.001f64.powf(1.0 / (self.seconds * sample_rate)) as f32
        } else {
            0.0
        };
    }

    fn reset(&mut self) {
        self.level = None;
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let level = match self.level {
            Some(level) => input[0] + (level - input[0]) * self.keep,
            None => input[0],
        };
        self.level = Some(level);
        [level].into()
    }
}

/// Follows the level of the louder side, or of the louder side of its key while it has one, and above the threshold turns both sides down alike, so the level comes out 1 dB higher for each ratio dB it rises.
/// The level is the last peak let go over the relax time, which the level then rises to over the clamp time and falls to over the relax time again, so it holds still through the cycle of a low note.
/// Inputs: left, right, threshold in dB, ratio, makeup in dB, clamp time, relax time, the key's left and right, and whether it has a key.
#[derive(Clone)]
struct Compressor {
    peak: f32,
    level: f32,
    rate: f32,
}

impl Compressor {
    fn new() -> Self {
        Compressor {
            peak: 0.0,
            level: 0.0,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Compressor {
    const ID: u64 = 0x6d78_0014;
    type Inputs = U10;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        (self.peak, self.level) = (0.0, 0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U10>) -> Frame<f32, U2> {
        let keep = |seconds: f32| {
            if seconds > 0.0 {
                0.001f32.powf(1.0 / (seconds * self.rate))
            } else {
                0.0
            }
        };
        let (clamp, relax) = (keep(input[5]), keep(input[6]));
        let size = if input[9] > 0.5 {
            input[7].abs().max(input[8].abs())
        } else {
            input[0].abs().max(input[1].abs())
        };
        self.peak = size.max(size + (self.peak - size) * relax);
        let towards = if self.peak > self.level { clamp } else { relax };
        self.level = self.peak + (self.level - self.peak) * towards;
        let over = 20.0 * self.level.max(1e-9).log10() - input[2];
        let lowered = if over > 0.0 {
            over * (1.0 / input[3].max(1.0) - 1.0)
        } else {
            0.0
        };
        let gain = db_amp(lowered + input[4]);
        [input[0] * gain, input[1] * gain].into()
    }
}

/// Airwindows' DeRez, as mx's design describes it: each sample held at the sample rate, its step softened by blending in the sample held before, then rounded away from zero to a step of bits.
/// Inputs: sound, sample rate in Hz, bits.
#[derive(Clone)]
struct Crusher {
    since: f32,
    held: f32,
    before: f32,
    rate: f32,
}

impl Crusher {
    fn new() -> Self {
        Crusher {
            since: f32::INFINITY,
            held: 0.0,
            before: 0.0,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Crusher {
    const ID: u64 = 0x6d78_0015;
    type Inputs = U3;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        (self.since, self.held, self.before) = (f32::INFINITY, 0.0, 0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U1> {
        let rate = input[1].clamp(1.0, self.rate);
        let period = self.rate / rate;
        if self.since >= period {
            self.before = self.held;
            self.held = input[0];
            self.since -= period;
            if self.since >= period {
                self.since = 0.0;
            }
        }
        self.since += 1.0;
        let soften = (1.0 + rate / self.rate) / 2.0;
        let softened = self.held * soften + self.before * (1.0 - soften);
        let step = 2.0 / 2f32.powf(input[2].clamp(1.0, 24.0));
        [softened.signum() * (softened.abs() / step).ceil() * step].into()
    }
}

/// Echoes a phase apart, alternating between one side and the other, each `feedback` times the one before and worn by the cuts and the drive once more.
/// Inputs: left, right, phase in seconds, feedback, the side of the first echo from -1 to 1, high_cut and low_cut as MIDI notes, drive.
#[derive(Clone)]
struct PingPong {
    most: f64,
    first: Vec<f32>,
    second: Vec<f32>,
    at: usize,
    rate: f32,
    wear: (Wear, Wear),
}

impl PingPong {
    fn new(most: f64) -> Self {
        let mut ping = PingPong {
            most,
            first: Vec::new(),
            second: Vec::new(),
            at: 0,
            rate: DEFAULT_SR as f32,
            wear: (Wear::default(), Wear::default()),
        };
        ping.set_sample_rate(DEFAULT_SR);
        ping
    }

    fn read(buffer: &[f32], at: usize, frames: f32) -> f32 {
        let n = buffer.len();
        let whole = frames.floor() as usize;
        let fraction = frames - frames.floor();
        let a = buffer[(at + n - whole) % n];
        let b = buffer[(at + n - whole - 1) % n];
        a + (b - a) * fraction
    }
}

impl AudioNode for PingPong {
    const ID: u64 = 0x6d78_0016;
    type Inputs = U8;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        let n = (self.most * sample_rate).ceil() as usize + 4;
        self.first = vec![0.0; n];
        self.second = vec![0.0; n];
        self.at = 0;
    }

    fn reset(&mut self) {
        self.first.fill(0.0);
        self.second.fill(0.0);
        self.wear = (Wear::default(), Wear::default());
    }

    fn tick(&mut self, input: &Frame<f32, U8>) -> Frame<f32, U2> {
        let limit = (self.first.len() - 3) as f32;
        let frames = (input[2] * self.rate).clamp(1.0, limit);
        let feedback = input[3].clamp(0.0, 0.999);
        self.at = (self.at + 1) % self.first.len();
        let (high_cut, low_cut, drive) = (input[5], input[6], input[7]);
        let first = Self::read(&self.first, self.at, frames);
        let first = self.wear.0.pass(first, high_cut, low_cut, drive, self.rate) * feedback;
        let second = Self::read(&self.second, self.at, frames);
        let second = self
            .wear
            .1
            .pass(second, high_cut, low_cut, drive, self.rate)
            * feedback;
        self.first[self.at] = input[0] + input[1] + second;
        self.second[self.at] = first;
        let place = |side: f32| {
            let angle = (side + 1.0) * FRAC_PI_4;
            (angle.cos(), angle.sin())
        };
        let (fl, fr) = place(input[4]);
        let (sl, sr) = place(-input[4]);
        [
            input[0] + first * fl + second * sl,
            input[1] + first * fr + second * sr,
        ]
        .into()
    }
}
