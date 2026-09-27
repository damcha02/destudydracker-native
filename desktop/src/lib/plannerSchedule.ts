import type { CalendarEntry, DailyTodo, Exam, Holiday, Semester, TimetableEvent, TimetableEventKind } from "../types";

/**
 * The single canonical constructor for a TimetableEvent. Every creation path (the main calendar's
 * click-to-create modal, the Manage Semesters window's inline scheduler, the sheet release/due
 * dual-scheduler) must build events through this function instead of writing object literals by
 * hand, so the stored schema can never drift between entry points - the same repeatWeekly flag,
 * the same empty occurrenceOverrides/completedOccurrences, the same null defaults, every time.
 * The weekly-recurrence weekday is never stored separately - it is always implicit in `date`
 * (`new Date(date).getDay()`), read out by expandWeekdayFrom in this same module.
 */
export function makeTimetableEvent(params: {
  id: string;
  semesterId: string;
  courseId: string;
  taskId: string;
  kind: TimetableEventKind;
  label: string;
  date: string;
  time: string;
  endTime?: string | null;
  repeatWeekly: boolean;
  url?: string | null;
  createdAt?: string;
}): TimetableEvent {
  return {
    id: params.id,
    semesterId: params.semesterId,
    courseId: params.courseId,
    taskId: params.taskId,
    kind: params.kind,
    label: params.label,
    date: params.date,
    time: params.time,
    endTime: params.endTime ?? null,
    repeatWeekly: params.repeatWeekly,
    recurrenceEndDate: null,
    occurrenceOverrides: {},
    url: params.url ?? null,
    completedOccurrences: [],
    createdAt: params.createdAt ?? new Date().toISOString(),
  };
}

function clockToMinutes(time: string): number {
  const [hours, minutes] = time.split(":").map(Number);
  return hours * 60 + minutes;
}

/** The clock time `durationMinutes` after `start`, or null if that would run past midnight. */
export function endTimeFor(start: string, durationMinutes: number): string | null {
  if (!/^\d{1,2}:\d{2}$/.test(start) || !(durationMinutes > 0)) return null;
  const total = clockToMinutes(start) + durationMinutes;
  if (total > 24 * 60) return null;
  const clamped = Math.min(total, 23 * 60 + 59);
  return `${String(Math.floor(clamped / 60)).padStart(2, "0")}:${String(clamped % 60).padStart(2, "0")}`;
}

/** Minutes between two clock times, or `fallback` when the end is missing or not after the start. */
export function durationBetween(start: string, end: string | null | undefined, fallback: number): number {
  if (!end) return fallback;
  const minutes = clockToMinutes(end) - clockToMinutes(start);
  return minutes > 0 ? minutes : fallback;
}

/** True for a real calendar date written YYYY-MM-DD (some webviews fall back to a plain text box for date inputs). */
export function isValidIsoDate(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  return toIsoDate(parseIsoDate(value)) === value;
}

export function parseIsoDate(iso: string): Date {
  const [year, month, day] = iso.split("-").map(Number);
  return new Date(year, (month || 1) - 1, day || 1);
}

