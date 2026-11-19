// mxs in the audio thread: it starts an engine at the context's rate, takes notes as they are posted, and renders each block as the browser asks for it.
// Not every browser gives a worklet performance.now, so the load falls back to whole milliseconds, which are averaged over a second.
const now = () => (globalThis.performance ? performance.now() : Date.now());

class Mxs extends AudioWorkletProcessor {
  constructor({ processorOptions }) {
    super();
    this.x = new WebAssembly.Instance(processorOptions.module, {}).exports;
    this.x.start(sampleRate, processorOptions.voices);
    this.setSynth("piano");
    this.frames = 0;
    this.busy = 0;
    this.blocks = 0;
    this.worst = 0;
    this.port.onmessage = ({ data }) => {
      const at = this.frames / sampleRate;
      for (const [after, note, vel, sustain] of data.notes) this.x.note(at + after, note, vel, sustain);
      if (data.click !== undefined) this.click = data.click;
    };
  }

  setSynth(name) {
    // A worklet has no TextEncoder, and synth names are ASCII.
    const bytes = Array.from(name, (c) => c.charCodeAt(0));
    new Uint8Array(this.x.memory.buffer, this.x.synth_name(bytes.length), bytes.length).set(bytes);
  }

  process(_inputs, outputs) {
    const [left, right] = outputs[0], frames = left.length;
    const began = now();
    const block = new Float32Array(this.x.memory.buffer, this.x.render(frames), frames * 2);
    left.set(block.subarray(0, frames));
    if (right) right.set(block.subarray(frames));
    const took = now() - began;
    this.busy += took;
    this.worst = Math.max(this.worst, took / (1000 * frames / sampleRate));
    const first = this.click === undefined ? -1 : block.subarray(0, frames).findIndex((x) => Math.abs(x) > 0.001);
    if (first >= 0) {
      this.port.postMessage({ heard: currentTime + first / sampleRate - this.click });
      this.click = undefined;
    }
    this.frames += frames;
    if (++this.blocks * frames >= sampleRate) {
      this.port.postMessage({ load: this.busy / 1000 / (this.blocks * frames / sampleRate), worst: this.worst, seconds: this.frames / sampleRate });
      this.busy = 0;
      this.blocks = 0;
      this.worst = 0;
    }
    return true;
  }
}
registerProcessor("mxs", Mxs);
