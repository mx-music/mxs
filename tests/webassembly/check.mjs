// Prints the phrase's levels rendered by the WebAssembly module given, one to a line, or the panic that stopped it: node check.mjs module.wasm
import { readFileSync } from "node:fs";

const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
const x = instance.exports;
try {
  const at = x.render();
  console.log(Array.from(new Float64Array(x.memory.buffer, at, 40)).join("\n"));
} catch (error) {
  const at = x.panic_message();
  const length = new DataView(x.memory.buffer).getUint32(at, true);
  console.log("panic: " + new TextDecoder().decode(new Uint8Array(x.memory.buffer, at + 4, length)));
  process.exit(1);
}
