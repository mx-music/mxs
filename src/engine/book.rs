//! The book of what plays: chains, parts and sends, what is due and when, and the voices kept ready.

use super::*;

/// The options a note may be given and still take a voice kept ready, besides a pedal of 0 or 1.
const READY_OPTIONS: [&str; 5] = ["note", "vel", "bend", "brightness", "pressure"];

impl Book {
    pub(super) fn check_chains(&self, chains: &[u64]) -> Result<(), Error> {
        match chains
            .iter()
            .find(|chain| !self.chains.contains_key(chain) || self.ended.contains(chain))
        {
            Some(chain) => Err(Error::UnknownChain(*chain)),
            None => Ok(()),
        }
    }

    /// The chains `chain` plays into or keys, each of which has to play after it.
    fn fed_by(&self, chain: u64) -> Vec<u64> {
        let sends = self.sends.keys().filter(|(from, _)| *from == chain);
        let mut fed: Vec<u64> = sends.map(|(_, to)| *to).collect();
        fed.extend(self.routes.get(&chain));
        let keyed = self.keys.iter().filter(|(_, by)| **by == chain);
        fed.extend(keyed.map(|(keyed, _)| *keyed));
        fed.sort_unstable();
        fed.dedup();
        fed
    }

    /// The chains on the way from `from` to `to` through what each feeds, both included, if there is one.
    fn way(&self, from: u64, to: u64, seen: &mut Vec<u64>) -> Option<Vec<u64>> {
        if from == to {
            return Some(vec![to]);
        }
        if seen.contains(&from) {
            return None;
        }
        seen.push(from);
        self.fed_by(from).into_iter().find_map(|next| {
            let mut way = self.way(next, to, seen)?;
            way.insert(0, from);
            Some(way)
        })
    }

    /// Makes room for `from` to feed or key `to`: refuses a loop, naming its chains, and has `to`, and what it feeds, play after `from`.
    pub(super) fn feed(&mut self, from: u64, to: u64) -> Result<(), Error> {
        if let Some(way) = self.way(to, from, &mut Vec::new()) {
            let name = |chain: &u64| {
                let part = self.parts.iter().find(|(_, key)| *key == chain);
                part.map_or_else(|| chain.to_string(), |(name, _)| name.clone())
            };
            let mut chains = vec![name(&from)];
            chains.extend(way.iter().filter(|chain| **chain != from).map(name));
            return Err(Error::FeedsItself(chains));
        }
        let mut raise = vec![(to, self.ranks.get(&from).copied().unwrap_or(0) + 1)];
        while let Some((chain, least)) = raise.pop() {
            if self.ranks.get(&chain).copied().unwrap_or(0) < least {
                self.ranks.insert(chain, least);
                self.send(Command::Rank {
                    key: chain,
                    rank: least,
                })?;
                raise.extend(self.fed_by(chain).into_iter().map(|next| (next, least + 1)));
            }
        }
        Ok(())
    }

    /// Adds voices built ready to those kept for their synths.
    pub(super) fn keep(&mut self, kept: Vec<(Arc<str>, Built)>) {
        for (synth, built) in kept {
            self.ready.entry(synth.to_string()).or_default().push(built);
        }
    }

    /// A voice kept ready for the note `sound` drafts, given its note and vel and placed as the draft is, when `event` gives no other option and one is kept.
    pub(super) fn take_ready(&mut self, event: &Event, sound: &SoundDraft) -> Option<Built> {
        let Event::Note {
            synth,
            options,
            held,
            ..
        } = event
        else {
            return None;
        };
        let takes = |(name, value): &(String, Value)| match name.as_str() {
            "pedal" => matches!(value, Value::Number(x) if *x == 0.0 || *x == 1.0),
            name => READY_OPTIONS.contains(&name),
        };
        if !options.iter().all(takes) {
            return None;
        }
        let mut built = self.ready.get_mut(synth)?.pop()?;
        let hold = built.ready.as_ref()?;
        let given = Given {
            note: sound.options.number("note"),
            vel: sound.options.number("vel"),
            held: *held,
            pedal: if options.iter().any(|(name, _)| name == "pedal") {
                sound.options.number("pedal")
            } else {
                0.0
            },
            pressure: sound.options.number("pressure"),
        };
        let (lasts, release) = hold.give(given);
        built.lasts = lasts;
        if let Some((_, seconds)) = &mut built.release {
            *seconds = release;
        }
        if let Some(flag) = &built.held {
            flag.set(if *held { 1.0 } else { 0.0 });
        }
        // A note that follows these reads them from its first sample.
        for name in READY_OPTIONS {
            built.controls.set(name, sound.options.number(name) as f32);
        }
        built.id = sound.id;
        built.at = sound.at;
        built.beat = sound.beat;
        built.stamp = sound.stamp;
        built.out = sound.out;
        Some(built)
    }

