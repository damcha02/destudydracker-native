// Stage 17: dependency-free PNG utilities for the visual-parity comparison (Node only; no npm
// packages). Decodes 8-bit RGB/RGBA non-interlaced PNGs (what Chrome and GDI+ emit), and can
//   side  <a.png> <b.png> <out.png> [x y w h [scale]]   crop the same region from both, upscale
//                                                        (nearest), and place them side by side
//                                                        with a diff strip underneath
//   pair  <a.png> <b.png> <out.png>                     production | native, unmodified
//   diff  <a.png> <b.png>                               print whole-image mean abs difference
//   tiles <a.png> <b.png> [n]                           mean abs difference per n x n tile grid
import { readFileSync, writeFileSync } from "node:fs";
import { inflateSync, deflateSync } from "node:zlib";

export function decodePNG(path) {
  const buf = readFileSync(path);
  let pos = 8;
  let width = 0, height = 0, bitDepth = 0, colorType = 0, interlace = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0); height = data.readUInt32BE(4);
      bitDepth = data[8]; colorType = data[9]; interlace = data[12];
    } else if (type === "IDAT") idat.push(data);
    else if (type === "IEND") break;
    pos += 12 + len;
  }
  if (bitDepth !== 8 || interlace !== 0 || (colorType !== 2 && colorType !== 6)) throw new Error(`unsupported PNG (depth ${bitDepth}, color ${colorType}, interlace ${interlace})`);
  const bpp = colorType === 6 ? 4 : 3;
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * bpp;
  const out = Buffer.alloc(width * height * 4);
  let prev = Buffer.alloc(stride);
  let rp = 0;
  for (let y = 0; y < height; y++) {
    const ft = raw[rp++];
    const line = Buffer.from(raw.subarray(rp, rp + stride));
    rp += stride;
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? line[x - bpp] : 0, b = prev[x], c = x >= bpp ? prev[x - bpp] : 0;
      let v = line[x];
      if (ft === 1) v += a;
      else if (ft === 2) v += b;
      else if (ft === 3) v += (a + b) >> 1;
      else if (ft === 4) { const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c); v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c; }
      line[x] = v & 255;
    }
    for (let x = 0; x < width; x++) {
      out[(y * width + x) * 4] = line[x * bpp];
      out[(y * width + x) * 4 + 1] = line[x * bpp + 1];
      out[(y * width + x) * 4 + 2] = line[x * bpp + 2];
      out[(y * width + x) * 4 + 3] = bpp === 4 ? line[x * bpp + 3] : 255;
    }
    prev = line;
  }
  return { width, height, data: out };
}

const crcTable = (() => { const t = new Uint32Array(256); for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } return t; })();
const crc32 = (b) => { let c = 0xffffffff; for (const x of b) c = crcTable[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
export function encodePNG(img, path) {
  const { width, height, data } = img;
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) { raw[y * (width * 4 + 1)] = 0; data.copy(raw, y * (width * 4 + 1) + 1, y * width * 4, (y + 1) * width * 4); }
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(width, 0); ihdr.writeUInt32BE(height, 4); ihdr[8] = 8; ihdr[9] = 6;
  writeFileSync(path, Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]));
}

export function crop(img, x, y, w, h, scale = 1) {
  const out = { width: w * scale, height: h * scale, data: Buffer.alloc(w * scale * h * scale * 4, 255) };
  for (let yy = 0; yy < h * scale; yy++) for (let xx = 0; xx < w * scale; xx++) {
    const sx = x + Math.floor(xx / scale), sy = y + Math.floor(yy / scale);
    const o = (yy * out.width + xx) * 4;
    if (sx < 0 || sy < 0 || sx >= img.width || sy >= img.height) { out.data[o] = 40; out.data[o + 1] = 0; out.data[o + 2] = 40; continue; }
    img.data.copy(out.data, o, (sy * img.width + sx) * 4, (sy * img.width + sx) * 4 + 4);
  }
  return out;
}

export function diffImage(a, b) {
  const w = Math.min(a.width, b.width), h = Math.min(a.height, b.height);
  const out = { width: w, height: h, data: Buffer.alloc(w * h * 4, 255) };
  let sum = 0;
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const ia = (y * a.width + x) * 4, ib = (y * b.width + x) * 4, o = (y * w + x) * 4;
    const d = (Math.abs(a.data[ia] - b.data[ib]) + Math.abs(a.data[ia + 1] - b.data[ib + 1]) + Math.abs(a.data[ia + 2] - b.data[ib + 2])) / 3;
    sum += d;
    const v = Math.min(255, d * 3);
    out.data[o] = v; out.data[o + 1] = v * 0.4; out.data[o + 2] = 255 - v; // blue = same, red = different
  }
  return { img: out, mean: sum / (w * h) };
}

function stack(images, vertical = false, gap = 6) {
  const w = vertical ? Math.max(...images.map((i) => i.width)) : images.reduce((s, i) => s + i.width, 0) + gap * (images.length - 1);
  const h = vertical ? images.reduce((s, i) => s + i.height, 0) + gap * (images.length - 1) : Math.max(...images.map((i) => i.height));
  const out = { width: w, height: h, data: Buffer.alloc(w * h * 4, 255) };
  let off = 0;
  for (const im of images) {
    for (let y = 0; y < im.height; y++) for (let x = 0; x < im.width; x++) {
      const ox = vertical ? x : off + x, oy = vertical ? off + y : y;
      im.data.copy(out.data, (oy * w + ox) * 4, (y * im.width + x) * 4, (y * im.width + x) * 4 + 4);
    }
    off += (vertical ? im.height : im.width) + gap;
  }
  return out;
}

const [cmd, pa, pb, ...rest] = process.argv.slice(2);
if (cmd === "side") {
  const a = decodePNG(pa), b = decodePNG(pb);
  const out = rest[0];
  const [x, y, w, h, scale] = rest.slice(1).map(Number);
  const region = (img) => (w ? crop(img, x, y, w, h, scale || 1) : img);
  const ra = region(a), rb = region(b);
  const { img: d, mean } = diffImage(ra, rb);
  encodePNG(stack([stack([ra, rb]), d], true), out);
  console.log(`side-by-side ${out}  region mean abs diff = ${mean.toFixed(2)}`);
} else if (cmd === "pair") {
  // production | native, no diff strip (what is committed under docs/stage17-screenshots)
  const a = decodePNG(pa), b = decodePNG(pb);
  encodePNG(stack([a, b]), rest[0]);
  console.log(`pair ${rest[0]} ${a.width}+${b.width} x ${a.height}`);
} else if (cmd === "diff") {
  const a = decodePNG(pa), b = decodePNG(pb);
  console.log(`sizes: ${a.width}x${a.height} vs ${b.width}x${b.height}; mean abs diff = ${diffImage(a, b).mean.toFixed(3)}`);
} else if (cmd === "tiles") {
  const a = decodePNG(pa), b = decodePNG(pb);
  const n = Number(rest[0] ?? 8);
  const w = Math.min(a.width, b.width), h = Math.min(a.height, b.height);
  for (let ty = 0; ty < n; ty++) {
    const row = [];
    for (let tx = 0; tx < n; tx++) {
      const x0 = Math.floor((tx * w) / n), y0 = Math.floor((ty * h) / n), x1 = Math.floor(((tx + 1) * w) / n), y1 = Math.floor(((ty + 1) * h) / n);
      row.push(diffImage(crop(a, x0, y0, x1 - x0, y1 - y0), crop(b, x0, y0, x1 - x0, y1 - y0)).mean.toFixed(1).padStart(5));
    }
    console.log(row.join(" "));
  }
}
