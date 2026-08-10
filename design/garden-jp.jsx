/* ============================================================
   Japanese Garden of Knowledge — generative, data-driven.

   Same mapping idea as the Garden of Knowledge, Japanese flora:
   · session      → momiji / sakura / matsu / take / hagi / suzuki …
   · milestone    → sakura or tsutsuji in the course color
   · streak ≥ 5   → golden kiku (chrysanthemum) + hotaru fireflies
   · high stage   → the Old Maple, then the Great Pine
   · 9 growth stages (0..8); density keeps rising until the ground
     is fully covered (moss carpet + multi-band planting).
   ============================================================ */

function jgHash(s) { let h = 2166136261; for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); } return h >>> 0; }
function jgRng(seed) { let a = seed | 0; return function () { a = a + 0x6D2B79F5 | 0; let t = Math.imul(a ^ a >>> 15, 1 | a); t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t; return ((t ^ t >>> 14) >>> 0) / 4294967296; }; }
const JG_LEAF = 'oklch(0.60 0.10 148)';
const JG_LEAF_DEEP = 'oklch(0.42 0.075 158)';
const JG_SOIL = 'oklch(0.48 0.045 60)';
const JG_GOLD = 'oklch(0.80 0.13 85)';
const jgCache = {};
function jgParse(c) {
  if (c === 'white') return [1, 0, null];
  if (c === 'black') return [0, 0, null];
  const m = /oklch\(([\d.]+)[ ,]+([\d.]+)[ ,]+([-\d.]+)/.exec(c);
  return m ? [+m[1], +m[2], +m[3]] : [0.6, 0.06, 150];
}
function jgMix(a, b, pct) {
  const k = a + '|' + b + '|' + Math.round(pct);
  if (jgCache[k]) return jgCache[k];
  const p = Math.max(0, Math.min(100, pct)) / 100;
  const A = jgParse(a), B = jgParse(b);
  const l = A[0] + (B[0] - A[0]) * p;
  const c = A[1] + (B[1] - A[1]) * p;
  let h = A[2];
  if (B[2] != null && A[2] != null) { let d = B[2] - A[2]; if (d > 180) d -= 360; if (d < -180) d += 360; h = (A[2] + d * p + 360) % 360; }
  else if (A[2] == null) h = B[2] || 0;
  return (jgCache[k] = 'oklch(' + l.toFixed(3) + ' ' + c.toFixed(3) + ' ' + (h == null ? 0 : h.toFixed(1)) + ')');
}
function jgGreen(rng) { return jgMix(JG_LEAF, JG_LEAF_DEEP, 20 + rng() * 60); }

const JG_MOSS = 'oklch(0.52 0.07 145)';
const JG_PINE = 'oklch(0.42 0.055 155)';
const JG_MAPLE = 'oklch(0.58 0.14 32)';
const JG_MAPLE2 = 'oklch(0.66 0.13 55)';
const JG_SAKURA = 'oklch(0.86 0.055 350)';
const JG_BAMBOO = 'oklch(0.72 0.09 120)';
const JG_STONE = 'oklch(0.62 0.012 250)';

function jgLeaf(x, y, len, dir, lift = 0.7) {
  const tx = x + dir * len, ty = y - len * lift;
  return `M${x} ${y} Q ${x + dir * len * 0.15} ${y - len * 0.8} ${tx} ${ty} Q ${x + dir * len * 0.72} ${y - len * 0.12} ${x} ${y} Z`;
}
/* 5-lobe maple leaf, drawn around 0,0 then placed by transform */
function jgMapleLeaf(cx, cy, r, rot, fill, key, opacity) {
  const p = `M0 ${-r} Q ${r * 0.34} ${-r * 0.62} ${r * 0.34} ${-r * 0.34} Q ${r * 0.74} ${-r * 0.6} ${r * 0.86} ${-r * 0.38} Q ${r * 0.66} ${-r * 0.06} ${r * 0.46} ${r * 0.14} Q ${r * 0.74} ${r * 0.5} ${r * 0.56} ${r * 0.66} Q ${r * 0.22} ${r * 0.42} 0 ${r * 0.36} Q ${-r * 0.22} ${r * 0.42} ${-r * 0.56} ${r * 0.66} Q ${-r * 0.74} ${r * 0.5} ${-r * 0.46} ${r * 0.14} Q ${-r * 0.66} ${-r * 0.06} ${-r * 0.86} ${-r * 0.38} Q ${-r * 0.74} ${-r * 0.6} ${-r * 0.34} ${-r * 0.34} Q ${-r * 0.34} ${-r * 0.62} 0 ${-r} Z`;
  return <path key={key} d={p} fill={fill} opacity={opacity == null ? 0.96 : opacity} transform={`translate(${cx} ${cy}) rotate(${rot})`} />;
}
/* lancet leaf (bamboo, iris blade) */
function jgBlade(x, y, len, dir, droop, w = 0.22) {
  const tx = x + dir * len, ty = y + droop;
  return `M${x} ${y} Q ${x + dir * len * 0.5} ${y - len * w} ${tx} ${ty} Q ${x + dir * len * 0.5} ${y + len * w * 0.6} ${x} ${y} Z`;
}

/* ---------- species (viewBox 0 0 48 58, anchored 24,58) ---------- */

function JGMoss({ rng, detail }) {
  return (
    <g>
      {Array.from({ length: 3 }).map((_, i) => (
        <ellipse key={i} cx={12 + rng() * 24} cy={55 - rng() * 3} rx={7 + rng() * 7} ry={2.6 + rng() * 2} fill={jgMix(JG_MOSS, JG_LEAF, Math.round(rng() * 45))} opacity={0.75} />
      ))}
      {detail !== 0 && Array.from({ length: 7 }).map((_, i) => {
        const x = 8 + rng() * 32, y = 51 + rng() * 6;
        return <path key={'t' + i} d={`M${x} ${y} q ${(rng() - 0.5) * 2} -1.6 ${(rng() - 0.5) * 3} -${1.8 + rng() * 2}`} stroke={jgMix(JG_MOSS, JG_LEAF, 40)} strokeWidth="0.55" fill="none" strokeLinecap="round" opacity="0.75" />;
      })}
    </g>
  );
}

function JGSuzuki({ rng }) { /* susuki grass */
  const n = 5 + Math.floor(rng() * 4);
  return (
    <g>
      {Array.from({ length: n }).map((_, i) => {
        const dx = (rng() - 0.5) * 28, h = 18 + rng() * 26, g = jgGreen(rng);
        const tipx = 24 + dx, tipy = 58 - h;
        return (
          <g key={i}>
            <path d={`M${23.4} 58 Q ${24 + dx * 0.2} ${58 - h * 0.62} ${tipx} ${tipy} Q ${24 + dx * 0.3} ${58 - h * 0.6} ${24.6} 58 Z`} fill={g} opacity={0.55 + rng() * 0.35} />
            <path d={`M24 58 Q ${24 + dx * 0.2} ${58 - h * 0.6} ${tipx} ${tipy}`} stroke={jgMix(g, 'white', 14)} strokeWidth="0.5" fill="none" opacity="0.5" />
          </g>
        );
      })}
      {rng() > 0.4 && (() => { const px = 24 + (rng() - 0.5) * 12, py = 22 + rng() * 6; return (
        <g>
          <path d={`M24 58 Q ${px - 2} ${py + 14} ${px} ${py + 4}`} stroke={jgGreen(rng)} strokeWidth="0.9" fill="none" strokeLinecap="round" />
          {Array.from({ length: 9 }).map((_, k) => (
            <path key={k} d={`M${px} ${py + 5} q ${(k % 2 ? 1 : -1) * (1.4 + rng() * 2)} -2.4 ${(k % 2 ? 1 : -1) * (2.4 + rng() * 3)} -${4 + rng() * 4}`}
              stroke={jgMix(JG_GOLD, 'white', 40)} strokeWidth="0.7" fill="none" strokeLinecap="round" opacity="0.7" />
          ))}
        </g>
      ); })()}
    </g>
  );
}

function JGShoot({ rng }) { /* young shoot */
  const g = jgGreen(rng);
  return (
    <g>
      <path d="M24 58 Q 24.6 51 24 45" stroke={JG_LEAF_DEEP} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d={jgLeaf(24, 46, 7 + rng() * 3, -1)} fill={g} />
      <path d={jgLeaf(24, 47.5, 6 + rng() * 3, 1)} fill={jgMix(g, JG_LEAF, 40)} />
      <path d="M24 46 L 20 42.5" stroke={jgMix(g, 'white', 22)} strokeWidth="0.45" opacity="0.7" />
      <path d="M24 47.5 L 28 44.5" stroke={jgMix(g, 'white', 22)} strokeWidth="0.45" opacity="0.7" />
    </g>
  );
}

function JGBamboo({ rng, maturity }) {
  const stalks = 2 + Math.floor(rng() * 3);
  return (
    <g>
      {Array.from({ length: stalks }).map((_, i) => {
        const x = 24 + (i - (stalks - 1) / 2) * (5 + rng() * 3);
        const h = 30 + rng() * 18 + maturity * 8;
        const segs = 4 + Math.floor(rng() * 3);
        const lean = (rng() - 0.5) * 5;
        const cane = jgMix(JG_BAMBOO, i % 2 ? JG_PINE : 'white', 8 + Math.round(rng() * 16));
        const top = x + lean;
        return (
          <g key={i}>
            <path d={`M${x - 1.1} 58 Q ${x + lean * 0.4 - 1.1} ${58 - h * 0.55} ${top - 0.85} ${58 - h} L ${top + 0.85} ${58 - h} Q ${x + lean * 0.4 + 1.1} ${58 - h * 0.55} ${x + 1.1} 58 Z`} fill={cane} />
            <path d={`M${x - 0.5} 58 Q ${x + lean * 0.4 - 0.5} ${58 - h * 0.55} ${top - 0.35} ${58 - h}`} stroke={jgMix(cane, 'white', 30)} strokeWidth="0.6" fill="none" opacity="0.7" />
            {Array.from({ length: segs }).map((_, j) => {
              const f = (j + 1) / segs, y = 58 - h * f, sx = x + lean * f;
              return <g key={j}>
                <path d={`M${sx - 1.25} ${y} q 1.25 0.9 2.5 0`} stroke={jgMix(cane, 'black', 34)} strokeWidth="0.7" fill="none" />
                {j > segs - 4 && Array.from({ length: 3 }).map((_, k) => {
                  const dir = (j + k) % 2 ? 1 : -1, len = 7 + rng() * 6;
                  return <path key={k} d={jgBlade(sx, y - k * 1.6, len, dir, 3 + rng() * 3, 0.17)} fill={jgMix(JG_PINE, JG_BAMBOO, 30 + Math.round(rng() * 45))} opacity="0.92" />;
                })}
              </g>;
            })}
          </g>
        );
      })}
    </g>
  );
}

function JGMomiji({ rng, color, maturity, detail }) { /* japanese maple */
  const h = 26 + rng() * 12 + maturity * 10;
  const red = rng() > 0.45 ? JG_MAPLE : JG_MAPLE2;
  const bark = jgMix(JG_SOIL, 'black', 32);
  const crown = [[24, 58 - h, 10.2], [24 - 9, 58 - h * 0.84, 7.6], [24 + 9.5, 58 - h * 0.81, 8], [24, 58 - h * 0.66, 8.6], [24 - 4.5, 58 - h * 1.03, 5.2], [24 + 5, 58 - h * 1.0, 4.8]];
  const leaves = [];
  const n = detail === 0 ? 10 : 16 + Math.round(maturity * 8);
  for (let i = 0; i < n; i++) {
    const b = crown[Math.floor(rng() * crown.length)];
    const a = rng() * Math.PI * 2, rad = Math.sqrt(rng()) * b[2] * 0.95;
    const tint = rng();
    const fill = jgMix(tint < 0.34 ? JG_MAPLE : tint < 0.7 ? JG_MAPLE2 : jgMix(red, JG_MOSS, 30), tint < 0.5 ? 'black' : 'white', 6 + Math.round(rng() * 14));
    leaves.push(jgMapleLeaf(b[0] + Math.cos(a) * rad, b[1] + Math.sin(a) * rad * 0.8, 2 + rng() * 1.6, rng() * 360, fill, 'l' + i, 0.9 + rng() * 0.1));
  }
  return (
    <g>
      <path d={`M22.4 58 C 22.2 ${58 - h * 0.28} 24.6 ${58 - h * 0.44} 23.3 ${58 - h * 0.64} L 24.9 ${58 - h * 0.64} C 25.6 ${58 - h * 0.44} 25 ${58 - h * 0.28} 25.7 58 Z`} fill={bark} />
      <path d={`M24 ${58 - h * 0.44} Q 30 ${58 - h * 0.58} 32.6 ${58 - h * 0.72}`} stroke={bark} strokeWidth="1.3" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.5} Q 18 ${58 - h * 0.62} 15.4 ${58 - h * 0.76}`} stroke={bark} strokeWidth="1.2" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.6} Q 26.5 ${58 - h * 0.74} 27.5 ${58 - h * 0.86}`} stroke={bark} strokeWidth="0.9" fill="none" strokeLinecap="round" />
      {crown.map((b, i) => <ellipse key={i} cx={b[0]} cy={b[1]} rx={b[2]} ry={b[2] * 0.78} fill={jgMix(red, 'black', 16 + Math.round(rng() * 12))} opacity="0.5" />)}
      {leaves}
      {Array.from({ length: maturity > 0.7 ? 5 : 2 }).map((_, i) => jgMapleLeaf(12 + rng() * 24, 53 - rng() * 4, 1.5 + rng(), rng() * 360, jgMix(color || red, 'black', 8), 'g' + i, 0.75))}
    </g>
  );
}

