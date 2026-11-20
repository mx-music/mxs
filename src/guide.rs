//! Guides to what the engine does, one subject each.
//! Every program in them runs as one of the crate's tests.
//!
//! | Guide | What it covers |
//! | --- | --- |
//! | [`architecture`] | The threads of a live engine, what crosses between them, and what an offline engine does instead. |
//! | [`events`] | What an event is, how its options are checked, and how a note starts, changes and ends. |
//! | [`signal_path`] | The way from a note to the output: effect chains, and parts with their gain, mute and solo. |
//! | [`time`] | Frames, seconds, system time and beats; the tempo map, passes and cuts. |
//! | [`reports`] | What the engine says of itself to a subscriber, and when. |
//! | [`live`] | Devices, latency, how far ahead to hand a note over, whether the engine keeps up, a MIDI keyboard, and a MIDI input played on a part. |
//! | [`building`] | A synth or an effect of your own, in Rust or as data. |
//! | [`samples`] | Sound files, the sound card's input, and recording to a file. |
//! | [`tables`] | The synths that play measured tables, and loading those tables at run time. |
//!
//! The same pages are in the repository's `docs/guide`, where each links the next by its file.

#![allow(unused_imports)]

use crate::*;

// The pages link each other by file, for whoever reads them in the repository; the first definition of a link wins, so these point the same links at the modules here.
macro_rules! guide {
    ($($name:ident),*) => {
        $(
            #[doc = "[architecture]: architecture"]
            #[doc = "[events]: events"]
            #[doc = "[signal_path]: signal_path"]
            #[doc = "[time]: time"]
            #[doc = "[reports]: reports"]
            #[doc = "[live]: live"]
            #[doc = "[building]: building"]
            #[doc = "[samples]: samples"]
            #[doc = "[tables]: tables"]
            #[doc = ""]
            #[doc = include_str!(concat!("../docs/guide/", stringify!($name), ".md"))]
            pub mod $name {}
        )*
    };
}

guide!(
    architecture,
    events,
    signal_path,
    time,
    reports,
    live,
    building,
    samples,
    tables
);
