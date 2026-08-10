/* ============================================================
   Dashboard — answers "What should I do today?"
   Three switchable directions: Focus · Cockpit · Analyst
   ============================================================ */

/* ---------- Today card ---------- */
function TodayCard({ data }) {
  const mins = ST.minutesToday(data);
  const mo = ST.momentum(data);
  const dirIcon = mo.dir === 'up' ? 'arrowUp' : mo.dir === 'down' ? 'arrowDown' : 'minus';
  const dirCol = mo.dir === 'up' ? 'var(--ok)' : mo.dir === 'down' ? 'var(--warn)' : 'var(--ink-4)';
  return (
    <Card raised style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between' }}>
        <div>
          <div className="eyebrow" style={{ marginBottom: 6 }}>Today · {ST.today.toLocaleDateString('en-GB', { weekday: 'long', day: 'numeric', month: 'long' })}</div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
            <span className="mono" style={{ fontSize: 42, fontWeight: 600, letterSpacing: '-0.02em', lineHeight: 1 }}>{ST.fmtMins(mins)}</span>
            <span style={{ fontSize: 13, color: 'var(--ink-3)' }}>focused</span>
          </div>
        </div>
        <div style={{ width: 40, height: 40, borderRadius: 12, background: 'var(--accent-soft)', color: 'var(--accent)', display: 'grid', placeItems: 'center', border: '1px solid var(--accent-line)' }}>
          <Icon name="focus" size={20} />
        </div>
      </div>
      <div style={{ display: 'flex', gap: 10 }}>
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 9, padding: '10px 12px', background: 'var(--surface-inset)', borderRadius: 'var(--r-sm)', border: '1px solid var(--line-soft)' }}>
          <Icon name="flame" size={18} style={{ color: 'var(--warn)' }} />
          <div>
            <div className="mono" style={{ fontSize: 17, fontWeight: 600, lineHeight: 1 }}>{data.streak}</div>
            <div className="eyebrow" style={{ fontSize: 9, marginTop: 3 }}>Day streak</div>
          </div>
        </div>
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 9, padding: '10px 12px', background: 'var(--surface-inset)', borderRadius: 'var(--r-sm)', border: '1px solid var(--line-soft)' }}>
          <Icon name={dirIcon} size={18} style={{ color: dirCol }} />
          <div>
            <div className="mono" style={{ fontSize: 17, fontWeight: 600, lineHeight: 1, color: dirCol }}>{mo.pct > 0 ? '+' : ''}{mo.pct}%</div>
            <div className="eyebrow" style={{ fontSize: 9, marginTop: 3 }}>Momentum</div>
          </div>
        </div>
      </div>
    </Card>
  );
}

/* ---------- Weekly focus chart ---------- */
function WeeklyChart({ data, height = 130 }) {
  const wk = ST.weeklyFocus(data);
  const max = Math.max(60, ...wk.map(d => d.minutes));
  return (
    <div style={{ display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between', gap: 8, height }}>
      {wk.map((d, i) => {
        const h = Math.max(3, (d.minutes / max) * (height - 28));
        return (
          <div key={d.day} style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 7, height: '100%', justifyContent: 'flex-end' }}>
            <span className="mono" style={{ fontSize: 9.5, color: d.isToday ? 'var(--accent)' : 'var(--ink-4)', fontWeight: 600 }}>{d.minutes > 0 ? d.minutes : ''}</span>
            <div style={{ width: '100%', maxWidth: 30, height: h, borderRadius: 6,
              background: d.isToday ? 'var(--accent)' : 'var(--surface-2)',
              border: '1px solid', borderColor: d.isToday ? 'var(--accent)' : 'var(--line)',
              transformOrigin: 'bottom', animation: `growUp .6s cubic-bezier(.2,.8,.3,1) ${i * 0.05}s both` }} />
            <span style={{ fontSize: 10.5, color: d.isToday ? 'var(--ink)' : 'var(--ink-4)', fontWeight: d.isToday ? 600 : 400 }}>{d.label}</span>
          </div>
        );
      })}
    </div>
  );
}

