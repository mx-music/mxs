# Changelog for mxs v0.6

## Unreleased

### 1. Enhancements

  * [synths] `pandeiro`'s fingertip stroke, `pandeiro_jingles`, is the heel's stroke played lighter, with a little of the hand and the head in it: its jingles answer 13 ms after the fingers land and are 7 dB down after 40 ms and 22 dB down after 80 ms, as the light strokes of jacksonjust's "Loop Pandeiro_127" on Freesound are, and the stroke is 6 dB quieter

## v0.6.6 (2026-11-23)

### 1. Enhancements

  * [synths] `pandeiro`'s `muted` on the thumb, the heel and the fingertips follows the recordings' strokes on a pressed head: the head keeps its pitches, its lowest tone thumps and dies within 50 ms, and it sounds at its second tone, 1.7 times the note, where before every tone rose a whole tone and rang a fifth as long; the thumb's jingles are half as loud under it
  * [synths] No two strokes of `pandeiro` sound alike: each draws its own noise, how loud its jingles are, how soon they answer and how loud each of the head's tones is, and every stroke is 0.6 dB quieter to leave the loudest of them room

## v0.6.5 (2026-11-23)

### 1. Enhancements

  * [synths] `ganza`'s up stroke is 10 dB quieter between 4 and 6.3 kHz, as the small shaker of the recordings is; its down stroke and `shaker` have the samples they had
  * [synths] `clave` is 5 dB louder at its overtone 2.72 times its note, between 4 and 6.3 kHz, as the recorded claves are
  * [synths] `tabla_tas`, and `tabla` playing tas, has its burst of noise at the note 7 dB quieter, so its lowest band starts and fades as the recorded bol does
  * [synths] `guiro` is 12 dB louder at its overtone three times its note, between 4 and 6.3 kHz, as the recorded güiros are
  * [synths] `berimbau` has its wire 7 to 10 dB louder above 6.3 kHz as the stick strikes, fading there as before, and 3 dB louder near 2 kHz, as the recorded strokes are
  * [synths] `cuica` settles at its note: a stroke played alone settled 38 cents over it at the note it comes with and 182 cents over at 79, and the strokes still differ from one to the next as the recorded ones do
  * [synths] `triangle` is 4 dB louder between 1.6 and 4 kHz, and a hard stroke a further 4 to 6 dB louder between 1 and 4 kHz, as the recorded triangles are
  * [synths] The pandeiro's thumb, heel and slap are refitted to strokes of MeanRaccoon's "pandeiro A collection" sorted by ear: the jingles answer the hand a moment after it lands, 33 ms after a thumb and 13 ms after a heel or a slap, the slap rings the head's own modes under the hand, and its noise sits near 460 Hz
  * [synths] The pandeiro's head is struck by a plain impulse, so its tones keep the same balance at every note, where a burst of noise in the strike gave each note its own

## v0.6.4 (2026-11-23)

### 1. Bug fixes

  * [effects] A sweep stays on the beat for as long as it runs: `slicer`, `tremolo`, `wobble`, `flanger` and `phaser`, and a graph's `sweep`, fell 4 ms behind for every minute they ran at a cycle of half a second, and a slicer's pulse now starts within a frame of its beat after a minute; a render with a sweep has other samples than before

## v0.6.3 (2026-11-23)

### 1. Enhancements

  * [effects] Add `width`: `side`, from 0, mono, through 1, the sound as it came, to 2, twice the difference between its sides, and `mono_below`, a MIDI note below which everything is put in the centre, off at 0
  * [effects] Add `wah`, a resonant low-pass whose cutoff follows the level of the louder side coming in: at the note `cutoff_min` with silence, at `cutoff_max` with a sound at full scale, and between them by the level in dB over the 40 dB under full scale, raised or lowered by `sensitivity`; it opens over `clamp_ms` and closes over `relax_ms`, and rings at its cutoff by `res`
  * [effects] `tremolo` and `slicer` take `spread`, from 0 to 1, the share of half a cycle the right side's cycle runs after the left's, so at 1 the sound moves from side to side; at 0 the samples are as they were

## v0.6.2 (2026-11-23)

