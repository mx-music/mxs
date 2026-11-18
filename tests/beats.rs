use fundsp::prelude::*;
use mxs::{
    Engine, Error, Event, Note, Pass, RATE, Report, Spec, Synth, Target, Tempo, Topic, Value,
    effects, synths,
};

fn engine() -> Engine {
    let mut engine = Engine::offline(RATE);
    synths::all().into_iter().for_each(|s| engine.register(s));
    effects::all()
        .into_iter()
        .for_each(|e| engine.register_effect(e));
    engine.register(click());
    engine.register(held());
    engine.register(Synth::new("level").build(|_, _| Note::rings(dc(0.25), 8.0)));
    engine.start_part("lead", &[]).unwrap();
    engine
}

/// A level of 0.25 from the note's first frame, for 10 ms.
fn click() -> Synth {
    Synth::new("click").build(|_, _| Note::rings(dc(0.25), 0.01))
}

/// A level that falls silent over a release of one beat.
fn held() -> Synth {
    Synth::new("held")
        .options(Spec::envelope(0.0, 1.0))
        .build(|options, _| Note::envelope(dc(0.25), options))
}

fn clicks(beats: &[f64]) -> Pass {
    let events = beats
        .iter()
        .map(|beat| (*beat, Event::note(None, "click", &[])));
    Pass {
        events: events.collect(),
        ..Pass::default()
    }
}

/// The frames at which sound starts after at least 1 ms of silence.
fn onsets(samples: &[f32]) -> Vec<usize> {
    let mut onsets = Vec::new();
    let mut silent = 48;
    for (frame, sample) in samples.iter().enumerate() {
        if sample.abs() > 0.01 {
            if silent >= 48 {
                onsets.push(frame);
            }
            silent = 0;
        } else {
            silent += 1;
        }
    }
    onsets
}

fn differ(a: &[f32], b: &[f32]) {
    let count = a.iter().zip(b).filter(|(x, y)| x != y).count();
    let first = a.iter().zip(b).position(|(x, y)| x != y);
    let most = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0f32, f32::max);
    println!("{count} differ, first at {first:?}, by up to {most}");
}

fn whole_beats() -> Vec<f64> {
    (0..8).map(f64::from).collect()
}

#[test]
fn at_120_beats_a_minute_each_event_starts_at_the_frame_of_its_beat() {
    let mut engine = engine();
    engine.pass("lead", &clicks(&whole_beats())).unwrap();
    let (left, _) = engine.render(4.0).unwrap();
    assert_eq!(
        onsets(&left),
        [0, 24_000, 48_000, 72_000, 96_000, 120_000, 144_000, 168_000]
    );
    assert_eq!(engine.beat(), 8.0);
    assert_eq!(engine.beat_seconds(5.0), 2.5);
    assert_eq!(engine.beat_at(2.5), 5.0);
}

#[test]
fn a_tempo_change_moves_the_events_already_handed_over() {
    let mut engine = engine();
    engine.pass("lead", &clicks(&whole_beats())).unwrap();
    engine.set_tempo(4.0, Tempo::Hold(60.0)).unwrap();
    let (left, _) = engine.render(6.0).unwrap();
    assert_eq!(
        onsets(&left),
        [0, 24_000, 48_000, 72_000, 96_000, 144_000, 192_000, 240_000]
    );
}

#[test]
fn a_tempo_moving_from_120_to_60_over_4_beats_puts_beat_4_at_frame_133084() {
    let mut engine = engine();
    let moving = Tempo::Move {
        from: 120.0,
        to: 60.0,
        over: 4.0,
    };
    engine.set_tempo(0.0, moving).unwrap();
    engine.pass("lead", &clicks(&[0.0, 4.0, 5.0])).unwrap();
    let (left, _) = engine.render(4.0).unwrap();
    assert_eq!(onsets(&left), [0, 133_084, 181_084]);
}

#[test]
fn a_release_of_one_beat_rings_twice_as_long_at_60_beats_a_minute() {
    let rings = |bpm: f64| {
        let mut engine = engine();
        engine.set_tempo(0.0, Tempo::Hold(bpm)).unwrap();
        let events = vec![(0.0, Event::note(None, "held", &[]))];
        let pass = Pass {
            events,
            ..Pass::default()
        };
        engine.pass("lead", &pass).unwrap();
        let (left, _) = engine.render(2.0).unwrap();
        left.iter().rposition(|sample| *sample != 0.0).unwrap() as f64
    };
    let (slow, fast) = (rings(60.0), rings(120.0));
    assert!((fast - 24_000.0).abs() < 100.0, "{fast} frames at 120");
    assert!((slow - 48_000.0).abs() < 100.0, "{slow} frames at 60");
}

