//! Musical time: a tempo map, which says at which of the engine's seconds each beat falls.

/// The tempo from a beat on, for [`Engine::set_tempo`](crate::Engine::set_tempo).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tempo {
    /// Held at this many beats a minute.
    Hold(f64),
    /// Moving evenly from one tempo to another over a number of beats, then held at the second.
    Move {
        /// The beats a minute it starts at.
        from: f64,
        /// The beats a minute it ends at.
        to: f64,
        /// The beats it takes.
        over: f64,
    },
}

/// How often the tempo is read again while it moves, in beats, by whatever follows it.
const STEP: f64 = 0.25;

impl Tempo {
    fn held(self) -> Tempo {
        let positive = |bpm: f64| if bpm > 0.0 { bpm } else { 120.0 };
        match self {
            Tempo::Hold(bpm) => Tempo::Hold(positive(bpm)),
            Tempo::Move { from, to, over } if over > 0.0 => Tempo::Move {
                from: positive(from),
                to: positive(to),
                over,
            },
            Tempo::Move { to, .. } => Tempo::Hold(positive(to)),
        }
    }

    /// The beats a minute `beats` after it starts.
    fn bpm(self, beats: f64) -> f64 {
        match self {
            Tempo::Hold(bpm) => bpm,
            Tempo::Move { from, to, over } => from + (to - from) * (beats / over).clamp(0.0, 1.0),
        }
    }

    /// The seconds its first `beats` last.
    fn seconds(self, beats: f64) -> f64 {
        match self {
            Tempo::Hold(bpm) => beats * 60.0 / bpm,
            Tempo::Move { from, to, over } => {
                let moving = beats.min(over);
                let moved = if from == to {
                    moving * 60.0 / from
                } else {
                    60.0 * over / (to - from) * (self.bpm(moving) / from).ln()
                };
                moved + (beats - over).max(0.0) * 60.0 / to
            }
        }
    }

    /// The beats its first `seconds` hold.
    fn beats(self, seconds: f64) -> f64 {
        match self {
            Tempo::Hold(bpm) => seconds * bpm / 60.0,
            Tempo::Move { from, to, over } => {
                let moving = self.seconds(over);
                if seconds >= moving {
                    over + (seconds - moving) * to / 60.0
                } else if from == to {
                    seconds * from / 60.0
                } else {
                    let rate = (to - from) / over;
                    from / rate * ((seconds * rate / 60.0).exp() - 1.0)
                }
            }
        }
    }
}

/// The tempo at every beat, and the engine's seconds at which each beat falls.
#[derive(Clone, Debug)]
pub(crate) struct TempoMap {
    /// Each change of tempo, in order of its beat; never empty.
    changes: Vec<Change>,
    /// The last beat at which the tempo was read, for the beat reports and the options that follow it.
    pointed: f64,
}

/// A change of tempo: the beat it starts at, the engine's seconds at which that beat falls, and the tempo from there.
#[derive(Clone, Copy, Debug)]
struct Change {
    beat: f64,
    seconds: f64,
    tempo: Tempo,
}

impl TempoMap {
    pub fn new() -> TempoMap {
        TempoMap {
            changes: vec![Change {
                beat: 0.0,
                seconds: 0.0,
                tempo: Tempo::Hold(120.0),
            }],
            pointed: f64::NEG_INFINITY,
        }
    }

    /// Places beat 0 at `seconds` of the engine's clock, moving the whole map with it, and reads the tempo again from the start.
    pub fn place(&mut self, seconds: f64) {
        self.anchor(self.seconds_at(0.0), seconds);
        self.pointed = f64::NEG_INFINITY;
    }

    /// Changes the tempo from `beat` on, in place of whatever the map held from there; beat 0 stays where it falls.
    pub fn set(&mut self, beat: f64, tempo: Tempo) {
        let zero = self.seconds_at(0.0);
        let seconds = self.seconds_at(beat);
        self.changes.retain(|change| change.beat < beat);
        self.changes.push(Change {
            beat,
            seconds,
            tempo: tempo.held(),
        });
        self.anchor(self.seconds_at(0.0), zero);
        self.pointed = self.pointed.min(beat.next_down());
    }

