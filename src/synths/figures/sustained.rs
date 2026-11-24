//! A held note's sound: bowed, blown or played on an organ, held until let go, with the recordings' vibrato and a player's breath.

use super::struck::Struck;
use super::*;

/// A held note played from figures fitted to recordings of held notes: it holds until `hold` seconds or until its gate lets it go, then each partial and band falls at its slope.
/// Its pitch follows `note` as it slides, and its vibrato, `vibrato` when given in place of the recordings', fades in until the note settles.
/// With `legato` at 1 it speaks at once, from where the recordings settle.
/// With a `breath`, the note is played as a wind player plays it, see `Breath`.
/// Its gain at a vel is `gain`.
/// Built ready, it keeps room for any note of the figures, and starts again from the note and vel the engine gives it.
pub(in crate::synths) fn held_note(
    figures: &'static Figures,
    gain: impl Fn(f64) -> f64 + Send + Sync + 'static,
    vibrato: Option<f64>,
    legato: f64,
    breath: Option<Breath>,
    expressed: bool,
    o: &Options,
    ctx: &mut Context,
) -> Note {
    let note = o.number("note");
    let vel = effort(o).clamp(0.0, 1.0);
    let ready = ctx.asks_ready();
    let mut voice = if ready {
        figures.room()
    } else {
        Voice::empty()
    };
    figures.fill(&mut voice, note, vel);
    vibrate(&mut voice, vibrato);
    let skip = legato.clamp(0.0, 1.0) * voice.vibrato.map_or(0.0, |v| v.settles);
    let hold = if o.held() {
        f64::INFINITY
    } else {
        o.number("attack") + o.number("decay") + o.number("sustain")
    };
    let falls = if breath.is_some() { 5.0 } else { voice.falls() };
    let under = voice.loudest() - 100.0;
    let gate = shared(1.0);
    let level = gain(vel);
    let node = Sustained {
        voice,
        falls,
        under,
        base: note,
        hold,
        gate: gate.clone(),
        rate: DEFAULT_SR,
        sines: vec![],
        bands: vec![],
        sample: 0,
        let_go: None,
        phase: 0.0,
        skip,
        figures,
        vel,
        vibrato,
        breath,
        origin: 0.0,
        gliding: 0.0,
        caught: f64::NEG_INFINITY,
        swelled: 0.0,
        blown: vel,
        dipped: false,
        last_vel: vel,
        deepen: 1.0,
        slur: 0.0,
        slurred: f64::NEG_INFINITY,
        slurring: BEND,
        expressed: if expressed {
            (o.number("bend"), o.number("brightness"))
        } else {
            (0.0, 0.5)
        },
        loud: 1.0,
        sounding: 1.0,
        register: breath.map_or(1.0, |b| {
            register(figures, note, b.even, &mut Voice::empty())
        }),
        pace: 1.0,
        inhale: vec![],
        inhaled: None,
        fading: vec![],
        warm: vec![],
        gain: 1.0,
        refill: None,
        rates: ([0.0; 8], 0),
    };
    let mut node = node;
    node.deepen = node.vary();
    let expressions = if expressed {
        o.follow("bend") | o.follow("brightness")
    } else {
        Net::wrap(Box::new(dc((0.0, 0.5))))
    };
    let notes = o.follow("note") | followed_effort(o) | expressions;
    if !ready {
        node.set_sample_rate(DEFAULT_SR);
        return Note {
            sound: notes >> (Net::wrap(Box::new(An(node))) * level as f32),
            end: hold + falls,
            release: Some((gate, falls)),
            held: None,
        };
    }
    let room = Mutex::new(figures.room());
    let hold_for = Ready::new(move |given: Given| {
        let mut voice = room.lock().unwrap_or_else(|e| e.into_inner());
        let vel = pressed(given.vel, given.pressure).clamp(0.0, 1.0);
        figures.fill(&mut voice, given.note, vel);
        let falls = if breath.is_some() { 5.0 } else { voice.falls() };
        let hold = if given.held { f64::INFINITY } else { hold };
        (hold + falls, falls)
    });
    ctx.built_ready(hold_for.clone());
    node.gain = level as f32;
    node.refill = Some(HeldRefill {
        hold: hold_for,
        plain: hold,
        gain: Arc::new(gain),
        legato,
        breath,
        working: figures.room(),
    });
    node.set_sample_rate(DEFAULT_SR);
    Note {
        sound: notes >> Net::wrap(Box::new(An(node))),
        end: hold + falls,
        release: Some((gate, falls)),
        held: None,
    }
}

