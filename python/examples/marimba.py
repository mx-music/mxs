"""Writes mxs's marimba as an mxs synth file, marimba.json, and plays a phrase on it: into phrase.wav, or aloud with --live."""

import json
import sys
import time

import mxs

defs = {}


class E:
    """A value worked out from the note's options: arithmetic on it writes calls instead of computing."""

    def __init__(self, graph):
        self.graph = graph

    def _call(self, name, *args):
        return E({"call": name, "args": [encode(a) for a in args]})

    def __add__(self, o): return self._call("add", self, o)
    def __radd__(self, o): return self._call("add", o, self)
    def __sub__(self, o): return self._call("sub", self, o)
    def __rsub__(self, o): return self._call("sub", o, self)
    def __mul__(self, o): return self._call("mul", self, o)
    def __rmul__(self, o): return self._call("mul", o, self)
    def __truediv__(self, o): return self._call("div", self, o)
    def __rtruediv__(self, o): return self._call("div", o, self)
    def __pow__(self, o): return self._call("pow", self, o)
    def __rpow__(self, o): return self._call("pow", o, self)
    def __lt__(self, o): return self._call("lt", self, o)
    def __gt__(self, o): return self._call("gt", self, o)


def encode(value):
    if isinstance(value, E):
        return value.graph
    if isinstance(value, list):
        return [encode(v) for v in value]
    return float(value)


def call(name, *args, **named):
    graph = {"call": name}
    if args:
        graph["args"] = [encode(a) for a in args]
    if named:
        graph["named"] = {k: encode(v) for k, v in named.items()}
    return E(graph)


def let(name, value):
    """Names a value once, so the file holds it once however often it is used."""
    defs[name] = encode(value)
    return E({"ref": name})


def option(name): return E({"option": name})
def maximum(a, b): return call("max", a, b)
def minimum(a, b): return call("min", a, b)
def log2(x): return call("log2", x)
def hz(x): return call("hz", x)
def db(x): return call("db", x)
def select(condition, then, otherwise): return call("select", condition, then, otherwise)


def clamp(x, low, high): return call("clamp", x, low, high)
def exp(x): return call("exp", x)


# Each of the marimba's partials as a multiple of the note, and its shape at vel 40, 80 and 127 out of 127, at every sixth note from C2 to C7.
# A shape is two resonators: the first's level in dB against the peak of the note's own partial over the first 20 ms, both their time constants' natural logarithms, and the second's level as a share of the first's, negative where the partial swells.
# Read from the University of Iowa's 183 yarn recordings at pp, mf and ff as fitted by gradient descent.
MODES = [
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
]
# A time constant times the natural logarithm of 1000 is the seconds a resonator takes to fall 60 dB.
FALL = 6.907755278982137
VELS = [40 / 127, 80 / 127, 1]

note = option("note")
freq = let("freq", hz(note))
vel = option("vel")

# Where the note and vel sit among the shapes: the four around them sound in shares, straight along the notes and along the vel's octaves.
at = let("at", clamp((note - 36) / 6, 0, 10))
low = let("low", minimum(at - call("mod", at, 1), 9))
v = let("v", maximum(vel, VELS[0]))
soft = let("soft", call("le", v, VELS[1]))
up = let("up", select(soft, log2(v / VELS[0]) / log2(VELS[1] / VELS[0]), log2(v / VELS[1]) / log2(VELS[2] / VELS[1])))
past = let("past", at - low)
along = [let(f"note{i}", call("eq", low, i) * (1 - past) + call("eq", low, i - 1) * past) for i in range(11)]
across = [let(f"vel{j}", value) for j, value in enumerate([select(soft, 1 - up, 0), select(soft, up, 1 - up), select(soft, 0, up)])]

# The note rings as long as the slowest fundamental among the four shapes around it.
lower = select(soft, 0, 1)
ring = 0
for j in range(3):
    for i in range(11):
        corner = maximum(call("eq", lower, j), call("eq", lower, j - 1)) * maximum(call("eq", low, i), call("eq", low, i - 1))
        ring = maximum(ring, select(corner, FALL * exp(MODES[0][1][j][i][1]), 0))
ring = let("ring", ring)

strike = let("strike", call("impulse"))
modes = []
for m, (k, shapes) in enumerate(MODES):
    heard = select(freq * k < 15_000, 1, 0)
    for j in range(3):
        for i in range(11):
            level, fade, second, second_fade = shapes[j][i]
            if level < -100:
                continue
            # Resonators at one frequency struck together add up, so the shapes around the note sound in their shares, and those of no share drop out.
            first = let(f"level{m}_{j}_{i}", heard * (1.25 * (along[i] * across[j])) * db(level))
            modes.append(call("mode", strike, hz=freq * k, level=first, fade=FALL * exp(fade)))
            modes.append(call("mode", strike, hz=freq * k, level=first * second, fade=FALL * exp(second_fade)))
struck = call("mix", modes)


def band(low, poles, high, release, level):
    sound = call("lowpass", call("noise", E({"name": "white"})), hz=low, poles=poles)
    sound = call("highpass", sound, hz=high)
    return call("gain", call("perc", sound, attack=0.0005, release=release), level)


tap = call("add", band(2200, 6, 1000, 0.12, 0.55), band(4000, 4, 2500, 0.05, 0.3))
sound = call("gain", call("add", struck, call("mul", tap, (vel / 0.6) ** 0.5)), 0.1 * (vel / 0.6) ** 1.9)
graph = call("rings", call("note_envelope", sound), seconds=ring * 4 / 3)


def spec(name, kind, default, doc, low=None, high=None):
    out = {"name": name, "kind": kind, "default": default, "doc": doc}
    if low is not None:
        out["range"] = [low, high]
    return out


synth = {
    "format": "mxs-synth",
    "version": 1,
    "name": "marimba",
    "doc": "A marimba, measured from the University of Iowa's marimba struck with yarn mallets.",
    "options": [
        spec("note", "note", 52, "The pitch."),
        spec("vel", "float", 0.6, "How hard the bar is struck.", 0.05, 1),
        spec("attack", "seconds", 0, "How long the note takes to rise to attack_level.", 0, None),
        spec("decay", "seconds", 0, "How long it then takes to fall to decay_level.", 0, None),
        spec("sustain", "seconds", 1000, "How long it then holds.", 0, None),
        spec("release", "seconds", 0.3, "How long it then takes to fall silent.", 0, None),
        spec("attack_level", "float", 1, "The level the attack rises to."),
        spec("decay_level", "float", -1, "The level the decay falls to; sustain_level when below 0."),
        spec("sustain_level", "float", 1, "The level the note holds at."),
    ],
    "defs": defs,
    "clauses": [{"matches": {}, "graph": encode(graph)}],
}
with open("marimba.json", "w") as file:
    json.dump(synth, file, separators=(",", ":"))

live = "--live" in sys.argv
engine = mxs.Engine.live() if live else mxs.Engine.offline()
engine.load_synth("marimba.json")
start = engine.now + 0.1
phrase = [(0, 60, 0.8), (0.25, 64, 0.5), (0.5, 67, 0.6), (0.75, 72, 0.9), (1.25, 71, 0.5), (1.5, 67, 0.6), (2, 64, 0.5), (2.5, 60, 0.8)]
for beat, note, vel in phrase:
    engine.note(start + beat, "marimba", note=note, vel=vel)
if live:
    time.sleep(6)
else:
    engine.record_output("phrase.wav")
    engine.render(6)
    engine.stop_recording_output()
