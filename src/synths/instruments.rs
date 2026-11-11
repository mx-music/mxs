//! mx's instruments measured from recordings, after their designs in mx's priv/synthdefs/mx-instruments.scd: plucked strings, winds and the organ.

use crate::nodes::{between, db, decay2, hit, hz, modes, pink_noise, segment_at, segments, shape, white_noise};
use crate::{Context, Note, Options, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;

pub fn all() -> Vec<Synth> {
    vec![
        guitar(),
        plucked("cavaquinho", 0.1, 1.2, 1.05, &[420.0, 1100.0], &[1.0]),
        plucked("bandolim", 0.12, 1.6, 1.1, &[380.0, 900.0], &[1.0, 1.0025]),
        flute(),
        clarinet(),
        oboe(),
        bassoon(),
        tenor_sax(),
        brass_wind("trumpet", 4.9, 40, 164.8, &[2.0, 6.0, 11.0, 19.0, 23.0, 28.0, 30.0, 28.0, 22.0, 12.0, -2.0, -15.0], -12.0, Brass::Trumpet),
        brass_wind("trombone", 4.95, 64, 82.4, &[5.0, 12.0, 18.0, 21.0, 20.0, 24.0, 25.0, 26.0, 16.0, 6.0, -5.0, -18.0, -25.0], -12.0, Brass::Trombone),
        brass_wind("horn", 5.0, 64, 65.4, &[7.0, -6.0, 8.0, 18.0, 22.0, 25.0, 33.0, 14.0, 2.0, -14.0, -27.0, -32.0, -59.0], -9.0, Brass::Horn),
        brass_wind("tuba", 5.0, 64, 43.65, &[-2.0, 5.0, 23.0, 17.0, 24.0, 17.0, 12.0, 9.0, -4.0, -12.0, -26.0, -43.0, -57.0], -13.0, Brass::Tuba),
        organ(),
        marimba(),
        vibraphone(),
        xylophone(),
        glockenspiel(),
        harp(),
        timpani(),
        sitar(),
        harmonium(),
        tanpura(),
        bowed("violin", Bowed { lowest: 196.0, body: &[-2.0, 12.0, 20.0, 19.0, 18.0, 14.0, 11.0, 15.0, 10.0, -3.0, -10.0, -21.0], top: -19.0, dark_from: 300.0, dark: 5.0, swell: (1.1, 0.75), bow: -23.0, peak: 333.0, rise: 18.0, soft_rise: 6.0, fall: 12.0, soft_fall: 3.0, fade: 2.1, fade_slope: -0.55, pluck: 0.13, gain: 0.6, count: 32, dark_power: 2.5, level: 0.0 }),
        bowed("viola", Bowed { lowest: 130.8, body: &[2.0, 20.0, 36.0, 31.0, 25.0, 30.0, 29.0, 25.0, 26.0, 13.0, 2.0, -12.0, -26.0], top: -10.0, dark_from: 400.0, dark: 2.0, swell: (0.95, 0.45), bow: -15.0, peak: 350.0, rise: 17.0, soft_rise: 3.0, fall: 14.0, soft_fall: 4.5, fade: 3.7, fade_slope: -0.7, pluck: 0.13, gain: 0.3, count: 32, dark_power: 2.5, level: -15.0 }),
        bowed("cello", Bowed { lowest: 65.4, body: &[0.0, 14.0, 19.0, 20.0, 17.0, 15.0, 12.0, 15.0, 12.0, 10.0, 7.0, -3.0, -18.0, -28.0], top: -10.0, dark_from: 300.0, dark: 4.0, swell: (0.95, 0.3), bow: -23.0, peak: 144.0, rise: 14.0, soft_rise: 3.0, fall: 10.0, soft_fall: 6.5, fade: 4.0, fade_slope: -0.15, pluck: 0.13, gain: 0.6, count: 32, dark_power: 2.5, level: 0.0 }),
        bowed("double_bass", Bowed { lowest: 41.2, body: &[0.0, 20.0, 28.0, 26.0, 21.0, 17.0, 10.0, 2.0, -6.0, -4.0, -8.0, -14.0, -18.0, -27.0, -30.0], top: -10.0, dark_from: 70.0, dark: 3.0, swell: (0.45, 0.18), bow: -23.0, peak: 84.0, rise: 24.0, soft_rise: 5.0, fall: 19.5, soft_fall: 2.0, fade: 3.9, fade_slope: 0.0, pluck: 0.13, gain: 0.6, count: 64, dark_power: 1.5, level: 0.0 }),
    ]
}

/// A sine at `hz`, starting `phase` radians in.
pub(crate) fn sine_from(hz: f64, phase: f64) -> Net {
    net(dc(hz as f32)) >> net(An(Sine::<f32>::with_phase((phase / (2.0 * PI)).rem_euclid(1.0) as f32)))
}

/// The seconds until resonators of `levels`, fading 60 dB over `rings`, all fall below 80 dB under full scale once scaled by `gain`, as scsynth would free them.
pub(crate) fn silent_after(levels: &[f64], rings: &[f64], gain: f64) -> f64 {
    levels
        .iter()
        .zip(rings)
        .filter(|(l, _)| l.abs() * gain > 1e-4)
        .map(|(l, r)| r * 20.0 * (l.abs() * gain / 1e-4).log10() / 60.0)
        .fold(0.1, f64::max)
}

pub(crate) fn net(unit: impl AudioUnit + 'static) -> Net {
    Net::wrap(Box::new(unit))
}

/// A band-pass with a peak of 1, whose width is `rq` times its centre.
pub(crate) fn bpf(f: f64, rq: f64) -> Net {
    net(bandpass_hz(f.clamp(20.0, 20_000.0) as f32, (1.0 / rq) as f32)) * rq as f32
}

/// The level in dB at each of `hz` of a curve given every half octave up from `lowest`, then falling by `top` over one more octave.
pub(crate) fn shaped(hz: &[f64], lowest: f64, body: &[f64], top: f64) -> Vec<f64> {
    let mut slopes: Vec<f64> = body.windows(2).map(|w| (w[1] - w[0]) * 2.0).collect();
    slopes.push(top);
    let bends: Vec<f64> = slopes.windows(2).map(|w| w[1] - w[0]).collect();
    let most = (body.len() - 1) as f64 * 0.5 + 1.0;
    hz.iter()
        .map(|&f| {
            let octaves = (f / lowest).log2().clamp(0.0, most);
            body[0]
                + octaves * slopes[0]
                + bends.iter().enumerate().map(|(i, bend)| bend * (octaves - (i + 1) as f64 * 0.5).max(0.0)).sum::<f64>()
        })
        .collect()
}

/// Sines at `ratios` times the pitch its input gives, each at its level and starting phase in radians, as one sound.
pub(crate) fn partials(pitch: Net, ratios: &[f64], levels: &[f64], phases: &[f64]) -> Net {
    let kept: Vec<(f32, f32, f32)> = ratios
        .iter()
        .zip(levels)
        .zip(phases)
        .filter(|((_, l), _)| **l != 0.0)
        .map(|((r, l), p)| (*r as f32, *l as f32, (*p / (2.0 * PI)).rem_euclid(1.0) as f32))
        .collect();
    pitch >> net(An(Partials { start: kept.iter().map(|k| k.2).collect(), ratios: kept.iter().map(|k| (k.0, k.1)).collect(), phases: kept.iter().map(|k| k.2).collect(), rate: DEFAULT_SR as f32 }))
}

/// The phases mx's additive designs start their harmonics at, so they do not all peak together.
pub(crate) fn spread_phases(count: usize) -> Vec<f64> {
    (1..=count).map(|n| PI * n as f64 * (n as f64 - 1.0) / 24.0).collect()
}

fn harmonics(count: usize) -> Vec<f64> {
    (1..=count).map(|n| n as f64).collect()
}

/// The note's frequency, gliding with its note option, swung by a vibrato `depth` semitones deep at `rate` that grows in as the note holds.
fn vibrato(o: &Options, depth: f64, rate: f64) -> Net {
    vibrato_after(o, depth, rate, 0.25, 1.2)
}

/// A vibrato that waits `wait` seconds, then grows in over `grow`.
fn vibrato_after(o: &Options, depth: f64, rate: f64, wait: f64, grow: f64) -> Net {
    let grows = segments(&[0.0, 0.0, 1.0], &[wait, grow], &[2.0, 2.0]);
    let swing = sine_from(rate, 0.0) * grows * depth as f32;
    (o.follow("note") | swing) >> net(map(|x: &Frame<f32, U2>| (hz(x[0] as f64) * 2f64.powf(x[1] as f64 / 12.0)) as f32))
}

/// A rise to full over `seconds`, though never over more than part of the note's sustain.
fn swell(o: &Options, seconds: f64) -> Net {
    segments(&[0.0, 1.0], &[seconds.min(o.number("sustain") * 0.4).max(1e-4)], &[-2.0])
}

fn breath(ctx: &mut Context, centre: f64, level: f64, vel: f64) -> Net {
    (white_noise(ctx.seed()) >> bpf(centre, 1.0)) * (db(level) * vel * vel) as f32
}

fn audible(levels: Vec<f64>, freq: f64, ratios: &[f64], below: f64) -> Vec<f64> {
    levels.iter().zip(ratios).map(|(l, r)| if freq * r < below { db(*l) } else { 0.0 }).collect()
}

fn wind_options(name: &str, vibrato: f64) -> crate::SynthDecl {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is blown."))
        .option(Spec::float("vibrato").default(vibrato).range(0.0, 2.0).doc("The vibrato's depth in semitones, reached as the note holds."))
        .options(Spec::envelope(0.5, 0.2))
}

fn guitar() -> Synth {
    Synth::new("guitar")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the string is plucked."))
        .options(Spec::envelope(1000.0, 2.0))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let stretched: Vec<f64> = (1..=48).map(|n| n as f64 * (1.0 + 0.00009 * (n * n) as f64).sqrt()).collect();
            let octave = (freq / 1000.0).log2();
            let line = |value: f64, at: f64, slope: f64| -> Vec<f64> {
                let base = value + slope * (octave - (at / 1000.0).log2());
                stretched.iter().map(|s| base + slope * s.log2()).collect()
            };
            let zip = |a: Vec<f64>, b: Vec<f64>, f: fn(f64, f64) -> f64| -> Vec<f64> { a.into_iter().zip(b).map(|(x, y)| f(x, y)).collect() };
            let mf = zip(zip(line(-5.2, 120.0, 4.9), zip(line(-8.4, 320.0, -13.8), line(-35.0, 2000.0, -9.0), f64::max), f64::min), line(-43.0, 3200.0, -25.0), f64::min);
            let spread: Vec<f64> = line(11.0, 120.0, 1.9).into_iter().map(|x| x.clamp(11.0, 21.0)).collect();
            let levels: Vec<f64> = (0..48)
                .map(|i| {
                    let pluck = ((i + 1) as f64 * PI * 0.17).sin().abs() + 0.03;
                    if freq * stretched[i] < 16_000.0 { db(mf[i] + spread[i] * ((vel - 0.6) / 0.7)) * pluck } else { 0.0 }
                })
                .collect();
            let fades = zip(
                zip(zip(line(4.3f64.log2(), 320.0, -0.72), line(5.6f64.log2(), 320.0, -0.53), f64::max), line(2.6f64.log2(), 800.0, -0.75), f64::min),
                line(3.6f64.log2(), 800.0, -1.25),
                f64::min,
            );
            let near: Vec<f64> = line(0.0, 200.0, 1.0);
            let drained: Vec<f64> = fades.iter().zip(&near).map(|(f, n)| 2f64.powf(*f) * (1.0 - 0.55 * (-8.0 * n * n).exp())).collect();
            let hz_: Vec<f64> = stretched.iter().map(|s| freq * s).collect();
            let fast = modes(net(impulse::<U1>()), &hz_, &levels, &drained);
            let slow_hz: Vec<f64> = hz_.iter().take(8).map(|f| f * 1.0004).collect();
            let slow_rings: Vec<f64> = drained.iter().take(8).map(|r| r * 1.6).collect();
            let slow_levels: Vec<f64> = levels.iter().take(8).map(|l| l * 0.25).collect();
            let slow = modes(net(impulse::<U1>()), &slow_hz, &slow_levels, &slow_rings);
            let pick = (white_noise(ctx.seed()) >> bpf(freq * 8.0, 1.2)) * segments(&[0.0, 1.0, 0.0], &[0.0005, 0.006], &[-4.0, -4.0]) * (vel * 0.2) as f32;
            // It falls silent where scsynth would free it, 80 dB down.
            let rings = levels
                .iter()
                .zip(&drained)
                .chain(slow_levels.iter().zip(&slow_rings))
                .filter(|(l, _)| **l * 0.075 > 1e-4)
                .map(|(l, r)| r * (20.0 * (l * 0.075 / 1e-4).log10()) / 60.0)
                .fold(0.1, f64::max);
            Note::envelope((fast + slow) * 0.075 + pick, o).within(rings)
        })
}

