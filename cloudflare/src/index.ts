export interface Env {
  DB: D1Database;
  FEED_IMAGES: R2Bucket;
  RATE_LIMIT_KV: KVNamespace;
  IP_HASH_SECRET?: string;
  DEVICE_SECRET_HASH_SECRET?: string;
  R2_ALERT_WEBHOOK_URL?: string;
}

type LeaderboardPeriod = "daily" | "weekly" | "overall";
type LeaderboardScope = "global" | "friends" | "squad";
type SquadRole = "leader" | "co_leader" | "elder" | "member";
type SquadScorePeriod = "daily" | "season" | "overall";
type SocialAvatarStyle = "classic" | "serif" | "cursive" | "graffiti" | "pixel" | "mono";
type SocialAvatar =
  | { kind: "letter"; letter: string; style: SocialAvatarStyle }
  | { kind: "icon"; icon: string }
  | { kind: "photo"; name: string; url: string; mimeType: string };

interface SyncPayload {
  user: {
    userId: string;
    deviceSecret: string;
    friendCode: string;
    displayName: string;
    avatar?: unknown;
    isPrivate?: boolean;
    showHoursToFriends?: boolean;
    lifetimeStudyMinutes?: number;
    lifetimeStudySessions?: number;
    device?: {
      fingerprintHash?: string;
      label?: string;
    };
    app?: AppMetadata;
    /** Honeypot trap field. The real client never sends this — any non-empty value here is a strong abuse signal. */
    referralCode?: unknown;
  };
  stats: Array<{
    date: string;
    minutes: number;
    sessions: number;
  }>;
  feedPosts?: Array<{
    id: string;
    type: "session" | "milestone";
    subject?: string;
    detail?: string;
    note?: string;
    icon?: string;
    minutes?: number;
    presetLabel?: string;
    createdAt?: string;
    poll?: {
      question?: string;
      multiple?: boolean;
      options?: Array<{ id?: string; text?: string }>;
    } | null;
  }>;
}

interface AppMetadata {
  version?: string;
  platform?: string;
  runtimeChannel?: string;
}

type SyncFeedPoll = NonNullable<NonNullable<SyncPayload["feedPosts"]>[number]["poll"]> | null | undefined;

const corsHeaders = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "GET,POST,OPTIONS",
  "access-control-allow-headers": "content-type",
};

const MAX_JSON_BODY_BYTES = 256 * 1024;
const MAX_SYNC_STAT_ROWS = 370;
const MAX_DAILY_MINUTES = 24 * 60;
const MAX_DAILY_SESSIONS = 200;
const MAX_ID_LENGTH = 80;
const MAX_SECRET_LENGTH = 120;
const MAX_COMMENT_LENGTH = 220;
const MAX_POLL_QUESTION_LENGTH = 180;
const MAX_POLL_OPTION_LENGTH = 100;
const MAX_POLL_OPTIONS = 12;
const MAX_SQUAD_NAME_LENGTH = 48;
const MAX_SQUAD_MEMBERS = 4;
const MAX_SQUAD_MESSAGE_LENGTH = 500;
const SQUAD_SEASON_START = "2026-07-29";
const SQUAD_SEASON_END = "2026-08-31";
const SQUAD_NEXT_SEASON_START = "2026-09-01";
const SQUAD_NEXT_SEASON_END = "2027-02-28";
const SQUAD_ZERO_POINT_DATES = new Set(["2026-08-05"]);
const SERVER_TIME_ZONE = "Europe/Zurich";
const SQUAD_SCORE_BACKFILL_DAYS = 14;
const MAX_FEED_IMAGE_BYTES = 5 * 1024 * 1024;
const MAX_PROFILE_AVATAR_BYTES = 256 * 1024;
const FEED_IMAGE_TTL_MS = 5 * 24 * 60 * 60 * 1000;
const R2_STORAGE_WARNING_BYTES = 400 * 1024 * 1024;
const R2_STORAGE_HARD_BYTES = 500 * 1024 * 1024;
const R2_CLASS_A_WARNING_MONTHLY = 40_000;
const R2_CLASS_A_HARD_MONTHLY = 50_000;
const R2_CLASS_B_WARNING_MONTHLY = 200_000;
const R2_CLASS_B_HARD_MONTHLY = 250_000;
const R2_OWNER_FRIEND_CODE = "ZRWL-WKNF";

// --- Abuse detection tuning (see PRIVACY.md for what this telemetry is and why) ---
const NEW_ACCOUNT_RATE_LIMIT_WINDOW_SECONDS = 3600;
const NEW_ACCOUNT_RATE_LIMIT_MAX_PER_IP = 5;
const NEW_ACCOUNT_RATE_LIMIT_FLAG_THRESHOLD = 3;
const ABUSE_DATA_RETENTION_DAYS = 90;
const HONEYPOT_PATHS = new Set(["/admin", "/.env", "/wp-login.php", "/api/debug", "/.git/config"]);
const FRIEND_CODE_ALPHABET = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const FRIEND_CODE_PATTERN = new RegExp(`^[${FRIEND_CODE_ALPHABET}]{4}-[${FRIEND_CODE_ALPHABET}]{4}$`);
const MAX_INITIAL_LIFETIME_MINUTES_FLAG = 20_000;
const MAX_INITIAL_LIFETIME_SESSIONS_FLAG = 2_000;
const MAX_PLAUSIBLE_STUDY_MINUTES_PER_DAY = 20 * 60;
const STANDALONE_FLAG_REASONS = new Set(["honeypot_field", "bulk_backfill"]);
const MAX_USER_AGENT_LENGTH = 300;
const MAX_LOG_PATH_LENGTH = 200;
const MAX_LOG_DETAIL_LENGTH = 300;
const VERIFIED_SESSION_HEARTBEAT_MS = 15 * 60 * 1000;
const VERIFIED_SESSION_GRACE_MS = 5 * 60 * 1000;
const VERIFIED_SESSION_MAX_MS = 4 * 60 * 60 * 1000;
// Hard ceiling per offline-credit reconciliation call, independent of the plausibility-rate cap
// below — bounds worst-case payload/claim size (e.g. a multi-day trip) rather than daily rate.
const OFFLINE_RECONCILE_MAX_CLAIM_MS = 24 * 60 * 60 * 1000;
const MAX_OFFLINE_INTERVALS = 500;

const avatarStyles = new Set<SocialAvatarStyle>(["classic", "serif", "cursive", "graffiti", "pixel", "mono"]);
const avatarIcons = new Set(["✦", "★", "◆", "☘", "☾", "☀", "♜", "♞", "⚡", "☕", "📚", "🧠", "🔥", "🌊", "🌿", "🪐", "🚀", "🎯", "🏆", "🛡", "🦉", "🐢", "🐺", "🐱", "🍄", "🌙", "🌸", "🍀", "💎", "🎲", "🎧", "📝", "🔮", "🧩", "🕹", "📖", "🧪", "🛰", "🌌", "🦊"]);
const feedImageTypes = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);

function json(data: unknown, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      ...corsHeaders,
      "content-type": "application/json; charset=utf-8",
    },
  });
}

function text(message: string, status = 400) {
  return new Response(message, { status, headers: corsHeaders });
}

async function readJson<T>(request: Request): Promise<T> {
  const contentLength = Number(request.headers.get("content-length") ?? 0);
  if (contentLength > MAX_JSON_BODY_BYTES) {
    throw new Response("Request body is too large.", { status: 413, headers: corsHeaders });
  }
  return request.json() as Promise<T>;
}

async function readParamsOrJson<T extends Record<string, unknown>>(request: Request): Promise<T> {
  if (request.method === "GET") {
    return Object.fromEntries(new URL(request.url).searchParams.entries()) as T;
  }
  return readJson<T>(request);
}

function requiredText(value: unknown, label: string, maxLength: number) {
  const text = String(value ?? "").trim();
  if (!text) throw new Response(`Missing ${label}.`, { status: 400, headers: corsHeaders });
  if (text.length > maxLength) throw new Response(`${label} is too long.`, { status: 400, headers: corsHeaders });
  return text;
}

function cleanUserId(value: unknown) {
  return requiredText(value, "userId", MAX_ID_LENGTH);
}

function cleanDeviceSecret(value: unknown) {
  return requiredText(value, "deviceSecret", MAX_SECRET_LENGTH);
}

function cleanCode(value: unknown) {
  return String(value ?? "").trim().toUpperCase();
}

function cleanName(value: unknown) {
  const name = String(value ?? "").trim().slice(0, 48);
  return name || "Student";
}

function firstAvatarLetter(name: string) {
  return (name.trim()[0] || "S").toUpperCase();
}

function cleanAvatar(value: unknown, displayName: string): SocialAvatar {
  if (!value || typeof value !== "object") return { kind: "letter", letter: firstAvatarLetter(displayName), style: "classic" };
  const record = value as Record<string, unknown>;
  if (record.kind === "icon" && typeof record.icon === "string" && avatarIcons.has(record.icon)) {
    return { kind: "icon", icon: record.icon };
  }
  if (record.kind === "letter") {
    const letter = typeof record.letter === "string" && /^[A-Z]$/i.test(record.letter) ? record.letter.toUpperCase() : firstAvatarLetter(displayName);
    const style = typeof record.style === "string" && avatarStyles.has(record.style as SocialAvatarStyle) ? record.style as SocialAvatarStyle : "classic";
    return { kind: "letter", letter, style };
  }
  if (record.kind === "photo") {
    const rawUrl = typeof record.url === "string" ? record.url : "";
    const url = rawUrl.startsWith("data:image/") || /^https:\/\/[^/]+\/profile\/avatar\//.test(rawUrl) ? rawUrl : "";
    if (url) {
      return {
        kind: "photo",
        name: typeof record.name === "string" ? record.name.slice(0, 180) : "photo",
        url,
        mimeType: typeof record.mimeType === "string" && record.mimeType.startsWith("image/") ? record.mimeType : "image/webp",
      };
    }
  }
  return { kind: "letter", letter: firstAvatarLetter(displayName), style: "classic" };
}

function parseAvatar(value: unknown, displayName: string): SocialAvatar {
  if (typeof value !== "string") return cleanAvatar(null, displayName);
  try {
    return cleanAvatar(JSON.parse(value), displayName);
  } catch {
    return cleanAvatar(null, displayName);
  }
}

export function parseListAvatar(value: unknown, displayName: string): SocialAvatar {
  const avatar = parseAvatar(value, displayName);
  if (avatar.kind !== "photo") return avatar;
  if (!avatar.url.startsWith("data:image/")) return avatar;
  return { kind: "letter", letter: firstAvatarLetter(displayName), style: "classic" };
}

// Called on every sync so photo avatars never sit as raw base64 in avatar_json — a data: URI
// avatar (client couldn't/didn't upload it, e.g. socialConfigured was false at save time) is
// uploaded to R2 here instead, since parseListAvatar hides data: URIs from everyone but the
// owner and nothing would otherwise ever retry the upload. Best-effort: on any upload failure
// (decode error, R2 budget exceeded) falls back to a letter avatar rather than persisting the
// base64 blob into D1, since sync must not hard-fail over an avatar issue.
async function resolveAvatarForSync(
  env: Env,
  origin: string,
  userId: string,
  rawAvatar: unknown,
  displayName: string,
  existingAvatarJson: string | null | undefined,
  existingAvatarSizeBytes: number | null | undefined,
): Promise<{ avatarJson: string; avatarSizeBytes: number }> {
  const avatar = cleanAvatar(rawAvatar, displayName);
  if (avatar.kind !== "photo" || !avatar.url.startsWith("data:image/")) {
    return { avatarJson: JSON.stringify(avatar), avatarSizeBytes: avatar.kind === "photo" ? Number(existingAvatarSizeBytes ?? 0) : 0 };
  }
  const image = profileAvatarBytesFromDataUrl(avatar.url);
  const previousAvatar = parseAvatar(existingAvatarJson, displayName);
  const uploaded = image
    ? await uploadAvatarBytesToR2(env, origin, userId, image, avatar.name, previousAvatar, Number(existingAvatarSizeBytes ?? 0))
    : null;
  if (uploaded) return { avatarJson: JSON.stringify(uploaded), avatarSizeBytes: image!.bytes.byteLength };
  const fallback: SocialAvatar = { kind: "letter", letter: firstAvatarLetter(displayName), style: "classic" };
  return { avatarJson: JSON.stringify(fallback), avatarSizeBytes: 0 };
}

