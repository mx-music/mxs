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

## How far ahead to hand a note over

Building a note's voice is the costly part of starting it: about a millisecond for a piano or a cello note, a tenth of that for a berimbau stroke.

- A live engine builds voices on a thread of its own, in the order the notes were handed over, so `play` returns once the events are checked, and neither the caller nor the audio thread waits for a voice.
- A note therefore has to be handed over before its time by at least what it and the notes handed over ahead of it take to build: about 12 ms for a chord of ten piano notes.
- A note handed over with less time starts as soon as it is built, and `warnings` counts it among the events that arrived after their time.
- A change, a release or a stop handed over after a note reaches it, though the note's voice may not be built yet when the call returns.
- An offline engine has no thread of its own and builds each voice in the call that hands the note over.

## Whether the engine keeps up

A live engine has to fill each buffer before the sound card needs it, and a buffer filled too late is heard as a gap.

- `load` says how long filling took as a share of the time the buffers last, the mean and the worst since it was last called, and how many buffers were late and by how much. An offline engine has no deadline, so it counts none late, and still gives the shares, each stretch it renders counting as a buffer.
- `warnings` says how many buffers were late since it was last called, beside the events that arrived after their time and the notes stolen.
- `behind` says how many seconds the engine's clock has fallen behind the system's since it started, which is the gaps in the sound and the drift between the two clocks.

After a gap, the engine's clock is behind for good, and `seconds_at` and `play_at` catch up with it by at most a millisecond a second.
Until they have, an event given a system time plays late by what is left of the gap; `behind` shows the gap within a second.

## Parts

A part is a group of sounds with a name, which can be turned down, muted or soloed as one.

```rust
use mxs::{Engine, Event, RATE, Value, synths};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));

    let bass = engine.start_part("bass", &[])?;
    let note = Event::Note {
        id: None,
        synth: "piano".to_string(),
        options: vec![("note".to_string(), Value::Number(36.0))],
        chain: Some(bass),
    };
    engine.play(&[(0.0, note)])?;
    engine.set_gain("bass", 0.5, 0.0)?;
    engine.mute("bass", true, 1.0)?;
    engine.mute("bass", false, 2.0)?;
    engine.render(3.0)?;
    Ok(())
}
```

- `start_part` takes the part's name and its effects, as `start_effects` takes them, or none. It returns what a note, a sample or a live input names as its `chain` to play into the part, and `part` gives the same for a name later.
- `set_gain`, `mute` and `solo` take the part's name and a time in seconds, and reach their level over 10 ms from that time, so nothing clicks.
- They apply after the part's effects, so a reverb's tail is turned down with its part.
- A muted part plays on unheard: a note handed over before the mute is not heard, and a note that started while the part was muted is heard from where it has got to once the part is unmuted.
- While any part is soloed, only the soloed parts are heard. A part both muted and soloed is silent. A sound that plays into no part is heard as ever.
- `stop_part` removes a part once its sounds and its effects' tails have ended, and frees its name at once.
- A recording of the output holds what is heard.
- A levels report gives each part's level after its gain, by its name.
- A name that no part was started under is refused, and the error names it.

## Musical time

The engine keeps a tempo map, which says at which of its seconds each beat falls, so a caller can hand events over in beats, bars ahead, and still change the tempo.

```rust
use mxs::{Engine, Event, Pass, RATE, Tempo, synths};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    engine.start_part("lead", &[])?;

    engine.place_beats(0.0)?;
    engine.set_tempo(0.0, Tempo::Hold(96.0))?;
    let notes = [60.0, 64.0, 67.0, 72.0];
    let events = notes.iter().enumerate().map(|(beat, note)| {
        let options = [("note", *note), ("sustain", 0.5), ("release", 0.5)];
        (beat as f64, Event::note(None, "piano", &options))
    });
    let pass = Pass {
        from: 0.0,
        to: 4.0,
        label: 1,
        notice: Some(1.0),
        events: events.collect(),
    };
    engine.pass("lead", &pass)?;
    engine.set_tempo(2.0, Tempo::Move { from: 96.0, to: 60.0, over: 2.0 })?;
    engine.cut("lead", 3.0)?;
    engine.render(4.0)?;
    Ok(())
}
```

