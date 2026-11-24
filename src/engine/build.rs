//! Making sounds before they play: drafts of notes, the thread that builds them, and the engine's own sample and grain sounds.

use super::*;

/// Builds a draft's sound, which is the costly part of starting a note and needs nothing of the engine.
pub(super) fn build(draft: SoundDraft, rate: f64) -> Result<Built, Error> {
    let options = &draft.options;
    let owner = options.owner.clone();
    let mut ctx = match draft.ready {
        true => Context::ready(owner.clone()),
        false => Context::new(owner.clone()),
    };
    let note = match draft.make {
        Make::Note(build) => build(options, &mut ctx),
        Make::Sample(data, stretched) => sample_note(data, stretched, options),
        Make::Grains(data) => grains_note(data, options),
        Make::Input(1) => Note::rings(Net::wrap(Box::new(pass())), f64::INFINITY),
        Make::Input(_) => Note::rings(Net::wrap(Box::new(multipass::<U2>())), f64::INFINITY),
    };
    let placing = options.follow("pan") | options.follow("amp");
    let mut unit = match note.sound.outputs() {
        1 => (note.sound | placing) >> Net::wrap(Box::new(An(PanMono))),
        2 => (note.sound | placing) >> Net::wrap(Box::new(An(Balance))),
        outputs => {
            return Err(Error::BadSound {
                owner: owner.to_string(),
                outputs,
            });
        }
    };
    unit.set_sample_rate(rate);
    unit.allocate();
    Ok(Built {
        id: draft.id,
        at: draft.at,
        beat: draft.beat,
        stamp: draft.stamp,
        out: draft.out,
        input: draft.input,
        unit,
        lasts: note.end,
        controls: options.take_controls(),
        release: note.release,
        held: note.held,
        ready: ctx.take_ready(),
    })
}

