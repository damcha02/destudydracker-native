/* ============================================================
   Garden of Knowledge — generative, organic, data-driven.

   Mapping (all deterministic — same data → same garden):
   · each study session   → a plant; species picked by hashed id,
                            maturity by minutes, blossom = course color
   · each completed task  → a flower (course color petals)
   · streak ≥ 5           → golden glow-bloom + fireflies
   · stage 5 / streak ≥10 → the Wise Tree
   · stage (0..5)         → ambient grass/sprout density
   Placement: Mitchell best-candidate w/ seeded PRNG per item id,
   in a perspective ground band (t=0 back … t=1 front); scale,
   opacity and z-order follow depth. Keep-out zone under the
   bottom-left stats so text stays readable.
   ============================================================ */

/* ---------- deterministic randomness ---------- */
function gkHash(s) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
  return h >>> 0;
}
function gkRng(seed) {
  let a = seed | 0;
  return function () {
    a = a + 0x6D2B79F5 | 0;
    let t = Math.imul(a ^ a >>> 15, 1 | a);
    t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
    return ((t ^ t >>> 14) >>> 0) / 4294967296;
  };
}
function gkGreen(rng) {
  return `color-mix(in oklab, var(--garden), var(--garden-deep) ${Math.round(15 + rng() * 65)}%)`;
}
function gkMix(a, b, pct) { return `color-mix(in oklab, ${a}, ${b} ${pct}%)`; }

/* ---------- tiny path helpers ---------- */
function gkLeaf(x, y, len, dir, lift = 0.7) {
  const tx = x + dir * len, ty = y - len * lift;
  return `M${x} ${y} Q ${x + dir * len * 0.15} ${y - len * 0.8} ${tx} ${ty} Q ${x + dir * len * 0.72} ${y - len * 0.12} ${x} ${y} Z`;
}

/* ============================================================
   Species — each returns SVG children for viewBox 0 0 48 58,
   anchored at bottom-center (24, 58).
   ============================================================ */
function GKGrass({ rng }) {
  const n = 4 + Math.floor(rng() * 3);
  return (
    <g>
      {Array.from({ length: n }).map((_, i) => {
        const dx = (rng() - 0.5) * 26, h = 15 + rng() * 22;
        return <path key={i} d={`M24 58 Q ${24 + dx * 0.25} ${58 - h * 0.55} ${24 + dx} ${58 - h}`}
          stroke={gkGreen(rng)} strokeWidth={1.4 + rng()} fill="none" strokeLinecap="round" opacity={0.7 + rng() * 0.3} />;
      })}
    </g>
  );
}

function GKSprout({ rng }) {
  const g = gkGreen(rng);
  return (
    <g>
      <path d="M24 58 Q 24.5 51 24 46" stroke="var(--garden-deep)" strokeWidth="1.8" fill="none" strokeLinecap="round" />
      <path d={gkLeaf(24, 47, 8 + rng() * 3, -1)} fill={g} />
      <path d={gkLeaf(24, 47, 7 + rng() * 3, 1)} fill={gkMix(g, 'var(--garden)', 40)} />
    </g>
  );
}

function GKLeafy({ rng, color, maturity }) {
  const h = 24 + rng() * 16 + maturity * 8;
  const bend = (rng() - 0.5) * 9;
  const topX = 24 + bend * 0.7, topY = 58 - h;
  const k = 3 + Math.floor(rng() * 3);
  return (
    <g>
      <path d={`M24 58 Q ${24 + bend} ${58 - h * 0.55} ${topX} ${topY}`}
        stroke="var(--garden-deep)" strokeWidth="1.9" fill="none" strokeLinecap="round" />
      {Array.from({ length: k }).map((_, i) => {
        const f = 0.28 + 0.62 * (i / k);
        const lx = 24 + bend * f, ly = 58 - h * f;
        const dir = i % 2 === 0 ? -1 : 1;
        return <path key={i} d={gkLeaf(lx, ly, 6.5 + rng() * 5 + maturity * 2, dir)} fill={gkGreen(rng)} />;
      })}
      {maturity > 0.6 && <circle cx={topX} cy={topY - 1.5} r="3" fill={color} opacity="0.95" />}
    </g>
  );
}

