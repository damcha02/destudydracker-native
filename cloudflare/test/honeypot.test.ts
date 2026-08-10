import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { sanitizeForLog } from "../src/index";

describe("honeypot decoy routes", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM abuse_events"),
      env.DB.prepare("DELETE FROM user_flag_events"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("returns a generic 404 and logs the hit", async () => {
    const response = await SELF.fetch("https://test.local/admin");
    expect(response.status).toBe(404);
    const rows = await env.DB.prepare("SELECT event_type AS eventType, path FROM abuse_events").all<{ eventType: string; path: string }>();
    expect(rows.results).toHaveLength(1);
    expect(rows.results[0].eventType).toBe("honeypot_route");
    expect(rows.results[0].path).toBe("/admin");
  });
});

describe("sanitizeForLog — direct unit test (control characters must not reach a real HTTP header in this test)", () => {
  it("strips control characters and truncates to the given length", () => {
    const raw = `evil${String.fromCharCode(0)}agent${"x".repeat(1000)}`;
    const sanitized = sanitizeForLog(raw, 300);
    expect(sanitized.length).toBeLessThanOrEqual(300);
    expect(/\p{Cc}/u.test(sanitized)).toBe(false);
  });
});

describe("honeypot trap field on /sync", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM user_flag_events"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("flags an account that fills the referralCode trap field, on its own", async () => {
    const response = await SELF.fetch("https://test.local/sync", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        user: { userId: "trap-user", deviceSecret: "trap-secret", friendCode: "BCDK-2349", displayName: "trap-user", referralCode: "definitely-not-a-real-field" },
        stats: [],
      }),
    });
    expect(response.status).toBe(200);
    const user = await env.DB.prepare("SELECT is_flagged AS isFlagged FROM users WHERE id = 'trap-user'").first<{ isFlagged: number }>();
    expect(user?.isFlagged).toBe(1);
    const reasons = await env.DB.prepare("SELECT reason FROM user_flag_events WHERE user_id = 'trap-user'").all<{ reason: string }>();
    expect(reasons.results.map((row) => row.reason)).toContain("honeypot_field");
  });
});
