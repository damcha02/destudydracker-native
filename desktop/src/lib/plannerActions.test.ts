import { describe, expect, it } from "vitest";
import { defaultState } from "./storage";
import { applyUndoPatch, convertTodoRepeat, countCompletedUnitOccurrences, diffForUndo, getOverdueTodos, isEmptyUndoPatch, setTodoOccurrenceTime, splitRecurringTodoAt, unitDecrementFor } from "./plannerActions";
import type { AppState, DailyTodo, Task, TimetableEvent } from "../types";

function todo(overrides: Partial<DailyTodo> = {}): DailyTodo {
  return {
    id: "t1", date: "2026-09-07", time: "09:00", endTime: "09:30", title: "Review", notes: "", completed: false, completedAt: null,
    createdAt: "2026-01-01T00:00:00.000Z", repeatWeekly: false, completedOccurrences: [], recurrenceEndDate: null, skippedOccurrences: [], occurrenceTimes: {},
    ...overrides,
  };
}

function task(id: string): Task {
  return { id, semesterId: "s", courseId: "c", title: id, subtype: "Lecture", unitLabel: "unit", totalUnits: 4, completedUnits: 2, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" } as Task;
}

function event(id: string, taskId: string, completed: string[], kind: TimetableEvent["kind"] = "occurrence"): TimetableEvent {
  return {
    id, semesterId: "s", courseId: "c", kind, taskId, label: id, date: "2026-09-07", time: "10:00", endTime: "12:00", repeatWeekly: true,
    recurrenceEndDate: null, occurrenceOverrides: {}, url: null, completedOccurrences: completed, createdAt: "2026-01-01T00:00:00.000Z",
  };
}

const base: AppState = { ...defaultState, tasks: [task("a"), task("b")], dailyTodos: [todo({ id: "t1" }), todo({ id: "t2" })] };

describe("undo patches", () => {
  it("restores only the deleted items and leaves later, unrelated changes alone", () => {
    const after = { ...base, tasks: base.tasks.filter((item) => item.id !== "a") };
    const patch = diffForUndo(base, after);
    expect(patch.removed).toHaveLength(1);
    const later: AppState = { ...after, lifetimeStudyMinutes: 999, dailyTodos: after.dailyTodos.map((item) => (item.id === "t1" ? { ...item, completed: true } : item)) };
    const restored = applyUndoPatch(later, patch);
    expect(restored.tasks.map((item) => item.id)).toEqual(["a", "b"]);
    expect(restored.lifetimeStudyMinutes).toBe(999);
    expect(restored.dailyTodos[0].completed).toBe(true);
  });

  it("restores items a delete modified, and cleared timer references only if still cleared", () => {
    const withTimer: AppState = { ...base, timer: { ...base.timer, taskId: "a" } };
    const after: AppState = { ...withTimer, tasks: withTimer.tasks.filter((item) => item.id !== "a"), timer: { ...withTimer.timer, taskId: null } };
    const patch = diffForUndo(withTimer, after);
    expect(applyUndoPatch(after, patch).timer.taskId).toBe("a");
    const retargeted: AppState = { ...after, timer: { ...after.timer, taskId: "b" } };
    expect(applyUndoPatch(retargeted, patch).timer.taskId).toBe("b");
  });

  it("is empty when nothing changed", () => {
    expect(isEmptyUndoPatch(diffForUndo(base, base))).toBe(true);
  });
});

describe("completed-unit accounting", () => {
  it("counts non-release completions per task", () => {
    const events = [event("e1", "a", ["1", "2"]), event("e2", "a", ["3"], "sheet-release"), event("e3", "b", ["4"])];
    expect(countCompletedUnitOccurrences(events, "a")).toBe(2);
  });

  it("does not subtract occurrences that were beyond the capped total", () => {
    expect(unitDecrementFor(3, 4, 1)).toBe(1);
    expect(unitDecrementFor(6, 4, 1)).toBe(0);
    expect(unitDecrementFor(6, 4, 3)).toBe(1);
  });
});

describe("to-do series helpers", () => {
  it("keeps completion when toggling repeat on and off", () => {
    const done = todo({ completed: true, completedAt: "2026-09-07T10:00:00.000Z" });
    const repeating = convertTodoRepeat(done, true);
    expect(repeating.completedOccurrences).toEqual(["2026-09-07"]);
    const back = convertTodoRepeat(repeating, false);
    expect(back.completed).toBe(true);
    expect(convertTodoRepeat(todo({ repeatWeekly: true, completedOccurrences: ["2026-09-14"] }), false).completed).toBe(false);
  });

  it("splits a series at an occurrence, dividing completions", () => {
    const series = todo({ repeatWeekly: true, completedOccurrences: ["2026-09-07", "2026-09-21"], skippedOccurrences: ["2026-09-28"] });
    const [original, sibling] = splitRecurringTodoAt(series, "2026-09-21", "14:00", "14:30", () => "new");
    expect(original.recurrenceEndDate).toBe("2026-09-20");
    expect(original.completedOccurrences).toEqual(["2026-09-07"]);
    expect(sibling).toMatchObject({ id: "new", date: "2026-09-21", time: "14:00", completedOccurrences: ["2026-09-21"], skippedOccurrences: ["2026-09-28"] });
    expect(splitRecurringTodoAt(series, "2026-09-07", "14:00", "14:30", () => "x")).toHaveLength(1);
  });

  it("overrides a single occurrence's time", () => {
    expect(setTodoOccurrenceTime(todo({ repeatWeekly: true }), "2026-09-14", "16:00", null).occurrenceTimes["2026-09-14"]).toEqual({ time: "16:00", endTime: null });
  });

  it("finds overdue, unfinished, non-repeating to-dos", () => {
    const list = [todo({ id: "old", date: "2026-09-01" }), todo({ id: "done", date: "2026-09-01", completed: true }), todo({ id: "rep", date: "2026-09-01", repeatWeekly: true }), todo({ id: "today", date: "2026-09-18" })];
    expect(getOverdueTodos(list, "2026-09-18").map((item) => item.id)).toEqual(["old"]);
  });
});
