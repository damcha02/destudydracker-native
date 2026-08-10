ALTER TABLE app_announcements ADD COLUMN target_version TEXT;

CREATE INDEX IF NOT EXISTS idx_app_announcements_target_version ON app_announcements(target_version);
