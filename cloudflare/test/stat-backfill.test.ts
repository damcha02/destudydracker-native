import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";

// All friend codes below use only the real generator's alphabet (no I/O/0/1), so the
// weak format_atypical signal never fires and each test isolates the signal it's checking.

async function sync(userId: string, deviceSecret: string, friendCode: string, lifetimeStudyMinutes: number, lifetimeStudySessions = 10) {
  return SELF.fetch("https://test.local/sync", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      user: { userId, deviceSecret, friendCode, displayName: userId, lifetimeStudyMinutes, lifetimeStudySessions },
      stats: [],
    }),
  });
}

async function getUser(userId: string) {
  return env.DB.prepare("SELECT is_flagged AS isFlagged, lifetime_study_minutes AS lifetimeStudyMinutes, updated_at AS updatedAt FROM users WHERE id = ?")
    .bind(userId).first<{ isFlagged: number; lifetimeStudyMinutes: number; updatedAt: string }>();
}

async function getReasons(userId: string) {
  const rows = await env.DB.prepare("SELECT reason FROM user_flag_events WHERE user_id = ?").bind(userId).all<{ reason: string }>();
  return rows.results.map((row) => row.reason);
}

describe("stat backfill / manipulation limits", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM user_flag_events"),
      env.DB.prepare("DELETE FROM daily_stats"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("a first sync just below the threshold stays completely unflagged — protects the normal offline-first workflow", async () => {
    const response = await sync("below-threshold", "below-secret", "BCDF-2345", 19_999);
    expect(response.status).toBe(200);
    const user = await getUser("below-threshold");
    expect(user?.isFlagged).toBe(0);
  });

  it("a first sync just above the threshold is flagged with bulk_backfill (standalone-flaggable)", async () => {
    const response = await sync("above-threshold", "above-secret", "BCDG-2346", 20_001);
    expect(response.status).toBe(200);
    const user = await getUser("above-threshold");
    expect(user?.isFlagged).toBe(1);
    expect(await getReasons("above-threshold")).toContain("bulk_backfill");
  });

  it("an implausible follow-up jump is detected using the PRE-update row, not a value already overwritten", async () => {
    const userId = "growth-user";
    const deviceSecret = "growth-secret";
    // Seed directly: 100 minutes, last updated 3 days ago. Real-world max plausible growth
    // over 3 days is nowhere near the 100,000-minute jump this test sends.
    await env.DB.prepare(`
      INSERT INTO users (id, device_secret, friend_code, display_name, lifetime_study_minutes, lifetime_study_sessions, created_at, updated_at, last_seen_at)
      VALUES (?, ?, 'BCDH-2347', ?, 100, 5, datetime('now', '-3 days'), datetime('now', '-3 days'), datetime('now', '-3 days'))
    `).bind(userId, deviceSecret, userId).run();

    const response = await sync(userId, deviceSecret, "BCDH-2347", 100_000);
    expect(response.status).toBe(200);

    // If the ordering were wrong (signal computed after the UPDATE, or from a fresh re-read),
    // this would compare the new row against itself and never fire.
    expect(await getReasons(userId)).toContain("implausible_stat_growth");

    // The write itself must still happen despite the signal being recorded.
    const user = await getUser(userId);
    expect(user?.lifetimeStudyMinutes).toBe(100_000);
    const updatedAt = new Date(`${user!.updatedAt.replace(" ", "T")}Z`);
    expect(Date.now() - updatedAt.getTime()).toBeLessThan(60_000);
  });
});