### 1. Enhancements

  * [effects] `distortion` works its curve out at four times the engine's rate and filters what it adds above half the engine's rate, so a high, bright sound driven hard keeps its energy on its harmonics, 50 dB under the sound on a saw at C7 where it was 20 dB under; a low sound has the harmonics it had, and no sample is the same
  * [effects] `distortion` takes `curve`, `soft`, as it was, or `hard`, which cuts the sound off flat, and `tone`, a MIDI note for a low-pass after it, off at 0
  * [effects] Add `EffectDecl::late`, the frames after its input an effect's sound comes out, by which the engine holds back the dry sound it mixes it with; `distortion` is 62 frames late
  * [graph] Add `reed`, a reed or the lips blown on a bore and sounding through its bell, with the options mxs's reed and brass winds give theirs, `sway`, the slow random drift of a wind player's pitch and level, and `fingers`, a note as a wind player's fingers hold it through a fast glide; `wind_bands` takes 33 gains, eleven at each of pp, mf and ff, for all of mx's bands
  * [graph] Add `figures`, a note played from one of mxs's tables as its voice plays it: struck, as `kept`, `fitted`, `evened` or `following`, held, as `held` or `blown`, or as the `piano`; a synth naming a table mxs has not is refused when it registers, and a synth's tables are the ones its graph names
  * [synths] `load_table` keeps a table mxs does not carry, such as one fitted with mx, under its file's name, for a graph's `figures` to play; loaded again, it is replaced

## v0.6.1 (2026-11-23)

### 1. Enhancements

  * [effects] `echo` and `ping_pong` take `high_cut` and `low_cut`, MIDI notes above and below which each echo is cut once more than the one before, and `drive`, from 0 to 1, which rounds each echo off once more than the one before, as a tape echo's repeats grow duller; with none given the samples are as they were
  * [effects] `phaser` given no `mix` is half dry, where its notches are deepest; it passed only the turned sound, with no notch
  * [engine] An effect that lists `mix`, `pre_amp` or `amp` among its own options gives it its own default, in place of the one every effect has

## v0.6.0 (2026-11-22)

### 1. Breaking changes

  * [effects] `reverb` is a new reverb: a room of `size` metres whose sound fades 60 dB in `decay_seconds`, starts after `predelay_ms`, is cut below the note `low_cut` and dulled by `damp`; a sound's room is louder on the sound's own side, and `decay_seconds`, `low_cut` and `damp` change while it plays. The reverb it replaces and its `room` option are gone
  * [effects] `compressor` is set as a compressor usually is: `threshold` in dB, -18 unless given, `ratio`, 4 unless given, and `makeup` in dB after it, in place of a threshold as a plain level, `slope_above` and `slope_below`; it follows the louder side and turns both down alike, and `relax_ms` is 150 unless given, so it no longer bends each cycle of a bass note

### 2. Enhancements

  * [effects] Add `phaser`, which sweeps notches through the sound between the notes `low` and `high` once every `phase`: `stages` all-passes, 2, 4, 6 or 8, cut one notch for every two when mixed half and half with the dry sound, so it is used with `mix: 0.5`; `feedback` raises the sound between the notches, and `stereo_invert_wave: 1` sweeps the left side the other way
  * [engine] Add `Engine::send`, which sends a share of a chain's sound into another chain at a level that `Target::Send` changes while it plays, `Engine::route`, which sends all of it there in place of the main output, and `Engine::key`, which has a chain's compressor follow another chain's level; the sound arrives at the same sample, a chain that would feed itself is refused, and a chain that others feed plays until they have ended
  * [effects] An effect reads the chain that keys its own with `Options::key`
  * [engine] Add `Engine::start_main_effects`, the effects everything the engine plays goes through after each part's own effects, gain, mute and solo; a change reaches them by the chain it returns
  * [effects] Add `limiter`, which turns both sides down alike just before a peak on either would pass `ceiling`, in dB below full scale, and lets them back up over `relax_ms`; no sample passes the ceiling, and its sound comes out 5 ms late
  * [graph] `lowpass` and `highpass` given `poles: 1` are filters of one pole, falling 6 dB an octave, as mxs's snare, toms, congas and cuíca use; one given `q` beside it is refused when registered
  * [graph] Add `mod`, the remainder of its first argument after its second, from 0 up to the second for any first, so an envelope can start over at a period
  * [graph] `time` takes `every`, the seconds after which it starts over, worked out as a Rust voice does, so it starts over on the same sample, where `mod` of `time` can start over one sample early
  * [graph] Add `clicks`, clicks at random times at a `rate` a second, and `struck`, which rises over `attack` and falls over `fall` at each click of its sound, as mxs's shakers, cabasa and guiro use
  * [graph] `mode` given a sound in `hz`, or in its list, rings at the frequency the sound gives at each sample, gliding as mxs's sitar does
  * [graph] `impulse` takes `at`, `period` and `until`: one at `at` seconds and again every `period` before `until`, as the tanpura plucks its strings
  * [graph] Add `draw`, the note's numbers from 0 up to 1 from a seed that mixes in how many notes of the synth came before, and `noise` takes `drawn`, seeded from the same, as mxs's caxixi and cuíca draw theirs; a synth whose draws take different mixes is refused when registered
  * [graph] Add `fed_back`, a delay fed back into itself through a one-pole low-pass and high-pass inside its loop, letting its input in for the first `let_in` seconds only, as mxs's pluck plays its string and an echo its repeats

