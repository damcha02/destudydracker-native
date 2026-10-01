//! Wabi-Sabi derivation semantics on small hand-built states. The golden comparison against the
//! running production app (whole Dashboard/sidebar/Quiet text for four fixtures) lives in the app
//! crate (`src/wabi_view.rs` tests), next to the backup importer it needs.

use super::*;
use crate::academic::{
    unit_decrement_for, CalendarEntry, CalendarEntryId, CourseId, DailyTodoId, ExamId, ExamKind,
    LocalDate, OccurrenceOverride, Priority, SemesterId, TaskId, TimetableEventId, UnitAmount,
};
use crate::dashboard::{DashboardInput, FixedOffsetClock, DEFAULT_DAILY_GOAL_MINUTES};
use crate::timer::WallTimestamp;

const ZURICH: FixedOffsetClock = FixedOffsetClock::new(2 * 3600);

fn d(s: &str) -> CivilDate {
    CivilDate::parse_iso(s).unwrap()
}
fn ld(s: &str) -> LocalDate {
    LocalDate::parse(s).unwrap()
}
fn ts() -> WallTimestamp {
    WallTimestamp::from_unix_millis(1_790_000_000_000)
}
/// 2026-09-30 12:00 +02:00
fn now() -> WallTimestamp {
    WallTimestamp::from_unix_millis(d("2026-09-30").days() * 86_400_000 + 10 * 3_600_000)
}

fn semester(id: &str, start: &str, end: &str) -> Semester {
    let mut s = Semester::new(SemesterId::new(id), format!("Sem {id}"), ts());
    s.start_date = Some(ld(start));
    s.end_date = Some(ld(end));
    s
}
fn course(id: &str, semester: &str, name: &str) -> Course {
    Course {
        id: CourseId::new(id),
        semester_id: SemesterId::new(semester),
        name: name.into(),
        color: "#8fb4ff".into(),
        target_grade: 5.0,
        created_at: ts(),
        external_url: None,
    }
}
fn task(id: &str, course: &str, title: &str, subtype: TaskSubtype, total: u32, done: u32) -> Task {
    let mut t = Task::new(
        TaskId::new(id),
        SemesterId::new("s"),
        CourseId::new(course),
        title.into(),
        subtype,
        "Sheet".into(),
        ts(),
    );
    t.total_units = total;
    t.completed_units = done;
    t
}
#[allow(clippy::too_many_arguments)]
fn event(
    id: &str,
    course: &str,
    task: &str,
    kind: TimetableEventKind,
    date: &str,
    time: &str,
    end: Option<&str>,
    weekly: bool,
) -> TimetableEvent {
    TimetableEvent {
        id: TimetableEventId::new(id),
        semester_id: SemesterId::new("s"),
        course_id: CourseId::new(course),
        kind,
        task_id: TaskId::new(task),
        label: format!("Label {id}"),
        date: ld(date),
        time: time.into(),
        end_time: end.map(str::to_string),
        repeat_weekly: weekly,
        recurrence_end_date: None,
        occurrence_overrides: Default::default(),
        url: None,
        completed_occurrences: Vec::new(),
        created_at: ts(),
    }
}
fn todo(id: &str, title: &str, date: &str, time: Option<&str>, weekly: bool) -> DailyTodo {
    DailyTodo {
        id: DailyTodoId::new(id),
        date: ld(date),
        time: time.map(str::to_string),
        end_time: None,
        title: title.into(),
        notes: String::new(),
        completed: false,
        completed_at: None,
        created_at: ts(),
        repeat_weekly: weekly,
        completed_occurrences: Vec::new(),
        recurrence_end_date: None,
        skipped_occurrences: Vec::new(),
        occurrence_times: Default::default(),
    }
}
fn entry(id: &str, task: &str, date: &str, start: Option<&str>, completed: bool) -> CalendarEntry {
    CalendarEntry {
        id: CalendarEntryId::new(id),
        task_id: TaskId::new(task),
        date: ld(date),
        unit_amount: UnitAmount::Whole,
        unit_start: None,
        completed,
        completed_at: None,
        created_at: ts(),
        start_time: start.map(str::to_string),
        end_time: None,
        ad_hoc_title: None,
        ad_hoc_semester_id: None,
        ad_hoc_course_id: None,
    }
}