/// A plucked string: harmonics shaped by where it is plucked, the higher ones dying sooner, through the body's resonances; tremolo picks it again that many times a second.
fn plucked(name: &'static str, pick: f64, ring: f64, bright: f64, body: &'static [f64], courses: &'static [f64]) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the string is plucked."))
        .option(Spec::float("tremolo").default(0.0).range(0.0, f64::INFINITY).doc("How many times a second a held note is picked; 0 picks it once."))
        .options(Spec::envelope(0.0, 0.8))
        .build(move |o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let note = o.number("note");
            let freq = hz(note);
            let tremolo = o.number("tremolo");
            let since = move |t: f64| if tremolo > 0.0 { t % (1.0 / tremolo.max(0.01)) } else { t };
            let length = ring * 2f64.powf((60.0 - note) / 24.0);
            let mut strings = Net::wrap(Box::new(zero()));
            for n in 1..=16 {
                let n = n as f64;
                if freq * n >= 14_000.0 {
                    continue;
                }
                let level = ((n * PI * pick).sin().abs() + 0.02) * n.powf(-(bright + (1.0 - vel) * 0.8)) / courses.len() as f64;
                let fall = length / (1.0 + (n - 1.0) * 0.35);
                let fade = shape(move |t| segment_at(&[0.0, 1.0, 0.0], &[0.001, fall], &[-4.0, -4.0], since(t)));
                for detune in courses {
                    strings = strings + sine_from(freq * n * detune, 0.0) * fade.clone() * level as f32;
                }
            }
            let damp = shape(move |t| segment_at(&[1.0, 0.15, 1.0], &[0.004, 0.012], &[0.0, 0.0], since(t)));
            let attack_level = shape(move |t| segment_at(&[0.0, 1.0, 0.0], &[0.0005, 0.008], &[-4.0, -4.0], since(t)));
            let attack = (white_noise(ctx.seed()) >> bpf(freq * 6.0, 1.0)) * attack_level * (vel * 0.3) as f32;
            let sound = strings * damp + attack;
            let mut bodies = net(pass());
            for &f in body {
                bodies = bodies & (bpf(f, 0.4) * 0.5);
            }
            Note::envelope((sound >> bodies) * ((0.3 + vel * 0.7) * 0.3) as f32, o)
        })
}

