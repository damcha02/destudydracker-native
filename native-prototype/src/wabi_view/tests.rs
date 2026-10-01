//! Stage 19 golden tests: the Wabi-Sabi sidebar, Dashboard and Quiet mode, derived from the same
//! synthetic backups the *running production app* rendered
//! (`tests/fixtures/dashboard/production-text/wabi-sabi-*.txt`, captured with
//! `scripts/visual-parity/capture-prod.mjs --style wabi-sabi` at 2026-09-30 12:00 Europe/Zurich).
//! The native data is flattened into production's `innerText` shape (" | " between text nodes) so
//! the comparison is literal - text, order, presence of each button.

use super::*;
use study_tracker_core::dashboard::{DashboardInput, FixedOffsetClock, DEFAULT_DAILY_GOAL_MINUTES};
use study_tracker_core::timer::WallTimestamp;

const ZURICH_SUMMER: FixedOffsetClock = FixedOffsetClock::new(2 * 3600);

fn pinned_now() -> WallTimestamp {
    WallTimestamp::from_unix_millis(
        CivilDate::from_ymd(2026, 9, 30).unwrap().days() * 86_400_000 + 10 * 3_600_000,
    )
}

fn load(name: &str) -> (AcademicState, String) {
    let (raw, golden) = match name {
        "wabi" => (
            include_str!("../../tests/fixtures/dashboard/wabi.json"),
            include_str!("../../tests/fixtures/dashboard/production-text/wabi-sabi-wabi.txt"),
        ),
        "wabi-noplan" => (
            include_str!("../../tests/fixtures/dashboard/wabi-noplan.json"),
            include_str!(
                "../../tests/fixtures/dashboard/production-text/wabi-sabi-wabi-noplan.txt"
            ),
        ),
        "wabi-sheets" => (
            include_str!("../../tests/fixtures/dashboard/wabi-sheets.json"),
            include_str!(
                "../../tests/fixtures/dashboard/production-text/wabi-sabi-wabi-sheets.txt"
            ),
        ),
        "empty" => (
            include_str!("../../tests/fixtures/dashboard/empty.json"),
            include_str!("../../tests/fixtures/dashboard/production-text/wabi-sabi-empty.txt"),
        ),
        other => panic!("unknown fixture {other}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
    let state = parsed["state"].as_object().unwrap();
    let (mut academic, warnings) =
        crate::persistence::migration_academic::convert_academic(state, pinned_now());
    // The `wabi` fixtures deliberately contain one time-less sheet deadline, which production's
    // loader drops too (see `the_importer_drops_what_production_drops`).
    assert!(
        warnings.iter().all(|w| w.contains("missing time")),
        "unexpected conversion warnings: {warnings:?}"
    );
    academic.lifetime_study_minutes = state["lifetimeStudyMinutes"].as_u64().unwrap();
    (academic, golden.replace("\r\n", "\n"))
}

fn section<'g>(golden: &'g str, name: &str) -> &'g str {
    let start = golden.find(&format!("## {name}\n")).unwrap() + name.len() + 4;
    golden[start..].lines().next().unwrap_or("")
}

fn controller_for(state: &AcademicState) -> (WabiController, DashboardMetrics) {
    let metrics = DashboardMetrics::compute(DashboardInput {
        state,
        now: pinned_now(),
        clock: &ZURICH_SUMMER,
        daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
    });
    let mut c = WabiController::new(false);
    c.sync(state, &metrics, 1, &ZURICH_SUMMER, None);
    (c, metrics)
}

fn sidebar_text(c: &WabiController) -> String {
    let s = c.sidebar_data();
    format!(
        "Kokoro | {} | OVERALL | {} | ? | Today | \u{4eca}\u{65e5} | Plan | Timer | Notes | Rest | Circle | TENDED TODAY | {} | QUIET MODE \u{2192}",
        s.score, s.label, s.tended
    )
}

