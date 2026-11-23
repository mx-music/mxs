# Tables

Some of mxs's synths play measured tables: the partials and noise of each note fitted to recordings of the real instrument.
The tables make about 9.4 MB of a program built with mxs, so a web page that plays only the piano would download every instrument's tables before it plays a note.

## The synths that need tables

[`Synth::tables`] names the files of the tables a synth plays, and gives none for the others.

| Synth | Tables |
| --- | --- |
| cello | `cello.mxh` |
| deep_bass | `deep_bass.mxf` |
| electric_bass | `electric_bass.mxf` |
| electric_guitar | `electric_guitar.mxf` |
| electric_piano | `electric_piano.mxf` |
| flute | `flute.mxh` |
| glockenspiel | `glockenspiel.mxf` |
| guitar | `guitar.mxf` |
| harp | `harp.mxf` |
| piano | `piano.mxf` and `piano_pedal.mxf` |
| timpani | `timpani.mxf` |
| vibraphone | `vibraphone.mxf` |
| xylophone | `xylophone.mxf` |

The files are in mxs's `src/synths/figures`.

## Building without them

The `tables` feature, on by default, compiles every table in, and a program needs to do nothing more.
A program built with `default-features = false` has none compiled in, and loads each table a synth needs with [`synths::load_table`], from the bytes of its file, which it reads or fetches, before it plays that synth.
Until then, a note of the synth is refused with [`Error::TablesNotLoaded`], which names the synth and the files still to load.
The synths that need no tables play as before.

This program loads the piano's tables from their files and plays a note.
With the tables compiled in, `load_table` checks the bytes and keeps the table compiled in, so the program runs either way.

```rust
use mxs::{Engine, Event, RATE, synths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));

    for file in synths::piano().tables() {
        let bytes = std::fs::read(format!("src/synths/figures/{file}"))?;
        synths::load_table(file, &bytes)?;
    }
    engine.play(&[(0.0, Event::note(None, "piano", &[("note", 60.0)]))])?;
    let (left, _right) = engine.render(0.5)?;
    assert!(left.iter().any(|x| x.abs() > 0.01));
    Ok(())
}
```

- A table loaded once stays loaded for every engine in the program.
- `load_table` refuses a file name mxs has no table of, and bytes that are not a whole table.
- A piano phrase renders the same samples from tables loaded this way as from tables compiled in.
- Built without the tables for `wasm32-unknown-unknown`, the module that renders the phrase of `tests/webassembly` is under 3 MB.

[The guide to samples][samples] covers the other sounds a program loads from bytes.

[samples]: samples.md
