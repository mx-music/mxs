# mxs

mxs is a sound engine in Rust, built on [FunDSP](https://github.com/SamiPerttu/fundsp).
It comes with 89 synths, most of them fitted to recordings of real instruments, and 17 effects.
You give it notes, each stamped with a time in seconds, and it plays them on the sound card or renders them offline.

mxs is the engine of [mx](https://github.com/mx-music/mx), a library for writing music in Elixir, and it works without mx: everything mx plays goes through the API described here.

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

`Engine::offline` plays into memory instead of the sound card, as fast as the machine allows.

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

`render` returns the left and the right channel as samples at 48 kHz.
The example `salsa` writes them to a WAV file.

## The synths

| Family | Synths |
|---|---|
| Piano and plucked strings | piano, guitar, electric_bass, cavaquinho, bandolim, harp, sitar, tanpura |
| Bowed strings | violin, viola, cello, double_bass |
| Winds | flute, clarinet, oboe, bassoon, tenor_sax, trumpet, trombone, horn, tuba |
| Organs and reeds | organ, harmonium, accordion |
| Tuned percussion | marimba, vibraphone, xylophone, glockenspiel, timpani |
| Drums | a drum kit, Latin and Brazilian percussion, the tabla and the berimbau, 50 synths in all |
| Basic waveforms | saw, square, tri, pulse, noise, fm, pluck, supersaw, acid, brass |

- Most of them are fitted to recordings of the real instruments.
- The winds and the bowed strings are physical models, so a change of pitch while a note holds is a slur.
- Each synth declares its options with a kind, a default, a range and one sentence of documentation, and the engine checks every note against them.

[The documentation of the `synths` module](https://docs.rs/mxs/latest/mxs/synths/) lists every synth with its options, and a program can read the same list:

```rust
use mxs::synths;

fn main() {
    for synth in synths::all() {
        let options: Vec<&str> = synth.specs().iter().map(|spec| spec.name.as_str()).collect();
        println!("{}: {}", synth.name(), options.join(", "));
    }
}
```

[mx's guide to its instruments](https://github.com/mx-music/mx/blob/main/documentation/instruments.md) names the recordings each synth is measured from.

## Effects

Notes play into a chain of effects, in order.
`start_effects` makes a chain and returns the number notes name to play into it.

```rust
use mxs::{Engine, Event, RATE, Value, effects, synths};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all().into_iter().for_each(|e| engine.register_effect(e));

    let echo = ("echo".to_string(), vec![("phase".to_string(), Value::Number(0.25))]);
    let reverb = ("reverb".to_string(), vec![("room".to_string(), Value::Number(0.6))]);
    let chain = engine.start_effects(&[echo, reverb])?;

    let note = Event::Note {
        id: None,
        synth: "vibraphone".to_string(),
        options: vec![("note".to_string(), Value::Number(67.0))],
        chain: Some(chain),
    };
    engine.play(&[(0.0, note)])?;
    engine.render(3.0)?;
    Ok(())
}
```

The effects are bitcrusher, bpf, chorus, compressor, distortion, echo, flanger, hpf, level, lpf, pan, ping_pong, reverb, rlpf, slicer, tremolo and wobble.
[The documentation of the `effects` module](https://docs.rs/mxs/latest/mxs/effects/) lists the options of each.

## Changing a sound while it plays

A note started with an `id` can be changed, let go or stopped later.
`Event::Set` gives options new values, and an option's `_slide` partner makes the change glide over that many seconds.

```rust
use mxs::{Engine, Event, RATE, Target, synths};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));

    let held = Event::note(Some(1), "trombone", &[("note", 50.0), ("sustain", 3.0)]);
    let slur = Event::Set {
        target: Target::Sounds(vec![1]),
        options: vec![("note".to_string(), 55.0), ("note_slide".to_string(), 0.3)],
    };
    let let_go = Event::Release { ids: vec![1] };

    engine.play(&[(0.0, held), (1.0, slur), (2.0, let_go)])?;
    engine.render(3.0)?;
    Ok(())
}
```

A change can also reach every sound of an effect chain, or one of its effects, through `Target`.

## Samples and live input

- `load_sample` reads a FLAC, WAV or AIFF file, and `Event::Sample` plays it.
- `Engine::live_with_input` also opens the sound card's input, and `Event::Input` plays it through an effect chain.
- `record_output` writes what the engine plays to a WAV file.

## A synth of your own

mx's synths are built only from what the crate makes public, so a synth of yours is written the same way and registered next to them.
A synth is a name, its options, and a function that builds one note's sound from the note's options:

```rust
use fundsp::prelude::*;
use mxs::nodes::{db, hz, modes};
use mxs::{Engine, Event, Note, RATE, Spec, Synth};

fn main() -> Result<(), mxs::Error> {
    let plate = Synth::new("plate")
        .option(Spec::note("note").default(60.0).doc("The pitch."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard the plate is struck."))
        .build(|note, _ctx| {
            let freq = hz(note.number("note"));
            let strike = Net::wrap(Box::new(impulse::<U1>()));
            let struck = modes(strike, &[freq, freq * 2.76], &[1.0, db(-6.0)], &[3.0, 1.2]);
            Note::rings(struck * (0.2 * note.number("vel")) as f32, 4.0)
        });

    let mut engine = Engine::offline(RATE);
    engine.register(plate);
    engine.play(&[(0.0, Event::note(None, "plate", &[("vel", 0.9)]))])?;
    let (left, _right) = engine.render(1.0)?;
    assert!(left.iter().any(|sample| sample.abs() > 0.001));
    Ok(())
}
```

- The sound is a FunDSP unit, so FunDSP's own units and operators work, and the `nodes` module adds the ones mx's synths are made of, such as `modes`, a bank of resonators struck by their input.
- A note returns its sound together with how it ends: `Note::rings` after a number of seconds, or `Note::envelope` by the note's envelope options.
- The `graph` module builds a synth from data at run time, with no compiling, which is how mx plays a synth written in Elixir.

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

- `salsa` is a short jazzy salsa on the piano, a plucked bass and Latin percussion, written as a primer on using the engine directly. It plays through the default output, or into the WAV file you give it.
- `devices` lists the output and input devices.
- The others check timing, such as `drift` and `clock_check`, or cost, such as `cost`. Those that record what plays need the BlackHole 2ch virtual device installed.

## Tests and checks

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release --all-features
```

The synth and effect lists in the documentation are written from the code.
After a synth or an effect changes, write them again:

```sh
MXS_CATALOG=write cargo test --release --test catalog
```

## License

mxs is under the [MIT license](https://github.com/mx-music/mxs/blob/main/LICENSE).