fn flute() -> Synth {
    wind_options("flute", 0.25).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let soft = ((1.0 - vel) / 0.7).max(0.0);
        let ratios = harmonics(16);
        let register = (note - 59.0).clamp(0.0, 30.0);
        let fundamental = register * (0.25 + 0.75 * vel);
        let levels: Vec<f64> = ratios
            .iter()
            .map(|&n| {
                let overtone = if n == 1.0 { 0.0 } else { [1.0, 0.0, -8.0][Ord::min((n - 2.0) as usize, 2)] - 12.0 * (n / 4.0).log2().max(0.0) };
                fundamental + overtone + 0.05 * register * n.log2() - soft.powf(3.5) * 11.0 * n.log2()
            })
            .collect();
        let depth = o.number("vibrato") * vel.powf(0.65);
        let tone = partials(vibrato(o, depth, 5.0), &ratios, &audible(levels, freq, &ratios, 16_000.0), &spread_phases(16));
        let sound = (tone * 0.0106 + breath(ctx, 2000.0, -43.5, vel)) * swell(o, 0.25);
        Note::envelope(sound, o)
    })
}

fn clarinet() -> Synth {
    wind_options("clarinet", 0.0).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let soft = ((1.0 - vel) / 0.7).max(0.0);
        let ratios = harmonics(24);
        let upper = ((note - 68.0) / 3.0).clamp(0.0, 1.0);
        let top = ((note - 88.0) / 3.0).clamp(0.0, 1.0);
        let fundamental = (note - 50.0).clamp(0.0, 18.0) * (0.1 + 0.35 * soft) + upper * 12.0 - (note - 88.0).max(0.0) * 1.3;
        let levels: Vec<f64> = ratios
            .iter()
            .map(|&n| {
                let odd = (n as usize) % 2 == 1;
                let low = if n == 1.0 {
                    0.0
                } else if odd {
                    -3.0 - 3.0 * (n / 3.0).log2() - 15.0 * (n / 5.0).log2().max(0.0)
                } else {
                    -28.0 + (n / 4.0).log2().max(0.0)
                };
                let high = if n == 1.0 { 0.0 } else { -17.0 - 10.0 * (n / 2.0).log2() + if n == 2.0 { top * 12.0 } else { 0.0 } };
                fundamental + low * (1.0 - upper) + high * upper - soft.powi(3) * 8.0 * n.log2()
            })
            .collect();
        let tone = partials(vibrato(o, o.number("vibrato"), 5.0), &ratios, &audible(levels, freq, &ratios, 16_000.0), &spread_phases(24));
        let sound = (tone * 0.1 + breath(ctx, 1500.0, -30.0, vel)) * swell(o, 0.155 + 0.185 * vel.powi(3)) * vel.powf(1.4) as f32;
        Note::envelope(sound, o)
    })
}

/// A reed whose harmonics follow a curve of the body, darker above `dark_from` the softer it is played.
#[allow(clippy::too_many_arguments)]
fn reed(
    o: &Options,
    count: usize,
    rate: f64,
    lowest: f64,
    body: &[f64],
    top: f64,
    extra: impl Fn(f64, f64) -> f64,
) -> (Net, f64, f64) {
    let vel = o.number("vel").clamp(0.0, 1.0);
    let note = o.number("note");
    let freq = hz(note);
    let ratios = harmonics(count);
    let hz_: Vec<f64> = ratios.iter().map(|n| n * freq).collect();
    let curve = shaped(&hz_, lowest, body, top);
    let levels: Vec<f64> = curve.iter().zip(&hz_).map(|(c, f)| c + extra(*f, note)).collect();
    let tone = partials(vibrato(o, o.number("vibrato"), rate), &ratios, &audible(levels, freq, &ratios, 16_000.0), &spread_phases(count));
    (tone, vel, note)
}

fn oboe() -> Synth {
    wind_options("oboe", 0.0).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let soft = ((1.0 - vel) / 0.7).max(0.0);
        let body = [2.0, 0.0, 2.0, 2.0, 4.0, 14.0, -6.0, -10.0, -12.0, -33.0, -40.0];
        let (tone, vel, _) = reed(o, 24, 5.0, 233.1, &body, -20.0, |f, note| (note - 58.0) * 0.2 - soft.powf(2.5) * 8.0 * (f / 833.0).log2().max(0.0));
        let sound = (tone * 0.05 + breath(ctx, 1500.0, -36.0, vel)) * swell(o, 0.253 - 0.12 * vel.powi(3)) * vel as f32;
        Note::envelope(sound, o)
    })
}

