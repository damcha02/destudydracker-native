// Stage 22a golden-fixture generator. Runs the PRODUCTION Worker (cloudflare/src/index.ts, a
// scratch copy) inside local workerd with a local D1 - no network, no Cloudflare account - and
// prints every response the native client must understand as `GOLDEN <json>` lines. Synthetic
// users only. Request credentials are never printed.
import { env, SELF } from "cloudflare:test";
import { describe, expect, it } from "vitest";

const ORIGIN = "https://test.local";

async function call(name: string, method: string, path: string, body?: Record<string, unknown> | FormData) {
  const init: RequestInit = { method };
  if (body instanceof FormData) init.body = body;
  else if (body) {
    init.body = JSON.stringify(body);
    init.headers = { "content-type": "application/json" };
  }
  const response = await SELF.fetch(`${ORIGIN}${path}`, init);
  const contentType = response.headers.get("content-type");
  const bytes = new Uint8Array(await response.arrayBuffer());
  const isText = !contentType || contentType.startsWith("application/json") || contentType.startsWith("text/");
  const record = {
    name,
    method,
    // the path only: the two Skribbl GETs carry credentials in the query
    path: path.split("?")[0],
    status: response.status,
    contentType,
    body: isText ? new TextDecoder().decode(bytes) : null,
    bodyBase64: isText ? null : btoa(String.fromCharCode(...bytes)),
  };
  console.log(`GOLDEN ${JSON.stringify(record)}`);
  return { status: response.status, json: isText && contentType?.startsWith("application/json") ? JSON.parse(record.body!) : null };
}

const users = {
  ada: { userId: "golden-ada", deviceSecret: "secret-ada", friendCode: "ADAA-2345", displayName: "Ada Lovelace", avatar: { kind: "letter", letter: "a", style: "pixel" } },
  bob: { userId: "golden-bob", deviceSecret: "secret-bob", friendCode: "BOBB-2345", displayName: "Bob", avatar: { kind: "icon", icon: "🦊" } },
  cho: { userId: "golden-cho", deviceSecret: "secret-cho", friendCode: "CHOO-2345", displayName: "張偉 — Zoë 🦊", avatar: { kind: "photo", name: "me.webp", url: `${ORIGIN}/profile/avatar/avatars%2Fgolden-cho%2F1.webp`, mimeType: "image/webp" } },
  dan: { userId: "golden-dan", deviceSecret: "secret-dan", friendCode: "DANN-2345", displayName: "Dan Private", isPrivate: true },
  eve: { userId: "golden-eve", deviceSecret: "secret-eve", friendCode: "EVEE-2345", displayName: "Eve", showHoursToFriends: false },
  rtl: { userId: "golden-rtl", deviceSecret: "secret-rtl", friendCode: "RTLL-2345", displayName: "שלום עולם" },
  long: { userId: "golden-long", deviceSecret: "secret-long", friendCode: "LONG-2345", displayName: "A very long display name that goes on and on and on forever" },
};

type User = (typeof users)[keyof typeof users];
const auth = (u: User) => ({ userId: u.userId, deviceSecret: u.deviceSecret });

function syncBody(u: User & Record<string, unknown>, stats: Array<{ date: string; minutes: number; sessions: number }> = []) {
  return {
    user: {
      ...u,
      lifetimeStudyMinutes: stats.reduce((s, r) => s + r.minutes, 0),
      lifetimeStudySessions: stats.reduce((s, r) => s + r.sessions, 0),
      device: { fingerprintHash: "0123456789abcdef", label: "linux x86_64 development" },
      app: { version: "0.1.0", platform: "Linux x86_64", runtimeChannel: "development" },
    },
    stats,
    feedPosts: [],
  };
}

function serverToday() {
  const parts = new Intl.DateTimeFormat("en", { timeZone: "Europe/Zurich", year: "numeric", month: "2-digit", day: "2-digit" }).formatToParts(new Date());
  const v = (t: string) => parts.find((p) => p.type === t)?.value ?? "";
  return `${v("year")}-${v("month")}-${v("day")}`;
}

