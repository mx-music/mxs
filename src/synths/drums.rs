//! mx's drums and hand percussion, after their designs in mx's priv/synthdefs/mx-drums.scd.
//! Their seeded modes are drawn from mxs's own generator, so they sit at other frequencies than scsynth's draws, with the same levels and rings at each band.

use crate::nodes::{Draws, between, db, decay2, dust, exp_segments, hit, hz, modes, segments, shape, struck_gliding, white_noise};
use super::instruments::{partials, spread_phases};
use crate::{Context, Note, Options, Spec, Synth};
use fundsp::prelude::*;

pub fn all() -> Vec<Synth> {
    let mut drums = vec![
        kick(),
        snare(),
        clap(),
        crash(),
        hat("closed_hihat", &CLOSED_LEVELS, &CLOSED_RINGS, 0.8, false),
        hat("open_hihat", &OPEN_LEVELS, &OPEN_RINGS, 5.0, true),
        tom("low_tom", 41.85, &LOW_TOM),
        tom("mid_tom", 43.4, &HIGH_TOM),
        tom("high_tom", 45.0, &HIGH_TOM),
        rimshot(),
        cowbell(),
        clave(),
        agogo("low_agogo", &LOW_AGOGO),
        agogo("high_agogo", &HIGH_AGOGO),
        shake("ganza", &GANZA),
        shake("shaker", &SHAKER),
        conga("low_conga", &LOW_CONGA),
        conga("mid_conga", &MID_CONGA),
        conga("high_conga", &HIGH_CONGA),
        timbale("low_timbale", 62.0),
        timbale("high_timbale", 67.0),
        timbale_shell(),
        pandeiro("pandeiro", None),
        guiro(),
        triangle(),
        tambourine("tambourine", false),
        tambourine("tambourine_shake", true),
        cabasa(),
        cajon(),
        bongo("low_bongo", &LOW_BONGO),
        surdo(),
        cuica(),
        tamborim(),
        caxixi(),
        bongo("high_bongo", &HIGH_BONGO),
        tabla("tabla", None),
    ];
    for stroke in PANDEIRO.iter().map(|s| s.name) {
        drums.push(pandeiro(&format!("pandeiro_{stroke}"), Some(stroke)));
    }
    for bol in BOLS.iter().map(|b| b.name) {
        drums.push(tabla(&format!("tabla_{bol}"), Some(bol)));
    }
    drums.into_iter().map(trimmed).collect()
}

/// How many dB each drum's default stroke measured louder than scsynth's render of it, which it is turned down by.
/// Their strikes' noise comes from another generator than scsynth's, and in a short burst it sets the modes ringing a few dB louder or softer.
/// The timbales' noise is not seeded on scsynth either, so theirs are the mean of four renders, which spread over 3 dB.
const TRIMS: [(&str, f64); 17] = [
    ("snare", 3.3),
    ("crash", 4.9),
    ("closed_hihat", 3.2),
    ("open_hihat", 5.4),
    ("mid_tom", 5.0),
    ("clave", 4.4),
    ("ganza", 2.3),
    ("low_conga", 3.4),
    ("mid_conga", -3.2),
    ("low_timbale", 5.2),
    ("high_timbale", 2.5),
    ("guiro", 5.5),
    ("triangle", 3.5),
    ("tambourine", 7.2),
    ("tambourine_shake", 6.6),
    ("cajon", -3.5),
    ("low_bongo", -5.2),
];

fn trimmed(synth: Synth) -> Synth {
    let Some(&(_, louder)) = TRIMS.iter().find(|(name, _)| *name == synth.name) else { return synth };
    let gain = db(-louder) as f32;
    let build = synth.build.clone();
    Synth {
        build: std::sync::Arc::new(move |o, ctx| {
            let note = build(o, ctx);
            Note { sound: note.sound * gain, ..note }
        }),
        ..synth
    }
}

fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

fn noise(ctx: &mut Context) -> Net {
    white_noise(ctx.seed())
}

fn impulse_net() -> Net {
    net(impulse::<U1>())
}

fn hpf(f: f64) -> Net {
    net(highpass_hz(f.clamp(20.0, 20_000.0) as f32, std::f32::consts::FRAC_1_SQRT_2))
}

fn lpf(f: f64) -> Net {
    net(lowpass_hz(f.clamp(20.0, 20_000.0) as f32, std::f32::consts::FRAC_1_SQRT_2))
}

/// A band-pass with a peak of 1, whose width is `rq` times its centre.
fn bpf(f: f64, rq: f64) -> Net {
    net(bandpass_hz(f.clamp(20.0, 20_000.0) as f32, (1.0 / rq) as f32)) * rq as f32
}

/// A shelf above `f`, raised by `gain` dB.
fn high_shelf(f: f64, gain: f64) -> Net {
    net(highshelf_hz(f.clamp(20.0, 20_000.0) as f32, std::f32::consts::FRAC_1_SQRT_2, db(gain) as f32))
}

/// A shelf below `f`, raised by `gain` dB.
fn low_shelf(f: f64, gain: f64) -> Net {
    net(lowshelf_hz(f.clamp(20.0, 20_000.0) as f32, std::f32::consts::FRAC_1_SQRT_2, db(gain) as f32))
}

/// A bell at `f` whose width is `rq` times it, raised by `gain` dB.
fn peak(f: f64, rq: f64, gain: f64) -> Net {
    net(bell_hz(f.clamp(20.0, 20_000.0) as f32, (1.0 / rq) as f32, db(gain) as f32))
}

fn resonant_lowpass(f: f64, rq: f64) -> Net {
    net(lowpass_hz(f.clamp(20.0, 20_000.0) as f32, (1.0 / rq) as f32))
}

fn resonant_highpass(f: f64, rq: f64) -> Net {
    net(highpass_hz(f.clamp(20.0, 20_000.0) as f32, (1.0 / rq) as f32))
}

fn amps(levels: &[f64]) -> Vec<f64> {
    levels.iter().map(|&l| db(l)).collect()
}

fn scaled(ratios: &[f64], freq: f64) -> Vec<f64> {
    ratios.iter().map(|r| r * freq).collect()
}

fn times(values: &[f64], by: f64) -> Vec<f64> {
    values.iter().map(|v| v * by).collect()
}

fn vel(o: &Options, low: f64) -> f64 {
    o.number("vel").clamp(low, 1.0)
}

fn strike_options(note: f64) -> [Spec; 2] {
    [
        Spec::note("note").default(note).doc("The drum's pitch."),
        Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is struck."),
    ]
}

/// Many close modes drawn from a seed between `low` and `high` Hz, sorted, each with the level in dB and ring at its band, in groups of 20 that `group` may rescale.
fn plate(seed_count: usize, low: f64, high: f64) -> Vec<Vec<f64>> {
    let mut draws = Draws::new(1956);
    let mut hz: Vec<f64> = (0..seed_count).map(|_| draws.exp(low, high)).collect();
    hz.sort_by(f64::total_cmp);
    hz.chunks(20).map(|c| c.to_vec()).collect()
}

fn kick() -> Synth {
    Synth::new("kick").option(Spec::note("note").default(34.0).doc("The drum's pitch.")).build(|o, ctx| {
        let note = o.number("note");
        let sweep = segments(&[26.0, 0.0], &[0.07], &[-6.0])
            >> net(map(move |x: &Frame<f32, U1>| hz(note + x[0] as f64) as f32));
        let body = (sweep >> net(An(Sine::<f32>::with_phase(0.25)))) * hit(0.001, 0.55, -5.0);
        let beater = (noise(ctx) >> hpf(1800.0)) * hit(0.0005, 0.012, -4.0) * 0.3;
        Note::rings((body + beater) >> net(map(|x: &Frame<f32, U1>| x[0].tanh())), 0.7)
    })
}

fn snare() -> Synth {
    Synth::new("snare").options(strike_options(55.0)).build(|o, ctx| {
        let vel = vel(o, 0.0);
        let freq = hz(o.number("note"));
        let stick = impulse_net() + (noise(ctx) >> lpf(4000.0)) * hit(0.0005, 0.004, -4.0) * 0.3;
        let hard = ((vel - 0.65) / 0.175).clamp(0.0, 1.0);
        let ratios = [1.0, 1.2, 1.46, 1.66, 2.18, 2.8];
        let levels: Vec<f64> = [-3.0, 0.0, -3.0, -3.0, -14.0, -14.0]
            .iter()
            .zip([0.0, 0.0, -11.0, -11.0, 4.0, 4.0])
            .map(|(l, h)| db(l + h * hard))
            .collect();
        let rings = [0.65, 0.65, 0.65, 0.6, 0.55, 0.55];
        let hz_ = scaled(&ratios, freq);
        let head = modes(stick.clone(), &hz_, &levels, &times(&rings, 0.07))
            + modes(stick, &hz_, &times(&levels, db(-30.0)), &rings);
        let shake = || exp_segments(&[0.0001, 1.0, 0.001], &[0.001, 0.5]);
        let wash = noise(ctx) >> hpf(500.0) >> low_shelf(1500.0, 6.0) >> high_shelf(6000.0, -6.0);
        let dark = wash >> high_shelf(2000.0, (vel - 0.65).min(0.0) * 20.0);
        let cutoff = shake() >> net(map(|x: &Frame<f32, U1>| (7500.0 * x[0].max(0.0).powf(0.15)).max(2000.0)));
        let wires = ((dark | cutoff | net(dc(std::f32::consts::FRAC_1_SQRT_2))) >> net(lowpass::<f32>()))
            * shake()
            * db(-42.5 - (vel - 0.65).abs() * 14.0) as f32;
        let sound = (head * db(-32.0) as f32 + wires) * (vel / 0.6).powf(3.8) as f32;
        Note::rings(sound, 1.5)
    })
}

