/* ============================================================
   Knowledge Garden — grows with study activity (bars style)
   Each course = a plant; height grows with minutes studied.
   Weekly total drives overall lushness / stage.
   ============================================================ */

function KnowledgeGarden({ data, height = 150, compact }) {
  const stage = ST.gardenStage(data);                 // 0..5
  const courses = ST.allCourses(data).filter(c => c.semId === 'sem-fs26');
  const maxMin = Math.max(60, ...courses.map(c => ST.courseMinutes(data, c.id)));
  const wkTotal = ST.weeklyFocus(data).reduce((a, d) => a + d.minutes, 0);

  const stageNames = ['Dormant', 'Sprouting', 'Growing', 'Leafy', 'Blooming', 'Flourishing'];

  return (
    <div style={{ position: 'relative', height, borderRadius: 'var(--r-md)', overflow: 'hidden',
      background: 'linear-gradient(180deg, var(--garden-sky), transparent 75%)',
      border: '1px solid var(--line-soft)' }}>
      {/* soft sun */}
      <div style={{ position: 'absolute', top: 14, right: 18, width: 30, height: 30, borderRadius: 99,
        background: 'radial-gradient(circle, var(--garden) 0%, transparent 70%)', opacity: 0.5 }} />
      {/* soil line */}
      <div style={{ position: 'absolute', left: 0, right: 0, bottom: 0, height: 14,
        background: 'var(--garden-soil)', opacity: 0.5 }} />

      {/* plants */}
      <div style={{ position: 'absolute', left: 0, right: 0, bottom: 14, display: 'flex',
        alignItems: 'flex-end', justifyContent: 'space-around', padding: '0 16px', gap: 8 }}>
        {courses.map((c, i) => {
          const mins = ST.courseMinutes(data, c.id);
          const grow = Math.max(0.12, mins / maxMin);   // 0..1
          const h = (height - 40) * grow;
          const col = ST.color(c.color);
          const leaves = Math.min(4, Math.max(1, Math.round(grow * 4)));
          return (
            <div key={c.id} title={`${c.name} · ${ST.fmtMins(mins)}`}
              style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', position: 'relative', flex: 1, minWidth: 0 }}>
              {/* leaves */}
              <div style={{ position: 'relative', width: 4, height: h, transformOrigin: 'bottom',
                animation: `growUp .8s cubic-bezier(.2,.8,.3,1) ${i * 0.08}s both` }}>
                <div style={{ position: 'absolute', left: 1, top: 0, bottom: 0, width: 2.5,
                  background: `linear-gradient(180deg, ${col}, var(--garden-deep))`, borderRadius: 2 }} />
                {Array.from({ length: leaves }).map((_, li) => {
                  const top = (li + 0.6) / (leaves + 0.4) * h;
                  const side = li % 2 === 0 ? -1 : 1;
                  return (
                    <span key={li} style={{ position: 'absolute', top, left: side < 0 ? -8 : 4,
                      width: 9, height: 6, background: col, opacity: 0.85,
                      borderRadius: side < 0 ? '60% 10% 60% 10%' : '10% 60% 10% 60%',
                      transform: `rotate(${side * 18}deg)` }} />
                  );
                })}
                {/* bud at top when grown */}
                {grow > 0.55 && <span style={{ position: 'absolute', top: -5, left: -2.5,
                  width: 9, height: 9, borderRadius: 99, background: col, boxShadow: `0 0 8px ${col}66` }} />}
              </div>
              {!compact && <Dot color={c.color} size={6} />}
            </div>
          );
        })}
      </div>

      {/* stage label */}
      <div style={{ position: 'absolute', top: 12, left: 14, display: 'flex', alignItems: 'center', gap: 8 }}>
        <div className="eyebrow" style={{ color: 'var(--garden)', fontSize: 9.5 }}>Knowledge Garden</div>
        <div style={{ display: 'flex', gap: 3 }}>
          {Array.from({ length: 5 }).map((_, i) => (
            <span key={i} style={{ width: 5, height: 5, borderRadius: 99,
              background: i < stage ? 'var(--garden)' : 'var(--line)', transition: 'background .4s' }} />
          ))}
        </div>
      </div>
      <div style={{ position: 'absolute', bottom: 20, left: 14, display: 'flex', alignItems: 'baseline', gap: 7 }}>
        <span className="serif" style={{ fontSize: 15, color: 'var(--ink)', fontStyle: 'italic' }}>{stageNames[stage]}</span>
        <span className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{ST.fmtMins(wkTotal)} this week</span>
      </div>
    </div>
  );
}

window.KnowledgeGarden = KnowledgeGarden;
