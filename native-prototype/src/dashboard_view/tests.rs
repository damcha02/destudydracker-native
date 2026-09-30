//! Stage 17 Dashboard tests at the application layer: golden values recorded from the *running
//! production app* (`tests/fixtures/dashboard/production-text/*.txt`, produced by
//! `scripts/visual-parity/capture-prod.mjs` with a pinned date/timezone against the same
//! synthetic fixtures), recomputation policy, and robustness (empty profile, dangling course
//! references, stress dataset). The pure calculations are unit-tested next to their code in
//! `study-tracker-core::dashboard`.

use super::*;
use crate::academic_controller::{AcademicController, NullAcademicPersistencePort};
use study_tracker_core::academic::{
    AcademicState, CalendarEntryId, CourseId, SessionId, SessionKind, StudySession,
};
use study_tracker_core::dashboard::FixedOffsetClock;

/// Europe/Zurich in summer - the timezone the fixtures were generated and the production pages
/// rendered in.
const ZURICH_SUMMER: FixedOffsetClock = FixedOffsetClock::new(2 * 3600);

/// 2026-09-30T12:00:00+02:00, the pinned "now" of every golden.
fn pinned_now() -> WallTimestamp {
    WallTimestamp::from_unix_millis(
        CivilDate::from_ymd(2026, 9, 30).unwrap().days() * 86_400_000 + 10 * 3_600_000,
    )
}

fn load_fixture(name: &str) -> AcademicState {
    let raw = match name {
        "empty" => include_str!("../../tests/fixtures/dashboard/empty.json"),
        "small" => include_str!("../../tests/fixtures/dashboard/small.json"),
        "realistic" => include_str!("../../tests/fixtures/dashboard/realistic.json"),
        other => panic!("unknown fixture {other}"),
    };
    let parsed: serde_json::Value = serde_json::from_str(raw).unwrap();
    let state = parsed["state"].as_object().unwrap();
    let (mut academic, warnings) =
        crate::persistence::migration_academic::convert_academic(state, pinned_now());
    assert!(
        warnings.is_empty(),
        "fixture must convert cleanly: {warnings:?}"
    );
    // The backup carries lifetime totals; the converter keeps them (they are not recomputed).
    academic.lifetime_study_minutes = state["lifetimeStudyMinutes"].as_u64().unwrap();
    academic
}

fn metrics_for(name: &str) -> (AcademicState, DashboardMetrics) {
    let state = load_fixture(name);
    let m = DashboardMetrics::compute(DashboardInput {
        state: &state,
        now: pinned_now(),
        clock: &ZURICH_SUMMER,
        daily_goal_minutes: DEFAULT_DAILY_GOAL_MINUTES,
    });
    (state, m)
}

fn text(s: &SharedString) -> String {
    s.to_string()
}

fn rows<T: Clone + 'static>(m: &ModelRc<T>) -> Vec<T> {
    use slint::Model;
    m.iter().collect()
}

fn ui(full: bool) -> DashboardUiState {
    DashboardUiState {
        full,
        ..DashboardUiState::default()
    }
}

fn data_for(name: &str, full: bool) -> FnDashboardData {
    let (state, m) = metrics_for(name);
    let sessions = SessionDays::new(&state.sessions, &ZURICH_SUMMER);
    let timeline = focus_timeline(
        FocusRange::Week,
        m.today,
        &sessions,
        &state.courses,
        state.lifetime_study_minutes,
    );
    build_data(&m, Some(&timeline), &ui(full), &ZURICH_SUMMER)
}

// --- golden metrics (every number below is what the production WebView printed) ------------------