fn bassoon() -> Synth {
    wind_options("bassoon", 0.0).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let soft = ((1.0 - vel) / 0.7).max(0.0);
        let body = [0.0, -4.0, 2.0, -1.0, 6.0, 9.0, 18.0, 6.0, -8.0, -10.0, -18.0, -28.0, -34.0, -43.0, -48.0];
        let (tone, vel, _) = reed(o, 32, 5.5, 58.27, &body, -10.0, |f, _| -(soft * soft * 6.0 * (f / 500.0).log2().max(0.0)));
        let speaks = (vel * (1.12 - 0.88 * vel) - 0.16).max(0.03);
        let sound = (tone * 0.025 + breath(ctx, 800.0, -42.0, vel)) * swell(o, speaks) * vel.powf(1.2) as f32;
        Note::envelope(sound, o)
    })
}

fn tenor_sax() -> Synth {
    wind_options("tenor_sax", 0.0).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let soft = ((1.0 - vel) / 0.7).max(0.0);
        let ratios = harmonics(32);
        let hz_: Vec<f64> = ratios.iter().map(|n| n * freq).collect();
        let body = [0.0, -6.0, -6.0, 4.0, 6.0, 0.0, -16.0, -16.0, -18.0, -22.0, -30.0, -40.0, -50.0];
        let curve = shaped(&hz_, 103.8, &body, -7.0);
        let short = (1.0 - (o.number("sustain") - 0.15) / 0.35).clamp(0.0, 1.0);
        let darkness: Vec<f64> = hz_.iter().map(|f| soft * 6.0 * (f / 600.0).log2().max(0.0)).collect();
        let held: Vec<f64> = (0..32).map(|i| curve[i] + (note - 44.0) * 0.1 - darkness[i]).collect();
        let onset: Vec<f64> = (0..32).map(|i| short * (20.0 * (hz_[i] / 400.0).log2().clamp(0.0, 1.0) + 0.5 * darkness[i])).collect();
        // The onset fades out over 0.4 s, so the tone is its held levels plus a brighter set that fades.
        let fading = segments(&[1.0, 0.0], &[0.4], &[-4.0]);
        let depth = o.number("vibrato") * 0.146;
        let steady = partials(vibrato(o, depth, 4.94), &ratios, &audible(held.clone(), freq, &ratios, 16_000.0), &spread_phases(32));
        let tone = if short > 0.0 {
            let bright: Vec<f64> = (0..32).map(|i| held[i] + onset[i]).collect();
            let brighter = partials(vibrato(o, depth, 4.94), &ratios, &audible(bright, freq, &ratios, 16_000.0), &spread_phases(32));
            let fade = fading.clone();
            steady * (net(dc(1.0)) - fade.clone()) + brighter * fade
        } else {
            steady
        };
        let sound = (tone * 0.07 + breath(ctx, 1500.0, -33.0, vel)) * swell(o, 0.15 - 0.1 * vel) * vel.powf(1.25) as f32;
        Note::envelope(sound, o)
    })
}

#[derive(Clone, Copy)]
enum Brass {
    Trumpet,
    Trombone,
    Horn,
    Tuba,
}

#[allow(clippy::too_many_arguments)]
fn brass_wind(name: &'static str, rate: f64, count: usize, lowest: f64, body: &'static [f64], top: f64, kind: Brass) -> Synth {
    wind_options(name, 0.0).build(move |o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let soft = ((0.6 - vel) / 0.3).max(0.0);
        let loud = ((vel - 0.6) / 0.4).max(0.0);
        let ratios = harmonics(count);
        let hz_: Vec<f64> = ratios.iter().map(|n| n * freq).collect();
        let curve = shaped(&hz_, lowest, body, top);
        let tilt = |f: f64| match kind {
            Brass::Trumpet => -soft * 20.0 * (f / 1200.0).log2().max(0.0) + loud * 4.0 * (f / 2000.0).log2().max(0.0),
            Brass::Trombone => -soft.powf(0.77) * 7.0 * (f / 278.0).log2().max(0.0) + loud * 16.0 * (f / 1000.0).log2().max(0.0),
            Brass::Horn => -soft.powf(0.77) * 4.0 * (f / 600.0).log2().max(0.0) + loud * 6.0 * (f / 600.0).log2().clamp(0.0, 1.5),
            Brass::Tuba => -soft * 6.0 * (f / 231.0).log2().max(0.0) + loud * 10.0 * (f / 360.0).log2().clamp(0.0, 3.5),
        };
        let mut levels = audible(curve.iter().zip(&hz_).map(|(c, f)| c + tilt(*f)).collect(), freq, &ratios, 16_000.0);
        // The horn's and the tuba's notes are levelled to their whole, then raised with the note as Iowa's are.
        let even = match kind {
            Brass::Horn => Some(db((note - 60.0).clamp(0.0, 12.0) * 0.6)),
            Brass::Tuba => Some(db((note - 48.0).clamp(0.0, 12.0) * 0.5)),
            _ => None,
        };
        if let Some(raise) = even {
            let whole = levels.iter().map(|l| l * l).sum::<f64>().sqrt().max(1e-12);
            levels.iter_mut().for_each(|l| *l *= raise / whole);
        }
        let tone = partials(vibrato(o, o.number("vibrato"), rate), &ratios, &levels, &spread_phases(count));
        let (gain, centre, air, speaks, power) = match kind {
            Brass::Trumpet => (0.004, 1500.0, -48.0, 0.3 - 0.156 * vel, 0.6),
            Brass::Trombone => (0.0017, 1000.0, -57.0, 0.37 - 0.13 * vel, 0.6),
            Brass::Horn => (0.3, 800.0, -50.0, 0.09 + 0.06 * vel, 1.25),
            Brass::Tuba => (0.21, 300.0, -53.0, 0.02 + 0.2 * vel, 2.1),
        };
        let sound = (tone * gain + breath(ctx, centre, air, vel)) * swell(o, speaks) * vel.powf(power) as f32;
        Note::envelope(sound, o)
    })
}

