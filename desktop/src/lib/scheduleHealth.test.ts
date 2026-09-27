import { describe, expect, it } from "vitest";
import { getScheduleHealth } from "./scheduleHealth";
import { makeTimetableEvent } from "./plannerSchedule";
import type { Semester } from "../types";

const semester: Semester = { id: "s", name: "HS", createdAt: "", startDate: "2026-09-01", endDate: "2026-12-20", phase: "semester", archived: false, archivedAt: null };
const lecture = (overrides: Partial<Parameters<typeof makeTimetableEvent>[0]> = {}) =>
  makeTimetableEvent({ id: "e", semesterId: "s", courseId: "c", taskId: "t", kind: "occurrence", label: "Lecture", date: "2026-09-02", time: "10:00", repeatWeekly: true, ...overrides });
const base = { holidays: [], semesters: [semester], exams: [], today: "2026-09-20" };

describe("getScheduleHealth", () => {
  it("is null when nothing is scheduled", () => {
    expect(getScheduleHealth({ ...base, events: [] })).toBeNull();
  });

  it("scores 100 when every past occurrence is done, however little of the semester is finished", () => {
    const event = lecture({});
    event.completedOccurrences = ["2026-09-02", "2026-09-09", "2026-09-16"];
    const result = getScheduleHealth({ ...base, events: [event] });
    expect(result?.missed).toBe(0);
    expect(result?.score).toBe(100);
  });

  it("drops with missed occurrences", () => {
    const event = lecture({});
    event.completedOccurrences = ["2026-09-02"];
    const result = getScheduleHealth({ ...base, events: [event] });
    expect(result?.missed).toBe(2);
    expect(result!.score).toBeLessThan(60);
  });

  it("ignores sheet releases", () => {
    const release = lecture({ kind: "sheet-release", label: "Sheet" });
    expect(getScheduleHealth({ ...base, events: [release] })).toBeNull();
  });
});
