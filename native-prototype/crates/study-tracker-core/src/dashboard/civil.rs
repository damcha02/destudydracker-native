//! Civil-date arithmetic and the local-clock boundary for the Dashboard metrics (Stage 17).
//!
//! Production's `metrics.ts` does all of its day math with JavaScript `Date` in the *local*
//! timezone. This module provides just enough of that to port it without a timezone library:
//!
//! - [`CivilDate`]: a proleptic-Gregorian day number (days since 1970-01-01) with year/month/day,
//!   JS-style weekday (`0 = Sunday`), and day addition. Pure integer math (Howard Hinnant's
//!   `days_from_civil`/`civil_from_days`), no allocation, no I/O.
//! - [`LocalClock`]: the *only* place a timezone decision enters the metrics. The metrics never
//!   read the system clock or timezone themselves: the caller hands in a clock. The real app uses
//!   a per-instant `chrono::Local` implementation (matches JS, which applies the timezone rules of
//!   each instant); tests use [`FixedOffsetClock`] so every date is deterministic.

use crate::academic::LocalDate;
use crate::timer::WallTimestamp;

const MS_PER_DAY: i64 = 86_400_000;

/// A calendar day, independent of any timezone. Ordered chronologically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDate(i64);

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

impl CivilDate {
    /// `day` may run past the month's end (`2026-02-30` -> 2026-03-02): V8's date parser rolls
    /// over the same way, and production feeds user-entered strings straight into `new Date`.
    /// Month `1..=12` and day `1..=31` are required; anything else is an invalid date (`None`),
    /// which production's `NaN` comparisons treat as "never matches".
    pub fn from_ymd(year: i32, month: u32, day: u32) -> Option<Self> {
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return None;
        }
        Some(Self(days_from_civil(year as i64, month as i64, day as i64)))
    }

    pub const fn from_days(days_since_epoch: i64) -> Self {
        Self(days_since_epoch)
    }

    pub const fn days(self) -> i64 {
        self.0
    }

    pub fn ymd(self) -> (i32, u32, u32) {
        let (y, m, d) = civil_from_days(self.0);
        (y as i32, m as u32, d as u32)
    }

    pub fn year(self) -> i32 {
        self.ymd().0
    }

    pub fn month(self) -> u32 {
        self.ymd().1
    }

    pub fn day(self) -> u32 {
        self.ymd().2
    }

    /// JavaScript `Date.prototype.getDay()`: `0 = Sunday ... 6 = Saturday`.
    pub fn weekday(self) -> u32 {
        (self.0 + 4).rem_euclid(7) as u32
    }

    pub fn add_days(self, delta: i64) -> Self {
        Self(self.0 + delta)
    }

    pub fn days_until(self, other: CivilDate) -> i64 {
        other.0 - self.0
    }

    /// `YYYY-MM-DD` (production's `isoDate`).
    pub fn to_iso(self) -> String {
        let (y, m, d) = self.ymd();
        format!("{y:04}-{m:02}-{d:02}")
    }

    /// Parses the leading `YYYY-MM-DD` of a string (so a full ISO timestamp also works, like
    /// `value.slice(0, 10)` in production).
    pub fn parse_iso(value: &str) -> Option<Self> {
        let bytes = value.as_bytes();
        if bytes.len() < 10 || bytes[4] != b'-' || bytes[7] != b'-' {
            return None;
        }
        let num = |range: std::ops::Range<usize>| -> Option<u32> {
            let part = value.get(range)?;
            if part.bytes().all(|b| b.is_ascii_digit()) {
                part.parse().ok()
            } else {
                None
            }
        };
        Self::from_ymd(num(0..4)? as i32, num(5..7)?, num(8..10)?)
    }

    pub fn from_local_date(date: &LocalDate) -> Option<Self> {
        Self::parse_iso(date.as_str())
    }

    /// Midnight UTC of this date as an instant: what JavaScript's `new Date("YYYY-MM-DD")` yields
    /// for a *date-only* string (ISO date-only strings are UTC, date-time strings are local).
    pub fn utc_midnight(self) -> WallTimestamp {
        WallTimestamp::from_unix_millis(self.0 * MS_PER_DAY)
    }
}

/// `toISOString()` for an instant: `YYYY-MM-DDTHH:MM:SS.mmmZ` (UTC). Needed because production
/// string-compares these (`a.startTime ?? a.createdAt`), so the exact text matters.
pub fn to_iso_utc_string(ts: WallTimestamp) -> String {
    let ms = ts.unix_millis;
    let days = ms.div_euclid(MS_PER_DAY);
    let rem = ms.rem_euclid(MS_PER_DAY);
    let (y, m, d) = CivilDate::from_days(days).ymd();
    let (h, mi, s, milli) = (
        rem / 3_600_000,
        (rem / 60_000) % 60,
        (rem / 1000) % 60,
        rem % 1000,
    );
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
}

/// The timezone boundary. See the module docs.
pub trait LocalClock {
    /// The local calendar date an instant falls on (`isoDate(new Date(instant))`).
    fn local_date(&self, instant: WallTimestamp) -> CivilDate;
    /// The instant of local midnight at the start of `date` (`new Date("YYYY-MM-DDT00:00:00")`).
    fn local_midnight(&self, date: CivilDate) -> WallTimestamp;
}

/// A constant UTC offset. The deterministic clock for tests (and a documented fallback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedOffsetClock {
    pub offset_seconds: i32,
}

impl FixedOffsetClock {
    pub const UTC: Self = Self { offset_seconds: 0 };

    pub const fn new(offset_seconds: i32) -> Self {
        Self { offset_seconds }
    }
}