    fn frames(&self, seconds: f64) -> u64 {
        (seconds.max(0.0) * self.rate).round() as u64
    }

    pub(super) fn send(&mut self, mut command: Command) -> Result<(), Error> {
        for _ in 0..1000 {
            match self.commands.push(command) {
                Ok(()) => return Ok(()),
                Err(rtrb::PushError::Full(back)) => command = back,
            }
            self.drain();
            if !self.live {
                return Err(Error::Busy);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(Error::Busy)
    }

    pub(super) fn drain(&mut self) {
        while let Ok(garbage) = self.garbage.pop() {
            if let Garbage::Chain(chain) = &garbage {
                let key = chain.key;
                self.ended.insert(key);
                self.sends
                    .retain(|(from, to), _| *from != key && *to != key);
                self.routes.retain(|from, to| *from != key && *to != key);
                self.keys.retain(|chain, by| *chain != key && *by != key);
                self.ranks.remove(&key);
                self.hub.forget_part(key);
                self.followers.retain(|follower| follower.0 != key);
                self.score.retain(|scored| scored.part != key);
                self.in_beats.remove(&key);
                self.endings.remove(&key);
                self.labels.remove(&key);
            }
        }
    }

    /// Forgets sounds that ended over a second ago, and frees what the mixer has finished with.
    pub(super) fn sweep(&mut self) {
        self.drain();
        let now = self.clock.load(Ordering::Acquire) as f64 / self.rate;
        self.sounds.retain(|_, sound| sound.end + 1.0 > now);
        let sounds = &self.sounds;
        self.ids.retain(|_, key| sounds.contains_key(key));
    }

    /// The sound's voice, starting at its time or now if that has passed, and what the engine keeps of it.
    pub(super) fn start(&mut self, built: Built) -> (Box<Voice>, Sound) {
        let key = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let now = self.clock.load(Ordering::Acquire);
        let start = Ord::max(self.frames(built.at), now);
        let late = now.saturating_sub(self.frames(built.at));
        let end = start.saturating_add(Ord::max(self.frames(built.lasts), 1));
        let fade = Ord::max(self.frames(END_FADE), 1);
        let voice = Voice {
            key,
            out: built.out,
            input: built.input,
            unit: Box::new(built.unit),
            start,
            late,
            end,
            fade,
            level: 0.0,
            stolen: false,
            slot: None,
        };
        let chain = match built.out {
            Out::Chain(key) => Some(key),
            _ => None,
        };
        let sound = Sound {
            key,
            chain,
            beat: built.beat,
            start: built.at,
            end: built.at + built.lasts,
            controls: built.controls,
            release: built.release,
        };
        (Box::new(voice), sound)
    }

    pub(super) fn now(&self) -> f64 {
        self.clock.load(Ordering::Acquire) as f64 / self.rate
    }

    /// Whether anything waits for its time: an event in beats, a report of a pass, or a beat at which the tempo is another.
    pub(super) fn pending(&self) -> bool {
        !self.score.is_empty()
            || !self.notices.is_empty()
            || self.endings.values().any(|(_, _, told)| !told)
            || self.map.unread()
    }

    /// Takes what falls before `horizon` seconds out of what waits: the events in beats, placed at their seconds with their options in beats turned into seconds, and for each beat at which the tempo is another, a change to the effect options that follow it.
    /// It sends the reports of those beats, and holds those of the passes that start and of those about to end.
    pub(super) fn due(&mut self, horizon: f64) -> Vec<Draft> {
        let now = self.now();
        let playing = self.map.beat_at(now);
        let until = self.map.beat_at(horizon);
        let count = self.score.partition_point(|scored| scored.beat < until);
        let taken: Vec<Scored> = self.score.drain(..count).collect();
        let mut due = Vec::with_capacity(taken.len());
        let mut dropped = 0;
        for scored in taken {
            // A release or a stop is kept however late, so that no note is left ringing.
            let ends = matches!(
                scored.draft,
                Draft::Change(_, Event::Stop { .. } | Event::Release { .. })
            );
            if playing - scored.beat >= LATE && !ends {
                dropped += 1;
                continue;
            }
            let (beat, part) = (scored.beat, scored.part);
            let at = self.map.seconds_at(beat);
            let per_beat = 60.0 / self.map.bpm_at(beat);
            if let (Some(label), Some(part)) = (scored.starts, self.hub.part_name(part)) {
                let seconds = at.max(now);
                let started = Report::PassStarted {
                    seconds,
                    part,
                    label,
                    beat,
                };
                self.notices.push((seconds, started));
            }
            due.push(match scored.draft {
                Draft::Sound(mut sound) => {
                    sound.at = at;
                    sound.beat = Some(beat);
                    sound.stamp = self.cuts.0;
                    sound.options.in_beats(per_beat);
                    Draft::Sound(sound)
                }
                Draft::Change(_, Event::Set { target, options }) => {
                    let options = self.in_seconds(&target, options, per_beat);
                    Draft::Change(at, Event::Set { target, options })
                }
                Draft::Change(_, event) => Draft::Change(at, event),
            });
        }
        self.voices.dropped.fetch_add(dropped, Ordering::Relaxed);
        for (part, (to, notice, told)) in &mut self.endings {
            let seconds = self.map.seconds_at(*to - *notice);
            if !*told && seconds < horizon {
                *told = true;
                if let Some(part) = self.hub.part_name(*part) {
                    let ending = Report::PassEnding {
                        seconds,
                        part,
                        beat: *to,
                    };
                    self.notices.push((seconds, ending));
                }
            }
        }
        // Of the beats already past, only the last still says what the tempo is.
        let mut points = Vec::new();
        while let Some((point, at)) = self.map.read_point(horizon) {
            if at <= now {
                points.clear();
            }
            points.push((point, at));
        }
        for (beat, seconds) in points {
            let bpm = self.map.bpm_at(beat);
            let system = self.timing.as_ref().and_then(|timing| {
                let timing = timing.lock().unwrap_or_else(|e| e.into_inner());
                let heard = timing.heard(seconds)?;
                Some(SystemTime::UNIX_EPOCH + Duration::from_secs_f64(heard.max(0.0)))
            });
            self.hub.keep(Report::Beat {
                seconds,
                beat,
                bpm,
                system,
            });
            for (chain, index, name, beats) in &self.followers {
                let target = Target::Effect(*chain, *index);
                let options = vec![(name.clone(), beats * 60.0 / bpm)];
                let set = Event::Set { target, options };
                due.push(Draft::Change(seconds.max(now), set));
            }
        }
        due
    }

    /// A change's options in seconds, given in beats in a pass: each `_slide`, and an effect's options declared in beats, which follow the tempo from then on.
    fn in_seconds(
        &mut self,
        target: &Target,
        options: Vec<(String, f64)>,
        per_beat: f64,
    ) -> Vec<(String, f64)> {
        let mut seconds = Vec::with_capacity(options.len());
        for (name, value) in options {
            let effect = match target {
                Target::Effect(chain, index) => Some((*chain, *index)),
                _ => None,
            };
            let declared = effect.filter(|(chain, index)| {
                let names = self.in_beats.get(chain).and_then(|names| names.get(*index));
                names.is_some_and(|names| names.contains(&name))
            });
            if let Some((chain, index)) = declared {
                self.followers
                    .retain(|f| (f.0, f.1, f.2.as_str()) != (chain, index, name.as_str()));
                self.followers.push((chain, index, name.clone(), value));
            }
            let timed = declared.is_some() || name.ends_with("_slide");
            seconds.push((name, if timed { value * per_beat } else { value }));
        }
        seconds
    }

    /// Sends the reports of passes whose seconds the clock has reached.
    pub(super) fn tell(&mut self) {
        let now = self.now();
        let mut reached = Vec::new();
        self.notices.retain(|(seconds, report)| {
            let keep = *seconds > now;
            if !keep {
                reached.push(report.clone());
            }
            keep
        });
        reached.sort_by(|a, b| a.seconds().total_cmp(&b.seconds()));
        reached.into_iter().for_each(|report| self.hub.send(report));
    }

    /// Drops what a part was handed for `beat` and after: the events not yet placed, and the sounds placed that have not started.
    pub(super) fn cut(&mut self, part: u64, beat: f64) -> Result<(), Error> {
        self.score
            .retain(|scored| scored.part != part || scored.beat < beat);
        self.endings.remove(&part);
        self.notices.retain(|(_, report)| match report {
            Report::PassStarted { beat: starts, .. } => *starts < beat,
            _ => true,
        });
        let (count, cuts) = &mut self.cuts;
        cuts.push((*count, part, beat));
        *count += 1;
        let kept = cuts.len().saturating_sub(32);
        cuts.drain(..kept);
        let now = self.now();
        let later = |sound: &Sound| {
            sound.chain == Some(part) && sound.start > now && sound.beat.is_some_and(|b| b >= beat)
        };
        let keys: Vec<u64> = self
            .sounds
            .values()
            .filter(|sound| later(sound))
            .map(|sound| sound.key)
            .collect();
        for key in keys {
            self.sounds.remove(&key);
            self.send(Command::Drop { key })?;
        }
        Ok(())
    }

    /// Whether a sound of a pass was cut while it was being built.
    fn was_cut(&self, built: &Built) -> bool {
        let (Some(beat), Out::Chain(part)) = (built.beat, built.out) else {
            return false;
        };
        let cut = |(before, of, from): &(u64, u64, f64)| {
            *before >= built.stamp && *of == part && beat >= *from
        };
        self.cuts.1.iter().any(cut)
    }

    /// Sends a batch to the mixer: its sounds first, so that a change can reach a note of the same batch.
    pub(super) fn commit(&mut self, items: Vec<Item>) -> Result<(), Error> {
        let mut commands = Vec::with_capacity(items.len());
        let mut changes = Vec::new();
        for item in items {
            match item {
                Item::Sound(built) if self.was_cut(&built) => {}
                Item::Sound(built) => {
                    let id = built.id;
                    let (voice, sound) = self.start(*built);
                    if let Some(id) = id {
                        self.ids.insert(id, sound.key);
                    }
                    self.sounds.insert(sound.key, sound);
                    commands.push(Command::Start(voice));
                }
                Item::Change(at, event) => changes.push((at, event)),
            }
        }
        for (at, event) in changes {
            match event {
                Event::Set { target, options } => {
                    for change in self.changes(at, &target, &options) {
                        commands.push(Command::Change(Box::new(change)));
                    }
                }
                Event::Stop { ids } => {
                    for key in ids.iter().filter_map(|id| self.ids.get(id)) {
                        if let Some(sound) = self.sounds.get_mut(key) {
                            sound.end = sound.end.min(at.max(sound.start) + END_FADE);
                        }
                        commands.push(Command::Stop {
                            key: *key,
                            at: self.frames(at),
                        });
                    }
                }
                Event::Release { ids } => {
                    for key in ids
                        .iter()
                        .filter_map(|id| self.ids.get(id))
                        .copied()
                        .collect::<Vec<_>>()
                    {
                        let Some(sound) = self.sounds.get_mut(&key) else {
                            continue;
                        };
                        let at = at.max(sound.start);
                        let (stop, gate) = match &sound.release {
                            Some((gate, seconds)) => (at + seconds, Some(gate.clone())),
                            // A sound with no release of its own rings until it fades, as a drum does once the hand leaves it.
                            None if sound.end.is_finite() => continue,
                            None => (at, None),
                        };
                        sound.end = sound.end.min(stop + END_FADE);
                        if let Some(gate) = gate {
                            let control = Control {
                                target: gate,
                                slide: shared(0.0),
                            };
                            commands.push(Command::Change(Box::new(Change {
                                at: self.frames(at),
                                control,
                                value: Some(0.0),
                                slide: None,
                            })));
                        }
                        commands.push(Command::Stop {
                            key,
                            at: self.frames(stop),
                        });
                    }
                }
                _ => {}
            }
        }
        for command in commands {
            self.send(command)?;
        }
        Ok(())
    }

    fn send_changes(&self, at: f64, from: u64, to: u64, options: &[(String, f64)]) -> Vec<Change> {
        let value = |name: &str| options.iter().find(|(option, _)| option == name);
        let (level, slide) = (value("level"), value("level_slide"));
        let control = self
            .sends
            .get(&(from, to))
            .filter(|_| level.or(slide).is_some());
        let change = control.map(|control| Change {
            at: self.frames(at),
            control: control.clone(),
            value: level.map(|(_, level)| *level as f32),
            slide: slide.map(|(_, slide)| *slide as f32),
        });
        change.into_iter().collect()
    }

    fn changes(&self, at: f64, target: &Target, options: &[(String, f64)]) -> Vec<Change> {
        let controls: Vec<&Controls> = match target {
            Target::Sounds(ids) => ids
                .iter()
                .filter_map(|id| self.ids.get(id))
                .filter_map(|key| self.sounds.get(key))
                .map(|s| &s.controls)
                .collect(),
            Target::Part(chain) => self
                .sounds
                .values()
                .filter(|s| s.chain == Some(*chain) && s.start <= at && at < s.end)
                .map(|s| &s.controls)
                .collect(),
            Target::Effect(chain, index) => self
                .chains
                .get(chain)
                .and_then(|c| c.get(*index))
                .into_iter()
                .collect(),
            Target::Send(from, to) => {
                return self.send_changes(at, *from, *to, options);
            }
        };
        let mut changes = Vec::new();
        let mut by_option: HashMap<&str, (Option<f32>, Option<f32>)> = HashMap::new();
        for (name, value) in options {
            match name.strip_suffix("_slide") {
                Some(base) => by_option.entry(base).or_default().1 = Some(*value as f32),
                None => by_option.entry(name).or_default().0 = Some(*value as f32),
            }
        }
        for controls in controls {
            for (name, (value, slide)) in &by_option {
                if let Some(control) = controls.get(name) {
                    changes.push(Change {
                        at: self.frames(at),
                        control: control.clone(),
                        value: *value,
                        slide: *slide,
                    });
                }
            }
        }
        changes
    }
}