/// What a held voice built ready starts again with: the hold the engine gives its note and vel through, its gain at a vel, its legato, its breath as it was built, and a voice to work in.
#[derive(Clone)]
struct HeldRefill {
    hold: Arc<Ready>,
    /// The seconds a note not held is held for before it falls.
    plain: f64,
    gain: Arc<dyn Fn(f64) -> f64 + Send + Sync>,
    legato: f64,
    breath: Option<Breath>,
    working: Voice,
}

/// The voice with `vibrato` in semitones in place of the recordings'.
fn vibrated(mut voice: Voice, vibrato: Option<f64>) -> Voice {
    vibrate(&mut voice, vibrato);
    voice
}

/// Gives the voice `vibrato` in semitones in place of the recordings'.
fn vibrate(voice: &mut Voice, vibrato: Option<f64>) {
    if let Some(depth) = vibrato {
        // Recordings played without vibrato give it a player's: 5.4 Hz, full a second in.
        let recorded = voice.vibrato.filter(|v| v.rate > 0.0).unwrap_or(Vibrato {
            settles: 1.0,
            rate: 5.4,
            depth: 0.0,
        });
        voice.vibrato = Some(Vibrato {
            depth: depth * 100.0,
            ..recorded
        });
    }
}

impl Voice {
    // The time in its attack at which the fundamental first stands within 10 dB of its last level.
    fn tongued(&self) -> f64 {
        let Some((_, shape)) = self.partials.first() else {
            return 0.0;
        };
        let held = shape.levels[shape.levels.len() - 1];
        let at = shape
            .levels
            .iter()
            .position(|l| *l >= held - 10.0)
            .unwrap_or(0);
        self.times[at]
    }

    pub(super) fn loudest(&self) -> f64 {
        self.partials
            .iter()
            .flat_map(|(_, s)| s.levels.iter())
            .cloned()
            .fold(f64::MIN, f64::max)
    }

    /// The seconds every partial and band takes, once let go from its last level, to fall 100 dB under the loudest partial, at most 5.
    fn falls(&self) -> f64 {
        let under = self.loudest() - 100.0;
        self.partials
            .iter()
            .map(|(_, s)| s)
            .chain(self.noise.iter().map(|(_, s)| s))
            .map(|s| {
                let from = s.levels.iter().cloned().fold(f64::MIN, f64::max);
                if from <= under || s.tail >= 0.0 {
                    0.0
                } else {
                    (from - under) / -s.tail
                }
            })
            .fold(0.05, f64::max)
            .min(5.0)
    }
}

/// How a wind player plays held figures: a change of note slurs to the new note's figures from where they settle, with a catch as the breath settles, a note reached through a dip in vel is tongued and plays its recorded attack, the figures follow vel as it swells or eases, and each partial's level wanders slowly as a breath does.
#[derive(Clone, Copy)]
pub(in crate::synths) struct Breath {
    /// How far the fundamental's level wanders, in dB.
    pub wander: f64,
    /// How far the other partials' levels wander, in dB.
    pub wander_above: f64,
    /// How far the partials dip in a slur's catch, in dB.
    pub catch: f64,
    /// How far the vibrato's depth differs from note to note, as the spread of the log of its factor.
    pub vary: f64,
    /// Whether a breath is taken once the note is let go.
    pub inhale: bool,
    /// How much of the recordings' rise in loudness up the range a player evens out, from 0 to 1.
    pub even: f64,
    pub seed: u64,
}

