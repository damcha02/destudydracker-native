//! Task (Stage 16). Production reference: `desktop/src/types.ts`'s `Task`/`TaskSubtype`/
//! `Priority`, `App.tsx`'s `addTask`/`removeTask`.

use serde::{Deserialize, Serialize};

use super::date::LocalDate;
use super::ids::{CourseId, SemesterId, TaskId};
use crate::timer::WallTimestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskSubtype {
    Lecture,
    Session,
    Sheet,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub semester_id: SemesterId,
    pub course_id: CourseId,
    pub title: String,
    pub subtype: TaskSubtype,
    pub unit_label: String,
    /// Production's own comment (`App.tsx`) is preserved as domain knowledge, not just code
    /// history: no task-creation UI takes a manual `totalUnits`/`completedUnits` input - both are
    /// recomputed from how many of the task's projected occurrences (weekly recurrence expanded
    /// across the semester, holidays excluded) are checked off. Stage 16 does not migrate that
    /// recurrence-expansion engine (`plannerSchedule.ts`) - these two fields are carried through
    /// as plain data, defaulting to `0`/`0` for a freshly created task, exactly as production does.
    pub total_units: u32,
    pub completed_units: u32,
    pub due_date: Option<LocalDate>,
    pub priority: Priority,
    pub notes: String,
    pub created_at: WallTimestamp,
    /// Production v0.1.67 (wabi exam prep): a revision task planned after the semester ended, whose
    /// `total_units` is set by hand rather than derived from the schedule. Absent in older stores.
    #[serde(default, skip_serializing_if = "is_false")]
    pub prep: bool,
    /// The semester task this prep task was repeated from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prep_of: Option<TaskId>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl Task {
    pub fn new(
        id: TaskId,
        semester_id: SemesterId,
        course_id: CourseId,
        title: String,
        subtype: TaskSubtype,
        unit_label: String,
        created_at: WallTimestamp,
    ) -> Self {
        Self {
            id,
            semester_id,
            course_id,
            title,
            subtype,
            unit_label,
            total_units: 0,
            completed_units: 0,
            due_date: None,
            priority: Priority::Medium,
            notes: String::new(),
            created_at,
            prep: false,
            prep_of: None,
        }
    }
}