/// A live engine's thread that builds voices, so that neither the caller nor the audio thread waits for them.
pub(super) struct Builder {
    drafts: Option<mpsc::Sender<Vec<Draft>>>,
    /// How many batches are handed over and not yet sent to the mixer.
    waiting: Arc<(Mutex<usize>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl Builder {
    pub(super) fn start(book: Arc<Mutex<Book>>, rate: f64) -> Builder {
        let (drafts, handed) = mpsc::channel::<Vec<Draft>>();
        let waiting = Arc::new((Mutex::new(0usize), Condvar::new()));
        let counting = waiting.clone();
        let thread = std::thread::Builder::new()
            .name("mxs builder".to_string())
            .spawn(move || {
                // Asleep until a batch is handed over, unless something in beats waits for its time.
                let mut pending = true;
                loop {
                    let handed_over = match pending {
                        true => handed.recv_timeout(TICK).map_err(|error| {
                            matches!(error, mpsc::RecvTimeoutError::Disconnected)
                        }),
                        false => handed.recv().map_err(|_| true),
                    };
                    let (mut batch, counted) = match handed_over {
                        Ok(batch) => (batch, true),
                        Err(false) => (Vec::new(), false),
                        Err(true) => break,
                    };
                    {
                        let mut book = book.lock().unwrap_or_else(|e| e.into_inner());
                        let horizon = book.now() + LEAD;
                        batch.extend(book.due(horizon));
                        book.tell();
                        pending = book.pending();
                    }
                    let mut said: Vec<String> = Vec::new();
                    let mut items = Vec::with_capacity(batch.len());
                    let mut kept = Vec::new();
                    for draft in batch {
                        match draft {
                            Draft::Sound(sound) => {
                                let owner = sound.options.owner.clone();
                                let ready = sound.ready;
                                let built = std::panic::catch_unwind(
                                    std::panic::AssertUnwindSafe(|| build(*sound, rate)),
                                );
                                match built {
                                    Ok(Ok(built)) if ready => kept.push((owner, built)),
                                    Ok(Ok(built)) => items.push(Item::Sound(Box::new(built))),
                                    Ok(Err(error)) => {
                                        said.push(format!("{error}, so its note did not play"))
                                    }
                                    Err(_) => said.push(format!(
                                        "{owner} panicked as its note was built, so the note did not play"
                                    )),
                                }
                            }
                            Draft::Change(at, event) => items.push(Item::Change(at, event)),
                        }
                    }
                    if counted || !items.is_empty() || !said.is_empty() {
                        let mut book = book.lock().unwrap_or_else(|e| e.into_inner());
                        book.keep(kept);
                        book.sweep();
                        if let Err(error) = book.commit(items) {
                            said.push(format!("{error}, so some events did not play"));
                        }
                        let now = book.now();
                        said.into_iter()
                            .for_each(|message| book.hub.warn(now, message));
                    }
                    if counted {
                        let (count, settled) = &*counting;
                        *count.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
                        settled.notify_all();
                    }
                }
            })
            .expect("the builder thread starts");
        Builder {
            drafts: Some(drafts),
            waiting,
            thread: Some(thread),
        }
    }

    pub(super) fn hand(&self, batch: Vec<Draft>) {
        if let Some(hand) = self.handle() {
            hand.hand(batch);
        }
    }

    /// A way to hand the builder drafts from another thread.
    #[cfg_attr(not(feature = "keys"), allow(dead_code))]
    pub(super) fn handle(&self) -> Option<Hand> {
        let drafts = self.drafts.clone()?;
        let waiting = self.waiting.clone();
        Some(Hand { drafts, waiting })
    }

    pub(super) fn settle(&self) {
        let (count, settled) = &*self.waiting;
        let mut count = count.lock().unwrap_or_else(|e| e.into_inner());
        while *count > 0 {
            count = settled.wait(count).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// Hands drafts to the builder thread, which builds and plays them in the order they were handed over.
pub(super) struct Hand {
    drafts: mpsc::Sender<Vec<Draft>>,
    waiting: Arc<(Mutex<usize>, Condvar)>,
}

impl Hand {
    pub(super) fn hand(&self, batch: Vec<Draft>) {
        let (count, _) = &*self.waiting;
        *count.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        let _ = self.drafts.send(batch);
    }
}

impl Drop for Builder {
    fn drop(&mut self) {
        self.drafts.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn placing_specs() -> Vec<Spec> {
    vec![
        Spec::float("amp")
            .default(1.0)
            .range(0.0, 16.0)
            .doc("The level the sound is scaled by."),
        Spec::float("pan")
            .default(0.0)
            .range(-1.0, 1.0)
            .doc("Where the sound sits, from -1, left, to 1, right."),
    ]
}

pub(super) fn note_specs() -> Vec<Spec> {
    let mut specs = placing_specs();
    specs.extend(Spec::expressions(&["bend", "brightness", "pressure"]));
    specs.push(
        Spec::float("vibrato")
            .default(0.0)
            .range(0.0, 2.0)
            .doc("The vibrato's depth in semitones, on a synth that lists it among its own; the others ignore it."),
    );
    specs.push(
        Spec::float("pedal")
            .default(0.0)
            .range(0.0, 1.0)
            .doc("How far the sustain pedal is down, on a synth that lists it among its own; the others ignore it."),
    );
    specs
}

pub(super) fn blending_specs() -> Vec<Spec> {
    vec![
        Spec::float("mix")
            .default(1.0)
            .range(0.0, 1.0)
            .doc("How much of the processed sound replaces the dry one."),
        Spec::float("pre_amp")
            .default(1.0)
            .range(0.0, 16.0)
            .doc("The level the sound is scaled by before the effect."),
        Spec::float("amp")
            .default(1.0)
            .range(0.0, 16.0)
            .doc("The level the blend is scaled by."),
    ]
}

pub(super) fn sample_specs() -> Vec<Spec> {
    vec![
        Spec::float("rate").default(1.0).doc("How fast the sample plays, backwards when below 0."),
        Spec::float("stretch").default(1.0).range(0.01, 100.0).doc("What its length is multiplied by, its pitch kept."),
        Spec::float("pitch").default(0.0).range(-48.0, 48.0).doc("How many semitones it is shifted by, its length kept."),
        Spec::float("start").default(0.0).range(0.0, 1.0).doc("Where it starts, as a fraction of its length."),
        Spec::float("finish").default(1.0).range(0.0, 1.0).doc("Where it finishes, as a fraction of its length."),
        Spec::beats("attack").default(0.0).range(0.0, f64::INFINITY).doc("How long it takes to rise to attack_level."),
        Spec::beats("decay").default(0.0).range(0.0, f64::INFINITY).doc("How long it then takes to fall to decay_level."),
        Spec::beats("sustain").default(-1.0).doc("How long it then holds; the rest of the sample when below 0."),
        Spec::beats("release").default(0.0).range(0.0, f64::INFINITY).doc("How long it then takes to fall silent."),
        Spec::float("attack_level").default(1.0).range(0.0, f64::INFINITY).doc("The level the attack rises to."),
        Spec::float("decay_level").default(-1.0).doc("The level the decay falls to; sustain_level when below 0."),
        Spec::float("sustain_level").default(1.0).range(0.0, f64::INFINITY).doc("The level it holds at."),
        Spec::boolean("normalize").default(0.0).doc("Whether it is scaled so its file's loudest moment reaches full scale, before amp, in both channels at the centre for a mono file."),
    ]
}

pub(super) fn grains_specs() -> Vec<Spec> {
    vec![
        Spec::float("position").default(0.5).range(0.0, 1.0).doc("Where in the sample the grains are taken from, as a fraction of its length."),
        Spec::float("duration").default(0.1).range(0.001, 10.0).doc("How long each grain lasts, in seconds."),
        Spec::float("density").default(20.0).range(0.01, 1000.0).doc("How many grains start a second."),
        Spec::float("pitch").default(0.0).range(-48.0, 48.0).doc("The grains' pitch, in semitones from the sample's own."),
        Spec::float("spread").default(0.0).range(0.0, 1.0).doc("How far each grain's position strays at random either way, as a fraction of the sample's length."),
        Spec::float("width").default(0.0).range(0.0, 1.0).doc("How far each grain strays at random across the stereo field, from 0, in the middle, to 1, anywhere from left to right."),
        Spec::float("seed").default(0.0).doc("The seed of the random positions and places; grains with the same seed play the same."),
        Spec::beats("attack").default(0.0).range(0.0, f64::INFINITY).doc("How long the grains take to rise to full level."),
        Spec::beats("sustain").default(1.0).range(0.0, f64::INFINITY).doc("How long they then hold."),
        Spec::beats("release").default(0.0).range(0.0, f64::INFINITY).doc("How long they then take to fall silent."),
    ]
}

fn grains_note(data: Arc<SampleData>, options: &Options) -> Note {
    let moving = [
        "position", "duration", "density", "pitch", "spread", "width",
    ];
    let inputs = moving
        .iter()
        .map(|name| options.follow(name))
        .reduce(|all, next| all | next)
        .expect("grains have options");
    let seed = options.number("seed").to_bits();
    let grains = inputs >> Net::wrap(Box::new(An(Grains::new(data, seed))));
    let (attack, sustain, release) = (
        options.number("attack"),
        options.number("sustain"),
        options.number("release"),
    );
    let level = crate::nodes::shape(move |t| {
        if t < attack {
            t / attack
        } else if t < attack + sustain {
            1.0
        } else {
            (1.0 - (t - attack - sustain) / release).clamp(0.0, 1.0)
        }
    });
    let level = level >> Net::wrap(Box::new(split::<U2>()));
    Note::rings(grains * level, attack + sustain + release)
}

/// A sample's note; one whose event named a stretch or a pitch reads through a [`Stretcher`], and one that named neither through a [`Player`].
fn sample_note(data: Arc<SampleData>, stretched: bool, options: &Options) -> Note {
    let rate = options.number("rate");
    let (from, to) = (options.number("start"), options.number("finish"));
    let span = (to - from).abs() * data.frames() as f64 / data.rate;
    let stretch = if stretched {
        options.number("stretch")
    } else {
        1.0
    };
    let length = span / rate.abs().max(0.001) * stretch;
    let mono = data.channels.len() == 1;
    // A mono sample is placed at equal power, so it is raised by as much again as its pan lowers it at the centre.
    let centre = if mono { std::f32::consts::SQRT_2 } else { 1.0 };
    let gain = if options.number("normalize") > 0.5 && data.peak > 0.0 {
        centre / data.peak
    } else {
        1.0
    };
    let player = if stretched {
        (options.follow("rate") | options.follow("stretch") | options.follow("pitch"))
            >> Net::wrap(Box::new(An(Stretcher::new(data, from, to, rate))))
    } else {
        options.follow("rate") >> Net::wrap(Box::new(An(Player::new(data, from, to, rate))))
    } * gain;
    let player = if mono {
        player >> Net::wrap(Box::new(pass() | sink()))
    } else {
        player
    };
    let sustain = options.number("sustain");
    let (attack, decay, release) = (
        options.number("attack"),
        options.number("decay"),
        options.number("release"),
    );
    let hold = if sustain < 0.0 {
        (length - attack - decay - release).max(0.0)
    } else {
        sustain
    };
    let peak = options.number("attack_level");
    let held = options.number("sustain_level");
    let decayed = Some(options.number("decay_level"))
        .filter(|&l| l >= 0.0)
        .unwrap_or(held);
    let level = crate::nodes::shape(move |t| {
        let along = |from: f64, to: f64, x: f64| from + (to - from) * x.clamp(0.0, 1.0);
        if t < attack {
            peak * t / attack
        } else if t < attack + decay {
            along(peak, decayed, (t - attack) / decay)
        } else if t < attack + decay + hold {
            along(decayed, held, (t - attack - decay) / hold)
        } else {
            along(held, 0.0, (t - attack - decay - hold) / release)
        }
    });
    let level = if mono {
        level
    } else {
        level >> Net::wrap(Box::new(split::<U2>()))
    };
    Note::rings(player * level, attack + decay + hold + release)
}

/// A mono sound placed by its pan at equal power, and scaled by its amp: inputs sound, pan, amp.
#[derive(Clone)]
struct PanMono;

impl AudioNode for PanMono {
    const ID: u64 = 0x6d78_0005;
    type Inputs = U3;
    type Outputs = U2;

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U2> {
        let angle = (input[1].clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
        let x = input[0] * input[2];
        [x * angle.cos(), x * angle.sin()].into()
    }
}

/// A stereo sound turned toward one side by its pan, and scaled by its amp: inputs left, right, pan, amp.
#[derive(Clone)]
struct Balance;

impl AudioNode for Balance {
    const ID: u64 = 0x6d78_0006;
    type Inputs = U4;
    type Outputs = U2;

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U2> {
        let pan = input[2].clamp(-1.0, 1.0);
        [
            input[0] * (1.0 - pan).min(1.0) * input[3],
            input[1] * (1.0 + pan).min(1.0) * input[3],
        ]
        .into()
    }
}

/// A sound held back by a number of frames.
#[derive(Clone)]
pub(super) struct Held {
    frames: Vec<f32>,
    at: usize,
}

impl Held {
    pub(super) fn new(frames: usize) -> Self {
        Held {
            frames: vec![0.0; frames],
            at: 0,
        }
    }
}

impl AudioNode for Held {
    const ID: u64 = 0x6d78_0080;
    type Inputs = U1;
    type Outputs = U1;

    fn reset(&mut self) {
        self.frames.fill(0.0);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let out = self.frames[self.at];
        self.frames[self.at] = input[0];
        self.at = (self.at + 1) % self.frames.len();
        [out].into()
    }
}

/// The dry and processed sound blended by mix and scaled by amp: inputs dry left and right, wet left and right, mix, amp.
#[derive(Clone)]
pub(super) struct Blend;

impl AudioNode for Blend {
    const ID: u64 = 0x6d78_0007;
    type Inputs = U6;
    type Outputs = U2;

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U2> {
        let mix = input[4].clamp(0.0, 1.0);
        let amp = input[5];
        [
            (input[0] * (1.0 - mix) + input[2] * mix) * amp,
            (input[1] * (1.0 - mix) + input[3] * mix) * amp,
        ]
        .into()
    }
}

impl Engine {
    /// Takes a voice kept ready for each note of `events` that one can play, in place of its draft, with the synth of each taken, to replace it; the rest of the drafts are left to build.
    pub(super) fn take_ready(
        &mut self,
        events: &[(f64, Event)],
        drafts: Vec<Draft>,
    ) -> (Vec<Draft>, Vec<Item>, Vec<Arc<str>>) {
        let mut book = self.book();
        if book.ready.values().all(Vec::is_empty) {
            return (drafts, Vec::new(), Vec::new());
        }
        let mut left = Vec::with_capacity(drafts.len());
        let mut taken = Vec::new();
        let mut replace = Vec::new();
        for (draft, (_, event)) in drafts.into_iter().zip(events) {
            match draft {
                Draft::Sound(sound) => match book.take_ready(event, &sound) {
                    Some(built) => {
                        replace.push(sound.options.owner.clone());
                        taken.push(Item::Sound(Box::new(built)));
                    }
                    None => left.push(Draft::Sound(sound)),
                },
                change => left.push(change),
            }
        }
        (left, taken, replace)
    }

    /// Builds a voice to keep ready for each synth named, on the builder thread of a live engine and at once on an offline one.
    pub(super) fn replace_ready(&mut self, synths: &[Arc<str>]) {
        let mut drafts = Vec::with_capacity(synths.len());
        for synth in synths {
            if let Ok(mut draft) = self.note_draft(None, synth, &[], false, 0.0, Out::Main) {
                draft.ready = true;
                drafts.push(Draft::Sound(Box::new(draft)));
            }
        }
        if let Some(builder) = &self.builder {
            builder.hand(drafts);
            return;
        }
        let rate = self.rate;
        let built: Vec<(Arc<str>, Built)> = drafts
            .into_iter()
            .filter_map(|draft| match draft {
                Draft::Sound(sound) => {
                    let owner = sound.options.owner.clone();
                    build(*sound, rate).ok().map(|built| (owner, built))
                }
                Draft::Change(..) => None,
            })
            .collect();
        self.book().keep(built);
    }

    /// Keeps `count` voices of `synth` ready, built before any note asks for one, so a note of it, held or not, handed over with no options but note, vel, bend, brightness, pressure and a pedal of 0 or 1, starts from one in the block being filled, with nothing built or allocated on the audio thread.
    /// Each voice taken is replaced off the audio thread: on a live engine's builder thread, and on an offline one as the note is handed over.
    /// The voices are built here, so the call takes as long as building them; a count of 0 stops keeping voices of `synth` ready.
    /// Only notes handed over by seconds or frames take a ready voice; a note in a pass is built as before.
    /// A synth fitted to recordings of struck notes can be kept ready; one that builds each note for its note and vel alone is refused with [`Error::CannotKeepReady`].
    pub fn keep_ready(&mut self, synth: &str, count: usize) -> Result<(), Error> {
        self.check_running()?;
        let mut kept = Vec::with_capacity(count);
        for _ in 0..Ord::max(count, 1) {
            let mut draft = self.note_draft(None, synth, &[], false, 0.0, Out::Main)?;
            draft.ready = true;
            let built = build(draft, self.rate)?;
            if built.ready.is_none() {
                return Err(Error::CannotKeepReady(synth.to_string()));
            }
            kept.push(built);
        }
        kept.truncate(count);
        self.book().ready.insert(synth.to_string(), kept);
        Ok(())
    }

    /// How many voices of `synth` are kept ready now.
    pub fn kept_ready(&self, synth: &str) -> usize {
        self.book().ready.get(synth).map_or(0, Vec::len)
    }

    /// An event checked, to play at `at` seconds into the output it names, or into `part` when one is given.
    pub(super) fn draft(
        &mut self,
        at: f64,
        event: &Event,
        part: Option<u64>,
    ) -> Result<Draft, Error> {
        let out = |chain: &Option<u64>| part.or(*chain).map_or(Out::Main, Out::Chain);
        Ok(match event {
            Event::Note {
                id,
                synth,
                options,
                chain,
                held,
            } => Draft::Sound(Box::new(self.note_draft(
                *id,
                synth,
                options,
                *held,
                at,
                out(chain),
            )?)),
            Event::Sample {
                id,
                sample,
                options,
                chain,
            } => Draft::Sound(Box::new(self.sample_draft(
                *id,
                *sample,
                options,
                at,
                out(chain),
            )?)),
            Event::Grains {
                id,
                sample,
                options,
                chain,
            } => Draft::Sound(Box::new(self.grains_draft(
                *id,
                *sample,
                options,
                at,
                out(chain),
            )?)),
            Event::Input {
                id,
                input,
                channels,
                options,
                chain,
            } => Draft::Sound(Box::new(self.input_draft(
                *id,
                *input,
                *channels,
                options,
                at,
                out(chain),
            )?)),
            change => Draft::Change(at, change.clone()),
        })
    }

    pub(super) fn note_draft(
        &mut self,
        id: Option<u64>,
        synth: &str,
        given: &[(String, Value)],
        held: bool,
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let registered = self
            .synths
            .get(synth)
            .ok_or_else(|| Error::UnknownSynth(synth.to_string()))?;
        tables_loaded(&registered.item)?;
        let (owner, specs, build) = (
            registered.owner.clone(),
            registered.specs.clone(),
            registered.item.build.clone(),
        );
        let values = self.check(&owner, &specs, given)?;
        let options = Options::new(owner, specs, values).holding(held);
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: None,
            options,
            make: Make::Note(build),
            ready: false,
        })
    }

    pub(super) fn sample_draft(
        &mut self,
        id: Option<u64>,
        sample: u64,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let data = self
            .samples
            .get(&sample)
            .ok_or(Error::UnknownSample(sample))?
            .clone();
        let (owner, specs) = self.sampler.clone();
        let values = self.check(&owner, &specs, given)?;
        let stretched = given.iter().any(|(name, _)| {
            ["stretch", "pitch", "stretch_slide", "pitch_slide"].contains(&name.as_str())
        });
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: None,
            options: Options::new(owner, specs, values),
            make: Make::Sample(data, stretched),
            ready: false,
        })
    }

    pub(super) fn grains_draft(
        &mut self,
        id: Option<u64>,
        sample: u64,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let data = self
            .samples
            .get(&sample)
            .ok_or(Error::UnknownSample(sample))?
            .clone();
        let (owner, specs) = self.grainer.clone();
        let values = self.check(&owner, &specs, given)?;
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: None,
            options: Options::new(owner, specs, values),
            make: Make::Grains(data),
            ready: false,
        })
    }

    pub(super) fn input_draft(
        &mut self,
        id: Option<u64>,
        input: usize,
        channels: usize,
        given: &[(String, Value)],
        at: f64,
        out: Out,
    ) -> Result<SoundDraft, Error> {
        let channels = channels.clamp(1, 2);
        if input + channels > Ord::min(self.inputs, INPUTS) {
            return Err(Error::UnknownInput(input + channels - 1));
        }
        let (owner, specs) = self.input.clone();
        let values = self.check(&owner, &specs, given)?;
        Ok(SoundDraft {
            id,
            at,
            beat: None,
            stamp: 0,
            out,
            input: Some((input, channels)),
            options: Options::new(owner, specs, values),
            make: Make::Input(channels),
            ready: false,
        })
    }
}