## v0.5.0 (2026-11-22)

### 1. Enhancements

  * [effects] Add `eq`, an equaliser of three bands: a low shelf, a peak and a high shelf, each at a MIDI note, `low_at`, `mid_at` and `high_at`, and raised or lowered by `low_gain`, `mid_gain` and `high_gain` in dB, with `mid_q` for how narrow the peak is
  * [graph] `mode` given lists for `hz`, `level` and `fade` sets a resonator ringing for each, all struck by the one sound, as mxs's drums strike their modes, and lists of different lengths are refused when registered; `noise` takes a `seed`, which fixes its numbers instead of drawing a seed from the note
  * [graph] Add `share`, which builds a sound once for a graph that uses it several times as its `input`, so a noise in it plays the same numbers at each use, as a clone of it does in Rust

### 2. Bug fixes

  * [graph] An effect whose graph takes its input in a sound that is multiplied by 0, or in the side a `select` leaves out when its condition is known as the effect starts, is built and plays the rest, where it panicked
  * [effects] A change reaches the options that were read once as an effect started: `phase_offset` of `slicer`, `tremolo`, `flanger` and `wobble`, `smooth_ms` of `slicer`, `invert_flange` and `stereo_invert_wave` of `flanger`, `filter` of `wobble`, and `pan_start` of `ping_pong`; a switch goes over in 10 ms, so it does not click
  * [engine] An effect that cannot be built when its chain starts is the error `CannotBuild`, naming the effect and why, where `start_effects` and `start_part` panicked
  * [synths] `berimbau`'s coin rattle, with `buzz` above 0, starts its pulse at phase 0, where it started at a phase of its own, so it plays the same as the berimbau written in the graph

## v0.4.1 (2026-11-22)

### 1. Enhancements

  * [graph] An effect's graph that is `sides(left, right)` is built over both sides, each from `left()` and `right()`, so one side can reach the other, and one wrapped in `tail` says how long it sounds on, in seconds or as long as an option says
  * [graph] Add `sweep`, a level from 0 to 1 with the five waves of the effects' own sweeps, `follower`, the level of a sound, `allpass` and `lag`; each is refused when registered if given an argument by name it does not take

### 2. Bug fixes

  * [graph] The `saw`, `square`, `triangle` and `pulse` nodes start at phase 0, as they were meant to, where each started at a phase of its own, so a synth written in the graph plays a saw or a pulse the same as mxs's own `saw` and `pulse` do
  * [synths] `square`, `tri` and `fm` start their waves, and `brass` its vibrato, at phase 0, as `saw` and `pulse` do, where each started at a phase of its own, so each plays the same as the same synth written in the graph

## v0.4.0 (2026-11-21)

