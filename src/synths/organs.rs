//! mx's organ, harmonium and accordion.

use super::common::{along, bpf, harmonics, net, partials, spread_phases, swell};
use crate::nodes::{db, hz, pink_noise, segments, white_noise};
use crate::{Note, Spec, Synth};
use fundsp::prelude::*;

/// Every synth of this family mx ships.
pub fn all() -> Vec<Synth> {
    vec![organ(), harmonium(), accordion()]
}

fn organ() -> Synth {
    Synth::new("organ")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("loud").default(0.0).range(0.0, 1.0).doc(
                "1 plays the loud set, a principal chorus, instead of the quiet stopped flute.",
            ),
        )
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
                    let flute = if odd {
                        (-8.0 - 0.25 * (key - 36.0)) * octave
                    } else {
                        -45.0
                    } + if n == 3.0 {
                        10.0 * ((42.0 - key) / 6.0).max(0.0)
                    } else {
                        0.0
                    };
                    let chorus = (4.0 - 0.4 * (key - 36.0)) * octave;
                    let slope = 18.0 - loud * 2.0;
                    let top = slope
                        * (freq * n / freq.max(740.0 + loud * 220.0))
                            .log2()
                            .max(0.0)
                            .min(27.0 / slope)
                        + 30.0 * (freq * n / 5000.0).log2().max(0.0);
                    let quiet_level = (1.5 * (key - 45.0)).min(0.0);
                    let loud_level =
                        -0.83 * (60.0 - key).max(0.0) - 0.25 * (note - 60.0).max(0.0) + 6.0;
                    let level = db(flute + quiet_level - top) * (1.0 - loud)
                        + db(chorus + loud_level - top) * loud;
                    if freq * n < 16_000.0 { level } else { 0.0 }
                })
                .collect();
            let pitch =
                || o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let pipes = partials(pitch(), &ratios, &levels, &spread_phases(24));
            let loud_level = -0.83 * (60.0 - key).max(0.0) - 0.25 * (note - 60.0).max(0.0) + 6.0;
            let bourdon_level = if note >= 60.0 {
                loud * db(-13.0) * db(loud_level)
            } else {
                0.0
            };
            let bourdon = partials(
                pitch(),
                &[0.5, 1.5, 2.5],
                &[bourdon_level, 0.2 * bourdon_level, 0.05 * bourdon_level],
                &[0.0; 3],
            );
            let speak = segments(&[0.0, 1.0], &[0.12 + loud * 0.04], &[3.0]);
            let wind = (pink_noise(ctx.seed()) >> bpf(freq * 2.0, 1.0)) * speak.clone() * 0.005;
            Note::envelope((pipes + bourdon) * speak * 0.03 + wind, o)
        })
}

fn harmonium() -> Synth {
    Synth::new("harmonium")
        .option(Spec::note("note").default(52.0).doc("The pitch."))
        .option(
            Spec::float("vel")
                .default(0.6)
                .range(0.0, 1.0)
                .doc("How hard the bellows are pumped."),
        )
        .options(Spec::envelope(0.5, 0.2))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let centres = [
                65.4, 113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0,
                12649.0,
            ];
            let table = [
                0.0, 6.85, 3.1, 3.8, -6.5, 3.9, 4.7, -1.0, -10.4, -27.1, -45.7, -55.6,
            ];
            let ratios = harmonics(48);
            let levels: Vec<f64> = ratios
                .iter()
                .map(|n| {
                    if freq * n < 16_000.0 {
                        db(along(&centres, &table, freq * n) + (vel - 0.6) * 3.0 * n.log2())
                    } else {
                        0.0
                    }
                })
                .collect();
            let pitch = o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let tone = partials(pitch, &ratios, &levels, &spread_phases(48));
            let mut noise = net(zero());
            for (centre, level) in centres[1..].iter().zip([
                2.4, 0.9, -8.8, -5.7, -5.6, -6.5, -7.0, -10.5, -17.5, -27.5, -40.5,
            ]) {
                noise = noise
                    + (white_noise(ctx.seed()) >> bpf(*centre, 0.3) >> bpf(*centre, 0.3))
                        * db(level) as f32;
            }
            let sound = (tone + noise) * swell(o, 0.3) * (0.006 * (vel / 0.6).powf(1.2)) as f32;
            Note::envelope(sound, o)
        })
}

/// An accordion's right hand: two reeds a few Hz apart, beating, with the harmonium's bands of harmonics and noise refitted to hammondman's "Accordeon master".
fn accordion() -> Synth {
    Synth::new("accordion")
        .option(Spec::note("note").default(60.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the bellows are pressed."))
        .option(Spec::float("detune").default(ACCORDION_DETUNE).range(0.0, 20.0).doc("How many Hz the second reed sounds above the first, which beat that many times a second; 0 sounds one reed."))
        .options(Spec::envelope(0.5, 0.15))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let freq = hz(o.number("note"));
            let detune = o.number("detune");
            let centres = [65.4, 113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5019.0, 7937.0, 12649.0];
            let table = [0.0, -1.3, 1.8, 9.2, 7.7, 2.4, 0.5, -1.5, -4.8, -8.3, -23.0, -41.5];
            let ratios = harmonics(48);
            let levels: Vec<f64> = ratios.iter().map(|n| if freq * n < 16_000.0 { db(along(&centres, &table, freq * n) + (vel - 0.6) * 3.0 * n.log2()) } else { 0.0 }).collect();
            // The louder reed sounds the note and the softer one above it, as the pitch heard follows the louder.
            let pitch = o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32));
            let mut tone = partials(pitch.clone(), &ratios, &levels, &spread_phases(48));
            if detune > 0.0 {
                let above = pitch >> net(map(move |x: &Frame<f32, U1>| x[0] + detune as f32));
                tone = (tone + partials(above, &ratios, &levels, &spread_phases(48)) * ACCORDION_SECOND as f32) * (1.0 / (1.0 + ACCORDION_SECOND)) as f32;
            }
            let mut noise = net(zero());
            for (centre, level) in centres[1..].iter().zip([-8.2, -8.2, -13.6, -16.3, -14.7, -9.9, -10.1, -8.4, -13.1, 1.4, -27.8]) {
                noise = noise + (white_noise(ctx.seed()) >> bpf(*centre, 0.3) >> bpf(*centre, 0.3)) * db(level) as f32;
            }
            // It holds about as loud as the cello and the clarinet, which a melody instrument sits beside.
            let sound = (tone + noise) * swell(o, ACCORDION_ATTACK) * (0.036 * (vel / 0.6).powf(1.2)) as f32;
            Note::envelope(sound, o)
        })
}

const ACCORDION_DETUNE: f64 = 4.0;
const ACCORDION_SECOND: f64 = 0.5;
const ACCORDION_ATTACK: f64 = 0.065;
