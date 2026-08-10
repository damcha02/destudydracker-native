/* ============================================================
   Vault — Obsidian-compatible Knowledge Base (redesigned)
   Calm, literary, scalable space-based layout
   ============================================================ */

const VAULT_FOLDERS = ['Daily', 'References', 'Summaries'];

const VAULT_SPACES = [
  { id: 'daily', label: 'Daily', icon: 'cal' },
  { id: 'references', label: 'References', icon: 'book' },
  { id: 'summaries', label: 'Summaries', icon: 'layers' },
];

/* ---- Sample reference markdown for demo ---- */
const SAMPLE_REF_MD = [
  '# Series & Convergence',
  '',
  '## Key Definitions',
  '',
  '**Convergent Series:** A series converges if the sequence of partial sums converges to a finite limit.',
  '',
  '**Absolute Convergence:** A series converges absolutely if the sum of absolute values converges. Absolute convergence implies convergence, but not vice versa.',
  '',
  '## Convergence Tests',
  '',
  '### Ratio Test',
  'Given a series, compute the limit L = lim |a_{n+1} / a_n|:',
  '- If L < 1: converges absolutely',
  '- If L > 1: diverges',
  '- If L = 1: inconclusive',
  '',
  '### Root Test',
  'Compute L = lim sup |a_n|^{1/n}:',
  '- If L < 1: converges absolutely',
  '- If L > 1: diverges',
  '- If L = 1: inconclusive',
  '',
  '## Important Examples',
  '',
  '### Geometric Series',
  'The geometric series converges for |r| < 1 and equals 1/(1-r). This is the foundation for many convergence comparisons.',
  '',
  '### p-Series',
  'The p-series converges if and only if p > 1. This result is fundamental for comparison tests and integral test applications.',
  '',
  '## Study Notes',
  '',
  '> The ratio test is usually the first tool to reach for. When it gives L = 1, try comparison or the integral test instead.',
  '',
  '- Practice problems from Chapter 8, exercises 1-15',
  '- Focus on edge cases where ratio test is inconclusive',
  '- Review alternating series estimation theorem',
  '- Connection to Taylor series is important for the final',
  '',
  '## Next Steps',
  '',
  '- [ ] Complete problem set 8.3',
  '- [ ] Review Taylor series connection',
  '- [ ] Practice power series radius of convergence',
  '- [x] Review basic convergence definitions',
  '- [x] Memorize comparison test criteria',
].join('\n');

/* ---- Build daily note from session data ---- */
function buildDailyNote(data) {
  const dateStr = ST.iso(ST.today);
  const todays = data.sessions.filter(s => (s.start || '').slice(0, 10) === dateStr);
  const mins = todays.reduce((a, s) => a + s.minutes, 0);
  let md = `# Daily Note — ${dateStr}\n\n`;
  md += `> Focused **${ST.fmtMins(mins)}** across ${todays.length} session${todays.length !== 1 ? 's' : ''} · ${data.streak}-day streak\n\n`;
  md += `## Sessions\n`;
  if (todays.length === 0) md += `_No sessions logged yet today._\n`;
  todays.forEach(s => {
    const c = ST.courseById(data, s.courseId);
    md += `\n### ${s.goal || s.preset} · ${ST.fmtMins(s.minutes)}\n`;
    md += `- **Course:** ${c ? c.name : 'General'} · **Preset:** ${s.preset} · **Confidence:** ${s.confidence}/5\n`;
    if (s.learned) md += `- **Learned:** ${s.learned}\n`;
    if (s.blocker) md += `- **Blocker:** ${s.blocker}\n`;
    if (s.next) md += `- **Next:** ${s.next}\n`;
  });
  md += `\n## Tomorrow's focus\n`;
  ST.allTasks(data).filter(t => t.done < t.total).sort((a, b) => ST.urgency(b) - ST.urgency(a)).slice(0, 3)
    .forEach(t => { md += `- [ ] ${t.title} _(${t.courseName}, ${ST.taskRemaining(t)} units)_\n`; });
  return md;
}