fn base_state() -> AcademicState {
    let mut s = AcademicState::new();
    s.semesters.push(semester("s", "2026-08-20", "2026-12-20"));
    s.courses.push(course("c1", "s", "Analysis II"));
    s.courses.push(course("c2", "s", "Physics"));
    s
}

fn dashboard(state: &AcademicState, selected: Option<&str>) -> WabiDashboard {
    let metrics = DashboardMetrics::compute(DashboardInput {
        state,
        now: now(),
        clock: &ZURICH,
        daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
    });
    wabi_dashboard(WabiInput {
        state,
        metrics: &metrics,
        clock: &ZURICH,
        selected_task_id: selected,
        twelve_hour: false,
    })
}

#[test]
fn an_empty_profile_has_the_production_placeholder_and_no_actions() {
    let state = AcademicState::new();
    let dash = dashboard(&state, None);
    assert_eq!(dash.today_label, "Wednesday, Sep 30, 2026");
    assert_eq!(dash.remaining_label, "2h to today's goal");
    assert_eq!(dash.one_thing.title, "Plan the next study block");
    assert_eq!(
        dash.one_thing.meta,
        "No task pinned for today \u{b7} open the planner calendar to place one unit."
    );
    assert_eq!(dash.one_thing.start, None);
    assert_eq!(dash.one_thing.mark_done, None);
    assert!(dash.deadlines.is_empty() && dash.planned.is_empty());
}

#[test]
fn the_daily_timeline_sorts_by_time_and_puts_untimed_rows_last() {
    let mut state = base_state();
    state
        .tasks
        .push(task("t1", "c1", "Lectures", TaskSubtype::Lecture, 0, 0));
    state.timetable_events.push(event(
        "lec",
        "c1",
        "t1",
        TimetableEventKind::Occurrence,
        "2026-09-02",
        "14:15",
        Some("16:00"),
        true,
    ));
    state
        .daily_todos
        .push(todo("any", "Any time", "2026-09-30", None, false));
    state
        .daily_todos
        .push(todo("early", "Early", "2026-09-30", Some("08:00"), false));
    let dash = dashboard(&state, None);
    let titles: Vec<&str> = dash.planned.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, ["Early", "Label lec", "Any time"]);
    assert_eq!(dash.planned[1].due, "14:15\u{2013}16:00");
    assert_eq!(dash.planned[1].subject, "Analysis II");
    assert_eq!(dash.planned[2].due, "any time");
    assert_eq!(dash.planned[0].subject, "To-do");
}

#[test]
fn repeating_todos_follow_their_weekday_skips_and_per_date_completion() {
    let mut weekly = todo("w", "Weekly review", "2026-09-16", Some("17:00"), true);
    assert!(todo_occurs_on(&weekly, d("2026-09-30")));
    assert!(!todo_occurs_on(&weekly, d("2026-09-29")));
    assert!(
        !todo_occurs_on(&weekly, d("2026-09-09")),
        "before the anchor"
    );
    weekly.skipped_occurrences.push("2026-09-30".into());
    assert!(!todo_occurs_on(&weekly, d("2026-09-30")));
    weekly.skipped_occurrences.clear();
    weekly.recurrence_end_date = Some(ld("2026-09-29"));
    assert!(!todo_occurs_on(&weekly, d("2026-09-30")), "series ended");

    let mut state = base_state();
    let mut w = todo("w", "Weekly review", "2026-09-16", Some("17:00"), true);
    w.completed_occurrences.push("2026-09-30".into());
    w.occurrence_times
        .insert("2026-09-30".into(), (Some("18:30".into()), None));
    state.daily_todos.push(w);
    let dash = dashboard(&state, None);
    assert!(dash.planned[0].completed);
    assert_eq!(
        dash.planned[0].due, "18:30",
        "this-occurrence-only time override"
    );
}

