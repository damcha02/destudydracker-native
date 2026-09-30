import type { AppState, Course, TaskSubtype } from "../types";
import { expandDailyTodoDates, expandTimetableEvents, getTodoOccurrenceTime, parseIsoDate, toIsoDate } from "./plannerSchedule";

export type PinwallTimeframe = "week" | "month" | "semester";
export type PinwallGrouping = "subject" | "type";
export type PinwallItemType = "lecture" | "session" | "sheet-due" | "unit" | "study-block" | "todo";

/** Which piece of state a pinwall checkbox flips - mirrors the three completion stores the planner already has. */
export type PinwallRef =
  | { kind: "event"; id: string; occurrenceDate: string }
  | { kind: "todo"; id: string; occurrenceDate: string }
  | { kind: "calendar-entry"; id: string };

export interface PinwallItem {
  key: string;
  type: PinwallItemType;
  title: string;
  courseId: string | null;
  date: string;
  time: string | null;
  endTime: string | null;
  ref: PinwallRef;
}

export const pinwallTypeLabels: Record<PinwallItemType, string> = {
  lecture: "Lecture",
  session: "Exercise Session",
  "sheet-due": "Sheet Due",
  unit: "Course Unit",
  "study-block": "Study Block",
  todo: "To-Do",
};

const typeOrder: PinwallItemType[] = ["sheet-due", "lecture", "session", "unit", "study-block", "todo"];

function occurrenceType(subtype: TaskSubtype | undefined): PinwallItemType {
  if (subtype === "Lecture") return "lecture";
  if (subtype === "Session") return "session";
  return "unit";
}

function addDays(iso: string, days: number): string {
  const date = parseIsoDate(iso);
  date.setDate(date.getDate() + days);
  return toIsoDate(date);
}

/** First day (inclusive) a timeframe reaches back to. Weeks start on Monday, like the calendar grid. */
export function pinwallRangeStart(timeframe: PinwallTimeframe, todayIso: string, state: Pick<AppState, "semesters">): string {
  if (timeframe === "week") {
    const mondayOffset = (parseIsoDate(todayIso).getDay() + 6) % 7;
    return addDays(todayIso, -mondayOffset);
  }
  if (timeframe === "month") return addDays(todayIso, -30);
  // "All semester": back to the earliest start of any semester still running. Without dated
  // semesters there's no natural bound, so everything on record counts.
  const starts = state.semesters
    .filter((semester) => !semester.archived && semester.startDate && semester.startDate <= todayIso)
    .map((semester) => semester.startDate as string);
  return starts.length ? starts.reduce((min, start) => (start < min ? start : min)) : "0000-01-01";
}

function isDue(date: string, time: string | null, todayIso: string, nowTime: string): boolean {
  if (date < todayIso) return true;
  if (date > todayIso) return false;
  return !time || time <= nowTime;
}

/**
 * Every unchecked item scheduled between the timeframe's start and right now: subject-bound
 * timetable occurrences (lectures, sessions, sheet deadlines - releases are informational and never
 * count as units, so they're left off), study blocks placed on the calendar, and to-dos, including
 * each projected occurrence of a repeating one. Today's items only count once their time has come.
 */
export function collectPinwallItems(
  state: Pick<AppState, "semesters" | "courses" | "tasks" | "timetableEvents" | "holidays" | "calendarEntries" | "dailyTodos">,
  timeframe: PinwallTimeframe,
  now: Date,
): PinwallItem[] {
  const todayIso = toIsoDate(now);
  const nowTime = `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
  const rangeStart = pinwallRangeStart(timeframe, todayIso, state);
  const tasksById = new Map(state.tasks.map((task) => [task.id, task]));
  const items: PinwallItem[] = [];

  for (const semester of state.semesters) {
    for (const { event, date } of expandTimetableEvents(state.timetableEvents, state.holidays, semester, rangeStart, todayIso)) {
      if (event.kind === "sheet-release") continue;
      if (event.completedOccurrences.includes(date) || !isDue(date, event.time, todayIso, nowTime)) continue;
      items.push({
        key: `event:${event.id}:${date}`,
        type: event.kind === "sheet-deadline" ? "sheet-due" : occurrenceType(tasksById.get(event.taskId)?.subtype),
        title: event.label,
        courseId: event.courseId,
        date,
        time: event.time,
        endTime: event.endTime,
        ref: { kind: "event", id: event.id, occurrenceDate: date },
      });
    }
  }

  const activeSemesterIds = new Set(state.semesters.filter((semester) => !semester.archived).map((semester) => semester.id));
  for (const entry of state.calendarEntries) {
    if (entry.completed || entry.date < rangeStart || !isDue(entry.date, entry.startTime ?? null, todayIso, nowTime)) continue;
    const task = tasksById.get(entry.taskId);
    const semesterId = task?.semesterId ?? entry.adHocSemesterId;
    if (semesterId && !activeSemesterIds.has(semesterId)) continue;
    items.push({
      key: `calendar-entry:${entry.id}`,
      type: "study-block",
      title: entry.adHocTitle ?? task?.title ?? "Study block",
      courseId: task?.courseId ?? entry.adHocCourseId ?? null,
      date: entry.date,
      time: entry.startTime ?? null,
      endTime: entry.endTime ?? null,
      ref: { kind: "calendar-entry", id: entry.id },
    });
  }

  for (const todo of state.dailyTodos) {
    for (const date of expandDailyTodoDates(todo, rangeStart, todayIso)) {
      const done = todo.repeatWeekly ? todo.completedOccurrences.includes(date) : todo.completed;
      const { time, endTime } = getTodoOccurrenceTime(todo, date);
      if (done || !isDue(date, time, todayIso, nowTime)) continue;
      items.push({
        key: `todo:${todo.id}:${date}`,
        type: "todo",
        title: todo.title,
        courseId: null,
        date,
        time,
        endTime,
        ref: { kind: "todo", id: todo.id, occurrenceDate: date },
      });
    }
  }

  return items.sort(comparePinwallItems);
}

/** Oldest first, then by time of day (untimed items last within a day). */
export function comparePinwallItems(a: PinwallItem, b: PinwallItem): number {
  if (a.date !== b.date) return a.date < b.date ? -1 : 1;
  const at = a.time ?? "99:99";
  const bt = b.time ?? "99:99";
  return at === bt ? a.title.localeCompare(b.title) : at < bt ? -1 : 1;
}

export interface PinwallGroup {
  id: string;
  label: string;
  color: string | null;
  items: PinwallItem[];
}

/** Buckets items into subject cards (courses in semester order, loose to-dos last) or item-type cards. */
export function groupPinwallItems(items: PinwallItem[], grouping: PinwallGrouping, courses: Course[]): PinwallGroup[] {
  if (grouping === "type") {
    return typeOrder
      .map((type) => ({ id: type, label: pinwallTypeLabels[type], color: null, items: items.filter((item) => item.type === type) }))
      .filter((group) => group.items.length);
  }
  const groups: PinwallGroup[] = courses
    .map((course) => ({ id: course.id, label: course.name, color: course.color, items: items.filter((item) => item.courseId === course.id) }))
    .filter((group) => group.items.length);
  const knownCourseIds = new Set(courses.map((course) => course.id));
  const loose = items.filter((item) => !item.courseId || !knownCourseIds.has(item.courseId));
  if (loose.length) {
    const label = loose.every((item) => item.type === "todo") ? "Personal To-Dos" : "No Subject";
    groups.push({ id: "__personal", label, color: null, items: loose });
  }
  return groups;
}
