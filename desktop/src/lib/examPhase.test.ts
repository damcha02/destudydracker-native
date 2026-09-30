import { describe, expect, it } from "vitest";
import type { Exam, Semester, Task, TimetableEvent } from "../types";
import { expandTimetableEvents } from "./plannerSchedule";
import { examKindOf, getLastExamDate, getRunway, getSemesterStage, isCourseActiveInPrep, makePrepCopy } from "./examPhase";

const semester = (over: Partial<Semester> = {}): Semester => ({ id: "s", name: "HS", createdAt: "", startDate: "2026-09-14", endDate: "2026-12-25", phase: "semester", archived: false, archivedAt: null, ...over });
const exam = (id: string, courseId: string, examDate: string, over: Partial<Exam> = {}): Exam => ({ id, semesterId: "s", courseId, title: id, examDate, weight: 40, preparedness: 0, location: "", ...over });

describe("examKindOf", () => {
  it("defaults to session", () => {
    expect(examKindOf(exam("e", "a", "2027-01-20"))).toBe("session");
    expect(examKindOf(exam("e", "a", "2027-01-20", { kind: "project" }))).toBe("project");
  });
});

describe("getSemesterStage", () => {
  const exams = [exam("m", "a", "2026-11-10", { kind: "midterm" }), exam("s", "a", "2027-01-20")];
  it("is lectures up to and including the end date", () => {
    expect(getSemesterStage(semester(), exams, "2026-12-25")).toBe("lectures");
  });
  it("is prep from the day after the end date until the last exam", () => {
    expect(getSemesterStage(semester(), exams, "2026-12-26")).toBe("prep");
    expect(getSemesterStage(semester(), exams, "2027-01-20")).toBe("prep");
    expect(getSemesterStage(semester(), exams, "2027-01-21")).toBe("done");
  });
  it("stays in prep after the end date when no exam follows it", () => {
    expect(getSemesterStage(semester(), [exams[0]], "2027-03-01")).toBe("prep");
  });
  it("ignores in-semester exams when finding the last exam", () => {
    expect(getLastExamDate(semester(), exams)).toBe("2027-01-20");
  });
  it("honours a manual early end", () => {
    expect(getSemesterStage(semester({ phase: "exam-prep" }), exams, "2026-10-01")).toBe("prep");
  });
  it("is lectures without an end date", () => {
    expect(getSemesterStage(semester({ endDate: null }), exams, "2027-06-01")).toBe("lectures");
  });
});

describe("isCourseActiveInPrep", () => {
  it("is true only for subjects with an exam ahead", () => {
    const exams = [exam("e", "a", "2027-01-20")];
    expect(isCourseActiveInPrep("a", exams, "2027-01-02")).toBe(true);
    expect(isCourseActiveInPrep("b", exams, "2027-01-02")).toBe(false);
    expect(isCourseActiveInPrep("a", exams, "2027-01-21")).toBe(false);
  });
});

describe("makePrepCopy", () => {
  it("keeps the total, resets progress and links back to the original", () => {
    const task: Task = { id: "t", semesterId: "s", courseId: "a", title: "Sheets", subtype: "Sheet", unitLabel: "Sheet", totalUnits: 12, completedUnits: 12, dueDate: "2026-12-01", priority: "medium", notes: "", createdAt: "" };
    const copy = makePrepCopy(task, "n", "now");
    expect(copy).toMatchObject({ id: "n", totalUnits: 12, completedUnits: 0, dueDate: null, prep: true, prepOf: "t" });
    expect(makePrepCopy(copy, "n2", "now").prepOf).toBe("t");
  });
});

describe("getRunway", () => {
  it("counts calendar and working days inclusive of both ends", () => {
    const runway = getRunway("2026-10-05", "2026-10-09")!; // Mon..Fri
    expect(runway.daysLeft).toBe(4);
    expect(runway.workingDays).toBe(5);
    expect(runway.days).toHaveLength(5);
  });
  it("skips weekends", () => {
    expect(getRunway("2026-10-09", "2026-10-12")!.workingDays).toBe(2); // Fri, Mon
  });
  it("is null after the deadline and zero on the day", () => {
    expect(getRunway("2026-10-10", "2026-10-09")).toBeNull();
    expect(getRunway("2026-10-09", "2026-10-09")!.daysLeft).toBe(0);
  });
});

describe("scheduling prep tasks past the semester end", () => {
  const ev = (taskId: string, date: string, over: Partial<TimetableEvent> = {}): TimetableEvent => ({
    id: taskId, semesterId: "s", courseId: "a", kind: "occurrence", taskId, label: taskId, date, time: "", endTime: null,
    repeatWeekly: true, recurrenceEndDate: null, occurrenceOverrides: {}, url: null, completedOccurrences: [], createdAt: "", ...over,
  });
  const events = [ev("lecture", "2026-12-07"), ev("prep", "2027-01-04")];
  const options = { prepTaskIds: new Set(["prep"]), prepEndDate: "2027-01-18" };

  it("runs prep events after the end date, up to the last exam, and stops lectures at the end date", () => {
    const occ = expandTimetableEvents(events, [], semester(), "2026-12-01", "2027-02-28", options);
    const dates = (id: string) => occ.filter((o) => o.event.taskId === id).map((o) => o.date);
    expect(dates("lecture")).toEqual(["2026-12-07", "2026-12-14", "2026-12-21"]);
    expect(dates("prep")).toEqual(["2027-01-04", "2027-01-11", "2027-01-18"]);
  });
  it("still keeps prep events when the semester is manually ended early, but hides lectures", () => {
    const occ = expandTimetableEvents(events, [], semester({ phase: "exam-prep" }), "2026-12-01", "2027-02-28", options);
    expect(new Set(occ.map((o) => o.event.taskId))).toEqual(new Set(["prep"]));
  });
  it("is unchanged without options", () => {
    expect(expandTimetableEvents(events, [], semester(), "2026-12-01", "2027-02-28").every((o) => o.event.taskId === "lecture")).toBe(true);
  });
});
