//! mx's cello.bowed, after its design in mx's priv/synthdefs/mx-instruments.scd, on a bowed waveguide of its own instead of sc3-plugins' DWGBowed.

use crate::nodes::{db, hz, pink_noise, shape};
use crate::{Note, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;

/// Where the bow meets the string, as a fraction of its length from the bridge.
const POSITION: f64 = 0.2;

/// The centres of the bands the cello is measured in, from 80 Hz to 4 kHz.
const BANDS: [f64; 8] = [113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0];

/// The gain in dB at each band, fitted to the University of Iowa's cello at pp, mf and ff.
const GAINS: [[f64; 8]; 3] = [
    [18.5, 16.8, 9.3, 8.1, 24.4, 3.6, 20.8, 18.7],
    [20.0, 15.2, 9.6, 6.5, 26.3, 9.3, 27.8, 19.2],
    [7.0, 7.4, 1.6, 3.0, 17.3, 4.6, 22.4, 14.4],
];

/// The whole sound's level in dB at pp, mf and ff: as loud as today's cello at mf, and 14.8 dB louder at ff than at pp, as the recordings are.
const LEVELS: [f64; 3] = [-47.45, -47.2, -31.65];

const EQ: [&str; 8] = ["eq0", "eq1", "eq2", "eq3", "eq4", "eq5", "eq6", "eq7"];

pub fn cello() -> Synth {
    let mut synth = Synth::new("cello.bowed")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the string is bowed."))
        .option(Spec::float("vibrato").default(0.25).range(0.0, 2.0).doc("The vibrato's depth in semitones, reached as the note holds."))
        .options(Spec::envelope(1.0, 0.3));
    for (name, hz) in EQ.iter().zip(BANDS) {
        let doc = format!("The gain in dB of the band at {hz} Hz, in place of the one fitted for the note's vel; for fitting.");
        synth = synth.option(Spec::db(name).range(-60.0, 60.0).doc(&doc));
    }
    synth.build(|options, ctx| {
        let freq = hz(options.number("note"));
        let vel = options.number("vel");
        let sustain = options.number("sustain");
        let by_vel = |pp: f64, mf: f64, ff: f64| {
            pp + (mf - pp) * ((vel - 0.3) / 0.3).clamp(0.0, 1.0) + (ff - mf) * ((vel - 0.6) / 0.4).clamp(0.0, 1.0)
        };

        let mut sound = Net::wrap(Box::new(string(freq, vel)))
            >> highpass_hz(65.4 * 0.75, 0.541)
            >> highpass_hz(65.4 * 0.75, 1.307);
        for (i, hz) in BANDS.into_iter().enumerate() {
            let gain = options.given(EQ[i]).unwrap_or_else(|| by_vel(GAINS[0][i], GAINS[1][i], GAINS[2][i]));
            sound = sound >> bell_hz(hz as f32, 1.4, db(gain) as f32);
        }
        // A delay that swings after the string gives the vibrato, which grows in after 0.3 seconds.
        let rate_hz = 5.9;
        let depth = options.number("vibrato") * 2f64.ln() / 12.0 / (2.0 * PI * rate_hz);
        let swing = shape(move |t| {
            let grown = ((t - 0.3).clamp(0.0, 1.0)).powi(2);
            0.002 + (2.0 * PI * rate_hz * t + PI / 2.0).sin() * depth * grown
        });
        let cutoff = (2000.0 + vel * vel * 7000.0) as f32;
        sound = (sound | swing) >> tap(0.0, 0.01) >> lowpass_hz(cutoff, 0.541) >> lowpass_hz(cutoff, 1.307);
        let hiss = (pink_noise(ctx.seed()) >> highpass_hz((1800.0 - vel * 400.0) as f32, 0.707))
            * db(by_vel(-35.5, -35.5, -30.5)) as f32;
        sound = (sound + hiss) >> dcblock::<f32>();

        // The string swells in, slower the softer it is bowed, though never over more than part of its sustain.
        let swell = ((0.95 - 0.3 * vel).min(sustain * 0.4)).max(1e-3);
        let rising = shape(move |t| 1.0 - (1.0 - (t / swell).min(1.0)).powi(2));
        let level = (vel.powf(0.6) * db(by_vel(LEVELS[0], LEVELS[1], LEVELS[2]))) as f32;
        Note::envelope(sound * rising * level, options)
    })
}

/// The bowed string: a waveguide whose two halves meet at the bow, where friction holds the string or lets it slip.
/// Its delays are worked out at 48 kHz, the rate mx plays at.
fn string(freq: f64, vel: f64) -> An<impl AudioNode<Inputs = U0, Outputs = U1> + use<>> {
    let rate = crate::RATE;
    let period = rate / freq;
    let speed = 0.2 + vel * 0.6;
    // The harder the bow presses, the wider the speeds at which it holds the string.
    let width = 0.15 * (1.0 + vel * 0.6);
    // Each end of the string loses a little, the bridge more, and the highs most.
    let (neck_cut, neck_loss) = (6000.0, 0.995f32);
    let (bridge_cut, bridge_loss) = (3000.0, 0.97f32);

    // Each half's round trip takes its share of the period, less the feedback's sample and its filter's delay at the note.
    let lag = |cut: f64| {
        let mut filter = lowpole_hz::<f64>(cut);
        filter.set_sample_rate(rate);
        let phase = filter.response(0, freq).map_or(0.0, |r| -r.arg());
        phase / (2.0 * PI * freq / rate)
    };
    let neck_delay = ((1.0 - POSITION) * period - 1.0 - lag(neck_cut)) / rate;
    let bridge_delay = (POSITION * period - 1.0 - lag(bridge_cut)) / rate;

    let bow = envelope(move |t: f64| speed * (0.05 + 0.95 * (t / 0.15).min(1.0)));
    // The string sticks to the bow while their speeds are close, and slips once they part.
    let junction = map(move |i: &Frame<f32, U3>| {
        let (bow, from_neck, from_bridge) = (i[0], i[1], i[2]);
        let slip = bow - (from_neck + from_bridge);
        let held = slip / (1.0 + (slip / width as f32).powi(4));
        (from_bridge + held, from_neck + held)
    });
    let neck = (pass() | dc(neck_delay as f32)) >> tap(0.0, 0.1) >> (lowpole_hz(neck_cut as f32) * -neck_loss);
    let bridge = (pass() | dc(bridge_delay as f32)) >> tap(0.0, 0.1) >> (lowpole_hz(bridge_cut as f32) * -bridge_loss);
    let waveguide = feedback((bow | multipass::<U2>()) >> junction >> (neck | bridge));
    dc((0.0, 0.0)) >> waveguide >> (sink() | pass())
}