export function toIsoDate(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function addDays(date: Date, days: number): Date {
  const next = new Date(date);
  next.setDate(next.getDate() + days);
  return next;
}

function maxIso(a: string | null, b: string): string {
  if (!a) return b;
  return a > b ? a : b;
}

function minIso(a: string | null, b: string): string {
  if (!a) return b;
  return a < b ? a : b;
}

function daysBetween(startIso: string, endIso: string): number {
  return Math.round((parseIsoDate(endIso).getTime() - parseIsoDate(startIso).getTime()) / 86400000);
}

function isSemesterScheduleActive(semester: Semester): boolean {
  return !semester.archived && semester.phase === "semester";
}

export function isHoliday(holidays: Holiday[], semesterId: string, dateIso: string): boolean {
  return holidays.some((holiday) => holiday.semesterId === semesterId && dateIso >= holiday.startDate && dateIso <= holiday.endDate);
}

export function expandWeekdayFrom(anchorDateIso: string, rangeStartIso: string, rangeEndIso: string): string[] {
  if (rangeStartIso > rangeEndIso) return [];
  const weekday = parseIsoDate(anchorDateIso).getDay();
  const rangeStart = parseIsoDate(maxIso(anchorDateIso, rangeStartIso));
  const offset = (weekday - rangeStart.getDay() + 7) % 7;
  let cursor = addDays(rangeStart, offset);
  const dates: string[] = [];
  while (toIsoDate(cursor) <= rangeEndIso) {
    dates.push(toIsoDate(cursor));
    cursor = addDays(cursor, 7);
  }
  return dates;
}

export interface TimetableEventOccurrence {
  event: TimetableEvent;
  date: string;
}

export function expandTimetableEvents(
  events: TimetableEvent[],
  holidays: Holiday[],
  semester: Semester,
  rangeStartIso: string,
  rangeEndIso: string,
): TimetableEventOccurrence[] {
  if (!isSemesterScheduleActive(semester)) return [];
  const effectiveEndBound = minIso(semester.endDate, rangeEndIso);
  const effectiveStart = maxIso(semester.startDate, rangeStartIso);
  if (effectiveStart > effectiveEndBound) return [];

  const occurrences: TimetableEventOccurrence[] = [];
  for (const event of events) {
    if (event.semesterId !== semester.id) continue;
    const seriesEnd = minIso(event.recurrenceEndDate, effectiveEndBound);
    if (event.repeatWeekly) {
      if (effectiveStart <= seriesEnd) {
        for (const date of expandWeekdayFrom(event.date, effectiveStart, seriesEnd)) {
          const override = event.occurrenceOverrides[date];
          if (override?.skipped) continue;
          if (override?.date) {
            if (override.date >= effectiveStart && override.date <= effectiveEndBound && !isHoliday(holidays, semester.id, override.date)) {
              occurrences.push({ event: { ...event, date: override.date, time: override.time ?? event.time, endTime: override.endTime ?? event.endTime }, date: override.date });
            }
            continue;
          }
          if (isHoliday(holidays, semester.id, date)) continue;
          occurrences.push({ event, date });
        }
      }
    } else if (event.date >= effectiveStart && event.date <= effectiveEndBound) {
      occurrences.push({ event, date: event.date });
    }
  }
  return occurrences;
}

/**
 * Projects a to-do's occurrence dates within an inclusive date range: just its own date if it
 * doesn't repeat, or every matching weekday from its anchor date onward if it does. Mirrors
 * countEventOccurrenceDates's role for TimetableEvent, at to-do scale (no holidays to exclude).
 */
export function expandDailyTodoDates(todo: DailyTodo, rangeStartIso: string, rangeEndIso: string): string[] {
  if (!todo.date) return [];
  if (!todo.repeatWeekly) {
    return todo.date >= rangeStartIso && todo.date <= rangeEndIso ? [todo.date] : [];
  }
  const seriesEnd = minIso(todo.recurrenceEndDate, rangeEndIso);
  return expandWeekdayFrom(todo.date, rangeStartIso, seriesEnd).filter((date) => !todo.skippedOccurrences.includes(date));
}

/** The time a to-do shows at on one occurrence date, honoring a "this occurrence only" override. */
export function getTodoOccurrenceTime(todo: DailyTodo, occurrenceDate: string): { time: string | null; endTime: string | null } {
  const override = todo.repeatWeekly ? todo.occurrenceTimes[occurrenceDate] : undefined;
  return override ?? { time: todo.time, endTime: todo.endTime };
}

/**
 * Projects one event's occurrence dates within an inclusive date range, honoring skip/move
 * overrides and holiday exclusions - the same per-event logic expandTimetableEvents uses, but
 * without that function's semester-phase/archived gate. Used to compute a true occurrence count
 * (e.g. a Lecture task's total from its weekly schedule) that shouldn't reset to zero just
 * because the semester has since moved into exam prep.
 */
export function countEventOccurrenceDates(event: TimetableEvent, holidays: Holiday[], semesterId: string, rangeStartIso: string, rangeEndIso: string): string[] {
  if (rangeStartIso > rangeEndIso) return [];
  const dates: string[] = [];
  if (event.repeatWeekly) {
    const seriesEnd = minIso(event.recurrenceEndDate, rangeEndIso);
    if (rangeStartIso <= seriesEnd) {
      for (const date of expandWeekdayFrom(event.date, rangeStartIso, seriesEnd)) {
        const override = event.occurrenceOverrides[date];
        if (override?.skipped) continue;
        if (override?.date) {
          if (override.date >= rangeStartIso && override.date <= rangeEndIso && !isHoliday(holidays, semesterId, override.date)) dates.push(override.date);
          continue;
        }
        if (isHoliday(holidays, semesterId, date)) continue;
        dates.push(date);
      }
    }
  } else if (event.date >= rangeStartIso && event.date <= rangeEndIso) {
    dates.push(event.date);
  }
  return dates;
}

/** Moves a single occurrence of a recurring event to a new date/time without affecting the rest of the series. */
export function moveSingleOccurrence(event: TimetableEvent, originalDateIso: string, newDate: string, newTime: string, newEndTime: string | null): TimetableEvent {
  return {
    ...event,
    occurrenceOverrides: {
      ...event.occurrenceOverrides,
      [originalDateIso]: { date: newDate, time: newTime, endTime: newEndTime },
    },
  };
}

/**
 * Splits a recurring event at the moved occurrence: the original series is truncated to end
 * the day before `originalDateIso`, and a new sibling event picks up the series from the moved
 * occurrence's new date/weekday/time onward. Completion history is intentionally not carried
 * over to the new series.
 */
export function splitRecurringEventAt(
  event: TimetableEvent,
  originalDateIso: string,
  newDate: string,
  newTime: string,
  newEndTime: string | null,
  makeId: () => string,
): { updatedOriginal: TimetableEvent; newEvent: TimetableEvent } {
  const dayBefore = toIsoDate(addDays(parseIsoDate(originalDateIso), -1));
  const updatedOriginal: TimetableEvent = {
    ...event,
    recurrenceEndDate: minIso(event.recurrenceEndDate, dayBefore),
  };
  const newEvent: TimetableEvent = {
    ...event,
    id: makeId(),
    date: newDate,
    time: newTime,
    endTime: newEndTime,
    recurrenceEndDate: null,
    completedOccurrences: [],
    occurrenceOverrides: {},
    createdAt: new Date().toISOString(),
  };
  return { updatedOriginal, newEvent };
}

export function shouldAutoTransitionToExamPrep(semester: Semester, todayIso: string): boolean {
  if (semester.archived || semester.phase !== "semester" || !semester.endDate) return false;
  return todayIso > semester.endDate;
}

export function getSemesterWeekNumber(semester: Semester, dateIso: string): number | null {
  if (!semester.startDate || dateIso < semester.startDate) return null;
  return Math.floor(daysBetween(semester.startDate, dateIso) / 7) + 1;
}

export type DailyTimelineKind = "occurrence" | "sheet-release" | "sheet-deadline" | "exam" | "calendar-entry" | "todo";

export interface DailyTimelineRow {
  id: string;
  kind: DailyTimelineKind;
  time: string | null;
  endTime: string | null;
  sortMinutes: number;
  title: string;
  courseId: string | null;
  taskId?: string | null;
  url?: string | null;
  completed?: boolean;
  refId: string;
  occurrenceDate: string;
}

function timeToSortMinutes(time: string | null | undefined): number {
  if (!time) return 24 * 60 + 1;
  const [hours, minutes] = time.split(":").map(Number);
  return hours * 60 + minutes;
}

export interface DailyTimelineInputs {
  eventOccurrences: TimetableEventOccurrence[];
  exams: Exam[];
  calendarEntries: CalendarEntry[];
  dailyTodos: DailyTodo[];
}

export function buildDailyTimeline(dateIso: string, inputs: DailyTimelineInputs): DailyTimelineRow[] {
  const rows: DailyTimelineRow[] = [];

  for (const occurrence of inputs.eventOccurrences) {
    if (occurrence.date !== dateIso) continue;
    const { event } = occurrence;
    const title = event.kind === "sheet-release" ? `${event.label} released` : event.kind === "sheet-deadline" ? `${event.label} due` : event.label;
    rows.push({
      id: `${event.kind}:${event.id}:${dateIso}`,
      kind: event.kind,
      time: event.time,
      endTime: event.endTime,
      sortMinutes: timeToSortMinutes(event.time),
      title,
      courseId: event.courseId,
      taskId: event.taskId,
      url: event.url,
      completed: event.completedOccurrences.includes(dateIso),
      refId: event.id,
      occurrenceDate: dateIso,
    });
  }

  for (const exam of inputs.exams) {
    if (exam.examDate.slice(0, 10) !== dateIso) continue;
    rows.push({
      id: `exam:${exam.id}`,
      kind: "exam",
      time: null,
      endTime: null,
      sortMinutes: timeToSortMinutes(null),
      title: exam.title,
      courseId: exam.courseId,
      refId: exam.id,
      occurrenceDate: dateIso,
    });
  }

  for (const entry of inputs.calendarEntries) {
    if (entry.date !== dateIso) continue;
    rows.push({
      id: `calendar-entry:${entry.id}`,
      kind: "calendar-entry",
      time: entry.startTime ?? null,
      endTime: entry.endTime ?? null,
      sortMinutes: timeToSortMinutes(entry.startTime),
      title: entry.adHocTitle ?? "Task",
      courseId: entry.adHocCourseId ?? null,
      completed: entry.completed,
      refId: entry.id,
      occurrenceDate: dateIso,
    });
  }

  for (const todo of inputs.dailyTodos) {
    if (expandDailyTodoDates(todo, dateIso, dateIso).length === 0) continue;
    const { time, endTime } = getTodoOccurrenceTime(todo, dateIso);
    rows.push({
      id: `todo:${todo.id}`,
      kind: "todo",
      time,
      endTime,
      sortMinutes: timeToSortMinutes(time),
      title: todo.title,
      courseId: null,
      completed: todo.repeatWeekly ? todo.completedOccurrences.includes(dateIso) : todo.completed,
      refId: todo.id,
      occurrenceDate: dateIso,
    });
  }

  return rows.sort((a, b) => a.sortMinutes - b.sortMinutes);
}

export interface OverlapLayoutItem {
  id: string;
  startMinutes: number;
  endMinutes: number;
}

export interface OverlapLayoutSlot {
  column: number;
  columnCount: number;
}

/**
 * Column-splits a day's timed items so overlapping entries (startA < endB && startB < endA) sit
 * side by side instead of stacking on top of each other. Items are grouped into connected
 * overlap clusters (touching endpoints, e.g. one item's end === another's start, do not count as
 * overlapping), then columns are assigned greedily within each cluster. Items with no overlap at
 * all get columnCount 1 so callers can skip any layout override for the common case.
 */
export function computeOverlapLayout(items: OverlapLayoutItem[]): Map<string, OverlapLayoutSlot> {
  const layout = new Map<string, OverlapLayoutSlot>();
  const sorted = [...items].sort((a, b) => a.startMinutes - b.startMinutes || a.endMinutes - b.endMinutes);

  let cluster: OverlapLayoutItem[] = [];
  let clusterEnd = -Infinity;

  function flushCluster() {
    if (!cluster.length) return;
    const columnEnds: number[] = [];
    const columnByItemId = new Map<string, number>();
    for (const item of cluster) {
      let placedColumn = -1;
      for (let column = 0; column < columnEnds.length; column += 1) {
        if (columnEnds[column] <= item.startMinutes) {
          columnEnds[column] = item.endMinutes;
          placedColumn = column;
          break;
        }
      }
      if (placedColumn === -1) {
        columnEnds.push(item.endMinutes);
        placedColumn = columnEnds.length - 1;
      }
      columnByItemId.set(item.id, placedColumn);
    }
    const columnCount = columnEnds.length;
    for (const item of cluster) {
      layout.set(item.id, { column: columnByItemId.get(item.id) ?? 0, columnCount });
    }
    cluster = [];
  }

  for (const item of sorted) {
    if (cluster.length && item.startMinutes >= clusterEnd) {
      flushCluster();
      clusterEnd = -Infinity;
    }
    cluster.push(item);
    clusterEnd = Math.max(clusterEnd, item.endMinutes);
  }
  flushCluster();

  return layout;
}
