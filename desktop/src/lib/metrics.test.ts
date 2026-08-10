import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  calculateAggregateWorkload,
  getCourseHealth,
  getCourseHealthMap,
  getCourseMinutesMap,
  getFirstSessionDate,
  getLocalMonthlyStats,
  getSemesterHealth,
  getSemesterHealthMap,
  getSessionDaySet,
  getStudySessionCount,
  getWeeklyCourseCount,
  groupTasksByCourseId,
  groupTasksBySemesterId,
} from "./metrics";
import type { AppState, Course, Exam, Semester, StudySession, Task } from "../types";

function task(overrides: Partial<Task>): Task {
  return {
    id: "task",
    semesterId: "semester",
    courseId: "course",
    title: "Task",
    unitLabel: "Unit",
    totalUnits: 10,
    completedUnits: 0,
    dueDate: null,
    priority: "medium",
    notes: "",
    createdAt: "2026-08-01T00:00:00.000Z",
    ...overrides,
  };
}

function course(overrides: Partial<Course>): Course {
  return {
    id: "course",
    semesterId: "semester",
    name: "Course",
    color: "#8fb4ff",
    targetGrade: 4,
    createdAt: "2026-08-01T00:00:00.000Z",
    ...overrides,
  };
}

function semester(overrides: Partial<Semester>): Semester {
  return {
    id: "semester",
    name: "Semester",
    createdAt: "2026-08-01T00:00:00.000Z",
    ...overrides,
  };
}

function exam(overrides: Partial<Exam>): Exam {
  return {
    id: "exam",
    semesterId: "semester",
    courseId: "course",
    title: "Exam",
    examDate: "2026-08-15",
    weight: 40,
    preparedness: 35,
    ...overrides,
  };
}

function session(overrides: Partial<StudySession>): StudySession {
  return {
    id: "session",
    semesterId: "semester",
    courseId: "course",
    taskId: null,
    kind: "study",
    goal: "",
    learned: "",
    blocker: "",
    nextStep: "",
    confidence: 3,
    startedAt: "2026-08-01T10:00:00.000Z",
    endedAt: "2026-08-01T10:30:00.000Z",
    minutes: 30,
    presetLabel: "Focus 25/5",
    ...overrides,
  };
}

describe("calculateAggregateWorkload", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-01T12:00:00"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("adds each task's deadline-specific daily pace", () => {
    const workload = calculateAggregateWorkload([
      task({ id: "first", totalUnits: 10, dueDate: "2026-08-11" }),
      task({ id: "second", totalUnits: 20, dueDate: "2026-08-21" }),
    ]);

    expect(workload.unitsPerDay).toBe(2);
    expect(workload.remainingUnits).toBe(30);
    expect(workload.nearestDueDate).toBe("2026-08-11");
  });

  it("counts due-today and overdue work as today's pace", () => {
    const workload = calculateAggregateWorkload([
      task({ id: "today", totalUnits: 3, dueDate: "2026-08-01" }),
      task({ id: "overdue", totalUnits: 4, dueDate: "2026-07-31" }),
      task({ id: "future", totalUnits: 10, dueDate: "2026-08-11" }),
    ]);

    expect(workload.unitsPerDay).toBe(8);
  });

  it("excludes completed and undated units from a dated pace", () => {
    const workload = calculateAggregateWorkload([
      task({ id: "dated", totalUnits: 10, completedUnits: 2, dueDate: "2026-08-11" }),
      task({ id: "complete", totalUnits: 8, completedUnits: 8, dueDate: "2026-08-02" }),
      task({ id: "undated", totalUnits: 5 }),
    ]);

    expect(workload.unitsPerDay).toBe(0.8);
    expect(workload.undatedRemainingUnits).toBe(5);
    expect(workload.message).toContain("5 undated units are not included");
  });

  it("uses the existing fallback when no unfinished work has a due date", () => {
    const workload = calculateAggregateWorkload([task({ totalUnits: 5 })]);

    expect(workload.unitsPerDay).toBe(5);
    expect(workload.message).toContain("Add due dates");
  });
});

