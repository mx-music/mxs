# Changelog for mxs v0.1

## Unreleased

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