fn organ() -> Synth {
    Synth::new("organ")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("loud").default(0.0).range(0.0, 1.0).doc("1 plays the loud set, a principal chorus, instead of the quiet stopped flute."))
        .options(Spec::envelope(1.0, 0.15))
        .build(|o, ctx| {
            let loud = o.number("loud").clamp(0.0, 1.0);
            let note = o.number("note");
            let freq = hz(note);
            let key = note.max(36.0);
            let ratios = harmonics(24);
            let levels: Vec<f64> = ratios
                .iter()
                .map(|&n| {
                    let octave = n.log2();
                    let odd = (n as usize) % 2 == 1;
                    let flute = if odd { (-8.0 - 0.25 * (key - 36.0)) * octave } else { -45.0 } + if n == 3.0 { 10.0 * ((42.0 - key) / 6.0).max(0.0) } else { 0.0 };
                    let chorus = (4.0 - 0.4 * (key - 36.0)) * octave;
                    let slope = 18.0 - loud * 2.0;
                    let top = slope * (freq * n / freq.max(740.0 + loud * 220.0)).log2().max(0.0).min(27.0 / slope) + 30.0 * (freq * n / 5000.0).log2().max(0.0);
                    let quiet_level = (1.5 * (key - 45.0)).min(0.0);
                    let loud_level = -0.83 * (60.0 - key).max(0.0) - 0.25 * (note - 60.0).max(0.0) + 6.0;
                    let level = db(flute + quiet_level - top) * (1.0 - loud) + db(chorus + loud_level - top) * loud;
                    if freq * n < 16_000.0 { level } else { 0.0 }
                })
                .collect();
            let pitch = || o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let pipes = partials(pitch(), &ratios, &levels, &spread_phases(24));
            let loud_level = -0.83 * (60.0 - key).max(0.0) - 0.25 * (note - 60.0).max(0.0) + 6.0;
            let bourdon_level = if note >= 60.0 { loud * db(-13.0) * db(loud_level) } else { 0.0 };
            let bourdon = partials(pitch(), &[0.5, 1.5, 2.5], &[bourdon_level, 0.2 * bourdon_level, 0.05 * bourdon_level], &[0.0; 3]);
            let speak = segments(&[0.0, 1.0], &[0.12 + loud * 0.04], &[3.0]);
            let chiff = (white_noise(ctx.seed()) >> bpf(freq * 3.0, 0.5)) * segments(&[0.0, 1.0, 0.0], &[0.005, 0.06], &[-4.0, -4.0]) * 0.05;
            let wind = (pink_noise(ctx.seed()) >> bpf(freq * 2.0, 1.0)) * 0.005;
            Note::envelope((pipes + bourdon) * speak * 0.03 + chiff + wind, o)
        })
}

fn struck(name: &str, release: f64) -> crate::SynthDecl {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is struck."))
        .options(Spec::envelope(1000.0, release))
}

fn lpf(f: f64) -> Net {
    net(butterpass_hz(f as f32))
}

fn hpf(f: f64) -> Net {
    net(highpass_hz(f as f32, std::f32::consts::FRAC_1_SQRT_2))
}

fn above(f: f64, corner: f64) -> f64 {
    (f / corner).log2().max(0.0)
}

fn marimba() -> Synth {
    struck("marimba", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let ring = 4.6 * 2f64.powf((note - 36.0) / -17.0) * 2f64.powf((note - 78.0).max(0.0) / -16.0);
        let swell = 0.45 * 2f64.powf((note - 36.0) / -18.0);
        let ratios = [1.0, 1.0, 4.0, 10.0, 19.0, 30.0, 3.0, 5.0, 7.0, 9.0];
        let mallet = |f: f64, corner: f64, slope: f64| slope * above(f, corner) + 24.0 * above(f, 3000.0);
        let fourth = -12.0 - mallet(freq * 4.0, 600.0, 57.0 * (1.0 - vel)) - 10.0 * above(freq * 4.0, 900.0) - 20.0 * (370.0 / (freq * 4.0)).log2().max(0.0);
        let overtone = |n: f64| 10.0 + 14.5 * vel.log2() - mallet(freq * n, 1100.0, 33.4 - 11.4 * vel);
        let low_tube = -22.0 - 60.0 * (((note < 46.0) as u8 + (note > 62.0) as u8) as f64);
        let tube = [(3.0, low_tube), (5.0, -26.0), (7.0, -26.0), (9.0, -26.0)].map(|(n, l)| l - mallet(freq * n, 600.0, 12.0 + 45.0 * (1.0 - vel)));
        let mut levels = vec![1.25, -1.0];
        levels.extend([fourth, overtone(10.0), overtone(19.0) - 8.0, overtone(30.0) - 12.0].into_iter().chain(tube).map(db));
        let levels: Vec<f64> = levels.iter().zip(ratios).map(|(l, r)| if freq * r < 15_000.0 { *l } else { 0.0 }).collect();
        let mut fades = vec![ring, swell];
        fades.extend([0.3, 0.1, 0.08, 0.06, 0.35, 0.35, 0.35, 0.35].iter().zip(&ratios[2..]).map(|(k, n)| ring * k * (1500.0 / (freq * n)).min(1.0).powf(0.8)));
        let hz_: Vec<f64> = ratios.iter().map(|r| freq * r).collect();
        let bar = modes(net(impulse::<U1>()), &hz_, &levels, &fades);
        let seed = ctx.seed();
        let low_tap = (white_noise(seed) >> lpf(2200.0) >> lpf(2200.0) >> lpf(2200.0) >> hpf(1000.0)) * hit(0.0005, 0.12, -4.0) * 0.55;
        let high_tap = (white_noise(seed + 1) >> lpf(4000.0) >> lpf(4000.0) >> hpf(2500.0)) * hit(0.0005, 0.05, -4.0) * 0.3;
        let gain = 0.1 * (vel / 0.6).powf(1.9);
        let sound = (bar + (low_tap + high_tap) * (vel / 0.6).sqrt() as f32) * gain as f32;
        Note::envelope(sound, o).within(silent_after(&levels, &fades, gain))
    })
}

fn vibraphone() -> Synth {
    struck("vibraphone", 0.3)
        .option(Spec::float("motor").default(0.0).range(0.0, f64::INFINITY).doc("How many times a second the discs in the tubes pulse the note; 0 leaves the motor off."))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.05, 1.0);
            let note = o.number("note");
            let freq = hz(note);
            let motor = o.number("motor");
            let ring = 13.0;
            let swell = 0.45 * 2f64.powf((note - 36.0) / -18.0);
            let ratios = [4.0, 10.0, 16.0];
            let slope = 20.0 * (1.0 - vel);
            let mallet = |f: f64| slope * above(f, 1120.0) + 16.0 * above(f, 3080.0);
            let overtones: Vec<f64> = [-20.0, 12.0, -16.0].iter().zip(ratios).map(|(l, r)| if freq * r < 15_000.0 { db(l + 2.0 * vel.log2() - mallet(freq * r)) } else { 0.0 }).collect();
            let fades: Vec<f64> = [3.5, 1.35, 2.5].iter().zip(ratios).map(|(k, n)| k * (1500.0 / (freq * n)).min(1.0).powf(0.8)).collect();
            let pulse = shape(move |t| if motor > 0.0 { 1.0 - 0.25 * (1.0 - (2.0 * PI * motor * t).cos()) } else { 1.0 });
            let tone = modes(net(impulse::<U1>()), &[freq, freq], &[1.25, -1.0], &[ring, swell]) * pulse;
            let bar = modes(net(impulse::<U1>()), &ratios.map(|r| freq * r), &overtones, &fades);
            let harmonics = [2.0, 3.0].map(|n| db(-21.0 - 6.0 * above(freq * n, 400.0)));
            let tube_fades = [2.0, 3.0].map(|n| ring * 0.74 * (800.0 / (freq * n)).min(1.0));
            let tube = modes(net(impulse::<U1>()), &[freq * 2.0, freq * 3.0], &harmonics, &tube_fades);
            let tap = (white_noise(ctx.seed()) >> lpf(4000.0) >> lpf(4000.0) >> hpf(1500.0)) * hit(0.0005, 0.06, -4.0) * (0.45 * (vel / 0.6).powf(-1.1)) as f32;
            let gain = 0.1 * (vel / 0.6).powf(1.9);
            let all: Vec<f64> = [1.25].iter().chain(&overtones).chain(&harmonics).copied().collect();
            let rings: Vec<f64> = [ring].iter().chain(&fades).chain(&tube_fades).copied().collect();
            Note::envelope((tone + bar + tube + tap) * gain as f32, o).within(silent_after(&all, &rings, gain))
        })
}

