import { env } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { isFriendCodeAtypical, maybeFlagUser, recordFlagSignal } from "../src/index";

async function seedUser(userId: string, friendCode: string) {
  await env.DB.prepare(`
    INSERT INTO users (id, device_secret, friend_code, display_name)
    VALUES (?, ?, ?, ?)
  `).bind(userId, `${userId}-secret`, friendCode, userId).run();
}

function fakeRequest() {
  return new Request("https://test.local/sync", { method: "POST" });
}

async function isFlagged(userId: string) {
  const user = await env.DB.prepare("SELECT is_flagged AS isFlagged FROM users WHERE id = ?").bind(userId).first<{ isFlagged: number }>();
  return user?.isFlagged === 1;
}

describe("race-free flagging", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM user_flag_events"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("concurrent signals from different requests for the same user both persist — no clobbering", async () => {
    await seedUser("race-user", "BCDF-2345");
    await Promise.all([
      recordFlagSignal(env, "race-user", "honeypot_field"),
      recordFlagSignal(env, "race-user", "rapid_ip_signups"),
    ]);
    const rows = await env.DB.prepare("SELECT reason FROM user_flag_events WHERE user_id = ?").bind("race-user").all<{ reason: string }>();
    expect(rows.results.map((row) => row.reason).sort()).toEqual(["honeypot_field", "rapid_ip_signups"]);
  });

  it("a friend-code-shape signal alone never flags", async () => {
    await seedUser("shape-only-user", "BCDG-2346");
    await maybeFlagUser(env, "shape-only-user", ["format_atypical"], fakeRequest());
    expect(await isFlagged("shape-only-user")).toBe(false);
  });

  it("a friend-code-shape signal combined with a second signal does flag", async () => {
    await seedUser("shape-plus-user", "BCDH-2347");
    await maybeFlagUser(env, "shape-plus-user", ["format_atypical", "rapid_ip_signups"], fakeRequest());
    expect(await isFlagged("shape-plus-user")).toBe(true);
  });

  it("a standalone-strong signal (honeypot_field) flags on its own", async () => {
    await seedUser("honeypot-user", "BCDJ-2348");
    await maybeFlagUser(env, "honeypot-user", ["honeypot_field"], fakeRequest());
    expect(await isFlagged("honeypot-user")).toBe(true);
  });
});

describe("isFriendCodeAtypical — never rejects purely for repeated characters", () => {
  it("treats a repeated-character code like AAAA-BBBB as structurally valid (not atypical)", () => {
    // The real generator draws independently from the alphabet each position — repetition
    // is a legitimate, if statistically unlikely, output. Flagging it outright would be wrong.
    expect(isFriendCodeAtypical("AAAA-BBBB")).toBe(false);
  });

  it("treats a normally-generated code as structurally valid", () => {
    expect(isFriendCodeAtypical("XGFY-ZJ92")).toBe(false);
  });

  it("flags genuinely malformed shapes (wrong case, excluded characters, missing hyphen)", () => {
    expect(isFriendCodeAtypical("aaaa-bbbb")).toBe(true);
    expect(isFriendCodeAtypical("AAAA-BBB")).toBe(true);
    expect(isFriendCodeAtypical("AAAABBBB")).toBe(true);
    expect(isFriendCodeAtypical("IIII-0000")).toBe(true); // I and 0 are excluded from the real alphabet
  });
});
