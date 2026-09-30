//! Planner/calendar value objects (Stage 16): `TimetableEvent`, `Holiday`, `DailyTodo`,
//! `CalendarEntry`. Production reference: `desktop/src/types.ts`.
//!
//! These four are kept as **separate, distinct types**, not flattened into one "calendar item"
//! union, because production treats them as genuinely different concepts sharing only "has a
//! date": `TimetableEvent` is a recurring class/lecture/sheet-deadline slot tied to a course and
//! task; `Holiday` is a semester-wide date range with no course/task at all; `DailyTodo` is a
//! personal to-do with no semester/course/task association; `CalendarEntry` is a scheduled
//! work-unit allocation against a specific `Task`. Flattening them would lose exactly the
//! distinctions production's own Planner logic depends on.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::date::LocalDate;
use super::ids::{
    CalendarEntryId, CourseId, DailyTodoId, HolidayId, SemesterId, TaskId, TimetableEventId,
};
use crate::timer::WallTimestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimetableEventKind {
    Occurrence,
    SheetRelease,
    SheetDeadline,
}

/// A per-occurrence override for one instance of a repeating `TimetableEvent`, keyed by that
/// occurrence's local date. A `BTreeMap` (not a `HashMap`) so iteration/serialization order is
/// deterministic (section 23's ordering requirement), matching this crate's general policy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OccurrenceOverride {
    pub skipped: bool,
    pub date: Option<LocalDate>,
    pub time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimetableEvent {
    pub id: TimetableEventId,
    pub semester_id: SemesterId,
    pub course_id: CourseId,
    pub kind: TimetableEventKind,
    pub task_id: TaskId,
    pub label: String,
    /// Local calendar date this recurring event's series anchors to.
    pub date: LocalDate,
    /// Local time-of-day, `HH:MM` (see `date.rs` module docs) - not a `LocalDate`/`WallTimestamp`,
    /// since it names a recurring time slot, not a specific instant.
    pub time: String,
    pub end_time: Option<String>,
    pub repeat_weekly: bool,
    pub recurrence_end_date: Option<LocalDate>,
    pub occurrence_overrides: BTreeMap<String, OccurrenceOverride>,
    pub url: Option<String>,
    /// Local dates (as strings, matching each occurrence's own date key) marked complete.
    pub completed_occurrences: Vec<String>,
    pub created_at: WallTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holiday {
    pub id: HolidayId,
    pub semester_id: SemesterId,
    pub start_date: LocalDate,
    pub end_date: LocalDate,
    pub label: String,
    pub created_at: WallTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyTodo {
    pub id: DailyTodoId,
    pub date: LocalDate,
    pub time: Option<String>,
    pub end_time: Option<String>,
    pub title: String,
    pub notes: String,
    pub completed: bool,
    pub completed_at: Option<WallTimestamp>,
    pub created_at: WallTimestamp,
    pub repeat_weekly: bool,
    pub completed_occurrences: Vec<String>,
    pub recurrence_end_date: Option<LocalDate>,
    pub skipped_occurrences: Vec<String>,
    pub occurrence_times: BTreeMap<String, (Option<String>, Option<String>)>,
}

/// `unitAmount` is restricted to exactly these three fractional values in production
/// (`record.unitAmount === 0.5 || record.unitAmount === 0.25 ? record.unitAmount : 1`) - modeled
/// as an enum rather than a bare `f64` so an invalid value is a compile-time impossibility for
/// anything constructed in Rust, and the one normalization rule (default to `Whole`) is explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnitAmount {
    Whole,
    Half,
    Quarter,
}

impl UnitAmount {
    pub fn as_f64(self) -> f64 {
        match self {
            UnitAmount::Whole => 1.0,
            UnitAmount::Half => 0.5,
            UnitAmount::Quarter => 0.25,
        }
    }

    /// Mirrors production's own tolerant normalization: anything other than exactly `0.5`/`0.25`
    /// becomes a whole unit, never rejected or clamped some other way.
    pub fn from_f64(value: f64) -> Self {
        if value == 0.5 {
            UnitAmount::Half
        } else if value == 0.25 {
            UnitAmount::Quarter
        } else {
            UnitAmount::Whole
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub id: CalendarEntryId,
    pub task_id: TaskId,
    pub date: LocalDate,
    pub unit_amount: UnitAmount,
    pub unit_start: Option<u32>,
    pub completed: bool,
    pub completed_at: Option<WallTimestamp>,
    pub created_at: WallTimestamp,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    /// An ad-hoc entry (no real `Task` behind it - a one-off calendar block) carries its own
    /// title/semester/course directly instead of resolving them through `task_id`. Production
    /// allows this to coexist with a (possibly dangling) `task_id`; both are preserved as-is.
    pub ad_hoc_title: Option<String>,
    pub ad_hoc_semester_id: Option<SemesterId>,
    pub ad_hoc_course_id: Option<CourseId>,
}
