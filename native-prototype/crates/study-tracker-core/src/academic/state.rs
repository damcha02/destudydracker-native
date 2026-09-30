//! `AcademicState` (Stage 16): the in-memory aggregate for the academic/planner domain, plain
//! Rust with no I/O, mirroring the Timer domain's own shape (a single owned state struct with
//! explicit mutation methods) rather than one giant `AppState`-style blob copied from production's
//! React state shape. Production's `AppState` is a persistence/UI aggregation convenience, not a
//! domain model - see `docs/stage16-academic-domain.md` section 4 for the full reasoning.
//!
//! CRUD/cascade-delete/ordering/retention rules here are all *pure* - no clock reads, no file
//! I/O, no randomness (identifiers and "now" are always supplied by the caller), so every rule is
//! directly, deterministically unit-testable.

use serde::{Deserialize, Serialize};

use super::course::Course;
use super::exam::Exam;
use super::ids::{CourseId, ExamId, SemesterId, SessionId, TaskId};
use super::planner::{CalendarEntry, DailyTodo, Holiday, TimetableEvent};
use super::semester::Semester;
use super::session::StudySession;
use super::task::Task;
use crate::timer::WallTimestamp;

/// Matches production's own retention policy exactly (`App.tsx`'s `SESSION_HISTORY_DAYS`/
/// `SESSION_HISTORY_MAX`): sessions older than 365 days are dropped, and the list is additionally
/// capped at 3000 records (whichever is more restrictive for a given history).
pub const SESSION_HISTORY_DAYS: i64 = 365;
pub const SESSION_HISTORY_MAX: usize = 3000;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AcademicState {
    pub semesters: Vec<Semester>,
    pub courses: Vec<Course>,
    pub tasks: Vec<Task>,
    pub exams: Vec<Exam>,
    pub sessions: Vec<StudySession>,
    pub timetable_events: Vec<TimetableEvent>,
    pub holidays: Vec<Holiday>,
    pub daily_todos: Vec<DailyTodo>,
    pub calendar_entries: Vec<CalendarEntry>,
    /// Matches production's own running totals (`AppState.lifetimeStudyMinutes`/
    /// `lifetimeStudySessions`) - accumulated once per session added, never recomputed from the
    /// (pruned) session list, so lifetime totals survive history pruning exactly as production's
    /// do.
    pub lifetime_study_minutes: u64,
    pub lifetime_study_sessions: u64,
}

impl AcademicState {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Semester ------------------------------------------------------------------------

    /// Appended at the end - matches production's `[...current.semesters, semester]` (creation
    /// order is display order; see section 23 of the stage doc).
    pub fn add_semester(&mut self, semester: Semester) {
        self.semesters.push(semester);
    }

    pub fn rename_semester(&mut self, id: &SemesterId, name: String) -> bool {
        match self.semesters.iter_mut().find(|s| &s.id == id) {
            Some(semester) => {
                semester.name = name;
                true
            }
            None => false,
        }
    }

    /// Cascades exactly as production's `removeSemester` does: every course in this semester,
    /// and everything *those* courses' removal would itself cascade (tasks, exams, timetable
    /// events, and calendar entries belonging to those tasks), plus this semester's own holidays.
    /// Historical `sessions` are never touched - a session naming a now-deleted semester/course/
    /// task simply keeps that (now-dangling) id, exactly matching production (see section 20 of
    /// the stage doc, "Deletion semantics").
    pub fn remove_semester(&mut self, id: &SemesterId) {
        let course_ids: Vec<CourseId> = self
            .courses
            .iter()
            .filter(|c| &c.semester_id == id)
            .map(|c| c.id.clone())
            .collect();
        let task_ids: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|t| &t.semester_id == id)
            .map(|t| t.id.clone())
            .collect();

