// The piano solo of mx's examples/solo/piano.exs, stroke for stroke: [seconds, note, vel, sustain in seconds] for its six passes of 8 beats at 84 beats a minute.
const BEAT = 60 / 84;
const keyboard = [];
for (let n = 21; n <= 108; n++) if ([0, 4, 7, 9].includes(n % 12)) keyboard.push(n);
const chords = [[48, 55, 60, 64], [45, 52, 57, 60], [41, 48, 53, 57], [43, 50, 55, 59]];

function strokes(section) {
  switch (section) {
    case "sweep": {
      const notes = keyboard.concat([...keyboard].reverse());
      return notes.map((note, i) => [i * 8 / notes.length, [note], 0.6, 0.5]);
    }
    case "soft_chords":
      return chords.map((chord, i) => [i * 2, chord, 0.2, 1.8]);
    case "loud_chords":
      return chords.map((chord, i) => [i * 2, chord.map((n) => n + 12), 0.75, 1.8]);
    case "runs":
      return chords.flatMap((chord, i) => {
        const tones = chord.map((n) => n + 12).concat(chord.map((n) => n + 24));
        const run = tones.concat([...tones].reverse());
        return [0, 1, 2, 3, 4, 5, 6, 7].flatMap((step) => {
          const beat = i * 2 + step * 0.25;
          return step === 0
            ? [[beat, [chord[0] - 12], 0.7, 1.9], [beat, [run[step]], 0.55, 0.3]]
            : [[beat, [run[step]], 0.5, 0.3]];
        });
      });
    case "extremes":
      return [[0, [21], 1, 3], [1, [108], 1, 2.5], [3, [108], 0.15, 2], [4, [21], 0.15, 3], [6, [48, 55, 60, 64, 67], 0.1, 2]];
  }
}

export const SECTIONS = ["sweep", "soft_chords", "loud_chords", "runs", "runs", "extremes"];
export const SECONDS = SECTIONS.length * 8 * BEAT;

export function solo() {
  return SECTIONS.flatMap((section, pass) =>
    strokes(section).flatMap(([beat, notes, vel, sustain]) =>
      notes.map((note) => [(pass * 8 + beat) * BEAT, note, vel, sustain * BEAT])));
}