fn clap() -> Synth {
    Synth::new("clap").option(Spec::note("note").default(60.0).doc("The clap's pitch.")).build(|o, ctx| {
        let freq = hz(o.number("note"));
        let hands = segments(
            &[0.0, 1.0, 0.2, 1.0, 0.2, 0.9, 0.1, 0.8, 0.0],
            &[0.001, 0.008, 0.001, 0.009, 0.001, 0.011, 0.001, 0.32],
            &[-6.0; 8],
        );
        Note::rings((noise(ctx) >> bpf(freq * 4.2, 0.9)) * hands * 1.8, 0.6)
    })
}

fn crash() -> Synth {
    Synth::new("crash")
        .options(strike_options(60.0))
        .option(Spec::beats("sustain").default(9.0).range(0.0, f64::INFINITY).doc("How long it rings before it is choked."))
        .option(Spec::beats("release").default(1.0).range(0.0, f64::INFINITY).doc("How long the choke takes."))
        .build(|o, ctx| {
            let vel = vel(o, 0.0);
            let scale = 2f64.powf((o.number("note") - 60.0) / 12.0);
            let bands = [300.0, 500.0, 800.0, 1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0];
            let levels = [-30.0, -14.0, -43.0, -30.0, -23.0, -24.0, -18.0, -34.0, -24.0];
            let rings = [11.0, 9.6, 7.06, 5.28, 4.36, 3.38, 2.83, 1.93, 0.8];
            let shorter = [0.0, 0.75, 0.68, 0.51, 0.4, 0.31, 0.18, 0.0, 0.0];
            let stick = (noise(ctx) >> hpf(1000.0) >> high_shelf(3000.0, (vel - 0.6) * 4.0)) * hit(0.0005, 0.01, -4.0);
            let mut bank = Net::wrap(Box::new(zero()));
            for some in plate(160, 300.0, 15000.0) {
                let mean = some.iter().sum::<f64>() / some.len() as f64;
                let longer = (vel / 0.55).powf(-between(mean, &bands, &shorter));
                let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
                let l: Vec<f64> = some.iter().map(|&f| db(between(f, &bands, &levels))).collect();
                let r: Vec<f64> = some.iter().map(|&f| between(f, &bands, &rings) * longer).collect();
                bank = bank + modes(stick.clone(), &f, &l, &r);
            }
            let crack = (noise(ctx) >> hpf(6000.0 * scale)) * hit(0.001, 0.06, -4.0) * 2.0;
            let thud = (noise(ctx) >> bpf(330.0 * scale, 1.0) >> hpf(200.0 * scale) >> lpf(400.0 * scale) >> lpf(400.0 * scale))
                * hit(0.001, 0.035, -4.0)
                * 31.6;
            let (held, release) = (o.number("sustain"), o.number("release"));
            let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
            let sound = (bank * 0.1 + crack + thud) * 0.126 * choke * (vel / 0.6).powf(1.7) as f32;
            Note::rings(sound, held + release)
        })
}

const HAT_BANDS: [f64; 10] = [200.0, 300.0, 500.0, 800.0, 1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0];
const CLOSED_LEVELS: [f64; 10] = [-15.0, -12.0, -11.0, -31.0, -20.0, -18.0, -21.0, -5.0, -8.0, -4.0];
const CLOSED_RINGS: [f64; 10] = [0.6, 0.64, 0.66, 0.67, 0.57, 0.55, 0.53, 0.49, 0.41, 0.28];
const OPEN_LEVELS: [f64; 10] = [-16.0, -12.0, -1.0, -20.0, -13.0, -15.0, -9.0, -3.0, -8.0, -4.0];
const OPEN_RINGS: [f64; 10] = [2.4, 2.5, 2.5, 2.5, 2.5, 2.6, 2.6, 2.5, 2.6, 2.7];

fn hat(name: &'static str, levels: &'static [f64; 10], rings: &'static [f64; 10], length: f64, pedal: bool) -> Synth {
    let mut decl = Synth::new(name)
        .options(strike_options(92.0))
        .option(Spec::beats("release").default(0.05).range(0.0, f64::INFINITY).doc("How long the hat takes to close."));
    if pedal {
        decl = decl.option(Spec::beats("sustain").default(length).range(0.0, f64::INFINITY).doc("How long it rings open."));
    }
    decl.build(move |o, ctx| {
        let vel = vel(o, 0.0);
        let scale = 2f64.powf((o.number("note") - 92.0) / 12.0);
        let held = if pedal { o.number("sustain") } else { length };
        let release = o.number("release");
        let soft = (1.0 - vel) / 0.7;
        let stick = (noise(ctx) >> hpf(1000.0) >> high_shelf(3000.0, (vel - 0.6) * 4.0)) * hit(0.0005, 0.005, -4.0);
        let mut bank = Net::wrap(Box::new(zero()));
        for some in plate(120, 150.0, 16000.0) {
            let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
            let l: Vec<f64> = some.iter().map(|&f| db(between(f, &HAT_BANDS, levels))).collect();
            let r: Vec<f64> = some.iter().map(|&f| between(f, &HAT_BANDS, rings)).collect();
            bank = bank + modes(stick.clone(), &f, &l, &r);
        }
        let dark = bank >> peak(1300.0, 1.5, soft * 6.0);
        let sizzle = (noise(ctx) >> hpf(7000.0 * scale)) * hit(0.001, 0.05, -4.0) * 0.1;
        let thud = (noise(ctx) >> bpf(320.0 * scale, 0.2) >> hpf(250.0 * scale) >> hpf(250.0 * scale) >> lpf(400.0 * scale))
            * hit(0.001, 0.06, -4.0)
            * db(soft.sqrt() * 50.0 - 20.0) as f32;
        let choke = segments(&[1.0, 1.0, 0.0], &[held, release], &[0.0, -4.0]);
        let sound = (dark * 0.1 + sizzle + thud) * 0.0525 * choke * (vel / 0.6).powf(2.8) as f32;
        Note::rings(sound, held + release)
    })
}

struct Tom {
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    skin: [f64; 7],
    gain: f64,
    curve: f64,
    bright: [f64; 4],
}

const HIGH_TOM: Tom = Tom {
    ratios: &[1.0, 1.16, 1.32, 1.95, 2.55, 2.71, 2.99, 3.08, 3.59, 6.78],
    levels: &[0.0, -8.0, -11.0, -10.0, -7.7, -11.0, -4.6, -7.8, -10.7, -10.8],
    rings: &[1.47, 1.47, 1.5, 2.1, 1.0, 0.79, 0.79, 0.78, 0.92, 0.62],
    skin: [-10.0, -12.0, -24.0, -25.0, -24.0, -35.0, -39.0],
    gain: -35.1,
    curve: 1.32,
    bright: [1500.0, 0.0, 12.0, 16.0],
};

const LOW_TOM: Tom = Tom {
    ratios: &[1.0, 1.18, 1.39, 1.59, 1.78, 2.88, 3.0, 3.21, 3.73, 4.1, 4.22],
    levels: &[0.0, -9.4, -12.1, -12.9, -15.2, -9.3, -9.8, -14.7, -14.6, -6.9, -13.2],
    rings: &[1.6, 1.6, 1.56, 1.5, 1.4, 1.5, 1.5, 1.5, 0.95, 0.93, 1.06],
    skin: [-6.0, -10.0, -26.0, -23.0, -18.0, -30.0, -32.0],
    gain: -32.0,
    curve: 0.79,
    bright: [250.0, 24.0, 20.0, 10.0],
};

const SKIN_BANDS: [f64; 7] = [500.0, 800.0, 1300.0, 2000.0, 3200.0, 5000.0, 8000.0];

fn tom(name: &'static str, note: f64, t: &'static Tom) -> Synth {
    Synth::new(name).options(strike_options(note)).build(move |o, ctx| {
        let vel = vel(o, 0.0);
        let freq = hz(o.number("note"));
        let f0 = hz(note);
        let below = (vel - 0.65).min(0.0);
        let above = (vel - 0.65).max(0.0);
        let struck = impulse_net() + noise(ctx) * hit(0.0005, 0.003, -4.0) * 0.3;
        let stick = struck >> high_shelf(t.bright[0], below * t.bright[1]) >> high_shelf(2500.0, below * t.bright[2] + above * t.bright[3]);
        let hz_ = scaled(t.ratios, freq);
        let levels = amps(t.levels);
        let head = modes(stick.clone(), &hz_, &levels, &times(t.rings, 0.08))
            + modes(stick.clone(), &hz_, &times(&levels, db(-10.0)), t.rings);
        let mut dense = Net::wrap(Box::new(zero()));
        for some in plate(60, 400.0, 7000.0) {
            let f: Vec<f64> = some.iter().map(|f| f / f0 * freq).collect();
            let l: Vec<f64> = some.iter().map(|&f| db(between(f, &SKIN_BANDS, &t.skin))).collect();
            let r: Vec<f64> = some.iter().map(|&f| between(f, &SKIN_BANDS, &[0.9, 0.6, 0.55, 0.6, 0.6, 0.6, 0.5])).collect();
            dense = dense + modes(stick.clone(), &f, &l, &r);
        }
        let sound = (head + dense * db(-11.0) as f32) * (db(t.gain) * (vel / 0.65).powf(t.curve)) as f32;
        Note::rings(sound, 2.5)
    })
}