        self.calendar_entries
            .retain(|entry| !task_ids.contains(&entry.task_id));
        self.semesters.retain(|s| &s.id != id);
        self.courses.retain(|c| &c.semester_id != id);
        self.tasks.retain(|t| &t.semester_id != id);
        self.exams.retain(|e| &e.semester_id != id);
        self.timetable_events.retain(|e| &e.semester_id != id);
        self.holidays.retain(|h| &h.semester_id != id);
        let _ = course_ids; // kept for symmetry/documentation with remove_course; not needed further here
    }

    // --- Course ----------------------------------------------------------------------------

    pub fn add_course(&mut self, course: Course) {
        self.courses.push(course);
    }

    pub fn update_course(&mut self, course: Course) -> bool {
        match self.courses.iter_mut().find(|c| c.id == course.id) {
            Some(existing) => {
                *existing = course;
                true
            }
            None => false,
        }
    }

    /// Cascades exactly as production's `removeCourse`: tasks, exams, and timetable events for
    /// this course, plus calendar entries belonging to any of those tasks. Sessions untouched.
    pub fn remove_course(&mut self, id: &CourseId) {
        let task_ids: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|t| &t.course_id == id)
            .map(|t| t.id.clone())
            .collect();
        self.calendar_entries
            .retain(|entry| !task_ids.contains(&entry.task_id));
        self.courses.retain(|c| &c.id != id);
        self.tasks.retain(|t| &t.course_id != id);
        self.exams.retain(|e| &e.course_id != id);
        self.timetable_events.retain(|e| &e.course_id != id);
    }

    // --- Task ------------------------------------------------------------------------------

    pub fn add_task(&mut self, task: Task) {
        self.tasks.push(task);
    }

    pub fn update_task(&mut self, task: Task) -> bool {
        match self.tasks.iter_mut().find(|t| t.id == task.id) {
            Some(existing) => {
                *existing = task;
                true
            }
            None => false,
        }
    }

    /// Cascades exactly as production's `removeTask`: calendar entries and timetable events that
    /// reference this task. Sessions untouched.
    pub fn remove_task(&mut self, id: &TaskId) {
        self.tasks.retain(|t| &t.id != id);
        self.calendar_entries.retain(|entry| &entry.task_id != id);
        self.timetable_events.retain(|event| &event.task_id != id);
    }

    // --- Exam ------------------------------------------------------------------------------

    pub fn add_exam(&mut self, exam: Exam) {
        self.exams.push(exam);
    }

    pub fn update_exam(&mut self, exam: Exam) -> bool {
        match self.exams.iter_mut().find(|e| e.id == exam.id) {
            Some(existing) => {
                *existing = exam;
                true
            }
            None => false,
        }
    }

    /// No cascade - nothing in production references an `Exam` by id.
    pub fn remove_exam(&mut self, id: &ExamId) {
        self.exams.retain(|e| &e.id != id);
    }

    // --- StudySession ------------------------------------------------------------------------

    /// The exactly-once/idempotent insertion point every Timer-completion path (live, manual,
    /// recovered) goes through - see `session_service.rs`. `new_sessions` are prepended (newest
    /// first, matching production's `[...newSessions, ...existing]`); any whose `id` already
    /// exists is silently skipped rather than duplicated (this is *the* mechanism that makes a
    /// repeated recovery of the same abandoned Timer range safe - see the stage doc's Timer ->
    /// session integration section). Returns the ids that were actually inserted, so a caller can
    /// tell whether anything new happened. Lifetime totals accumulate only for what was actually
    /// inserted; history is pruned by `now` afterward.
    pub fn add_study_sessions(
        &mut self,
        new_sessions: Vec<StudySession>,
        now: WallTimestamp,
    ) -> Vec<SessionId> {
        let mut inserted_ids = Vec::new();
        let mut to_prepend = Vec::new();
        for session in new_sessions {
            if self
                .sessions
                .iter()
                .any(|existing| existing.id == session.id)
            {
                continue;
            }
            if matches!(
                session.kind,
                super::session::SessionKind::Study | super::session::SessionKind::Exam
            ) {
                self.lifetime_study_minutes += u64::from(session.minutes);
                self.lifetime_study_sessions += 1;
            }
            inserted_ids.push(session.id.clone());
            to_prepend.push(session);
        }
        to_prepend.append(&mut self.sessions);
        self.sessions = to_prepend;
        prune_session_history(&mut self.sessions, now);
        inserted_ids
    }

    pub fn remove_study_session(&mut self, id: &SessionId) {
        self.sessions.retain(|s| &s.id != id);
    }

    // --- Planner (lighter CRUD - append/replace/remove, no cascades of their own) ------------

    pub fn add_timetable_event(&mut self, event: TimetableEvent) {
        self.timetable_events.push(event);
    }

    pub fn remove_timetable_event(&mut self, id: &super::ids::TimetableEventId) {
        self.timetable_events.retain(|e| &e.id != id);
    }

    pub fn add_holiday(&mut self, holiday: Holiday) {
        self.holidays.push(holiday);
    }

    pub fn remove_holiday(&mut self, id: &super::ids::HolidayId) {
        self.holidays.retain(|h| &h.id != id);
    }

    pub fn add_daily_todo(&mut self, todo: DailyTodo) {
        self.daily_todos.push(todo);
    }

    pub fn remove_daily_todo(&mut self, id: &super::ids::DailyTodoId) {
        self.daily_todos.retain(|t| &t.id != id);
    }

    pub fn add_calendar_entry(&mut self, entry: CalendarEntry) {
        self.calendar_entries.push(entry);
    }

    pub fn remove_calendar_entry(&mut self, id: &super::ids::CalendarEntryId) {
        self.calendar_entries.retain(|e| &e.id != id);
    }

    /// Production's `toggleCalendarEntry` (App.tsx): flips a planned unit's completion and moves
    /// its task's `completedUnits` by the change in *whole* completed units (so two "1/2 unit"
    /// entries together advance the task by one, and un-ticking one of them takes it back). The
    /// Dashboard's checkbox uses this. Returns `false` when the entry does not exist.
    pub fn toggle_calendar_entry(
        &mut self,
        id: &super::ids::CalendarEntryId,
        now: WallTimestamp,
    ) -> bool {
        let Some(index) = self.calendar_entries.iter().position(|e| &e.id == id) else {
            return false;
        };
        let task_id = self.calendar_entries[index].task_id.clone();
        let before = completed_calendar_whole_units(&self.calendar_entries, &task_id);
        let completing = !self.calendar_entries[index].completed;
        {
            let entry = &mut self.calendar_entries[index];
            entry.completed = completing;
            entry.completed_at = completing.then_some(now);
        }
        let after = completed_calendar_whole_units(&self.calendar_entries, &task_id);
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
            let next = i64::from(task.completed_units) + (after - before);
            task.completed_units = next.clamp(0, i64::from(task.total_units)) as u32;
        }
        true
    }
}

/// `getCompletedCalendarWholeUnits`: `floor(sum of completed amounts + 0.0001)` for one task.
pub fn completed_calendar_whole_units(entries: &[CalendarEntry], task_id: &TaskId) -> i64 {
    let amount: f64 = entries
        .iter()
        .filter(|e| e.completed && &e.task_id == task_id)
        .map(|e| e.unit_amount.as_f64())
        .sum();
    (amount + 0.0001).floor() as i64
}

/// Standalone so `session_service.rs` and tests can call it directly without going through a full
/// `add_study_sessions` call (e.g. to test retention in isolation from insertion/dedup).
pub fn prune_session_history(sessions: &mut Vec<StudySession>, now: WallTimestamp) {
    let cutoff_millis = now
        .unix_millis
        .saturating_sub(SESSION_HISTORY_DAYS * 24 * 60 * 60 * 1000);
    sessions.retain(|session| session.ended_at.unix_millis >= cutoff_millis);
    sessions.truncate(SESSION_HISTORY_MAX);
}
