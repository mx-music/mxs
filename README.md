# mxs

mxs is a sound engine in Rust, built on [FunDSP](https://github.com/SamiPerttu/fundsp).
You give it notes, each stamped with a time, and it plays them on the sound card or renders them offline, each starting at its exact sample.
It comes with a library of instruments, most of them fitted to recordings of the real ones, and with the usual effects.

mxs is the engine of [mx](https://github.com/mx-music/mx), a library for writing music in Elixir, and a library of its own: everything mx plays goes through the API described here.

## A first program

Add mxs to a project with `cargo add mxs`, then play four notes on the marimba:

```rust,no_run
use mxs::{Engine, Event, synths};
use std::{thread, time::Duration};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::live()?;
    synths::all().into_iter().for_each(|s| engine.register(s));

    let start = engine.now() + 0.1;
    let events: Vec<(f64, Event)> = [60.0, 64.0, 67.0, 72.0]
        .iter()
        .enumerate()
        .map(|(i, &note)| {
            let at = start + 0.5 * i as f64;
            (at, Event::note(None, "marimba", &[("note", note), ("vel", 0.7)]))
        })
        .collect();
    engine.play(&events)?;

    thread::sleep(Duration::from_secs(4));
    Ok(())
}
```

Using mxs takes three steps:

1. Make an engine, and register the synths it should know by name.
2. Describe the music as events: a time in seconds on the engine's clock, and what to play.
3. Hand the events to `play`. The engine sounds each one at its time, so they can all be given ahead.

`note` is a MIDI note, where 60 is middle C, and `vel` is how hard the note is played, up to 1.

## Rendering offline

`Engine::offline` plays into memory instead of the sound card, as fast as the machine allows, and gives the same samples on every run.

```rust
use mxs::{Engine, Event, RATE, synths};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));

    let chord = [48.0, 55.0, 64.0].map(|note| {
        let options = [("note", note), ("vel", 0.6), ("sustain", 2.0)];
        (0.0, Event::note(None, "piano", &options))
    });
    engine.play(&chord)?;

    let (left, right) = engine.render(4.0)?;
    assert_eq!(left.len(), 4 * RATE as usize);
    assert_eq!(left.len(), right.len());
    Ok(())
}
```

`render` returns the left and the right channel as samples at 48 kHz, and `record_output` writes them to a WAV file.

An offline engine also runs in WebAssembly, as `wasm32-unknown-unknown`, with no imports, so a web page or Node can render with it.
There, a sample is loaded from its bytes with `load_sample_bytes`, and a live engine, worker threads, files read by path, MIDI and audio input do not work.
Built without the default `tables` feature, the module carries none of the instruments' measured tables, and a page loads only those it plays, as [the guide to tables](docs/guide/tables.md) shows.

## What it does

Each of these has a guide, with diagrams and programs that run as the crate's tests.

| Guide | What it covers |
| --- | --- |
| [How the engine is built][architecture] | A live engine fills audio on a thread that never allocates or waits, and builds voices on another. An offline engine is repeatable to the sample. A synth that panics does not take the process down. |
| [Events and the life of a note][events] | Notes, samples and live input are started, changed, let go and stopped by events. Every option is checked against what its synth declares before anything plays. A voice limit steals the quietest notes. |
| [Effects, parts and the signal path][signal_path] | Sounds play into chains of effects. A part is a chain with a name, a gain, a mute and a solo. |
| [Frames, seconds and beats][time] | Events are timed in frames, in seconds, by the system's clock or in beats. The engine keeps a tempo map, so music handed over in beats follows a tempo that changes after. |
| [What the engine reports][reports] | A subscriber is sent the clock, the levels, wave and spectrum of the output, the count of notes, the beat, and the warnings. |
| [Playing live][live] | The output device, its rate and its buffer are chosen by name. The engine says how long filling takes, how many buffers were late, and how far its clock has drifted from the system's. A MIDI keyboard plays a synth. |
| [A synth or an effect of your own][building] | A synth is a name, its options and a function that returns a FunDSP unit. It can also be built from data at run time, with no compiling. |
| [Samples, live input and recording][samples] | FLAC, WAV and AIFF files play at any rate, the sound card's input plays through the effects, and the output is recorded to a WAV file. |

## The instruments and the effects

`synths::all()` gives every synth that comes with the crate, and `synths::families()` gives them by family: the piano and plucked strings, bowed strings, winds, organs and reeds, tuned percussion, a drum kit with Latin, Brazilian and Indian percussion, and basic waveforms.

- Most of them are fitted to recordings of the real instruments, and the documentation of each of those names its recordings.
- The winds are physical models, so a change of pitch while a note holds is a slur.
- Each synth declares its options with a kind, a default, a range and one sentence of documentation, and the engine checks every note against them.

[The catalog of synths][synths] lists every one with its options, and [the catalog of effects][effects] does the same for the effects.
Both are written from the code, and a program can read the same lists:

```rust
use mxs::synths;

fn main() {
    for synth in synths::all() {
        let options: Vec<&str> = synth.specs().iter().map(|spec| spec.name.as_str()).collect();
        println!("{}: {}", synth.name(), options.join(", "));
        println!("  {}", synth.doc());
    }
}
```

## From the command line

A clone of the repository plays without any code written:

```sh
cargo run --release --bin play                # a short phrase on the piano
cargo run --release --bin play -- berimbau    # four bars of the toque of Angola
cargo run --release --bin play -- cello out.wav
```

`play [synth] [out.wav]` plays through the default output, or writes the WAV file when you give one.
Any synth other than the piano and the berimbau plays the phrase's top line an octave down.

`render <synth> <seconds> <out.wav> [name=value ...]` writes one note to a stereo 48 kHz float WAV:

```sh
cargo run --release --bin render -- piano 3 piano.wav note=60 vel=0.8
```

`keys` plays a synth from a MIDI keyboard.
Keys play notes, the sustain pedal holds them and the pitch bend wheel bends them.

```sh
cargo run --release --features keys --bin keys -- --list
cargo run --release --features keys --bin keys -- --synth piano --buffer 128
```

`--input` and `--device` pick the MIDI input and the output device by name.

## Examples

`cargo run --release --example NAME` runs one of the programs in `examples/`.
Each of these plays through the default output, or renders into the WAV file given after `--`.

| Example | What it shows |
| --- | --- |
| `phrase` | The shortest program that plays something. |
| `parts` | Two parts with their effects, a filter that opens, a mute and a gain. |
| `own_synth` | Two synths written in the example, one that rings and one that slides under a held note. |
| `sample` | A sound file played at several rates, forwards and backwards. |
| `beats` | Music handed over in beats, a bar at a time, while the tempo slows. |
| `salsa` | A short jazzy salsa on the piano, a plucked bass and Latin percussion. |
| `pandeiro` | A pandeiro solo, as a primer on phrasing one drum. |

`devices` lists the output and input devices.
The others check timing, such as `drift` and `clock_check`, or cost, such as `cost`.
Those that record what plays need the BlackHole 2ch virtual device installed.

## Tests and checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features
```

The checks of a live engine play through BlackHole 2ch, a virtual sound card, so that nothing is heard, and run only when asked for:

```sh
cargo test --release --test live -- --ignored --test-threads 1
```

The catalogs of synths and effects in `docs/` are written from the code.
After a synth or an effect changes, write them again:

```sh
MXS_CATALOG=write cargo test --release --test catalog
```

The check that an offline engine renders in WebAssembly as it does natively needs the `wasm32-unknown-unknown` target and Node, and runs only when asked for:

```sh
cargo test --release --test webassembly -- --ignored
```

The checks of a build without the measured tables compiled in run apart:

```sh
cargo test --no-default-features --test tables
```

`cargo doc --open` builds the documentation with its diagrams, which need a connection to load the script that draws them.

## License

mxs is under the [MIT license](https://github.com/mx-music/mxs/blob/main/LICENSE).

A sample's stretch and pitch and the `pitch_shift` effect are written for mxs from published methods, WSOLA (Verhelst and Roelands, 1993) and overlapping grains of a delay line, and take no code from any library under the GPL or LGPL, such as Rubber Band or SoundTouch.

[architecture]: docs/guide/architecture.md
[events]: docs/guide/events.md
[signal_path]: docs/guide/signal_path.md
[time]: docs/guide/time.md
[reports]: docs/guide/reports.md
[live]: docs/guide/live.md
[building]: docs/guide/building.md
[samples]: docs/guide/samples.md
[synths]: docs/synths.md
[effects]: docs/effects.md