fn rimshot() -> Synth {
    Synth::new("rimshot").option(Spec::note("note").default(74.0).doc("The rim's pitch.")).build(|o, ctx| {
        let freq = hz(o.number("note"));
        let rim = modes(impulse_net(), &scaled(&[1.0, 2.72, 4.3], freq), &[1.0, 0.5, 0.3], &[0.06, 0.04, 0.02]) * 0.25;
        let head = (noise(ctx) >> bpf(2400.0, 1.0)) * hit(0.0005, 0.05, -4.0) * 0.6;
        Note::rings(rim + head, 0.3)
    })
}

fn cowbell() -> Synth {
    Synth::new("cowbell")
        .options(strike_options(70.07))
        .option(Spec::float("muted").default(0.0).range(0.0, 1.0).doc("1 damps the bell with the hand."))
        .build(|o, ctx| {
            let vel = vel(o, 0.05);
            let muted = o.number("muted").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let bright = vel - 0.6;
            let strike = impulse_net() + (noise(ctx) >> hpf(3000.0)) * hit(0.0002, 0.002, -4.0) * 0.3;
            let ratios = [0.925, 0.955, 1.0, 1.034, 1.07, 1.093, 2.09, 2.115, 3.016, 3.63, 3.88, 4.04, 5.43, 7.41, 9.4, 10.5, 12.6, 14.7, 17.0, 21.0];
            let levels = [-26.0, -18.0, 0.0, -1.0, -7.0, -12.0, -18.0, -13.0, -27.0, -22.0, -14.0, -18.0, -7.0, -15.0, -20.0, -21.0, -29.0, -31.0, -33.0, -38.0];
            let tone = [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -11.0, -11.0, -11.0, -18.0, -18.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0];
            let mute = [-8.0, -8.0, -8.0, -8.0, -8.0, -8.0, -12.0, -12.0, 7.0, 7.0, 7.0, 7.0, 4.0, 4.0, 11.0, 11.0, 11.0, 19.0, 19.0, 19.0];
            let worn = 0.6 + 0.4 * vel;
            let rings = [0.8, 0.8, 0.8, 0.8, 0.8, 0.8, 0.65, 0.65, 1.35, 1.05, 1.05, 1.05, 0.9 * worn, 0.9 * worn, 0.9, 0.9, 0.9, 0.75, 0.75, 0.6];
            let a: Vec<f64> = (0..20).map(|i| db(levels[i] + tone[i] * bright + mute[i] * muted)).collect();
            let f = scaled(&ratios, freq);
            let bell = modes(strike.clone(), &f, &a, &times(&rings, 0.13))
                + modes(strike, &f, &times(&a, 0.06), &times(&rings, 1.0 - muted * 0.1));
            Note::rings(bell * (db(-27.5) * (vel / 0.6).powf(2.3)) as f32, 1.5)
        })
}

fn clave() -> Synth {
    Synth::new("clave").options(strike_options(95.2)).build(|o, ctx| {
        let vel = vel(o, 0.05);
        let freq = hz(o.number("note"));
        let bright = (vel - 0.6) * 30.0;
        let strike = impulse_net() + (noise(ctx) >> hpf(1000.0)) * hit(0.0002, 0.002, -4.0) * 0.3;
        let ratios = [0.65, 1.0, 1.08, 1.5, 2.72, 3.6, 6.2];
        let levels: Vec<f64> = [-24.0, 0.0, -6.0, -6.0, -9.5, -20.0, -22.0]
            .iter()
            .zip([0.0, 0.0, 0.0, 0.6, 0.6, 1.3, 1.0])
            .map(|(l, b)| db(l + b * bright))
            .collect();
        let rings = [0.75, 0.75, 0.75, 0.72, 0.65, 0.5, 0.4];
        let f = scaled(&ratios, freq);
        let stick = modes(strike.clone(), &f, &levels, &times(&rings, 0.08)) + modes(strike, &f, &times(&levels, 0.1), &rings);
        Note::rings(stick * (db(-21.8) * (vel / 0.6).powf(1.72)) as f32, 1.0)
    })
}

struct Agogo {
    note: f64,
    gain: f64,
    strike: [f64; 3],
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    length: f64,
}

const LOW_AGOGO: Agogo = Agogo {
    note: 84.0,
    gain: 0.026,
    strike: [7.0, 7800.0, 0.0],
    ratios: &[0.953, 1.0, 2.197, 2.197, 2.85, 3.249, 3.763, 3.91, 4.021, 4.657, 5.848, 6.488, 7.3, 8.1, 9.39],
    levels: &[0.3, 1.0, 0.5, 0.2, 0.65, 0.8, 0.3, 0.25, 0.45, 0.4, 0.5, 0.5, 0.6, 0.45, 0.45],
    rings: &[2.6, 2.0, 0.25, 1.0, 0.7, 1.0, 0.62, 0.6, 0.72, 0.52, 0.57, 0.45, 0.4, 0.35, 0.29],
    length: 2.6,
};

const HIGH_AGOGO: Agogo = Agogo {
    note: 90.07,
    gain: 0.035,
    strike: [4.0, 20000.0, 6.0],
    ratios: &[0.671, 0.704, 1.0, 1.47, 2.206, 3.226, 3.572, 4.013, 4.598, 5.24, 5.9, 6.6],
    levels: &[0.04, 0.04, 1.0, 0.1, 0.25, 0.4, 0.1, 0.35, 0.8, 0.6, 0.5, 0.45],
    rings: &[0.75, 1.6, 0.56, 0.75, 1.05, 0.85, 0.65, 0.62, 0.42, 0.5, 0.38, 0.34],
    length: 1.5,
};

fn agogo(name: &'static str, a: &'static Agogo) -> Synth {
    Synth::new(name).options(strike_options(a.note)).build(move |o, ctx| {
        let vel = vel(o, 0.0);
        let freq = hz(o.number("note"));
        let click = impulse_net() + (noise(ctx) >> hpf(3000.0)) * hit(0.0002, 0.002, -4.0) * 0.5;
        let stick = click >> lpf(4000.0 + vel * 12000.0);
        let bell = modes(stick, &scaled(a.ratios, freq), a.levels, a.rings);
        let hiss = (noise(ctx) >> hpf(1200.0) >> hpf(1200.0)) * exp_segments(&[0.001, 1.0, 0.001], &[0.0005, 0.2]) * a.strike[0] as f32;
        let thump = (noise(ctx) >> bpf(freq * 0.47, 0.8)) * exp_segments(&[0.001, 1.0, 0.001], &[0.001, 0.15]) * a.strike[2] as f32;
        let top = (a.strike[1] * (0.4 + vel)).clamp(1000.0, 20000.0);
        let sound = (bell + (hiss >> lpf(top)) + thump) * (a.gain * vel.powf(1.4) / 0.6f64.powf(1.4)) as f32;
        Note::rings(sound, a.length)
    })
}

struct Shake {
    note: f64,
    gain: f64,
    down: [f64; 11],
    upward: [f64; 11],
    stages: usize,
    dip: f64,
    darken: f64,
    length: f64,
}

const GANZA: Shake = Shake {
    note: 111.0,
    gain: 0.33,
    down: [0.045, 0.72, 2.6, 1.45, -8.0, 0.21, 0.72, 1.0, 1.0, -4.0, 5.0],
    upward: [0.025, 0.2, 2.4, 1.4, -12.0, 0.13, 0.2, 1.0, 1.0, -6.0, 5.0],
    stages: 1,
    dip: 0.75,
    darken: 1.0,
    length: 0.26,
};

const SHAKER: Shake = Shake {
    note: 103.0,
    gain: 0.3,
    down: [0.02, 0.85, 2.6, 0.8, -3.0, 1.1, 0.2, 0.35, 0.65, 3.0, 5.0],
    upward: [0.15, 0.85, 1.6, 0.6, -11.0, 1.1, 0.85, 1.0, 0.9, -6.0, 5.0],
    stages: 3,
    dip: 0.6,
    darken: 0.45,
    length: 1.2,
};

fn shake(name: &'static str, s: &'static Shake) -> Synth {
    Synth::new(name)
        .options(strike_options(s.note))
        .option(Spec::float("up").default(0.0).range(0.0, 1.0).doc("1 shakes upward instead of down."))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let up = o.number("up").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let shape: Vec<f64> = (0..11).map(|i| s.down[i] + up * (s.upward[i] - s.down[i])).collect();
            let burst = segments(&[0.0, 1.0, 0.0], &[shape[0], shape[6]], &[2.0, shape[9]]);
            let tail = segments(&[0.0, 1.0], &[shape[0]], &[2.0]) * exp_segments(&[1.0, 1.0, 0.001], &[shape[0], shape[1]]);
            let ending = segments(&[1.0, 1.0, 0.0], &[shape[5], 0.04], &[0.0, 0.0]);
            let keep = shape[7] as f32;
            let mixed = (coloured(&shape, s, freq, vel, ctx) | burst | tail)
                >> net(map(move |x: &Frame<f32, U4>| x[0] * x[2] * (1.0 - keep) + x[1] * x[3] * keep));
            Note::rings(mixed * ending * (s.gain * vel / 0.6) as f32, s.length)
        })
}