#[test]
fn a_slide_given_in_beats_lasts_those_beats() {
    let slid = |bpm: f64| {
        let mut engine = engine();
        engine.set_tempo(0.0, Tempo::Hold(bpm)).unwrap();
        let options = [("amp", 1.0)];
        let change = Event::Set {
            target: Target::Sounds(vec![1]),
            options: vec![("amp".to_string(), 0.0), ("amp_slide".to_string(), 1.0)],
        };
        let events = vec![
            (0.0, Event::note(Some(1), "level", &options)),
            (1.0, change),
        ];
        let pass = Pass {
            events,
            ..Pass::default()
        };
        engine.pass("lead", &pass).unwrap();
        let (left, _) = engine.render(3.0).unwrap();
        left.iter()
            .rposition(|sample| sample.abs() > 0.001)
            .unwrap() as f64
            / RATE
    };
    // The change comes at beat 1 and takes a beat, so the note is silent at beat 2.
    let (fast, slow) = (slid(120.0), slid(60.0));
    assert!((fast - 1.0).abs() < 0.02, "silent at {fast} s at 120");
    assert!((slow - 2.0).abs() < 0.02, "silent at {slow} s at 60");
}

#[test]
fn a_cut_leaves_the_events_from_its_beat_on_unheard_and_another_pass_takes_their_place() {
    let mut engine = engine();
    engine.pass("lead", &clicks(&whole_beats())).unwrap();
    engine.cut("lead", 4.0).unwrap();
    engine.pass("lead", &clicks(&[4.0, 5.5])).unwrap();
    let (left, _) = engine.render(4.0).unwrap();
    assert_eq!(onsets(&left), [0, 24_000, 48_000, 72_000, 96_000, 132_000]);
}

#[test]
fn a_cut_takes_back_an_event_already_placed_that_has_not_started() {
    let mut engine = engine();
    engine.pass("lead", &clicks(&whole_beats())).unwrap();
    // Beat 4 falls at 2 s, 100 ms after the cut, so it was placed at its frame already.
    let (mut left, _) = engine.render(1.9).unwrap();
    engine.cut("lead", 4.0).unwrap();
    left.extend(engine.render(2.1).unwrap().0);
    assert_eq!(onsets(&left), [0, 24_000, 48_000, 72_000]);
    let nowhere = engine.cut("nowhere", 4.0).unwrap_err();
    assert!(matches!(nowhere, Error::UnknownPart(_)));
}

#[test]
fn a_tempo_change_moves_an_event_200_ms_away_and_not_one_100_ms_away() {
    let moved = |before: f64| {
        let mut engine = engine();
        engine.pass("lead", &clicks(&[0.0, 2.0])).unwrap();
        let (mut left, _) = engine.render(1.0 - before).unwrap();
        engine.set_tempo(0.0, Tempo::Hold(60.0)).unwrap();
        left.extend(engine.render(2.0).unwrap().0);
        onsets(&left)
    };
    assert_eq!(moved(0.2), [0, 96_000]);
    assert_eq!(moved(0.1), [0, 48_000]);
}

#[test]
fn an_event_a_tenth_of_a_beat_late_plays_at_once_and_one_half_a_beat_late_is_dropped() {
    let mut engine = engine();
    let (mut left, _) = engine.render(1.0).unwrap();
    assert_eq!(engine.warnings(), Vec::<String>::new());
    engine.pass("lead", &clicks(&[1.5, 1.9, 3.0])).unwrap();
    left.extend(engine.render(1.0).unwrap().0);
    assert_eq!(onsets(&left), [48_000, 72_000]);
    assert_eq!(
        engine.warnings(),
        [
            "1 event arrived after their time, by up to 50.0 ms, and played at once",
            "1 event was dropped, handed over a quarter of a beat or more after the beat"
        ]
    );
}

#[test]
fn a_pass_about_to_end_and_a_pass_with_a_new_label_are_reported() {
    let mut engine = engine();
    let reports = engine.subscribe(&[Topic::Passes]);
    let pass = |from: f64, label: u64| Pass {
        from,
        to: from + 32.0,
        label,
        notice: Some(4.0),
        events: vec![(from + 1.0, Event::note(None, "click", &[]))],
    };
    engine.pass("lead", &pass(0.0, 7)).unwrap();
    engine.render(13.9).unwrap();
    let started = Report::PassStarted {
        seconds: 0.5,
        part: "lead".to_string(),
        label: 7,
        beat: 1.0,
    };
    assert_eq!(reports.next(), Some(started));
    assert_eq!(reports.next(), None);
    engine.render(0.2).unwrap();
    let ending = Report::PassEnding {
        seconds: 14.0,
        part: "lead".to_string(),
        beat: 32.0,
    };
    assert_eq!(reports.next(), Some(ending));

    // A pass that follows with the same label is not reported as started, and one with another label is.
    engine.pass("lead", &pass(32.0, 7)).unwrap();
    engine.render(4.0).unwrap();
    assert_eq!(reports.next(), None);
    engine.pass("lead", &pass(64.0, 8)).unwrap();
    engine.render(16.0).unwrap();
    let ending = Report::PassEnding {
        seconds: 46.0,
        part: "lead".to_string(),
        beat: 96.0,
    };
    let started = Report::PassStarted {
        seconds: 32.5,
        part: "lead".to_string(),
        label: 8,
        beat: 65.0,
    };
    assert_eq!(reports.next(), Some(started));
    assert_eq!(reports.next(), None);
    engine.render(12.0).unwrap();
    assert_eq!(reports.next(), Some(ending));
}