#[test]
fn realistic_profile_metrics_match_production() {
    let (_, m) = metrics_for("realistic");
    assert_eq!(m.today_minutes, 53, "'53m of 2h done today.'");
    assert_eq!(m.goal_progress_percent, 44, "'44% · STREAK 9 D'");
    assert_eq!(m.streak_days, 9);
    assert_eq!(m.week_number, 40, "'WEEK 40'");
    assert_eq!(
        m.weekly_total_minutes,
        13 * 60 + 35,
        "'13h 35m logged this week.'"
    );
    assert_eq!(m.lifetime_minutes, 5006, "'83H 26M LOGGED ...'");
    assert_eq!(m.session_day_count, 46, "'... ACROSS 46 DAYS SINCE JUL 23'");
    assert_eq!(m.first_session_label.as_deref(), Some("Jul 23"));
    assert_eq!(m.open_task_count, 14, "'14 open tasks'");
    assert_eq!(m.total_units_left, 45, "'45 units left'");
    assert_eq!(
        format_dashboard_rate(m.units_per_day),
        "4.7",
        "'4.7 units/day'"
    );
    assert_eq!(m.queue_open_count, 3, "'3 planned today'");
    assert_eq!(m.queue.len(), 4, "'3 OPEN · 4 PLANNED TODAY'");
    assert_eq!(
        (m.overall_score, m.overall_label),
        (62, "Steady"),
        "'OVERALL SCORE | Steady | 62'"
    );
    assert_eq!(m.nearest_deadline_label.as_deref(), Some("Oct 3"));
    let exams: Vec<(&str, &str)> = m
        .exams
        .iter()
        .map(|e| (e.title.as_str(), e.date_label.as_str()))
        .collect();
    assert_eq!(
        exams,
        [
            ("Midterm", "Oct 9"),
            ("Final", "Oct 24"),
            ("Lab Exam", "Nov 10")
        ],
        "the past 'Old Quiz' is excluded"
    );
    let scores: Vec<(String, u32)> = m
        .courses
        .iter()
        .map(|c| (c.name.clone(), c.score))
        .collect();
    assert_eq!(
        scores,
        [
            ("Analysis II".to_string(), 55),
            ("Linear Algebra".to_string(), 71),
            ("Physics".to_string(), 78),
            ("Programming".to_string(), 86),
            ("Statistics".to_string(), 93)
        ],
        "the archived semester's course is not on the radar"
    );
}

fn format_dashboard_rate(value: f64) -> String {
    study_tracker_core::dashboard::format::to_fixed(value, 1)
}

#[test]
fn small_profile_metrics_match_production() {
    let (_, m) = metrics_for("small");
    assert_eq!(
        (m.today_minutes, m.goal_progress_percent, m.streak_days),
        (45, 38, 1),
        "'45m / 2h', '38% OF DAILY GOAL · STREAK 1 D'"
    );
    assert_eq!(
        (m.open_task_count, m.total_units_left),
        (3, 15),
        "'3 open tasks', '15 units left'"
    );
    assert_eq!(
        format_dashboard_rate(m.units_per_day),
        "2.4",
        "'2.4 UNITS/DAY'"
    );
    assert_eq!(m.weekly_total_minutes, 45, "'45m logged this week.'");
    assert_eq!((m.overall_score, m.overall_label), (57, "Steady"));
    assert_eq!(m.courses.len(), 1);
    assert_eq!(
        (m.courses[0].score, m.courses[0].minutes),
        (57, 45),
        "'Analysis II 57 ... 45m • Target 5.5'"
    );
    assert_eq!(m.exams[0].days_until, 18, "'18 DAYS Midterm'");
    assert_eq!(
        m.first_session_label.as_deref(),
        Some("Sep 30"),
        "'45M LOGGED ACROSS 1 DAY SINCE SEP 30'"
    );
    assert_eq!(m.session_day_count, 1);
}