#[test]
fn coming_up_lists_released_unsolved_deadlines_nearest_first_with_next_marker() {
    let mut state = base_state();
    state
        .tasks
        .push(task("sh", "c1", "Exercise Sheet", TaskSubtype::Sheet, 0, 0));
    state
        .tasks
        .push(task("sh2", "c2", "Problem Set", TaskSubtype::Sheet, 0, 0));
    // weekly sheets: released Monday, due the next Monday 23:59 (gap 7 days)
    state.timetable_events.push(event(
        "rel",
        "c1",
        "sh",
        TimetableEventKind::SheetRelease,
        "2026-09-07",
        "08:00",
        None,
        true,
    ));
    state.timetable_events.push(event(
        "due",
        "c1",
        "sh",
        TimetableEventKind::SheetDeadline,
        "2026-09-14",
        "23:59",
        None,
        true,
    ));
    // a sheet whose release is still in the future is not out yet
    state.timetable_events.push(event(
        "rel2",
        "c2",
        "sh2",
        TimetableEventKind::SheetRelease,
        "2026-10-02",
        "08:00",
        None,
        false,
    ));
    state.timetable_events.push(event(
        "due2",
        "c2",
        "sh2",
        TimetableEventKind::SheetDeadline,
        "2026-10-09",
        "12:00",
        None,
        false,
    ));
    let dash = dashboard(&state, None);
    // weekly due dates from today: Oct 5 (released Sep 28), Oct 12 (released Oct 5 - future!) ...
    assert_eq!(dash.deadlines.len(), 1, "{:?}", dash.deadlines);
    let first = &dash.deadlines[0];
    assert!(first.next);
    assert_eq!(first.status, "Analysis II \u{b7} released Sep 28");
    assert_eq!(first.due, "Oct 5 \u{b7} 23:59");
    assert_eq!(first.due_relative, "in 5 days");
    assert_eq!(first.task_id.as_deref(), Some("sh"));

    // ticking the occurrence removes it from COMING UP
    assert!(state.toggle_timetable_occurrence(&TimetableEventId::new("due"), "2026-10-05"));
    assert!(dashboard(&state, None).deadlines.is_empty());
}

#[test]
fn a_deadline_today_and_tomorrow_read_like_production() {
    let mut state = base_state();
    state
        .tasks
        .push(task("a", "c1", "A", TaskSubtype::Sheet, 0, 0));
    state
        .tasks
        .push(task("b", "c2", "B", TaskSubtype::Sheet, 0, 0));
    state.timetable_events.push(event(
        "da",
        "c1",
        "a",
        TimetableEventKind::SheetDeadline,
        "2026-09-30",
        "18:00",
        None,
        false,
    ));
    state.timetable_events.push(event(
        "db",
        "c2",
        "b",
        TimetableEventKind::SheetDeadline,
        "2026-10-01",
        "09:00",
        None,
        false,
    ));
    let dash = dashboard(&state, None);
    let rel: Vec<&str> = dash
        .deadlines
        .iter()
        .map(|x| x.due_relative.as_str())
        .collect();
    assert_eq!(rel, ["today", "tomorrow"]);
    assert_eq!(
        dash.deadlines[0].status, "Analysis II",
        "no release scheduled counts as out"
    );
}

#[test]
fn one_thing_prefers_the_selected_planned_task_then_the_first_open_entry() {
    let mut state = base_state();
    state.tasks.push(task(
        "t1",
        "c1",
        "Exercise Sheet 1",
        TaskSubtype::Sheet,
        6,
        2,
    ));
    state
        .tasks
        .push(task("t2", "c2", "Reading", TaskSubtype::Other, 3, 0));
    state
        .calendar_entries
        .push(entry("e1", "t1", "2026-09-30", Some("09:00"), false));
    state
        .calendar_entries
        .push(entry("e2", "t2", "2026-09-30", Some("11:00"), false));
    let dash = dashboard(&state, None);
    assert_eq!(dash.one_thing.title, "Exercise Sheet 1");
    assert_eq!(dash.one_thing.start, Some(StartTarget::Task("t1".into())));
    assert_eq!(
        dash.one_thing.mark_done,
        Some(MarkTarget::CalendarEntry("e1".into()))
    );
    let picked = dashboard(&state, Some("t2"));
    assert_eq!(picked.one_thing.title, "Reading");
    assert!(picked
        .planned
        .iter()
        .any(|r| r.selected && r.title == "Reading"));
    assert_eq!(picked.remaining_label, "2 open \u{b7} 2h to today's goal");
}

