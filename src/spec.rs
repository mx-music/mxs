//! The options a synth or an effect takes, each declared with its kind, default, range and documentation.

/// An option's value: a number, or a name from the values its declaration lists.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number.
    Number(f64),
    /// A name from the values the option lists.
    Name(String),
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Name(value.to_string())
    }
}

/// What an option measures, which says how a note gives it and how the engine checks it.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A MIDI note, fractions allowed.
    Note,
    /// A frequency in hertz.
    Hz,
    /// A level in decibels.
    Db,
    /// A time in seconds.
    Seconds,
    /// A time in milliseconds.
    Ms,
    /// A time that follows the tempo: given in beats in a pass, and in seconds to [`Engine::play`](crate::Engine::play).
    Beats,
    /// Any number.
    Float,
    /// A whole number.
    Integer,
    /// 0 or 1.
    Boolean,
    /// One of a list of names.
    Name(Vec<String>),
}

/// The declaration of one option.
#[derive(Clone, Debug)]
pub struct Spec {
    /// What notes call the option.
    pub name: String,
    /// What the option measures.
    pub kind: Kind,
    /// The value it takes when a note leaves it out, if it has one.
    pub default: Option<Value>,
    /// The lowest and highest number it accepts, if bounded.
    pub range: Option<(f64, f64)>,
    /// Whether every note must give it.
    pub required: bool,
    /// What it means, for whoever writes a note.
    pub doc: String,
}

impl Spec {
    /// An option called `name` of the kind given, with no default, range or documentation yet.
    pub fn new(name: &str, kind: Kind) -> Self {
        Spec {
            name: name.to_string(),
            kind,
            default: None,
            range: None,
            required: false,
            doc: String::new(),
        }
    }

    /// A MIDI note option, fractions allowed.
    pub fn note(name: &str) -> Self {
        Self::new(name, Kind::Note)
    }

    /// An option in hertz.
    pub fn hz(name: &str) -> Self {
        Self::new(name, Kind::Hz)
    }

    /// An option in decibels.
    pub fn db(name: &str) -> Self {
        Self::new(name, Kind::Db)
    }

    /// An option in seconds.
    pub fn seconds(name: &str) -> Self {
        Self::new(name, Kind::Seconds)
    }

    /// An option in milliseconds.
    pub fn ms(name: &str) -> Self {
        Self::new(name, Kind::Ms)
    }

    /// An option that is a time that follows the tempo: given in beats in a pass, and in seconds to [`Engine::play`](crate::Engine::play).
    pub fn beats(name: &str) -> Self {
        Self::new(name, Kind::Beats)
    }

    /// An option that is any number.
    pub fn float(name: &str) -> Self {
        Self::new(name, Kind::Float)
    }

    /// An option that is a whole number.
    pub fn integer(name: &str) -> Self {
        Self::new(name, Kind::Integer)
    }

    /// An option that is 0 or 1.
    pub fn boolean(name: &str) -> Self {
        Self::new(name, Kind::Boolean).range(0.0, 1.0)
    }

    /// An option that takes one of `values`, given by its name.
    pub fn name(name: &str, values: &[&str]) -> Self {
        Self::new(
            name,
            Kind::Name(values.iter().map(|v| v.to_string()).collect()),
        )
    }

    /// The value the option takes when a note does not give one.
    pub fn default(mut self, value: impl Into<Value>) -> Self {
        self.default = Some(value.into());
        self
    }

    /// The lowest and highest number the option accepts.
    pub fn range(mut self, low: f64, high: f64) -> Self {
        self.range = Some((low, high));
        self
    }

    /// Makes the option one every note must give.
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// What the option means, for whoever writes a note.
    pub fn doc(mut self, doc: &str) -> Self {
        self.doc = doc.to_string();
        self
    }

    /// The expressions every note takes, which can change while it sounds: bend, brightness and pressure.
    /// A synth declares those of `names` it follows, so that it can be built apart from an engine, which gives every note all three.
    pub fn expressions(names: &[&str]) -> Vec<Spec> {
        [
            Spec::float("bend")
                .default(0.0)
                .range(-120.0, 120.0)
                .doc("Semitones the pitch is moved by, without starting the note again."),
            Spec::float("brightness")
                .default(0.5)
                .range(0.0, 1.0)
                .doc("How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller."),
            Spec::float("pressure")
                .default(0.0)
                .range(0.0, 1.0)
                .doc("How hard a held note is pressed once it has started."),
        ]
        .into_iter()
        .filter(|spec| names.contains(&spec.name.as_str()))
        .collect()
    }

    /// The options of the note's envelope, all in beats: attack, decay to decay_level, hold at sustain_level for sustain, then release, with this synth's own sustain and release.
    pub fn envelope(sustain: f64, release: f64) -> Vec<Spec> {
        vec![
            Spec::beats("attack")
                .default(0.0)
                .range(0.0, f64::INFINITY)
                .doc("How long the note takes to rise to attack_level."),
            Spec::beats("decay")
                .default(0.0)
                .range(0.0, f64::INFINITY)
                .doc("How long it then takes to fall to decay_level."),
            Spec::beats("sustain")
                .default(sustain)
                .range(0.0, f64::INFINITY)
                .doc("How long it then holds, moving to sustain_level."),
            Spec::beats("release")
                .default(release)
                .range(0.0, f64::INFINITY)
                .doc("How long it then takes to fall silent."),
            Spec::float("attack_level")
                .default(1.0)
                .range(0.0, f64::INFINITY)
                .doc("The level the attack rises to."),
            Spec::float("decay_level")
                .default(-1.0)
                .doc("The level the decay falls to; sustain_level when below 0."),
            Spec::float("sustain_level")
                .default(1.0)
                .range(0.0, f64::INFINITY)
                .doc("The level the note holds at."),
        ]
    }

    /// The value to play, held to the range, and whether it had to be held; or why the value does not fit.
    pub(crate) fn check(&self, value: &Value) -> Result<(Value, bool), String> {
        match (&self.kind, value) {
            (Kind::Name(values), Value::Name(name)) if values.contains(name) => {
                Ok((value.clone(), false))
            }
            (Kind::Name(values), _) => Err(format!("takes one of {}", values.join(", "))),
            (_, Value::Name(_)) => Err("takes a number".to_string()),
            (_, Value::Number(x)) if !x.is_finite() => Err("takes a finite number".to_string()),
            (_, Value::Number(x)) => match self.range {
                Some((low, high)) if *x < low || *x > high => {
                    Ok((Value::Number(x.clamp(low, high)), true))
                }
                _ => Ok((value.clone(), false)),
            },
        }
    }
}
