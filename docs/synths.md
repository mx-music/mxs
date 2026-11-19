## The synths

| Family | Synths |
|---|---|
| Basic waveforms | [`saw`](#saw), [`square`](#square), [`tri`](#tri), [`pulse`](#pulse), [`noise`](#noise), [`fm`](#fm), [`pluck`](#pluck), [`supersaw`](#supersaw), [`acid`](#acid), [`brass`](#brass) |
| Piano and plucked strings | [`piano`](#piano), [`guitar`](#guitar), [`electric_bass`](#electric_bass), [`deep_bass`](#deep_bass), [`cavaquinho`](#cavaquinho), [`bandolim`](#bandolim), [`harp`](#harp), [`sitar`](#sitar), [`tanpura`](#tanpura), [`electric_piano`](#electric_piano), [`electric_guitar`](#electric_guitar) |
| Bowed strings | [`violin`](#violin), [`viola`](#viola), [`cello`](#cello), [`double_bass`](#double_bass) |
| Winds | [`flute`](#flute), [`flute_figured`](#flute_figured), [`clarinet`](#clarinet), [`oboe`](#oboe), [`bassoon`](#bassoon), [`tenor_sax`](#tenor_sax), [`trumpet`](#trumpet), [`trombone`](#trombone), [`horn`](#horn), [`tuba`](#tuba) |
| Organs and reeds | [`organ`](#organ), [`harmonium`](#harmonium), [`accordion`](#accordion) |
| Tuned percussion | [`marimba`](#marimba), [`vibraphone`](#vibraphone), [`xylophone`](#xylophone), [`glockenspiel`](#glockenspiel), [`timpani`](#timpani) |
| Drums | [`berimbau`](#berimbau), [`kick`](#kick), [`snare`](#snare), [`clap`](#clap), [`crash`](#crash), [`ride`](#ride), [`closed_hihat`](#closed_hihat), [`open_hihat`](#open_hihat), [`low_tom`](#low_tom), [`mid_tom`](#mid_tom), [`high_tom`](#high_tom), [`rimshot`](#rimshot), [`cross_stick`](#cross_stick), [`pedal_hihat`](#pedal_hihat), [`cowbell`](#cowbell), [`clave`](#clave), [`low_agogo`](#low_agogo), [`high_agogo`](#high_agogo), [`ganza`](#ganza), [`shaker`](#shaker), [`low_conga`](#low_conga), [`mid_conga`](#mid_conga), [`high_conga`](#high_conga), [`low_timbale`](#low_timbale), [`high_timbale`](#high_timbale), [`timbale_shell`](#timbale_shell), [`guiro`](#guiro), [`triangle`](#triangle), [`tambourine`](#tambourine), [`tambourine_shake`](#tambourine_shake), [`cabasa`](#cabasa), [`cajon`](#cajon), [`low_bongo`](#low_bongo), [`high_bongo`](#high_bongo), [`pandeiro`](#pandeiro), [`surdo`](#surdo), [`cuica`](#cuica), [`tamborim`](#tamborim), [`caxixi`](#caxixi), [`pandeiro_thumb`](#pandeiro_thumb), [`pandeiro_heel`](#pandeiro_heel), [`pandeiro_slap`](#pandeiro_slap), [`pandeiro_jingles`](#pandeiro_jingles), [`tabla`](#tabla), [`tabla_na`](#tabla_na), [`tabla_tun`](#tabla_tun), [`tabla_te`](#tabla_te), [`tabla_tas`](#tabla_tas), [`tabla_ghe`](#tabla_ghe), [`tabla_ke`](#tabla_ke) |

## What every note takes

Besides its synth's own options, every note takes these.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `amp` | a number, 0 to 16 | 1 | The level the sound is scaled by. |
| `pan` | a number, -1 to 1 | 0 | Where the sound sits, from -1, left, to 1, right. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |
| `pressure` | a number, 0 to 1 | 0 | How hard a held note is pressed once it has started. |

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

A sawtooth wave under a resonant low-pass filter.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `square`

A square wave under a resonant low-pass filter.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `tri`

A triangle wave under a resonant low-pass filter.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `pulse`

A pulse wave of any width under a resonant low-pass filter.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 100 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `pulse_width` | a number, 0.01 to 0.99 | 0.3 | The part of each cycle that is up. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `noise`

Noise under a resonant low-pass filter.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 110 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `fm`

A sine wave whose frequency another sine wave swings.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `divisor` | a number, 0.01 or more | 2 | The note's frequency over the modulator's. |
| `depth` | a number | 1 | How far the modulator swings the frequency, in times the note's. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `pluck`

A plain plucked string, which rings for as long as asked and dulls as it rings.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `ring` | seconds, 0 or more | 3 | How many seconds the string takes to fall 60 dB. |
| `coef` | a number, -0.99 to 0.99 | 0.3 | How fast the string loses its brightness. |

It takes the envelope, with a `sustain` of 0 and a `release` of 2 unless given.

### `supersaw`

Sawtooth waves spread around the note, under a resonant low-pass filter.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 110 | The MIDI note above which the low-pass cuts. |
| `res` | a number, 0 to 1 | 0.2 | How much the low-pass rings at its cutoff. |
| `detune` | a number | 0.2 | How far in semitones the saws spread around the note. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 0 and a `release` of 1 unless given.

### `acid`

An acid bass, a wave under a resonant filter that closes as the note sounds.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `cutoff` | a MIDI note | 70 | The MIDI note the filter closes to. |
| `res` | a number, 0 to 1 | 0.7 | How much the filter rings at its cutoff. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.5 unless given.

### `brass`

A plain brass sound, brighter the harder it is blown.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is blown. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.25 unless given.

## Piano and plucked strings

### `piano`

A grand piano fitted partial by partial to the Steinway B of the Versilian Community Sample Library from A#0 to G#7, with the sustain pedal and without, and played from A0 to C8. What a note is made of can be had without playing it, from the same options: its harmonics, each with a frequency, a level and the seconds it takes to fall 60 dB, the partials between them as twins of the nearest harmonic, the noise of the strike and the knock under 600 Hz.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0.05 to 1 | 0.6 | How hard the key is struck. |
| `pedal` | a number, 0 to 1 | 0 | How far the sustain pedal is down, letting the other strings ring. |
| `aftersound` | a number, 0 to 1 | 0.5 | How far each partial fades in two stages, fast after the strike and then slowly: 0.5 as the Steinway's do, 0 at one rate, and 1 with a deeper fast stage. |
| `uneven` | a number, 0 to 1 | 1 | How far each key that was not recorded differs from the recorded keys either side in its partials' levels. |
| `instrument` | a whole number | 0 | Which piano's keys `uneven` draws, each number another instrument. |
| `knock` | a number, 0 to 1 | 1 | How loud the hammer's knock under 600 Hz is: 1 as recorded, 0 left out. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1 and a `release` of 1 unless given.

### `guitar`

A nylon-string guitar fitted partial by partial to the University of Iowa's guitar from E2 to B5, and played down to B1, the low string of a 7-string guitar. A note rings until it fades by itself.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 2 unless given.

### `electric_bass`

An electric bass played with the fingers, fitted partial by partial to the notes of the bass_electronic_018 instrument of Google's NSynth dataset from E1 to D#4. A note rings until it fades by itself.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 33 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.1 unless given.

### `deep_bass`

A dark bass played with the fingers, fitted partial by partial to the notes of the bass_electronic_025 and bass_electronic_027 instruments of Google's NSynth dataset from E1 to D#4. A note rings long until it fades by itself.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 33 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.1 unless given.

### `cavaquinho`

A cavaquinho, with steel strings, brighter and shorter than the guitar.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `tremolo` | a number, 0 or more | 0 | How many times a second a held note is picked; 0 picks it once. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.8 unless given.

### `bandolim`

A bandolim, the Brazilian mandolin, with steel strings and a tremolo.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is plucked. |
| `tremolo` | a number, 0 or more | 0 | How many times a second a held note is picked; 0 picks it once. |

It takes the envelope, with a `sustain` of 0 and a `release` of 0.8 unless given.

### `harp`

A folk harp, the kind the Paraguayan and llanera harps are, fitted partial by partial to the folk harp of the Versilian Community Sample Library from C2 to G#6.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 2 unless given.

### `sitar`

A plucked sitar note with its sympathetic strings, fitted to "sitar01.flac" on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sa` | a MIDI note | 49 | The note the sympathetic strings are tuned to, and an octave above it. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 2 unless given.

### `tanpura`

A tanpura's drone, four strings plucked in turn with the note as Sa, fitted to "Tanpura 10" by sankalp on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | Sa, the note the drone is tuned to. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the strings are plucked. |
| `gap` | seconds, 0.05 or more | 0.8 | The time between plucks; a cycle of the four strings takes six gaps. |

It takes the envelope, with a `sustain` of 16 and a `release` of 4 unless given.

### `electric_piano`

An electric piano whose hammers strike tines, fitted to the notes of the keyboard_electronic_098 instrument of Google's NSynth dataset from E1 to E6. A note rings while the key is held until it fades by itself.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the key is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.15 unless given.

### `electric_guitar`

A solid-body electric guitar picked clean, fitted to the notes of the guitar_electronic_028 instrument of Google's NSynth dataset from E2 to D6. A note rings until it fades by itself.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is picked. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.08 unless given.

## Bowed strings

### `violin`

A violin, bowed or plucked, its bowed notes fitted partial by partial to the University of Iowa's arco violin at pp, mf and ff from G3 to B7, played there without vibrato, so its vibrato is a player's; a bowed note follows its note as it slides, so notes slurred in one bow are one note whose note changes.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slides to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is bowed or plucked. |
| `pizz` | a number, 0 to 1 | 0 | 1 plucks the string instead of bowing it. |
| `vibrato` | a number, 0 to 2 | 0.25 | The vibrato's depth in semitones, reached as the note holds; the recordings are played without one. |
| `legato` | a number, 0 to 1 | 0 | 1 changes bow from a note still sounding: the string speaks at once, from where the recordings settle, instead of from silence. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.3 unless given.

### `viola`

A viola, bowed or plucked as the violin is, fitted to the University of Iowa's viola from C3 to Bb6.

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

A cello, bowed or plucked, its bowed notes fitted partial by partial to the University of Iowa's arco cello at pp, mf and ff from C2 to Bb5, played there without vibrato, so its vibrato is a player's.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch; a change while the note holds slides to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the string is bowed or plucked. |
| `pizz` | a number, 0 to 1 | 0 | 1 plucks the string instead of bowing it. |
| `vibrato` | a number, 0 to 2 | 0.25 | The vibrato's depth in semitones, reached as the note holds; the recordings are played without one. |
| `legato` | a number, 0 to 1 | 0 | 1 changes bow from a note still sounding: the string speaks at once, from where the recordings settle, instead of from silence. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.3 unless given.

### `double_bass`

A double bass at the pitch it sounds, bowed or plucked as the violin is, fitted to the University of Iowa's double bass from E1 to G4.

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

A flute, a model of an air jet at the end of a bore, so a change of note is a slur on the same breath. Fitted to the University of Iowa's flute from B3 to Db7 and VSCO 2's flute.

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

### `flute_figured`

A flute played from figures fitted partial by partial to the University of Iowa's flute; temporary, to compare with the flute.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 72 | The pitch; a change while the note holds slurs to the new one. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the flute is blown; a change while the note holds swells or eases it. |
| `vibrato` | a number, 0 to 2 |  | The vibrato's depth in semitones, in place of the recordings'. |
| `inhale` | a number, 0 to 1 | 0 | 1 takes a breath once the note is let go, as a player does at the end of a phrase. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `clarinet`

A Bb clarinet at the pitch it sounds, a model of a reed on a bore, fitted to the University of Iowa's Bb clarinet from D3 to C7.

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

An oboe, a model of a reed on a bore, fitted to the University of Iowa's oboe from Bb3 to Ab6.

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

A bassoon, a model of a reed on a bore, fitted to the University of Iowa's bassoon from Bb1 to D5.

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

A tenor saxophone at the pitch it sounds, a model of a reed on a bore, fitted to the tenor saxophone of the Versilian Community Sample Library from Ab2 to E5.

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

A Bb trumpet at the pitch it sounds, a model of lips on a bore, fitted to the University of Iowa's Bb trumpet from E3 to Eb6.

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

A tenor trombone, a model of lips on a bore, fitted to the University of Iowa's tenor trombone from E2 to C5.

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

A French horn, a model of lips on a bore, fitted to VSCO 2's F horn from C2 to F5.

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

A tuba, a model of lips on a bore, fitted to VSCO 2's tuba from Bb1 to Bb3.

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

A pipe organ fitted to VSCO 2's organ, with a quiet set, a stopped flute, and a loud one, a principal chorus.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |
| `loud` | a number, 0 to 1 | 0 | 1 plays the loud set, a principal chorus, instead of the quiet stopped flute. |

It takes the envelope, with a `sustain` of 1 and a `release` of 0.15 unless given.

### `harmonium`

The reed organ pumped by hand that accompanies Hindustani singing, fitted to the "Harmonium" pack by cabled_mess on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the bellows are pumped. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.2 unless given.

### `accordion`

An accordion's right hand: two reeds a few Hz apart, beating, fitted to "Accordeon master" by hammondman on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the bellows are pressed. |
| `detune` | a number, 0 to 20 | 4 | How many Hz the second reed sounds above the first, which beat that many times a second; 0 sounds one reed. |

It takes the envelope, with a `sustain` of 0.5 and a `release` of 0.15 unless given.

## Tuned percussion

### `marimba`

A marimba, fitted partial by partial to the University of Iowa's marimba struck with yarn mallets, from C2 to C7, and played down to F1.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `vibraphone`

A vibraphone with its motor, fitted partial by partial to the University of Iowa's vibraphone played with the motor off, from C3 to F6.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |
| `motor` | a number, 0 or more | 0 | How many times a second the discs in the tubes pulse the note; 0 leaves the motor off. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `xylophone`

A xylophone, fitted partial by partial to the University of Iowa's xylophone struck with rosewood mallets, from F4 to C8.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `glockenspiel`

A glockenspiel, fitted partial by partial to the University of Iowa's orchestral bells struck with plastic mallets, from C5 to E8.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

### `timpani`

A timpani, whose partials are not whole multiples of its note, fitted partial by partial to the five timpani of VSCO 2, from 92 to 153 Hz.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 52 | The pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bend` | a number, -120 to 120 | 0 | Semitones the pitch is moved by, without starting the note again. |
| `brightness` | a number, 0 to 1 | 0.5 | How bright the sound is: 0.5 is the synth's own, above it brighter, below it duller. |

It takes the envelope, with a `sustain` of 1000 and a `release` of 0.3 unless given.

## Drums

### `berimbau`

The berimbau of capoeira: the string struck by the stick, open, with the coin pressed or buzzed, and swung by the gourd. Fitted to "Berimbau jam without caxixi" by EliasArtista on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 39.11 | The open string's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard the stick strikes. |
| `coin` | a number, 0 to 1 | 0 | How hard the coin presses the wire, raising it a whole tone. |
| `buzz` | a number, 0 to 1 | 0 | How loosely the coin rests on the wire, rattling against it. |
| `gourd` | a number, 0 to 20 | 2.3 | How many times a second the gourd swings against the belly. |

### `kick`

A kick drum.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 34 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `beater` | a number, 0 to 1 | 1 | How hard the beater is; 0 is a felt beater, which leaves out the click. |

### `snare`

A snare drum, fitted to the snare of VSCO 2 with its snares on.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 55 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `snares` | a number, 0 to 1 | 1 | 0 releases the wires from the head. |
| `rim` | a number, 0 to 1 | 0 | 1 strikes the rim with the head, a rimshot. |
| `buzz` | a number, 0 to 1 | 0 | 1 presses the stick into the head so it bounces. |

### `clap`

A hand clap.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The clap's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |

### `crash`

A crash cymbal, fitted to the crash cymbal of VSCO 2.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 9 | How long it rings before it is choked. |
| `release` | seconds, 0 or more | 1 | How long the choke takes. |

### `ride`

A ride cymbal.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 8 | How long it rings before it is damped. |
| `release` | seconds, 0 or more | 3 | How long the damping takes. |
| `bell` | a number, 0 to 1 | 0 | 1 strikes the bell instead of the bow. |

### `closed_hihat`

A closed hi-hat, fitted to the hi-hat of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 92 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `release` | seconds, 0 or more | 0.05 | How long the hat takes to close. |

### `open_hihat`

An open hi-hat, fitted to the hi-hat of the Versilian Community Sample Library, which a shorter sustain closes as the pedal does.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 92 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `release` | seconds, 0 or more | 0.05 | How long the hat takes to close. |
| `sustain` | seconds, 0 or more | 5 | How long it rings open. |

### `low_tom`

A low tom, fitted to the low tom of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 41.85 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `mid_tom`

A middle tom, the high tom tuned between the high and the low one.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 43.4 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `high_tom`

A high tom, fitted to the high tom of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 45 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `rimshot`

A rimshot.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 74 | The rim's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |

### `cross_stick`

A cross stick, the stick laid on the snare's head and struck on its rim.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 53.6 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `pedal_hihat`

A hi-hat closed with the pedal.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `cowbell`

A cowbell, open or muted by the hand, fitted to the cowbells of VSCO 2 and the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 70.07 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 damps the bell with the hand. |

### `clave`

Claves, fitted to those of the Versilian Community Sample Library and VSCO 2.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 95.2 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `low_agogo`

The low bell of an agogô, fitted to the agogô of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 84 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `high_agogo`

The high bell of an agogô, fitted to the agogô of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 90.07 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `ganza`

A ganzá, a small shaker, with a down and an up stroke, fitted to the small shaker of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 111 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `up` | a number, 0 to 1 | 0 | 1 shakes upward instead of down. |

### `shaker`

A large shaker, with a down and an up stroke, fitted to the large shaker of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 103 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `up` | a number, 0 to 1 | 0 | 1 shakes upward instead of down. |

### `low_conga`

The tumba, the low conga, with open, muted and slapped strokes, fitted to VSCO 2's congas.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 48.93 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `mute` | a number, 0 to 1 | 0 | 1 taps the head with the fingers. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the head. |
| `damp` | a number, 0 to 1 | 0 | 1 leaves the hand on the head, which stops the tone. |

### `mid_conga`

The conga, the middle drum of the three, with open, muted and slapped strokes, fitted to VSCO 2's congas.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 51.88 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `mute` | a number, 0 to 1 | 0 | 1 taps the head with the fingers. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the head. |
| `damp` | a number, 0 to 1 | 0 | 1 leaves the hand on the head, which stops the tone. |

### `high_conga`

The quinto, the high conga, with open, muted and slapped strokes, fitted to VSCO 2's congas.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 56.62 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `mute` | a number, 0 to 1 | 0 | 1 taps the head with the fingers. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the head. |
| `damp` | a number, 0 to 1 | 0 | 1 leaves the hand on the head, which stops the tone. |

### `low_timbale`

The low timbale.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 62 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |

### `high_timbale`

The high timbale.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 67 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |

### `timbale_shell`

A stroke on the timbale's shell, as cáscara is played.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 93 | The shell's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is played, which scales its level by vel / 0.6. |

### `guiro`

A güiro, scraped for as long as its sustain, fitted to the University of Iowa's güiro and those of VSCO 2 and the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 93.29 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0.02 or more | 0.25 | How long the scrape lasts. |

### `triangle`

A triangle, which a hand damps, fitted to the University of Iowa's 6- and 8-inch triangles.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 69 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 30 | How long the bar rings before the hand damps it. |
| `release` | seconds, 0 or more | 1 | How long the hand takes to damp it. |

### `tambourine`

A tambourine struck, fitted to the University of Iowa's three tambourines.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 69 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `tambourine_shake`

A tambourine shaken for as long as its sustain.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 69 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0.02 or more | 0.25 | How long it is shaken. |

### `cabasa`

A cabasa, struck or turned, fitted to the cabasa of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 96 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `rub` | a number, 0 to 1 | 0 | 1 turns the beads against the cylinder for the sustain. |
| `sustain` | seconds, 0.01 or more | 0.1 | How long a rub lasts. |

### `cajon`

A cajón, from its bass tone to its slap, fitted to the cajón of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 40 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `slap` | a number, 0 to 1 | 0 | 1 slaps the front plate instead of the bass tone. |

### `low_bongo`

The low bongo, open or muted, fitted to the bongos of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 60 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 mutes the head with the hand. |

### `high_bongo`

The high bongo, open or muted, fitted to the bongos of the Versilian Community Sample Library.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 70 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 mutes the head with the hand. |

### `pandeiro`

The pandeiro of choro and samba, whose `stroke` option picks the thumb, the heel, the slap or the jingles, fitted to pandeiro recordings on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |
| `stroke` | one of `thumb`, `heel`, `slap`, `jingles` | `thumb` | The stroke: the thumb, the heel of the hand, a slap or the fingertips on the jingles. |

### `surdo`

The large drum that holds samba's beat, struck with a felt mallet, open or muted, fitted to three loops by plotzki on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 36.31 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 mutes the head with the hand, so it rings higher and shorter. |

### `cuica`

The friction drum of samba: each stroke is played back from the nearest of 28 strokes measured in reinsamba's pack "Cuica" on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 65.65 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `sustain` | seconds, 0 or more | 0 | How long the stroke lasts, as one of a run of strokes; 0 is a stroke played alone, at its own length. |
| `scoop` | a number, -12 to 12 |  | The semitones under its note the stroke starts, or over it when below 0; with none, as the nearest recorded stroke does. |

### `tamborim`

The small high drum of samba struck with a stick, fitted to "Tamborim Sound.ogg" by Samba-x on Wikimedia Commons.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 72.03 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |

### `caxixi`

The woven basket shaker a berimbau player shakes, fitted to "Caxixi.ogg" by Freddythehat on Wikimedia Commons.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is shaken. |

### `pandeiro_thumb`

The pandeiro's thumb stroke, its low tone.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `pandeiro_heel`

The pandeiro's heel stroke.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `pandeiro_slap`

The pandeiro's slap.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `pandeiro_jingles`

The pandeiro's jingles.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `note` | a MIDI note | 49 | The drum's pitch. |
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `muted` | a number, 0 to 1 | 0 | 1 presses a finger of the holding hand on the head from behind, so it rings higher and shorter. |

### `tabla`

The tabla, whose `bol` option picks the stroke of the dayan or the bayan, fitted to the "tabla bols" pack by mmiron on Freesound.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `bol` | one of `na`, `tun`, `te`, `tas`, `ghe`, `ke` | `na` | The stroke: na, tun, te and tas on the dayan, ghe and ke on the bayan. |
| `note` | a MIDI note |  | The stroke's pitch; each bol has its own when not given. |

### `tabla_na`

Na, a stroke of the dayan, the tabla's higher drum.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_tun`

Tun, a stroke of the dayan, the tabla's higher drum.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_te`

Te, a stroke of the dayan, the tabla's higher drum.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_tas`

Tas, a stroke of the dayan, the tabla's higher drum.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 63 | The stroke's pitch. |

### `tabla_ghe`

Ghe, a stroke of the bayan, the tabla's lower drum, which a change of note bends as the palm does.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 42.25 | The stroke's pitch. |

### `tabla_ke`

Ke, a stroke of the bayan, the tabla's lower drum.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `vel` | a number, 0 to 1 | 0.6 | How hard it is struck. |
| `note` | a MIDI note | 44.75 | The stroke's pitch. |

