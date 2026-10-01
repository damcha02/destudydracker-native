//! The Wabi-Sabi style's own surfaces (Stage 19): its Dashboard ("Today"), the Kokoro sidebar and
//! Quiet mode. Ported from production `App.tsx` (`renderWabiSabiDashboard`, `renderWabiSidebar`,
//! `renderWabiQuietMode`, `getFieldDashboardData`, `getOpenRowsForDate`, `getWabiUnitInfo`) and
//! `lib/plannerSchedule.ts` (`buildDailyTimeline`, `expandDailyTodoDates`) / `lib/examPhase.ts`.
//!
//! Like the rest of this module: pure, no clock reads (the caller passes "today"), no Slint. The
//! inputs are the canonical [`AcademicState`] plus the already-computed Field Notebook
//! [`DashboardMetrics`] (the Wabi-Sabi Dashboard shares `getFieldDashboardData` with it).

use std::collections::{HashMap, HashSet};

use super::civil::{js_date_only_as_local, CivilDate, LocalClock};
use super::format::{display_time, format_minutes, format_month_day, format_unit_amount};
use super::schedule::{expand_timetable_events_with, ExpandOptions, Occurrence};
use super::{DashboardMetrics, QueueEntry};
use crate::academic::{
    AcademicState, Course, DailyTodo, Exam, Semester, SemesterPhase, Task, TaskSubtype,
    TimetableEvent, TimetableEventKind,
};

/// What a "mark" (the round button left of a row, or MARK DONE) toggles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkTarget {
    CalendarEntry(String),
    TimetableOccurrence { event_id: String, date: String },
    Todo { todo_id: String, date: String },
}

/// What START sends to the Timer (`focusTaskFromDashboard` / `focusTodoInTimer`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartTarget {
    Task(String),
    Todo(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineKind {
    Occurrence,
    SheetRelease,
    SheetDeadline,
    Todo,
}

/// `DailyTimelineRow` for the two input kinds the Wabi-Sabi surfaces feed it (event occurrences
/// and to-dos; exams/calendar entries are always passed empty there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineRow {
    pub kind: TimelineKind,
    pub time: Option<String>,
    pub end_time: Option<String>,
    pub sort_minutes: u32,
    pub title: String,
    pub course_id: Option<String>,
    pub task_id: Option<String>,
    pub completed: bool,
    pub ref_id: String,
    pub occurrence_date: CivilDate,
}

impl TimelineRow {
    pub fn mark(&self) -> MarkTarget {
        match self.kind {
            TimelineKind::Todo => MarkTarget::Todo {
                todo_id: self.ref_id.clone(),
                date: self.occurrence_date.to_iso(),
            },
            _ => MarkTarget::TimetableOccurrence {
                event_id: self.ref_id.clone(),
                date: self.occurrence_date.to_iso(),
            },
        }
    }
}

/// `timeToSortMinutes`: untimed rows sort last (`24 * 60 + 1`).
fn time_to_sort_minutes(time: Option<&str>) -> u32 {
    let Some(time) = time else {
        return 24 * 60 + 1;
    };
    let mut parts = time.split(':');
    let hours: u32 = parts
        .next()
        .and_then(|h| h.trim().parse().ok())
        .unwrap_or(0);
    let minutes: u32 = parts
        .next()
        .and_then(|m| m.trim().parse().ok())
        .unwrap_or(0);
    hours * 60 + minutes
}

/// `expandDailyTodoDates(todo, date, date).length > 0`.
pub fn todo_occurs_on(todo: &DailyTodo, date: CivilDate) -> bool {
    let Some(anchor) = CivilDate::from_local_date(&todo.date) else {
        return false;
    };
    if !todo.repeat_weekly {
        return anchor == date;
    }
    let series_end = todo
        .recurrence_end_date
        .as_ref()
        .and_then(CivilDate::from_local_date)
        .map_or(date, |end| end.min(date));
    super::schedule::expand_weekday_from(anchor, date, series_end)
        .into_iter()
        .any(|d| !todo.skipped_occurrences.iter().any(|s| s == &d.to_iso()))
}

