//! Europe/Zurich civil dates (Stage 22a).
//!
//! Daily Skribbl's day is the **Worker's** day: `todayIso()` = `serverDateIso()` =
//! `Intl.DateTimeFormat("en", { timeZone: "Europe/Zurich" })`. Not the user's local date (which
//! Wordle/Travle use) and not UTC.
//!
//! Switzerland follows the EU summer-time rule (Directive 2000/84/EC, in force for Switzerland
//! since 1996): CET = UTC+1; CEST = UTC+2 from the last Sunday of March 01:00 UTC to the last
//! Sunday of October 01:00 UTC. That rule is all `Intl` applies for any date this app can meet,
//! so it is implemented here directly - no timezone database dependency. Dates before 1996 are
//! outside the rule's validity and are not used by any Skribbl path; should the EU/Swiss rule
//! ever change, this one function is the place to update.

use crate::dashboard::civil::CivilDate;
use crate::timer::WallTimestamp;

const HOUR_MS: i64 = 3_600_000;

/// The last Sunday of `month` in `year`, as a civil date.
fn last_sunday(year: i32, month: u32) -> CivilDate {
    let first_of_next = if month == 12 {
        CivilDate::from_ymd(year + 1, 1, 1)
    } else {
        CivilDate::from_ymd(year, month + 1, 1)
    }
    .expect("valid month start");
    let last = first_of_next.add_days(-1);
    last.add_days(-i64::from(last.weekday()))
}

/// True while `instant` falls in Central European Summer Time.
pub fn is_summer_time(instant: WallTimestamp) -> bool {
    let utc_year = CivilDate::from_days(instant.unix_millis.div_euclid(86_400_000)).year();
    let start = last_sunday(utc_year, 3).days() * 86_400_000 + HOUR_MS;
    let end = last_sunday(utc_year, 10).days() * 86_400_000 + HOUR_MS;
    (start..end).contains(&instant.unix_millis)
}

/// The UTC offset of Europe/Zurich at `instant`, in milliseconds (+1 h or +2 h).
pub fn offset_ms(instant: WallTimestamp) -> i64 {
    if is_summer_time(instant) {
        2 * HOUR_MS
    } else {
        HOUR_MS
    }
}

/// `serverDateIso(instant)`: the Europe/Zurich calendar date of an instant.
pub fn zurich_date(instant: WallTimestamp) -> CivilDate {
    let local = instant.unix_millis + offset_ms(instant);
    CivilDate::from_days(local.div_euclid(86_400_000))
}

/// The instant Europe/Zurich `date` begins (`serverDayStartIso`). Midnight is never inside a
/// transition (those happen at 02:00/03:00 local), so the offset just before midnight is exact.
pub fn zurich_day_start(date: CivilDate) -> WallTimestamp {
    let utc_midnight = date.days() * 86_400_000;
    // the offset that applies at that local midnight: try +1 h, then +2 h
    for offset in [HOUR_MS, 2 * HOUR_MS] {
        let candidate = WallTimestamp::from_unix_millis(utc_midnight - offset);
        if offset_ms(candidate) == offset {
            return candidate;
        }
    }
    WallTimestamp::from_unix_millis(utc_midnight - HOUR_MS)
}
