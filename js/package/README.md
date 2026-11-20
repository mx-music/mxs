# mxs for the web

mxs is a sound engine whose synths are fitted to recordings of real instruments.
This package plays it on a web page: live in an audio worklet it brings, or offline into arrays of samples, with mxs's synths and effects, synths and effects read from files, and sound files the page fetched.

The package's name on npm is not settled, as `mxs` there is another package; until it is, build it from mxs's `js` folder with `build.sh`, which needs a Rust toolchain with the `wasm32-unknown-unknown` target.

```js
import { live } from "mxs";

button.onclick = async () => {
  const engine = await live();
  await engine.note(0, "piano", { note: 60, vel: 0.6 });
};
```

A browser starts sound only after a click or a key press, so `live` is called from one.
[example/index.html](example/index.html) plays a phrase on the piano when its button is pressed; serve the package's folder with any static server and open it.

## Tables

Fifteen synths, the piano among them, play measured tables, 10.6 MB in all, which the package does not load until the page plays one of those synths.
The first note of such a synth fetches its tables from the `tables` folder beside `mxs.js`, and plays once they are loaded; a page that plays only the piano fetches `piano.mxf` and `piano_pedal.mxf`, about 2.7 MB, and no other table.
`await engine.loadTables("piano")` fetches them before the first note, and the engine dispatches a `loaded` event, whose `detail` names the synth and the tables, once they are loaded.
`synths()` gives each synth's `tables`.

## Live and offline

`live(options)` starts an engine in an audio worklet, on a new audio context connected to its destination, or on the `context` given.
`offline(options)` starts one that renders on the page's own thread, at `rate` frames a second, 48000 unless given; `engine.render(seconds)` returns the next seconds as two `Float32Array`s, left and right.
Both take `voices`, the most sounding at once, and the URLs of `wasm`, `mxs.wasm`, and of the `tables` folder, which default to those beside `mxs.js`.

## Playing

Every call returns a promise; times are in seconds from now.

- `note(after, synth, options, { id, chain, held })` plays a note with the synth's options by name, and gives the number later calls reach it by; a held note sounds until released.
- `sample(after, sample, options, { id, chain })` plays a sound file `loadSample` loaded.
- `set(after, ids, options)`, `release(after, ids)` and `stop(after, ids)` change, let go and end sounds.
- `play(events)` plays several events at once, all or none, each with its `kind`: note, sample, set, release or stop.
- `startEffects([["reverb", { room: 0.6 }]])` starts a chain of effects, and gives the number a note's `chain` plays into it by.
- `synths()` and `effects()` give the catalog: each one's name and options, with each option's kind, default, range and doc.
- `loadSynth(text)` and `loadEffect(text)` register a synth or an effect from an `mxs-synth` or `mxs-effect` file, as mxs's guide to building one describes.
- `loadSample(bytes, name)` loads a FLAC, WAV or AIFF file from its bytes, such as those of a `fetch`, and gives the number `sample` plays it by.
- A live engine's `close()` stops it.

An error in mxs rejects the promise with an `MxsError` carrying mxs's message, such as "no synth named nonesuch".

## Tests

`node test/run.mjs` in mxs's `js` folder runs the package's tests in headless Chromium and WebKit, after `build.sh`; it needs Playwright, imported from the path in `PLAYWRIGHT` or as `playwright`.
