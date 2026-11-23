## The effects

[`level`](#level), [`echo`](#echo), [`lpf`](#lpf), [`hpf`](#hpf), [`distortion`](#distortion), [`pan`](#pan), [`slicer`](#slicer), [`compressor`](#compressor), [`chorus`](#chorus), [`bitcrusher`](#bitcrusher), [`tremolo`](#tremolo), [`rlpf`](#rlpf), [`bpf`](#bpf), [`flanger`](#flanger), [`wobble`](#wobble), [`ping_pong`](#ping_pong), [`pitch_shift`](#pitch_shift), [`eq`](#eq), [`limiter`](#limiter), [`reverb`](#reverb), [`phaser`](#phaser).

## What every effect takes

Besides its own options, every effect takes these.
An effect that lists one of them among its own, as `phaser` lists `mix`, gives it its own default.

| Option | Takes | Default | What it does |
|---|---|---|---|
| `mix` | a number, 0 to 1 | 1 | How much of the processed sound replaces the dry one. |
| `pre_amp` | a number, 0 to 16 | 1 | The level the sound is scaled by before the effect. |
| `amp` | a number, 0 to 16 | 1 | The level the blend is scaled by. |

### `level`

### `echo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 0.25 | The time between echoes. |
| `decay` | seconds | 2 | How long the echoes take to die away. |
| `max_phase` | seconds, 0 to 60 | 2 | The longest phase it can change to. |

### `lpf`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `cutoff` | a MIDI note | 100 | The MIDI note above which it cuts. |

### `hpf`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `cutoff` | a MIDI note | 100 | The MIDI note below which it cuts. |

### `distortion`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `distort` | a number, 0 to 1 | 0.5 | How hard it drives the sound. |

### `pan`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `pan` | a number, -1 to 1 | 0 | Where it turns the sound, from -1, left, to 1, right. |

### `slicer`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 0.25 | The length of a cycle. |
| `phase_offset` | a number, 0 to 1 | 0 | Where in the cycle it starts. |
| `pulse_width` | a number, 0 to 1 | 0.5 | The part of the cycle that is on. |
| `amp_min` | a number | 0 | The level while off. |
| `amp_max` | a number | 1 | The level while on. |
| `smooth_ms` | milliseconds, 0 or more | 0 | How many milliseconds each change takes. |

### `compressor`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `threshold` | a number, -80 to 0 | -18 | The level above which it lowers the sound, in dB below full scale. |
| `ratio` | a number, 1 to 100 | 4 | How many dB the sound has to rise above the threshold to come out 1 dB louder. |
| `makeup` | a number, -40 to 40 | 0 | How many dB it raises the sound afterwards. |
| `clamp_ms` | milliseconds, 0 or more | 10 | How many milliseconds it takes to lower the sound. |
| `relax_ms` | milliseconds, 0 or more | 150 | How many milliseconds it takes to let go. |

### `chorus`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 0.25 | How long the delay takes to sweep back and forth. |
| `decay` | seconds | 0.00001 | How long the delayed sound feeds back. |
| `max_phase` | seconds, 0 to 60 | 1 | The longest phase it can change to. |

### `bitcrusher`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `sample_rate` | hertz, 1 or more | 10000 | The rate in Hz it holds samples at. |
| `bits` | a number, 1 to 24 | 8 | How many bits it rounds each sample to. |
| `cutoff` | a MIDI note | 0 | A MIDI note for a low-pass after it, off at 0. |

### `tremolo`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 4 | How long one sweep takes. |
| `wave` | a number, 0 to 4 | 2 | The sweep's shape: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic. |
| `invert_wave` | a number, 0 to 1 | 0 | 1 flips the sweep. |
| `phase_offset` | a number, 0 to 1 | 0 | Where in the sweep it starts. |
| `depth` | a number, 0 to 1 | 0.5 | How far it turns the sound down. |

### `rlpf`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `cutoff` | a MIDI note | 100 | The MIDI note above which it cuts. |
| `res` | a number, 0 to 1 | 0.5 | How much it rings at its cutoff. |

### `bpf`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `centre` | a MIDI note | 100 | The MIDI note of the band it keeps. |
| `res` | a number, 0 to 1 | 0.6 | How narrow the band is. |

### `flanger`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 4 | How long one sweep takes. |
| `wave` | a number, 0 to 4 | 4 | The sweep's shape: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic. |
| `invert_wave` | a number, 0 to 1 | 0 | 1 flips the sweep. |
| `phase_offset` | a number, 0 to 1 | 0 | Where in the sweep it starts. |
| `stereo_invert_wave` | a number, 0 to 1 | 0 | 1 sweeps the left channel the other way. |
| `delay_ms` | milliseconds | 5 | The delay it sweeps up from. |
| `depth_ms` | milliseconds | 5 | How far it sweeps. |
| `max_delay_ms` | milliseconds, 1 to 1000 | 20 | The longest delay. |
| `feedback` | a number, 0 to 1 | 0 | How strongly the delayed sound feeds back. |
| `decay` | seconds | 2 | How long the delayed sound feeds back. |
| `invert_flange` | a number, 0 to 1 | 0 | 1 subtracts the delayed sound instead. |

### `wobble`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 0.5 | How long one sweep takes. |
| `wave` | a number, 0 to 4 | 0 | The sweep's shape: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic. |
| `invert_wave` | a number, 0 to 1 | 0 | 1 flips the sweep. |
| `phase_offset` | a number, 0 to 1 | 0 | Where in the sweep it starts. |
| `pulse_width` | a number, 0 to 1 | 0.5 | The part of a cycle that is up, for wave 1. |
| `cutoff_min` | a MIDI note | 60 | The MIDI note the filter sweeps down to. |
| `cutoff_max` | a MIDI note | 120 | The MIDI note the filter sweeps up to. |
| `res` | a number, 0 to 1 | 0.8 | How much the filter rings at its cutoff. |
| `filter` | a number, 0 to 1 | 0 | 0 for a low-pass and 1 for a high-pass. |

### `ping_pong`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0.001 or more | 0.25 | The time between echoes. |
| `feedback` | a number, 0 to 0.999 | 0.5 | How much of each echo the next keeps. |
| `max_phase` | seconds, 0.01 to 60 | 1 | The longest phase it can change to. |
| `pan_start` | a number, -1 to 1 | 1 | The side of the first echo, from -1, left, to 1, right. |

### `pitch_shift`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `pitch` | a number, -24 to 24 | 0 | How many semitones the sound is shifted by; the effect delays the sound by up to 40 ms. |

### `eq`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `low_at` | a MIDI note | 36 | The MIDI note below which the low band acts. |
| `low_gain` | a number | 0 | How far the low band is raised or lowered, in dB. |
| `mid_at` | a MIDI note | 60 | The MIDI note the mid band is around. |
| `mid_gain` | a number | 0 | How far the mid band is raised or lowered, in dB. |
| `mid_q` | a number, 0.1 to 20 | 1 | How narrow the mid band is: the higher, the narrower. |
| `high_at` | a MIDI note | 96 | The MIDI note above which the high band acts. |
| `high_gain` | a number | 0 | How far the high band is raised or lowered, in dB. |

### `limiter`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `ceiling` | a number, -60 to 0 | -1 | The level no peak passes, in dB below full scale. |
| `relax_ms` | milliseconds, 1 to 10000 | 100 | How many milliseconds it takes to let the sound back up. |

### `reverb`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `size` | a number, 5 to 60 | 20 | The size of the room in metres, read as the effect starts. |
| `decay_seconds` | seconds, 0.1 to 60 | 2 | How many seconds its sound takes to fade 60 dB. |
| `predelay_ms` | milliseconds, 0 to 500 | 0 | The gap in milliseconds before the room starts, read as the effect starts. |
| `low_cut` | a MIDI note | 0 | The MIDI note below which the room is cut, 0 for none. |
| `damp` | a number, 0 to 1 | 0.5 | How much the room dulls the high frequencies. |

### `phaser`

| Option | Takes | Default | What it does |
|---|---|---|---|
| `phase` | seconds, 0 or more | 4 | How long one sweep takes. |
| `wave` | a number, 0 to 4 | 3 | The sweep's shape: 0 saw, 1 pulse, 2 triangle, 3 sine and 4 cubic. |
| `invert_wave` | a number, 0 to 1 | 0 | 1 flips the sweep. |
| `phase_offset` | a number, 0 to 1 | 0 | Where in the sweep it starts. |
| `stereo_invert_wave` | a number, 0 to 1 | 0 | 1 sweeps the left channel the other way. |
| `stages` | a whole number, 2 to 8 | 4 | How many stages it has, 2, 4, 6 or 8, one notch for every two, read as the effect starts. |
| `low` | a MIDI note | 60 | The MIDI note its sweep starts from. |
| `high` | a MIDI note | 108 | The MIDI note its sweep reaches. |
| `feedback` | a number, 0 to 0.95 | 0 | How much of its sound feeds back, which raises the sound between the notches. |
| `mix` | a number, 0 to 1 | 0.5 | How much of the turned sound replaces the dry one; the notches are deepest at 0.5. |

