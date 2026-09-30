//! Pure ports of production's `metrics.ts` calculations the Dashboard reads. Every function here
//! is a transliteration of the TypeScript original (function names in doc comments), including its
//! quirks; nothing is "improved". None of them read a clock, timezone, or the filesystem: the
//! caller passes `today` and a [`LocalClock`].

use std::collections::HashSet;

use super::civil::{js_date_only_as_local, CivilDate, LocalClock};
use super::format::js_round;
use crate::academic::{Course, Exam, LocalDate, StudySession, Task};
use crate::timer::WallTimestamp;

/// `clamp` in production: `Math.max(min, Math.min(max, value))` (never panics when `min > max`,
/// unlike `f64::clamp`).
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    min.max(max.min(value))
}

/// `daysUntil(date)`: whole days from `today` to the date's *local-viewed* midnight (see
/// [`js_date_only_as_local`] for the date-only-string quirk). `None` for an unparseable date.
pub fn days_until(clock: &dyn LocalClock, today: CivilDate, date: &LocalDate) -> Option<i64> {
    let target = CivilDate::from_local_date(date)?;
    Some(today.days_until(js_date_only_as_local(clock, target)))
}

/// `getRemainingUnits`.
pub fn remaining_units(task: &Task) -> u32 {
    task.total_units.saturating_sub(task.completed_units)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Workload {
    pub total_units: f64,
    pub completed_units: f64,
    pub remaining_units: f64,
    pub progress: f64,
    pub units_per_day: f64,
    pub days_left: Option<i64>,
    pub nearest_due_date: Option<CivilDate>,
    pub undated_remaining_units: u32,
}

/// `getNearestDeadline`: the earliest due date among unfinished, dated tasks (ties keep the first
/// in list order, like JavaScript's stable sort).
fn nearest_deadline(
    tasks: &[&Task],
    today: CivilDate,
    clock: &dyn LocalClock,
) -> Option<(CivilDate, i64)> {
    let mut best: Option<(CivilDate, i64)> = None;
    for task in tasks {
        if remaining_units(task) == 0 {
            continue;
        }
        let Some(due) = task.due_date.as_ref() else {
            continue;
        };
        let Some(days) = days_until(clock, today, due) else {
            continue;
        };
        if best.map_or(true, |(_, best_days)| days < best_days) {
            best = CivilDate::from_local_date(due).map(|d| (d, days));
        }
    }
    best
}

/// `calculateAggregateWorkload` (numeric fields only; the human message belongs to the Planner).
pub fn calculate_aggregate_workload(
    tasks: &[&Task],
    today: CivilDate,
    clock: &dyn LocalClock,
) -> Workload {
    let total_units: f64 = tasks.iter().map(|t| f64::from(t.total_units.max(1))).sum();
    let completed_units: f64 = tasks
        .iter()
        .map(|t| clamp(f64::from(t.completed_units), 0.0, f64::from(t.total_units)))
        .sum();
    let remaining = (total_units - completed_units).max(0.0);
    let unfinished: Vec<&&Task> = tasks.iter().filter(|t| remaining_units(t) > 0).collect();
    let dated: Vec<&&&Task> = unfinished.iter().filter(|t| t.due_date.is_some()).collect();
    let nearest = nearest_deadline(tasks, today, clock);
    let undated_remaining: u32 = unfinished
        .iter()
        .filter(|t| t.due_date.is_none())
        .map(|t| remaining_units(t))
        .sum();
    let progress = if total_units > 0.0 {
        js_round(completed_units / total_units * 100.0)
    } else {
        0.0
    };

    if remaining <= 0.0 {
        return Workload {
            total_units,
            completed_units,
            remaining_units: remaining,
            progress,
            units_per_day: 0.0,
            days_left: Some(0),
            nearest_due_date: nearest.map(|n| n.0),
            undated_remaining_units: undated_remaining,
        };
    }
    if dated.is_empty() {
        return Workload {
            total_units,
            completed_units,
            remaining_units: remaining,
            progress,
            units_per_day: remaining,
            days_left: nearest.map(|n| n.1),
            nearest_due_date: nearest.map(|n| n.0),
            undated_remaining_units: undated_remaining,
        };
    }
    let units_per_day: f64 = dated
        .iter()
        .map(|task| {
            let remaining = f64::from(remaining_units(task));
            match task
                .due_date
                .as_ref()
                .and_then(|d| days_until(clock, today, d))
            {
                Some(due_in) if due_in <= 0 => remaining,
                Some(due_in) => remaining / due_in as f64,
                // Unparseable date: production's NaN would poison the whole pace; ignore the task
                // instead (input a date picker cannot produce - see docs "Production quirks").
                None => 0.0,
            }
        })
        .sum();
    Workload {
        total_units,
        completed_units,
        remaining_units: remaining,
        progress,
        units_per_day,
        days_left: nearest.map(|n| n.1),
        nearest_due_date: nearest.map(|n| n.0),
        undated_remaining_units: undated_remaining,
    }
}

/// `getExamPressure`.
pub fn exam_pressure(exams: &[&Exam], today: CivilDate, clock: &dyn LocalClock) -> f64 {
    exams.iter().fold(0.0, |penalty, exam| {
        let Some(due_in) = days_until(clock, today, &exam.exam_date) else {
            return penalty;
        };
        if !(0..=10).contains(&due_in) {
            return penalty;
        }
        let urgency = (10.0 - due_in as f64) / 10.0;
        penalty + urgency * (100.0 - exam.preparedness) * 0.22
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Health {
    pub score: u32,
    pub label: &'static str,
    pub overdue: u32,
    pub due_soon: u32,
    pub units_per_day: f64,
}

/// `getHealthLabel`.
pub fn health_label(score: u32) -> &'static str {
    if score < 35 {
        "Critical"
    } else if score < 55 {
        "Watch"
    } else if score < 75 {
        "Steady"
    } else {
        "Strong"
    }
}

/// `calculateWorkloadHealth`.
pub fn workload_health(
    tasks: &[&Task],
    exams: &[&Exam],
    today: CivilDate,
    clock: &dyn LocalClock,
) -> Health {
    let total_units: f64 = tasks.iter().map(|t| f64::from(t.total_units.max(1))).sum();
    let finished: f64 = tasks
        .iter()
        .map(|t| clamp(f64::from(t.completed_units), 0.0, f64::from(t.total_units)))
        .sum();
    if total_units == 0.0 {
        return Health {
            score: 0,
            label: "Preparing",
            overdue: 0,
            due_soon: 0,
            units_per_day: 0.0,
        };
    }
    let progress_percent = finished / total_units * 100.0;
    let units_per_day = calculate_aggregate_workload(tasks, today, clock).units_per_day;
    let manageability = 100.0 - clamp((units_per_day - 1.0) * 10.0, 0.0, 45.0);
    let due_in = |task: &Task| {
        task.due_date
            .as_ref()
            .and_then(|d| days_until(clock, today, d))
    };
    let overdue = tasks
        .iter()
        .filter(|t| due_in(t).is_some_and(|d| d < 0) && remaining_units(t) > 0)
        .count() as u32;
    let due_soon = tasks
        .iter()
        .filter(|t| remaining_units(t) > 0 && due_in(t).is_some_and(|d| (0..=3).contains(&d)))
        .count() as u32;
    let score = clamp(
        js_round(
            progress_percent * 0.65 + manageability * 0.35
                - f64::from(overdue) * 12.0
                - f64::from(due_soon) * 2.0
                - exam_pressure(exams, today, clock),
        ),
        0.0,
        100.0,
    ) as u32;
    Health {
        score,
        label: health_label(score),
        overdue,
        due_soon,
        units_per_day,
    }
}

/// A course's health record with its calendar-based override applied (`withScheduleHealth`).
pub fn with_schedule_health(
    base: Health,
    schedule: Option<super::schedule::ScheduleHealth>,
) -> Health {
    match schedule {
        Some(schedule) => Health {
            score: schedule.score,
            label: health_label(schedule.score),
            ..base
        },
        None => base,
    }
}

/// `getUpcomingExams` (every exam whose date is today or later, soonest first, at most 4).
/// Note: production does **not** restrict this to non-archived semesters.
pub fn upcoming_exams<'a>(
    exams: &'a [Exam],
    today: CivilDate,
    clock: &dyn LocalClock,
) -> Vec<(&'a Exam, i64)> {
    let mut with_days: Vec<(&Exam, i64)> = exams
        .iter()
        .filter_map(|exam| days_until(clock, today, &exam.exam_date).map(|d| (exam, d)))
        .filter(|(_, d)| *d >= 0)
        .collect();
    with_days.sort_by_key(|(_, d)| *d); // stable, like Array.prototype.sort
    with_days.truncate(4);
    with_days
}

/// Sessions paired with the local calendar date each one ended on (`sessionDateKey`), computed
/// once per Dashboard refresh so no metric rescans the history or re-runs timezone lookups.
pub struct SessionDays<'a> {
    pub sessions: &'a [StudySession],
    pub ended_day: Vec<CivilDate>,
}

impl<'a> SessionDays<'a> {
    pub fn new(sessions: &'a [StudySession], clock: &dyn LocalClock) -> Self {
        let ended_day = sessions
            .iter()
            .map(|s| clock.local_date(s.ended_at))
            .collect();
        Self {
            sessions,
            ended_day,
        }
    }

    /// `getTodayMinutes`: total minutes of every session (any kind) that ended on `day`.
    pub fn minutes_on(&self, day: CivilDate) -> u64 {
        self.sessions
            .iter()
            .zip(&self.ended_day)
            .filter(|(_, d)| **d == day)
            .map(|(s, _)| u64::from(s.minutes))
            .sum()
    }

    /// `getSessionDaySet`.
    pub fn day_set(&self) -> HashSet<CivilDate> {
        self.ended_day.iter().copied().collect()
    }

    /// `getStreakDays`: consecutive days with *any* session (study, break or exam; any length,
    /// including zero minutes; recovered/imported alike) ending today, or ending yesterday when
    /// today has none yet. Zero when neither today nor yesterday has a session.
    pub fn streak_days(&self, today: CivilDate) -> u32 {
        let days = self.day_set();
        let mut cursor = today;
        if !days.contains(&cursor) {
            cursor = cursor.add_days(-1);
        }
        let mut streak = 0;
        while days.contains(&cursor) {
            streak += 1;
            cursor = cursor.add_days(-1);
        }
        streak
    }

    /// `getWeeklyActivity`: the seven days ending `today` (a *rolling* window, not the calendar
    /// week), oldest first.
    pub fn weekly_activity(&self, today: CivilDate) -> [(CivilDate, u64); 7] {
        let mut buckets = [(today, 0u64); 7];
        for (index, bucket) in buckets.iter_mut().enumerate() {
            bucket.0 = today.add_days(index as i64 - 6);
        }
        for (session, day) in self.sessions.iter().zip(&self.ended_day) {
            if let Some(bucket) = buckets.iter_mut().find(|b| b.0 == *day) {
                bucket.1 += u64::from(session.minutes);
            }
        }
        buckets
    }

    /// `getFirstSessionDate`: the earliest `started_at` (the instant, formatted by the caller).
    pub fn first_started_at(&self) -> Option<WallTimestamp> {
        self.sessions.iter().map(|s| s.started_at).min()
    }

    /// `getFocusMomentum`.
    pub fn focus_momentum(&self, today: CivilDate) -> &'static str {
        let total: u64 = self.weekly_activity(today).iter().map(|b| b.1).sum();
        let average = total as f64 / 7.0;
        let streak = self.streak_days(today);
        if streak >= 5 && average >= 35.0 {
            "Surging"
        } else if streak >= 5 {
            "Stable"
        } else if streak >= 3 {
            "Recovering"
        } else if average >= 110.0 {
            "Surging"
        } else if average >= 70.0 {
            "Stable"
        } else if average >= 35.0 {
            "Recovering"
        } else {
            "Slipping"
        }
    }
}

