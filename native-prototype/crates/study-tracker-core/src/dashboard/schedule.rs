//! Calendar/timetable projection used by the Dashboard's workload and health numbers. Ported from
//! production's `plannerSchedule.ts` (`expandWeekdayFrom`, `expandTimetableEvents`),
//! `scheduleWorkload.ts` (`getScheduledUnits`, `calculateScheduledWorkload`) and
//! `scheduleHealth.ts` (`getScheduleHealth`). Only the parts the Dashboard reads are ported; the
//! Planner-only helpers stay in production until the Planner migrates.

use std::collections::HashMap;

use super::civil::CivilDate;
use super::metrics::{calculate_aggregate_workload, exam_pressure, Workload};
use crate::academic::{
    Course, Exam, Holiday, Semester, SemesterPhase, Task, TaskId, TimetableEvent,
    TimetableEventKind,
};

/// Only occurrences and sheet deadlines count as a unit of work; a sheet's release is
/// informational (`isCountedKind` / `counted` in production).
fn is_counted(event: &TimetableEvent) -> bool {
    matches!(
        event.kind,
        TimetableEventKind::Occurrence | TimetableEventKind::SheetDeadline
    )
}

fn is_semester_schedule_active(semester: &Semester) -> bool {
    !semester.archived && semester.phase == SemesterPhase::Semester
}

fn is_holiday(holidays: &[Holiday], semester: &Semester, date: CivilDate) -> bool {
    holidays.iter().any(|holiday| {
        holiday.semester_id == semester.id
            && match (
                CivilDate::from_local_date(&holiday.start_date),
                CivilDate::from_local_date(&holiday.end_date),
            ) {
                (Some(start), Some(end)) => date >= start && date <= end,
                _ => false,
            }
    })
}

/// `expandWeekdayFrom`: every date from the later of `anchor`/`range_start` up to `range_end` that
/// shares the anchor's weekday.
pub fn expand_weekday_from(
    anchor: CivilDate,
    range_start: CivilDate,
    range_end: CivilDate,
) -> Vec<CivilDate> {
    if range_start > range_end {
        return Vec::new();
    }
    let weekday = anchor.weekday() as i64;
    let start = anchor.max(range_start);
    let offset = (weekday - start.weekday() as i64 + 7) % 7;
    let mut cursor = start.add_days(offset);
    let mut dates = Vec::new();
    while cursor <= range_end {
        dates.push(cursor);
        cursor = cursor.add_days(7);
    }
    dates
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Occurrence<'a> {
    pub event: &'a TimetableEvent,
    pub date: CivilDate,
}

/// `expandTimetableEvents` for one semester, honoring skip/move overrides and holidays. Returns
/// nothing for an archived or exam-prep semester (the schedule only runs while `phase ==
/// "semester"`).
pub fn expand_timetable_events<'a>(
    events: &'a [&'a TimetableEvent],
    holidays: &[Holiday],
    semester: &Semester,
    range_start: CivilDate,
    range_end: CivilDate,
) -> Vec<Occurrence<'a>> {
    if !is_semester_schedule_active(semester) {
        return Vec::new();
    }
    let semester_end = semester
        .end_date
        .as_ref()
        .and_then(CivilDate::from_local_date);
    let semester_start = semester
        .start_date
        .as_ref()
        .and_then(CivilDate::from_local_date);
    let effective_end = semester_end.map_or(range_end, |end| end.min(range_end));
    let effective_start = semester_start.map_or(range_start, |start| start.max(range_start));
    if effective_start > effective_end {
        return Vec::new();
    }

    let mut occurrences = Vec::new();
    for event in events.iter().copied() {
        if event.semester_id != semester.id {
            continue;
        }
        let Some(event_date) = CivilDate::from_local_date(&event.date) else {
            continue;
        };
        let series_end = event
            .recurrence_end_date
            .as_ref()
            .and_then(CivilDate::from_local_date)
            .map_or(effective_end, |end| end.min(effective_end));
        if event.repeat_weekly {
            if effective_start <= series_end {
                for date in expand_weekday_from(event_date, effective_start, series_end) {
                    let key = date.to_iso();
                    let override_entry = event.occurrence_overrides.get(&key);
                    if override_entry.is_some_and(|o| o.skipped) {
                        continue;
                    }
                    if let Some(moved) = override_entry.and_then(|o| o.date.as_ref()) {
                        if let Some(moved) = CivilDate::from_local_date(moved) {
                            if moved >= effective_start
                                && moved <= effective_end
                                && !is_holiday(holidays, semester, moved)
                            {
                                occurrences.push(Occurrence { event, date: moved });
                            }
                        }
                        continue;
                    }
                    if is_holiday(holidays, semester, date) {
                        continue;
                    }
                    occurrences.push(Occurrence { event, date });
                }
            }
        } else if event_date >= effective_start && event_date <= effective_end {
            occurrences.push(Occurrence {
                event,
                date: event_date,
            });
        }
    }
    occurrences
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledUnit {
    pub date: CivilDate,
    pub done: bool,
}