    /// Moves the map so beat 0, which falls at `from`, falls at `to`.
    fn anchor(&mut self, from: f64, to: f64) {
        if from != to {
            for change in &mut self.changes {
                change.seconds = to + (change.seconds - from);
            }
        }
    }

    /// Whether a point where the tempo changes is still to be read.
    pub fn unread(&self) -> bool {
        self.next_point(self.pointed).is_some()
    }

    /// Reads the next point where the tempo changes, with the engine's seconds it falls at, when that is before `before`.
    pub fn read_point(&mut self, before: f64) -> Option<(f64, f64)> {
        let point = self.next_point(self.pointed)?;
        let at = self.seconds_at(point);
        if at >= before {
            return None;
        }
        self.pointed = point;
        Some((point, at))
    }

    /// The tempo at the engine's `seconds`, or at the last point read when that is later.
    pub fn bpm_read(&self, seconds: f64) -> f64 {
        self.bpm_at(self.pointed.max(self.beat_at(seconds)))
    }

    /// The change that holds at `beat`: the last that starts at or before it, or the first.
    fn at(&self, beat: f64) -> Change {
        let index = self
            .changes
            .iter()
            .rposition(|change| change.beat <= beat)
            .unwrap_or(0);
        self.changes[index]
    }

    /// The engine's seconds at which `beat` falls; a beat before the first change keeps that change's starting tempo.
    pub fn seconds_at(&self, beat: f64) -> f64 {
        let change = self.at(beat);
        if beat < change.beat {
            return change.seconds + (beat - change.beat) * 60.0 / change.tempo.bpm(0.0);
        }
        change.seconds + change.tempo.seconds(beat - change.beat)
    }

    /// The beat that falls at the engine's `seconds`.
    pub fn beat_at(&self, seconds: f64) -> f64 {
        let index = self
            .changes
            .iter()
            .rposition(|change| change.seconds <= seconds)
            .unwrap_or(0);
        let change = self.changes[index];
        let left = seconds - change.seconds;
        if left < 0.0 {
            return change.beat + left * change.tempo.bpm(0.0) / 60.0;
        }
        change.beat + change.tempo.beats(left)
    }

    /// The beats a minute at `beat`.
    pub fn bpm_at(&self, beat: f64) -> f64 {
        let change = self.at(beat);
        change.tempo.bpm((beat - change.beat).max(0.0))
    }