/// The gain that takes `even` of the way from a note's held loudness at vel 1 to that of the middle recorded note.
/// It works the loudness out in `working`.
fn register(figures: &Figures, note: f64, even: f64, working: &mut Voice) -> f64 {
    let mut held = |note: f64| {
        figures.fill(working, note, 1.0);
        let power: f64 = working
            .partials
            .iter()
            .map(|(_, s)| 10f64.powf(s.levels[s.levels.len() - 1] / 10.0))
            .sum();
        10.0 * power.max(1e-30).log10()
    };
    10f64.powf(-even * (held(note) - held(figures.middle)) / 20.0)
}

/// Where a note's figures start again from.
#[derive(Clone, Copy, PartialEq)]
enum From {
    /// Where the recordings settle, for a slur.
    Settled,
    /// Where their attack stands 10 dB under the held level, for a note tongued on a breath that keeps flowing, as a played Syrinx's tongued notes dip 8 dB at the median.
    Attack,
    /// The same time, for a change of vel.
    Same,
}

#[derive(Clone)]
struct Sustained {
    voice: Voice,
    falls: f64,
    under: f64,
    base: f64,
    hold: f64,
    gate: Shared,
    rate: f64,
    sines: Vec<Sine>,
    bands: Vec<Band>,
    sample: usize,
    let_go: Option<f64>,
    phase: f64,
    skip: f64,
    figures: &'static Figures,
    vel: f64,
    vibrato: Option<f64>,
    breath: Option<Breath>,
    origin: f64,
    gliding: f64,
    caught: f64,
    swelled: f64,
    blown: f64,
    dipped: bool,
    last_vel: f64,
    deepen: f64,
    slur: f64,
    slurred: f64,
    slurring: f64,
    /// The bend and brightness at its input.
    expressed: (f64, f64),
    loud: f64,
    sounding: f64,
    register: f64,
    pace: f64,
    inhale: Vec<(Band, f64)>,
    inhaled: Option<f64>,
    fading: Vec<Sine>,
    /// Every band below half the rate, warmed, which a note takes the bands it sounds from.
    warm: Vec<Band>,
    gain: f32,
    refill: Option<HeldRefill>,
    /// The first rates it was set to, and how many times it was set.
    rates: ([f64; 8], usize),
}

impl Sustained {
    // Starts the note the voice holds again from its first sample, within the room set aside.
    fn restart(&mut self) {
        fill_sines(&self.voice, self.rate, &mut self.sines);
        fill_bands(&self.voice, &self.warm, &mut self.bands);
        self.sample = 0;
        self.let_go = None;
        self.phase = 0.0;
        self.origin = 0.0;
        self.gliding = 0.0;
        self.fading.clear();
        self.read(self.base, self.vel);
    }

    // Starts again from the note and vel the engine gave it, when it gave one, as `held_note` builds a note.
    fn refill(&mut self) {
        let Some(refill) = &mut self.refill else {
            return;
        };
        let Some(given) = refill.hold.given() else {
            return;
        };
        let (note, vel) = (
            given.note,
            pressed(given.vel, given.pressure).clamp(0.0, 1.0),
        );
        self.hold = if given.held {
            f64::INFINITY
        } else {
            refill.plain
        };
        let breath = refill.breath;
        self.register = breath.map_or(1.0, |b| {
            register(self.figures, note, b.even, &mut refill.working)
        });
        self.gain = (refill.gain)(vel) as f32;
        let legato = refill.legato;
        self.figures.fill(&mut self.voice, note, vel);
        vibrate(&mut self.voice, self.vibrato);
        self.skip = legato.clamp(0.0, 1.0) * self.voice.vibrato.map_or(0.0, |v| v.settles);
        self.falls = if breath.is_some() {
            5.0
        } else {
            self.voice.falls()
        };
        self.under = self.voice.loudest() - 100.0;
        self.base = note;
        self.vel = vel;
        self.blown = vel;
        self.last_vel = vel;
        self.breath = breath;
        self.deepen = self.vary();
        // A note built takes a draw for each of its sines each time its rate is set, so this one takes those of the rates set before the last again.
        let (rates, set) = self.rates;
        for rate in &rates[..Ord::min(set, rates.len()).saturating_sub(1)] {
            let sines = self
                .voice
                .partials
                .iter()
                .filter(|(freq, shape)| {
                    *freq < rate / 2.0 && shape.levels.iter().any(|l| *l > -140.0)
                })
                .count();
            for _ in 0..sines {
                self.uniform();
            }
        }
        self.restart();
    }

