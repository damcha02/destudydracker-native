/* ============================================================
   Focus Fossil — geological strata study visualization
   Turns study history into a sedimentary core sample.
   Each day = a column, each study session = a mineral layer.
   ============================================================ */

// ---- Fossil palette (self-contained warm theme) ----
const FP = {
  bg:           '#1a1510',
  surface:      '#231e17',
  surfaceLight: '#2e2720',
  amber:        '#d4a04a',
  amberDim:     '#9e7a38',
  amberSoft:    'rgba(212,160,74,0.15)',
  amberGlow:    'rgba(212,160,74,0.3)',
  bone:         '#e8dcc8',
  boneDim:      '#b8a890',
  clay:         '#8a7660',
  sand:         '#c4a878',
  earth:        '#3d3428',
  earthLight:   '#4d4234',
  bedrock:      '#100c06',
};

// ---- Milestone thresholds ----
const FOSSIL_MS = [
  { hours: 10,   label: 'Seed Fossil',      desc: '10 hours of study unearthed' },
  { hours: 25,   label: 'Shell Fragment',    desc: '25 hours — patterns forming' },
  { hours: 50,   label: 'Ammonite',          desc: '50 hours — taking shape' },
  { hours: 100,  label: 'Crystal Cluster',   desc: '100 hours crystallized' },
  { hours: 250,  label: 'Complete Specimen', desc: '250 hours — a rare find' },
  { hours: 500,  label: 'Ancient Artifact',  desc: '500 hours — legendary' },
  { hours: 1000, label: 'Golden Record',     desc: '1000 hours — transcendent' },
];

// ---- Deterministic hash for organic variation ----
function fRand(seed) {
  let s = Math.imul(seed | 0, 2654435761);
  s = Math.imul((s >>> 16) ^ s, 0x45d9f3b);
  s = Math.imul((s >>> 16) ^ s, 0x45d9f3b);
  return (((s >>> 16) ^ s) >>> 0) / 4294967296;
}

// ---- Milestone SVG icons ----
function FossilMsIcon({ hours, size = 14 }) {
  const s = { width: size, height: size, display: 'block' };
  if (hours <= 10) return (
    <svg viewBox="0 0 16 16" fill="none" style={s}>
      <circle cx="8" cy="8" r="5" stroke="currentColor" strokeWidth="1.4" />
      <circle cx="8" cy="8" r="2" fill="currentColor" />
    </svg>
  );
  if (hours <= 25) return (
    <svg viewBox="0 0 16 16" fill="none" style={s}>
      <path d="M10.5 3C7 2.5 4 6 4 9.5S6 14 8 14s4-2 4-5.5S11.5 3 10.5 3z"
        stroke="currentColor" strokeWidth="1.3" />
      <path d="M6.5 9.5c1-2 2.5-3 3.5-2.5" stroke="currentColor" strokeWidth="0.9" opacity="0.5" />
    </svg>
  );
  if (hours <= 50) return (
    <svg viewBox="0 0 16 16" fill="none" style={s}>
      <circle cx="8" cy="8" r="5.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M8 5a3 3 0 1 0 0 6" stroke="currentColor" strokeWidth="1.1" />
      <circle cx="8" cy="8" r="1.3" fill="currentColor" />
    </svg>
  );
  if (hours <= 100) return (
    <svg viewBox="0 0 16 16" fill="none" style={s}>
      <path d="M8 1L12.5 6 8 15 3.5 6z" stroke="currentColor" strokeWidth="1.3" />
      <path d="M3.5 6h9" stroke="currentColor" strokeWidth="0.9" opacity="0.6" />
    </svg>
  );
  return (
    <svg viewBox="0 0 16 16" fill="none" style={s}>
      <path d="M8 1.5l2 4.2h4.5l-3.5 2.8 1.2 4.3L8 10.5l-4.2 2.3 1.2-4.3L1.5 5.7H6z"
        stroke="currentColor" strokeWidth="1.1" fill="currentColor" fillOpacity="0.2" />
    </svg>
  );
}

