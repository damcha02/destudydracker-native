import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { parseListAvatar } from "../src/index";

// A real, tiny (68-byte) 1x1 transparent PNG — small enough to stay well under
// MAX_PROFILE_AVATAR_BYTES, but a genuine data:image/ URL so profileAvatarBytesFromDataUrl
// actually decodes it.
const TINY_PNG_DATA_URL =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";

function photoAvatar(url: string) {
  return { kind: "photo", name: "avatar.png", url, mimeType: "image/png" };
}

async function sync(userId: string, deviceSecret: string, friendCode: string, avatar: unknown) {
  return SELF.fetch("https://test.local/sync", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      user: { userId, deviceSecret, friendCode, displayName: userId, avatar },
      stats: [],
    }),
  });
}

async function getUser(userId: string) {
  return env.DB.prepare("SELECT avatar_json AS avatarJson, avatar_size_bytes AS avatarSizeBytes FROM users WHERE id = ?")
    .bind(userId)
    .first<{ avatarJson: string; avatarSizeBytes: number }>();
}

describe("profile avatars auto-upload to R2 during sync", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM users"),
      env.DB.prepare("DELETE FROM r2_usage_monthly"),
    ]);
  });

  it("a data: URI avatar is uploaded to R2 and replaced with an https:// URL", async () => {
    const response = await sync("photo-user", "photo-secret", "BCDF-2345", photoAvatar(TINY_PNG_DATA_URL));
    expect(response.status).toBe(200);

    const user = await getUser("photo-user");
    const avatar = JSON.parse(user!.avatarJson);
    expect(avatar.kind).toBe("photo");
    expect(avatar.url).toMatch(/^https:\/\/.+\/profile\/avatar\//);
    expect(user!.avatarSizeBytes).toBeGreaterThan(0);

    const key = decodeURIComponent(new URL(avatar.url).pathname.replace("/profile/avatar/", ""));
    expect(await env.FEED_IMAGES.get(key)).not.toBeNull();

    // This is the actual regression: a friend/leaderboard viewer must now see the photo,
    // not a letter fallback — parseListAvatar only downgrades raw data: URI avatars.
    expect(parseListAvatar(user!.avatarJson, "photo-user").kind).toBe("photo");
  });

  it("re-syncing a new photo deletes the previously uploaded R2 object", async () => {
    await sync("replace-user", "replace-secret", "BCDG-2346", photoAvatar(TINY_PNG_DATA_URL));
    const first = await getUser("replace-user");
    const firstAvatar = JSON.parse(first!.avatarJson);
    const firstKey = decodeURIComponent(new URL(firstAvatar.url).pathname.replace("/profile/avatar/", ""));
    expect(await env.FEED_IMAGES.get(firstKey)).not.toBeNull();

    await sync("replace-user", "replace-secret", "BCDG-2346", photoAvatar(TINY_PNG_DATA_URL));
    const second = await getUser("replace-user");
    const secondAvatar = JSON.parse(second!.avatarJson);
    expect(secondAvatar.url).not.toBe(firstAvatar.url);
    expect(await env.FEED_IMAGES.get(firstKey)).toBeNull();
  });

  it("falls back to a letter avatar (never persists raw base64) when the R2 class-A budget is exhausted", async () => {
    const month = new Date().toISOString().slice(0, 7);
    await env.DB.prepare(`
      INSERT INTO r2_usage_monthly (month, class_a_ops, class_b_ops, updated_at)
      VALUES (?, 50000, 0, CURRENT_TIMESTAMP)
    `).bind(month).run();

    const response = await sync("budget-user", "budget-secret", "BCDH-2347", photoAvatar(TINY_PNG_DATA_URL));
    expect(response.status).toBe(200); // sync itself must not hard-fail over an avatar issue

    const user = await getUser("budget-user");
    const avatar = JSON.parse(user!.avatarJson);
    expect(avatar.kind).toBe("letter");
    expect(user!.avatarJson).not.toContain("data:image/");
    expect(user!.avatarSizeBytes).toBe(0);
  });

  it("an already-uploaded https:// photo avatar passes through unchanged", async () => {
    await sync("noop-user", "noop-secret", "BCDJ-2348", photoAvatar(TINY_PNG_DATA_URL));
    const first = await getUser("noop-user");
    const firstAvatar = JSON.parse(first!.avatarJson);

    // Re-sync with the exact avatar the server just returned (mirrors what the desktop
    // client's local state would echo back on the next regular sync).
    await sync("noop-user", "noop-secret", "BCDJ-2348", firstAvatar);
    const second = await getUser("noop-user");
    expect(JSON.parse(second!.avatarJson).url).toBe(firstAvatar.url);
    expect(second!.avatarSizeBytes).toBe(first!.avatarSizeBytes);
  });

  it("a non-photo avatar (icon/letter) is stored as-is with zero avatar_size_bytes", async () => {
    const response = await sync("icon-user", "icon-secret", "BCDK-2349", { kind: "letter", letter: "Q", style: "classic" });
    expect(response.status).toBe(200);
    const user = await getUser("icon-user");
    expect(JSON.parse(user!.avatarJson)).toEqual({ kind: "letter", letter: "Q", style: "classic" });
    expect(user!.avatarSizeBytes).toBe(0);
  });
});