    // A held level at `t`: the shape's level until the note is let go, its last level after the last time, and falling at its slope after the letting go.
    // A band's level runs in straight lines of power between the times, as the fit reads it.
    fn level(&self, shape: &Shape, t: f64, power: bool) -> f64 {
        let times = &self.voice.times;
        let at = |t: f64| {
            let last = times.len() - 1;
            if t >= times[last] {
                shape.levels[last]
            } else if power {
                10.0 * shape.power_at(times, t).max(1e-30).log10()
            } else {
                shape.at(times, t)
            }
        };
        match self.let_go {
            Some(go) if t > go => {
                // A slope too slow to reach the floor before the note ends, as `falls` is at most 5 seconds, is steepened to reach it then.
                let from = at((go - self.origin) * self.pace + self.skip);
                let tail = shape.tail.min(-(from - self.under).max(0.0) / self.falls);
                from + tail * (t - go)
            }
            _ => at((t - self.origin) * self.pace + self.skip),
        }
    }

    // Moves to the figures of `note` at `vel`: each sine goes on as the nearest harmonic of the new note, a band as the same band, and the rest fade out, all over the slur.
    fn revoice(&mut self, note: f64, vel: f64, t: f64, from: From) {
        if let Some(breath) = self.breath.filter(|_| from != From::Same) {
            self.register = register(self.figures, note, breath.even, &mut Voice::empty());
        }
        let voice = vibrated(self.figures.voice(note, vel), self.vibrato);
        let (old, new) = (hz(self.base), hz(note));
        let mut sines = std::mem::take(&mut self.sines);
        let mut bands = std::mem::take(&mut self.bands);
        let (opening_sines, opening_bands) = Struck::opening(voice.clone(), self.rate);
        for mut sine in opening_sines {
            let harmonic = voice.partials[sine.shape].0 / new;
            let nearest = sines.iter().position(|s| {
                !s.done && (self.voice.partials[s.shape].0 / old / harmonic - 1.0).abs() < 0.03
            });
            if let Some(k) = nearest {
                let was = sines.swap_remove(k);
                sine.phasor = was.phasor;
                sine.amplitude = was.amplitude;
                sine.drift = was.drift;
            } else {
                sine.amplitude = 0.0;
            }
            self.sines.push(sine);
        }
        for mut band in opening_bands {
            let edges = voice.noise[band.shape].0;
            band.rise = 0.0;
            match bands
                .iter()
                .position(|b| !b.done && self.voice.noise[b.shape].0 == edges)
            {
                Some(k) => {
                    let was = bands.swap_remove(k);
                    band.state = was.state;
                    band.noise = was.noise;
                    band.amplitude = was.amplitude;
                }
                None => band.amplitude = 0.0,
            }
            self.bands.push(band);
        }
        let samples = SLUR * self.rate;
        for mut sine in sines.into_iter().filter(|s| !s.done) {
            sine.step = -sine.amplitude / samples;
            self.fading.push(sine);
        }
        // A slurred step of a semitone or two glides, as a played Syrinx's do, while a leap moves within 8 ms, as its leaps take 0 to 7 between the quartiles, since a jump at once clicks; a tongue stops the air between the two.
        if from != From::Same {
            let step = (self.base - note).abs() <= 2.5;
            self.slur = if from == From::Settled || !step {
                self.base - note
            } else {
                0.0
            };
            self.slurred = t;
            let (u, v) = (self.uniform(), self.uniform());
            let drawn = (BEND
                * (0.5
                    * (-2.0 * u.max(1e-12).ln()).sqrt()
                    * (2.0 * std::f64::consts::PI * v).cos())
                .exp())
            .clamp(0.015, 0.04);
            self.slurring = if from == From::Settled && step {
                drawn
            } else {
                LEAP
            };
        }
        match from {
            From::Settled => {
                self.skip = voice.vibrato.map_or(0.0, |v| v.settles);
                self.origin = t;
                self.caught = t;
                self.pace = 1.0;
                self.deepen = self.vary();
            }
            From::Attack => {
                self.skip = voice.tongued();
                self.origin = t;
                self.pace = TONGUE;
                self.deepen = self.vary();
            }
            From::Same => {}
        }
        self.under = voice.loudest() - 100.0;
        self.voice = voice;
        self.base = note;
        self.vel = vel;
        // A tongue stops the air at once, so a tongued note drops to its attack faster than a slur moves.
        self.gliding = t + if from == From::Attack {
            SLUR / 3.0
        } else {
            SLUR
        };
    }

