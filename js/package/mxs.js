// mxs on a web page: its synths and effects played live in an audio worklet, or rendered offline into arrays of samples.
// Fifteen synths play measured tables, which an engine fetches the first time the page plays one of them.

import { Core, MxsError } from "./core.js";

export { MxsError };

const modules = new Map();
const fetched = new Map();

function compile(url) {
  const key = String(url);
  if (!modules.has(key)) {
    const compiled = WebAssembly.compileStreaming
      ? WebAssembly.compileStreaming(fetch(key))
      : fetch(key)
          .then((response) => response.arrayBuffer())
          .then((bytes) => WebAssembly.compile(bytes));
    modules.set(key, compiled);
  }
  return modules.get(key);
}

// Each table is fetched once for every engine on the page.
function table(url) {
  if (!fetched.has(url)) {
    const bytes = fetch(url).then((response) => {
      if (!response.ok) throw new MxsError(`could not fetch ${url}: ${response.status} ${response.statusText}`);
      return response.arrayBuffer();
    });
    fetched.set(url, bytes);
    bytes.catch(() => fetched.delete(url));
  }
  return fetched.get(url);
}

function defaults(options) {
  return {
    voices: 0,
    wasm: new URL("mxs.wasm", import.meta.url),
    tables: new URL("tables/", import.meta.url),
    ...options,
  };
}

/** What a live and an offline engine share: playing notes and samples, the catalog, and loading synths, effects, samples and tables. */
class Engine extends EventTarget {
  #tables;
  #catalog;
  #loaded = new Set();
  #loading = new Map();
  #next = 1;

  constructor(tables) {
    super();
    this.#tables = tables;
  }

  /** mxs's synths, and those loaded, each with its name, doc, options and the tables it plays. */
  synths() {
    return this.request({ call: { op: "synths" } });
  }

  /** mxs's effects, and those loaded, each with its name and options. */
  effects() {
    return this.request({ call: { op: "effects" } });
  }

  async #needs(synth) {
    this.#catalog ??= this.synths().then((synths) => new Map(synths.map((s) => [s.name, s.tables])));
    return (await this.#catalog).get(synth) ?? [];
  }

