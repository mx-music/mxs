# Voices across several cores

These are the notes of mxs-00011, a spike on filling the voices of one block on several cores instead of one.
The measurements come from `src/spike_parallel.rs`, which runs with `cargo test --release --lib spike_parallel -- --ignored --nocapture --test-threads 1`.
They were taken on 11 November 2026 on a Mac with 6 performance cores and 12 efficiency cores, at 48 kHz, in blocks of 64 frames, over two runs.

## How the spike fills voices

Each worker thread owns a fixed share of the voices for the whole run.
At each block, the workers wait at a barrier, fill their voices into a buffer of their own per voice, and wait at a second barrier.
One thread then adds the voices' buffers in the order of the voices, as the mixer does today.
The spike fills offline, as fast as it can, so it measures how much work fits in a block, not how a live stream keeps its deadlines.

## How many notes play at once

These are 240 notes rendered for 2 seconds, the notes at once being 240 times 2 seconds over the time taken.

- Piano, held: 216 on 1 thread, 409 to 417 on 2, 763 to 778 on 4, 1047 to 1056 on 6, and 1067 to 1070 on 8.
- Berimbau: 54 to 61 on 1 thread, 100 to 105 on 2, 178 on 4, 230 to 236 on 6, and 276 to 278 on 8.

The piano scales almost linearly up to the 6 performance cores and gains nothing past them.
The berimbau keeps gaining past 6 threads, though less, as its notes are costlier and the efficiency cores still add to them.

Recommendation: fill voices on as many workers as there are performance cores, which multiplies the notes at once by about 4.9 for the piano and 4 for the berimbau on this Mac.

## What it costs when few notes play

These are the microseconds taken by one block of 64 frames, out of the 1333 it lasts.

- 1 piano note: 5.0 to 5.2 µs on 1 thread, 7.4 to 7.9 µs on 2 or 4.
- 4 piano notes: 20.2 to 20.9 µs on 1 thread, 20.5 to 21.6 µs on 2 or 4.
- 16 piano notes: 82 to 84 µs on 1 thread, 58.5 to 60.4 µs on 2, 38.5 to 38.8 µs on 4.

Waking the workers and waiting for them costs about 2.5 µs a block, 0.2 percent of the block's time.
At 4 notes the workers gain nothing, and at 16 they halve the time; I didn't measure between.

Recommendation: fill on one thread while few notes sound, and hand voices to the workers above a count measured between 4 and 16.

## Latency

Filling in parallel adds no latency: each block is filled within the same callback, at the same buffer size.
The risk is a missed deadline instead: a worker that the system does not run at the audio thread's priority can be preempted, and the whole block waits for it.
cpal has a `realtime` feature that promotes a thread to real-time priority through the audio_thread_priority crate; I haven't checked that crate's licence, nor tried it on the workers.
On macOS, the workers would best join the output device's audio workgroup, which tells the system they serve the audio thread's deadline.

Recommendation: run the workers at real-time priority, and measure dropped buffers live with a full piano texture before turning parallel filling on by default.

## The same samples on every run

The spike's output was the same, sample for sample, on 1, 2, 4, 6 and 8 threads, for both instruments.
This holds because each voice fills its own buffer and the buffers are added in the order of the voices, whichever thread filled them.
Adding a partial sum per worker would be cheaper, but floats added in another order round differently, so the samples would change with the number of threads.

Recommendation: keep a buffer per voice and add them in voice order.

## Effect chains

Each effect chain reads only the voices sent into it and adds its output to the main mix, so chains do not depend on each other and can be filled in parallel once the voices of the block are done.
A chain's own effects run one after another and cannot be split.
I didn't measure the chains' cost, and mx's songs use a handful of chains at most, so I expect the gain to be small next to the voices'.

Recommendation: fill the chains in parallel only after the voices are, and only if a song with several costly chains shows the need.

## The voice limit

The default limit of 48 notes at once follows one thread's berimbau figure.
With workers it could rise with their number, to about 4 times 48 on this Mac's performance cores.

Recommendation: set the default limit from the number of workers once voices fill in parallel.
