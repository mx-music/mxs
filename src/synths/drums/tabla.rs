//! The tabla, and each of its bols played alone.

use super::{amps, high_shelf, impulse_net, lpf, net, noise, scaled, vel};
use crate::nodes::{db, exp_segments, hz, modes, segments, struck_gliding};
use crate::{Note, Spec, Synth};
use fundsp::prelude::*;

/// The synths of this file, each of the bols alone besides the tabla.
pub(super) fn all() -> Vec<Synth> {
    let mut synths = vec![tabla("tabla", None)];
    for bol in BOLS.iter().map(|b| b.name) {
        synths.push(tabla(&format!("tabla_{bol}"), Some(bol)));
    }
    synths
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
        noise: [
            -90.0, -90.0, -90.0, -90.0, -90.0, -90.0, 8.2, 1.5, -19.9, -26.1, -38.7,
        ],
        fades: [
            0.3, 2.02, 0.166, 1.291, 0.417, 0.315, 0.166, 0.216, 0.407, 0.285, 0.258,
        ],
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
        noise: [
            -90.0, -90.0, -90.0, -90.0, -90.0, -4.0, -8.0, -20.3, -24.2, -26.8, -30.0,
        ],
        fades: [
            13.236, 2.553, 2.332, 2.01, 0.503, 0.479, 0.66, 1.334, 0.777, 1.104, 0.43,
        ],
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
        noise: [
            -90.0, -90.0, -90.0, -90.0, 17.2, 21.0, 17.2, 13.0, 10.1, 5.7, -8.8,
        ],
        fades: [
            0.451, 0.21, 0.13, 0.128, 0.173, 0.121, 0.136, 0.124, 0.102, 0.106, 0.143,
        ],
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
        noise: [
            -90.0, -90.0, 53.5, -90.0, 48.0, -90.0, 23.7, 13.9, 8.1, -2.0, -4.0,
        ],
        fades: [
            10.448, 2.321, 0.12, 2.027, 0.1, 0.259, 0.465, 0.546, 0.248, 0.359, 0.299,
        ],
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
        noise: [
            -90.0, -90.0, -90.0, -90.0, 3.1, -5.5, -17.9, -25.6, -31.3, -38.9, -40.8,
        ],
        fades: [
            3.528, 5.727, 1.629, 0.553, 0.433, 0.453, 1.63, 10.221, 12.0, 11.327, 12.0,
        ],
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
        noise: [
            -90.0, -90.0, -90.0, -90.0, 18.9, 10.3, 4.1, -2.3, -5.4, -12.6, -23.2,
        ],
        fades: [0.05; 11],
        gain: 0.081,
        high: 2900.0,
    },
];

/// The tabla, whose `bol` option picks the stroke; `fixed` makes one of the old names that each played one stroke.
/// Each stroke has its own default note, so a note given to the tabla is the stroke's pitch, as with the old names.
fn tabla(name: &str, fixed: Option<&'static str>) -> Synth {
    let mut decl = Synth::new(name).option(
        Spec::float("vel")
            .default(0.6)
            .range(0.0, 1.0)
            .doc("How hard it is struck."),
    );
    match fixed {
        Some(bol) => {
            let b = BOLS.iter().find(|b| b.name == bol).expect("a bol");
            decl = decl.option(
                Spec::note("note")
                    .default(b.note)
                    .doc("The stroke's pitch."),
            );
        }
        None => {
            let names: Vec<&str> = BOLS.iter().map(|b| b.name).collect();
            decl = decl
                .option(
                    Spec::name("bol", &names).default("na").doc(
                        "The stroke: na, tun, te and tas on the dayan, ghe and ke on the bayan.",
                    ),
                )
                .option(
                    Spec::note("note")
                        .doc("The stroke's pitch; each bol has its own when not given."),
                );
        }
    }
    decl.build(move |o, ctx| {
        let picked = fixed.unwrap_or_else(|| o.name("bol"));
        let b = BOLS
            .iter()
            .find(|b| b.name == picked)
            .expect("a bol the option allows");
        let note = if fixed.is_some() {
            o.number("note")
        } else {
            o.given("note").unwrap_or(b.note)
        };
        let vel = vel(o, 0.0);
        let freq = hz(note);
        let finger = impulse_net() >> lpf(3000.0);
        // Ghe's pitch glides with its note option, as a palm pressing the bayan's head bends it.
        let gliding = b.name == "ghe" && (fixed.is_some() || o.given("note").is_some());
        let pitch = if gliding {
            o.follow("note") >> net(map(|x: &Frame<f32, U1>| hz(x[0] as f64) as f32))
        } else {
            net(dc(freq as f32))
        };
        let head = if gliding {
            let levels = amps(b.levels);
            let bank =
                b.ratios
                    .iter()
                    .zip(&levels)
                    .zip(b.rings)
                    .map(|((&ratio, &level), &ring)| {
                        net(map(move |x: &Frame<f32, U2>| -> Frame<f32, U2> {
                            [x[0], x[1] * ratio as f32].into()
                        })) >> struck_gliding(level, ring)
                    });
            (finger | pitch.clone()) >> bank.reduce(|a, b| a & b).expect("a bol has modes")
        } else {
            modes(finger, &scaled(b.ratios, freq), &amps(b.levels), b.rings)
        };
        let centres = [
            113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, b.high, 5019.0, 7937.0, 12649.0,
        ];
        let mut hiss = Net::wrap(Box::new(zero()));
        for (i, centre) in centres.iter().enumerate() {
            if b.noise[i] <= -90.0 {
                continue;
            }
            let at = (pitch.clone() * (centre / hz(b.note)) as f32)
                >> net(map(|x: &Frame<f32, U1>| x[0].clamp(20.0, 20_000.0)));
            let narrow = |sound: Net| {
                (sound | at.clone() | net(dc(1.0 / 0.35))) >> (net(bandpass::<f32>()) * 0.35)
            };
            let band = narrow(narrow(narrow(narrow(noise(ctx)))));
            let fall = segments(&[0.0, 1.0], &[0.001], &[0.0])
                * exp_segments(&[1.0, 1.0, 0.001], &[0.001, b.fades[i]]);
            hiss = hiss + band * fall * db(b.noise[i]) as f32;
        }
        let sound = (head + hiss) >> high_shelf(1500.0, (vel - 0.6) * 5.0);
        Note::rings(sound * (b.gain * (vel / 0.6).powf(1.5)) as f32, b.length)
    })
}