/// The shaker's beads coloured as its design gives: high-passed at a bottom that sinks, dipped, low-passed at a top that falls, brightened by vel, and the same darkened, as two outputs.
fn coloured(shape: &[f64], s: &Shake, freq: f64, vel: f64, ctx: &mut Context) -> Net {
    let beads = ((net(dc(4000.0)) >> dust(ctx.seed())) >> decay2(0.0001, 0.001)) * noise(ctx) + noise(ctx) * 0.1;
    let lowest = freq * shape[3];
    let bottom = exp_segments(&[lowest, lowest * shape[8]], &[shape[1]]) >> net(map(|x: &Frame<f32, U1>| x[0].clamp(20.0, 20000.0)));
    let mut colour = beads;
    for _ in 0..s.stages {
        colour = (colour | bottom.clone() | net(dc(1.0))) >> net(highpass::<f32>());
    }
    let dipped = colour >> peak((freq * s.dip).clamp(20.0, 20000.0), 0.5, shape[4]);
    let highest = freq * shape[2];
    let top = exp_segments(&[highest, highest * s.darken], &[shape[1]]) >> net(map(|x: &Frame<f32, U1>| x[0].clamp(20.0, 20000.0)));
    let bright = ((dipped | top | net(dc(1.0))) >> net(lowpass::<f32>())) >> high_shelf(freq * 1.2, (vel - 0.6) * 8.0);
    bright >> (net(pass()) ^ resonant_lowpass(freq * shape[10], 1.0))
}

struct Conga {
    note: f64,
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    skin: [f64; 7],
    skin_rings: [f64; 7],
    tap: [f64; 7],
    tap_rings: [f64; 7],
    head_tap: &'static [f64],
    tap_gain: f64,
    curve: f64,
    bright: [f64; 2],
    gain: f64,
}

const LOW_CONGA: Conga = Conga {
    note: 48.93,
    ratios: &[0.558, 0.78, 0.85, 1.0, 1.12, 1.525, 2.14, 2.24, 2.32, 2.42],
    levels: &[-7.0, -21.0, -23.0, 0.0, -27.0, -12.0, -20.0, -13.0, -14.0, -12.0],
    rings: &[0.5, 1.05, 1.05, 1.08, 1.08, 1.05, 0.8, 0.8, 0.8, 0.75],
    skin: [-23.0, -34.0, -53.5, -45.0, -37.0, -41.0, -51.0],
    skin_rings: [0.64, 0.5, 0.65, 0.7, 0.57, 0.53, 0.89],
    tap: [-14.0, -21.0, -40.0, -28.0, -38.0, -35.0, -43.0],
    tap_rings: [0.6, 0.6, 0.8, 0.8, 1.2, 1.0, 1.3],
    head_tap: &[-40.0, -38.0, -38.0, -41.0, -38.0, -33.0, -26.0, -26.0, -26.0, -26.0],
    tap_gain: -9.0,
    curve: 0.79,
    bright: [-6.0, 5.0],
    gain: -33.8,
};

const MID_CONGA: Conga = Conga {
    note: 51.88,
    ratios: &[0.487, 0.6, 0.66, 0.785, 0.944, 1.0, 1.13, 1.19, 1.627, 2.126, 2.226, 2.437, 3.5],
    levels: &[-5.5, -29.0, -30.0, -26.0, -19.0, 0.0, -24.0, -30.0, -20.0, -32.0, -22.0, -23.0, -16.0],
    rings: &[0.78, 0.9, 0.9, 1.05, 1.05, 1.05, 1.05, 1.05, 0.85, 0.62, 0.62, 0.62, 0.7],
    skin: [-22.0, -35.0, -55.0, -52.0, -45.0, -48.0, -58.0],
    skin_rings: [0.7, 0.6, 0.8, 0.8, 0.6, 0.6, 0.6],
    tap: [-12.0, -17.6, -18.2, -24.5, -14.0, -4.0, -15.0],
    tap_rings: [0.6, 0.6, 0.6, 0.7, 0.7, 0.5, 0.6],
    head_tap: &[-18.0, -18.0, -18.0, -18.0, -18.0, -28.0, -28.0, -28.0, -30.0, -30.0, -30.0, -30.0, -28.0],
    tap_gain: -23.1,
    curve: 1.17,
    bright: [-5.5, 3.0],
    gain: -23.3,
};

const HIGH_CONGA: Conga = Conga {
    note: 56.62,
    ratios: &[0.372, 0.856, 0.943, 1.0, 1.12, 1.618, 1.664, 2.247, 2.415],
    levels: &[-12.0, -33.0, -26.5, 0.0, -33.6, -14.5, -17.5, -25.0, -39.5],
    rings: &[0.9, 0.72, 0.71, 0.7, 0.7, 0.8, 0.8, 0.47, 0.47],
    skin: [-19.0, -34.0, -53.5, -50.0, -44.0, -46.0, -52.0],
    skin_rings: [0.48, 0.62, 0.72, 1.0, 0.85, 1.15, 1.4],
    tap: [-20.0, -28.6, -22.4, -16.0, -16.8, -2.0, -17.2],
    tap_rings: [1.3, 1.3, 1.1, 1.1, 1.2, 0.35, 1.2],
    head_tap: &[-11.0, -19.0, -19.0, -19.0, -19.0, -33.0, -33.0, -30.0, -30.0],
    tap_gain: -32.4,
    curve: 1.42,
    bright: [-1.5, 3.0],
    gain: -22.8,
};

fn conga(name: &'static str, c: &'static Conga) -> Synth {
    Synth::new(name)
        .options(strike_options(c.note))
        .option(Spec::float("mute").default(0.0).range(0.0, 1.0).doc("1 taps the head with the fingers."))
        .option(Spec::float("slap").default(0.0).range(0.0, 1.0).doc("1 slaps the head."))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let mute = o.number("mute").clamp(0.0, 1.0);
            let slap = o.number("slap").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mix = |a: f64, b: f64| a + (b - a) * mute;
            let hand = impulse_net()
                + (noise(ctx) >> lpf(2000.0 + vel * 6000.0 + slap * 6000.0)) * hit(0.0005, 0.003 + slap * 0.01, -4.0) * 0.3;
            let levels: Vec<f64> = c.levels.iter().zip(c.head_tap).map(|(l, t)| db(l + mute * t - slap * 6.0)).collect();
            let rings = times(c.rings, mix(1.0, 0.7) * (1.0 - slap * 0.75));
            let head = modes(hand.clone(), &scaled(c.ratios, freq), &levels, &rings);
            let skin_levels: Vec<f64> = (0..7).map(|i| mix(c.skin[i], c.tap[i]) + slap * 12.0).collect();
            let skin_ring: Vec<f64> = (0..7).map(|i| mix(c.skin_rings[i], c.tap_rings[i]) * (1.0 - slap * 0.5)).collect();
            let mut skin = Net::wrap(Box::new(zero()));
            for some in plate(40, 400.0, 10000.0) {
                let l: Vec<f64> = some.iter().map(|&f| db(between(f, &SKIN_BANDS, &skin_levels))).collect();
                let r: Vec<f64> = some.iter().map(|&f| between(f, &SKIN_BANDS, &skin_ring)).collect();
                skin = skin + modes(hand.clone(), &some, &l, &r);
            }
            let crack = (noise(ctx) >> bpf(3000.0, 0.7)) * hit(0.0005, 0.04, -4.0) * (slap * 4.0) as f32;
            let tone = (vel - 0.6).min(0.0) / -0.3 * c.bright[0] + (vel - 0.6).max(0.0) / 0.4 * c.bright[1];
            let sound = (head + skin + crack) >> peak(900.0, 2.0, tone);
            Note::rings(sound * (db(c.gain + mute * c.tap_gain) * (vel / 0.6).powf(c.curve)) as f32, 1.31)
        })
}

fn timbale(name: &'static str, note: f64) -> Synth {
    Synth::new(name).option(Spec::note("note").default(note).doc("The drum's pitch.")).build(|o, ctx| {
        let freq = hz(o.number("note"));
        let stick = (noise(ctx) >> hpf(1500.0)) * hit(0.0004, 0.008, -4.0);
        let head = modes(stick.clone() + impulse_net(), &scaled(&[1.0, 1.59, 2.14, 2.65], freq), &[1.0, 0.5, 0.35, 0.2], &[0.35, 0.22, 0.15, 0.1]) * 0.09;
        let shell = modes(stick, &scaled(&[1.0, 1.47, 2.09, 2.56], freq * 4.1), &[0.4, 0.3, 0.2, 0.15], &[0.9, 0.7, 0.5, 0.4]) * 0.05;
        Note::rings(head + shell, 1.2)
    })
}

fn timbale_shell() -> Synth {
    Synth::new("timbale_shell").option(Spec::note("note").default(93.0).doc("The shell's pitch.")).build(|o, ctx| {
        let freq = hz(o.number("note"));
        let stick = (noise(ctx) >> hpf(3000.0)) * hit(0.0002, 0.003, -4.0);
        let shell = modes(stick.clone() + impulse_net(), &scaled(&[1.0, 1.38, 1.83, 2.41], freq), &[1.0, 0.6, 0.4, 0.25], &[0.05, 0.04, 0.03, 0.02]) * 0.2;
        Note::rings(shell + stick * 0.4, 0.25)
    })
}

struct Stroke {
    name: &'static str,
    gain: f64,
    head: [f64; 5],
    rings: [f64; 5],
    crack: [f64; 3],
    jingle: [f64; 2],
    length: f64,
}

const PANDEIRO: [Stroke; 4] = [
    Stroke { name: "thumb", gain: 0.0507, head: [1.0, 0.5, 0.18, 0.1, 0.05], rings: [0.55, 0.8, 0.6, 0.3, 0.2], crack: [3.0, 0.15, 8.0], jingle: [1.0, 0.5], length: 1.6 },
    Stroke { name: "heel", gain: 0.074, head: [0.25, 1.0, 0.45, 0.2, 0.12], rings: [0.12, 0.5, 0.3, 0.2, 0.15], crack: [10.0, 0.15, 8.0], jingle: [3.5, 0.4], length: 1.0 },
    Stroke { name: "slap", gain: 0.221, head: [0.01, 0.015, 0.015, 0.01, 0.01], rings: [0.6, 0.6, 0.5, 0.4, 0.3], crack: [6.0, 0.13, 5.0], jingle: [1.5, 0.3], length: 0.8 },
    Stroke { name: "jingles", gain: 0.582, head: [0.035, 0.025, 0.012, 0.006, 0.003], rings: [0.4, 0.4, 0.3, 0.2, 0.1], crack: [1.0, 0.08, 10.0], jingle: [1.0, 0.35], length: 1.2 },
];

