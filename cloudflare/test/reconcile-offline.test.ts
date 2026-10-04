import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";

async function request(path: string, body: Record<string, unknown>) {
  return SELF.fetch(`https://test.local${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function createUser(userId: string, deviceSecret: string, friendCode: string) {
  const response = await request("/sync", {
    user: { userId, deviceSecret, friendCode, displayName: userId, lifetimeStudyMinutes: 0, lifetimeStudySessions: 0 },
    stats: [],
  });
  expect(response.status).toBe(200);
}

/**
 * Starts, backdates, and finishes a verified session so it becomes a 'finished' anchor with a
 * known credited-minutes boundary. `lastHeartbeatMinutesAgo` must exceed 120
 * (VERIFIED_SESSION_NORMAL_CREDIT_GRACE_MS, the normal-credit grace window) so the credited end
 * lands meaningfully before "now", leaving real room between the returned `gapStart` and actual
 * test-execution time for offline intervals to occupy.
 */
async function createFinishedAnchor(userId: string, deviceSecret: string, startedMinutesAgo: number, lastHeartbeatMinutesAgo: number) {
  const start = await request("/verified-session/start", { userId, deviceSecret });
  const { sessionId } = await start.json<{ sessionId: string }>();
  const now = Date.now();
  await env.DB.prepare("UPDATE verified_study_sessions SET started_at = ?, last_heartbeat_at = ? WHERE id = ?")
    .bind(new Date(now - startedMinutesAgo * 60_000).toISOString(), new Date(now - lastHeartbeatMinutesAgo * 60_000).toISOString(), sessionId).run();

  const finish = await request("/verified-session/finish", { userId, deviceSecret, sessionId });
  expect(finish.status).toBe(200);

  // Read back the server's own record of the boundary rather than recomputing it in the test,
  // so the test tracks exactly what handleVerifiedSessionReconcileOffline will see as the anchor.
  const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
    .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
  const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;
  return { sessionId, creditedMinutes: row!.creditedMinutes, gapStart };
}

describe("offline verified-session reconciliation", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM verified_daily_stats"),
      env.DB.prepare("DELETE FROM verified_daily_stats_offline"),
      env.DB.prepare("DELETE FROM verified_offline_reconciliations"),
      env.DB.prepare("DELETE FROM verified_study_sessions"),
      env.DB.prepare("DELETE FROM user_flag_events"),
      env.DB.prepare("DELETE FROM leaderboard_baselines"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("credits a genuine offline gap into verified_daily_stats_offline, distinct from the live-verified table", async () => {
    await createUser("offline-user", "offline-secret", "BCDF-2345");
    const { sessionId, creditedMinutes: liveCreditedMinutes, gapStart } = await createFinishedAnchor("offline-user", "offline-secret", 130, 125);
    expect(liveCreditedMinutes).toBeGreaterThan(0);

    // 5 real minutes of room exist between gapStart and "now" here (125 - 120); stay well inside it.
    const intervalEnd = new Date(gapStart + 3 * 60_000).toISOString();
    const response = await request("/verified-session/reconcile-offline", {
      userId: "offline-user",
      deviceSecret: "offline-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: intervalEnd }],
    });
    expect(response.status).toBe(200);
    const body = await response.json<{ ok: boolean; creditedMinutes: number; cappedFromClaimedMinutes: number }>();
    expect(body.ok).toBe(true);
    expect(body.creditedMinutes).toBeGreaterThan(0);
    expect(body.cappedFromClaimedMinutes).toBe(0);

    const offlineRow = await env.DB.prepare("SELECT SUM(minutes) AS minutes FROM verified_daily_stats_offline WHERE user_id = ?")
      .bind("offline-user").first<{ minutes: number }>();
    expect(Number(offlineRow?.minutes)).toBe(body.creditedMinutes);

    // The live-verified table must be untouched by the offline endpoint — only what /finish wrote earlier.
    const liveRow = await env.DB.prepare("SELECT SUM(minutes) AS minutes FROM verified_daily_stats WHERE user_id = ?")
      .bind("offline-user").first<{ minutes: number }>();
    expect(Number(liveRow?.minutes)).toBe(liveCreditedMinutes);
  });

  it("truncates a claim that exceeds the plausibility cap and reports how much was cut", async () => {
    await createUser("cap-user", "cap-secret", "BCDG-2346");
    // 15 real minutes of room (135 - 120) between gapStart and "now".
    const { sessionId, gapStart } = await createFinishedAnchor("cap-user", "cap-secret", 140, 135);

    // A real single interval can never claim more than actual elapsed wall-clock time (it's
    // clamped to "now" server-side) — so simulate an implausible claim the way a buggy or
    // malicious client actually could: submitting overlapping/duplicate intervals to inflate
    // the total beyond what real elapsed time between the anchor and now could support.
    const oneInterval = { startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 14 * 60_000).toISOString() };
    const response = await request("/verified-session/reconcile-offline", {
      userId: "cap-user",
      deviceSecret: "cap-secret",
      anchorSessionId: sessionId,
      intervals: [oneInterval, oneInterval, oneInterval],
    });
    expect(response.status).toBe(200);
    const body = await response.json<{ ok: boolean; creditedMinutes: number; cappedFromClaimedMinutes: number }>();
    expect(body.cappedFromClaimedMinutes).toBeGreaterThan(0);

    const ledger = await env.DB.prepare("SELECT claimed_minutes AS claimedMinutes, credited_minutes AS creditedMinutes, was_capped AS wasCapped FROM verified_offline_reconciliations WHERE user_id = ?")
      .bind("cap-user").first<{ claimedMinutes: number; creditedMinutes: number; wasCapped: number }>();
    expect(ledger?.wasCapped).toBe(1);
    expect(ledger!.creditedMinutes).toBeLessThan(ledger!.claimedMinutes);
  });

  it("clamps claims to the server-known anchor — a client cannot claim time already covered by the live path", async () => {
    await createUser("clamp-user", "clamp-secret", "BCDH-2347");
    const { sessionId, gapStart } = await createFinishedAnchor("clamp-user", "clamp-secret", 130, 125);

    // Claim starting 20 minutes before the real gap start, i.e. reaching back into
    // already-live-credited time, but only 3 real minutes past the true gap start.
    const claimedStart = new Date(gapStart - 20 * 60_000).toISOString();
    const claimedEnd = new Date(gapStart + 3 * 60_000).toISOString();
    const response = await request("/verified-session/reconcile-offline", {
      userId: "clamp-user",
      deviceSecret: "clamp-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: claimedStart, endedAt: claimedEnd }],
    });
    expect(response.status).toBe(200);
    const body = await response.json<{ creditedMinutes: number }>();
    // If the pre-gap portion were not clamped away, this would credit ~23 minutes instead of ~3.
    expect(body.creditedMinutes).toBeLessThanOrEqual(3);

    const ledger = await env.DB.prepare("SELECT gap_started_at AS gapStartedAt FROM verified_offline_reconciliations WHERE user_id = ?")
      .bind("clamp-user").first<{ gapStartedAt: string }>();
    expect(new Date(ledger!.gapStartedAt).getTime()).toBe(gapStart);
  });

  it("records a chainTipHash mismatch for audit without rejecting or blocking credit", async () => {
    await createUser("chain-user", "chain-secret", "BCDJ-2348");
    const { sessionId, gapStart } = await createFinishedAnchor("chain-user", "chain-secret", 130, 125);

    const response = await request("/verified-session/reconcile-offline", {
      userId: "chain-user",
      deviceSecret: "chain-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 3 * 60_000).toISOString() }],
      chainTipHash: "not-a-real-hash",
    });
    expect(response.status).toBe(200);
    const body = await response.json<{ ok: boolean; creditedMinutes: number }>();
    expect(body.ok).toBe(true);
    expect(body.creditedMinutes).toBeGreaterThan(0);

    const ledger = await env.DB.prepare("SELECT chain_verified AS chainVerified FROM verified_offline_reconciliations WHERE user_id = ?")
      .bind("chain-user").first<{ chainVerified: number }>();
    expect(ledger?.chainVerified).toBe(0);
  });

  it("verifies a chainTipHash that an honest client would actually compute over its raw, unclamped submission", async () => {
    await createUser("honest-chain-user", "honest-chain-secret", "BCDL-234N");
    const { sessionId, gapStart } = await createFinishedAnchor("honest-chain-user", "honest-chain-secret", 130, 125);

    // Mirrors desktop/src/lib/social.ts's hashOfflineIntervals exactly: sha256 over the raw
    // {startedAt, endedAt} pairs as sent, before the server applies any gap/now clamping.
    const intervals = [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 3 * 60_000).toISOString() }];
    const canonical = JSON.stringify(intervals.map((i) => ({ startedAt: i.startedAt, endedAt: i.endedAt })));
    const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(canonical));
    const chainTipHash = Array.from(new Uint8Array(digest)).map((byte) => byte.toString(16).padStart(2, "0")).join("");

    const response = await request("/verified-session/reconcile-offline", {
      userId: "honest-chain-user",
      deviceSecret: "honest-chain-secret",
      anchorSessionId: sessionId,
      intervals,
      chainTipHash,
    });
    expect(response.status).toBe(200);
    const body = await response.json<{ ok: boolean; creditedMinutes: number }>();
    expect(body.creditedMinutes).toBeGreaterThan(0);

    const ledger = await env.DB.prepare("SELECT chain_verified AS chainVerified FROM verified_offline_reconciliations WHERE user_id = ?")
      .bind("honest-chain-user").first<{ chainVerified: number }>();
    expect(ledger?.chainVerified).toBe(1);
  });

  it("a single capped claim alone does not flag the account, but combined with a second distinct signal it does", async () => {
    await createUser("flag-user", "flag-secret", "BCDK-2349");
    const { sessionId, gapStart } = await createFinishedAnchor("flag-user", "flag-secret", 140, 135);

    const oneInterval = { startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 14 * 60_000).toISOString() };
    const capped = await request("/verified-session/reconcile-offline", {
      userId: "flag-user",
      deviceSecret: "flag-secret",
      anchorSessionId: sessionId,
      intervals: [oneInterval, oneInterval, oneInterval],
    });
    const cappedBody = await capped.json<{ cappedFromClaimedMinutes: number }>();
    expect(cappedBody.cappedFromClaimedMinutes).toBeGreaterThan(0);

    const reasons = await env.DB.prepare("SELECT reason FROM user_flag_events WHERE user_id = ?").bind("flag-user").all<{ reason: string }>();
    expect(reasons.results.map((row) => row.reason)).toEqual(["offline_credit_capped"]);

    const afterCapped = await env.DB.prepare("SELECT is_flagged AS isFlagged FROM users WHERE id = 'flag-user'").first<{ isFlagged: number }>();
    expect(afterCapped?.isFlagged).toBe(0);

    // A second, distinct weak signal (atypical friend-code shape on a re-sync) combined with the
    // earlier capped-claim signal should now cross the 2-reason auto-flag threshold.
    await request("/sync", {
      user: { userId: "flag-user", deviceSecret: "flag-secret", friendCode: "not-a-real-code-shape", displayName: "flag-user" },
      stats: [],
    });
    const afterSecondSignal = await env.DB.prepare("SELECT is_flagged AS isFlagged FROM users WHERE id = 'flag-user'").first<{ isFlagged: number }>();
    expect(afterSecondSignal?.isFlagged).toBe(1);
  });
});
