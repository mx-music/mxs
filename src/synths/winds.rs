//! The winds' physical models: an air jet for the flute, a reed for the clarinet, oboe, bassoon and tenor sax, and the lips for the brass.
//! Each is a digital waveguide after Julius O. Smith III, "Physical Audio Signal Processing", <https://ccrma.stanford.edu/~jos/pasp/>, and Perry R. Cook, "A meta-wind-instrument physical model", ICMC 1992.
//! The bore is a delay line as long as the note's period, so a change of note carries the same column of air to the new pitch, and the breath's pressure follows vel while the note holds.
//! After the model come a peak at each band mx measures, its gain fitted to each instrument's recordings at pp, mf and ff and following vel between them.

use crate::nodes::{db, hz, shape, white_noise};
use crate::synth::SynthDecl;
use crate::{Context, Note, Options, Spec, Synth};
use fundsp::prelude::*;
use std::f32::consts::TAU;

/// The centres of the bands mx measures, from 80 Hz to 4 kHz, and of three more above them, up to 16 kHz.
const BANDS: [f32; 11] = [
    113.0, 200.0, 316.0, 502.0, 794.0, 1265.0, 2000.0, 3162.0, 5012.0, 7943.0, 12589.0,
];

const EQ: [&str; 11] = [
    "eq0", "eq1", "eq2", "eq3", "eq4", "eq5", "eq6", "eq7", "eq8", "eq9", "eq10",
];

/// A conical bore whose corner follows the pitch: the corner in Hz above which its end lets the sound out, the centre in Hz of its breath's noise, how far its sound falls at a slur and the seconds it takes to come back to a third, and how much its pitch, level and steepening drift.
const fn cone(bell: f32, air: f32, slur: [f32; 2], drift: [f32; 3]) -> Bore {
    Bore {
        conical: true,
        corner: 2.0,
        cutoff: 0.0,
        keep: 0.95,
        bell,
        bell_corner: 0.0,
        smooth: 2.0,
        zeta: 0.4,
        relative: false,
        air,
        tongue: 0.25,
        kick: 0.0,
        catch: slur[0],
        mend: slur[1],
        gasp: 0.0,
        jitter: drift[0],
        shimmer: drift[1],
        wander: drift[2],
    }
}

pub fn all() -> Vec<Synth> {
    vec![
        flute(),
        flute_figured(),
        reed_wind(
            "clarinet",
            5.0,
            1.0,
            Bore {
                conical: false,
                cutoff: 1500.0,
                bell_corner: 2.0,
                smooth: 0.0,
                relative: true,
                tongue: 0.0,
                kick: 1.25,
                ..cone(1500.0, 1500.0, [0.5, 0.03], [1.0, 0.3, 0.0])
            },
            &CLARINET,
        ),
        reed_wind(
            "oboe",
            5.0,
            1.0,
            cone(1000.0, 1500.0, [0.46, 0.03], [2.0, 0.2, 0.1]),
            &OBOE,
        ),
        reed_wind(
            "bassoon",
            5.5,
            1.0,
            cone(400.0, 800.0, [0.5, 0.03], [2.0, 0.3, 0.1]),
            &BASSOON,
        ),
        reed_wind(
            "tenor_sax",
            4.94,
            TENOR_SAX_VIBRATO,
            cone(800.0, 1500.0, [0.64, 0.03], [0.8, 0.25, 0.08]),
            &TENOR_SAX,
        ),
        reed_wind(
            "trumpet",
            4.9,
            1.0,
            cone(1200.0, 1500.0, [0.68, 0.065], [2.0, 0.2, 0.04]),
            &TRUMPET,
        ),
        reed_wind(
            "trombone",
            4.95,
            1.0,
            cone(700.0, 1000.0, [0.67, 0.065], [2.0, 0.35, 0.05]),
            &TROMBONE,
        ),
        reed_wind(
            "horn",
            5.0,
            1.0,
            cone(500.0, 800.0, [0.79, 0.065], [2.5, 0.35, 0.08]),
            &HORN,
        ),
        reed_wind(
            "tuba",
            5.0,
            1.0,
            cone(300.0, 300.0, [0.77, 0.065], [2.5, 0.4, 0.08]),
            &TUBA,
        ),
    ]
}

/// A delay line read at a fraction of a sample by cubic Lagrange interpolation.
#[derive(Clone)]
struct Line {
    samples: Vec<f32>,
    at: usize,
}

impl Line {
    fn new(length: usize) -> Line {
        Line {
            samples: vec![0.0; length.next_power_of_two()],
            at: 0,
        }
    }

    fn write(&mut self, x: f32) {
        self.at = (self.at + 1) & (self.samples.len() - 1);
        self.samples[self.at] = x;
    }

    /// The sample written `delay` samples before the last, which must lie between 1 and the line's length less 3.
    fn read(&self, delay: f32) -> f32 {
        let mask = self.samples.len() - 1;
        let whole = delay.floor();
        let d = delay - whole;
        let back = |k: f32| self.samples[(self.at + self.samples.len() - k as usize) & mask];
        let (a, b, c, e) = (
            back(whole - 1.0),
            back(whole),
            back(whole + 1.0),
            back(whole + 2.0),
        );
        -a * d * (d - 1.0) * (d - 2.0) / 6.0 + b * (d + 1.0) * (d - 1.0) * (d - 2.0) / 2.0
            - c * (d + 1.0) * d * (d - 2.0) / 2.0
            + e * (d + 1.0) * d * (d - 1.0) / 6.0
    }

    fn clear(&mut self) {
        self.samples.iter_mut().for_each(|x| *x = 0.0);
    }
}

/// A one-pole low-pass, y = (1 - a) x + a y.
#[derive(Clone, Default)]
struct Loss {
    y: f32,
}

impl Loss {
    fn pole(corner: f32, rate: f32) -> f32 {
        (-TAU * corner / rate).exp()
    }

    fn tick(&mut self, x: f32, a: f32) -> f32 {
        self.y = (1.0 - a) * x + a * self.y;
        self.y
    }

    /// The samples it delays a sine at `w` radians per sample.
    fn lag(a: f32, w: f32) -> f32 {
        (a * w.sin()).atan2(1.0 - a * w.cos()) / w
    }
}

/// A one-pole, one-zero filter that takes out the steady part of a signal.
#[derive(Clone, Default)]
struct DcBlock {
    x: f32,
    y: f32,
}

impl DcBlock {
    fn tick(&mut self, x: f32) -> f32 {
        self.tick_at(x, 0.997)
    }

    fn tick_at(&mut self, x: f32, r: f32) -> f32 {
        self.y = x - self.x + r * self.y;
        self.x = x;
        self.y
    }
}

/// The flute's air jet: the breath crosses the embouchure and is swung across the edge by the bore's sound half a period before, flowing into the bore on one side or the other as a sigmoid of its swing.
/// Its inputs are the pitch in Hz, the breath's pressure, and white noise for the jet's turbulence.
/// A jet delayed half a period favours the odd harmonics, so the output takes a part of the square of the bore's sound for the even ones, as an edge the jet does not split evenly gives.
/// Its second output is the side of the edge the jet flows to, from 0 to 1, which its turbulence follows.
#[derive(Clone)]
pub(crate) struct Jet {
    bore: Line,
    jet: Line,
    loss: Loss,
    drive: DcBlock,
    out: DcBlock,
    /// The bore's corner as a multiple of the pitch, or in Hz, never under three times the pitch, when `corner_hz` is given.
    corner: f32,
    pub(crate) corner_hz: f32,
    /// How much of the bore's sound comes back from the open end.
    reflection: f32,
    /// How much of the jet's swing is turbulence.
    turbulence: f32,
    /// The part of the square of the sound in the output, and the part it falls to from C5 to C6.
    even: f32,
    pub(crate) high_even: f32,
    /// How much the breath drifts as a player's does, and its slow noise, filtered twice; near the threshold 1 sways a held note 8 to 11 dB, and 0.2 about 1.5, as the public-domain Flute 2 on Wikimedia Commons does.
    pub(crate) wander: f32,
    drift: [f32; 2],
    /// How far the breath falls while a slur moves the pitch, from 0 to 1, and the fall as it follows.
    pub(crate) catch: f32,
    fall: f32,
    last: f32,
    /// The breath at which the jet takes half a period to cross to the edge, or 0 to hold it there.
    /// The jet's speed grows as the square root of the breath, so blowing harder shortens its crossing and raises the pitch, as Terrien, Vergez, de la Cuadra and Fabre's jet-drive model has it, https://arxiv.org/abs/1402.3143.
    pub(crate) tuned: f32,
    /// How much louder a note sounds above middle C, as the power of its pitch's ratio to it.
    pub(crate) lift: f32,
    /// The bore's and the jet's delays the sample before, those a jump of note fades from, and how far it has faded, from 0 to 1.
    delays: [f32; 2],
    from: [f32; 2],
    fade: f32,
    rate: f32,
}