/* ---- Inline markdown helper ---- */
function inlineMd(s) {
  return s
    .replace(/\*\*(.+?)\*\*/g, '<strong style="color:var(--ink);font-weight:600">$1</strong>')
    .replace(/_(.+?)_/g, '<em style="color:var(--ink-3)">$1</em>')
    .replace(/`(.+?)`/g, '<code style="font-family:var(--font-mono);font-size:0.88em;padding:1px 5px;border-radius:4px;background:var(--surface-2);color:var(--ink-2)">$1</code>')
    .replace(/\[ \]/g, '☐')
    .replace(/\[x\]/g, '☑');
}

/* ---- Enhanced Markdown Preview ---- */
function VaultMarkdownPreview({ md, compact }) {
  const lines = (md || '').split('\n');
  const sz = compact ? 13 : 14.5;
  return (
    <div className="vault-prose" style={{ fontFamily: 'var(--font-sans)', fontSize: sz, lineHeight: 1.78, color: 'var(--ink-2)', letterSpacing: '0.005em' }}>
      {lines.map((ln, i) => {
        if (ln.startsWith('# ')) return (
          <div key={i} className="serif" style={{ fontSize: compact ? 21 : 25, fontWeight: 600, color: 'var(--ink)', margin: i === 0 ? '0 0 4px' : '28px 0 4px', letterSpacing: '-0.025em', lineHeight: 1.2 }}>{ln.slice(2)}</div>
        );
        if (ln.startsWith('### ')) return (
          <div key={i} style={{ fontSize: sz + 0.5, fontWeight: 600, color: 'var(--ink)', margin: '16px 0 4px', lineHeight: 1.3 }}>{ln.slice(4)}</div>
        );
        if (ln.startsWith('## ')) return (
          <div key={i} style={{ fontSize: compact ? 10.5 : 11, fontWeight: 500, color: 'var(--accent)', fontFamily: 'var(--font-mono)', letterSpacing: '0.12em', textTransform: 'uppercase', margin: '22px 0 8px', paddingBottom: 6, borderBottom: '1px solid var(--line-soft)' }}>{ln.slice(3)}</div>
        );
        if (ln.startsWith('> ')) return (
          <div key={i} style={{ borderLeft: '2px solid var(--accent-line)', paddingLeft: 14, margin: '8px 0', color: 'var(--ink-3)', fontStyle: 'italic', fontSize: sz - 0.5 }} dangerouslySetInnerHTML={{ __html: inlineMd(ln.slice(2)) }} />
        );
        if (ln.startsWith('- [x] ')) return (
          <div key={i} style={{ paddingLeft: 2, display: 'flex', alignItems: 'flex-start', gap: 8, margin: '3px 0', color: 'var(--ink-4)' }}>
            <span style={{ color: 'var(--ok)', marginTop: 3, flexShrink: 0 }}><Icon name="check" size={13} stroke={2.2} /></span>
            <span style={{ textDecoration: 'line-through' }} dangerouslySetInnerHTML={{ __html: inlineMd(ln.slice(6)) }} />
          </div>
        );
        if (ln.startsWith('- [ ] ')) return (
          <div key={i} style={{ paddingLeft: 2, display: 'flex', alignItems: 'flex-start', gap: 8, margin: '3px 0' }}>
            <span style={{ width: 13, height: 13, borderRadius: 3, border: '1.5px solid var(--line-strong)', marginTop: 4, flexShrink: 0 }}></span>
            <span dangerouslySetInnerHTML={{ __html: inlineMd(ln.slice(6)) }} />
          </div>
        );
        if (ln.startsWith('- ')) return (
          <div key={i} style={{ paddingLeft: 2, display: 'flex', gap: 8, margin: '2px 0' }}>
            <span style={{ color: 'var(--ink-4)', flexShrink: 0, lineHeight: 1.78 }}>·</span>
            <span dangerouslySetInnerHTML={{ __html: inlineMd(ln.slice(2)) }} />
          </div>
        );
        if (ln.trim() === '') return <div key={i} style={{ height: 8 }} />;
        return <div key={i} style={{ margin: '2px 0' }} dangerouslySetInnerHTML={{ __html: inlineMd(ln) }} />;
      })}
    </div>
  );
}

