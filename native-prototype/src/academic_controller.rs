//! Application-layer academic/planner controller (Stage 16).
//!
//! Mirrors `timer_controller.rs`'s own shape deliberately: owns the pure domain state
//! ([`AcademicState`], `study-tracker-core`) plus a boxed persistence port, exposes CRUD methods
//! that mutate the domain and persist the result, and is the one place
//! [`crate::session_service::split_segments_into_daily_sessions`] is actually called from -
//! turning a Timer completion's [`TimerApplicationEffect::SessionRangeReady`] into real,
//! persisted [`StudySession`]s. `study-tracker-core` itself never sees any of this; it only
//! produces plain domain values.

use chrono::FixedOffset;

use crate::session_service::{
    fresh_session_id, recovered_session_id, split_segments_into_daily_sessions,
};
use crate::timer_controller::TimerApplicationEffect;
use study_tracker_core::academic::{
    AcademicState, CalendarEntry, Course, CourseId, DailyTodo, DailyTodoId, Exam, ExamId, Holiday,
    HolidayId, Semester, SemesterId, SessionId, Task, TaskId, TimetableEvent, TimetableEventId,
};
use study_tracker_core::timer::{ClockObservation, CompletionReason};

/// The persistence boundary for the academic domain, exactly analogous to
/// [`crate::timer_controller::TimerPersistencePort`]. `study-tracker-core` never sees this trait.
pub trait AcademicPersistencePort {
    fn persist(&mut self, state: &AcademicState);
    #[allow(dead_code)] // called by AcademicController::load_or_new; see its own doc comment
    fn load(&self) -> Option<AcademicState>;
}

/// The production-runtime default until a real caller wants otherwise: a no-op, matching
/// `NullPersistencePort`'s own reasoning exactly (see `timer_controller.rs`) - never wired into
/// `main.rs`'s actual startup path (which always uses `persistence::FileAcademicPersistencePort`),
/// kept for tests and any future scratch/demo caller.
#[derive(Debug, Default)]
pub struct NullAcademicPersistencePort;

impl AcademicPersistencePort for NullAcademicPersistencePort {
    fn persist(&mut self, _state: &AcademicState) {}
    fn load(&self) -> Option<AcademicState> {
        None
    }
}

pub struct AcademicController {
    state: AcademicState,
    persistence: Box<dyn AcademicPersistencePort>,
    /// Bumped by every mutation (`persist` is the single choke point all of them pass through). The
    /// Dashboard's derived metrics are cached against this counter: they are recomputed when it
    /// changes (a session was added, a task ticked off, ...) and never merely because a Timer
    /// display tick happened. Not part of equality and never persisted.
    revision: u64,
}

impl std::fmt::Debug for AcademicController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcademicController")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

// Same reasoning as `TimerController`'s own impls: the boxed port carries no comparable/clonable
// state that matters for equality - two controllers with the same domain state are equal
// regardless of which port instance backs them.
impl PartialEq for AcademicController {
    fn eq(&self, other: &Self) -> bool {
        self.state == other.state
    }
}
impl Clone for AcademicController {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            persistence: Box::new(NullAcademicPersistencePort),
            revision: self.revision,
        }
    }
}

impl AcademicController {
    pub fn new(persistence: Box<dyn AcademicPersistencePort>) -> Self {
        Self {
            state: AcademicState::new(),
            persistence,
            revision: 0,
        }
    }

    /// Real startup entry point: loads whatever the port already has (a fresh profile has
    /// nothing, exactly like `TimerController`'s own restore path treats a missing snapshot).
    pub fn load_or_new(persistence: Box<dyn AcademicPersistencePort>) -> Self {
        let mut state = persistence.load().unwrap_or_default();
        // Production derives scheduled tasks' unit counts on every load (Stage 19). In memory only:
        // a load never writes; the next real mutation persists the synced values.
        state.sync_task_units_from_schedule();
        Self {
            state,
            persistence,
            revision: 0,
        }
    }