impl Jet {
    pub(crate) fn new(corner: f32, turbulence: f32, even: f32) -> Jet {
        Jet {
            bore: Line::new(4096),
            jet: Line::new(4096),
            loss: Loss::default(),
            drive: DcBlock::default(),
            out: DcBlock::default(),
            corner,
            corner_hz: 0.0,
            reflection: 0.5,
            turbulence,
            even,
            high_even: even,
            wander: 0.0,
            drift: [0.0; 2],
            catch: 0.0,
            fall: 0.0,
            last: 0.0,
            tuned: 0.0,
            lift: 0.0,
            delays: [0.0; 2],
            from: [0.0; 2],
            fade: 1.0,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Jet {
    const ID: u64 = 0x6d78_0060;
    type Inputs = U3;
    type Outputs = U2;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.bore.clear();
        self.jet.clear();
        self.loss = Loss::default();
        self.drive = DcBlock::default();
        self.out = DcBlock::default();
        self.drift = [0.0; 2];
        self.fall = 0.0;
        self.last = 0.0;
        self.delays = [0.0; 2];
        self.fade = 1.0;
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U2> {
        let noise = input[2];
        // The noise through two low-passes at 4 Hz, scaled so a wander of 1 sways the level about half a dB and the pitch about two cents, as the Philharmonia's flute does.
        let slow = Loss::pole(4.0, self.rate);
        self.drift[0] = (1.0 - slow) * noise + slow * self.drift[0];
        self.drift[1] = (1.0 - slow) * self.drift[0] + slow * self.drift[1];
        let drift = self.drift[1] * 250.0 * self.wander;
        // A slur moving faster than an octave a second, faster than any vibrato, lets the breath fall, and it comes back over about 20 ms.
        let octave = input[0].max(20.0).log2();
        let moving = if self.last == 0.0 {
            0.0
        } else {
            (((octave - self.last).abs() * self.rate - 1.0) / 2.0).clamp(0.0, 1.0)
        };
        self.last = octave;
        self.fall = if moving > self.fall {
            moving
        } else {
            Loss::pole(8.0, self.rate) * self.fall
        };
        let freq = (input[0] * 2f32.powf(drift * 6.0 / 1200.0)).clamp(20.0, 5000.0);
        let breath = input[1].max(0.0) * (1.0 + 0.035 * drift) * (1.0 - self.catch * self.fall);
        let period = self.rate / freq;
        let w = TAU * freq / self.rate;
        // A bore loses its sound by frequency, at its walls and open end, so a corner in Hz dulls the high notes' upper harmonics, which VSCO 2's flute has 10 dB under a corner at a multiple of the pitch.
        let corner = if self.corner_hz > 0.0 {
            self.corner_hz.max(3.0 * freq)
        } else {
            self.corner * freq
        };
        let a = Loss::pole(corner, self.rate);
        let u = self.loss.y;
        self.jet.write(self.drive.tick(u) + self.turbulence * noise);
        // A player rolls the flute in or out to stay in tune, taking back most of the change: the Philharmonia's flute rises 15 cents from mf to ff at A4 and 11 at G5, where the square root alone gives 38.
        let crossing = if self.tuned == 0.0 {
            1.0
        } else {
            (self.tuned / breath.max(1.0e-3)).powf(0.17).clamp(0.5, 2.0)
        };
        // A jump of note fades from the old delays to the new over 4 ms, as a tone hole opens, where switching at once clicks.
        let delays = [
            (period - 1.0 - Loss::lag(a, w)).max(2.0),
            0.5 * period * crossing,
        ];
        if self.delays[0] > 0.0 && (delays[0] - self.delays[0]).abs() > 0.01 * delays[0] {
            (self.from, self.fade) = (self.delays, 0.0);
        }
        self.delays = delays;
        self.fade = (self.fade + 1.0 / (0.004 * self.rate)).min(1.0);
        let swing =
            self.fade * self.jet.read(delays[1]) + (1.0 - self.fade) * self.jet.read(self.from[1]);
        let side = (1.5 * swing).tanh();
        let flow = -breath * side;
        self.bore.write(self.reflection * u + flow);
        // The loop delays by the line, the low-pass, and the sample before the low-pass is read again.
        let b = self.fade * self.bore.read(delays[0])
            + (1.0 - self.fade) * self.bore.read(self.from[0]);
        self.loss.tick(b, a);
        // A player blows a higher note harder, and the played Syrinx grows 0.3 dB a semitone up its runs, but harder breath here saturates the jet into a square wave, so the note is only made louder.
        let lift = (freq / 261.6).max(1.0).powf(self.lift);
        // VSCO 2's flute keeps its 2nd harmonic about 23 dB under the fundamental from C6 up, played soft or loud, where lower it grows as it is played louder.
        let even =
            self.high_even + (self.even - self.high_even) * (1046.5 / freq).log2().clamp(0.0, 1.0);
        [self.out.tick(b + even * b * b) * lift, 0.5 + 0.5 * side].into()
    }
}

/// A reed, or the lips, as a valve the pressure across it opens and closes, at the end of a bore.
/// The clarinet's bore is a cylinder, closed at the reed and open at the bell, whose sound comes back inverted after half a period, so it favours the odd harmonics.
/// A conical bore, as the oboe's, bassoon's and saxophone's, and the brass's, whose bell and mouthpiece bring their resonances to whole multiples, is a cylinder beside a short one as long as its missing apex, both open, with every harmonic.
/// Its inputs are the pitch in Hz, the mouth's pressure, and white noise for the breath's turbulence.
/// Its outputs are the wave that reaches the bell, which the bell radiates, the pitch it plays, and how far a slur's dip has yet to come back, from 0 to 1.
#[derive(Clone)]
pub(crate) struct Reed {
    bore: Line,
    loss: Loss,
    /// A conical bore's missing apex, a short open tube beside the bore, as part of the whole length.
    apex: Line,
    apex_loss: Loss,
    pub(crate) apex_part: f32,
    conical: bool,
    /// How freely the reed lets the air through, the embouchure's ζ, from 0 to 1.
    pub(crate) zeta: f32,
    /// The corner above which the bore's open holes or bell let the sound out instead of sending it back, `corner` times the pitch and never under `cutoff` Hz, and what the bore keeps at each round trip.
    /// A corner that follows the pitch plays every note alike, so a bore that sounds one note sounds them all; one in Hz, as a clarinet's row of open holes gives, lets a low note keep more harmonics than a high one.
    cutoff: f32,
    corner: f32,
    keep: f32,
    turbulence: f32,
    /// Whether the mouth's pressure is a multiple of the least that sounds the note, which grows as the bore loses more of the pitch.
    pub(crate) relative: bool,
    /// The bore's and the apex's delays the sample before, the ones a jump of note fades from, and how far it has faded, from 0 to 1.
    delay: f32,
    from: f32,
    short: f32,
    short_from: f32,
    fade: f32,
    /// The part of the mouth's pressure the tongue's release sends into the bore, and the samples left of it, which the breath's first arrival starts.
    pub(crate) tongue_part: f32,
    tongue: i32,
    pulse: f32,
    /// How far the mouth's pressure falls at a jump of note, how far it has yet to come back, from 0 to 1, and the seconds it takes to come back to a third, as a slur in the Philharmonia's legato phrases dips 6 to 9 dB over 25 to 35 ms on a reed and 13 to 14 dB over 65 ms on a brass wind.
    pub(crate) catch: f32,
    fall: f32,
    pub(crate) mend: f32,
    rate: f32,
}

impl Reed {
    pub(crate) fn new(conical: bool, cutoff: f32, corner: f32, keep: f32) -> Reed {
        Reed {
            bore: Line::new(8192),
            loss: Loss::default(),
            apex: Line::new(4096),
            apex_loss: Loss::default(),
            apex_part: 0.3,
            conical,
            zeta: 0.4,
            cutoff,
            corner,
            keep,
            turbulence: 0.005,
            relative: false,
            delay: 0.0,
            from: 0.0,
            short: 1.0,
            short_from: 1.0,
            fade: 1.0,
            tongue_part: 0.0,
            tongue: -1,
            pulse: 1.0,
            catch: 0.0,
            fall: 0.0,
            mend: 0.02,
            rate: DEFAULT_SR as f32,
        }
    }
}

/// The pole of the low-pass a bore's end sends its sound back through, at `freq` Hz, its corner at `cutoff` Hz and never under `corner` times the pitch.
fn end_pole(freq: f32, cutoff: f32, corner: f32, rate: f32) -> f32 {
    Loss::pole((corner * freq).max(cutoff).min(0.45 * rate), rate)
}

impl AudioNode for Reed {
    const ID: u64 = 0x6d78_0062;
    type Inputs = U3;
    type Outputs = U3;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.bore.clear();
        self.loss = Loss::default();
        self.apex.clear();
        self.apex_loss = Loss::default();
        self.delay = 0.0;
        self.fade = 1.0;
        self.tongue = -1;
        self.fall = 0.0;
    }

    fn tick(&mut self, input: &Frame<f32, U3>) -> Frame<f32, U3> {
        let freq = input[0].clamp(20.0, 5000.0);
        let noise = input[2];
        self.fall *= (-1.0 / (self.mend * self.rate)).exp();
        let period = self.rate / freq * 2f32.powf(SCOOP * self.fall / 1200.0);
        let w = TAU * freq / self.rate;
        let a = end_pole(freq, self.cutoff, self.corner, self.rate);
        // A reed starts to sound where the flow's slope with pressure, ζ (3γ - 1) / (2 √γ), makes up what the bore loses of the pitch at each round trip, (1 - g) / (1 + g).
        let least = if self.relative {
            let g = self.keep * (1.0 - a) / (1.0 - 2.0 * a * w.cos() + a * a).sqrt();
            let lost = (1.0 - g) / (1.0 + g);
            let root = (2.0 * lost + (4.0 * lost * lost + 12.0 * self.zeta * self.zeta).sqrt())
                / (6.0 * self.zeta);
            root * root
        } else {
            1.0
        };
        let mouth = input[1].max(0.0)
            * least
            * (1.0 + self.turbulence * noise)
            * (1.0 - self.catch * self.fall);
        // A cone is played as Dalmont, Gilbert and Kergomard's cylindrical saxophone: the bore beside a tube as long as the missing apex, both open, whose resonances fall on every harmonic of their joint length.
        let back = if self.conical {
            -0.5 * (self.loss.y + self.apex_loss.y)
        } else {
            -self.loss.y
        };
        // The flow through the reed is ζ (1 - γ + p) √(γ - p), the mouth's pressure γ and the mouthpiece's p as parts of the pressure that shuts the reed, as Dalmont, Gilbert and Kergomard's elementary model has it, https://hal.science/hal-00474987, shut from γ - p = 1 and reversed when p passes γ.
        // The bore takes the flow as p less twice the wave coming back, so the pressure solves p - 2 back = flow(p), which has one root while ζ is under 1; two tubes side by side take twice the flow, so back is their mean.
        let flow = |p: f32| {
            let across = mouth - p;
            if across >= 1.0 {
                0.0
            } else {
                self.zeta * (1.0 - across) * across.abs().sqrt() * across.signum()
            }
        };
        let (mut low, mut high) = (2.0 * back - 2.0, 2.0 * back + 2.0);
        for _ in 0..30 {
            let mid = 0.5 * (low + high);
            if mid - 2.0 * back - flow(mid) > 0.0 {
                high = mid
            } else {
                low = mid
            }
        }
        let pressure = 0.5 * (low + high);
        // The tongue's release sends a pulse of the mouth's pressure into the bore over a period, where the bore's own noise takes a third of a second to grow into a low note and Iowa's bassoon speaks in 0.07 to 0.16; it rises and falls smoothly, as a step would click once differenced at the bell.
        if self.tongue < 0 && mouth > 0.1 {
            (self.tongue, self.pulse) = (period as i32, period);
        }
        let pulse = if self.tongue > 0 {
            self.tongue -= 1;
            self.tongue_part * mouth * (0.5 - 0.5 * (TAU * self.tongue as f32 / self.pulse).cos())
        } else {
            0.0
        };
        self.bore.write(pressure + self.loss.y + pulse);
        self.apex.write(pressure + self.apex_loss.y);
        let delay = if self.conical {
            period * (1.0 - self.apex_part)
        } else {
            period / 2.0
        };
        let delay = (delay - 1.0 - Loss::lag(a, w)).max(2.0);
        let short = (period * self.apex_part - 1.0 - Loss::lag(a, w)).max(1.0);
        // A jump of note fades from the old delays to the new over 4 ms, as the flute's does, where switching at once clicks.
        if self.delay > 0.0 && (delay - self.delay).abs() > 0.01 * delay {
            (self.from, self.short_from, self.fade) = (self.delay, self.short, 0.0);
        }
        (self.delay, self.short) = (delay, short);
        // The dip falls as the fade goes, where falling at once clicks.
        if self.fade < 1.0 {
            self.fall = self.fall.max(self.fade);
        }
        self.fade = (self.fade + 1.0 / (0.004 * self.rate)).min(1.0);
        let b = self.fade * self.bore.read(delay) + (1.0 - self.fade) * self.bore.read(self.from);
        self.loss.tick(b * self.keep, a);
        if self.conical {
            let b = self.fade * self.apex.read(short)
                + (1.0 - self.fade) * self.apex.read(self.short_from);
            self.apex_loss.tick(b * self.keep, a);
        }
        [b, freq, self.fall].into()
    }
}

/// The cents flat a note starts after a slur, coming up as the dip comes back, as the notes of the Philharmonia's legato phrases start 13 to 16 cents flat.
const SCOOP: f32 = 20.0;

/// The samples the bell's line delays a wave at rest.
const STEEP: f32 = 60.0;

/// The end of a bore, which lets out what it does not send back: the wave that reaches it less the same wave through a low-pass at `cutoff` Hz, never under `corner` times the pitch.
/// A loud wave in a brass bore steepens on its way, its crests travelling faster than its troughs, so its upper harmonics grow with its level, as Msallam, Dequidt, Caussé and Tassart's trombone has them, "Physical model of the trombone including nonlinear effects", JASA 2000; here each sample arrives earlier by `steep` radians of the note's period for each unit of its pressure.
/// A crest cannot overtake the trough before it: once the wave is a shock, a sawtooth, more steepening leaves it one, so the steepening stops short of what the wave's steepest slope allows.
/// On a reed wind the same stage stands in for the reed's closing, which sharpens the wave's edge the harder the reed is blown.
/// Its inputs are the wave, the pitch in Hz, how far a slur's dip has yet to come back, the steepening, the part of the wave the breath has let through so far, from 0 to 1, and the part of the steepening the player's drift leaves, from 0 to 1.
#[derive(Clone)]
struct Bell {
    line: Line,
    before: f32,
    slope: f32,
    low: Loss,
    cutoff: f32,
    corner: f32,
    /// The corner, as a multiple of the pitch, of the two low-passes that round the wave before it steepens, as a bore's losses do, or 0 for none.
    smooth: f32,
    smoothed: [f32; 2],
    /// The dB louder the sound is an octave up, from middle C.
    lift: f32,
    /// How far the wave falls at a slur, from 0 to 1.
    catch: f32,
    rate: f32,
}

impl Bell {
    fn new(cutoff: f32, corner: f32) -> Bell {
        Bell {
            line: Line::new(128),
            before: 0.0,
            slope: 0.0,
            low: Loss::default(),
            cutoff,
            corner,
            smooth: 0.0,
            smoothed: [0.0; 2],
            lift: 0.0,
            catch: 0.0,
            rate: DEFAULT_SR as f32,
        }
    }
}

impl AudioNode for Bell {
    const ID: u64 = 0x6d78_0063;
    type Inputs = U6;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.line.clear();
        (self.before, self.slope) = (0.0, 0.0);
        self.low = Loss::default();
        self.smoothed = [0.0; 2];
    }

    fn tick(&mut self, input: &Frame<f32, U6>) -> Frame<f32, U1> {
        let freq = input[1].clamp(20.0, 5000.0);
        let smooth = Loss::pole(self.smooth * freq, self.rate);
        // At a slur the breath catches, and the wave falls and comes back with it.
        let mut wave = input[0] * input[4] * (1.0 - self.catch * input[2]);
        if self.smooth > 0.0 {
            for y in self.smoothed.iter_mut() {
                *y = (1.0 - smooth) * wave + smooth * *y;
                wave = *y;
            }
        }
        self.line.write(wave);
        // The wave's steepest slope over the last few periods, falling by half in 30 ms.
        let slope = (wave - self.before).abs();
        self.before = wave;
        self.slope = slope.max(self.slope * 0.5f32.powf(1.0 / (0.03 * self.rate)));
        // The steepening is in periods over 2π for each unit of pressure, so a note steepens as much at any pitch.
        let steep = (input[3].max(0.0) * self.rate / (TAU * freq))
            .min(SHOCK / self.slope.max(1e-6))
            * input[5];
        let wave = if steep > 0.0 {
            #[allow(clippy::let_and_return)]
            // The pressure that arrives now left earlier or later by its own level, found in two steps from the one that would arrive with no steepening.
            let at = |p: f32| (STEEP - steep * p).clamp(2.0, 2.0 * STEEP - 2.0);
            let first = self.line.read(at(self.line.read(STEEP)));
            self.line.read(at(first))
        } else {
            wave
        };
        let a = end_pole(freq, self.cutoff, self.corner, self.rate);
        let lift = db((self.lift * (freq / 261.63).log2()) as f64) as f32;
        [(wave - self.low.tick(wave, a)) * lift].into()
    }
}

/// How near a shock the steepening may bring the wave, where 1 is a shock.
const SHOCK: f32 = 1.0;

/// A slow random sway of about unit size, the sum of one that turns 1.3 times a second and one that turns 7, as a player's pitch and level drift and flutter while a note holds.
#[derive(Clone)]
struct Sway {
    seed: u64,
    state: u64,
    turns: [[f32; 3]; 2],
    rate: f32,
}

impl Sway {
    fn new(seed: u64) -> Sway {
        Sway {
            seed,
            state: seed | 1,
            turns: [[0.0; 3]; 2],
            rate: DEFAULT_SR as f32,
        }
    }

