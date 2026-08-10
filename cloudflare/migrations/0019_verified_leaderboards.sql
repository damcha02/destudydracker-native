CREATE TABLE IF NOT EXISTS leaderboard_baselines (
  user_id TEXT PRIMARY KEY,
  minutes INTEGER NOT NULL DEFAULT 0,
  sessions INTEGER NOT NULL DEFAULT 0,
  imported_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO leaderboard_baselines (user_id, minutes, sessions)
SELECT id, lifetime_study_minutes, lifetime_study_sessions
FROM users;

CREATE TABLE IF NOT EXISTS leaderboard_daily_baselines (
  user_id TEXT NOT NULL,
  date TEXT NOT NULL,
  minutes INTEGER NOT NULL DEFAULT 0,
  sessions INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, date),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

INSERT INTO leaderboard_daily_baselines (user_id, date, minutes, sessions)
SELECT user_id, date, minutes, sessions
FROM daily_stats;

CREATE TABLE IF NOT EXISTS verified_study_sessions (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  started_at TEXT NOT NULL,
  last_heartbeat_at TEXT NOT NULL,
  finished_at TEXT,
  credited_minutes INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL CHECK (status IN ('active', 'finished')),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_verified_study_sessions_one_active
  ON verified_study_sessions(user_id) WHERE status = 'active';
CREATE INDEX IF NOT EXISTS idx_verified_study_sessions_user_started
  ON verified_study_sessions(user_id, started_at DESC);

CREATE TABLE IF NOT EXISTS verified_daily_stats (
  user_id TEXT NOT NULL,
  date TEXT NOT NULL,
  minutes INTEGER NOT NULL DEFAULT 0,
  sessions INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, date),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_verified_daily_stats_date ON verified_daily_stats(date);

CREATE VIEW IF NOT EXISTS competitive_daily_stats AS
SELECT user_id, date, SUM(minutes) AS minutes, SUM(sessions) AS sessions
FROM (
  SELECT user_id, date, minutes, sessions FROM leaderboard_daily_baselines
  UNION ALL
  SELECT user_id, date, minutes, sessions FROM verified_daily_stats
)
GROUP BY user_id, date;

CREATE VIEW IF NOT EXISTS competitive_user_totals AS
SELECT u.id AS user_id,
  COALESCE(b.minutes, 0) + COALESCE(SUM(v.minutes), 0) AS minutes,
  COALESCE(b.sessions, 0) + COALESCE(SUM(v.sessions), 0) AS sessions
FROM users u
LEFT JOIN leaderboard_baselines b ON b.user_id = u.id
LEFT JOIN verified_daily_stats v ON v.user_id = u.id
GROUP BY u.id, b.minutes, b.sessions;
