import { env, SELF } from "cloudflare:test";
import { beforeEach, describe, expect, it } from "vitest";
import { hashDeviceSecret, verifyDeviceSecret } from "../src/index";

async function sync(userId: string, deviceSecret: string, friendCode: string) {
  return SELF.fetch("https://test.local/sync", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      user: { userId, deviceSecret, friendCode, displayName: userId },
      stats: [],
    }),
  });
}

async function getUser(userId: string) {
  return env.DB.prepare("SELECT device_secret_hash AS deviceSecretHash FROM users WHERE id = ?")
    .bind(userId).first<{ deviceSecretHash: string | null }>();
}

async function seedLegacyPlaintextUser(userId: string, deviceSecret: string, friendCode: string) {
  await env.DB.prepare(`
    INSERT INTO users (id, device_secret, device_secret_hash, friend_code, display_name)
    VALUES (?, ?, NULL, ?, ?)
  `).bind(userId, deviceSecret, friendCode, userId).run();
}

describe("device_secret hashing migration", () => {
  beforeEach(async () => {
    await env.DB.prepare("DELETE FROM users").run();
  });

  it("a brand-new account is hashed immediately at creation", async () => {
    const response = await sync("new-account", "new-account-secret", "BCDF-2345");
    expect(response.status).toBe(200);
    const user = await getUser("new-account");
    expect(user?.deviceSecretHash).not.toBeNull();

    // And a follow-up sync with the same secret must succeed via the hash path.
    const second = await sync("new-account", "new-account-secret", "BCDF-2345");
    expect(second.status).toBe(200);
  });

  it("a legacy plaintext-only row is lazily backfilled on its next authenticated request", async () => {
    await seedLegacyPlaintextUser("legacy-account", "legacy-secret", "BCDG-2346");
    expect((await getUser("legacy-account"))?.deviceSecretHash).toBeNull();

    const response = await sync("legacy-account", "legacy-secret", "BCDG-2346");
    expect(response.status).toBe(200);
    expect((await getUser("legacy-account"))?.deviceSecretHash).not.toBeNull();

    // A follow-up request must now succeed via the hash path with the same real secret.
    const followUp = await sync("legacy-account", "legacy-secret", "BCDG-2346");
    expect(followUp.status).toBe(200);
  });

  it("rejects a wrong secret both before and after the hash backfill", async () => {
    await seedLegacyPlaintextUser("guarded-account", "correct-secret", "BCDH-2347");
    const wrongBeforeBackfill = await sync("guarded-account", "wrong-secret", "BCDH-2347");
    expect(wrongBeforeBackfill.status).toBe(403);

    await sync("guarded-account", "correct-secret", "BCDH-2347");
    expect((await getUser("guarded-account"))?.deviceSecretHash).not.toBeNull();

    const wrongAfterBackfill = await sync("guarded-account", "wrong-secret", "BCDH-2347");
    expect(wrongAfterBackfill.status).toBe(403);
  });

  it("verifyDeviceSecret fails closed for an already-migrated row when the secret is missing, but a still-plaintext row keeps working", async () => {
    const envWithoutSecret = { ...env, DEVICE_SECRET_HASH_SECRET: undefined } as typeof env;

    const migratedHash = await hashDeviceSecret(env, "some-real-secret");
    expect(migratedHash).not.toBeNull();
    const migratedUser = { device_secret: "some-real-secret", device_secret_hash: migratedHash };
    await expect(verifyDeviceSecret(envWithoutSecret, "migrated-user", migratedUser, "some-real-secret")).rejects.toBeInstanceOf(Response);

    const legacyUser = { device_secret: "another-real-secret", device_secret_hash: null };
    await expect(verifyDeviceSecret(envWithoutSecret, "legacy-user", legacyUser, "another-real-secret")).resolves.toBeUndefined();
  });
});
