ALTER TABLE users ADD COLUMN device_fingerprint_hash TEXT;
ALTER TABLE users ADD COLUMN device_label TEXT;
ALTER TABLE users ADD COLUMN device_seen_at TEXT;

CREATE INDEX IF NOT EXISTS idx_users_device_fingerprint_hash ON users(device_fingerprint_hash);
