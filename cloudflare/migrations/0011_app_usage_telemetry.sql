ALTER TABLE users ADD COLUMN app_version TEXT;
ALTER TABLE users ADD COLUMN app_platform TEXT;
ALTER TABLE users ADD COLUMN app_runtime_channel TEXT;
ALTER TABLE users ADD COLUMN app_seen_at TEXT;

CREATE TABLE IF NOT EXISTS app_telemetry_installs (
  install_id TEXT PRIMARY KEY,
  app_version TEXT NOT NULL DEFAULT '',
  app_platform TEXT NOT NULL DEFAULT '',
  app_runtime_channel TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_app_telemetry_installs_last_seen ON app_telemetry_installs(last_seen_at DESC);
