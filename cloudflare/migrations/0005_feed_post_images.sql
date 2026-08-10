ALTER TABLE feed_posts ADD COLUMN image_key TEXT;
ALTER TABLE feed_posts ADD COLUMN image_mime_type TEXT;
ALTER TABLE feed_posts ADD COLUMN image_expires_at TEXT;
ALTER TABLE feed_posts ADD COLUMN image_expired_at TEXT;
ALTER TABLE feed_posts ADD COLUMN image_size_bytes INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_feed_posts_image_expires ON feed_posts(image_expires_at);

CREATE TABLE IF NOT EXISTS r2_usage_monthly (
  month TEXT PRIMARY KEY,
  storage_bytes INTEGER NOT NULL DEFAULT 0,
  class_a_ops INTEGER NOT NULL DEFAULT 0,
  class_b_ops INTEGER NOT NULL DEFAULT 0,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS r2_alert_state (
  id TEXT PRIMARY KEY CHECK (id = 'global'),
  level TEXT NOT NULL DEFAULT 'ok',
  notified_at TEXT,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