    fn draw(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

impl AudioNode for Sway {
    const ID: u64 = 0x6d78_0064;
    type Inputs = U0;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        self.state = self.seed | 1;
        self.turns = [[0.0; 3]; 2];
    }

    fn tick(&mut self, _: &Frame<f32, U0>) -> Frame<f32, U1> {
        let mut sum = 0.0;
        for (turn, (hz, part)) in [(1.3, 1.6), (7.0, 1.2)].into_iter().enumerate() {
            let [from, to, at] = self.turns[turn];
            let at = at + hz / self.rate;
            self.turns[turn] = if at >= 1.0 {
                [to, self.draw(), at - 1.0]
            } else {
                [from, to, at]
            };
            let [from, to, at] = self.turns[turn];
            sum += part * (from + (to - from) * (0.5 - 0.5 * (std::f32::consts::PI * at).cos()));
        }
        [sum].into()
    }
}

fn sway(seed: u64) -> Net {
    Net::wrap(Box::new(An(Sway::new(seed))))
}

/// The semitones a second above which a gliding note is a change of fingering, where a slower one is a bend.
const FINGERED: f32 = 6.0;

/// The longest the fingers hold the note they left, in seconds; a glide that lasts longer is a glissando, and the bore follows the rest of it.
const HELD: f32 = 0.08;

/// A player's fingers, or a brass player's lips and valves: the bore cannot pass through the pitches between two notes, so while the note glides fast they hold the one it left, and take the one it reaches when it stops.
/// In the Philharmonia's legato phrases the pitch is between two notes for 0 to 26 ms of a slur, where a glide spends all of its time there.
#[derive(Clone)]
struct Fingers {
    held: f32,
    before: f32,
    gliding: f32,
    started: bool,
    rate: f32,
}

impl AudioNode for Fingers {
    const ID: u64 = 0x6d78_0066;
    type Inputs = U1;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
    }

