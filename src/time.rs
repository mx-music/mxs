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

/// The tempo at every beat, and the engine's seconds at which beat 0 falls.
#[derive(Clone, Debug)]
pub(crate) struct TempoMap {
    origin: f64,
    /// Each change of tempo and the beat it starts at, in order; never empty.
    changes: Vec<(f64, Tempo)>,
}

impl TempoMap {
    pub fn new() -> TempoMap {
        TempoMap {
            origin: 0.0,
            changes: vec![(0.0, Tempo::Hold(120.0))],
        }
    }

    /// Places beat 0 at `seconds` of the engine's clock.
    pub fn place(&mut self, seconds: f64) {
        self.origin = seconds;
    }

    /// Changes the tempo from `beat` on, in place of whatever the map held from there.
    pub fn set(&mut self, beat: f64, tempo: Tempo) {
        self.changes.retain(|change| change.0 < beat);
        self.changes.push((beat, tempo.held()));
    }

    /// The change that holds at `beat`: the last that starts at or before it, or the first.
    fn at(&self, beat: f64) -> usize {
        self.changes
            .iter()
            .rposition(|change| change.0 <= beat)
            .unwrap_or(0)
    }

    /// Seconds from the first change to `beat`, negative for a beat before it.
    fn elapsed(&self, beat: f64) -> f64 {
        let (first, tempo) = self.changes[0];
        if beat < first {
            return (beat - first) * 60.0 / tempo.bpm(0.0);
        }
        let mut seconds = 0.0;
        for (index, (start, tempo)) in self.changes.iter().enumerate() {
            let end = self.changes.get(index + 1).map_or(f64::INFINITY, |c| c.0);
            seconds += tempo.seconds(beat.min(end) - start);
            if beat <= end {
                break;
            }
        }
        seconds
    }

    /// The engine's seconds at which `beat` falls.
    pub fn seconds_at(&self, beat: f64) -> f64 {
        self.origin + self.elapsed(beat) - self.elapsed(0.0)
    }

    /// The beat that falls at the engine's `seconds`.
    pub fn beat_at(&self, seconds: f64) -> f64 {
        let (first, tempo) = self.changes[0];
        let mut left = seconds - self.origin + self.elapsed(0.0);
        if left < 0.0 {
            return first + left * tempo.bpm(0.0) / 60.0;
        }
        for (index, (start, tempo)) in self.changes.iter().enumerate() {
            let Some((end, _)) = self.changes.get(index + 1) else {
                return start + tempo.beats(left);
            };
            let lasts = tempo.seconds(end - start);
            if left < lasts {
                return start + tempo.beats(left);
            }
            left -= lasts;
        }
        first
    }

    /// The beats a minute at `beat`.
    pub fn bpm_at(&self, beat: f64) -> f64 {
        let (start, tempo) = self.changes[self.at(beat)];
        tempo.bpm((beat - start).max(0.0))
    }

    /// The first beat after `after` at which the tempo is another: where a change starts, and every quarter beat while the tempo moves.
    pub fn next_point(&self, after: f64) -> Option<f64> {
        let index = self.at(after);
        let next = self.changes.get(index + 1).map(|change| change.0);
        let (start, tempo) = self.changes[index];
        let moving = match tempo {
            Tempo::Move { over, .. } if after >= start && after < start + over => {
                let step = start + (((after - start) / STEP).floor() + 1.0) * STEP;
                Some(step.min(start + over))
            }
            _ => None,
        };
        let first = Some(self.changes[0].0).filter(|first| after < *first);
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