- `place_beats` places beat 0 at one of the engine's seconds. It is at second 0 until placed.
- `set_tempo` changes the tempo from a beat on, and replaces whatever the map held from that beat on. `Tempo::Hold` holds a tempo in beats a minute, and `Tempo::Move` moves evenly from one tempo to another over a number of beats, then holds the second. The tempo is 120 until changed.
- `pass` hands a part a pass: a first beat, a last beat, a label of the caller's choosing, and events each at its beat. Each event starts at the frame its beat falls on.
- In a pass, an option a synth or an effect declares in beats, and every `_slide`, is given in beats, and lasts as long as those beats last at the tempo of the event's beat. In `play` those options are in seconds, as before.
- The engine builds each event of a pass and places it at its frame 150 ms before it sounds. A change to the map moves every event further off than that, and leaves the nearer ones where they were.
- `cut` drops every event of a part at or after a beat that has not started, so that another pass can be handed over in its place. A note that started before the cut rings on, and a release dropped with the rest no longer ends it.
- An event handed over after its beat plays at once when it is less than a quarter of a beat late, and is dropped otherwise. `warnings` counts both.
- `start_part_in_beats` starts a part whose effects take their options in beats. Those an effect lets change while it runs, such as the time between an echo's repeats, follow the tempo as it changes, read again at each change and every quarter beat while the tempo moves. One set in seconds by `play` stops following.
- `beat` gives the beat of the frame the engine fills next, `beat_at` the beat at one of its seconds, and `beat_seconds` the seconds at which a beat falls.
- Beats are floating point numbers, so a third of a beat is approximate. Each event is placed from its own beat, never by adding lengths up.
- Events at seconds play beside events in beats, wherever the map is.
- An offline render gives the same samples whatever the length of each stretch `render` is asked for.
- `fill` is the layer under the tempo map. It fills one block after another from events given at frames by `play_at_frames`, or at seconds, and needs neither the map nor a sound card. It does not place events in beats, which `render` does.
- Reading song files, patterns and loops stays with the caller.

## What the engine reports

`subscribe` takes the kinds of report wanted and returns the subscription, which the engine then sends reports to without being asked.

```rust,no_run
use mxs::{Engine, Report, Topic};
use std::time::Duration;

fn main() -> Result<(), mxs::Error> {
    let engine = Engine::live()?;
    let reports = engine.subscribe(&[Topic::Levels, Topic::Voices]);
    while let Some(report) = reports.wait(Duration::from_secs(1)) {
        match report {
            Report::Levels(levels) => println!("{:.2} s: peak {:?}", levels.seconds, levels.peak),
            Report::Voices { sounding, .. } => println!("{sounding} notes sound"),
            _ => {}
        }
    }
    Ok(())
}
```

Every report carries the engine's seconds of the sample it describes.

| Kind | What it holds | When it is sent |
| --- | --- | --- |
| `Topic::Clock` | The engine's seconds, the system time at which the sound card plays them, and the output's delay, which that time includes. | At once, whenever the two clocks move apart by more than 2 ms, and at least once a second. An offline engine has no system time and sends none. |
| `Topic::Levels` | For the left and the right channel of the main output, the loudest sample and the RMS level, where 1 is full scale; whether a sample reached full scale; the wave as 64 points from -1 to 1; the level in dB of 25 bands from 50 Hz to 16 kHz, each about a third of an octave, which `Levels::band` gives the edges of; and the loudest sample and the RMS level of each part, by its name. | Twenty times a second, each report covering the twentieth of a second before it. The bands cover the last 8192 frames. |
| `Topic::Voices` | How many notes sound. | About once a second. |
| `Topic::Output` | The device's name, the rate in Hz, and the output's delay in milliseconds; an offline engine has no device and no delay. | At once. The engine keeps its device for as long as it runs, so no second one follows. |
| `Topic::Warnings` | One warning, worded as `warnings` words it. | At once for a value held to its range or a note that could not be built, and within a second for the counts of late buffers, late events and stolen notes. |
| `Topic::Beats` | A beat, the engine's seconds at which it falls, the tempo from that beat on, and the system time at which the sound card plays it, which a live engine knows. | At once, with the last one sent, then for each change to the tempo map, up to 150 ms before its beat, and every quarter beat while the tempo moves. |
| `Topic::Passes` | `Report::PassStarted`: the part, the label and the beat of the first event of a pass whose label is another than that of the pass handed to the part before it. `Report::PassEnding`: the part and the last beat of its last pass, when that beat is the pass's `notice` in beats away and no other pass was handed to the part. | Once the engine's clock reaches the seconds the report carries. |

- A kind nobody subscribes to is not worked out, and subscribing changes no sample of the output.
- A live engine works its reports out on a thread of its own: the audio thread only copies its output into a ring buffer while levels are wanted, and never waits.
- An offline engine works them out as it renders, the same reports on every run for the same events and the same calls to `render`.
- `next` takes the oldest report that waits, and `wait` also waits for one to arrive.
- 64 reports wait for a subscriber at most. Past that the oldest are dropped, and a `Report::Dropped` in their place says how many.
- `unsubscribe` ends one kind, and dropping the subscription ends all of them.
- `warnings`, `now` and `clock` answer as before, and a warning sent as a report is still given by `warnings`.

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

The checks of a live engine play through BlackHole 2ch, a virtual sound card, so that nothing is heard, and run only when asked for:

```sh
cargo test --release --test live -- --ignored --test-threads 1
```

The synth and effect lists in the documentation are written from the code.
After a synth or an effect changes, write them again:

```sh
MXS_CATALOG=write cargo test --release --test catalog
```

## License

mxs is under the [MIT license](https://github.com/mx-music/mxs/blob/main/LICENSE).