    fn reset(&mut self) {
        (self.gliding, self.started) = (0.0, false);
    }

    fn tick(&mut self, input: &Frame<f32, U1>) -> Frame<f32, U1> {
        let note = input[0];
        if !self.started {
            (self.held, self.before, self.started) = (note, note, true);
        }
        let fast = (note - self.before).abs() * self.rate > FINGERED;
        self.before = note;
        if fast && self.gliding < HELD {
            self.gliding += 1.0 / self.rate;
        } else {
            self.held = note;
            if !fast {
                self.gliding = 0.0;
            }
        }
        [self.held].into()
    }
}

/// Peaks at each band mx measures, their gains following vel between the ones fitted at pp, mf and ff, or held at those given for fitting, then the level for vel.
/// Its inputs are the sound and vel.
#[derive(Clone)]
struct Bands {
    gains: [[f32; 11]; 3],
    given: [Option<f32>; 11],
    levels: [f32; 3],
    vel: f32,
    level: f32,
    coefficients: [[f32; 5]; 11],
    state: [[f32; 4]; 11],
    rate: f32,
}

/// A value at `vel` between its values at pp, mf and ff, played at vel 0.3, 0.6 and 1.
fn by_vel(vel: f32, at: [f32; 3]) -> f32 {
    at[0]
        + (at[1] - at[0]) * ((vel - 0.3) / 0.3).clamp(0.0, 1.0)
        + (at[2] - at[1]) * ((vel - 0.6) / 0.4).clamp(0.0, 1.0)
}

impl Bands {
    fn new(gains: [[f32; 11]; 3], given: [Option<f32>; 11], levels: [f32; 3]) -> Bands {
        Bands {
            gains,
            given,
            levels,
            vel: -1.0,
            level: 0.0,
            coefficients: [[0.0; 5]; 11],
            state: [[0.0; 4]; 11],
            rate: DEFAULT_SR as f32,
        }
    }

    fn tune(&mut self, vel: f32) {
        self.vel = vel;
        // Below pp the breath fades the sound towards silence.
        self.level = db(by_vel(vel, self.levels) as f64) as f32 * (vel / 0.3).min(1.0);
        for (band, centre) in BANDS.into_iter().enumerate() {
            let gain = self.given[band].unwrap_or_else(|| {
                by_vel(
                    vel,
                    [
                        self.gains[0][band],
                        self.gains[1][band],
                        self.gains[2][band],
                    ],
                )
            });
            // A peaking filter after Robert Bristow-Johnson's "Cookbook formulae for audio EQ biquad filter coefficients".
            let a = 10f32.powf(gain / 40.0);
            let w = TAU * centre.min(0.45 * self.rate) / self.rate;
            let alpha = w.sin() / (2.0 * 1.4);
            let a0 = 1.0 + alpha / a;
            self.coefficients[band] = [
                (1.0 + alpha * a) / a0,
                -2.0 * w.cos() / a0,
                (1.0 - alpha * a) / a0,
                -2.0 * w.cos() / a0,
                (1.0 - alpha / a) / a0,
            ];
        }
    }
}

impl AudioNode for Bands {
    const ID: u64 = 0x6d78_0061;
    type Inputs = U2;
    type Outputs = U1;

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.rate = sample_rate as f32;
        self.vel = -1.0;
    }

