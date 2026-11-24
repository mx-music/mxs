use mxs::graph::{Clause, Graph};
use mxs::{Effect, Engine, Event, RATE, Spec, Synth, Value};

const MARIMBA: &str = include_str!("../marimba.json");

fn n(x: f64) -> Graph {
    Graph::Number(x)
}

fn opt(name: &str) -> Graph {
    Graph::Option(name.to_string())
}

fn call(name: &str, args: Vec<Graph>) -> Graph {
    Graph::call(name, args, vec![])
}

fn two(name: &str, a: Graph, b: Graph) -> Graph {
    call(name, vec![a, b])
}

fn select(condition: Graph, then: Graph, otherwise: Graph) -> Graph {
    call("select", vec![condition, then, otherwise])
}

fn log2(x: Graph) -> Graph {
    call("log2", vec![x])
}

/// The marimba's shapes as `python/examples/marimba.py` gives them, copied from mxs's marimba.
#[allow(
    clippy::approx_constant,
    reason = "fitted logarithms of time constants, two of which round to -3.14"
)]
const MODES: [(f64, [[[f64; 4]; 11]; 3]); 8] = [
    (
        1.0,
        [
            [
                [8.90, -0.56, -0.84, -2.14],
                [8.62, -0.77, -0.90, -2.84],
                [13.98, -1.25, -0.92, -2.26],
                [7.41, -1.13, -0.92, -3.01],
                [5.53, -1.52, -0.89, -3.37],
                [1.34, -1.55, -0.79, -4.04],
                [-0.67, -1.93, -0.86, -4.74],
                [-4.24, -2.20, 0.41, -4.60],
                [-1.50, -2.84, -1.00, -5.80],
                [-1.45, -2.98, 0.06, -3.90],
                [0.63, -3.49, 0.00, 2.00],
            ],
            [
                [10.69, -0.55, -0.89, -2.21],
                [8.78, -0.78, -0.91, -2.68],
                [12.44, -1.12, -0.93, -2.26],
                [5.94, -1.10, -0.94, -3.14],
                [6.98, -1.57, -0.91, -3.04],
                [-0.29, -1.51, -0.84, -4.16],
                [-1.02, -1.97, -0.95, -4.87],
                [-2.00, -2.34, -0.81, -5.42],
                [-30.70, -2.15, 30.00, -2.87],
                [-29.81, -2.59, 27.78, -3.06],
                [-0.13, -3.52, 0.11, -4.68],
            ],
            [
                [5.59, -0.51, -0.86, -2.26],
                [8.77, -0.81, -0.91, -2.52],
                [13.03, -1.08, -0.93, -2.20],
                [7.33, -1.08, -0.88, -2.92],
                [6.52, -1.53, -0.87, -3.16],
                [-0.24, -1.45, -0.81, -4.28],
                [-1.78, -1.91, -0.91, -4.90],
                [-4.60, -2.31, -1.00, -5.77],
                [-2.97, -2.71, 0.55, -4.89],
                [-4.27, -3.00, 0.58, -3.54],
                [18.24, -3.22, -0.89, -3.20],
            ],
        ],
    ),
    (
        2.0,
        [
            [
                [-40.40, -1.53, -0.94, -2.26],
                [-9.97, -2.47, -0.98, -2.45],
                [-50.80, -1.59, 9.01, -4.88],
                [-60.80, -1.55, -0.88, -4.54],
                [-150.00, -5.00, 0.00, -5.00],
                [-37.14, -2.69, -1.00, -2.77],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-86.09, -2.40, 30.00, -3.90],
                [-50.75, -4.38, -0.02, -2.79],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-15.14, -2.18, -0.98, -2.17],
                [-26.55, -4.82, 0.04, -0.83],
                [-57.25, -0.55, 30.00, -4.79],
                [-60.18, -0.97, 2.86, -2.19],
                [-47.80, -2.08, -0.74, -4.23],
                [-50.60, -2.10, 22.59, -5.66],
                [-53.93, -2.48, 12.26, -5.22],
                [-47.37, -2.94, -0.60, -3.85],
                [-52.72, -3.14, 1.15, -5.06],
                [-35.45, -5.27, 0.02, -3.05],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-50.66, -2.77, 11.31, -4.66],
                [-30.10, -4.66, 0.06, -0.88],
                [-48.58, -0.75, 3.77, -4.24],
                [-45.70, -1.57, 1.82, -5.14],
                [-37.48, -1.94, -0.41, -3.36],
                [-45.52, -2.13, 30.00, -6.11],
                [-43.31, -2.65, 3.06, -4.58],
                [-46.62, -3.06, 7.13, -5.08],
                [-48.34, -3.35, 5.05, -5.84],
                [-33.35, -5.65, 0.15, -3.61],
                [-20.53, -6.00, 0.03, -3.68],
            ],
        ],
    ),
    (
        3.0,
        [
            [
                [-55.65, 0.60, -0.75, -1.16],
                [-150.00, -5.00, 0.00, -5.00],
                [-34.06, -1.60, 7.99, -3.95],
                [-21.24, -4.34, 0.07, -1.68],
                [-12.79, -2.30, -1.00, -2.31],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-22.91, -1.96, -0.97, -1.95],
                [-64.62, 0.38, 30.00, -4.76],
                [-28.28, -1.55, 5.67, -4.37],
                [-28.03, -1.86, 4.37, -4.91],
                [5.30, -2.61, -1.00, -2.62],
                [-62.53, -1.96, 30.00, -6.06],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-54.31, -0.18, 8.50, -4.32],
                [2.03, -1.10, -1.00, -1.10],
                [-33.12, -1.17, 16.15, -4.55],
                [15.70, -2.23, -1.00, -2.24],
                [15.20, -2.52, -1.00, -2.52],
                [-32.04, -2.94, -0.77, -3.29],
                [-10.54, -3.06, -0.98, -3.05],
                [-39.49, -5.26, 0.00, -2.12],
                [-34.03, -5.89, 0.01, -3.07],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        4.0,
        [
            [
                [-20.22, -1.62, -0.39, -4.39],
                [-15.40, -1.75, 1.58, -4.10],
                [-6.96, -2.12, -0.22, -2.63],
                [-35.24, -2.55, 15.15, -5.98],
                [-43.20, -2.79, 2.30, -5.21],
                [-49.75, -3.42, -0.01, -1.50],
                [-22.81, -3.54, -0.95, -3.53],
                [-53.43, -5.21, -0.00, -2.13],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-7.48, -1.85, -0.64, -2.42],
                [-8.75, -1.85, 0.41, -3.72],
                [-15.62, -1.93, 1.41, -4.59],
                [-27.49, -2.22, 2.38, -3.13],
                [-29.31, -2.80, 1.47, -4.07],
                [-40.03, -3.46, 0.00, -0.49],
                [-37.39, -3.84, 0.00, 2.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-46.64, -0.91, 7.99, -1.83],
                [-12.72, -1.84, 1.69, -4.50],
                [-7.92, -1.99, 0.25, -3.94],
                [-14.34, -2.39, 0.11, -3.17],
                [-8.56, -3.12, -0.68, -3.10],
                [-45.88, -3.06, 12.42, -3.44],
                [-27.69, -3.81, 0.00, 2.00],
                [-58.38, -4.18, -0.01, -1.75],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        5.0,
        [
            [
                [-25.20, -3.19, -0.05, -1.31],
                [-53.97, -1.92, 3.03, -4.41],
                [-24.53, -4.88, 0.04, -1.37],
                [-61.14, -1.19, 30.00, -5.76],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-150.00, -5.00, 0.00, -5.00],
                [-25.19, -4.20, 0.02, -1.03],
                [-39.07, -1.88, 10.13, -5.10],
                [3.82, -2.36, -1.00, -2.37],
                [-28.25, -2.58, -0.98, -2.81],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-48.18, -1.29, 5.06, -3.02],
                [-35.86, -3.73, -0.11, -1.50],
                [-31.32, -1.81, 7.58, -4.63],
                [-33.49, -1.97, 3.91, -4.91],
                [-43.34, -2.06, 6.61, -4.93],
                [-17.94, -2.02, -0.98, -2.02],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        7.0,
        [
            [
                [-55.84, -2.59, 4.56, -4.29],
                [-26.55, -4.67, 0.01, -2.07],
                [-48.53, -4.33, 0.12, -1.30],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-31.87, -3.76, 0.10, -2.02],
                [-31.71, -4.62, 0.05, -1.68],
                [-44.27, -1.91, -0.67, -2.32],
                [-8.70, -2.55, -1.00, -2.56],
                [-13.30, -2.67, -1.00, -2.68],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-52.63, -1.93, 8.02, -4.58],
                [-54.83, -1.88, 8.97, -4.53],
                [-50.61, -1.51, 16.27, -4.73],
                [-53.89, -1.90, -0.76, -2.35],
                [-52.91, -2.15, -0.88, -2.90],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        10.0,
        [
            [
                [-0.57, -2.54, -0.83, -5.25],
                [15.85, -2.48, -0.97, -2.47],
                [-20.30, -3.04, -0.09, -2.24],
                [-31.13, -3.61, 0.59, -4.49],
                [-40.07, -4.60, 0.01, -3.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-19.78, -2.00, 25.16, -2.71],
                [-12.71, -2.76, 3.85, -3.11],
                [18.05, -2.91, -0.94, -2.89],
                [9.74, -3.23, -0.97, -3.23],
                [-26.49, -3.94, -0.08, -3.24],
                [-40.15, -5.98, 0.02, -4.11],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-3.27, -2.46, 4.00, -3.18],
                [9.10, -2.95, -0.89, -4.72],
                [36.06, -2.84, -0.98, -2.83],
                [-2.68, -3.78, 0.01, -2.48],
                [14.46, -3.67, -0.97, -3.67],
                [-48.04, -5.15, 0.01, -1.55],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
    (
        19.0,
        [
            [
                [-29.67, -4.75, 0.03, -2.80],
                [-31.88, -4.24, -0.21, -3.41],
                [-35.34, -5.52, -0.00, -2.56],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-25.13, -2.98, 3.38, -3.69],
                [-19.44, -3.44, 0.00, 2.00],
                [-19.16, -4.39, -0.97, -4.38],
                [-32.14, -5.39, -0.00, -2.88],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
            [
                [-44.39, -2.40, 30.00, -3.35],
                [13.13, -3.39, -0.96, -3.38],
                [-25.02, -5.10, 0.09, -3.59],
                [-1.39, -4.56, -0.96, -4.54],
                [-9.90, -5.90, -0.90, -5.88],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
                [-150.00, -5.00, 0.00, -5.00],
            ],
        ],
    ),
];
const FALL: f64 = 6.907_755_278_982_137;
const VELS: [f64; 3] = [40.0 / 127.0, 80.0 / 127.0, 1.0];

/// mxs's marimba built in Rust, as `marimba.json` writes it.
fn marimba() -> Synth {
    let (note, vel) = (opt("note"), opt("vel"));
    let freq = call("hz", vec![note.clone()]);
    let eq = |a: &Graph, b: f64| two("eq", a.clone(), n(b));
    let at = call(
        "clamp",
        vec![
            two("div", two("sub", note.clone(), n(36.0)), n(6.0)),
            n(0.0),
            n(10.0),
        ],
    );
    let low = two(
        "min",
        two("sub", at.clone(), two("mod", at.clone(), n(1.0))),
        n(9.0),
    );
    let v = two("max", vel.clone(), n(VELS[0]));
    let soft = two("le", v.clone(), n(VELS[1]));
    let up = select(
        soft.clone(),
        two(
            "div",
            log2(two("div", v.clone(), n(VELS[0]))),
            log2(n(VELS[1] / VELS[0])),
        ),
        two(
            "div",
            log2(two("div", v.clone(), n(VELS[1]))),
            log2(n(VELS[2] / VELS[1])),
        ),
    );
    let past = two("sub", at.clone(), low.clone());
    let along: Vec<Graph> = (0..11)
        .map(|i| {
            let i = i as f64;
            two(
                "add",
                two("mul", eq(&low, i), two("sub", n(1.0), past.clone())),
                two("mul", eq(&low, i - 1.0), past.clone()),
            )
        })
        .collect();
    let rest = two("sub", n(1.0), up.clone());
    let across = [
        select(soft.clone(), rest.clone(), n(0.0)),
        select(soft.clone(), up.clone(), rest),
        select(soft.clone(), n(0.0), up),
    ];
    let fall = |fade: f64| two("mul", n(FALL), call("exp", vec![n(fade)]));

    let lower = select(soft, n(0.0), n(1.0));
    let mut ring = n(0.0);
    for j in 0..3 {
        for i in 0..11 {
            let (jf, i_f) = (j as f64, i as f64);
            let corner = two(
                "mul",
                two("max", eq(&lower, jf), eq(&lower, jf - 1.0)),
                two("max", eq(&low, i_f), eq(&low, i_f - 1.0)),
            );
            ring = two(
                "max",
                ring,
                select(corner, fall(MODES[0].1[j][i][1]), n(0.0)),
            );
        }
    }

    let strike = call("impulse", vec![]);
    let mode = |hz: Graph, level: Graph, fade: Graph| {
        Graph::call(
            "mode",
            vec![strike.clone()],
            vec![("hz", hz), ("level", level), ("fade", fade)],
        )
    };
    let mut modes = vec![];
    for (k, shapes) in MODES {
        let hz = two("mul", freq.clone(), n(k));
        let heard = select(two("lt", hz.clone(), n(15_000.0)), n(1.0), n(0.0));
        for j in 0..3 {
            for i in 0..11 {
                let [level, fade, second, second_fade] = shapes[j][i];
                if level < -100.0 {
                    continue;
                }
                let share = two("mul", along[i].clone(), across[j].clone());
                let first = two(
                    "mul",
                    two("mul", heard.clone(), two("mul", n(1.25), share)),
                    call("db", vec![n(level)]),
                );
                modes.push(mode(hz.clone(), first.clone(), fall(fade)));
                modes.push(mode(
                    hz.clone(),
                    two("mul", first, n(second)),
                    fall(second_fade),
                ));
            }
        }
    }
    let struck = call("mix", vec![Graph::List(modes)]);
    let band = |low: f64, poles: f64, high: f64, release: f64, level: f64| {
        let noise = call("noise", vec![Graph::Name("white".to_string())]);
        let sound = Graph::call(
            "lowpass",
            vec![noise],
            vec![("hz", n(low)), ("poles", n(poles))],
        );
        let sound = Graph::call("highpass", vec![sound], vec![("hz", n(high))]);
        let struck = Graph::call(
            "perc",
            vec![sound],
            vec![("attack", n(0.0005)), ("release", n(release))],
        );
        two("gain", struck, n(level))
    };
    let tap = two(
        "add",
        band(2200.0, 6.0, 1000.0, 0.12, 0.55),
        band(4000.0, 4.0, 2500.0, 0.05, 0.3),
    );
    let by_vel = |power: f64| two("pow", two("div", vel.clone(), n(0.6)), n(power));
    let sound = two(
        "gain",
        two("add", struck, two("mul", tap, by_vel(0.5))),
        two("mul", n(0.1), by_vel(1.9)),
    );
    let graph = Graph::call(
        "rings",
        vec![call("note_envelope", vec![sound])],
        vec![("seconds", two("div", two("mul", ring, n(4.0)), n(3.0)))],
    );
    let unbounded = |spec: Spec| spec.range(0.0, f64::INFINITY);
    let specs = vec![
        Spec::note("note").default(52.0).doc("The pitch."),
        Spec::float("vel")
            .default(0.6)
            .range(0.05, 1.0)
            .doc("How hard the bar is struck."),
        unbounded(Spec::seconds("attack").default(0.0)),
        unbounded(Spec::seconds("decay").default(0.0)),
        unbounded(Spec::seconds("sustain").default(1000.0)),
        unbounded(Spec::seconds("release").default(0.3)),
        Spec::float("attack_level").default(1.0),
        Spec::float("decay_level").default(-1.0),
        Spec::float("sustain_level").default(1.0),
    ];
    mxs::graph::synth(
        "marimba",
        specs,
        vec![Clause {
            matches: vec![],
            graph,
        }],
    )
    .unwrap()
}

fn render(synth: Synth, note: f64, vel: f64) -> (Vec<f32>, Vec<f32>) {
    let mut engine = Engine::offline(RATE);
    let name = synth.name().to_string();
    engine.register(synth);
    let options = [("note", note), ("vel", vel)];
    engine
        .play(&[(0.0, Event::note(None, &name, &options))])
        .unwrap();
    engine.render(3.0).unwrap()
}

#[test]
fn the_marimba_file_renders_the_samples_of_the_same_graph_built_in_rust() {
    for (note, vel) in [(36.0, 0.3), (52.0, 0.55), (68.0, 0.8), (84.0, 1.0)] {
        let read = render(mxs::graph::synth_from_json(MARIMBA).unwrap(), note, vel);
        let built = render(marimba(), note, vel);
        assert!(
            read.0.iter().any(|s| s.abs() > 0.001),
            "note {note} is silent"
        );
        assert!(
            read == built,
            "note {note} at vel {vel} renders other samples"
        );
    }
}

#[test]
fn the_marimba_file_loads_from_its_path_with_the_options_it_declares() {
    let synth = mxs::graph::synth_from_json_file("tests/marimba.json").unwrap();
    assert_eq!(synth.name(), "marimba");
    let vel = synth.specs().iter().find(|s| s.name == "vel").unwrap();
    assert_eq!(vel.range, Some((0.05, 1.0)));
    assert_eq!(vel.doc, "How hard the bar is struck.");
    let release = synth.specs().iter().find(|s| s.name == "release").unwrap();
    assert_eq!(release.range, Some((0.0, f64::INFINITY)));
}

fn file(body: &str) -> String {
    format!(
        r#"{{"format": "mxs-synth", "version": 1, "name": "plate", "options": [{{"name": "note", "kind": "note", "default": 60}}], {body}}}"#
    )
}

fn refused(text: &str) -> String {
    match mxs::graph::synth_from_json(text) {
        Ok(_) => panic!("the file was read: {text}"),
        Err(reason) => reason,
    }
}

const RINGS: &str = r#"{"call": "rings", "args": [{"call": "mode", "args": [{"call": "impulse"}], "named": {"hz": {"ref": "freq"}, "level": 0.2, "fade": 1}}], "named": {"seconds": 1}}"#;

#[test]
fn a_file_of_a_newer_version_is_refused_naming_its_version_and_the_newest_this_reads() {
    let newer = file(&format!(r#""clauses": [{{"graph": {RINGS}}}]"#))
        .replace(r#""version": 1"#, r#""version": 2"#);
    let reason = refused(&newer);
    assert!(
        reason.contains("version 2") && reason.contains("version 1"),
        "{reason}"
    );
}

#[test]
fn a_ref_to_a_name_the_file_does_not_define_is_refused_naming_it() {
    let reason = refused(&file(&format!(r#""clauses": [{{"graph": {RINGS}}}]"#)));
    assert!(reason.contains("ref freq names no def"), "{reason}");
    assert!(
        reason.contains("clauses[0].graph.args[0].named.hz"),
        "{reason}"
    );
}

#[test]
fn a_def_that_refers_to_itself_is_refused() {
    let looped = file(&format!(
        r#""defs": {{"freq": {{"call": "add", "args": [{{"ref": "freq"}}, 1]}}}}, "clauses": [{{"graph": {RINGS}}}]"#
    ));
    assert!(refused(&looped).contains("defs.freq refers to itself"));
}

#[test]
fn a_lowpass_without_hz_is_refused_with_the_path_to_the_call() {
    let graph = r#"{"call": "rings", "args": [{"call": "lowpass", "args": [{"call": "noise", "args": [{"name": "white"}]}]}], "named": {"seconds": 1}}"#;
    let reason = refused(&file(&format!(r#""clauses": [{{"graph": {graph}}}]"#)));
    assert_eq!(reason, "clauses[0].graph.args[0]: lowpass needs hz");
}

#[test]
fn a_file_missing_a_comma_is_refused_with_its_line_and_column() {
    let text = "{\n  \"format\": \"mxs-synth\"\n  \"version\": 1\n}";
    let reason = refused(text);
    assert!(
        reason.starts_with("not JSON") && reason.contains("line 3 column 3"),
        "{reason}"
    );
}

#[test]
fn a_call_outside_the_vocabulary_is_refused_with_its_path() {
    let graph = r#"{"call": "rings", "args": [{"call": "wobble"}], "named": {"seconds": 1}}"#;
    let reason = refused(&file(&format!(r#""clauses": [{{"graph": {graph}}}]"#)));
    assert_eq!(
        reason,
        "clauses[0].graph.args[0]: wobble is not in the vocabulary"
    );
}

#[test]
fn an_effect_file_is_not_read_as_a_synth() {
    let reason = refused(r#"{"format": "mxs-effect", "version": 1, "name": "dark", "graph": 0}"#);
    assert!(reason.contains("mxs-effect"), "{reason}");
}

fn through(effect: Effect) -> Vec<f32> {
    let mut engine = Engine::offline(RATE);
    engine.register(mxs::graph::synth_from_json(MARIMBA).unwrap());
    let name = effect.name().to_string();
    engine.register_effect(effect);
    let chain = engine.start_effects(&[(name, vec![])]).unwrap();
    engine
        .play(&[(
            0.0,
            Event::Note {
                id: None,
                synth: "marimba".to_string(),
                options: vec![("note".to_string(), Value::Number(72.0))],
                chain: Some(chain),
                held: false,
            },
        )])
        .unwrap();
    engine.render(1.0).unwrap().0
}

#[test]
fn an_effect_file_of_a_lowpass_filters_a_chain_as_the_same_effect_built_in_rust() {
    let text = r#"{"format": "mxs-effect", "version": 1, "name": "dark",
        "options": [{"name": "hz", "kind": "hz", "default": 400, "range": [20, 20000]}],
        "graph": {"call": "lowpass", "args": [{"call": "input"}], "named": {"hz": {"option": "hz"}}}}"#;
    let built = mxs::graph::effect(
        "dark",
        vec![Spec::hz("hz").default(400.0).range(20.0, 20000.0)],
        Graph::call(
            "lowpass",
            vec![call("input", vec![])],
            vec![("hz", opt("hz"))],
        ),
    )
    .unwrap();
    let read = through(mxs::graph::effect_from_json(text).unwrap());
    assert!(read.iter().any(|s| s.abs() > 0.001));
    assert!(read == through(built));
    let dry = render(mxs::graph::synth_from_json(MARIMBA).unwrap(), 72.0, 0.6).0;
    let energy = |s: &[f32]| s.iter().map(|x| x * x).sum::<f32>();
    assert!(energy(&read) < energy(&dry[..read.len()]) * 0.9);
}

/// The 64-bit FNV-1a hash of `samples`' bits, as tests/defaults.rs and the Python package's tests take it.
#[cfg(all(target_os = "macos", target_arch = "aarch64", debug_assertions))]
fn fnv(samples: impl Iterator<Item = f32>) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for sample in samples {
        for byte in sample.to_bits().to_le_bytes() {
            hash = (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    hash
}

// The Python package's tests check its render of the marimba file against this hash, of a debug build on Apple silicon, as tests/defaults.rs's are.
#[test]
#[cfg(all(target_os = "macos", target_arch = "aarch64", debug_assertions))]
fn the_marimba_file_renders_middle_c_to_the_samples_the_python_package_checks() {
    let (left, right) = render(mxs::graph::synth_from_json(MARIMBA).unwrap(), 60.0, 0.6);
    let second = RATE as usize;
    let hash = fnv(left[..second].iter().chain(&right[..second]).copied());
    assert_eq!(format!("{hash:016x}"), "aa2f2eeca3b73ad9");
}

#[test]
fn mxs_s_marimba_renders_as_the_marimba_file_to_within_60_db() {
    let shipped = || {
        mxs::synths::all()
            .into_iter()
            .find(|s| s.name() == "marimba")
            .unwrap()
    };
    for (note, vel) in [(36.0, 0.3), (52.0, 0.55), (68.0, 0.8), (84.0, 1.0)] {
        let file = render(mxs::graph::synth_from_json(MARIMBA).unwrap(), note, vel).0;
        let ours = render(shipped(), note, vel).0;
        let power = |xs: &[f32]| xs.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
        let apart: Vec<f32> = file.iter().zip(&ours).map(|(a, b)| a - b).collect();
        let db = 10.0 * (power(&apart) / power(&file)).log10();
        println!("note {note} at vel {vel}: {db:.1} dB apart");
        assert!(db < -60.0, "note {note} at vel {vel} is {db:.1} dB apart");
    }
}