/// `getCourseMinutesMap`: total minutes per course id (sessions with no course are skipped).
pub fn course_minutes(sessions: &[StudySession]) -> std::collections::HashMap<&str, u64> {
    let mut map = std::collections::HashMap::new();
    for session in sessions {
        if let Some(course) = &session.course_id {
            *map.entry(course.as_str()).or_insert(0) += u64::from(session.minutes);
        }
    }
    map
}

/// `inferUnitLabel` / `cleanUnitLabel` (App.tsx).
pub fn clean_unit_label(label: &str, title: &str) -> String {
    let trimmed = label.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    let t = title.to_lowercase();
    if t.contains("lecture") {
        "Lecture"
    } else if t.contains("sheet") || t.contains("exercise") {
        "Sheet"
    } else if t.contains("exam") {
        "Exam"
    } else if t.contains("chapter") {
        "Chapter"
    } else if t.contains("reading") {
        "Reading"
    } else {
        "Task"
    }
    .to_string()
}

pub fn course_color_fallback(course: Option<&Course>) -> Option<&str> {
    course.map(|c| c.color.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::academic::{
        CourseId, ExamId, SemesterId, SessionId, SessionKind, TaskId, TaskSubtype,
    };
    use crate::dashboard::civil::FixedOffsetClock;

    const UTC: FixedOffsetClock = FixedOffsetClock::UTC;

    fn d(s: &str) -> CivilDate {
        CivilDate::parse_iso(s).unwrap()
    }
    fn ld(s: &str) -> LocalDate {
        LocalDate::parse(s).unwrap()
    }
    fn ts(iso_day: &str, hour: u32, minute: u32) -> WallTimestamp {
        WallTimestamp::from_unix_millis(
            d(iso_day).days() * 86_400_000
                + i64::from(hour) * 3_600_000
                + i64::from(minute) * 60_000,
        )
    }
    fn session(id: &str, end_day: &str, end_hour: u32, minutes: u32) -> StudySession {
        let ended = ts(end_day, end_hour, 0);
        StudySession {
            id: SessionId::new(id),
            semester_id: None,
            course_id: Some(CourseId::new("c1")),
            task_id: None,
            kind: SessionKind::Study,
            goal: String::new(),
            learned: String::new(),
            blocker: String::new(),
            next_step: String::new(),
            confidence: 3,
            started_at: WallTimestamp::from_unix_millis(
                ended.unix_millis - i64::from(minutes) * 60_000,
            ),
            ended_at: ended,
            minutes,
            preset_label: String::new(),
        }
    }
    fn task(id: &str, total: u32, done: u32, due: Option<&str>) -> Task {
        let mut t = Task::new(
            TaskId::new(id),
            SemesterId::new("s"),
            CourseId::new("c1"),
            id.into(),
            TaskSubtype::Sheet,
            "Sheet".into(),
            WallTimestamp::from_unix_millis(0),
        );
        t.total_units = total;
        t.completed_units = done;
        t.due_date = due.map(ld);
        t
    }
    fn exam(id: &str, date: &str, prep: f64) -> Exam {
        let mut e = Exam::new(
            ExamId::new(id),
            SemesterId::new("s"),
            CourseId::new("c1"),
            id.into(),
            ld(date),
        );
        e.preparedness = prep;
        e
    }

    #[test]
    fn empty_history_has_no_streak_no_minutes_no_nan() {
        let sessions: Vec<StudySession> = vec![];
        let sd = SessionDays::new(&sessions, &UTC);
        let today = d("2026-09-30");
        assert_eq!(sd.streak_days(today), 0);
        assert_eq!(sd.minutes_on(today), 0);
        assert!(sd.weekly_activity(today).iter().all(|b| b.1 == 0));
        assert_eq!(sd.first_started_at(), None);
        assert_eq!(sd.focus_momentum(today), "Slipping");
    }

    #[test]
    fn streak_counts_today_or_yesterday_and_stops_at_the_first_gap() {
        let today = d("2026-09-30");
        // today + 3 earlier consecutive days, a gap, then an older day.
        let s = vec![
            session("a", "2026-09-30", 10, 30),
            session("b", "2026-09-29", 10, 30),
            session("c", "2026-09-28", 10, 30),
            session("d", "2026-09-27", 10, 30),
            session("e", "2026-09-24", 10, 30),
        ];
        assert_eq!(SessionDays::new(&s, &UTC).streak_days(today), 4);

        // Yesterday-only keeps the streak alive (today not required).
        let yesterday_run = vec![
            session("a", "2026-09-29", 10, 5),
            session("b", "2026-09-28", 10, 5),
        ];
        assert_eq!(SessionDays::new(&yesterday_run, &UTC).streak_days(today), 2);

        // Two days ago only: broken -> 0.
        let broken = vec![session("a", "2026-09-28", 10, 5)];
        assert_eq!(SessionDays::new(&broken, &UTC).streak_days(today), 0);

        // Today + a gap yesterday: streak is exactly 1.
        let gap = vec![
            session("a", "2026-09-30", 1, 5),
            session("b", "2026-09-28", 10, 5),
        ];
        assert_eq!(SessionDays::new(&gap, &UTC).streak_days(today), 1);
    }

    #[test]
    fn zero_minute_and_break_sessions_still_count_as_an_active_day() {
        let today = d("2026-09-30");
        let mut zero = session("z", "2026-09-30", 10, 0);
        zero.kind = SessionKind::Break;
        assert_eq!(SessionDays::new(&[zero], &UTC).streak_days(today), 1);
    }

    #[test]
    fn streak_crosses_month_year_and_leap_boundaries() {
        let mut s = Vec::new();
        for day in ["2028-03-02", "2028-03-01", "2028-02-29", "2028-02-28"] {
            s.push(session(day, day, 12, 10));
        }
        assert_eq!(
            SessionDays::new(&s, &UTC).streak_days(d("2028-03-02")),
            4,
            "leap day 2028-02-29 is a real day"
        );
        let mut y = Vec::new();
        for day in ["2027-01-02", "2027-01-01", "2026-12-31", "2026-12-30"] {
            y.push(session(day, day, 12, 10));
        }
        assert_eq!(
            SessionDays::new(&y, &UTC).streak_days(d("2027-01-02")),
            4,
            "year boundary"
        );
        let nonleap = vec![
            session("a", "2027-03-01", 12, 10),
            session("b", "2027-02-28", 12, 10),
        ];
        assert_eq!(
            SessionDays::new(&nonleap, &UTC).streak_days(d("2027-03-01")),
            2
        );
    }

    #[test]
    fn a_session_is_attributed_to_the_local_day_it_ends_on() {
        // 23:30Z on the 29th is already the 30th at UTC+2: the local day decides the bucket
        // (production: `isoDate(new Date(endedAt))`), so a midnight-crossing session ending after
        // local midnight belongs to the new day.
        let s = vec![session("late", "2026-09-29", 23, 50)];
        let plus2 = FixedOffsetClock::new(2 * 3600);
        assert_eq!(SessionDays::new(&s, &plus2).minutes_on(d("2026-09-30")), 50);
        assert_eq!(SessionDays::new(&s, &plus2).minutes_on(d("2026-09-29")), 0);
        assert_eq!(SessionDays::new(&s, &UTC).minutes_on(d("2026-09-29")), 50);
    }

    #[test]
    fn multiple_sessions_same_day_sum_and_seven_day_window_boundary() {
        let today = d("2026-09-30");
        let s = vec![
            session("a", "2026-09-30", 9, 25),
            session("b", "2026-09-30", 15, 35),
            session("edge_in", "2026-09-24", 12, 40), // today - 6: inside
            session("edge_out", "2026-09-23", 12, 99), // today - 7: outside
        ];
        let sd = SessionDays::new(&s, &UTC);
        assert_eq!(sd.minutes_on(today), 60);
        let week = sd.weekly_activity(today);
        assert_eq!(week[0].0, d("2026-09-24"));
        assert_eq!(week[6].0, today);
        assert_eq!(
            week.iter().map(|b| b.1).sum::<u64>(),
            25 + 35 + 40,
            "the 7th-previous day is excluded"
        );
    }

    #[test]
    fn health_of_an_empty_task_list_is_preparing_zero() {
        let h = workload_health(&[], &[], d("2026-09-30"), &UTC);
        assert_eq!((h.score, h.label), (0, "Preparing"));
    }

    #[test]
    fn aggregate_workload_matches_hand_computation() {
        let today = d("2026-09-30");
        let a = task("a", 6, 2, Some("2026-10-04")); // 4 left, due in 4 days -> 1.0/day
        let b = task("b", 4, 4, Some("2026-10-01")); // finished
        let c = task("c", 3, 0, None); // undated: 3 left, excluded from pace
        let w = calculate_aggregate_workload(&[&a, &b, &c], today, &UTC);
        assert_eq!(w.total_units, 13.0);
        assert_eq!(w.completed_units, 6.0);
        assert_eq!(w.remaining_units, 7.0);
        assert_eq!(w.progress, 46.0); // round(6/13*100) = 46.15
        assert_eq!(w.units_per_day, 1.0);
        assert_eq!(w.undated_remaining_units, 3);
        assert_eq!(w.nearest_due_date, Some(d("2026-10-04")));
        assert_eq!(w.days_left, Some(4));
    }

    #[test]
    fn overdue_and_due_now_tasks_count_all_remaining_units_today() {
        let today = d("2026-09-30");
        let overdue = task("o", 5, 1, Some("2026-09-25"));
        let w = calculate_aggregate_workload(&[&overdue], today, &UTC);
        assert_eq!(w.units_per_day, 4.0);
        assert_eq!(w.days_left, Some(-5));
        let h = workload_health(&[&overdue], &[], today, &UTC);
        assert_eq!(h.overdue, 1);
    }

    #[test]
    fn health_score_combines_progress_pace_overdue_and_exam_pressure() {
        let today = d("2026-09-30");
        // progress 50% -> 32.5; pace 1.0/day -> manageability 100 -> 35; nothing overdue.
        let t = task("t", 10, 5, Some("2026-10-05")); // 5 left, due in 5 -> 1.0/day; due in 5 (> 3) not "due soon"
        let h = workload_health(&[&t], &[], today, &UTC);
        assert_eq!(h.score, 68); // round(32.5 + 35) = round(67.5) = 68 (JS rounds halves up)
                                 // An exam in 5 days with 40% prepared: urgency 0.5 * 60 * 0.22 = 6.6 -> 67.5 - 6.6 = 60.9 -> 61
        let e = exam("e", "2026-10-05", 40.0);
        assert_eq!(workload_health(&[&t], &[&e], today, &UTC).score, 61);
        // past / far-future exams exert no pressure
        assert_eq!(
            exam_pressure(
                &[&exam("p", "2026-09-29", 0.0), &exam("f", "2026-10-11", 0.0)],
                today,
                &UTC
            ),
            0.0
        );
        assert!(
            exam_pressure(&[&exam("today", "2026-09-30", 0.0)], today, &UTC) > 21.0,
            "an exam today at 0% prepared: 1.0 * 100 * 0.22 = 22"
        );
        assert_eq!(health_label(75), "Strong");
        assert_eq!(health_label(74), "Steady");
        assert_eq!(health_label(54), "Watch");
        assert_eq!(health_label(34), "Critical");
    }

    #[test]
    fn upcoming_exams_are_soonest_first_capped_at_four_and_exclude_the_past() {
        let today = d("2026-09-30");
        let exams = vec![
            exam("past", "2026-09-29", 0.0),
            exam("c", "2026-10-20", 0.0),
            exam("a", "2026-10-02", 0.0),
            exam("today", "2026-09-30", 0.0),
            exam("b", "2026-10-09", 0.0),
            exam("d", "2026-11-01", 0.0),
        ];
        let up: Vec<&str> = upcoming_exams(&exams, today, &UTC)
            .iter()
            .map(|(e, _)| e.title.as_str())
            .collect();
        assert_eq!(up, ["today", "a", "b", "c"]);
    }

    #[test]
    fn days_until_reproduces_the_date_only_quirk_west_of_utc() {
        let today = d("2026-09-30");
        let date = ld("2026-10-09");
        assert_eq!(days_until(&UTC, today, &date), Some(9));
        assert_eq!(
            days_until(&FixedOffsetClock::new(2 * 3600), today, &date),
            Some(9),
            "Zurich"
        );
        assert_eq!(
            days_until(&FixedOffsetClock::new(-7 * 3600), today, &date),
            Some(8),
            "production shows one day less west of UTC"
        );
        assert_eq!(days_until(&UTC, today, &ld("2026-13-01")), None);
    }

    #[test]
    fn momentum_thresholds() {
        let today = d("2026-09-30");
        // average 35/day (245 min over 7 days) but only a 1-day streak -> Recovering.
        let s = vec![session("a", "2026-09-30", 10, 245)];
        assert_eq!(
            SessionDays::new(&s, &UTC).focus_momentum(today),
            "Recovering"
        );
        let surge: Vec<StudySession> = (0..5)
            .map(|i| {
                session(
                    &format!("s{i}"),
                    &d("2026-09-30").add_days(-i).to_iso(),
                    10,
                    40,
                )
            })
            .collect();
        assert_eq!(
            SessionDays::new(&surge, &UTC).focus_momentum(today),
            "Stable",
            "streak 5 but 5 * 40 / 7 = 28.6 < 35 average -> Stable, not Surging"
        );
    }

    #[test]
    fn unit_label_inference_matches_production() {
        assert_eq!(clean_unit_label("", "Lecture notes"), "Lecture");
        assert_eq!(clean_unit_label("", "Exercise 3"), "Sheet");
        assert_eq!(clean_unit_label("  Chapter ", "x"), "Chapter");
        assert_eq!(clean_unit_label("", "misc"), "Task");
    }
}