#[test]
fn empty_profile_shows_zeros_not_nan_and_matches_production_text() {
    let (_, m) = metrics_for("empty");
    assert_eq!(
        (
            m.today_minutes,
            m.streak_days,
            m.open_task_count,
            m.total_units_left,
            m.queue.len(),
            m.courses.len(),
            m.exams.len()
        ),
        (0, 0, 0, 0, 0, 0, 0)
    );
    assert_eq!(m.units_per_day, 0.0);
    // Production prints "Critical 0" for an empty profile (overall health 0 -> "Critical"): kept.
    assert_eq!((m.overall_score, m.overall_label), (0, "Critical"));
    assert!(!m.has_sessions && m.first_session_label.is_none());

    let quiet = data_for("empty", false);
    assert_eq!(text(&quiet.subtitle_strong), "Choose a study block");
    assert_eq!(
        text(&quiet.subtitle_rest),
        " · Select a task below, or place one in the planner calendar."
    );
    assert!(
        !quiet.has_start,
        "no 'Start focus' in the empty Quiet layout"
    );
    assert_eq!(text(&quiet.today_line), "0m of 2h done today.");
    assert_eq!(
        rows(&quiet.pace_lines).iter().map(text).collect::<Vec<_>>(),
        ["0 units left", "0.0 units/day", "0 open tasks"]
    );
    assert_eq!(text(&quiet.planned_today), "0 planned today");
    assert!(rows(&quiet.ahead).is_empty());
    assert_eq!(text(&quiet.goal_caption), "0% · STREAK 0 D");

    let full = data_for("empty", true);
    assert_eq!(text(&full.next_code), "DESK");
    assert_eq!(text(&full.next_title), "Choose a study block");
    assert_eq!(
        text(&full.next_meta),
        "Select one planned task from the queue."
    );
    assert_eq!(text(&full.pace_note), "No logged focus yet this week.");
    assert_eq!(
        text(&full.margin_note),
        "Pin one task to today so the dashboard can answer the next-action question."
    );
    assert_eq!(text(&full.week_caption), "0M THIS WEEK");
    assert_eq!(
        text(&full.footer),
        "YOUR TIMELINE BEGINS WHEN YOU COMPLETE YOUR FIRST SESSION."
    );
    assert!(!full.has_history, "'No focus history yet'");
    assert_eq!(
        text(&full.subtitle_rest),
        "Semester work · 0 open tasks · 0 units left"
    );
}

#[test]
fn quiet_realistic_text_matches_the_production_render() {
    let d = data_for("realistic", false);
    assert_eq!(
        text(&d.stamp_line),
        "WEDNESDAY, SEP 30, 2026 · SEMESTER DESK · WEEK 40"
    );
    assert_eq!(text(&d.title_text), "On the desk");
    assert_eq!(text(&d.subtitle_strong), "Exercise Sheet 1");
    assert_eq!(
        text(&d.subtitle_rest),
        " · Sheet 3 of 6 · Analysis II · due Oct 3"
    );
    assert!(d.has_start);
    assert_eq!(text(&d.today_line), "53m of 2h done today.");
    let quiet_rows = rows(&d.quiet_rows);
    let shown: Vec<(String, String, String, bool)> = quiet_rows
        .iter()
        .map(|r| {
            (
                text(&r.title),
                text(&r.course),
                text(&r.caption_upper),
                r.selected,
            )
        })
        .collect();
    assert_eq!(
        shown,
        [
            (
                "Exercise Sheet 1".into(),
                "Analysis II".into(),
                "DUE OCT 3".into(),
                true
            ),
            (
                "Lecture Notes 2".into(),
                "Linear Algebra".into(),
                "DUE OCT 12".into(),
                false
            ),
            (
                "Exercise Sheet 1".into(),
                "Programming".into(),
                "DUE OCT 15".into(),
                false
            ),
        ]
    );
    assert_eq!(
        rows(&d.ahead)
            .iter()
            .map(|a| (text(&a.label), text(&a.value)))
            .collect::<Vec<_>>(),
        [
            ("Midterm".into(), "Oct 9".into()),
            ("Final".into(), "Oct 24".into()),
            ("Lab Exam".into(), "Nov 10".into()),
            ("Nearest deadline".into(), "Oct 3".into())
        ]
    );
    assert_eq!(text(&d.focused), "53m");
    assert_eq!(text(&d.goal), "2h");
    assert_eq!(text(&d.goal_caption), "44% · STREAK 9 D");
}