### 1. Breaking changes

  * [effects] An effect's time that is not in beats names its unit: slicer's `smooth` is `smooth_ms`, flanger's `delay`, `depth` and `max_delay` are `delay_ms`, `depth_ms` and `max_delay_ms`, and compressor's `clamp_time` and `relax_time` are `clamp_ms` and `relax_ms`, all in milliseconds
  * [engine] `Kind` has an `Ms` variant, a time in milliseconds, made with `Spec::ms`, so code that matches every variant adds it
  * [synths] `violin` is a bowed string again, modelled after the University of Iowa's arco violin from G3 to C6 with its bow noise, the scratch of each bow stroke and a slow drift of pitch and tone; its measured table `violin.mxh` is gone, so fourteen synths have tables, and the engine refuses to keep violin voices ready
  * [synths] `violin` takes the bowed strings' options, `bow_speed`, `bow_force`, `body_hz`, `rosin`, `hiss`, `bow_position` and the bands' gains, and reads `pressure` as a note starts rather than while it sounds

## v0.3.1 (2026-11-21)

### 1. Bug fixes

  * [engine] A part muted, or left out of a solo, before it has made a sound is silent from its first frame, where its first 10 ms were heard while the mute glided in

## v0.3.0 (2026-11-20)

### 1. Breaking changes

  * [engine] `Error` has a `TablesNotLoaded` variant, so code that matches every variant adds it
  * [synths] `synths::families` gives each family as a `Family`, and sorts the synths into the seven families mx Studio's Sounds panel shows: keys, plucked strings, bowed strings, winds, mallets, synths and drums

### 2. Enhancements

  * [synths] The `tables` feature, on by default, compiles in the measured tables of the fifteen synths fitted to recordings; built without it, a program loads a table with `synths::load_table` from its bytes, `Synth::tables` names those a synth plays, and a note of a synth whose tables are not loaded is refused with `Error::TablesNotLoaded`
  * [synths] `Synth::family` says which family a synth belongs to, and `Synth::range` the lowest and highest notes it is meant to play, none for a synth with no range of its own; the synths' catalog says both
  * [graph] `Synth::from_json` and `Effect::from_json` read a synth or an effect from a JSON file, and `from_json_file` from its path, refusing a mistake with the path to it in the file

## v0.2.1 (2026-11-19)

### 1. Enhancements

  * [synths] Add `DEFAULT_VEL`, the vel a note plays at when it gives none and the vel each synth's levels and brightness are fitted at

### 2. Bug fixes

  * [engine] On macOS, an engine counts the performance cores by asking the kernel, not a `sysctl` process, whose wait could hang the first engine made inside the BEAM

## v0.2.0 (2026-11-19)

### 1. Breaking changes

  * [engine] `Event::Note` has a `held` field; `Event::note` builds it as before, and code that writes the struct out gives `held: false`
  * [synths] The winds' bore option `cutoff` is now `least_corner`, so that `cutoff` always names a low-pass's corner
  * [synths] The flute's and tenor sax's `vibrato` is in semitones: the tenor sax's old 1 is 0.145, and the flute's old depth is the new one times 3.78, so its old default of 0.17 is 0.045

### 2. Enhancements

  * [engine] `Event::held` starts a note that sounds until an `Event::Release` lets it go, from a voice kept ready when there is one; a note that rings by itself rings on when it is let go
  * [synths] Every synth takes `bend`, `brightness` and `pressure` while a note sounds: nearly all follow bend, those under a low-pass or with a spectral tilt follow brightness, and pressure plays the winds, violin and cello harder, from `vel` towards 1
  * [synths] Every note takes `vibrato` and `pedal`, the flute's and tenor sax's vibrato in semitones
  * [synths] The fifteen synths without `vel` take one, which scales their level
  * [engine] `Engine::play_midi` plays a MIDI input on a part, each key held until it comes up and started from a voice kept ready, with the sustain pedal and the pitch bend wheel; `change_midi`, `stop_midi` and `midi_inputs` go with it, and `Topic::Keys` reports each key and an input that goes away
  * [engine] `Engine::input_specs` says which options the sound card's input takes
  * [engine] `Engine::effect_tails` says how many seconds each effect sounds on after its input stops
  * [graph] Add `saturate`, `clip`, `fold` and `ladder`, and let a filter's q move; an option that takes names can be compared with a name, and is refused anywhere else
  * [engine] `Engine::analyse` reports the loudness, pitch and onsets of an input, a sample, a chain or the output under `Topic::Analysis`, until `stop_analysis`
  * [engine] `Engine::create_buffer` and `record_buffer` record into a buffer while playing, `save_sample` writes a sample or buffer to a WAV file, and `free_sample` lets one go
  * [engine] `Event::Grains` plays a sample or a buffer as overlapping grains, with the options `grains_specs` lists
  * [engine] A sample takes `stretch` and `pitch`, which change its length and its pitch apart, and the `pitch_shift` effect shifts a chain's pitch
  * [engine] Voices are filled on worker threads, one per performance core, once eight or more sound at once, with the same output sample for sample; `Engine::set_workers` changes their number
  * [engine] An offline engine renders in WebAssembly, which has no clock, with the same samples as natively

