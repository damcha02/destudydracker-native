CREATE TABLE IF NOT EXISTS squad_member_history (
  id TEXT PRIMARY KEY,
  squad_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('leader', 'co_leader', 'elder', 'member')),
  joined_at TEXT NOT NULL,
  left_at TEXT,
  FOREIGN KEY (squad_id) REFERENCES squads(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS squad_daily_scores (
  squad_id TEXT NOT NULL,
  date TEXT NOT NULL,
  average_minutes REAL NOT NULL DEFAULT 0,
  total_minutes INTEGER NOT NULL DEFAULT 0,
  member_count INTEGER NOT NULL DEFAULT 0,
  rank INTEGER NOT NULL DEFAULT 0,
  points INTEGER NOT NULL DEFAULT 0,
  scored_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (squad_id, date),
  FOREIGN KEY (squad_id) REFERENCES squads(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_squad_member_history_squad_dates ON squad_member_history(squad_id, joined_at, left_at);
CREATE INDEX IF NOT EXISTS idx_squad_member_history_user_dates ON squad_member_history(user_id, joined_at, left_at);
CREATE INDEX IF NOT EXISTS idx_squad_daily_scores_date_rank ON squad_daily_scores(date, rank);
CREATE INDEX IF NOT EXISTS idx_squad_daily_scores_squad_date ON squad_daily_scores(squad_id, date);

INSERT INTO squad_member_history (id, squad_id, user_id, role, joined_at)
SELECT 'backfill-' || squad_id || '-' || user_id, squad_id, user_id, role, joined_at
FROM squad_members
WHERE NOT EXISTS (
  SELECT 1
  FROM squad_member_history h
  WHERE h.squad_id = squad_members.squad_id
    AND h.user_id = squad_members.user_id
    AND h.left_at IS NULL
);