describe("groupTasksByCourseId / groupTasksBySemesterId", () => {
  it("returns an empty map for no tasks", () => {
    expect(groupTasksByCourseId([])).toEqual(new Map());
    expect(groupTasksBySemesterId([])).toEqual(new Map());
  });

  it("groups tasks across multiple courses and semesters", () => {
    const tasks = [
      task({ id: "a", courseId: "c1", semesterId: "s1" }),
      task({ id: "b", courseId: "c1", semesterId: "s1" }),
      task({ id: "c", courseId: "c2", semesterId: "s2" }),
    ];

    const byCourse = groupTasksByCourseId(tasks);
    expect(byCourse.get("c1")?.map((t) => t.id)).toEqual(["a", "b"]);
    expect(byCourse.get("c2")?.map((t) => t.id)).toEqual(["c"]);

    const bySemester = groupTasksBySemesterId(tasks);
    expect(bySemester.get("s1")?.map((t) => t.id)).toEqual(["a", "b"]);
    expect(bySemester.get("s2")?.map((t) => t.id)).toEqual(["c"]);
  });

  it("still groups tasks whose courseId/semesterId points at a deleted course/semester", () => {
    const tasks = [task({ id: "orphan", courseId: "deleted-course", semesterId: "deleted-semester" })];
    expect(groupTasksByCourseId(tasks).get("deleted-course")?.map((t) => t.id)).toEqual(["orphan"]);
    expect(groupTasksBySemesterId(tasks).get("deleted-semester")?.map((t) => t.id)).toEqual(["orphan"]);
  });
});

describe("getCourseMinutesMap", () => {
  it("returns an empty map for no sessions", () => {
    expect(getCourseMinutesMap([])).toEqual(new Map());
  });

  it("sums minutes per course across multiple sessions", () => {
    const sessions = [
      session({ id: "a", courseId: "c1", minutes: 25 }),
      session({ id: "b", courseId: "c1", minutes: 30 }),
      session({ id: "c", courseId: "c2", minutes: 10 }),
    ];
    const map = getCourseMinutesMap(sessions);
    expect(map.get("c1")).toBe(55);
    expect(map.get("c2")).toBe(10);
  });

  it("excludes sessions with no courseId, matching getCourseMinutes's existing behavior", () => {
    const sessions = [session({ id: "a", courseId: null, minutes: 99 })];
    expect(getCourseMinutesMap(sessions).size).toBe(0);
  });

  it("still records minutes for a session referencing a deleted course (simply never looked up)", () => {
    const sessions = [session({ id: "a", courseId: "deleted-course", minutes: 15 })];
    expect(getCourseMinutesMap(sessions).get("deleted-course")).toBe(15);
  });
});

describe("getCourseHealthMap / getSemesterHealthMap", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-01T12:00:00"));
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("matches calling getCourseHealth/getSemesterHealth directly, for multiple courses/semesters", () => {
    const courses = [course({ id: "c1", semesterId: "s1" }), course({ id: "c2", semesterId: "s1" })];
    const semesters = [semester({ id: "s1" })];
    const tasks = [
      task({ id: "t1", courseId: "c1", semesterId: "s1", dueDate: "2026-07-30" }), // overdue
      task({ id: "t2", courseId: "c2", semesterId: "s1", completedUnits: 10 }),
    ];
    const exams = [exam({ id: "e1", courseId: "c1", semesterId: "s1", examDate: "2026-08-05" })];
    const state = { tasks, exams } as AppState;

    const healthByCourse = getCourseHealthMap(courses, groupTasksByCourseId(tasks), exams);
    for (const c of courses) {
      expect(healthByCourse.get(c.id)).toEqual(getCourseHealth(state, c));
    }

    const healthBySemester = getSemesterHealthMap(semesters, groupTasksBySemesterId(tasks), exams);
    for (const s of semesters) {
      expect(healthBySemester.get(s.id)).toEqual(getSemesterHealth(state, s));
    }
  });

  it("produces a valid (not missing) entry for a course/semester with no tasks or exams", () => {
    const courses = [course({ id: "empty" })];
    const semesters = [semester({ id: "empty" })];
    const healthByCourse = getCourseHealthMap(courses, groupTasksByCourseId([]), []);
    const healthBySemester = getSemesterHealthMap(semesters, groupTasksBySemesterId([]), []);
    expect(healthByCourse.get("empty")).toEqual({ score: 0, label: "Preparing", overdue: 0, dueSoon: 0, unitsPerDay: 0 });
    expect(healthBySemester.get("empty")).toEqual({ score: 0, label: "Preparing", overdue: 0, dueSoon: 0, unitsPerDay: 0 });
  });

  it("reflects the day boundary: a task due yesterday counts as overdue", () => {
    // System time fixed at 2026-08-01T12:00 (see beforeEach); a task due 2026-07-31 is overdue.
    const courses = [course({ id: "c1" })];
    const tasks = [task({ id: "t1", courseId: "c1", dueDate: "2026-07-31" })];
    const healthByCourse = getCourseHealthMap(courses, groupTasksByCourseId(tasks), []);
    expect(healthByCourse.get("c1")?.overdue).toBe(1);
  });
});

