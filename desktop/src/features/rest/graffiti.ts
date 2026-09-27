/**
 * Turns an icon into a spray-painted stencil, the way street-art stencils actually look:
 *  - each part of the picture is sprayed in its own colour (the icon's colours, posterised and made
 *    punchy), with the bright highlights left as stippled bare wall;
 *  - a dark painted outline holds the shape together;
 *  - a faint, soft overspray in the picture's main colour hugs the edges;
 *  - drips run down from the painted areas in the local paint colour.
 * The image is meant to be shown with `mix-blend-mode: multiply`, so the brick and mortar show through the paint.
 * Results are cached per icon + colour.
 */
const R = 192; // working resolution
const cache = new Map<string, string>();

const BAYER = [
  [0, 32, 8, 40, 2, 34, 10, 42],
  [48, 16, 56, 24, 50, 18, 58, 26],
  [12, 44, 4, 36, 14, 46, 6, 38],
  [60, 28, 52, 20, 62, 30, 54, 22],
  [3, 35, 11, 43, 1, 33, 9, 41],
  [51, 19, 59, 27, 49, 17, 57, 25],
  [15, 47, 7, 39, 13, 45, 5, 37],
  [63, 31, 55, 23, 61, 29, 53, 21],
];

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

function hexToRgb(hex: string): [number, number, number] {
  const value = hex.replace("#", "");
  return [parseInt(value.slice(0, 2), 16), parseInt(value.slice(2, 4), 16), parseInt(value.slice(4, 6), 16)];
}

