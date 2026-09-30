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
        }
    }
}
