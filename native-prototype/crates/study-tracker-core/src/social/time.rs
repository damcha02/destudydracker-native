//! Social timestamps (Stage 22a): `parseSocialTimestamp`, `isRecentlyActive` and
//! `getNextAutoSyncAt` from production, on the core's own clock types.

use serde::{Deserialize, Serialize};

use crate::dashboard::civil::{CivilDate, LocalClock};
use crate::timer::WallTimestamp;

use super::limits::MAX_TIMESTAMP_LEN;

/// `isRecentlyActive`'s default window: 45 minutes ("live" friends).
pub const RECENTLY_ACTIVE_MS: i64 = 45 * 60 * 1000;
/// `WABI_ATTENDANCE_WINDOW_MS`: 48 hours (Wabi Circle attendance list).
pub const WABI_ATTENDANCE_WINDOW_MS: i64 = 48 * 60 * 60 * 1000;
/// `formatProfileSeenAt` shows the time of day when the timestamp is within 2 days.
pub const SEEN_RECENT_MS: i64 = 2 * 24 * 60 * 60 * 1000;
/// `SOCIAL_SYNC_INTERVAL_MS`: 12 hours.
pub const SOCIAL_SYNC_INTERVAL_MS: i64 = 12 * 60 * 60 * 1000;

/// A server or client timestamp, as an instant. Serialised as unix milliseconds in the native
/// store (never as the server's text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SocialTimestamp(pub i64);

fn digits(s: &str) -> Option<u32> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok())?
}

fn date_millis(y: &str, m: &str, d: &str) -> Option<i64> {
    let (y, m, d) = (digits(y)?, digits(m)?, digits(d)?);
    if !(1..=31).contains(&d) {
        return None;
    }
    let date = CivilDate::from_ymd(i32::try_from(y).ok()?, m, d)?;
    // reject roll-over dates like 02-31: the round trip must be exact
    (date.ymd() == (y as i32, m, d)).then(|| date.days() * 86_400_000)
}

fn time_millis(t: &str) -> Option<i64> {
    // HH:MM[:SS[.fff]]
    let (hms, frac) = t.split_once('.').map_or((t, ""), |(a, b)| (a, b));
    let mut parts = hms.split(':');
    let h = digits(parts.next()?)?;
    let mi = digits(parts.next()?)?;
    let s = parts.next().map_or(Some(0), digits)?;
    if parts.next().is_some() || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let ms = if frac.is_empty() {
        0
    } else {
        let f: String = frac.chars().take(3).collect();
        if !f.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        format!("{f:0<3}").parse::<i64>().ok()?
    };
    Some(i64::from(h) * 3_600_000 + i64::from(mi) * 60_000 + i64::from(s) * 1000 + ms)
}

impl SocialTimestamp {
    pub fn from_wall(wall: WallTimestamp) -> Self {
        Self(wall.unix_millis)
    }

    pub fn wall(self) -> WallTimestamp {
        WallTimestamp::from_unix_millis(self.0)
    }

    /// `parseSocialTimestamp`: SQLite's `YYYY-MM-DD HH:MM:SS` (UTC, as the Worker stores
    /// `CURRENT_TIMESTAMP`), ISO 8601 with `Z` or a `±HH:MM` offset, or a bare `YYYY-MM-DD`
    /// (UTC midnight, as `new Date` reads it). Anything else - including an impossible date - is
    /// `None` (production's `NaN`).
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() || raw.len() > MAX_TIMESTAMP_LEN || !raw.is_ascii() {
            return None;
        }
        let (date, rest) = raw.split_at(raw.len().min(10));
        let mut dp = date.split('-');
        let (y, m, d) = (dp.next()?, dp.next()?, dp.next()?);
        if dp.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
            return None;
        }
        let day = date_millis(y, m, d)?;
        if rest.is_empty() {
            return Some(Self(day));
        }
        let rest = rest.strip_prefix(' ').or_else(|| rest.strip_prefix('T'))?;
        // SQLite form: no zone at all, UTC by the Worker's convention
        if !rest.contains(['Z', '+', '-']) {
            return raw
                .as_bytes()
                .get(10)
                .filter(|b| **b == b' ')
                .and_then(|_| time_millis(rest).map(|t| Self(day + t)));
        }
        let (time, offset_ms) = if let Some(t) = rest.strip_suffix('Z') {
            (t, 0)
        } else {
            let i = rest.rfind(['+', '-'])?;
            let (t, off) = rest.split_at(i);
            let sign = if off.starts_with('-') { -1 } else { 1 };
            let (oh, om) = off[1..].split_once(':').unwrap_or((&off[1..], "00"));
            let (oh, om) = (digits(oh)?, digits(om)?);
            if oh > 23 || om > 59 {
                return None;
            }
            (
                t,
                sign * (i64::from(oh) * 3_600_000 + i64::from(om) * 60_000),
            )
        };
        Some(Self(day + time_millis(time)? - offset_ms))
    }

    /// ISO 8601 UTC with milliseconds (`new Date().toISOString()`), for the wire.
    pub fn to_iso(self) -> String {
        crate::dashboard::civil::to_iso_utc_string(self.wall())
    }
}

/// `isRecentlyActive(value, maxAgeMs)`: a parsed timestamp younger than the window. A timestamp
/// in the future counts as active (production compares `now - ts < max`).
pub fn is_recently_active(
    value: Option<SocialTimestamp>,
    now: WallTimestamp,
    max_age_ms: i64,
) -> bool {
    value.is_some_and(|ts| now.unix_millis - ts.0 < max_age_ms)
}

/// `getNextAutoSyncAt`: the earliest of now + 12 h, the next local midnight and the next local
/// Monday 00:00.
pub fn next_auto_sync_at(now: WallTimestamp, clock: &dyn LocalClock) -> WallTimestamp {
    let today = clock.local_date(now);
    let next_day = clock.local_midnight(today.add_days(1));
    let monday_offset = (i64::from(today.weekday()) + 6) % 7;
    let next_week = clock.local_midnight(today.add_days(7 - monday_offset));
    let interval = now.unix_millis.saturating_add(SOCIAL_SYNC_INTERVAL_MS);
    WallTimestamp::from_unix_millis(
        interval
            .min(next_day.unix_millis)
            .min(next_week.unix_millis),
    )
}