impl LocalClock for FixedOffsetClock {
    fn local_date(&self, instant: WallTimestamp) -> CivilDate {
        let local_ms = instant.unix_millis + i64::from(self.offset_seconds) * 1000;
        CivilDate::from_days(local_ms.div_euclid(MS_PER_DAY))
    }

    fn local_midnight(&self, date: CivilDate) -> WallTimestamp {
        WallTimestamp::from_unix_millis(
            date.days() * MS_PER_DAY - i64::from(self.offset_seconds) * 1000,
        )
    }
}

/// Production's `new Date("YYYY-MM-DD")` followed by local reads (`setHours(0,0,0,0)`,
/// `Intl.DateTimeFormat`): the date-only string is UTC midnight, then viewed in *local* time. East
/// of UTC (e.g. Zurich) that is the same date; **west of UTC it is the previous day** - a real
/// production quirk (`daysUntil`/`formatDate` are one day early for users in the Americas). It is
/// reproduced, not fixed (Stage 17 brief: document production quirks, do not silently fix them);
/// see `docs/stage17-dashboard.md`, "Production quirks".
pub fn js_date_only_as_local(clock: &dyn LocalClock, date: CivilDate) -> CivilDate {
    clock.local_date(date.utc_midnight())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_round_trips_and_knows_weekdays() {
        let d = CivilDate::from_ymd(2026, 9, 30).unwrap();
        assert_eq!(d.ymd(), (2026, 9, 30));
        assert_eq!(d.weekday(), 3, "2026-09-30 is a Wednesday");
        assert_eq!(CivilDate::from_ymd(1970, 1, 1).unwrap().days(), 0);
        assert_eq!(
            CivilDate::from_ymd(1970, 1, 1).unwrap().weekday(),
            4,
            "Thursday"
        );
        assert_eq!(d.to_iso(), "2026-09-30");
    }

    #[test]
    fn leap_day_and_year_boundaries() {
        let feb28 = CivilDate::from_ymd(2028, 2, 28).unwrap();
        assert_eq!(
            feb28.add_days(1).to_iso(),
            "2028-02-29",
            "2028 is a leap year"
        );
        assert_eq!(feb28.add_days(2).to_iso(), "2028-03-01");
        assert_eq!(
            CivilDate::from_ymd(2027, 2, 28)
                .unwrap()
                .add_days(1)
                .to_iso(),
            "2027-03-01"
        );
        assert_eq!(
            CivilDate::from_ymd(2026, 12, 31)
                .unwrap()
                .add_days(1)
                .to_iso(),
            "2027-01-01"
        );
        assert_eq!(
            CivilDate::from_ymd(2100, 2, 28)
                .unwrap()
                .add_days(1)
                .to_iso(),
            "2100-03-01",
            "2100 is not a leap year"
        );
        assert_eq!(
            CivilDate::from_ymd(2000, 2, 28)
                .unwrap()
                .add_days(1)
                .to_iso(),
            "2000-02-29"
        );
    }

    #[test]
    fn overflowing_day_rolls_over_like_v8_and_invalid_parts_are_none() {
        assert_eq!(
            CivilDate::from_ymd(2026, 2, 30).unwrap().to_iso(),
            "2026-03-02"
        );
        assert!(CivilDate::from_ymd(2026, 13, 1).is_none());
        assert!(CivilDate::from_ymd(2026, 0, 1).is_none());
        assert!(CivilDate::from_ymd(2026, 1, 0).is_none());
        assert!(CivilDate::parse_iso("nope").is_none());
        assert_eq!(
            CivilDate::parse_iso("2026-09-30T23:59:59.000Z")
                .unwrap()
                .to_iso(),
            "2026-09-30"
        );
    }

    #[test]
    fn iso_string_matches_to_iso_string() {
        let ts = WallTimestamp::from_unix_millis(1_790_000_000_123);
        assert_eq!(to_iso_utc_string(ts), "2026-09-21T14:13:20.123Z");
    }

    #[test]
    fn fixed_offset_clock_shifts_the_local_day() {
        let just_before_utc_midnight = CivilDate::from_ymd(2026, 9, 30)
            .unwrap()
            .utc_midnight()
            .plus_seconds(86_400 - 60);
        assert_eq!(
            FixedOffsetClock::UTC
                .local_date(just_before_utc_midnight)
                .to_iso(),
            "2026-09-30"
        );
        assert_eq!(
            FixedOffsetClock::new(2 * 3600)
                .local_date(just_before_utc_midnight)
                .to_iso(),
            "2026-10-01"
        );
        assert_eq!(
            FixedOffsetClock::new(-5 * 3600)
                .local_date(just_before_utc_midnight)
                .to_iso(),
            "2026-09-30"
        );
        let midnight = FixedOffsetClock::new(2 * 3600)
            .local_midnight(CivilDate::from_ymd(2026, 10, 1).unwrap());
        assert_eq!(
            FixedOffsetClock::UTC.local_date(midnight).to_iso(),
            "2026-09-30",
            "02:00 local midnight is 22:00Z the day before"
        );
    }

    #[test]
    fn date_only_quirk_east_same_day_west_previous_day() {
        let d = CivilDate::from_ymd(2026, 10, 9).unwrap();
        assert_eq!(
            js_date_only_as_local(&FixedOffsetClock::new(2 * 3600), d).to_iso(),
            "2026-10-09"
        );
        assert_eq!(
            js_date_only_as_local(&FixedOffsetClock::UTC, d).to_iso(),
            "2026-10-09"
        );
        assert_eq!(
            js_date_only_as_local(&FixedOffsetClock::new(-7 * 3600), d).to_iso(),
            "2026-10-08"
        );
    }
}
