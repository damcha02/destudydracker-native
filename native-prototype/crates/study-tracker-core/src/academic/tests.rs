use super::*;
use crate::timer::WallTimestamp;

fn wall(seconds: i64) -> WallTimestamp {
    WallTimestamp::from_unix_millis(seconds * 1000)
}

fn semester(id: &str) -> Semester {
    Semester::new(SemesterId::from(id), format!("Semester {id}"), wall(0))
}

fn course(id: &str, semester_id: &str) -> Course {
    Course::new(
        CourseId::from(id),
        SemesterId::from(semester_id),
        format!("Course {id}"),
        "blue".to_string(),
        wall(0),
    )
}

fn task(id: &str, semester_id: &str, course_id: &str) -> Task {
    Task::new(
        TaskId::from(id),
        SemesterId::from(semester_id),
        CourseId::from(course_id),
        format!("Task {id}"),
        TaskSubtype::Other,
        "Unit".to_string(),
        wall(0),
    )
}

fn exam(id: &str, semester_id: &str, course_id: &str) -> Exam {
    Exam::new(
        ExamId::from(id),
        SemesterId::from(semester_id),
        CourseId::from(course_id),
        format!("Exam {id}"),
        LocalDate::parse("2026-12-01").unwrap(),
    )
}

fn session(id: &str, ended_at_secs: i64, minutes: u32, kind: SessionKind) -> StudySession {
    StudySession {
        id: SessionId::from(id),
        semester_id: None,
        course_id: None,
        task_id: None,
        kind,
        goal: String::new(),
        learned: String::new(),
        blocker: String::new(),
        next_step: String::new(),
        confidence: 0,
        started_at: wall(ended_at_secs - i64::from(minutes) * 60),
        ended_at: wall(ended_at_secs),
        minutes,
        preset_label: "Pomodoro 25/5".to_string(),
    }
}

// --- Semester ---------------------------------------------------------------------------

#[test]
fn semesters_are_appended_in_creation_order() {
    let mut state = AcademicState::new();
    state.add_semester(semester("a"));
    state.add_semester(semester("b"));
    state.add_semester(semester("c"));
    let ids: Vec<&str> = state.semesters.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["a", "b", "c"]);
}

#[test]
fn renaming_a_semester_updates_it_in_place() {
    let mut state = AcademicState::new();
    state.add_semester(semester("a"));
    assert!(state.rename_semester(&SemesterId::from("a"), "Fall 2026".to_string()));
    assert_eq!(state.semesters[0].name, "Fall 2026");
    assert!(!state.rename_semester(&SemesterId::from("missing"), "x".to_string()));
}

#[test]
fn removing_a_semester_cascades_to_its_courses_tasks_exams_timetable_and_holidays() {
    let mut state = AcademicState::new();
    state.add_semester(semester("s1"));
    state.add_semester(semester("s2"));
    state.add_course(course("c1", "s1"));
    state.add_task(task("t1", "s1", "c1"));
    state.add_exam(exam("e1", "s1", "c1"));
    state.add_holiday(Holiday {
        id: HolidayId::from("h1"),
        semester_id: SemesterId::from("s1"),
        start_date: LocalDate::parse("2026-12-20").unwrap(),
        end_date: LocalDate::parse("2027-01-05").unwrap(),
        label: "Winter break".to_string(),
        created_at: wall(0),
    });
    state.add_calendar_entry(CalendarEntry {
        id: CalendarEntryId::from("ce1"),
        task_id: TaskId::from("t1"),
        date: LocalDate::parse("2026-10-01").unwrap(),
        unit_amount: UnitAmount::Whole,
        unit_start: None,
        completed: false,
        completed_at: None,
        created_at: wall(0),
        start_time: None,
        end_time: None,
        ad_hoc_title: None,
        ad_hoc_semester_id: None,
        ad_hoc_course_id: None,
    });

    state.remove_semester(&SemesterId::from("s1"));

    assert!(state.semesters.iter().all(|s| s.id.as_str() != "s1"));
    assert_eq!(state.semesters.len(), 1, "s2 must survive");
    assert!(state.courses.is_empty());
    assert!(state.tasks.is_empty());
    assert!(state.exams.is_empty());
    assert!(state.holidays.is_empty());
    assert!(
        state.calendar_entries.is_empty(),
        "calendar entries for a deleted task must cascade too"
    );
}

#[test]
fn removing_a_semester_never_touches_historical_sessions() {
    let mut state = AcademicState::new();
    state.add_semester(semester("s1"));
    let mut s = session("sess1", 1000, 30, SessionKind::Study);
    s.semester_id = Some(SemesterId::from("s1"));
    state.add_study_sessions(vec![s], wall(2000));

    state.remove_semester(&SemesterId::from("s1"));

    assert_eq!(
        state.sessions.len(),
        1,
        "the session must survive semester deletion"
    );
    assert_eq!(
        state.sessions[0].semester_id,
        Some(SemesterId::from("s1")),
        "the dangling reference is preserved, not nulled out or cascade-deleted"
    );
}