    // Sets up the breath taken after the note, 45 dB under its held level, in its octaves.
    fn take_breath(&mut self, t: f64) {
        let held: f64 = self
            .voice
            .partials
            .iter()
            .map(|(_, s)| 10f64.powf(s.levels[s.levels.len() - 1] / 10.0) / 2.0)
            .sum::<f64>()
            * self.loud
            * self.loud;
        let total = held * 10f64.powf(-INHALE_UNDER / 10.0);
        let whole: f64 = INHALE_OCTAVES
            .iter()
            .map(|(_, _, w)| 10f64.powf(w / 10.0))
            .sum();
        let seed = self.breath.map_or(1, |b| b.seed);
        self.inhale = INHALE_OCTAVES
            .iter()
            .enumerate()
            .filter(|(_, (_, high, _))| *high < self.rate / 2.0)
            .map(|(k, (low, high, w))| {
                let filter = band_filter(*low, *high, self.rate);
                // Uniform noise from -1 to 1 has a power of a third, which the filter scales by its response's energy.
                let power = total * 10f64.powf(w / 10.0) / whole;
                let band = Band {
                    shape: k,
                    filter,
                    state: [[0.0; 4]; 4],
                    noise: seed ^ (k as u64 + 7).wrapping_mul(0xbf58_476d_1ce4_e5b9),
                    rise: 0.0,
                    scale: 1.0,
                    amplitude: 0.0,
                    step: 0.0,
                    done: false,
                };
                (band, (power / (response_energy(filter) / 3.0)).sqrt())
            })
            .collect();
        self.inhaled = Some(t + 0.08);
    }

    // The breath's level at `t`: in over 40 ms, falling 12 dB as the lungs fill, out over the last 40 ms.
    fn inhaling(&mut self, t: f64) {
        let Some(from) = self.inhaled else {
            return;
        };
        let x = t - from;
        let shape = if !(0.0..INHALE).contains(&x) {
            0.0
        } else {
            let rise = (x / 0.04).min(1.0);
            let fall = ((INHALE - x) / 0.04).min(1.0);
            (0.5 - 0.5 * (std::f64::consts::PI * rise).cos())
                * (0.5 - 0.5 * (std::f64::consts::PI * fall).cos())
                * 10f64.powf(-12.0 * x / INHALE / 20.0)
        };
        for (band, amplitude) in self.inhale.iter_mut() {
            band.step = (shape * *amplitude - band.amplitude) / BLOCK as f64;
        }
    }

    // A uniform draw from 0 to 1 from the breath's seed, mixed first so that seeds alike give draws apart.
    fn uniform(&mut self) -> f64 {
        let Some(breath) = self.breath.as_mut() else {
            return 0.5;
        };
        breath.seed = breath.seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = breath.seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        (z ^ (z >> 31)) as f64 / u64::MAX as f64
    }

