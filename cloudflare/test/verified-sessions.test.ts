import { env, SELF } from "cloudflare:test";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

async function request(path: string, body: Record<string, unknown>) {
  return SELF.fetch(`https://test.local${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function createUser(userId: string, deviceSecret: string, friendCode: string, minutes = 0) {
  const response = await request("/sync", {
    user: {
      userId,
      deviceSecret,
      friendCode,
      displayName: userId,
      lifetimeStudyMinutes: minutes,
      lifetimeStudySessions: minutes ? 1 : 0,
    },
    stats: [],
  });
  expect(response.status).toBe(200);
}

describe("verified leaderboard sessions", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM verified_daily_stats"),
      env.DB.prepare("DELETE FROM verified_study_sessions"),
      env.DB.prepare("DELETE FROM leaderboard_daily_baselines"),
      env.DB.prepare("DELETE FROM leaderboard_baselines"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not let a later client sync rewrite the frozen leaderboard baseline", async () => {
    await createUser("baseline-user", "baseline-secret", "BCDF-2345", 120);
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, ?, ?)")
      .bind("baseline-user", 120, 1).run();

    const changed = await request("/sync", {
      user: {
        userId: "baseline-user",
        deviceSecret: "baseline-secret",
        friendCode: "BCDF-2345",
        displayName: "baseline-user",
        lifetimeStudyMinutes: 999_999,
        lifetimeStudySessions: 999_999,
      },
      stats: [{ date: "2026-08-08", minutes: 1440, sessions: 200 }],
    });
    expect(changed.status).toBe(200);

    const response = await request("/leaderboard", {
      userId: "baseline-user",
      deviceSecret: "baseline-secret",
      scope: "global",
      period: "overall",
    });
    const payload = await response.json<{ entries: Array<{ userId: string; minutes: number }> }>();
    expect(payload.entries.find((entry) => entry.userId === "baseline-user")?.minutes).toBe(120);
  });

  it("credits only server-created verified session records", async () => {
    await createUser("verified-user", "verified-secret", "BCDG-2346");
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 0, 0)")
      .bind("verified-user").run();

    const start = await request("/verified-session/start", { userId: "verified-user", deviceSecret: "verified-secret" });
    expect(start.status).toBe(200);
    const { sessionId } = await start.json<{ sessionId: string }>();
    const now = Date.now();
    await env.DB.prepare("UPDATE verified_study_sessions SET started_at = ?, last_heartbeat_at = ? WHERE id = ?")
      .bind(new Date(now - 20 * 60_000).toISOString(), new Date(now - 5 * 60_000).toISOString(), sessionId).run();

    const finish = await request("/verified-session/finish", { userId: "verified-user", deviceSecret: "verified-secret", sessionId });
    expect(finish.status).toBe(200);
    const row = await env.DB.prepare("SELECT SUM(minutes) AS minutes FROM verified_daily_stats WHERE user_id = ?")
      .bind("verified-user").first<{ minutes: number }>();
    expect(Number(row?.minutes)).toBeGreaterThan(0);
    expect(Number(row?.minutes)).toBeLessThanOrEqual(20);
  });

  it("settles a stale active session before returning the leaderboard", async () => {
    // Pin the worker's own clock (not just this test file's) to a fixed instant, so its
    // internal `new Date()` calls — todayIso() for the leaderboard's date filter,
    // finishVerifiedSession's `now` — agree deterministically with the fixture below, regardless
    // of real wall-clock time. Confirmed empirically that vi.setSystemTime propagates into the
    // @cloudflare/vitest-pool-workers worker isolate, not just this test's own process.
    // 2020-03-15T10:00Z is safely mid-morning in Europe/Zurich (SERVER_TIME_ZONE, index.ts:94;
    // CET = UTC+1 in March, before that year's DST transition), nowhere near local midnight.
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2020-03-15T10:00:00.000Z"));

    await createUser("stale-user", "stale-secret", "BCDH-2347");
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 0, 0)")
      .bind("stale-user").run();

    const start = await request("/verified-session/start", { userId: "stale-user", deviceSecret: "stale-secret" });
    expect(start.status).toBe(200);
    const { sessionId } = await start.json<{ sessionId: string }>();
    // last_heartbeat_at must be at least HEARTBEAT_MS+GRACE_MS (20 min) before "now" for
    // settleStaleVerifiedSession to actually consider the session stale and settle it —
    // otherwise it's left 'active' and contributes nothing yet, independent of this bug.
    await env.DB.prepare("UPDATE verified_study_sessions SET started_at = ?, last_heartbeat_at = ? WHERE id = ?")
      .bind("2020-03-15T09:30:00.000Z", "2020-03-15T09:35:00.000Z", sessionId).run();

    const response = await request("/leaderboard", {
      userId: "stale-user",
      deviceSecret: "stale-secret",
      scope: "global",
      period: "daily",
    });
    expect(response.status).toBe(200);
    const payload = await response.json<{ entries: Array<{ userId: string; minutes: number; sessions: number }> }>();
    const self = payload.entries.find((entry) => entry.userId === "stale-user");

    // normalCreditBoundary = min(startedAt+4h, lastHeartbeatAt+2h) = min(13:30Z, 11:35Z) = 11:35Z,
    // well after the pinned "now" (10:00Z) — so creditedEnd = now, not the boundary:
    // creditedMinutes = now(10:00Z) - startedAt(09:30Z) = 30 exactly.
    expect(self?.minutes).toBe(30);
    expect(self?.sessions).toBe(1);
    const session = await env.DB.prepare("SELECT status, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ status: string; creditedMinutes: number }>();
    expect(session?.status).toBe("finished");
    expect(Number(session?.creditedMinutes)).toBe(self?.minutes);
  });

  it("does not settle a fresh active session before returning the leaderboard", async () => {
    await createUser("fresh-user", "fresh-secret", "BCDJ-2348");
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 0, 0)")
      .bind("fresh-user").run();

    const start = await request("/verified-session/start", { userId: "fresh-user", deviceSecret: "fresh-secret" });
    expect(start.status).toBe(200);
    const { sessionId } = await start.json<{ sessionId: string }>();
    const now = Date.now();
    await env.DB.prepare("UPDATE verified_study_sessions SET started_at = ?, last_heartbeat_at = ? WHERE id = ?")
      .bind(new Date(now - 10 * 60_000).toISOString(), new Date(now - 5 * 60_000).toISOString(), sessionId).run();

    const response = await request("/leaderboard", {
      userId: "fresh-user",
      deviceSecret: "fresh-secret",
      scope: "global",
      period: "daily",
    });
    expect(response.status).toBe(200);

    const session = await env.DB.prepare("SELECT status, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ status: string; creditedMinutes: number }>();
    expect(session?.status).toBe("active");
    expect(Number(session?.creditedMinutes)).toBe(0);
  });

  it("settles a stale active session before returning a sync snapshot", async () => {
    await createUser("sync-stale-user", "sync-stale-secret", "BCDK-2349");
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 0, 0)")
      .bind("sync-stale-user").run();

    const start = await request("/verified-session/start", { userId: "sync-stale-user", deviceSecret: "sync-stale-secret" });
    expect(start.status).toBe(200);
    const { sessionId } = await start.json<{ sessionId: string }>();
    const now = Date.now();
    await env.DB.prepare("UPDATE verified_study_sessions SET started_at = ?, last_heartbeat_at = ? WHERE id = ?")
      .bind(new Date(now - 90 * 60_000).toISOString(), new Date(now - 70 * 60_000).toISOString(), sessionId).run();

    const sync = await request("/sync", {
      user: {
        userId: "sync-stale-user",
        deviceSecret: "sync-stale-secret",
        friendCode: "BCDK-2349",
        displayName: "sync-stale-user",
        lifetimeStudyMinutes: 90,
        lifetimeStudySessions: 1,
      },
      stats: [],
    });
    expect(sync.status).toBe(200);
    const payload = await sync.json<{ social: { cachedLeaderboards: { global: { daily: Array<{ userId: string; minutes: number }> } } } }>();
    const self = payload.social.cachedLeaderboards.global.daily.find((entry) => entry.userId === "sync-stale-user");

    expect(self?.minutes).toBeGreaterThan(0);
    const session = await env.DB.prepare("SELECT status FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ status: string }>();
    expect(session?.status).toBe("finished");
  });

  // Regression tests for the LEFT JOIN competitive_daily_stats ... WHERE ds.date = ? bug: putting
  // the date filter in WHERE instead of the JOIN's ON condition silently turns the LEFT JOIN into
  // an INNER JOIN, dropping any user with no matching daily-stats row for the requested date
  // entirely instead of keeping them with a correct 0-minute total (or, when they do have rows,
  // scoping strictly to the wrong side of a local calendar-day boundary).
  it("a user with no competitive_daily_stats row for today still appears in the daily leaderboard, with 0 minutes", async () => {
    await createUser("norows-user", "norows-secret", "BCDF-2350");
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 0, 0)")
      .bind("norows-user").run();
    // Deliberately no verified_daily_stats / verified_daily_stats_offline / leaderboard_daily_baselines
    // rows at all for this user — competitive_daily_stats has nothing for them on any date.

    const response = await request("/leaderboard", {
      userId: "norows-user",
      deviceSecret: "norows-secret",
      scope: "global",
      period: "daily",
    });
    expect(response.status).toBe(200);
    const payload = await response.json<{ entries: Array<{ userId: string; minutes: number; sessions: number }> }>();
    const self = payload.entries.find((entry) => entry.userId === "norows-user");

    // Before the fix, this user would be missing from `entries` entirely (LEFT JOIN behaving as
    // INNER JOIN); the fix keeps them present with a correct zero.
    expect(self).toBeDefined();
    expect(self?.minutes).toBe(0);
    expect(self?.sessions).toBe(0);

    // Same bug, same fix, exercised through /player-stats' non-overall getStats() path.
    const stats = await request("/player-stats", {
      userId: "norows-user",
      deviceSecret: "norows-secret",
      targetUserId: "norows-user",
    });
    expect(stats.status).toBe(200);
    const statsPayload = await stats.json<{ daily: { minutes: number; sessions: number } }>();
    expect(statsPayload.daily.minutes).toBe(0);
    expect(statsPayload.daily.sessions).toBe(0);
  });

  it("a user with data on both sides of the Europe/Zurich local-midnight boundary sees only today's minutes in the daily leaderboard", async () => {
    // Pin the worker's clock so todayIso() resolves deterministically to 2020-03-15, matching
    // the fixture rows below regardless of real wall-clock time.
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2020-03-15T10:00:00.000Z"));

    await createUser("midnight-lb-user", "midnight-lb-secret", "BCDF-2351");
    await env.DB.prepare("INSERT INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 0, 0)")
      .bind("midnight-lb-user").run();

    await env.DB.batch([
      env.DB.prepare("INSERT INTO verified_daily_stats (user_id, date, minutes, sessions) VALUES (?, ?, ?, ?)")
        .bind("midnight-lb-user", "2020-03-14", 45, 2),
      env.DB.prepare("INSERT INTO verified_daily_stats (user_id, date, minutes, sessions) VALUES (?, ?, ?, ?)")
        .bind("midnight-lb-user", "2020-03-15", 30, 1),
    ]);

    const response = await request("/leaderboard", {
      userId: "midnight-lb-user",
      deviceSecret: "midnight-lb-secret",
      scope: "global",
      period: "daily",
    });
    expect(response.status).toBe(200);
    const payload = await response.json<{ entries: Array<{ userId: string; minutes: number; sessions: number }> }>();
    const self = payload.entries.find((entry) => entry.userId === "midnight-lb-user");

    // Present (not dropped, per the LEFT JOIN fix) and scoped to exactly today's row — not
    // yesterday's, and not the sum of both.
    expect(self).toBeDefined();
    expect(self?.minutes).toBe(30);
    expect(self?.sessions).toBe(1);

    // The weekly view spans both dates, so it should see the full total.
    const weekly = await request("/leaderboard", {
      userId: "midnight-lb-user",
      deviceSecret: "midnight-lb-secret",
      scope: "global",
      period: "weekly",
    });
    const weeklyPayload = await weekly.json<{ entries: Array<{ userId: string; minutes: number }> }>();
    const weeklySelf = weeklyPayload.entries.find((entry) => entry.userId === "midnight-lb-user");
    expect(weeklySelf?.minutes).toBe(75);
  });
});