function GKFlower({ rng, color }) {
  const h = 22 + rng() * 14;
  const bend = (rng() - 0.5) * 7;
  const cx = 24 + bend * 0.7, cy = 58 - h;
  const rot = rng() * 60, petals = 5 + Math.floor(rng() * 2);
  return (
    <g>
      <path d={`M24 58 Q ${24 + bend} ${58 - h * 0.55} ${cx} ${cy}`}
        stroke="var(--garden-deep)" strokeWidth="1.7" fill="none" strokeLinecap="round" />
      <path d={gkLeaf(24 + bend * 0.4, 58 - h * 0.42, 7 + rng() * 3, rng() > 0.5 ? 1 : -1)} fill={gkGreen(rng)} />
      {Array.from({ length: petals }).map((_, i) => (
        <ellipse key={i} cx={cx} cy={cy - 4.4} rx="2.9" ry="4.8" fill={color} opacity="0.92"
          transform={`rotate(${rot + i * (360 / petals)} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r="2.5" fill={gkMix(color, 'white', 45)} />
    </g>
  );
}

function GKHerb({ rng, color }) {
  const stems = 3 + Math.floor(rng() * 2);
  return (
    <g>
      {Array.from({ length: stems }).map((_, i) => {
        const dx = (i - (stems - 1) / 2) * (5 + rng() * 3), h = 20 + rng() * 14;
        const g = gkGreen(rng);
        return (
          <g key={i}>
            <path d={`M24 58 Q ${24 + dx * 0.5} ${58 - h * 0.5} ${24 + dx} ${58 - h}`}
              stroke={g} strokeWidth="1.5" fill="none" strokeLinecap="round" />
            {[0.72, 0.86, 1].map((f, j) => (
              <circle key={j} cx={24 + dx * f} cy={58 - h * f} r={1.7 - j * 0.25} fill={color} opacity="0.9" />
            ))}
          </g>
        );
      })}
    </g>
  );
}

function GKMushroom({ rng }) {
  const capW = 10 + rng() * 4, capH = 9 + rng() * 4;
  const cap = 'oklch(0.63 0.075 40)';
  return (
    <g>
      <rect x="21.6" y="45" width="4.8" height="13" rx="2.4" fill={gkMix('var(--garden-soil)', 'white', 42)} />
      <path d={`M${24 - capW} 46.5 Q 24 ${46.5 - capH * 2} ${24 + capW} 46.5 Z`} fill={cap} />
      <circle cx={24 - capW * 0.4} cy={42.5} r="1.2" fill={gkMix(cap, 'white', 50)} />
      <circle cx={24 + capW * 0.35} cy={40.5} r="0.95" fill={gkMix(cap, 'white', 50)} />
    </g>
  );
}

function GKBush({ rng, color, maturity }) {
  const berries = maturity > 0.55 ? 3 : 0;
  return (
    <g>
      <ellipse cx="15" cy="50" rx={8 + rng() * 3} ry={7 + rng() * 2} fill={gkGreen(rng)} />
      <ellipse cx="30" cy="47" rx={10 + rng() * 3} ry={9 + rng() * 3} fill={gkGreen(rng)} />
      <ellipse cx="24" cy="52" rx={9 + rng() * 2} ry={6 + rng() * 2} fill={gkMix(gkGreen(rng), 'var(--garden)', 30)} />
      {Array.from({ length: berries }).map((_, i) => (
        <circle key={i} cx={12 + rng() * 24} cy={42 + rng() * 10} r="1.7" fill={color} opacity="0.95" />
      ))}
    </g>
  );
}

function GKFern({ rng }) {
  const h = 26 + rng() * 14, curl = 3 + rng() * 4;
  const g = gkGreen(rng);
  const fronds = [];
  for (let i = 0; i < 8; i++) {
    const f = 0.18 + 0.78 * (i / 8);
    const x = 24 + curl * Math.sin(f * 2.4), y = 58 - h * f;
    const len = (1 - f) * 9 + 2;
    fronds.push(<path key={'l' + i} d={`M${x} ${y} Q ${x - len * 0.7} ${y - len * 0.4} ${x - len} ${y - len * 0.9}`} stroke={g} strokeWidth="1.3" fill="none" strokeLinecap="round" />);
    fronds.push(<path key={'r' + i} d={`M${x} ${y} Q ${x + len * 0.7} ${y - len * 0.4} ${x + len} ${y - len * 0.9}`} stroke={g} strokeWidth="1.3" fill="none" strokeLinecap="round" />);
  }
  return (
    <g>
      <path d={`M24 58 Q ${24 + curl * 2} ${58 - h * 0.5} 24 ${58 - h}`}
        stroke="var(--garden-deep)" strokeWidth="1.7" fill="none" strokeLinecap="round" />
      {fronds}
    </g>
  );
}

function GKGlowFlower({ rng }) {
  const cx = 24, cy = 26, rot = rng() * 60;
  return (
    <g style={{ filter: 'drop-shadow(0 0 5px var(--warn))' }}>
      <circle cx={cx} cy={cy} r="11" fill="var(--warn)" opacity="0.13" />
      <path d={`M24 58 Q 26 ${42} ${cx} ${cy}`} stroke="var(--garden-deep)" strokeWidth="1.8" fill="none" strokeLinecap="round" />
      <path d={gkLeaf(25, 46, 8, -1)} fill={gkGreen(rng)} />
      <path d={gkLeaf(25.2, 42, 7, 1)} fill={gkGreen(rng)} />
      {Array.from({ length: 6 }).map((_, i) => (
        <ellipse key={i} cx={cx} cy={cy - 5} rx="3.1" ry="5.4" fill="var(--warn)" opacity="0.95"
          transform={`rotate(${rot + i * 60} ${cx} ${cy})`} />
      ))}
      <circle cx={cx} cy={cy} r="2.8" fill={gkMix('var(--warn)', 'white', 55)} />
    </g>
  );
}

function GKTree({ rng, wise }) {
  const gold = 'var(--warn)';
  return (
    <g style={wise ? { filter: 'drop-shadow(0 0 6px color-mix(in oklab, var(--warn), transparent 45%))' } : null}>
      <path d="M24 58 C 22.5 48 25.5 42 24 32" stroke={gkMix('var(--garden-soil)', 'black', 22)} strokeWidth="3.4" fill="none" strokeLinecap="round" />
      <path d="M24 44 Q 30 40 33 36" stroke={gkMix('var(--garden-soil)', 'black', 22)} strokeWidth="1.8" fill="none" strokeLinecap="round" />
      <path d="M24 40 Q 18 36 15.5 33" stroke={gkMix('var(--garden-soil)', 'black', 22)} strokeWidth="1.8" fill="none" strokeLinecap="round" />
      <circle cx="24" cy="21" r="12.5" fill={gkGreen(rng)} />
      <circle cx="13.5" cy="28" r="8" fill={gkGreen(rng)} />
      <circle cx="34.5" cy="27" r="8.5" fill={gkGreen(rng)} />
      <circle cx="24" cy="30" r="9" fill={gkMix(gkGreen(rng), 'var(--garden)', 35)} />
      {wise && Array.from({ length: 5 }).map((_, i) => (
        <circle key={i} cx={12 + rng() * 24} cy={16 + rng() * 14} r="1.25" fill={gold} />
      ))}
    </g>
  );
}

const GK_SPECIES = {
  grass: GKGrass, sprout: GKSprout, leafy: GKLeafy, flower: GKFlower,
  herb: GKHerb, mushroom: GKMushroom, bush: GKBush, fern: GKFern,
  glow: GKGlowFlower, tree: GKTree,
};
const GK_BASE_W = { grass: 0.82, sprout: 0.52, leafy: 0.95, flower: 0.92, herb: 0.9, mushroom: 0.58, bush: 1.12, fern: 0.95, glow: 1.05, tree: 1.95 };

/* ============================================================
   Garden data → plant list + deterministic placement
   ============================================================ */
function gkPickSpecies(rng, maturity) {
  if (maturity < 0.35) return rng() < 0.5 ? 'sprout' : 'grass';
  if (maturity < 0.7) return ['leafy', 'herb', 'fern', 'mushroom'][Math.floor(rng() * 4)];
  return ['leafy', 'bush', 'flower', 'fern'][Math.floor(rng() * 4)];
}

function gkBuildItems(data, stage, wide) {
  const courses = ST.allCourses(data).filter(c => c.semId === 'sem-fs26');
  const cmap = {}; courses.forEach(c => { cmap[c.id] = c; });
  const items = [];

  // wise tree — very high progress or long streak
  const wise = stage >= 5 || (data.streak || 0) >= 10;
  if (wise) items.push({ id: 'gk-tree', kind: 'tree', wise: true, maturity: 1, label: 'The Wise Tree', sub: 'A flourishing week of study', fixed: true });

  // streak → golden glow bloom
  if ((data.streak || 0) >= 5) {
    items.push({ id: 'gk-streak', kind: 'glow', maturity: 1, label: `${data.streak}-day streak`, sub: 'Rare bloom · keep it alive' });
  }

  // sessions → plants
  const todayT = new Date(ST.iso(ST.today) + 'T00:00:00').getTime();
  data.sessions.filter(s => s.semId === 'sem-fs26').forEach(s => {
    const c = cmap[s.courseId]; if (!c) return;
    const rng = gkRng(gkHash(s.id + '-sp'));
    const maturity = Math.min(1, s.minutes / 90);
    const ageDays = (todayT - new Date((s.start || '').slice(0, 10) + 'T00:00:00').getTime()) / 86400000;
    items.push({
      id: s.id, kind: gkPickSpecies(rng, maturity), color: ST.color(c.color), maturity,
      fresh: ageDays <= 1.5, label: c.name,
      sub: `${s.preset} · ${ST.fmtMins(s.minutes)}${ageDays <= 1.5 ? ' · freshly grown' : ''}`,
    });
  });

  // completed tasks → flowers
  courses.forEach(c => (c.tasks || []).forEach(t => {
    if (t.total > 0 && t.done >= t.total) {
      items.push({ id: t.id, kind: 'flower', color: ST.color(c.color), maturity: 1, label: c.name, sub: `Milestone · ${t.title}` });
    }
  }));

  // ambient filler — density follows stage
  const ambient = Math.round(([4, 8, 12, 16, 21, 25][stage] || 4) * (wide ? 1 : 0.55));
  for (let i = 0; i < ambient; i++) {
    const rng = gkRng(gkHash('amb-' + i));
    items.push({ id: 'amb-' + i, kind: rng() < 0.6 ? 'grass' : (rng() < 0.5 ? 'sprout' : 'fern'), maturity: 0.25 + rng() * 0.3, ambient: true });
  }
  return items;
}

function gkPlace(items) {
  const placed = [];
  for (const it of items) {
    const rng = gkRng(gkHash(it.id + '-pos'));
    if (it.fixed) { placed.push({ ...it, x: 0.68 + rng() * 0.16, t: 0.42 + rng() * 0.16 }); continue; }
    let best = null, bestD = -1;
    for (let k = 0; k < 9; k++) {
      const x = 0.03 + 0.94 * rng();
      const t = Math.pow(rng(), 0.82);
      if (x < 0.42 && t > 0.62) continue;           // keep-out: bottom-left stats
      let d = Infinity;
      for (const p of placed) d = Math.min(d, Math.hypot((x - p.x) * 1.7, t - p.t));
      if (d > bestD) { bestD = d; best = { x, t }; }
    }
    if (!best) best = { x: 0.45 + rng() * 0.3, t: 0.3 + rng() * 0.25 };
    placed.push({ ...it, ...best });
  }
  return placed.sort((a, b) => a.t - b.t);
}

/* ============================================================
   Widget
   ============================================================ */
const GK_STYLE = `
@media (prefers-reduced-motion: no-preference) {
  .gk-plant-in { animation: growUp .9s cubic-bezier(.2,.8,.3,1) both; transform-origin: bottom center; }
  .gk-sway svg { animation: gkSway var(--gk-dur, 5.5s) ease-in-out var(--gk-del, 0s) infinite; transform-origin: 50% 100%; }
  @keyframes gkSway { 0%, 100% { transform: rotate(-1.4deg); } 50% { transform: rotate(1.6deg); } }
  .gk-firefly { animation: gkTwinkle var(--gk-dur, 3s) ease-in-out var(--gk-del, 0s) infinite; }
  @keyframes gkTwinkle { 0%, 100% { opacity: .15; transform: translateY(0); } 50% { opacity: .9; transform: translateY(-5px); } }
  .gk-butterfly { animation: gkDrift 13s ease-in-out infinite alternate; }
  @keyframes gkDrift { 0% { transform: translate(0, 0) rotate(-4deg); } 33% { transform: translate(-46px, 12px) rotate(5deg); }
    66% { transform: translate(-90px, -4px) rotate(-3deg); } 100% { transform: translate(-130px, 10px) rotate(4deg); } }
  .gk-wing-l { animation: gkFlapL .5s ease-in-out infinite alternate; transform-origin: 10px 8px; }
  .gk-wing-r { animation: gkFlapR .5s ease-in-out infinite alternate; transform-origin: 10px 8px; }
  @keyframes gkFlapL { from { transform: scaleX(1); } to { transform: scaleX(.45); } }
  @keyframes gkFlapR { from { transform: scaleX(1); } to { transform: scaleX(.45); } }
}
.gk-fresh svg { filter: brightness(1.14) saturate(1.1); }
`;

function KnowledgeGarden({ data, height = 200, compact }) {
  const stage = ST.gardenStage(data);
  const stageNames = ['Dormant', 'Sprouting', 'Growing', 'Leafy', 'Blooming', 'Flourishing'];
  const wkTotal = ST.weeklyFocus(data).reduce((a, d) => a + d.minutes, 0);
  const [tip, setTip] = React.useState(null);
  const wrapRef = React.useRef(null);
  const [wide, setWide] = React.useState(true);

  React.useLayoutEffect(() => {
    const el = wrapRef.current; if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver(() => setWide(el.clientWidth > 430));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const plants = React.useMemo(() => gkPlace(gkBuildItems(data, stage, wide)), [data, stage, wide]);
  const hScale = Math.max(0.85, Math.min(1.1, height / 200));
  const streak = data.streak || 0;
  const fireflies = streak >= 5 ? Math.min(6, 2 + Math.floor(streak / 3)) : 0;

  const showTip = (e, p) => {
    const r = wrapRef.current.getBoundingClientRect();
    setTip({ x: Math.max(8, Math.min(r.width - 8, e.clientX - r.left)), y: Math.max(26, e.clientY - r.top), label: p.label, sub: p.sub });
  };

  return (
    <div ref={wrapRef} style={{ position: 'relative', height, borderRadius: 'var(--r-md)', overflow: 'hidden',
      border: '1px solid var(--line-soft)',
      background: 'linear-gradient(180deg, var(--garden-sky), color-mix(in oklab, var(--garden-sky), var(--garden-soil) 26%))' }}>
      <style>{GK_STYLE}</style>

      {/* soft sun */}
      <div style={{ position: 'absolute', top: 10, right: 16, width: 44, height: 44, borderRadius: 99,
        background: 'radial-gradient(circle, var(--garden) 0%, transparent 68%)', opacity: 0.45 }}></div>

      {/* ground */}
      <div style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: '62%' }}>
        <svg preserveAspectRatio="none" viewBox="0 0 100 8" style={{ position: 'absolute', top: -7, left: 0, width: '100%', height: 8, display: 'block' }}>
          <path d="M0 8 Q 14 2.5 32 4.8 T 64 3.6 T 100 5.2 L 100 8 Z" fill="var(--garden-soil)" opacity="0.42"></path>
        </svg>
        <div style={{ position: 'absolute', inset: 0, opacity: 0.5,
          background: 'linear-gradient(180deg, color-mix(in oklab, var(--garden-soil), var(--garden-sky) 40%), var(--garden-soil))' }}></div>
        <div style={{ position: 'absolute', inset: 0, opacity: 0.22,
          backgroundImage: 'radial-gradient(circle, var(--ink-4) 0.8px, transparent 1.1px), radial-gradient(circle, var(--ink-4) 0.7px, transparent 1px)',
          backgroundSize: '34px 22px, 23px 17px', backgroundPosition: '0 0, 12px 9px' }}></div>
      </div>

      {/* plants */}
      {plants.map((p, i) => {
        const Species = GK_SPECIES[p.kind];
        if (!Species) return null;
        const rng = gkRng(gkHash(p.id + '-draw'));
        const depth = 0.45 + 0.55 * p.t;
        const w = height * 0.38 * (GK_BASE_W[p.kind] || 1) * depth * (0.72 + 0.38 * (p.maturity || 0.5)) * hScale;
        const bottomPct = 2 + (1 - p.t) * 52;
        const interactive = !p.ambient;
        return (
          <div key={p.id} className={'gk-plant-in' + (p.fresh ? ' gk-fresh' : '')}
            onMouseEnter={interactive ? (e) => showTip(e, p) : undefined}
            onMouseLeave={interactive ? () => setTip(null) : undefined}
            style={{ position: 'absolute', left: `${p.x * 100}%`, bottom: `${bottomPct}%`, width: w,
              transform: 'translateX(-50%)', zIndex: Math.round(p.t * 40) + 1,
              opacity: p.ambient ? 0.5 + 0.35 * p.t : 0.72 + 0.28 * p.t,
              animationDelay: `${(i % 12) * 0.05}s`, pointerEvents: interactive ? 'auto' : 'none' }}>
            <span className="gk-sway" style={{ display: 'block', '--gk-dur': `${4.5 + (gkHash(p.id) % 40) / 10}s`, '--gk-del': `${-(gkHash(p.id) % 60) / 10}s` }}>
              <svg viewBox="0 0 48 58" style={{ display: 'block', width: '100%', overflow: 'visible' }}>
                <Species rng={rng} color={p.color} maturity={p.maturity} wise={p.wise}></Species>
              </svg>
            </span>
          </div>
        );
      })}

      {/* fireflies (streak) */}
      {Array.from({ length: fireflies }).map((_, i) => {
        const rng = gkRng(gkHash('fly-' + i));
        return <span key={i} className="gk-firefly" style={{ position: 'absolute', left: `${8 + rng() * 84}%`, top: `${12 + rng() * 38}%`,
          width: 3.5, height: 3.5, borderRadius: 99, background: 'var(--warn)', zIndex: 45,
          boxShadow: '0 0 7px 1.5px color-mix(in oklab, var(--warn), transparent 30%)',
          '--gk-dur': `${2.4 + rng() * 2.4}s`, '--gk-del': `${-rng() * 4}s` }}></span>;
      })}

      {/* butterfly (blooming+) */}
      {stage >= 4 && (
        <div className="gk-butterfly" style={{ position: 'absolute', top: '20%', right: '14%', zIndex: 46, pointerEvents: 'none' }}>
          <svg viewBox="0 0 20 16" width="17">
            <ellipse className="gk-wing-l" cx="6" cy="7" rx="5" ry="4" fill="var(--accent)" opacity="0.85"></ellipse>
            <ellipse className="gk-wing-r" cx="14" cy="7" rx="5" ry="4" fill="var(--accent)" opacity="0.85"></ellipse>
            <rect x="9.3" y="3.5" width="1.4" height="9" rx="0.7" fill="var(--ink-3)"></rect>
          </svg>
        </div>
      )}

      {/* header */}
      <div style={{ position: 'absolute', top: 12, left: 14, display: 'flex', alignItems: 'center', gap: 8, zIndex: 50 }}>
        <div className="eyebrow" style={{ color: 'var(--garden)', fontSize: 9.5 }}>Garden of Knowledge</div>
        <div style={{ display: 'flex', gap: 3 }}>
          {Array.from({ length: 5 }).map((_, i) => (
            <span key={i} style={{ width: 5, height: 5, borderRadius: 99,
              background: i < stage ? 'var(--garden)' : 'var(--line)', transition: 'background .4s' }}></span>
          ))}
        </div>
      </div>

      {/* stats — bottom-left, inside the placement keep-out zone */}
      <div style={{ position: 'absolute', bottom: 10, left: 10, zIndex: 50, display: 'flex', alignItems: 'baseline', gap: 7,
        padding: '4px 10px 4px 8px', borderRadius: 'var(--r-sm)',
        background: 'color-mix(in oklab, var(--surface), transparent 30%)', backdropFilter: 'blur(3px)' }}>
        <span className="serif" style={{ fontSize: 15, color: 'var(--ink)', fontStyle: 'italic' }}>{stageNames[stage]}</span>
        <span className="mono" style={{ fontSize: 10.5, color: 'var(--ink-3)' }}>{ST.fmtMins(wkTotal)} this week</span>
      </div>

      {/* empty-state nudge */}
      {stage === 0 && wkTotal === 0 && !compact && (
        <div className="mono" style={{ position: 'absolute', bottom: '38%', left: '50%', transform: 'translateX(-50%)', zIndex: 50,
          fontSize: 10, color: 'var(--ink-4)', letterSpacing: '0.06em', whiteSpace: 'nowrap' }}>
          log a session to plant your first sprout
        </div>
      )}

      {/* tooltip */}
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

window.KnowledgeGarden = KnowledgeGarden;
