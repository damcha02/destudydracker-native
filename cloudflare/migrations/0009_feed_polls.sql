CREATE TABLE IF NOT EXISTS feed_polls (
  post_id TEXT PRIMARY KEY,
  question TEXT NOT NULL DEFAULT '',
  multiple INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (post_id) REFERENCES feed_posts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS feed_poll_options (
  id TEXT PRIMARY KEY,
  post_id TEXT NOT NULL,
  text TEXT NOT NULL DEFAULT '',
  sort_order INTEGER NOT NULL DEFAULT 0,
  FOREIGN KEY (post_id) REFERENCES feed_posts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS feed_poll_votes (
  post_id TEXT NOT NULL,
  option_id TEXT NOT NULL,
  user_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (option_id, user_id),
  FOREIGN KEY (post_id) REFERENCES feed_posts(id) ON DELETE CASCADE,
  FOREIGN KEY (option_id) REFERENCES feed_poll_options(id) ON DELETE CASCADE,
  FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_feed_poll_options_post ON feed_poll_options(post_id, sort_order ASC);
CREATE INDEX IF NOT EXISTS idx_feed_poll_votes_post ON feed_poll_votes(post_id);
CREATE INDEX IF NOT EXISTS idx_feed_poll_votes_user ON feed_poll_votes(user_id);