#[test]
fn full_realistic_text_matches_the_production_render() {
    let d = data_for("realistic", true);
    assert_eq!(text(&d.stamp_line), "WEDNESDAY, SEP 30, 2026 · WEEK 40");
    assert_eq!(text(&d.title_text), "Today’s desk");
    assert_eq!(
        text(&d.subtitle_rest),
        "Semester work · 14 open tasks · 45 units left · nearest deadline Oct 3"
    );
    assert_eq!(text(&d.goal_caption), "44% OF DAILY GOAL · STREAK 9 D");
    assert_eq!(text(&d.queue_caption), "3 OPEN · 4 PLANNED TODAY");
    assert_eq!(
        (text(&d.next_code), text(&d.next_title)),
        ("ANAL".into(), "Exercise Sheet 1".into())
    );
    assert_eq!(text(&d.next_meta), "Sheet 3 of 6 · Analysis II · due Oct 3");
    let q = rows(&d.queue_rows);
    assert_eq!(
        q.len(),
        4,
        "the Full queue lists completed entries too, after the open ones"
    );
    assert_eq!(
        q.iter().map(|r| text(&r.chip)).collect::<Vec<_>>(),
        ["selected", "1/2 unit", "1/4 unit", "done"]
    );
    assert_eq!(
        q.iter().map(|r| text(&r.unit_label)).collect::<Vec<_>>(),
        [
            "Sheet 3 of 6",
            "1/2 unit lectures",
            "1/4 unit sheets",
            "Chapter 8 of 12"
        ]
    );
    let c = rows(&d.courses);
    assert_eq!(
        text(&c[0].detail),
        "Autumn Semester • 3 tasks • 12h 35m • Target 5.5"
    );
    assert_eq!(text(&c[0].score), "55");
    let e = rows(&d.exams);
    assert_eq!(text(&e[0].meta), "Analysis II • 40% weight");
    assert_eq!(
        (text(&e[0].days), text(&e[0].prep_text)),
        ("9".into(), "35%".into())
    );
    assert_eq!(
        (text(&d.pace_left), text(&d.pace_open), text(&d.pace_rate)),
        ("45 units".into(), "14".into(), "4.7".into())
    );
    assert_eq!(text(&d.pace_note), "13h 35m logged this week.");
    assert_eq!(
        text(&d.margin_note),
        "Midterm is first. Block review time before chasing lower-pressure tasks."
    );
    assert_eq!(text(&d.week_caption), "13H 35M THIS WEEK");
    assert_eq!(
        text(&d.footer),
        "83H 26M LOGGED ACROSS 46 DAYS SINCE JUL 23"
    );
    // weekly chart
    let stats = rows(&d.stats);
    assert_eq!(
        stats
            .iter()
            .map(|s| (text(&s.label), text(&s.value), text(&s.sub)))
            .collect::<Vec<_>>(),
        [
            ("ACTIVE DAYS".into(), "3".into(), "/ week".into()),
            ("STREAK".into(), "9".into(), "days".into()),
            ("BIGGEST DAY".into(), "2h 53m".into(), "Sep 28".into()),
            ("LATEST FIND".into(), "Ammonite".into(), "at 50h".into()),
        ]
    );
    assert_eq!(
        rows(&d.legend)
            .iter()
            .map(|l| text(&l.name))
            .collect::<Vec<_>>(),
        ["Statistics", "Linear Algebra", "General", "Programming"]
    );
    assert_eq!(
        rows(&d.discovered)
            .iter()
            .map(|l| text(&l.label))
            .collect::<Vec<_>>(),
        ["Seed Fossil", "Shell Fragment", "Ammonite"]
    );
    assert_eq!(
        rows(&d.range_labels).iter().map(text).collect::<Vec<_>>(),
        ["THIS WEEK", "7D", "14D", "30D", "60D", "1Y"]
    );
    let days = rows(&d.days);
    assert_eq!(days.len(), 7);
    assert_eq!(
        days.iter().map(|x| text(&x.label)).collect::<Vec<_>>(),
        ["M", "T", "W", "T", "F", "S", "S"]
    );
    assert!(
        days[2].is_today && !days[3].is_today,
        "Wed 30 Sep is today; the week starts on Monday"
    );
    assert!(days[3].empty && days[6].empty, "future days are 'eroded'");
}

