// The bytes of a WAV file of a decaying 330 Hz tone, a quarter of a second long, as tests/webassembly makes it.
window.tone = () => {
  const frames = 12000;
  const bytes = new DataView(new ArrayBuffer(44 + frames * 2));
  const text = (at, s) => [...s].forEach((c, i) => bytes.setUint8(at + i, c.charCodeAt(0)));
  text(0, "RIFF");
  bytes.setUint32(4, 36 + frames * 2, true);
  text(8, "WAVEfmt ");
  bytes.setUint32(16, 16, true);
  bytes.setUint16(20, 1, true);
  bytes.setUint16(22, 1, true);
  bytes.setUint32(24, 48000, true);
  bytes.setUint32(28, 96000, true);
  bytes.setUint16(32, 2, true);
  bytes.setUint16(34, 16, true);
  text(36, "data");
  bytes.setUint32(40, frames * 2, true);
  for (let i = 0; i < frames; i++) {
    const t = i / 48000;
    bytes.setInt16(44 + i * 2, Math.trunc(Math.sin(2 * Math.PI * 330 * t) * Math.exp(-12 * t) * 16000), true);
  }
  return bytes.buffer;
};

// The loudest sample of a render, left or right.
window.peak = ([left, right]) => {
  let loudest = 0;
  for (const x of left) loudest = Math.max(loudest, Math.abs(x));
  for (const x of right) loudest = Math.max(loudest, Math.abs(x));
  return loudest;
};