  /**
   * Loads the tables the synth named plays, fetching each the first time any engine on the page needs it.
   * A note loads them by itself; once they are loaded, the engine dispatches a "loaded" event whose detail names the synth and the tables.
   */
  loadTables(synth) {
    if (!this.#loading.has(synth)) {
      const loading = this.#load(synth);
      this.#loading.set(synth, loading);
      loading.catch(() => this.#loading.delete(synth));
    }
    return this.#loading.get(synth);
  }

  async #load(synth) {
    const files = (await this.#needs(synth)).filter((file) => !this.#loaded.has(file));
    if (files.length === 0) return;
    await Promise.all(
      files.map(async (file) => {
        const bytes = await table(new URL(file, this.#tables).href);
        await this.request({ table: { file, bytes } });
        this.#loaded.add(file);
      }),
    );
    this.dispatchEvent(new CustomEvent("loaded", { detail: { synth, tables: files } }));
  }

  /**
   * Plays a note of `synth` `after` seconds from now with its options by name, and gives the number later calls reach it by.
   * The first note of a synth that plays tables waits for them, and plays `after` seconds from when they are loaded.
   * A held note sounds until released or stopped.
   */
  async note(after, synth, options = {}, { id, chain, held } = {}) {
    await this.loadTables(synth);
    id ??= this.#next++;
    await this.play([{ kind: "note", after, synth, options, id, chain, held }]);
    return id;
  }

  /** Plays the sound file `loadSample` gave `sample` for, `after` seconds from now, with the sample player's options, and gives the number later calls reach it by. */
  async sample(after, sample, options = {}, { id, chain } = {}) {
    id ??= this.#next++;
    await this.play([{ kind: "sample", after, sample, options, id, chain }]);
    return id;
  }

  /** Changes options of the sounds given, `after` seconds from now. */
  set(after, ids, options) {
    return this.play([{ kind: "set", after, ids: [ids].flat(), options }]);
  }

  /** Lets the sounds given go, `after` seconds from now, as keys let up. */
  release(after, ids) {
    return this.play([{ kind: "release", after, ids: [ids].flat() }]);
  }

  /** Ends the sounds given, `after` seconds from now, fading them over 10 ms. */
  stop(after, ids) {
    return this.play([{ kind: "stop", after, ids: [ids].flat() }]);
  }

  /** Plays events as mxs takes them, all or none: each has a kind, note, sample, set, release or stop, and `after`, in seconds from now. */
  play(events) {
    return this.request({ call: { op: "play", events } });
  }

  /** Starts a chain of effects, each a name or a [name, options] pair, and gives the number a note's `chain` plays into it by. */
  startEffects(effects) {
    const chain = effects.map((effect) => (typeof effect === "string" ? [effect] : effect));
    return this.request({ call: { op: "start_effects", effects: chain } });
  }

  /** Registers the synth in the text, or object, of an mxs-synth file, and gives its name. */
  async loadSynth(file) {
    const text = typeof file === "string" ? file : JSON.stringify(file);
    const name = await this.request({ call: { op: "load_synth", text } });
    this.#catalog = undefined;
    return name;
  }

  /** Registers the effect in the text, or object, of an mxs-effect file, and gives its name. */
  loadEffect(file) {
    const text = typeof file === "string" ? file : JSON.stringify(file);
    return this.request({ call: { op: "load_effect", text } });
  }

  /** Loads a FLAC, WAV or AIFF file from its bytes, and gives the number `sample` plays it by. */
  loadSample(bytes, name = "sample") {
    return this.request({ sample: { name, bytes } });
  }
}

/** An engine playing in an audio worklet. */
export class Live extends Engine {
  #asked = 0;
  #waiting = new Map();
  #owned;

  constructor(context, node, tables, owned) {
    super(tables);
    this.context = context;
    this.node = node;
    this.#owned = owned;
    node.port.onmessage = ({ data: { id, ok, error } }) => {
      const { resolve, reject } = this.#waiting.get(id);
      this.#waiting.delete(id);
      if (error === undefined) resolve(ok);
      else reject(new MxsError(error));
    };
  }

  request(message) {
    return new Promise((resolve, reject) => {
      const id = this.#asked++;
      this.#waiting.set(id, { resolve, reject });
      this.node.port.postMessage({ id, ...message });
    });
  }

  /** Stops the engine, and closes its audio context when it made it. */
  async close() {
    this.node.disconnect();
    if (this.#owned) await this.context.close();
  }
}

/** An engine that renders when asked, on the page's own thread. */
export class Offline extends Engine {
  constructor(core, tables) {
    super(tables);
    this.core = core;
  }

  async request({ call, table, sample }) {
    if (call) return this.core.call(call);
    if (table) return this.core.loadTable(table.file, table.bytes);
    return this.core.loadSample(sample.name, sample.bytes);
  }

  /** The rate the engine renders at, in frames a second. */
  get rate() {
    return this.core.rate;
  }

  /** The next `seconds` of sound, as two arrays of samples, left and right. */
  render(seconds) {
    const frames = Math.round(seconds * this.core.rate);
    const block = this.core.render(frames);
    return [block.slice(0, frames), block.slice(frames)];
  }
}

/**
 * Starts an engine that plays through an audio context, a new one unless `context` is given, connected to its destination.
 * Call it from a click or a key press, as browsers start sound only then.
 * Options: `context`, `voices` sounding at once, and the URLs of `wasm`, mxs.wasm, and of the `tables` folder, which default to those beside this file.
 */
export async function live(options = {}) {
  const { context, voices, wasm, tables } = defaults(options);
  const audio = context ?? new AudioContext({ latencyHint: "interactive" });
  const module = await compile(wasm);
  await audio.audioWorklet.addModule(new URL("core.js", import.meta.url));
  const node = new AudioWorkletNode(audio, "mxs", {
    numberOfInputs: 0,
    outputChannelCount: [2],
    processorOptions: { module, voices },
  });
  node.connect(audio.destination);
  await audio.resume();
  return new Live(audio, node, tables, !context);
}

/**
 * Starts an engine that renders offline at `rate` frames a second, 48000 unless given.
 * Options: `rate`, `voices` sounding at once, and the URLs of `wasm` and of the `tables` folder.
 */
export async function offline(options = {}) {
  const { rate = 48000, voices, wasm, tables } = defaults(options);
  const instance = await WebAssembly.instantiate(await compile(wasm), {});
  return new Offline(new Core(instance, rate, voices), tables);
}
