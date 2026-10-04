import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";

async function postJson(path: string, body: Record<string, unknown>) {
  return SELF.fetch(`https://test.local${path}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function createUser(userId: string, deviceSecret: string, friendCode: string) {
  const response = await postJson("/sync", {
    user: {
      userId,
      deviceSecret,
      friendCode,
      displayName: userId,
      lifetimeStudyMinutes: 0,
      lifetimeStudySessions: 0,
    },
    stats: [],
  });
  expect(response.status).toBe(200);
}

function webpFile() {
  return new File([new Uint8Array(2048).fill(0x80)], "drawing.webp", { type: "image/webp" });
}

function addIsoDays(date: string, delta: number) {
  const [year, month, day] = date.split("-").map(Number);
  const value = new Date(Date.UTC(year, month - 1, day));
  value.setUTCDate(value.getUTCDate() + delta);
  return value.toISOString().slice(0, 10);
}

async function submitDrawing(userId: string, deviceSecret: string) {
  const form = new FormData();
  form.set("userId", userId);
  form.set("deviceSecret", deviceSecret);
  form.set("image", webpFile());
  return SELF.fetch("https://test.local/skribbl/submit", { method: "POST", body: form });
}

describe("daily skribbl", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM skribbl_votes"),
      env.DB.prepare("DELETE FROM skribbl_drawings"),
      env.DB.prepare("DELETE FROM skribbl_daily_winners"),
      env.DB.prepare("DELETE FROM skribbl_daily_themes"),
      env.DB.prepare("UPDATE skribbl_themes SET last_used_date = NULL"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("assigns today's theme and reports not submitted", async () => {
    await createUser("skribbl-a", "a-secret", "AAAA-1111");
    const response = await SELF.fetch("https://test.local/skribbl/theme?userId=skribbl-a&deviceSecret=a-secret");
    expect(response.status).toBe(200);
    const payload = await response.json<{ date: string; theme: string; submitted: boolean }>();
    expect(payload.submitted).toBe(false);
    expect(payload.theme.length).toBeGreaterThan(0);
    expect(payload.date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("submits a drawing once and rejects duplicates", async () => {
    await createUser("skribbl-a", "a-secret", "AAAA-1111");

    const first = await submitDrawing("skribbl-a", "a-secret");
    expect(first.status).toBe(200);
    const submitted = await first.json<{ ok: boolean }>();
    expect(submitted.ok).toBe(true);

    const theme = await SELF.fetch("https://test.local/skribbl/theme?userId=skribbl-a&deviceSecret=a-secret")
      .then((response) => response.json<{ submitted: boolean }>());
    expect(theme.submitted).toBe(true);

    const second = await submitDrawing("skribbl-a", "a-secret");
    expect(second.status).toBe(409);
  });

  it("lists the gallery and enforces the no-vote-on-own-drawing rule", async () => {
    await createUser("skribbl-a", "a-secret", "AAAA-1111");
    await createUser("skribbl-b", "b-secret", "BBBB-2222");
    expect((await submitDrawing("skribbl-a", "a-secret")).status).toBe(200);
    expect((await submitDrawing("skribbl-b", "b-secret")).status).toBe(200);

    const gallery = await postJson("/skribbl/gallery", { userId: "skribbl-a", deviceSecret: "a-secret" });
    expect(gallery.status).toBe(200);
    const page = await gallery.json<{ drawings: Array<{ id: string; isSelf: boolean; voteScore: number }> }>();
    expect(page.drawings.length).toBe(2);
    const own = page.drawings.find((drawing) => drawing.isSelf);
    const other = page.drawings.find((drawing) => !drawing.isSelf);
    expect(own).toBeDefined();
    expect(other).toBeDefined();

    const ownVote = await postJson("/skribbl/vote", { userId: "skribbl-a", deviceSecret: "a-secret", drawingId: own!.id, vote: 1 });
    expect(ownVote.status).toBe(403);

    const vote = await postJson("/skribbl/vote", { userId: "skribbl-a", deviceSecret: "a-secret", drawingId: other!.id, vote: 1 });
    expect(vote.status).toBe(200);
    expect((await vote.json<{ score: number }>()).score).toBe(1);

    const flip = await postJson("/skribbl/vote", { userId: "skribbl-a", deviceSecret: "a-secret", drawingId: other!.id, vote: -1 });
    expect(flip.status).toBe(200);
    expect((await flip.json<{ score: number }>()).score).toBe(-1);
  });

  it("reports yesterday's top-voted drawing on the leaderboard", async () => {
    await createUser("skribbl-a", "a-secret", "AAAA-1111");
    await createUser("skribbl-b", "b-secret", "BBBB-2222");

    const today = await SELF.fetch("https://test.local/skribbl/theme?userId=skribbl-a&deviceSecret=a-secret")
      .then((response) => response.json<{ date: string }>());
    const yesterday = addIsoDays(today.date, -1);
    const themeRow = await env.DB.prepare("SELECT id FROM skribbl_themes LIMIT 1").first<{ id: number }>();

    for (const [userId, secret] of [["skribbl-a", "a-secret"], ["skribbl-b", "b-secret"]] as const) {
      await env.DB.prepare(`
        INSERT INTO skribbl_drawings (id, date, user_id, theme_id, r2_object_key, mime_type, size_bytes)
        VALUES (?, ?, ?, ?, ?, ?, ?)
      `).bind(`draw-${userId}`, yesterday, userId, themeRow!.id, `drawings/${yesterday}/${userId}.webp`, "image/webp", 1024).run();
    }
    await env.DB.prepare("INSERT INTO skribbl_votes (drawing_id, user_id, vote_value) VALUES (?, ?, ?)")
      .bind("draw-skribbl-b", "skribbl-a", 1).run();
    await env.DB.prepare("INSERT INTO skribbl_votes (drawing_id, user_id, vote_value) VALUES (?, ?, ?)")
      .bind("draw-skribbl-a", "skribbl-b", 1).run();
    await env.DB.prepare("INSERT INTO skribbl_votes (drawing_id, user_id, vote_value) VALUES (?, ?, ?)")
      .bind("draw-skribbl-a", "skribbl-a", 1).run();

    const response = await SELF.fetch("https://test.local/skribbl/leaderboard?userId=skribbl-a&deviceSecret=a-secret");
    expect(response.status).toBe(200);
    const payload = await response.json<{ winner: { userId: string; score: number } | null }>();
    expect(payload.winner).not.toBeNull();
    expect(payload.winner!.userId).toBe("skribbl-a");
    expect(payload.winner!.score).toBe(2);
  });
});
