import type { Holiday, Semester, Task, TimetableEvent } from "../types";
import { calculateAggregateWorkload } from "./metrics";
import { expandTimetableEvents } from "./plannerSchedule";

export type ScheduledUnit = { date: string; done: boolean };
export type ScheduledUnitsByTask = Map<string, ScheduledUnit[]>;

const WEEK = 7;

function addDaysIso(iso: string, days: number) {
  const date = new Date(`${iso}T00:00:00`);
  date.setDate(date.getDate() + days);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

function daysBetween(fromIso: string, toIso: string) {
  return Math.round((new Date(`${toIso}T00:00:00`).getTime() - new Date(`${fromIso}T00:00:00`).getTime()) / 86400000);
}

/**
 * Every unit of work the calendar asks for, per task: each lecture / session / sheet due date
 * (a sheet's release is informational and never counts), with whether it has been ticked off.
 * The calendar is where these tasks' dates live - the tasks themselves carry no due date - so the
 * workload has to be read from here rather than from Task.dueDate.
 */
export function getScheduledUnits(events: TimetableEvent[], holidays: Holiday[], semesters: Semester[], today: string): ScheduledUnitsByTask {
  const counted = events.filter((event) => event.kind === "occurrence" || event.kind === "sheet-deadline");
  const map: ScheduledUnitsByTask = new Map();
  if (!counted.length) return map;
  const horizon = addDaysIso(today, 400);
  for (const semester of semesters) {
    for (const occurrence of expandTimetableEvents(counted, holidays, semester, "1970-01-01", horizon)) {
      const list = map.get(occurrence.event.taskId) ?? [];
      list.push({ date: occurrence.date, done: occurrence.event.completedOccurrences.includes(occurrence.date) });
      map.set(occurrence.event.taskId, list);
    }
  }
  return map;
}

/** Pace for the coming week: everything overdue plus everything due in the next 7 days, per day. */
function weeklyPace(units: ScheduledUnit[], today: string) {
  const weekEnd = addDaysIso(today, WEEK - 1);
  const backlog = units.filter((unit) => !unit.done && unit.date < today).length;
  const thisWeek = units.filter((unit) => !unit.done && unit.date >= today && unit.date <= weekEnd).length;
  return { backlog, thisWeek, perDay: (backlog + thisWeek) / WEEK };
}

const plural = (count: number, word: string) => `${count} ${word}${count === 1 ? "" : "s"}`;

/**
 * Workload across tasks, read from the calendar. Tasks with scheduled units use their real
 * schedule; a task with no schedule falls back to its own due date if it has one, and is otherwise
 * left out of the pace (rather than being treated as due today).
 */
export function calculateScheduledWorkload(tasks: Task[], scheduled: ScheduledUnitsByTask, today: string) {
  const scheduledTasks = tasks.filter((task) => (scheduled.get(task.id)?.length ?? 0) > 0);
  const other = tasks.filter((task) => !(scheduled.get(task.id)?.length));
  const units = scheduledTasks.flatMap((task) => scheduled.get(task.id) ?? []);

  const legacy = calculateAggregateWorkload(other.filter((task) => task.dueDate));
  const undatedRemainingUnits = other.filter((task) => !task.dueDate).reduce((sum, task) => sum + Math.max(0, task.totalUnits - task.completedUnits), 0);

  if (!units.length) {
    return { ...legacy, undatedRemainingUnits: undatedRemainingUnits + legacy.undatedRemainingUnits };
  }

  const done = units.filter((unit) => unit.done).length;
  const remaining = units.length - done;
  const pace = weeklyPace(units, today);
  const nextDate = units.filter((unit) => !unit.done).map((unit) => (unit.date < today ? today : unit.date)).sort()[0] ?? null;
  const totalUnits = units.length + legacy.totalUnits;
  const completedUnits = done + legacy.completedUnits;
  const unitsPerDay = pace.perDay + legacy.unitsPerDay;

  let message: string;
  if (remaining + legacy.remainingUnits <= 0) message = "Everything scheduled is done. Use the timer for revision or new work.";
  else if (pace.backlog + pace.thisWeek === 0 && !legacy.unitsPerDay) message = `Nothing due in the next ${WEEK} days. ${plural(remaining, "unit")} still scheduled later.`;
  else message = `${unitsPerDay.toFixed(1)} units/day this week: ${plural(pace.thisWeek, "unit")} due in the next ${WEEK} days${pace.backlog ? `, plus ${pace.backlog} overdue` : ""}.`;

  return {
    totalUnits,
    completedUnits,
    remainingUnits: remaining + legacy.remainingUnits,
    progress: totalUnits ? Math.round((completedUnits / totalUnits) * 100) : 0,
    unitsPerDay,
    daysLeft: nextDate ? Math.max(0, daysBetween(today, nextDate)) : legacy.daysLeft,
    nearestDueDate: nextDate ?? legacy.nearestDueDate,
    undatedRemainingUnits: undatedRemainingUnits + legacy.undatedRemainingUnits,
    message,
  };
}

/** Per-task version of the same idea; null when the task has nothing scheduled (use its own due date). */
export function calculateScheduledDailyWork(task: Task, units: ScheduledUnit[] | undefined, today: string) {
  if (!units?.length) return null;
  const remaining = units.filter((unit) => !unit.done);
  if (!remaining.length) return { unitsPerDay: 0, daysLeft: 0 as number | null, message: "Everything scheduled is done. Keep this as revision only." };
  const pace = weeklyPace(units, today);
  const next = remaining.map((unit) => (unit.date < today ? today : unit.date)).sort()[0];
  const label = task.unitLabel.trim().toLowerCase() || "unit";
  const message = pace.backlog + pace.thisWeek === 0
    ? `Next ${label} is in ${daysBetween(today, next)} days - nothing due this week.`
    : `${plural(pace.thisWeek, label)} due in the next ${WEEK} days${pace.backlog ? `, plus ${pace.backlog} overdue` : ""}.`;
  return { unitsPerDay: pace.perDay, daysLeft: Math.max(0, daysBetween(today, next)) as number | null, message };
}