function addIsoDays(date: string, delta: number) {
  const [year, month, day] = date.split("-").map(Number);
  const value = new Date(Date.UTC(year, month - 1, day));
  value.setUTCDate(value.getUTCDate() + delta);
  return value.toISOString().slice(0, 10);
}

// a real 2x2 PNG (white)
const PNG_2X2 = Uint8Array.from(atob("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAYAAABytg0kAAAADklEQVR4nGP4DwUMMAYAj4IP8TylVlEAAAAASUVORK5CYII="), (c) => c.charCodeAt(0));

function drawingForm(u: User, date: string, file: File) {
  const form = new FormData();
  form.set("userId", u.userId);
  form.set("deviceSecret", u.deviceSecret);
  form.set("date", date);
  form.set("image", file);
  return form;
}

describe("golden", () => {
  it("records every 22a operation", async () => {
    const today = serverToday();
    // --- accounts (/sync/v2 creates them) ---
    await call("sync-create-ada", "POST", "/sync/v2", syncBody(users.ada, [{ date: today, minutes: 50, sessions: 2 }]));
    for (const u of [users.bob, users.cho, users.dan, users.eve, users.rtl, users.long]) await call(`sync-create-${u.userId}`, "POST", "/sync/v2", syncBody(u));
    await call("sync-wrong-secret", "POST", "/sync/v2", syncBody({ ...users.ada, deviceSecret: "not-the-secret" }));
    await call("sync-missing-user", "POST", "/sync/v2", { stats: [] });
    await call("sync-duplicate-code", "POST", "/sync/v2", syncBody({ userId: "golden-dup", deviceSecret: "secret-dup", friendCode: "ADAA-2345", displayName: "Dup" }));

    // --- presence ---
    await call("presence-ok", "POST", "/presence", { ...auth(users.ada), app: { version: "0.1.0", platform: "Linux x86_64", runtimeChannel: "development" } });
    await call("presence-unknown-user", "POST", "/presence", { userId: "nobody", deviceSecret: "x" });

    // --- friends ---
    await call("friends-status-empty", "POST", "/friends/status/v2", auth(users.ada));
    await call("friend-request-to-bob", "POST", "/friends/request", { ...auth(users.ada), friendCode: "BOBB-2345" });
    const bobView = await call("friends-status-bob-incoming", "POST", "/friends/status/v2", auth(users.bob));
    const requestId = bobView.json.social.incomingFriendRequests[0].id;
    await call("friend-respond-accept", "POST", "/friends/respond", { ...auth(users.bob), requestId, response: "accepted" });
    await call("friend-respond-again", "POST", "/friends/respond", { ...auth(users.bob), requestId, response: "accepted" });
    await call("friend-request-unknown-code", "POST", "/friends/request", { ...auth(users.ada), friendCode: "ZZZZ-9999" });
    await call("friend-request-self", "POST", "/friends/request", { ...auth(users.ada), friendCode: "ADAA-2345" });
    await call("friend-request-already-friends", "POST", "/friends/request", { ...auth(users.ada), friendCode: "BOBB-2345" });
    await call("friend-request-to-cho", "POST", "/friends/request", { ...auth(users.ada), friendCode: "chOO-2345 " });
    await call("friend-request-to-cho-again", "POST", "/friends/request", { ...auth(users.ada), friendCode: "CHOO-2345" });
    await call("friend-request-eve-to-ada", "POST", "/friends/request", { ...auth(users.eve), friendCode: "ADAA-2345" });
    await call("friends-status-ada-mixed", "POST", "/friends/status/v2", auth(users.ada));
    const adaIncoming = (await call("friends-status-ada-before-decline", "POST", "/friends/status/v2", auth(users.ada))).json.social.incomingFriendRequests[0].id;
    await call("friend-respond-decline", "POST", "/friends/respond", { ...auth(users.ada), requestId: adaIncoming, response: "declined" });
    // the reciprocal case: cho sends to ada while ada->cho is pending => instant friendship
    await call("friend-request-reciprocal", "POST", "/friends/request", { ...auth(users.cho), friendCode: "ADAA-2345" });
    // eve and ada become friends (eve hides hours)
    await call("friend-request-eve-again", "POST", "/friends/request", { ...auth(users.eve), friendCode: "ADAA-2345" });
    const eveReq = (await call("friends-status-ada-eve-pending", "POST", "/friends/status/v2", auth(users.ada))).json.social.incomingFriendRequests[0].id;
    await call("friend-respond-accept-eve", "POST", "/friends/respond", { ...auth(users.ada), requestId: eveReq, response: "accepted" });
    for (const u of [users.rtl, users.long]) {
      await call(`friend-request-${u.userId}`, "POST", "/friends/request", { ...auth(u), friendCode: "ADAA-2345" });
    }
    await call("friends-status-ada-final", "POST", "/friends/status/v2", auth(users.ada));
    await call("friends-status-wrong-secret", "POST", "/friends/status/v2", { userId: users.ada.userId, deviceSecret: "nope" });

    // --- leaderboard data: verified minutes (what counts since migration 0019) ---
    const verified: Array<[User, string, number, number]> = [
      [users.ada, today, 50, 2],
      [users.bob, today, 120, 3],
      [users.cho, today, 50, 1], // tie with ada
      [users.eve, today, 80, 1],
      [users.dan, today, 300, 5], // private: hidden from global
      [users.bob, addIsoDays(today, -1), 60, 1],
    ];
    for (const [u, date, minutes, sessions] of verified) {
      await env.DB.prepare("INSERT INTO verified_daily_stats (user_id, date, minutes, sessions) VALUES (?, ?, ?, ?) ON CONFLICT(user_id, date) DO UPDATE SET minutes = excluded.minutes, sessions = excluded.sessions").bind(u.userId, date, minutes, sessions).run();
    }
    await env.DB.prepare("UPDATE leaderboard_baselines SET minutes = 1000, sessions = 20 WHERE user_id = ?").bind(users.long.userId).run();
    await env.DB.prepare("INSERT OR IGNORE INTO leaderboard_baselines (user_id, minutes, sessions) VALUES (?, 1000, 20)").bind(users.long.userId).run();
    for (const scope of ["friends", "global", "squad"]) {
      for (const period of ["daily", "weekly", "overall"]) {
        await call(`leaderboard-${scope}-${period}`, "POST", "/leaderboard", { ...auth(users.ada), scope, period });
      }
    }
    await call("leaderboard-unknown-user", "POST", "/leaderboard", { userId: "nobody", deviceSecret: "x", scope: "global", period: "daily" });

    // --- player stats ---
    await call("player-stats-friend", "POST", "/player-stats", { ...auth(users.ada), targetUserId: users.bob.userId });
    await call("player-stats-hidden-hours", "POST", "/player-stats", { ...auth(users.ada), targetUserId: users.eve.userId });
    await call("player-stats-self", "POST", "/player-stats", { ...auth(users.ada), targetUserId: users.ada.userId });
    await call("player-stats-not-friend", "POST", "/player-stats", { ...auth(users.ada), targetUserId: users.dan.userId });
    await call("player-stats-missing", "POST", "/player-stats", { ...auth(users.ada), targetUserId: "ghost" });

    // --- Daily Skribbl ---
    await call("skribbl-leaderboard-no-winner", "GET", `/skribbl/leaderboard?userId=${users.ada.userId}&deviceSecret=${users.ada.deviceSecret}`);
    await call("skribbl-theme-fresh", "GET", `/skribbl/theme?userId=${users.ada.userId}&deviceSecret=${users.ada.deviceSecret}`);
    await call("skribbl-theme-unknown-user", "GET", `/skribbl/theme?userId=nobody&deviceSecret=x`);
    await call("skribbl-gallery-empty", "POST", "/skribbl/gallery", { ...auth(users.ada), date: today, offset: 0, limit: 16 });
    await call("skribbl-submit-wrong-date", "POST", "/skribbl/submit", drawingForm(users.ada, addIsoDays(today, -1), new File([PNG_2X2], "drawing.png", { type: "image/png" })));
    await call("skribbl-submit-gif", "POST", "/skribbl/submit", drawingForm(users.ada, today, new File([PNG_2X2], "drawing.gif", { type: "image/gif" })));
    await call("skribbl-submit-too-large", "POST", "/skribbl/submit", drawingForm(users.ada, today, new File([new Uint8Array(1.5 * 1024 * 1024 + 1)], "drawing.png", { type: "image/png" })));
    const submitted = await call("skribbl-submit-ok", "POST", "/skribbl/submit", drawingForm(users.ada, today, new File([PNG_2X2], "drawing.png", { type: "image/png" })));
    await call("skribbl-submit-duplicate", "POST", "/skribbl/submit", drawingForm(users.ada, today, new File([PNG_2X2], "drawing.png", { type: "image/png" })));
    await call("skribbl-theme-submitted", "GET", `/skribbl/theme?userId=${users.ada.userId}&deviceSecret=${users.ada.deviceSecret}`);
    const key = new URL(submitted.json.imageUrl).pathname;
    await call("skribbl-drawing-png", "GET", key);
    await call("skribbl-drawing-missing", "GET", "/skribbl/drawing/drawings%2Fnope.png");
    // 19 more artists -> 20 drawings, two gallery pages
    for (let i = 0; i < 19; i += 1) {
      const u = { userId: `golden-artist-${i}`, deviceSecret: `secret-artist-${i}`, friendCode: `ART${String.fromCharCode(65 + i)}-2345`, displayName: `Artist ${i}` } as User;
      await SELF.fetch(`${ORIGIN}/sync/v2`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(syncBody(u)) });
      await SELF.fetch(`${ORIGIN}/skribbl/submit`, { method: "POST", body: drawingForm(u, today, new File([PNG_2X2], "drawing.png", { type: "image/png" })) });
    }
    const page1 = await call("skribbl-gallery-page1", "POST", "/skribbl/gallery", { ...auth(users.ada), date: today, offset: 0, limit: 16 });
    await call("skribbl-gallery-page2", "POST", "/skribbl/gallery", { ...auth(users.ada), date: today, offset: page1.json.nextOffset, limit: 16 });
    const other = page1.json.drawings.find((d: { isSelf: boolean }) => !d.isSelf).id;
    const own = page1.json.drawings.find((d: { isSelf: boolean }) => d.isSelf).id;
    await call("skribbl-vote-up", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: other, vote: 1 });
    await call("skribbl-vote-down", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: other, vote: -1 });
    await call("skribbl-vote-down-bob", "POST", "/skribbl/vote", { ...auth(users.bob), drawingId: other, vote: -1 });
    await call("skribbl-vote-clear", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: other, vote: 0 });
    await call("skribbl-vote-own", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: own, vote: 1 });
    await call("skribbl-vote-invalid", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: other, vote: 2 });
    await call("skribbl-vote-missing", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: "nope", vote: 1 });
    await call("skribbl-vote-up-again", "POST", "/skribbl/vote", { ...auth(users.ada), drawingId: other, vote: 1 });
    await call("skribbl-gallery-after-votes", "POST", "/skribbl/gallery", { ...auth(users.ada), date: today, offset: 0, limit: 16 });
    // yesterday's drawings -> a winner (the cron snapshot has not run: the live query answers)
    const yesterday = addIsoDays(today, -1);
    await env.DB.prepare("INSERT OR IGNORE INTO skribbl_themes (id, theme) VALUES (999, 'Golden yesterday')").run();
    await env.DB.prepare("INSERT INTO skribbl_drawings (id, date, user_id, theme_id, r2_object_key, mime_type, size_bytes) VALUES ('y1', ?, ?, 999, 'drawings/y/bob.png', 'image/png', 10), ('y2', ?, ?, 999, 'drawings/y/cho.png', 'image/png', 10)").bind(yesterday, users.bob.userId, yesterday, users.cho.userId).run();
    await env.DB.prepare("INSERT INTO skribbl_votes (drawing_id, user_id, vote_value) VALUES ('y2', ?, 1), ('y2', ?, 1), ('y1', ?, -1)").bind(users.ada.userId, users.eve.userId, users.ada.userId).run();
    await call("skribbl-leaderboard-winner", "GET", `/skribbl/leaderboard?userId=${users.ada.userId}&deviceSecret=${users.ada.deviceSecret}`);
    expect(true).toBe(true);
  });
});
