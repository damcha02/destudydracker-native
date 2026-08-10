CREATE TABLE IF NOT EXISTS squad_score_jobs (
  date TEXT PRIMARY KEY,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_squad_score_jobs_created_at ON squad_score_jobs(created_at);