    fn reset(&mut self) {
        self.state = [[0.0; 4]; 11];
        self.vel = -1.0;
    }

    fn tick(&mut self, input: &Frame<f32, U2>) -> Frame<f32, U1> {
        let vel = input[1].clamp(0.0, 1.0);
        if (vel - self.vel).abs() > 1e-4 {
            self.tune(vel);
        }
        let mut x = input[0];
        for ([b0, b1, b2, a1, a2], [x1, x2, y1, y2]) in
            self.coefficients.iter().zip(self.state.iter_mut())
        {
            let y = *b0 * x + *b1 * *x1 + *b2 * *x2 - *a1 * *y1 - *a2 * *y2;
            (*x2, *x1, *y2, *y1) = (*x1, x, *y1, y);
            x = y;
        }
        [x * self.level].into()
    }
}

/// An instrument's figures, fitted to its recordings at pp, mf and ff.
pub(crate) struct Fit {
    /// The gain in dB at each band.
    pub gains: [[f32; 11]; 3],
    /// The whole sound's level in dB.
    pub levels: [f32; 3],
    /// The breath's pressure.
    pub breath: [f32; 3],
    /// The seconds the breath takes to reach its pressure from silence.
    pub rise: [f32; 3],
    /// The breath's noise in dB, against the sound of the model.
    pub air: [f32; 3],
    /// The radians of the note's period a unit of pressure arrives earlier at the bell, which steepens a loud wave.
    pub steep: [f32; 3],
    /// The cents the model plays sharp at middle C, which the player takes back, with their change by the semitone from there and by the semitone squared.
    pub sharp: [[f32; 3]; 3],
    /// The dB louder the sound is an octave up.
    pub lift: f32,
}

