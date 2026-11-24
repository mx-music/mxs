//! What went wrong when mxs was asked to do something.

/// What went wrong when the engine was asked to do something; its `Display` says it in a sentence.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// No synth is registered by this name.
    UnknownSynth(String),
    /// No effect is registered by this name.
    UnknownEffect(String),
    /// No sample was loaded under this id.
    UnknownSample(u64),
    /// No effect chain was started under this id.
    UnknownChain(u64),
    /// A change named a send that [`Engine::send`](crate::Engine::send) has not made, from one chain into another.
    UnknownSend(u64, u64),
    /// A send, a route or a key would make a chain feed itself; the chains of the loop are named, the first the one that would feed itself.
    FeedsItself(Vec<String>),
    /// A chain was given more sends than it holds.
    TooManySends {
        /// The chain given one send too many.
        chain: u64,
        /// The most sends a chain holds.
        most: usize,
    },
    /// No part was started under this name.
    UnknownPart(String),
    /// A part of this name was started and not stopped.
    PartExists(String),
    /// A sound was given an option its declaration does not list.
    UnknownOption {
        /// The synth, effect or player it was given to.
        owner: String,
        /// The option's name.
        option: String,
    },
    /// An option was given a value its declaration does not accept.
    BadValue {
        /// The synth, effect or player it was given to.
        owner: String,
        /// The option's name.
        option: String,
        /// Why the value is refused, worded to follow the option's name.
        reason: String,
    },
    /// A required option was left out.
    MissingOption {
        /// The synth, effect or player that needs it.
        owner: String,
        /// The option's name.
        option: String,
    },
    /// A synth or an effect made a sound of neither 1 nor 2 channels.
    BadSound {
        /// The synth or effect that made it.
        owner: String,
        /// How many channels the sound had.
        outputs: usize,
    },
    /// An effect could not be built from the options it was given.
    CannotBuild {
        /// The effect.
        owner: String,
        /// Why it could not be built.
        reason: String,
    },
    /// A sample could not be loaded or is not usable; the reason.
    Sample(String),
    /// A device could not be used; the reason.
    Device(String),
    /// The sound card has no input of this number, counted from 0.
    UnknownInput(usize),
    /// No analysis runs under this id.
    UnknownAnalysis(u64),
    /// A recording could not start or finish; the reason.
    Record(String),
    /// More events are waiting than the engine can take.
    Busy,
    /// The call needs a live engine, and this one renders offline.
    Offline,
    /// The call needs an offline engine, and this one plays live.
    Live,
    /// The engine failed while filling audio, and has to be started again.
    Failed,
    /// The output device of this name went away, and the engine has to be started again.
    DeviceLost(String),
    /// The synth of this name builds each note for its note and vel alone, so no voice of it can be kept ready.
    CannotKeepReady(String),
    /// A MIDI input could not be played; the reason.
    Midi(String),
    /// The synth plays measured tables, and these files of them are not loaded, in a build of mxs without the `tables` feature.
    TablesNotLoaded {
        /// The synth played.
        synth: String,
        /// The files of its tables not loaded yet.
        tables: Vec<String>,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnknownSynth(name) => write!(f, "no synth named {name}"),
            Error::UnknownEffect(name) => write!(f, "no effect named {name}"),
            Error::UnknownSample(id) => write!(f, "no sample loaded as {id}"),
            Error::UnknownChain(id) => write!(f, "no effect chain {id}"),
            Error::UnknownSend(from, to) => write!(f, "no send from chain {from} into chain {to}"),
            Error::FeedsItself(chains) => {
                let first = &chains[0];
                write!(f, "chain {first} would feed itself")?;
                match &chains[1..] {
                    [] => Ok(()),
                    [through @ .., last] if !through.is_empty() => {
                        let through = through.join(", ");
                        write!(f, ", through {through} and {last}")
                    }
                    through => {
                        let last = &through[0];
                        write!(f, ", through {last}")
                    }
                }
            }
            Error::TooManySends { chain, most } => {
                write!(f, "chain {chain} has {most} sends, the most a chain holds")
            }
            Error::UnknownPart(name) => write!(f, "no part named {name}"),
            Error::PartExists(name) => write!(f, "a part named {name} is already started"),
            Error::UnknownOption { owner, option } => write!(f, "{owner} has no option {option}"),
            Error::BadValue {
                owner,
                option,
                reason,
            } => write!(f, "{owner}'s option {option} {reason}"),
            Error::MissingOption { owner, option } => {
                write!(f, "{owner} needs the option {option}")
            }
            Error::BadSound { owner, outputs } => {
                write!(f, "{owner} made a sound of {outputs} channels, not 1 or 2")
            }
            Error::CannotBuild { owner, reason } => {
                write!(f, "effect {owner} could not be built: {reason}")
            }
            Error::Sample(reason) => write!(f, "{reason}"),
            Error::Device(reason) => write!(f, "{reason}"),
            Error::UnknownInput(input) => {
                let number = input + 1;
                write!(f, "the sound card has no input {number}")
            }
            Error::UnknownAnalysis(id) => write!(f, "no analysis runs as {id}"),
            Error::Record(reason) => write!(f, "{reason}"),
            Error::Busy => write!(f, "the engine has more events waiting than it can take"),
            Error::Offline => write!(f, "this engine renders offline and has no output"),
            Error::Live => write!(f, "this engine plays live and cannot render"),
            Error::Failed => write!(
                f,
                "the engine failed while filling audio, and has to be started again"
            ),
            Error::DeviceLost(name) => write!(
                f,
                "the output device {name} went away, and the engine has to be started again"
            ),
            Error::TablesNotLoaded { synth, tables } => {
                let tables = tables.join(" and ");
                write!(
                    f,
                    "the {synth}'s tables are not loaded: load {tables} first"
                )
            }
            Error::CannotKeepReady(name) => write!(
                f,
                "{name} builds each note for its note and vel alone, so no voice of it can be kept ready"
            ),
            Error::Midi(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for Error {}