#[test]
fn the_beat_and_its_tempo_are_reported_on_start_and_on_each_tempo_change() {
    let mut engine = engine();
    let reports = engine.subscribe(&[Topic::Beats]);
    engine.render(1.0).unwrap();
    let beat = |seconds: f64, beat: f64, bpm: f64| Report::Beat {
        seconds,
        beat,
        bpm,
        system: None,
    };
    assert_eq!(reports.next(), Some(beat(0.0, 0.0, 120.0)));
    assert_eq!(reports.next(), None);
    engine.set_tempo(8.0, Tempo::Hold(60.0)).unwrap();
    engine.render(1.0).unwrap();
    assert_eq!(reports.next(), None);
    engine.render(3.0).unwrap();
    assert_eq!(reports.next(), Some(beat(4.0, 8.0, 60.0)));
    engine.place_beats(1.0).unwrap();
    assert_eq!(reports.next(), Some(beat(5.0, 8.0, 60.0)));
    // Whoever subscribes later is sent the last one at once.
    let late = engine.subscribe(&[Topic::Beats]);
    assert_eq!(late.next(), Some(beat(5.0, 8.0, 60.0)));
}

// (beat, note, vel, beats held): the opening of the play command's piano phrase.
const PHRASE: &[(f64, f64, f64, f64)] = &[
    (0.0, 48.0, 0.5, 4.0),
    (0.0, 55.0, 0.4, 4.0),
    (0.0, 64.0, 0.6, 1.0),
    (1.0, 67.0, 0.6, 1.0),
    (2.0, 72.0, 0.7, 1.0),
    (3.0, 71.0, 0.6, 1.0),
    (4.0, 45.0, 0.5, 4.0),
    (4.0, 52.0, 0.4, 4.0),
    (4.0, 69.0, 0.6, 1.5),
    (5.5, 67.0, 0.5, 0.5),
    (6.0, 64.0, 0.6, 2.0),
];
const BPM: f64 = 84.0;

/// The phrase's notes, each at its beat, with its sustain and its release of one beat multiplied by `per_beat`.
fn phrase(per_beat: f64) -> Vec<(f64, Event)> {
    let note = |&(beat, note, vel, held): &(f64, f64, f64, f64)| {
        let options = [
            ("note", note),
            ("vel", vel),
            ("sustain", held * per_beat),
            ("release", per_beat),
        ];
        (beat, Event::note(None, "piano", &options))
    };
    PHRASE.iter().map(note).collect()
}

fn in_beats() -> Engine {
    let mut engine = engine();
    engine.set_tempo(0.0, Tempo::Hold(BPM)).unwrap();
    let pass = Pass {
        events: phrase(1.0),
        ..Pass::default()
    };
    engine.pass("lead", &pass).unwrap();
    engine
}

fn in_steps(engine: &mut Engine, step: usize, frames: usize) -> Vec<f32> {
    let mut left = Vec::with_capacity(frames);
    while left.len() < frames {
        let length = Ord::min(step, frames - left.len());
        left.extend(engine.render(length as f64 / RATE).unwrap().0);
    }
    left
}

#[test]
fn events_in_seconds_play_the_same_whatever_the_tempo_map_holds() {
    let per_beat = 60.0 / BPM;
    let in_seconds = || {
        let mut engine = engine();
        let timed = |(beat, event): (f64, Event)| (beat * per_beat, event);
        let events: Vec<(f64, Event)> = phrase(per_beat).into_iter().map(timed).collect();
        engine.play(&events).unwrap();
        engine
    };
    let (alone, _) = in_seconds().render(8.0).unwrap();
    let mut engine = in_seconds();
    engine.set_tempo(0.0, Tempo::Hold(33.0)).unwrap();
    engine.place_beats(1.7).unwrap();
    engine.pass("lead", &clicks(&[40.0])).unwrap();
    let (beside, _) = engine.render(8.0).unwrap();
    assert!(alone.iter().any(|sample| sample.abs() > 0.01));
    assert!(alone == beside);
}

