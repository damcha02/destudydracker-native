//! Size limits for Social data (Stage 22a).
//!
//! Two kinds, kept apart on purpose:
//!
//! - **Production limits** - the Worker's own `MAX_*` constants and `cleanText`/`requiredText`
//!   bounds (`cloudflare/src/index.ts`) and the client's input limits (`SocialScreen.tsx`
//!   `maxLength`). Values the server can never legitimately send beyond these.
//! - **Defensive limits** - production has none on the client; these only protect the native
//!   client from a pathological or hostile payload and are set well above anything the
//!   production server can produce, so they never truncate valid data.

/// `MAX_ID_LENGTH` (Worker): userId, request id, drawing id.
pub const MAX_ID_LEN: usize = 80;
/// `MAX_SECRET_LENGTH` (Worker): deviceSecret.
pub const MAX_SECRET_LEN: usize = 120;
/// Display names: `cleanName` slices to 48 UTF-16 units; the Profile input has `maxLength={48}`.
pub const MAX_DISPLAY_NAME_UTF16: usize = 48;
/// Friend codes: production generates `XXXX-XXXX`; legacy/atypical codes still exist server-side
/// (`isFriendCodeAtypical`), so the defensive bound is wider than the pattern.
pub const MAX_FRIEND_CODE_LEN: usize = 32;
/// Avatar photo file name (`cleanAvatar`: `name.slice(0, 180)`).
pub const MAX_AVATAR_NAME_LEN: usize = 180;
/// Avatar photo URL: the Worker's own `${origin}/profile/avatar/<key>` is far shorter. Defensive.
pub const MAX_AVATAR_URL_LEN: usize = 512;
/// Leaderboards: the Worker returns at most 50 rows (`LIMIT 50`). Defensive cap above that.
pub const MAX_LEADERBOARD_ENTRIES: usize = 200;
/// Friends / requests: production has no server cap. Defensive cap (a list this long is already
/// far beyond any real account; anything more is dropped, never allocated past this).
pub const MAX_FRIENDS: usize = 2_000;
pub const MAX_FRIEND_REQUESTS: usize = 500;
/// Timestamps as text (`2026-09-30 12:00:00` / ISO 8601). Defensive.
pub const MAX_TIMESTAMP_LEN: usize = 40;
/// Minutes / sessions totals: the Worker caps lifetime totals at 1,000,000 sessions and per-day
/// minutes at 1440; a leaderboard total above this cannot be real. Defensive.
pub const MAX_TOTAL_MINUTES: u64 = 100_000_000;
pub const MAX_TOTAL_SESSIONS: u64 = 10_000_000;
/// `MAX_SYNC_STAT_ROWS` (client and Worker).
pub const MAX_SYNC_STAT_ROWS: usize = 370;
/// `MAX_SYNC_BODY_BYTES` (client) / `MAX_JSON_BODY_BYTES` (Worker).
pub const MAX_SYNC_BODY_BYTES: usize = 256 * 1024;