### 3. Bug fixes

  * [synths] A fitted note leaves out the noise band that crosses the knock's 600 Hz when its centre is under it

# Changelog for mxs v0.1

## v0.1.6 (2026-11-19)

### 1. Enhancements

  * [engine] `Engine::keep_ready` keeps voices of an instrument fitted to recordings built ahead, so a note that gives only `note` and `vel` starts in the next buffer, allocating nothing on the audio thread, with the same samples as a voice built for it; each voice taken is replaced on the builder thread, and `Engine::kept_ready` says how many are kept
  * [engine] `Error::CannotKeepReady` refuses to keep voices of a synth that builds each note for its note and vel alone
  * [engine] A live engine started on the default output follows the system's default when it changes: it sends a `Topic::Output` report naming the new device, lines its clock up with the system's again within half a second, and warns when the new device runs at another rate

## v0.1.5 (2026-11-18)

### 1. Enhancements

  * [synths] A fitted synth plays each noise band as loud at any sample rate as at 48 kHz, the rate its figures' noise gains are written for; figures fitted to recordings at another rate were up to 4.6 dB too quiet in a band, and NSynth's, at 16 kHz, gain from a refit
  * [synths] Refit `electric_bass` with its noise gains written for 48 kHz, so its strike plays the noise of its recordings
  * [synths] A fitted synth works out each noise band's gain and warmed filters once for each band and rate, shared by every instrument, so a note builds in about 0.1 ms instead of 3 to 5 ms, with the same samples

## v0.1.4 (2026-11-18)

### 1. Enhancements

  * [docs] Add guides, in `docs/guide` and in the crate's documentation as the `guide` module, each with diagrams and programs that run as tests: how the engine is built, events and the life of a note, effects and parts, time, reports, playing live, a synth or an effect of your own, and samples, input and recording
  * [docs] `Engine` lists its calls by what they are for, the `graph` module lists the calls a graph is made of, and the items the documentation names are links
  * [examples] Add `phrase`, `parts`, `own_synth`, `sample` and `beats`, each of which plays live or renders into a WAV file
  * [synths] Add `flute_figured`, a flute played from figures fitted partial by partial to the University of Iowa's flute, with slurs, tongued notes and breaths; it is temporary, to compare with `flute`

## v0.1.3 (2026-11-18)

