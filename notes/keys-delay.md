# From a key to the sound

cargo run --release --features keys --example virtual_keyboard -- target/release/keys OUT_DIR [BUFFER] plays a keyboard part from a virtual MIDI source into the keys program, playing into BlackHole 2ch, and records BlackHole's input, so it needs no keyboard and makes no sound in the room.
It writes the frame of the recording each message was sent at, so the delay from a message to the first sample it makes can be read off the recording.

On 12 November 2026, on a MacBook Pro at 48 kHz, 20 notes started 4.96 to 7.31 ms after their messages at 128 frames, 6.03 ms by the median.
At 256 frames they started 7.71 to 12.54 ms after, 10.19 ms by the median.
The spread at each size is about one buffer: a note waits for the next buffer, and the device holds another before it plays.
So keys asks for 128 frames unless told otherwise.

The recording's timing comes from the input's capture timestamps, so the delay includes whatever BlackHole adds between its output and its input; a keyboard's own USB delay is not in it.
