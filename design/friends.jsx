/* ============================================================
   Friends Tab — "Study Arena"
   Game-like competitive social hub
   Squad (clan) + Friends (individual) as separate concepts
   ============================================================ */

/* ---- Arena background decoration ---- */
function ArenaBg() {
  return (
    <div className="arena-bg" aria-hidden="true">
      <div className="arena-bg-orb arena-bg-orb--1" />
      <div className="arena-bg-orb arena-bg-orb--2" />
      <div className="arena-bg-orb arena-bg-orb--3" />
    </div>
  );
}

/* ---- Rank badge (1-3 special, rest plain) ---- */
function RankBadge({ rank, size = 'md' }) {
  const s = size === 'lg' ? 52 : size === 'md' ? 32 : 24;
  const fs = size === 'lg' ? 22 : size === 'md' ? 14 : 11;
  const colors = {
    1: { bg: 'linear-gradient(145deg, #FFD700, #FFA000)', shadow: '0 2px 12px #FFD70066', text: '#4A3800' },
    2: { bg: 'linear-gradient(145deg, #C0D8F0, #8AAEC8)', shadow: '0 2px 12px #8AAEC866', text: '#2A3E50' },
    3: { bg: 'linear-gradient(145deg, #E8A060, #C07840)', shadow: '0 2px 12px #C0784066', text: '#3E2510' },
  };
  const c = colors[rank] || { bg: 'var(--surface-2)', shadow: 'none', text: 'var(--ink-3)' };
  return (
    <div style={{
      width: s, height: s, borderRadius: rank <= 3 ? s * 0.3 : s * 0.25,
      background: c.bg, boxShadow: c.shadow,
      display: 'grid', placeItems: 'center',
      fontSize: fs, fontWeight: 800, color: c.text,
      fontFamily: 'var(--font-sans)',
      border: rank <= 3 ? '2px solid rgba(255,255,255,0.3)' : '1px solid var(--line)',
      flexShrink: 0,
    }}>
      {rank <= 3 ? ['🥇','🥈','🥉'][rank-1] : `#${rank}`}
    </div>
  );
}

/* ---- Avatar circle ---- */
function PlayerAvatar({ name, size = 44, isSelf, glow }) {
  const initials = (name || '??').split(/[\s_]/).map(w => w[0]).join('').slice(0, 2).toUpperCase();
  const hue = name ? [...name].reduce((a, c) => a + c.charCodeAt(0), 0) % 360 : 200;
  const bg = isSelf
    ? 'linear-gradient(135deg, var(--accent), var(--accent-strong))'
    : `linear-gradient(135deg, oklch(0.65 0.12 ${hue}), oklch(0.55 0.14 ${hue + 30}))`;
  return (
    <div style={{
      width: size, height: size, borderRadius: size * 0.32,
      background: bg, display: 'grid', placeItems: 'center',
      fontSize: size * 0.36, fontWeight: 700, color: '#fff',
      fontFamily: 'var(--font-sans)', letterSpacing: '-0.02em',
      flexShrink: 0,
      boxShadow: glow ? `0 0 20px var(--accent-soft), 0 4px 14px rgba(0,0,0,0.25)` : '0 2px 8px rgba(0,0,0,0.2)',
      border: isSelf ? '2.5px solid var(--accent-line)' : '2px solid rgba(255,255,255,0.15)',
    }}>
      {initials}
    </div>
  );
}

/* ---- Squad badge (clan emblem) ---- */
function SquadEmblem({ tag, size = 44 }) {
  return (
    <div style={{
      width: size, height: size, borderRadius: size * 0.28,
      background: 'linear-gradient(145deg, var(--warn), oklch(0.72 0.14 50))',
      display: 'grid', placeItems: 'center',
      fontSize: size * 0.38, fontWeight: 800, color: '#3A2800',
      fontFamily: 'var(--font-sans)', letterSpacing: '-0.02em',
      flexShrink: 0,
      border: '2.5px solid rgba(255,255,255,0.25)',
      boxShadow: '0 2px 12px rgba(200,160,0,0.3)',
    }}>
      {tag}
    </div>
  );
}

