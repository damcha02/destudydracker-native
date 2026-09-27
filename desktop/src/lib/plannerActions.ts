import type { AppState, DailyTodo, TimetableEvent } from "../types";
import { parseIsoDate, toIsoDate } from "./plannerSchedule";

const undoableListKeys = ["semesters", "courses", "tasks", "exams", "calendarEntries", "timetableEvents", "holidays", "dailyTodos"] as const;
type UndoableListKey = (typeof undoableListKeys)[number];
type Identified = { id: string };
type TimerRefs = { semesterId: string | null; courseId: string | null; taskId: string | null };

export interface UndoPatch {
  removed: { key: UndoableListKey; item: Identified; index: number }[];
  modified: { key: UndoableListKey; item: Identified }[];
  timer: { before: TimerRefs; after: TimerRefs } | null;
}

function timerRefs(state: AppState): TimerRefs {
  return { semesterId: state.timer.semesterId, courseId: state.timer.courseId, taskId: state.timer.taskId };
}

/**
 * Records only what a delete actually changed - removed items (with their position), items the
 * delete modified (e.g. a task's completed-units count), and timer references it cleared - so
 * undoing it can't roll back unrelated state such as a study session finished in the meantime.
 */
export function diffForUndo(before: AppState, after: AppState): UndoPatch {
  const patch: UndoPatch = { removed: [], modified: [], timer: null };
  for (const key of undoableListKeys) {
    const beforeList = before[key] as Identified[];
    const afterById = new Map((after[key] as Identified[]).map((item) => [item.id, item]));
    beforeList.forEach((item, index) => {
      const next = afterById.get(item.id);
      if (!next) patch.removed.push({ key, item, index });
      else if (next !== item) patch.modified.push({ key, item });
    });
  }
  const beforeRefs = timerRefs(before);
  const afterRefs = timerRefs(after);
  if (JSON.stringify(beforeRefs) !== JSON.stringify(afterRefs)) patch.timer = { before: beforeRefs, after: afterRefs };
  return patch;
}

export function isEmptyUndoPatch(patch: UndoPatch): boolean {
  return !patch.removed.length && !patch.modified.length && !patch.timer;
}

export function applyUndoPatch(current: AppState, patch: UndoPatch): AppState {
  const next: AppState = { ...current };
  const writable = next as unknown as Record<UndoableListKey, Identified[]>;
  for (const key of undoableListKeys) {
    const modifiedById = new Map(patch.modified.filter((entry) => entry.key === key).map((entry) => [entry.item.id, entry.item]));
    let list = (current[key] as Identified[]).map((item) => modifiedById.get(item.id) ?? item);
    const restored = patch.removed.filter((entry) => entry.key === key).sort((a, b) => a.index - b.index);
    for (const entry of restored) {
      if (list.some((item) => item.id === entry.item.id)) continue;
      list = [...list.slice(0, entry.index), entry.item, ...list.slice(entry.index)];
    }
    writable[key] = list;
  }
  if (patch.timer) {
    const now = timerRefs(current);
    const cleared = (Object.keys(patch.timer.after) as (keyof TimerRefs)[]).every((field) => now[field] === patch.timer!.after[field]);
    if (cleared) next.timer = { ...current.timer, ...patch.timer.before };
  }
  return next;
}

/** Completed occurrences that count toward a task's completed units (sheet releases never do). */
export function countCompletedUnitOccurrences(events: TimetableEvent[], taskId: string): number {
  return events.reduce((sum, event) => (event.taskId === taskId && event.kind !== "sheet-release" ? sum + event.completedOccurrences.length : sum), 0);
}

/**
 * How much a task's completed-units count should drop when `removedCount` completed occurrences
 * are unchecked/removed. Occurrences beyond `totalUnits` were never counted (the count is capped),
 * so they must not be subtracted either or the count drifts below the real number.
 */
export function unitDecrementFor(completedBefore: number, totalUnits: number, removedCount: number): number {
  const uncounted = Math.max(0, completedBefore - totalUnits);
  return Math.max(0, removedCount - uncounted);
}

/** Moving completion state between a to-do's two storage shapes when "Repeat weekly" is toggled. */
export function convertTodoRepeat(todo: DailyTodo, repeatWeekly: boolean): DailyTodo {
  if (todo.repeatWeekly === repeatWeekly) return todo;
  if (repeatWeekly) {
    return {
      ...todo,
      repeatWeekly: true,
      completedOccurrences: todo.completed ? [todo.date] : todo.completedOccurrences,
    };
  }
  const doneOnAnchor = todo.completedOccurrences.includes(todo.date);
  return {
    ...todo,
    repeatWeekly: false,
    completed: doneOnAnchor,
    completedAt: doneOnAnchor ? todo.completedAt ?? new Date().toISOString() : null,
    completedOccurrences: [],
    recurrenceEndDate: null,
    skippedOccurrences: [],
    occurrenceTimes: {},
  };
}

/** Non-repeating, unfinished to-dos dated before `todayIso`. */
export function getOverdueTodos(todos: DailyTodo[], todayIso: string): DailyTodo[] {
  return todos.filter((todo) => !todo.repeatWeekly && !todo.completed && todo.date && todo.date < todayIso);
}

/** Moves the time of one occurrence of a repeating to-do, leaving the rest of the series alone. */
export function setTodoOccurrenceTime(todo: DailyTodo, occurrenceDate: string, time: string | null, endTime: string | null): DailyTodo {
  return { ...todo, occurrenceTimes: { ...todo.occurrenceTimes, [occurrenceDate]: { time, endTime } } };
}

/**
 * Applies a new time to a repeating to-do from `occurrenceDate` onward. On the series' first date
 * that's just an in-place update; otherwise the series is cut the day before and a sibling to-do
 * carries on from `occurrenceDate` (with its own share of completions, skips and time overrides).
 */
export function splitRecurringTodoAt(
  todo: DailyTodo,
  occurrenceDate: string,
  time: string | null,
  endTime: string | null,
  makeId: () => string,
): DailyTodo[] {
  if (occurrenceDate <= todo.date) {
    return [{ ...todo, time, endTime, occurrenceTimes: {} }];
  }
  const dayBefore = new Date(parseIsoDate(occurrenceDate));
  dayBefore.setDate(dayBefore.getDate() - 1);
  const fromHere = (dates: string[]) => dates.filter((date) => date >= occurrenceDate);
  const beforeHere = (dates: string[]) => dates.filter((date) => date < occurrenceDate);
  const timesFrom = Object.fromEntries(Object.entries(todo.occurrenceTimes).filter(([date]) => date < occurrenceDate));
  const original: DailyTodo = {
    ...todo,
    recurrenceEndDate: toIsoDate(dayBefore),
    completedOccurrences: beforeHere(todo.completedOccurrences),
    skippedOccurrences: beforeHere(todo.skippedOccurrences),
    occurrenceTimes: timesFrom,
  };
  const sibling: DailyTodo = {
    ...todo,
    id: makeId(),
    date: occurrenceDate,
    time,
    endTime,
    completed: false,
    completedAt: null,
    completedOccurrences: fromHere(todo.completedOccurrences),
    skippedOccurrences: fromHere(todo.skippedOccurrences),
    occurrenceTimes: {},
    createdAt: new Date().toISOString(),
  };
  return [original, sibling];
}