describe("getSessionDaySet / getFirstSessionDate", () => {
  it("returns empty/null for no sessions", () => {
    expect(getSessionDaySet([])).toEqual(new Set());
    expect(getFirstSessionDate([])).toBeNull();
  });

  it("computes the distinct set of session days", () => {
    // Mid-day UTC timestamps, well clear of a local-midnight rollover in any real timezone
    // (isoDate uses local date components, same as the original App.tsx code).
    const sessions = [
      session({ id: "a", endedAt: "2026-08-01T12:00:00.000Z" }),
      session({ id: "b", endedAt: "2026-08-01T13:00:00.000Z" }),
      session({ id: "c", endedAt: "2026-08-02T12:00:00.000Z" }),
    ];
    expect(getSessionDaySet(sessions)).toEqual(new Set(["2026-08-01", "2026-08-02"]));
  });

  it("finds the chronologically earliest session's startedAt, regardless of array order", () => {
    const sessions = [
      session({ id: "later", startedAt: "2026-08-05T10:00:00.000Z" }),
      session({ id: "earliest", startedAt: "2026-08-01T09:00:00.000Z" }),
      session({ id: "middle", startedAt: "2026-08-03T09:00:00.000Z" }),
    ];
    expect(getFirstSessionDate(sessions)).toBe("2026-08-01T09:00:00.000Z");
  });

  it("keeps the first-encountered session when multiple share the earliest startedAt", () => {
    const tied = "2026-08-01T09:00:00.000Z";
    const sessions = [session({ id: "first", startedAt: tied }), session({ id: "second", startedAt: tied })];
    // Matches Array.prototype.sort's stability: the first equal-key element stays first.
    expect(getFirstSessionDate(sessions)).toBe(tied);
  });
});

describe("getStudySessionCount / getWeeklyCourseCount", () => {
  it("counts only study/exam sessions, excluding breaks", () => {
    const sessions = [session({ kind: "study" }), session({ kind: "exam" }), session({ kind: "break" })];
    expect(getStudySessionCount(sessions)).toBe(2);
  });

  it("counts distinct courses studied within the trailing 7-day window", () => {
    const sessions = [
      session({ courseId: "c1", kind: "study", endedAt: "2026-08-01T10:00:00.000Z" }),
      session({ courseId: "c2", kind: "study", endedAt: "2026-07-30T10:00:00.000Z" }),
      session({ courseId: "c3", kind: "study", endedAt: "2026-07-20T10:00:00.000Z" }), // outside the window
      session({ courseId: null, kind: "study", endedAt: "2026-08-01T10:00:00.000Z" }), // no course
      session({ courseId: "c1", kind: "break", endedAt: "2026-08-01T10:00:00.000Z" }), // not study/exam
    ];
    expect(getWeeklyCourseCount(sessions, "2026-08-01")).toBe(2);
  });
});

describe("getLocalMonthlyStats", () => {
  it("sums minutes/sessions for study/exam sessions within the current month only", () => {
    // All timestamps sit at mid-day, well clear of any month boundary, so this test is not
    // sensitive to the test runner's local timezone (getLocalMonthlyStats compares against a
    // local-time month boundary, same as the original App.tsx code it was extracted from).
    const sessions = [
      session({ kind: "study", endedAt: "2026-08-05T12:00:00.000Z", minutes: 25 }),
      session({ kind: "exam", endedAt: "2026-08-15T12:00:00.000Z", minutes: 60 }),
      session({ kind: "study", endedAt: "2026-07-20T12:00:00.000Z", minutes: 40 }), // previous month
      session({ kind: "break", endedAt: "2026-08-10T12:00:00.000Z", minutes: 10 }), // not study/exam
    ];
    const result = getLocalMonthlyStats(sessions, new Date("2026-08-20T12:00:00.000Z"));
    expect(result).toEqual({ minutes: 85, sessions: 2 });
  });

  it("returns zero for no sessions in the current month", () => {
    expect(getLocalMonthlyStats([], new Date("2026-08-20T12:00:00.000Z"))).toEqual({ minutes: 0, sessions: 0 });
  });
});
