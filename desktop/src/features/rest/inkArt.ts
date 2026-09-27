/**
 * Redraws an icon as a pen-and-ink illustration in black ink only: a brush outline round the shape, fine ink
 * lines along the picture's inner edges, and hatching / cross-hatching where it is dark (the darker, the denser),
 * with bare paper for the lights. No colour at all. Meant to be shown with `mix-blend-mode: multiply`.
 * Results are cached per icon.
 */
const R = 192;
const INK: [number, number, number] = [26, 24, 22];
const cache = new Map<string, string>();

function seeded(seed: number) {
  let state = seed | 0;
  return () => {
    state = (state + 0x6d2b79f5) | 0;
    let t = Math.imul(state ^ (state >>> 15), 1 | state);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function hash(text: string) {
  let value = 2166136261;
  for (let index = 0; index < text.length; index += 1) {
    value ^= text.charCodeAt(index);
    value = Math.imul(value, 16777619);
  }
  return value >>> 0;
}

function makeCanvas() {
  const canvas = document.createElement("canvas");
  canvas.width = R;
  canvas.height = R;
  return canvas;
}

export function inkArt(icon: string): string {
  const cached = cache.get(icon);
  if (cached) return cached;
  const random = seeded(hash(icon));

  const glyph = makeCanvas();
  const gctx = glyph.getContext("2d")!;
  gctx.textAlign = "center";
  gctx.textBaseline = "middle";
  gctx.font = `${R * 0.66}px "Noto Color Emoji", "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", sans-serif`;
  gctx.fillText(icon, R / 2, R * 0.5);
  const src = gctx.getImageData(0, 0, R, R).data;

  // the picture laid on white paper, as three channels, plus the silhouette
  const inside = new Uint8Array(R * R);
  const chan = [new Float32Array(R * R), new Float32Array(R * R), new Float32Array(R * R)];
  const tone = new Float32Array(R * R); // 0 = bare paper, 1 = darkest
  for (let index = 0; index < R * R; index += 1) {
    const at = index * 4;
    const alpha = src[at + 3] / 255;
    for (let c = 0; c < 3; c += 1) chan[c][index] = (src[at + c] / 255) * alpha + (1 - alpha);
    if (src[at + 3] >= 110) {
      inside[index] = 1;
      const light = 0.3 * chan[0][index] + 0.59 * chan[1][index] + 0.11 * chan[2][index];
      tone[index] = Math.max(0, Math.min(1, (1 - light) * 1.25 - 0.06));
    }
  }

  const ink = new Uint8Array(R * R); // 0..255 amount of ink at each pixel
  const mark = (x: number, y: number, amount: number) => {
    if (x < 0 || y < 0 || x >= R || y >= R) return;
    const index = y * R + x;
    if (amount > ink[index]) ink[index] = amount;
  };

  // hatching: diagonal lines where it is dark, crossed where it is darker, solid where it is darkest
  const period = 9;
  const lineWidth = 2.6;
  for (let y = 0; y < R; y += 1) {
    for (let x = 0; x < R; x += 1) {
      const index = y * R + x;
      if (!inside[index]) continue;
      const t = tone[index];
      if (t > 0.9) {
        mark(x, y, 235);
        continue;
      }
      const a = (x + y) % period;
      const b = (x - y + R * 4) % period;
      if (t > 0.22 && a < lineWidth) mark(x, y, 215);
      if (t > 0.55 && b < lineWidth) mark(x, y, 205);
      if (t > 0.74 && (x + y * 2) % 5 < 2) mark(x, y, 190);
    }
  }

  // inner edges: where colour or lightness jumps, draw a fine ink line
  for (let y = 1; y < R - 1; y += 1) {
    for (let x = 1; x < R - 1; x += 1) {
      const index = y * R + x;
      if (!inside[index]) continue;
      let magnitude = 0;
      for (let c = 0; c < 3; c += 1) {
        const p = chan[c];
        const gx = -p[index - R - 1] - 2 * p[index - 1] - p[index + R - 1] + p[index - R + 1] + 2 * p[index + 1] + p[index + R + 1];
        const gy = -p[index - R - 1] - 2 * p[index - R] - p[index - R + 1] + p[index + R - 1] + 2 * p[index + R] + p[index + R + 1];
        magnitude = Math.max(magnitude, Math.hypot(gx, gy));
      }
      if (magnitude > 0.75) {
        mark(x, y, 240);
        mark(x + 1, y, 220);
        mark(x, y + 1, 220);
      }
    }
  }

  // the brush outline round the whole shape: strong, a little uneven like a loaded brush
  const band = 4;
  for (let y = 0; y < R; y += 1) {
    for (let x = 0; x < R; x += 1) {
      const index = y * R + x;
      if (!inside[index]) continue;
      let edge = false;
      for (let dy = -band; dy <= band && !edge; dy += 1) {
        for (let dx = -band; dx <= band; dx += 1) {
          const nx = x + dx;
          const ny = y + dy;
          if (nx < 0 || ny < 0 || nx >= R || ny >= R || !inside[ny * R + nx]) {
            edge = true;
            break;
          }
        }
      }
      if (edge && random() > 0.05) mark(x, y, 250);
    }
  }

  const out = makeCanvas();
  const octx = out.getContext("2d")!;
  const image = octx.createImageData(R, R);
  for (let index = 0; index < R * R; index += 1) {
    if (!ink[index]) continue;
    image.data[index * 4] = INK[0];
    image.data[index * 4 + 1] = INK[1];
    image.data[index * 4 + 2] = INK[2];
    image.data[index * 4 + 3] = ink[index];
  }
  octx.putImageData(image, 0, 0);

  // a hair of bleed so the lines look inked, not pixel-drawn
  const soft = makeCanvas();
  const sctx = soft.getContext("2d")!;
  sctx.filter = "blur(0.7px)";
  sctx.drawImage(out, 0, 0);
  const url = soft.toDataURL("image/png");
  cache.set(icon, url);
  return url;
}
