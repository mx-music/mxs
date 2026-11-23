// The mxs package's tests, in headless Chromium and WebKit: node test/run.mjs, after build.sh.
// It imports Playwright from the path in PLAYWRIGHT, or "playwright", and runs WebKit from MXS_WEBKIT when that is set.

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const { chromium, webkit } = await import(process.env.PLAYWRIGHT ?? "playwright");
const root = fileURLToPath(new URL("../..", import.meta.url));
const types = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".json": "application/json" };

const server = createServer(async (request, response) => {
  const path = normalize(decodeURIComponent(new URL(request.url, "http://x").pathname));
  try {
    const body = await readFile(join(root, path));
    response.writeHead(200, { "content-type": types[extname(path)] ?? "application/octet-stream" });
    response.end(body);
  } catch {
    response.writeHead(404);
    response.end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;

// The phrase of tests/webassembly rendered natively: the level of each tenth of a second over 4 seconds.
const native = execFileSync("cargo", ["run", "--release", "--quiet", "--manifest-path", join(root, "tests/webassembly/Cargo.toml")], { encoding: "utf8" })
  .trim()
  .split("\n")
  .map(Number);

const tests = {
  async "a phrase rendered offline has the native render's level in every tenth of a second, within 0.01 dB"(page) {
    const levels = await page.evaluate(async () => {
      const engine = await mxs.offline();
      const chain = await engine.startEffects([["reverb", { decay_seconds: 2 }]]);
      const sample = await engine.loadSample(tone(), "tone.wav");
      for (const [i, note] of [60, 64, 67, 72].entries()) {
        await engine.note(i * 0.5, "piano", { note, sustain: 0.4 }, { chain });
        await engine.note(i * 0.5 + 0.25, "marimba", { note: note + 12 }, { chain });
      }
      await engine.sample(2.5, sample, {}, { chain });
      return Array.from({ length: 40 }, () => {
        const [left, right] = engine.render(0.1);
        let sum = 0;
        for (const x of left) sum += x * x;
        for (const x of right) sum += x * x;
        return 10 * Math.log10(sum / (left.length + right.length) + 1e-20);
      });
    });
    check(levels.length === native.length, `${levels.length} levels`);
    levels.forEach((level, tenth) => check(Math.abs(level - native[tenth]) <= 0.01, `at ${tenth / 10} s ${level} dB against ${native[tenth]} dB`));
  },

  async "a page that plays only the piano fetches the piano's tables and no other's, and says when they are loaded"(page, fetched) {
    const loaded = await page.evaluate(async () => {
      const engine = await mxs.offline();
      const said = [];
      engine.addEventListener("loaded", ({ detail }) => said.push(detail));
      await engine.note(0, "piano", { note: 60 });
      await engine.note(0.5, "piano", { note: 64 });
      return said;
    });
    check(JSON.stringify(fetched()) === JSON.stringify(["piano.mxf", "piano_pedal.mxf"]), `fetched ${fetched()}`);
    check(JSON.stringify(loaded) === JSON.stringify([{ synth: "piano", tables: ["piano.mxf", "piano_pedal.mxf"] }]), JSON.stringify(loaded));
  },

  async "a synth loaded from the marimba file plays"(page, fetched) {
    const loudest = await page.evaluate(async () => {
      const engine = await mxs.offline();
      const name = await engine.loadSynth(await (await fetch("/tests/marimba.json")).text());
      await engine.note(0, name, { note: 72 });
      return peak(engine.render(0.5));
    });
    check(loudest > 0.01, `the marimba peaks at ${loudest}`);
    check(fetched().length === 0, `fetched ${fetched()}`);
  },

  async "a sample loaded from fetched bytes plays"(page) {
    const loudest = await page.evaluate(async () => {
      const engine = await mxs.offline();
      const url = URL.createObjectURL(new Blob([tone()], { type: "audio/wav" }));
      const sample = await engine.loadSample(await (await fetch(url)).arrayBuffer(), "tone.wav");
      await engine.sample(0, sample);
      return peak(engine.render(0.2));
    });
    check(loudest > 0.1, `the sample peaks at ${loudest}`);
  },

  async "a note of a synth that does not exist raises an exception naming it"(page) {
    const raised = await page.evaluate(async () => {
      const engine = await mxs.offline();
      try {
        await engine.note(0, "nonesuch", { note: 60 });
        return "nothing";
      } catch (error) {
        return `${error.name}: ${error.message}`;
      }
    });
    check(raised === "MxsError: no synth named nonesuch", raised);
  },

  async "a live engine plays a piano note in its audio worklet"(page, fetched) {
    const loudest = await page.evaluate(async () => {
      const engine = await mxs.live();
      const analyser = engine.context.createAnalyser();
      engine.node.connect(analyser);
      await engine.note(0, "piano", { note: 60, vel: 0.8 });
      await new Promise((resolve) => setTimeout(resolve, 300));
      const wave = new Float32Array(analyser.fftSize);
      analyser.getFloatTimeDomainData(wave);
      await engine.close();
      return Math.max(...wave.map(Math.abs));
    });
    check(loudest > 0.01, `the worklet's output peaks at ${loudest}`);
    check(JSON.stringify(fetched()) === JSON.stringify(["piano.mxf", "piano_pedal.mxf"]), `fetched ${fetched()}`);
  },

  async "a live engine's error reaches the page as an exception"(page) {
    const raised = await page.evaluate(async () => {
      const engine = await mxs.live();
      try {
        await engine.note(0, "piano", { note: 60, nonesuch: 1 });
        return "nothing";
      } finally {
        await engine.close();
      }
    }).catch((error) => error.message);
    check(raised.includes("MxsError: piano has no option nonesuch"), raised);
  },

  async "the documentation's page plays a phrase on the piano when its button is pressed"(page) {
    await page.goto(`${origin}/js/package/example/index.html`);
    await page.click("#play");
    await page.waitForFunction(() => /^(Playing|mxs could not)/.test(document.querySelector("#said").textContent), null, { timeout: 15000 });
    const said = await page.textContent("#said");
    check(said.startsWith("Playing"), said);
  },
};

let failed = 0;

function check(ok, message) {
  if (!ok) throw new Error(message);
}

const browsers = [
  ["chromium", chromium, {}],
  ["webkit", webkit, process.env.MXS_WEBKIT ? { executablePath: process.env.MXS_WEBKIT } : {}],
];
for (const [name, type, options] of browsers) {
  const browser = await type.launch(options);
  for (const [title, test] of Object.entries(tests)) {
    const context = await browser.newContext();
    const page = await context.newPage();
    const tables = [];
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (path.startsWith("/js/package/tables/")) tables.push(path.slice("/js/package/tables/".length));
    });
    await page.goto(`${origin}/js/test/page.html`);
    await page.waitForFunction(() => window.ready);
    await page.addScriptTag({ path: fileURLToPath(new URL("helpers.js", import.meta.url)) });
    try {
      await test(page, () => [...tables].sort());
      console.log(`ok ${name}: ${title}`);
    } catch (error) {
      failed++;
      console.log(`FAILED ${name}: ${title}\n  ${error.message.split("\n")[0]}`);
    }
    await context.close();
  }
  await browser.close();
}
server.close();
console.log(failed ? `${failed} failed` : "all passed");
process.exit(failed ? 1 : 0);