// ---- Stat display ----
function FossilStat({ label, value, sub }) {
  return (
    <div style={{ minWidth: 0, flex: '1 1 auto' }}>
      <div style={{
        fontFamily: 'var(--font-mono)', fontSize: 9.5, fontWeight: 600,
        letterSpacing: '0.1em', textTransform: 'uppercase',
        color: FP.clay, marginBottom: 4,
      }}>{label}</div>
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 5 }}>
        <span style={{
          fontFamily: 'var(--font-mono)', fontSize: 17, fontWeight: 600,
          color: FP.bone, letterSpacing: '-0.01em', lineHeight: 1,
        }}>{value}</span>
        {sub && <span style={{ fontSize: 10, color: FP.clay }}>{sub}</span>}
      </div>
    </div>
  );
}

// ---- Strata header icon ----
function StrataIcon({ size = 14 }) {
  return (
    <svg width={size} height={size} viewBox="0 0 14 14" fill="none">
      <rect x="1" y="1.5" width="12" height="2.8" rx="1.2" fill={FP.amber} opacity="0.3" />
      <rect x="1" y="5.5" width="12" height="2.8" rx="1.2" fill={FP.amber} opacity="0.5" />
      <rect x="1" y="9.5" width="12" height="2.8" rx="1.2" fill={FP.amber} opacity="0.75" />
    </svg>
  );
}

// ---- Format minutes ----
function fossilFmt(m) {
  const h = Math.floor(m / 60), mm = m % 60;
  return h > 0 ? (mm > 0 ? `${h}h ${mm}m` : `${h}h`) : `${m}m`;
}

// ---- Format date ----
function fossilDate(ds) {
  const d = new Date(ds + 'T00:00:00');
  return d.toLocaleDateString('en-GB', { weekday: 'short', day: 'numeric', month: 'short' });
}

/* ============================================================
   Main component
   ============================================================ */
