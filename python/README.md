# mxs for Python

mxs is a sound engine whose synths are fitted to recordings of real instruments.
This package plays it from Python: an engine live on a sound card or offline, mxs's synths and effects, and synths and effects read from files.

Install it with `pip install mxs-audio` and import it as `mxs`, or build it from mxs's `python` folder with `pip install .`, which needs a Rust toolchain.

```python
import mxs

engine = mxs.Engine.offline(48000)
engine.note(0, "piano", note=60, vel=0.6)
left, right = engine.render(1.0)
```

- `mxs.Engine.offline(rate)` renders when asked, and `mxs.Engine.live(device)` plays through the output device named, or the default one.
- `note(at, synth, **options)` plays a note at `at` seconds; `id=` lets `set`, `release` and `stop` reach it later, `chain=` plays it into a chain from `start_effects`, and `held=True` makes it sound until released.
- `load_sample(path)` and `sample(at, sample, **options)` play a sound file.
- `load_synth(path)` and `load_effect(path)` register a synth or an effect from a file in the format mxs's `graph` module describes.
- `render(seconds)` gives an offline engine's next seconds as two arrays of floats, and `record_output(path)` and `stop_recording_output()` write the output to a WAV file.
- An error in mxs raises `mxs.Error` with mxs's message.

[examples/marimba.py](examples/marimba.py) writes mx's marimba as a file, the way a Python program writes an instrument, and plays a phrase on it: run it as `python examples/marimba.py` to write `phrase.wav`, or with `--live` to hear it.

The tests run with `python -m unittest discover tests` after `maturin develop`.
