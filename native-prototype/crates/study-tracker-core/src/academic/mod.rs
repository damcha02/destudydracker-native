//! Academic/planner domain (Stage 16): semesters, courses, tasks, exams, study sessions, and
//! planner/calendar value objects. See `docs/stage16-academic-domain.md` for the full production
//! inventory and design writeup this module implements against.
//!
//! Independent of `timer` in both directions except for reusing [`crate::timer::WallTimestamp`]
//! for absolute-instant fields (see `date.rs`) - the same dependency-direction rule as the rest of
//! this crate applies here too: no Slint, no filesystem, no OS calls, no randomness.

mod course;
mod date;
mod exam;
mod ids;
mod planner;
mod semester;
mod session;
mod state;
mod task;

#[cfg(test)]
mod tests;

pub use course::{
    clamp_target_grade, Course, DEFAULT_TARGET_GRADE, MAX_TARGET_GRADE, MIN_TARGET_GRADE,
};
pub use date::LocalDate;
pub use exam::Exam;
pub use ids::{
    CalendarEntryId, CourseId, DailyTodoId, ExamId, HolidayId, SemesterId, SessionId, TaskId,
    TimetableEventId,
};
pub use planner::{
    CalendarEntry, DailyTodo, Holiday, OccurrenceOverride, TimetableEvent, TimetableEventKind,
    UnitAmount,
};
pub use semester::{Semester, SemesterPhase};
pub use session::{SessionKind, StudySession};
pub use state::{prune_session_history, AcademicState, SESSION_HISTORY_DAYS, SESSION_HISTORY_MAX};
pub use task::{Priority, Task, TaskSubtype};