/* one 5-petal blossom */
function jgBlossom(cx, cy, r, pink, rot, key) {
  return (
    <g key={key}>
      {[0, 1, 2, 3, 4].map(i => (
        <ellipse key={i} cx={cx} cy={cy - r * 0.62} rx={r * 0.44} ry={r * 0.62} fill={pink}
          transform={`rotate(${rot + i * 72} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r={r * 0.26} fill={jgMix(JG_GOLD, pink, 35)} />
    </g>
  );
}

function JGSakura({ rng, color, maturity, detail }) {
  const h = 26 + rng() * 12 + maturity * 10;
  const pink = color ? jgMix(color, JG_SAKURA, 74) : JG_SAKURA;
  const deep = jgMix(pink, JG_MAPLE, 22);
  const bark = jgMix(JG_SOIL, 'black', 34);
  const canopy = [[24, 58 - h, 9.8], [24 - 9, 58 - h * 0.86, 7.4], [24 + 9.4, 58 - h * 0.83, 7.8], [24, 58 - h * 0.68, 8.4], [24 - 4, 58 - h * 1.04, 5.4], [24 + 5, 58 - h * 1.02, 5]];
  const flowers = [];
  const nF = detail === 0 ? 7 : 11 + Math.round(maturity * 7);
  for (let i = 0; i < nF; i++) {
    const b = canopy[Math.floor(rng() * canopy.length)];
    const a = rng() * Math.PI * 2, rad = Math.sqrt(rng()) * b[2] * 0.92;
    flowers.push(jgBlossom(b[0] + Math.cos(a) * rad, b[1] + Math.sin(a) * rad * 0.78, 2.1 + rng() * 1.5, rng() < 0.35 ? deep : pink, rng() * 72, 'f' + i));
  }
  return (
    <g>
      <path d={`M24 58 C 22.6 ${58 - h * 0.24} 25.6 ${58 - h * 0.44} 24 ${58 - h * 0.6}`} stroke={bark} strokeWidth="2.8" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.46} Q 29.5 ${58 - h * 0.6} 32 ${58 - h * 0.76}`} stroke={bark} strokeWidth="1.5" fill="none" strokeLinecap="round" />
      <path d={`M24 ${58 - h * 0.52} Q 18.5 ${58 - h * 0.66} 16 ${58 - h * 0.8}`} stroke={bark} strokeWidth="1.4" fill="none" strokeLinecap="round" />
      {canopy.map((p, i) => <ellipse key={i} cx={p[0]} cy={p[1]} rx={p[2]} ry={p[2] * 0.74} fill={jgMix(pink, i % 2 ? 'white' : JG_MAPLE, 10 + Math.round(rng() * 10))} opacity="0.55" />)}
      {flowers}
      {Array.from({ length: 5 }).map((_, i) => <circle key={i} cx={10 + rng() * 28} cy={46 + rng() * 11} r="1" fill={pink} opacity="0.8" />)}
    </g>
  );
}

function JGMatsu({ rng, wise, detail }) { /* pruned pine */
  const gold = JG_GOLD;
  const pads = [[24, 22, 13, 4.6], [12, 31, 8.5, 3.4], [36, 29, 9, 3.6], [24, 36, 10, 3.6], [30, 44, 7, 2.8]];
  const bark = jgMix(JG_SOIL, 'black', 28);
  return (
    <g>
      <path d="M22.2 58 C 18.6 48 25.6 42 21.6 34 C 19.8 30 22.6 27 22.8 24 L 25.2 24 C 25.4 27 22.4 30 24.4 34 C 28.4 42 21.6 48 25.8 58 Z" fill={bark} />
      {[0, 1, 2].map(i => <path key={i} d={`M${22.6 + i * 0.9} ${54 - i * 3} q 0.6 -6 -0.3 -11`} stroke={jgMix(bark, 'white', 16)} strokeWidth="0.35" fill="none" opacity="0.55" />)}
      <path d="M23.6 38 Q 30 36 33 31" stroke={bark} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d="M23.2 34 Q 17 32 13.5 30" stroke={bark} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      {pads.map((p, i) => (
        <g key={i}>
          <ellipse cx={p[0]} cy={p[1]} rx={p[2]} ry={p[3]} fill={jgMix(JG_PINE, JG_LEAF, 12 + Math.round(rng() * 34))} opacity="0.95" />
          <ellipse cx={p[0] - p[2] * 0.25} cy={p[1] - p[3] * 0.45} rx={p[2] * 0.55} ry={p[3] * 0.5} fill={jgMix(JG_PINE, 'white', 12)} opacity="0.5" />
          {detail !== 0 && Array.from({ length: 8 }).map((_, k) => {
            const a = rng() * Math.PI * 2, rr = Math.sqrt(rng());
            const nx = p[0] + Math.cos(a) * p[2] * rr, ny = p[1] + Math.sin(a) * p[3] * rr;
            const dir = rng() < 0.5 ? -1 : 1;
            return <path key={k} d={`M${nx} ${ny} l ${dir * (1.2 + rng() * 1.6)} ${-(1.4 + rng() * 1.8)}`} stroke={jgMix(JG_PINE, rng() < 0.5 ? 'white' : 'black', 10 + Math.round(rng() * 18))} strokeWidth="0.5" strokeLinecap="round" opacity="0.8" />;
          })}
        </g>
      ))}
      {wise && Array.from({ length: 6 }).map((_, i) => <circle key={i} cx={11 + rng() * 26} cy={16 + rng() * 26} r="1.2" fill={gold} />)}
    </g>
  );
}

function JGTsutsuji({ rng, color, maturity, detail }) { /* clipped azalea mound */
  const blooms = detail === 0 ? 4 : (maturity > 0.5 ? 9 : 4);
  const petal = color || 'oklch(0.68 0.13 12)';
  const mounds = [[16, 51, 8 + rng() * 3, 6.5 + rng() * 2], [31, 49, 9 + rng() * 3, 7.5 + rng() * 2.5], [24, 52, 10 + rng() * 2, 6 + rng() * 2]];
  return (
    <g>
      {mounds.map((m, i) => <ellipse key={i} cx={m[0]} cy={m[1]} rx={m[2]} ry={m[3]} fill={i === 2 ? jgMix(jgGreen(rng), JG_MOSS, 35) : jgGreen(rng)} />)}
      {detail !== 0 && Array.from({ length: 10 }).map((_, i) => {
        const m = mounds[Math.floor(rng() * 3)];
        const a = rng() * Math.PI * 2, rr = Math.sqrt(rng());
        const x = m[0] + Math.cos(a) * m[2] * rr, y = m[1] + Math.sin(a) * m[3] * rr * 0.9;
        return <ellipse key={i} cx={x} cy={y} rx={1.5 + rng()} ry={0.9 + rng() * 0.5} fill={jgMix(JG_MOSS, rng() < 0.5 ? 'white' : 'black', 8 + Math.round(rng() * 16))} opacity="0.6"
          transform={`rotate(${-40 + rng() * 80} ${x} ${y})`} />;
      })}
      {Array.from({ length: blooms }).map((_, i) => {
        const m = mounds[Math.floor(rng() * 3)];
        const a = rng() * Math.PI * 2, rr = Math.sqrt(rng()) * 0.85;
        return jgBlossom(m[0] + Math.cos(a) * m[2] * rr, m[1] - m[3] * 0.35 + Math.sin(a) * m[3] * rr, 1.7 + rng() * 0.9,
          jgMix(petal, rng() < 0.4 ? 'white' : 'black', 6 + Math.round(rng() * 12)), rng() * 72, 'b' + i);
      })}
    </g>
  );
}

function JGIris({ rng, color }) { /* hanashobu */
  const stems = 2 + Math.floor(rng() * 3);
  return (
    <g>
      {Array.from({ length: stems }).map((_, i) => {
        const dx = (i - (stems - 1) / 2) * (5 + rng() * 3), h = 24 + rng() * 14;
        const cx = 24 + dx, cy = 58 - h;
        const violet = color || 'oklch(0.55 0.12 285)';
        const deep = jgMix(violet, 'black', 16), pale = jgMix(violet, 'white', 22);
        return (
          <g key={i}>
            {[-1, 1, -1].map((d, k) => (
              <path key={k} d={jgBlade(24 + dx * 0.4, 58, 20 + rng() * 8, d, -(16 + rng() * 10), 0.09)} fill={jgMix(jgGreen(rng), 'black', k * 5)} opacity={0.85} />
            ))}
            <path d={`M${24 + dx * 0.4} 58 L ${cx} ${cy}`} stroke={jgGreen(rng)} strokeWidth="1.2" strokeLinecap="round" />
            <path d={`M${cx} ${cy + 1.6} q -3.6 0.4 -4.6 3.4 q 2.6 1.4 4.6 -0.6 Z`} fill={deep} />
            <path d={`M${cx} ${cy + 1.6} q 3.6 0.4 4.6 3.4 q -2.6 1.4 -4.6 -0.6 Z`} fill={deep} />
            <path d={`M${cx} ${cy + 2} q -1.4 3.4 0 5.4 q 1.4 -2 0 -5.4 Z`} fill={jgMix(violet, 'black', 24)} />
            <ellipse cx={cx - 1.5} cy={cy - 1.4} rx="1.5" ry="2.8" fill={pale} transform={`rotate(-22 ${cx} ${cy})`} />
            <ellipse cx={cx + 1.5} cy={cy - 1.4} rx="1.5" ry="2.8" fill={pale} transform={`rotate(22 ${cx} ${cy})`} />
            <path d={`M${cx - 1.4} ${cy + 3.2} q 1.4 0.6 2.8 0`} stroke={jgMix(JG_GOLD, 'white', 30)} strokeWidth="0.7" fill="none" strokeLinecap="round" />
          </g>
        );
      })}
    </g>
  );
}

function JGHagi({ rng, color }) { /* bush clover — arching sprays */
  const arcs = 3 + Math.floor(rng() * 2);
  return (
    <g>
      {Array.from({ length: arcs }).map((_, i) => {
        const dir = i % 2 ? 1 : -1, h = 22 + rng() * 12, reach = 10 + rng() * 8;
        const g = jgGreen(rng), bloom = color || 'oklch(0.66 0.1 340)';
        const pt = f => [24 + dir * reach * f, 58 - h * (0.55 + 0.45 * (1 - Math.abs(f - 0.45) * 1.4))];
        return (
          <g key={i}>
            <path d={`M24 58 Q ${24 + dir * reach * 0.4} ${58 - h} ${24 + dir * reach} ${58 - h * 0.7}`} stroke={g} strokeWidth="1.1" fill="none" strokeLinecap="round" />
            {[0.25, 0.45, 0.65, 0.85].map((f, j) => {
              const [x, y] = pt(f);
              return <g key={'lf' + j}>
                <ellipse cx={x} cy={y - 1.6} rx="1.7" ry="1.1" fill={jgMix(g, 'white', 10)} opacity="0.9" transform={`rotate(${dir * -25} ${x} ${y})`} />
                <ellipse cx={x} cy={y + 1.6} rx="1.7" ry="1.1" fill={jgMix(g, 'black', 8)} opacity="0.9" transform={`rotate(${dir * 25} ${x} ${y})`} />
              </g>;
            })}
            {[0.4, 0.62, 0.86].map((f, j) => {
              const [x, y] = pt(f);
              return <g key={'b' + j}>
                <ellipse cx={x} cy={y + 0.4} rx="1.5" ry="1.1" fill={bloom} opacity="0.95" />
                <ellipse cx={x} cy={y - 0.6} rx="1" ry="0.9" fill={jgMix(bloom, 'white', 30)} opacity="0.95" />
              </g>;
            })}
          </g>
        );
      })}
    </g>
  );
}

function JGFern({ rng }) {
  const h = 24 + rng() * 14, curl = 3 + rng() * 4, g = jgGreen(rng);
  const fronds = [];
  for (let i = 0; i < 6; i++) {
    const f = 0.18 + 0.78 * (i / 6);
    const x = 24 + curl * Math.sin(f * 2.4), y = 58 - h * f, len = (1 - f) * 9 + 2;
    [-1, 1].forEach(d => {
      fronds.push(<path key={(d < 0 ? 'l' : 'r') + i} d={jgBlade(x, y, len, d, -len * 0.85, 0.3)} fill={jgMix(g, d < 0 ? 'black' : 'white', 6)} opacity="0.92" />);
      for (let k = 1; k < 4; k++) {
        const px = x + d * len * (k / 4), py = y - len * 0.85 * (k / 4);
        fronds.push(<path key={(d < 0 ? 'lp' : 'rp') + i + k} d={`M${px} ${py} l ${d * 1.6} -1.5`} stroke={jgMix(g, 'white', 14)} strokeWidth="0.4" opacity="0.6" />);
      }
    });
  }
  return <g><path d={`M24 58 Q ${24 + curl * 2} ${58 - h * 0.5} 24 ${58 - h}`} stroke={JG_LEAF_DEEP} strokeWidth="1.6" fill="none" strokeLinecap="round" />{fronds}</g>;
}

function JGStone({ rng }) { /* garden stone */
  return (
    <g>
      <ellipse cx="24" cy="57" rx={12 + rng() * 4} ry="2.6" fill="black" opacity="0.16" />
      <ellipse cx="24" cy="55" rx={11 + rng() * 5} ry={5 + rng() * 3} fill={jgMix(JG_STONE, 'black', 18)} opacity="0.9" />
      <ellipse cx={22 + rng() * 3} cy={52 + rng() * 2} rx={7 + rng() * 3} ry={3.4 + rng() * 1.6} fill={jgMix(JG_STONE, 'white', 16)} opacity="0.75" />
      {Array.from({ length: 5 }).map((_, i) => (
        <path key={i} d={`M${15 + rng() * 18} ${50 + rng() * 6} q ${2 + rng() * 3} ${-1 + rng() * 2} ${4 + rng() * 4} ${-0.5 + rng()}`} stroke={jgMix(JG_STONE, 'black', 34)} strokeWidth="0.4" fill="none" opacity="0.5" />
      ))}
      <ellipse cx="30" cy="56" rx="4.5" ry="2.4" fill={jgMix(JG_MOSS, 'black', 10)} opacity="0.65" />
      <ellipse cx="17" cy="55.5" rx="3" ry="1.6" fill={jgMix(JG_MOSS, 'black', 20)} opacity="0.5" />
    </g>
  );
}

function JGLantern({ rng }) { /* ishidōrō */
  return (
    <g>
      <ellipse cx="24" cy="57.5" rx="11" ry="2.6" fill="black" opacity="0.18" />
      <ellipse cx="24" cy="57" rx="9" ry="3" fill={jgMix(JG_STONE, 'black', 30)} opacity="0.8" />
      <rect x="21" y="42" width="6" height="14" fill={JG_STONE} />
      <rect x="21" y="42" width="1.6" height="14" fill={jgMix(JG_STONE, 'white', 18)} opacity="0.6" />
      <rect x="25.6" y="42" width="1.4" height="14" fill={jgMix(JG_STONE, 'black', 22)} opacity="0.6" />
      <rect x="17.5" y="38" width="13" height="4.5" rx="1" fill={jgMix(JG_STONE, 'white', 10)} />
      <rect x="19" y="29" width="10" height="9.5" rx="1" fill={jgMix(JG_STONE, 'white', 6)} />
      <rect x="21.2" y="31.5" width="5.6" height="5" rx="0.8" fill={JG_GOLD} opacity="0.8" />
      <path d="M14.5 29 L 33.5 29 L 28 23.5 L 20 23.5 Z" fill={jgMix(JG_STONE, 'black', 12)} />
      <circle cx="24" cy="21.5" r="2" fill={jgMix(JG_STONE, 'white', 14)} />
    </g>
  );
}

function JGKiku({ rng }) { /* golden chrysanthemum — streak bloom */
  const cx = 24, cy = 26, rot = rng() * 60;
  return (
    <g>
      <circle cx={cx} cy={cy} r="12" fill={JG_GOLD} opacity="0.12" />
      <path d={`M24 58 Q 26 42 ${cx} ${cy}`} stroke={JG_LEAF_DEEP} strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d={jgLeaf(25, 46, 8, -1)} fill={jgGreen(rng)} />
      <path d={jgLeaf(25.2, 41, 7, 1)} fill={jgGreen(rng)} />
      {Array.from({ length: 12 }).map((_, i) => (
        <ellipse key={i} cx={cx} cy={cy - 6} rx="1.7" ry="6" fill={JG_GOLD} opacity={i % 2 ? 0.75 : 0.95}
          transform={`rotate(${rot + i * 30} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r="2.6" fill={jgMix(JG_GOLD, 'white', 55)} />
    </g>
  );
}


function JGHut({ rng }) { /* small teahouse — chaya */
  const wood = jgMix(JG_SOIL, 'black', 18), woodL = jgMix(JG_SOIL, 'white', 14);
  const roof = jgMix(JG_STONE, 'black', 46), roofL = jgMix(JG_STONE, 'black', 30);
  const shoji = jgMix(JG_GOLD, 'white', 62);
  return (
    <g>
      <ellipse cx="24" cy="57" rx="17" ry="2.6" fill="black" opacity="0.18" />
      {/* raised platform */}
      <rect x="9" y="48" width="30" height="3.4" rx="0.8" fill={woodL} />
      {[11, 20, 28, 36].map((x, i) => <rect key={i} x={x} y="51" width="1.6" height="5" fill={wood} />)}
      {/* body */}
      <rect x="11.5" y="34" width="25" height="14" fill={jgMix(wood, 'black', 12)} />
      <rect x="13.5" y="36" width="8.5" height="10" fill={shoji} opacity="0.75" />
      <rect x="24" y="36" width="8.5" height="10" fill={shoji} opacity="0.55" />
      <line x1="17.75" y1="36" x2="17.75" y2="46" stroke={wood} strokeWidth="0.6" />
      <line x1="28.25" y1="36" x2="28.25" y2="46" stroke={wood} strokeWidth="0.6" />
      <line x1="13.5" y1="41" x2="32.5" y2="41" stroke={wood} strokeWidth="0.6" />
      {/* hipped roof with upturned eaves */}
      <path d="M24 17 L 43 33 Q 39 35.5 34.5 34.2 L 24 25.5 L 13.5 34.2 Q 9 35.5 5 33 Z" fill={roof} />
      <path d="M24 19.5 L 38.5 32 L 9.5 32 Z" fill={roofL} opacity="0.85" />
      <path d="M24 17 L 24 25.5" stroke={jgMix(roof, 'white', 16)} strokeWidth="0.7" />
      <rect x="22.6" y="14.6" width="2.8" height="3" rx="0.6" fill={jgMix(roof, 'white', 10)} />
      {/* step stone */}
      <ellipse cx="24" cy="55.5" rx="5" ry="2" fill={jgMix(JG_STONE, 'black', 24)} />
    </g>
  );
}

const JG_SPECIES = {
  moss: JGMoss, suzuki: JGSuzuki, shoot: JGShoot, bamboo: JGBamboo, momiji: JGMomiji,
  sakura: JGSakura, matsu: JGMatsu, tsutsuji: JGTsutsuji, iris: JGIris, hagi: JGHagi,
  fern: JGFern, stone: JGStone, lantern: JGLantern, kiku: JGKiku, hut: JGHut,
};
const JG_BASE_W = { moss: 0.9, suzuki: 0.85, shoot: 0.5, bamboo: 1.1, momiji: 1.75, sakura: 1.6, matsu: 1.95, tsutsuji: 1.15, iris: 0.95, hagi: 1, fern: 0.95, stone: 0.85, lantern: 0.95, kiku: 1.05, hut: 2 };

/* ---------- stage: 9 levels, keeps climbing ---------- */
const JG_THRESHOLDS = [0, 45, 120, 240, 400, 600, 850, 1150, 1500];
const JG_STAGE_NAMES = ['Bare ground', 'First shoots', 'Moss takes hold', 'Tea house', 'Bamboo grove', 'Koi pond', 'River & bridge', 'Full garden', 'Long tended'];
function jgStage(data) {
  const wk = ST.weeklyFocus(data).reduce((a, d) => a + d.minutes, 0);
  let s = 0; for (let i = 0; i < JG_THRESHOLDS.length; i++) if (wk >= JG_THRESHOLDS[i]) s = i;
  return s;
}

function jgPick(rng, maturity) {
  if (maturity < 0.3) return rng() < 0.55 ? 'shoot' : 'suzuki';
  if (maturity < 0.6) return ['fern', 'hagi', 'iris', 'suzuki', 'tsutsuji'][Math.floor(rng() * 5)];
  if (maturity < 0.85) return ['bamboo', 'tsutsuji', 'sakura', 'iris', 'momiji', 'hagi'][Math.floor(rng() * 6)];
  return ['sakura', 'momiji', 'sakura', 'bamboo', 'matsu'][Math.floor(rng() * 5)];
}

/* ============================================================
   Layout — a tended garden: one planting per bed, even spacing
   left to right, and growth (not crowding) once the beds are full.
   ============================================================ */
const JG_ROWS = [0.10, 0.36, 0.62, 0.88];

/* plant beds live in 0..1 (x, t); the water lives in 0..100 percent, with
   t = 0 at the back of the ground band. Convert, then reject anything whose
   footprint would land in a pond or the river. */
function jgOnWater(x, t, stage) {
  if (stage < 5) return false;
  const px = x * 100, py = 44 + t * 55;   /* matches bottomPct = 1 + (1 - t) * 55 */
  const inEllipse = (o, pad) => Math.pow((px - o.cx) / (o.rx + pad), 2) + Math.pow((py - o.cy) / (o.ry + pad * 0.5), 2) < 1;
  if (inEllipse(JG_UPPER, 3)) return true;
  if (stage >= 6) {
    if (inEllipse(JG_LOWER, 3)) return true;
    /* channel: distance to the flow centreline */
    const [ax, ay] = JG_FLOW[0], [bx, by] = JG_FLOW[1];
    const vx = bx - ax, vy = by - ay;
    const u = Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / (vx * vx + vy * vy)));
    const d = Math.hypot(px - (ax + vx * u), (py - (ay + vy * u)) * 1.9);
    if (d < 8) return true;
  }
  return false;
}

function jgCells(stage, wide) {
  const cols = wide ? 8 : 5, cw = 1 / cols;
  const upper = stage >= 5, lower = stage >= 6;
  const cells = [];
  for (let r = 0; r < JG_ROWS.length; r++) {
    for (let c = 0; c < cols; c++) {
      const x = (c + 0.5) / cols, t = JG_ROWS[r];
      if (upper && t > 0.5 && t < 0.8 && x > 0.12 && x < 0.46) continue;  /* upper pond */
      if (lower && t > 0.5 && t < 0.8 && x >= 0.3 && x < 0.62) continue;   /* river */
      if (lower && t > 0.8 && x > 0.46 && x < 0.92) continue;             /* lower pond */
      const low = t > 0.8 && x < 0.4;                             /* under the stats: ground cover only */
      cells.push({ x, t, cw, low, key: r + '-' + c });
    }
  }
  return cells.sort((a, b) => (jgHash('cell' + a.key) % 9973) - (jgHash('cell' + b.key) % 9973));
}

function jgBuildItems(data, stage) {
  const courses = ST.allCourses(data).filter(c => c.semId === 'sem-fs26');
  const cmap = {}; courses.forEach(c => { cmap[c.id] = c; });
  const streak = data.streak || 0;
  const heroes = [], features = [], sessions = [], milestones = [];

  if (stage >= 7 || streak >= 14) heroes.push({ id: 'jg-matsu', kind: 'matsu', wise: true, maturity: 1, anchor: [0.86, 0.36], label: 'The Great Pine', sub: 'Highest stage · a garden that keeps its shape' });
  if (stage >= 5 || streak >= 10) heroes.push({ id: 'jg-momiji', kind: 'momiji', maturity: 1, anchor: [0.14, 0.36], label: 'The Old Maple', sub: 'Weeks of steady study' });
  if (stage >= 3) features.push({ id: 'jg-hut', kind: 'hut', maturity: 1, anchor: [0.3, 0.1], label: 'Tea house', sub: 'Built once the grasses came in' });
  if (stage >= 4) features.push({ id: 'jg-lantern', kind: 'lantern', maturity: 1, anchor: [0.16, 0.62], label: 'Stone lantern', sub: 'Unlocked at the bamboo grove' });
  if (streak >= 5) features.push({ id: 'jg-kiku', kind: 'kiku', maturity: 1, anchor: [0.82, 0.62], label: streak + '-day streak', sub: 'Golden kiku · keep it alive' });

  const todayT = new Date(ST.iso(ST.today) + 'T00:00:00').getTime();
  data.sessions.filter(s => s.semId === 'sem-fs26').forEach(s => {
    const c = cmap[s.courseId]; if (!c) return;
    const rng = jgRng(jgHash(s.id + '-jp'));
    const maturity = Math.min(1, s.minutes / 90);
    const ageDays = (todayT - new Date((s.start || '').slice(0, 10) + 'T00:00:00').getTime()) / 86400000;
    sessions.push({ id: s.id, kind: jgPick(rng, maturity), color: ST.color(c.color), maturity, minutes: s.minutes, fresh: ageDays <= 1.5,
      label: c.name, sub: s.preset + ' · ' + ST.fmtMins(s.minutes) + (ageDays <= 1.5 ? ' · freshly planted' : '') });
  });
  sessions.sort((a, b) => b.minutes - a.minutes);

  courses.forEach(c => (c.tasks || []).forEach(t => {
    if (t.total > 0 && t.done >= t.total) {
      const rng = jgRng(jgHash(t.id + '-ms'));
      milestones.push({ id: t.id, kind: rng() < 0.5 ? 'sakura' : 'tsutsuji', color: ST.color(c.color), maturity: 1, label: c.name, sub: 'Milestone · ' + t.title });
    }
  }));

  return { heroes, features, sessions, milestones };
}

function jgAmbient(kind, i) {
  const rng = jgRng(jgHash('jamb-' + i));
  return { id: 'jamb-' + i, kind, maturity: 0.24 + rng() * 0.36, ambient: true };
}

function jgLayout(data, stage, wide) {
  const cells = jgCells(stage, wide);
  const free = cells.slice();
  const TALL = { momiji: 1, sakura: 1, matsu: 1, bamboo: 1, kiku: 1, lantern: 1, hut: 1 };
  const take = (anchor, rng, tall) => {
    const pool = tall ? free.filter(c => !c.low) : free;
    if (!pool.length) return null;
    let pick = pool[0];
    if (anchor) {
      let bd = Infinity;
      pool.forEach(c => { const d = Math.hypot((c.x - anchor[0]) * 1.4, c.t - anchor[1]); if (d < bd) { bd = d; pick = c; } });
    }
    free.splice(free.indexOf(pick), 1);
    const j = rng || jgRng(1);
    let x = Math.max(0.05, Math.min(0.95, pick.x + (j() - 0.5) * pick.cw * 0.44));
    let t = pick.t + (j() - 0.5) * 0.05;
    for (let k = 0; k < 4 && jgOnWater(x, t, stage); k++) {
      x = Math.max(0.05, Math.min(0.95, pick.x + (j() - 0.5) * pick.cw * 0.44));
      t = pick.t + (j() - 0.5) * 0.05;
    }
    if (jgOnWater(x, t, stage)) return null;
    return { x, t, cw: pick.cw, low: pick.low };
  };

  const { heroes, features, sessions, milestones } = jgBuildItems(data, stage);
  const placed = [];
  const push = (it, anchor) => {
    const spot = take(anchor, jgRng(jgHash(it.id + '-cell')), !!TALL[it.kind]);
    if (!spot) return false;
    placed.push({ ...it, ...spot });
    return true;
  };

  heroes.forEach(h => push(h, h.anchor));
  features.forEach(f => push(f, f.anchor));
  milestones.forEach(m => push(m));

  /* sessions get a bed each while beds remain; the rest feed growth */
  let surplus = 0;
  sessions.forEach(s => { if (!push(s)) surplus++; });

  /* ambient plantings fill the remaining beds, never beyond them */
  const FILL = [0.12, 0.26, 0.38, 0.48, 0.56, 0.62, 0.68, 0.74, 0.8];
  const target = Math.round(free.length * (FILL[stage] || 0.2));
  for (let i = 0; i < target; i++) {
    const rng = jgRng(jgHash('jamb-' + i));
    const r = rng();
    let kind;
    if (stage <= 1) kind = r < 0.6 ? 'moss' : 'shoot';
    else if (stage <= 3) kind = r < 0.34 ? 'moss' : r < 0.62 ? 'suzuki' : r < 0.84 ? 'shoot' : 'stone';
    else if (stage <= 5) kind = r < 0.26 ? 'moss' : r < 0.46 ? 'suzuki' : r < 0.62 ? 'fern' : r < 0.78 ? 'hagi' : r < 0.92 ? 'tsutsuji' : 'stone';
    else kind = r < 0.18 ? 'moss' : r < 0.32 ? 'suzuki' : r < 0.44 ? 'fern' : r < 0.58 ? 'hagi' : r < 0.72 ? 'tsutsuji' : r < 0.84 ? 'bamboo' : r < 0.93 ? 'momiji' : 'stone';
    const spot = free.find(c => c.low);
    push(jgAmbient(spot && i % 2 === 0 ? (r < 0.6 ? 'moss' : 'suzuki') : kind, i));
  }

  /* a full garden keeps growing upward, not outward */
  const fullness = 1 - free.length / Math.max(1, cells.length);
  const growth = 1 + Math.min(0.32, surplus * 0.02) + Math.max(0, fullness - 0.7) * 0.35;
  return { plants: placed.sort((a, b) => a.t - b.t), growth, surplus };
}

const JG_STYLE = `
@media (prefers-reduced-motion: no-preference) {
  .jg-in { animation: growUp .9s cubic-bezier(.2,.8,.3,1) both; transform-origin: bottom center; }
  .jg-sway svg { animation: jgSway var(--jg-dur, 6s) ease-in-out var(--jg-del, 0s) infinite; transform-origin: 50% 100%; }
  @keyframes jgSway { 0%, 100% { transform: rotate(-1.2deg); } 50% { transform: rotate(1.4deg); } }
  .jg-firefly { animation: jgTwinkle var(--jg-dur, 3s) ease-in-out var(--jg-del, 0s) infinite; }
  @keyframes jgTwinkle { 0%, 100% { opacity: .15; transform: translateY(0); } 50% { opacity: .9; transform: translateY(-5px); } }
  .jg-petal { animation: jgFall var(--jg-dur, 9s) linear var(--jg-del, 0s) infinite; }
  @keyframes jgFall { 0% { opacity: 0; transform: translate(0,-8px) rotate(0deg); } 12% { opacity: .9; }
    100% { opacity: 0; transform: translate(-34px, 150px) rotate(220deg); } }
  .jg-koi { animation: jgSwim var(--jg-dur, 16s) ease-in-out var(--jg-del, 0s) infinite alternate; }
  @keyframes jgSwim { 0% { transform: translate(-14px, 3px) rotate(-8deg); } 50% { transform: translate(10px, -4px) rotate(6deg); }
    100% { transform: translate(20px, 4px) rotate(-4deg); } }
  .jg-ripple { animation: jgRipple var(--jg-dur, 5s) ease-out var(--jg-del, 0s) infinite; }
  @keyframes jgRipple { 0% { transform: scale(.35); opacity: .5; } 100% { transform: scale(1.5); opacity: 0; } }
  .jg-flow { animation: jgFlow var(--jg-dur, 7s) linear var(--jg-del, 0s) infinite; }
  @keyframes jgFlow { to { stroke-dashoffset: var(--jg-off, -120); } }
}
.jg-fresh svg { filter: brightness(1.12) saturate(1.1); }
`;

/* ---------- water: upper pond, river, lower pond, bridge ---------- */
const JG_WATER = 'oklch(0.44 0.045 225)';

function JGKoi({ tone, left, top, dur, del, flip }) {
  return (
    <span className="jg-koi" style={{ position: 'absolute', left: left + '%', top: top + '%', display: 'block',
      '--jg-dur': dur + 's', '--jg-del': del + 's' }}>
      <svg viewBox="0 0 22 10" width="18" style={{ display: 'block', opacity: 0.92, transform: 'rotate(' + (flip || 0) + 'deg)' }}>
        <ellipse cx="9" cy="5" rx="7" ry="2.6" fill={tone}></ellipse>
        <path d="M16 5 L 21 2.2 L 20 5 L 21 7.8 Z" fill={tone} opacity="0.85"></path>
        <ellipse cx="7" cy="4.2" rx="2.2" ry="1.2" fill="rgba(255,255,255,.55)"></ellipse>
      </svg>
    </span>
  );
}

/* Water is drawn in a PIXEL viewBox (0 0 boxW boxH) so nothing is stretched:
   stroke widths stay uniform and the channel keeps its true on-screen width.
   Anchors are still authored as widget percentages. */
const JG_UPPER = { cx: 63, cy: 64, rx: 9.5, ry: 3.8 };
const JG_LOWER = { cx: 78, cy: 93, rx: 12.5, ry: 5 };
/* centreline of the stream as a cubic: both ends sit INSIDE a pond so the
   junctions are buried rather than butted, and the two control points give it
   a shallow meander instead of a single bow. */
const JG_FLOW = [[64.6, 65.8], [65.2, 74.5], [74.2, 81], [76.5, 91.5]];

function JGKoiSet({ box, fish }) {
  return fish.map((k, i) => (
    <JGKoi key={i} tone={k[0]} left={box.cx - box.rx + k[1] * box.rx * 2} top={box.cy - box.ry * 0.4 + k[2] * box.ry} dur={12 + i * 4} del={-i * 3} flip={k[3]}></JGKoi>
  ));
}

function JGWater({ stage, boxW = 390, boxH = 205 }) {
  const lower = stage >= 6;
  const X = p => p / 100 * boxW, Y = p => p / 100 * boxH;
  const rim = jgMix(JG_STONE, 'black', 46);
  const bank = jgMix(JG_STONE, 'black', 26);
  const deep = jgMix(JG_WATER, 'black', 18), lit = jgMix(JG_WATER, 'white', 16);

  /* stream: sample the cubic centreline, then offset it by a half-width that
     grows downstream and flares into a delta at the mouth. Both ends run a
     little way inside their pond, so the junctions read as one body of water. */
  const CP = JG_FLOW.map(p => [X(p[0]), Y(p[1])]);
  const bez = (u, i) => { const v = 1 - u;
    return v * v * v * CP[0][i] + 3 * v * v * u * CP[1][i] + 3 * v * u * u * CP[2][i] + u * u * u * CP[3][i]; };
  const dbez = (u, i) => { const v = 1 - u;
    return 3 * v * v * (CP[1][i] - CP[0][i]) + 6 * v * u * (CP[2][i] - CP[1][i]) + 3 * u * u * (CP[3][i] - CP[2][i]); };
  const at = u => [bez(u, 0), bez(u, 1)];
  /* [nx, ny, fx, fy] — unit normal and unit flow at u */
  const norm = u => { const tx = dbez(u, 0), ty = dbez(u, 1), m = Math.hypot(tx, ty) || 1;
    return [-ty / m, tx / m, tx / m, ty / m]; };
  const halfW = u => boxW * (0.011 + 0.015 * u) + (u > 0.76 ? boxW * 0.032 * Math.pow((u - 0.76) / 0.24, 2) : 0);
  const SN = 28;
  const offs = (sgn, k) => Array.from({ length: SN + 1 }, (_, i) => {
    const u = i / SN, p = at(u), n = norm(u);
    const wob = sgn ? Math.sin(u * 9.1 + (sgn > 0 ? 0 : 2.3)) * boxW * 0.005 : 0;   /* bank irregularity */
    const h = (halfW(u) + wob) * (sgn || k);
    return [p[0] + n[0] * h, p[1] + n[1] * h];
  });
  const poly = pts => pts.map((p, i) => (i ? 'L' : 'M') + p[0].toFixed(1) + ' ' + p[1].toFixed(1)).join(' ');
  const eL = offs(1), eR = offs(-1);
  const river = poly(eL) + ' ' + poly(eR.slice().reverse()).replace('M', 'L') + ' Z';
  const thread = k => poly(offs(0, k));

  /* the bridge crosses at u = 0.46, square to the flow there */
  const uB = 0.46, nB = norm(uB), cross = at(uB);
  const nx = nB[0], ny = nB[1], fx = nB[2], fy = nB[3];
  const hwMid = halfW(uB);

  const ell = (o, grow) => <ellipse cx={X(o.cx)} cy={Y(o.cy)} rx={X(o.rx) + grow} ry={Y(o.ry) + grow}></ellipse>;

  /* bridge: bank-to-bank in the SAME pixel space as the water. The deck is a
     quadrilateral extruded along the FLOW direction (not screen-Y), so it has
     real width across the river; the camber is raised in screen-Y on both edges. */
  const foot = hwMid + Math.max(20, boxW * 0.07);            /* well onto the land */
  const plank = Math.max(13, boxW * 0.038);                  /* walking width */
  const clampY = y => Math.max(plank, Math.min(boxH - plank * 1.2, y));
  const L = [cross[0] - nx * foot - fx * plank / 2, clampY(cross[1] - ny * foot - fy * plank / 2)];
  const R = [cross[0] + nx * foot - fx * plank / 2, clampY(cross[1] + ny * foot - fy * plank / 2)];
  /* a span seen end-on is foreshortened: the arch only shows as far as the
     span is horizontal on screen */
  const spanLen = Math.hypot(R[0] - L[0], R[1] - L[1]);
  const rise = Math.max(6, boxH * 0.075) * Math.abs(R[0] - L[0]) / Math.max(1, spanLen);
  const arc = (p, q, dy) => {
    const c = [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2 - rise * 2 + dy];
    return { d: 'M' + p[0] + ' ' + (p[1] + dy) + ' Q ' + c[0] + ' ' + c[1] + ' ' + q[0] + ' ' + (q[1] + dy), c: c };
  };
  const nearArc = arc(L, R, 0);
  const farL = [L[0] + fx * plank, L[1] + fy * plank], farR = [R[0] + fx * plank, R[1] + fy * plank];
  const farArc = arc(farL, farR, 0);
  const deckBody = nearArc.d + ' L ' + farR[0] + ' ' + farR[1] +
    ' Q ' + farArc.c[0] + ' ' + farArc.c[1] + ' ' + farL[0] + ' ' + farL[1] + ' Z';
  const onArc = (p, q, c, u) => [
    (1 - u) * (1 - u) * p[0] + 2 * (1 - u) * u * c[0] + u * u * q[0],
    (1 - u) * (1 - u) * p[1] + 2 * (1 - u) * u * c[1] + u * u * q[1],
  ];
  const postH = Math.max(9, boxH * 0.055);
  const railC = [nearArc.c[0], nearArc.c[1] - postH];
  const railTop = 'M' + L[0] + ' ' + (L[1] - postH) + ' Q ' + railC[0] + ' ' + railC[1] + ' ' + R[0] + ' ' + (R[1] - postH);

  return (
    <React.Fragment>
      <svg viewBox={'0 0 ' + boxW + ' ' + boxH} preserveAspectRatio="none"
        style={{ position: 'absolute', left: 0, top: 0, width: '100%', height: '100%', zIndex: 24, pointerEvents: 'none' }}>
        <defs>
          <linearGradient id="jgWaterFill" gradientUnits="userSpaceOnUse" x1={boxW * 0.5} y1={boxH * 0.55} x2={boxW * 0.72} y2={boxH}>
            <stop offset="0%" stopColor={lit}></stop>
            <stop offset="55%" stopColor={JG_WATER}></stop>
            <stop offset="100%" stopColor={deep}></stop>
          </linearGradient>
          <clipPath id="jgWaterClip">
            {ell(JG_UPPER, 0)}
            {lower && ell(JG_LOWER, 0)}
            {lower && <path d={river}></path>}
          </clipPath>
        </defs>
        {/* damp earth, then stone rim, then water */}
        <g fill={jgMix(JG_SOIL, 'black', 46)} stroke={jgMix(JG_SOIL, 'black', 46)} strokeWidth="7" strokeLinejoin="round" opacity="0.55">
          {ell(JG_UPPER, 3)}
          {lower && ell(JG_LOWER, 3)}
          {lower && <path d={river}></path>}
        </g>
        <g fill={rim} stroke={rim} strokeWidth="3" strokeLinejoin="round">
          {ell(JG_UPPER, 1)}
          {lower && ell(JG_LOWER, 1)}
          {lower && <path d={river}></path>}
        </g>
        <g fill="url(#jgWaterFill)">
          {ell(JG_UPPER, 0)}
          {lower && ell(JG_LOWER, 0)}
          {lower && <path d={river}></path>}
        </g>
        {/* sheen + current */}
        <ellipse cx={X(JG_UPPER.cx - 3)} cy={Y(JG_UPPER.cy - 1.2)} rx={X(JG_UPPER.rx) * 0.44} ry={Y(JG_UPPER.ry) * 0.34} fill="rgba(255,255,255,.16)"></ellipse>
        {lower && <ellipse cx={X(JG_LOWER.cx - 4)} cy={Y(JG_LOWER.cy - 1.4)} rx={X(JG_LOWER.rx) * 0.4} ry={Y(JG_LOWER.ry) * 0.32} fill="rgba(255,255,255,.14)"></ellipse>}
        {lower && (
          <g fill="none" strokeLinecap="round">
            <path d={poly(eL)} stroke="rgba(0,0,0,.26)" strokeWidth="1.5"></path>
            <path d={poly(eR)} stroke="rgba(255,255,255,.2)" strokeWidth="1.1"></path>
            {[[-0.5, 7.5, 0], [0, 6.2, -1.4], [0.52, 8.4, -2.8]].map((t, i) => (
              <path key={'th' + i} className="jg-flow" d={thread(t[0])} stroke="rgba(255,255,255,.3)" strokeWidth="1.1"
                strokeDasharray="6 17" style={{ '--jg-dur': t[1] + 's', '--jg-del': t[2] + 's', '--jg-off': -92 }}></path>
            ))}
            {[0.2, 0.35, 0.52, 0.68].map((u, i) => {
              const p = at(u), n = norm(u), h = halfW(u) * 0.62;
              return <path key={'rf' + i} stroke="rgba(255,255,255,.15)" strokeWidth="1"
                d={'M' + (p[0] - n[0] * h) + ' ' + (p[1] - n[1] * h) + ' Q ' + (p[0] + n[2] * 3.5) + ' ' + (p[1] + n[3] * 3.5) +
                  ' ' + (p[0] + n[0] * h) + ' ' + (p[1] + n[1] * h)}></path>;
            })}
          </g>
        )}
        {/* junctions: a stone lip where the stream leaves the upper pond, a
            spreading delta where it enters the lower one */}
        {lower && (() => {
          const hd = at(0.05), hn = norm(0.05), hw = halfW(0.05);
          const mo = at(0.95), mn = norm(0.95), mw = halfW(0.95);
          const arc = (p, n, h, d, col, w) => 'M' + (p[0] - n[0] * h) + ' ' + (p[1] - n[1] * h) +
            ' Q ' + (p[0] + n[2] * d) + ' ' + (p[1] + n[3] * d) + ' ' + (p[0] + n[0] * h) + ' ' + (p[1] + n[1] * h);
          return (
            <g fill="none" clipPath="url(#jgWaterClip)">
              <path d={arc(hd, hn, hw, 4)} stroke="rgba(255,255,255,.45)" strokeWidth="1.3"></path>
              {[0.55, 0.85, 1.15].map((k, i) => (
                <path key={'dl' + i} d={arc(mo, mn, mw * k, 5 * k)} strokeWidth="1"
                  stroke={'rgba(255,255,255,' + (0.28 - i * 0.07).toFixed(2) + ')'}></path>
              ))}
            </g>
          );
        })()}
        {lower && (() => {
          const hd = at(0.05), hn = norm(0.05), hw = halfW(0.05);
          return (
            <g fill="none">
              <ellipse cx={hd[0] - hn[0] * (hw + 5.2)} cy={hd[1] - hn[1] * (hw + 5.2)} rx="5" ry="3" fill={jgMix(JG_STONE, 'black', 22)}></ellipse>
              <ellipse cx={hd[0] + hn[0] * (hw + 3.4)} cy={hd[1] + hn[1] * (hw + 3.4)} rx="4.3" ry="2.6" fill={jgMix(JG_STONE, 'black', 34)}></ellipse>
            </g>
          );
        })()}

        {/* bridge, drawn in the same pixel space */}
        {lower && (
          <g>
            <ellipse cx={L[0] + fx * plank / 2} cy={L[1] + fy * plank / 2 + 2} rx={plank * 0.8} ry={plank * 0.34} fill={jgMix(JG_STONE, 'black', 48)}></ellipse>
            <ellipse cx={R[0] + fx * plank / 2} cy={R[1] + fy * plank / 2 + 2} rx={plank * 0.8} ry={plank * 0.34} fill={jgMix(JG_STONE, 'black', 48)}></ellipse>
            <path d={deckBody} fill={jgMix(JG_MAPLE, 'black', 18)}></path>
            <path d={farArc.d} stroke={jgMix(JG_MAPLE, 'black', 48)} strokeWidth="3" fill="none"></path>
            <path d={nearArc.d} stroke={jgMix(JG_MAPLE, 'white', 26)} strokeWidth="1.6" fill="none"></path>
            {[0.12, 0.5, 0.88].map((u, i) => {
              const p = onArc(L, R, nearArc.c, u);
              return <line key={'pl' + i} x1={p[0]} y1={p[1]} x2={p[0] + fx * plank} y2={p[1] + fy * plank}
                stroke={jgMix(JG_MAPLE, 'black', 36)} strokeWidth="1.1" opacity="0.7"></line>;
            })}
            {[0.06, 0.28, 0.5, 0.72, 0.94].map((u, i) => {
              const p = onArc(L, R, nearArc.c, u);
              return <rect key={i} x={p[0] - 1.9} y={p[1] - postH} width="3.8" height={postH + 1} rx="1.3" fill={jgMix(JG_MAPLE, 'black', 34)}></rect>;
            })}
            <path d={railTop} stroke={jgMix(JG_MAPLE, 'black', 20)} strokeWidth="3.4" fill="none" strokeLinecap="round"></path>
            <path d={railTop} stroke={jgMix(JG_MAPLE, 'white', 24)} strokeWidth="1.2" fill="none" strokeLinecap="round"></path>
          </g>
        )}
      </svg>

      {/* koi, pads, ripples */}
      <div style={{ position: 'absolute', left: 0, top: 0, width: '100%', height: '100%', zIndex: 25, pointerEvents: 'none' }}>
        <JGKoiSet box={JG_UPPER} fish={[['oklch(0.72 0.15 45)', 0.3, 0.4, 0], ['oklch(0.9 0.02 60)', 0.64, 1, -6]]}></JGKoiSet>
        {lower && <JGKoiSet box={JG_LOWER} fish={[['oklch(0.66 0.16 30)', 0.24, 0.35, -8], ['oklch(0.9 0.02 60)', 0.54, 1.1, 5], ['oklch(0.72 0.15 45)', 0.78, 0.5, -4]]}></JGKoiSet>}
        {[[JG_UPPER, 0.14, 1, 7]].concat(lower ? [[JG_LOWER, 0.12, 0.95, 9], [JG_LOWER, 0.84, 1.15, 7]] : []).map((p, i) => (
          <span key={i} style={{ position: 'absolute', left: (p[0].cx - p[0].rx + p[1] * p[0].rx * 2) + '%', top: (p[0].cy - p[0].ry * 0.3 + p[2] * p[0].ry) + '%',
            width: p[3], height: p[3] * 0.62, borderRadius: '50%', background: jgMix(JG_MOSS, 'black', 12), opacity: 0.85 }}></span>
        ))}
        <span className="jg-ripple" style={{ position: 'absolute', left: (JG_UPPER.cx + 2) + '%', top: (JG_UPPER.cy + 0.6) + '%', width: 14, height: 5,
          borderRadius: '50%', border: '1px solid rgba(255,255,255,.32)', '--jg-dur': '5s' }}></span>
        {lower && <span className="jg-ripple" style={{ position: 'absolute', left: (JG_LOWER.cx - 2) + '%', top: (JG_LOWER.cy + 0.6) + '%', width: 16, height: 6,
          borderRadius: '50%', border: '1px solid rgba(255,255,255,.28)', '--jg-dur': '6.5s', '--jg-del': '-2s' }}></span>}
      </div>

    </React.Fragment>
  );
}

function JapaneseGarden({ data, height = 205, compact }) {
  const stage = jgStage(data);
  const wkTotal = ST.weeklyFocus(data).reduce((a, d) => a + d.minutes, 0);
  const [tip, setTip] = React.useState(null);
  const wrapRef = React.useRef(null);
  const [boxW, setBoxW] = React.useState(390);
  const wide = boxW > 430;

  React.useLayoutEffect(() => {
    const el = wrapRef.current; if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(() => setBoxW(el.clientWidth));
    ro.observe(el); return () => ro.disconnect();
  }, []);

  const { plants, growth } = React.useMemo(() => jgLayout(data, stage, wide), [data, stage, wide]);
  const streak = data.streak || 0;
  const fireflies = streak >= 5 ? Math.min(7, 2 + Math.floor(streak / 3)) : 0;
  const petals = stage >= 6 ? 7 : stage >= 5 ? 4 : 0;

  const showTip = (e, p) => {
    const r = wrapRef.current.getBoundingClientRect();
    setTip({ x: Math.max(8, Math.min(r.width - 8, e.clientX - r.left)), y: Math.max(26, e.clientY - r.top), label: p.label, sub: p.sub });
  };

  return (
    <div ref={wrapRef} style={{ position: 'relative', height, borderRadius: 'var(--r-md)', overflow: 'hidden',
      border: '1px solid var(--line-soft)',
      background: 'linear-gradient(180deg, var(--garden-sky), color-mix(in oklab, var(--garden-sky), var(--garden-soil) 30%))' }}>
      <style>{JG_STYLE}</style>

      <div style={{ position: 'absolute', top: 12, right: 18, width: 34, height: 34, borderRadius: 99,
        background: jgMix(JG_MAPLE, 'white', 55), opacity: 0.16 }}></div>

      <svg preserveAspectRatio="none" viewBox="0 0 100 20" style={{ position: 'absolute', left: 0, right: 0, bottom: '38%', width: '100%', height: '22%', opacity: 0.3 }}>
        <path d="M0 20 Q 22 3 44 11 Q 62 17 78 6 Q 90 -1 100 9 L 100 20 Z" fill={JG_PINE}></path>
      </svg>

      {/* raked gravel + ground */}
      <div style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: '64%' }}>
        <svg preserveAspectRatio="none" viewBox="0 0 100 8" style={{ position: 'absolute', top: -7, left: 0, width: '100%', height: 8, display: 'block' }}>
          <path d="M0 8 Q 15 2.5 33 4.6 T 66 3.4 T 100 5.4 L 100 8 Z" fill="var(--garden-soil)" opacity="0.42"></path>
        </svg>
        <div style={{ position: 'absolute', inset: 0, opacity: 0.55,
          background: 'linear-gradient(180deg, color-mix(in oklab, var(--garden-soil), var(--garden-sky) 42%), var(--garden-soil))' }}></div>
        <div style={{ position: 'absolute', inset: 0, opacity: 0.1,
          backgroundImage: 'repeating-radial-gradient(120% 60% at 50% 118%, transparent 0 11px, var(--ink-4) 11px 12px)' }}></div>
      </div>

      {stage >= 5 && <JGWater stage={stage} boxW={boxW} boxH={height}></JGWater>}

      {/* plantings */}
      {plants.map((p, i) => {
        const Species = JG_SPECIES[p.kind]; if (!Species) return null;
        const rng = jgRng(jgHash(p.id + '-jdraw'));
        const depth = 0.62 + 0.38 * p.t;
        const bed = boxW * (p.cw || 0.125);
        const g = p.ambient ? 1 + (growth - 1) * 0.3 : growth;
        const bottomPct = 1 + (1 - p.t) * 55;
        const headroom = height * (1 - bottomPct / 100) * 0.94 / 1.21;
        let w = Math.min(bed * 0.66 * (JG_BASE_W[p.kind] || 1) * depth * (0.8 + 0.25 * (p.maturity || 0.5)) * g, headroom, height * 0.46);
        if (p.low) w = Math.min(w, height * 0.17);
        const interactive = !p.ambient;
        const half = (w / 2 + 2) / Math.max(1, boxW);   /* full canopy stays inside the frame */
        const left = Math.max(half, Math.min(1 - half, p.x));
        return (
          <div key={p.id} className={'jg-in' + (p.fresh ? ' jg-fresh' : '')}
            onMouseEnter={interactive ? (e) => showTip(e, p) : undefined}
            onMouseLeave={interactive ? () => setTip(null) : undefined}
            style={{ position: 'absolute', left: (left * 100) + '%', bottom: bottomPct + '%', width: w,
              marginLeft: -w / 2, zIndex: Math.round(p.t * 40) + 1,
              opacity: p.ambient ? 0.62 + 0.32 * p.t : 0.82 + 0.18 * p.t,
              animationDelay: ((i % 14) * 0.045) + 's', pointerEvents: interactive ? 'auto' : 'none' }}>
            {p.kind !== 'moss' && (
              <span style={{ position: 'absolute', left: '50%', bottom: -2, transform: 'translateX(-50%)',
                width: w * 0.55, height: w * 0.14, borderRadius: '50%',
                background: 'radial-gradient(closest-side, rgba(0,0,0,.3), transparent)', zIndex: -1, pointerEvents: 'none' }}></span>
            )}
            <span className={p.ambient || i > 8 ? '' : 'jg-sway'} style={{ display: 'block', '--jg-dur': (5 + (jgHash(p.id) % 40) / 10) + 's', '--jg-del': (-(jgHash(p.id) % 60) / 10) + 's' }}>
              <svg viewBox="0 0 48 58" shapeRendering="geometricPrecision" style={{ display: 'block', width: '100%', overflow: 'visible' }}>
                <Species rng={rng} color={p.color} maturity={p.maturity} wise={p.wise} detail={p.ambient ? 0 : 1}></Species>
              </svg>
            </span>
          </div>
        );
      })}

      {Array.from({ length: petals }).map((_, i) => {
        const rng = jgRng(jgHash('petal-' + i));
        return <span key={i} className="jg-petal" style={{ position: 'absolute', left: (10 + rng() * 80) + '%', top: (-4 + rng() * 20) + '%',
          width: 5, height: 4, borderRadius: '60% 40% 60% 40%', background: JG_SAKURA, opacity: 0.85, zIndex: 44, pointerEvents: 'none',
          '--jg-dur': (7 + rng() * 6) + 's', '--jg-del': (-rng() * 8) + 's' }}></span>;
      })}

      {Array.from({ length: fireflies }).map((_, i) => {
        const rng = jgRng(jgHash('jfly-' + i));
        return <span key={i} className="jg-firefly" style={{ position: 'absolute', left: (8 + rng() * 84) + '%', top: (14 + rng() * 36) + '%',
          width: 3.5, height: 3.5, borderRadius: 99, background: 'var(--warn)', zIndex: 45,
          boxShadow: '0 0 7px 1.5px color-mix(in oklab, var(--warn), transparent 30%)',
          '--jg-dur': (2.4 + rng() * 2.4) + 's', '--jg-del': (-rng() * 4) + 's' }}></span>;
      })}

      <div style={{ position: 'absolute', top: 12, left: 14, display: 'flex', alignItems: 'center', gap: 8, zIndex: 50 }}>
        <div className="eyebrow" style={{ color: 'var(--garden)', fontSize: 9.5 }}>Japanese Garden</div>
        <div style={{ display: 'flex', gap: 2.5 }}>
          {Array.from({ length: 8 }).map((_, i) => (
            <span key={i} style={{ width: 4.5, height: 4.5, borderRadius: 99,
              background: i < stage ? 'var(--garden)' : 'var(--line)', transition: 'background .4s' }}></span>
          ))}
        </div>
      </div>

      <div style={{ position: 'absolute', bottom: 10, left: 10, zIndex: 50, display: 'flex', alignItems: 'baseline', gap: 7,
        padding: '4px 10px 4px 8px', borderRadius: 'var(--r-sm)',
        background: 'color-mix(in oklab, var(--surface), transparent 30%)', backdropFilter: 'blur(3px)' }}>
        <span className="serif" style={{ fontSize: 15, color: 'var(--ink)', fontStyle: 'italic' }}>{JG_STAGE_NAMES[stage]}</span>
        <span className="mono" style={{ fontSize: 10.5, color: 'var(--ink-3)' }}>{ST.fmtMins(wkTotal)} this week</span>
      </div>

      {stage === 0 && wkTotal === 0 && !compact && (
        <div className="mono" style={{ position: 'absolute', bottom: '40%', left: '50%', transform: 'translateX(-50%)', zIndex: 50,
          fontSize: 10, color: 'var(--ink-4)', letterSpacing: '0.06em', whiteSpace: 'nowrap' }}>
          log a session to set the first stone
        </div>
      )}

      {tip && (
        <div style={{ position: 'absolute', left: tip.x, top: tip.y - 14, transform: 'translate(-50%, -100%)', zIndex: 60,
          pointerEvents: 'none', padding: '6px 10px', borderRadius: 'var(--r-sm)', maxWidth: 220,
          background: 'var(--surface-raised)', border: '1px solid var(--line)', boxShadow: 'var(--shadow-md)' }}>
          <div style={{ fontSize: 11.5, fontWeight: 600, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{tip.label}</div>
          {tip.sub && <div className="mono" style={{ fontSize: 9.5, color: 'var(--ink-3)', marginTop: 2, whiteSpace: 'nowrap' }}>{tip.sub}</div>}
        </div>
      )}
    </div>
  );
}

window.JapaneseGarden = JapaneseGarden;
window.jgStage = jgStage;
window.JG_STAGE_NAMES = JG_STAGE_NAMES;
