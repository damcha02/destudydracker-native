// Stage 19: derives the native Sakura assets from production's own files (desktop/public, read
// only). No npm packages: a small GIF (LZW) decoder and an area-average PNG downscaler.
//
//   node scripts/stage19-sakura-assets.mjs
//
// Writes assets/sakura/:
//   petal-0.png, petal-1.png   the two production blossom images (840 px wide originals), scaled to
//                              fit 72x72 (2x the largest 36 px petal) so the renderer never samples
//                              an 840 px texture down to 16-36 px every frame
//   leaves-00.png .. leaves-19.png
//                              the 20 frames of production's animated 200x200 texture GIF
//                              (130 ms per frame, disposal "restore to background", so every frame
//                              is complete on its own transparent canvas), with production's
//                              `body::after { opacity: 0.14 }` baked into the alpha channel: the same
//                              pixels, without a full-window opacity layer on every frame
import { readFileSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { decodePNG, encodePNG } from "./visual-parity/imgtool.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const pub = join(here, "..", "..", "desktop", "public");
const out = join(here, "..", "assets", "sakura");
mkdirSync(out, { recursive: true });

// --- petals ---------------------------------------------------------------------------------------
const PETALS = [
  "196-1963264_cherry-blossom-free-icon-flowey-i-am-not-cute.png",
  "flower-flowers-sakura-cherryblossom-tumblr-kawaii-ftest-sakura-flower-png-kawaii-115628518804pkqn9p63w.png",
];
function downscale(img, box) {
  const scale = Math.min(box / img.width, box / img.height);
  const w = Math.max(1, Math.round(img.width * scale));
  const h = Math.max(1, Math.round(img.height * scale));
  const data = Buffer.alloc(w * h * 4);
  for (let y = 0; y < h; y++) {
    const y0 = (y * img.height) / h, y1 = ((y + 1) * img.height) / h;
    for (let x = 0; x < w; x++) {
      const x0 = (x * img.width) / w, x1 = ((x + 1) * img.width) / w;
      let r = 0, g = 0, b = 0, a = 0, area = 0;
      for (let sy = Math.floor(y0); sy < Math.ceil(y1); sy++) {
        const wy = Math.min(sy + 1, y1) - Math.max(sy, y0);
        for (let sx = Math.floor(x0); sx < Math.ceil(x1); sx++) {
          const wx = Math.min(sx + 1, x1) - Math.max(sx, x0);
          const k = wx * wy;
          const i = (sy * img.width + sx) * 4;
          const al = img.data[i + 3] / 255;
          // premultiplied, so transparent pixels do not bleed their (meaningless) colour
          r += img.data[i] * al * k; g += img.data[i + 1] * al * k; b += img.data[i + 2] * al * k; a += al * k;
          area += k;
        }
      }
      const o = (y * w + x) * 4;
      const al = a / area;
      data[o] = al > 0 ? Math.round(r / a) : 0;
      data[o + 1] = al > 0 ? Math.round(g / a) : 0;
      data[o + 2] = al > 0 ? Math.round(b / a) : 0;
      data[o + 3] = Math.round(al * 255);
    }
  }
  return { width: w, height: h, data };
}
PETALS.forEach((name, i) => {
  const img = decodePNG(join(pub, name));
  const small = downscale(img, 72);
  encodePNG(small, join(out, `petal-${i}.png`));
  console.log(`petal-${i}.png ${small.width}x${small.height} (from ${img.width}x${img.height} ${name})`);
});

// --- GIF ------------------------------------------------------------------------------------------
function lzwDecode(minCodeSize, bytes, pixelCount) {
  const clear = 1 << minCodeSize, eoi = clear + 1;
  let codeSize = minCodeSize + 1, dict = [], next = 0;
  const reset = () => { dict = []; for (let i = 0; i < clear; i++) dict[i] = [i]; dict[clear] = []; dict[eoi] = null; next = eoi + 1; codeSize = minCodeSize + 1; };
  reset();
  const outIdx = [];
  let bit = 0, prev = null;
  const read = () => {
    let code = 0;
    for (let i = 0; i < codeSize; i++) {
      const byte = bytes[(bit + i) >> 3];
      if ((byte >> ((bit + i) & 7)) & 1) code |= 1 << i;
    }
    bit += codeSize;
    return code;
  };
  while (bit + codeSize <= bytes.length * 8 && outIdx.length < pixelCount) {
    const code = read();
    if (code === clear) { reset(); prev = null; continue; }
    if (code === eoi) break;
    let entry;
    if (code < next && dict[code]) entry = dict[code];
    else if (prev) entry = [...prev, prev[0]];
    else break;
    for (const v of entry) outIdx.push(v);
    if (prev && next < 4096) { dict[next++] = [...prev, entry[0]]; if (next === 1 << codeSize && codeSize < 12) codeSize++; }
    prev = entry;
  }
  return outIdx;
}

const gif = readFileSync(join(pub, "sakura-leaves-ezgif.com-gif-maker.gif"));
const W = gif.readUInt16LE(6), H = gif.readUInt16LE(8);
let pos = 13;
const gflags = gif[10];
let globalCT = null;
if (gflags & 0x80) { const n = 1 << ((gflags & 7) + 1); globalCT = gif.subarray(pos, pos + 3 * n); pos += 3 * n; }
let gce = null, frame = 0;
while (pos < gif.length) {
  const t = gif[pos];
  if (t === 0x21) {
    if (gif[pos + 1] === 0xf9) gce = { transparent: gif[pos + 3] & 1, index: gif[pos + 6], delay: gif.readUInt16LE(pos + 4) * 10 };
    pos += 2;
    while (gif[pos]) pos += gif[pos] + 1;
    pos++;
  } else if (t === 0x2c) {
    const fx = gif.readUInt16LE(pos + 1), fy = gif.readUInt16LE(pos + 3), fw = gif.readUInt16LE(pos + 5), fh = gif.readUInt16LE(pos + 7), pk = gif[pos + 9];
    pos += 10;
    let ct = globalCT;
    if (pk & 0x80) { const n = 1 << ((pk & 7) + 1); ct = gif.subarray(pos, pos + 3 * n); pos += 3 * n; }
    if (pk & 0x40) throw new Error("interlaced GIF frames are not supported");
    const minCode = gif[pos++];
    const chunks = [];
    while (gif[pos]) { chunks.push(gif.subarray(pos + 1, pos + 1 + gif[pos])); pos += gif[pos] + 1; }
    pos++;
    const idx = lzwDecode(minCode, Buffer.concat(chunks), fw * fh);
    // Disposal 2 (restore to background = transparent) on every frame: each frame stands alone.
    const data = Buffer.alloc(W * H * 4);
    for (let y = 0; y < fh; y++) for (let x = 0; x < fw; x++) {
      const v = idx[y * fw + x];
      if (v === undefined || (gce?.transparent && v === gce.index)) continue;
      const o = ((fy + y) * W + fx + x) * 4;
      data[o] = ct[v * 3]; data[o + 1] = ct[v * 3 + 1]; data[o + 2] = ct[v * 3 + 2]; data[o + 3] = Math.round(255 * 0.14);
    }
    const name = `leaves-${String(frame).padStart(2, "0")}.png`;
    encodePNG({ width: W, height: H, data }, join(out, name));
    console.log(`${name} ${W}x${H} delay=${gce?.delay}ms`);
    frame++;
    gce = null;
  } else if (t === 0x3b) break;
  else throw new Error(`unexpected GIF block 0x${t.toString(16)} at ${pos}`);
}