// --- Course -------------------------------------------------------------------------------

#[test]
fn removing_a_course_cascades_to_its_tasks_exams_timetable_and_their_calendar_entries() {
    let mut state = AcademicState::new();
    state.add_course(course("c1", "s1"));
    state.add_task(task("t1", "s1", "c1"));
    state.add_exam(exam("e1", "s1", "c1"));
    state.add_calendar_entry(CalendarEntry {
        id: CalendarEntryId::from("ce1"),
        task_id: TaskId::from("t1"),
        date: LocalDate::parse("2026-10-01").unwrap(),
        unit_amount: UnitAmount::Whole,
        unit_start: None,
        completed: false,
        completed_at: None,
        created_at: wall(0),
        start_time: None,
        end_time: None,
        ad_hoc_title: None,
        ad_hoc_semester_id: None,
        ad_hoc_course_id: None,
    });

    state.remove_course(&CourseId::from("c1"));

    assert!(state.courses.is_empty());
    assert!(state.tasks.is_empty());
    assert!(state.exams.is_empty());
    assert!(state.calendar_entries.is_empty());
}

#[test]
fn updating_a_missing_course_reports_failure_rather_than_inserting() {
    let mut state = AcademicState::new();
    assert!(!state.update_course(course("ghost", "s1")));
    assert!(state.courses.is_empty());
}

#[test]
fn a_course_may_reference_a_semester_id_that_does_not_exist() {
    // Production never validates this at creation time (addCourse has no semester-existence
    // check beyond "a semesterId was chosen from the dropdown") - dangling references are a
    // deletion-order artifact, not something creation guards against.
    let mut state = AcademicState::new();
    state.add_course(course("c1", "does-not-exist"));
    assert_eq!(state.courses.len(), 1);
}

// --- Task ---------------------------------------------------------------------------------

#[test]
fn removing_a_task_cascades_to_its_calendar_entries_and_timetable_events_only() {
    let mut state = AcademicState::new();
    state.add_task(task("t1", "s1", "c1"));
    state.add_calendar_entry(CalendarEntry {
        id: CalendarEntryId::from("ce1"),
        task_id: TaskId::from("t1"),
        date: LocalDate::parse("2026-10-01").unwrap(),
        unit_amount: UnitAmount::Whole,
        unit_start: None,
        completed: false,
        completed_at: None,
        created_at: wall(0),
        start_time: None,
        end_time: None,
        ad_hoc_title: None,
        ad_hoc_semester_id: None,
        ad_hoc_course_id: None,
    });
    state.add_timetable_event(TimetableEvent {
        id: TimetableEventId::from("te1"),
        semester_id: SemesterId::from("s1"),
        course_id: CourseId::from("c1"),
        kind: TimetableEventKind::Occurrence,
        task_id: TaskId::from("t1"),
        label: "Lecture".to_string(),
        date: LocalDate::parse("2026-10-01").unwrap(),
        time: "09:00".to_string(),
        end_time: Some("10:00".to_string()),
        repeat_weekly: true,
        recurrence_end_date: None,
        occurrence_overrides: Default::default(),
        url: None,
        completed_occurrences: Vec::new(),
        created_at: wall(0),
    });
    state.add_exam(exam("e1", "s1", "c1")); // must NOT be removed by a task deletion

    state.remove_task(&TaskId::from("t1"));

    assert!(state.tasks.is_empty());
    assert!(state.calendar_entries.is_empty());
    assert!(state.timetable_events.is_empty());
    assert_eq!(
        state.exams.len(),
        1,
        "exams are not cascaded from a task deletion"
    );
}

#[test]
fn a_task_may_reference_a_course_id_that_does_not_exist() {
    let mut state = AcademicState::new();
    state.add_task(task("t1", "s1", "does-not-exist"));
    assert_eq!(state.tasks.len(), 1);
}

// --- Exam ---------------------------------------------------------------------------------

#[test]
fn exams_have_no_cascade_and_preserve_creation_order() {
    let mut state = AcademicState::new();
    state.add_exam(exam("e1", "s1", "c1"));
    state.add_exam(exam("e2", "s1", "c1"));
    state.remove_course(&CourseId::from("c1")); // exams DO cascade from a course...
    assert!(state.exams.is_empty());

    let mut state2 = AcademicState::new();
    state2.add_exam(exam("e1", "s1", "c1"));
    state2.add_exam(exam("e2", "s1", "c1"));
    state2.remove_exam(&ExamId::from("e1")); // ...but removing one exam only removes that one
    assert_eq!(state2.exams.len(), 1);
    assert_eq!(state2.exams[0].id.as_str(), "e2");
}

// --- StudySession ---------------------------------------------------------------------------

