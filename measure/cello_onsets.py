"""Usage: python cello_onsets.py IOWA_CELLO_FOLDER src/synths/cello

The first quarter second of the first note of Iowa cello files, as onsets to play under the synth.

Each onset starts 10 ms before the note crosses 35 dB below the file's loudest 5 ms and 15 dB above its noise floor, fades out over its last 150 ms, is scaled so the note's level while it holds, from 0.6 to 1.2 seconds in, is 1, and is resampled to the pitch of the note it is named after, as the players' onsets are up to 71 cents off.
"""
import sys, os
import numpy as np
import soundfile as sf
from scipy.signal import resample, resample_poly

IOWA = sys.argv[1]
OUT = sys.argv[2]
FILES = {
    "pp": {"C2": "sulC.C2Gb2", "G2": "sulG.G2Bb2", "D3": "sulD.D3B3", "A3": "sulA.A3B3", "C4": "sulA.C4Bb4", "B4": "sulA.B4Ab5"},
    "mf": {"C2": "sulC.C2Ab2", "G2": "sulG.G2Bb2", "D3": "sulD.D3B3", "A3": "sulA.A3B3", "C4": "sulA.C4C5", "C5": "sulA.C5Ab5"},
    "ff": {"C2": "sulC.C2A2", "G2": "sulG.G2B2", "D3": "sulD.D3B3", "A3": "sulA.A3B3", "C4": "sulA.C4B4", "C5": "sulA.C5Bb5"},
}

NOTES = {"C2": 36, "G2": 43, "D3": 50, "A3": 57, "C4": 60, "B4": 71, "C5": 72}


def tune(onset, rate, freq):
    """Resamples an onset so its pitch from 80 ms to its end, found by autocorrelation within a fifth of freq, is freq."""
    w = onset[int(0.08 * rate) :]
    lags = np.arange(int(rate / (freq * 1.5)), int(rate / (freq / 1.5)) + 1)
    c = np.array([np.dot(w[: len(w) - lags[-1]], w[l : l + len(w) - lags[-1]]) for l in lags])
    i = int(c.argmax())
    shift = 0.5 * (c[i - 1] - c[i + 1]) / (c[i - 1] - 2 * c[i] + c[i + 1])
    found = rate / (lags[i] + shift)
    print(f"  found {1200 * np.log2(found / freq):+.0f} cents", end="")
    return resample(onset, round(len(onset) * found / freq))


for dynamic, notes in FILES.items():
    for note, name in notes.items():
        x, rate = sf.read(f"{IOWA}/Cello.arco.{dynamic}.{name}.aiff", always_2d=True)
        x = resample_poly(x.mean(axis=1), 160, 147)
        rate = 48000
        hop = rate // 200
        rms = np.sqrt(np.convolve(x ** 2, np.ones(hop) / hop, mode="same"))
        floor = np.percentile(rms[rms > 0], 10)
        start = int(np.argmax(rms > max(rms.max() * 10 ** (-35 / 20), floor * 10 ** (15 / 20)))) - rate // 100
        held = np.sqrt((x[start + int(0.6 * rate) : start + int(1.2 * rate)] ** 2).mean())
        onset = x[start : start + rate // 4] / held
        fade = int(0.15 * rate)
        onset[-fade:] *= np.cos(np.linspace(0, np.pi / 2, fade)) ** 2
        onset[: rate // 200] *= np.linspace(0, 1, rate // 200)
        onset = tune(onset, rate, 440 * 2 ** ((NOTES[note] - 69) / 12))
        path = os.path.join(OUT, f"onset_{dynamic}_{note}.wav")
        sf.write(path, onset.astype(np.float32), rate, subtype="FLOAT")
        print(path, f"start {start / rate:.3f} s, peak {np.abs(onset).max():.2f} against the held level")
