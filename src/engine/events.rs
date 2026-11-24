//! What the engine is told to play, and where each event comes from.

use super::*;

/// What a change applies to.
#[derive(Clone, Debug)]
pub enum Target {
    /// The notes and samples started with these ids.
    Sounds(Vec<u64>),
    /// Every note and sample sounding in a part or an effect chain.
    Part(u64),
    /// One effect of a chain, by its index.
    Effect(u64, usize),
    /// The send from one chain into another, as [`Engine::send`] made it; its one option is `level`.
    Send(u64, u64),
}

/// Something to play at a time.
/// [The guide to events](crate::guide::events) follows a note from the call that hands it over to the end of its sound.
#[derive(Clone, Debug)]
pub enum Event {
    /// A note of a synth; `id` lets a later change reach it, and `chain` is the effect chain it plays into.
    Note {
        /// What a later change reaches the note by.
        id: Option<u64>,
        /// The name of the synth.
        synth: String,
        /// The options the note is played with.
        options: Vec<(String, Value)>,
        /// The effect chain the note plays into, if any.
        chain: Option<u64>,
        /// Whether the note sounds until it is let go, with its sustain left unread.
        held: bool,
    },
    /// A sample [`Engine::load_sample`] returned, with the sample player's options.
    Sample {
        /// What a later change reaches the sample by.
        id: Option<u64>,
        /// The sample, as [`Engine::load_sample`] returned it.
        sample: u64,
        /// The options of the sample player.
        options: Vec<(String, Value)>,
        /// The effect chain the sample plays into, if any.
        chain: Option<u64>,
    },
    /// Overlapping grains of a sample or a buffer, with the options [`Engine::grains_specs`] lists.
    Grains {
        /// What a later change reaches the grains by.
        id: Option<u64>,
        /// The sample or buffer, as [`Engine::load_sample`] or [`Engine::create_buffer`] returned it.
        sample: u64,
        /// The options of the grains.
        options: Vec<(String, Value)>,
        /// The effect chain the grains play into, if any.
        chain: Option<u64>,
    },
    /// Channels of the sound card's input, from `input`, numbered from 0, with amp and pan, playing until stopped.
    Input {
        /// What a later change reaches the input by.
        id: Option<u64>,
        /// The first channel of the sound card's input, numbered from 0.
        input: usize,
        /// How many channels it reads from `input` up.
        channels: usize,
        /// The input's options, amp and pan.
        options: Vec<(String, Value)>,
        /// The effect chain the input plays into, if any.
        chain: Option<u64>,
    },
    /// New values for options of sounds already playing, each gliding over its `_slide` option when given.
    Set {
        /// The sounds the change applies to.
        target: Target,
        /// The new number of each option.
        options: Vec<(String, f64)>,
    },
    /// Ends the sounds started with these ids, fading them over 10 ms.
    Stop {
        /// The ids the sounds were started with.
        ids: Vec<u64>,
    },
    /// Lets the sounds started with these ids go, as keys let up: a note shaped by its envelope falls silent over its release, a sound that fades by itself rings on, and one that never ends fades over 10 ms.
    Release {
        /// The ids the sounds were started with.
        ids: Vec<u64>,
    },
}

/// A stretch of a part's music, with its events at their beats, which [`Engine::pass`] hands to the part.
/// [The guide to time](crate::guide::time) shows a program that hands a part one pass after another.
#[derive(Clone, Debug, Default)]
pub struct Pass {
    /// The beat it starts at.
    pub from: f64,
    /// The beat it ends at.
    pub to: f64,
    /// A number of the caller's choosing: a [`Report::PassStarted`] says when the first event of a pass starts whose label is another than that of the pass handed to the part before it.
    pub label: u64,
    /// How many beats before `to` a [`Report::PassEnding`] is sent, unless another pass was handed to the part by then; none for no report.
    pub notice: Option<f64>,
    /// Each event and its beat; the `chain` an event names is left aside, as the event plays into the pass's part.
    pub events: Vec<(f64, Event)>,
}

/// What [`Engine::record`] writes to its file.
#[derive(Clone, Copy, Debug)]
pub enum Source {
    /// One channel of the sound card's input, numbered from 0.
    Input(usize),
    /// A sample [`Engine::load_sample`] returned, played once in place of the input.
    Sample(u64),
}

/// Where the sound comes from that an analysis hears, for [`Engine::analyse`], or that a recording into a buffer takes, for [`Engine::record_buffer`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Origin {
    /// One channel of the sound card's input, numbered from 0.
    Input(usize),
    /// A sample [`Engine::load_sample`] returned, played once in place of the input, to the analysis or the recording alone.
    Sample(u64),
    /// An effect chain's output, or a part's, after its gain, mute and solo.
    Chain(u64),
    /// The main output.
    Output,
}

impl Event {
    /// A note of `synth` with number options and no effect chain; `id` lets a later change reach it.
    ///
    /// ```
    /// use mxs::Event;
    ///
    /// let middle_c = Event::note(None, "piano", &[("note", 60.0), ("vel", 0.8)]);
    /// let held = Event::note(Some(1), "organ", &[("note", 48.0), ("sustain", 600.0)]);
    /// let let_go = Event::Release { ids: vec![1] };
    /// ```
    pub fn note(id: Option<u64>, synth: &str, options: &[(&str, f64)]) -> Event {
        let options = options
            .iter()
            .map(|(k, v)| (k.to_string(), Value::Number(*v)))
            .collect();
        Event::Note {
            id,
            synth: synth.to_string(),
            options,
            chain: None,
            held: false,
        }
    }

    /// A note of `synth` that sounds until an [`Event::Release`] with its `id` lets it go, or until it fades by itself.
    ///
    /// ```
    /// use mxs::Event;
    ///
    /// let held = Event::held(1, "violin", &[("note", 67.0), ("vel", 0.5)]);
    /// let let_go = Event::Release { ids: vec![1] };
    /// ```
    pub fn held(id: u64, synth: &str, options: &[(&str, f64)]) -> Event {
        match Event::note(Some(id), synth, options) {
            Event::Note {
                id,
                synth,
                options,
                chain,
                ..
            } => Event::Note {
                id,
                synth,
                options,
                chain,
                held: true,
            },
            _ => unreachable!("a note is a note"),
        }
    }
}
