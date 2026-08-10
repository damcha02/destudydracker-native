/* ============================================================
   Study Tracker — App shell: reducer, header, nav, routing
   ============================================================ */

const STORAGE_KEY = 'studytracker.v1';

function loadState() {
  try { const raw = localStorage.getItem(STORAGE_KEY); if (raw) return JSON.parse(raw); } catch (e) {}
  return JSON.parse(JSON.stringify(STData));
}

function reducer(state, action) {
  const s = JSON.parse(JSON.stringify(state));
  const sem = (id) => s.semesters.find(x => x.id === id);
  const course = (semId, cId) => sem(semId)?.courses.find(c => c.id === cId);
  switch (action.type) {
    case 'toggleSem': { const x = sem(action.semId); if (x) x.expanded = !x.expanded; break; }
    case 'toggleCourse': { const c = course(action.semId, action.courseId); if (c) c.expanded = !c.expanded; break; }
    case 'addSem': s.semesters.unshift({ id: ST.uid('sem'), name: action.name, expanded: true, courses: [], exams: [] }); break;
    case 'removeSem': s.semesters = s.semesters.filter(x => x.id !== action.semId); break;
    case 'addCourse': { const x = sem(action.semId); if (x) x.courses.push({ id: ST.uid('c'), ...action.course, expanded: true, tasks: [] }); break; }
    case 'removeCourse': { const x = sem(action.semId); if (x) x.courses = x.courses.filter(c => c.id !== action.courseId); break; }
    case 'addTask': { const c = course(action.semId, action.courseId); if (c) c.tasks.push({ id: ST.uid('t'), ...action.task }); break; }
    case 'removeTask': { const c = course(action.semId, action.courseId); if (c) c.tasks = c.tasks.filter(t => t.id !== action.taskId); break; }
    case 'progress': { const c = course(action.semId, action.courseId); const t = c?.tasks.find(t => t.id === action.taskId); if (t) t.done = Math.max(0, Math.min(t.total, t.done + action.delta)); break; }
    case 'addExam': { const x = sem(action.semId); if (x) { x.exams = x.exams || []; x.exams.push({ id: ST.uid('e'), ...action.exam }); } break; }
    case 'removeExam': { const x = sem(action.semId); if (x) x.exams = (x.exams || []).filter(e => e.id !== action.examId); break; }
    case 'addSession': s.sessions.push(action.session); break;
    case 'reset': return JSON.parse(JSON.stringify(STData));
    default: break;
  }
  return s;
}

/* ---------- Overall score widget ---------- */
function ScoreBadge({ data }) {
  const score = ST.overallScore(data);
  const st = ST.scoreState(score);
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '7px 8px 7px 14px', borderRadius: 'var(--r-pill)', background: 'var(--surface)', border: '1px solid var(--line)' }}>
      <div>
        <div className="eyebrow" style={{ fontSize: 8.5, marginBottom: 1 }}>Overall score</div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 7 }}>
          <span style={{ width: 7, height: 7, borderRadius: 99, background: st.color }} />
          <span style={{ fontSize: 12.5, fontWeight: 700, color: st.color }}>{st.label}</span>
        </div>
      </div>
      <Ring value={score} size={42} stroke={5} color={st.color}>
        <span className="mono" style={{ fontSize: 14, fontWeight: 700 }}>{score}</span>
      </Ring>
    </div>
  );
}