### 1. Enhancements

  * [engine] Add musical time: the engine keeps a tempo map, placed by `Engine::place_beats` and changed by `set_tempo`, and `Engine::pass` hands a part events at their beats, with their options in beats; a tempo change moves every event more than 150 ms from sounding, `cut` drops a part's events from a beat on, `start_part_in_beats` starts a part whose effect options follow the tempo, and `Topic::Beats` and `Topic::Passes` report the beat, a pass that starts and a pass about to end
  * [engine] Add `Engine::fill` and `play_at_frames`, which fill one block after another from events given at frames
  * [engine] Voices are added up in the order they were handed over, so a render holds the same samples whatever the length of each stretch it is asked for; a render differs from an earlier version's by up to 6e-8 of full scale
  * [engine] Add parts: `Engine::start_part` names a group of sounds with its effects, and `set_gain`, `mute` and `solo` turn it down, silence it or leave it alone heard from a time on, after its effects; a levels report gives each part's level by its name
  * [engine] Add `Engine::subscribe`, which sends reports without being asked: the clock against the system's, the levels, wave and spectrum of the output twenty times a second, how many notes sound, the output device, and the warnings; a live engine works them out on a thread of its own, and an offline one as it renders
  * [engine] A live engine builds voices on a thread of its own, so `play` returns once its events are checked; a synth whose sound has neither 1 nor 2 channels is then left out and named in `warnings`, where an offline engine still refuses the batch
  * [engine] Add `Engine::load`, the time filling audio took as a share of the time it lasts and how many buffers reached the sound card late, and `Engine::behind`, how far the engine's clock has fallen behind the system's; `warnings` also says how many buffers were late
  * [synths] Refit `electric_piano` partial by partial to each of keyboard_electronic_098's 99 recordings, with the tine's bell, the strike's noise and every partial up to 8 kHz; a note or velocity not recorded takes the recordings nearest it, across velocity first; it is no longer an alpha version
  * [synths] Refit `electric_guitar` the same way to guitar_electronic_028's 97 recordings, as loud at A2 as before; it is no longer an alpha version
  * [synths] Refit `marimba` partial by partial to the University of Iowa's 183 yarn recordings at pp, mf and ff, its loudness by velocity following theirs, as loud at C4 at vel 1 as before
  * [synths] Refit `electric_bass` and `deep_bass` partial by partial to their NSynth recordings from E1 to D#4, as loud at A1 as before and growing as much louder with vel
  * [synths] Refit `harp` partial by partial to the folk harp's 57 recordings, each string as loud as the next at vel 1 since the recordings' strings stand up to 24 dB apart, and its velocities as far apart as the recordings', as loud at C4 at vel 1 as before
  * [synths] Refit `guitar` partial by partial to the University of Iowa's guitar at pp, mf and ff, each pitch from the string a player uses in first position, its loudness following the recordings, as loud at E3 at vel 1 as before
  * [synths] Refit `piano` partial by partial to the Steinway B's 126 recordings without the pedal and 126 with it, `pedal` blending the two; at their defaults `aftersound`, `knock` and `uneven` play the recordings as they are, `aftersound` bends each partial's fade toward one rate or deeper, `knock` scales the strike's noise under 600 Hz, and `uneven` varies only the 46 keys not recorded; C4 is as loud as before at each vel, with the pedal and without; every key at a vel carries the same energy, since the recordings are brought to the same peak, the keys above C4 stand under it at vel 0.6 as before, within 4 dB, and the recordings of E6 and of a few others from D6 to D7, whose tone the fit took for noise, are left out
  * [synths] Refit `vibraphone`, `xylophone`, `glockenspiel` and `timpani` partial by partial to the recordings they were fitted to, their loudness following the recordings; `motor` pulses the fitted note
  * [synths] Refit `cello`'s bowed notes partial by partial to the University of Iowa's 120 arco recordings at pp, mf and ff, from held figures that follow each partial until the bow lets go and hold its last level for a longer note; a note follows `note` as it slides, `legato` speaks at once from where the recordings settle, `pizz` plucks as before, and since the recordings have no vibrato, `vibrato` is still 0.25 semitone by default; as loud at C3 at vel 1 as before, and vel 0.3 stands 16 dB under vel 1, as the recordings' pp under their ff; `bow_speed`, `bow_force`, `bow_position`, `body_hz`, `rosin`, `hiss` and the `eq` bands are gone from it
  * [synths] Refit `violin`'s bowed notes the same way to the University of Iowa's 128 arco recordings from G3 to B7, each pitch from the highest string it can be played on; as loud at vel 1 as before on average from G3 to C7, its high notes up to 9 dB quieter and G3 2.9 dB louder, as the recordings stand; the same options are gone from it

## v0.1.2 (2026-11-17)

### 1. Enhancements

  * [synths] Add `deep_bass`, a dark bass played with the fingers, fitted to NSynth's bass_electronic_025 and bass_electronic_027
  * [synths] Add `electric_piano`, an alpha version of an electric piano whose hammers strike tines, fitted to NSynth's keyboard_electronic_098; its hard high notes are duller than the recordings, and it will be refitted
  * [synths] Add `electric_guitar`, an alpha version of a solid-body electric guitar picked clean, fitted to NSynth's guitar_electronic_028; it will be refitted

## v0.1.1 (2026-11-16)

### 1. Enhancements

  * [synths] Add `electric_bass`, an electric bass played with the fingers, fitted to NSynth's bass_electronic_018
  * [synths] Add `families`, which groups the synths of `all` by family
  * [synths] Add `piano_parts`, which gives the harmonics, resonators, thump and knock of a piano note without playing it
  * [Synth] Add `doc`, which says what a synth is and the recordings it is fitted to

## v0.1.0 (2026-11-14)

Initial release.
