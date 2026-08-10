CREATE TABLE IF NOT EXISTS verified_daily_stats_offline (
  user_id TEXT NOT NULL,
  date TEXT NOT NULL,
  minutes INTEGER NOT NULL DEFAULT 0,
  sessions INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id, date),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_verified_daily_stats_offline_date ON verified_daily_stats_offline(date);

-- Audit ledger: one row per reconciliation call (not per raw interval), for abuse review.
CREATE TABLE IF NOT EXISTS verified_offline_reconciliations (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  anchor_session_id TEXT NOT NULL,
  gap_started_at TEXT NOT NULL,
  gap_ended_at TEXT NOT NULL,
  claimed_minutes INTEGER NOT NULL,
  credited_minutes INTEGER NOT NULL,
  was_capped INTEGER NOT NULL DEFAULT 0,
  chain_tip_hash TEXT,
  chain_verified INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_verified_offline_reconciliations_user ON verified_offline_reconciliations(user_id, created_at DESC);

-- Extends the competitive_daily_stats/competitive_user_totals union from 0019 with a third,
-- discounted-weight source: offline-reconciled credit is real but lower-trust than a
-- live-witnessed heartbeat, so it counts at half weight toward competitive totals.
DROP VIEW IF EXISTS competitive_daily_stats;
CREATE VIEW competitive_daily_stats AS
SELECT user_id, date, SUM(minutes) AS minutes, SUM(sessions) AS sessions
FROM (
  SELECT user_id, date, minutes, sessions FROM leaderboard_daily_baselines
  UNION ALL
  SELECT user_id, date, minutes, sessions FROM verified_daily_stats
  UNION ALL
  SELECT user_id, date, minutes / 2, sessions FROM verified_daily_stats_offline
)
GROUP BY user_id, date;

DROP VIEW IF EXISTS competitive_user_totals;
CREATE VIEW competitive_user_totals AS
SELECT u.id AS user_id,
  COALESCE(b.minutes, 0) + COALESCE(SUM(v.minutes), 0) + COALESCE(SUM(o.minutes), 0) / 2 AS minutes,
  COALESCE(b.sessions, 0) + COALESCE(SUM(v.sessions), 0) + COALESCE(SUM(o.sessions), 0) AS sessions
FROM users u
LEFT JOIN leaderboard_baselines b ON b.user_id = u.id
LEFT JOIN verified_daily_stats v ON v.user_id = u.id
LEFT JOIN verified_daily_stats_offline o ON o.user_id = u.id
GROUP BY u.id, b.minutes, b.sessions;
