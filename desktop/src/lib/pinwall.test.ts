import { describe, expect, it } from "vitest";
import { defaultState } from "./storage";
import { collectPinwallItems, groupPinwallItems, pinwallRangeStart } from "./pinwall";
import type { AppState, CalendarEntry, Course, DailyTodo, Semester, Task, TimetableEvent } from "../types";

// Monday 2026-09-28, 14:00 local.
const now = new Date(2026, 8, 28, 14, 0);

const semester: Semester = {
  id: "s", name: "WS", createdAt: "", startDate: "2026-08-03", endDate: "2026-12-20", phase: "semester", archived: false, archivedAt: null,
};
const course: Course = { id: "c", semesterId: "s", name: "Analysis", color: "#336699", targetGrade: 1, createdAt: "", externalUrl: null };

function task(id: string, subtype: Task["subtype"]): Task {
  return { id, semesterId: "s", courseId: "c", title: id, subtype, unitLabel: "unit", totalUnits: 10, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "" };
}

function event(overrides: Partial<TimetableEvent>): TimetableEvent {
  return {
    id: "e", semesterId: "s", courseId: "c", kind: "occurrence", taskId: "lec", label: "Lecture", date: "2026-09-07", time: "10:00", endTime: "12:00",
    repeatWeekly: true, recurrenceEndDate: null, occurrenceOverrides: {}, url: null, completedOccurrences: [], createdAt: "", ...overrides,
  };
}

function todo(overrides: Partial<DailyTodo>): DailyTodo {
  return {
    id: "t", date: "2026-09-25", time: null, endTime: null, title: "Buy paper", notes: "", completed: false, completedAt: null, createdAt: "",
    repeatWeekly: false, completedOccurrences: [], recurrenceEndDate: null, skippedOccurrences: [], occurrenceTimes: {}, ...overrides,
  };
}

function entry(overrides: Partial<CalendarEntry>): CalendarEntry {
  return { id: "ce", taskId: "sheet", date: "2026-09-28", unitAmount: 1, completed: false, completedAt: null, createdAt: "", ...overrides };
}

function stateWith(overrides: Partial<AppState>): AppState {
  return { ...defaultState, semesters: [semester], courses: [course], tasks: [task("lec", "Lecture"), task("sheet", "Sheet")], ...overrides };
}

describe("pinwallRangeStart", () => {
  it("reaches back to Monday, 30 days, or the earliest running semester start", () => {
    expect(pinwallRangeStart("week", "2026-10-01", stateWith({}))).toBe("2026-09-28");
    expect(pinwallRangeStart("week", "2026-09-27", stateWith({}))).toBe("2026-09-21");
    expect(pinwallRangeStart("month", "2026-09-28", stateWith({}))).toBe("2026-08-29");
    expect(pinwallRangeStart("semester", "2026-09-28", stateWith({}))).toBe("2026-08-03");
  });
});

describe("collectPinwallItems", () => {
  it("lists unchecked past occurrences and skips completed ones", () => {
    const state = stateWith({ timetableEvents: [event({ completedOccurrences: ["2026-09-14"] })] });
    const dates = collectPinwallItems(state, "month", now).map((item) => item.date);
    // Mondays 09-07, 09-21 and today (10:00 already passed); 09-14 is checked off.
    expect(dates).toEqual(["2026-09-07", "2026-09-21", "2026-09-28"]);
    expect(collectPinwallItems(state, "week", now).map((item) => item.type)).toEqual(["lecture"]);
  });

  it("ignores items later today and sheet releases, and labels sheet deadlines", () => {
    const state = stateWith({
      timetableEvents: [
        event({ id: "later", date: "2026-09-28", time: "18:00", repeatWeekly: false }),
        event({ id: "rel", kind: "sheet-release", taskId: "sheet", date: "2026-09-28", time: "08:00", repeatWeekly: false }),
        event({ id: "due", kind: "sheet-deadline", taskId: "sheet", label: "Sheet 3", date: "2026-09-28", time: "09:00", repeatWeekly: false }),
      ],
    });
    const items = collectPinwallItems(state, "week", now);
    expect(items.map((item) => [item.ref, item.type])).toEqual([[{ kind: "event", id: "due", occurrenceDate: "2026-09-28" }, "sheet-due"]]);
  });

  it("covers study blocks and to-dos, including each open occurrence of a repeating one", () => {
    const state = stateWith({
      calendarEntries: [entry({}), entry({ id: "done", completed: true })],
      dailyTodos: [
        todo({ id: "once" }),
        todo({ id: "weekly", date: "2026-09-14", repeatWeekly: true, completedOccurrences: ["2026-09-21"], title: "Gym" }),
        todo({ id: "old", date: "2026-07-01" }),
      ],
    });
    const keys = collectPinwallItems(state, "month", now).map((item) => item.key);
    expect(keys).toEqual(["todo:weekly:2026-09-14", "todo:once:2026-09-25", "todo:weekly:2026-09-28", "calendar-entry:ce"]);
  });
});

describe("groupPinwallItems", () => {
  it("groups by subject with loose to-dos last, or by type", () => {
    const state = stateWith({ timetableEvents: [event({})], dailyTodos: [todo({ date: "2026-09-28" })] });
    const items = collectPinwallItems(state, "week", now);
    expect(groupPinwallItems(items, "subject", [course]).map((group) => group.label)).toEqual(["Analysis", "Personal To-Dos"]);
    expect(groupPinwallItems(items, "type", [course]).map((group) => group.label)).toEqual(["Lecture", "To-Do"]);
  });
});