/// The pandeiro, whose `stroke` option picks the stroke; `fixed` makes one of the old names that each played one stroke.
fn pandeiro(name: &str, fixed: Option<&'static str>) -> Synth {
    let mut decl = Synth::new(name).options(strike_options(49.0));
    if fixed.is_none() {
        let names: Vec<&str> = PANDEIRO.iter().map(|s| s.name).collect();
        decl = decl.option(Spec::name("stroke", &names).default("thumb").doc("The stroke: the thumb, the heel of the hand, a slap or the fingertips on the jingles."));
    }
    decl.build(move |o, ctx| {
        let picked = fixed.unwrap_or_else(|| o.name("stroke"));
        let s = PANDEIRO.iter().find(|s| s.name == picked).expect("a stroke the option allows");
        let vel = vel(o, 0.0);
        let freq = hz(o.number("note"));
        let stick = (impulse_net() + noise(ctx) * hit(0.0005, 0.004, -4.0)) >> lpf(800.0 + vel * 4000.0);
        let skin = modes(stick, &scaled(&[0.974, 1.652, 2.475, 3.25, 4.13], freq), &s.head, &s.rings);
        let body = (noise(ctx) >> bpf(freq * s.crack[2], 2.0))
            * (hit(0.001, s.crack[1], -4.0) + hit(0.001, s.crack[1] * 4.0, -4.0) * 0.06)
            * s.crack[0] as f32;
        let shaking = exp_segments(&[0.001, 1.0, 0.001], &[0.001, s.jingle[1]]);
        let rattle = noise(ctx) * shaking;
        let disc_ratios = [22.24, 24.01, 25.11, 29.26, 32.13, 35.68, 40.93, 83.71, 85.83, 87.68, 91.32, 92.08, 102.31, 106.03];
        let disc_levels = [0.25, 0.15, 0.15, 0.2, 0.25, 0.15, 0.15, 0.85, 1.0, 0.75, 0.55, 0.6, 0.6, 0.8];
        let disc_rings = [0.04, 0.04, 0.04, 0.04, 0.04, 0.04, 0.04, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08, 0.08];
        let rattling = rattle >> (net(pass()) ^ net(pass()));
        let discs = rattling
            >> (((net(pass()) * 0.02) >> modes(net(pass()), &scaled(&disc_ratios, freq), &disc_levels, &disc_rings))
                | (hpf(freq * 20.0) * 0.3));
        let jingles = (discs >> net(map(|x: &Frame<f32, U2>| x[0] + x[1]))) * s.jingle[0] as f32;
        let bright = (skin + body + jingles) >> high_shelf(2000.0, (vel - 0.6) * 5.0);
        Note::rings(bright * (s.gain * (vel / 0.6).powf(0.75)) as f32, s.length)
    })
}

fn guiro() -> Synth {
    Synth::new("guiro")
        .options(strike_options(93.29))
        .option(Spec::beats("sustain").default(0.25).range(0.02, f64::INFINITY).doc("How long the scrape lasts."))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let vel = vel(o, 0.05);
            let length = o.number("sustain").max(0.02);
            let bright = vel - 0.6;
            let rate = (55.0 / length).clamp(40.0, 125.0);
            // The ridges come at a rate that wanders by 15 percent, and each clicks at a level from 0.6 to 1.
            let mut draws = Draws::new(ctx.seed());
            let mut at = 0.0;
            let mut clicks: Vec<(f64, f64)> = Vec::new();
            while at < length + 0.02 {
                clicks.push((at, 0.6 + 0.4 * draws.draw()));
                at += 1.0 / (rate * (0.85 + 0.3 * draws.draw()));
            }
            let ridges = crate::nodes::clicks(clicks);
            let stroke = segments(&[0.0, 1.0, 1.0, 0.0], &[0.005, length, 0.01], &[0.0, 0.0, 0.0]);
            let ticks = ((ridges.clone() * 4.0) + (ridges >> decay2(0.0, 0.0015)) * noise(ctx)) * stroke;
            let ratios = [0.29, 0.37, 0.52, 0.67, 1.0, 1.53, 1.94, 3.0, 3.7, 4.4];
            let levels: Vec<f64> = [-22.0, -18.0, -9.0, -4.0, 0.0, -12.0, -10.0, -22.0, -32.0, -34.0]
                .iter()
                .zip([0.0, 0.0, 0.0, 0.0, 0.0, 8.0, 8.0, 20.0, 20.0, 20.0])
                .map(|(l, b)| db(l + b * bright))
                .collect();
            let rings = [0.16, 0.11, 0.12, 0.16, 0.16, 0.3, 0.2, 0.25, 0.5, 0.5];
            let f = scaled(&ratios, freq);
            let gourd = (modes(ticks.clone(), &f, &levels, &times(&rings, 0.2)) + modes(ticks, &f, &times(&levels, 0.05), &times(&rings, 4.0))) * 0.03;
            Note::rings(gourd * (db(-6.0) * (vel / 0.6).powf(1.15)) as f32, length + 0.5)
        })
}

fn triangle() -> Synth {
    Synth::new("triangle")
        .options(strike_options(69.0))
        .option(Spec::beats("sustain").default(30.0).range(0.0, f64::INFINITY).doc("How long the bar rings before the hand damps it."))
        .option(Spec::beats("release").default(1.0).range(0.0, f64::INFINITY).doc("How long the hand takes to damp it."))
        .build(|o, ctx| {
            let vel = vel(o, 0.05);
            let scale = 2f64.powf((o.number("note") - 69.0) / 12.0);
            let (held, release) = (o.number("sustain"), o.number("release"));
            let beater = impulse_net() + (noise(ctx) >> hpf(3000.0 + vel * 5000.0)) * hit(0.0002, 0.002, -4.0) * vel as f32;
            let bands = [1300.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0];
            let levels = [-40.0, -44.5, -21.5, -15.0, -2.0, -0.5];
            let rings = [10.0, 10.0, 9.0, 5.0, 3.0, 1.5];
            let groups = plate(120, 1000.0, 18000.0);
            let lowest = groups[0].iter().sum::<f64>() / groups[0].len() as f64;
            let mut bar = Net::wrap(Box::new(zero()));
            let mut longest: f64 = 0.0;
            for some in &groups {
                let mean = some.iter().sum::<f64>() / some.len() as f64;
                let damp = release * (mean / lowest).powf(-0.35);
                longest = longest.max(held + damp);
                let hand = exp_segments(&[1.0, 1.0, 0.001], &[held, damp]);
                let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
                let l: Vec<f64> = some.iter().map(|&f| db(between(f, &bands, &levels))).collect();
                let r: Vec<f64> = some.iter().map(|&f| between(f, &bands, &rings)).collect();
                bar = bar + modes(beater.clone(), &f, &l, &r) * hand;
            }
            let hard = (vel - 0.6).max(0.0) / 0.4;
            let sound = bar >> peak(1300.0, 0.8, hard * 8.0) >> low_shelf(3500.0, hard * 5.0);
            let level = db(-46.0) * (vel / 0.6).powf(2.6);
            // The bar falls below scsynth's silence threshold well before its 10 s rings run out.
            Note::rings(sound * level as f32, longest.min(held + 12.0))
        })
}

fn tambourine(name: &'static str, shaken: bool) -> Synth {
    let mut decl = Synth::new(name).options(strike_options(69.0));
    if shaken {
        decl = decl.option(Spec::beats("sustain").default(0.25).range(0.02, f64::INFINITY).doc("How long it is shaken."));
    }
    decl.build(move |o, ctx| {
        let vel = vel(o, 0.05);
        let scale = 2f64.powf((o.number("note") - 69.0) / 12.0);
        let rattle = 0.15 + vel * 0.6;
        let held = if shaken { o.number("sustain").max(0.02) } else { 0.0 };
        let shaking = || segments(&[0.0, 1.0, 1.0, 0.0], &[0.005, held, rattle], &[0.0, 0.0, -4.0]);
        let clicks = ((shaking() * 400.0) >> dust(ctx.seed())) * shaking();
        let strike = if shaken {
            Net::wrap(Box::new(zero()))
        } else {
            impulse_net() + (noise(ctx) >> hpf(2000.0)) * hit(0.0005, 0.004, -4.0)
        };
        let bands = HAT_BANDS;
        let levels = [-29.0, -24.0, -24.8, -17.5, -13.3, -17.0, -21.4, 0.0, -5.0, 0.0];
        let rings = [0.55, 0.6, 0.7, 0.5, 0.45, 0.2, 0.15, 0.15, 0.15, 0.15];
        let exciter = strike * 0.3 + (clicks >> hpf(2500.0) >> hpf(2500.0));
        let mut jingles = Net::wrap(Box::new(zero()));
        for some in plate(120, 170.0, 16000.0) {
            let f: Vec<f64> = some.iter().map(|f| f * scale).collect();
            let l: Vec<f64> = some.iter().map(|&f| db(between(f, &bands, &levels))).collect();
            let r: Vec<f64> = some.iter().map(|&f| between(f, &bands, &rings)).collect();
            jingles = jingles + modes(exciter.clone(), &f, &l, &r);
        }
        let sound = (jingles >> low_shelf(1000.0, (vel - 0.6).max(0.0) * 16.0)) * (db(-36.0) * (vel / 0.6).powf(1.9)) as f32;
        Note::rings(sound, 0.005 + held + rattle + 0.7)
    })
}