fn xylophone() -> Synth {
    struck("xylophone", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let ring = 1.45 * 2f64.powf((note - 65.0) / -10.0);
        let ratios = [1.0, 2.0, 3.0, 6.39];
        let shaped_levels = [0.0, -32.0 - 15.0 * above(freq * 2.0, 700.0), -7.0 - 10.0 * above(freq * 3.0, 1000.0), 12.0 - 10.0 * above(freq * 6.39, 3000.0)];
        let levels: Vec<f64> = (0..4).map(|i| if freq * ratios[i] < 15_000.0 { db(shaped_levels[i] + [0.0, -10.0, -1.0, 9.0][i] * vel.log2()) } else { 0.0 }).collect();
        let fades = [1.0, 0.23, 0.3, 0.1].map(|k| ring * k);
        let bar = modes(net(impulse::<U1>()), &ratios.map(|r| freq * r), &levels, &fades);
        let centre = (4000.0 * (vel / 0.6).powf(0.6).min(1.0)).clamp(500.0, 12_000.0);
        let click_level = 15.0 * 2f64.powf((note - 79.0).max(0.0) / -5.5) * (vel / 0.6).powf(1.2);
        let click = (white_noise(ctx.seed()) >> bpf(centre, 0.25)) * hit(0.0005, 0.04, -4.0) * click_level as f32;
        let gain = 0.015 * (vel / 0.6).powf(1.9);
        Note::envelope((bar + click) * gain as f32, o).within(silent_after(&levels, &fades, gain).max(0.05))
    })
}

fn glockenspiel() -> Synth {
    struck("glockenspiel", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let step = note - 72.0;
        let ring = 12.2 * 2f64.powf((note - 90.0).max(0.0) / -9.0);
        let ratios = [1.0, 3.3 - 0.016 * step, 7.08 - 0.0486 * step - 0.00117 * step * step, 11.32 - 0.296 * step + 0.00203 * step * step];
        let mallet = |f: f64| 17.0 * (1.0 - vel) * above(f, 2800.0);
        let shaped_levels = [0.0, -39.0 + 2.0 * step.min(16.0) - (step - 16.0).max(0.0), 12.0 - 0.2 * step, -8.0 - 3.0 * (step - 19.0).max(0.0)];
        let levels: Vec<f64> = (0..4)
            .map(|i| {
                let struck = if i == 0 { 0.0 } else { 2.0 * vel.log2() - mallet(freq * ratios[i]) };
                if freq * ratios[i] < 18_000.0 { db(shaped_levels[i] + struck) } else { 0.0 }
            })
            .collect();
        let fades: Vec<f64> = [ring, 1.0, 0.29, 1.1].iter().zip(ratios).map(|(k, r)| k * (4000.0 / (freq * r)).min(1.0).powf(0.8)).collect();
        let bar = modes(net(impulse::<U1>()), &ratios.map(|r| freq * r), &levels, &fades);
        let click = (white_noise(ctx.seed()) >> bpf(1750.0, 0.5)) * hit(0.0005, 0.036, -4.0) * (2.0 * (vel / 0.6).powf(-1.4)) as f32;
        let gain = 0.02 * (vel / 0.6).powf(1.9);
        Note::envelope((bar + click) * gain as f32, o).within(silent_after(&levels, &fades, gain))
    })
}

fn harp() -> Synth {
    struck("harp", 2.0).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.0, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let stiffness = 0.00024 * 2f64.powf((note - 60.0) / 17.7);
        let hz_: Vec<f64> = (1..=48).map(|n| freq * n as f64 * (1.0 + stiffness * (n * n) as f64).sqrt()).collect();
        let loud = (vel - 1.0) / 0.7;
        let levels: Vec<f64> = hz_
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let octaves = (f / 250.0).log2();
                let level = 20.0 + octaves.min(0.0) * 13.0 - octaves.max(0.0) * 9.0 + loud * (4.5 + 3.5 * octaves.max(0.0));
                let pluck = ((i + 1) as f64 * PI * 0.48).sin().abs() + 0.05;
                if *f < 16_000.0 { db(level) * pluck } else { 0.0 }
            })
            .collect();
        let fades: Vec<f64> = hz_.iter().map(|f| { let octaves = (f / 250.0).log2(); 15.6 * 2f64.powf(octaves.max(0.0) * -1.15 + octaves.min(0.0) * 0.3) }).collect();
        let string = modes(net(impulse::<U1>()), &hz_, &levels, &fades);
        let noise = (white_noise(ctx.seed()) >> bpf(freq * 6.0, 1.2)) * hit(0.0005, 0.006, -4.0) * (vel * 0.2) as f32;
        let gain = vel.powf(0.6);
        Note::envelope((string * 0.01 + noise) * gain as f32, o).within(silent_after(&levels, &fades, 0.01 * gain))
    })
}