    pub fn state(&self) -> &AcademicState {
        &self.state
    }

    /// Monotonic change counter; see the field's doc comment.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Bulk-replaces the whole academic domain and persists it - used by the
    /// `STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA` diagnostic hook (`main.rs`) to load a
    /// large, deterministic synthetic dataset for performance measurement
    /// (`docs/stage16-academic-domain.md` section 23-24), and available for a future real
    /// bulk-import UI to call instead of looping per-record `add_*` calls.
    pub fn replace_all(&mut self, state: AcademicState) {
        self.state = state;
        self.persist();
    }

    fn persist(&mut self) {
        // Every mutation passes through here, so this is where production's "after any change to
        // events/tasks/semesters/holidays" schedule-to-progress effect runs (Stage 19).
        self.state.sync_task_units_from_schedule();
        self.revision = self.revision.wrapping_add(1);
        self.persistence.persist(&self.state);
    }

    // --- Semester --------------------------------------------------------------------------
    pub fn add_semester(&mut self, semester: Semester) {
        self.state.add_semester(semester);
        self.persist();
    }
    #[allow(dead_code)] // real CRUD op, covered at the AcademicState level; no rename-UI yet
    pub fn rename_semester(&mut self, id: &SemesterId, name: String) -> bool {
        let changed = self.state.rename_semester(id, name);
        if changed {
            self.persist();
        }
        changed
    }
    pub fn remove_semester(&mut self, id: &SemesterId) {
        self.state.remove_semester(id);
        self.persist();
    }

    // --- Course ------------------------------------------------------------------------------
    pub fn add_course(&mut self, course: Course) {
        self.state.add_course(course);
        self.persist();
    }
    #[allow(dead_code)] // exercised by tests; a real edit-course UI is Stage 17+'s Planner polish
    pub fn update_course(&mut self, course: Course) -> bool {
        let changed = self.state.update_course(course);
        if changed {
            self.persist();
        }
        changed
    }
    pub fn remove_course(&mut self, id: &CourseId) {
        self.state.remove_course(id);
        self.persist();
    }