fn dashboard_text(c: &WabiController) -> String {
    let d = c.dash().unwrap();
    let mut parts: Vec<String> = vec![
        "TODAY".into(),
        d.today_label.clone(),
        d.remaining_label.clone(),
        "ONE THING".into(),
        d.one_thing.title.clone(),
        d.one_thing.meta.clone(),
    ];
    if d.one_thing.start.is_some() {
        parts.push("START".into());
    }
    if d.one_thing.mark_done.is_some() {
        parts.push("MARK DONE".into());
    }
    parts.push("SOMETHING ELSE".into());
    if !d.deadlines.is_empty() {
        parts.push("COMING UP".into());
        for row in &d.deadlines {
            parts.push(row.title.clone());
            parts.push(format!(
                "{}{}",
                if row.next { "NEXT" } else { "" },
                row.status
            ));
            parts.push(row.due.clone());
            parts.push(row.due_relative.clone());
            if row.task_id.is_some() {
                parts.push("FOCUS".into());
            }
        }
    }
    parts.push("PLANNED TODAY".into());
    if d.planned.is_empty() {
        parts.push("Nothing planned today. Add tasks from the planner calendar.".into());
    }
    for row in &d.planned {
        parts.push(row.title.clone());
        parts.push(row.subject.clone());
        if !row.amount.is_empty() {
            parts.push(row.amount.clone());
        }
        parts.push(row.due.clone());
    }
    parts.push("Click a title to make it the current task. Click the mark to close it out.".into());
    parts.join(" | ")
}

fn quiet_text(c: &WabiController) -> String {
    let q = c.quiet_data();
    format!(
        "NOW | {} | {} | 25:00 | START | DONE, LOG IT | LEAVE QUIET MODE",
        q.title, q.meta
    )
}

fn check(name: &str) {
    let (state, golden) = load(name);
    let (c, _) = controller_for(&state);
    assert_eq!(
        sidebar_text(&c),
        section(&golden, "sidebar"),
        "{name}: sidebar"
    );
    assert_eq!(
        dashboard_text(&c),
        section(&golden, "dashboard"),
        "{name}: dashboard"
    );
    assert_eq!(
        quiet_text(&c),
        section(&golden, "quiet-mode"),
        "{name}: quiet mode"
    );
}

#[test]
fn realistic_wabi_profile_matches_production_text() {
    check("wabi");
}

#[test]
fn without_planned_units_matches_production_text() {
    check("wabi-noplan");
}

#[test]
fn sheets_only_matches_production_text() {
    check("wabi-sheets");
}

#[test]
fn an_empty_profile_matches_production_text() {
    check("empty");
}

#[test]
fn the_importer_drops_what_production_drops_and_keeps_completions() {
    let (state, _) = load("wabi");
    // ev-due-3 has no time: production's normalizeTimetableEvents drops it.
    assert!(state
        .timetable_events
        .iter()
        .all(|e| e.id.as_str() != "ev-due-3"));
    // completedOccurrences survive the import (Stage 16 used to drop them).
    let lec = state
        .timetable_events
        .iter()
        .find(|e| e.id.as_str() == "ev-lec-1")
        .unwrap();
    assert_eq!(lec.completed_occurrences, ["2026-09-30"]);
    // ... and drive the derived unit counts ("Sheet 1 of 1" in production).
    let sheet = state
        .tasks
        .iter()
        .find(|t| t.id.as_str() == "task-0-0")
        .unwrap();
    assert_eq!((sheet.total_units, sheet.completed_units), (1, 0));
    let todo = state
        .daily_todos
        .iter()
        .find(|t| t.id.as_str() == "todo-2")
        .unwrap();
    assert!(todo.repeat_weekly);
}

#[test]
fn caching_recomputes_only_on_real_changes() {
    let (state, _) = load("wabi");
    let (mut c, metrics) = controller_for(&state);
    assert_eq!(c.recomputes(), 1);
    for _ in 0..5000 {
        // a Timer tick does not change any key input
        assert!(!c.sync(&state, &metrics, 1, &ZURICH_SUMMER, None));
    }
    assert_eq!(c.recomputes(), 1);
    assert!(
        c.sync(&state, &metrics, 2, &ZURICH_SUMMER, None),
        "revision bump"
    );
    c.ui_mut().selected_task = Some("task-1-1".into());
    assert!(
        c.sync(&state, &metrics, 2, &ZURICH_SUMMER, None),
        "selection"
    );
    assert_eq!(c.recomputes(), 3);
}

