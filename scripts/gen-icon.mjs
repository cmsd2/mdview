// Generates a simple 1024x1024 source PNG (app-icon.png) with no external
// deps — uses Node's built-in zlib to deflate raw RGBA pixels into a PNG.
// `npm run tauri icon app-icon.png` then derives the per-platform icon set.

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const SIZE = 1024;

function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const body = Buffer.concat([typeBuf, data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([len, body, crc]);
}

// Build RGBA pixels: a document glyph on a blue rounded field.
const px = Buffer.alloc(SIZE * SIZE * 4);
const bg = [13, 17, 23, 255]; // dark page
const accent = [88, 166, 255, 255]; // blue
const paper = [240, 246, 252, 255];

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    let color = bg;
    // rounded accent square
    const m = 96;
    if (x > m && x < SIZE - m && y > m && y < SIZE - m) color = accent;
    // white "page" in the middle
    const pm = 280;
    if (x > pm && x < SIZE - pm && y > pm - 60 && y < SIZE - pm + 60) color = paper;
    // text lines on the page
    const lineX = x > pm + 60 && x < SIZE - pm - 60;
    const onLine = [380, 470, 560, 650].some((ly) => y > ly && y < ly + 34);
    if (lineX && onLine) color = accent;

    const i = (y * SIZE + x) * 4;
    px[i] = color[0];
    px[i + 1] = color[1];
    px[i + 2] = color[2];
    px[i + 3] = color[3];
  }
}

// Add the per-scanline filter byte (0 = none).
const raw = Buffer.alloc(SIZE * (SIZE * 4 + 1));
for (let y = 0; y < SIZE; y++) {
  raw[y * (SIZE * 4 + 1)] = 0;
  px.copy(raw, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}

const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
const png = Buffer.concat([
  sig,
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync("app-icon.png", png);
console.log("wrote app-icon.png", png.length, "bytes");