    /// The first beat after `after` at which the tempo is another: where a change starts, and every quarter beat while the tempo moves.
    pub fn next_point(&self, after: f64) -> Option<f64> {
        let index = self
            .changes
            .iter()
            .rposition(|change| change.beat <= after)
            .unwrap_or(0);
        let next = self.changes.get(index + 1).map(|change| change.beat);
        let Change {
            beat: start, tempo, ..
        } = self.changes[index];
        let moving = match tempo {
            Tempo::Move { over, .. } if after >= start && after < start + over => {
                let step = start + (((after - start) / STEP).floor() + 1.0) * STEP;
                Some(step.min(start + over))
            }
            _ => None,
        };
        let first = Some(self.changes[0].beat).filter(|first| after < *first);
        [first, moving, next]
            .into_iter()
            .flatten()
            .min_by(f64::total_cmp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(map: &TempoMap, beat: f64) -> u64 {
        (map.seconds_at(beat) * 48_000.0).round() as u64
    }

    #[test]
    fn at_120_beats_a_minute_each_beat_falls_half_a_second_after_the_last() {
        let map = TempoMap::new();
        let frames: Vec<u64> = (0..8).map(|beat| frame(&map, f64::from(beat))).collect();
        assert_eq!(
            frames,
            [0, 24_000, 48_000, 72_000, 96_000, 120_000, 144_000, 168_000]
        );
        assert_eq!(map.beat_at(1.25), 2.5);
        assert_eq!(map.bpm_at(100.0), 120.0);
    }

    #[test]
    fn a_change_replaces_what_the_map_held_from_its_beat_on() {
        let mut map = TempoMap::new();
        map.set(6.0, Tempo::Hold(240.0));
        map.set(4.0, Tempo::Hold(60.0));
        assert_eq!(frame(&map, 4.0), 96_000);
        assert_eq!(frame(&map, 5.0), 144_000);
        assert_eq!(frame(&map, 7.0), 240_000);
        assert_eq!(map.beat_at(3.5), 5.5);
        assert_eq!((map.bpm_at(3.9), map.bpm_at(4.0)), (120.0, 60.0));
    }

    #[test]
    fn a_tempo_moving_from_120_to_60_over_4_beats_puts_beat_4_at_frame_133084() {
        let mut map = TempoMap::new();
        map.set(
            0.0,
            Tempo::Move {
                from: 120.0,
                to: 60.0,
                over: 4.0,
            },
        );
        assert_eq!(frame(&map, 4.0), 133_084);
        assert_eq!(frame(&map, 5.0), 133_084 + 48_000);
        assert_eq!(map.bpm_at(2.0), 90.0);
        assert_eq!(map.bpm_at(9.0), 60.0);
        for beat in [0.5, 1.0, 2.75, 4.0, 6.5] {
            assert!((map.beat_at(map.seconds_at(beat)) - beat).abs() < 1e-9);
        }
    }

    #[test]
    fn beat_0_falls_where_it_is_placed_and_beats_before_the_first_change_keep_its_tempo() {
        let mut map = TempoMap::new();
        map.place(10.0);
        map.set(8.0, Tempo::Hold(60.0));
        assert_eq!(map.seconds_at(0.0), 10.0);
        assert_eq!(map.seconds_at(-2.0), 9.0);
        assert_eq!(map.seconds_at(8.0), 14.0);
        assert_eq!(map.seconds_at(9.0), 15.0);
        assert_eq!(map.beat_at(9.0), -2.0);
        assert_eq!(map.beat_at(15.0), 9.0);
    }

    #[test]
    fn a_change_made_while_the_tempo_moves_cuts_the_move_short() {
        let mut map = TempoMap::new();
        let moving = Tempo::Move {
            from: 120.0,
            to: 60.0,
            over: 4.0,
        };
        map.set(0.0, moving);
        let two = map.seconds_at(2.0);
        map.set(2.0, Tempo::Hold(120.0));
        assert_eq!(map.seconds_at(2.0), two);
        assert_eq!(map.seconds_at(4.0), two + 1.0);
        assert!((map.beat_at(two + 0.5) - 3.0).abs() < 1e-12);
    }

    #[test]
    fn a_change_before_beat_0_leaves_beat_0_where_it_was_placed() {
        let mut map = TempoMap::new();
        map.place(10.0);
        map.set(-4.0, Tempo::Hold(60.0));
        assert_eq!(map.seconds_at(0.0), 10.0);
        assert_eq!(map.seconds_at(-4.0), 6.0);
        assert_eq!(map.beat_at(8.0), -2.0);
    }

    #[test]
    fn the_tempo_is_another_where_a_change_starts_and_every_quarter_beat_while_it_moves() {
        let mut map = TempoMap::new();
        assert_eq!(map.next_point(0.0), None);
        map.set(
            4.0,
            Tempo::Move {
                from: 120.0,
                to: 60.0,
                over: 0.6,
            },
        );
        map.set(8.0, Tempo::Hold(90.0));
        let mut points = Vec::new();
        let mut after = -1.0;
        while let Some(point) = map.next_point(after) {
            points.push(point);
            after = point;
        }
        assert_eq!(points, [0.0, 4.0, 4.25, 4.5, 4.6, 8.0]);
    }

    #[test]
    fn a_tempo_of_nothing_or_less_is_taken_as_120() {
        let mut map = TempoMap::new();
        map.set(0.0, Tempo::Hold(0.0));
        assert_eq!(map.seconds_at(2.0), 1.0);
    }
}