/// A wind's options: those every wind takes, then for fitting each band's gain, the level, the breath and its rise, in place of those fitted for the note's vel.
fn wind_options(name: &str, vibrato: f64, vibrato_doc: &str) -> SynthDecl {
    let mut synth = Synth::new(name)
        .option(Spec::note("note").default(52.0).doc("The pitch; a change while the note holds slurs to the new one."))
        .option(Spec::float("vel").default(0.6).range(0.0, 1.0).doc("How hard it is blown; a change while the note holds swells or fades it."))
        .option(Spec::float("vibrato").default(vibrato).range(0.0, 2.0).doc(vibrato_doc))
        .options(Spec::envelope(0.5, 0.2))
        .option(Spec::float("breath").range(0.0, 3.0).doc("The breath's pressure, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::seconds("rise").range(0.001, 2.0).doc("The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::db("air").range(-90.0, 0.0).doc("The breath's noise in dB, in place of the one fitted for the note's vel; for fitting."))
        .option(Spec::db("level").range(-60.0, 60.0).doc("The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting."));
    for (name, hz) in EQ.iter().zip(BANDS) {
        let doc = format!(
            "The gain in dB of the band at {hz} Hz, in place of the one fitted for the note's vel; for fitting."
        );
        synth = synth.option(Spec::db(name).range(-60.0, 60.0).doc(&doc));
    }
    synth
}

/// The note's frequency, following its note option as the player's fingers do, swung by a vibrato `depth` semitones deep at `rate` that waits a quarter second, then grows in over 1.2.
/// The player takes back the cents the model plays sharp, `sharp` at pp, mf and ff, and drifts by about `jitter` cents.
fn pitch(o: &Options, depth: f64, rate: f64, sharp: [[f32; 3]; 3], jitter: f32, seed: u64) -> Net {
    let swing = shape(move |t| {
        let grown = ((t - 0.25) / 1.2).clamp(0.0, 1.0);
        (std::f64::consts::TAU * rate * t).sin() * depth * grown * grown
    });
    let fingers = Fingers {
        held: 0.0,
        before: 0.0,
        gliding: 0.0,
        started: false,
        rate: DEFAULT_SR as f32,
    };
    let note = o.follow("note") >> Net::wrap(Box::new(An(fingers)));
    (note | swing | sway(seed) | o.follow("vel"))
        >> Net::wrap(Box::new(map(move |x: &Frame<f32, U4>| {
            let from = x[0] - 60.0;
            let sharp = by_vel(
                x[3].clamp(0.0, 1.0),
                sharp.map(|[at, slope, curve]| at + slope * from + curve * from * from),
            );
            let cents = jitter * x[2] - sharp;
            hz(x[0] as f64 + x[1] as f64 + cents as f64 / 100.0) as f32
        })))
}

/// A vibrato's swing, growing in after the note starts, its depth drifting slowly by about a third either way, as a player's does from note to note, and its rate wandering from cycle to cycle.
pub(crate) fn drifting_swing(depth: f64, rate: f64, seed: u64) -> Net {
    // The flute's vibrato was fitted with these seeds.
    let seed = seed ^ 0x76;
    let grown = shape(move |t| {
        let grown = ((t - 0.25) / 1.2).clamp(0.0, 1.0);
        depth * grown * grown
    });
    // A sine given no phase starts where FunDSP's hash of the whole graph puts it, which moves whenever the graph around it does.
    let rates = white_noise(seed ^ 0x9e)
        >> Net::wrap(Box::new(
            lowpass_hz(3.0, 0.5)
                >> map(move |x: &Frame<f32, U1>| rate as f32 * (1.0 + WANDER * x[0]))
                >> An(Sine::<f32>::with_phase(0.0)),
        ));
    let swing = rates * grown;
    let drift =
        white_noise(seed) >> Net::wrap(Box::new(lowpass_hz(0.7, 0.5) >> lowpass_hz(0.7, 0.5)));
    swing
        * (drift
            >> Net::wrap(Box::new(map(|x: &Frame<f32, U1>| {
                (1.0 + DRIFT * x[0]).max(0.0)
            }))))
}

/// The scale that makes the twice low-passed noise sway the vibrato's depth by about a third.
const DRIFT: f32 = 100.0;

/// The scale that makes the low-passed noise wander the vibrato's rate about 6 percent from cycle to cycle, where VSCO 2's flute's wanders 6 to 9.
const WANDER: f32 = 12.0;

/// The seconds the breath takes to reach its pressure, fitted for the vel the note starts at.
fn rise(o: &Options, fit: &Fit) -> f64 {
    let vel = o.number("vel").clamp(0.0, 1.0) as f32;
    o.given("rise")
        .unwrap_or_else(|| by_vel(vel, fit.rise) as f64)
        .max(1e-3)
}

/// How far the breath has risen `t` seconds into a rise of `rise`, from 0 to 1.
fn risen(t: f64, rise: f64) -> f64 {
    1.0 - (1.0 - (t / rise).min(1.0)).powi(2)
}

/// The breath's pressure following vel, rising from silence over `rise` seconds.
fn breath(o: &Options, fit: &Fit, rise: f64) -> Net {
    let at = fit.breath;
    let given = o.given("breath").map(|b| b as f32);
    let pressure = o.follow("vel")
        >> Net::wrap(Box::new(map(move |v: &Frame<f32, U1>| {
            given.unwrap_or_else(|| by_vel(v[0].clamp(0.0, 1.0), at))
        })));
    pressure * shape(move |t| risen(t, rise))
}

/// The model's sound with the breath's noise around `centre` Hz, through the fitted bands, at the fitted level for vel.
fn voiced(o: &Options, ctx: &mut Context, model: Net, fit: &Fit, centre: f32) -> Net {
    let given = EQ.map(|name| o.given(name).map(|g| g as f32));
    let levels = o.given("level").map_or(fit.levels, |l| [l as f32; 3]);
    let airs = o.given("air").map_or(fit.air, |a| [a as f32; 3]);
    let air = o.follow("vel")
        >> Net::wrap(Box::new(map(move |v: &Frame<f32, U1>| {
            db(by_vel(v[0].clamp(0.0, 1.0), airs) as f64) as f32
        })));
    let noise = (white_noise(ctx.seed()) >> Net::wrap(Box::new(bandpass_hz(centre, 1.0)))) * air;
    ((model + noise) | o.follow("vel"))
        >> Net::wrap(Box::new(An(Bands::new(fit.gains, given, levels))))
}

/// The air jet with the breath's `wander` and its `catch` at each slur, its sound and the side the jet flows to.
pub(crate) fn played_jet(
    pitch: Net,
    breath: Net,
    seed: u64,
    even: f64,
    wander: f64,
    catch: f64,
    corner_hz: f64,
    tuned: f64,
    lift: f64,
) -> Net {
    let mut jet = Jet::new(6.0, 0.02, even as f32);
    (jet.corner_hz, jet.high_even) = (corner_hz as f32, FLUTE_EVEN[0].min(even as f32));
    (jet.wander, jet.catch, jet.tuned, jet.lift) =
        (wander as f32, catch as f32, tuned as f32, lift as f32);
    (pitch | breath | white_noise(seed)) >> Net::wrap(Box::new(An(jet)))
}

/// The bands and level as a defsynth plays them, following `vel`: `gains` at pp, mf and ff, eight to each for the bands up to 3162 Hz, and `levels` at each.
pub(crate) fn wind_bands(sound: Net, vel: Net, gains: &[f64], levels: &[f64]) -> Net {
    let gain = |d: usize| {
        std::array::from_fn(|band| {
            if band < 8 {
                gains[d * 8 + band] as f32
            } else {
                0.0
            }
        })
    };
    let bands = Bands::new(
        [gain(0), gain(1), gain(2)],
        [None; 11],
        [levels[0] as f32, levels[1] as f32, levels[2] as f32],
    );
    (sound | vel) >> Net::wrap(Box::new(An(bands)))
}

/// The part of the square of the jet's sound in the flute's output, at pp, mf and ff: VSCO 2's flute has its 2nd harmonic 23 dB under the fundamental played soft and 12 played loud.
const FLUTE_EVEN: [f32; 3] = [0.3, 1.0, 2.0];

/// The cents the flute's model plays sharp at `note`, which the player takes back: measured from B3 to Db7, 4.6 at A4 and less by 0.093 a semitone up, as the jet's path leads the bore's more at low notes.
fn flute_sharp(note: f64) -> f64 {
    4.6 - 0.093 * (note - 69.0)
}

const FLUTE: Fit = Fit {
    gains: [[0.0; 11]; 3],
    levels: [-12.0, -12.0, -12.0],
    breath: [0.38, 0.45, 0.56],
    rise: [0.05, 0.05, 0.05],
    air: [-90.0, -90.0, -90.0],
    steep: [0.0; 3],
    sharp: [[0.0; 3]; 3],
    lift: 0.0,
};

fn flute() -> Synth {
    wind_options("flute", 0.17, "The vibrato's depth, reached as the note holds: a flute's vibrato is in the breath, which swells and eases by 0.6 of it, so 0.17 swings a held note's level 1.6 to 2.3 dB, as a played Syrinx's 2 and under VSCO 2's 2 to 4, and its pitch follows.")
        .option(Spec::float("lift").default(0.5).range(0.0, 1.0).doc("How much louder a higher note sounds, as a player blows it harder, as the power of its pitch over middle C; 0 sounds every note alike."))
        .option(Spec::float("wander").default(0.2).range(0.0, 4.0).doc("How much the breath drifts while a note holds, as a player's does; 0 holds it still."))
        .option(Spec::float("catch").default(0.15).range(0.0, 1.0).doc("How far the breath falls while a slur moves the pitch; 0 glides smoothly."))
        .option(Spec::float("corner").range(500.0, 20_000.0).doc("The bore's corner in Hz, never under three times the pitch, in place of the one fitted; for fitting."))
        .option(Spec::db("shelf").range(-30.0, 30.0).doc("The gain in dB above 4 kHz, in place of the one fitted; for fitting."))
        .option(Spec::db("hiss").range(-90.0, 0.0).doc("The breath's hiss above 3 kHz in dB, in place of the one fitted; for fitting."))
        .option(Spec::float("tuned").range(0.0, 3.0).doc("The breath at which the jet crosses to the edge in half a period, in place of the mf breath; 0 holds it there."))
        .option(Spec::float("pulse").range(0.0, 1.0).doc("How much the hiss follows the jet across the edge, from 0 steady to 1 only on one side, in place of the one fitted; for fitting."))
        .option(Spec::float("even").range(0.0, 4.0).doc("The part of the square of the jet's sound in the output, in place of the one fitted; for fitting."))
        .build(|o, ctx| {
            let vel = o.number("vel").clamp(0.0, 1.0);
            let even = o.given("even").unwrap_or_else(|| by_vel(vel as f32, FLUTE_EVEN) as f64);
            let pitch = o.follow("note") >> Net::wrap(Box::new(map(|x: &Frame<f32, U1>| hz(x[0] as f64 - flute_sharp(x[0] as f64) / 100.0) as f32)));
            let throb = drifting_swing(0.6 * o.number("vibrato") * vel.powf(0.65), 5.3, ctx.seed()) >> Net::wrap(Box::new(map(|x: &Frame<f32, U1>| 1.0 + x[0])));
            let rise = rise(o, &FLUTE);
            let jet = played_jet(pitch, breath(o, &FLUTE, rise) * throb, ctx.seed(), even, o.number("wander"), o.number("catch"), o.given("corner").unwrap_or(3000.0), o.given("tuned").unwrap_or(FLUTE.breath[1] as f64), o.number("lift"));
            // The breath hisses above 3 kHz where the jet strikes the edge, pulsing as it crosses, after the bands, so they do not colour it.
            let pulse = o.given("pulse").unwrap_or(1.0) as f32;
            let side = Net::wrap(Box::new(map(move |s: &Frame<f32, U1>| 1.0 - pulse + 2.0 * pulse * s[0])));
            let hiss = side * (white_noise(ctx.seed()) >> Net::wrap(Box::new(highpass_hz(3000.0, 0.707) >> lowpass_hz(10_000.0, 0.707)))) * breath(o, &FLUTE, rise) * db(o.given("hiss").unwrap_or(-36.0)) as f32;
            // mx's bands stop at 4 kHz, and above them the jet is brighter than the Philharmonia's flute and a played Syrinx, by 10 dB from 4 to 10 kHz, which a shelf takes away.
            let bright = Net::wrap(Box::new(highshelf_hz(4000.0, 0.707, db(o.given("shelf").unwrap_or(-8.0)) as f32)));
            let tone = voiced(o, ctx, Net::wrap(Box::new(pass())), &FLUTE, 2000.0) >> bright;
            // Under pp the breath would stop the note, so a player fading in from nothing, as the Philharmonia's flute swells from 27 dB under its peak, only plays it quieter.
            let quiet = o.follow("vel") >> Net::wrap(Box::new(map(|v: &Frame<f32, U1>| (v[0] / 0.3).clamp(0.0, 1.0).powf(1.5))));
            Note::envelope((jet >> (tone | hiss) >> Net::wrap(Box::new(join::<U2>()))) * quiet, o)
        })
}

/// The flute's figures, fitted partial by partial to the University of Iowa's flute played with vibrato at pp, mf and ff.
fn flute_figures() -> &'static super::figures::Figures {
    static FIGURES: std::sync::OnceLock<super::figures::Figures> = std::sync::OnceLock::new();
    FIGURES.get_or_init(|| super::figures::Figures::read(include_bytes!("figures/flute.mxh")))
}