fn cabasa() -> Synth {
    Synth::new("cabasa")
        .options(strike_options(96.0))
        .option(Spec::float("rub").default(0.0).range(0.0, 1.0).doc("1 turns the beads against the cylinder for the sustain."))
        .option(Spec::beats("sustain").default(0.1).range(0.01, f64::INFINITY).doc("How long a rub lasts."))
        .build(|o, ctx| {
            let freq = hz(o.number("note"));
            let vel = vel(o, 0.0);
            let rub = o.number("rub").clamp(0.0, 1.0);
            let hold = o.number("sustain").max(0.01) * rub;
            let burst = segments(&[0.0, 1.0, 1.0], &[0.002, hold], &[0.0, 0.0]) * exp_segments(&[1.0, 1.0, 1.0, 0.001], &[0.002, hold, 0.22]);
            let tail = segments(&[0.0, 1.0, 1.0], &[0.002, hold], &[0.0, 0.0]) * exp_segments(&[1.0, 1.0, 1.0, 0.001], &[0.002, hold, 0.9]);
            let beads = ((net(dc(6000.0)) >> dust(ctx.seed())) >> decay2(0.0001, 0.0008)) * noise(ctx) + noise(ctx) * 0.2;
            let bottom = freq * (0.6 + rub * 0.4);
            let colour = beads >> resonant_highpass(bottom, 1.41) >> resonant_highpass(bottom, 1.41);
            let shell = (noise(ctx) >> bpf(freq * 0.25, 1.0)) * (0.15 * (1.0 - rub)) as f32;
            let bright = ((colour >> high_shelf(freq * 3.0, 4.0) >> peak(freq * 2.4, 1.0, -3.0)) + shell) >> high_shelf(freq * 2.0, (vel - 0.6) * 6.0);
            let both = bright >> (net(pass()) ^ resonant_lowpass(freq * 3.5, 1.0));
            let level = (0.5 * (1.0 - rub * 0.52) * (vel / 0.6).powf(1.1)) as f32;
            let sound = (both | burst | tail) >> net(map(|x: &Frame<f32, U4>| x[0] * x[2] * 0.9 + x[1] * x[3] * 0.1));
            Note::rings(sound * level, 0.002 + hold + 0.9)
        })
}

fn cajon() -> Synth {
    Synth::new("cajon")
        .options(strike_options(40.0))
        .option(Spec::float("slap").default(0.0).range(0.0, 1.0).doc("1 slaps the front plate instead of the bass tone."))
        .build(|o, ctx| {
            let vel = vel(o, 0.0);
            let slap = o.number("slap").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mix = |bass: f64, slapped: f64| bass + (slapped - bass) * slap;
            let ringing = |exciter: Net, ratios: &[f64], levels: &[f64], rings: &[f64], tail: &[f64]| {
                let f = scaled(ratios, freq);
                let tails: Vec<f64> = levels.iter().zip(tail).map(|(l, t)| l * t).collect();
                modes(exciter.clone(), &f, levels, &times(rings, 0.25)) + modes(exciter, &f, &tails, rings)
            };
            let hand = (noise(ctx) >> lpf(2000.0 + slap * 8000.0 + vel * 4000.0)) * hit(0.0005, 0.004 + slap * 0.004, -4.0);
            let click = (noise(ctx) >> hpf(1000.0)) * hit(0.0003, 0.003, -4.0);
            let box_ = ringing(
                impulse_net() + hand.clone() * 0.3,
                &[0.983, 1.61, 2.2, 3.7],
                &[1.0, 0.45, mix(0.05, 1.1), mix(0.033, 0.23)],
                &[mix(0.46, 0.9), 0.3, mix(0.33, 0.44), mix(0.7, 0.46)],
                &[0.3, 0.3, mix(0.3, 0.15), 0.3],
            );
            let plate_ = ringing(
                hand,
                &[6.3, 6.5, 6.7, 6.9, 9.8],
                &[1.0, 1.0, 1.0, 1.0, mix(0.4, 0.55)],
                &[mix(0.45, 0.26), mix(0.45, 0.26), mix(0.45, 0.26), mix(0.45, 0.26), mix(0.6, 0.32)],
                &[0.15; 5],
            ) * mix(0.0014, 0.0225) as f32;
            let bass_levels = [0.3, 0.4, 0.5, 0.6, 0.25, 0.29, 0.49, 0.47, 0.47];
            let slap_levels = [2.3, 1.55, 1.6, 1.9, 0.7, 0.8, 1.16, 1.19, 1.19];
            let bass_rings = [1.0, 0.97, 0.94, 0.9, 0.85, 0.8, 0.72, 0.66, 0.6];
            let slap_rings = [0.255, 0.25, 0.28, 0.27, 0.255, 0.24, 0.216, 0.198, 0.18];
            let levels: Vec<f64> = (0..9).map(|i| mix(bass_levels[i], slap_levels[i])).collect();
            let rings: Vec<f64> = (0..9).map(|i| mix(0.5 * bass_rings[i], slap_rings[i])).collect();
            let shimmer = modes(click, &scaled(&[16.0, 23.0, 31.0, 44.0, 58.0, 75.0, 97.0, 128.0, 160.0], freq), &levels, &rings)
                * mix(0.00175, 0.0078) as f32;
            let sound = (box_ * 0.05 + plate_ + shimmer) >> high_shelf(1500.0, (vel - 0.6) * 5.0);
            Note::rings(sound * (mix(1.0, 0.106) * (vel / 0.6).powf(1.6)) as f32, 1.72)
        })
}

struct Bongo {
    note: f64,
    ratios: &'static [f64],
    open: &'static [f64],
    open_rings: &'static [f64],
    muted: &'static [f64],
    muted_rings: &'static [f64],
    fast: &'static [f64],
    tail: &'static [f64],
    curve: [f64; 2],
    gain: f64,
}

const LOW_BONGO: Bongo = Bongo {
    note: 60.0,
    ratios: &[0.46, 0.906, 1.005, 1.05, 1.548, 1.976, 2.9, 4.8, 7.6, 12.4, 19.5, 30.0],
    open: &[-31.8, -12.0, 0.0, -7.0, -11.5, -15.5, -21.0, -33.7, -34.0, -44.0, -45.0, -50.0],
    open_rings: &[1.97, 0.88, 0.91, 0.91, 0.79, 0.79, 0.68, 0.65, 0.62, 1.15, 1.26, 2.05],
    muted: &[-16.3, -7.0, 0.0, -7.0, -5.2, -9.2, -14.3, -35.4, -34.0, -38.0, -40.0, -45.0],
    muted_rings: &[0.79, 0.76, 0.79, 0.79, 0.75, 0.75, 0.7, 0.65, 0.69, 0.69, 0.63, 1.54],
    fast: &[0.25, 0.25, 0.18, 0.18, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25, 0.25],
    tail: &[0.2; 12],
    curve: [1.95, 2.45],
    gain: 0.02,
};

const HIGH_BONGO: Bongo = Bongo {
    note: 70.0,
    ratios: &[0.26, 0.45, 0.63, 0.665, 0.97, 0.987, 1.7, 2.7, 4.3, 7.0, 11.0, 17.0, 27.0],
    open: &[-35.2, -16.0, -0.9, -0.9, -6.0, 0.0, -18.2, -20.6, -17.8, -31.8, -36.6, -40.6, -50.6],
    open_rings: &[6.5, 0.95, 0.98, 0.98, 0.64, 0.64, 0.59, 0.64, 0.65, 1.22, 1.8, 2.61, 3.53],
    muted: &[-29.0, -14.5, -1.0, -1.0, -6.0, 0.0, -15.2, -22.2, -19.3, -30.6, -30.9, -32.1, -45.7],
    muted_rings: &[1.59, 0.79, 0.9, 0.9, 0.68, 0.68, 0.52, 0.65, 0.71, 0.69, 0.63, 0.85, 2.49],
    fast: &[0.25, 0.25, 0.12, 0.12, 0.7, 0.7, 1.0, 0.25, 0.15, 0.25, 0.25, 0.25, 0.25],
    tail: &[0.2, 0.2, 0.14, 0.14, 0.0, 0.0, 0.0, 0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
    curve: [1.95, 1.0],
    gain: 0.017,
};

fn bongo(name: &'static str, b: &'static Bongo) -> Synth {
    Synth::new(name)
        .options(strike_options(b.note))
        .option(Spec::float("muted").default(0.0).range(0.0, 1.0).doc("1 mutes the head with the hand."))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let mute = o.number("muted").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mix = |a: f64, b: f64| a + (b - a) * mute;
            let hand = impulse_net() + (noise(ctx) >> lpf(3000.0 + vel * 6000.0)) * hit(0.0005, 0.003, -4.0) * 0.3;
            let levels: Vec<f64> = b.open.iter().zip(b.muted).map(|(o, m)| db(mix(*o, *m))).collect();
            let rings: Vec<f64> = b.open_rings.iter().zip(b.muted_rings).map(|(o, m)| mix(*o, *m) * 1.28).collect();
            let f = scaled(b.ratios, freq);
            let fast: Vec<f64> = rings.iter().zip(b.fast).map(|(r, f)| r * f).collect();
            let tails: Vec<f64> = levels.iter().zip(b.tail).map(|(l, t)| l * t).collect();
            let head = modes(hand.clone(), &f, &levels, &fast) + modes(hand, &f, &tails, &rings);
            let level = b.gain * (vel / 0.6).powf(mix(b.curve[0], b.curve[1]));
            Note::rings((head >> high_shelf(1500.0, (vel - 0.6) * 5.0)) * level as f32, 1.18)
        })
}

