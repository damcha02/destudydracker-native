//! Academic-domain conversion for the production backup importer (Stage 16), extending
//! `migration.rs`'s pipeline to actually materialize Semester/Course/Task/Exam/StudySession and
//! the Planner value objects, instead of leaving them `Reserved` as Stage 15 did.
//!
//! Deliberately **field-level tolerant, per-record**, not a single strict `serde` struct
//! deserialize per entity (unlike the Timer section's `ProductionTimerState` in `migration.rs`,
//! which is a single nested object where an all-or-nothing parse is the right call). These are
//! *arrays* of independent records - production's own `storage.ts` normalizers
//! (`normalizeSemesters`, `normalizeExams`, ...) drop one malformed record via `flatMap` and keep
//! the rest, never rejecting the whole array over one bad entry; this module matches that
//! per-record policy exactly (see `docs/stage16-academic-domain.md` section 16, "Normalization").
//! A record missing a field central to its identity/meaning (e.g. a `Semester` with no `id`, an
//! `Exam` with no `examDate`) is skipped and reported; a record with only a malformed *optional*
//! field (e.g. an unparseable `startDate`) keeps the record and defaults just that field.
//!
//! **Ids are never regenerated.** Production's own id (a random `makeId()` string) becomes the
//! native id directly - see `docs/stage16-academic-domain.md` section 6 for why stable identity
//! across import matters (so a later re-import, or a Stage 17+ feature referencing an id from
//! before migration, keeps working).

use serde_json::{Map, Value};

use study_tracker_core::academic::{
    AcademicState, CalendarEntry, CalendarEntryId, Course, CourseId, DailyTodo, DailyTodoId, Exam,
    ExamId, Holiday, HolidayId, LocalDate, Priority, Semester, SemesterId, SemesterPhase,
    SessionId, SessionKind, StudySession, Task, TaskId, TaskSubtype, TimetableEvent,
    TimetableEventId, TimetableEventKind, UnitAmount,
};
use study_tracker_core::timer::WallTimestamp;

fn get_str<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    obj.get(key).and_then(Value::as_str)
}
fn get_string(obj: &Map<String, Value>, key: &str) -> Option<String> {
    get_str(obj, key).map(str::to_string)
}
fn get_string_or(obj: &Map<String, Value>, key: &str, default: &str) -> String {
    get_string(obj, key).unwrap_or_else(|| default.to_string())
}
fn get_f64_or(obj: &Map<String, Value>, key: &str, default: f64) -> f64 {
    obj.get(key).and_then(Value::as_f64).unwrap_or(default)
}
fn get_u32_or(obj: &Map<String, Value>, key: &str, default: u32) -> u32 {
    obj.get(key)
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| v as u32)
        .unwrap_or(default)
}
fn get_bool(obj: &Map<String, Value>, key: &str) -> bool {
    obj.get(key).and_then(Value::as_bool).unwrap_or(false)
}
fn get_local_date(obj: &Map<String, Value>, key: &str) -> Option<LocalDate> {
    get_str(obj, key).and_then(LocalDate::parse)
}
fn get_instant(obj: &Map<String, Value>, key: &str, fallback: WallTimestamp) -> WallTimestamp {
    get_str(obj, key)
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| WallTimestamp::from_unix_millis(dt.timestamp_millis()))
        .unwrap_or(fallback)
}
fn get_optional_instant(obj: &Map<String, Value>, key: &str) -> Option<WallTimestamp> {
    get_str(obj, key)
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| WallTimestamp::from_unix_millis(dt.timestamp_millis()))
}
fn as_object(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}
fn array_of_objects<'a>(
    state: &'a Map<String, Value>,
    key: &str,
) -> impl Iterator<Item = &'a Map<String, Value>> {
    state
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(as_object)
}

