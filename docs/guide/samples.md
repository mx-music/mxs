# Samples, live input and recording

Besides the notes of its synths, an engine plays sound files and the sound card's input, and writes what it plays to a file.
All three go through the same effect chains and parts as a note.

## Playing a sound file

[`Engine::load_sample`] reads a FLAC, WAV or AIFF file into memory and returns the number events name it by.
An `Event::Sample` then plays it, as often as wanted and at any rate.

This program records a piano note to a file, loads the file, and plays it back an octave down and backwards.

```rust
use mxs::{Engine, Event, RATE, Value, check_sample, synths};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    let path = std::env::temp_dir().join("mxs-guide-samples.wav");

    engine.record_output(&path, 0.0)?;
    engine.play(&[(0.0, Event::note(None, "piano", &[("note", 60.0)]))])?;
    engine.render(1.0)?;
    engine.stop_recording_output()?;
    let seconds = check_sample(&path).map_err(mxs::Error::Sample)?;
    assert!((seconds - 1.0).abs() < 0.001);

    let sample = engine.load_sample(&path)?;
    let backwards = Event::Sample {
        id: None,
        sample,
        options: vec![
            ("rate".to_string(), Value::Number(-0.5)),
            ("amp".to_string(), Value::Number(0.8)),
        ],
        chain: None,
    };
    engine.play(&[(1.0, backwards)])?;
    let (left, _right) = engine.render(2.0)?;
    assert!(left.iter().any(|sample| sample.abs() > 0.001));

    std::fs::remove_file(path).ok();
    Ok(())
}
```

The sample player takes these options, besides `amp` and `pan`.
[`Engine::sample_specs`] gives them to a program.

| Option | What it does |
| --- | --- |
| `rate` | How fast the sample plays, where 1 is its own speed and pitch; backwards when below 0. It can be changed while the sample plays. |
| `stretch` | What its length is multiplied by, its pitch kept. It can be changed while the sample plays. |
| `pitch` | How many semitones it is shifted by, its length kept. It can be changed while the sample plays. |
| `start`, `finish` | Where it starts and finishes, as fractions of its length. |
| `attack`, `decay`, `sustain`, `release` and their levels | An envelope, as a note's. `sustain` is the rest of the sample unless given. |
| `normalize` | 1 scales the sample so that its file's loudest moment reaches full scale. |

- A file at another rate than the engine's plays at its own speed and pitch.
- A mono file is placed by `pan`, and a stereo one is balanced by it.
- [`Engine::load_sample_bytes`] loads a file already in memory.
- [`check_sample`] gives a file's length from its headers alone, or says why the file cannot be played, and [`sample_peak`] gives its loudest sample. Neither needs an engine.

## The sound card's input

[`Engine::live_with_input`] opens the default input device beside the output, at the same rate, and [`Settings`] names another.
An `Event::Input` then plays one or two of its channels, counted from 0, until it is stopped.

```rust,no_run
use mxs::{Engine, Event, Value, effects};

fn main() -> Result<(), mxs::Error> {
    let mut engine = Engine::live_with_input()?;
    effects::all().into_iter().for_each(|e| engine.register_effect(e));

    let reverb = ("reverb".to_string(), vec![("room".to_string(), Value::Number(0.8))]);
    let chain = engine.start_effects(&[reverb])?;
    let microphone = Event::Input {
        id: Some(1),
        input: 0,
        channels: 1,
        options: vec![],
        chain: Some(chain),
    };
    engine.play(&[(engine.now(), microphone)])?;

    std::thread::sleep(std::time::Duration::from_secs(10));
    engine.play(&[(engine.now(), Event::Stop { ids: vec![1] })])?;
    Ok(())
}
```

- [`Engine::inputs`] says how many input channels the engine reads, which is 0 unless it was started with input.
- An input takes `amp` and `pan`, and plays into a chain or a part as a note does.
- Live input is not a note: it does not count toward the voice limit and is never stolen.
- When the input device runs ahead of the output, the engine skips input to stay within about 40 ms of it.

## Recording

There are two recordings, which can run at the same time.

| | [`Engine::record_output`] | [`Engine::record`] |
| --- | --- | --- |
| Records | The main output: everything that is heard. | One [`Source`]: a channel of the input, or a sample played once in its place. |
| Is the source heard | Yes. | No. It goes to the file only, apart from the main output. |
| Writes | A stereo 32-bit float WAV file. | A stereo 24-bit WAV file. |
| Ends with | [`Engine::stop_recording_output`] | [`Engine::stop_recording`] |

- Both start at a time in seconds on the engine's clock, and write the file on a thread of their own.
- Both work on an offline engine, where the file holds what `render` returns.
- The file's header is written again every half second, so a process killed while recording leaves a file that plays up to then.
- Starting a recording of a kind replaces the one of that kind already running.
- Dropping the engine ends both and closes their files.