function serverDateIso(date = new Date()) {
  const parts = new Intl.DateTimeFormat("en", {
    timeZone: SERVER_TIME_ZONE,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).formatToParts(date);
  const value = (type: string) => parts.find((part) => part.type === type)?.value ?? "";
  return `${value("year")}-${value("month")}-${value("day")}`;
}

function addIsoDays(date: string, days: number) {
  const [year, month, day] = date.split("-").map(Number);
  const next = new Date(Date.UTC(year, month - 1, day + days));
  return next.toISOString().slice(0, 10);
}

function serverTimeZoneOffsetMinutes(date: Date) {
  const parts = new Intl.DateTimeFormat("en", {
    timeZone: SERVER_TIME_ZONE,
    hourCycle: "h23",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).formatToParts(date);
  const value = (type: string) => Number(parts.find((part) => part.type === type)?.value ?? 0);
  const zonedAsUtc = Date.UTC(value("year"), value("month") - 1, value("day"), value("hour"), value("minute"), value("second"));
  return Math.round((zonedAsUtc - date.getTime()) / 60000);
}

function serverDayStartIso(date: string) {
  const [year, month, day] = date.split("-").map(Number);
  const utcGuess = new Date(Date.UTC(year, month - 1, day));
  const offsetMinutes = serverTimeZoneOffsetMinutes(utcGuess);
  return new Date(utcGuess.getTime() - offsetMinutes * 60 * 1000).toISOString();
}

function todayIso() {
  return serverDateIso();
}

function yesterdayIso() {
  return addIsoDays(todayIso(), -1);
}

function weekStartIso(date = new Date()) {
  const today = serverDateIso(date);
  const [year, month, day] = today.split("-").map(Number);
  const dayOfWeek = new Date(Date.UTC(year, month - 1, day)).getUTCDay();
  const mondayOffset = (dayOfWeek + 6) % 7;
  return addIsoDays(today, -mondayOffset);
}

function squadRankPoints(date: string, rank: number) {
  if (SQUAD_ZERO_POINT_DATES.has(date)) return 0;
  return rank === 1 ? 2 : rank === 2 ? 1 : 0;
}

function squadSeasonRange(date = todayIso()) {
  return date <= SQUAD_SEASON_END
    ? { start: SQUAD_SEASON_START, end: SQUAD_SEASON_END }
    : { start: SQUAD_NEXT_SEASON_START, end: SQUAD_NEXT_SEASON_END };
}

function isSquadSeasonDate(date: string) {
  return (date >= SQUAD_SEASON_START && date <= SQUAD_SEASON_END)
    || (date >= SQUAD_NEXT_SEASON_START && date <= SQUAD_NEXT_SEASON_END);
}

function friendPair(a: string, b: string) {
  return a < b ? [a, b] : [b, a];
}

// --- Abuse detection helpers (see PRIVACY.md for what's collected and why) ---

function sqliteTimestamp(date: Date) {
  return date.toISOString().slice(0, 19).replace("T", " ");
}

function getRequestIp(request: Request) {
  const ip = request.headers.get("cf-connecting-ip");
  if (!ip || ip === "127.0.0.1" || ip === "::1") return null;
  return ip;
}

function getRequestGeo(request: Request) {
  const cf = (request as Request & { cf?: { country?: string; asn?: number; asOrganization?: string } }).cf;
  return {
    country: typeof cf?.country === "string" ? cf.country : null,
    asn: typeof cf?.asn === "number" ? cf.asn : null,
    asOrganization: typeof cf?.asOrganization === "string" ? cf.asOrganization : null,
  };
}

function toHex(buffer: ArrayBuffer) {
  return Array.from(new Uint8Array(buffer)).map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function hmacHex(secret: string, value: string) {
  const key = await crypto.subtle.importKey("raw", new TextEncoder().encode(secret), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const signature = await crypto.subtle.sign("HMAC", key, new TextEncoder().encode(value));
  return toHex(signature);
}

/**
 * Plain (unkeyed) SHA-256 of a value — used only as a self-consistency check on client-submitted
 * offline-credit intervals (does the payload match the hash the client claims for it), not as an
 * auth/anti-forgery boundary. The hashing algorithm ships in the client bundle, so anyone willing
 * to recompute it can produce a matching hash for fabricated data; it only catches naive,
 * unintentional edits to already-stored data (e.g. a stray localStorage tweak), same caveat as
 * any client-side integrity check over data the client itself controls.
 */
async function sha256Hex(value: string) {
  return toHex(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value)));
}

/** Requires IP_HASH_SECRET — throws (fail closed) if it's not bound. Never falls back to an empty/default key. */
async function hashIp(env: Env, ip: string) {
  if (!env.IP_HASH_SECRET) throw new Response("Server misconfiguration: abuse-detection secret is not set.", { status: 500, headers: corsHeaders });
  return (await hmacHex(env.IP_HASH_SECRET, ip)).slice(0, 32);
}

/**
 * Best-effort variant for abuse-event logging paths (honeypots, flag events) that must never
 * fail the response over a hashing problem — unlike new-account creation, these are detection
 * paths, not an auth gate, so a missing secret or unusable IP just means no hash is recorded.
 */
async function tryHashIp(env: Env, request: Request) {
  const ip = getRequestIp(request);
  if (!ip || !env.IP_HASH_SECRET) return null;
  try {
    return await hashIp(env, ip);
  } catch {
    return null;
  }
}

/** Strips Unicode control characters and truncates. Everything persisted into abuse_events is attacker-reachable input. */
export function sanitizeForLog(value: unknown, maxLength: number) {
  return String(value ?? "")
    .replace(/\p{Cc}+/gu, " ")
    .trim()
    .slice(0, maxLength);
}

async function logAbuseEvent(env: Env, params: { eventType: string; request: Request; ipHash?: string | null; userId?: string | null; detail?: string }) {
  try {
    const geo = getRequestGeo(params.request);
    const userAgent = sanitizeForLog(params.request.headers.get("user-agent"), MAX_USER_AGENT_LENGTH);
    const path = sanitizeForLog(new URL(params.request.url).pathname, MAX_LOG_PATH_LENGTH);
    const detail = sanitizeForLog(params.detail, MAX_LOG_DETAIL_LENGTH);
    await env.DB.prepare(`
      INSERT INTO abuse_events (event_type, ip_hash, country, asn, as_organization, path, user_agent, user_id, detail)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(sanitizeForLog(params.eventType, 60), params.ipHash ?? null, geo.country, geo.asn, geo.asOrganization, path, userAgent, params.userId ?? null, detail).run();
  } catch (error) {
    console.error("Failed to log abuse event.", error);
  }
}

/** Coarse, eventually-consistent KV counter — a throttle/friction signal, not a strict enforcement boundary. */
async function checkAndIncrementNewAccountRateLimit(env: Env, ipHash: string) {
  const bucket = Math.floor(Date.now() / (NEW_ACCOUNT_RATE_LIMIT_WINDOW_SECONDS * 1000));
  const key = `newacct:${ipHash}:${bucket}`;
  const current = Number((await env.RATE_LIMIT_KV.get(key)) ?? "0");
  const count = current + 1;
  await env.RATE_LIMIT_KV.put(key, String(count), { expirationTtl: NEW_ACCOUNT_RATE_LIMIT_WINDOW_SECONDS + 300 });
  return { allowed: count <= NEW_ACCOUNT_RATE_LIMIT_MAX_PER_IP, count };
}

/** Structural check only. Never reject/flag purely for repeated characters — the real generator can legitimately produce e.g. AAAA-BBBB. One weak signal among several. */
export function isFriendCodeAtypical(code: string) {
  return !FRIEND_CODE_PATTERN.test(code);
}

function isHoneypotFieldFilled(payload: SyncPayload["user"]) {
  return payload.referralCode !== undefined && payload.referralCode !== null && String(payload.referralCode).trim() !== "";
}

/** Atomic, additive signal storage — no read-modify-write race. */
export async function recordFlagSignal(env: Env, userId: string, reason: string) {
  await env.DB.prepare("INSERT OR IGNORE INTO user_flag_events (user_id, reason) VALUES (?, ?)").bind(userId, reason).run();
}

export async function maybeFlagUser(env: Env, userId: string, reasons: string[], request: Request) {
  if (!reasons.length) return;
  for (const reason of reasons) await recordFlagSignal(env, userId, reason);

  const rows = await env.DB.prepare("SELECT reason FROM user_flag_events WHERE user_id = ?").bind(userId).all<{ reason: string }>();
  const allReasons = rows.results.map((row) => row.reason);
  const shouldFlag = allReasons.some((reason) => STANDALONE_FLAG_REASONS.has(reason)) || allReasons.length >= 2;
  if (!shouldFlag) return;

  // Guarded by is_flagged = 0: idempotent, so concurrent requests racing here is harmless.
  await env.DB.prepare("UPDATE users SET is_flagged = 1 WHERE id = ? AND is_flagged = 0").bind(userId).run();
  const ipHash = await tryHashIp(env, request);
  for (const reason of reasons) {
    await logAbuseEvent(env, { eventType: `flag:${reason}`, request, ipHash, userId, detail: reason });
  }
}

async function handleHoneypot(request: Request, env: Env) {
  const ipHash = await tryHashIp(env, request);
  await logAbuseEvent(env, { eventType: "honeypot_route", request, ipHash, userId: null });
  return text("Not found.", 404);
}

/** Purges/redacts everything older than the retention window. Takes `now` so it's deterministically testable. */
export async function purgeExpiredAbuseData(env: Env, now: Date = new Date()) {
  const cutoff = sqliteTimestamp(new Date(now.getTime() - ABUSE_DATA_RETENTION_DAYS * 24 * 60 * 60 * 1000));
  await env.DB.prepare("DELETE FROM abuse_events WHERE created_at < ?").bind(cutoff).run();
  await env.DB.prepare(`
    DELETE FROM user_flag_events
    WHERE created_at < ? AND user_id NOT IN (SELECT id FROM users WHERE is_flagged = 1)
  `).bind(cutoff).run();
  await env.DB.prepare(`
    UPDATE users SET signup_ip_hash = NULL, signup_country = NULL, signup_asn = NULL, signup_as_organization = NULL
    WHERE signup_ip_hash IS NOT NULL AND created_at < ?
  `).bind(cutoff).run();
}

export async function hashDeviceSecret(env: Env, deviceSecret: string) {
  if (!env.DEVICE_SECRET_HASH_SECRET) return null;
  return (await hmacHex(env.DEVICE_SECRET_HASH_SECRET, deviceSecret)).slice(0, 32);
}

/**
 * device_secret_hash is being migrated in lazily (see migrations/0018 + handleAdminMigrateDeviceSecrets).
 * If a row is already migrated (device_secret_hash set), verification MUST use the hash and fails
 * closed if DEVICE_SECRET_HASH_SECRET is missing — never falls back to a weaker comparison for an
 * already-migrated credential. Rows still on the legacy plaintext path are entirely unaffected by
 * that secret and keep working via plaintext comparison; a successful plaintext verification
 * opportunistically backfills the hash so the row self-migrates on next use.
 */
export async function verifyDeviceSecret(env: Env, userId: string, user: { device_secret: string; device_secret_hash: string | null }, deviceSecret: string) {
  if (user.device_secret_hash) {
    if (!env.DEVICE_SECRET_HASH_SECRET) throw new Response("Server misconfiguration: credential verification secret is not set.", { status: 500, headers: corsHeaders });
    const candidateHash = await hashDeviceSecret(env, deviceSecret);
    if (candidateHash !== user.device_secret_hash) throw new Response("Invalid device secret.", { status: 403, headers: corsHeaders });
    return;
  }
  if (user.device_secret !== deviceSecret) throw new Response("Invalid device secret.", { status: 403, headers: corsHeaders });
  const hash = await hashDeviceSecret(env, deviceSecret);
  if (hash) await env.DB.prepare("UPDATE users SET device_secret_hash = ? WHERE id = ?").bind(hash, userId).run();
}

async function verifyUser(env: Env, userId: string, deviceSecret: string) {
  userId = cleanUserId(userId);
  deviceSecret = cleanDeviceSecret(deviceSecret);
  const user = await env.DB.prepare("SELECT id, device_secret, device_secret_hash, friend_code AS friendCode FROM users WHERE id = ?").bind(userId).first<{ id: string; device_secret: string; device_secret_hash: string | null; friendCode: string }>();
  if (!user) throw new Response("User has not synced a profile yet.", { status: 404, headers: corsHeaders });
  await verifyDeviceSecret(env, userId, user, deviceSecret);
  return user;
}

type VerifiedSessionRow = {
  id: string;
  userId: string;
  startedAt: string;
  lastHeartbeatAt: string;
};

function parseServerTimestamp(value: string) {
  const parsed = new Date(value.endsWith("Z") ? value : `${value.replace(" ", "T")}Z`);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function verifiedSessionCredits(startedAt: Date, creditedMinutes: number) {
  const credits = new Map<string, number>();
  for (let minute = 0; minute < creditedMinutes; minute += 1) {
    const date = serverDateIso(new Date(startedAt.getTime() + minute * 60_000));
    credits.set(date, (credits.get(date) ?? 0) + 1);
  }
  return credits;
}

async function finishVerifiedSession(env: Env, session: VerifiedSessionRow, now = new Date()) {
  const startedAt = parseServerTimestamp(session.startedAt);
  const lastHeartbeatAt = parseServerTimestamp(session.lastHeartbeatAt);
  if (!startedAt || !lastHeartbeatAt) throw new Response("Verified session is invalid.", { status: 409, headers: corsHeaders });

  const creditedEnd = Math.min(
    now.getTime(),
    startedAt.getTime() + VERIFIED_SESSION_MAX_MS,
    lastHeartbeatAt.getTime() + VERIFIED_SESSION_HEARTBEAT_MS + VERIFIED_SESSION_GRACE_MS,
  );
  const creditedMinutes = Math.max(0, Math.floor((creditedEnd - startedAt.getTime()) / 60_000));
  const credits = verifiedSessionCredits(startedAt, creditedMinutes);
  const finishedDate = serverDateIso(new Date(creditedEnd));
  const finished = await env.DB.prepare(`
    UPDATE verified_study_sessions
    SET status = 'finished', finished_at = ?, credited_minutes = ?
    WHERE id = ? AND status = 'active'
  `).bind(now.toISOString(), creditedMinutes, session.id).run();
  if (!finished.meta.changes) return { creditedMinutes: 0, finishedAt: now.toISOString() };

  const statements: D1PreparedStatement[] = [];

  for (const [date, minutes] of credits) {
    statements.push(env.DB.prepare(`
      INSERT INTO verified_daily_stats (user_id, date, minutes, sessions, updated_at)
      VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
      ON CONFLICT(user_id, date) DO UPDATE SET
        minutes = verified_daily_stats.minutes + excluded.minutes,
        sessions = verified_daily_stats.sessions + excluded.sessions,
        updated_at = CURRENT_TIMESTAMP
    `).bind(session.userId, date, minutes, date === finishedDate ? 1 : 0));
  }
  if (statements.length) await env.DB.batch(statements);
  return { creditedMinutes, finishedAt: now.toISOString() };
}

async function handleVerifiedSessionStart(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const active = await env.DB.prepare(`
    SELECT id, user_id AS userId, started_at AS startedAt, last_heartbeat_at AS lastHeartbeatAt
    FROM verified_study_sessions WHERE user_id = ? AND status = 'active'
  `).bind(userId).first<VerifiedSessionRow>();
  const now = new Date();
  if (active) {
    const startedAt = parseServerTimestamp(active.startedAt);
    if (startedAt && now.getTime() - startedAt.getTime() < VERIFIED_SESSION_MAX_MS) {
      return json({ sessionId: active.id, startedAt: active.startedAt, resumed: true });
    }
    await finishVerifiedSession(env, active, now);
  }

  const sessionId = crypto.randomUUID();
  await env.DB.prepare(`
    INSERT INTO verified_study_sessions (id, user_id, started_at, last_heartbeat_at, status)
    VALUES (?, ?, ?, ?, 'active')
  `).bind(sessionId, userId, now.toISOString(), now.toISOString()).run();
  return json({ sessionId, startedAt: now.toISOString(), resumed: false });
}

async function handleVerifiedSessionHeartbeat(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; sessionId: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const sessionId = requiredText(payload.sessionId, "sessionId", MAX_ID_LENGTH);
  const result = await env.DB.prepare(`
    UPDATE verified_study_sessions SET last_heartbeat_at = ?
    WHERE id = ? AND user_id = ? AND status = 'active'
  `).bind(new Date().toISOString(), sessionId, userId).run();
  if (!result.meta.changes) return text("Verified session not found.", 404);
  return json({ ok: true });
}

async function handleVerifiedSessionFinish(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; sessionId: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const sessionId = requiredText(payload.sessionId, "sessionId", MAX_ID_LENGTH);
  const session = await env.DB.prepare(`
    SELECT id, user_id AS userId, started_at AS startedAt, last_heartbeat_at AS lastHeartbeatAt
    FROM verified_study_sessions WHERE id = ? AND user_id = ? AND status = 'active'
  `).bind(sessionId, userId).first<VerifiedSessionRow>();
  if (!session) return text("Verified session not found.", 404);
  return json({ ok: true, ...(await finishVerifiedSession(env, session)) });
}

/**
 * Ceiling on how many minutes could plausibly accrue in `elapsedMinutes` of wall-clock time,
 * per MAX_PLAUSIBLE_STUDY_MINUTES_PER_DAY, with a small grace buffer for clock skew/batched syncs.
 * Shared by the regular /sync growth check (upsertUser) and offline-credit reconciliation.
 */
function maxPlausibleMinutes(elapsedMinutes: number): number {
  const maxPlausibleDelta = (elapsedMinutes / (24 * 60)) * MAX_PLAUSIBLE_STUDY_MINUTES_PER_DAY;
  return Math.max(elapsedMinutes + 5, maxPlausibleDelta);
}

/**
 * Per-day-bucket credit split for an arbitrary list of (possibly disjoint) intervals — the
 * offline-reconciliation analog of `verifiedSessionCredits`, which only handles a single
 * contiguous span. One "session" credit is attributed to each interval's start date.
 */
function bucketOfflineCredits(intervals: Array<{ start: Date; end: Date }>) {
  const credits = new Map<string, { minutes: number; sessions: number }>();
  const bump = (date: string, minutes: number, sessions: number) => {
    const existing = credits.get(date) ?? { minutes: 0, sessions: 0 };
    credits.set(date, { minutes: existing.minutes + minutes, sessions: existing.sessions + sessions });
  };
  for (const { start, end } of intervals) {
    const totalMinutes = Math.max(0, Math.floor((end.getTime() - start.getTime()) / 60_000));
    for (let minute = 0; minute < totalMinutes; minute += 1) {
      bump(serverDateIso(new Date(start.getTime() + minute * 60_000)), 1, 0);
    }
    if (totalMinutes > 0) bump(serverDateIso(start), 0, 1);
  }
  return credits;
}

/**
 * Truncates a chronologically-sorted list of intervals to at most `capMinutes` total, clipping
 * (not dropping) whichever interval straddles the boundary — earliest offline time is credited
 * first, anything past the cap is left uncredited (surfaced to the user, not silently claimed).
 */
function capIntervalsToMinutes(intervals: Array<{ start: Date; end: Date }>, capMinutes: number) {
  const sorted = [...intervals].sort((a, b) => a.start.getTime() - b.start.getTime());
  let claimedMinutes = 0;
  let remaining = Math.max(0, Math.floor(capMinutes));
  const capped: Array<{ start: Date; end: Date }> = [];
  for (const interval of sorted) {
    const minutes = Math.max(0, Math.floor((interval.end.getTime() - interval.start.getTime()) / 60_000));
    claimedMinutes += minutes;
    if (remaining <= 0 || minutes <= 0) continue;
    if (minutes <= remaining) {
      capped.push(interval);
      remaining -= minutes;
    } else {
      capped.push({ start: interval.start, end: new Date(interval.start.getTime() + remaining * 60_000) });
      remaining = 0;
    }
  }
  const creditedMinutes = capped.reduce((sum, i) => sum + Math.max(0, Math.floor((i.end.getTime() - i.start.getTime()) / 60_000)), 0);
  return { capped, claimedMinutes, creditedMinutes };
}

/**
 * Credits genuinely-offline study time once the client reconnects. Offline time is inherently
 * lower-trust than a live-witnessed heartbeat (the client controls its own clock), so this never
 * merges into `verified_daily_stats` — it lands in a separate `verified_daily_stats_offline`
 * table, is bounded by the same plausibility-rate cap as the regular /sync path plus a hard
 * per-claim ceiling, and is routed through the existing weak-signal flagging machinery rather
 * than trusted outright.
 */
async function handleVerifiedSessionReconcileOffline(request: Request, env: Env) {
  const payload = await readJson<{
    userId: string;
    deviceSecret: string;
    anchorSessionId: string;
    intervals: Array<{ startedAt: string; endedAt: string }>;
    chainTipHash?: string;
  }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const anchorSessionId = requiredText(payload.anchorSessionId, "anchorSessionId", MAX_ID_LENGTH);

  const anchor = await env.DB.prepare(`
    SELECT id, started_at AS startedAt, last_heartbeat_at AS lastHeartbeatAt,
      credited_minutes AS creditedMinutes, status
    FROM verified_study_sessions WHERE id = ? AND user_id = ?
  `).bind(anchorSessionId, userId).first<{ id: string; startedAt: string; lastHeartbeatAt: string; creditedMinutes: number; status: string }>();
  if (!anchor) return text("Verified session not found.", 404);

  const anchorStartedAt = parseServerTimestamp(anchor.startedAt);
  if (!anchorStartedAt) throw new Response("Verified session is invalid.", { status: 409, headers: corsHeaders });

  // Authoritative gap start is derived server-side from where the live-heartbeat path's own cap
  // already left off — never from a client-supplied timestamp — so offline credit can only begin
  // where live credit stopped, with no overlap/double-counting between the two tiers.
  let gapStart: Date;
  if (anchor.status === "finished") {
    gapStart = new Date(anchorStartedAt.getTime() + anchor.creditedMinutes * 60_000);
  } else {
    const lastHeartbeatAt = parseServerTimestamp(anchor.lastHeartbeatAt);
    if (!lastHeartbeatAt) throw new Response("Verified session is invalid.", { status: 409, headers: corsHeaders });
    gapStart = new Date(Math.min(
      anchorStartedAt.getTime() + VERIFIED_SESSION_MAX_MS,
      lastHeartbeatAt.getTime() + VERIFIED_SESSION_HEARTBEAT_MS + VERIFIED_SESSION_GRACE_MS,
    ));
  }

  const now = new Date();
  const rawIntervals = Array.isArray(payload.intervals) ? payload.intervals.slice(0, MAX_OFFLINE_INTERVALS) : [];
  const intervals: Array<{ start: Date; end: Date }> = [];
  for (const raw of rawIntervals) {
    const start = parseServerTimestamp(String(raw?.startedAt ?? ""));
    const end = parseServerTimestamp(String(raw?.endedAt ?? ""));
    if (!start || !end || end <= start) continue;
    // Clamp to the actual gap — time already covered by the live path, or claimed in the future,
    // is never eligible for offline credit regardless of what the client sends.
    const clampedStart = new Date(Math.max(start.getTime(), gapStart.getTime()));
    const clampedEnd = new Date(Math.min(end.getTime(), now.getTime()));
    if (clampedEnd > clampedStart) intervals.push({ start: clampedStart, end: clampedEnd });
  }

  // Tamper-evidence only, not an auth boundary (see sha256Hex doc comment) — recorded for audit,
  // never used to reject or grant additional trust to the claim. Hashed over the RAW submitted
  // intervals (pre-clamp) so an honest client's own hash of what it's sending actually matches —
  // clamping/capping is a separate plausibility check applied after this, not part of the hash.
  const canonicalIntervals = JSON.stringify(rawIntervals.map((raw) => ({ startedAt: String(raw?.startedAt ?? ""), endedAt: String(raw?.endedAt ?? "") })));
  const recomputedHash = await sha256Hex(canonicalIntervals);
  const chainVerified = typeof payload.chainTipHash === "string" && payload.chainTipHash === recomputedHash;

  const elapsedMinutesSinceGap = Math.max(0, (now.getTime() - gapStart.getTime()) / 60_000);
  const capMinutes = Math.min(maxPlausibleMinutes(elapsedMinutesSinceGap), OFFLINE_RECONCILE_MAX_CLAIM_MS / 60_000);
  const { capped, claimedMinutes, creditedMinutes } = capIntervalsToMinutes(intervals, capMinutes);
  const wasCapped = creditedMinutes < claimedMinutes;

  const credits = bucketOfflineCredits(capped);
  if (credits.size) {
    const statements: D1PreparedStatement[] = [];
    for (const [date, { minutes, sessions }] of credits) {
      statements.push(env.DB.prepare(`
        INSERT INTO verified_daily_stats_offline (user_id, date, minutes, sessions, updated_at)
        VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(user_id, date) DO UPDATE SET
          minutes = verified_daily_stats_offline.minutes + excluded.minutes,
          sessions = verified_daily_stats_offline.sessions + excluded.sessions,
          updated_at = CURRENT_TIMESTAMP
      `).bind(userId, date, minutes, sessions));
    }
    await env.DB.batch(statements);
  }

  await env.DB.prepare(`
    INSERT INTO verified_offline_reconciliations
      (id, user_id, anchor_session_id, gap_started_at, gap_ended_at, claimed_minutes, credited_minutes, was_capped, chain_tip_hash, chain_verified)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  `).bind(crypto.randomUUID(), userId, anchorSessionId, gapStart.toISOString(), now.toISOString(), claimedMinutes, creditedMinutes, wasCapped ? 1 : 0, payload.chainTipHash ?? null, chainVerified ? 1 : 0).run();

  // Ordinary, uncapped reconciliation isn't a flag signal at all — using the feature as intended
  // is expected behavior, and it's already recorded in verified_offline_reconciliations for audit.
  // "offline_credit_capped" is deliberately NOT in STANDALONE_FLAG_REASONS, and deliberately the
  // *only* reason passed here: a single capped claim (e.g. a laptop asleep 10h with 3h legitimately
  // studied) must not auto-flag on its own — it needs one more distinct reason (from anywhere:
  // honeypot, atypical friend code, implausible /sync growth, etc.) to cross the 2-reason threshold.
  if (wasCapped) await maybeFlagUser(env, userId, ["offline_credit_capped"], request);

  const flaggedRow = await env.DB.prepare("SELECT is_flagged AS isFlagged FROM users WHERE id = ?").bind(userId).first<{ isFlagged: number }>();

  return json({
    ok: true,
    creditedMinutes,
    cappedFromClaimedMinutes: Math.max(0, claimedMinutes - creditedMinutes),
    flagged: Boolean(flaggedRow?.isFlagged),
  });
}

async function upsertUser(env: Env, payload: SyncPayload["user"], request: Request) {
  const userId = cleanUserId(payload.userId);
  const deviceSecret = cleanDeviceSecret(payload.deviceSecret);
  const friendCode = cleanCode(payload.friendCode);
  const displayName = cleanName(payload.displayName);
  const isPrivate = payload.isPrivate ? 1 : 0;
  const showHoursToFriends = payload.showHoursToFriends === false ? 0 : 1;
  const lifetimeStudyMinutes = cleanFiniteNumber(payload.lifetimeStudyMinutes, "lifetimeStudyMinutes", 10_000_000);
  const lifetimeStudySessions = cleanFiniteNumber(payload.lifetimeStudySessions, "lifetimeStudySessions", 1_000_000);
  const deviceFingerprintHash = cleanText(payload.device?.fingerprintHash, 120);
  const deviceLabel = cleanText(payload.device?.label, 120);
  const app = cleanAppMetadata(payload.app);
  if (!userId || !deviceSecret || !friendCode) throw new Response("Missing user identity.", { status: 400, headers: corsHeaders });

  const existing = await env.DB.prepare(
    "SELECT id, device_secret, device_secret_hash, lifetime_study_minutes, lifetime_study_sessions, updated_at, avatar_json, avatar_size_bytes FROM users WHERE id = ?"
  ).bind(userId).first<{ id: string; device_secret: string; device_secret_hash: string | null; lifetime_study_minutes: number; lifetime_study_sessions: number; updated_at: string; avatar_json: string | null; avatar_size_bytes: number | null }>();
  if (existing) await verifyDeviceSecret(env, userId, existing, deviceSecret);

  const { avatarJson: avatar, avatarSizeBytes } = await resolveAvatarForSync(
    env,
    new URL(request.url).origin,
    userId,
    payload.avatar,
    displayName,
    existing?.avatar_json,
    existing?.avatar_size_bytes,
  );

  const flagReasons: string[] = [];
  let signupIpHash: string | null = null;
  let signupGeo: { country: string | null; asn: number | null; asOrganization: string | null } = { country: null, asn: null, asOrganization: null };

  if (!existing) {
    // Fail closed unconditionally, independent of IP availability — a missing secret is an
    // operational misconfiguration, not an environmental reality like local dev's missing IP.
    if (!env.IP_HASH_SECRET) throw new Response("Server misconfiguration: abuse-detection secret is not set.", { status: 500, headers: corsHeaders });

    const ip = getRequestIp(request);
    if (ip) {
      signupIpHash = await hashIp(env, ip);
      signupGeo = getRequestGeo(request);
      const rate = await checkAndIncrementNewAccountRateLimit(env, signupIpHash);
      if (!rate.allowed) {
        await logAbuseEvent(env, { eventType: "rate_limit_blocked", request, ipHash: signupIpHash, userId, detail: `count=${rate.count}` });
        throw new Response("Too many new accounts created from this network recently. Please try again later.", { status: 429, headers: corsHeaders });
      }
      if (rate.count >= NEW_ACCOUNT_RATE_LIMIT_FLAG_THRESHOLD) flagReasons.push("rapid_ip_signups");
    }
    // IP unusable (e.g. local dev): skip IP-specific work entirely — no throttle, no abuse_events noise.
  }

  const codeOwner = await env.DB.prepare("SELECT id FROM users WHERE friend_code = ? AND id != ?").bind(friendCode, userId).first<{ id: string }>();
  if (codeOwner) throw new Response("Friend code is already in use.", { status: 409, headers: corsHeaders });

  if (isFriendCodeAtypical(friendCode)) flagReasons.push("format_atypical");
  if (isHoneypotFieldFilled(payload)) flagReasons.push("honeypot_field");

  if (existing) {
    // Compute the growth signal from the PRE-update row, before the UPDATE below overwrites it.
    const priorMinutes = existing.lifetime_study_minutes ?? 0;
    const priorUpdatedAt = existing.updated_at ? new Date(`${existing.updated_at.replace(" ", "T")}Z`) : null;
    if (priorUpdatedAt && !Number.isNaN(priorUpdatedAt.getTime())) {
      const elapsedMinutes = Math.max(0, (Date.now() - priorUpdatedAt.getTime()) / 60_000);
      const deltaMinutes = lifetimeStudyMinutes - priorMinutes;
      if (deltaMinutes > maxPlausibleMinutes(elapsedMinutes)) flagReasons.push("implausible_stat_growth");
    }

    await env.DB.prepare(`
      UPDATE users
      SET friend_code = ?, display_name = ?, avatar_json = ?, avatar_size_bytes = ?, is_private = ?, show_hours_to_friends = ?,
        lifetime_study_minutes = ?,
        lifetime_study_sessions = ?,
        device_fingerprint_hash = COALESCE(NULLIF(?, ''), device_fingerprint_hash),
        device_label = COALESCE(NULLIF(?, ''), device_label),
        device_seen_at = CASE WHEN ? != '' THEN CURRENT_TIMESTAMP ELSE device_seen_at END,
        app_version = COALESCE(NULLIF(?, ''), app_version),
        app_platform = COALESCE(NULLIF(?, ''), app_platform),
        app_runtime_channel = COALESCE(NULLIF(?, ''), app_runtime_channel),
        app_seen_at = CASE WHEN ? != '' OR ? != '' OR ? != '' THEN CURRENT_TIMESTAMP ELSE app_seen_at END,
        updated_at = CURRENT_TIMESTAMP, last_seen_at = CURRENT_TIMESTAMP
      WHERE id = ?
    `)
      .bind(friendCode, displayName, avatar, avatarSizeBytes, isPrivate, showHoursToFriends, lifetimeStudyMinutes, lifetimeStudySessions, deviceFingerprintHash, deviceLabel, deviceFingerprintHash, app.version, app.platform, app.runtimeChannel, app.version, app.platform, app.runtimeChannel, userId)
      .run();
  } else {
    if (lifetimeStudyMinutes > MAX_INITIAL_LIFETIME_MINUTES_FLAG || lifetimeStudySessions > MAX_INITIAL_LIFETIME_SESSIONS_FLAG) {
      flagReasons.push("bulk_backfill");
    }

    // New accounts are hashed immediately — no reason to wait for a "next" request when the
    // plaintext secret is already in hand right here. Falls back to NULL (legacy path) only if
    // DEVICE_SECRET_HASH_SECRET isn't configured; this never blocks account creation.
    const deviceSecretHash = await hashDeviceSecret(env, deviceSecret);

    await env.DB.prepare(`
      INSERT INTO users (id, device_secret, device_secret_hash, friend_code, display_name, avatar_json, avatar_size_bytes, is_private, show_hours_to_friends, lifetime_study_minutes, lifetime_study_sessions, device_fingerprint_hash, device_label, device_seen_at, app_version, app_platform, app_runtime_channel, app_seen_at, signup_ip_hash, signup_country, signup_asn, signup_as_organization)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULLIF(?, ''), NULLIF(?, ''), CASE WHEN ? != '' THEN CURRENT_TIMESTAMP ELSE NULL END, NULLIF(?, ''), NULLIF(?, ''), NULLIF(?, ''), CASE WHEN ? != '' OR ? != '' OR ? != '' THEN CURRENT_TIMESTAMP ELSE NULL END, ?, ?, ?, ?)
    `)
      .bind(userId, deviceSecret, deviceSecretHash, friendCode, displayName, avatar, avatarSizeBytes, isPrivate, showHoursToFriends, lifetimeStudyMinutes, lifetimeStudySessions, deviceFingerprintHash, deviceLabel, deviceFingerprintHash, app.version, app.platform, app.runtimeChannel, app.version, app.platform, app.runtimeChannel, signupIpHash, signupGeo.country, signupGeo.asn, signupGeo.asOrganization)
      .run();
  }

  await maybeFlagUser(env, userId, flagReasons, request);
}

function cleanText(value: unknown, max = 180) {
  return String(value ?? "").trim().slice(0, max);
}

function cleanFiniteNumber(value: unknown, label: string, max: number) {
  const number = Number(value ?? 0);
  if (!Number.isFinite(number)) throw new Response(`Invalid ${label}.`, { status: 400, headers: corsHeaders });
  return Math.min(max, Math.max(0, Math.round(number)));
}

function cleanAppMetadata(value: unknown) {
  const record = value && typeof value === "object" ? value as AppMetadata : {};
  return {
    version: cleanText(record.version, 40),
    platform: cleanText(record.platform, 120),
    runtimeChannel: cleanText(record.runtimeChannel, 80),
  };
}

function parseVersionParts(version: string) {
  const match = version.trim().match(/^(\d+(?:\.\d+)*)/);
  if (!match) return null;
  return match[1].split(".").map((part) => Number(part));
}

function compareVersions(a: string, b: string) {
  const aParts = parseVersionParts(a);
  const bParts = parseVersionParts(b);
  if (!aParts || !bParts) return null;
  const length = Math.max(aParts.length, bParts.length);
  for (let index = 0; index < length; index += 1) {
    const left = aParts[index] ?? 0;
    const right = bParts[index] ?? 0;
    if (left !== right) return left < right ? -1 : 1;
  }
  return 0;
}

function isOlderVersion(currentVersion: string, targetVersion: string) {
  const comparison = compareVersions(currentVersion, targetVersion);
  return comparison === null || comparison < 0;
}

function cleanPoll(value: SyncFeedPoll) {
  if (!value || typeof value !== "object") return null;
  const question = cleanText(value.question, MAX_POLL_QUESTION_LENGTH).replace(/\s+/g, " ");
  const rawOptions = Array.isArray(value.options) ? value.options : [];
  const seen = new Set<string>();
  const options = rawOptions.flatMap((option) => {
    if (!option || typeof option !== "object") return [];
    const text = cleanText(option.text, MAX_POLL_OPTION_LENGTH).replace(/\s+/g, " ");
    const dedupeKey = text.toLocaleLowerCase();
    if (!text || seen.has(dedupeKey)) return [];
    seen.add(dedupeKey);
    return [{ id: cleanText(option.id, 80) || crypto.randomUUID(), text }];
  }).slice(0, MAX_POLL_OPTIONS);
  if (!question || options.length < 2) return null;
  return { question, multiple: Boolean(value.multiple), options };
}

function feedImageUrl(request: Request, key: string | null) {
  if (!key) return null;
  return `${new URL(request.url).origin}/feed/image/${encodeURIComponent(key)}`;
}

function imageExpiresAt(now = new Date()) {
  return new Date(now.getTime() + FEED_IMAGE_TTL_MS).toISOString();
}

function usageMonth(date = new Date()) {
  return date.toISOString().slice(0, 7);
}

async function getR2Usage(env: Env, month = usageMonth()) {
  const [usage, storage] = await Promise.all([
    env.DB.prepare("SELECT class_a_ops AS classAOps, class_b_ops AS classBOps FROM r2_usage_monthly WHERE month = ?")
      .bind(month)
      .first<{ classAOps: number; classBOps: number }>(),
    env.DB.prepare("SELECT COALESCE((SELECT SUM(image_size_bytes) FROM feed_posts WHERE image_key IS NOT NULL), 0) + COALESCE((SELECT SUM(avatar_size_bytes) FROM users WHERE avatar_json LIKE '%/profile/avatar/%'), 0) AS storageBytes")
      .first<{ storageBytes: number }>(),
  ]);
  const storageBytes = Number(storage?.storageBytes ?? 0);
  const classAOps = Number(usage?.classAOps ?? 0);
  const classBOps = Number(usage?.classBOps ?? 0);
  return {
    month,
    storageBytes,
    classAOps,
    classBOps,
    warning: storageBytes >= R2_STORAGE_WARNING_BYTES || classAOps >= R2_CLASS_A_WARNING_MONTHLY || classBOps >= R2_CLASS_B_WARNING_MONTHLY,
    paused: storageBytes >= R2_STORAGE_HARD_BYTES || classAOps >= R2_CLASS_A_HARD_MONTHLY || classBOps >= R2_CLASS_B_HARD_MONTHLY,
    limits: {
      storageWarningBytes: R2_STORAGE_WARNING_BYTES,
      storageHardBytes: R2_STORAGE_HARD_BYTES,
      classAWarningMonthly: R2_CLASS_A_WARNING_MONTHLY,
      classAHardMonthly: R2_CLASS_A_HARD_MONTHLY,
      classBWarningMonthly: R2_CLASS_B_WARNING_MONTHLY,
      classBHardMonthly: R2_CLASS_B_HARD_MONTHLY,
    },
  };
}

async function incrementR2Usage(env: Env, values: { classA?: number; classB?: number }) {
  const month = usageMonth();
  await env.DB.prepare(`
    INSERT INTO r2_usage_monthly (month, class_a_ops, class_b_ops, updated_at)
    VALUES (?, ?, ?, CURRENT_TIMESTAMP)
    ON CONFLICT(month) DO UPDATE SET
      class_a_ops = class_a_ops + excluded.class_a_ops,
      class_b_ops = class_b_ops + excluded.class_b_ops,
      updated_at = CURRENT_TIMESTAMP
  `).bind(month, values.classA ?? 0, values.classB ?? 0).run();
}

function r2AlertLevel(usage: Awaited<ReturnType<typeof getR2Usage>>) {
  return usage.paused ? "paused" : usage.warning ? "warning" : "ok";
}

function formatUsageBytes(bytes: number) {
  return `${Math.round(bytes / (1024 * 1024))} MB`;
}

async function maybeNotifyR2Usage(env: Env, currentUsage?: Awaited<ReturnType<typeof getR2Usage>>) {
  if (!env.R2_ALERT_WEBHOOK_URL) return;
  const usage = currentUsage ?? await getR2Usage(env);
  const level = r2AlertLevel(usage);
  const existing = await env.DB.prepare("SELECT level FROM r2_alert_state WHERE id = 'global'").first<{ level: string }>();
  if ((existing?.level ?? "ok") === level) return;

  await env.DB.prepare(`
    INSERT INTO r2_alert_state (id, level, notified_at, updated_at)
    VALUES ('global', ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
    ON CONFLICT(id) DO UPDATE SET level = excluded.level, notified_at = excluded.notified_at, updated_at = CURRENT_TIMESTAMP
  `).bind(level).run();

  if (level === "ok") return;
  const title = level === "paused" ? "Study Tracker R2 paused" : "Study Tracker R2 warning";
  const body = `${title}: storage ${formatUsageBytes(usage.storageBytes)} / ${formatUsageBytes(usage.limits.storageHardBytes)}, writes ${usage.classAOps} / ${usage.limits.classAHardMonthly}, reads ${usage.classBOps} / ${usage.limits.classBHardMonthly}.`;
  await fetch(env.R2_ALERT_WEBHOOK_URL, {
    method: "POST",
    headers: {
      title,
      priority: level === "paused" ? "urgent" : "high",
      tags: level === "paused" ? "warning" : "eyes",
    },
    body,
  }).catch(() => undefined);
}

async function ownerR2Usage(env: Env, friendCode: string) {
  if (friendCode !== R2_OWNER_FRIEND_CODE) return undefined;
  return getR2Usage(env);
}

async function assertR2ClassABudget(env: Env, additionalOps: number) {
  const usage = await getR2Usage(env);
  await maybeNotifyR2Usage(env, usage);
  if (usage.classAOps + additionalOps > R2_CLASS_A_HARD_MONTHLY) {
    throw new Response("Image uploads are paused to keep R2 usage below the free tier.", { status: 429, headers: corsHeaders });
  }
  return usage;
}

async function assertR2ClassBBudget(env: Env) {
  const usage = await getR2Usage(env);
  await maybeNotifyR2Usage(env, usage);
  if (usage.classBOps + 1 > R2_CLASS_B_HARD_MONTHLY) {
    throw new Response("Image loading is paused to keep R2 usage below the free tier.", { status: 429, headers: corsHeaders });
  }
}

async function upsertFeedPosts(env: Env, userId: string, posts: SyncPayload["feedPosts"]) {
  if (!Array.isArray(posts) || !posts.length) return;
  const limitedPosts = posts.slice(0, 25);
  const postIds = limitedPosts.map((post) => cleanText(post.id, 80)).filter(Boolean);
  if (postIds.length) {
    const existing = await env.DB.prepare(`
      SELECT id, user_id AS userId
      FROM feed_posts
      WHERE id IN (${postIds.map(() => "?").join(",")})
    `).bind(...postIds).all<{ id: string; userId: string }>();
    if (existing.results.some((post) => post.userId !== userId)) {
      throw new Response("A feed post belongs to another user.", { status: 409, headers: corsHeaders });
    }
  }
  const statements = limitedPosts.map((post) => env.DB.prepare(`
    INSERT INTO feed_posts (id, user_id, type, subject, detail, note, icon, minutes, preset_label, created_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT(id) DO UPDATE SET
      subject = excluded.subject,
      detail = excluded.detail,
      note = excluded.note,
      icon = excluded.icon,
      minutes = excluded.minutes,
      preset_label = excluded.preset_label
  `).bind(
    cleanText(post.id, 80) || crypto.randomUUID(),
    userId,
    post.type === "milestone" ? "milestone" : "session",
    cleanText(post.subject, 80),
    cleanText(post.detail, 80),
    cleanText(post.note, 220),
    cleanText(post.icon, 8),
    cleanFiniteNumber(post.minutes, "minutes", MAX_DAILY_MINUTES),
    cleanText(post.presetLabel, 60),
    /^\d{4}-\d{2}-\d{2}T/.test(String(post.createdAt ?? "")) ? String(post.createdAt) : new Date().toISOString(),
  ));
  await env.DB.batch(statements);

  for (const post of limitedPosts) {
    const postId = cleanText(post.id, 80);
    if (!postId || !("poll" in post)) continue;
    const poll = cleanPoll(post.poll);
    if (!poll) {
      await env.DB.prepare("DELETE FROM feed_polls WHERE post_id = ?").bind(postId).run();
      continue;
    }
    await env.DB.prepare(`
      INSERT INTO feed_polls (post_id, question, multiple)
      VALUES (?, ?, ?)
      ON CONFLICT(post_id) DO UPDATE SET question = excluded.question, multiple = excluded.multiple
    `).bind(postId, poll.question, poll.multiple ? 1 : 0).run();
    const optionIds = poll.options.map((option) => option.id);
    const conflictingOptions = await env.DB.prepare(`SELECT id, post_id AS postId FROM feed_poll_options WHERE id IN (${optionIds.map(() => "?").join(",")})`).bind(...optionIds).all<{ id: string; postId: string }>();
    if (conflictingOptions.results.some((option) => option.postId !== postId)) {
      throw new Response("A poll option belongs to another post.", { status: 409, headers: corsHeaders });
    }
    const optionStatements = poll.options.map((option, index) => env.DB.prepare(`
      INSERT INTO feed_poll_options (id, post_id, text, sort_order)
      VALUES (?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET text = excluded.text, sort_order = excluded.sort_order
    `).bind(option.id, postId, option.text, index));
    if (optionStatements.length) await env.DB.batch(optionStatements);
    await env.DB.prepare(`
      DELETE FROM feed_poll_options
      WHERE post_id = ? AND id NOT IN (${poll.options.map(() => "?").join(",")})
    `).bind(postId, ...poll.options.map((option) => option.id)).run();
  }
}

async function upsertStats(env: Env, userId: string, stats: SyncPayload["stats"]) {
  const validStats = stats
    .filter((stat) => /^\d{4}-\d{2}-\d{2}$/.test(stat.date))
    .map((stat) => ({
      date: stat.date,
      minutes: cleanFiniteNumber(stat.minutes, "minutes", MAX_DAILY_MINUTES),
      sessions: cleanFiniteNumber(stat.sessions, "sessions", MAX_DAILY_SESSIONS),
    }));
  const cleanedStats = [...new Map(validStats
    .sort((a, b) => b.date.localeCompare(a.date))
    .slice(0, MAX_SYNC_STAT_ROWS)
    .map((stat) => [stat.date, stat])).values()];

  if (!cleanedStats.length) {
    const existingRows = await env.DB.prepare("SELECT date FROM daily_stats WHERE user_id = ?")
      .bind(userId)
      .all<{ date: string }>();
    if (existingRows.results.length) {
      await env.DB.prepare("DELETE FROM daily_stats WHERE user_id = ?").bind(userId).run();
    }
    return existingRows.results.map((row) => row.date);
  }

  const dates = [...new Set(cleanedStats.map((stat) => stat.date))];
  const minSyncedDate = dates.reduce((min, date) => date < min ? date : min, dates[0]);
  const deletesCanCoverAllDates = validStats.length < MAX_SYNC_STAT_ROWS;
  const existingRows = await env.DB.prepare(`
    SELECT date, minutes, sessions
    FROM daily_stats
    WHERE user_id = ? ${deletesCanCoverAllDates ? "" : "AND date >= ?"}
  `).bind(userId, ...(deletesCanCoverAllDates ? [] : [minSyncedDate])).all<{ date: string; minutes: number; sessions: number }>();
  const existing = new Map(existingRows.results.map((row) => [row.date, row]));
  const changedDates = dates.filter((date) => {
    const next = cleanedStats.find((stat) => stat.date === date);
    const current = existing.get(date);
    return Boolean(next && (!current || Number(current.minutes) !== next.minutes || Number(current.sessions) !== next.sessions));
  });
  const deletedDates = existingRows.results
    .filter((row) => !dates.includes(row.date))
    .map((row) => row.date);

  if (deletedDates.length) {
    const deleteClause = deletesCanCoverAllDates
      ? `DELETE FROM daily_stats WHERE user_id = ? AND date NOT IN (${dates.map(() => "?").join(",")})`
      : `DELETE FROM daily_stats WHERE user_id = ? AND date >= ? AND date NOT IN (${dates.map(() => "?").join(",")})`;
    await env.DB.prepare(deleteClause)
      .bind(userId, ...(deletesCanCoverAllDates ? [] : [minSyncedDate]), ...dates)
      .run();
  }

  const statements = cleanedStats.map((stat) => env.DB.prepare(
      "INSERT INTO daily_stats (user_id, date, minutes, sessions, updated_at) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP) ON CONFLICT(user_id, date) DO UPDATE SET minutes = excluded.minutes, sessions = excluded.sessions, updated_at = CURRENT_TIMESTAMP",
    ).bind(
      userId,
      stat.date,
      stat.minutes,
      stat.sessions,
    ));

  await env.DB.batch(statements);
  return [...changedDates, ...deletedDates];
}

async function reconcileLifetimeTotals(env: Env, userId: string) {
  await env.DB.prepare(`
    UPDATE users
    SET lifetime_study_minutes = MAX(lifetime_study_minutes, COALESCE((SELECT SUM(minutes) FROM daily_stats WHERE user_id = ?), 0)),
        lifetime_study_sessions = MAX(lifetime_study_sessions, COALESCE((SELECT SUM(sessions) FROM daily_stats WHERE user_id = ?), 0)),
        updated_at = CURRENT_TIMESTAMP
    WHERE id = ?
  `).bind(userId, userId, userId).run();
}

async function getFriendIds(env: Env, userId: string) {
  const rows = await env.DB.prepare("SELECT CASE WHEN user_low = ? THEN user_high ELSE user_low END AS id FROM friendships WHERE user_low = ? OR user_high = ?")
    .bind(userId, userId, userId)
    .all<{ id: string }>();
  return rows.results.map((row) => row.id);
}

async function canViewFeedPost(env: Env, userId: string, postId: string) {
  const post = await env.DB.prepare(`
    SELECT p.user_id AS userId, u.is_private AS isPrivate
    FROM feed_posts p
    JOIN users u ON u.id = p.user_id
    WHERE p.id = ?
  `).bind(postId).first<{ userId: string; isPrivate: number }>();
  if (!post) return { allowed: false, missing: true };
  if (post.userId === userId || !post.isPrivate) return { allowed: true, missing: false };

  const [userLow, userHigh] = friendPair(userId, post.userId);
  const friendship = await env.DB.prepare("SELECT 1 FROM friendships WHERE user_low = ? AND user_high = ?")
    .bind(userLow, userHigh)
    .first();
  return { allowed: Boolean(friendship), missing: false };
}

function leaderboardWhere(period: LeaderboardPeriod) {
  if (period === "daily") return { clause: "WHERE ds.date = ?", params: [todayIso()] };
  if (period === "weekly") return { clause: "WHERE ds.date >= ?", params: [weekStartIso()] };
  return { clause: "", params: [] };
}

function cleanSquadName(value: unknown) {
  const name = cleanText(value, MAX_SQUAD_NAME_LENGTH).replace(/\s+/g, " ");
  if (!name) throw new Response("Missing squad name.", { status: 400, headers: corsHeaders });
  return name;
}

function cleanSquadRole(value: unknown): SquadRole {
  return value === "leader" || value === "co_leader" || value === "elder" || value === "member" ? value : "member";
}

function roleRank(role: SquadRole) {
  return role === "leader" ? 4 : role === "co_leader" ? 3 : role === "elder" ? 2 : 1;
}

function canManageRequests(role: SquadRole) {
  return roleRank(role) > roleRank("member");
}

function canChangeRole(actorRole: SquadRole, targetRole: SquadRole, nextRole: SquadRole) {
  if (actorRole === "leader") return nextRole !== "leader";
  if (actorRole !== "co_leader") return false;
  return roleRank(targetRole) < roleRank("co_leader") && roleRank(nextRole) < roleRank("co_leader");
}

function canKick(actorRole: SquadRole, targetRole: SquadRole) {
  if (actorRole === "leader") return targetRole !== "leader";
  if (actorRole === "co_leader") return roleRank(targetRole) < roleRank("co_leader");
  if (actorRole === "elder") return targetRole === "member";
  return false;
}

async function getUserSquadMember(env: Env, userId: string) {
  return env.DB.prepare("SELECT squad_id AS squadId, role FROM squad_members WHERE user_id = ?")
    .bind(userId)
    .first<{ squadId: string; role: SquadRole }>();
}

async function getSquadMemberCount(env: Env, squadId: string) {
  const row = await env.DB.prepare("SELECT COUNT(*) AS count FROM squad_members WHERE squad_id = ?")
    .bind(squadId)
    .first<{ count: number }>();
  return Number(row?.count ?? 0);
}

async function closeSquadMemberHistory(env: Env, squadId: string, userId: string) {
  await env.DB.prepare("UPDATE squad_member_history SET left_at = CURRENT_TIMESTAMP WHERE squad_id = ? AND user_id = ? AND left_at IS NULL")
    .bind(squadId, userId)
    .run();
}

async function getSquadMemberIds(env: Env, userId: string) {
  const membership = await getUserSquadMember(env, userId);
  if (!membership) return [];
  const rows = await env.DB.prepare("SELECT user_id AS id FROM squad_members WHERE squad_id = ?")
    .bind(membership.squadId)
    .all<{ id: string }>();
  return rows.results.map((row) => row.id);
}

async function getSquadLeaderboard(env: Env, userId: string, period: LeaderboardPeriod) {
  const memberIds = await getSquadMemberIds(env, userId);
  return getSquadLeaderboardForMemberIds(env, userId, memberIds, period);
}

async function getSquadLeaderboardForMemberIds(env: Env, userId: string, memberIds: string[], period: LeaderboardPeriod) {
  if (!memberIds.length) return [];
  if (period === "overall") {
    const rows = await env.DB.prepare(`
      SELECT ranked.*,
        (SELECT MAX(date) FROM competitive_daily_stats WHERE user_id = ranked.userId) AS lastActiveDate
      FROM (
        SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
          sm.role AS role,
          totals.minutes AS minutes,
          totals.sessions AS sessions
        FROM users u
        JOIN competitive_user_totals totals ON totals.user_id = u.id
        JOIN squad_members sm ON sm.user_id = u.id
        WHERE u.id IN (${memberIds.map(() => "?").join(",")})
        ORDER BY minutes DESC, displayName ASC
        LIMIT 50
      ) ranked
      ORDER BY ranked.minutes DESC, ranked.displayName ASC
    `).bind(...memberIds).all<{
      userId: string;
      displayName: string;
      friendCode: string;
      avatarJson: string;
      role: SquadRole;
      minutes: number;
      sessions: number;
      lastActiveDate: string | null;
    }>();

    return rows.results.map((row, index) => ({
      ...row,
      avatar: parseListAvatar(row.avatarJson, row.displayName),
      avatarJson: undefined,
      minutes: Number(row.minutes),
      sessions: Number(row.sessions),
      rank: index + 1,
      isSelf: row.userId === userId,
    }));
  }
  const periodFilter = leaderboardWhere(period);
  const clauses = [`u.id IN (${memberIds.map(() => "?").join(",")})`];
  const params = [...memberIds, ...periodFilter.params];
  if (periodFilter.clause) clauses.push(periodFilter.clause.replace(/^WHERE\s+/, ""));
  const rows = await env.DB.prepare(`
    SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
      sm.role AS role,
      COALESCE(SUM(ds.minutes), 0) AS minutes,
      COALESCE(SUM(ds.sessions), 0) AS sessions,
      MAX(ds.date) AS lastActiveDate
    FROM users u
    JOIN squad_members sm ON sm.user_id = u.id
    LEFT JOIN competitive_daily_stats ds ON ds.user_id = u.id
    WHERE ${clauses.join(" AND ")}
    GROUP BY u.id, u.display_name, u.friend_code, u.avatar_json, sm.role, u.lifetime_study_minutes, u.lifetime_study_sessions
    ORDER BY minutes DESC, displayName ASC
    LIMIT 50
  `).bind(...params).all<{
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    role: SquadRole;
    minutes: number;
    sessions: number;
    lastActiveDate: string | null;
  }>();

  return rows.results.map((row, index) => ({
    ...row,
    avatar: parseListAvatar(row.avatarJson, row.displayName),
    avatarJson: undefined,
    minutes: Number(row.minutes),
    sessions: Number(row.sessions),
    rank: index + 1,
    isSelf: row.userId === userId,
  }));
}

async function scoreSquadDate(env: Env, date: string) {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date)) return;
  if (!isSquadSeasonDate(date)) return;
  const dayStart = serverDayStartIso(date);
  const dayEnd = new Date(new Date(serverDayStartIso(addIsoDays(date, 1))).getTime() - 1).toISOString();
  const rows = await env.DB.prepare(`
    SELECT s.id AS squadId, s.name,
      COUNT(DISTINCT h.user_id) AS memberCount,
      COUNT(DISTINCT CASE WHEN ds.minutes > 0 THEN h.user_id END) AS activeMemberCount,
      COALESCE(SUM(ds.minutes), 0) AS totalMinutes,
      COALESCE(SUM(ds.sessions), 0) AS totalSessions
    FROM squads s
    JOIN squad_member_history h ON h.squad_id = s.id AND h.joined_at <= ? AND (h.left_at IS NULL OR h.left_at >= ?)
    LEFT JOIN competitive_daily_stats ds ON ds.user_id = h.user_id AND ds.date = ?
    GROUP BY s.id, s.name
    HAVING memberCount >= 2
  `).bind(dayEnd, dayStart, date).all<{ squadId: string; name: string; memberCount: number; activeMemberCount: number; totalMinutes: number; totalSessions: number }>();

  const ranked = rows.results
    .map((row) => ({
      ...row,
      memberCount: Number(row.memberCount),
      activeMemberCount: Number(row.activeMemberCount),
      totalMinutes: Number(row.totalMinutes),
      totalSessions: Number(row.totalSessions),
      averageMinutes: Number(row.activeMemberCount) ? Number(row.totalMinutes) / Number(row.activeMemberCount) : 0,
    }))
    .sort((a, b) => b.averageMinutes - a.averageMinutes || b.totalMinutes - a.totalMinutes || a.name.localeCompare(b.name));

  let previousAverage: number | null = null;
  let previousRank = 0;
  const statements = [env.DB.prepare("DELETE FROM squad_daily_scores WHERE date = ?").bind(date), ...ranked.map((row, index) => {
    const rank = previousAverage !== null && row.averageMinutes === previousAverage ? previousRank : index + 1;
    previousAverage = row.averageMinutes;
    previousRank = rank;
    const points = squadRankPoints(date, rank);
    return env.DB.prepare(`
      INSERT INTO squad_daily_scores (squad_id, date, average_minutes, total_minutes, member_count, rank, points, scored_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
      ON CONFLICT(squad_id, date) DO UPDATE SET
        average_minutes = excluded.average_minutes,
        total_minutes = excluded.total_minutes,
        member_count = excluded.member_count,
        rank = excluded.rank,
        points = excluded.points,
        scored_at = CURRENT_TIMESTAMP
    `).bind(row.squadId, date, row.averageMinutes, row.totalMinutes, row.activeMemberCount, rank, points);
  })];
  await env.DB.batch(statements);
}

async function scoreMissingCompletedSquadDates(env: Env) {
  const today = todayIso();
  const backfillStart = addIsoDays(today, -SQUAD_SCORE_BACKFILL_DAYS);
  const since = backfillStart > SQUAD_SEASON_START ? backfillStart : SQUAD_SEASON_START;
  const rows = await env.DB.prepare(`
    SELECT ds.date
    FROM competitive_daily_stats ds
    WHERE ds.date >= ? AND ds.date < ?
    GROUP BY ds.date
    HAVING NOT EXISTS (
      SELECT 1
      FROM squad_daily_scores score
      WHERE score.date = ds.date
    )
    ORDER BY ds.date ASC
    LIMIT ?
  `).bind(since, today, SQUAD_SCORE_BACKFILL_DAYS).all<{ date: string }>();

  await Promise.all(rows.results.map((row) => scoreSquadDate(env, row.date)
    .catch((error) => console.error("Squad score backfill failed.", row.date, error))));
}

async function getSquadScoreLeaderboard(env: Env, period: SquadScorePeriod) {
  if (period === "daily") {
    const date = todayIso();
    const rows = await env.DB.prepare(`
      SELECT s.id AS squadId, s.name AS squadName, s.is_private AS isPrivate,
        COUNT(DISTINCT sm.user_id) AS memberCount,
        COUNT(DISTINCT CASE WHEN ds.minutes > 0 THEN sm.user_id END) AS activeMemberCount,
        COALESCE(SUM(ds.minutes), 0) AS totalMinutes,
        COALESCE(SUM(ds.sessions), 0) AS totalSessions
      FROM squads s
      JOIN squad_members sm ON sm.squad_id = s.id
      LEFT JOIN competitive_daily_stats ds ON ds.user_id = sm.user_id AND ds.date = ?
      GROUP BY s.id, s.name, s.is_private
      HAVING memberCount >= 2
      ORDER BY CASE WHEN activeMemberCount > 0 THEN CAST(totalMinutes AS REAL) / activeMemberCount ELSE 0 END DESC, totalMinutes DESC, s.name ASC
      LIMIT 50
    `).bind(date).all<{ squadId: string; squadName: string; isPrivate: number; memberCount: number; activeMemberCount: number; totalMinutes: number; totalSessions: number }>();
    let previousAverage: number | null = null;
    let previousRank = 0;
    return rows.results.map((row, index) => {
      const memberCount = Number(row.memberCount);
      const activeMemberCount = Number(row.activeMemberCount);
      const totalMinutes = Number(row.totalMinutes);
      const averageMinutes = activeMemberCount ? totalMinutes / activeMemberCount : 0;
      const rank = previousAverage !== null && averageMinutes === previousAverage ? previousRank : index + 1;
      previousAverage = averageMinutes;
      previousRank = rank;
      return {
        squadId: row.squadId,
        squadName: row.squadName,
        isPrivate: Boolean(row.isPrivate),
        memberCount,
        totalMinutes,
        totalSessions: Number(row.totalSessions),
        averageMinutes,
        rank,
        points: squadRankPoints(date, rank),
      };
    });
  }

  const season = squadSeasonRange();
  const periodFilter = period === "season" ? "WHERE score.date >= ? AND score.date <= ?" : "WHERE score.date >= ?";
  const params = period === "season" ? [season.start, season.end] : [SQUAD_SEASON_START];
  const rows = await env.DB.prepare(`
    SELECT s.id AS squadId, s.name AS squadName, s.is_private AS isPrivate,
      (SELECT COUNT(*) FROM squad_members sm WHERE sm.squad_id = s.id) AS currentMemberCount,
      COALESCE(SUM(score.points), 0) AS points,
      COALESCE(SUM(score.total_minutes), 0) AS totalMinutes,
      COALESCE(SUM(score.member_count), 0) AS memberCountSum,
      COUNT(score.date) AS scoredDays
    FROM squads s
    LEFT JOIN squad_daily_scores score ON score.squad_id = s.id
    ${periodFilter}
    GROUP BY s.id, s.name, s.is_private
    HAVING scoredDays > 0
    ORDER BY points DESC, CASE WHEN memberCountSum > 0 THEN CAST(totalMinutes AS REAL) / memberCountSum ELSE 0 END DESC, s.name ASC
    LIMIT 50
  `).bind(...params).all<{ squadId: string; squadName: string; isPrivate: number; currentMemberCount: number; points: number; totalMinutes: number; memberCountSum: number; scoredDays: number }>();

  let previousPoints: number | null = null;
  let previousRank = 0;
  return rows.results.map((row, index) => {
    const points = Number(row.points);
    const memberCountSum = Number(row.memberCountSum);
    const totalMinutes = Number(row.totalMinutes);
    const rank = previousPoints !== null && points === previousPoints ? previousRank : index + 1;
    previousPoints = points;
    previousRank = rank;
    return {
      squadId: row.squadId,
      squadName: row.squadName,
      isPrivate: Boolean(row.isPrivate),
      memberCount: Number(row.currentMemberCount),
      totalMinutes,
      totalSessions: 0,
      averageMinutes: memberCountSum ? totalMinutes / memberCountSum : 0,
      rank,
      points,
      scoredDays: Number(row.scoredDays),
    };
  });
}

async function getSquadDetails(env: Env, userId: string, squadId: string) {
  const today = todayIso();
  const yesterday = addIsoDays(today, -1);
  const [squad, userMembership, pending] = await Promise.all([
    env.DB.prepare(`
      SELECT s.id, s.name, s.is_private AS isPrivate, s.created_by_user_id AS createdByUserId, s.created_at AS createdAt,
        COALESCE(SUM(ds.minutes), 0) AS totalMinutes,
        COALESCE(SUM(ds.sessions), 0) AS totalSessions,
        COUNT(DISTINCT sm.user_id) AS memberCount
      FROM squads s
      LEFT JOIN squad_members sm ON sm.squad_id = s.id
      LEFT JOIN competitive_daily_stats ds ON ds.user_id = sm.user_id AND ds.date = ?
      WHERE s.id = ?
      GROUP BY s.id, s.name, s.is_private, s.created_by_user_id, s.created_at
    `).bind(today, squadId).first<{
      id: string;
      name: string;
      isPrivate: number;
      createdByUserId: string;
      createdAt: string;
      totalMinutes: number;
      totalSessions: number;
      memberCount: number;
    }>(),
    getUserSquadMember(env, userId),
    env.DB.prepare("SELECT 1 FROM squad_join_requests WHERE squad_id = ? AND user_id = ? AND status = 'pending'")
      .bind(squadId, userId)
      .first(),
  ]);
  if (!squad) return null;

  const previousDayStats = await env.DB.prepare(`
    SELECT COALESCE(SUM(ds.minutes), 0) AS totalMinutes,
      COALESCE(SUM(ds.sessions), 0) AS totalSessions,
      COUNT(DISTINCT sm.user_id) AS memberCount,
      COUNT(DISTINCT CASE WHEN ds.minutes > 0 THEN sm.user_id END) AS activeMemberCount
    FROM squad_members sm
      LEFT JOIN competitive_daily_stats ds ON ds.user_id = sm.user_id AND ds.date = ?
    WHERE sm.squad_id = ?
  `).bind(yesterday, squadId).first<{ totalMinutes: number; totalSessions: number; memberCount: number; activeMemberCount: number }>();

  const members = await env.DB.prepare(`
    SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
      sm.role, sm.joined_at AS joinedAt, u.last_seen_at AS lastSeenAt,
      COALESCE(SUM(ds.minutes), 0) AS minutes,
      COALESCE(SUM(ds.sessions), 0) AS sessions
    FROM squad_members sm
    JOIN users u ON u.id = sm.user_id
    LEFT JOIN competitive_daily_stats ds ON ds.user_id = sm.user_id AND ds.date = ?
    WHERE sm.squad_id = ?
    GROUP BY u.id, u.display_name, u.friend_code, u.avatar_json, sm.role, sm.joined_at, u.last_seen_at
    ORDER BY CASE sm.role WHEN 'leader' THEN 1 WHEN 'co_leader' THEN 2 WHEN 'elder' THEN 3 ELSE 4 END, sm.joined_at ASC
  `).bind(today, squadId).all<{
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    role: SquadRole;
    joinedAt: string;
    lastSeenAt: string | null;
    minutes: number;
    sessions: number;
  }>();

  const memberCount = Number(squad.memberCount);
  const previousDayMemberCount = Number(previousDayStats?.memberCount ?? 0);
  const previousDayActiveMemberCount = Number(previousDayStats?.activeMemberCount ?? 0);
  const previousDayTotalMinutes = Number(previousDayStats?.totalMinutes ?? 0);
  const action = userMembership
    ? userMembership.squadId === squadId ? "current" : "unavailable"
    : memberCount >= MAX_SQUAD_MEMBERS ? "full"
    : pending ? "pending"
    : squad.isPrivate ? "request"
    : "join";

  return {
    id: squad.id,
    name: squad.name,
    isPrivate: Boolean(squad.isPrivate),
    createdByUserId: squad.createdByUserId,
    createdAt: squad.createdAt,
    totalMinutes: Number(squad.totalMinutes),
    totalSessions: Number(squad.totalSessions),
    memberCount,
    maxMembers: MAX_SQUAD_MEMBERS,
    statsDate: today,
    previousDayDate: yesterday,
    previousDayTotalMinutes,
    previousDayTotalSessions: Number(previousDayStats?.totalSessions ?? 0),
    previousDayMemberCount,
    previousDayAverageMinutes: previousDayActiveMemberCount ? previousDayTotalMinutes / previousDayActiveMemberCount : 0,
    action,
    members: members.results.map((member) => ({
      ...member,
      avatar: parseListAvatar(member.avatarJson, member.displayName),
      avatarJson: undefined,
      minutes: Number(member.minutes),
      sessions: Number(member.sessions),
      isSelf: member.userId === userId,
    })),
  };
}

async function getSquadLeaderboards(env: Env, userId: string) {
  const memberIds = await getSquadMemberIds(env, userId);
  const [daily, weekly, overall] = await Promise.all([
    getSquadLeaderboardForMemberIds(env, userId, memberIds, "daily"),
    getSquadLeaderboardForMemberIds(env, userId, memberIds, "weekly"),
    getSquadLeaderboardForMemberIds(env, userId, memberIds, "overall"),
  ]);
  return { daily, weekly, overall };
}

async function getSquadSnapshot(env: Env, userId: string) {
  const membership = await getUserSquadMember(env, userId);
  if (!membership) {
    const outgoing = await env.DB.prepare(`
      SELECT r.id, r.squad_id AS squadId, s.name AS squadName, s.is_private AS isPrivate, r.status, r.created_at AS createdAt
      FROM squad_join_requests r
      JOIN squads s ON s.id = r.squad_id
      WHERE r.user_id = ? AND r.status = 'pending'
      ORDER BY r.created_at DESC
    `).bind(userId).all();
    return { squad: null, outgoingSquadRequests: outgoing.results, incomingSquadRequests: [], squadMessages: [] };
  }

  const squad = await env.DB.prepare(`
    SELECT s.id, s.name, s.is_private AS isPrivate, s.created_by_user_id AS createdByUserId, s.created_at AS createdAt,
      COALESCE(SUM(totals.minutes), 0) AS totalMinutes,
      COALESCE(SUM(totals.sessions), 0) AS totalSessions,
      COUNT(DISTINCT sm.user_id) AS memberCount
    FROM squads s
    JOIN squad_members sm ON sm.squad_id = s.id
    JOIN users u ON u.id = sm.user_id
    JOIN competitive_user_totals totals ON totals.user_id = u.id
    WHERE s.id = ?
    GROUP BY s.id, s.name, s.is_private, s.created_by_user_id, s.created_at
  `).bind(membership.squadId).first<{
    id: string;
    name: string;
    isPrivate: number;
    createdByUserId: string;
    createdAt: string;
    totalMinutes: number;
    totalSessions: number;
    memberCount: number;
  }>();

  const members = await env.DB.prepare(`
    SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
      sm.role, sm.joined_at AS joinedAt, u.last_seen_at AS lastSeenAt,
      totals.minutes AS minutes,
      totals.sessions AS sessions
    FROM squad_members sm
    JOIN users u ON u.id = sm.user_id
    JOIN competitive_user_totals totals ON totals.user_id = u.id
    WHERE sm.squad_id = ?
    ORDER BY CASE sm.role WHEN 'leader' THEN 1 WHEN 'co_leader' THEN 2 WHEN 'elder' THEN 3 ELSE 4 END, sm.joined_at ASC
  `).bind(membership.squadId).all<{
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    role: SquadRole;
    joinedAt: string;
    lastSeenAt: string | null;
    minutes: number;
    sessions: number;
  }>();

  const incoming = canManageRequests(membership.role) ? await env.DB.prepare(`
    SELECT r.id, r.squad_id AS squadId, r.user_id AS userId, u.display_name AS displayName, u.friend_code AS friendCode,
      u.avatar_json AS avatarJson, r.status, r.created_at AS createdAt
    FROM squad_join_requests r
    JOIN users u ON u.id = r.user_id
    WHERE r.squad_id = ? AND r.status = 'pending'
    ORDER BY r.created_at DESC
  `).bind(membership.squadId).all<{
    id: string;
    squadId: string;
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    status: string;
    createdAt: string;
  }>() : { results: [] };

  const messages = await env.DB.prepare(`
    SELECT m.id, m.squad_id AS squadId, m.user_id AS userId, u.display_name AS displayName, u.friend_code AS friendCode,
      u.avatar_json AS avatarJson, sm.role, m.body, m.created_at AS createdAt
    FROM squad_messages m
    JOIN users u ON u.id = m.user_id
    JOIN squad_members sm ON sm.squad_id = m.squad_id AND sm.user_id = m.user_id
    WHERE m.squad_id = ?
    ORDER BY m.created_at DESC
    LIMIT 60
  `).bind(membership.squadId).all<{
    id: string;
    squadId: string;
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    role: SquadRole;
    body: string;
    createdAt: string;
  }>();

  return {
    squad: squad ? {
      ...squad,
      isPrivate: Boolean(squad.isPrivate),
      totalMinutes: Number(squad.totalMinutes),
      totalSessions: Number(squad.totalSessions),
      memberCount: Number(squad.memberCount),
      myRole: membership.role,
      members: members.results.map((member) => ({
        ...member,
        avatar: parseListAvatar(member.avatarJson, member.displayName),
        avatarJson: undefined,
        minutes: Number(member.minutes),
        sessions: Number(member.sessions),
        isSelf: member.userId === userId,
      })),
    } : null,
    incomingSquadRequests: incoming.results.map((request) => ({
      ...request,
      avatar: parseListAvatar(request.avatarJson, request.displayName),
      avatarJson: undefined,
    })),
    outgoingSquadRequests: [],
    squadMessages: messages.results.reverse().map((message) => ({
      ...message,
      avatar: parseListAvatar(message.avatarJson, message.displayName),
      avatarJson: undefined,
      isSelf: message.userId === userId,
    })),
  };
}

async function getLeaderboard(env: Env, userId: string, scope: LeaderboardScope, period: LeaderboardPeriod) {
  if (scope === "squad") return getSquadLeaderboard(env, userId, period);
  const friendIds = scope === "friends" ? await getFriendIds(env, userId) : [];
  const allowedIds = scope === "friends" ? [userId, ...friendIds] : [];
  if (period === "overall") {
    const clauses: string[] = [];
    const params: string[] = [];
    if (scope === "global") clauses.push("u.is_private = 0");
    if (allowedIds.length) {
      clauses.push(`u.id IN (${allowedIds.map(() => "?").join(",")})`);
      params.push(...allowedIds);
    }
    if (scope === "friends") {
      clauses.push("(u.id = ? OR u.show_hours_to_friends = 1)");
      params.push(userId);
    }
    const whereClause = clauses.length ? `WHERE ${clauses.join(" AND ")}` : "";
    const rows = await env.DB.prepare(`
      SELECT ranked.*,
        (SELECT MAX(date) FROM competitive_daily_stats WHERE user_id = ranked.userId) AS lastActiveDate
      FROM (
        SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
          totals.minutes AS minutes,
          totals.sessions AS sessions
        FROM users u
        JOIN competitive_user_totals totals ON totals.user_id = u.id
        ${whereClause}
        ORDER BY minutes DESC, displayName ASC
        LIMIT 50
      ) ranked
      ORDER BY ranked.minutes DESC, ranked.displayName ASC
    `).bind(...params).all<{
      userId: string;
      displayName: string;
      friendCode: string;
      avatarJson: string;
      minutes: number;
      sessions: number;
      lastActiveDate: string | null;
    }>();

    return rows.results.map((row, index) => ({
      ...row,
      avatar: parseListAvatar(row.avatarJson, row.displayName),
      avatarJson: undefined,
      minutes: Number(row.minutes),
      sessions: Number(row.sessions),
      rank: index + 1,
      isSelf: row.userId === userId,
    }));
  }
  const periodFilter = leaderboardWhere(period);
  const clauses = [];
  const params = [...periodFilter.params];
  if (periodFilter.clause) clauses.push(periodFilter.clause.replace(/^WHERE\s+/, ""));
  if (scope === "global") clauses.push("u.is_private = 0");
  if (allowedIds.length) {
    clauses.push(`u.id IN (${allowedIds.map(() => "?").join(",")})`);
    params.push(...allowedIds);
  }
  if (scope === "friends") {
    clauses.push("(u.id = ? OR u.show_hours_to_friends = 1)");
    params.push(userId);
  }
  const whereClause = clauses.length ? `WHERE ${clauses.join(" AND ")}` : "";
  const rows = await env.DB.prepare(`
    SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
      COALESCE(SUM(ds.minutes), 0) AS minutes,
      COALESCE(SUM(ds.sessions), 0) AS sessions,
      MAX(ds.date) AS lastActiveDate
    FROM users u
    LEFT JOIN competitive_daily_stats ds ON ds.user_id = u.id
    ${whereClause}
    GROUP BY u.id, u.display_name, u.friend_code, u.avatar_json, u.lifetime_study_minutes, u.lifetime_study_sessions
    ORDER BY minutes DESC, displayName ASC
    LIMIT 50
  `).bind(...params).all<{
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    minutes: number;
    sessions: number;
    lastActiveDate: string | null;
  }>();

  return rows.results.map((row, index) => ({
    ...row,
    avatar: parseListAvatar(row.avatarJson, row.displayName),
    avatarJson: undefined,
    minutes: Number(row.minutes),
    sessions: Number(row.sessions),
    rank: index + 1,
    isSelf: row.userId === userId,
  }));
}

async function getFeed(request: Request, env: Env, userId: string, scope: LeaderboardScope) {
  const friendIds = scope === "friends" ? await getFriendIds(env, userId) : [];
  const viewerFriendIds = scope === "friends" ? friendIds : await getFriendIds(env, userId);
  const visibleReactorIds = new Set([userId, ...viewerFriendIds]);
  const visibleCommenterIds = visibleReactorIds;
  const allowedIds = scope === "friends" ? [userId, ...friendIds] : [];
  const clauses = scope === "global" ? ["u.is_private = 0"] : [`p.user_id IN (${allowedIds.map(() => "?").join(",") || "?"})`];
  const params = scope === "friends" ? (allowedIds.length ? allowedIds : [userId]) : [];
  const posts = await env.DB.prepare(`
    SELECT p.id, p.user_id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson,
      p.type, p.subject, p.detail, p.note, p.icon, p.minutes, p.preset_label AS presetLabel, p.created_at AS createdAt,
      p.image_key AS imageKey, p.image_mime_type AS imageMimeType, p.image_expires_at AS imageExpiresAt, p.image_expired_at AS imageExpiredAt
    FROM feed_posts p
    JOIN users u ON u.id = p.user_id
    WHERE ${clauses.join(" AND ")}
    ORDER BY p.created_at DESC
    LIMIT 40
  `).bind(...params).all<{
    id: string;
    userId: string;
    displayName: string;
    friendCode: string;
    avatarJson: string;
    type: "session" | "milestone";
    subject: string;
    detail: string;
    note: string;
    icon: string;
    minutes: number;
    presetLabel: string;
    createdAt: string;
    imageKey: string | null;
    imageMimeType: string | null;
    imageExpiresAt: string | null;
    imageExpiredAt: string | null;
  }>();

  const ids = posts.results.map((post) => post.id);
  const reactionCounts = new Map<string, Record<string, number>>();
  const reacted = new Map<string, Record<string, boolean>>();
  const reactedBy = new Map<string, Record<string, string[]>>();
  const comments = new Map<string, Array<{
    id: string;
    postId: string;
    userId: string;
    displayName: string;
    friendCode: string;
    avatar: SocialAvatar;
    body: string;
    createdAt: string;
    isSelf: boolean;
  }>>();
  const polls = new Map<string, { question: string; multiple: boolean; options: Array<{ id: string; text: string; votes: number; selected: boolean }>; totalVotes: number }>();
  if (ids.length) {
    const countRows = await env.DB.prepare(`
      SELECT post_id AS postId, emoji, COUNT(*) AS count
      FROM feed_reactions
      WHERE post_id IN (${ids.map(() => "?").join(",")})
      GROUP BY post_id, emoji
    `).bind(...ids).all<{ postId: string; emoji: string; count: number }>();
    countRows.results.forEach((row) => {
      reactionCounts.set(row.postId, { ...(reactionCounts.get(row.postId) ?? {}), [row.emoji]: Number(row.count) });
    });
    const reactedRows = await env.DB.prepare(`
      SELECT post_id AS postId, emoji
      FROM feed_reactions
      WHERE user_id = ? AND post_id IN (${ids.map(() => "?").join(",")})
    `).bind(userId, ...ids).all<{ postId: string; emoji: string }>();
    reactedRows.results.forEach((row) => {
      reacted.set(row.postId, { ...(reacted.get(row.postId) ?? {}), [row.emoji]: true });
    });
    const namesRows = await env.DB.prepare(`
      SELECT r.post_id AS postId, r.user_id AS userId, r.emoji, u.display_name AS displayName, u.is_private AS isPrivate
      FROM feed_reactions r
      JOIN users u ON u.id = r.user_id
      WHERE r.post_id IN (${ids.map(() => "?").join(",")})
      ORDER BY r.post_id, r.emoji, u.display_name
    `).bind(...ids).all<{ postId: string; userId: string; emoji: string; displayName: string; isPrivate: number }>();
    namesRows.results.forEach((row) => {
      if (row.isPrivate && !visibleReactorIds.has(row.userId)) return;
      const existing = reactedBy.get(row.postId) ?? {};
      const names = existing[row.emoji] ?? [];
      names.push(row.displayName);
      existing[row.emoji] = names;
      reactedBy.set(row.postId, existing);
    });

    const commentRows = await env.DB.prepare(`
      SELECT c.id, c.post_id AS postId, c.user_id AS userId, u.display_name AS displayName, u.friend_code AS friendCode,
        u.avatar_json AS avatarJson, u.is_private AS isPrivate, c.body, c.created_at AS createdAt
      FROM feed_comments c
      JOIN users u ON u.id = c.user_id
      WHERE c.post_id IN (${ids.map(() => "?").join(",")})
      ORDER BY c.created_at ASC
    `).bind(...ids).all<{ id: string; postId: string; userId: string; displayName: string; friendCode: string; avatarJson: string; isPrivate: number; body: string; createdAt: string }>();
    commentRows.results.forEach((row) => {
      if (row.isPrivate && !visibleCommenterIds.has(row.userId)) return;
      const existing = comments.get(row.postId) ?? [];
      existing.push({
        id: row.id,
        postId: row.postId,
        userId: row.userId,
        displayName: row.displayName,
        friendCode: row.friendCode,
        avatar: parseListAvatar(row.avatarJson, row.displayName),
        body: row.body,
        createdAt: row.createdAt,
        isSelf: row.userId === userId,
      });
      comments.set(row.postId, existing);
    });

    const pollRows = await env.DB.prepare(`
      SELECT post_id AS postId, question, multiple
      FROM feed_polls
      WHERE post_id IN (${ids.map(() => "?").join(",")})
    `).bind(...ids).all<{ postId: string; question: string; multiple: number }>();
    pollRows.results.forEach((row) => {
      polls.set(row.postId, { question: row.question, multiple: Boolean(row.multiple), options: [], totalVotes: 0 });
    });
    if (pollRows.results.length) {
      const optionRows = await env.DB.prepare(`
        SELECT o.id, o.post_id AS postId, o.text, COUNT(v.user_id) AS votes,
          MAX(CASE WHEN v.user_id = ? THEN 1 ELSE 0 END) AS selected
        FROM feed_poll_options o
        LEFT JOIN feed_poll_votes v ON v.option_id = o.id
        WHERE o.post_id IN (${ids.map(() => "?").join(",")})
        GROUP BY o.id, o.post_id, o.text, o.sort_order
        ORDER BY o.post_id, o.sort_order ASC
      `).bind(userId, ...ids).all<{ id: string; postId: string; text: string; votes: number; selected: number }>();
      optionRows.results.forEach((row) => {
        const poll = polls.get(row.postId);
        if (!poll) return;
        const votes = Number(row.votes ?? 0);
        poll.options.push({ id: row.id, text: row.text, votes, selected: Boolean(row.selected) });
        poll.totalVotes += votes;
      });
    }
  }

  return posts.results.map((post) => ({
    ...post,
    avatar: parseListAvatar(post.avatarJson, post.displayName),
    avatarJson: undefined,
    minutes: Number(post.minutes),
    isSelf: post.userId === userId,
    imageUrl: feedImageUrl(request, post.imageKey),
    poll: polls.get(post.id) ?? null,
    reactions: { fire: 0, brain: 0, clap: 0, ...(reactionCounts.get(post.id) ?? {}) },
    reacted: reacted.get(post.id) ?? {},
    reactedBy: reactedBy.get(post.id) ?? {},
    comments: comments.get(post.id) ?? [],
  }));
}

async function getSocialSnapshot(request: Request, env: Env, userId: string, includeCaches = true) {
  const friends = await env.DB.prepare(`
    SELECT u.id AS userId, u.display_name AS displayName, u.friend_code AS friendCode, u.avatar_json AS avatarJson, f.created_at AS friendsSince, u.last_seen_at AS lastSeenAt
    FROM friendships f
    JOIN users u ON u.id = CASE WHEN f.user_low = ? THEN f.user_high ELSE f.user_low END
    WHERE f.user_low = ? OR f.user_high = ?
    ORDER BY u.display_name ASC
  `).bind(userId, userId, userId).all();

  const incoming = await env.DB.prepare(`
    SELECT r.id, r.from_user_id AS fromUserId, r.to_user_id AS toUserId,
      from_u.display_name AS fromDisplayName, to_u.display_name AS toDisplayName,
      from_u.friend_code AS fromFriendCode, to_u.friend_code AS toFriendCode,
      from_u.avatar_json AS fromAvatarJson, to_u.avatar_json AS toAvatarJson,
      r.status, r.created_at AS createdAt
    FROM friend_requests r
    JOIN users from_u ON from_u.id = r.from_user_id
    JOIN users to_u ON to_u.id = r.to_user_id
    WHERE r.to_user_id = ? AND r.status = 'pending'
    ORDER BY r.created_at DESC
  `).bind(userId).all();

  const outgoing = await env.DB.prepare(`
    SELECT r.id, r.from_user_id AS fromUserId, r.to_user_id AS toUserId,
      from_u.display_name AS fromDisplayName, to_u.display_name AS toDisplayName,
      from_u.friend_code AS fromFriendCode, to_u.friend_code AS toFriendCode,
      from_u.avatar_json AS fromAvatarJson, to_u.avatar_json AS toAvatarJson,
      r.status, r.created_at AS createdAt
    FROM friend_requests r
    JOIN users from_u ON from_u.id = r.from_user_id
    JOIN users to_u ON to_u.id = r.to_user_id
    WHERE r.from_user_id = ? AND r.status = 'pending'
    ORDER BY r.created_at DESC
  `).bind(userId).all();

  const socialBasics = {
    friends: friends.results.map((friend) => ({
      ...friend,
      avatar: parseListAvatar((friend as { avatarJson?: string }).avatarJson, (friend as { displayName?: string }).displayName ?? "Student"),
      avatarJson: undefined,
    })),
    incomingFriendRequests: incoming.results.map((request) => ({
      ...request,
      fromAvatar: parseListAvatar((request as { fromAvatarJson?: string }).fromAvatarJson, (request as { fromDisplayName?: string }).fromDisplayName ?? "Student"),
      toAvatar: parseListAvatar((request as { toAvatarJson?: string }).toAvatarJson, (request as { toDisplayName?: string }).toDisplayName ?? "Student"),
      fromAvatarJson: undefined,
      toAvatarJson: undefined,
    })),
    outgoingFriendRequests: outgoing.results.map((request) => ({
      ...request,
      fromAvatar: parseListAvatar((request as { fromAvatarJson?: string }).fromAvatarJson, (request as { fromDisplayName?: string }).fromDisplayName ?? "Student"),
      toAvatar: parseListAvatar((request as { toAvatarJson?: string }).toAvatarJson, (request as { toDisplayName?: string }).toDisplayName ?? "Student"),
      fromAvatarJson: undefined,
      toAvatarJson: undefined,
    })),
  };

  if (!includeCaches) {
    return { social: { ...socialBasics, ...await getSquadSnapshot(env, userId) } };
  }

  const [globalDaily, globalWeekly, globalOverall, friendsDaily, friendsWeekly, friendsOverall, squadLeaderboards, squadScoreDaily, squadScoreSeason, squadScoreOverall, globalFeed, friendsFeed, squadSnapshot] = await Promise.all([
    getLeaderboard(env, userId, "global", "daily"),
    getLeaderboard(env, userId, "global", "weekly"),
    getLeaderboard(env, userId, "global", "overall"),
    getLeaderboard(env, userId, "friends", "daily"),
    getLeaderboard(env, userId, "friends", "weekly"),
    getLeaderboard(env, userId, "friends", "overall"),
    getSquadLeaderboards(env, userId),
    getSquadScoreLeaderboard(env, "daily"),
    getSquadScoreLeaderboard(env, "season"),
    getSquadScoreLeaderboard(env, "overall"),
    getFeed(request, env, userId, "global"),
    getFeed(request, env, userId, "friends"),
    getSquadSnapshot(env, userId),
  ]);
  const cachedLeaderboards = {
    global: { daily: globalDaily, weekly: globalWeekly, overall: globalOverall },
    friends: { daily: friendsDaily, weekly: friendsWeekly, overall: friendsOverall },
    squad: squadLeaderboards,
  };
  const cachedFeeds = {
    global: globalFeed,
    friends: friendsFeed,
  };
  return {
    social: {
      ...socialBasics,
      cachedLeaderboards,
      cachedFeeds,
      cachedSquadScoreLeaderboards: { daily: squadScoreDaily, season: squadScoreSeason, overall: squadScoreOverall },
      ...squadSnapshot,
    },
  };
}

async function handleSync(request: Request, env: Env, lightweight = false) {
  const payload = await readJson<SyncPayload>(request);
  if (!payload.user || typeof payload.user !== "object") return text("Missing user identity.", 400);
  if (!Array.isArray(payload.stats)) return text("Missing stats.", 400);
  await upsertUser(env, payload.user, request);
  const userId = cleanUserId(payload.user.userId);
  const changedStatDates = await upsertStats(env, userId, payload.stats);
  await reconcileLifetimeTotals(env, userId);
  await upsertFeedPosts(env, userId, payload.feedPosts);
  const today = todayIso();
  const datesToRescore = changedStatDates.filter((date) => date < today);
  if (datesToRescore.length) {
    await env.DB.batch(datesToRescore.map((date) => env.DB.prepare(
      "INSERT INTO squad_score_jobs (date) VALUES (?) ON CONFLICT(date) DO NOTHING",
    ).bind(date)));
  }
  if (lightweight) return json({ ok: true, syncedAt: new Date().toISOString() });
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleFeed(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; scope?: LeaderboardScope }>(request);
  const userId = cleanUserId(payload.userId);
  const deviceSecret = cleanDeviceSecret(payload.deviceSecret);
  const scope = payload.scope === "friends" ? "friends" : "global";
  const viewer = await verifyUser(env, userId, deviceSecret);
  await env.DB.prepare("UPDATE users SET last_seen_at = CURRENT_TIMESTAMP WHERE id = ?").bind(userId).run();
  const usage = await getR2Usage(env);
  await maybeNotifyR2Usage(env, usage);
  return json({ feed: await getFeed(request, env, userId, scope), r2Usage: viewer.friendCode === R2_OWNER_FRIEND_CODE ? usage : undefined });
}

async function getOwnedPostImage(env: Env, userId: string, postId: string) {
  const row = await env.DB.prepare("SELECT user_id AS userId, image_key AS imageKey, image_size_bytes AS imageSizeBytes FROM feed_posts WHERE id = ?")
    .bind(postId)
    .first<{ userId: string; imageKey: string | null; imageSizeBytes: number }>();
  if (!row) throw new Response("Feed post not found.", { status: 404, headers: corsHeaders });
  if (row.userId !== userId) throw new Response("You can only edit your own post image.", { status: 403, headers: corsHeaders });
  return row;
}

async function handleFeedImageUpload(request: Request, env: Env) {
  const form = await request.formData();
  const userId = String(form.get("userId") ?? "").trim();
  const deviceSecret = String(form.get("deviceSecret") ?? "");
  const postId = cleanText(form.get("postId"), 80);
  const owner = await verifyUser(env, userId, deviceSecret);

  const existing = await getOwnedPostImage(env, userId, postId);
  const image = form.get("image");
  if (!(image instanceof File)) return text("Missing image.", 400);
  if (!feedImageTypes.has(image.type)) return text("Use PNG, JPEG, WebP, or GIF images.", 400);
  if (image.size > MAX_FEED_IMAGE_BYTES) return text("Image is too large. Use an image under 5 MB.", 413);

  const classAOps = existing.imageKey ? 2 : 1;
  const usage = await assertR2ClassABudget(env, classAOps);
  const previousBytes = Number(existing.imageSizeBytes ?? 0);
  const nextStorageBytes = usage.storageBytes - previousBytes + image.size;
  if (nextStorageBytes > R2_STORAGE_HARD_BYTES) {
    return text("Image uploads are paused to keep R2 storage below the free tier.", 429);
  }

  const extension = image.type === "image/png" ? "png" : image.type === "image/webp" ? "webp" : image.type === "image/gif" ? "gif" : "jpg";
  const key = `feed-posts/${postId}/${crypto.randomUUID()}.${extension}`;
  await env.FEED_IMAGES.put(key, image.stream(), {
    httpMetadata: { contentType: image.type },
  });
  if (existing.imageKey) await env.FEED_IMAGES.delete(existing.imageKey).catch(() => undefined);
  await incrementR2Usage(env, { classA: classAOps });

  const expiresAt = imageExpiresAt();
  await env.DB.prepare(`
    UPDATE feed_posts
    SET image_key = ?, image_mime_type = ?, image_expires_at = ?, image_expired_at = NULL, image_size_bytes = ?
    WHERE id = ? AND user_id = ?
  `).bind(key, image.type, expiresAt, image.size, postId, userId).run();
  await env.DB.prepare(`
    INSERT INTO r2_usage_monthly (month, storage_bytes, updated_at)
    VALUES (?, ?, CURRENT_TIMESTAMP)
    ON CONFLICT(month) DO UPDATE SET storage_bytes = ?, updated_at = CURRENT_TIMESTAMP
  `).bind(usageMonth(), nextStorageBytes, nextStorageBytes).run();

  const nextUsage = await getR2Usage(env);
  await maybeNotifyR2Usage(env, nextUsage);
  return json({
    ok: true,
    imageKey: key,
    imageMimeType: image.type,
    imageExpiresAt: expiresAt,
    imageExpiredAt: null,
    imageUrl: feedImageUrl(request, key),
    r2Usage: owner.friendCode === R2_OWNER_FRIEND_CODE ? nextUsage : undefined,
  });
}

async function handleFeedImageDelete(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; postId: string }>(request);
  const userId = String(payload.userId ?? "").trim();
  const owner = await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const postId = cleanText(payload.postId, 80);
  const existing = await getOwnedPostImage(env, userId, postId);
  if (existing.imageKey) await assertR2ClassABudget(env, 1);
  if (existing.imageKey) await env.FEED_IMAGES.delete(existing.imageKey).catch(() => undefined);
  if (existing.imageKey) await incrementR2Usage(env, { classA: 1 });
  const usage = await getR2Usage(env);
  const nextStorageBytes = Math.max(0, usage.storageBytes - Number(existing.imageSizeBytes ?? 0));
  await env.DB.prepare(`
    UPDATE feed_posts
    SET image_key = NULL, image_mime_type = NULL, image_expires_at = NULL, image_expired_at = NULL, image_size_bytes = 0
    WHERE id = ? AND user_id = ?
  `).bind(postId, userId).run();
  await env.DB.prepare(`
    INSERT INTO r2_usage_monthly (month, storage_bytes, updated_at)
    VALUES (?, ?, CURRENT_TIMESTAMP)
    ON CONFLICT(month) DO UPDATE SET storage_bytes = ?, updated_at = CURRENT_TIMESTAMP
  `).bind(usageMonth(), nextStorageBytes, nextStorageBytes).run();
  const nextUsage = await getR2Usage(env);
  await maybeNotifyR2Usage(env, nextUsage);
  return json({ ok: true, r2Usage: owner.friendCode === R2_OWNER_FRIEND_CODE ? nextUsage : undefined });
}

async function handleFeedImageGet(request: Request, env: Env, key: string) {
  await assertR2ClassBBudget(env);
  const post = await env.DB.prepare("SELECT image_mime_type AS imageMimeType FROM feed_posts WHERE image_key = ? AND image_expires_at > ?")
    .bind(key, new Date().toISOString())
    .first<{ imageMimeType: string | null }>();
  if (!post) return text("Image not found.", 404);

  const object = await env.FEED_IMAGES.get(key);
  if (!object) return text("Image not found.", 404);
  await incrementR2Usage(env, { classB: 1 });
  await maybeNotifyR2Usage(env);
  return new Response(object.body, {
    headers: {
      ...corsHeaders,
      "content-type": post.imageMimeType || object.httpMetadata?.contentType || "application/octet-stream",
      "cache-control": "public, max-age=3600",
    },
  });
}

// Origin used for avatar URLs when no request is in hand — the scheduled() cron backfill has no
// incoming request to derive an origin from. Matches the img-src entry in the desktop app's CSP
// (desktop/src-tauri/tauri.conf.json) and the worker's real *.workers.dev host.
const WORKER_ORIGIN = "https://study-tracker-social.danil-poluyanov13.workers.dev";

function profileAvatarUrl(origin: string, key: string) {
  return `${origin}/profile/avatar/${encodeURIComponent(key)}`;
}

function profileAvatarKeyFromUrl(value: string) {
  try {
    const url = new URL(value);
    if (!url.pathname.startsWith("/profile/avatar/")) return null;
    return decodeURIComponent(url.pathname.slice("/profile/avatar/".length));
  } catch {
    return null;
  }
}

function profileAvatarBytesFromDataUrl(value: string) {
  const match = value.match(/^data:(image\/(?:png|jpeg|webp|gif));base64,(.+)$/);
  if (!match) return null;
  const binary = atob(match[2]);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return { mimeType: match[1], bytes };
}

// Shared by the direct upload endpoint and the sync-time auto-upload path (upsertUser): puts
// image bytes into R2 under a fresh key, deletes the previous key if replacing, and returns the
// resulting photo avatar. Returns null (never throws) when the R2 budget/storage cap would be
// exceeded, so callers can fall back instead of hard-failing.
async function uploadAvatarBytesToR2(
  env: Env,
  origin: string,
  userId: string,
  image: { mimeType: string; bytes: Uint8Array },
  name: string,
  previousAvatar: SocialAvatar | null,
  previousSizeBytes: number,
): Promise<SocialAvatar | null> {
  if (image.bytes.byteLength > MAX_PROFILE_AVATAR_BYTES) return null;
  const previousKey = previousAvatar?.kind === "photo" ? profileAvatarKeyFromUrl(previousAvatar.url) : null;
  const classAOps = previousKey ? 2 : 1;
  let usage: Awaited<ReturnType<typeof getR2Usage>>;
  try {
    usage = await assertR2ClassABudget(env, classAOps);
  } catch {
    return null;
  }
  const nextStorageBytes = usage.storageBytes - previousSizeBytes + image.bytes.byteLength;
  if (nextStorageBytes > R2_STORAGE_HARD_BYTES) return null;
  const key = `profile-avatars/${userId}/${crypto.randomUUID()}.webp`;
  await env.FEED_IMAGES.put(key, image.bytes, { httpMetadata: { contentType: image.mimeType } });
  await incrementR2Usage(env, { classA: classAOps });
  if (previousKey) await env.FEED_IMAGES.delete(previousKey).catch(() => undefined);
  return { kind: "photo", name, url: profileAvatarUrl(origin, key), mimeType: image.mimeType };
}

async function handleProfileAvatarUpload(request: Request, env: Env) {
  const form = await request.formData();
  const userId = String(form.get("userId") ?? "").trim();
  const deviceSecret = String(form.get("deviceSecret") ?? "");
  await verifyUser(env, userId, deviceSecret);

  const image = form.get("image");
  if (!(image instanceof File)) return text("Missing avatar image.", 400);
  if (!feedImageTypes.has(image.type)) return text("Use PNG, JPEG, WebP, or GIF images.", 400);
  if (image.size > MAX_PROFILE_AVATAR_BYTES) return text("Avatar image is too large.", 413);

  const existing = await env.DB.prepare("SELECT avatar_json AS avatarJson, avatar_size_bytes AS avatarSizeBytes FROM users WHERE id = ?")
    .bind(userId)
    .first<{ avatarJson: string; avatarSizeBytes: number }>();
  const previousAvatar = parseAvatar(existing?.avatarJson, "Student");
  const bytes = new Uint8Array(await image.arrayBuffer());
  const avatar = await uploadAvatarBytesToR2(
    env,
    new URL(request.url).origin,
    userId,
    { mimeType: image.type, bytes },
    cleanText(form.get("name"), 180) || "photo",
    previousAvatar,
    Number(existing?.avatarSizeBytes ?? 0),
  );
  if (!avatar) return text("Image uploads are paused to keep R2 storage below the free tier.", 429);

  await env.DB.prepare("UPDATE users SET avatar_json = ?, avatar_size_bytes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
    .bind(JSON.stringify(avatar), bytes.byteLength, userId)
    .run();
  return json({ avatar });
}

async function handleProfileAvatarGet(env: Env, key: string) {
  await assertR2ClassBBudget(env);
  const object = await env.FEED_IMAGES.get(key);
  if (!object) return text("Avatar not found.", 404);
  await incrementR2Usage(env, { classB: 1 });
  await maybeNotifyR2Usage(env);
  return new Response(object.body, {
    headers: {
      ...corsHeaders,
      "content-type": object.httpMetadata?.contentType ?? "image/webp",
      "cache-control": "public, max-age=86400",
    },
  });
}

// Backfills users whose avatar_json is still stuck as a raw base64 data: URI (set before the R2
// upload path existed, or before it ever ran for them — see resolveAvatarForSync, which now
// prevents new occurrences going forward). Safe to re-run: only touches rows still matching the
// LIKE filter, ordered by updated_at so repeated calls make steady progress through the backlog.
// Shared by the owner-only admin endpoint and the nightly cron in scheduled().
async function migrateStuckProfileAvatars(env: Env, origin: string, limit: number) {
  const rows = await env.DB.prepare(`
    SELECT id AS userId, display_name AS displayName, avatar_json AS avatarJson
    FROM users
    WHERE avatar_json LIKE '%data:image/%'
    ORDER BY updated_at ASC
    LIMIT ?
  `).bind(limit).all<{ userId: string; displayName: string; avatarJson: string }>();

  let migrated = 0;
  const skipped: Array<{ userId: string; reason: string }> = [];
  for (const row of rows.results) {
    const avatar = parseAvatar(row.avatarJson, row.displayName);
    if (avatar.kind !== "photo" || !avatar.url.startsWith("data:image/")) {
      skipped.push({ userId: row.userId, reason: "not a base64 photo avatar" });
      continue;
    }
    const image = profileAvatarBytesFromDataUrl(avatar.url);
    if (!image) {
      skipped.push({ userId: row.userId, reason: "unsupported data URL" });
      continue;
    }
    const uploaded = await uploadAvatarBytesToR2(env, origin, row.userId, image, avatar.name || "photo", avatar, 0);
    if (!uploaded) {
      skipped.push({ userId: row.userId, reason: "upload failed or R2 budget exceeded" });
      continue;
    }
    await env.DB.prepare("UPDATE users SET avatar_json = ?, avatar_size_bytes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
      .bind(JSON.stringify(uploaded), image.bytes.byteLength, row.userId)
      .run();
    migrated += 1;
  }

  return { scanned: rows.results.length, migrated, skipped };
}

async function handleAdminMigrateProfileAvatars(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; limit?: number }>(request);
  const owner = await verifyUser(env, cleanUserId(payload.userId), String(payload.deviceSecret ?? ""));
  if (owner.friendCode !== R2_OWNER_FRIEND_CODE) return text("Only the app owner can migrate profile avatars.", 403);

  const limit = Math.max(1, cleanFiniteNumber(payload.limit ?? 5, "limit", 200));
  const result = await migrateStuckProfileAvatars(env, new URL(request.url).origin, limit);
  return json({ ok: true, ...result });
}

/**
 * Batch-hashes device_secret for rows that haven't synced recently and so never hit the lazy
 * backfill in verifyDeviceSecret(). Safe to re-run: only ever touches device_secret_hash IS NULL
 * rows, updated_at ordering means repeated calls make steady progress through the backlog.
 */
async function handleAdminMigrateDeviceSecrets(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; limit?: number }>(request);
  const owner = await verifyUser(env, cleanUserId(payload.userId), String(payload.deviceSecret ?? ""));
  if (owner.friendCode !== R2_OWNER_FRIEND_CODE) return text("Only the app owner can migrate device secrets.", 403);
  if (!env.DEVICE_SECRET_HASH_SECRET) return text("DEVICE_SECRET_HASH_SECRET is not configured.", 500);

  const limit = Math.max(1, cleanFiniteNumber(payload.limit ?? 25, "limit", 500));
  const rows = await env.DB.prepare(`
    SELECT id AS userId, device_secret AS deviceSecret
    FROM users
    WHERE device_secret_hash IS NULL
    ORDER BY updated_at ASC
    LIMIT ?
  `).bind(limit).all<{ userId: string; deviceSecret: string }>();

  let migrated = 0;
  for (const row of rows.results) {
    const hash = await hashDeviceSecret(env, row.deviceSecret);
    if (!hash) continue; // defensive only — guarded above by the DEVICE_SECRET_HASH_SECRET check
    await env.DB.prepare("UPDATE users SET device_secret_hash = ? WHERE id = ?").bind(hash, row.userId).run();
    migrated += 1;
  }

  return json({ ok: true, scanned: rows.results.length, migrated });
}

async function handleFeedReaction(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; postId: string; emoji: string }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const postId = cleanText(payload.postId, 80);
  const visibility = await canViewFeedPost(env, userId, postId);
  if (visibility.missing) return text("Feed post not found.", 404);
  if (!visibility.allowed) return text("You cannot react to this feed post.", 403);

  const requestedEmoji = cleanText(payload.emoji, 8);
  const emoji = requestedEmoji && [...requestedEmoji].length <= 4 ? requestedEmoji : "fire";
  const existing = await env.DB.prepare("SELECT 1 FROM feed_reactions WHERE post_id = ? AND user_id = ? AND emoji = ?").bind(postId, userId, emoji).first();
  if (existing) {
    await env.DB.prepare("DELETE FROM feed_reactions WHERE post_id = ? AND user_id = ? AND emoji = ?").bind(postId, userId, emoji).run();
  } else {
    await env.DB.prepare("INSERT OR IGNORE INTO feed_reactions (post_id, user_id, emoji) VALUES (?, ?, ?)").bind(postId, userId, emoji).run();
  }
  return json({ ok: true });
}

async function getFeedPoll(env: Env, userId: string, postId: string) {
  const pollRow = await env.DB.prepare("SELECT question, multiple FROM feed_polls WHERE post_id = ?")
    .bind(postId)
    .first<{ question: string; multiple: number }>();
  if (!pollRow) return null;
  const optionRows = await env.DB.prepare(`
    SELECT o.id, o.text, COUNT(v.user_id) AS votes,
      MAX(CASE WHEN v.user_id = ? THEN 1 ELSE 0 END) AS selected
    FROM feed_poll_options o
    LEFT JOIN feed_poll_votes v ON v.option_id = o.id
    WHERE o.post_id = ?
    GROUP BY o.id, o.text, o.sort_order
    ORDER BY o.sort_order ASC
  `).bind(userId, postId).all<{ id: string; text: string; votes: number; selected: number }>();
  const options = optionRows.results.map((row) => ({
    id: row.id,
    text: row.text,
    votes: Number(row.votes ?? 0),
    selected: Boolean(row.selected),
  }));
  return {
    question: pollRow.question,
    multiple: Boolean(pollRow.multiple),
    options,
    totalVotes: options.reduce((sum, option) => sum + option.votes, 0),
  };
}

async function handleFeedPollVote(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; postId: string; optionId: string }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const postId = cleanText(payload.postId, 80);
  const optionId = cleanText(payload.optionId, 80);
  const visibility = await canViewFeedPost(env, userId, postId);
  if (visibility.missing) return text("Feed post not found.", 404);
  if (!visibility.allowed) return text("You cannot vote on this poll.", 403);

  const option = await env.DB.prepare(`
    SELECT o.id, p.multiple
    FROM feed_poll_options o
    JOIN feed_polls p ON p.post_id = o.post_id
    WHERE o.post_id = ? AND o.id = ?
  `).bind(postId, optionId).first<{ id: string; multiple: number }>();
  if (!option) return text("Poll option not found.", 404);

  const existing = await env.DB.prepare("SELECT 1 FROM feed_poll_votes WHERE option_id = ? AND user_id = ?")
    .bind(optionId, userId)
    .first();
  if (existing) {
    await env.DB.prepare("DELETE FROM feed_poll_votes WHERE option_id = ? AND user_id = ?").bind(optionId, userId).run();
  } else {
    if (!option.multiple) await env.DB.prepare("DELETE FROM feed_poll_votes WHERE post_id = ? AND user_id = ?").bind(postId, userId).run();
    await env.DB.prepare("INSERT OR IGNORE INTO feed_poll_votes (post_id, option_id, user_id) VALUES (?, ?, ?)").bind(postId, optionId, userId).run();
  }

  return json({ ok: true, poll: await getFeedPoll(env, userId, postId) });
}

async function handleFeedUpdate(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; postId: string; note: string }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));

  const postId = cleanText(payload.postId, 80);
  const existing = await env.DB.prepare("SELECT user_id AS userId FROM feed_posts WHERE id = ?").bind(postId).first<{ userId: string }>();
  if (!existing) return text("Feed post not found.", 404);
  if (existing.userId !== userId) return text("You can only edit your own posts.", 403);

  await env.DB.prepare("UPDATE feed_posts SET note = ? WHERE id = ? AND user_id = ?")
    .bind(cleanText(payload.note, 220), postId, userId)
    .run();
  return json({ ok: true });
}

async function handleFeedDelete(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; postId: string }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));

  const postId = cleanText(payload.postId, 80);
  const existing = await env.DB.prepare("SELECT user_id AS userId, image_key AS imageKey, image_size_bytes AS imageSizeBytes FROM feed_posts WHERE id = ?").bind(postId).first<{ userId: string; imageKey: string | null; imageSizeBytes: number }>();
  if (!existing) return text("Feed post not found.", 404);
  if (existing.userId !== userId) return text("You can only delete your own posts.", 403);

  if (existing.imageKey) await assertR2ClassABudget(env, 1);
  if (existing.imageKey) await env.FEED_IMAGES.delete(existing.imageKey).catch(() => undefined);
  if (existing.imageKey) {
    await incrementR2Usage(env, { classA: 1 });
    const usage = await getR2Usage(env);
    const nextStorageBytes = Math.max(0, usage.storageBytes - Number(existing.imageSizeBytes ?? 0));
    await env.DB.prepare(`
      INSERT INTO r2_usage_monthly (month, storage_bytes, updated_at)
      VALUES (?, ?, CURRENT_TIMESTAMP)
      ON CONFLICT(month) DO UPDATE SET storage_bytes = ?, updated_at = CURRENT_TIMESTAMP
    `).bind(usageMonth(), nextStorageBytes, nextStorageBytes).run();
    await maybeNotifyR2Usage(env);
  }
  await env.DB.prepare("DELETE FROM feed_posts WHERE id = ? AND user_id = ?").bind(postId, userId).run();
  return json({ ok: true });
}

async function handleFeedComment(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; postId: string; body: string }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const postId = cleanText(payload.postId, 80);
  const visibility = await canViewFeedPost(env, userId, postId);
  if (visibility.missing) return text("Feed post not found.", 404);
  if (!visibility.allowed) return text("You cannot comment on this feed post.", 403);

  const body = cleanText(payload.body, MAX_COMMENT_LENGTH);
  if (!body) return text("Comment cannot be empty.", 400);

  const id = crypto.randomUUID();
  await env.DB.prepare("INSERT INTO feed_comments (id, post_id, user_id, body) VALUES (?, ?, ?, ?)")
    .bind(id, postId, userId, body)
    .run();
  const row = await env.DB.prepare(`
    SELECT c.id, c.post_id AS postId, c.user_id AS userId, u.display_name AS displayName, u.friend_code AS friendCode,
      u.avatar_json AS avatarJson, c.body, c.created_at AS createdAt
    FROM feed_comments c
    JOIN users u ON u.id = c.user_id
    WHERE c.id = ?
  `).bind(id).first<{ id: string; postId: string; userId: string; displayName: string; friendCode: string; avatarJson: string; body: string; createdAt: string }>();
  if (!row) return text("Comment could not be created.", 500);

  return json({
    ok: true,
    comment: {
      id: row.id,
      postId: row.postId,
      userId: row.userId,
      displayName: row.displayName,
      friendCode: row.friendCode,
      avatar: parseListAvatar(row.avatarJson, row.displayName),
      body: row.body,
      createdAt: row.createdAt,
      isSelf: true,
    },
  });
}

async function handleFriendRequest(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; friendCode: string }>(request);
  const fromUserId = String(payload.userId ?? "").trim();
  await verifyUser(env, fromUserId, String(payload.deviceSecret ?? ""));

  const target = await env.DB.prepare("SELECT id FROM users WHERE friend_code = ?").bind(cleanCode(payload.friendCode)).first<{ id: string }>();
  if (!target) return text("No user with that friend code exists.", 404);
  if (target.id === fromUserId) return text("You cannot add yourself.", 400);

  const [userLow, userHigh] = friendPair(fromUserId, target.id);
  const existingFriendship = await env.DB.prepare("SELECT 1 FROM friendships WHERE user_low = ? AND user_high = ?").bind(userLow, userHigh).first();
  if (existingFriendship) return text("You are already friends.", 409);

  const reciprocalRequest = await env.DB.prepare("SELECT id FROM friend_requests WHERE from_user_id = ? AND to_user_id = ? AND status = 'pending'")
    .bind(target.id, fromUserId)
    .first<{ id: string }>();
  if (reciprocalRequest) {
    await env.DB.batch([
      env.DB.prepare("UPDATE friend_requests SET status = 'accepted', responded_at = CURRENT_TIMESTAMP WHERE id = ?").bind(reciprocalRequest.id),
      env.DB.prepare("INSERT OR IGNORE INTO friendships (user_low, user_high) VALUES (?, ?)").bind(userLow, userHigh),
    ]);
    return json(await getSocialSnapshot(request, env, fromUserId));
  }

  const id = crypto.randomUUID();
  await env.DB.prepare("INSERT INTO friend_requests (id, from_user_id, to_user_id, status) VALUES (?, ?, ?, 'pending') ON CONFLICT(from_user_id, to_user_id) DO UPDATE SET status = 'pending', responded_at = NULL")
    .bind(id, fromUserId, target.id)
    .run();
  return json(await getSocialSnapshot(request, env, fromUserId));
}

async function handlePresence(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; app?: AppMetadata }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const app = cleanAppMetadata(payload.app);
  await env.DB.prepare(`
    UPDATE users
    SET last_seen_at = CURRENT_TIMESTAMP,
      app_version = COALESCE(NULLIF(?, ''), app_version),
      app_platform = COALESCE(NULLIF(?, ''), app_platform),
      app_runtime_channel = COALESCE(NULLIF(?, ''), app_runtime_channel),
      app_seen_at = CASE WHEN ? != '' OR ? != '' OR ? != '' THEN CURRENT_TIMESTAMP ELSE app_seen_at END
    WHERE id = ?
  `).bind(app.version, app.platform, app.runtimeChannel, app.version, app.platform, app.runtimeChannel, userId).run();
  return json({ ok: true });
}

async function handleTelemetryHeartbeat(request: Request, env: Env) {
  const payload = await readJson<{ installId: string; app?: AppMetadata }>(request);
  const installId = requiredText(payload.installId, "installId", MAX_ID_LENGTH);
  const app = cleanAppMetadata(payload.app);
  await env.DB.prepare(`
    INSERT INTO app_telemetry_installs (install_id, app_version, app_platform, app_runtime_channel, last_seen_at)
    VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
    ON CONFLICT(install_id) DO UPDATE SET
      app_version = COALESCE(NULLIF(excluded.app_version, ''), app_telemetry_installs.app_version),
      app_platform = COALESCE(NULLIF(excluded.app_platform, ''), app_telemetry_installs.app_platform),
      app_runtime_channel = COALESCE(NULLIF(excluded.app_runtime_channel, ''), app_telemetry_installs.app_runtime_channel),
      last_seen_at = CURRENT_TIMESTAMP
  `).bind(installId, app.version, app.platform, app.runtimeChannel).run();
  return json({ ok: true });
}

async function handleFriendStatus(request: Request, env: Env, includeCaches = true) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string }>(request);
  const userId = cleanUserId(payload.userId);
  const deviceSecret = cleanDeviceSecret(payload.deviceSecret);
  await verifyUser(env, userId, deviceSecret);
  await env.DB.prepare("UPDATE users SET last_seen_at = CURRENT_TIMESTAMP WHERE id = ?").bind(userId).run();
  return json(await getSocialSnapshot(request, env, userId, includeCaches));
}

async function handleLeaderboard(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; scope?: LeaderboardScope; period?: LeaderboardPeriod }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const scope = payload.scope === "friends" || payload.scope === "squad" ? payload.scope : "global";
  const period = payload.period === "daily" || payload.period === "overall" ? payload.period : "weekly";
  return json({ entries: await getLeaderboard(env, userId, scope, period) });
}

async function handleSquadScoreboard(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; period?: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const period: SquadScorePeriod = payload.period === "daily" || payload.period === "overall" ? payload.period : "season";
  const cacheKey = new Request(`${new URL(request.url).origin}/squads/scoreboard?period=${period}`);
  const cache = await caches.open("default");
  const cached = await cache.match(cacheKey);
  if (cached) return cached;

  const response = json({ entries: await getSquadScoreLeaderboard(env, period) });
  response.headers.set("cache-control", "public, max-age=30");
  await cache.put(cacheKey, response.clone());
  return response;
}

async function handlePlayerStats(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; targetUserId: string }>(request);
  const userId = cleanUserId(payload.userId);
  const deviceSecret = cleanDeviceSecret(payload.deviceSecret);
  const targetUserId = cleanUserId(payload.targetUserId);
  await verifyUser(env, userId, deviceSecret);

  const targetUser = await env.DB.prepare("SELECT id, display_name AS displayName, friend_code AS friendCode, avatar_json AS avatarJson, last_seen_at AS lastSeenAt, show_hours_to_friends AS showHoursToFriends FROM users WHERE id = ?")
    .bind(targetUserId).first<{ id: string; displayName: string; friendCode: string; avatarJson: string; lastSeenAt: string | null; showHoursToFriends: number }>();
  if (!targetUser) return text("User not found.", 404);

  const [userLow, userHigh] = friendPair(userId, targetUserId);
  const areFriends = await env.DB.prepare("SELECT 1 FROM friendships WHERE user_low = ? AND user_high = ?").bind(userLow, userHigh).first();
  if (!areFriends && targetUserId !== userId) {
    return text("User is private.", 403);
  }
  const hoursVisible = targetUserId === userId || !areFriends || Boolean(targetUser.showHoursToFriends);

  async function getStats(period: LeaderboardPeriod) {
    if (period === "overall") {
      const row = await env.DB.prepare(`
        SELECT totals.minutes, totals.sessions,
          (SELECT MAX(date) FROM competitive_daily_stats WHERE user_id = users.id) AS lastActiveDate
        FROM users
        JOIN competitive_user_totals totals ON totals.user_id = users.id
        WHERE users.id = ?
      `).bind(targetUserId).first<{ minutes: number; sessions: number; lastActiveDate: string | null }>();
      return row ?? { minutes: 0, sessions: 0, lastActiveDate: null };
    }
    const periodFilter = leaderboardWhere(period);
    const clauses = [`u.id = ?`];
    const params: string[] = [targetUserId, ...periodFilter.params];
    if (periodFilter.clause) clauses.push(periodFilter.clause.replace(/^WHERE\s+/, ""));
    const row = await env.DB.prepare(`
      SELECT COALESCE(SUM(ds.minutes), 0) AS minutes, COALESCE(SUM(ds.sessions), 0) AS sessions, MAX(ds.date) AS lastActiveDate
      FROM users u
      LEFT JOIN competitive_daily_stats ds ON ds.user_id = u.id
      WHERE ${clauses.join(" AND ")}
      GROUP BY u.id
    `).bind(...params).first<{ minutes: number; sessions: number; lastActiveDate: string | null }>();
    return row ?? { minutes: 0, sessions: 0, lastActiveDate: null };
  }

  return json({
    displayName: targetUser.displayName,
    friendCode: targetUser.friendCode,
    avatar: parseAvatar(targetUser.avatarJson, targetUser.displayName),
    lastSeenAt: targetUser.lastSeenAt,
    hoursVisible,
    daily: hoursVisible ? await getStats("daily") : null,
    weekly: hoursVisible ? await getStats("weekly") : null,
    overall: hoursVisible ? await getStats("overall") : null,
  });
}

async function handleFriendResponse(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; requestId: string; response: "accepted" | "declined" }>(request);
  const userId = String(payload.userId ?? "").trim();
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));

  const friendRequest = await env.DB.prepare("SELECT id, from_user_id, to_user_id FROM friend_requests WHERE id = ? AND to_user_id = ? AND status = 'pending'")
    .bind(String(payload.requestId ?? ""), userId)
    .first<{ id: string; from_user_id: string; to_user_id: string }>();
  if (!friendRequest) return text("Friend request not found.", 404);

  const response = payload.response === "accepted" ? "accepted" : "declined";
  await env.DB.prepare("UPDATE friend_requests SET status = ?, responded_at = CURRENT_TIMESTAMP WHERE id = ?").bind(response, friendRequest.id).run();

  if (response === "accepted") {
    const [userLow, userHigh] = friendPair(friendRequest.from_user_id, friendRequest.to_user_id);
    await env.DB.prepare("INSERT OR IGNORE INTO friendships (user_low, user_high) VALUES (?, ?)").bind(userLow, userHigh).run();
  }

  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadCreate(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; name: string; isPrivate?: boolean }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const existing = await getUserSquadMember(env, userId);
  if (existing) return text("Leave your current squad before creating a new one.", 409);

  const squadId = crypto.randomUUID();
  await env.DB.batch([
    env.DB.prepare("INSERT INTO squads (id, name, is_private, created_by_user_id) VALUES (?, ?, ?, ?)")
      .bind(squadId, cleanSquadName(payload.name), payload.isPrivate ? 1 : 0, userId),
    env.DB.prepare("INSERT INTO squad_members (squad_id, user_id, role) VALUES (?, ?, 'leader')")
      .bind(squadId, userId),
    env.DB.prepare("INSERT INTO squad_member_history (id, squad_id, user_id, role, joined_at) VALUES (?, ?, ?, 'leader', CURRENT_TIMESTAMP)")
      .bind(crypto.randomUUID(), squadId, userId),
  ]);
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadSearch(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; query?: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const query = `%${cleanText(payload.query, 48)}%`;
  const userMembership = await getUserSquadMember(env, userId);
  const pendingRows = await env.DB.prepare("SELECT squad_id AS squadId FROM squad_join_requests WHERE user_id = ? AND status = 'pending'")
    .bind(userId)
    .all<{ squadId: string }>();
  const pendingIds = new Set(pendingRows.results.map((row) => row.squadId));
  const rows = await env.DB.prepare(`
    SELECT s.id, s.name, s.is_private AS isPrivate, s.created_at AS createdAt,
      COUNT(DISTINCT sm.user_id) AS memberCount,
      COALESCE(SUM(ds.minutes), 0) AS totalMinutes,
      COALESCE(SUM(ds.sessions), 0) AS totalSessions
    FROM squads s
    LEFT JOIN squad_members sm ON sm.squad_id = s.id
    LEFT JOIN competitive_daily_stats ds ON ds.user_id = sm.user_id
    WHERE s.name LIKE ?
    GROUP BY s.id, s.name, s.is_private, s.created_at
    ORDER BY memberCount DESC, totalMinutes DESC, s.name ASC
    LIMIT 30
  `).bind(query).all<{ id: string; name: string; isPrivate: number; createdAt: string; memberCount: number; totalMinutes: number; totalSessions: number }>();

  return json({
    squads: rows.results.map((squad) => {
      const memberCount = Number(squad.memberCount);
      const action = userMembership ? "unavailable" : memberCount >= MAX_SQUAD_MEMBERS ? "full" : pendingIds.has(squad.id) ? "pending" : squad.isPrivate ? "request" : "join";
      return {
        ...squad,
        isPrivate: Boolean(squad.isPrivate),
        memberCount,
        maxMembers: MAX_SQUAD_MEMBERS,
        totalMinutes: Number(squad.totalMinutes),
        totalSessions: Number(squad.totalSessions),
        action,
      };
    }),
  });
}

async function handleSquadDetails(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string; squadId: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, cleanDeviceSecret(payload.deviceSecret));
  const squadId = requiredText(payload.squadId, "squadId", MAX_ID_LENGTH);
  const squad = await getSquadDetails(env, userId, squadId);
  if (!squad) return text("Squad not found.", 404);
  return json({ squad });
}

async function handleSquadJoin(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; squadId: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  if (await getUserSquadMember(env, userId)) return text("Leave your current squad before joining another one.", 409);
  const squadId = requiredText(payload.squadId, "squadId", MAX_ID_LENGTH);
  const squad = await env.DB.prepare("SELECT id, is_private AS isPrivate FROM squads WHERE id = ?").bind(squadId).first<{ id: string; isPrivate: number }>();
  if (!squad) return text("Squad not found.", 404);
  if (await getSquadMemberCount(env, squadId) >= MAX_SQUAD_MEMBERS) return text("That squad is already full.", 409);

  if (squad.isPrivate) {
    const id = crypto.randomUUID();
    await env.DB.prepare(`
      INSERT INTO squad_join_requests (id, squad_id, user_id, status)
      VALUES (?, ?, ?, 'pending')
      ON CONFLICT(squad_id, user_id) DO UPDATE SET status = 'pending', responded_at = NULL, responded_by_user_id = NULL
    `).bind(id, squadId, userId).run();
  } else {
    const [insert] = await env.DB.batch([
      env.DB.prepare(`
      INSERT INTO squad_members (squad_id, user_id, role)
      SELECT ?, ?, 'member'
      WHERE (SELECT COUNT(*) FROM squad_members WHERE squad_id = ?) < ?
        AND NOT EXISTS (SELECT 1 FROM squad_members WHERE user_id = ?)
      `).bind(squadId, userId, squadId, MAX_SQUAD_MEMBERS, userId),
      env.DB.prepare(`
        INSERT INTO squad_member_history (id, squad_id, user_id, role, joined_at)
        SELECT ?, ?, ?, 'member', CURRENT_TIMESTAMP
        WHERE EXISTS (SELECT 1 FROM squad_members WHERE squad_id = ? AND user_id = ?)
          AND NOT EXISTS (SELECT 1 FROM squad_member_history WHERE squad_id = ? AND user_id = ? AND left_at IS NULL)
      `).bind(crypto.randomUUID(), squadId, userId, squadId, userId, squadId, userId),
      env.DB.prepare("UPDATE squads SET updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(squadId),
    ]);
    if ((insert.meta?.changes ?? 0) < 1) return text("That squad is already full.", 409);
  }
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadRespond(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; requestId: string; response: "accepted" | "declined" }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const actor = await getUserSquadMember(env, userId);
  if (!actor || !canManageRequests(actor.role)) return text("You cannot manage squad requests.", 403);
  const joinRequest = await env.DB.prepare("SELECT id, squad_id AS squadId, user_id AS userId FROM squad_join_requests WHERE id = ? AND squad_id = ? AND status = 'pending'")
    .bind(String(payload.requestId ?? ""), actor.squadId)
    .first<{ id: string; squadId: string; userId: string }>();
  if (!joinRequest) return text("Squad request not found.", 404);

  const response = payload.response === "accepted" ? "accepted" : "declined";
  if (response === "accepted") {
    if (await getUserSquadMember(env, joinRequest.userId)) return text("That user is already in a squad.", 409);
    if (await getSquadMemberCount(env, actor.squadId) >= MAX_SQUAD_MEMBERS) return text("That squad is already full.", 409);
    const [insert] = await env.DB.batch([
      env.DB.prepare(`
      INSERT INTO squad_members (squad_id, user_id, role)
      SELECT ?, ?, 'member'
      WHERE (SELECT COUNT(*) FROM squad_members WHERE squad_id = ?) < ?
        AND NOT EXISTS (SELECT 1 FROM squad_members WHERE user_id = ?)
      `).bind(actor.squadId, joinRequest.userId, actor.squadId, MAX_SQUAD_MEMBERS, joinRequest.userId),
      env.DB.prepare(`
        INSERT INTO squad_member_history (id, squad_id, user_id, role, joined_at)
        SELECT ?, ?, ?, 'member', CURRENT_TIMESTAMP
        WHERE EXISTS (SELECT 1 FROM squad_members WHERE squad_id = ? AND user_id = ?)
          AND NOT EXISTS (SELECT 1 FROM squad_member_history WHERE squad_id = ? AND user_id = ? AND left_at IS NULL)
      `).bind(crypto.randomUUID(), actor.squadId, joinRequest.userId, actor.squadId, joinRequest.userId, actor.squadId, joinRequest.userId),
      env.DB.prepare(`
        UPDATE squad_join_requests
        SET status = 'accepted', responded_at = CURRENT_TIMESTAMP, responded_by_user_id = ?
        WHERE id = ?
          AND EXISTS (SELECT 1 FROM squad_members WHERE squad_id = ? AND user_id = ?)
      `).bind(userId, joinRequest.id, actor.squadId, joinRequest.userId),
      env.DB.prepare("UPDATE squads SET updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(actor.squadId),
    ]);
    if ((insert.meta?.changes ?? 0) < 1) return text("That squad is already full.", 409);
  } else {
    await env.DB.prepare("UPDATE squad_join_requests SET status = 'declined', responded_at = CURRENT_TIMESTAMP, responded_by_user_id = ? WHERE id = ?")
      .bind(userId, joinRequest.id)
      .run();
  }
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadLeave(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const membership = await getUserSquadMember(env, userId);
  if (!membership) return text("You are not in a squad.", 404);
  const count = await getSquadMemberCount(env, membership.squadId);
  if (count <= 1) {
    await closeSquadMemberHistory(env, membership.squadId, userId);
    await env.DB.batch([
      env.DB.prepare("DELETE FROM squad_messages WHERE squad_id = ?").bind(membership.squadId),
      env.DB.prepare("DELETE FROM squad_join_requests WHERE squad_id = ?").bind(membership.squadId),
      env.DB.prepare("DELETE FROM squad_members WHERE squad_id = ?").bind(membership.squadId),
      env.DB.prepare("DELETE FROM squads WHERE id = ?").bind(membership.squadId),
    ]);
    return json(await getSocialSnapshot(request, env, userId));
  }

  await closeSquadMemberHistory(env, membership.squadId, userId);
  await env.DB.prepare("DELETE FROM squad_members WHERE squad_id = ? AND user_id = ?").bind(membership.squadId, userId).run();
  if (membership.role === "leader") {
    const nextLeader = await env.DB.prepare(`
      SELECT user_id AS userId FROM squad_members
      WHERE squad_id = ?
      ORDER BY CASE role WHEN 'co_leader' THEN 1 WHEN 'elder' THEN 2 ELSE 3 END, joined_at ASC
      LIMIT 1
    `).bind(membership.squadId).first<{ userId: string }>();
    if (nextLeader) await env.DB.batch([
      env.DB.prepare("UPDATE squad_members SET role = 'leader' WHERE squad_id = ? AND user_id = ?").bind(membership.squadId, nextLeader.userId),
      env.DB.prepare("UPDATE squad_member_history SET role = 'leader' WHERE squad_id = ? AND user_id = ? AND left_at IS NULL").bind(membership.squadId, nextLeader.userId),
    ]);
  }
  await env.DB.prepare("UPDATE squads SET updated_at = CURRENT_TIMESTAMP WHERE id = ?").bind(membership.squadId).run();
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadChat(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; body: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const membership = await getUserSquadMember(env, userId);
  if (!membership) return text("Join a squad before chatting.", 403);
  const body = cleanText(payload.body, MAX_SQUAD_MESSAGE_LENGTH);
  if (!body) return text("Message cannot be empty.", 400);
  await env.DB.prepare("INSERT INTO squad_messages (id, squad_id, user_id, body) VALUES (?, ?, ?, ?)")
    .bind(crypto.randomUUID(), membership.squadId, userId, body)
    .run();
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadChatDelete(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; messageId: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const membership = await getUserSquadMember(env, userId);
  if (!membership) return text("Join a squad before managing chat.", 403);
  const messageId = requiredText(payload.messageId, "messageId", MAX_ID_LENGTH);
  const result = await env.DB.prepare("DELETE FROM squad_messages WHERE id = ? AND squad_id = ? AND user_id = ?")
    .bind(messageId, membership.squadId, userId)
    .run();
  if ((result.meta?.changes ?? 0) < 1) return text("Message not found.", 404);
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadRole(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; targetUserId: string; role: SquadRole }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const actor = await getUserSquadMember(env, userId);
  if (!actor) return text("You are not in a squad.", 404);
  const targetUserId = cleanUserId(payload.targetUserId);
  if (targetUserId === userId) return text("You cannot change your own rank.", 400);
  const target = await env.DB.prepare("SELECT role FROM squad_members WHERE squad_id = ? AND user_id = ?")
    .bind(actor.squadId, targetUserId)
    .first<{ role: SquadRole }>();
  if (!target) return text("Squad member not found.", 404);
  const nextRole = cleanSquadRole(payload.role);
  if (!canChangeRole(actor.role, target.role, nextRole)) return text("You cannot assign that rank.", 403);
  await env.DB.prepare("UPDATE squad_members SET role = ? WHERE squad_id = ? AND user_id = ?")
    .bind(nextRole, actor.squadId, targetUserId)
    .run();
  await env.DB.prepare("UPDATE squad_member_history SET role = ? WHERE squad_id = ? AND user_id = ? AND left_at IS NULL")
    .bind(nextRole, actor.squadId, targetUserId)
    .run();
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadKick(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; targetUserId: string }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const actor = await getUserSquadMember(env, userId);
  if (!actor) return text("You are not in a squad.", 404);
  const targetUserId = cleanUserId(payload.targetUserId);
  if (targetUserId === userId) return text("Use leave squad instead.", 400);
  const target = await env.DB.prepare("SELECT role FROM squad_members WHERE squad_id = ? AND user_id = ?")
    .bind(actor.squadId, targetUserId)
    .first<{ role: SquadRole }>();
  if (!target) return text("Squad member not found.", 404);
  if (!canKick(actor.role, target.role)) return text("You cannot kick that member.", 403);
  await closeSquadMemberHistory(env, actor.squadId, targetUserId);
  await env.DB.prepare("DELETE FROM squad_members WHERE squad_id = ? AND user_id = ?").bind(actor.squadId, targetUserId).run();
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleSquadSettings(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; name: string; isPrivate?: boolean }>(request);
  const userId = cleanUserId(payload.userId);
  await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  const actor = await getUserSquadMember(env, userId);
  if (!actor || actor.role !== "leader") return text("Only the squad leader can edit squad settings.", 403);
  await env.DB.prepare("UPDATE squads SET name = ?, is_private = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
    .bind(cleanSquadName(payload.name), payload.isPrivate ? 1 : 0, actor.squadId)
    .run();
  return json(await getSocialSnapshot(request, env, userId));
}

async function handleCurrentAnnouncement(request: Request, env: Env) {
  const now = new Date().toISOString();
  const params = new URL(request.url).searchParams;
  const appVersion = cleanText(params.get("appVersion"), 40);
  const announcements = await env.DB.prepare(`
    SELECT id, title, body, target_version AS targetVersion, created_at AS createdAt, expires_at AS expiresAt
    FROM app_announcements
    WHERE active = 1 AND (expires_at IS NULL OR expires_at > ?)
    ORDER BY created_at DESC
    LIMIT 20
  `).bind(now).all<{ id: string; title: string; body: string; targetVersion: string | null; createdAt: string; expiresAt: string | null }>();

  const announcement = announcements.results.find((item) => !item.targetVersion || isOlderVersion(appVersion, item.targetVersion));

  return json({ announcement: announcement ?? null });
}

async function handleCreateUpdateNotice(request: Request, env: Env) {
  const payload = await readJson<{ userId: string; deviceSecret: string; targetVersion?: string }>(request);
  const userId = cleanUserId(payload.userId);
  const owner = await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  if (owner.friendCode !== R2_OWNER_FRIEND_CODE) return text("Only the app owner can notify users about updates.", 403);
  const targetVersion = cleanText(payload.targetVersion, 40);
  if (!parseVersionParts(targetVersion)) return text("Missing valid target version.", 400);

  const id = `update-${targetVersion}-${Date.now()}`;
  await env.DB.prepare("INSERT INTO app_announcements (id, title, body, target_version, active) VALUES (?, ?, ?, ?, 1)")
    .bind(id, "New update available.", "Go to Settings to update the app.", targetVersion)
    .run();

  return json({ ok: true, id, targetVersion });
}

async function handleAdminUsage(request: Request, env: Env) {
  const payload = await readParamsOrJson<{ userId: string; deviceSecret: string }>(request);
  const userId = cleanUserId(payload.userId);
  const owner = await verifyUser(env, userId, String(payload.deviceSecret ?? ""));
  if (owner.friendCode !== R2_OWNER_FRIEND_CODE) return text("Only the app owner can view usage data.", 403);

  const [summary, users, telemetry, flaggedUsers, abuseEvents] = await Promise.all([
    env.DB.prepare(`
      SELECT COUNT(*) AS userCount,
        SUM(CASE WHEN last_seen_at >= datetime('now', '-1 day') THEN 1 ELSE 0 END) AS active24h,
        SUM(CASE WHEN last_seen_at >= datetime('now', '-7 days') THEN 1 ELSE 0 END) AS active7d,
        SUM(CASE WHEN app_version IS NOT NULL AND app_version != '' THEN 1 ELSE 0 END) AS usersWithAppVersion,
        SUM(CASE WHEN is_flagged = 1 THEN 1 ELSE 0 END) AS flaggedCount
      FROM users
    `).first(),
    env.DB.prepare(`
      SELECT u.display_name AS displayName, u.friend_code AS friendCode, u.last_seen_at AS lastSeenAt,
        u.device_label AS deviceLabel, u.device_fingerprint_hash AS deviceFingerprintHash,
        u.app_version AS appVersion, u.app_platform AS appPlatform,
        u.app_runtime_channel AS appRuntimeChannel, u.app_seen_at AS appSeenAt,
        u.is_flagged AS isFlagged, u.signup_country AS signupCountry, u.signup_asn AS signupAsn,
        u.signup_as_organization AS signupAsOrganization,
        (SELECT GROUP_CONCAT(reason) FROM user_flag_events WHERE user_id = u.id) AS flaggedReason
      FROM users u
      ORDER BY u.last_seen_at DESC
      LIMIT 100
    `).all(),
    env.DB.prepare(`
      SELECT install_id AS installId, app_version AS appVersion, app_platform AS appPlatform,
        app_runtime_channel AS appRuntimeChannel, created_at AS createdAt, last_seen_at AS lastSeenAt
      FROM app_telemetry_installs
      ORDER BY last_seen_at DESC
      LIMIT 100
    `).all(),
    env.DB.prepare(`
      SELECT u.display_name AS displayName, u.friend_code AS friendCode, u.last_seen_at AS lastSeenAt,
        u.signup_country AS signupCountry, u.signup_asn AS signupAsn, u.signup_as_organization AS signupAsOrganization,
        (SELECT GROUP_CONCAT(reason) FROM user_flag_events WHERE user_id = u.id) AS flaggedReason
      FROM users u
      WHERE u.is_flagged = 1
      ORDER BY u.last_seen_at DESC
      LIMIT 200
    `).all(),
    env.DB.prepare(`
      SELECT event_type AS eventType, country, as_organization AS asOrganization, path, user_agent AS userAgent,
        user_id AS userId, detail, created_at AS createdAt
      FROM abuse_events
      ORDER BY created_at DESC
      LIMIT 100
    `).all(),
  ]);
  // Note: signup_ip_hash is intentionally never selected here — it's for internal
  // rate-limit/flagging correlation only and must never leave the server.

  return json({ summary, users: users.results, telemetry: telemetry.results, flaggedUsers: flaggedUsers.results, abuseEvents: abuseEvents.results });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method === "OPTIONS") return new Response(null, { headers: corsHeaders });

    try {
      const url = new URL(request.url);
      if (HONEYPOT_PATHS.has(url.pathname)) return await handleHoneypot(request, env);
      if (request.method === "GET" && url.pathname === "/health") return json({ ok: true });
      if (request.method === "GET" && url.pathname === "/announcements/current") return await handleCurrentAnnouncement(request, env);
      if (request.method === "POST" && url.pathname === "/announcements/update-notice") return await handleCreateUpdateNotice(request, env);
      if (request.method === "POST" && url.pathname === "/telemetry/heartbeat") return await handleTelemetryHeartbeat(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/admin/usage") return await handleAdminUsage(request, env);
      if (request.method === "POST" && url.pathname === "/admin/migrate-profile-avatars") return await handleAdminMigrateProfileAvatars(request, env);
      if (request.method === "POST" && url.pathname === "/admin/migrate-device-secrets") return await handleAdminMigrateDeviceSecrets(request, env);
      if (request.method === "GET" && url.pathname.startsWith("/profile/avatar/")) return await handleProfileAvatarGet(env, decodeURIComponent(url.pathname.slice("/profile/avatar/".length)));
      if (request.method === "POST" && url.pathname === "/profile/avatar") return await handleProfileAvatarUpload(request, env);
      if (request.method === "GET" && url.pathname.startsWith("/feed/image/")) return await handleFeedImageGet(request, env, decodeURIComponent(url.pathname.slice("/feed/image/".length)));
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/feed") return await handleFeed(request, env);
      if (request.method === "POST" && url.pathname === "/sync") return await handleSync(request, env);
      if (request.method === "POST" && url.pathname === "/sync/v2") return await handleSync(request, env, true);
      if (request.method === "POST" && url.pathname === "/verified-session/start") return await handleVerifiedSessionStart(request, env);
      if (request.method === "POST" && url.pathname === "/verified-session/heartbeat") return await handleVerifiedSessionHeartbeat(request, env);
      if (request.method === "POST" && url.pathname === "/verified-session/finish") return await handleVerifiedSessionFinish(request, env);
      if (request.method === "POST" && url.pathname === "/verified-session/reconcile-offline") return await handleVerifiedSessionReconcileOffline(request, env);
      if (request.method === "POST" && url.pathname === "/feed/react") return await handleFeedReaction(request, env);
      if (request.method === "POST" && url.pathname === "/feed/poll/vote") return await handleFeedPollVote(request, env);
      if (request.method === "POST" && url.pathname === "/feed/comment") return await handleFeedComment(request, env);
      if (request.method === "POST" && url.pathname === "/feed/update") return await handleFeedUpdate(request, env);
      if (request.method === "POST" && url.pathname === "/feed/delete") return await handleFeedDelete(request, env);
      if (request.method === "POST" && url.pathname === "/feed/image") return await handleFeedImageUpload(request, env);
      if (request.method === "POST" && url.pathname === "/feed/image/delete") return await handleFeedImageDelete(request, env);
      if (request.method === "POST" && url.pathname === "/friends/request") return await handleFriendRequest(request, env);
      if (request.method === "POST" && url.pathname === "/friends/respond") return await handleFriendResponse(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/friends/status") return await handleFriendStatus(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/friends/status/v2") return await handleFriendStatus(request, env, false);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/leaderboard") return await handleLeaderboard(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/squads/scoreboard") return await handleSquadScoreboard(request, env);
      if (request.method === "POST" && url.pathname === "/squads/create") return await handleSquadCreate(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/squads/search") return await handleSquadSearch(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/squads/details") return await handleSquadDetails(request, env);
      if (request.method === "POST" && url.pathname === "/squads/join") return await handleSquadJoin(request, env);
      if (request.method === "POST" && url.pathname === "/squads/respond") return await handleSquadRespond(request, env);
      if (request.method === "POST" && url.pathname === "/squads/leave") return await handleSquadLeave(request, env);
      if (request.method === "POST" && url.pathname === "/squads/chat") return await handleSquadChat(request, env);
      if (request.method === "POST" && url.pathname === "/squads/chat/delete") return await handleSquadChatDelete(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/squads/status") return await handleFriendStatus(request, env);
      if (request.method === "POST" && (url.pathname === "/squads/promote" || url.pathname === "/squads/demote")) return await handleSquadRole(request, env);
      if (request.method === "POST" && url.pathname === "/squads/kick") return await handleSquadKick(request, env);
      if (request.method === "POST" && url.pathname === "/squads/settings") return await handleSquadSettings(request, env);
      if (request.method === "POST" && url.pathname === "/presence") return await handlePresence(request, env);
      if ((request.method === "GET" || request.method === "POST") && url.pathname === "/player-stats") return await handlePlayerStats(request, env);
      return text("Not found.", 404);
    } catch (error: unknown) {
      if (error instanceof Response) return error;
      console.error(error);
      return text(error instanceof Error ? error.message : "Unexpected server error.", 500);
    }
  },
  async scheduled(_event: ScheduledEvent, env: Env): Promise<void> {
    await purgeExpiredAbuseData(env).catch((error) => console.error("Abuse-data retention purge failed.", error));
    await migrateStuckProfileAvatars(env, WORKER_ORIGIN, 25).catch((error) => console.error("Profile avatar migration failed.", error));
    const now = new Date().toISOString();
    const yesterday = yesterdayIso();
    const queued = await env.DB.prepare("SELECT date FROM squad_score_jobs ORDER BY created_at ASC LIMIT 7").all<{ date: string }>();
    for (const row of queued.results) {
      try {
        await scoreSquadDate(env, row.date);
        await env.DB.prepare("DELETE FROM squad_score_jobs WHERE date = ?").bind(row.date).run();
      } catch (error) {
        console.error("Queued squad scoring failed.", row.date, error);
      }
    }
    await scoreSquadDate(env, yesterday).catch((error) => console.error("Squad scoring failed.", error));
    await scoreMissingCompletedSquadDates(env);
    const usage = await getR2Usage(env);
    const remainingClassA = Math.max(0, R2_CLASS_A_HARD_MONTHLY - usage.classAOps);
    if (!remainingClassA) return;
    const rows = await env.DB.prepare(`
      SELECT id, image_key AS imageKey, image_size_bytes AS imageSizeBytes
      FROM feed_posts
      WHERE image_key IS NOT NULL AND image_expires_at IS NOT NULL AND image_expires_at <= ?
      LIMIT 100
    `).bind(now).all<{ id: string; imageKey: string; imageSizeBytes: number }>();

    let deleted = 0;
    let releasedBytes = 0;
    for (const row of rows.results.slice(0, remainingClassA)) {
      await env.FEED_IMAGES.delete(row.imageKey).catch(() => undefined);
      await incrementR2Usage(env, { classA: 1 });
      deleted += 1;
      releasedBytes += Number(row.imageSizeBytes ?? 0);
      await env.DB.prepare(`
        UPDATE feed_posts
        SET image_key = NULL, image_mime_type = NULL, image_expires_at = NULL, image_expired_at = ?, image_size_bytes = 0
        WHERE id = ? AND image_key = ?
      `).bind(now, row.id, row.imageKey).run();
    }
    if (deleted) {
      const nextStorageBytes = Math.max(0, usage.storageBytes - releasedBytes);
      await env.DB.prepare(`
        INSERT INTO r2_usage_monthly (month, storage_bytes, updated_at)
        VALUES (?, ?, CURRENT_TIMESTAMP)
        ON CONFLICT(month) DO UPDATE SET storage_bytes = ?, updated_at = CURRENT_TIMESTAMP
      `).bind(usageMonth(), nextStorageBytes, nextStorageBytes).run();
      await maybeNotifyR2Usage(env);
    }
  },
};
