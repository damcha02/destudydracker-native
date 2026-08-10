import { env } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { purgeExpiredAbuseData } from "../src/index";

const FIXED_NOW = new Date("2026-08-08T00:00:00Z");

function daysAgo(days: number) {
  return new Date(FIXED_NOW.getTime() - days * 24 * 60 * 60 * 1000).toISOString().slice(0, 19).replace("T", " ");
}

describe("purgeExpiredAbuseData — callable, deterministic retention (not just 'the cron runs it')", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM abuse_events"),
      env.DB.prepare("DELETE FROM user_flag_events"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("purges abuse_events older than the retention window and keeps newer ones", async () => {
    await env.DB.batch([
      env.DB.prepare("INSERT INTO abuse_events (event_type, created_at) VALUES ('honeypot_route', ?)").bind(daysAgo(91)),
      env.DB.prepare("INSERT INTO abuse_events (event_type, created_at) VALUES ('honeypot_route', ?)").bind(daysAgo(10)),
    ]);

    await purgeExpiredAbuseData(env, FIXED_NOW);

    const rows = await env.DB.prepare("SELECT created_at FROM abuse_events").all();
    expect(rows.results.length).toBe(1);
  });

  it("nulls all signup_* telemetry fields on users older than the retention window, keeps newer ones", async () => {
    await env.DB.prepare(`
      INSERT INTO users (id, device_secret, friend_code, display_name, signup_ip_hash, signup_country, signup_asn, signup_as_organization, created_at, updated_at, last_seen_at)
      VALUES ('old-user', 'secret', 'BCDF-2345', 'old-user', 'abc123', 'US', 1234, 'Example ISP', ?, ?, ?)
    `).bind(daysAgo(91), daysAgo(91), daysAgo(91)).run();
    await env.DB.prepare(`
      INSERT INTO users (id, device_secret, friend_code, display_name, signup_ip_hash, signup_country, signup_asn, signup_as_organization, created_at, updated_at, last_seen_at)
      VALUES ('new-user', 'secret', 'BCDG-2346', 'new-user', 'def456', 'US', 5678, 'Example ISP', ?, ?, ?)
    `).bind(daysAgo(10), daysAgo(10), daysAgo(10)).run();

    await purgeExpiredAbuseData(env, FIXED_NOW);

    const oldUser = await env.DB.prepare("SELECT signup_ip_hash AS ipHash, signup_country AS country, signup_asn AS asn, signup_as_organization AS asOrganization FROM users WHERE id = 'old-user'")
      .first<{ ipHash: string | null; country: string | null; asn: number | null; asOrganization: string | null }>();
    const newUser = await env.DB.prepare("SELECT signup_ip_hash AS ipHash FROM users WHERE id = 'new-user'").first<{ ipHash: string | null }>();

    expect(oldUser?.ipHash).toBeNull();
    expect(oldUser?.country).toBeNull();
    expect(oldUser?.asn).toBeNull();
    expect(oldUser?.asOrganization).toBeNull();
    expect(newUser?.ipHash).toBe("def456");
  });

  it("purges stale unflagged flag-event history but preserves history for currently-flagged accounts", async () => {
    await env.DB.batch([
      env.DB.prepare("INSERT INTO users (id, device_secret, friend_code, display_name, is_flagged) VALUES ('unflagged-user', 'secret', 'BCDH-2347', 'u', 0)"),
      env.DB.prepare("INSERT INTO users (id, device_secret, friend_code, display_name, is_flagged) VALUES ('flagged-user', 'secret', 'BCDJ-2348', 'f', 1)"),
    ]);
    await env.DB.batch([
      env.DB.prepare("INSERT INTO user_flag_events (user_id, reason, created_at) VALUES ('unflagged-user', 'format_atypical', ?)").bind(daysAgo(91)),
      env.DB.prepare("INSERT INTO user_flag_events (user_id, reason, created_at) VALUES ('flagged-user', 'honeypot_field', ?)").bind(daysAgo(91)),
    ]);

    await purgeExpiredAbuseData(env, FIXED_NOW);

    const unflaggedRows = await env.DB.prepare("SELECT 1 FROM user_flag_events WHERE user_id = 'unflagged-user'").all();
    const flaggedRows = await env.DB.prepare("SELECT 1 FROM user_flag_events WHERE user_id = 'flagged-user'").all();
    expect(unflaggedRows.results.length).toBe(0);
    expect(flaggedRows.results.length).toBe(1);
  });
});
