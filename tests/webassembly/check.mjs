// Prints the phrase's levels rendered by the WebAssembly module given, one to a line, or the panic that stopped it: node check.mjs module.wasm
// Given a folder and files after the module, it first hands the module each table, as a page does with the tables it fetched: node check.mjs module.wasm folder piano.mxf
import { readFileSync } from "node:fs";

const { instance } = await WebAssembly.instantiate(readFileSync(process.argv[2]), {});
const x = instance.exports;
const [folder, ...tables] = process.argv.slice(3);
for (const file of tables) {
  const name = new TextEncoder().encode(file);
  const bytes = readFileSync(`${folder}/${file}`);
  const at = x.room(name.length + bytes.length);
  const room = new Uint8Array(x.memory.buffer, at, name.length + bytes.length);
  room.set(name);
  room.set(bytes, name.length);
  if (x.load_table(name.length) !== 0) {
    console.log(`the module refused the table ${file}`);
    process.exit(1);
  }
}
try {
  const at = x.render();
  console.log(Array.from(new Float64Array(x.memory.buffer, at, 40)).join("\n"));
} catch (error) {
  const at = x.panic_message();
  const length = new DataView(x.memory.buffer).getUint32(at, true);
  console.log("panic: " + new TextDecoder().decode(new Uint8Array(x.memory.buffer, at + 4, length)));
  process.exit(1);
}
