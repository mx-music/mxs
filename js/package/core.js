// The engine of one WebAssembly instance of mxs: requests in JSON, tables and sound files from their bytes, and blocks of samples.
// This file is also the audio worklet's module, which plays an engine in the audio thread; a worklet has no TextEncoder or TextDecoder, so text is coded here.

/** An error mxs reports, carrying its message. */
export class MxsError extends Error {
  constructor(message) {
    super(message);
    this.name = "MxsError";
  }
}

function encode(text) {
  const bytes = [];
  for (const char of text) {
    const c = char.codePointAt(0);
    if (c < 0x80) bytes.push(c);
    else if (c < 0x800) bytes.push(0xc0 | (c >> 6), 0x80 | (c & 63));
    else if (c < 0x10000) bytes.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
    else bytes.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
  }
  return new Uint8Array(bytes);
}

function decode(bytes) {
  const codes = [];
  for (let i = 0; i < bytes.length; ) {
    const b = bytes[i++];
    if (b < 0x80) codes.push(b);
    else if (b < 0xe0) codes.push(((b & 31) << 6) | (bytes[i++] & 63));
    else if (b < 0xf0) codes.push(((b & 15) << 12) | ((bytes[i++] & 63) << 6) | (bytes[i++] & 63));
    else codes.push(((b & 7) << 18) | ((bytes[i++] & 63) << 12) | ((bytes[i++] & 63) << 6) | (bytes[i++] & 63));
  }
  let text = "";
  for (let i = 0; i < codes.length; i += 8192) text += String.fromCodePoint(...codes.slice(i, i + 8192));
  return text;
}

/** One engine in a WebAssembly instance of mxs. */
export class Core {
  constructor(instance, rate, voices = 0) {
    this.x = instance.exports;
    this.rate = rate;
    this.x.start(rate, voices);
  }

  // A panic reaches JavaScript as unreachable code, so its message is read back from the instance.
  run(f) {
    try {
      return f();
    } catch (error) {
      if (!(error instanceof WebAssembly.RuntimeError)) throw error;
      throw new MxsError(`mxs failed: ${this.text(this.x.panic_message())}`);
    }
  }

  text(at) {
    const length = new DataView(this.x.memory.buffer).getUint32(at, true);
    return decode(new Uint8Array(this.x.memory.buffer, at + 4, length));
  }

  write(...parts) {
    const length = parts.reduce((sum, part) => sum + part.length, 0);
    const at = this.x.room(length);
    const room = new Uint8Array(this.x.memory.buffer, at, length);
    let offset = 0;
    for (const part of parts) {
      room.set(part, offset);
      offset += part.length;
    }
  }

  answer(at) {
    const { ok, error } = JSON.parse(this.text(at));
    if (error !== undefined) throw new MxsError(error);
    return ok;
  }

  /** The answer to a request, or the MxsError mxs gives. */
  call(request) {
    return this.run(() => {
      const bytes = encode(JSON.stringify(request));
      this.write(bytes);
      return this.answer(this.x.call(bytes.length));
    });
  }

  /** Loads the table in the file named, such as piano.mxf, from its bytes. */
  loadTable(file, bytes) {
    return this.run(() => {
      const name = encode(file);
      this.write(name, new Uint8Array(bytes));
      return this.answer(this.x.load_table(name.length));
    });
  }

  /** Loads a sound file from its bytes, and gives the number a sample plays it by. */
  loadSample(name, bytes) {
    return this.run(() => {
      const shown = encode(name);
      this.write(shown, new Uint8Array(bytes));
      return this.answer(this.x.load_sample(shown.length));
    });
  }

  /** The next `frames` frames, the left channel and then the right, in the instance's memory until its next call. */
  render(frames) {
    return this.run(() => {
      const at = this.x.render(frames);
      return new Float32Array(this.x.memory.buffer, at, frames * 2);
    });
  }
}

if (typeof registerProcessor === "function") {
  registerProcessor(
    "mxs",
    class extends AudioWorkletProcessor {
      constructor({ processorOptions: { module, voices } }) {
        super();
        this.core = new Core(new WebAssembly.Instance(module, {}), sampleRate, voices);
        this.port.onmessage = ({ data: { id, call, table, sample } }) => {
          try {
            let ok;
            if (call) ok = this.core.call(call);
            else if (table) ok = this.core.loadTable(table.file, table.bytes);
            else ok = this.core.loadSample(sample.name, sample.bytes);
            this.port.postMessage({ id, ok });
          } catch (error) {
            this.port.postMessage({ id, error: error.message });
          }
        };
      }

      process(_inputs, outputs) {
        const [left, right] = outputs[0];
        const frames = left.length;
        const block = this.core.render(frames);
        left.set(block.subarray(0, frames));
        if (right) right.set(block.subarray(frames));
        return true;
      }
    },
  );
}
