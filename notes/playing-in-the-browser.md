# Playing in the browser

These are the notes of mxs-00010, a spike on playing mxs on a web page.
They were written on 19 November 2026 at mxs 09282b1, on a MacBook Pro with an Apple M5 Pro running macOS 27.2.
The prototype is in [`browser`](browser): `build.sh` builds it, and `python3 -m http.server -d browser/public 5198` serves it.

## What builds

mxs and every one of its instruments build for `wasm32-unknown-unknown`, with no bindings generator and no imports, so the module runs in a browser, in an audio worklet and in Node alike.
The prototype's crate, `mxs_web`, wraps one offline engine in four C functions: `start`, `synth_name`, `note` and `render`.

One thing in mxs had to change: the offline render timed itself for the load it reports, and WebAssembly without a host has no clock, so every render panicked.
Commit 09282b1 skips the timing in WebAssembly, where an offline engine now reports no load.

What does not work in WebAssembly:

- A live engine: `Engine::live` panics, because it starts threads, and a browser's WebAssembly has none unless the page is built for shared memory; the page's audio worklet is the live engine instead, rendering a block each time the browser asks.
- Worker threads: an engine fills every voice on the one thread, for the same reason.
- Files: loading a sample by path fails with "operation not supported on this platform", and `load_sample_bytes` works, as the prototype checked with a WAV built in Node; a page fetches the file and hands mxs its bytes.
- MIDI and audio input, which go through the same threads and devices.

Recommendation: keep WebAssembly to the offline engine driven a block at a time, which needs nothing from mxs but the change above, and check it on every change, since nothing caught the clock until now.

## How fast

The measure is the share of real time that rendering takes with some number of held piano notes, at 48 kHz in blocks of 128 frames, on one thread, timed over a second after the notes start.

| Where | 32 notes | 64 notes | Notes at once |
| --- | --- | --- | --- |
| Native, release build | 29% | 59% | about 110 |
| Chromium 156, headless | 63% | 128% | about 50 |
| WebKit 27.2, headless | 55% | 111% | about 58 |

WebAssembly runs mxs at about half its native speed here.
A build with WebAssembly's SIMD instructions was no faster: 63% for 32 notes in Chromium again.
Playing the solo live used about 45% of the audio thread in both browsers, by a coarse measure, since a worklet in either browser has only whole milliseconds to time with.

I did not measure Firefox: Playwright's Firefox would not start inside the sandbox this ran in, and failed with "Could not find profile folder" whichever profile it was given.
WebKit stands in for Safari here, as it is Safari's engine in Playwright's build, not Safari itself.

Recommendation: about 50 piano notes at once is enough for a page that plays a phrase or a song of a few parts, so a page needs no worker threads; a song that needs more renders offline and plays the result.

## Latency

The page stamps a click with the audio clock and the worklet answers with the audio clock's time of the note's first sample.
Over ten clicks in each browser, the first sample left the worklet from 0 to 2.7 ms after the click, so a note starts in the block after the message arrives, and mxs adds nothing to it.
The browser then adds its own latency: Chromium reported a base latency of 5.3 ms and an output latency that grew from 0 to 8 ms, and WebKit 2.7 ms, but headless browsers have no sound card, so these are not what a listener hears.
The latency from a click to sound out of a speaker needs a microphone or a loopback cable, which belongs with mxs-00020's checks on real hardware.

## Playing a phrase

A page compiles the module once with `WebAssembly.compileStreaming`, hands the compiled module to an `AudioWorkletNode` in its `processorOptions`, and the worklet instantiates it and starts an engine at the context's rate.
The page then posts notes as `[seconds from now, note, vel, sustain]`, and the worklet plays each at its own frame count plus those seconds.
[`browser/public/index.html`](browser/public/index.html) does this in under 100 lines with its measurements, and [`browser/public/worklet.js`](browser/public/worklet.js) in 52.

Two things a worklet lacks that a page has: `TextEncoder`, so the worklet writes a synth's name byte by byte, and `performance.now`, which neither browser gave it, as its block times came in whole milliseconds in both.

The solo is mx's `examples/solo/piano.exs`, written stroke for stroke in [`browser/public/solo.js`](browser/public/solo.js); mx sends the same notes, beats and options, which I checked against `Mx.Pass.events`.
Rendered in either browser, each of its 35 seconds has the same level as the same notes rendered natively by mxs, to 0.01 dB.
mx's own render of the song matches to 0.2 dB, except for the two seconds that hold the lowest A, which are 2.4 and 5 dB softer from mx; since mxs gives the browser's level from the same notes, the difference is in how mx hands the notes to the engine, perhaps as held notes it releases, which I did not check.

## How big

The module is 12.6 MB, 6.7 MB compressed with gzip and 5.2 MB with Brotli.
10.6 MB of it are 16 tables of 15 measured instruments, which mxs compiles in, and the code is the other 2 MB or so.
A page that plays only the piano downloads the tables of every instrument.

Recommendation: let a program load an instrument's tables at run time, so a page fetches the piano's two tables, 2.6 MB with the pedal's, when it first plays it, and the module shrinks to its code.

## Recommendations, and their tickets

- Check the WebAssembly build on every change, rendering a phrase in Node and comparing it with the native render: drafted as mxs-00046.
- Load an instrument's measured tables at run time, so a page downloads only what it plays: drafted as mxs-00047.
- A JavaScript package that plays mxs on a page, with the worklet and the calls above, and loading a synth from a file as mxs-00044 does: drafted as mxs-00048.
- Worker threads on a page, through shared memory: not drafted, as a page plays about 50 piano notes on one thread, and shared memory makes every page that uses it serve two extra headers.