/* ---- Sync status pill ---- */
function SyncPill({ status }) {
  const map = {
    ready: { label: 'Arena Synced', color: 'var(--ok)' },
    local: { label: 'Local Only', color: 'var(--warn)' },
    issue: { label: 'Sync Issue', color: 'var(--danger)' },
  };
  const m = map[status] || map.ready;
  return (
    <span style={{
      display: 'inline-flex', alignItems: 'center', gap: 5,
      padding: '3px 10px 3px 7px', borderRadius: 'var(--r-pill)',
      fontSize: 11, fontWeight: 600, fontFamily: 'var(--font-mono)',
      background: `color-mix(in oklch, ${m.color} 12%, transparent)`,
      color: m.color, border: `1px solid color-mix(in oklch, ${m.color} 25%, transparent)`,
    }}>
      <span style={{ width: 6, height: 6, borderRadius: 99, background: m.color, boxShadow: `0 0 6px ${m.color}` }} />
      {m.label}
    </span>
  );
}

/* ---- Player ID Card ---- */
function PlayerCard({ player, stats }) {
  const [copied, setCopied] = useState(false);
  const copyCode = () => {
    navigator.clipboard?.writeText(player.friendCode).catch(() => {});
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };
  return (
    <div className="arena-player-card">
      <div className="arena-player-card__inner">
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <PlayerAvatar name={player.displayName} size={56} isSelf glow />
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 18, fontWeight: 700, color: 'var(--ink)', lineHeight: 1.2 }}>{player.displayName}</div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 6, flexWrap: 'wrap' }}>
              <span className="arena-code-plate" onClick={copyCode} title="Copy Player Tag">
                <Icon name="key" size={12} />
                <span>{player.friendCode}</span>
                <span className="arena-code-copy">{copied ? '✓' : '⧉'}</span>
              </span>
              <SyncPill status={player.syncStatus} />
            </div>
          </div>
        </div>
        <div className="arena-player-stats">
          <div className="arena-mini-stat">
            <span className="arena-mini-stat__icon" style={{ background: 'linear-gradient(135deg, #FFD700, #FFA000)' }}>
              <Icon name="bolt" size={14} style={{ color: '#4A3800' }} />
            </span>
            <div>
              <div className="arena-mini-stat__val">{ST.fmtMins(stats.today.minutes)}</div>
              <div className="arena-mini-stat__label">Today</div>
            </div>
          </div>
          <div className="arena-mini-stat">
            <span className="arena-mini-stat__icon" style={{ background: 'linear-gradient(135deg, var(--accent), var(--accent-strong))' }}>
              <Icon name="flame" size={14} style={{ color: '#fff' }} />
            </span>
            <div>
              <div className="arena-mini-stat__val">{ST.fmtMins(stats.week.minutes)}</div>
              <div className="arena-mini-stat__label">This Week</div>
            </div>
          </div>
          <div className="arena-mini-stat">
            <span className="arena-mini-stat__icon" style={{ background: 'linear-gradient(135deg, var(--garden), var(--garden-deep))' }}>
              <Icon name="layers" size={14} style={{ color: '#fff' }} />
            </span>
            <div>
              <div className="arena-mini-stat__val">{ST.fmtMins(stats.overall.minutes)}</div>
              <div className="arena-mini-stat__label">All Time</div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ---- Podium (top 3) ---- */
