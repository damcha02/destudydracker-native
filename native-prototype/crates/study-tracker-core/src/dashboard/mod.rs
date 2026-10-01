//! Production Dashboard metrics (Stage 17): pure functions from [`AcademicState`] (+ "now" and a
//! local clock) to everything the production "Field Notebook" Dashboard displays.
//!
//! ```text
//! AcademicState ──► DashboardMetrics::compute ──► (app layer) DashboardViewModel ──► Slint
//! ```
//!
//! Nothing here reads the system clock, a timezone database, the filesystem, or Slint, and nothing
//! is persisted: a Dashboard is always recomputed from canonical academic data. Production
//! reference: `desktop/src/lib/metrics.ts`, `lib/scheduleWorkload.ts`, `lib/scheduleHealth.ts`,
//! `lib/plannerSchedule.ts` and the `App.tsx` Dashboard render/derivation code; see
//! `docs/stage17-dashboard.md` for the calculation-by-calculation mapping and the production
//! quirks deliberately preserved.

pub mod civil;
pub mod focus;
pub mod format;
pub mod metrics;
pub mod schedule;
pub mod wabi;

use std::collections::HashMap;

pub use civil::{CivilDate, FixedOffsetClock, LocalClock};
pub use focus::{focus_timeline, FocusRange, FocusTimeline, FOCUS_MILESTONES};
pub use metrics::SessionDays;

use civil::{js_date_only_as_local, to_iso_utc_string};
use format::{
    format_field_today_label, format_month_day, format_swiss_grade, format_unit_amount,
    format_unit_label, js_number, js_round,
};
use metrics::{
    clamp, clean_unit_label, course_minutes, health_label, remaining_units, upcoming_exams,
    with_schedule_health, workload_health,
};
use schedule::{calculate_scheduled_workload, events_for_course, schedule_health, scheduled_units};

use crate::academic::{
    completed_calendar_whole_units, AcademicState, CalendarEntry, Course, Priority, Semester, Task,
};
use crate::timer::WallTimestamp;

/// Production's default daily focus goal (`settings.dailyGoalMinutes`, `storage.ts`).
pub const DEFAULT_DAILY_GOAL_MINUTES: u32 = 120;

#[derive(Clone, Copy)]
pub struct DashboardInput<'a> {
    pub state: &'a AcademicState,
    pub now: WallTimestamp,
    pub clock: &'a dyn LocalClock,
    pub daily_goal_minutes: u32,
}