#[test]
fn without_planned_units_one_thing_falls_back_to_the_first_open_timeline_row_then_a_deadline() {
    let mut state = base_state();
    state.daily_todos.push(todo(
        "x",
        "Email the tutor",
        "2026-09-30",
        Some("09:30"),
        false,
    ));
    let dash = dashboard(&state, None);
    assert_eq!(dash.one_thing.title, "Email the tutor");
    assert_eq!(dash.one_thing.meta, "To-do \u{b7} 09:30");
    assert_eq!(
        dash.one_thing.start,
        Some(StartTarget::Todo("Email the tutor".into()))
    );
    assert_eq!(
        dash.one_thing.mark_done,
        Some(MarkTarget::Todo {
            todo_id: "x".into(),
            date: "2026-09-30".into()
        })
    );

    let mut state = base_state();
    state
        .tasks
        .push(task("sh", "c2", "Problem Set", TaskSubtype::Sheet, 0, 0));
    state.timetable_events.push(event(
        "due",
        "c2",
        "sh",
        TimetableEventKind::SheetDeadline,
        "2026-10-02",
        "12:00",
        None,
        false,
    ));
    state.sync_task_units_from_schedule();
    let dash = dashboard(&state, None);
    assert_eq!(
        dash.one_thing.title, "Problem Set 1",
        "getWabiUnitInfo heading"
    );
    assert_eq!(dash.one_thing.meta, "Physics \u{b7} due Oct 2");
    assert_eq!(dash.one_thing.start, Some(StartTarget::Task("sh".into())));
    assert_eq!(
        dash.one_thing.mark_done, None,
        "production offers no MARK DONE here"
    );
}

#[test]
fn twelve_hour_locales_show_am_pm() {
    assert_eq!(display_time("09:30", true), "9:30 AM");
    assert_eq!(display_time("21:05", true), "9:05 PM");
    assert_eq!(display_time("00:00", true), "12:00 AM");
    assert_eq!(display_time("12:00", true), "12:00 PM");
    assert_eq!(display_time("09:30", false), "09:30");
    assert_eq!(
        display_time("930", false),
        "09:30",
        "parseTimeInput backtracks"
    );
    assert_eq!(display_time("9:30pm", false), "21:30");
    assert_eq!(display_time("nonsense", false), "nonsense");
}

#[test]
fn prep_tasks_run_past_the_semester_end_until_its_last_exam() {
    let mut state = AcademicState::new();
    state
        .semesters
        .push(semester("s", "2026-02-20", "2026-09-20"));
    state.courses.push(course("c1", "s", "Analysis II"));
    let mut prep = task("p", "c1", "Exercise Sheet 1", TaskSubtype::Sheet, 4, 0);
    prep.prep = true;
    state.tasks.push(prep);
    state.exams.push(Exam {
        id: ExamId::new("x"),
        semester_id: SemesterId::new("s"),
        course_id: CourseId::new("c1"),
        title: "Final".into(),
        exam_date: ld("2026-10-15"),
        weight: 100.0,
        preparedness: 0.0,
        location: String::new(),
        kind: Some(ExamKind::Session),
    });
    state.timetable_events.push(event(
        "pe",
        "c1",
        "p",
        TimetableEventKind::Occurrence,
        "2026-09-23",
        "10:00",
        None,
        true,
    ));
    assert_eq!(
        semester_stage(&state.semesters[0], &state.exams, d("2026-09-30")),
        SemesterStage::Prep
    );
    assert_eq!(
        semester_stage(&state.semesters[0], &state.exams, d("2026-09-10")),
        SemesterStage::Lectures
    );
    assert_eq!(
        semester_stage(&state.semesters[0], &state.exams, d("2026-10-16")),
        SemesterStage::Done
    );
    let dash = dashboard(&state, None);
    assert_eq!(
        dash.planned.len(),
        1,
        "the prep lecture shows today although the semester ended"
    );
    // Without the Wabi-Sabi options (every other style) it would not exist.
    let events: Vec<&TimetableEvent> = state.timetable_events.iter().collect();
    assert!(crate::dashboard::schedule::expand_timetable_events(
        &events,
        &[],
        &state.semesters[0],
        d("2026-09-30"),
        d("2026-09-30")
    )
    .is_empty());
    let sidebar = wabi_sidebar(
        &state,
        &DashboardMetrics::compute(DashboardInput {
            state: &state,
            now: now(),
            clock: &ZURICH,
            daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
        }),
    );
    assert!(sidebar.semester_in_prep);
    assert_eq!(sidebar.courses[0].days_to_exam.as_deref(), Some("15d"));
}

