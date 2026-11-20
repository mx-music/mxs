"""Writes mx's marimba sketch as an mxs synth file, marimba.json, and plays a phrase on it: into phrase.wav, or aloud with --live."""

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


def mallet(hz, corner, slope):
    return slope * maximum(log2(hz / corner), 0) + 24 * maximum(log2(hz / 3000), 0)


note = option("note")
freq = let("freq", hz(note))
vel = option("vel")

# The seconds a note fades 60 dB in: 4.6 at C2, halving every 17 semitones, and every 16 more above F#5.
ring = let("ring", 4.6 * 2 ** ((note - 36) / -17) * 2 ** (maximum(note - 78, 0) / -16))
# The tube takes a moment to answer the bar, so the note starts at a quarter of its level.
swell = 0.45 * 2 ** ((note - 36) / -18)

fourth = (-12 - mallet(freq * 4, 600, 57 * (1 - vel)) - 10 * maximum(log2(freq * 4 / 900), 0)
          - 20 * maximum(log2(370 / (freq * 4)), 0))


def overtone(n):
    return 10 + 14.5 * log2(vel) - mallet(freq * n, 1100, 33.4 - 11.4 * vel)


bar = [(4, fourth, 0.3), (10, overtone(10), 0.1), (19, overtone(19) - 8, 0.08), (30, overtone(30) - 12, 0.06)]
twelfth = select(note < 46, -82, select(note > 62, -82, -22))
tube = [(n, level - mallet(freq * n, 600, 12 + 45 * (1 - vel)), 0.35)
        for n, level in [(3, twelfth), (5, -26), (7, -26), (9, -26)]]

strike = let("strike", call("impulse"))
struck = call("mix", [
    call("mode", strike, hz=freq, level=1.25, fade=ring),
    call("mode", strike, hz=freq, level=-1, fade=swell),
] + [
    # Overtones above 1.5 kHz die sooner the higher they are.
    call("mode", strike, hz=freq * n, level=select(freq * n < 15_000, db(level), 0),
         fade=ring * fade * minimum(1500 / (freq * n), 1) ** 0.8)
    for n, level, fade in bar + tube
])


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