/* ---------- Palette picker ---------- */
const PALETTES = [
  { id: 'default', name: 'Default', note: 'Slate & soft blue', sw: ['oklch(0.235 0.018 252)', 'oklch(0.70 0.10 245)', 'oklch(0.76 0.07 150)'] },
  { id: 'original', name: 'Original', note: 'Coral on slate', sw: ['#282c34', '#9cdef2', '#e06c75'] },
  { id: 'midnight', name: 'Midnight', note: 'Ink black & red', sw: ['#0d1117', '#c9d1d9', '#f85149'] },
  { id: 'paper', name: 'Paper', note: 'Cream & mustard', sw: ['#faf8f5', '#d5d0c8', '#c5ac4a'] },
  { id: 'cyberpunk', name: 'Cyberpunk', note: 'Neon cyan & magenta', sw: ['#0a0a0f', '#0ff0fc', '#e040fb'] },
  { id: 'retrowave', name: 'Retrowave', note: 'Synthwave pink', sw: ['#1a1a2e', '#533483', '#e94560'] },
  { id: 'forest', name: 'Forest', note: 'Woodland green', sw: ['#1b2a1b', '#a8d5a2', '#7cb871'] },
  { id: 'ocean', name: 'Ocean', note: 'Deep-sea blue', sw: ['#0b1a2c', '#64d2ff', '#4facfe'] },
  { id: 'ume', name: 'Ume', note: 'Plum blossom', sw: ['#2b1b2e', '#f5c2e7', '#f5a0c0'] },
  { id: 'copper', name: 'Copper', note: 'Burnished copper', sw: ['#1c1410', '#e8c39e', '#d4764e'] },
  { id: 'organs', name: 'Organs', note: 'Cream & oxblood', sw: ['#0a0406', '#efe1c8', '#c83240'] },
  { id: 'lavender', name: 'Lavender', note: 'Soft violet', sw: ['#f3eef8', '#cec3de', '#9b6dcc'] },
  { id: 'gpt', name: 'GPT', note: 'Monochrome grey', sw: ['#212121', '#ececec', '#949494'] },
  { id: 'claude', name: 'Claude', note: 'Clay & cream', sw: ['#262624', '#f5f4f0', '#c6613f'] },
  { id: 'cute', name: 'Cute', note: 'Kawaii pink', sw: ['#fff0f5', '#f0c0d0', '#ff6b9d'] },
];
function PalettePicker({ palette, setPalette }) {
  const [open, setOpen] = useState(false);
  const ref = React.useRef(null);
  useEffect(() => {
    if (!open) return;
    const h = (e) => { if (ref.current && !ref.current.contains(e.target)) setOpen(false); };
    document.addEventListener('mousedown', h);
    return () => document.removeEventListener('mousedown', h);
  }, [open]);
  const active = PALETTES.find(p => p.id === palette) || PALETTES[0];
  return (
    <div ref={ref} style={{ position: 'relative' }}>
      <button onClick={() => setOpen(o => !o)} title="Color theme" style={{ height: 38, padding: '0 12px', borderRadius: 'var(--r-md)', border: '1px solid var(--line)', background: 'var(--surface)', color: 'var(--ink-2)', display: 'inline-flex', alignItems: 'center', gap: 8, cursor: 'pointer', fontFamily: 'var(--font-sans)', fontSize: 12.5, fontWeight: 600 }}>
        <span style={{ display: 'inline-flex' }}>
          {active.sw.map((c, i) => <span key={i} style={{ width: 11, height: 11, borderRadius: 99, background: c, marginLeft: i ? -4 : 0, border: '1.5px solid var(--surface)' }} />)}
        </span>
        {active.name}
        <Icon name="chevronDown" size={13} style={{ color: 'var(--ink-4)' }} />
      </button>
      {open && (
        <div className="fade-up" style={{ position: 'absolute', top: '100%', right: 0, marginTop: 8, zIndex: 60, width: 232, background: 'var(--surface-raised)', border: '1px solid var(--line)', borderRadius: 'var(--r-md)', boxShadow: 'var(--shadow-lg)', padding: 6 }}>
          <div className="eyebrow" style={{ padding: '6px 10px 4px' }}>Color theme</div>
          {PALETTES.map(p => {
            const on = p.id === palette;
            return (
              <button key={p.id} onClick={() => { setPalette(p.id); setOpen(false); }} style={{ width: '100%', display: 'flex', alignItems: 'center', gap: 11, padding: '9px 10px', borderRadius: 'var(--r-sm)', cursor: 'pointer', textAlign: 'left', background: on ? 'var(--accent-soft)' : 'transparent', border: '1px solid', borderColor: on ? 'var(--accent-line)' : 'transparent' }}>
                <span style={{ display: 'inline-flex', flexShrink: 0 }}>
                  {p.sw.map((c, i) => <span key={i} style={{ width: 15, height: 15, borderRadius: 99, background: c, marginLeft: i ? -5 : 0, border: '1.5px solid var(--surface-raised)' }} />)}
                </span>
                <span style={{ flex: 1, minWidth: 0 }}>
                  <span style={{ display: 'block', fontSize: 13, fontWeight: 600, color: on ? 'var(--accent)' : 'var(--ink)' }}>{p.name}</span>
                  <span style={{ display: 'block', fontSize: 11, color: 'var(--ink-4)' }}>{p.note}</span>
                </span>
                {on && <Icon name="check" size={15} style={{ color: 'var(--accent)' }} />}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}

/* ---------- Header ---------- */
function Header({ data, theme, setTheme, palette, setPalette, onBackup }) {
  return (
    <header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16, padding: '16px 28px', borderBottom: '1px solid var(--line)', background: 'color-mix(in oklch, var(--bg) 80%, transparent)', backdropFilter: 'blur(12px)', position: 'sticky', top: 0, zIndex: 50 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
        <div style={{ width: 38, height: 38, borderRadius: 11, background: 'linear-gradient(145deg, var(--accent), var(--accent-strong))', display: 'grid', placeItems: 'center', color: 'oklch(0.18 0.02 250)', boxShadow: 'var(--shadow-md)' }}>
          <Icon name="leaf" size={21} />
        </div>
        <div>
          <div className="serif" style={{ fontSize: 18, fontWeight: 600, letterSpacing: '-0.01em', lineHeight: 1.1 }}>Study Tracker</div>
          <div style={{ fontSize: 11, color: 'var(--ink-4)', marginTop: 1 }}>Semester planning, focus tracking, workload clarity, and study notes.</div>
        </div>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <ScoreBadge data={data} />
        <Btn variant="solid" size="md" icon="download" onClick={onBackup}>Backup JSON</Btn>
        <PalettePicker palette={palette} setPalette={setPalette} />
        <button onClick={() => setTheme(theme === 'dark' ? 'light' : 'dark')} title="Toggle theme" style={{ width: 38, height: 38, borderRadius: 'var(--r-md)', border: '1px solid var(--line)', background: 'var(--surface)', color: 'var(--ink-2)', display: 'grid', placeItems: 'center', cursor: 'pointer' }}>
          <Icon name={theme === 'dark' ? 'sun' : 'moon'} size={18} />
        </button>
      </div>
    </header>
  );
}

/* ---------- Nav ---------- */
const NAV = [
  { id: 'dashboard', label: 'Dashboard', icon: 'dashboard' },
  { id: 'planner', label: 'Planner', icon: 'planner' },
  { id: 'timer', label: 'Timer', icon: 'timer' },
  { id: 'vault', label: 'Vault + AI', icon: 'vault' },
  { id: 'friends', label: 'Friends', icon: 'target' },
];
function Nav({ tab, setTab }) {
  return (
    <nav style={{ display: 'flex', gap: 4, padding: '0 28px', borderBottom: '1px solid var(--line)', background: 'color-mix(in oklch, var(--bg) 80%, transparent)', position: 'sticky', top: 71, zIndex: 49, backdropFilter: 'blur(12px)' }}>
      {NAV.map(n => {
        const active = tab === n.id;
        return (
          <button key={n.id} onClick={() => setTab(n.id)} style={{
            display: 'inline-flex', alignItems: 'center', gap: 8, padding: '14px 16px 13px',
            background: 'none', border: 'none', borderBottom: '2px solid', borderColor: active ? 'var(--accent)' : 'transparent',
            color: active ? 'var(--ink)' : 'var(--ink-4)', fontSize: 13.5, fontWeight: 600, cursor: 'pointer',
            fontFamily: 'var(--font-sans)', transition: 'all .15s', marginBottom: -1,
          }}>
            <Icon name={n.icon} size={17} />{n.label}
          </button>
        );
      })}
    </nav>
  );
}

/* ---------- Toast ---------- */
function Toast({ msg }) {
  if (!msg) return null;
  return (
    <div style={{ position: 'fixed', bottom: 24, left: '50%', transform: 'translateX(-50%)', zIndex: 100,
      padding: '12px 20px', borderRadius: 'var(--r-pill)', background: 'var(--ink)', color: 'var(--bg)',
      fontSize: 13, fontWeight: 600, boxShadow: 'var(--shadow-lg)', display: 'flex', alignItems: 'center', gap: 8 }} className="fade-up">
      <Icon name="check" size={16} />{msg}
    </div>
  );
}

/* ---------- App ---------- */
function App() {
  const [state, dispatch] = React.useReducer(reducer, null, loadState);
  const [tab, setTab] = useState('dashboard');
  const [dashDir, setDashDir] = useState('cockpit');
  const [theme, setTheme] = useState(() => localStorage.getItem('studytracker.theme') || 'dark');
  const [palette, setPalette] = useState(() => localStorage.getItem('studytracker.palette') || 'default');
  const [linkedTaskId, setLinkedTaskId] = useState(state.selectedTaskId || null);
  const [selectedTaskId, setSelectedTaskId] = useState(state.selectedTaskId || null);
  const [toast, setToast] = useState('');

  useEffect(() => { document.documentElement.setAttribute('data-theme', theme); localStorage.setItem('studytracker.theme', theme); }, [theme]);
  useEffect(() => { document.documentElement.setAttribute('data-palette', palette); localStorage.setItem('studytracker.palette', palette); }, [palette]);
  useEffect(() => { try { localStorage.setItem(STORAGE_KEY, JSON.stringify(state)); } catch (e) {} }, [state]);

  const showToast = (m) => { setToast(m); setTimeout(() => setToast(''), 2200); };

  const focusTask = (task) => { setLinkedTaskId(task.id); setSelectedTaskId(task.id); setTab('timer'); showToast(`“${task.title}” sent to timer`); };
  const backup = () => {
    const blob = new Blob([JSON.stringify(state, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob); const a = document.createElement('a');
    a.href = url; a.download = `studytracker-backup-${ST.iso(ST.today)}.json`; a.click(); URL.revokeObjectURL(url);
    showToast('Backup downloaded');
  };

  return (
    <div style={{ minHeight: '100vh' }}>
      <Header data={state} theme={theme} setTheme={setTheme} palette={palette} setPalette={setPalette} onBackup={backup} />
      <Nav tab={tab} setTab={setTab} />
      <main style={{ maxWidth: 1340, margin: '0 auto', padding: '26px 28px 80px' }}>
        {tab === 'dashboard' && <Dashboard data={state} onFocus={focusTask} selectedTaskId={selectedTaskId} onSelectTask={setSelectedTaskId} dir={dashDir} setDir={setDashDir} />}
        {tab === 'planner' && <Planner data={state} dispatch={dispatch} onFocus={focusTask} selectedTaskId={selectedTaskId} />}
        {tab === 'timer' && <Timer data={state} dispatch={(a) => { dispatch(a); if (a.type === 'addSession') showToast('Session saved'); }} linkedTaskId={linkedTaskId} setLinkedTaskId={setLinkedTaskId} />}
        {tab === 'vault' && <VaultAI data={state} dispatch={dispatch} />}
        {tab === 'friends' && <FriendsTab data={state} />}
      </main>
      <Toast msg={toast} />
    </div>
  );
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
