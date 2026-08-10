import type { AppState, Course, Exam, Semester, StudySession, Task } from "../types";

export function clamp(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

export function formatMinutes(minutes: number) {
  if (!minutes) return "0m";
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return mins ? `${hours}h ${mins}m` : `${hours}h`;
}

export function formatDate(date: string) {
  return new Intl.DateTimeFormat("en", {
    month: "short",
    day: "numeric",
  }).format(new Date(date));
}

export function isoDate(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function sessionDateKey(session: StudySession) {
  return isoDate(new Date(session.endedAt));
}

export function daysUntil(date: string) {
  const target = new Date(date);
  target.setHours(0, 0, 0, 0);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return Math.round((target.getTime() - today.getTime()) / 86400000);
}

export function getTaskProgress(task: Task) {
  const total = Math.max(task.totalUnits, 1);
  return Math.round((task.completedUnits / total) * 100);
}

export function getSemesterCourses(state: AppState, semesterId: string) {
  return state.courses.filter((course) => course.semesterId === semesterId);
}

export function getCourseTasks(state: AppState, courseId: string) {
  return state.tasks.filter((task) => task.courseId === courseId);
}

export function getCourseExams(state: AppState, courseId: string) {
  return state.exams.filter((exam) => exam.courseId === courseId);
}

export function getSemesterTasks(state: AppState, semesterId: string) {
  return state.tasks.filter((task) => task.semesterId === semesterId);
}

export function getCourseMinutes(state: AppState, courseId: string) {
  return state.sessions
    .filter((session) => session.courseId === courseId)
    .reduce((sum, session) => sum + session.minutes, 0);
}

export function getRemainingUnits(task: Task) {
  return Math.max(0, task.totalUnits - task.completedUnits);
}

export function calculateDailyWork(task: Task) {
  const remaining = getRemainingUnits(task);
  if (remaining <= 0) {
    return { unitsPerDay: 0, daysLeft: 0, message: "Finished. Keep this as revision only." };
  }

  if (!task.dueDate) {
    return {
      unitsPerDay: remaining,
      daysLeft: null,
      message: `${remaining} units left. Add a due date for a realistic daily target.`,
    };
  }

  const daysLeft = daysUntil(task.dueDate);
  if (daysLeft <= 0) {
    return {
      unitsPerDay: remaining,
      daysLeft,
      message: `Due now. You need to clear ${remaining} units today.`,
    };
  }

  const unitsPerDay = remaining / daysLeft;
  return {
    unitsPerDay,
    daysLeft,
    message: `${unitsPerDay.toFixed(1)} units per day keeps this on track.`,
  };
}

export function calculateAggregateWorkload(tasks: Task[]) {
  const totalUnits = tasks.reduce((sum, task) => sum + Math.max(task.totalUnits, 1), 0);
  const completedUnits = tasks.reduce(
    (sum, task) => sum + clamp(task.completedUnits, 0, task.totalUnits),
    0,
  );
  const remainingUnits = Math.max(0, totalUnits - completedUnits);
  const unfinishedTasks = tasks.filter((task) => getRemainingUnits(task) > 0);
  const datedTasks = unfinishedTasks.filter((task) => task.dueDate);
  const nearestDueDate = getNearestDeadline(tasks);
  const daysLeft = nearestDueDate ? daysUntil(nearestDueDate) : null;
  const undatedRemainingUnits = unfinishedTasks
    .filter((task) => !task.dueDate)
    .reduce((sum, task) => sum + getRemainingUnits(task), 0);

  if (remainingUnits <= 0) {
    return {
      totalUnits,
      completedUnits,
      remainingUnits,
      progress: totalUnits ? Math.round((completedUnits / totalUnits) * 100) : 0,
      unitsPerDay: 0,
      daysLeft: 0,
      nearestDueDate,
      undatedRemainingUnits,
      message: "Everything tracked is complete. Use the timer for revision or new work.",
    };
  }

  if (!datedTasks.length) {
    return {
      totalUnits,
      completedUnits,
      remainingUnits,
      progress: totalUnits ? Math.round((completedUnits / totalUnits) * 100) : 0,
      unitsPerDay: remainingUnits,
      daysLeft,
      nearestDueDate,
      undatedRemainingUnits,
      message: "Add due dates to get a realistic overall pace.",
    };
  }

  const unitsPerDay = datedTasks.reduce((sum, task) => {
    const dueIn = daysUntil(task.dueDate ?? "");
    const remaining = getRemainingUnits(task);
    return sum + (dueIn <= 0 ? remaining : remaining / dueIn);
  }, 0);
  const deadlineCount = new Set(datedTasks.map((task) => task.dueDate)).size;
  const deadlineLabel = deadlineCount === 1 ? "deadline" : "deadlines";
  const undatedMessage = undatedRemainingUnits
    ? ` ${undatedRemainingUnits} undated units are not included in this pace.`
    : "";

  return {
    totalUnits,
    completedUnits,
    remainingUnits,
    progress: totalUnits ? Math.round((completedUnits / totalUnits) * 100) : 0,
    unitsPerDay,
    daysLeft,
    nearestDueDate,
    undatedRemainingUnits,
    message: `${unitsPerDay.toFixed(1)} units/day across ${deadlineCount} ${deadlineLabel}.${undatedMessage}`,
  };
}

function getOverdueTasks(tasks: Task[]) {
  return tasks.filter((task) => task.dueDate && daysUntil(task.dueDate) < 0 && getRemainingUnits(task) > 0);
}

function getDueSoonTasks(tasks: Task[]) {
  return tasks.filter((task) => {
    if (!task.dueDate || getRemainingUnits(task) <= 0) return false;
    const dueIn = daysUntil(task.dueDate);
    return dueIn >= 0 && dueIn <= 3;
  });
}

function getExamPressure(exams: Exam[]) {
  return exams.reduce((penalty, exam) => {
    const dueIn = daysUntil(exam.examDate);
    if (dueIn < 0 || dueIn > 10) return penalty;
    const urgency = (10 - dueIn) / 10;
    return penalty + urgency * (100 - exam.preparedness) * 0.22;
  }, 0);
}

function getNearestDeadline(tasks: Task[]) {
  return tasks
    .filter((task) => task.dueDate && getRemainingUnits(task) > 0)
    .sort((a, b) => daysUntil(a.dueDate ?? "") - daysUntil(b.dueDate ?? ""))[0]?.dueDate ?? null;
}

function getUnitsPerDay(tasks: Task[]) {
  return calculateAggregateWorkload(tasks).unitsPerDay;
}

function getPacePenalty(unitsPerDay: number) {
  return clamp((unitsPerDay - 1) * 10, 0, 45);
}

function getHealthLabel(score: number) {
  let label = "Strong";
  if (score < 75) label = "Steady";
  if (score < 55) label = "Watch";
  if (score < 35) label = "Critical";
  return label;
}

function calculateWorkloadHealth(tasks: Task[], exams: Exam[]) {
  const totalUnits = tasks.reduce((sum, task) => sum + Math.max(task.totalUnits, 1), 0);
  const finishedUnits = tasks.reduce((sum, task) => sum + clamp(task.completedUnits, 0, task.totalUnits), 0);

  if (!totalUnits) {
    return { score: 0, label: "Preparing", overdue: 0, dueSoon: 0, unitsPerDay: 0 };
  }

  const progressPercent = (finishedUnits / totalUnits) * 100;
  const unitsPerDay = getUnitsPerDay(tasks);
  const manageabilityScore = 100 - getPacePenalty(unitsPerDay);
  const overdueTasks = getOverdueTasks(tasks);
  const dueSoonTasks = getDueSoonTasks(tasks);
  const overduePenalty = overdueTasks.length * 12;
  const dueSoonPenalty = dueSoonTasks.length * 2;
  const examPenalty = getExamPressure(exams);
  const score = clamp(
    Math.round(progressPercent * 0.65 + manageabilityScore * 0.35 - overduePenalty - dueSoonPenalty - examPenalty),
    0,
    100,
  );

  return {
    score,
    label: getHealthLabel(score),
    overdue: overdueTasks.length,
    dueSoon: dueSoonTasks.length,
    unitsPerDay,
  };
}

export function getCourseHealth(state: AppState, course: Course) {
  const tasks = getCourseTasks(state, course.id);
  const exams = getCourseExams(state, course.id);
  return calculateWorkloadHealth(tasks, exams);
}

export function getSemesterHealth(state: AppState, semester: Semester) {
  const tasks = getSemesterTasks(state, semester.id);
  const exams = state.exams.filter((exam) => exam.semesterId === semester.id);
  return calculateWorkloadHealth(tasks, exams);
}

/** Single O(tasks) pass instead of the O(courses * tasks) cost of calling getCourseTasks per course. */
export function groupTasksByCourseId(tasks: Task[]): Map<string, Task[]> {
  const map = new Map<string, Task[]>();
  for (const task of tasks) {
    const list = map.get(task.courseId);
    if (list) list.push(task);
    else map.set(task.courseId, [task]);
  }
  return map;
}

/** Single O(tasks) pass instead of the O(semesters * tasks) cost of calling getSemesterTasks per semester. */
export function groupTasksBySemesterId(tasks: Task[]): Map<string, Task[]> {
  const map = new Map<string, Task[]>();
  for (const task of tasks) {
    const list = map.get(task.semesterId);
    if (list) list.push(task);
    else map.set(task.semesterId, [task]);
  }
  return map;
}

/** Single O(sessions) pass instead of the O(courses * sessions) cost of calling getCourseMinutes per course. */
export function getCourseMinutesMap(sessions: StudySession[]): Map<string, number> {
  const map = new Map<string, number>();
  for (const session of sessions) {
    if (!session.courseId) continue;
    map.set(session.courseId, (map.get(session.courseId) ?? 0) + session.minutes);
  }
  return map;
}

/**
 * `tasksByCourseId` should come from groupTasksByCourseId(state.tasks) - passing it in rather
 * than re-deriving it here lets a caller share one grouping pass across this and other
 * consumers (e.g. a course-tasks list needed for its own display, not just health).
 */
export function getCourseHealthMap(courses: Course[], tasksByCourseId: Map<string, Task[]>, exams: Exam[]): Map<string, ReturnType<typeof calculateWorkloadHealth>> {
  const map = new Map<string, ReturnType<typeof calculateWorkloadHealth>>();
  for (const course of courses) {
    const courseExams = exams.filter((exam) => exam.courseId === course.id);
    map.set(course.id, calculateWorkloadHealth(tasksByCourseId.get(course.id) ?? [], courseExams));
  }
  return map;
}

/** `tasksBySemesterId` should come from groupTasksBySemesterId(state.tasks) - see getCourseHealthMap. */
export function getSemesterHealthMap(semesters: Semester[], tasksBySemesterId: Map<string, Task[]>, exams: Exam[]): Map<string, ReturnType<typeof calculateWorkloadHealth>> {
  const map = new Map<string, ReturnType<typeof calculateWorkloadHealth>>();
  for (const semester of semesters) {
    const semesterExams = exams.filter((exam) => exam.semesterId === semester.id);
    map.set(semester.id, calculateWorkloadHealth(tasksBySemesterId.get(semester.id) ?? [], semesterExams));
  }
  return map;
}

export function getOverallHealth(state: AppState) {
  if (!state.tasks.length) return 0;
  return calculateWorkloadHealth(state.tasks, state.exams).score;
}

export function getWeeklyActivity(sessions: StudySession[], today = new Date()) {
  const buckets = Array.from({ length: 7 }, (_, index) => {
    const date = new Date(today);
    date.setDate(today.getDate() - (6 - index));
    return {
      key: isoDate(date),
      label: new Intl.DateTimeFormat("en", { weekday: "short" }).format(date),
      minutes: 0,
    };
  });

  const map = new Map(buckets.map((bucket) => [bucket.key, bucket]));
  sessions.forEach((session) => {
    const key = sessionDateKey(session);
    const bucket = map.get(key);
    if (bucket) bucket.minutes += session.minutes;
  });
  return buckets;
}

export function getTopPendingTasks(state: AppState) {
  return [...state.tasks]
    .filter((task) => getRemainingUnits(task) > 0)
    .sort((a, b) => {
      const aDue = a.dueDate ? daysUntil(a.dueDate) : 999;
      const bDue = b.dueDate ? daysUntil(b.dueDate) : 999;
      if (aDue !== bDue) return aDue - bDue;
      const priorityWeight = { high: 0, medium: 1, low: 2 };
      return priorityWeight[a.priority] - priorityWeight[b.priority];
    })
    .slice(0, 5);
}

export function getUpcomingExams(state: AppState) {
  return [...state.exams]
    .filter((exam) => daysUntil(exam.examDate) >= 0)
    .sort((a, b) => daysUntil(a.examDate) - daysUntil(b.examDate))
    .slice(0, 4);
}

export function getTodaySessions(state: AppState, today = isoDate()) {
  return state.sessions.filter((session) => sessionDateKey(session) === today);
}

export function getTodayMinutes(state: AppState, today = isoDate()) {
  return getTodaySessions(state, today).reduce((sum, session) => sum + session.minutes, 0);
}

export function getUnitsCompletedToday(state: AppState, today = isoDate()) {
  return state.calendarEntries
    .filter((entry) => entry.completedAt && isoDate(new Date(entry.completedAt)) === today)
    .reduce((sum, entry) => sum + (entry.unitAmount ?? 1), 0);
}

export function getStreakDays(state: AppState, today = isoDate()) {
  const sessionDays = new Set(state.sessions.map(sessionDateKey));
  let streak = 0;
  const cursor = new Date(`${today}T00:00:00`);

  if (!sessionDays.has(isoDate(cursor))) {
    cursor.setDate(cursor.getDate() - 1);
  }

  while (sessionDays.has(isoDate(cursor))) {
    streak += 1;
    cursor.setDate(cursor.getDate() - 1);
  }
  return streak;
}

export function getFocusMomentum(state: AppState, today = isoDate()) {
  const week = getWeeklyActivity(state.sessions, new Date(`${today}T00:00:00`));
  const total = week.reduce((sum, day) => sum + day.minutes, 0);
  const average = total / week.length;
  const streak = getStreakDays(state, today);

  if (streak >= 5 && average >= 35) return "Surging";
  if (streak >= 5) return "Stable";
  if (streak >= 3) return "Recovering";
  if (average >= 110) return "Surging";
  if (average >= 70) return "Stable";
  if (average >= 35) return "Recovering";
  return "Slipping";
}

export function getSessionDaySet(sessions: StudySession[]): Set<string> {
  return new Set(sessions.map(sessionDateKey));
}

export function getFirstSessionDate(sessions: StudySession[]): string | null {
  let earliest: StudySession | null = null;
  let earliestMs = Infinity;
  for (const session of sessions) {
    const ms = new Date(session.startedAt).getTime();
    if (ms < earliestMs) {
      earliestMs = ms;
      earliest = session;
    }
  }
  return earliest ? earliest.startedAt : null;
}

export function getStudySessionCount(sessions: StudySession[]): number {
  return sessions.filter((session) => session.kind === "study" || session.kind === "exam").length;
}

/** Distinct courses with a study/exam session in the 7-day window ending `today`. */
export function getWeeklyCourseCount(sessions: StudySession[], today = isoDate()): number {
  const cutoff = new Date(`${today}T00:00:00`).getTime() - 6 * 86400000;
  const ids = new Set(
    sessions
      .filter((session) => (session.kind === "study" || session.kind === "exam") && session.courseId)
      .filter((session) => new Date(`${(session.endedAt || session.startedAt).slice(0, 10)}T00:00:00`).getTime() >= cutoff)
      .map((session) => session.courseId),
  );
  return ids.size;
}

/** `now` defaults to `new Date()` so the only behavior change from memoizing the caller is *when* this reads wall-clock time, not what it computes. */
export function getLocalMonthlyStats(sessions: StudySession[], now: Date = new Date()): { minutes: number; sessions: number } {
  const monthStart = new Date(now);
  monthStart.setDate(1);
  monthStart.setHours(0, 0, 0, 0);
  return sessions
    .filter((session) => (session.kind === "study" || session.kind === "exam") && new Date(session.endedAt) >= monthStart)
    .reduce((acc, session) => ({ minutes: acc.minutes + Math.max(0, Math.round(session.minutes)), sessions: acc.sessions + 1 }), { minutes: 0, sessions: 0 });
}

export function buildDailyNoteMarkdown(state: AppState, noteDate = isoDate()) {
  const courseLookup = new Map(state.courses.map((course) => [course.id, course]));
  const sessions = state.sessions.filter((session) => sessionDateKey(session) === noteDate);
  const totalMinutes = sessions.reduce((sum, session) => sum + session.minutes, 0);
  const learned = sessions.map((session) => session.learned).filter(Boolean);
  const blockers = sessions.map((session) => session.blocker).filter(Boolean);
  const nextSteps = sessions.map((session) => session.nextStep).filter(Boolean);

  return `# ${noteDate}

## Study Summary
- Total focused time: ${formatMinutes(totalMinutes)}
- Sessions logged: ${sessions.length}

## What I Studied Today
${sessions.length ? sessions.map((session) => `- ${courseLookup.get(session.courseId ?? "")?.name ?? "General"}: ${session.goal || "Focused work"} (${session.minutes} min)`).join("\n") : "- No sessions logged yet."}

## What I Learned
${learned.length ? learned.map((item) => `- ${item}`).join("\n") : "- Add a quick reflection after each session."}

## What Is Still Weak
${blockers.length ? blockers.map((item) => `- ${item}`).join("\n") : "- Nothing logged yet."}

## What To Do Next
${nextSteps.length ? nextSteps.map((item) => `- ${item}`).join("\n") : "- Pick the next highest-impact task."}
`;
}
