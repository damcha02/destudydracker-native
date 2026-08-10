import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";

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
});