/// The surdo de primeira's open stroke rings on its lowest mode, 66.6 Hz in plotzki's loops, with the mode at twice it 10 dB down; the muted stroke, the hand on the head, loses that mode and rings higher and shorter.
/// Each set is ratios to the open stroke's lowest mode, levels in dB and seconds to fade 60 dB; above them, modes in each band from 250 Hz to 4 kHz fill the felt mallet's thud.
const SURDO_OPEN: [[f64; 8]; 3] = [
    [1.0, 1.69, 2.0, 2.38, 2.61, 3.0, 3.32, 4.12],
    [0.0, -26.0, -15.5, -16.0, -12.5, -18.5, -21.5, -24.5],
    [1.45, 1.25, 1.25, 1.1, 1.1, 1.0, 1.0, 1.2],
];
const SURDO_MUTED: [[f64; 8]; 3] = [
    [1.82, 2.03, 2.36, 2.57, 2.76, 3.11, 5.01, 5.71],
    [0.0, -6.6, -7.2, -8.3, -0.4, -3.7, 0.0, -6.5],
    [0.52, 0.68, 0.64, 0.64, 0.67, 0.69, 0.44, 0.48],
];
/// The higher modes' ratios, their levels in dB as struck and the seconds they fade 60 dB in, at each band from 250 Hz to 4 kHz; each leaves a tail 20 dB down that fades as slowly as the recordings' bands.
const SURDO_HIGH: [[f64; 6]; 4] = [
    [4.9, 7.6, 12.0, 19.0, 30.0, 47.5],
    [-24.0, -12.3, -5.5, -23.0, -37.0, -34.0],
    [0.5, 0.25, 0.25, 0.2, 0.2, 0.2],
    [1.2, 2.4, 2.4, 2.4, 2.5, 3.4],
];

fn surdo() -> Synth {
    Synth::new("surdo")
        .options(strike_options(36.31))
        .option(Spec::float("muted").default(0.0).range(0.0, 1.0).doc("1 mutes the head with the hand, so it rings higher and shorter."))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let mute = o.number("muted").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let mallet = impulse_net() + (noise(ctx) >> lpf(1500.0 + vel * 3000.0)) * hit(0.001, 0.006, -4.0) * 0.2;
            let set = |modes: &[[f64; 8]; 3], by: f64| -> (Vec<f64>, Vec<f64>, Vec<f64>) {
                (scaled(&modes[0], freq), modes[1].iter().map(|l| db(*l) * by).collect(), modes[2].to_vec())
            };
            let (open_hz, open_levels, open_rings) = set(&SURDO_OPEN, 1.0 - mute);
            let (muted_hz, muted_levels, muted_rings) = set(&SURDO_MUTED, mute * db(SURDO_MUTED_LEVEL));
            let high_hz = scaled(&SURDO_HIGH[0], freq);
            let high_tails: Vec<f64> = SURDO_HIGH[1].iter().map(|l| db(l - 20.0)).collect();
            let tail_rings: Vec<f64> = SURDO_HIGH[3].iter().map(|r| r * (1.0 - 0.6 * mute)).collect();
            let head = modes(mallet.clone(), &open_hz, &open_levels, &open_rings)
                + modes(mallet.clone(), &muted_hz, &muted_levels, &muted_rings)
                + modes(mallet.clone(), &high_hz, &amps(&SURDO_HIGH[1]), &SURDO_HIGH[2])
                + modes(mallet, &high_hz, &high_tails, &tail_rings);
            let level = 0.05 * (vel / 0.6).powf(1.6);
            Note::rings((head >> high_shelf(800.0, (vel - 0.6) * 6.0)) * level as f32, 3.6)
        })
}

const SURDO_MUTED_LEVEL: f64 = -6.0;

/// One stroke of the cuíca, as reinsamba's pack "Cuica" on Freesound has them: the low strokes, with the head free, at note 65, and the high ones, with it pressed, at note 79, between which every figure moves with the note.
/// The membrane leaves nearly a pure tone of the stick's saw-toothed rub, its harmonics in dB against the fundamental.
const CUICA_NOTES: [f64; 2] = [65.0, 79.0];
const CUICA_HARMONICS: [[f64; 8]; 2] = [[0.0, -15.0, -30.0, -35.0, -51.0, -49.0, -58.0, -58.0], [0.0, -25.0, -28.0, -41.0, -48.0, -56.0, -64.0, -71.0]];
/// The cents under the note the stroke starts at, and the seconds it scoops up into it.
const CUICA_SCOOPS: [[f64; 2]; 2] = [[198.0, 0.11], [140.0, 0.065]];
/// The stroke's level and the seconds between, a 30 ms rise and a hold that the low strokes keep longer than the high ones.
const CUICA_LEVELS: [[f64; 6]; 2] = [[0.14, 0.9, 1.0, 0.56, 0.18, 0.0], [0.16, 0.8, 1.0, 0.4, 0.25, 0.0]];
const CUICA_TIMES: [[f64; 5]; 2] = [[0.03, 0.06, 0.06, 0.05, 0.015], [0.03, 0.02, 0.06, 0.055, 0.015]];

fn cuica() -> Synth {
    Synth::new("cuica")
        .options(strike_options(CUICA_NOTES[0]))
        .build(move |o, _ctx| {
            let vel = vel(o, 0.0);
            let note = o.number("note");
            let high = ((note - CUICA_NOTES[0]) / (CUICA_NOTES[1] - CUICA_NOTES[0])).clamp(0.0, 1.0);
            let mix = |pair: [f64; 2]| pair[0] + (pair[1] - pair[0]) * high;
            let [under, scoop] = [0, 1].map(|i| mix([CUICA_SCOOPS[0][i], CUICA_SCOOPS[1][i]]));
            let settled = hz(note);
            let pitch = shape(move |t| {
                let along = (t / scoop).clamp(0.0, 1.0);
                let risen = along * along * (3.0 - 2.0 * along);
                settled * 2f64.powf(-under * (1.0 - risen) / 1200.0)
            });
            let ratios: Vec<f64> = (1..=8).map(|n| n as f64).collect();
            let levels: Vec<f64> = (0..8).map(|i| db(mix([CUICA_HARMONICS[0][i], CUICA_HARMONICS[1][i]]) + (vel - 0.6) * 4.0 * (i as f64)).min(1.0)).collect();
            let tone = partials(pitch, &ratios, &levels, &spread_phases(8));
            let stroke: Vec<f64> = (0..6).map(|i| mix([CUICA_LEVELS[0][i], CUICA_LEVELS[1][i]])).collect();
            let times: Vec<f64> = (0..5).map(|i| mix([CUICA_TIMES[0][i], CUICA_TIMES[1][i]])).collect();
            let length: f64 = times.iter().sum();
            let envelope = segments(&stroke, &times, &[0.0; 5]);
            let level = CUICA_LEVEL * (vel / 0.6).powf(1.5);
            Note::rings(tone * envelope * level as f32, length + 0.01)
        })
}

/// The stroke's level, as loud as the first design's.
const CUICA_LEVEL: f64 = 0.05;

/// One stroke of the stick on the tamborim: its head rings on its lowest mode, 524 Hz in the Commons recording, with strong modes near 1.8 and 2.5 times it, the second outlasting the first; two quiet modes below it and higher modes with the stick's crack fill the bands from 160 Hz to 10 kHz.
/// Each mode is a ratio to the lowest, a level in dB and the seconds it fades 60 dB in.
const TAMBORIM: [[f64; 13]; 3] = [
    [0.38, 0.55, 1.0, 1.8, 2.5, 3.2, 4.0, 5.0, 6.3, 8.0, 10.0, 13.0, 17.0],
    [-26.0, -17.0, 0.0, -2.0, -3.5, -8.0, -9.0, -10.0, -11.0, -15.0, -18.0, -18.0, -22.0],
    [0.68, 0.58, 0.45, 0.85, 0.9, 0.85, 0.75, 0.7, 0.7, 0.6, 0.5, 0.45, 0.4],
];
const TAMBORIM_CRACK: f64 = -12.0;

fn tamborim() -> Synth {
    Synth::new("tamborim")
        .options(strike_options(72.03))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            let freq = hz(o.number("note"));
            let stick = impulse_net() + (noise(ctx) >> hpf(2000.0)) * hit(0.0003, 0.004, -4.0) * 0.3;
            let head = modes(stick, &scaled(&TAMBORIM[0], freq), &amps(&TAMBORIM[1]), &TAMBORIM[2]);
            let crack = (noise(ctx) >> hpf(3000.0) >> lpf(12000.0)) * hit(0.0005, 0.03, -4.0) * db(TAMBORIM_CRACK) as f32;
            let level = 0.02 * (vel / 0.6).powf(1.4);
            Note::rings((head + crack >> high_shelf(2000.0, (vel - 0.6) * 6.0)) * level as f32, 1.4)
        })
}

