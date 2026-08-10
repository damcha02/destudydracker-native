ALTER TABLE users ADD COLUMN is_flagged INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN signup_ip_hash TEXT;
ALTER TABLE users ADD COLUMN signup_country TEXT;
ALTER TABLE users ADD COLUMN signup_asn INTEGER;
ALTER TABLE users ADD COLUMN signup_as_organization TEXT;

CREATE INDEX IF NOT EXISTS idx_users_is_flagged ON users(is_flagged);

CREATE TABLE IF NOT EXISTS user_flag_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id TEXT NOT NULL,
  reason TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE (user_id, reason),
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_user_flag_events_created ON user_flag_events(created_at);

CREATE TABLE IF NOT EXISTS abuse_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT NOT NULL,
  ip_hash TEXT,
  country TEXT,
  asn INTEGER,
  as_organization TEXT,
  path TEXT,
  user_agent TEXT,
  user_id TEXT,
  detail TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_abuse_events_created ON abuse_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_abuse_events_ip_hash ON abuse_events(ip_hash);
