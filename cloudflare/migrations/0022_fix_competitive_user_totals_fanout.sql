-- competitive_user_totals (0020) LEFT JOINs verified_daily_stats and
-- verified_daily_stats_offline directly onto users, then SUMs both under the same
-- GROUP BY. Both tables have one row per date, so joining them side by side before
-- aggregating produces a cross product: a user with 7 verified_daily_stats rows and
-- 2 verified_daily_stats_offline rows gets 14 joined rows, so SUM(v.minutes) counts
-- each verified day twice and SUM(o.minutes) counts each offline day 7 times.
-- Leaderboard/overall totals end up inflated for anyone who has ever used offline
-- reconciliation. Fix: pre-aggregate each source per user in its own subquery before
-- joining, so every join is 1:1. The intentional half-weight on offline minutes
-- (see 0020's comment, mirrored by the non-buggy competitive_daily_stats view) is
-- preserved unchanged; sessions stay unhalved for both sources.
DROP VIEW IF EXISTS competitive_user_totals;
CREATE VIEW competitive_user_totals AS
SELECT u.id AS user_id,
  COALESCE(b.minutes, 0) + COALESCE(v.minutes, 0) + COALESCE(o.minutes, 0) / 2 AS minutes,
  COALESCE(b.sessions, 0) + COALESCE(v.sessions, 0) + COALESCE(o.sessions, 0) AS sessions
FROM users u
LEFT JOIN leaderboard_baselines b ON b.user_id = u.id
LEFT JOIN (
  SELECT user_id, SUM(minutes) AS minutes, SUM(sessions) AS sessions
  FROM verified_daily_stats
  GROUP BY user_id
) v ON v.user_id = u.id
LEFT JOIN (
  SELECT user_id, SUM(minutes) AS minutes, SUM(sessions) AS sessions
  FROM verified_daily_stats_offline
  GROUP BY user_id
) o ON o.user_id = u.id;