/// `buildDailyTimeline(date, { eventOccurrences, exams: [], calendarEntries: [], dailyTodos })`.
pub fn build_daily_timeline(
    date: CivilDate,
    occurrences: &[Occurrence<'_>],
    todos: &[DailyTodo],
) -> Vec<TimelineRow> {
    let iso = date.to_iso();
    let mut rows = Vec::new();
    for occurrence in occurrences.iter().filter(|o| o.date == date) {
        let event = occurrence.event;
        let (kind, title) = match event.kind {
            TimetableEventKind::SheetRelease => (
                TimelineKind::SheetRelease,
                format!("{} released", event.label),
            ),
            TimetableEventKind::SheetDeadline => {
                (TimelineKind::SheetDeadline, format!("{} due", event.label))
            }
            TimetableEventKind::Occurrence => (TimelineKind::Occurrence, event.label.clone()),
        };
        rows.push(TimelineRow {
            kind,
            time: Some(occurrence.time().to_string()),
            end_time: occurrence.end_time().map(str::to_string),
            sort_minutes: time_to_sort_minutes(Some(occurrence.time())),
            title,
            course_id: Some(event.course_id.as_str().to_string()),
            task_id: Some(event.task_id.as_str().to_string()),
            completed: event.completed_occurrences.iter().any(|d| d == &iso),
            ref_id: event.id.as_str().to_string(),
            occurrence_date: date,
        });
    }
    for todo in todos.iter().filter(|t| todo_occurs_on(t, date)) {
        // `getTodoOccurrenceTime`: a "this occurrence only" time override for repeating to-dos.
        let (time, end_time) = match todo
            .repeat_weekly
            .then(|| todo.occurrence_times.get(&iso))
            .flatten()
        {
            Some((time, end)) => (time.clone(), end.clone()),
            None => (todo.time.clone(), todo.end_time.clone()),
        };
        rows.push(TimelineRow {
            kind: TimelineKind::Todo,
            sort_minutes: time_to_sort_minutes(time.as_deref()),
            time,
            end_time,
            title: todo.title.clone(),
            course_id: None,
            task_id: None,
            completed: if todo.repeat_weekly {
                todo.completed_occurrences.iter().any(|d| d == &iso)
            } else {
                todo.completed
            },
            ref_id: todo.id.as_str().to_string(),
            occurrence_date: date,
        });
    }
    // `rows.sort((a, b) => a.sortMinutes - b.sortMinutes)` - stable.
    rows.sort_by_key(|r| r.sort_minutes);
    rows
}

/// `getLastExamDate`: the latest exam of a semester that falls after its end date.
pub fn last_exam_date(semester: &Semester, exams: &[Exam]) -> Option<CivilDate> {
    let end = semester.end_date.as_ref().map(|d| d.as_str().to_string());
    exams
        .iter()
        .filter(|e| e.semester_id == semester.id)
        .filter(|e| {
            end.as_deref()
                .map_or(true, |end| e.exam_date.as_str() > end)
        })
        .map(|e| e.exam_date.as_str())
        .max()
        .and_then(CivilDate::parse_iso)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemesterStage {
    Lectures,
    Prep,
    Done,
}

/// `getSemesterStage`.
pub fn semester_stage(semester: &Semester, exams: &[Exam], today: CivilDate) -> SemesterStage {
    if semester.phase == SemesterPhase::ExamPrep {
        return SemesterStage::Prep;
    }
    let today_iso = today.to_iso();
    match semester.end_date.as_ref() {
        None => return SemesterStage::Lectures,
        Some(end) if today_iso.as_str() <= end.as_str() => return SemesterStage::Lectures,
        Some(_) => {}
    }
    match last_exam_date(semester, exams) {
        Some(last) if today > last => SemesterStage::Done,
        _ => SemesterStage::Prep,
    }
}

/// `getWabiUnitInfo`: "Serie 2" / "2 of 13" - the next unit to do of a unit-counting task.
pub fn wabi_unit_info(task: &Task) -> (String, Option<String>) {
    let unit_based = task.total_units > 0 && task.subtype != TaskSubtype::Other;
    let number = task.total_units.min((task.completed_units + 1).max(1));
    if unit_based {
        (
            format!("{} {number}", task.title),
            Some(format!("{number} of {}", task.total_units)),
        )
    } else {
        (task.title.clone(), None)
    }
}

/// Everything the Wabi-Sabi surfaces need beyond [`DashboardMetrics`].
#[derive(Clone, Copy)]
pub struct WabiInput<'a> {
    pub state: &'a AcademicState,
    pub metrics: &'a DashboardMetrics,
    pub clock: &'a dyn LocalClock,
    /// `selectedTaskId` (a title click makes a planned task the current one).
    pub selected_task_id: Option<&'a str>,
    /// `displayTime`'s 12-hour vs 24-hour choice (the machine's locale).
    pub twelve_hour: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WabiOneThing {
    pub title: String,
    pub meta: String,
    pub start: Option<StartTarget>,
    pub mark_done: Option<MarkTarget>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WabiDeadline {
    pub title: String,
    pub next: bool,
    /// `{course ?? "General"}{ · released Sep 23}`.
    pub status: String,
    /// `Sep 30 · 18:00`.
    pub due: String,
    /// `today` / `tomorrow` / `in 3 days`.
    pub due_relative: String,
    pub task_id: Option<String>,
    pub mark: MarkTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WabiPlannedRow {
    pub title: String,
    pub subject: String,
    /// `1 unit` etc. for calendar entries; empty for timeline rows.
    pub amount: String,
    pub due: String,
    pub completed: bool,
    pub selected: bool,
    pub mark: MarkTarget,
    /// A title click selects this task (calendar entries with a task only).
    pub select_task: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WabiDashboard {
    pub today_label: String,
    pub remaining_label: String,
    pub one_thing: WabiOneThing,
    pub deadlines: Vec<WabiDeadline>,
    pub planned: Vec<WabiPlannedRow>,
}

struct Lookups<'a> {
    tasks: HashMap<&'a str, &'a Task>,
    courses: HashMap<&'a str, &'a Course>,
}

impl<'a> Lookups<'a> {
    fn new(state: &'a AcademicState) -> Self {
        Self {
            tasks: state.tasks.iter().map(|t| (t.id.as_str(), t)).collect(),
            courses: state.courses.iter().map(|c| (c.id.as_str(), c)).collect(),
        }
    }
    fn course_name(&self, id: &str) -> Option<&'a str> {
        self.courses.get(id).map(|c| c.name.as_str())
    }
}

/// `wabiExpandOptions`: every prep task id, and per semester its last exam date.
fn expand_options<'a>(state: &'a AcademicState, semester: &Semester) -> ExpandOptions<'a> {
    ExpandOptions {
        prep_task_ids: state
            .tasks
            .iter()
            .filter(|t| t.prep)
            .map(|t| t.id.as_str())
            .collect::<HashSet<_>>(),
        prep_end_date: last_exam_date(semester, &state.exams),
    }
}

/// Occurrences of every active semester's events in `[start, end]`, with the Wabi-Sabi prep options.
fn wabi_occurrences<'a>(
    state: &'a AcademicState,
    events: &'a [&'a TimetableEvent],
    start: CivilDate,
    end: CivilDate,
) -> Vec<Occurrence<'a>> {
    state
        .semesters
        .iter()
        .filter(|s| !s.archived)
        .flat_map(|semester| {
            let options = expand_options(state, semester);
            expand_timetable_events_with(
                events,
                &state.holidays,
                semester,
                start,
                end,
                Some(&options),
            )
        })
        .collect()
}