/// A flute played from figures fitted to its recordings partial by partial, a change of note a slur to the new note's figures, its loudness following vel as the recordings' loudest partial does.
fn flute_figured() -> Synth {
    Synth::new("flute_figured")
        .option(
            Spec::note("note")
                .default(72.0)
                .doc("The pitch; a change while the note holds slurs to the new one."),
        )
        .option(
            Spec::float("vel").default(0.6).range(0.0, 1.0).doc(
                "How hard the flute is blown; a change while the note holds swells or eases it.",
            ),
        )
        .option(
            Spec::float("vibrato")
                .range(0.0, 2.0)
                .doc("The vibrato's depth in semitones, in place of the recordings'."),
        )
        .option(Spec::float("inhale").default(0.0).range(0.0, 1.0).doc(
            "1 takes a breath once the note is let go, as a player does at the end of a phrase.",
        ))
        .options(Spec::envelope(0.5, 0.2))
        .build(|o, ctx| {
            let figures = flute_figures();
            // A slur dips the partials 1 dB, and the vibrato's depth spreads from note to note as a played Syrinx's does, 4 to 13 cents between its quartiles.
            let breath = super::figures::Breath {
                wander: 0.5,
                wander_above: 1.5,
                catch: 1.0,
                vary: 0.6,
                inhale: o.number("inhale") >= 0.5,
                even: 1.0,
                seed: ctx.seed(),
            };
            super::figures::held_note(
                figures,
                FLUTE_FIGURED_LEVEL,
                o.given("vibrato"),
                0.0,
                Some(breath),
                o,
            )
        })
}

/// The figured flute's gain at vel 1, as loud as the flute playing the same Sarabande.
const FLUTE_FIGURED_LEVEL: f64 = 3.4;

/// A reed or brass wind's bore and how it is played.
struct Bore {
    /// Whether it is a cone.
    conical: bool,
    /// The corner above which it stops sending the sound back, as a multiple of the pitch and never under `cutoff` Hz, and what it keeps at each round trip.
    corner: f32,
    cutoff: f32,
    keep: f32,
    /// The corner in Hz above which its end lets the sound out, never under `bell_corner` times the pitch.
    bell: f32,
    bell_corner: f32,
    /// The corner, as a multiple of the pitch, of the low-passes that round the wave before it steepens, or 0 for none.
    smooth: f32,
    /// How freely the reed lets the air through.
    zeta: f32,
    /// Whether the breath is a multiple of the least that sounds the note.
    relative: bool,
    /// The centre in Hz of the breath's noise.
    air: f32,
    /// The part of the mouth's pressure the tongue's release sends in, and the breath a soft note starts at before it falls to its own, or 0 to start at its own.
    tongue: f32,
    kick: f32,
    /// How far the sound falls at a slur, the seconds it takes to come back to a third, and how far the breath falls with it.
    catch: f32,
    mend: f32,
    gasp: f32,
    /// The cents the pitch drifts, the dB the level does, and the part of itself the steepening does, while a note holds.
    jitter: f32,
    shimmer: f32,
    wander: f32,
}

// Each reed and brass wind's bands, steepening and rise are fitted at pp, mf and ff to the harmonics and attacks of the Philharmonia's and the University of Iowa's notes, VSCO 2's for the horn and tuba and VCSL's for the tenor sax; its noise to their harmonics-to-noise; its levels and lift keep each note as loud as it was before the fit; its sharp is the cents the model plays sharp.
const CLARINET: Fit = Fit {
    gains: [
        [2.5, 1.0, -1.5, -1.2, -4.1, -1.5, 0.6, 3.0, 2.3, -1.0, -1.4],
        [1.1, 0.5, -0.5, 0.7, -1.1, -1.7, -1.9, 5.4, 2.0, -2.2, -1.8],
        [0.5, 1.4, -1.4, -0.1, 1.6, 0.6, -2.7, 0.5, 3.4, -2.0, -1.3],
    ],
    levels: [7.8, 9.6, 8.8],
    breath: [1.08, 1.3, 1.6],
    rise: [0.2, 0.1, 0.1],
    air: [-60.0, -60.0, -45.5],
    steep: [0.0, 0.0, 0.0],
    sharp: [
        [0.5561, -0.0234, -0.0005],
        [4.6487, 0.1855, -0.0036],
        [9.9079, 0.4126, -0.0097],
    ],
    lift: -3.76,
};
const OBOE: Fit = Fit {
    gains: [
        [-1.8, -8.4, -6.6, -3.0, -1.3, 7.9, 0.1, 2.6, 5.3, 2.0, 3.2],
        [-1.3, -6.4, -6.4, -6.2, -3.3, 9.3, 3.0, 6.1, 3.9, -0.1, 1.8],
        [-1.2, -5.7, -7.3, -6.0, -3.7, 6.9, -2.3, 5.5, 5.8, 2.6, 3.0],
    ],
    levels: [-5.1, 2.5, 7.9],
    breath: [0.6, 0.62, 0.66],
    rise: [0.35, 0.25, 0.2],
    air: [-50.0, -60.0, -51.0],
    steep: [1.0, 1.5, 3.2],
    sharp: [[8.7797, 0.0, 0.0], [7.5615, 0.0, 0.0], [5.2375, 0.0, 0.0]],
    lift: -3.65,
};
const BASSOON: Fit = Fit {
    gains: [
        [-5.6, -1.4, 2.3, 13.7, 3.1, 1.7, -0.1, 1.3, 2.3, 1.4, 0.8],
        [-8.4, -5.4, -0.1, 9.4, 1.4, 7.0, 2.2, 1.9, 3.5, 5.4, 4.1],
        [-10.0, -6.8, -1.2, 8.1, 0.8, 6.5, 3.5, 3.9, 6.1, 6.7, 4.3],
    ],
    levels: [-19.4, -5.7, -0.3],
    breath: [0.6, 0.62, 0.66],
    rise: [0.3, 0.15, 0.15],
    air: [-35.0, -50.0, -42.5],
    steep: [3.2, 3.2, 3.2],
    sharp: [[8.7327, 0.0, 0.0], [7.517, 0.0, 0.0], [5.1921, 0.0, 0.0]],
    lift: -4.60,
};
/// The semitones the tenor sax's vibrato at 1 swings, which measures 10.8 cents deep, as VCSL's tenor saxophone played with vibrato.
const TENOR_SAX_VIBRATO: f64 = 0.145;
const TENOR_SAX: Fit = Fit {
    gains: [
        [-4.5, -12.0, -5.2, -3.0, -0.5, -0.3, 2.9, 8.5, 5.6, 4.6, 2.8],
        [-10.8, -11.1, -2.3, 0.3, -5.5, -2.1, 3.9, 9.5, 9.9, 6.9, 4.7],
        [
            -9.8, -10.6, -3.8, -1.3, -8.3, -3.8, 1.8, 5.7, 7.6, 12.1, 7.8,
        ],
    ],
    levels: [0.3, 9.4, 12.9],
    breath: [0.6, 0.62, 0.66],
    rise: [0.01, 0.012, 0.006],
    air: [-54.5, -60.0, -58.0],
    steep: [1.0, 1.5, 3.2],
    sharp: [[8.7338, 0.0, 0.0], [7.5151, 0.0, 0.0], [5.1983, 0.0, 0.0]],
    lift: -7.47,
};
const TRUMPET: Fit = Fit {
    gains: [
        [
            -4.7, -11.7, -4.7, -1.7, -1.3, 12.7, 11.1, 2.9, 0.2, -0.6, -0.1,
        ],
        [-3.2, -9.4, -7.8, -4.3, -2.3, 3.4, 5.5, 8.7, 4.1, 3.9, 1.8],
        [-3.8, -10.8, -10.0, -5.1, -5.0, 1.0, 2.8, 7.2, 6.3, 9.8, 4.3],
    ],
    levels: [0.9, 9.7, 13.3],
    breath: [0.6, 0.62, 0.66],
    rise: [0.35, 0.35, 0.2],
    air: [-60.0, -55.0, -55.0],
    steep: [1.0, 3.2, 3.2],
    sharp: [[8.7541, 0.0, 0.0], [7.5353, 0.0, 0.0], [5.2181, 0.0, 0.0]],
    lift: -9.31,
};
const TROMBONE: Fit = Fit {
    gains: [
        [-8.0, -1.3, 0.4, 3.9, 5.1, 6.1, -0.4, -0.5, 1.1, 2.2, 1.1],
        [-11.2, -7.3, -4.1, -0.9, 0.7, 5.4, 6.4, 10.2, 7.5, 2.8, 1.3],
        [
            -12.7, -13.2, -11.3, -7.0, -5.1, 0.7, 5.0, 8.6, 14.8, 16.7, 5.4,
        ],
    ],
    levels: [-21.9, -4.7, 12.0],
    breath: [0.6, 0.62, 0.66],
    rise: [0.3, 0.3, 0.25],
    air: [-37.0, -45.5, -50.0],
    steep: [2.2, 3.2, 3.2],
    sharp: [[8.7325, 0.0, 0.0], [7.5093, 0.0, 0.0], [5.1892, 0.0, 0.0]],
    lift: -7.77,
};
const HORN: Fit = Fit {
    gains: [
        [-4.0, 0.8, 4.5, 8.3, 4.7, 0.4, -3.7, -5.5, -3.9, -0.2, 1.1],
        [-9.2, -1.9, 4.4, 7.5, 5.3, 7.2, 4.7, -1.9, -1.6, 0.6, 1.1],
        [-12.8, -8.4, -1.7, 3.3, 1.2, 0.4, 7.5, 8.2, 7.5, 6.4, 3.1],
    ],
    levels: [-14.4, -9.7, 4.2],
    breath: [0.6, 0.62, 0.66],
    rise: [0.2, 0.25, 0.25],
    air: [-35.5, -35.5, -42.0],
    steep: [2.2, 3.2, 3.2],
    sharp: [[8.7341, 0.0, 0.0], [7.5077, 0.0, 0.0], [5.186, 0.0, 0.0]],
    lift: -5.56,
};
const TUBA: Fit = Fit {
    gains: [
        [3.5, 5.5, 5.1, 3.1, 8.0, 2.8, -1.9, -0.7, 0.0, 0.1, 0.0],
        [-3.2, 3.8, 7.0, 5.5, 6.9, 4.8, 0.3, -0.1, 0.2, 0.5, 0.1],
        [-12.5, -4.3, -0.0, -0.3, 5.7, 8.8, 10.1, 8.6, 4.5, 1.3, 0.3],
    ],
    levels: [-23.9, -16.6, 0.7],
    breath: [0.6, 0.62, 0.66],
    rise: [0.25, 0.22, 0.18],
    air: [-30.0, -30.0, -36.0],
    steep: [2.2, 3.2, 3.2],
    sharp: [[8.7129, 0.0, 0.0], [7.4601, 0.0, 0.0], [5.1423, 0.0, 0.0]],
    lift: -3.93,
};