function Podium({ top3 }) {
  if (!top3 || top3.length < 3) return null;
  const order = [top3[1], top3[0], top3[2]];
  const heights = [100, 130, 80];
  return (
    <div className="arena-podium">
      {order.map((p, i) => {
        const rank = p.rank;
        return (
          <div key={p.id} className={`arena-podium__col ${p.isSelf ? 'arena-podium__col--self' : ''}`}>
            <PlayerAvatar name={p.displayName} size={i === 1 ? 56 : 44} isSelf={p.isSelf} />
            <div className="arena-podium__name">{p.displayName}</div>
            <div className="arena-podium__mins mono">{ST.fmtMins(p.minutes)}</div>
            <div className="arena-podium__bar" style={{ height: heights[i] }}>
              <RankBadge rank={rank} size={i === 1 ? 'lg' : 'md'} />
              <div className="arena-podium__sessions mono">{p.sessions} sessions</div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ---- Leaderboard row ---- */
function LeaderboardRow({ entry }) {
  const isSelf = entry.isSelf;
  return (
    <div className={`arena-lb-row ${isSelf ? 'arena-lb-row--self' : ''}`}>
      <RankBadge rank={entry.rank} size="sm" />
      <PlayerAvatar name={entry.displayName} size={34} isSelf={isSelf} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ fontSize: 13.5, fontWeight: 600, color: isSelf ? 'var(--accent)' : 'var(--ink)' }}>
            {entry.displayName}{isSelf ? ' (You)' : ''}
          </span>
          {entry.role && (
            <span style={{
              fontSize: 9, fontWeight: 700, padding: '1px 5px', borderRadius: 'var(--r-pill)',
              background: entry.role === 'Leader' ? 'linear-gradient(135deg, #FFD700, #FFA000)' : 'var(--surface-2)',
              color: entry.role === 'Leader' ? '#4A3800' : 'var(--ink-4)',
              textTransform: 'uppercase', letterSpacing: '0.05em',
            }}>{entry.role}</span>
          )}
        </div>
        <div className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{entry.friendCode}</div>
      </div>
      <div style={{ textAlign: 'right', flexShrink: 0 }}>
        <div className="mono" style={{ fontSize: 15, fontWeight: 700, color: isSelf ? 'var(--accent)' : 'var(--ink)' }}>{ST.fmtMins(entry.minutes)}</div>
        <div className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{entry.sessions} sess.</div>
      </div>
    </div>
  );
}

/* ---- Leaderboard panel ---- */
function ArenaLeaderboard() {
  const [scope, setScope] = useState('global');
  const [period, setPeriod] = useState('weekly');
  const leaderboard = FriendsData.makeLeaderboard(scope, period);
  const top3 = leaderboard.slice(0, 3);
  const rest = leaderboard.slice(3);

  const scopes = [
    { value: 'global', label: 'World Arena', icon: 'layers' },
    { value: 'friends', label: 'Friends', icon: 'target' },
    { value: 'squad', label: 'Squad', icon: 'flag' },
  ];
  const periods = [
    { value: 'daily', label: 'Daily Sprint', icon: 'bolt' },
    { value: 'weekly', label: 'Weekly League', icon: 'flame' },
    { value: 'overall', label: 'Hall of Focus', icon: 'layers' },
  ];
  const activeScope = scopes.find(s => s.value === scope);
  const activePeriod = periods.find(p => p.value === period);

  return (
    <div className="arena-leaderboard">
      <div className="arena-leaderboard__header">
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span className="arena-title-icon">⚔</span>
          <div>
            <div className="arena-leaderboard__title">Arena Standings</div>
            <div style={{ fontSize: 12, color: 'var(--ink-3)', marginTop: 2 }}>
              {activeScope?.label} · {activePeriod?.label}
            </div>
          </div>
        </div>
      </div>

      {/* Scope toggle — 3 options */}
      <div className="arena-scope-toggle">
        {scopes.map(s => (
          <button key={s.value} className={`arena-scope-btn ${scope === s.value ? 'arena-scope-btn--active' : ''}`}
            onClick={() => setScope(s.value)}>
            <Icon name={s.icon} size={15} />
            {s.label}
          </button>
        ))}
      </div>

      {/* Period chips */}
      <div className="arena-period-chips">
        {periods.map(p => (
          <button key={p.value} className={`arena-period-chip ${period === p.value ? 'arena-period-chip--active' : ''}`}
            onClick={() => setPeriod(p.value)}>
            <Icon name={p.icon} size={13} />
            {p.label}
          </button>
        ))}
      </div>

      {/* Podium */}
      {top3.length >= 3 ? <Podium top3={top3} /> : null}

      {/* Rows */}
      <div className="arena-lb-rows">
        {(top3.length >= 3 ? rest : leaderboard).map(e => <LeaderboardRow key={e.id + e.rank} entry={e} />)}
        {leaderboard.length === 0 && (
          <div className="arena-empty">
            <div style={{ fontSize: 28, marginBottom: 8 }}>🏟</div>
            <div style={{ fontWeight: 600, fontSize: 14 }}>No contenders yet</div>
            <div style={{ color: 'var(--ink-4)', fontSize: 12.5, marginTop: 4 }}>
              {scope === 'squad' ? 'Your squad rankings will appear here.' : 'Start studying to claim your rank!'}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

/* ---- Request card ---- */
function RequestCard({ req, type, onAccept, onDecline }) {
  return (
    <div className="arena-request-card">
      <PlayerAvatar name={req.displayName} size={38} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13.5, fontWeight: 600 }}>{req.displayName}</div>
        <div className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{req.friendCode}</div>
        {req.message && <div style={{ fontSize: 11.5, color: 'var(--ink-3)', marginTop: 3, fontStyle: 'italic' }}>"{req.message}"</div>}
        {type === 'outgoing' && <div style={{ fontSize: 11, color: 'var(--ink-4)', marginTop: 3 }}>Sent {req.sentAt}</div>}
      </div>
      {type === 'incoming' ? (
        <div style={{ display: 'flex', gap: 6, flexShrink: 0 }}>
          <button className="arena-btn arena-btn--accept" onClick={onAccept}>Accept</button>
          <button className="arena-btn arena-btn--decline" onClick={onDecline}>Decline</button>
        </div>
      ) : (
        <span className="arena-pending-badge">Pending</span>
      )}
    </div>
  );
}

/* ---- Squad invite card ---- */
function SquadInviteCard({ invite, onAccept, onDecline }) {
  return (
    <div className="arena-request-card">
      <SquadEmblem tag={invite.tag} size={42} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, fontWeight: 700 }}>{invite.squadName}</div>
        <div style={{ fontSize: 11.5, color: 'var(--ink-3)', marginTop: 2 }}>
          {invite.memberCount} members · Invited by {invite.invitedBy}
        </div>
      </div>
      <div style={{ display: 'flex', gap: 6, flexShrink: 0 }}>
        <button className="arena-btn arena-btn--accept" onClick={onAccept}>Join</button>
        <button className="arena-btn arena-btn--decline" onClick={onDecline}>Decline</button>
      </div>
    </div>
  );
}

/* ---- Friend card ---- */
function FriendCard({ friend }) {
  return (
    <div className="arena-squad-card">
      <PlayerAvatar name={friend.displayName} size={36} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 13, fontWeight: 600 }}>{friend.displayName}</div>
        <div className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{friend.friendCode}</div>
      </div>
    </div>
  );
}

/* ---- Squad member card ---- */
function SquadMemberCard({ member }) {
  const isSelf = member.isSelf;
  return (
    <div className={`arena-squad-card ${isSelf ? 'arena-squad-card--self' : ''}`}>
      <PlayerAvatar name={member.displayName} size={36} isSelf={isSelf} />
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
          <span style={{ fontSize: 13, fontWeight: 600, color: isSelf ? 'var(--accent)' : 'var(--ink)' }}>
            {member.displayName}
          </span>
          {member.role === 'Leader' && (
            <span style={{
              fontSize: 8.5, fontWeight: 700, padding: '1px 5px', borderRadius: 'var(--r-pill)',
              background: 'linear-gradient(135deg, #FFD700, #FFA000)', color: '#4A3800',
              textTransform: 'uppercase', letterSpacing: '0.04em',
            }}>Leader</span>
          )}
          {member.role === 'Co-Leader' && (
            <span style={{
              fontSize: 8.5, fontWeight: 700, padding: '1px 5px', borderRadius: 'var(--r-pill)',
              background: 'var(--surface-2)', color: 'var(--ink-3)',
              textTransform: 'uppercase', letterSpacing: '0.04em',
            }}>Co-Lead</span>
          )}
        </div>
        <div className="mono" style={{ fontSize: 10.5, color: 'var(--ink-4)' }}>{member.friendCode}</div>
      </div>
      <div className="mono" style={{ fontSize: 12, fontWeight: 600, color: 'var(--ink-3)', flexShrink: 0 }}>
        {ST.fmtMins(member.minutes)}
      </div>
    </div>
  );
}

/* ---- Squad Panel ---- */
function SquadPanel() {
  const squad = FriendsData.SQUAD;
  const [squadInvites, setSquadInvites] = useState(FriendsData.SQUAD_INVITES_IN);

  return (
    <div className="arena-squad-hq">
      {/* Current squad header */}
      <div className="arena-squad-section">
        <div style={{ display: 'flex', alignItems: 'center', gap: 14 }}>
          <SquadEmblem tag={squad.tag} size={50} />
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 17, fontWeight: 700, color: 'var(--ink)' }}>{squad.name}</div>
            <div style={{ display: 'flex', gap: 12, marginTop: 4 }}>
              <span className="mono" style={{ fontSize: 11, color: 'var(--ink-3)' }}>
                <Icon name="target" size={11} style={{ verticalAlign: '-1px', marginRight: 3 }} />
                {squad.memberCount} members
              </span>
              <span className="mono" style={{ fontSize: 11, color: 'var(--ink-3)' }}>
                <Icon name="layers" size={11} style={{ verticalAlign: '-1px', marginRight: 3 }} />
                Rank #{squad.rank}
              </span>
            </div>
          </div>
        </div>
      </div>

      {/* Squad members */}
      <div className="arena-squad-section">
        <div className="arena-squad-section__head">
          <span style={{ fontSize: 16 }}>🛡</span>
          <span>Squad Members</span>
          <span className="arena-badge">{squad.members.length}</span>
        </div>
        <div className="arena-squad-list">
          {squad.members.map(m => <SquadMemberCard key={m.id} member={m} />)}
        </div>
      </div>

      {/* Squad invites */}
      {squadInvites.length > 0 && (
        <div className="arena-squad-section">
          <div className="arena-squad-section__head">
            <span style={{ fontSize: 16 }}>📩</span>
            <span>Squad Invites</span>
            <span className="arena-badge">{squadInvites.length}</span>
          </div>
          {squadInvites.map(inv => (
            <SquadInviteCard key={inv.id} invite={inv}
              onAccept={() => setSquadInvites(prev => prev.filter(i => i.id !== inv.id))}
              onDecline={() => setSquadInvites(prev => prev.filter(i => i.id !== inv.id))} />
          ))}
        </div>
      )}
    </div>
  );
}

