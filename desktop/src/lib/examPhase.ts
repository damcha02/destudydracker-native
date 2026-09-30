import type { Course, Exam, ExamKind, Semester, Task } from "../types";
import { parseIsoDate, toIsoDate } from "./plannerSchedule";

/**
 * Wabi-sabi exams. Every exam has a kind: midterm, endterm, semester-end and project submission
 * happen inside the semester and are one-off dated items; "session" exams fall in the exam session
 * after the semester. The semester itself has three stages that follow from its dates and exams,
 * with no manual switch: lectures -> prep (day after the end date until the last exam) -> done.
 */
export const EXAM_KINDS: { id: ExamKind; label: string }[] = [
  { id: "midterm", label: "Midterm" },
  { id: "endterm", label: "Endterm" },
  { id: "semester-end", label: "Semester-end exam" },
  { id: "project", label: "Project submission" },
  { id: "session", label: "Session exam" },
];

export function examKindLabel(kind: ExamKind): string {
  return EXAM_KINDS.find((item) => item.id === kind)?.label ?? "Exam";
}

/** Absent means "session" - what every exam was before kinds existed. */
export function examKindOf(exam: Exam): ExamKind {
  return exam.kind ?? "session";
}

/** Shouted label for the calendar: "EXAM: SEMESTER-END", "EXAM: SESSION", "MIDTERM", "ENDTERM", "PROJECT_DEADLINE". */
export function examCalendarLabel(kind: ExamKind): string {
  switch (kind) {
    case "midterm": return "MIDTERM";
    case "endterm": return "ENDTERM";
    case "project": return "PROJECT_DEADLINE";
    case "semester-end": return "EXAM: SEMESTER-END";
    default: return "EXAM: SESSION";
  }
}

export type SemesterStage = "lectures" | "prep" | "done";

function addDays(dateIso: string, days: number): string {
  const date = parseIsoDate(dateIso);
  date.setDate(date.getDate() + days);
  return toIsoDate(date);
}

export function daysBetweenIso(fromIso: string, toIso: string): number {
  return Math.round((parseIsoDate(toIso).getTime() - parseIsoDate(fromIso).getTime()) / 86400000);
}

/** The latest exam date of a semester (any kind that falls after its end), or null. */
export function getLastExamDate(semester: Semester, exams: Exam[]): string | null {
  const dates = exams
    .filter((exam) => exam.semesterId === semester.id && (!semester.endDate || exam.examDate > semester.endDate))
    .map((exam) => exam.examDate)
    .sort();
  return dates.length ? dates[dates.length - 1] : null;
}

export function getSemesterStage(semester: Semester, exams: Exam[], todayIso: string): SemesterStage {
  if (semester.phase === "exam-prep") return "prep";
  if (!semester.endDate || todayIso <= semester.endDate) return "lectures";
  const last = getLastExamDate(semester, exams);
  return last && todayIso > last ? "done" : "prep";
}

/** During prep a subject is "active" while it still has an exam or project submission ahead; the others fade into the background. */
export function isCourseActiveInPrep(courseId: string, exams: Exam[], todayIso: string): boolean {
  return exams.some((exam) => exam.courseId === courseId && exam.examDate >= todayIso);
}

export function getNextExam(exams: Exam[], courseId: string, todayIso: string): Exam | null {
  return exams
    .filter((exam) => exam.courseId === courseId && exam.examDate >= todayIso)
    .sort((a, b) => a.examDate.localeCompare(b.examDate))[0] ?? null;
}

/** Repeats a semester task for exam prep: same scope (its total), nothing done yet. */
export function makePrepCopy(task: Task, id: string, createdAt: string): Task {
  return { ...task, id, totalUnits: task.totalUnits, completedUnits: 0, dueDate: null, createdAt, prep: true, prepOf: task.prepOf ?? task.id };
}

export interface Runway {
  daysLeft: number;
  /** Mon-Fri days from today up to and including the deadline. */
  workingDays: number;
  /** Every calendar day from today to the deadline, inclusive. */
  days: string[];
}

/** The days between today and a deadline, for the "days left" highlight. Null once the deadline has passed. */
export function getRunway(todayIso: string, deadlineIso: string): Runway | null {
  const daysLeft = daysBetweenIso(todayIso, deadlineIso);
  if (daysLeft < 0) return null;
  const days: string[] = [];
  let workingDays = 0;
  for (let offset = 0; offset <= daysLeft; offset += 1) {
    const iso = addDays(todayIso, offset);
    days.push(iso);
    const weekday = parseIsoDate(iso).getDay();
    if (weekday !== 0 && weekday !== 6) workingDays += 1;
  }
  return { daysLeft, workingDays, days };
}

export function courseHasPrepWork(course: Course, tasks: Task[]): boolean {
  return tasks.some((task) => task.courseId === course.id && task.prep);
}
