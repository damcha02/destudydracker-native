import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";

function secretFor(userId: string) {
  return `${userId}-secret`;
}

async function createUser(userId: string, friendCode: string, isPrivate: boolean) {
  const response = await SELF.fetch("https://test.local/sync", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      user: { userId, deviceSecret: secretFor(userId), friendCode, displayName: userId, isPrivate },
      stats: [],
    }),
  });
  expect(response.status).toBe(200);
}

async function makeFriends(userA: string, userB: string) {
  const [low, high] = userA < userB ? [userA, userB] : [userB, userA];
  await env.DB.prepare("INSERT OR IGNORE INTO friendships (user_low, user_high) VALUES (?, ?)").bind(low, high).run();
}

async function playerStats(userId: string, targetUserId: string) {
  return SELF.fetch("https://test.local/player-stats", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ userId, deviceSecret: secretFor(userId), targetUserId }),
  });
}

describe("/player-stats friendship gating", () => {
  beforeEach(async () => {
    await env.DB.batch([
      env.DB.prepare("DELETE FROM friendships"),
      env.DB.prepare("DELETE FROM daily_stats"),
      env.DB.prepare("DELETE FROM users"),
    ]);
  });

  it("self-access succeeds regardless of is_private", async () => {
    await createUser("self-user", "AAAA-1111", true);
    const response = await playerStats("self-user", "self-user");
    expect(response.status).toBe(200);
  });

  it("an accepted friend can view a private target", async () => {
    await createUser("friend-a", "AAAA-2222", true);
    await createUser("friend-b", "AAAA-3333", false);
    await makeFriends("friend-a", "friend-b");
    const response = await playerStats("friend-b", "friend-a");
    expect(response.status).toBe(200);
  });

  it("a non-friend is rejected from a private target", async () => {
    await createUser("private-owner", "AAAA-4444", true);
    await createUser("stranger", "AAAA-5555", false);
    const response = await playerStats("stranger", "private-owner");
    expect(response.status).toBe(403);
  });

  it("a non-friend is rejected from a PUBLIC target too — regression test for the friendship-bypass removal", async () => {
    await createUser("public-owner", "AAAA-6666", false);
    await createUser("stranger-2", "AAAA-7777", false);
    const response = await playerStats("stranger-2", "public-owner");
    expect(response.status).toBe(403);
  });

  it("a nonexistent target returns 404", async () => {
    await createUser("lonely-user", "AAAA-8888", false);
    const response = await playerStats("lonely-user", "does-not-exist");
    expect(response.status).toBe(404);
  });
});