#[test]
fn dangling_and_missing_course_references_never_crash_and_become_general() {
    // The realistic fixture has one session pointing at a deleted course and one with no course.
    let (state, m) = metrics_for("realistic");
    assert!(state.sessions.iter().any(|s| s
        .course_id
        .as_ref()
        .is_some_and(|c| c.as_str() == "course-deleted-gone")));
    assert!(m
        .courses
        .iter()
        .all(|c| c.course_id != "course-deleted-gone"));
    let d = data_for("realistic", true);
    assert!(
        rows(&d.legend).iter().any(|l| text(&l.name) == "General"),
        "unknown/absent course -> General layer"
    );
}

// --- recomputation policy -----------------------------------------------------------------------------

fn controller_with(state: AcademicState) -> AcademicController {
    let mut c = AcademicController::new(Box::new(NullAcademicPersistencePort));
    c.replace_all(state);
    c
}

#[test]
fn unchanged_revision_never_recomputes_however_many_timer_ticks_happen() {
    let academic = controller_with(load_fixture("realistic"));
    let mut dash = DashboardController::new(ui(true));
    assert!(dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER
    ));
    assert_eq!(
        dash.stats(),
        ComputeStats {
            metrics: 1,
            timelines: 1
        }
    );
    for tick in 0..5_000 {
        // What the 100 ms timer path does: a cheap staleness check, then (not) a sync.
        let now = WallTimestamp::from_unix_millis(pinned_now().unix_millis + tick * 100);
        if dash.is_stale(academic.revision()) {
            dash.sync(academic.state(), academic.revision(), now, &ZURICH_SUMMER);
        }
    }
    assert_eq!(
        dash.stats(),
        ComputeStats {
            metrics: 1,
            timelines: 1
        },
        "5000 simulated timer ticks recomputed nothing"
    );
    // A sync call itself is also idempotent when nothing changed.
    assert!(!dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER
    ));
    assert_eq!(dash.stats().metrics, 1);
}

#[test]
fn layout_or_selection_changes_do_not_recompute_metrics_but_a_new_range_recomputes_only_the_chart()
{
    let academic = controller_with(load_fixture("realistic"));
    let mut dash = DashboardController::new(ui(false));
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    assert_eq!(
        dash.stats(),
        ComputeStats {
            metrics: 1,
            timelines: 0
        },
        "Quiet never builds the chart"
    );
    dash.ui_mut().full = true;
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    assert_eq!(
        dash.stats(),
        ComputeStats {
            metrics: 1,
            timelines: 1
        }
    );
    dash.ui_mut().range = FocusRange::Days(30);
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    assert_eq!(
        dash.stats(),
        ComputeStats {
            metrics: 1,
            timelines: 2
        },
        "metrics untouched, chart rebuilt for the new range"
    );
    dash.ui_mut().selected_entry = Some("cal-1".into());
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    assert_eq!(
        dash.stats(),
        ComputeStats {
            metrics: 1,
            timelines: 2
        },
        "selection is view state only"
    );
}

#[test]
fn a_timer_completion_session_updates_the_dashboard_without_a_restart() {
    let mut academic = controller_with(load_fixture("small"));
    let mut dash = DashboardController::new(ui(false));
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    assert_eq!(dash.metrics().unwrap().today_minutes, 45);

    // Exactly what `route_timer_effects` does on completion: add a StudySession (a 25-minute block
    // that ended at 11:00 local = 09:00Z on the pinned day).
    let ended = WallTimestamp::from_unix_millis(pinned_now().unix_millis - 3_600_000);
    let session = StudySession {
        id: SessionId::new("timer-completion"),
        semester_id: None,
        course_id: Some(CourseId::new("course-0")),
        task_id: None,
        kind: SessionKind::Study,
        goal: String::new(),
        learned: String::new(),
        blocker: String::new(),
        next_step: String::new(),
        confidence: 3,
        started_at: WallTimestamp::from_unix_millis(ended.unix_millis - 25 * 60_000),
        ended_at: ended,
        minutes: 25,
        preset_label: "Pomodoro".into(),
    };
    let before = academic.revision();
    academic.route_timer_effects(
        &[],
        chrono::FixedOffset::east_opt(0).unwrap(),
        study_tracker_core::timer::ClockObservation {
            monotonic_millis: 0,
            wall: pinned_now(),
        },
    );
    assert_eq!(
        academic.revision(),
        before,
        "an effect list with no session changes nothing"
    );
    let mut next = academic.state().clone();
    next.add_study_sessions(vec![session], pinned_now());
    academic.replace_all(next);

    assert!(
        dash.is_stale(academic.revision()),
        "the academic revision moved, so the dashboard is stale"
    );
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    let m = dash.metrics().unwrap();
    assert_eq!(m.today_minutes, 70, "45 + 25");
    assert_eq!(m.lifetime_minutes, 45 + 25);
    assert_eq!(dash.stats().metrics, 2);
}

