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

async function startSession(userId: string, deviceSecret: string) {
  const start = await request("/verified-session/start", { userId, deviceSecret });
  expect(start.status).toBe(200);
  const { sessionId } = await start.json<{ sessionId: string }>();
  return sessionId;
}

async function backdate(sessionId: string, startedAt: Date, lastHeartbeatAt: Date) {
  await env.DB.prepare("UPDATE verified_study_sessions SET started_at = ?, last_heartbeat_at = ? WHERE id = ?")
    .bind(startedAt.toISOString(), lastHeartbeatAt.toISOString(), sessionId).run();
}

async function offlineSum(userId: string) {
  const row = await env.DB.prepare("SELECT SUM(minutes) AS minutes FROM verified_daily_stats_offline WHERE user_id = ?")
    .bind(userId).first<{ minutes: number | null }>();
  return Number(row?.minutes ?? 0);
}

async function normalSum(userId: string) {
  const row = await env.DB.prepare("SELECT SUM(minutes) AS minutes FROM verified_daily_stats WHERE user_id = ?")
    .bind(userId).first<{ minutes: number | null }>();
  return Number(row?.minutes ?? 0);
}

describe("verified-session 2-hour normal-credit grace window", () => {
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

  it("1. a 20-minute heartbeat gap is fully normal-credited", async () => {
    await createUser("gap20-user", "gap20-secret", "BCDF-2401");
    const sessionId = await startSession("gap20-user", "gap20-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 20 * 60_000), new Date(now - 20 * 60_000));

    const finish = await request("/verified-session/finish", { userId: "gap20-user", deviceSecret: "gap20-secret", sessionId });
    expect(finish.status).toBe(200);
    const body = await finish.json<{ creditedMinutes: number }>();
    expect(body.creditedMinutes).toBeGreaterThanOrEqual(19);
    expect(body.creditedMinutes).toBeLessThanOrEqual(21);
    expect(await offlineSum("gap20-user")).toBe(0);
  });

  it("2. a 1h59m heartbeat gap is fully normal-credited", async () => {
    await createUser("gap119-user", "gap119-secret", "BCDF-2402");
    const sessionId = await startSession("gap119-user", "gap119-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 119 * 60_000), new Date(now - 119 * 60_000));

    const finish = await request("/verified-session/finish", { userId: "gap119-user", deviceSecret: "gap119-secret", sessionId });
    const body = await finish.json<{ creditedMinutes: number }>();
    expect(body.creditedMinutes).toBeGreaterThanOrEqual(118);
    expect(body.creditedMinutes).toBeLessThanOrEqual(120);
    expect(await offlineSum("gap119-user")).toBe(0);
  });

  it("3. a gap of exactly 2 hours is fully normal-credited, and nothing remains for offline reconciliation", async () => {
    await createUser("gap120-user", "gap120-secret", "BCDF-2403");
    const sessionId = await startSession("gap120-user", "gap120-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 120 * 60_000), new Date(now - 120 * 60_000));

    const finish = await request("/verified-session/finish", { userId: "gap120-user", deviceSecret: "gap120-secret", sessionId });
    const body = await finish.json<{ creditedMinutes: number }>();
    expect(body.creditedMinutes).toBeGreaterThanOrEqual(119);
    expect(body.creditedMinutes).toBeLessThanOrEqual(120);

    // Nothing legitimate is left in the gap — a follow-up reconcile-offline call for whatever
    // sliver remains after the boundary must credit essentially nothing.
    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;
    const reconcile = await request("/verified-session/reconcile-offline", {
      userId: "gap120-user",
      deviceSecret: "gap120-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date().toISOString() }],
    });
    const reconcileBody = await reconcile.json<{ creditedMinutes: number }>();
    expect(reconcileBody.creditedMinutes).toBeLessThanOrEqual(1);
  });

  it("4. a 2h01m gap splits into ~2h normal credit and ~1m offline credit", async () => {
    await createUser("gap121-user", "gap121-secret", "BCDF-2404");
    const sessionId = await startSession("gap121-user", "gap121-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 121 * 60_000), new Date(now - 121 * 60_000));

    const finish = await request("/verified-session/finish", { userId: "gap121-user", deviceSecret: "gap121-secret", sessionId });
    const finishBody = await finish.json<{ creditedMinutes: number }>();
    expect(finishBody.creditedMinutes).toBeGreaterThanOrEqual(119);
    expect(finishBody.creditedMinutes).toBeLessThanOrEqual(120);

    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;
    const reconcile = await request("/verified-session/reconcile-offline", {
      userId: "gap121-user",
      deviceSecret: "gap121-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date().toISOString() }],
    });
    const reconcileBody = await reconcile.json<{ creditedMinutes: number }>();
    expect(reconcileBody.creditedMinutes).toBeGreaterThanOrEqual(0);
    expect(reconcileBody.creditedMinutes).toBeLessThanOrEqual(3);
  });

  it("5. a 5-hour gap splits into exactly 2h normal credit and 3h offline credit", async () => {
    await createUser("gap5h-user", "gap5h-secret", "BCDF-2405");
    const sessionId = await startSession("gap5h-user", "gap5h-secret");
    const now = Date.now();
    // started_at === last_heartbeat_at so "elapsed since last heartbeat" is exactly "elapsed since
    // start" — isolates the 2h grace boundary from the unrelated 4h absolute session cap.
    await backdate(sessionId, new Date(now - 300 * 60_000), new Date(now - 300 * 60_000));

    const finish = await request("/verified-session/finish", { userId: "gap5h-user", deviceSecret: "gap5h-secret", sessionId });
    const finishBody = await finish.json<{ creditedMinutes: number }>();
    expect(finishBody.creditedMinutes).toBeGreaterThanOrEqual(119);
    expect(finishBody.creditedMinutes).toBeLessThanOrEqual(120);

    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;
    const reconcile = await request("/verified-session/reconcile-offline", {
      userId: "gap5h-user",
      deviceSecret: "gap5h-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date().toISOString() }],
    });
    const reconcileBody = await reconcile.json<{ creditedMinutes: number }>();
    expect(reconcileBody.creditedMinutes).toBeGreaterThanOrEqual(178);
    expect(reconcileBody.creditedMinutes).toBeLessThanOrEqual(181);
  });

  it("6. reconnect after laptop sleep — offline reconciliation against a still-active anchor uses the same 2h boundary", async () => {
    await createUser("sleep-user", "sleep-secret", "BCDF-2406");
    const sessionId = await startSession("sleep-user", "sleep-secret");
    const now = Date.now();
    // Session never explicitly finished — client detects the gap itself and reconciles while the
    // server still considers the anchor 'active'.
    await backdate(sessionId, new Date(now - 150 * 60_000), new Date(now - 150 * 60_000));

    const reconcile = await request("/verified-session/reconcile-offline", {
      userId: "sleep-user",
      deviceSecret: "sleep-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(now - 150 * 60_000).toISOString(), endedAt: new Date().toISOString() }],
    });
    expect(reconcile.status).toBe(200);
    const body = await reconcile.json<{ creditedMinutes: number }>();
    // Only the portion past the 2h boundary (150 - 120 = 30 min) is offline-eligible; the rest
    // stays unclaimed by this endpoint (it's normal-credit territory, settled separately).
    expect(body.creditedMinutes).toBeGreaterThanOrEqual(28);
    expect(body.creditedMinutes).toBeLessThanOrEqual(31);

    const session = await env.DB.prepare("SELECT status FROM verified_study_sessions WHERE id = ?").bind(sessionId).first<{ status: string }>();
    expect(session?.status).toBe("active");
  });

  it("7. a duplicate reconciliation request for the same anchor does not add credit twice", async () => {
    await createUser("dup-user", "dup-secret", "BCDF-2407");
    const sessionId = await startSession("dup-user", "dup-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 140 * 60_000), new Date(now - 140 * 60_000));
    await request("/verified-session/finish", { userId: "dup-user", deviceSecret: "dup-secret", sessionId });

    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;
    const payload = {
      userId: "dup-user",
      deviceSecret: "dup-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date().toISOString() }],
    };

    const first = await request("/verified-session/reconcile-offline", payload);
    const firstBody = await first.json<{ creditedMinutes: number }>();
    expect(firstBody.creditedMinutes).toBeGreaterThan(0);
    const afterFirst = await offlineSum("dup-user");
    expect(afterFirst).toBe(firstBody.creditedMinutes);

    // Exact same request, resubmitted (retry, double-click, replay).
    const second = await request("/verified-session/reconcile-offline", payload);
    const secondBody = await second.json<{ creditedMinutes: number }>();
    expect(secondBody.creditedMinutes).toBe(0);
    expect(await offlineSum("dup-user")).toBe(afterFirst);
  });

  it("8. a duplicate finish request for the same session does not add credit twice", async () => {
    await createUser("dupfinish-user", "dupfinish-secret", "BCDF-2408");
    const sessionId = await startSession("dupfinish-user", "dupfinish-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 30 * 60_000), new Date(now - 30 * 60_000));

    const first = await request("/verified-session/finish", { userId: "dupfinish-user", deviceSecret: "dupfinish-secret", sessionId });
    const firstBody = await first.json<{ creditedMinutes: number }>();
    expect(firstBody.creditedMinutes).toBeGreaterThan(0);
    const afterFirst = await normalSum("dupfinish-user");
    expect(afterFirst).toBe(firstBody.creditedMinutes);

    // The session is already 'finished', so the handler's own active-session lookup rejects the
    // retry with 404 before any credit logic runs — still "no duplicate credit", just via a
    // different (also safe) path than a same-session double /finish mid-flight would take.
    const second = await request("/verified-session/finish", { userId: "dupfinish-user", deviceSecret: "dupfinish-secret", sessionId });
    expect(second.status).toBe(404);
    expect(await normalSum("dupfinish-user")).toBe(afterFirst);
  });

  it("9. resumed heartbeats after a gap do not re-credit the already-covered grace interval", async () => {
    await createUser("resume-user", "resume-secret", "BCDF-2409");
    const sessionId = await startSession("resume-user", "resume-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 150 * 60_000), new Date(now - 150 * 60_000));

    const firstReconcile = await request("/verified-session/reconcile-offline", {
      userId: "resume-user",
      deviceSecret: "resume-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(now - 150 * 60_000).toISOString(), endedAt: new Date().toISOString() }],
    });
    const firstBody = await firstReconcile.json<{ creditedMinutes: number }>();
    expect(firstBody.creditedMinutes).toBeGreaterThan(0);
    const afterFirst = await offlineSum("resume-user");

    // Heartbeats resume on the still-active session (network came back).
    const heartbeat = await request("/verified-session/heartbeat", { userId: "resume-user", deviceSecret: "resume-secret", sessionId });
    expect(heartbeat.status).toBe(200);

    // A client mistakenly re-attempting to reconcile the same old window should get nothing —
    // both because the session-derived boundary has moved forward (fresh heartbeat) and because
    // the ledger floor already covers it.
    const secondReconcile = await request("/verified-session/reconcile-offline", {
      userId: "resume-user",
      deviceSecret: "resume-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(now - 150 * 60_000).toISOString(), endedAt: new Date().toISOString() }],
    });
    const secondBody = await secondReconcile.json<{ creditedMinutes: number }>();
    expect(secondBody.creditedMinutes).toBe(0);
    expect(await offlineSum("resume-user")).toBe(afterFirst);
  });

  it("10. overlapping intervals across separate calls do not double count", async () => {
    await createUser("overlap-user", "overlap-secret", "BCDF-2410");
    const sessionId = await startSession("overlap-user", "overlap-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 140 * 60_000), new Date(now - 140 * 60_000));
    await request("/verified-session/finish", { userId: "overlap-user", deviceSecret: "overlap-secret", sessionId });

    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;

    // First call claims the first half of the remaining gap.
    const first = await request("/verified-session/reconcile-offline", {
      userId: "overlap-user",
      deviceSecret: "overlap-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 8 * 60_000).toISOString() }],
    });
    const firstBody = await first.json<{ creditedMinutes: number }>();
    expect(firstBody.creditedMinutes).toBeGreaterThan(0);
    const afterFirst = await offlineSum("overlap-user");

    // Second call overlaps the first entirely and claims further into the (already-elapsed,
    // backdated) window — the floor from call 1 already covers everything up through its own
    // real call-time, so this must not re-credit the overlapping portion.
    const second = await request("/verified-session/reconcile-offline", {
      userId: "overlap-user",
      deviceSecret: "overlap-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 15 * 60_000).toISOString() }],
    });
    const secondBody = await second.json<{ creditedMinutes: number }>();
    expect(secondBody.creditedMinutes).toBe(0);
    expect(await offlineSum("overlap-user")).toBe(afterFirst);
  });

  it("11. a grace interval crossing midnight splits credit correctly between calendar dates", async () => {
    await createUser("midnight-user", "midnight-secret", "BCDF-2411");
    const sessionId = await startSession("midnight-user", "midnight-secret");
    // Fixed, absolute timestamps (not relative to "now") so the test is deterministic regardless
    // of when it actually runs. Day-bucketing uses SERVER_TIME_ZONE (Europe/Zurich, index.ts:94),
    // not UTC — in January that's UTC+1 (CET, no DST), so 22:30 UTC is 23:30 local. Both
    // startedAt and lastHeartbeatAt sit 30 minutes before local midnight; the 2h grace boundary
    // then lands 90 minutes after local midnight.
    await backdate(sessionId, new Date("2026-01-01T22:30:00.000Z"), new Date("2026-01-01T22:30:00.000Z"));

    const finish = await request("/verified-session/finish", { userId: "midnight-user", deviceSecret: "midnight-secret", sessionId });
    const body = await finish.json<{ creditedMinutes: number }>();
    expect(body.creditedMinutes).toBe(120);

    const rows = await env.DB.prepare("SELECT date, minutes FROM verified_daily_stats WHERE user_id = ? ORDER BY date")
      .bind("midnight-user").all<{ date: string; minutes: number }>();
    expect(rows.results).toEqual([
      { date: "2026-01-01", minutes: 30 },
      { date: "2026-01-02", minutes: 90 },
    ]);
  });

  it("12. a stale/reordered request cannot extend credit backward past what's already covered", async () => {
    await createUser("reorder-user", "reorder-secret", "BCDF-2412");
    const sessionId = await startSession("reorder-user", "reorder-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 140 * 60_000), new Date(now - 140 * 60_000));
    await request("/verified-session/finish", { userId: "reorder-user", deviceSecret: "reorder-secret", sessionId });

    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;

    // A "fresh" request lands first and claims up through real now.
    const fresh = await request("/verified-session/reconcile-offline", {
      userId: "reorder-user",
      deviceSecret: "reorder-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date().toISOString() }],
    });
    const freshBody = await fresh.json<{ creditedMinutes: number }>();
    expect(freshBody.creditedMinutes).toBeGreaterThan(0);
    const afterFresh = await offlineSum("reorder-user");
    const ledgerBefore = await env.DB.prepare("SELECT gap_started_at AS gapStartedAt FROM verified_offline_reconciliations WHERE anchor_session_id = ? ORDER BY created_at DESC LIMIT 1")
      .bind(sessionId).first<{ gapStartedAt: string }>();

    // A stale/delayed request, sent before the fresh one but arriving after, tries to claim the
    // same already-covered window again.
    const stale = await request("/verified-session/reconcile-offline", {
      userId: "reorder-user",
      deviceSecret: "reorder-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: new Date(gapStart + 2 * 60_000).toISOString() }],
    });
    const staleBody = await stale.json<{ creditedMinutes: number }>();
    expect(staleBody.creditedMinutes).toBe(0);
    expect(await offlineSum("reorder-user")).toBe(afterFresh);

    // The floor only ever moves forward — never backward.
    const ledgerAfter = await env.DB.prepare("SELECT gap_started_at AS gapStartedAt FROM verified_offline_reconciliations WHERE anchor_session_id = ? ORDER BY created_at DESC LIMIT 1")
      .bind(sessionId).first<{ gapStartedAt: string }>();
    expect(new Date(ledgerAfter!.gapStartedAt).getTime()).toBeGreaterThanOrEqual(new Date(ledgerBefore!.gapStartedAt).getTime());
  });

  it("13. a malicious far-future claimed timestamp is clamped to real elapsed time, not trusted", async () => {
    await createUser("future-user", "future-secret", "BCDF-2413");
    const sessionId = await startSession("future-user", "future-secret");
    const now = Date.now();
    await backdate(sessionId, new Date(now - 140 * 60_000), new Date(now - 140 * 60_000));
    await request("/verified-session/finish", { userId: "future-user", deviceSecret: "future-secret", sessionId });

    const row = await env.DB.prepare("SELECT started_at AS startedAt, credited_minutes AS creditedMinutes FROM verified_study_sessions WHERE id = ?")
      .bind(sessionId).first<{ startedAt: string; creditedMinutes: number }>();
    const gapStart = new Date(row!.startedAt).getTime() + row!.creditedMinutes * 60_000;

    const farFuture = new Date(now + 10 * 365 * 24 * 60 * 60 * 1000).toISOString();
    const response = await request("/verified-session/reconcile-offline", {
      userId: "future-user",
      deviceSecret: "future-secret",
      anchorSessionId: sessionId,
      intervals: [{ startedAt: new Date(gapStart).toISOString(), endedAt: farFuture }],
    });
    expect(response.status).toBe(200);
    const body = await response.json<{ creditedMinutes: number }>();
    // Real elapsed room here is only ~20 minutes (140 - 120); nowhere close to 10 years.
    expect(body.creditedMinutes).toBeLessThanOrEqual(25);
  });

  it("14. reconciling against a session that doesn't exist claims nothing", async () => {
    await createUser("noanchor-user", "noanchor-secret", "BCDF-2414");
    const response = await request("/verified-session/reconcile-offline", {
      userId: "noanchor-user",
      deviceSecret: "noanchor-secret",
      anchorSessionId: "nonexistent-session-id",
      intervals: [{ startedAt: new Date(Date.now() - 3 * 60 * 60_000).toISOString(), endedAt: new Date().toISOString() }],
    });
    expect(response.status).toBe(404);
    expect(await offlineSum("noanchor-user")).toBe(0);
  });

  it("bonus: the 4-hour absolute session cap still binds even when the heartbeat is fresh", async () => {
    await createUser("maxcap-user", "maxcap-secret", "BCDF-2415");
    const sessionId = await startSession("maxcap-user", "maxcap-secret");
    const now = Date.now();
    // Started 5 hours ago but heartbeats have been healthy/recent throughout — the 2h grace
    // window is nowhere near binding; only the 4h absolute cap should limit credit.
    await backdate(sessionId, new Date(now - 300 * 60_000), new Date(now - 1 * 60_000));

    const finish = await request("/verified-session/finish", { userId: "maxcap-user", deviceSecret: "maxcap-secret", sessionId });
    const body = await finish.json<{ creditedMinutes: number }>();
    expect(body.creditedMinutes).toBeGreaterThanOrEqual(239);
    expect(body.creditedMinutes).toBeLessThanOrEqual(240);
  });
});