/// `formatDate("YYYY-MM-DD")` with production's UTC-midnight quirk (see `js_date_only_as_local`).
fn format_date(clock: &dyn LocalClock, date: CivilDate) -> String {
    format_month_day(js_date_only_as_local(clock, date))
}

fn join_dot<'s>(parts: impl IntoIterator<Item = Option<&'s str>>) -> String {
    parts
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" \u{b7} ")
}

/// `getFieldDashboardData`'s `nextEntry`: the selected task's entry, else the first open entry,
/// else the first entry.
fn next_entry<'q>(
    queue: &'q [QueueEntry],
    state: &AcademicState,
    selected: Option<&str>,
) -> Option<&'q QueueEntry> {
    let entry_task = |entry: &QueueEntry| {
        state
            .calendar_entries
            .iter()
            .find(|e| e.id.as_str() == entry.entry_id)
            .map(|e| e.task_id.as_str().to_string())
    };
    selected
        .and_then(|sel| queue.iter().find(|e| entry_task(e).as_deref() == Some(sel)))
        .or_else(|| queue.iter().find(|e| !e.completed))
        .or_else(|| queue.first())
}

pub fn wabi_dashboard(input: WabiInput<'_>) -> WabiDashboard {
    let WabiInput {
        state,
        metrics,
        clock,
        selected_task_id,
        twelve_hour,
    } = input;
    let today = metrics.today;
    let lookups = Lookups::new(state);
    let events: Vec<&TimetableEvent> = state.timetable_events.iter().collect();

    // --- "3 open · 1h 7m to today's goal" -------------------------------------------------------
    let minutes_to_goal =
        u64::from(metrics.daily_goal_minutes).saturating_sub(metrics.today_minutes);
    let open = if metrics.queue_open_count > 0 {
        format!("{} open \u{b7} ", metrics.queue_open_count)
    } else {
        String::new()
    };
    let remaining_label = if minutes_to_goal > 0 {
        format!("{open}{} to today's goal", format_minutes(minutes_to_goal))
    } else {
        format!("{open}goal met")
    };

    // --- COMING UP: released, unsolved sheet deadlines in the next ~two months ------------------
    let horizon = wabi_occurrences(state, &events, today.add_days(-14), today.add_days(60));
    let mut coming: Vec<(&Occurrence<'_>, Option<CivilDate>, i64)> = horizon
        .iter()
        .filter(|o| {
            o.event.kind == TimetableEventKind::SheetDeadline
                && o.date >= today
                && !o
                    .event
                    .completed_occurrences
                    .iter()
                    .any(|d| d == &o.date.to_iso())
        })
        .map(|o| {
            // The release that belongs to this due date sits the same gap before it as the two
            // series' first dates (`gapDays`).
            let release = state.timetable_events.iter().find(|e| {
                e.kind == TimetableEventKind::SheetRelease && e.task_id == o.event.task_id
            });
            let release_date = release.and_then(|release| {
                let gap = o
                    .event_date()
                    .zip(CivilDate::from_local_date(&release.date))
                    .map_or(0, |(due, rel)| rel.days_until(due));
                Some(o.date.add_days(-gap))
            });
            (o, release_date, today.days_until(o.date))
        })
        .filter(|(_, release, _)| release.map_or(true, |r| r <= today))
        .collect();
    coming.sort_by(|a, b| {
        let key = |o: &Occurrence<'_>| {
            let time = o.time();
            format!(
                "{}{}",
                o.date.to_iso(),
                if time.is_empty() { "99:99" } else { time }
            )
        };
        key(a.0).cmp(&key(b.0))
    });
    coming.truncate(6);
    let deadlines: Vec<WabiDeadline> = coming
        .iter()
        .enumerate()
        .map(|(index, (o, release, days_left))| {
            let course = lookups
                .course_name(o.event.course_id.as_str())
                .unwrap_or("General");
            let released = release
                .map(|r| format!(" \u{b7} released {}", format_date(clock, r)))
                .unwrap_or_default();
            let time = o.time();
            WabiDeadline {
                title: o.event.label.clone(),
                next: index == 0,
                status: format!("{course}{released}"),
                due: if time.is_empty() {
                    format_date(clock, o.date)
                } else {
                    format!(
                        "{} \u{b7} {}",
                        format_date(clock, o.date),
                        display_time(time, twelve_hour)
                    )
                },
                due_relative: match days_left {
                    0 => "today".to_string(),
                    1 => "tomorrow".to_string(),
                    n => format!("in {n} days"),
                },
                task_id: lookups
                    .tasks
                    .get(o.event.task_id.as_str())
                    .map(|t| t.id.as_str().to_string()),
                mark: MarkTarget::TimetableOccurrence {
                    event_id: o.event.id.as_str().to_string(),
                    date: o.date.to_iso(),
                },
            }
        })
        .collect();

    // --- today's to-dos and course items (sheets show under COMING UP instead) -----------------
    let today_occurrences = wabi_occurrences(state, &events, today, today);
    let today_rows: Vec<TimelineRow> =
        build_daily_timeline(today, &today_occurrences, &state.daily_todos)
            .into_iter()
            .filter(|r| {
                !matches!(
                    r.kind,
                    TimelineKind::SheetRelease | TimelineKind::SheetDeadline
                )
            })
            .collect();

    // --- ONE THING -------------------------------------------------------------------------------
    let next = next_entry(&metrics.queue, state, selected_task_id);
    let next_task = next.and_then(|entry| {
        state
            .calendar_entries
            .iter()
            .find(|e| e.id.as_str() == entry.entry_id)
            .and_then(|e| lookups.tasks.get(e.task_id.as_str()).copied())
    });
    let mut one = WabiOneThing {
        title: next_task
            .map(|t| t.title.clone())
            .or_else(|| next.filter(|e| !e.has_task).map(|e| e.title.clone()))
            .unwrap_or_else(|| "Plan the next study block".to_string()),
        meta: match next {
            Some(entry) => format!(
                "{} \u{b7} {} \u{b7} {}",
                entry.unit_label,
                entry.course_name.as_deref().unwrap_or("General focus"),
                entry
                    .due_label
                    .as_ref()
                    .map(|d| format!("due {d}"))
                    .unwrap_or_else(|| entry.time_range.clone())
            ),
            None => "No task pinned for today \u{b7} open the planner calendar to place one unit."
                .to_string(),
        },
        start: next_task.map(|t| StartTarget::Task(t.id.as_str().to_string())),
        mark_done: next.map(|e| MarkTarget::CalendarEntry(e.entry_id.clone())),
    };
    if next.is_none() {
        if let Some(row) = today_rows.iter().find(|r| !r.completed) {
            let row_task = (row.kind != TimelineKind::Todo)
                .then(|| {
                    state
                        .timetable_events
                        .iter()
                        .find(|e| e.id.as_str() == row.ref_id)
                })
                .flatten()
                .and_then(|e| lookups.tasks.get(e.task_id.as_str()).copied());
            one.title = row_task.map_or_else(|| row.title.clone(), |t| wabi_unit_info(t).0);
            let course = row
                .course_id
                .as_deref()
                .and_then(|id| lookups.course_name(id));
            let when = row
                .time
                .as_deref()
                .map(|t| display_time(t, twelve_hour))
                .unwrap_or_else(|| "any time today".to_string());
            one.meta = join_dot([
                Some(if row.kind == TimelineKind::Todo {
                    "To-do"
                } else {
                    course.unwrap_or("")
                }),
                Some(when.as_str()),
            ]);
            one.start = match row_task {
                Some(task) => Some(StartTarget::Task(task.id.as_str().to_string())),
                None if row.kind == TimelineKind::Todo => {
                    Some(StartTarget::Todo(row.title.clone()))
                }
                None => None,
            };
            one.mark_done = Some(row.mark());
        } else if let Some((o, _, _)) = coming.first() {
            let task = lookups.tasks.get(o.event.task_id.as_str()).copied();
            one.title = task.map_or_else(|| o.event.label.clone(), |t| wabi_unit_info(t).0);
            let due = format!("due {}", format_date(clock, o.date));
            one.meta = join_dot([
                lookups.course_name(o.event.course_id.as_str()),
                Some(due.as_str()),
            ]);
            one.start = task.map(|t| StartTarget::Task(t.id.as_str().to_string()));
            one.mark_done = None;
        } else {
            one.start = None;
            one.mark_done = None;
        }
    }

    // --- PLANNED TODAY: timeline rows, then the calendar entries ---------------------------------
    let mut planned: Vec<WabiPlannedRow> = today_rows
        .iter()
        .map(|row| WabiPlannedRow {
            title: row.title.clone(),
            subject: if row.kind == TimelineKind::Todo {
                "To-do".to_string()
            } else {
                row.course_id
                    .as_deref()
                    .and_then(|id| lookups.course_name(id))
                    .unwrap_or("General")
                    .to_string()
            },
            amount: String::new(),
            due: match (&row.time, &row.end_time) {
                (Some(t), Some(end)) if !end.is_empty() => format!(
                    "{}\u{2013}{}",
                    display_time(t, twelve_hour),
                    display_time(end, twelve_hour)
                ),
                (Some(t), _) => display_time(t, twelve_hour),
                (None, _) => "any time".to_string(),
            },
            completed: row.completed,
            selected: false,
            mark: row.mark(),
            select_task: None,
        })
        .collect();
    for entry in &metrics.queue {
        let task_id = state
            .calendar_entries
            .iter()
            .find(|e| e.id.as_str() == entry.entry_id)
            .map(|e| e.task_id.as_str())
            .filter(|id| lookups.tasks.contains_key(id));
        planned.push(WabiPlannedRow {
            title: entry.title.clone(),
            subject: entry
                .course_name
                .clone()
                .unwrap_or_else(|| "General".to_string()),
            amount: format_unit_amount(entry.amount),
            due: entry
                .due_label
                .clone()
                .unwrap_or_else(|| entry.time_range.clone()),
            completed: entry.completed,
            selected: task_id.is_some() && task_id == selected_task_id,
            mark: MarkTarget::CalendarEntry(entry.entry_id.clone()),
            select_task: task_id.map(str::to_string),
        });
    }

    WabiDashboard {
        today_label: metrics.today_label.clone(),
        remaining_label,
        one_thing: one,
        deadlines,
        planned,
    }
}

