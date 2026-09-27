import type { Exam, Holiday, Semester, TimetableEvent } from "../types";
import { getExamPressure } from "./metrics";
import { expandTimetableEvents } from "./plannerSchedule";

function addDaysIso(iso: string, days: number) {
  const date = new Date(`${iso}T00:00:00`);
  date.setDate(date.getDate() + days);
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

/** Only these count as a unit of work: a sheet's release is informational. */
const isCountedKind = (event: TimetableEvent) => event.kind === "occurrence" || event.kind === "sheet-deadline";

export type ScheduleHealth = { score: number; missed: number; onTime: number; dueSoon: number };

/**
 * Health measured against the calendar instead of raw completion: what should already be done
 * (every counted occurrence dated before today) versus what actually is. Being on schedule scores
 * high at any point in the semester; only missed items, sheets due within three days that are still
 * open, and imminent exams lower it. Returns null when nothing is scheduled yet, so callers can
 * fall back to the completion-based score.
 */
export function getScheduleHealth(input: {
  events: TimetableEvent[];
  holidays: Holiday[];
  semesters: Semester[];
  exams: Exam[];
  today: string;
}): ScheduleHealth | null {
  const { events, holidays, semesters, exams, today } = input;
  const counted = events.filter(isCountedKind);
  if (!counted.length) return null;

  const yesterday = addDaysIso(today, -1);
  const soonEnd = addDaysIso(today, 3);
  let missed = 0;
  let onTime = 0;
  let dueSoon = 0;
  let scheduledAtAll = false;

  for (const semester of semesters) {
    for (const occurrence of expandTimetableEvents(counted, holidays, semester, "1970-01-01", soonEnd)) {
      scheduledAtAll = true;
      const done = occurrence.event.completedOccurrences.includes(occurrence.date);
      if (occurrence.date <= yesterday) {
        if (done) onTime += 1;
        else missed += 1;
      } else if (occurrence.event.kind === "sheet-deadline" && !done) {
        dueSoon += 1;
      }
    }
  }
  if (!scheduledAtAll) return null;

  const past = missed + onTime;
  const missedRatio = past ? missed / past : 0;
  const raw = 100 - missedRatio * 70 - missed * 4 - dueSoon * 2 - getExamPressure(exams);
  return { score: Math.max(0, Math.min(100, Math.round(raw))), missed, onTime, dueSoon };
}

export function scheduleHealthLabel(score: number) {
  return score >= 75 ? "Strong" : score >= 55 ? "Steady" : score >= 35 ? "Watch" : "Critical";
}

/** Keeps a completion-based health record's shape but takes the score and label from the schedule. */
export function withScheduleHealth<T extends { score: number; label: string }>(base: T, schedule: ScheduleHealth | null): T {
  return schedule ? { ...base, score: schedule.score, label: scheduleHealthLabel(schedule.score) } : base;
}