/* ============================================================
   Space Navigation — compact, scalable tab bar
   ============================================================ */
function SpaceNav({ active, onChange }) {
  return (
    <div className="vault-nav" style={{ display: 'flex', alignItems: 'center', gap: 1, borderBottom: '1px solid var(--line-soft)', marginBottom: 20, overflowX: 'auto' }}>
      {VAULT_SPACES.map(sp => {
        const on = sp.id === active;
        return (
          <button key={sp.id} onClick={() => onChange(sp.id)} style={{
            display: 'inline-flex', alignItems: 'center', gap: 6,
            padding: '9px 14px 8px', fontSize: 12.5,
            fontWeight: on ? 600 : 500, fontFamily: 'var(--font-sans)',
            color: on ? 'var(--accent)' : 'var(--ink-3)',
            background: 'none', border: 'none',
            borderBottom: '2px solid', borderColor: on ? 'var(--accent)' : 'transparent',
            cursor: 'pointer',
            opacity: 1, transition: 'color .15s',
            marginBottom: -1, whiteSpace: 'nowrap', flexShrink: 0,
          }}>
            <Icon name={sp.icon} size={14} stroke={on ? 2 : 1.5} />
            {sp.label}
          </button>
        );
      })}
    </div>
  );
}

/* ============================================================
   Vault Settings — collapsible config panel
   ============================================================ */
function VaultSettings({ data }) {
  const [vaultName, setVaultName] = useState(data.meta.vaultName);
  const [apiKey, setApiKey] = useState(data.meta.apiKey);
  const [model, setModel] = useState(data.meta.model);
  return (
    <div className="fade-up vault-settings-grid" style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 20 }}>
      <Card>
        <CardHead eyebrow="Obsidian vault" icon="vault" title="Vault configuration" />
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <Field label="Vault name"><Input value={vaultName} onChange={e => setVaultName(e.target.value)} /></Field>
          <Field label="Vault path">
            <div style={{ ...inputStyle, display: 'flex', alignItems: 'center', gap: 8, color: 'var(--ink-3)', fontFamily: 'var(--font-mono)', fontSize: 12, background: 'var(--surface-2)' }}>
              <Icon name="folder" size={13} style={{ color: 'var(--ink-4)', flexShrink: 0 }} />{data.meta.vaultPath}/{vaultName}
            </div>
          </Field>
          <div>
            <span className="eyebrow" style={{ fontSize: 10, display: 'block', marginBottom: 8 }}>Folders</span>
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: 5 }}>
              {VAULT_FOLDERS.map(f => <Chip key={f} size="sm" icon="folder" color="var(--ink-3)">{f}</Chip>)}
            </div>
          </div>
          <Btn variant="solid" icon="vault" size="sm">Relink vault</Btn>
        </div>
      </Card>
      <Card>
        <CardHead eyebrow="AI Foundation" icon="sparkle" title="Anthropic setup" />
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <Field label="API key"><Input type="password" value={apiKey} placeholder="sk-ant-…" onChange={e => setApiKey(e.target.value)} /></Field>
          <Field label="Model"><Input value={model} onChange={e => setModel(e.target.value)} /></Field>
        </div>
        <div style={{ marginTop: 14, padding: '10px 12px', borderRadius: 'var(--r-sm)', background: 'var(--warn-soft)', border: '1px solid', borderColor: 'color-mix(in oklch, var(--warn) 25%, transparent)', display: 'flex', gap: 8, alignItems: 'flex-start' }}>
          <Icon name="info" size={14} style={{ color: 'var(--warn)', flexShrink: 0, marginTop: 1 }} />
          <span style={{ fontSize: 11.5, color: 'var(--ink-2)', lineHeight: 1.5 }}>
            Summaries are stored as course-based PDFs and image cheatsheets inside the vault.
          </span>
        </div>
      </Card>
    </div>
  );
}