#[test]
fn row_clicks_map_back_to_typed_actions() {
    let (state, _) = load("wabi");
    let (c, _) = controller_for(&state);
    assert_eq!(
        c.planned_mark(0),
        Some(WabiAction::Toggle(MarkTarget::Todo {
            todo_id: "todo-0".into(),
            date: "2026-09-30".into()
        }))
    );
    assert_eq!(
        c.deadline_focus(0),
        Some(WabiAction::Start(StartTarget::Task("task-3-0".into())))
    );
    assert_eq!(
        c.planned_select(0),
        None,
        "a to-do title opens the planner, not a selection"
    );
    assert!(matches!(c.planned_select(5), Some(WabiAction::Select(_))));
    assert_eq!(c.deadline_mark(99), None);
}

#[test]
fn a_toggle_through_the_controller_updates_the_dashboard_and_persists_once() {
    use crate::academic_controller::{AcademicController, AcademicPersistencePort};
    use std::cell::Cell;
    use std::rc::Rc;
    struct Counting(Rc<Cell<u32>>);
    impl AcademicPersistencePort for Counting {
        fn persist(&mut self, _state: &AcademicState) {
            self.0.set(self.0.get() + 1);
        }
        fn load(&self) -> Option<AcademicState> {
            None
        }
    }
    let (state, _) = load("wabi");
    let writes = Rc::new(Cell::new(0));
    let mut academic = AcademicController::new(Box::new(Counting(Rc::clone(&writes))));
    academic.replace_all(state);
    let before = writes.get();
    assert!(academic.toggle_timetable_occurrence(
        &study_tracker_core::academic::TimetableEventId::new("ev-lec-0"),
        "2026-09-30"
    ));
    assert_eq!(writes.get(), before + 1);
    let metrics = DashboardMetrics::compute(DashboardInput {
        state: academic.state(),
        now: pinned_now(),
        clock: &ZURICH_SUMMER,
        daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
    });
    let mut c = WabiController::new(false);
    c.sync(
        academic.state(),
        &metrics,
        academic.revision(),
        &ZURICH_SUMMER,
        None,
    );
    let lecture = c
        .dash()
        .unwrap()
        .planned
        .iter()
        .find(|r| r.title == "Lecture Notes 1")
        .unwrap();
    assert!(lecture.completed);
}

#[test]
fn timer_cards_follow_production_order_and_are_reused_between_ticks() {
    let facts = TimerFacts {
        clock: "25:00".into(),
        idle: true,
        running: false,
        endless: false,
        phase_label: "WORK \u{b7} 25 MIN".into(),
        can_log: false,
        selected_mode: 0,
        heading: "General focus".into(),
    };
    let empty: ModelRc<WabiModeCard> = ModelRc::new(VecModel::default());
    let (data, replaced) = timer_data(&facts, &empty);
    let cards = replaced.expect("first build creates the model");
    let titles: Vec<String> = cards.iter().map(|c| c.title.to_string()).collect();
    assert_eq!(
        titles,
        [
            "Pomodoro",
            "Deep Work",
            "Sprint",
            "Exam",
            "\u{221e} Endless",
            "Custom"
        ]
    );
    assert!(cards.row_data(0).unwrap().active);
    assert!(!cards.row_data(2).unwrap().available);
    assert_eq!(data.primary.as_str(), "START");
    let ticked = TimerFacts {
        clock: "24:59".into(),
        idle: false,
        running: true,
        ..facts
    };
    let (data, replaced) = timer_data(&ticked, &cards);
    assert!(replaced.is_none(), "a tick never replaces the cards model");
    assert_eq!(data.primary.as_str(), "PAUSE");
}