#[test]
fn schedule_sync_derives_totals_from_projected_occurrences_and_ticks() {
    let mut state = base_state();
    state.tasks.push(task(
        "t",
        "c1",
        "Lecture Notes",
        TaskSubtype::Lecture,
        99,
        99,
    ));
    let mut lec = event(
        "lec",
        "c1",
        "t",
        TimetableEventKind::Occurrence,
        "2026-08-20",
        "10:00",
        None,
        true,
    );
    lec.completed_occurrences = vec!["2026-08-27".into(), "2026-12-31".into()];
    lec.occurrence_overrides.insert(
        "2026-09-03".into(),
        OccurrenceOverride {
            skipped: true,
            ..Default::default()
        },
    );
    state.timetable_events.push(lec);
    // releases never count
    state.timetable_events.push(event(
        "rel",
        "c1",
        "t",
        TimetableEventKind::SheetRelease,
        "2026-08-21",
        "08:00",
        None,
        true,
    ));
    assert!(state.sync_task_units_from_schedule());
    // Thursdays Aug 20 .. Dec 17 = 18, minus the skipped Sep 3 = 17; one tick inside the range.
    assert_eq!(
        (state.tasks[0].total_units, state.tasks[0].completed_units),
        (17, 1)
    );
    assert!(!state.sync_task_units_from_schedule(), "idempotent");
    // A task with no events keeps its hand-set numbers.
    state
        .tasks
        .push(task("free", "c2", "Reading", TaskSubtype::Other, 5, 2));
    state.sync_task_units_from_schedule();
    assert_eq!(
        (state.tasks[1].total_units, state.tasks[1].completed_units),
        (5, 2)
    );
}

#[test]
fn toggling_occurrences_and_todos_matches_production() {
    let mut state = base_state();
    state
        .tasks
        .push(task("t", "c1", "Sheet", TaskSubtype::Sheet, 1, 0));
    state.timetable_events.push(event(
        "due",
        "c1",
        "t",
        TimetableEventKind::SheetDeadline,
        "2026-09-30",
        "18:00",
        None,
        false,
    ));
    state.timetable_events.push(event(
        "rel",
        "c1",
        "t",
        TimetableEventKind::SheetRelease,
        "2026-09-23",
        "08:00",
        None,
        false,
    ));
    let id = TimetableEventId::new("due");
    assert!(state.toggle_timetable_occurrence(&id, "2026-09-30"));
    assert_eq!(state.tasks[0].completed_units, 1);
    assert!(state.toggle_timetable_occurrence(&id, "2026-09-30"));
    assert_eq!(state.tasks[0].completed_units, 0);
    // a release mark is "seen" state only
    assert!(state.toggle_timetable_occurrence(&TimetableEventId::new("rel"), "2026-09-23"));
    assert_eq!(state.tasks[0].completed_units, 0);
    assert!(!state.toggle_timetable_occurrence(&TimetableEventId::new("missing"), "2026-09-30"));

    state
        .daily_todos
        .push(todo("one", "One-off", "2026-09-30", None, false));
    state
        .daily_todos
        .push(todo("rep", "Weekly", "2026-09-16", None, true));
    assert!(state.toggle_daily_todo_occurrence(&DailyTodoId::new("one"), "2026-09-30", ts()));
    assert!(state.daily_todos[0].completed && state.daily_todos[0].completed_at.is_some());
    assert!(state.toggle_daily_todo_occurrence(&DailyTodoId::new("rep"), "2026-09-30", ts()));
    assert_eq!(state.daily_todos[1].completed_occurrences, ["2026-09-30"]);
    assert!(!state.daily_todos[1].completed);
    assert!(state.toggle_daily_todo_occurrence(&DailyTodoId::new("rep"), "2026-09-30", ts()));
    assert!(state.daily_todos[1].completed_occurrences.is_empty());
}