function FocusFossil({ data }) {
  const [range, setRange] = React.useState(60);
  const [tip, setTip] = React.useState(null);
  const [activeMs, setActiveMs] = React.useState(null);

  // ---- Process data ----
  const info = React.useMemo(() => {
    const courses = ST.allCourses(data).filter(c => c.semId === 'sem-fs26');
    const courseMap = {};
    courses.forEach(c => { courseMap[c.id] = c; });

    const relevant = data.sessions
      .filter(s => courseMap[s.courseId])
      .sort((a, b) => (a.start || '').localeCompare(b.start || ''));

    const allTimeMins = relevant.reduce((a, s) => a + s.minutes, 0);
    const rangeStart = ST.addDays(-range + 1);

    // Cumulative before visible window
    let cum = 0;
    relevant.forEach(s => {
      if ((s.start || '').slice(0, 10) < rangeStart) cum += s.minutes;
    });

    const days = [];
    const msHits = [];
    let maxMins = 30;

    for (let i = range - 1; i >= 0; i--) {
      const ds = ST.addDays(-i);
      const daySess = relevant.filter(s => (s.start || '').slice(0, 10) === ds);
      const totMins = daySess.reduce((a, s) => a + s.minutes, 0);
      const prevCum = cum;
      cum += totMins;
      if (totMins > maxMins) maxMins = totMins;

      // Milestone crossings
      FOSSIL_MS.forEach(m => {
        if (prevCum < m.hours * 60 && cum >= m.hours * 60) {
          msHits.push({ ...m, dayIdx: days.length, date: ds });
        }
      });

      // Group by course
      const byCourse = {};
      daySess.forEach(s => {
        const c = courseMap[s.courseId];
        if (!c) return;
        if (!byCourse[c.id]) byCourse[c.id] = { course: c, mins: 0 };
        byCourse[c.id].mins += s.minutes;
      });

      days.push({
        date: ds, isToday: i === 0, totMins,
        layers: Object.values(byCourse).sort((a, b) => b.mins - a.mins),
        cum, idx: days.length,
      });
    }

    const activeDays = days.filter(d => d.totMins > 0).length;
    const biggest = days.reduce((mx, d) => d.totMins > mx.totMins ? d : mx, { totMins: 0, date: '' });
    const achieved = FOSSIL_MS.filter(m => allTimeMins >= m.hours * 60);
    const latest = achieved.length ? achieved[achieved.length - 1] : null;

    const courseSet = new Set();
    days.forEach(d => d.layers.forEach(l => courseSet.add(l.course.id)));
    const activeCourses = courses.filter(c => courseSet.has(c.id));

    return { days, msHits, allTimeMins, activeDays, biggest, maxMins, activeCourses, achieved, latest };
  }, [data, range]);

  const { days, msHits, allTimeMins, activeDays, biggest, maxMins, activeCourses, latest } = info;

  const MAX_H = 130;
  const colW = range <= 14 ? 32 : range <= 30 ? 16 : 10;
  const colGap = range <= 14 ? 4 : range <= 30 ? 3 : 2;
  const hovDay = tip ? days[tip.idx] : null;

  // ---- Empty state ----
  if (allTimeMins === 0) {
    return (
      <div style={{
        background: `linear-gradient(175deg, ${FP.surface}, ${FP.bg})`,
        borderRadius: 16, border: `1px solid ${FP.earth}`,
        padding: '48px 32px', textAlign: 'center',
        boxShadow: `0 8px 32px rgba(0,0,0,0.4), inset 0 1px 0 ${FP.earthLight}`,
      }}>
        <div style={{ margin: '0 auto 16px', opacity: 0.4 }}><StrataIcon size={40} /></div>
        <div style={{ fontFamily: 'var(--font-serif)', fontSize: 18, color: FP.bone, marginBottom: 8 }}>
          No fossil record yet
        </div>
        <div style={{ fontSize: 13, color: FP.clay, lineHeight: 1.6, maxWidth: 320, margin: '0 auto' }}>
          Complete your first study block to begin uncovering your fossil record.
        </div>
      </div>
    );
  }

  return (
    <div className="fossil-card" style={{
      background: `linear-gradient(175deg, ${FP.surface} 0%, ${FP.bg} 55%)`,
      borderRadius: 16, border: `1px solid ${FP.earth}`,
      overflow: 'visible', fontFamily: 'var(--font-sans)', position: 'relative',
      boxShadow: `0 10px 40px rgba(0,0,0,0.5), inset 0 1px 0 ${FP.earthLight}`,
    }}>

      {/* ======== Header ======== */}
      <div style={{ padding: '22px 24px 0', position: 'relative', zIndex: 2 }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 16 }}>
          <div>
            <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 10, fontWeight: 600,
              letterSpacing: '0.14em', textTransform: 'uppercase',
              color: FP.amberDim, marginBottom: 6,
              display: 'flex', alignItems: 'center', gap: 6,
            }}>
              <StrataIcon size={13} />
              Study strata
            </div>
            <div style={{
              fontFamily: 'var(--font-serif)', fontSize: 22, fontWeight: 500,
              color: FP.bone, letterSpacing: '-0.015em', lineHeight: 1.15,
            }}>Focus Fossil</div>
          </div>
          <div style={{ textAlign: 'right', flexShrink: 0 }}>
            <div style={{
              fontFamily: 'var(--font-mono)', fontSize: 24, fontWeight: 600,
              color: FP.amber, lineHeight: 1.15, letterSpacing: '-0.02em',
              whiteSpace: 'nowrap',
            }}>{fossilFmt(allTimeMins)}</div>
            <div style={{ fontSize: 11, color: FP.clay, marginTop: 3 }}>total focused</div>
          </div>
        </div>

        {/* Range toggle */}
        <div style={{
          display: 'inline-flex', gap: 2, marginTop: 14,
          background: FP.bedrock, borderRadius: 8, padding: 3,
        }}>
          {[14, 30, 60].map(r => (
            <button key={r} onClick={() => { setRange(r); setTip(null); }} style={{
              padding: '5px 14px', fontSize: 11, fontWeight: 600,
              fontFamily: 'var(--font-mono)', border: 'none', borderRadius: 6,
              cursor: 'pointer', transition: 'all 0.15s',
              background: r === range ? FP.earthLight : 'transparent',
              color: r === range ? FP.amber : FP.clay,
              boxShadow: r === range ? `0 1px 4px rgba(0,0,0,0.3)` : 'none',
            }}>{r}d</button>
          ))}
        </div>
      </div>

      {/* ======== Strata Timeline ======== */}
      <div style={{ padding: '18px 24px 0', position: 'relative', zIndex: 2 }}>
        <div className="fossil-strata" style={{
          position: 'relative',
          background: `linear-gradient(180deg, ${FP.bedrock}90, ${FP.bedrock}50 55%, ${FP.earth}18)`,
          borderRadius: 10, padding: '14px 10px 8px',
          border: `1px solid ${FP.earth}50`,
          boxShadow: `inset 0 -20px 40px -10px rgba(140,100,50,0.06)`,
        }}>
          {/* Warm bedrock glow at bottom */}
          <div style={{
            position: 'absolute', left: 0, right: 0, bottom: 0, height: 30,
            background: `linear-gradient(180deg, transparent, rgba(160,120,50,0.06))`,
            borderRadius: '0 0 10px 10px', pointerEvents: 'none',
          }} />
          {/* Depth guide lines */}
          {[0.25, 0.5, 0.75].map(pct => (
            <div key={pct} style={{
              position: 'absolute', left: 10, right: 10,
              top: 14 + MAX_H * (1 - pct),
              height: 1, pointerEvents: 'none',
              background: `linear-gradient(90deg, transparent, ${FP.earth}28 15%, ${FP.earth}28 85%, transparent)`,
            }} />
          ))}

          {/* Day columns */}
          <div style={{
            display: 'flex', gap: colGap, alignItems: 'flex-end',
            height: MAX_H, overflowX: 'auto',
            scrollbarWidth: 'none',
          }}>
            {days.map((day, i) => {
              // Square-root scale: makes small sessions visible, big ones don't dominate
              const h = day.totMins > 0 ? Math.max(6, Math.sqrt(day.totMins / maxMins) * MAX_H) : 0;
              const isHov = tip?.idx === i;
              const hasTip = tip !== null;

              return (
                <div key={day.date}
                  onMouseEnter={(e) => {
                    const r = e.currentTarget.getBoundingClientRect();
                    setTip({ idx: i, x: r.left + r.width / 2, y: r.top });
                  }}
                  onMouseLeave={() => setTip(null)}
                  style={{
                    width: colW, flexShrink: 0,
                    height: '100%', display: 'flex', flexDirection: 'column',
                    justifyContent: 'flex-end', cursor: 'pointer',
                    position: 'relative',
                    opacity: hasTip ? (isHov ? 1 : 0.45) : 1,
                    transition: 'opacity 0.18s',
                  }}>

                  {/* Empty day: subtle eroded mark */}
                  {day.totMins === 0 && (
                    <div style={{
                      display: 'flex', flexDirection: 'column', alignItems: 'center',
                      justifyContent: 'flex-end', height: '100%', paddingBottom: 2,
                    }}>
                      <div style={{
                        width: Math.max(2, colW * 0.3), height: 6,
                        borderRadius: 1,
                        background: FP.earth,
                        opacity: 0.25,
                      }} />
                    </div>
                  )}

                  {/* Strata layers */}
                  {day.totMins > 0 && (
                    <div style={{
                      height: h, display: 'flex', flexDirection: 'column-reverse',
                      gap: 1,
                      filter: isHov ? `brightness(1.3) drop-shadow(0 0 6px ${FP.amberGlow})` : 'none',
                      transition: 'filter 0.18s',
                    }}>
                      {day.layers.map((layer, li) => {
                        const lh = Math.max(4, (layer.mins / day.totMins) * h - 1);
                        const seed = i * 97 + li * 31;
                        const col = ST.color(layer.course.color);
                        const wPct = 82 + fRand(seed) * 18;
                        const r1 = 1.5 + fRand(seed + 1) * 4;
                        const r2 = 1.5 + fRand(seed + 2) * 4;
                        const r3 = 0.5 + fRand(seed + 3) * 2.5;
                        const r4 = 0.5 + fRand(seed + 4) * 2.5;

                        return (
                          <div key={layer.course.id} style={{
                            height: lh, flexShrink: 0,
                            background: `linear-gradient(175deg, ${col}, color-mix(in srgb, ${col} 80%, #1a1510))`,
                            borderRadius: `${r1}px ${r2}px ${r3}px ${r4}px`,
                            width: `${wPct}%`, margin: '0 auto',
                            boxShadow: `inset 0 1px 0 rgba(255,255,255,0.15), inset 0 -1px 0 rgba(0,0,0,0.2)`,
                          }} />
                        );
                      })}
                    </div>
                  )}

                  {/* Today flag */}
                  {day.isToday && (
                    <>
                      <div style={{
                        position: 'absolute', top: -6, left: '50%',
                        transform: 'translateX(-50%)', zIndex: 3,
                      }}>
                        <svg width="16" height="22" viewBox="0 0 16 22" fill="none">
                          <line x1="2.5" y1="1" x2="2.5" y2="21" stroke={FP.amber} strokeWidth="1.8" opacity="0.5" />
                          <polygon points="3,1 14,5 3,9" fill={FP.amber} />
                        </svg>
                      </div>
                      {/* Glow under today column */}
                      <div style={{
                        position: 'absolute', bottom: -4, left: '50%',
                        transform: 'translateX(-50%)',
                        width: colW + 6, height: 4, borderRadius: 2,
                        background: FP.amber, opacity: 0.35,
                        boxShadow: `0 0 10px ${FP.amber}60`,
                      }} />
                    </>
                  )}
                </div>
              );
            })}
          </div>

          {/* Bedrock line */}
          <div style={{
            height: 3,
            background: `linear-gradient(90deg, ${FP.earth}40, ${FP.amber}55, ${FP.earthLight}aa, ${FP.amber}55, ${FP.earth}40)`,
            borderRadius: 1.5, marginTop: 4,
            boxShadow: `0 1px 4px rgba(160,120,50,0.15)`,
          }} />

          {/* Milestone markers */}
          {msHits.length > 0 && (
            <div style={{
              display: 'flex', gap: colGap, height: 22, marginTop: 5,
            }}>
              {days.map((day, i) => {
                const ms = msHits.find(m => m.dayIdx === i);
                if (!ms) return <div key={i} style={{ width: colW, flexShrink: 0 }} />;
                const isActive = activeMs === ms.hours;
                return (
                  <div key={i} style={{
                    width: colW, flexShrink: 0,
                    display: 'flex', flexDirection: 'column', alignItems: 'center',
                    position: 'relative', cursor: 'pointer',
                    color: FP.amber,
                  }} onClick={(e) => { e.stopPropagation(); setActiveMs(isActive ? null : ms.hours); }}>
                    <div style={{
                      animation: 'fossilPulse 2.5s ease-in-out infinite',
                      filter: isActive ? `drop-shadow(0 0 4px ${FP.amberGlow})` : 'none',
                    }}>
                      <FossilMsIcon hours={ms.hours} size={13} />
                    </div>
                    <div style={{
                      fontFamily: 'var(--font-mono)', fontSize: 7, fontWeight: 700,
                      color: FP.amberDim, marginTop: 1, lineHeight: 1,
                    }}>{ms.hours}h</div>

                    {/* Milestone popup */}
                    {isActive && (
                      <div className="fossil-ms-popup" style={{
                        position: 'absolute', bottom: '100%', left: '50%',
                        transform: 'translateX(-50%)', marginBottom: 6,
                        background: FP.surface, border: `1px solid ${FP.amber}50`,
                        borderRadius: 9, padding: '9px 14px', whiteSpace: 'nowrap',
                        boxShadow: `0 6px 20px rgba(0,0,0,0.55), 0 0 0 1px ${FP.earth}`,
                        zIndex: 10, textAlign: 'center',
                      }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 6, justifyContent: 'center', marginBottom: 3 }}>
                          <FossilMsIcon hours={ms.hours} size={14} />
                          <span style={{ fontSize: 12, fontWeight: 700, color: FP.amber }}>{ms.label}</span>
                        </div>
                        <div style={{ fontSize: 10.5, color: FP.clay }}>{ms.desc}</div>
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}

          {/* Date labels */}
          <div style={{ display: 'flex', gap: colGap, marginTop: msHits.length > 0 ? 2 : 6, minHeight: 12 }}>
            {days.map((day, i) => {
              const d = new Date(day.date + 'T00:00:00');
              let text = null;
              let strong = false;

              if (range <= 14) {
                text = ['S','M','T','W','T','F','S'][d.getDay()];
                strong = day.isToday;
              } else {
                const isMonthStart = i === 0 || d.getDate() === 1;
                if (isMonthStart) {
                  text = ['Jan','Feb','Mar','Apr','May','Jun','Jul','Aug','Sep','Oct','Nov','Dec'][d.getMonth()];
                  strong = true;
                }
              }

              if (!text) return <div key={i} style={{ width: colW, flexShrink: 0 }} />;

              return (
                <div key={i} style={{
                  width: colW, flexShrink: 0,
                  fontFamily: 'var(--font-mono)',
                  fontSize: range <= 14 ? 9 : 7.5,
                  fontWeight: strong ? 700 : 500,
                  color: day.isToday ? FP.amber : strong ? FP.boneDim : FP.clay,
                  textAlign: 'center',
                  overflow: 'visible', whiteSpace: 'nowrap',
                  lineHeight: 1,
                }}>{text}</div>
              );
            })}
          </div>
        </div>
      </div>

      {/* ======== Hover tooltip (fixed position) ======== */}
      {tip && hovDay && (
        <div style={{
          position: 'fixed', left: tip.x, top: tip.y - 10,
          transform: 'translate(-50%, -100%)',
          background: `linear-gradient(180deg, ${FP.surfaceLight}, ${FP.surface})`,
          border: `1px solid ${FP.earthLight}`,
          borderRadius: 10, padding: '10px 14px', minWidth: 150,
          boxShadow: `0 8px 28px rgba(0,0,0,0.55)`,
          zIndex: 1000, pointerEvents: 'none',
          fontFamily: 'var(--font-sans)',
        }}>
          <div style={{
            fontSize: 11.5, fontWeight: 600, color: FP.bone, marginBottom: 5,
            display: 'flex', alignItems: 'center', gap: 6,
          }}>
            {fossilDate(hovDay.date)}
            {hovDay.isToday && (
              <span style={{
                fontSize: 9, fontFamily: 'var(--font-mono)', fontWeight: 700,
                color: FP.amber, letterSpacing: '0.05em',
              }}>TODAY</span>
            )}
          </div>

          {hovDay.totMins > 0 ? (
            <>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                {hovDay.layers.map(l => (
                  <div key={l.course.id} style={{
                    display: 'flex', alignItems: 'center', gap: 7,
                  }}>
                    <span style={{
                      width: 7, height: 7, borderRadius: 2, flexShrink: 0,
                      background: ST.color(l.course.color),
                      boxShadow: `0 0 3px ${ST.color(l.course.color)}40`,
                    }} />
                    <span style={{ fontSize: 11, color: FP.boneDim, flex: 1 }}>{l.course.name}</span>
                    <span style={{
                      fontFamily: 'var(--font-mono)', fontSize: 10.5, fontWeight: 600,
                      color: FP.bone,
                    }}>{fossilFmt(l.mins)}</span>
                  </div>
                ))}
              </div>
              <div style={{
                borderTop: `1px solid ${FP.earth}60`, paddingTop: 5, marginTop: 6,
                display: 'flex', justifyContent: 'space-between', alignItems: 'center',
              }}>
                <span style={{ fontSize: 10, color: FP.clay }}>Total</span>
                <span style={{
                  fontFamily: 'var(--font-mono)', fontSize: 12, fontWeight: 700, color: FP.amber,
                }}>{fossilFmt(hovDay.totMins)}</span>
              </div>
            </>
          ) : (
            <div style={{ fontSize: 11, color: FP.clay, fontStyle: 'italic' }}>
              No study — rest day
            </div>
          )}
        </div>
      )}

      {/* ======== Legend ======== */}
      <div style={{
        padding: '16px 24px 4px',
        display: 'flex', flexWrap: 'wrap', gap: '6px 18px',
        position: 'relative', zIndex: 2,
      }}>
        <span style={{
          fontFamily: 'var(--font-mono)', fontSize: 9, fontWeight: 600,
          letterSpacing: '0.1em', textTransform: 'uppercase',
          color: FP.earth, marginRight: 4, alignSelf: 'center',
        }}>Courses</span>
        {activeCourses.map(c => (
          <div key={c.id} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{
              width: 8, height: 8, borderRadius: 2,
              background: ST.color(c.color),
              boxShadow: `0 0 4px ${ST.color(c.color)}25`,
            }} />
            <span style={{ fontSize: 11, color: FP.boneDim }}>{c.name}</span>
          </div>
        ))}
      </div>

      {/* ======== Achieved milestones strip ======== */}
      {info.achieved.length > 0 && (
        <div style={{
          padding: '8px 24px',
          display: 'flex', flexWrap: 'wrap', gap: '6px 14px',
          alignItems: 'center',
          position: 'relative', zIndex: 2,
        }}>
          <span style={{
            fontFamily: 'var(--font-mono)', fontSize: 9, fontWeight: 600,
            letterSpacing: '0.1em', textTransform: 'uppercase',
            color: FP.earth, marginRight: 2,
          }}>Discovered</span>
          {info.achieved.map(m => (
            <div key={m.hours} style={{
              display: 'inline-flex', alignItems: 'center', gap: 4,
              padding: '3px 8px', borderRadius: 6,
              background: FP.amberSoft, border: `1px solid ${FP.amber}20`,
              color: FP.amber,
            }}>
              <FossilMsIcon hours={m.hours} size={11} />
              <span style={{ fontFamily: 'var(--font-mono)', fontSize: 9.5, fontWeight: 600 }}>{m.label}</span>
            </div>
          ))}
        </div>
      )}

      {/* ======== Stats footer ======== */}
      <div style={{
        padding: '14px 24px 22px',
        display: 'flex', gap: 20, flexWrap: 'wrap',
        borderTop: `1px solid ${FP.earth}40`,
        marginTop: 4,
        position: 'relative', zIndex: 2,
      }}>
        <FossilStat label="Active days" value={activeDays} sub={`/ ${range}`} />
        <FossilStat label="Streak" value={data.streak} sub="days" />
        {biggest.totMins > 0 && (
          <FossilStat
            label="Biggest day"
            value={fossilFmt(biggest.totMins)}
            sub={(() => {
              try { return new Date(biggest.date + 'T00:00:00').toLocaleDateString('en-GB', { day: 'numeric', month: 'short' }); }
              catch { return ''; }
            })()}
          />
        )}
        {latest && (
          <FossilStat label="Latest find" value={latest.label} sub={`at ${latest.hours}h`} />
        )}
      </div>
    </div>
  );
}

Object.assign(window, { FocusFossil, FP, FOSSIL_MS, FossilMsIcon });