#[test]
fn blocks_filled_from_events_at_frames_hold_the_samples_of_the_same_events_in_beats() {
    let frames = 8 * RATE as usize;
    let (in_beats, _) = in_beats().render(8.0).unwrap();

    let per_beat = 60.0 / BPM;
    let mut engine = engine();
    let part = engine.part("lead").unwrap();
    let at_frame = |(beat, event): (f64, Event)| {
        let Event::Note {
            id, synth, options, ..
        } = event
        else {
            unreachable!()
        };
        let chain = Some(part);
        let event = Event::Note {
            id,
            synth,
            options,
            chain,
        };
        ((beat * per_beat * RATE).round() as u64, event)
    };
    let events: Vec<(u64, Event)> = phrase(per_beat).into_iter().map(at_frame).collect();
    engine.play_at_frames(&events).unwrap();
    let (mut left, mut right) = (vec![0.0; frames], vec![0.0; frames]);
    for (left, right) in left.chunks_mut(128).zip(right.chunks_mut(128)) {
        engine.fill(left, right).unwrap();
    }
    assert!(left.iter().any(|sample| sample.abs() > 0.01));
    differ(&left, &in_beats);
    assert!(left == in_beats);
}

#[test]
fn a_render_holds_the_same_samples_64_frames_at_a_time_and_4096_at_a_time() {
    let program = || {
        let mut engine = in_beats();
        let echo = ("echo".to_string(), vec![]);
        engine.start_part_in_beats("echoed", &[echo]).unwrap();
        engine.pass("echoed", &clicks(&[0.5, 3.0, 6.25])).unwrap();
        let moving = Tempo::Move {
            from: BPM,
            to: 120.0,
            over: 2.0,
        };
        engine.set_tempo(4.0, moving).unwrap();
        engine
    };
    let frames = 6 * RATE as usize;
    let small = in_steps(&mut program(), 64, frames);
    let large = in_steps(&mut program(), 4096, frames);
    assert!(small.iter().any(|sample| sample.abs() > 0.01));
    differ(&small, &large);
    assert!(small == large);
}

fn echoed() -> Engine {
    let mut engine = engine();
    let options = [("phase", 1.0), ("decay", 8.0), ("max_phase", 4.0)]
        .map(|(name, value)| (name.to_string(), Value::Number(value)));
    let echo = ("echo".to_string(), options.to_vec());
    engine.start_part_in_beats("echoed", &[echo]).unwrap();
    engine.pass("echoed", &clicks(&[4.0])).unwrap();
    engine
}

#[test]
fn an_echo_a_beat_apart_repeats_twice_as_far_apart_once_the_tempo_halves() {
    let (steady, _) = echoed().render(5.0).unwrap();
    assert_eq!(onsets(&steady)[..4], [96_000, 120_000, 144_000, 168_000]);

    let mut engine = echoed();
    engine.set_tempo(4.0, Tempo::Hold(60.0)).unwrap();
    let (halved, _) = engine.render(5.0).unwrap();
    assert_eq!(onsets(&halved)[..3], [96_000, 144_000, 192_000]);
}

#[test]
fn an_effect_option_set_in_beats_in_a_pass_follows_the_tempo_and_one_set_in_seconds_no_longer_does()
{
    let set = |phase: f64, part: u64| Event::Set {
        target: Target::Effect(part, 0),
        options: vec![("phase".to_string(), phase)],
    };
    let mut engine = echoed();
    let part = engine.part("echoed").unwrap();
    let pass = Pass {
        events: vec![(0.0, set(2.0, part))],
        ..Pass::default()
    };
    engine.pass("echoed", &pass).unwrap();
    let (two_beats, _) = engine.render(5.0).unwrap();
    assert_eq!(onsets(&two_beats)[..3], [96_000, 144_000, 192_000]);

    let mut engine = echoed();
    engine.play(&[(0.0, set(0.25, part))]).unwrap();
    engine.set_tempo(4.0, Tempo::Hold(60.0)).unwrap();
    let (fixed, _) = engine.render(5.0).unwrap();
    assert_eq!(onsets(&fixed)[..3], [96_000, 108_000, 120_000]);
}

#[test]
fn a_pass_with_a_mistake_or_for_an_unknown_part_plays_nothing() {
    let mut engine = engine();
    let events = vec![
        (0.0, Event::note(None, "click", &[])),
        (1.0, Event::note(None, "nothing", &[])),
    ];
    let pass = Pass {
        events,
        ..Pass::default()
    };
    assert!(matches!(
        engine.pass("lead", &pass),
        Err(Error::UnknownSynth(_))
    ));
    let nowhere = engine.pass("nowhere", &clicks(&[0.0])).unwrap_err();
    assert_eq!(nowhere.to_string(), "no part named nowhere");
    let (left, _) = engine.render(1.0).unwrap();
    assert!(left.iter().all(|sample| *sample == 0.0));
}