#[test]
fn unit_decrement_ignores_occurrences_beyond_the_total() {
    assert_eq!(
        unit_decrement_for(5, 3, 1),
        0,
        "two uncounted extras absorb the removal"
    );
    assert_eq!(unit_decrement_for(3, 3, 1), 1);
    assert_eq!(unit_decrement_for(0, 3, 1), 1);
}

#[test]
fn quiet_mode_uses_the_pick_unless_its_entry_is_done_and_a_running_goal_wins() {
    let mut state = base_state();
    state.tasks.push(task(
        "t1",
        "c1",
        "Exercise Sheet 1",
        TaskSubtype::Sheet,
        6,
        2,
    ));
    state
        .tasks
        .push(task("t2", "c2", "Reading", TaskSubtype::Other, 3, 0));
    state
        .calendar_entries
        .push(entry("e1", "t1", "2026-09-30", Some("09:00"), false));
    state
        .calendar_entries
        .push(entry("e2", "t2", "2026-09-30", Some("11:00"), false));
    let metrics = DashboardMetrics::compute(DashboardInput {
        state: &state,
        now: now(),
        clock: &ZURICH,
        daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
    });
    let q = wabi_quiet(&state, &metrics, &ZURICH, None, None, None);
    assert_eq!(
        q.title, "Exercise Sheet 1 3",
        "unit heading of the next task"
    );
    assert_eq!(
        q.others,
        [("t2".into(), "Reading".into(), "Physics".into())]
    );
    let q = wabi_quiet(&state, &metrics, &ZURICH, Some("t2"), None, None);
    assert_eq!(q.title, "Reading");
    assert_eq!(q.meta, "Physics");
    let q = wabi_quiet(
        &state,
        &metrics,
        &ZURICH,
        Some("t2"),
        None,
        Some("  Revise notes "),
    );
    assert_eq!(
        (q.title.as_str(), q.meta.as_str()),
        ("Revise notes", "To-do")
    );
}

#[test]
fn moved_occurrences_show_their_new_time() {
    let mut state = base_state();
    state
        .tasks
        .push(task("t", "c1", "Lectures", TaskSubtype::Lecture, 0, 0));
    let mut lec = event(
        "lec",
        "c1",
        "t",
        TimetableEventKind::Occurrence,
        "2026-09-23",
        "10:00",
        Some("12:00"),
        true,
    );
    lec.occurrence_overrides.insert(
        "2026-09-30".into(),
        OccurrenceOverride {
            skipped: false,
            date: Some(ld("2026-09-30")),
            time: Some("15:00".into()),
            end_time: Some("16:30".into()),
        },
    );
    state.timetable_events.push(lec);
    let dash = dashboard(&state, None);
    assert_eq!(dash.planned.len(), 1);
    assert_eq!(dash.planned[0].due, "15:00\u{2013}16:30");
    let _ = Priority::Medium; // keep the import list honest for future cases
}

#[test]
fn an_occurrence_moved_from_another_day_is_only_projected_from_its_original_date() {
    // Production quirk (preserved): `expandTimetableEvents` walks the series' *original* dates in
    // the requested range and then relocates moved ones, so a single-day query for the new date
    // does not see an occurrence moved there from a day outside that range.
    let mut state = base_state();
    state
        .tasks
        .push(task("t", "c1", "Lectures", TaskSubtype::Lecture, 0, 0));
    let mut lec = event(
        "lec",
        "c1",
        "t",
        TimetableEventKind::Occurrence,
        "2026-09-29",
        "10:00",
        None,
        true,
    );
    lec.occurrence_overrides.insert(
        "2026-09-29".into(),
        OccurrenceOverride {
            skipped: false,
            date: Some(ld("2026-09-30")),
            time: Some("15:00".into()),
            end_time: None,
        },
    );
    state.timetable_events.push(lec);
    assert!(dashboard(&state, None).planned.is_empty());
}