/// The caxixi's bands from 400 Hz: their lowest edges, levels in dB, and each shake's envelope, a fast fall over its first tens of milliseconds plus a quieter tail, as gain in dB and the two time constants.
const CAXIXI: [[f64; 8]; 5] = [
    [400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0, 10000.0],
    [-36.0, -28.0, -14.5, -5.8, -5.4, -3.6, 1.5, -4.0],
    [0.027, 0.027, 0.027, 0.026, 0.036, 0.034, 0.022, 0.019],
    [-15.0, -15.0, -15.0, -16.7, -23.5, -28.0, -26.2, -28.5],
    [0.9, 0.9, 0.9, 0.42, 0.44, 0.41, 0.21, 0.33],
];
const CAXIXI_PEAK: f64 = 8.0;

fn caxixi() -> Synth {
    let shakes = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    Synth::new("caxixi")
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is shaken."))
        .build(move |o, ctx| {
            let vel = vel(o, 0.0);
            // Each shake throws the seeds differently, so its strongest peak moves, as the recording's does from 1.7 to 9.6 kHz.
            let shake = shakes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let seed = ctx.seed() ^ shake.wrapping_mul(0x9e37_79b9_7f4a_7c15);
            let mut draws = Draws::new(seed);
            let top = 1734.0 * (9556.0f64 / 1734.0).powf(draws.draw().powf(1.6));
            let mut seeds = Net::wrap(Box::new(zero()));
            for band in 0..8 {
                let low = CAXIXI[0][band];
                let high = if band == 7 { 16000.0 } else { CAXIXI[0][band + 1] };
                let centre = (low * high).sqrt();
                let (fast, tail, slow) = (CAXIXI[2][band], db(CAXIXI[3][band]), CAXIXI[4][band]);
                let envelope = shape(move |t| (t / 0.002).min(1.0) * ((-t / fast).exp() + tail * (-t / slow).exp()));
                seeds = seeds + (white_noise(seed.wrapping_add(band as u64)) >> bpf(centre, (high - low) / centre)) * envelope * db(CAXIXI[1][band]) as f32;
            }
            let level = 0.3 * (vel / 0.6).powf(1.2);
            Note::rings((seeds >> hpf(700.0) >> hpf(700.0) >> peak(top, 0.25, CAXIXI_PEAK)) * level as f32, 3.0)
        })
}

struct Bol {
    name: &'static str,
    note: f64,
    length: f64,
    ratios: &'static [f64],
    levels: &'static [f64],
    rings: &'static [f64],
    noise: [f64; 11],
    fades: [f64; 11],
    gain: f64,
    high: f64,
}

const BOLS: [Bol; 6] = [
    Bol {
        name: "na",
        note: 63.0,
        length: 2.0,
        ratios: &[1.0, 1.789, 2.6931, 3.5927, 3.6525, 4.3152, 4.6887],
        levels: &[-20.2, -25.0, 0.0, -25.9, -4.8, -19.3, -3.1],
        rings: &[0.166, 1.291, 0.417, 0.312, 0.312, 0.123, 0.123],
        noise: [-90.0, -90.0, -90.0, -90.0, -90.0, -90.0, 8.2, 1.5, -19.9, -26.1, -38.7],
        fades: [0.3, 2.02, 0.166, 1.291, 0.417, 0.315, 0.166, 0.216, 0.407, 0.285, 0.258],
        gain: 0.077,
        high: 2900.0,
    },
    Bol {
        name: "tun",
        note: 63.0,
        length: 3.0,
        ratios: &[1.0108, 1.7922, 2.7313],
        levels: &[0.0, -27.7, -22.8],
        rings: &[2.332, 2.006, 0.502],
        noise: [-90.0, -90.0, -90.0, -90.0, -90.0, -4.0, -8.0, -20.3, -24.2, -26.8, -30.0],
        fades: [13.236, 2.553, 2.332, 2.01, 0.503, 0.479, 0.66, 1.334, 0.777, 1.104, 0.43],
        gain: 0.162,
        high: 2900.0,
    },
    Bol {
        name: "te",
        note: 63.0,
        length: 0.6,
        ratios: &[0.3757, 0.6049, 0.7405, 0.9951, 1.1908, 1.7359],
        levels: &[-2.4, 0.0, -2.8, -5.3, -2.2, -8.5],
        rings: &[0.14, 0.11, 0.14, 0.114, 0.133, 0.128],
        noise: [-90.0, -90.0, -90.0, -90.0, 17.2, 21.0, 17.2, 13.0, 10.1, 5.7, -8.8],
        fades: [0.451, 0.21, 0.13, 0.128, 0.173, 0.121, 0.136, 0.124, 0.102, 0.106, 0.143],
        gain: 0.043,
        high: 2900.0,
    },
    Bol {
        name: "tas",
        note: 63.0,
        length: 4.0,
        ratios: &[1.0002, 1.2098, 1.7796, 2.696, 3.557, 3.6364],
        levels: &[-1.5, 6.7, 12.7, 3.2, 5.2, 14.5],
        rings: &[0.34, 0.129, 2.027, 0.375, 0.261, 0.256],
        noise: [-90.0, -90.0, 53.5, -90.0, 48.0, -90.0, 23.7, 13.9, 8.1, -2.0, -4.0],
        fades: [10.448, 2.321, 0.12, 2.027, 0.1, 0.259, 0.465, 0.546, 0.248, 0.359, 0.299],
        gain: 0.0133,
        high: 2900.0,
    },
    Bol {
        name: "ghe",
        note: 42.25,
        length: 5.0,
        ratios: &[1.0, 1.8376, 2.8494, 3.0246, 4.0406, 4.9808],
        levels: &[0.0, -28.8, -28.2, -21.0, -36.7, -24.1],
        rings: &[3.528, 5.735, 1.599, 1.599, 0.874, 0.553],
        noise: [-90.0, -90.0, -90.0, -90.0, 3.1, -5.5, -17.9, -25.6, -31.3, -38.9, -40.8],
        fades: [3.528, 5.727, 1.629, 0.553, 0.433, 0.453, 1.63, 10.221, 12.0, 11.327, 12.0],
        gain: 0.14,
        high: 3700.0,
    },
    Bol {
        name: "ke",
        note: 44.75,
        length: 0.1,
        ratios: &[1.0, 1.8376, 2.8494, 4.0406],
        levels: &[0.0, -7.1, -10.0, -6.6],
        rings: &[0.06, 0.05, 0.05, 0.04],
        noise: [-90.0, -90.0, -90.0, -90.0, 18.9, 10.3, 4.1, -2.3, -5.4, -12.6, -23.2],
        fades: [0.05; 11],
        gain: 0.081,
        high: 2900.0,
    },
];

/// The tabla, whose `bol` option picks the stroke; `fixed` makes one of the old names that each played one stroke.
/// Each stroke has its own default note, so a note given to the tabla is the stroke's pitch, as with the old names.
fn tabla(name: &str, fixed: Option<&'static str>) -> Synth {
    let mut decl = Synth::new(name).option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is struck."));
    match fixed {
        Some(bol) => {
            let b = BOLS.iter().find(|b| b.name == bol).expect("a bol");
            decl = decl.option(Spec::note("note").default(b.note).doc("The stroke's pitch."));
        }
        None => {
            let names: Vec<&str> = BOLS.iter().map(|b| b.name).collect();
            decl = decl
                .option(Spec::name("bol", &names).default("na").doc("The stroke: na, tun, te and tas on the dayan, ghe and ke on the bayan."))
                .option(Spec::note("note").doc("The stroke's pitch; each bol has its own when not given."));
        }
    }
    decl.build(move |o, ctx| {
        let picked = fixed.unwrap_or_else(|| o.name("bol"));
        let b = BOLS.iter().find(|b| b.name == picked).expect("a bol the option allows");
        let note = if fixed.is_some() { o.number("note") } else { o.given("note").unwrap_or(b.note) };
        let vel = vel(o, 0.0);
        let freq = hz(note);
        let finger = impulse_net() >> lpf(3000.0);
        // Ghe's pitch glides with its note option, as a palm pressing the bayan's head bends it.
        let gliding = b.name == "ghe" && (fixed.is_some() || o.given("note").is_some());
        let pitch = if gliding { o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32)) } else { net(dc(freq as f32)) };
        let head = if gliding {
            let levels = amps(b.levels);
            let bank = b.ratios.iter().zip(&levels).zip(b.rings).map(|((&ratio, &level), &ring)| {
                net(map(move |x: &Frame<f32, U2>| -> Frame<f32, U2> { [x[0], x[1] * ratio as f32].into() })) >> struck_gliding(level, ring)
            });
            (finger | pitch.clone()) >> bank.reduce(|a, b| a & b).expect("a bol has modes")
        } else {
            modes(finger, &scaled(b.ratios, freq), &amps(b.levels), b.rings)
        };
        let centres = [113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, b.high, 5019.0, 7937.0, 12649.0];
        let mut hiss = Net::wrap(Box::new(zero()));
        for (i, centre) in centres.iter().enumerate() {
            if b.noise[i] <= -90.0 {
                continue;
            }
            let at = (pitch.clone() * (centre / hz(b.note)) as f32) >> net(map(|x: &Frame<f32, U1>| x[0].clamp(20.0, 20_000.0)));
            let narrow = |sound: Net| (sound | at.clone() | net(dc(1.0 / 0.35))) >> (net(bandpass::<f32>()) * 0.35);
            let band = narrow(narrow(narrow(narrow(noise(ctx)))));
            let fall = segments(&[0.0, 1.0], &[0.001], &[0.0]) * exp_segments(&[1.0, 1.0, 0.001], &[0.001, b.fades[i]]);
            hiss = hiss + band * fall * db(b.noise[i]) as f32;
        }
        let sound = (head + hiss) >> high_shelf(1500.0, (vel - 0.6) * 5.0);
        Note::rings(sound * (b.gain * (vel / 0.6).powf(1.5)) as f32, b.length)
    })
}