/// The whole Stage 16 academic conversion: every array-shaped section production's backup may
/// contain, each converted tolerantly (per-record). `import_time` is the fallback wall-clock
/// value used only for the rare record missing its own `createdAt` (matching production's own
/// `new Date().toISOString()` fallback in the same situation).
pub fn convert_academic(
    state: &Map<String, Value>,
    import_time: WallTimestamp,
) -> (AcademicState, Vec<String>) {
    let mut result = AcademicState::new();
    let mut warnings = Vec::new();

    for obj in array_of_objects(state, "semesters") {
        match convert_semester(obj, import_time) {
            Ok(semester) => result.add_semester(semester),
            Err(reason) => warnings.push(format!("semesters: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "courses") {
        match convert_course(obj, import_time) {
            Ok(course) => result.add_course(course),
            Err(reason) => warnings.push(format!("courses: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "tasks") {
        match convert_task(obj, import_time) {
            Ok(task) => result.add_task(task),
            Err(reason) => warnings.push(format!("tasks: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "exams") {
        match convert_exam(obj) {
            Ok(exam) => result.add_exam(exam),
            Err(reason) => warnings.push(format!("exams: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "timetableEvents") {
        match convert_timetable_event(obj, import_time) {
            Ok(event) => result.add_timetable_event(event),
            Err(reason) => warnings.push(format!("timetableEvents: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "holidays") {
        match convert_holiday(obj, import_time) {
            Ok(holiday) => result.add_holiday(holiday),
            Err(reason) => warnings.push(format!("holidays: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "dailyTodos") {
        match convert_daily_todo(obj, import_time) {
            Ok(todo) => result.add_daily_todo(todo),
            Err(reason) => warnings.push(format!("dailyTodos: skipped a record - {reason}")),
        }
    }
    for obj in array_of_objects(state, "calendarEntries") {
        match convert_calendar_entry(obj, import_time) {
            Ok(entry) => result.add_calendar_entry(entry),
            Err(reason) => warnings.push(format!("calendarEntries: skipped a record - {reason}")),
        }
    }
    // Sessions imported last and via `add_study_sessions` (not a plain push): this exercises the
    // exact same dedup-by-id and lifetime-totals-accumulation path a live Timer completion would,
    // so importing a backup that already contains a `recovered-...`-id session and later actually
    // recovering that same session natively cannot double-count it either.
    let mut sessions = Vec::new();
    for obj in array_of_objects(state, "sessions") {
        match convert_study_session(obj) {
            Ok(session) => sessions.push(session),
            Err(reason) => warnings.push(format!("sessions: skipped a record - {reason}")),
        }
    }
    // Reverse so the *earliest* session ends up prepended last, keeping the resulting order
    // newest-first exactly like production's own array does (`add_study_sessions` prepends).
    sessions.reverse();
    result.add_study_sessions(sessions, import_time);

    (result, warnings)
}

fn convert_semester(
    obj: &Map<String, Value>,
    import_time: WallTimestamp,
) -> Result<Semester, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let name = get_string(obj, "name").ok_or("missing name")?;
    Ok(Semester {
        id: SemesterId::new(id),
        name,
        created_at: get_instant(obj, "createdAt", import_time),
        start_date: get_local_date(obj, "startDate"),
        end_date: get_local_date(obj, "endDate"),
        phase: if get_str(obj, "phase") == Some("exam-prep") {
            SemesterPhase::ExamPrep
        } else {
            SemesterPhase::Semester
        },
        archived: get_bool(obj, "archived"),
        archived_at: get_optional_instant(obj, "archivedAt"),
    })
}

fn convert_course(obj: &Map<String, Value>, import_time: WallTimestamp) -> Result<Course, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let semester_id = get_string(obj, "semesterId").ok_or("missing semesterId")?;
    let name = get_string(obj, "name").ok_or("missing name")?;
    Ok(Course {
        id: CourseId::new(id),
        semester_id: SemesterId::new(semester_id),
        name,
        color: get_string_or(obj, "color", ""),
        target_grade: study_tracker_core::academic::clamp_target_grade(get_f64_or(
            obj,
            "targetGrade",
            4.0,
        )),
        created_at: get_instant(obj, "createdAt", import_time),
        external_url: get_string(obj, "externalUrl"),
    })
}

fn task_subtype(value: Option<&str>) -> TaskSubtype {
    match value {
        Some("Lecture") => TaskSubtype::Lecture,
        Some("Session") => TaskSubtype::Session,
        Some("Sheet") => TaskSubtype::Sheet,
        _ => TaskSubtype::Other,
    }
}

fn priority(value: Option<&str>) -> Priority {
    match value {
        Some("low") => Priority::Low,
        Some("high") => Priority::High,
        _ => Priority::Medium,
    }
}

fn convert_task(obj: &Map<String, Value>, import_time: WallTimestamp) -> Result<Task, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let semester_id = get_string(obj, "semesterId").ok_or("missing semesterId")?;
    let course_id = get_string(obj, "courseId").ok_or("missing courseId")?;
    let title = get_string(obj, "title").ok_or("missing title")?;
    Ok(Task {
        id: TaskId::new(id),
        semester_id: SemesterId::new(semester_id),
        course_id: CourseId::new(course_id),
        title,
        subtype: task_subtype(get_str(obj, "subtype")),
        unit_label: get_string_or(obj, "unitLabel", "Unit"),
        total_units: get_u32_or(obj, "totalUnits", 0),
        completed_units: get_u32_or(obj, "completedUnits", 0),
        due_date: get_local_date(obj, "dueDate"),
        priority: priority(get_str(obj, "priority")),
        notes: get_string_or(obj, "notes", ""),
        created_at: get_instant(obj, "createdAt", import_time),
    })
}

fn convert_exam(obj: &Map<String, Value>) -> Result<Exam, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let semester_id = get_string(obj, "semesterId").ok_or("missing semesterId")?;
    let course_id = get_string(obj, "courseId").ok_or("missing courseId")?;
    let title = get_string(obj, "title").ok_or("missing title")?;
    let exam_date = get_local_date(obj, "examDate").ok_or("missing or malformed examDate")?;
    Ok(Exam {
        id: ExamId::new(id),
        semester_id: SemesterId::new(semester_id),
        course_id: CourseId::new(course_id),
        title,
        exam_date,
        weight: get_f64_or(obj, "weight", 0.0),
        preparedness: get_f64_or(obj, "preparedness", 0.0),
        location: get_string_or(obj, "location", ""),
    })
}

fn session_kind(value: Option<&str>) -> Option<SessionKind> {
    match value {
        Some("study") => Some(SessionKind::Study),
        Some("exam") => Some(SessionKind::Exam),
        Some("break") => Some(SessionKind::Break),
        _ => None,
    }
}

fn convert_study_session(obj: &Map<String, Value>) -> Result<StudySession, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let kind = session_kind(get_str(obj, "kind")).ok_or("missing or unrecognized kind")?;
    let started_at =
        get_optional_instant(obj, "startedAt").ok_or("missing or malformed startedAt")?;
    let ended_at = get_optional_instant(obj, "endedAt").ok_or("missing or malformed endedAt")?;
    Ok(StudySession {
        id: SessionId::new(id),
        semester_id: get_string(obj, "semesterId").map(SemesterId::new),
        course_id: get_string(obj, "courseId").map(CourseId::new),
        task_id: get_string(obj, "taskId").map(TaskId::new),
        kind,
        goal: get_string_or(obj, "goal", ""),
        learned: get_string_or(obj, "learned", ""),
        blocker: get_string_or(obj, "blocker", ""),
        next_step: get_string_or(obj, "nextStep", ""),
        confidence: get_u32_or(obj, "confidence", 0).min(5) as u8,
        started_at,
        ended_at,
        minutes: get_u32_or(obj, "minutes", 0),
        preset_label: get_string_or(obj, "presetLabel", ""),
    })
}

fn timetable_event_kind(value: Option<&str>) -> TimetableEventKind {
    match value {
        Some("sheet-release") => TimetableEventKind::SheetRelease,
        Some("sheet-deadline") => TimetableEventKind::SheetDeadline,
        _ => TimetableEventKind::Occurrence,
    }
}

fn convert_timetable_event(
    obj: &Map<String, Value>,
    import_time: WallTimestamp,
) -> Result<TimetableEvent, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let semester_id = get_string(obj, "semesterId").ok_or("missing semesterId")?;
    let course_id = get_string(obj, "courseId").ok_or("missing courseId")?;
    let task_id = get_string(obj, "taskId").ok_or("missing taskId")?;
    let date = get_local_date(obj, "date").ok_or("missing or malformed date")?;
    Ok(TimetableEvent {
        id: TimetableEventId::new(id),
        semester_id: SemesterId::new(semester_id),
        course_id: CourseId::new(course_id),
        kind: timetable_event_kind(get_str(obj, "kind")),
        task_id: TaskId::new(task_id),
        label: get_string_or(obj, "label", ""),
        date,
        time: get_string_or(obj, "time", "00:00"),
        end_time: get_string(obj, "endTime"),
        repeat_weekly: get_bool(obj, "repeatWeekly"),
        recurrence_end_date: get_local_date(obj, "recurrenceEndDate"),
        // Per-occurrence overrides and completed-occurrence lists are a nested, comparatively
        // low-value structure for a first import pass (they refine an already-migrated recurring
        // event, never change its existence) - left empty rather than parsed field-by-field here;
        // preserved losslessly at the *raw* level regardless, since Reserved-section preservation
        // (see `migration.rs`) keeps the untouched original `timetableEvents` array recoverable.
        occurrence_overrides: Default::default(),
        url: get_string(obj, "url"),
        completed_occurrences: Vec::new(),
        created_at: get_instant(obj, "createdAt", import_time),
    })
}

fn convert_holiday(
    obj: &Map<String, Value>,
    import_time: WallTimestamp,
) -> Result<Holiday, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let semester_id = get_string(obj, "semesterId").ok_or("missing semesterId")?;
    let start_date = get_local_date(obj, "startDate").ok_or("missing or malformed startDate")?;
    let end_date = get_local_date(obj, "endDate").ok_or("missing or malformed endDate")?;
    Ok(Holiday {
        id: HolidayId::new(id),
        semester_id: SemesterId::new(semester_id),
        start_date,
        end_date,
        label: get_string_or(obj, "label", ""),
        created_at: get_instant(obj, "createdAt", import_time),
    })
}

fn convert_daily_todo(
    obj: &Map<String, Value>,
    import_time: WallTimestamp,
) -> Result<DailyTodo, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let date = get_local_date(obj, "date").ok_or("missing or malformed date")?;
    Ok(DailyTodo {
        id: DailyTodoId::new(id),
        date,
        time: get_string(obj, "time"),
        end_time: get_string(obj, "endTime"),
        title: get_string_or(obj, "title", ""),
        notes: get_string_or(obj, "notes", ""),
        completed: get_bool(obj, "completed"),
        completed_at: get_optional_instant(obj, "completedAt"),
        created_at: get_instant(obj, "createdAt", import_time),
        repeat_weekly: get_bool(obj, "repeatWeekly"),
        completed_occurrences: Vec::new(),
        recurrence_end_date: get_local_date(obj, "recurrenceEndDate"),
        skipped_occurrences: Vec::new(),
        occurrence_times: Default::default(),
    })
}

fn convert_calendar_entry(
    obj: &Map<String, Value>,
    import_time: WallTimestamp,
) -> Result<CalendarEntry, String> {
    let id = get_string(obj, "id").ok_or("missing id")?;
    let task_id = get_string(obj, "taskId").ok_or("missing taskId")?;
    let date = get_local_date(obj, "date").ok_or("missing or malformed date")?;
    Ok(CalendarEntry {
        id: CalendarEntryId::new(id),
        task_id: TaskId::new(task_id),
        date,
        unit_amount: UnitAmount::from_f64(get_f64_or(obj, "unitAmount", 1.0)),
        unit_start: obj
            .get("unitStart")
            .and_then(Value::as_f64)
            .map(|v| v as u32),
        completed: get_bool(obj, "completed"),
        completed_at: get_optional_instant(obj, "completedAt"),
        created_at: get_instant(obj, "createdAt", import_time),
        start_time: get_string(obj, "startTime"),
        end_time: get_string(obj, "endTime"),
        ad_hoc_title: get_string(obj, "adHocTitle"),
        ad_hoc_semester_id: get_string(obj, "adHocSemesterId").map(SemesterId::new),
        ad_hoc_course_id: get_string(obj, "adHocCourseId").map(CourseId::new),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    #[test]
    fn a_full_state_converts_every_section() {
        let state = obj(json!({
            "semesters": [{"id": "s1", "name": "Fall", "createdAt": "2026-01-01T00:00:00.000Z"}],
            "courses": [{"id": "c1", "semesterId": "s1", "name": "Analysis II", "color": "blue", "targetGrade": 5.5, "createdAt": "2026-01-01T00:00:00.000Z"}],
            "tasks": [{"id": "t1", "semesterId": "s1", "courseId": "c1", "title": "Sheet 3", "subtype": "Sheet", "unitLabel": "Sheet", "createdAt": "2026-01-01T00:00:00.000Z"}],
            "exams": [{"id": "e1", "semesterId": "s1", "courseId": "c1", "title": "Midterm", "examDate": "2026-12-01"}],
            "sessions": [{"id": "sess1", "kind": "study", "startedAt": "2026-09-27T10:00:00.000Z", "endedAt": "2026-09-27T10:30:00.000Z", "minutes": 30, "presetLabel": "Pomodoro 25/5"}],
            "timetableEvents": [{"id": "te1", "semesterId": "s1", "courseId": "c1", "taskId": "t1", "kind": "occurrence", "date": "2026-09-28", "time": "09:00"}],
            "holidays": [{"id": "h1", "semesterId": "s1", "startDate": "2026-12-20", "endDate": "2027-01-05", "label": "Winter break"}],
            "dailyTodos": [{"id": "d1", "date": "2026-09-28", "title": "Buy notebook"}],
            "calendarEntries": [{"id": "ce1", "taskId": "t1", "date": "2026-09-28", "unitAmount": 1}],
        }));

        let (result, warnings) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        assert!(
            warnings.is_empty(),
            "a clean fixture must produce no warnings: {warnings:?}"
        );
        assert_eq!(result.semesters.len(), 1);
        assert_eq!(result.courses.len(), 1);
        assert_eq!(result.tasks.len(), 1);
        assert_eq!(result.exams.len(), 1);
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(result.timetable_events.len(), 1);
        assert_eq!(result.holidays.len(), 1);
        assert_eq!(result.daily_todos.len(), 1);
        assert_eq!(result.calendar_entries.len(), 1);
        assert_eq!(result.courses[0].target_grade, 5.5);
        assert_eq!(result.lifetime_study_minutes, 30);
    }

    #[test]
    fn a_record_missing_its_id_is_skipped_and_reported_not_rejecting_the_whole_import() {
        let state = obj(json!({
            "semesters": [
                {"name": "no id here"},
                {"id": "s1", "name": "Fall"},
            ],
        }));
        let (result, warnings) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        assert_eq!(
            result.semesters.len(),
            1,
            "the good record still comes through"
        );
        assert_eq!(result.semesters[0].id.as_str(), "s1");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("semesters"));
    }

    #[test]
    fn a_malformed_optional_field_defaults_instead_of_rejecting_the_record() {
        let state = obj(json!({
            "semesters": [{"id": "s1", "name": "Fall", "startDate": "not-a-date"}],
        }));
        let (result, warnings) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        assert_eq!(
            result.semesters.len(),
            1,
            "a malformed *optional* field must not drop the record"
        );
        assert_eq!(result.semesters[0].start_date, None);
        assert!(warnings.is_empty());
    }

    #[test]
    fn ids_are_preserved_exactly_never_regenerated() {
        let state = obj(json!({
            "courses": [{"id": "the-exact-original-id", "semesterId": "s1", "name": "x"}],
        }));
        let (result, _warnings) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        assert_eq!(result.courses[0].id.as_str(), "the-exact-original-id");
    }

    #[test]
    fn an_exam_missing_its_date_is_skipped() {
        let state = obj(json!({
            "exams": [{"id": "e1", "semesterId": "s1", "courseId": "c1", "title": "Midterm"}],
        }));
        let (result, warnings) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        assert!(result.exams.is_empty());
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn duplicate_session_ids_across_two_imports_are_not_double_counted() {
        let state = obj(json!({
            "sessions": [{"id": "sess1", "kind": "study", "startedAt": "2026-09-27T10:00:00.000Z", "endedAt": "2026-09-27T10:30:00.000Z", "minutes": 30}],
        }));
        let (mut result, _warnings) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        assert_eq!(result.lifetime_study_minutes, 30);
        assert_eq!(result.sessions.len(), 1);

        // Importing the exact same backup again into the same (already-populated) state must not
        // double the lifetime total or the session count - proves the conversion's own sessions
        // go through `AcademicState::add_study_sessions`'s dedup, not a plain append.
        let (again, _warnings2) = convert_academic(&state, WallTimestamp::from_unix_millis(0));
        let newly_inserted =
            result.add_study_sessions(again.sessions, WallTimestamp::from_unix_millis(0));
        assert!(
            newly_inserted.is_empty(),
            "the duplicate session id must not be reported as newly inserted"
        );
        assert_eq!(result.sessions.len(), 1);
        assert_eq!(
            result.lifetime_study_minutes, 30,
            "re-adding the same import must not double lifetime totals"
        );
    }
}