/* ---- Friends Panel ---- */
function FriendsPanel() {
  const [friendCode, setFriendCode] = useState('');
  const [incoming, setIncoming] = useState(FriendsData.FRIEND_REQUESTS_IN);
  const [outgoing, setOutgoing] = useState(FriendsData.FRIEND_REQUESTS_OUT);
  const [friends, setFriends] = useState(FriendsData.FRIENDS_LIST);
  const [sent, setSent] = useState(false);

  const handleSend = () => {
    if (!friendCode.trim()) return;
    setOutgoing(prev => [...prev, { id: 'req-new-' + Date.now(), displayName: friendCode, friendCode: friendCode, sentAt: 'Just now' }]);
    setFriendCode('');
    setSent(true);
    setTimeout(() => setSent(false), 2000);
  };
  const handleAccept = (id) => {
    const req = incoming.find(r => r.id === id);
    setIncoming(prev => prev.filter(r => r.id !== id));
    if (req) setFriends(prev => [...prev, { ...req }]);
  };
  const handleDecline = (id) => setIncoming(prev => prev.filter(r => r.id !== id));

  return (
    <div className="arena-squad-hq">
      {/* Add friend */}
      <div className="arena-squad-section">
        <div className="arena-squad-section__head">
          <span style={{ fontSize: 16 }}>📡</span>
          <span>Add Friend</span>
        </div>
        <div style={{ display: 'flex', gap: 8 }}>
          <input className="arena-input" type="text"
            placeholder="Enter Player Tag, e.g. ABCD-1234"
            value={friendCode} onChange={e => setFriendCode(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && handleSend()} />
          <button className="arena-btn arena-btn--send" onClick={handleSend}>
            {sent ? '✓ Sent!' : 'Send'}
          </button>
        </div>
      </div>

      {/* Incoming */}
      {incoming.length > 0 && (
        <div className="arena-squad-section">
          <div className="arena-squad-section__head">
            <span style={{ fontSize: 16 }}>⚡</span>
            <span>Friend Requests</span>
            <span className="arena-badge">{incoming.length}</span>
          </div>
          {incoming.map(r => (
            <RequestCard key={r.id} req={r} type="incoming"
              onAccept={() => handleAccept(r.id)}
              onDecline={() => handleDecline(r.id)} />
          ))}
        </div>
      )}

      {/* Outgoing */}
      {outgoing.length > 0 && (
        <div className="arena-squad-section">
          <div className="arena-squad-section__head">
            <span style={{ fontSize: 16 }}>📨</span>
            <span>Pending Requests</span>
          </div>
          {outgoing.map(r => <RequestCard key={r.id} req={r} type="outgoing" />)}
        </div>
      )}

      {/* Friends list */}
      <div className="arena-squad-section">
        <div className="arena-squad-section__head">
          <span style={{ fontSize: 16 }}>👥</span>
          <span>Your Friends</span>
          <span className="arena-badge">{friends.length}</span>
        </div>
        {friends.length === 0 ? (
          <div className="arena-empty">
            <div style={{ fontSize: 28, marginBottom: 8 }}>🤝</div>
            <div style={{ fontWeight: 600, fontSize: 14 }}>No friends yet</div>
            <div style={{ color: 'var(--ink-4)', fontSize: 12.5, marginTop: 4 }}>Share your Player Tag to connect!</div>
          </div>
        ) : (
          <div className="arena-squad-grid">
            {friends.map(f => <FriendCard key={f.id} friend={f} />)}
          </div>
        )}
      </div>
    </div>
  );
}

/* ---- Main Friends Tab ---- */
function FriendsTab({ data }) {
  const stats = FriendsData.LOCAL_STATS;
  const player = FriendsData.PLAYER;
  const [socialTab, setSocialTab] = useState('squad');

  return (
    <div className="arena-root">
      <ArenaBg />

      <div className="arena-hero-header">
        <span className="arena-hero-title serif">Study Arena</span>
        <span className="arena-hero-sub">Compete. Focus. Rise.</span>
      </div>

      <div className="arena-layout">
        {/* Left column — Player card + social panels */}
        <div className="arena-left">
          <PlayerCard player={player} stats={stats} />

          {/* Squad / Friends tab switcher */}
          <div className="arena-social-tabs">
            <button className={`arena-social-tab ${socialTab === 'squad' ? 'arena-social-tab--active' : ''}`}
              onClick={() => setSocialTab('squad')}>
              <span style={{ fontSize: 15 }}>🛡</span>
              Squad
            </button>
            <button className={`arena-social-tab ${socialTab === 'friends' ? 'arena-social-tab--active' : ''}`}
              onClick={() => setSocialTab('friends')}>
              <span style={{ fontSize: 15 }}>👥</span>
              Friends
            </button>
          </div>

          {socialTab === 'squad' ? <SquadPanel /> : <FriendsPanel />}
        </div>

        {/* Right column — Leaderboard */}
        <div className="arena-right">
          <ArenaLeaderboard />
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { FriendsTab });