/// The sidebar's "current" semester and its courses (Plan/Timer submenus).
#[derive(Debug, Clone, PartialEq)]
pub struct WabiSidebarCourse {
    pub id: String,
    pub name: String,
    pub color: String,
    /// Exam prep, no exam ahead: drawn faded (`.wabi-plan-course.quiet`).
    pub quiet: bool,
    /// Exam prep, exam ahead: shows "12d"/"today".
    pub days_to_exam: Option<String>,
    pub tasks: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WabiSidebar {
    pub score: u32,
    pub label: &'static str,
    /// 0 ok, 1 steady, 2 watch, 3 critical (`wabiScoreColor`).
    pub tone: u8,
    pub tended_today: String,
    pub goal_percent: u32,
    pub semester_name: Option<String>,
    pub semester_in_prep: bool,
    pub courses: Vec<WabiSidebarCourse>,
}

pub fn wabi_sidebar(state: &AcademicState, metrics: &DashboardMetrics) -> WabiSidebar {
    let today = metrics.today;
    let today_iso = today.to_iso();
    let score = metrics.overall_score;
    let active: Vec<&Semester> = state.semesters.iter().filter(|s| !s.archived).collect();
    // The active semester whose dates contain today (kept in focus through its exam prep), else
    // the first active one.
    let in_focus = |s: &&Semester| match (&s.start_date, &s.end_date) {
        (Some(start), Some(end)) => {
            today_iso.as_str() >= start.as_str()
                && (today_iso.as_str() <= end.as_str()
                    || semester_stage(s, &state.exams, today) == SemesterStage::Prep)
        }
        _ => false,
    };
    let current = active
        .iter()
        .find(|s| in_focus(s))
        .or_else(|| active.first())
        .copied();
    let stage = current.map(|s| semester_stage(s, &state.exams, today));
    let in_prep = stage == Some(SemesterStage::Prep);
    let courses = current
        .map(|semester| {
            state
                .courses
                .iter()
                .filter(|c| c.semester_id == semester.id)
                .map(|course| {
                    let ahead = state
                        .exams
                        .iter()
                        .filter(|e| {
                            e.course_id == course.id && e.exam_date.as_str() >= today_iso.as_str()
                        })
                        .min_by(|a, b| a.exam_date.as_str().cmp(b.exam_date.as_str()));
                    let prep_active = in_prep && ahead.is_some();
                    WabiSidebarCourse {
                        id: course.id.as_str().to_string(),
                        name: course.name.clone(),
                        color: course.color.clone(),
                        quiet: in_prep && !prep_active,
                        days_to_exam: ahead
                            .filter(|_| prep_active)
                            .and_then(|e| CivilDate::from_local_date(&e.exam_date))
                            .map(|d| match today.days_until(d) {
                                0 => "today".to_string(),
                                n => format!("{n}d"),
                            }),
                        tasks: state
                            .tasks
                            .iter()
                            .filter(|t| t.course_id == course.id)
                            .map(|t| (t.id.as_str().to_string(), t.title.clone()))
                            .collect(),
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    WabiSidebar {
        score,
        label: match score {
            75.. => "Strong",
            55..=74 => "Steady",
            35..=54 => "Watch",
            _ => "Critical",
        },
        tone: match score {
            75.. => 0,
            55..=74 => 1,
            35..=54 => 2,
            _ => 3,
        },
        tended_today: format_minutes(metrics.today_minutes),
        goal_percent: metrics.goal_progress_percent,
        semester_name: current.map(|s| s.name.clone()),
        semester_in_prep: in_prep,
        courses,
    }
}

/// Quiet mode's focus line and the "other open tasks" dropdown.
#[derive(Debug, Clone, PartialEq)]
pub struct WabiQuiet {
    pub title: String,
    pub meta: String,
    /// (task id, title, course name) of the other open planned tasks.
    pub others: Vec<(String, String, String)>,
}

/// `renderWabiQuietMode`. `quiet_task_id` is the dropdown pick (production also lets the Timer's
/// linked task win; Timer task linking is not migrated, so there is never one natively), and
/// `timer_goal` the Timer's goal text while a session is active.
pub fn wabi_quiet(
    state: &AcademicState,
    metrics: &DashboardMetrics,
    clock: &dyn LocalClock,
    quiet_task_id: Option<&str>,
    selected_task_id: Option<&str>,
    timer_goal: Option<&str>,
) -> WabiQuiet {
    let lookups = Lookups::new(state);
    let entry_task = |entry: &QueueEntry| -> Option<&Task> {
        state
            .calendar_entries
            .iter()
            .find(|e| e.id.as_str() == entry.entry_id)
            .and_then(|e| lookups.tasks.get(e.task_id.as_str()).copied())
    };
    let quiet_task = quiet_task_id.and_then(|id| lookups.tasks.get(id).copied());
    let quiet_entry = quiet_task.and_then(|task| {
        metrics
            .queue
            .iter()
            .find(|e| entry_task(e).is_some_and(|t| t.id == task.id))
    });
    let use_quiet = quiet_task.is_some() && !quiet_entry.is_some_and(|e| e.completed);
    let next = next_entry(&metrics.queue, state, selected_task_id);
    let next_task = next.and_then(entry_task);
    let focus_task = if use_quiet { quiet_task } else { next_task };
    let goal_only = timer_goal.map(str::trim).filter(|g| !g.is_empty());

    let title = match (goal_only, focus_task) {
        (Some(goal), _) => goal.to_string(),
        (None, Some(task)) => wabi_unit_info(task).0,
        (None, None) => next
            .filter(|e| !e.has_task)
            .map(|e| e.title.clone())
            .unwrap_or_else(|| "Plan the next study block".to_string()),
    };
    let meta = if goal_only.is_some() {
        "To-do".to_string()
    } else if let (true, Some(task)) = (use_quiet, quiet_task) {
        let due = task
            .due_date
            .as_ref()
            .and_then(CivilDate::from_local_date)
            .map(|d| format!("due {}", format_date(clock, d)));
        let position = wabi_unit_info(task).1;
        join_dot([
            Some(
                lookups
                    .course_name(task.course_id.as_str())
                    .unwrap_or("General focus"),
            ),
            position.as_deref(),
            due.as_deref(),
        ])
    } else {
        match next {
            Some(entry) => format!(
                "{} \u{b7} {} \u{b7} {}",
                entry.unit_label,
                entry.course_name.as_deref().unwrap_or("General focus"),
                entry
                    .due_label
                    .as_ref()
                    .map(|d| format!("due {d}"))
                    .unwrap_or_else(|| entry.time_range.clone())
            ),
            None => "No task pinned for today \u{b7} open the planner calendar to place one unit."
                .to_string(),
        }
    };
    let others = metrics
        .queue
        .iter()
        .filter(|e| !e.completed)
        .filter_map(|e| entry_task(e).map(|t| (e, t)))
        .filter(|(_, t)| focus_task.map_or(true, |f| f.id != t.id))
        .map(|(_, t)| {
            (
                t.id.as_str().to_string(),
                t.title.clone(),
                lookups
                    .course_name(t.course_id.as_str())
                    .unwrap_or("General")
                    .to_string(),
            )
        })
        .collect();
    WabiQuiet {
        title,
        meta,
        others,
    }
}

#[cfg(test)]
mod tests;
