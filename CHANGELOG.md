# Changelog for mxs v0.1

## Unreleased

### 1. Enhancements

  * [synths] Refit `electric_piano` partial by partial to each of keyboard_electronic_098's 99 recordings, with the tine's bell, the strike's noise and every partial up to 8 kHz; a note or velocity not recorded takes the recordings nearest it, across velocity first; it is no longer an alpha version
  * [synths] Refit `electric_guitar` the same way to guitar_electronic_028's 97 recordings, as loud at A2 as before; it is no longer an alpha version
  * [synths] Refit `marimba` partial by partial to the University of Iowa's 183 yarn recordings at pp, mf and ff, its loudness by velocity following theirs, as loud at C4 at vel 1 as before
  * [synths] Refit `electric_bass` and `deep_bass` partial by partial to their NSynth recordings from E1 to D#4, as loud at A1 as before and growing as much louder with vel
  * [synths] Refit `harp` partial by partial to the folk harp's 57 recordings, each string as loud as the next at vel 1 since the recordings' strings stand up to 24 dB apart, and its velocities as far apart as the recordings', as loud at C4 at vel 1 as before
  * [synths] Refit `guitar` partial by partial to the University of Iowa's guitar at pp, mf and ff, each pitch from the string a player uses in first position, its loudness following the recordings, as loud at E3 at vel 1 as before
  * [synths] Refit `piano` partial by partial to the Steinway B's 126 recordings without the pedal and 126 with it, `pedal` blending the two; at their defaults `aftersound`, `knock` and `uneven` play the recordings as they are, `aftersound` bends each partial's fade toward one rate or deeper, `knock` scales the strike's noise under 600 Hz, and `uneven` varies only the 46 keys not recorded; C4 is as loud as before at each vel, with the pedal and without; every key at a vel carries the same energy, since the recordings are brought to the same peak, the keys above C4 stand under it at vel 0.6 as before, within 4 dB, and the recordings of E6 and of a few others from D6 to D7, whose tone the fit took for noise, are left out
  * [synths] Refit `vibraphone`, `xylophone`, `glockenspiel` and `timpani` partial by partial to the recordings they were fitted to, their loudness following the recordings; `motor` pulses the fitted note

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
