"""Usage: python cello_body.py "IOWA_CELLO_FOLDER/Cello.arco.mf.*.aiff" src/synths/cello/body.wav

The cello body's response, from the noise Iowa's bowed cello holds between its harmonics.

In each held stretch of each file, the level of every 1/6 octave is the 20th percentile of its bins, which falls between the harmonics.
The levels are averaged in dB over every stretch, smoothed, and turned into a minimum-phase impulse response.
"""
import sys, glob
import numpy as np
import soundfile as sf

RATE = 44100
N = 4096
files = sorted(glob.glob(sys.argv[1]))
out = sys.argv[2]

freqs = np.fft.rfftfreq(N, 1 / RATE)
centres = 80 * 2 ** (np.arange(0, 6 * np.log2(16000 / 80) + 1) / 6)
sums = np.zeros(len(centres))
count = 0
window = np.hanning(N)

for path in files:
    x, rate = sf.read(path, always_2d=True)
    assert rate == RATE, rate
    x = x.mean(axis=1)
    frames = np.lib.stride_tricks.sliding_window_view(x, N)[:: N // 2]
    level = np.sqrt((frames ** 2).mean(axis=1))
    loud = level > level.max() * 10 ** (-20 / 20)
    for frame in frames[loud]:
        mag = np.abs(np.fft.rfft(frame * window))
        bands = []
        for c in centres:
            inside = (freqs >= c * 2 ** (-1 / 12)) & (freqs < c * 2 ** (1 / 12))
            near = np.argsort(np.abs(freqs - c))[:8] if inside.sum() < 8 else inside
            bands.append(np.percentile(mag[near], 20))
        bands = 20 * np.log10(np.maximum(np.array(bands), 1e-12))
        if np.all(np.isfinite(bands)):
            sums += bands - bands.max()
            count += 1

envelope = sums / count
smooth = np.convolve(np.pad(envelope, 1, mode="edge"), [0.25, 0.5, 0.25], mode="valid")
for c, e in zip(centres, smooth):
    print(f"{c:7.0f} Hz {e:6.1f} dB")
print(count, "frames", file=sys.stderr)

OUT = 48000
full = np.fft.rfftfreq(N, 1 / OUT)
db = np.interp(np.log2(np.maximum(full, 1)), np.log2(centres), smooth)
mag = 10 ** (db / 20)
cep = np.fft.irfft(np.log(np.maximum(mag, 1e-9)), N)
fold = np.zeros(N)
fold[0] = cep[0]
fold[1 : N // 2] = 2 * cep[1 : N // 2]
fold[N // 2] = cep[N // 2]
ir = np.fft.irfft(np.exp(np.fft.rfft(fold)), N)
ir = ir[:2048] * np.hanning(4096)[2048:]
ir /= np.sqrt((ir ** 2).sum())
sf.write(out, ir.astype(np.float32), OUT, subtype="FLOAT")