pub type ScheduledUnitsByTask = HashMap<TaskId, Vec<ScheduledUnit>>;

const WEEK: i64 = 7;

/// `getScheduledUnits`: every unit of work the calendar asks for, per task, over `[1970-01-01,
/// today + 400 days]` for each (non-archived) semester passed in.
pub fn scheduled_units(
    events: &[TimetableEvent],
    holidays: &[Holiday],
    semesters: &[&Semester],
    today: CivilDate,
) -> ScheduledUnitsByTask {
    let counted: Vec<&TimetableEvent> = events.iter().filter(|e| is_counted(e)).collect();
    let mut map = ScheduledUnitsByTask::new();
    if counted.is_empty() {
        return map;
    }
    let horizon = today.add_days(400);
    let epoch = CivilDate::from_days(0);
    for semester in semesters {
        for occurrence in expand_timetable_events(&counted, holidays, semester, epoch, horizon) {
            let done = occurrence
                .event
                .completed_occurrences
                .iter()
                .any(|d| *d == occurrence.date.to_iso());
            map.entry(occurrence.event.task_id.clone())
                .or_default()
                .push(ScheduledUnit {
                    date: occurrence.date,
                    done,
                });
        }
    }
    map
}

struct WeeklyPace {
    backlog: u32,
    this_week: u32,
    per_day: f64,
}

fn weekly_pace(units: &[ScheduledUnit], today: CivilDate) -> WeeklyPace {
    let week_end = today.add_days(WEEK - 1);
    let backlog = units.iter().filter(|u| !u.done && u.date < today).count() as u32;
    let this_week = units
        .iter()
        .filter(|u| !u.done && u.date >= today && u.date <= week_end)
        .count() as u32;
    WeeklyPace {
        backlog,
        this_week,
        per_day: f64::from(backlog + this_week) / WEEK as f64,
    }
}