/* ============================================================
   Daily Note Space
   ============================================================ */
function DailyNoteSpace({ data }) {
  const [dateOffset, setDateOffset] = useState(0);
  const [editing, setEditing] = useState(false);
  const currentDate = new Date(ST.today.getTime() + dateOffset * ST.DAY);
  const dateStr = ST.iso(currentDate);
  const isToday = dateOffset === 0;

  const baseMd = isToday ? buildDailyNote(data) : `# Daily Note — ${dateStr}\n\n_No note found for this date. Generate one from your sessions or start writing._`;
  const [editorContent, setEditorContent] = useState(baseMd);
  React.useEffect(() => {
    const md = isToday ? buildDailyNote(data) : `# Daily Note — ${dateStr}\n\n_No note found for this date. Generate one from your sessions or start writing._`;
    setEditorContent(md);
    setEditing(false);
  }, [dateOffset]);

  const dayName = currentDate.toLocaleDateString('en-GB', { weekday: 'short' });
  const dateDisplay = currentDate.toLocaleDateString('en-GB', { day: 'numeric', month: 'long', year: 'numeric' });
  const todayMins = ST.minutesToday(data);
  const todaySessions = data.sessions.filter(s => (s.start || '').slice(0, 10) === dateStr).length;

  return (
    <div className="fade-up">
      {/* Toolbar */}
      <div className="vault-toolbar" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16, flexWrap: 'wrap', gap: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <Btn size="sm" variant="ghost" icon="chevron" style={{ transform: 'scaleX(-1)' }} onClick={() => setDateOffset(d => d - 1)} title="Previous day" />
          <button onClick={() => setDateOffset(0)} style={{
            display: 'flex', alignItems: 'center', gap: 7,
            padding: '5px 14px', borderRadius: 'var(--r-sm)',
            background: isToday ? 'var(--accent-soft)' : 'var(--surface-2)',
            border: '1px solid', borderColor: isToday ? 'var(--accent-line)' : 'var(--line)',
            color: isToday ? 'var(--accent)' : 'var(--ink)',
            fontSize: 12.5, fontWeight: 600, fontFamily: 'var(--font-sans)', cursor: 'pointer', whiteSpace: 'nowrap',
          }}>
            <Icon name="cal" size={13} />
            {dayName}, {dateDisplay}
          </button>
          <Btn size="sm" variant="ghost" icon="chevron" onClick={() => setDateOffset(d => d + 1)} title="Next day" />
          {isToday && todaySessions > 0 && (
            <div className="vault-stats" style={{ display: 'flex', alignItems: 'center', gap: 5, marginLeft: 6 }}>
              <Chip size="sm" icon="clock">{ST.fmtMins(todayMins)}</Chip>
              <Chip size="sm" icon="flame">{data.streak}d streak</Chip>
            </div>
          )}
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          {isToday && <Btn size="sm" variant="primary" icon="bolt" onClick={() => setEditorContent(buildDailyNote(data))}>Generate</Btn>}
          <Btn size="sm" variant={editing ? 'ghost' : 'solid'} icon={editing ? 'x' : 'edit'} onClick={() => setEditing(!editing)}>{editing ? 'Close' : 'Edit'}</Btn>
          <Btn size="sm" variant="ghost" icon="download">Export</Btn>
        </div>
      </div>

      {/* Content */}
      {editing ? (
        <div className="vault-split" style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 0, borderRadius: 'var(--r-lg)', overflow: 'hidden', border: '1px solid var(--line)', minHeight: 480 }}>
          <div style={{ background: 'var(--surface-inset)', padding: '16px 20px', display: 'flex', flexDirection: 'column' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
              <span className="eyebrow" style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Icon name="edit" size={11} />Editor</span>
              <Btn size="sm" variant="primary" icon="save" onClick={() => setEditing(false)}>Save</Btn>
            </div>
            <textarea value={editorContent} onChange={e => setEditorContent(e.target.value)} style={{
              flex: 1, width: '100%', minHeight: 420,
              fontFamily: 'var(--font-mono)', fontSize: 12.5, lineHeight: 1.7,
              color: 'var(--ink-2)', background: 'transparent',
              border: 'none', outline: 'none', resize: 'none', padding: 0,
            }} />
          </div>
          <div style={{ background: 'var(--surface)', padding: '16px 24px', borderLeft: '1px solid var(--line)', overflowY: 'auto' }}>
            <span className="eyebrow" style={{ display: 'flex', alignItems: 'center', gap: 5, marginBottom: 12 }}><Icon name="focus" size={11} />Preview</span>
            <VaultMarkdownPreview md={editorContent} compact />
          </div>
        </div>
      ) : (
        <div style={{ maxWidth: 780 }}>
          <Card style={{ padding: '32px 36px' }}>
            <VaultMarkdownPreview md={editorContent} />
          </Card>
          {/* Recent exports */}
          <div style={{ marginTop: 20 }}>
            <div className="eyebrow" style={{ marginBottom: 10 }}>Recent exports</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              {[
                { date: ST.addDays(-1), path: `Daily/${ST.addDays(-1)}.md` },
                { date: ST.addDays(-2), path: `Daily/${ST.addDays(-2)}.md` },
                { date: ST.addDays(-3), path: `Daily/${ST.addDays(-3)}.md` },
              ].map((h, i) => (
                <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 12px', borderRadius: 'var(--r-sm)', background: i === 0 ? 'var(--surface)' : 'transparent', border: '1px solid', borderColor: i === 0 ? 'var(--line-soft)' : 'transparent' }}>
                  <Icon name="file" size={14} style={{ color: 'var(--ink-4)', flexShrink: 0 }} />
                  <span className="mono" style={{ fontSize: 11.5, color: 'var(--ink-3)', flex: 1 }}>{h.path}</span>
                  <span className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{h.date}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ============================================================
   References Space
   ============================================================ */
function SemCourseBar({ semId, setSemId, courseId, setCourseId, semesters, courses, editing, setEditing, hasFile }) {
  return (
    <div className="vault-toolbar" style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16, flexWrap: 'wrap', gap: 10 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
        <div style={{ position: 'relative', flexShrink: 0 }}>
          <select value={semId} onChange={e => setSemId(e.target.value)} style={{
            appearance: 'none', fontFamily: 'var(--font-sans)', fontSize: 12.5, fontWeight: 600,
            color: 'var(--ink)', background: 'var(--surface-2)',
            border: '1px solid var(--line)', borderRadius: 'var(--r-sm)',
            padding: '5px 28px 5px 10px', cursor: 'pointer', outline: 'none',
          }}>
            {semesters.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}
          </select>
          <Icon name="chevronDown" size={12} style={{ position: 'absolute', right: 8, top: '50%', transform: 'translateY(-50%)', color: 'var(--ink-4)', pointerEvents: 'none' }} />
        </div>
        <span style={{ color: 'var(--ink-4)', fontSize: 13 }}>/</span>
        <div style={{ display: 'flex', gap: 3, flexWrap: 'wrap' }}>
          {courses.map(c => {
            const on = c.id === courseId;
            return (
              <button key={c.id} onClick={() => setCourseId(c.id)} style={{
                display: 'flex', alignItems: 'center', gap: 5,
                padding: '4px 11px', borderRadius: 'var(--r-pill)',
                fontSize: 12, fontWeight: on ? 600 : 500, fontFamily: 'var(--font-sans)', whiteSpace: 'nowrap',
                background: on ? 'var(--accent-soft)' : 'transparent',
                color: on ? 'var(--accent)' : 'var(--ink-3)',
                border: '1px solid', borderColor: on ? 'var(--accent-line)' : 'var(--line-soft)',
                cursor: 'pointer', transition: 'all .15s',
              }}>
                <Dot color={c.color} size={6} />{c.name}
              </button>
            );
          })}
        </div>
      </div>
      {hasFile && setEditing && (
        <Btn size="sm" variant={editing ? 'ghost' : 'primary'} icon={editing ? 'x' : 'edit'} onClick={() => setEditing(!editing)}>
          {editing ? 'Close' : 'Edit'}
        </Btn>
      )}
    </div>
  );
}

function ReferencesSpace({ data }) {
  const activeSem = data.semesters[0];
  const [semId, setSemId] = useState(activeSem?.id || '');
  const [courseId, setCourseId] = useState('');
  const [editing, setEditing] = useState(false);
  const [editorContent, setEditorContent] = useState(SAMPLE_REF_MD);

  const semesters = data.semesters;
  const currentSem = semesters.find(s => s.id === semId);
  const courses = currentSem?.courses || [];
  const currentCourse = courses.find(c => c.id === courseId);

  React.useEffect(() => {
    if (courses.length > 0 && !courses.find(c => c.id === courseId)) {
      setCourseId(courses[0].id);
    }
  }, [semId]);

  React.useEffect(() => {
    setEditing(false);
    // Demo: only Analysis has content
    if (courseId === 'c-analysis') {
      setEditorContent(SAMPLE_REF_MD);
    }
  }, [courseId]);

  const hasFile = courseId === 'c-analysis';

  // No courses in this semester
  if (semesters.length > 0 && courses.length === 0) {
    return (
      <div className="fade-up">
        <SemCourseBar semId={semId} setSemId={setSemId} courseId={courseId} setCourseId={setCourseId} semesters={semesters} courses={courses} />
        <Card style={{ maxWidth: 440, margin: '32px auto' }}>
          <Empty icon="book" title="No courses yet" sub="Add courses in the Planner tab, then come back to write reference notes." action={<Btn variant="solid" icon="plus">Go to Planner</Btn>} />
        </Card>
      </div>
    );
  }

  return (
    <div className="fade-up">
      <SemCourseBar semId={semId} setSemId={setSemId} courseId={courseId} setCourseId={setCourseId}
        semesters={semesters} courses={courses} editing={editing} setEditing={setEditing} hasFile={hasFile} />

      {!hasFile ? (
        <Card style={{ maxWidth: 440, margin: '32px auto' }}>
          <Empty icon="file" title="No reference notes yet"
            sub={`Start writing reference notes for ${currentCourse?.name || 'this course'}.`}
            action={<Btn variant="primary" icon="edit" onClick={() => { setEditing(true); setEditorContent(`# ${currentCourse?.name || 'Reference Notes'}\n\n## Overview\n\n`); }}>Start writing</Btn>} />
        </Card>
      ) : editing ? (
        <div className="vault-split" style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 0, borderRadius: 'var(--r-lg)', overflow: 'hidden', border: '1px solid var(--line)', minHeight: 520 }}>
          <div style={{ background: 'var(--surface-inset)', padding: '16px 20px', display: 'flex', flexDirection: 'column' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 10 }}>
              <span className="eyebrow" style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Icon name="edit" size={11} />Markdown</span>
              <Btn size="sm" variant="primary" icon="save" onClick={() => setEditing(false)}>Save</Btn>
            </div>
            <textarea value={editorContent} onChange={e => setEditorContent(e.target.value)} style={{
              flex: 1, width: '100%', minHeight: 460,
              fontFamily: 'var(--font-mono)', fontSize: 12.5, lineHeight: 1.7,
              color: 'var(--ink-2)', background: 'transparent',
              border: 'none', outline: 'none', resize: 'none', padding: 0,
            }} />
          </div>
          <div style={{ background: 'var(--surface)', padding: '16px 28px', borderLeft: '1px solid var(--line)', overflowY: 'auto' }}>
            <span className="eyebrow" style={{ display: 'flex', alignItems: 'center', gap: 5, marginBottom: 12 }}><Icon name="focus" size={11} />Preview</span>
            <VaultMarkdownPreview md={editorContent} />
          </div>
        </div>
      ) : (
        <div style={{ maxWidth: 800 }}>
          <Card style={{ padding: '36px 42px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 18, flexWrap: 'wrap' }}>
              <Chip size="sm" icon="folder" color="var(--ink-3)" style={{ whiteSpace: 'nowrap' }}>References</Chip>
              <span className="mono" style={{ fontSize: 11, color: 'var(--ink-4)', whiteSpace: 'nowrap' }}>{currentCourse?.name}.md</span>
            </div>
            <VaultMarkdownPreview md={editorContent} />
          </Card>
        </div>
      )}
    </div>
  );
}

/* ============================================================
   Summaries Space
   ============================================================ */
function SummariesSpace() {
  return (
    <Card style={{ maxWidth: 520, margin: '40px auto' }}>
      <Empty icon="layers"
        title="Summaries and cheatsheets"
        sub="Add PDFs, formula sheets, scanned notes, and cheatsheet images by course. The desktop app stores them in Summaries/{semester}/{course}." />
    </Card>
  );
}

/* ============================================================
   Main Vault Tab
   ============================================================ */
function VaultAI({ data, dispatch }) {
  const [space, setSpace] = useState('daily');
  const [showSettings, setShowSettings] = useState(false);
  const vaultLinked = !!data.meta.vaultName;

  return (
    <div className="fade-up" style={{ display: 'flex', flexDirection: 'column' }}>
      {/* Header */}
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 16, marginBottom: 14 }}>
        <div>
          <h1 className="serif" style={{ fontSize: 27, fontWeight: 500, margin: 0, letterSpacing: '-0.02em' }}>Vault</h1>
          <p style={{ margin: '4px 0 0', fontSize: 13, color: 'var(--ink-3)' }}>Your Obsidian-compatible knowledge base</p>
        </div>
        {vaultLinked && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '5px 12px', borderRadius: 'var(--r-pill)', background: 'var(--surface)', border: '1px solid var(--line-soft)' }}>
              <span style={{ width: 6, height: 6, borderRadius: 99, background: 'var(--ok)', boxShadow: '0 0 5px var(--ok)' }}></span>
              <span className="mono" style={{ fontSize: 11, color: 'var(--ink-3)' }}>{data.meta.vaultName}</span>
            </div>
            <Btn size="sm" variant="quiet" icon={showSettings ? 'x' : 'layers'} onClick={() => setShowSettings(s => !s)} title="Vault settings" />
          </div>
        )}
      </div>

      {/* Settings */}
      {showSettings && <VaultSettings data={data} />}

      {/* No vault empty state */}
      {!vaultLinked ? (
        <Card style={{ maxWidth: 480, margin: '30px auto' }}>
          <Empty icon="vault" title="No vault linked yet"
            sub="Connect an Obsidian-compatible vault folder. Notes are saved as standard .md files you can open anywhere."
            action={
              <div style={{ display: 'flex', gap: 8, justifyContent: 'center' }}>
                <Btn variant="primary" icon="folder">Link existing vault</Btn>
                <Btn variant="solid" icon="vault">Create new vault</Btn>
              </div>
            } />
        </Card>
      ) : (
        <>
          <SpaceNav active={space} onChange={setSpace} />
          {space === 'daily' && <DailyNoteSpace data={data} />}
          {space === 'references' && <ReferencesSpace data={data} />}
          {space === 'summaries' && <SummariesSpace />}
        </>
      )}
    </div>
  );
}

Object.assign(window, { VAULT_FOLDERS, VaultAI, buildDailyNote });