    // The vibrato's depth for a note, a player's being deeper on one note than the next.
    fn vary(&mut self) -> f64 {
        let Some(vary) = self.breath.map(|b| b.vary) else {
            return 1.0;
        };
        let (u, v) = (self.uniform(), self.uniform());
        (vary * (-2.0 * u.max(1e-12).ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()).exp()
    }

    // A slow wander from -1 to 1, as a breath's: white noise through a one-pole lowpass at 1 Hz, kept at unit spread.
    fn wander(&mut self, drift: f64) -> f64 {
        if self.breath.is_none() {
            return 0.0;
        }
        let white = self.uniform() * 2.0 - 1.0;
        let pole = 1.0 - (-2.0 * std::f64::consts::PI * BLOCK as f64 / self.rate).exp();
        drift + pole * (white * (3.0 * (2.0 - pole) / pole).sqrt() - drift)
    }

    // Plays a change of note or vel as a wind player does.
    fn breathe(&mut self, note: f64, vel: f64, t: f64) {
        if self.let_go.is_some() {
            return;
        }
        if self.breath.is_none() {
            // A bowed string has no breath, but follows how hard it is played as vel or pressure moves.
            if (vel - self.vel).abs() > 0.03 && t >= self.swelled + SWELL {
                self.swelled = t;
                self.revoice(self.base, vel, t, From::Same);
            }
            return;
        }
        // The breath's level is the loudest vel lately, easing over a fifth of a second, so a quick dip under half of it is the tongue.
        let ease = (-(BLOCK as f64) / self.rate / 0.2).exp();
        self.blown = vel.max(self.blown * ease);
        if vel < 0.5 * self.blown {
            self.dipped = true;
        }
        let moved = (note - self.base).abs() >= 0.5;
        let rising = vel > self.last_vel + 1e-4;
        self.last_vel = vel;
        // The figures and loudness follow the breath, not the tongue's dip, which is played as the attack.
        let blown = self.blown;
        self.loud = (blown / self.figures.softest()).min(1.0) * self.register;
        if self.dipped && (moved || rising) {
            self.dipped = false;
            self.revoice(note, blown, t, From::Attack);
        } else if moved {
            self.revoice(note, blown, t, From::Settled);
        } else if (blown - self.vel).abs() > 0.03 && t >= self.swelled + SWELL {
            self.swelled = t;
            self.revoice(self.base, blown, t, From::Same);
        }
    }

    // The levels and frequencies at the next reading, BLOCK samples on, for each sine and band to glide to, with the note at `note`.
    fn read(&mut self, note: f64, vel: f64) {
        let t = (self.sample + BLOCK) as f64 / self.rate;
        if self.let_go.is_none() && (t >= self.hold || self.gate.value() <= 0.0) {
            self.let_go = Some(t.min(self.hold.max(0.0)));
            if self.breath.is_some_and(|b| b.inhale) {
                self.take_breath(t);
            }
        }
        self.breathe(note, vel, t);
        self.inhaling(t);
        let vibrato = self.voice.vibrato.unwrap_or_default();
        let swing = if vibrato.depth > 0.0 && vibrato.rate > 0.0 {
            self.phase += 2.0 * std::f64::consts::PI * vibrato.rate * BLOCK as f64 / self.rate;
            (t / vibrato.settles.max(0.05)).min(1.0) * self.phase.sin()
        } else {
            0.0
        };
        let x = ((t - self.slurred) / self.slurring).clamp(0.0, 1.0);
        let slur = self.slur * (0.5 + 0.5 * (std::f64::consts::PI * x).cos());
        let (bend, brightness) = self.expressed;
        let ratio = 2f64.powf(
            (note - self.base + slur + bend) / 12.0 + self.deepen * vibrato.depth * swing / 1200.0,
        );
        // A brightness away from 0.5 tilts the levels by up to 3 dB an octave either way.
        let tilt = (brightness != 0.5).then_some((brightness - 0.5) * 6.0);
        let gone = self.let_go.is_some_and(|go| t > go);
        let gliding = ((self.gliding - t) * self.rate).max(BLOCK as f64);
        self.fading.retain(|s| s.amplitude > 0.0);
        // A slur's catch: the partials dip as the breath settles on the new note.
        let catch = self.breath.map_or(0.0, |b| {
            let x = (t - self.caught) / CATCH;
            if (0.0..1.0).contains(&x) {
                b.catch * (std::f64::consts::PI * x).sin()
            } else {
                0.0
            }
        });
        // The wander comes in over the second after the figures start to hold.
        let local = t - self.origin + self.skip;
        let wandering = ((local - 0.5) / 1.0).clamp(0.0, 1.0);
        for k in 0..self.sines.len() {
            if self.sines[k].done {
                continue;
            }
            let drift = self.wander(self.sines[k].drift);
            self.sines[k].drift = drift;
            let (freq, shape) = &self.voice.partials[self.sines[k].shape];
            let depth = self.breath.map_or(0.0, |b| {
                if (freq / hz(self.base) - 1.0).abs() < 0.03 {
                    b.wander
                } else {
                    b.wander_above
                }
            });
            let mut level = self.level(shape, t, false)
                + self.voice.swings[self.sines[k].shape] * swing
                + wandering * depth * drift
                - catch;
            if let Some(tilt) = tilt {
                level += tilt * (freq / hz(self.base)).log2();
            }
            let omega =
                2.0 * std::f64::consts::PI * (freq * ratio).min(0.49 * self.rate) / self.rate;
            let sine = &mut self.sines[k];
            sine.turn = (omega.cos(), omega.sin());
            let target = (t / RISE).min(1.0) * 10f64.powf(level / 20.0);
            sine.step = (target - sine.amplitude) / gliding;
            if level < -140.0 && gone {
                sine.done = true;
            }
            let size = (sine.phasor.0 * sine.phasor.0 + sine.phasor.1 * sine.phasor.1).sqrt();
            sine.phasor = (sine.phasor.0 / size, sine.phasor.1 / size);
        }
        for k in 0..self.bands.len() {
            if self.bands[k].done {
                continue;
            }
            let shape = &self.voice.noise[self.bands[k].shape].1;
            let level = self.level(shape, t, true);
            let band = &mut self.bands[k];
            let rising = if t >= band.rise {
                1.0
            } else {
                0.5 - 0.5 * (std::f64::consts::PI * t / band.rise).cos()
            };
            let target = rising * 10f64.powf(level / 20.0);
            band.step = (target - band.amplitude) / gliding;
            if level < -140.0 && gone {
                band.done = true;
            }
        }
    }
}

impl AudioNode for Sustained {
    const ID: u64 = 0x6d78_0071;
    type Inputs = U4;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        let (rates, set) = &mut self.rates;
        if let Some(rate) = rates.get_mut(*set) {
            *rate = sample_rate;
        }
        *set += 1;
        self.rate = sample_rate;
        self.warm = warm_bands(&self.voice, sample_rate);
        self.sines = Vec::with_capacity(self.voice.partials.capacity());
        self.bands = Vec::with_capacity(self.warm.len());
        self.restart();
    }