/// `calculateScheduledWorkload`: tasks with scheduled units use their real schedule; an
/// unscheduled task falls back to its own due date if it has one and is otherwise left out of the
/// pace (not treated as due today).
pub fn calculate_scheduled_workload(
    tasks: &[&Task],
    scheduled: &ScheduledUnitsByTask,
    today: CivilDate,
    clock: &dyn super::civil::LocalClock,
) -> Workload {
    let has_units = |task: &&Task| scheduled.get(&task.id).is_some_and(|u| !u.is_empty());
    let scheduled_tasks: Vec<&Task> = tasks.iter().filter(|t| has_units(t)).copied().collect();
    let other: Vec<&Task> = tasks.iter().filter(|t| !has_units(t)).copied().collect();
    let units: Vec<ScheduledUnit> = scheduled_tasks
        .iter()
        .flat_map(|t| scheduled.get(&t.id).cloned().unwrap_or_default())
        .collect();

    let dated_other: Vec<&Task> = other
        .iter()
        .filter(|t| t.due_date.is_some())
        .copied()
        .collect();
    let legacy = calculate_aggregate_workload(&dated_other, today, clock);
    let undated_remaining: u32 = other
        .iter()
        .filter(|t| t.due_date.is_none())
        .map(|t| t.total_units.saturating_sub(t.completed_units))
        .sum();

    if units.is_empty() {
        return Workload {
            undated_remaining_units: undated_remaining + legacy.undated_remaining_units,
            ..legacy
        };
    }

    let done = units.iter().filter(|u| u.done).count() as f64;
    let remaining = units.len() as f64 - done;
    let pace = weekly_pace(&units, today);
    let next_date = units
        .iter()
        .filter(|u| !u.done)
        .map(|u| if u.date < today { today } else { u.date })
        .min();
    let total_units = units.len() as f64 + legacy.total_units;
    let completed_units = done + legacy.completed_units;
    let units_per_day = pace.per_day + legacy.units_per_day;
    let _ = (pace.backlog, pace.this_week); // used only by the (Planner-only) message text in production

    Workload {
        total_units,
        completed_units,
        remaining_units: remaining + legacy.remaining_units,
        progress: if total_units > 0.0 {
            super::format::js_round(completed_units / total_units * 100.0)
        } else {
            0.0
        },
        units_per_day,
        days_left: next_date
            .map(|d| today.days_until(d).max(0))
            .or(legacy.days_left),
        nearest_due_date: next_date.or(legacy.nearest_due_date),
        undated_remaining_units: undated_remaining + legacy.undated_remaining_units,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduleHealth {
    pub score: u32,
    pub missed: u32,
    pub on_time: u32,
    pub due_soon: u32,
}

/// `getScheduleHealth`: health measured against the calendar (what should already be done versus
/// what is), `None` when nothing is scheduled so callers fall back to the completion-based score.
pub fn schedule_health(
    events: &[&TimetableEvent],
    holidays: &[Holiday],
    semesters: &[&Semester],
    exams: &[&Exam],
    today: CivilDate,
    clock: &dyn super::civil::LocalClock,
) -> Option<ScheduleHealth> {
    let counted: Vec<&TimetableEvent> = events.iter().filter(|e| is_counted(e)).copied().collect();
    if counted.is_empty() {
        return None;
    }
    let yesterday = today.add_days(-1);
    let soon_end = today.add_days(3);
    let epoch = CivilDate::from_days(0);
    let (mut missed, mut on_time, mut due_soon) = (0u32, 0u32, 0u32);
    let mut scheduled_at_all = false;
    for semester in semesters {
        for occurrence in expand_timetable_events(&counted, holidays, semester, epoch, soon_end) {
            scheduled_at_all = true;
            let done = occurrence
                .event
                .completed_occurrences
                .iter()
                .any(|d| *d == occurrence.date.to_iso());
            if occurrence.date <= yesterday {
                if done {
                    on_time += 1;
                } else {
                    missed += 1;
                }
            } else if occurrence.event.kind == TimetableEventKind::SheetDeadline && !done {
                due_soon += 1;
            }
        }
    }
    if !scheduled_at_all {
        return None;
    }
    let past = missed + on_time;
    let missed_ratio = if past > 0 {
        f64::from(missed) / f64::from(past)
    } else {
        0.0
    };
    let raw = 100.0
        - missed_ratio * 70.0
        - f64::from(missed) * 4.0
        - f64::from(due_soon) * 2.0
        - exam_pressure(exams, today, clock);
    Some(ScheduleHealth {
        score: super::format::js_round(raw).clamp(0.0, 100.0) as u32,
        missed,
        on_time,
        due_soon,
    })
}

/// A course's own timetable events, for `withScheduleHealth`.
pub fn events_for_course<'a>(
    events: &'a [TimetableEvent],
    course: &Course,
) -> Vec<&'a TimetableEvent> {
    events.iter().filter(|e| e.course_id == course.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::academic::{
        CourseId, Holiday, HolidayId, OccurrenceOverride, SemesterId, TimetableEventId,
    };
    use crate::dashboard::civil::FixedOffsetClock;
    use crate::timer::WallTimestamp;
    use std::collections::BTreeMap;

    fn d(s: &str) -> CivilDate {
        CivilDate::parse_iso(s).unwrap()
    }
    fn ld(s: &str) -> crate::academic::LocalDate {
        crate::academic::LocalDate::parse(s).unwrap()
    }

    fn semester(id: &str, start: &str, end: &str) -> Semester {
        let mut s = Semester::new(
            SemesterId::new(id),
            id.into(),
            WallTimestamp::from_unix_millis(0),
        );
        s.start_date = Some(ld(start));
        s.end_date = Some(ld(end));
        s
    }

    fn event(
        id: &str,
        sem: &str,
        task: &str,
        kind: TimetableEventKind,
        date: &str,
        weekly: bool,
    ) -> TimetableEvent {
        TimetableEvent {
            id: TimetableEventId::new(id),
            semester_id: SemesterId::new(sem),
            course_id: CourseId::new("c"),
            kind,
            task_id: TaskId::new(task),
            label: String::new(),
            date: ld(date),
            time: "09:00".into(),
            end_time: None,
            repeat_weekly: weekly,
            recurrence_end_date: None,
            occurrence_overrides: BTreeMap::new(),
            url: None,
            completed_occurrences: vec![],
            created_at: WallTimestamp::from_unix_millis(0),
        }
    }

    #[test]
    fn weekday_expansion_starts_at_the_anchor_and_steps_weekly() {
        // 2026-09-28 is a Monday.
        let dates = expand_weekday_from(d("2026-09-28"), d("2026-09-01"), d("2026-10-20"));
        let iso: Vec<String> = dates.iter().map(|x| x.to_iso()).collect();
        assert_eq!(
            iso,
            ["2026-09-28", "2026-10-05", "2026-10-12", "2026-10-19"]
        );
        // Range start after the anchor: next matching weekday on/after it.
        let later: Vec<String> =
            expand_weekday_from(d("2026-09-28"), d("2026-10-06"), d("2026-10-20"))
                .iter()
                .map(|x| x.to_iso())
                .collect();
        assert_eq!(later, ["2026-10-12", "2026-10-19"]);
        assert!(expand_weekday_from(d("2026-09-28"), d("2026-11-01"), d("2026-10-01")).is_empty());
    }

    #[test]
    fn overrides_holidays_and_inactive_semesters() {
        let sem = semester("s", "2026-09-01", "2026-12-20");
        let mut ev = event(
            "e",
            "s",
            "t",
            TimetableEventKind::Occurrence,
            "2026-09-28",
            true,
        );
        ev.occurrence_overrides.insert(
            "2026-10-05".into(),
            OccurrenceOverride {
                skipped: true,
                ..Default::default()
            },
        );
        ev.occurrence_overrides.insert(
            "2026-10-12".into(),
            OccurrenceOverride {
                skipped: false,
                date: Some(ld("2026-10-14")),
                time: None,
                end_time: None,
            },
        );
        let holidays = vec![Holiday {
            id: HolidayId::new("h"),
            semester_id: SemesterId::new("s"),
            start_date: ld("2026-10-19"),
            end_date: ld("2026-10-25"),
            label: String::new(),
            created_at: WallTimestamp::from_unix_millis(0),
        }];
        let refs = [&ev];
        let occ = expand_timetable_events(&refs, &holidays, &sem, d("2026-09-01"), d("2026-11-02"));
        let iso: Vec<String> = occ.iter().map(|o| o.date.to_iso()).collect();
        assert_eq!(
            iso,
            ["2026-09-28", "2026-10-14", "2026-10-26", "2026-11-02"],
            "10-05 skipped, 10-12 moved to 10-14, 10-19 is inside the 19..25 holiday, 10-26 is not"
        );

        let mut archived = sem.clone();
        archived.archived = true;
        assert!(expand_timetable_events(
            &refs,
            &holidays,
            &archived,
            d("2026-09-01"),
            d("2026-11-02")
        )
        .is_empty());
        let mut prep = sem.clone();
        prep.phase = SemesterPhase::ExamPrep;
        assert!(
            expand_timetable_events(&refs, &holidays, &prep, d("2026-09-01"), d("2026-11-02"))
                .is_empty(),
            "exam-prep semesters have no running schedule"
        );
    }

    #[test]
    fn one_off_events_respect_range_and_semester_bounds() {
        let sem = semester("s", "2026-09-01", "2026-12-20");
        let inside = event(
            "a",
            "s",
            "t",
            TimetableEventKind::SheetDeadline,
            "2026-10-01",
            false,
        );
        let outside = event(
            "b",
            "s",
            "t",
            TimetableEventKind::SheetDeadline,
            "2027-01-10",
            false,
        );
        let refs = [&inside, &outside];
        let occ = expand_timetable_events(&refs, &[], &sem, d("1970-01-01"), d("2030-01-01"));
        assert_eq!(occ.len(), 1);
    }

    #[test]
    fn schedule_health_is_none_without_counted_events_and_penalises_misses() {
        let clock = FixedOffsetClock::UTC;
        let sem = semester("s", "2026-09-01", "2026-12-20");
        let release = event(
            "r",
            "s",
            "t",
            TimetableEventKind::SheetRelease,
            "2026-09-10",
            false,
        );
        assert!(
            schedule_health(&[&release], &[], &[&sem], &[], d("2026-09-30"), &clock).is_none(),
            "a release is not a counted unit"
        );
        assert!(schedule_health(&[], &[], &[&sem], &[], d("2026-09-30"), &clock).is_none());

        let mut done = event(
            "d",
            "s",
            "t",
            TimetableEventKind::SheetDeadline,
            "2026-09-10",
            false,
        );
        done.completed_occurrences.push("2026-09-10".into());
        let missed = event(
            "m",
            "s",
            "t",
            TimetableEventKind::SheetDeadline,
            "2026-09-20",
            false,
        );
        let soon = event(
            "n",
            "s",
            "t",
            TimetableEventKind::SheetDeadline,
            "2026-10-02",
            false,
        );
        let h = schedule_health(
            &[&done, &missed, &soon],
            &[],
            &[&sem],
            &[],
            d("2026-09-30"),
            &clock,
        )
        .unwrap();
        assert_eq!((h.on_time, h.missed, h.due_soon), (1, 1, 1));
        // 100 - 0.5*70 - 1*4 - 1*2 - 0 = 59
        assert_eq!(h.score, 59);
    }

    #[test]
    fn scheduled_workload_uses_calendar_units_and_weekly_pace() {
        let clock = FixedOffsetClock::UTC;
        let sem = semester("s", "2026-09-01", "2026-12-20");
        // Weekly Monday lecture from 09-28 through the semester.
        let ev = event(
            "e",
            "s",
            "t1",
            TimetableEventKind::Occurrence,
            "2026-09-28",
            true,
        );
        let units = scheduled_units(&[ev], &[], &[&sem], d("2026-09-30"));
        assert_eq!(
            units.get(&TaskId::new("t1")).unwrap().len(),
            12,
            "09-28 .. 12-14 weekly"
        );
        let mut task = Task::new(
            TaskId::new("t1"),
            SemesterId::new("s"),
            CourseId::new("c"),
            "L".into(),
            crate::academic::TaskSubtype::Lecture,
            "Lecture".into(),
            WallTimestamp::from_unix_millis(0),
        );
        task.total_units = 12;
        let w = calculate_scheduled_workload(&[&task], &units, d("2026-09-30"), &clock);
        // today is Wed 09-30: the past Monday (09-28) is backlog (1) + next Monday 10-05 is in the
        // 7-day window (09-30..10-06) -> (1 + 1) / 7.
        assert!((w.units_per_day - 2.0 / 7.0).abs() < 1e-12);
        assert_eq!(w.total_units, 12.0);
    }
}