/// One row of "planned today" (`todayCalendarEntries` resolved against tasks/courses/semesters).
#[derive(Debug, Clone, PartialEq)]
pub struct QueueEntry {
    pub entry_id: String,
    pub has_task: bool,
    /// Task title, else the ad-hoc title, else "Calendar task".
    pub title: String,
    /// `getCalendarEntryUnitLabel`: "Sheet 3 of 6", "1/2 unit lectures", or "1/4 unit".
    pub unit_label: String,
    pub amount: f64,
    pub completed: bool,
    /// The task's priority (Medium when the entry has no task).
    pub priority: Priority,
    pub course_name: Option<String>,
    pub course_color: Option<String>,
    pub semester_name: Option<String>,
    /// `formatDate(task.dueDate)` ("Oct 9"), if the task has a due date.
    pub due_label: Option<String>,
    /// `formatTimeRange`: "Unscheduled", "09:00", or "09:00 - 10:00".
    pub time_range: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExamRow {
    pub title: String,
    pub course_name: Option<String>,
    /// `${exam.weight}` as JavaScript would print it.
    pub weight_label: String,
    pub preparedness: f64,
    pub days_until: i64,
    pub date_label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CourseRow {
    pub course_id: String,
    pub name: String,
    pub color: String,
    pub score: u32,
    pub semester_name: Option<String>,
    pub task_count: usize,
    pub minutes: u64,
    /// `formatSwissGrade(course.targetGrade)`.
    pub target_label: String,
    pub overdue: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DashboardMetrics {
    pub today: CivilDate,
    pub today_label: String,
    pub week_number: u32,
    pub daily_goal_minutes: u32,
    pub today_minutes: u64,
    pub goal_progress_percent: u32,
    pub streak_days: u32,
    /// Rolling last-7-days total (`weeklyActivity` sum) - *not* the Monday-based chart week.
    pub weekly_total_minutes: u64,
    /// `AppState.lifetimeStudyMinutes` (survives history pruning).
    pub lifetime_minutes: u64,
    pub session_day_count: usize,
    pub has_sessions: bool,
    /// `formatDate(firstSessionDate)`.
    pub first_session_label: Option<String>,
    pub open_task_count: usize,
    pub total_units_left: u64,
    pub units_per_day: f64,
    pub queue: Vec<QueueEntry>,
    pub queue_open_count: usize,
    pub exams: Vec<ExamRow>,
    /// Earliest unfinished task due date, else the next exam's date (`nearestDeadline`).
    pub nearest_deadline_label: Option<String>,
    pub earliest_exam_title: Option<String>,
    pub courses: Vec<CourseRow>,
    pub overall_score: u32,
    pub overall_label: &'static str,
}

impl DashboardMetrics {
    pub fn compute(input: DashboardInput<'_>) -> Self {
        let DashboardInput {
            state,
            now,
            clock,
            daily_goal_minutes,
        } = input;
        let today = clock.local_date(now);
        let goal = daily_goal_minutes.max(1);

        let active_semester_ids: std::collections::HashSet<&str> = state
            .semesters
            .iter()
            .filter(|s| !s.archived)
            .map(|s| s.id.as_str())
            .collect();
        let active_semesters: Vec<&Semester> =
            state.semesters.iter().filter(|s| !s.archived).collect();
        let active_courses: Vec<&Course> = state
            .courses
            .iter()
            .filter(|c| active_semester_ids.contains(c.semester_id.as_str()))
            .collect();
        let active_tasks: Vec<&Task> = state
            .tasks
            .iter()
            .filter(|t| active_semester_ids.contains(t.semester_id.as_str()))
            .collect();
        let active_exams: Vec<&crate::academic::Exam> = state
            .exams
            .iter()
            .filter(|e| active_semester_ids.contains(e.semester_id.as_str()))
            .collect();

        let sessions = SessionDays::new(&state.sessions, clock);
        let today_minutes = sessions.minutes_on(today);
        let goal_progress_percent = clamp(
            js_round(today_minutes as f64 / f64::from(goal) * 100.0),
            0.0,
            100.0,
        ) as u32;
        let streak_days = sessions.streak_days(today);
        let weekly_total_minutes: u64 = sessions.weekly_activity(today).iter().map(|b| b.1).sum();
        let day_set = sessions.day_set();
        let first_session_label = sessions
            .first_started_at()
            .map(|ts| format_month_day(clock.local_date(ts)));

        let open_task_count = state
            .tasks
            .iter()
            .filter(|t| remaining_units(t) > 0)
            .count();
        let total_units_left: u64 = state
            .tasks
            .iter()
            .map(|t| u64::from(remaining_units(t)))
            .sum();

        let units = scheduled_units(
            &state.timetable_events,
            &state.holidays,
            &active_semesters,
            today,
        );
        let workload = calculate_scheduled_workload(&active_tasks, &units, today, clock);

        // --- queue ("planned today") ------------------------------------------------------------
        let task_lookup: HashMap<&str, &Task> =
            state.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
        let course_lookup: HashMap<&str, &Course> =
            state.courses.iter().map(|c| (c.id.as_str(), c)).collect();
        let semester_lookup: HashMap<&str, &Semester> =
            state.semesters.iter().map(|s| (s.id.as_str(), s)).collect();
        let today_iso = today.to_iso();
        let mut today_entries: Vec<&CalendarEntry> = state
            .calendar_entries
            .iter()
            .filter(|e| {
                e.date.as_str() == today_iso
                    && (task_lookup.contains_key(e.task_id.as_str())
                        || e.ad_hoc_title.as_deref().is_some_and(|t| !t.is_empty()))
            })
            .collect();
        // `(a.startTime ?? a.createdAt).localeCompare(...)` - an HH:MM string is compared against
        // an ISO timestamp when an entry has no start time; reproduced as plain string order.
        today_entries.sort_by(|a, b| {
            a.completed.cmp(&b.completed).then_with(|| {
                let key = |e: &CalendarEntry| {
                    e.start_time
                        .clone()
                        .unwrap_or_else(|| to_iso_utc_string(e.created_at))
                };
                key(a).cmp(&key(b))
            })
        });
        let queue: Vec<QueueEntry> = today_entries
            .iter()
            .map(|entry| {
                let task = task_lookup.get(entry.task_id.as_str()).copied();
                let course = match task {
                    Some(t) => course_lookup.get(t.course_id.as_str()).copied(),
                    None => entry
                        .ad_hoc_course_id
                        .as_ref()
                        .and_then(|id| course_lookup.get(id.as_str()).copied()),
                };
                let semester = match task {
                    Some(t) => semester_lookup.get(t.semester_id.as_str()).copied(),
                    None => entry
                        .ad_hoc_semester_id
                        .as_ref()
                        .and_then(|id| semester_lookup.get(id.as_str()).copied()),
                };
                QueueEntry {
                    entry_id: entry.id.as_str().to_string(),
                    has_task: task.is_some(),
                    title: task
                        .map(|t| t.title.clone())
                        .or_else(|| entry.ad_hoc_title.clone().filter(|t| !t.is_empty()))
                        .unwrap_or_else(|| "Calendar task".to_string()),
                    unit_label: calendar_entry_unit_label(entry, task, &state.calendar_entries),
                    amount: entry.unit_amount.as_f64(),
                    completed: entry.completed,
                    priority: task.map_or(Priority::Medium, |t| t.priority),
                    course_name: course.map(|c| c.name.clone()),
                    course_color: course.map(|c| c.color.clone()),
                    semester_name: semester.map(|s| s.name.clone()),
                    due_label: task
                        .and_then(|t| t.due_date.as_ref())
                        .and_then(CivilDate::from_local_date)
                        .map(|d| format_month_day(js_date_only_as_local(clock, d))),
                    time_range: format_time_range(entry),
                }
            })
            .collect();
        let queue_open_count = queue.iter().filter(|e| !e.completed).count();

        // --- exams ------------------------------------------------------------------------------
        let upcoming = upcoming_exams(&state.exams, today, clock);
        let exams: Vec<ExamRow> = upcoming
            .iter()
            .map(|(exam, days)| ExamRow {
                title: exam.title.clone(),
                course_name: course_lookup
                    .get(exam.course_id.as_str())
                    .map(|c| c.name.clone()),
                weight_label: js_number(exam.weight),
                preparedness: exam.preparedness,
                days_until: *days,
                date_label: CivilDate::from_local_date(&exam.exam_date)
                    .map(|d| format_month_day(js_date_only_as_local(clock, d)))
                    .unwrap_or_default(),
            })
            .collect();

        // `nearestTaskDeadline` = lexicographically smallest due date among *all* unfinished dated
        // tasks (archived semesters included, overdue included); else the next exam's date.
        let nearest_task_deadline = state
            .tasks
            .iter()
            .filter(|t| remaining_units(t) > 0)
            .filter_map(|t| t.due_date.as_ref())
            .min();
        let nearest_deadline_label = nearest_task_deadline
            .or_else(|| upcoming.first().map(|(e, _)| &e.exam_date))
            .and_then(CivilDate::from_local_date)
            .map(|d| format_month_day(js_date_only_as_local(clock, d)));

        // --- course radar -----------------------------------------------------------------------
        let minutes_by_course = course_minutes(&state.sessions);
        let mut tasks_by_course: HashMap<&str, Vec<&Task>> = HashMap::new();
        for task in &state.tasks {
            tasks_by_course
                .entry(task.course_id.as_str())
                .or_default()
                .push(task);
        }
        let courses: Vec<CourseRow> = active_courses
            .iter()
            .map(|course| {
                let course_tasks = tasks_by_course
                    .get(course.id.as_str())
                    .cloned()
                    .unwrap_or_default();
                let course_exams: Vec<&crate::academic::Exam> = state
                    .exams
                    .iter()
                    .filter(|e| e.course_id == course.id)
                    .collect();
                let base = workload_health(&course_tasks, &course_exams, today, clock);
                let semester = semester_lookup.get(course.semester_id.as_str()).copied();
                let health = match semester {
                    Some(semester) => with_schedule_health(
                        base,
                        schedule_health(
                            &events_for_course(&state.timetable_events, course),
                            &state.holidays,
                            &[semester],
                            &course_exams,
                            today,
                            clock,
                        ),
                    ),
                    None => base,
                };
                CourseRow {
                    course_id: course.id.as_str().to_string(),
                    name: course.name.clone(),
                    color: course.color.clone(),
                    score: health.score,
                    semester_name: semester.map(|s| s.name.clone()),
                    task_count: course_tasks.len(),
                    minutes: minutes_by_course
                        .get(course.id.as_str())
                        .copied()
                        .unwrap_or(0),
                    target_label: format_swiss_grade(course.target_grade),
                    overdue: health.overdue,
                }
            })
            .collect();

        // --- overall score (header pill) --------------------------------------------------------
        let all_events: Vec<&crate::academic::TimetableEvent> =
            state.timetable_events.iter().collect();
        let overall_score = schedule_health(
            &all_events,
            &state.holidays,
            &active_semesters,
            &active_exams,
            today,
            clock,
        )
        .map(|s| s.score)
        .unwrap_or_else(|| {
            if active_tasks.is_empty() {
                0
            } else {
                workload_health(&active_tasks, &active_exams, today, clock).score
            }
        });

        DashboardMetrics {
            today,
            today_label: format_field_today_label(today),
            week_number: week_number(now, today, clock),
            daily_goal_minutes: goal,
            today_minutes,
            goal_progress_percent,
            streak_days,
            weekly_total_minutes,
            lifetime_minutes: state.lifetime_study_minutes,
            session_day_count: day_set.len(),
            has_sessions: !state.sessions.is_empty(),
            first_session_label,
            open_task_count,
            total_units_left,
            units_per_day: workload.units_per_day,
            queue,
            queue_open_count,
            earliest_exam_title: upcoming.first().map(|(e, _)| e.title.clone()),
            exams,
            nearest_deadline_label,
            courses,
            overall_score,
            overall_label: health_label(overall_score),
        }
    }

    /// `healthClass`: which tone the header pill takes (production CSS class).
    pub fn overall_tone(&self) -> &'static str {
        match self.overall_score {
            75.. => "strong",
            55..=74 => "steady",
            35..=54 => "watch",
            _ => "critical",
        }
    }
}

/// `Math.ceil((((now - yearStart) / 86400000) + yearStart.getDay() + 1) / 7)`, where `yearStart`
/// is local midnight on Jan 1 of the current year and `now` the current instant (so the week
/// number can tick over mid-day exactly like production's).
pub fn week_number(now: WallTimestamp, today: CivilDate, clock: &dyn LocalClock) -> u32 {
    let jan1 = CivilDate::from_ymd(today.year(), 1, 1).expect("January 1st is always valid");
    let since = (now.unix_millis - clock.local_midnight(jan1).unix_millis) as f64 / 86_400_000.0;
    ((since + f64::from(jan1.weekday()) + 1.0) / 7.0).ceil() as u32
}

/// `formatTimeRange`.
pub fn format_time_range(entry: &CalendarEntry) -> String {
    match (&entry.start_time, &entry.end_time) {
        (None, _) => "Unscheduled".to_string(),
        (Some(start), Some(end)) if !end.is_empty() => format!("{start} - {end}"),
        (Some(start), _) => start.clone(),
    }
}

/// `getCalendarEntryUnitStart` + `getCalendarEntryUnitLabel`.
fn calendar_entry_unit_label(
    entry: &CalendarEntry,
    task: Option<&Task>,
    all_entries: &[CalendarEntry],
) -> String {
    let amount = entry.unit_amount.as_f64();
    let Some(task) = task else {
        return format_unit_amount(amount);
    };
    if amount == 1.0 {
        let unit_start = match entry.unit_start {
            Some(start) => clamp(f64::from(start).floor(), 1.0, f64::from(task.total_units)),
            None => {
                let mut task_entries: Vec<&CalendarEntry> = all_entries
                    .iter()
                    .filter(|e| e.task_id == task.id)
                    .collect();
                task_entries.sort_by(|a, b| {
                    a.date
                        .as_str()
                        .cmp(b.date.as_str())
                        .then_with(|| {
                            a.start_time
                                .as_deref()
                                .unwrap_or("")
                                .cmp(b.start_time.as_deref().unwrap_or(""))
                        })
                        .then_with(|| {
                            to_iso_utc_string(a.created_at).cmp(&to_iso_utc_string(b.created_at))
                        })
                });
                let base = clamp(
                    f64::from(task.completed_units)
                        - completed_calendar_whole_units(all_entries, &task.id) as f64,
                    0.0,
                    f64::from(task.total_units),
                );
                let mut before = 0.0;
                for item in &task_entries {
                    if item.id == entry.id {
                        break;
                    }
                    before += item.unit_amount.as_f64();
                }
                clamp(
                    (base + before).floor() + 1.0,
                    1.0,
                    f64::from(task.total_units),
                )
            }
        };
        format!(
            "{} {} of {}",
            clean_unit_label(&task.unit_label, &task.title),
            unit_start as u32,
            task.total_units
        )
    } else {
        format!(
            "{} {}",
            format_unit_amount(amount),
            format_unit_label(&task.unit_label, 2)
        )
    }
}
