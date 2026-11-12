//! mx's tuned percussion: the marimba, the vibraphone, the xylophone, the glockenspiel and the timpani.

use super::common::{above, bpf, hpf, lpf, net, silent_after, struck};
use crate::nodes::{between, db, hit, hz, modes, shape, white_noise};
use crate::{Note, Spec, Synth};
use fundsp::prelude::*;
use std::f64::consts::PI;

/// Every synth of this family mx ships.
pub fn all() -> Vec<Synth> {
    vec![
        marimba(),
        vibraphone(),
        xylophone(),
        glockenspiel(),
        timpani(),
    ]
}

fn marimba() -> Synth {
    struck("marimba", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let ring =
            4.6 * 2f64.powf((note - 36.0) / -17.0) * 2f64.powf((note - 78.0).max(0.0) / -16.0);
        let swell = 0.45 * 2f64.powf((note - 36.0) / -18.0);
        let ratios = [1.0, 1.0, 4.0, 10.0, 19.0, 30.0, 3.0, 5.0, 7.0, 9.0];
        let mallet =
            |f: f64, corner: f64, slope: f64| slope * above(f, corner) + 24.0 * above(f, 3000.0);
        let fourth = -12.0
            - mallet(freq * 4.0, 600.0, 57.0 * (1.0 - vel))
            - 10.0 * above(freq * 4.0, 900.0)
            - 20.0 * (370.0 / (freq * 4.0)).log2().max(0.0);
        let overtone =
            |n: f64| 10.0 + 14.5 * vel.log2() - mallet(freq * n, 1100.0, 33.4 - 11.4 * vel);
        let low_tube = -22.0 - 60.0 * (((note < 46.0) as u8 + (note > 62.0) as u8) as f64);
        let tube = [(3.0, low_tube), (5.0, -26.0), (7.0, -26.0), (9.0, -26.0)]
            .map(|(n, l)| l - mallet(freq * n, 600.0, 12.0 + 45.0 * (1.0 - vel)));
        let mut levels = vec![1.25, -1.0];
        levels.extend(
            [
                fourth,
                overtone(10.0),
                overtone(19.0) - 8.0,
                overtone(30.0) - 12.0,
            ]
            .into_iter()
            .chain(tube)
            .map(db),
        );
        let levels: Vec<f64> = levels
            .iter()
            .zip(ratios)
            .map(|(l, r)| if freq * r < 15_000.0 { *l } else { 0.0 })
            .collect();
        let mut fades = vec![ring, swell];
        fades.extend(
            [0.3, 0.1, 0.08, 0.06, 0.35, 0.35, 0.35, 0.35]
                .iter()
                .zip(&ratios[2..])
                .map(|(k, n)| ring * k * (1500.0 / (freq * n)).min(1.0).powf(0.8)),
        );
        let hz_: Vec<f64> = ratios.iter().map(|r| freq * r).collect();
        let bar = modes(net(impulse::<U1>()), &hz_, &levels, &fades);
        let low_tap =
            (white_noise(ctx.seed()) >> lpf(2200.0) >> lpf(2200.0) >> lpf(2200.0) >> hpf(1000.0))
                * hit(0.0005, 0.12, -4.0)
                * 0.55;
        let high_tap = (white_noise(ctx.seed()) >> lpf(4000.0) >> lpf(4000.0) >> hpf(2500.0))
            * hit(0.0005, 0.05, -4.0)
            * 0.3;
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
        let shaped_levels = [
            0.0,
            -32.0 - 15.0 * above(freq * 2.0, 700.0),
            -7.0 - 10.0 * above(freq * 3.0, 1000.0),
            12.0 - 10.0 * above(freq * 6.39, 3000.0),
        ];
        let levels: Vec<f64> = (0..4)
            .map(|i| {
                if freq * ratios[i] < 15_000.0 {
                    db(shaped_levels[i] + [0.0, -10.0, -1.0, 9.0][i] * vel.log2())
                } else {
                    0.0
                }
            })
            .collect();
        let fades = [1.0, 0.23, 0.3, 0.1].map(|k| ring * k);
        let bar = modes(
            net(impulse::<U1>()),
            &ratios.map(|r| freq * r),
            &levels,
            &fades,
        );
        let centre = (4000.0 * (vel / 0.6).powf(0.6).min(1.0)).clamp(500.0, 12_000.0);
        let click_level = 15.0 * 2f64.powf((note - 79.0).max(0.0) / -5.5) * (vel / 0.6).powf(1.2);
        let click = (white_noise(ctx.seed()) >> bpf(centre, 0.25))
            * hit(0.0005, 0.04, -4.0)
            * click_level as f32;
        let gain = 0.015 * (vel / 0.6).powf(1.9);
        Note::envelope((bar + click) * gain as f32, o)
            .within(silent_after(&levels, &fades, gain).max(0.05))
    })
}

