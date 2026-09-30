//! Date/time policy (Stage 16, see `docs/stage16-academic-domain.md` section 13 for the full
//! writeup). Production stores three genuinely different kinds of time value under plain
//! `string`/`string | null` TypeScript fields, and conflating them during migration would change
//! their meaning - so the native domain models them as three distinct types instead of one:
//!
//! - **Absolute instant** (`created_at`, `archived_at`, session `started_at`/`ended_at`,
//!   `completed_at`): a real moment in wall-clock time, independent of any timezone. Reuses the
//!   Timer domain's existing [`crate::timer::WallTimestamp`] (unix milliseconds) rather than a
//!   second type with the same meaning.
//! - **Local calendar date** ([`LocalDate`], `YYYY-MM-DD`): an academic-calendar concept - a
//!   semester's start date, a task's due date, an exam's date - that names a *day*, not an
//!   instant. Production never attaches a timezone to these (`startDate: string | null` is just
//!   whatever a `<input type="date">` produces) and neither does this type; converting one to a
//!   UTC timestamp would be lossy and, near a local midnight boundary, could silently shift which
//!   calendar day it names - exactly what this stage's brief warns against, so it is deliberately
//!   never done.
//! - **Local time-of-day** (`HH:MM` strings on `TimetableEvent`/`DailyTodo`/`CalendarEntry`): a
//!   recurring weekly time slot, not tied to a specific date or timezone at all. Kept as plain
//!   `String`/`Option<String>` fields (matching production's own lack of a stronger type there)
//!   rather than introducing a fourth wrapper type for a handful of fields.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A local calendar date, `YYYY-MM-DD`, with no time-of-day or timezone component. Validated only
/// for the shape production itself relies on (four digits, two digits, two digits) - not a real
/// calendar-correctness check (e.g. "2026-02-30" is accepted, exactly as a raw string would be in
/// production; nothing in production validates this more strictly either).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LocalDate(String);

impl LocalDate {
    /// Returns `None` if `value` is not shaped like `YYYY-MM-DD` - the same "reject, don't guess"
    /// policy production's own normalizers use for a field that is load-bearing for ordering.
    pub fn parse(value: &str) -> Option<Self> {
        let bytes = value.as_bytes();
        let shape_ok = bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[..4].iter().all(u8::is_ascii_digit)
            && bytes[5..7].iter().all(u8::is_ascii_digit)
            && bytes[8..10].iter().all(u8::is_ascii_digit);
        shape_ok.then(|| Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LocalDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_date() {
        assert_eq!(
            LocalDate::parse("2026-09-28").unwrap().as_str(),
            "2026-09-28"
        );
    }

    #[test]
    fn rejects_malformed_shapes_without_panicking() {
        assert!(LocalDate::parse("").is_none());
        assert!(LocalDate::parse("2026-9-28").is_none());
        assert!(LocalDate::parse("not-a-date").is_none());
        assert!(LocalDate::parse("2026-09-28T00:00:00Z").is_none());
        assert!(LocalDate::parse("2026/09/28").is_none());
    }

    #[test]
    fn orders_lexicographically_which_matches_calendar_order_for_this_fixed_width_shape() {
        let a = LocalDate::parse("2026-01-01").unwrap();
        let b = LocalDate::parse("2026-12-31").unwrap();
        assert!(a < b);
    }
}