/* ---------- Urgent tasks ---------- */
function UrgentTasks({ data, onFocus, selectedId, onSelect, limit = 5 }) {
  const tasks = ST.allTasks(data)
    .filter(t => t.done < t.total)
    .sort((a, b) => ST.urgency(b) - ST.urgency(a))
    .slice(0, limit);
  if (tasks.length === 0) return <Empty icon="check" title="All clear" sub="No urgent tasks right now." />;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      {tasks.map(t => {
        const d = ST.daysUntil(t.due);
        const overdue = ST.isOverdue(t);
        const soon = d != null && d >= 0 && d <= 2;
        const sel = t.id === selectedId;
        return (
          <div key={t.id} onClick={() => onSelect && onSelect(t.id)} style={{
            display: 'flex', alignItems: 'center', gap: 12, padding: '11px 13px', borderRadius: 'var(--r-md)',
            background: sel ? 'var(--accent-soft)' : 'var(--surface-inset)',
            border: '1px solid', borderColor: sel ? 'var(--accent-line)' : 'var(--line-soft)',
            cursor: 'pointer', transition: 'all .15s',
          }}>
            <div style={{ width: 4, alignSelf: 'stretch', borderRadius: 99, background: ST.color(t.color) }} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 5 }}>
                <span style={{ fontSize: 13.5, fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{t.title}</span>
                <PriorityTag p={t.priority} />
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
                <span className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)', display: 'inline-flex', alignItems: 'center', gap: 4 }}><Dot color={t.color} size={6} />{t.courseName}</span>
                <span className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{ST.taskRemaining(t)} units left</span>
              </div>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 6 }}>
              <Chip size="sm" color={overdue ? 'var(--danger)' : soon ? 'var(--warn)' : 'var(--ink-3)'}
                bg={overdue ? 'var(--danger-soft)' : soon ? 'var(--warn-soft)' : 'var(--surface-2)'}
                style={{ borderColor: 'transparent' }}>{ST.fmtDue(t.due)}</Chip>
              <Btn size="sm" variant="ghost" icon="play" onClick={(e) => { e.stopPropagation(); onFocus && onFocus(t); }} style={{ height: 26 }}>Focus</Btn>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ---------- Course radar ---------- */
function CourseRadar({ data }) {
  const courses = ST.allCourses(data).filter(c => c.semId === 'sem-fs26');
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 9 }}>
      {courses.map(c => {
        const health = ST.courseHealth(data, c);
        const mins = ST.courseMinutes(data, c.id);
        const overdue = ST.courseOverdue(c);
        const st = ST.scoreState(health);
        return (
          <div key={c.id} style={{ display: 'flex', alignItems: 'center', gap: 12, paddingBottom: 9, borderBottom: '1px solid var(--line-soft)' }}>
            <Dot color={c.color} size={10} ring />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, marginBottom: 6 }}>
                <span style={{ fontSize: 13, fontWeight: 600, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{c.name}</span>
                <span className="mono" style={{ fontSize: 12, fontWeight: 600, color: st.color }}>{health}</span>
              </div>
              <Bar value={health} color={st.color} height={5} />
              <div style={{ display: 'flex', gap: 11, marginTop: 7 }}>
                <span className="mono" style={{ fontSize: 10, color: 'var(--ink-4)' }}>{c.tasks.length} tasks</span>
                <span className="mono" style={{ fontSize: 10, color: 'var(--ink-4)' }}>{ST.fmtMins(mins)}</span>
                {overdue > 0 && <span className="mono" style={{ fontSize: 10, color: 'var(--danger)' }}>{overdue} overdue</span>}
                <span className="mono" style={{ fontSize: 10, color: 'var(--ink-4)', marginLeft: 'auto' }}>→ {c.target.toFixed(1)}</span>
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ---------- Exam runway ---------- */
function ExamRunway({ data }) {
  const exams = ST.allExams(data)
    .filter(e => { const d = ST.daysUntil(e.date); return d != null && d >= 0; })
    .sort((a, b) => ST.daysUntil(a.date) - ST.daysUntil(b.date));
  if (exams.length === 0) return <Empty icon="cal" title="No upcoming exams" sub="Add exams in the Planner." />;
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 12 }}>
      {exams.map(e => {
        const course = ST.courseById(data, e.courseId);
        const d = ST.daysUntil(e.date);
        const urgent = d <= 10;
        const prepCol = e.prep >= 70 ? 'var(--ok)' : e.prep >= 40 ? 'var(--warn)' : 'var(--danger)';
        return (
          <div key={e.id} style={{ display: 'flex', alignItems: 'center', gap: 13 }}>
            <div style={{ textAlign: 'center', width: 42, flexShrink: 0 }}>
              <div className="mono" style={{ fontSize: 21, fontWeight: 600, lineHeight: 1, color: urgent ? 'var(--warn)' : 'var(--ink)' }}>{d}</div>
              <div className="eyebrow" style={{ fontSize: 8.5, marginTop: 2 }}>days</div>
            </div>
            <div style={{ width: 1, alignSelf: 'stretch', background: 'var(--line)' }} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 3, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{e.title}</div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
                <span className="mono" style={{ fontSize: 10, color: 'var(--ink-4)', display: 'inline-flex', alignItems: 'center', gap: 4 }}>{course && <Dot color={course.color} size={6} />}{course ? course.name : '—'}</span>
                <span className="mono" style={{ fontSize: 10, color: 'var(--ink-4)' }}>{e.weight}% weight</span>
              </div>
            </div>
            <div style={{ width: 54, textAlign: 'right' }}>
              <div className="mono" style={{ fontSize: 13, fontWeight: 600, color: prepCol }}>{e.prep}%</div>
              <div style={{ marginTop: 5 }}><Bar value={e.prep} color={prepCol} height={4} /></div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ============================================================
   Dashboard shell + 3 directions
   ============================================================ */
function Dashboard({ data, onFocus, selectedTaskId, onSelectTask, dir, setDir }) {
  const wkTotal = ST.weeklyFocus(data).reduce((a, d) => a + d.minutes, 0);
  return (
    <div className="fade-up" style={{ display: 'flex', flexDirection: 'column', gap: 18 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: 12 }}>
        <div>
          <h1 className="serif" style={{ fontSize: 27, fontWeight: 500, margin: 0, letterSpacing: '-0.02em' }}>Good morning, Lena</h1>
          <p style={{ margin: '5px 0 0', fontSize: 13.5, color: 'var(--ink-3)' }}>Here's what deserves your focus today.</p>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span className="eyebrow" style={{ fontSize: 9 }}>Layout</span>
          <Segmented size="sm" value={dir} onChange={setDir} options={[
            { value: 'focus', label: 'Focus', icon: 'flag' },
            { value: 'cockpit', label: 'Cockpit', icon: 'grid' },
            { value: 'analyst', label: 'Analyst', icon: 'layers' },
          ]} />
        </div>
      </div>

      {dir === 'cockpit' && (
        <div className="dash-3col" style={{ display: 'grid', gridTemplateColumns: 'minmax(270px,1fr) minmax(330px,1.4fr) minmax(270px,1fr)', gap: 16, alignItems: 'start' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <TodayCard data={data} />
            <Card><KnowledgeGarden data={data} height={215} /></Card>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <Card>
              <CardHead eyebrow="Do this next" icon="bolt" title="Highest-impact tasks" sub="Ranked by deadline, weight & priority" />
              <UrgentTasks data={data} onFocus={onFocus} selectedId={selectedTaskId} onSelect={onSelectTask} limit={4} />
            </Card>
            <Card>
              <CardHead eyebrow="Last 7 days" icon="clock" title="Weekly focus" right={<Chip icon="clock">{ST.fmtMins(wkTotal)}</Chip>} />
              <WeeklyChart data={data} />
            </Card>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <Card><CardHead eyebrow="Course radar" icon="target" title="Where you stand" /><CourseRadar data={data} /></Card>
            <Card><CardHead eyebrow="Exam runway" icon="cal" title="What's coming" /><ExamRunway data={data} /></Card>
          </div>
        </div>
      )}

      {dir === 'focus' && (
        <div className="dash-focus" style={{ display: 'grid', gridTemplateColumns: 'minmax(0,1.6fr) minmax(280px,1fr)', gap: 16, alignItems: 'start' }}>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <Card raised pad={24}>
              <CardHead eyebrow="Do this next" icon="bolt" title="Your top priorities" sub="The few things that move your semester forward most" />
              <UrgentTasks data={data} onFocus={onFocus} selectedId={selectedTaskId} onSelect={onSelectTask} limit={6} />
            </Card>
            <Card><CardHead eyebrow="Exam runway" icon="cal" title="What's coming" /><ExamRunway data={data} /></Card>
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
            <TodayCard data={data} />
            <Card pad={16}><KnowledgeGarden data={data} height={205} /></Card>
            <Card>
              <CardHead eyebrow="Last 7 days" icon="clock" title="Weekly focus" />
              <WeeklyChart data={data} height={110} />
            </Card>
          </div>
        </div>
      )}

      {dir === 'analyst' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
          <div className="dash-stats" style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 16 }}>
            <Card><Stat big label="Focused today" value={ST.fmtMins(ST.minutesToday(data))} icon="focus" /></Card>
            <Card><Stat big label="This week" value={ST.fmtMins(wkTotal)} icon="clock" /></Card>
            <Card><Stat big label="Day streak" value={data.streak} icon="flame" color="var(--warn)" /></Card>
            <Card><Stat big label="Open tasks" value={ST.allTasks(data).filter(t=>t.done<t.total).length} icon="list" /></Card>
          </div>
          <div className="dash-2col" style={{ display: 'grid', gridTemplateColumns: 'minmax(0,1.3fr) minmax(280px,1fr)', gap: 16, alignItems: 'start' }}>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <Card>
                <CardHead eyebrow="Last 7 days" icon="clock" title="Weekly focus" right={<Chip icon="clock">{ST.fmtMins(wkTotal)}</Chip>} />
                <WeeklyChart data={data} height={160} />
              </Card>
              <Card><CardHead eyebrow="Do this next" icon="bolt" title="Highest-impact tasks" /><UrgentTasks data={data} onFocus={onFocus} selectedId={selectedTaskId} onSelect={onSelectTask} limit={4} /></Card>
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
              <Card><CardHead eyebrow="Course radar" icon="target" title="Where you stand" /><CourseRadar data={data} /></Card>
              <Card><CardHead eyebrow="Exam runway" icon="cal" title="What's coming" /><ExamRunway data={data} /></Card>
            </div>
          </div>
        </div>
      )}

      {/* Focus Fossil — shown if fossil.jsx is loaded */}
      {typeof FocusFossil !== 'undefined' && <FocusFossil data={data} />}
    </div>
  );
}

Object.assign(window, { Dashboard, TodayCard, WeeklyChart, UrgentTasks, CourseRadar, ExamRunway });