fn glockenspiel() -> Synth {
    struck("glockenspiel", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let note = o.number("note");
        let freq = hz(note);
        let step = note - 72.0;
        let ring = 12.2 * 2f64.powf((note - 90.0).max(0.0) / -9.0);
        let ratios = [
            1.0,
            3.3 - 0.016 * step,
            7.08 - 0.0486 * step - 0.00117 * step * step,
            11.32 - 0.296 * step + 0.00203 * step * step,
        ];
        let mallet = |f: f64| 17.0 * (1.0 - vel) * above(f, 2800.0);
        let shaped_levels = [
            0.0,
            -39.0 + 2.0 * step.min(16.0) - (step - 16.0).max(0.0),
            12.0 - 0.2 * step,
            -8.0 - 3.0 * (step - 19.0).max(0.0),
        ];
        let levels: Vec<f64> = (0..4)
            .map(|i| {
                let struck = if i == 0 {
                    0.0
                } else {
                    2.0 * vel.log2() - mallet(freq * ratios[i])
                };
                if freq * ratios[i] < 18_000.0 {
                    db(shaped_levels[i] + struck)
                } else {
                    0.0
                }
            })
            .collect();
        let fades: Vec<f64> = [ring, 1.0, 0.29, 1.1]
            .iter()
            .zip(ratios)
            .map(|(k, r)| k * (4000.0 / (freq * r)).min(1.0).powf(0.8))
            .collect();
        let bar = modes(
            net(impulse::<U1>()),
            &ratios.map(|r| freq * r),
            &levels,
            &fades,
        );
        let click = (white_noise(ctx.seed()) >> bpf(1750.0, 0.5))
            * hit(0.0005, 0.036, -4.0)
            * (2.0 * (vel / 0.6).powf(-1.4)) as f32;
        let gain = 0.02 * (vel / 0.6).powf(1.9);
        Note::envelope((bar + click) * gain as f32, o).within(silent_after(&levels, &fades, gain))
    })
}

fn timpani() -> Synth {
    struck("timpani", 0.3).build(|o, ctx| {
        let vel = o.number("vel").clamp(0.05, 1.0);
        let freq = hz(o.number("note"));
        let ratios = [
            1.0, 1.487, 1.70, 1.95, 2.11, 2.40, 2.70, 2.82, 3.18, 3.33, 3.70, 4.2, 4.7, 5.05, 0.75,
            0.83, 0.885, 1.18,
        ];
        let base = [
            2.0, -13.0, -27.0, -21.0, -24.0, -27.0, -16.0, -14.0, -18.0, -19.0, -16.0, -18.0,
            -20.0, -22.0, -13.0, -8.0, -7.0, -20.0,
        ];
        let lengths = [
            2.1, 6.0, 5.0, 6.0, 6.0, 5.0, 4.0, 4.0, 3.5, 3.5, 3.0, 3.0, 2.5, 2.0, 1.8, 1.8, 1.8,
            2.5,
        ];
        let hz_: Vec<f64> = ratios.iter().map(|r| freq * r).collect();
        let tilt = (vel - 0.65).min(0.0) * 40.0 + (vel - 0.65).max(0.0) * 18.0;
        let tilted = |f: f64| tilt * (f / 150.0).log2().clamp(0.0, 2.0);
        let levels: Vec<f64> = (0..18)
            .map(|i| {
                if hz_[i] < 15_000.0 {
                    db(base[i] + tilted(hz_[i]))
                } else {
                    0.0
                }
            })
            .collect();
        let sooner = (vel - 0.65).max(0.0) * 1.25 + (vel - 0.65).min(0.0) * 2.86;
        let rings: Vec<f64> = (0..18)
            .map(|i| {
                lengths[i] * (1.0 - sooner * (1.0 - (hz_[i] / 500.0).log2().abs() / 1.3).max(0.0))
            })
            .collect();
        let seed = ctx.seed();
        let mallet = || {
            net(impulse::<U1>())
                + (white_noise(seed) >> lpf(1000.0 + vel * 3000.0)) * hit(0.001, 0.006, -4.0) * 0.2
        };
        let quick: Vec<f64> = rings.iter().map(|r| r * 0.05).collect();
        let quieter: Vec<f64> = levels.iter().map(|l| l * db(-6.0)).collect();
        let drum = modes(mallet(), &hz_, &levels, &quick) + modes(mallet(), &hz_, &quieter, &rings);
        let bands = [1300.0, 2000.0, 3200.0, 5000.0, 8000.0];
        let mut draws = crate::nodes::Draws::new(1956);
        let mut skin_hz: Vec<f64> = (0..40).map(|_| draws.exp(900.0, 7000.0)).collect();
        skin_hz.sort_by(f64::total_cmp);
        let skin_levels: Vec<f64> = skin_hz
            .iter()
            .map(|f| db(between(*f, &bands, &[-36.0, -51.0, -50.0, -56.0, -68.0]) + tilted(*f)))
            .collect();
        let skin_rings: Vec<f64> = skin_hz
            .iter()
            .map(|f| between(*f, &bands, &[1.8, 0.6, 0.6, 0.5, 0.42]))
            .collect();
        let skin_at: Vec<f64> = skin_hz.iter().map(|f| f * freq / 150.0).collect();
        let skin = modes(mallet(), &skin_at, &skin_levels, &skin_rings);
        // 3.9 dB under the design's level, as its mallet's noise, drawn from another generator than scsynth's, sets the drum ringing that much louder than scsynth's render.
        let gain = db(-34.0 - 3.9) * (vel / 0.6).powi(2);
        let all: Vec<f64> = quieter.iter().chain(&skin_levels).copied().collect();
        let longest: Vec<f64> = rings.iter().chain(&skin_rings).copied().collect();
        Note::envelope((drum + skin) * gain as f32, o).within(silent_after(&all, &longest, gain))
    })
}