/// The seconds the breath takes to reach its pressure, as fast as a tongue lets it go, and the seconds its kick takes to fall to a third.
const SPEAKS: f64 = 0.01;
const KICK: f64 = 0.05;

/// A reed or brass wind whose vibrato option swings `unit` semitones at 1.
fn reed_wind(name: &'static str, rate: f64, unit: f64, bore: Bore, fit: &'static Fit) -> Synth {
    let doc = if unit == 1.0 {
        "The vibrato's depth in semitones, reached as the note holds."
    } else {
        "The vibrato's depth, reached as the note holds, where 1 is the depth of the recordings played with vibrato."
    };
    let fitting = |name: &str, high: f64, what: &str| {
        let doc = format!("{what}, in place of the fitted one; for fitting.");
        Spec::float(name).range(0.0, high).doc(&doc)
    };
    wind_options(name, 0.0, doc)
        .option(fitting("kick", 3.0, "The breath a soft note starts at"))
        .option(fitting("catch", 1.0, "How far the sound falls at a slur"))
        .option(fitting(
            "mend",
            1.0,
            "The seconds a slur's dip takes to come back to a third",
        ))
        .option(fitting("gasp", 1.0, "How far the breath falls at a slur"))
        .option(fitting(
            "corner",
            100.0,
            "The bore's corner as a multiple of the pitch",
        ))
        .option(fitting(
            "cutoff",
            20_000.0,
            "The least the bore's corner is, in Hz",
        ))
        .option(fitting(
            "keep",
            1.0,
            "What the bore keeps at each round trip",
        ))
        .option(fitting(
            "zeta",
            1.0,
            "How freely the reed lets the air through",
        ))
        .option(fitting(
            "apex",
            0.5,
            "A cone's missing apex, as part of its whole length",
        ))
        .option(fitting(
            "bell",
            20_000.0,
            "The corner in Hz above which the bore's end lets the sound out",
        ))
        .option(fitting(
            "tongue",
            2.0,
            "The part of the mouth's pressure the tongue's release sends in",
        ))
        .option(fitting(
            "steep",
            50.0,
            "The radians of the note's period a unit of pressure arrives earlier at the bell",
        ))
        .option(fitting(
            "jitter",
            50.0,
            "The cents the pitch drifts while a note holds",
        ))
        .option(fitting(
            "shimmer",
            6.0,
            "The dB the level drifts while a note holds",
        ))
        .option(fitting(
            "wander",
            4.0,
            "The part of itself the steepening drifts while a note holds",
        ))
        .option(Spec::float("lift").range(-24.0, 24.0).doc(
            "The dB louder the sound is an octave up, in place of the fitted one; for fitting.",
        ))
        .option(
            Spec::float("sharp")
                .range(-100.0, 100.0)
                .doc("The cents the model plays sharp, in place of the fitted ones; for fitting."),
        )
        .build(move |o, ctx| {
            let or = |name: &str, fitted: f32| o.given(name).map_or(fitted, |given| given as f32);
            let mut reed = Reed::new(
                bore.conical,
                or("cutoff", bore.cutoff),
                or("corner", bore.corner),
                or("keep", bore.keep),
            );
            reed.zeta = or("zeta", bore.zeta);
            reed.apex_part = or("apex", 0.3);
            reed.relative = bore.relative;
            reed.tongue_part = or("tongue", bore.tongue);
            (reed.catch, reed.mend) = (or("gasp", bore.gasp), or("mend", bore.mend));
            let sharp = o
                .given("sharp")
                .map_or(fit.sharp, |cents| [[cents as f32, 0.0, 0.0]; 3]);
            let pitch = pitch(
                o,
                o.number("vibrato") * unit,
                rate,
                sharp,
                or("jitter", bore.jitter),
                ctx.seed(),
            );
            let steeps = o
                .given("steep")
                .map_or(fit.steep, |steep| [steep as f32; 3]);
            let wander = or("wander", bore.wander);
            let steep = o.follow("vel")
                >> Net::wrap(Box::new(map(move |x: &Frame<f32, U1>| {
                    by_vel(x[0].clamp(0.0, 1.0), steeps)
                })));
            // The drift only eases the steepening, which is already as near a shock as it may be.
            let eased = sway(ctx.seed())
                >> Net::wrap(Box::new(map(move |x: &Frame<f32, U1>| {
                    1.0 - (wander * (1.0 + x[0])).clamp(0.0, 1.0)
                })));
            // A reed near the least breath that sounds it takes a third of a second to speak, where a player's tongue starts a soft note in under a tenth: the breath comes at once, with a kick that dies away, and the bell lets the wave through as the player's breath rises.
            let rise = rise(o, fit);
            let let_through = shape(move |t| risen(t, rise));
            let start = by_vel(o.number("vel").clamp(0.0, 1.0) as f32, fit.breath);
            let kick = (or("kick", bore.kick) - start).max(0.0) as f64;
            let kicked = shape(move |t| kick * (-t / KICK).exp() * risen(t, SPEAKS));
            let breath = breath(o, fit, rise.min(SPEAKS)) + kicked;
            let model = (pitch | breath | white_noise(ctx.seed())) >> Net::wrap(Box::new(An(reed)));
            let mut bell = Bell::new(or("bell", bore.bell), bore.bell_corner);
            (bell.smooth, bell.lift, bell.catch) =
                (bore.smooth, or("lift", fit.lift), or("catch", bore.catch));
            let sound = (model | steep | let_through | eased) >> Net::wrap(Box::new(An(bell)));
            let depth = or("shimmer", bore.shimmer);
            let shimmer = sway(ctx.seed())
                >> Net::wrap(Box::new(map(move |x: &Frame<f32, U1>| {
                    db((depth * x[0]) as f64) as f32
                })));
            Note::envelope(voiced(o, ctx, sound, fit, bore.air) * shimmer, o)
        })
}
