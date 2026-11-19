# Instruments as data

These are the notes of mxs-00012, a spike on loading an instrument from a file at run time instead of compiling it in.
They were written on 19 November 2026 against mxs 0.1.6 and the commits after it on main.

## What already exists

mxs already builds a synth from data: `graph::Graph` is a tree of numbers, names, options, lists and calls of the vocabulary in `src/graph.rs`, and `Synth::graph(name, specs, clauses)` and `Effect::graph` build from it when a note starts, with no compiling.
mx's `defsynth` traces Elixir into that tree, encodes it as Erlang terms, and its NIF turns the terms into a `Graph`.
So a file format needs no new engine: it is a way to write `Graph`, `Spec` and `Clause` down, and a loader that turns the file into them.

## The format

Recommendation: a JSON file, one synth or effect to a file, which mxs loads with `Synth::from_json` and `Effect::from_json`, or from a path.

JSON, because Python and JavaScript read and write it with nothing installed, every editor shows it, and it maps one to one onto `Graph`.
TOML and YAML have no natural form for a deep tree of calls, and a binary format would save little on files of about 10 KB.

```json
{
  "format": "mxs-synth",
  "version": 1,
  "name": "plate",
  "doc": "A metal plate struck with a hard mallet.",
  "options": [
    {"name": "note", "kind": "note", "default": 60, "doc": "The pitch."},
    {"name": "vel", "kind": "float", "default": 0.6, "range": [0, 1], "doc": "How hard it is struck."}
  ],
  "defs": {"freq": {"call": "hz", "args": [{"option": "note"}]}},
  "clauses": [
    {"matches": {}, "graph": {"call": "rings", "args": [
      {"call": "mode", "args": [{"call": "impulse"}], "named": {"hz": {"ref": "freq"}, "level": 0.2, "fade": 3}}
    ], "named": {"seconds": 4}}}
  ]
}
```

- `format` is `mxs-synth` or `mxs-effect`, and `version` is a whole number; mxs refuses a format it does not know and a version newer than its own, naming both, and reads every older version.
- `options` lists the options as `Spec` declares them: `name`, `kind` (one of `note`, `hz`, `db`, `seconds`, `beats`, `float`, `integer`, `boolean`, or `name` with `names`), `default`, `range` as a pair whose `null` is unbounded, `required`, and `doc`.
- A value in a graph is a JSON number, `{"option": name}`, `{"name": name}` for a name such as a noise's colour, a JSON array for a list, `{"call": name, "args": [...], "named": {...}}` with either part left out when empty, or `{"ref": name}`.
- `defs` names values that a graph uses more than once; `{"ref": name}` stands for the value, a def may refer to defs before it, and a ref to a def that does not exist is refused.
  A def is a value only, with no parameters: the program that writes the file has functions of its own, as `mallet` below shows.
- `clauses` are `Clause`s: `matches` maps options to the numbers or names a note must have for the clause to apply, and `graph` is what it builds.
- Every error the vocabulary already reports, such as an unknown call or a call missing a named argument, is reported with the path to the value in the file, such as `clauses[0].graph.args[0].named.hz`.

## Writing one from Python or JavaScript

A program writes the file by building the tree, and the easiest way is a small class whose arithmetic writes calls instead of computing, as mx's `Expr` does in Elixir.
[`instruments-as-data/marimba.py`](instruments-as-data/marimba.py) does that in 140 lines of Python with nothing installed, half of them the options and the helpers, and a JavaScript version is the same, with functions in place of the operators JavaScript cannot overload.

Playing the file needs mxs in the program's process or beside it.
Recommendation: a Python package built with PyO3, which wraps `Engine` and adds `load_synth(path)`, and for JavaScript the WebAssembly build that mxs-00010 spikes, with the same `load_synth`.
Until then, a program can hand the file to mx, whose NIF already builds a `Graph`, once mx reads these files.

## The marimba

`marimba.py` writes mx's marimba sketch, `test/support/instruments/marimba.ex` in mx, as [`instruments-as-data/marimba.json`](instruments-as-data/marimba.json), following the defsynth line for line.
It names `freq`, `ring` and `strike` as defs, and writes `mallet`, a `defsoundp` in mx, as a Python function, so its nine uses are expanded into the file.

A throwaway test loaded that file and the graph mx traces from the defsynth, written as JSON from `Mx.Instrument.encode`, into `Graph`s: the two were equal, and four notes from C2 at vel 0.3 to C6 at vel 1 rendered the same samples from both.
The file is 10.7 KB written compactly, 0.4 KB of it the defs; the traced graph, with no defs, is 15 KB.

## What the format cannot express

The format is `Graph` written down, so it can express what `Graph` can, and these a synth written in Rust can and it cannot:

- Anything per sample that the vocabulary has no call for: a new filter, a physical model or an oscillator mxs does not have needs Rust, and then a call added to the vocabulary for files to use.
- Loops and functions: a file holds the expanded tree, so a synth of many partials grows with them; the writing program loops, and `defs` keep shared values from repeating.
- Data beyond numbers: the piano and the other instruments measured as figures read `.mxf` tables of partials, and no call reads a table or a sample, so they stay in Rust; a `table` value in a later version would let a file carry one.
- Feedback: no call feeds a sound back into itself, so a delay that feeds back, the loop mxs-00015 left open, cannot be written.
- Options are read when the note starts, except those a call follows while the note sounds, such as a filter's `hz`; a Rust synth can read its options as it likes.
- A clause matches exact values of options; a range of notes is written with `select` inside one clause.

## Recommendations, and their tickets

- Load a synth or an effect from a JSON file in the format above, with the version check and errors that name a path in the file: drafted as mxs-00044.
- A Python package that plays mxs, with `load_synth`: drafted as mxs-00045.
- JavaScript through WebAssembly: mxs-00010 already spikes playing in the browser, and its notes should cover `load_synth`, so I drafted no ticket of its own.
- A `table` value, so a file can carry measured partials: not drafted, as no one has asked to load a figured instrument from a file; I'm not sure it is worth the format's second version.