#[test]
fn ticking_a_planned_unit_changes_units_left_and_the_queue() {
    let mut academic = controller_with(load_fixture("small"));
    let mut dash = DashboardController::new(ui(true));
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    assert_eq!(dash.metrics().unwrap().total_units_left, 15);
    let id = CalendarEntryId::new("cal-0");
    assert!(academic.toggle_calendar_entry(&id, pinned_now()));
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    let m = dash.metrics().unwrap();
    assert_eq!(
        m.total_units_left, 14,
        "a whole unit completed -> one fewer unit left"
    );
    assert_eq!(m.queue_open_count, 0);
    assert!(
        !academic.toggle_calendar_entry(&CalendarEntryId::new("nope"), pinned_now()),
        "unknown id: no change, no revision bump"
    );
}

#[test]
fn the_local_date_rolling_over_makes_the_cache_stale() {
    let academic = controller_with(load_fixture("small"));
    let mut dash = DashboardController::new(ui(false));
    dash.sync(
        academic.state(),
        academic.revision(),
        pinned_now(),
        &ZURICH_SUMMER,
    );
    let same_day = WallTimestamp::from_unix_millis(pinned_now().unix_millis + 3 * 3_600_000); // 15:00 local
    let next_day = WallTimestamp::from_unix_millis(pinned_now().unix_millis + 13 * 3_600_000); // 01:00 local on Oct 1
    assert!(!dash.is_stale_for(academic.revision(), same_day, &ZURICH_SUMMER));
    assert!(dash.is_stale_for(academic.revision(), next_day, &ZURICH_SUMMER));
    dash.sync(
        academic.state(),
        academic.revision(),
        next_day,
        &ZURICH_SUMMER,
    );
    assert_eq!(
        dash.metrics().unwrap().today_minutes,
        0,
        "a new day starts at zero"
    );
    assert_eq!(
        dash.metrics().unwrap().streak_days,
        1,
        "yesterday's session keeps the streak alive"
    );
}

#[test]
fn stress_dataset_computes_quickly_and_produces_data() {
    let state = crate::synthetic_dataset::build_synthetic_academic_state();
    assert!(state.sessions.len() >= 2000);
    let started = std::time::Instant::now();
    let m = DashboardMetrics::compute(DashboardInput {
        state: &state,
        now: pinned_now(),
        clock: &ZURICH_SUMMER,
        daily_goal_minutes: 120,
    });
    let metrics_time = started.elapsed();
    let sessions = SessionDays::new(&state.sessions, &ZURICH_SUMMER);
    let started = std::time::Instant::now();
    let year = focus_timeline(
        FocusRange::Days(365),
        m.today,
        &sessions,
        &state.courses,
        state.lifetime_study_minutes,
    );
    let timeline_time = started.elapsed();
    let d = build_data(&m, Some(&year), &ui(true), &ZURICH_SUMMER);
    assert_eq!(rows(&d.days).len(), 365);
    eprintln!(
        "stress dashboard: metrics {metrics_time:?}, 1y timeline {timeline_time:?} (debug build)"
    );
    // Generous bound (debug build, shared CI): the point is "milliseconds, not seconds".
    assert!(metrics_time < std::time::Duration::from_secs(2));
    assert!(timeline_time < std::time::Duration::from_secs(2));
}

