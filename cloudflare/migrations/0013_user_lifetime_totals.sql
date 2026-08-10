ALTER TABLE users ADD COLUMN lifetime_study_minutes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN lifetime_study_sessions INTEGER NOT NULL DEFAULT 0;

UPDATE users
SET lifetime_study_minutes = COALESCE((
  SELECT SUM(minutes)
  FROM daily_stats
  WHERE daily_stats.user_id = users.id
), 0),
lifetime_study_sessions = COALESCE((
  SELECT SUM(sessions)
  FROM daily_stats
  WHERE daily_stats.user_id = users.id
), 0);