fn timpani() -> Synth {
    struck("timpani", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let freq = hz(o.number("note"));
        let ratios = [1.0, 1.487, 1.70, 1.95, 2.11, 2.40, 2.70, 2.82, 3.18, 3.33, 3.70, 4.2, 4.7, 5.05, 0.75, 0.83, 0.885, 1.18];
        let base = [2.0, -13.0, -27.0, -21.0, -24.0, -27.0, -16.0, -14.0, -18.0, -19.0, -16.0, -18.0, -20.0, -22.0, -13.0, -8.0, -7.0, -20.0];
        let lengths = [2.1, 6.0, 5.0, 6.0, 6.0, 5.0, 4.0, 4.0, 3.5, 3.5, 3.0, 3.0, 2.5, 2.0, 1.8, 1.8, 1.8, 2.5];
        let hz_: Vec<f64> = ratios.iter().map(|r| freq * r).collect();
        let tilt = (vel - 0.65).min(0.0) * 40.0 + (vel - 0.65).max(0.0) * 18.0;
        let tilted = |f: f64| tilt * (f / 150.0).log2().clamp(0.0, 2.0);
        let levels: Vec<f64> = (0..18).map(|i| if hz_[i] < 15_000.0 { db(base[i] + tilted(hz_[i])) } else { 0.0 }).collect();
        let sooner = (vel - 0.65).max(0.0) * 1.25 + (vel - 0.65).min(0.0) * 2.86;
        let rings: Vec<f64> = (0..18).map(|i| lengths[i] * (1.0 - sooner * (1.0 - (hz_[i] / 500.0).log2().abs() / 1.3).max(0.0))).collect();
        let seed = ctx.seed();
        let mallet = || net(impulse::<U1>()) + (white_noise(seed) >> lpf(1000.0 + vel * 3000.0)) * hit(0.001, 0.006, -4.0) * 0.2;
        let quick: Vec<f64> = rings.iter().map(|r| r * 0.05).collect();
        let quieter: Vec<f64> = levels.iter().map(|l| l * db(-6.0)).collect();
        let drum = modes(mallet(), &hz_, &levels, &quick) + modes(mallet(), &hz_, &quieter, &rings);
        let bands = [1300.0, 2000.0, 3200.0, 5000.0, 8000.0];
        let mut draws = crate::nodes::Draws::new(1956);
        let mut skin_hz: Vec<f64> = (0..40).map(|_| draws.exp(900.0, 7000.0)).collect();
        skin_hz.sort_by(f64::total_cmp);
        let skin_levels: Vec<f64> = skin_hz.iter().map(|f| db(between(*f, &bands, &[-36.0, -51.0, -50.0, -56.0, -68.0]) + tilted(*f))).collect();
        let skin_rings: Vec<f64> = skin_hz.iter().map(|f| between(*f, &bands, &[1.8, 0.6, 0.6, 0.5, 0.42])).collect();
        let skin_at: Vec<f64> = skin_hz.iter().map(|f| f * freq / 150.0).collect();
        let skin = modes(mallet(), &skin_at, &skin_levels, &skin_rings);
        // 3.9 dB under the design's level, as its mallet's noise, drawn from another generator than scsynth's, sets the drum ringing that much louder than scsynth's render.
        let gain = db(-34.0 - 3.9) * (vel / 0.6).powi(2);
        let all: Vec<f64> = quieter.iter().chain(&skin_levels).copied().collect();
        let longest: Vec<f64> = rings.iter().chain(&skin_rings).copied().collect();
        Note::envelope((drum + skin) * gain as f32, o).within(silent_after(&all, &longest, gain))
    })
}

/// A value read at `f` from `values` at the bands' `centres`, along straight lines in octaves, carried on below the lowest for up to two bands' widths.
fn along(centres: &[f64], values: &[f64], f: f64) -> f64 {
    let x = f.log2();
    values[0]
        + values
            .windows(2)
            .enumerate()
            .map(|(i, w)| {
                let (low, high) = (centres[i].log2(), centres[i + 1].log2());
                (w[1] - w[0]) * ((x - low) / (high - low)).clamp(if i == 0 { -2.0 } else { 0.0 }, 1.0)
            })
            .sum::<f64>()
}

fn sitar() -> Synth {
    struck("sitar", 2.0)
        .option(Spec::note("sa").default(49.0).doc("The note the sympathetic strings are tuned to, and an octave above it."))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let centres = [200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0, 12649.0];
            let level_table = [10.0, 19.7, 31.5, 30.3, 22.5, 11.9, 17.9, -0.3, -9.0, -15.0];
            let fade_table = [12.68, 9.83, 12.68, 9.79, 9.32, 5.55, 3.97, 3.77, 2.4, 2.5];
            let hz_: Vec<f64> = (1..=90).map(|n| freq * n as f64).collect();
            let levels: Vec<f64> = hz_.iter().enumerate().map(|(i, f)| if *f < 16_000.0 { db(along(&centres, &level_table, *f) + (vel - 0.6) * 4.0 * ((i + 1) as f64).log2()) } else { 0.0 }).collect();
            let fades: Vec<f64> = hz_.iter().map(|f| along(&centres, &fade_table, *f)).collect();
            let string = modes(net(impulse::<U1>()), &hz_, &levels, &fades);
            let sa = hz(o.number("sa"));
            let sympathetic_hz = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 2.0, 4.0, 6.0, 8.0].map(|r| sa * r);
            let sympathetic = modes(lpf(4000.0) * 0.0001, &sympathetic_hz, &[1.0, 0.5, 0.35, 0.25, 0.2, 0.15, 0.7, 0.35, 0.25, 0.15], &[6.0; 10]);
            let pick = (white_noise(ctx.seed()) >> bpf(freq * 12.0, 1.2)) * hit(0.0005, 0.008, -4.0) * 0.3;
            let gain = 0.0007 * (vel / 0.6).powf(1.5);
            let sound = (string >> (net(pass()) & sympathetic)) * gain as f32 + pick * vel as f32;
            Note::envelope(sound, o).within(silent_after(&levels, &fades, gain) + 6.0)
        })
}

fn harmonium() -> Synth {
    Synth::new("harmonium")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the bellows are pumped."))
        .options(Spec::envelope(0.5, 0.2))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let centres = [65.4, 113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0, 12649.0];
            let table = [0.0, 6.85, 3.1, 3.8, -6.5, 3.9, 4.7, -1.0, -10.4, -27.1, -45.7, -55.6];
            let ratios = harmonics(48);
            let levels: Vec<f64> = ratios.iter().map(|n| if freq * n < 16_000.0 { db(along(&centres, &table, freq * n) + (vel - 0.6) * 3.0 * n.log2()) } else { 0.0 }).collect();
            let pitch = o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let tone = partials(pitch, &ratios, &levels, &spread_phases(48));
            let mut noise = net(zero());
            for (centre, level) in centres[1..].iter().zip([2.4, 0.9, -8.8, -5.7, -5.6, -6.5, -7.0, -10.5, -17.5, -27.5, -40.5]) {
                noise = noise + (white_noise(ctx.seed()) >> bpf(*centre, 0.3) >> bpf(*centre, 0.3)) * db(level) as f32;
            }
            let sound = (tone + noise) * swell(o, 0.3) * (0.006 * (vel / 0.6).powf(1.2)) as f32;
            Note::envelope(sound, o)
        })
}

