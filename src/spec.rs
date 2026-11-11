//! The options a synth or an effect takes, each declared with its kind, default, range and documentation.

/// An option's value: a number, or a name from the values its declaration lists.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
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

/// What an option measures, which says how a song writes it and how mx checks it.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// A MIDI note, fractions allowed.
    Note,
    Hz,
    Db,
    Seconds,
    /// A time a song writes in beats, which mx turns into seconds at the tempo before it reaches the engine.
    Beats,
    Float,
    Integer,
    Boolean,
    /// One of a list of names.
    Name(Vec<String>),
}

/// The declaration of one option.
#[derive(Clone, Debug)]
pub struct Spec {
    pub name: String,
    pub kind: Kind,
    pub default: Option<Value>,
    pub range: Option<(f64, f64)>,
    pub required: bool,
    pub doc: String,
}

impl Spec {
    pub fn new(name: &str, kind: Kind) -> Self {
        Spec { name: name.to_string(), kind, default: None, range: None, required: false, doc: String::new() }
    }

    pub fn note(name: &str) -> Self {
        Self::new(name, Kind::Note)
    }

    pub fn hz(name: &str) -> Self {
        Self::new(name, Kind::Hz)
    }

    pub fn db(name: &str) -> Self {
        Self::new(name, Kind::Db)
    }

    pub fn seconds(name: &str) -> Self {
        Self::new(name, Kind::Seconds)
    }

    pub fn beats(name: &str) -> Self {
        Self::new(name, Kind::Beats)
    }

    pub fn float(name: &str) -> Self {
        Self::new(name, Kind::Float)
    }

    pub fn integer(name: &str) -> Self {
        Self::new(name, Kind::Integer)
    }

    pub fn boolean(name: &str) -> Self {
        Self::new(name, Kind::Boolean).range(0.0, 1.0)
    }

    pub fn name(name: &str, values: &[&str]) -> Self {
        Self::new(name, Kind::Name(values.iter().map(|v| v.to_string()).collect()))
    }

    pub fn default(mut self, value: impl Into<Value>) -> Self {
        self.default = Some(value.into());
        self
    }

    pub fn range(mut self, low: f64, high: f64) -> Self {
        self.range = Some((low, high));
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn doc(mut self, doc: &str) -> Self {
        self.doc = doc.to_string();
        self
    }

    /// The options of the note's envelope, all in beats: attack, decay to decay_level, hold at sustain_level for sustain, then release, with this synth's own sustain and release.
    pub fn envelope(sustain: f64, release: f64) -> Vec<Spec> {
        vec![
            Spec::beats("attack").default(0.0).range(0.0, f64::INFINITY).doc("How long the note takes to rise to attack_level."),
            Spec::beats("decay").default(0.0).range(0.0, f64::INFINITY).doc("How long it then takes to fall to decay_level."),
            Spec::beats("sustain").default(sustain).range(0.0, f64::INFINITY).doc("How long it then holds, moving to sustain_level."),
            Spec::beats("release").default(release).range(0.0, f64::INFINITY).doc("How long it then takes to fall silent."),
            Spec::float("attack_level").default(1.0).range(0.0, f64::INFINITY).doc("The level the attack rises to."),
            Spec::float("decay_level").default(-1.0).doc("The level the decay falls to; sustain_level when below 0."),
            Spec::float("sustain_level").default(1.0).range(0.0, f64::INFINITY).doc("The level the note holds at."),
        ]
    }

    /// The value to play, held to the range, and whether it had to be held; or why the value does not fit.
    pub(crate) fn check(&self, value: &Value) -> Result<(Value, bool), String> {
        match (&self.kind, value) {
            (Kind::Name(values), Value::Name(name)) if values.contains(name) => Ok((value.clone(), false)),
            (Kind::Name(values), _) => Err(format!("takes one of {}", values.join(", "))),
            (_, Value::Name(_)) => Err("takes a number".to_string()),
            (_, Value::Number(x)) if !x.is_finite() => Err("takes a finite number".to_string()),
            (_, Value::Number(x)) => match self.range {
                Some((low, high)) if *x < low || *x > high => Ok((Value::Number(x.clamp(low, high)), true)),
                _ => Ok((value.clone(), false)),
            },
        }
    }
}