// --- geometry / helpers ---------------------------------------------------------------------------------

#[test]
fn fossil_rand_is_bit_exact_with_the_javascript_original() {
    // Reference values computed with production's own `fossilRand` (Math.imul) in Node.
    for (seed, expected) in [
        (0, 0.0),
        (1, 0.244917003438),
        (31, 0.805175160058),
        (97, 0.767215391854),
        (128, 0.478730646661),
        (12345, 0.004489383660),
        (36501, 0.692689337069),
    ] {
        assert!(
            (fossil_rand(seed) - expected).abs() < 1e-11,
            "seed {seed}: {} vs {expected}",
            fossil_rand(seed)
        );
    }
}

#[test]
fn chart_geometry_follows_production_formulas() {
    let d = data_for("realistic", true);
    let days = rows(&d.days);
    // Monday 28th: 2h53m = 173 minutes is the biggest day -> full column height (100 = 105 - 5).
    assert!((days[0].column_height - 100.0).abs() < 1e-3);
    assert_eq!(d.chart_height, 105.0);
    assert_eq!(d.column_gap, 4.0);
    // Layers stack bottom-up with a 1px gap, each at least 4px tall.
    let layers = rows(&days[0].layers);
    assert!(layers.len() >= 2);
    assert_eq!(layers[0].bottom, 0.0);
    assert!((layers[1].bottom - (layers[0].height + 1.0)).abs() < 1e-3);
    assert!(layers
        .iter()
        .all(|l| l.height >= 4.0 && (0.82..=1.0).contains(&l.width_frac)));
    // Rest days have no layers and no height.
    assert_eq!(days[3].column_height, 0.0);
    assert!(rows(&days[3].layers).is_empty());
}

#[test]
fn css_colours_parse_hex_named_and_fall_back() {
    assert_eq!(
        parse_css_color("#8fb4ff"),
        Color::from_rgb_u8(0x8f, 0xb4, 0xff)
    );
    assert_eq!(parse_css_color("#fff"), Color::from_rgb_u8(255, 255, 255));
    assert_eq!(parse_css_color("blue"), Color::from_rgb_u8(0, 0, 255));
    assert_eq!(parse_css_color("not-a-colour"), FALLBACK_COURSE_COLOR);
    assert_eq!(parse_css_color(""), FALLBACK_COURSE_COLOR);
}

#[test]
fn selecting_a_row_changes_the_subtitle_and_selection_only() {
    let (state, m) = metrics_for("realistic");
    let _ = &state;
    let mut ui_state = ui(false);
    ui_state.selected_entry = Some(
        m.queue
            .iter()
            .filter(|e| !e.completed)
            .nth(1)
            .unwrap()
            .entry_id
            .clone(),
    );
    let d = build_data(&m, None, &ui_state, &ZURICH_SUMMER);
    assert_eq!(text(&d.subtitle_strong), "Lecture Notes 2");
    assert_eq!(rows(&d.quiet_rows).iter().filter(|r| r.selected).count(), 1);
    // An id that is not in the visible list falls back to the first row, as production does.
    ui_state.selected_entry = Some("does-not-exist".into());
    assert_eq!(
        text(&build_data(&m, None, &ui_state, &ZURICH_SUMMER).subtitle_strong),
        "Exercise Sheet 1"
    );
}

#[test]
fn chart_filter_matches_css_grayscale_then_contrast() {
    // #ef8f8f (a course layer) through grayscale(.25) contrast(.92), computed by hand:
    // luma = 0.2126*239 + 0.7152*143 + 0.0722*143 = 163.4; r = ((239 + (163.4-239)*.25) - 127.5)*.92 + 127.5 = 212.7; g = 146.5
    let out = chart_filter(Color::from_rgb_u8(0xef, 0x8f, 0x8f));
    assert_eq!((out.red(), out.green(), out.blue()), (213, 146, 146));
    let grey = chart_filter(Color::from_rgb_u8(128, 128, 128));
    assert_eq!(
        (grey.red(), grey.green(), grey.blue()),
        (128, 128, 128),
        "a neutral grey is (almost) a fixed point"
    );
}
