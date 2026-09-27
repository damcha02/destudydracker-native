import { describe, expect, it } from "vitest";
import { calculateScheduledDailyWork, calculateScheduledWorkload, getScheduledUnits } from "./scheduleWorkload";
import { makeTimetableEvent } from "./plannerSchedule";
import type { Semester, Task } from "../types";

const semester: Semester = { id: "s", name: "HS", createdAt: "", startDate: "2026-09-01", endDate: "2026-12-20", phase: "semester", archived: false, archivedAt: null };
const task = (id: string, overrides: Partial<Task> = {}): Task => ({
  id, semesterId: "s", courseId: "c", title: "Lecture", subtype: "Lecture", unitLabel: "Lecture", totalUnits: 15, completedUnits: 1,
  dueDate: null, priority: "medium", notes: "", createdAt: "", ...overrides,
} as Task);
const weekly = (taskId: string, extra = {}) => makeTimetableEvent({ id: `e-${taskId}`, semesterId: "s", courseId: "c", taskId, kind: "occurrence", label: "L", date: "2026-09-02", time: "10:00", repeatWeekly: true, ...extra });
const today = "2026-09-20";

describe("scheduled workload", () => {
  it("does not treat undated, calendar-scheduled tasks as due today", () => {
    const events = [weekly("t")];
    events[0].completedOccurrences = ["2026-09-02", "2026-09-09", "2026-09-16"];
    const units = getScheduledUnits(events, [], [semester], today);
    const work = calculateScheduledWorkload([task("t")], units, today);
    // one lecture in the next 7 days (Sep 23), nothing overdue -> about 0.14 units/day, not "all of it today"
    expect(work.unitsPerDay).toBeCloseTo(1 / 7, 2);
    expect(work.daysLeft).toBe(3);
    expect(work.nearestDueDate).toBe("2026-09-23");
  });

  it("counts missed occurrences as overdue backlog", () => {
    const events = [weekly("t")];
    events[0].completedOccurrences = ["2026-09-02"];
    const units = getScheduledUnits(events, [], [semester], today);
    const work = calculateScheduledWorkload([task("t")], units, today);
    expect(work.unitsPerDay).toBeCloseTo(3 / 7, 2); // Sep 9 + Sep 16 overdue, Sep 23 this week
  });

  it("leaves unscheduled undated tasks out of the pace", () => {
    const work = calculateScheduledWorkload([task("free", { totalUnits: 10, completedUnits: 0 })], new Map(), today);
    expect(work.unitsPerDay).toBeLessThan(10);
    expect(work.undatedRemainingUnits).toBe(10);
  });

  it("gives a per-task pace, or null when nothing is scheduled", () => {
    const units = getScheduledUnits([weekly("t")], [], [semester], today);
    expect(calculateScheduledDailyWork(task("t"), units.get("t"), today)).not.toBeNull();
    expect(calculateScheduledDailyWork(task("x"), undefined, today)).toBeNull();
  });
});
