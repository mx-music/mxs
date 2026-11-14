## The synths

| Family | Synths |
|---|---|
| Basic waveforms | [`saw`](#saw), [`square`](#square), [`tri`](#tri), [`pulse`](#pulse), [`noise`](#noise), [`fm`](#fm), [`pluck`](#pluck), [`supersaw`](#supersaw), [`acid`](#acid), [`brass`](#brass) |
| Piano and plucked strings | [`piano`](#piano), [`guitar`](#guitar), [`cavaquinho`](#cavaquinho), [`bandolim`](#bandolim), [`harp`](#harp), [`sitar`](#sitar), [`tanpura`](#tanpura) |
| Bowed strings | [`violin`](#violin), [`viola`](#viola), [`cello`](#cello), [`double_bass`](#double_bass) |
| Winds | [`flute`](#flute), [`clarinet`](#clarinet), [`oboe`](#oboe), [`bassoon`](#bassoon), [`tenor_sax`](#tenor_sax), [`trumpet`](#trumpet), [`trombone`](#trombone), [`horn`](#horn), [`tuba`](#tuba) |
| Organs and reeds | [`organ`](#organ), [`harmonium`](#harmonium), [`accordion`](#accordion) |
| Tuned percussion | [`marimba`](#marimba), [`vibraphone`](#vibraphone), [`xylophone`](#xylophone), [`glockenspiel`](#glockenspiel), [`timpani`](#timpani) |
| Drums | [`berimbau`](#berimbau), [`kick`](#kick), [`snare`](#snare), [`clap`](#clap), [`crash`](#crash), [`ride`](#ride), [`closed_hihat`](#closed_hihat), [`open_hihat`](#open_hihat), [`low_tom`](#low_tom), [`mid_tom`](#mid_tom), [`high_tom`](#high_tom), [`rimshot`](#rimshot), [`cross_stick`](#cross_stick), [`pedal_hihat`](#pedal_hihat), [`cowbell`](#cowbell), [`clave`](#clave), [`low_agogo`](#low_agogo), [`high_agogo`](#high_agogo), [`ganza`](#ganza), [`shaker`](#shaker), [`low_conga`](#low_conga), [`mid_conga`](#mid_conga), [`high_conga`](#high_conga), [`low_timbale`](#low_timbale), [`high_timbale`](#high_timbale), [`timbale_shell`](#timbale_shell), [`guiro`](#guiro), [`triangle`](#triangle), [`tambourine`](#tambourine), [`tambourine_shake`](#tambourine_shake), [`cabasa`](#cabasa), [`cajon`](#cajon), [`low_bongo`](#low_bongo), [`high_bongo`](#high_bongo), [`pandeiro`](#pandeiro), [`surdo`](#surdo), [`cuica`](#cuica), [`tamborim`](#tamborim), [`caxixi`](#caxixi), [`pandeiro_thumb`](#pandeiro_thumb), [`pandeiro_heel`](#pandeiro_heel), [`pandeiro_slap`](#pandeiro_slap), [`pandeiro_jingles`](#pandeiro_jingles), [`tabla`](#tabla), [`tabla_na`](#tabla_na), [`tabla_tun`](#tabla_tun), [`tabla_te`](#tabla_te), [`tabla_tas`](#tabla_tas), [`tabla_ghe`](#tabla_ghe), [`tabla_ke`](#tabla_ke) |

## What every note takes

Besides its synth's own options, every note takes these.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `amp` | a number, 0 to 16 | 1 | The level the sound is scaled by. |
| `pan` | a number, -1 to 1 | 0 | Where the sound sits, from -1, left, to 1, right. |

## The envelope

A synth that takes the envelope takes these seven options, with its own defaults for `sustain` and `release`.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `attack` | seconds, 0 or more | 0 | How long the note takes to rise to attack_level. |
| `decay` | seconds, 0 or more | 0 | How long it then takes to fall to decay_level. |
| `sustain` | seconds, 0 or more | | How long it then holds, moving to sustain_level. |
| `release` | seconds, 0 or more | | How long it then takes to fall silent. |
| `attack_level` | a number, 0 or more | 1 | The level the attack rises to. |
| `decay_level` | a number | -1 | The level the decay falls to; sustain_level when below 0. |
| `sustain_level` | a number, 0 or more | 1 | The level the note holds at. |

## Basic waveforms

### `saw`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `square`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `tri`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `pulse`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `pulse_width` | a number, 0.01 to 0.99 | 0.3 | The part of each cycle that is up. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `noise`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 110 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `fm`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `divisor` | a number, 0.01 or more | 2 | The note's frequency over the modulator's. |
| `depth` | a number | 1 | How far the modulator swings the frequency, in times the note's. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `pluck`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `ring` | seconds, 0 or more | 3 | How many seconds the string takes to fall 60 dB. |
| `coef` | a number, -0.99 to 0.99 | 0.3 | How fast the string loses its brightness. |

It takes the envelope, with a `sustain` of 0 and a `release` of 2 unless given.

### `supersaw`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 110 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `detune` | a number | 0.2 | How far in semitones the saws spread around the note. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `acid`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `cutoff` | a MIDI note | 70 | The MIDI note the filter closes to. |
| `res` | a number, 0 to 1 | 0.7 | How much the filter rings at its cutoff. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.5 unless given.

### `brass`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.25 unless given.

## Piano and plucked strings

### `piano`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0.05 to 1 | 0.6 | How hard the key is struck. |
| `pedal` | a number, 0 to 1 | 0 | How far the sustain pedal is down, letting the other strings ring. |
| `aftersound` | a number, 0 to 1 | 0.5 | How far each harmonic fades in two stages, fast after the strike and then slowly, as the Steinway's do. |
| `uneven` | a number, 0 to 1 | 1 | How far each key differs from its neighbours in its harmonics' levels, fades and beating. |
| `instrument` | a whole number | 0 | Which piano's keys `uneven` draws, each number another instrument. |
| `knock` | a number, 0 to 1 | 1 | How loud the hammer's knock through the soundboard is. |

It takes the envelope, with a `sustain` of 1 and a `release` of 1 unless given.

### `guitar`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 2 unless given.

### `cavaquinho`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `tremolo` | a number, 0 or more | 0 | How many times a second a held note is picked; 0 picks it once. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.8 unless given.

### `bandolim`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `tremolo` | a number, 0 or more | 0 | How many times a second a held note is picked; 0 picks it once. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.8 unless given.

### `harp`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 2 unless given.

### `sitar`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sa` | a MIDI note | 49 | The note the sympathetic strings are tuned to, and an octave above it. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 2 unless given.

### `tanpura`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | Sa, the note the drone is tuned to. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the strings are plucked. |
| `gap` | seconds, 0.05 or more | 0.8 | The time between plucks; a cycle of the four strings takes six gaps. |

It takes the envelope, with a `sustain` of 16 and a `release` of 4 unless given.

## Bowed strings

### `violin`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is bowed or plucked. |
| `pizz` | a number, 0 to 1 | 0 | 1 plucks the string instead of bowing it. |
| `vibrato` | a number, 0 to 2 | 0.25 | The vibrato's depth in semitones, reached as the note holds. |
| `bow_speed` | a number, 0.1 to 4 | 1 | How fast the bow moves, against its speed at the note's vel. |
| `bow_force` | a number, 0.1 to 4 | 1 | How hard the bow presses, against its force at the note's vel: the harder, the wider the speeds at which it holds the string. |
| `legato` | a number, 0 to 1 | 0 | 1 changes bow from a note still sounding: the string speaks at once instead of swelling in from silence. |
| `body_hz` | hertz, 20 to 2000 |  | The corner below which the body radiates little, in place of the one fitted; for fitting. |
| `rosin` | a number, 0 to 2 |  | How much the rosin's noise sways the bow's speed, in place of the one fitted; for fitting. |
| `hiss` | decibels, -90 to 0 |  | The bow's hiss in dB, in place of the one fitted for the note's vel; for fitting. |
| `bow_position` | a number, 0.05 to 0.5 | 0.12 | Where the bow meets the string, as a fraction of its length from the bridge. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.3 unless given.

### `viola`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is bowed or plucked. |
| `pizz` | a number, 0 to 1 | 0 | 1 plucks the string instead of bowing it. |
| `vibrato` | a number, 0 to 2 | 0.25 | The vibrato's depth in semitones, reached as the note holds. |
| `bow_speed` | a number, 0.1 to 4 | 1 | How fast the bow moves, against its speed at the note's vel. |
| `bow_force` | a number, 0.1 to 4 | 1 | How hard the bow presses, against its force at the note's vel: the harder, the wider the speeds at which it holds the string. |
| `legato` | a number, 0 to 1 | 0 | 1 changes bow from a note still sounding: the string speaks at once instead of swelling in from silence. |
| `body_hz` | hertz, 20 to 2000 |  | The corner below which the body radiates little, in place of the one fitted; for fitting. |
| `rosin` | a number, 0 to 2 |  | How much the rosin's noise sways the bow's speed, in place of the one fitted; for fitting. |
| `hiss` | decibels, -90 to 0 |  | The bow's hiss in dB, in place of the one fitted for the note's vel; for fitting. |
| `bow_position` | a number, 0.05 to 0.5 | 0.12 | Where the bow meets the string, as a fraction of its length from the bridge. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.3 unless given.

### `cello`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is bowed or plucked. |
| `pizz` | a number, 0 to 1 | 0 | 1 plucks the string instead of bowing it. |
| `vibrato` | a number, 0 to 2 | 0.25 | The vibrato's depth in semitones, reached as the note holds. |
| `bow_speed` | a number, 0.1 to 4 | 1 | How fast the bow moves, against its speed at the note's vel. |
| `bow_force` | a number, 0.1 to 4 | 1 | How hard the bow presses, against its force at the note's vel: the harder, the wider the speeds at which it holds the string. |
| `legato` | a number, 0 to 1 | 0 | 1 changes bow from a note still sounding: the string speaks at once instead of swelling in from silence. |
| `body_hz` | hertz, 20 to 2000 |  | The corner below which the body radiates little, in place of the one fitted; for fitting. |
| `rosin` | a number, 0 to 2 |  | How much the rosin's noise sways the bow's speed, in place of the one fitted; for fitting. |
| `hiss` | decibels, -90 to 0 |  | The bow's hiss in dB, in place of the one fitted for the note's vel; for fitting. |
| `bow_position` | a number, 0.05 to 0.5 | 0.12 | Where the bow meets the string, as a fraction of its length from the bridge. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.3 unless given.

### `double_bass`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is bowed or plucked. |
| `pizz` | a number, 0 to 1 | 0 | 1 plucks the string instead of bowing it. |
| `vibrato` | a number, 0 to 2 | 0.25 | The vibrato's depth in semitones, reached as the note holds. |
| `bow_speed` | a number, 0.1 to 4 | 1 | How fast the bow moves, against its speed at the note's vel. |
| `bow_force` | a number, 0.1 to 4 | 1 | How hard the bow presses, against its force at the note's vel: the harder, the wider the speeds at which it holds the string. |
| `legato` | a number, 0 to 1 | 0 | 1 changes bow from a note still sounding: the string speaks at once instead of swelling in from silence. |
| `body_hz` | hertz, 20 to 2000 |  | The corner below which the body radiates little, in place of the one fitted; for fitting. |
| `rosin` | a number, 0 to 2 |  | How much the rosin's noise sways the bow's speed, in place of the one fitted; for fitting. |
| `hiss` | decibels, -90 to 0 |  | The bow's hiss in dB, in place of the one fitted for the note's vel; for fitting. |
| `bow_position` | a number, 0.05 to 0.5 | 0.12 | Where the bow meets the string, as a fraction of its length from the bridge. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.3 unless given.

## Winds

### `flute`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0.17 | The vibrato's depth, reached as the note holds: a flute's vibrato is in the breath, which swells and eases by 0.6 of it, so 0.17 swings a held note's level 1.6 to 2.3 dB, as a played Syrinx's 2 and under VSCO 2's 2 to 4, and its pitch follows. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `lift` | a number, 0 to 1 | 0.5 | How much louder a higher note sounds, as a player blows it harder, as the power of its pitch over middle C; 0 sounds every note alike. |
| `wander` | a number, 0 to 4 | 0.2 | How much the breath drifts while a note holds, as a player's does; 0 holds it still. |
| `catch` | a number, 0 to 1 | 0.15 | How far the breath falls while a slur moves the pitch; 0 glides smoothly. |
| `corner` | a number, 500 to 20000 |  | The bore's corner in Hz, never under three times the pitch, in place of the one fitted; for fitting. |
| `shelf` | decibels, -30 to 30 |  | The gain in dB above 4 kHz, in place of the one fitted; for fitting. |
| `hiss` | decibels, -90 to 0 |  | The breath's hiss above 3 kHz in dB, in place of the one fitted; for fitting. |
| `tuned` | a number, 0 to 3 |  | The breath at which the jet crosses to the edge in half a period, in place of the mf breath; 0 holds it there. |
| `pulse` | a number, 0 to 1 |  | How much the hiss follows the jet across the edge, from 0 steady to 1 only on one side, in place of the one fitted; for fitting. |
| `even` | a number, 0 to 4 |  | The part of the square of the jet's sound in the output, in place of the one fitted; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `clarinet`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `oboe`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `bassoon`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `tenor_sax`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth, reached as the note holds, where 1 is the depth of the recordings played with vibrato. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `trumpet`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `trombone`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `horn`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `tuba`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown; a change while the note holds swells or fades it. |
| `vibrato` | a number, 0 to 2 | 0 | The vibrato's depth in semitones, reached as the note holds. |
| `breath` | a number, 0 to 3 |  | The breath's pressure, in place of the one fitted for the note's vel; for fitting. |
| `rise` | seconds, 0.001 to 2 |  | The seconds the breath takes to reach its pressure, in place of the one fitted for the note's vel; for fitting. |
| `air` | decibels, -90 to 0 |  | The breath's noise in dB, in place of the one fitted for the note's vel; for fitting. |
| `level` | decibels, -60 to 60 |  | The whole sound's level in dB, in place of the one fitted for the note's vel; for fitting. |
| `eq0` | decibels, -60 to 60 |  | The gain in dB of the band at 113 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq1` | decibels, -60 to 60 |  | The gain in dB of the band at 200 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq2` | decibels, -60 to 60 |  | The gain in dB of the band at 316 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq3` | decibels, -60 to 60 |  | The gain in dB of the band at 502 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq4` | decibels, -60 to 60 |  | The gain in dB of the band at 794 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq5` | decibels, -60 to 60 |  | The gain in dB of the band at 1265 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq6` | decibels, -60 to 60 |  | The gain in dB of the band at 2000 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq7` | decibels, -60 to 60 |  | The gain in dB of the band at 3162 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq8` | decibels, -60 to 60 |  | The gain in dB of the band at 5012 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq9` | decibels, -60 to 60 |  | The gain in dB of the band at 7943 Hz, in place of the one fitted for the note's vel; for fitting. |
| `eq10` | decibels, -60 to 60 |  | The gain in dB of the band at 12589 Hz, in place of the one fitted for the note's vel; for fitting. |
| `kick` | a number, 0 to 3 |  | The breath a soft note starts at, in place of the fitted one; for fitting. |
| `catch` | a number, 0 to 1 |  | How far the sound falls at a slur, in place of the fitted one; for fitting. |
| `mend` | a number, 0 to 1 |  | The seconds a slur's dip takes to come back to a third, in place of the fitted one; for fitting. |
| `gasp` | a number, 0 to 1 |  | How far the breath falls at a slur, in place of the fitted one; for fitting. |
| `corner` | a number, 0 to 100 |  | The bore's corner as a multiple of the pitch, in place of the fitted one; for fitting. |
| `cutoff` | a number, 0 to 20000 |  | The least the bore's corner is, in Hz, in place of the fitted one; for fitting. |
| `keep` | a number, 0 to 1 |  | What the bore keeps at each round trip, in place of the fitted one; for fitting. |
| `zeta` | a number, 0 to 1 |  | How freely the reed lets the air through, in place of the fitted one; for fitting. |
| `apex` | a number, 0 to 0.5 |  | A cone's missing apex, as part of its whole length, in place of the fitted one; for fitting. |
| `bell` | a number, 0 to 20000 |  | The corner in Hz above which the bore's end lets the sound out, in place of the fitted one; for fitting. |
| `tongue` | a number, 0 to 2 |  | The part of the mouth's pressure the tongue's release sends in, in place of the fitted one; for fitting. |
| `steep` | a number, 0 to 50 |  | The radians of the note's period a unit of pressure arrives earlier at the bell, in place of the fitted one; for fitting. |
| `jitter` | a number, 0 to 50 |  | The cents the pitch drifts while a note holds, in place of the fitted one; for fitting. |
| `shimmer` | a number, 0 to 6 |  | The dB the level drifts while a note holds, in place of the fitted one; for fitting. |
| `wander` | a number, 0 to 4 |  | The part of itself the steepening drifts while a note holds, in place of the fitted one; for fitting. |
| `lift` | a number, -24 to 24 |  | The dB louder the sound is an octave up, in place of the fitted one; for fitting. |
| `sharp` | a number, -100 to 100 |  | The cents the model plays sharp, in place of the fitted ones; for fitting. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

## Organs and reeds

### `organ`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `loud` | a number, 0 to 1 | 0 | 1 plays the loud set, a principal chorus, instead of the quiet stopped flute. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.15 unless given.

### `harmonium`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the bellows are pumped. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `accordion`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the bellows are pressed. |
| `detune` | a number, 0 to 20 | 4 | How many Hz the second reed sounds above the first, which beat that many times a second; 0 sounds one reed. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.15 unless given.

## Tuned percussion

### `marimba`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `vibraphone`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `motor` | a number, 0 or more | 0 | How many times a second the discs in the tubes pulse the note; 0 leaves the motor off. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `xylophone`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `glockenspiel`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `timpani`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

## Drums

### `berimbau`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 39.11 | The open string's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the stick strikes. |
| `coin` | a number, 0 to 1 | 0 | How hard the coin presses the wire, raising it a whole tone. |
| `buzz` | a number, 0 to 1 | 0 | How loosely the coin rests on the wire, rattling against it. |
| `gourd` | a number, 0 to 20 | 2.3 | How many times a second the gourd swings against the belly. |

### `kick`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 34 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `beater` | a number, 0 to 1 | 1 | How hard the beater is; 0 is a felt beater, which leaves out the click. |

### `snare`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 55 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `snares` | a number, 0 to 1 | 1 | 0 releases the wires from the head. |
| `rim` | a number, 0 to 1 | 0 | 1 strikes the rim with the head, a rimshot. |
| `buzz` | a number, 0 to 1 | 0 | 1 presses the stick into the head so it bounces. |

### `clap`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The clap's pitch. |

### `crash`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 9 | How long it rings before it is choked. |
| `release` | seconds, 0 or more | 1 | How long the choke takes. |

### `ride`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 8 | How long it rings before it is damped. |
| `release` | seconds, 0 or more | 3 | How long the damping takes. |
| `bell` | a number, 0 to 1 | 0 | 1 strikes the bell instead of the bow. |

### `closed_hihat`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 92 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `release` | seconds, 0 or more | 0.05 | How long the hat takes to close. |

### `open_hihat`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 92 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `release` | seconds, 0 or more | 0.05 | How long the hat takes to close. |
| `sustain` | seconds, 0 or more | 5 | How long it rings open. |

### `low_tom`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 41.85 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `mid_tom`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 43.4 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `high_tom`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 45 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `rimshot`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 74 | The rim's pitch. |

### `cross_stick`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 53.6 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `pedal_hihat`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `cowbell`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 70.07 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 damps the bell with the hand. |

### `clave`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 95.2 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `low_agogo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 84 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `high_agogo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 90.07 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `ganza`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 111 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `up` | a number, 0 to 1 | 0 | 1 shakes upward instead of down. |

### `shaker`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 103 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `up` | a number, 0 to 1 | 0 | 1 shakes upward instead of down. |

### `low_conga`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 48.93 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `mute` | a number, 0 to 1 | 0 | 1 taps the head with the fingers. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the head. |
| `damp` | a number, 0 to 1 | 0 | 1 leaves the hand on the head, which stops the tone. |

### `mid_conga`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 51.88 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `mute` | a number, 0 to 1 | 0 | 1 taps the head with the fingers. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the head. |
| `damp` | a number, 0 to 1 | 0 | 1 leaves the hand on the head, which stops the tone. |

### `high_conga`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 56.62 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `mute` | a number, 0 to 1 | 0 | 1 taps the head with the fingers. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the head. |
| `damp` | a number, 0 to 1 | 0 | 1 leaves the hand on the head, which stops the tone. |

### `low_timbale`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 62 | The drum's pitch. |

### `high_timbale`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 67 | The drum's pitch. |

### `timbale_shell`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 93 | The shell's pitch. |

### `guiro`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 93.29 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0.02 or more | 0.25 | How long the scrape lasts. |

### `triangle`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 69 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 30 | How long the bar rings before the hand damps it. |
| `release` | seconds, 0 or more | 1 | How long the hand takes to damp it. |

### `tambourine`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 69 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `tambourine_shake`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 69 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0.02 or more | 0.25 | How long it is shaken. |

### `cabasa`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 96 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `rub` | a number, 0 to 1 | 0 | 1 turns the beads against the cylinder for the sustain. |
| `sustain` | seconds, 0.01 or more | 0.1 | How long a rub lasts. |

### `cajon`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 40 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the front plate instead of the bass tone. |

### `low_bongo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 mutes the head with the hand. |

### `high_bongo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 70 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 mutes the head with the hand. |

### `pandeiro`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |
| `stroke` | one of `thumb`, `heel`, `slap`, `jingles` | `thumb` | The stroke: the thumb, the heel of the hand, a slap or the fingertips on the jingles. |

### `surdo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 36.31 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 mutes the head with the hand, so it rings higher and shorter. |

### `cuica`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 65.65 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 0 | How long the stroke lasts, as one of a run of strokes; 0 is a stroke played alone, at its own length. |
| `scoop` | a number, -12 to 12 |  | The semitones under its note the stroke starts, or over it when below 0; with none, as the nearest recorded stroke does. |

### `tamborim`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 72.03 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `caxixi`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is shaken. |

### `pandeiro_thumb`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `pandeiro_heel`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `pandeiro_slap`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `pandeiro_jingles`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `tabla`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bol` | one of `na`, `tun`, `te`, `tas`, `ghe`, `ke` | `na` | The stroke: na, tun, te and tas on the dayan, ghe and ke on the bayan. |
| `note` | a MIDI note |  | The stroke's pitch; each bol has its own when not given. |

### `tabla_na`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_tun`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_te`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_tas`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_ghe`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 42.25 | The stroke's pitch. |

### `tabla_ke`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 44.75 | The stroke's pitch. |

