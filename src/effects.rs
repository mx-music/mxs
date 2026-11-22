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
//!     let reverb = ("reverb".to_string(), vec![("room".to_string(), Value::Number(0.6))]);
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
        reverb(),
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
            Spec::float("room")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How big the room is."),
        )
        .option(
            Spec::float("damp")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How much the room dulls the high frequencies."),
        )
        .tail_from(|o| (o.number("room") * 10.0 + 1.0).min(11.0))
        .build(|o, _| {
            (net(multipass::<U2>()) | o.follow("room") | o.follow("damp"))
                >> net(An(Freeverb::new()))
        })
}

fn echo() -> Effect {
    Effect::new("echo")
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
                let comb = (net(pass()) | o.follow("phase") | o.follow("decay"))
                    >> net(An(Comb::new(most, false)));
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
        .tail(0.0)
        .build(|o, _| {
            per_channel(|| {
                (net(pass()) | o.follow("distort"))
                    >> net(map(|x: &Frame<f32, U2>| {
                        let drive = 1.0 + x[1].clamp(0.0, 1.0) * 30.0;
                        (x[0] * drive).tanh() / drive.tanh()
                    }))
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
        .tail(0.0)
        .build(|o, _| {
            let open = (rate_of(o, "phase")
                | o.follow("pulse_width")
                | net(dc(1.0))
                | net(dc(0.0))
                | eased(o, "phase_offset"))
                >> net(An(Shifted::new(o.number("phase_offset") as f32, true)));
            let low = o.follow("amp_min");
            let level = (open | low | o.follow("amp_max"))
                >> net(map(|x: &Frame<f32, U3>| x[1] + x[0] * (x[2] - x[1])));
            let level = (level | o.follow("smooth_ms")) >> net(An(Smooth::new()));
            (net(multipass::<U2>()) | level)
                >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U2> {
                    [x[0] * x[2], x[1] * x[2]].into()
                }))
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
                .default(0.2)
                .range(0.0001, f64::INFINITY)
                .doc("The level above which it lowers the sound."),
        )
        .option(
            Spec::float("slope_below")
                .default(1.0)
                .doc("How much the level changes below the threshold."),
        )
        .option(
            Spec::float("slope_above")
                .default(0.5)
                .doc("How much the level changes above the threshold."),
        )
        .option(
            Spec::ms("clamp_ms")
                .default(10.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds it takes to lower the sound."),
        )
        .option(
            Spec::ms("relax_ms")
                .default(10.0)
                .range(0.0, f64::INFINITY)
                .doc("How many milliseconds it takes to let go."),
        )
        .tail(0.1)
        .build(|o, _| {
            let controls = o.follow("threshold")
                | o.follow("slope_below")
                | o.follow("slope_above")
                | (o.follow("clamp_ms") * 0.001)
                | (o.follow("relax_ms") * 0.001);
            (net(multipass::<U2>()) | controls) >> net(An(Compander::new()))
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

fn tremolo() -> Effect {
    wave_options(Effect::new("tremolo"), 2.0, 4.0)
        .option(
            Spec::float("depth")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How far it turns the sound down."),
        )
        .tail(0.0)
        .build(|o, _| {
            let level = (sweep(o, net(dc(0.5))) | o.follow("depth"))
                >> net(map(|x: &Frame<f32, U2>| 1.0 - x[1] * x[0]));
            (net(multipass::<U2>()) | level)
                >> net(map(|x: &Frame<f32, U3>| -> Frame<f32, U2> {
                    [x[0] * x[2], x[1] * x[2]].into()
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
    Effect::new("ping_pong")
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
                | eased(o, "pan_start"))
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

/// Jezar's Freeverb, in the public domain: eight damped combs and four all-passes per channel, the right channel's tuned 23 samples longer.
/// Inputs: left, right, room, damp; it gives the wet sound only.
#[derive(Clone)]
struct Freeverb {
    combs: [Vec<Vec<f32>>; 2],
    comb_at: [[usize; 8]; 2],
    stores: [[f32; 8]; 2],
    allpasses: [Vec<Vec<f32>>; 2],
    allpass_at: [[usize; 4]; 2],
}

const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASSES: [usize; 4] = [556, 441, 341, 225];
const SPREAD: usize = 23;

impl Freeverb {
    fn new() -> Self {
        let mut verb = Freeverb {
            combs: [Vec::new(), Vec::new()],
            comb_at: [[0; 8]; 2],
            stores: [[0.0; 8]; 2],
            allpasses: [Vec::new(), Vec::new()],
            allpass_at: [[0; 4]; 2],
        };
        verb.set_sample_rate(DEFAULT_SR);
        verb
    }
}

impl AudioNode for Freeverb {
    const ID: u64 = 0x6d78_0011;
    type Inputs = U4;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let scale = sample_rate / 44_100.0;
        let length = |n: usize| Ord::max((n as f64 * scale).round() as usize, 1);
        for side in 0..2 {
            self.combs[side] = COMBS
                .iter()
                .map(|&n| vec![0.0; length(n + side * SPREAD)])
                .collect();
            self.allpasses[side] = ALLPASSES
                .iter()
                .map(|&n| vec![0.0; length(n + side * SPREAD)])
                .collect();
        }
        self.comb_at = [[0; 8]; 2];
        self.allpass_at = [[0; 4]; 2];
        self.stores = [[0.0; 8]; 2];
    }

    fn reset(&mut self) {
        self.set_sample_rate(DEFAULT_SR);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let feedback = input[2].clamp(0.0, 1.0) * 0.28 + 0.7;
        let damp = input[3].clamp(0.0, 1.0) * 0.4;
        let sound = (input[0] + input[1]) * 0.015;
        let mut out = [0.0f32; 2];
        for (side, out) in out.iter_mut().enumerate() {
            let mut sum = 0.0;
            for k in 0..8 {
                let buffer = &mut self.combs[side][k];
                let at = self.comb_at[side][k];
                let y = buffer[at];
                self.stores[side][k] = y * (1.0 - damp) + self.stores[side][k] * damp;
                buffer[at] = sound + self.stores[side][k] * feedback;
                self.comb_at[side][k] = (at + 1) % buffer.len();
                sum += y;
            }
            for k in 0..4 {
                let buffer = &mut self.allpasses[side][k];
                let at = self.allpass_at[side][k];
                let held = buffer[at];
                buffer[at] = sum + held * 0.5;
                sum = held - sum;
                self.allpass_at[side][k] = (at + 1) % buffer.len();
            }
            *out = sum;
        }
        out.into()
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
        let p = self.phase;
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
        self.phase = (self.phase + input[0].max(0.0) / self.rate).fract();
        [level * self.level].into()
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

/// Follows the level of each channel and turns it down above the threshold, or up below it, by the slopes.
/// Inputs: left, right, threshold, slope below, slope above, clamp time, relax time.
#[derive(Clone)]
struct Compander {
    level: [f32; 2],
    rate: f32,
}

impl Compander {
    fn new() -> Self {
        Compander {
            level: [0.0; 2],
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Compander {
    const ID: u64 = 0x6d78_0014;
    type Inputs = U7;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.level = [0.0; 2];
    }

    fn tick(&mut self, input: &Frame<f32, U7>) -> Frame<f32, U2> {
        let threshold = input[2].max(0.0001);
        let keep = |seconds: f32| {
            if seconds > 0.0 {
                0.001f32.powf(1.0 / (seconds * self.rate))
            } else {
                0.0
            }
        };
        let (clamp, relax) = (keep(input[5]), keep(input[6]));
        let mut out = [0.0f32; 2];
        for side in 0..2 {
            let x = input[side];
            let size = x.abs();
            let keep = if size > self.level[side] {
                clamp
            } else {
                relax
            };
            self.level[side] = size + (self.level[side] - size) * keep;
            let level = self.level[side];
            let gain = if level > threshold {
                (level / threshold).powf(input[4] - 1.0)
            } else if level > 0.0 {
                (level / threshold).powf(input[3] - 1.0)
            } else {
                1.0
            };
            out[side] = x * gain.min(1000.0);
        }
        out.into()
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

/// Echoes a phase apart, alternating between one side and the other, each `feedback` times the one before.
/// Inputs: left, right, phase in seconds, feedback, the side of the first echo from -1 to 1.
#[derive(Clone)]
struct PingPong {
    most: f64,
    first: Vec<f32>,
    second: Vec<f32>,
    at: usize,
    rate: f32,
}

impl PingPong {
    fn new(most: f64) -> Self {
        let mut ping = PingPong {
            most,
            first: Vec::new(),
            second: Vec::new(),
            at: 0,
            rate: DEFAULT_SR as f32,
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
    type Inputs = U5;
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
    }

    fn tick(&mut self, input: &Frame<f32, U5>) -> Frame<f32, U2> {
        let limit = (self.first.len() - 3) as f32;
        let frames = (input[2] * self.rate).clamp(1.0, limit);
        let feedback = input[3].clamp(0.0, 0.999);
        self.at = (self.at + 1) % self.first.len();
        let first = Self::read(&self.first, self.at, frames) * feedback;
        let second = Self::read(&self.second, self.at, frames) * feedback;
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
