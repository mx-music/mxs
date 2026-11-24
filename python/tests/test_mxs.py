"""The package's tests: python -m unittest discover tests, after maturin develop, which builds the debug build the hashes are of."""

import os
import platform
import subprocess
import sys
import tempfile
import unittest

import mxs

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
APPLE_SILICON = sys.platform == "darwin" and platform.machine() == "arm64"


def fnv(*arrays):
    """The 64-bit FNV-1a hash of the samples' bytes, as mxs's tests/defaults.rs takes it."""
    hash = 0xCBF29CE484222325
    for samples in arrays:
        for byte in samples.tobytes():
            hash = ((hash ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{hash:016x}"


class TestEngine(unittest.TestCase):
    def test_an_offline_engine_renders_a_second_of_piano_into_two_arrays_of_48000_samples(self):
        engine = mxs.Engine.offline(48000)
        engine.note(0, "piano", note=60)
        left, right = engine.render(1.0)
        self.assertEqual((len(left), len(right)), (48000, 48000))
        self.assertGreater(max(abs(s) for s in left), 0.01)

    @unittest.skipUnless(APPLE_SILICON, "the hashes are of Apple silicon")
    def test_the_piano_renders_the_samples_mxs_renders_in_rust(self):
        with open(os.path.join(ROOT, "tests", "defaults.txt")) as file:
            kept = dict(line.split() for line in file)
        engine = mxs.Engine.offline()
        engine.note(0, "piano")
        self.assertEqual(fnv(*engine.render(2.0)), kept["piano"])

    @unittest.skipUnless(APPLE_SILICON, "the hashes are of Apple silicon")
    def test_the_marimba_file_marimba_py_writes_renders_the_samples_mxs_renders_in_rust(self):
        writer = os.path.join(HERE, "..", "examples", "marimba.py")
        with tempfile.TemporaryDirectory() as folder:
            subprocess.run([sys.executable, writer], cwd=folder, check=True, capture_output=True)
            path = os.path.join(folder, "marimba.json")
            engine = mxs.Engine.offline()
            self.assertEqual(engine.load_synth(path), "marimba")
        engine.note(0, "marimba", note=60, vel=0.6)
        # tests/files.rs in mxs pins the same hash.
        self.assertEqual(fnv(*engine.render(1.0)), "8f7f932fe355e35d")

    def test_a_note_of_a_synth_that_does_not_exist_raises_naming_it(self):
        engine = mxs.Engine.offline()
        with self.assertRaisesRegex(mxs.Error, "kazoo"):
            engine.note(0, "kazoo", note=60)

    def test_a_file_mxs_cannot_read_raises_with_its_reason(self):
        with tempfile.TemporaryDirectory() as folder:
            path = os.path.join(folder, "broken.json")
            with open(path, "w") as file:
                file.write('{"format": "mxs-synth", "version": 2, "name": "x"}')
            with self.assertRaisesRegex(mxs.Error, "version 2"):
                mxs.Engine.offline().load_synth(path)

    def test_an_effect_file_filters_the_notes_played_into_its_chain(self):
        effect = '{"format": "mxs-effect", "version": 1, "name": "dark", "graph": {"call": "lowpass", "args": [{"call": "input"}], "named": {"hz": 300}}}'
        energy = lambda samples: sum(s * s for s in samples)
        with tempfile.TemporaryDirectory() as folder:
            path = os.path.join(folder, "dark.json")
            with open(path, "w") as file:
                file.write(effect)
            heard = []
            for dark in [False, True]:
                engine = mxs.Engine.offline()
                chain = engine.start_effects([(engine.load_effect(path), None)]) if dark else None
                engine.note(0, "saw", note=60, chain=chain)
                heard.append(energy(engine.render(0.5)[0]))
        self.assertLess(heard[1], heard[0] * 0.5)

    def test_the_documentations_program_runs_as_written(self):
        program = os.path.join(HERE, "..", "examples", "marimba.py")
        with tempfile.TemporaryDirectory() as folder:
            subprocess.run([sys.executable, program], cwd=folder, check=True)
            self.assertTrue(os.path.exists(os.path.join(folder, "marimba.json")))
            self.assertGreater(os.path.getsize(os.path.join(folder, "phrase.wav")), 48000 * 2 * 4 * 5)


if __name__ == "__main__":
    unittest.main()