    fn reset(&mut self) {
        self.set_sample_rate(self.rate);
    }

    fn tick(&mut self, input: &Frame<f32, U4>) -> Frame<f32, U1> {
        self.expressed = (input[2] as f64, input[3] as f64);
        if self.sample == 0 && self.refill.is_some() {
            self.refill();
        }
        if self.sample > 0 && self.sample.is_multiple_of(BLOCK) {
            self.read(input[0] as f64, input[1] as f64);
        }
        let mut sum = 0.0;
        for sine in self.sines.iter_mut().filter(|s| !s.done) {
            sum += sine.amplitude * sine.phasor.1;
            let (re, im) = sine.phasor;
            sine.phasor = (
                re * sine.turn.0 - im * sine.turn.1,
                re * sine.turn.1 + im * sine.turn.0,
            );
            sine.amplitude += sine.step;
        }
        for sine in self.fading.iter_mut() {
            sum += sine.amplitude.max(0.0) * sine.phasor.1;
            let (re, im) = sine.phasor;
            sine.phasor = (
                re * sine.turn.0 - im * sine.turn.1,
                re * sine.turn.1 + im * sine.turn.0,
            );
            sine.amplitude += sine.step;
        }
        for band in self.bands.iter_mut().filter(|b| !b.done) {
            sum += band.amplitude * band.filtered();
            band.amplitude += band.step;
        }
        // Over about 10 ms, as a step in loudness from one note to the next is otherwise a click.
        self.sounding += (self.loud - self.sounding) / (0.01 * self.rate);
        let mut breath = 0.0;
        for (band, _) in self.inhale.iter_mut() {
            breath += band.amplitude * band.filtered();
            band.amplitude += band.step;
        }
        self.sample += 1;
        [(sum * self.sounding + breath) as f32 * self.gain].into()
    }
}
