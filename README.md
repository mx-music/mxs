# mxs

mxs is a sound engine on FunDSP, with 85 synths and 17 effects.
You register synths and effects by name, send it events stamped with seconds, and it plays them on the default output or renders them offline.

## Hear it

```
cargo run --release --bin play                # a short phrase on the piano
cargo run --release --bin play -- berimbau    # four bars of the toque of Angola
cargo run --release --bin play -- cello out.wav
```

`play [synth] [out.wav]` plays through the default output, or writes the WAV file when you give one.
Any synth other than the piano and the berimbau plays the phrase's top line an octave down.

To render one note:

```
cargo run --release --bin render -- piano 3 piano.wav note=60 vel=0.8
```

`render <synth> <seconds> <out.wav> [name=value ...]` writes a stereo 48 kHz float WAV.

## Play from a MIDI keyboard

```
cargo run --release --features keys --bin keys -- --list
cargo run --release --features keys --bin keys -- --synth piano --buffer 128
```

Keys play notes, the sustain pedal holds them and the pitch bend wheel bends them.
`--input` and `--device` pick the MIDI input and the output device by name.

## Use it as a library

```rust
use mxs::{Engine, Event, RATE, synths};

let mut engine = Engine::offline(RATE);
synths::all().into_iter().for_each(|s| engine.register(s));
engine.play(&[(0.0, Event::note(None, "piano", &[("note", 60.0), ("sustain", 1.0)]))])?;
let (left, right) = engine.render(3.0)?;
```

`Engine::live()` plays on the default output instead, and `play` then takes times from `engine.now()`.
`Event` also starts samples and input channels, changes sounds that are playing, and stops or releases them.

Each synth declares its options with a kind, default, range and documentation.
`engine.synths()` and `engine.effects()` list what is registered, and `Synth::specs()` lists one synth's options.
Almost every synth takes `note`, and most take `vel`.
Pitched synths also take the envelope's `attack`, `decay`, `sustain` and `release`, but most drums take no `sustain`.

You can write your own synth against the same public API that `synths` uses, and register it next to mx's.

## Examples

`cargo run --release --example NAME` runs one of the programs in `examples/`.
`salsa` is a short jazzy salsa on the piano, a plucked bass and Latin percussion, written as a primer on using the engine directly, and plays through the default output or into the WAV file you give it.
`devices` lists the output and input devices.
The others check timing, such as `drift` and `clock_check`, or cost, such as `cost`.
Those that use BlackHole to record what plays need the BlackHole 2ch virtual device installed.

## Tests and checks

```
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features
```