export function graffitiArt(icon: string, color: string): string {
  const key = `${icon}|${color}`;
  const cached = cache.get(key);
  if (cached) return cached;

  const random = seeded(hash(key));
  const [cr, cg, cb] = hexToRgb(color);
  const deep = [Math.round(cr * 0.62), Math.round(cg * 0.62), Math.round(cb * 0.62)];
  void deep;

  // 1) draw the picture, then read it back as tones
  const glyph = makeCanvas();
  const gctx = glyph.getContext("2d")!;
  gctx.textAlign = "center";
  gctx.textBaseline = "middle";
  gctx.font = `${R * 0.62}px "Noto Color Emoji", "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", sans-serif`;
  gctx.fillText(icon, R / 2, R * 0.46);
  const src = gctx.getImageData(0, 0, R, R).data;

  const inside = new Uint8Array(R * R); // the silhouette
  const paint = new Float32Array(R * R); // how much paint each pixel wants, 0..1
  const colour = new Uint8Array(R * R * 3); // the paint colour of each pixel
  let meanR = 0;
  let meanG = 0;
  let meanB = 0;
  let saturation = 0;
  let count = 0;
  for (let index = 0; index < R * R; index += 1) {
    const at = index * 4;
    if (src[at + 3] < 110) continue;
    inside[index] = 1;
    meanR += src[at];
    meanG += src[at + 1];
    meanB += src[at + 2];
    saturation += Math.max(src[at], src[at + 1], src[at + 2]) - Math.min(src[at], src[at + 1], src[at + 2]);
    count += 1;
  }
  const mono = count === 0 || saturation / count < 22; // plain text symbols: paint them in the spray colour
  const main: [number, number, number] = mono || count === 0
    ? [Math.round(cr * 0.85), Math.round(cg * 0.85), Math.round(cb * 0.85)]
    : [Math.round(meanR / count), Math.round(meanG / count), Math.round(meanB / count)];
  for (let index = 0; index < R * R; index += 1) {
    if (!inside[index]) continue;
    const at = index * 4;
    if (mono) {
      paint[index] = 1;
      colour[index * 3] = main[0];
      colour[index * 3 + 1] = main[1];
      colour[index * 3 + 2] = main[2];
      continue;
    }
    const light = (0.3 * src[at] + 0.59 * src[at + 1] + 0.11 * src[at + 2]) / 255;
    // solid paint everywhere except the brightest highlights, which fade into bare wall
    paint[index] = light < 0.8 ? 1 : Math.max(0, 1 - (light - 0.8) / 0.2);
    // punchy, slightly deepened spray colours in a few flat levels
    const mean = (src[at] + src[at + 1] + src[at + 2]) / 3;
    for (let channel = 0; channel < 3; channel += 1) {
      const boosted = (mean + (src[at + channel] - mean) * 1.35) * 0.92;
      const level = Math.round(Math.max(0, Math.min(255, boosted)) / 255 * 5) * (255 / 5);
      colour[index * 3 + channel] = level;
    }
  }

  // 2) outline = inside pixels next to the edge
  const outline = new Uint8Array(R * R);
  const band = 3;
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
      if (edge) outline[index] = 1;
    }
  }

  const out = makeCanvas();
  const octx = out.getContext("2d")!;

  // 3) soft overspray hugging the shape (airbrushed, low opacity)
  const mask = makeCanvas();
  const mctx = mask.getContext("2d")!;
  const maskData = mctx.createImageData(R, R);
  for (let index = 0; index < R * R; index += 1) {
    if (!inside[index]) continue;
    maskData.data[index * 4] = main[0];
    maskData.data[index * 4 + 1] = main[1];
    maskData.data[index * 4 + 2] = main[2];
    maskData.data[index * 4 + 3] = 255;
  }
  mctx.putImageData(maskData, 0, 0);
  octx.save();
  octx.filter = `blur(${R * 0.028}px)`;
  octx.globalAlpha = 0.3;
  octx.drawImage(mask, 0, 0);
  octx.restore();

  // 4) the stencil paint: solid where dark, stippled spray grain in the mid tones
  const stencil = octx.getImageData(0, 0, R, R);
  const put = (index: number, red: number, green: number, blue: number, alpha: number) => {
    const at = index * 4;
    // paint over what is there (overspray) instead of replacing it
    const under = stencil.data[at + 3] / 255;
    const over = alpha / 255;
    const total = over + under * (1 - over);
    if (total <= 0) return;
    stencil.data[at] = (red * over + stencil.data[at] * under * (1 - over)) / total;
    stencil.data[at + 1] = (green * over + stencil.data[at + 1] * under * (1 - over)) / total;
    stencil.data[at + 2] = (blue * over + stencil.data[at + 2] * under * (1 - over)) / total;
    stencil.data[at + 3] = total * 255;
  };
  for (let y = 0; y < R; y += 1) {
    for (let x = 0; x < R; x += 1) {
      const index = y * R + x;
      if (!inside[index]) continue;
      if (outline[index]) {
        if (random() > 0.06) put(index, 30, 24, 22, 240); // dark painted edge, a whisper of missed spots
        continue;
      }
      const want = paint[index];
      if (want <= 0.04) continue;
      const threshold = (BAYER[y & 7][x & 7] + 0.5) / 64 + (random() - 0.5) * 0.3; // noise breaks up the regular dot grid
      if (want > threshold && random() > 0.025) {
        const mottle = (random() - 0.5) * 14;
        put(index, colour[index * 3] + mottle, colour[index * 3 + 1] + mottle, colour[index * 3 + 2] + mottle, 240 + random() * 15);
      }
    }
  }
  octx.putImageData(stencil, 0, 0);

  // 5) drips running down from the painted areas
  const painted = stencil.data;
  const dripCount = 2 + Math.floor(random() * 2);
  for (let drip = 0; drip < dripCount; drip += 1) {
    const x = Math.floor(R * (0.26 + random() * 0.48));
    let bottom = -1;
    for (let y = R - 1; y >= 0; y -= 1) {
      if (painted[(y * R + x) * 4 + 3] > 200) {
        bottom = y;
        break;
      }
    }
    if (bottom < 0) continue;
    const length = R * (0.06 + random() * 0.11);
    const width = R * (0.011 + random() * 0.008);
    // the drip carries the colour it runs from (a little above the outline)
    const source = Math.max(0, bottom - Math.round(R * 0.03)) * R + x;
    const dr = inside[source] ? colour[source * 3] : main[0];
    const dg = inside[source] ? colour[source * 3 + 1] : main[1];
    const db = inside[source] ? colour[source * 3 + 2] : main[2];
    const gradient = octx.createLinearGradient(0, bottom, 0, bottom + length);
    gradient.addColorStop(0, `rgba(${dr},${dg},${db},0.95)`);
    gradient.addColorStop(1, `rgba(${dr},${dg},${db},0.75)`);
    octx.fillStyle = gradient;
    octx.fillRect(x - width / 2, bottom - 2, width, length + 2);
    octx.beginPath();
    octx.arc(x, bottom + length, width * 0.9, 0, Math.PI * 2);
    octx.fill();
  }

  // 6) a few tiny specks of spatter right at the edge
  octx.fillStyle = `rgba(${main[0]},${main[1]},${main[2]},0.6)`;
  for (let speck = 0; speck < 6; speck += 1) {
    const angle = random() * Math.PI * 2;
    const radius = R * (0.29 + random() * 0.08);
    octx.beginPath();
    octx.arc(R / 2 + Math.cos(angle) * radius, R * 0.46 + Math.sin(angle) * radius, 0.8 + random() * 1.2, 0, Math.PI * 2);
    octx.fill();
  }

  const url = out.toDataURL("image/png");
  cache.set(key, url);
  return url;
}