fn tanpura() -> Synth {
    Synth::new("tanpura")
        .option(Spec::note("note").default(52.0).doc("Sa, the note the drone is tuned to."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the strings are plucked."))
        .option(Spec::seconds("gap").default(0.8).range(0.05, f64::INFINITY).doc("The seconds between plucks; a cycle of the four strings takes six gaps."))
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
                body[0] + octaves * slopes[0] + slopes.windows(2).enumerate().map(|(i, w)| (w[1] - w[0]) * (octaves - (i + 1) as f64).max(0.0)).sum::<f64>() + 40.0 * (f / 100.0).min(1.0).log10()
            };
            let plucks = |beat: f64| net(An(Pulses { start: beat * gap, period: 6.0 * gap, until, next: beat * gap, t: 0.0, dt: 1.0 / DEFAULT_SR }));
            let mut sound = net(zero());
            for (ratio, beat, offset) in [(0.75, 0.0, -11.0), (1.0, 2.0, 0.0), (1.0017, 3.0, 0.0), (0.4994, 4.0, -3.0)] {
                let string_hz = freq * ratio;
                for n in 1..=24 {
                    let n = n as f64;
                    if string_hz * n >= 12_000.0 {
                        continue;
                    }
                    let level = db(curve(138.6 * ratio * n) + offset);
                    let bloom = 0.005 + 1.722 * (n - 1.0) / 47.0;
                    let fade = 25.872 * n.powf(-0.35);
                    sound = sound + sine_from(string_hz * n, n * 2.4 + beat) * (plucks(beat) >> decay2(bloom, fade)) * level as f32;
                }
                let count = (16_000.0 / string_hz).floor() as usize;
                if count > 24 {
                    let ratios: Vec<f64> = (25..=count).map(|k| k as f64).collect();
                    let levels = vec![1.0 / count as f64; ratios.len()];
                    let train = partials(net(dc(string_hz as f32)), &ratios, &levels, &vec![0.0; ratios.len()]);
                    let high = train >> hpf(string_hz * 24.5) >> hpf(string_hz * 24.5) >> lpf(7000.0);
                    sound = sound + high * (plucks(beat) >> decay2(1.7, 7.0)) * db(45.0 + offset) as f32;
                }
            }
            let all_plucks = plucks(0.0) + plucks(2.0) + plucks(3.0) + plucks(4.0);
            let mut buzz = net(zero());
            for (centre, level) in [200.0, 320.0, 500.0, 800.0, 1260.0, 2000.0, 3200.0, 5000.0, 8000.0, 12500.0].iter().zip([-12.0, 9.0, 26.0, -7.0, 16.0, 4.0, -4.0, 0.0, -6.0, -11.0]) {
                let at = freq / 138.6 * centre;
                buzz = buzz + (white_noise(ctx.seed()) >> bpf(at, 0.3) >> bpf(at, 0.3)) * db(level) as f32;
            }
            let buzz = buzz * (all_plucks >> decay2(0.05, 6.0));
            Note::envelope((sound + buzz * 18.75) * (0.004 * (0.3 + vel * 0.7)) as f32, o)
        })
}

/// The figures a bowed string is measured by: its held harmonics' curve from `lowest`, and its plucked harmonics' peak, slopes and fades.
struct Bowed {
    lowest: f64,
    body: &'static [f64],
    top: f64,
    dark_from: f64,
    dark: f64,
    swell: (f64, f64),
    bow: f64,
    peak: f64,
    rise: f64,
    soft_rise: f64,
    fall: f64,
    soft_fall: f64,
    fade: f64,
    fade_slope: f64,
    pluck: f64,
    gain: f64,
    count: usize,
    dark_power: f64,
    level: f64,
}

/// A bowed string, which pizz: 1 plucks instead.
fn bowed(name: &'static str, s: Bowed) -> Synth {
    Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the string is bowed or plucked."))
        .option(Spec::float("pizz").default(0.0).range(0.0, 1.0).doc("1 plucks the string instead of bowing it."))
        .option(Spec::float("vibrato").default(0.25).range(0.0, 2.0).doc("The vibrato's depth in semitones, reached as the note holds."))
        .options(Spec::envelope(1.0, 0.3))
        .build(move |o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let pizz = o.number("pizz").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let soft = ((1.0 - vel) / 0.7).max(0.0);
            let ratios = harmonics(s.count);
            let hz_: Vec<f64> = ratios.iter().map(|n| freq * n).collect();
            let gain = vel.powf(s.gain) * db(s.level);
            let mut sound = net(zero());
            if pizz < 1.0 {
                let curve = shaped(&hz_, s.lowest, s.body, s.top);
                let levels: Vec<f64> = (0..s.count).map(|i| if hz_[i] < 16_000.0 { db(curve[i] - soft.powf(s.dark_power) * s.dark * above(hz_[i], s.dark_from)) } else { 0.0 }).collect();
                let held = partials(vibrato_after(o, o.number("vibrato"), 5.4, 0.3, 1.0), &ratios, &levels, &spread_phases(s.count)) * 0.008
                    + (pink_noise(ctx.seed()) >> bpf(freq * 3.0, 1.5)) * (db(s.bow) * vel) as f32;
                sound = sound + held * swell(o, s.swell.0 - s.swell.1 * vel) * (1.0 - pizz) as f32;
            }
            let mut end = f64::INFINITY;
            if pizz > 0.0 {
                let octaves: Vec<f64> = hz_.iter().map(|f| (f / s.peak).log2()).collect();
                let levels: Vec<f64> = (0..s.count)
                    .map(|i| {
                        let p = octaves[i];
                        let level = p.min(0.0) * (s.rise + s.soft_rise * soft.powi(3)) - p.max(0.0) * (s.fall + s.soft_fall * soft);
                        if hz_[i] < 16_000.0 { db(level) } else { 0.0 }
                    })
                    .collect();
                let fades: Vec<f64> = octaves.iter().map(|p| s.fade * 2f64.powf(p * s.fade_slope)).collect();
                sound = sound + modes(net(impulse::<U1>()), &hz_, &levels, &fades) * (s.pluck * pizz) as f32;
                if pizz >= 1.0 {
                    end = silent_after(&levels, &fades, s.pluck);
                }
            }
            Note::envelope(sound * gain as f32, o).within(end)
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

/// Sines summed, each at a ratio of the pitch its input gives.
#[derive(Clone)]
struct Partials {
    start: Vec<f32>,
    ratios: Vec<(f32, f32)>,
    phases: Vec<f32>,
    rate: f32,
}

impl AudioNode for Partials {
    const ID: u64 = 0x6d78_0040;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.phases.clone_from(&self.start);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let step = input[0] / self.rate;
        let mut sum = 0.0;
        for ((ratio, level), phase) in self.ratios.iter().zip(self.phases.iter_mut()) {
            sum += level * (std::f32::consts::TAU * *phase).sin();
            *phase = (*phase + step * ratio).fract();
        }
        [sum].into()
    }
}
