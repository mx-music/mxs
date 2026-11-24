//! Checks that need a sound device: a live engine, the system's default output and MIDI inputs; each runs only when asked for.

mod live;
mod midi;
mod output;