#[test]
fn new_sessions_are_prepended_newest_first() {
    let mut state = AcademicState::new();
    state.add_study_sessions(
        vec![session("s1", 100, 10, SessionKind::Study)],
        wall(1_000_000),
    );
    state.add_study_sessions(
        vec![session("s2", 200, 10, SessionKind::Study)],
        wall(1_000_000),
    );
    let ids: Vec<&str> = state.sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        ["s2", "s1"],
        "the most recently added session comes first"
    );
}

#[test]
fn re_adding_a_session_with_the_same_id_is_a_no_op_not_a_duplicate() {
    let mut state = AcademicState::new();
    let inserted = state.add_study_sessions(
        vec![session("dup", 100, 25, SessionKind::Study)],
        wall(1_000_000),
    );
    assert_eq!(inserted.len(), 1);
    let inserted_again = state.add_study_sessions(
        vec![session("dup", 100, 25, SessionKind::Study)],
        wall(1_000_000),
    );
    assert!(
        inserted_again.is_empty(),
        "the duplicate id must not be reported as newly inserted"
    );
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(
        state.lifetime_study_minutes, 25,
        "lifetime totals must not double-count either"
    );
}

#[test]
fn lifetime_totals_only_count_study_and_exam_sessions_never_break() {
    let mut state = AcademicState::new();
    state.add_study_sessions(
        vec![
            session("study", 100, 25, SessionKind::Study),
            session("exam", 200, 90, SessionKind::Exam),
            session("break", 300, 5, SessionKind::Break),
        ],
        wall(1_000_000),
    );
    assert_eq!(state.lifetime_study_minutes, 115);
    assert_eq!(state.lifetime_study_sessions, 2);
    assert_eq!(
        state.sessions.len(),
        3,
        "a Break session is still stored in history..."
    );
}

#[test]
fn session_history_older_than_365_days_is_pruned() {
    let mut state = AcademicState::new();
    let now_secs = 400 * 24 * 60 * 60; // day 400
    let old = session("old", 10 * 24 * 60 * 60, 30, SessionKind::Study); // day 10 - 390 days ago
    let recent = session("recent", (400 - 5) * 24 * 60 * 60, 30, SessionKind::Study); // 5 days ago
    state.add_study_sessions(vec![old, recent], wall(now_secs));
    let ids: Vec<&str> = state.sessions.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        ["recent"],
        "a session older than SESSION_HISTORY_DAYS is dropped"
    );
}

#[test]
fn session_history_is_capped_at_the_maximum_count() {
    let mut state = AcademicState::new();
    let now_secs = 10_000_000i64;
    let now = wall(now_secs);
    // All well within the 365-day age cutoff, so only the count cap is exercised here.
    let many: Vec<StudySession> = (0..(SESSION_HISTORY_MAX + 10))
        .map(|i| {
            session(
                &format!("s{i}"),
                now_secs - 1000 + i as i64,
                1,
                SessionKind::Study,
            )
        })
        .collect();
    state.add_study_sessions(many, now);
    assert_eq!(state.sessions.len(), SESSION_HISTORY_MAX);
}

#[test]
fn removing_one_session_does_not_affect_others_or_lifetime_totals() {
    let mut state = AcademicState::new();
    state.add_study_sessions(
        vec![
            session("a", 100, 10, SessionKind::Study),
            session("b", 200, 20, SessionKind::Study),
        ],
        wall(1_000_000),
    );
    state.remove_study_session(&SessionId::from("a"));
    assert_eq!(state.sessions.len(), 1);
    assert_eq!(state.sessions[0].id.as_str(), "b");
    assert_eq!(
        state.lifetime_study_minutes, 30,
        "lifetime totals are a running counter, not recomputed from the (now-pruned) list"
    );
}

// --- Planner value objects -----------------------------------------------------------------

#[test]
fn planner_items_append_and_remove_independently() {
    let mut state = AcademicState::new();
    state.add_daily_todo(DailyTodo {
        id: DailyTodoId::from("d1"),
        date: LocalDate::parse("2026-10-01").unwrap(),
        time: None,
        end_time: None,
        title: "Buy notebook".to_string(),
        notes: String::new(),
        completed: false,
        completed_at: None,
        created_at: wall(0),
        repeat_weekly: false,
        completed_occurrences: Vec::new(),
        recurrence_end_date: None,
        skipped_occurrences: Vec::new(),
        occurrence_times: Default::default(),
    });
    assert_eq!(state.daily_todos.len(), 1);
    state.remove_daily_todo(&DailyTodoId::from("d1"));
    assert!(state.daily_todos.is_empty());
}

#[test]
fn unit_amount_normalizes_exactly_like_production() {
    assert_eq!(UnitAmount::from_f64(1.0), UnitAmount::Whole);
    assert_eq!(UnitAmount::from_f64(0.5), UnitAmount::Half);
    assert_eq!(UnitAmount::from_f64(0.25), UnitAmount::Quarter);
    assert_eq!(
        UnitAmount::from_f64(0.75),
        UnitAmount::Whole,
        "anything else defaults to whole"
    );
    assert_eq!(UnitAmount::from_f64(-1.0), UnitAmount::Whole);
}