    // --- Task --------------------------------------------------------------------------------
    #[allow(dead_code)] // real CRUD op, covered at the AcademicState level; no Task-add UI yet
    pub fn add_task(&mut self, task: Task) {
        self.state.add_task(task);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn update_task(&mut self, task: Task) -> bool {
        let changed = self.state.update_task(task);
        if changed {
            self.persist();
        }
        changed
    }
    #[allow(dead_code)] // real CRUD op, covered at the AcademicState level; no Task-remove UI yet
    pub fn remove_task(&mut self, id: &TaskId) {
        self.state.remove_task(id);
        self.persist();
    }

    // --- Exam --------------------------------------------------------------------------------
    #[allow(dead_code)] // real CRUD op, covered at the AcademicState level; no Exam-add UI yet
    pub fn add_exam(&mut self, exam: Exam) {
        self.state.add_exam(exam);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn update_exam(&mut self, exam: Exam) -> bool {
        let changed = self.state.update_exam(exam);
        if changed {
            self.persist();
        }
        changed
    }
    #[allow(dead_code)] // real CRUD op, covered at the AcademicState level; no Exam-remove UI yet
    pub fn remove_exam(&mut self, id: &ExamId) {
        self.state.remove_exam(id);
        self.persist();
    }

    // --- Planner (kept for parity/testability; no Stage 16 UI surface uses these yet - see
    // docs/stage16-academic-domain.md section 22) ------------------------------------------
    #[allow(dead_code)]
    pub fn add_timetable_event(&mut self, event: TimetableEvent) {
        self.state.add_timetable_event(event);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn remove_timetable_event(&mut self, id: &TimetableEventId) {
        self.state.remove_timetable_event(id);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn add_holiday(&mut self, holiday: Holiday) {
        self.state.add_holiday(holiday);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn remove_holiday(&mut self, id: &HolidayId) {
        self.state.remove_holiday(id);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn add_daily_todo(&mut self, todo: DailyTodo) {
        self.state.add_daily_todo(todo);
        self.persist();
    }
    #[allow(dead_code)]
    pub fn remove_daily_todo(&mut self, id: &DailyTodoId) {
        self.state.remove_daily_todo(id);
        self.persist();
    }
    /// The Dashboard checkbox (production's `toggleCalendarEntry`): flips a planned unit and moves
    /// its task's completed units. Persists (and bumps the revision) only if the entry exists.
    pub fn toggle_calendar_entry(
        &mut self,
        id: &study_tracker_core::academic::CalendarEntryId,
        now: study_tracker_core::timer::WallTimestamp,
    ) -> bool {
        let changed = self.state.toggle_calendar_entry(id, now);
        if changed {
            self.persist();
        }
        changed
    }
    /// A Wabi-Sabi Dashboard mark on a lecture/sheet occurrence (`toggleTimetableOccurrence`).
    pub fn toggle_timetable_occurrence(&mut self, id: &TimetableEventId, date: &str) -> bool {
        let changed = self.state.toggle_timetable_occurrence(id, date);
        if changed {
            self.persist();
        }
        changed
    }
    /// A Wabi-Sabi Dashboard mark on a to-do (`toggleDailyTodoOccurrence`).
    pub fn toggle_daily_todo_occurrence(
        &mut self,
        id: &DailyTodoId,
        date: &str,
        now: study_tracker_core::timer::WallTimestamp,
    ) -> bool {
        let changed = self.state.toggle_daily_todo_occurrence(id, date, now);
        if changed {
            self.persist();
        }
        changed
    }
    #[allow(dead_code)]
    pub fn add_calendar_entry(&mut self, entry: CalendarEntry) {
        self.state.add_calendar_entry(entry);
        self.persist();
    }

    /// The Timer -> StudySession bridge (the brief's "hard acceptance item"). Called from both
    /// `AppModel::apply_timer_command` (live/manual completion) and
    /// `AppModel::with_timer_persistence` (startup recovery) with whatever
    /// `TimerApplicationEffect`s that command/restore just produced - the same call shape either
    /// way, matching production's own reuse of one session-building function for both situations.
    ///
    /// `local_offset` is the OS's local timezone offset, captured once by the caller (see
    /// `session_service`'s module docs for why this is a deliberate, documented simplification
    /// rather than a per-instant timezone lookup). Returns the ids that were newly inserted (empty
    /// if the effects contained no session-producing completion, or if every session they
    /// described was already present - the recovery-dedup case).
    pub fn route_timer_effects(
        &mut self,
        effects: &[TimerApplicationEffect],
        local_offset: FixedOffset,
        clock: ClockObservation,
    ) -> Vec<SessionId> {
        let mut inserted = Vec::new();
        for effect in effects {
            if let TimerApplicationEffect::SessionRangeReady {
                phase,
                reason,
                segments,
                context,
                preset_label,
            } = effect
            {
                let phase = *phase;
                let sessions = if *reason == CompletionReason::AbandonedRecovery {
                    split_segments_into_daily_sessions(
                        segments,
                        phase,
                        context,
                        preset_label,
                        local_offset,
                        move |start, end| recovered_session_id(phase, start, end),
                    )
                } else {
                    let now_millis = clock.wall.unix_millis;
                    split_segments_into_daily_sessions(
                        segments,
                        phase,
                        context,
                        preset_label,
                        local_offset,
                        move |_start, _end| fresh_session_id(now_millis),
                    )
                };
                inserted.extend(self.state.add_study_sessions(sessions, clock.wall));
            }
        }
        if !inserted.is_empty() {
            self.persist();
        }
        inserted
    }
}
