// Stage 17 visual-parity fixture generator.
//
// Emits a production "backup v2" JSON (the same shape `desktop/src/lib/storage.ts`'s
// `buildBackup`/`restoreBackup` use, and that the native importer already reads) filled with
// ENTIRELY SYNTHETIC data. Every date is relative to "today" (local time), so the production web
// build and the native app - both importing this same file on the same day - render the same
// Dashboard. No personal data, nothing read from any real profile.
//
//   node gen-fixture.mjs <scenario> <out.json> [--today YYYY-MM-DD]
//
// scenario: empty | small | realistic | wabi
//   wabi (Stage 19) = realistic + the planner data the Wabi-Sabi Dashboard reads: weekly lectures,
//   sheet release/deadline pairs, to-dos (timed, any-time, repeating) and an exam-prep task. The
//   three older scenarios are byte-identical to what Stage 17 committed.
import { writeFileSync } from "node:fs";

const [scenarioArg = "realistic", outPath = "fixture.json", ...rest] = process.argv.slice(2);
const wabi = scenarioArg === "wabi";
const scenario = wabi ? "realistic" : scenarioArg;
const todayArg = rest[0] === "--today" ? rest[1] : null;
// Wall-clock times are built with an explicit, fixed UTC offset (default +02:00 = Europe/Zurich in
// summer) so the committed fixtures are byte-identical on every machine and every timezone; the
// production render pins the same timezone with Emulation.setTimezoneOverride.
const tzArg = rest.indexOf("--tz") >= 0 ? rest[rest.indexOf("--tz") + 1] : "+02:00";
const tzMinutes = (tzArg[0] === "-" ? -1 : 1) * (Number(tzArg.slice(1, 3)) * 60 + Number(tzArg.slice(4, 6)));
const now = todayArg ? new Date(`${todayArg}T12:00:00${tzArg}`) : new Date();

const pad = (n) => String(n).padStart(2, "0");
// A Date whose UTC getters read as the wall clock in the fixture timezone.
const wall = (d) => new Date(d.getTime() + tzMinutes * 60000);
const iso = (d) => { const w = wall(d); return `${w.getUTCFullYear()}-${pad(w.getUTCMonth() + 1)}-${pad(w.getUTCDate())}`; };
const dayOffset = (n) => {
  const w = wall(now);
  w.setUTCDate(w.getUTCDate() + n);
  return new Date(w.getTime() - tzMinutes * 60000);
};
const at = (n, h, m) => {
  const w = wall(dayOffset(n));
  w.setUTCHours(h, m, 0, 0);
  return new Date(w.getTime() - tzMinutes * 60000);
};
// xorshift: deterministic, so two runs on the same day produce byte-identical fixtures.
let seed = 0x9e3779b9;
const rnd = () => {
  seed ^= seed << 13; seed >>>= 0;
  seed ^= seed >>> 17;
  seed ^= seed << 5; seed >>>= 0;
  return seed / 4294967296;
};

const createdAt = at(-120, 9, 0).toISOString();
const semester = (id, name, start, end, archived) => ({
  id, name, createdAt, startDate: iso(dayOffset(start)), endDate: iso(dayOffset(end)),
  phase: "semester", archived, archivedAt: archived ? at(-100, 9, 0).toISOString() : null,
});
const semesters = [];
const courses = [];
const tasks = [];
const exams = [];
const sessions = [];
const calendarEntries = [];

const palette = ["#8fb4ff", "#98c379", "#e6bd73", "#dca0ff", "#ef8f8f"];
const courseDefs = [
  ["Analysis II", 0, 5.5], ["Linear Algebra", 1, 5.0], ["Physics", 2, 4.5],
  ["Programming", 3, 6.0], ["Statistics", 4, 5.0],
];

if (scenario !== "empty") {
  semesters.push(semester("sem-active", "Autumn Semester", -40, 80, false));
  if (scenario === "realistic") semesters.push(semester("sem-old", "Spring Semester", -230, -100, true));
  const n = scenario === "small" ? 1 : 5;
  courseDefs.slice(0, n).forEach(([name, ci, target], i) => {
    courses.push({ id: `course-${i}`, semesterId: "sem-active", name, color: palette[ci], targetGrade: target, createdAt, externalUrl: null });
  });
  if (scenario === "realistic") {
    courses.push({ id: "course-old", semesterId: "sem-old", name: "Archived Course", color: "#b0b0b0", targetGrade: 5, createdAt, externalUrl: null });
  }
  courses.forEach((c, i) => {
    const count = c.id === "course-old" ? 2 : 3;
    for (let k = 0; k < count; k++) {
      const total = 6 + k * 3;
      tasks.push({
        id: `task-${i}-${k}`, semesterId: c.semesterId, courseId: c.id,
        title: ["Exercise Sheet", "Lecture Notes", "Reading"][k] + ` ${k + 1}`,
        subtype: ["Sheet", "Lecture", "Other"][k], unitLabel: ["Sheet", "Lecture", "Chapter"][k],
        totalUnits: total, completedUnits: Math.min(total, 2 + k * 2 + i), dueDate: iso(dayOffset(3 + i * 4 + k * 5 - (i === 2 && k === 0 ? 6 : 0))),
        priority: ["high", "medium", "low"][k], notes: "", createdAt,
      });
    }
  });
  // Exams: three upcoming, one already past (must be excluded from the runway).
  const examDefs = scenario === "small"
    ? [["Midterm", 0, 18, 40, 35]]
    : [["Midterm", 0, 9, 40, 35], ["Final", 1, 24, 60, 60], ["Lab Exam", 2, 41, 30, 82], ["Old Quiz", 3, -6, 10, 100]];
  examDefs.forEach(([title, ci, off, weight, prep], i) => {
    exams.push({ id: `exam-${i}`, semesterId: "sem-active", courseId: `course-${ci}`, title: `${title}`, examDate: iso(dayOffset(off)), weight, preparedness: prep, location: "Room 101" });
  });
  // Planned today: a mix of completed / open / timed.
  const planned = scenario === "small" ? 1 : 4;
  for (let i = 0; i < planned; i++) {
    calendarEntries.push({
      id: `cal-${i}`, taskId: `task-${i % courses.filter((c) => c.semesterId === "sem-active").length}-${i % 3}`, date: iso(dayOffset(0)),
      unitAmount: [1, 0.5, 1, 0.25][i], completed: i === 2, completedAt: i === 2 ? at(0, 8, 30).toISOString() : null,
      createdAt, startTime: i === 3 ? null : `${pad(9 + i * 2)}:00`, endTime: i === 3 ? null : `${pad(10 + i * 2)}:00`,
    });
  }
}

const addSession = (off, h, m, minutes, courseId, kind = "study") => {
  const start = at(off, h, m);
  const end = new Date(start.getTime() + minutes * 60000);
  sessions.push({
    id: `session-${sessions.length}`, semesterId: courseId ? "sem-active" : null, courseId, taskId: null, kind,
    goal: "Synthetic focus block", learned: "", blocker: "", nextStep: "", confidence: 3,
    startedAt: start.toISOString(), endedAt: end.toISOString(), minutes, presetLabel: "Deep Work",
  });
};

if (scenario === "small") {
  addSession(0, 10, 0, 45, "course-0");
} else if (scenario === "realistic") {
  // 70 days of history. Streak = today + 8 previous days (offsets 0..-8), a gap at -9, then
  // irregular activity, so the "current streak" is not simply "the whole history".
  for (let off = 0; off >= -69; off--) {
    if (off === -9 || off === -10) continue;
    if (off < -9 && rnd() < 0.4) continue;
    const blocks = 1 + Math.floor(rnd() * 3);
    for (let b = 0; b < blocks; b++) {
      const ci = Math.floor(rnd() * 5);
      addSession(off, 8 + b * 3, 0, 20 + Math.floor(rnd() * 70), `course-${ci}`);
    }
  }
  addSession(-2, 21, 0, 30, null);                // General (no course)
  addSession(-3, 20, 0, 40, "course-deleted-gone"); // dangling course reference
  addSession(-1, 7, 0, 55, "course-1", "exam");
}

const timetableEvents = [];
const dailyTodos = [];
if (wabi) {
  const event = (id, courseIndex, kind, taskId, label, dateOff, time, endTime, repeatWeekly, completed = []) => ({
    id, semesterId: "sem-active", courseId: `course-${courseIndex}`, kind, taskId, label,
    date: iso(dayOffset(dateOff)), time, endTime, repeatWeekly, recurrenceEndDate: null,
    occurrenceOverrides: {}, url: null, completedOccurrences: completed.map((o) => iso(dayOffset(o))), createdAt,
  });
  // Weekly lectures anchored on today's weekday (so one occurs today); one already ticked today.
  timetableEvents.push(event("ev-lec-0", 0, "occurrence", "task-0-1", "Lecture Notes 1", -35, "10:15", "12:00", true));
  timetableEvents.push(event("ev-lec-1", 1, "occurrence", "task-1-1", "Lecture Notes 2", -35, "14:15", "16:00", true, [0]));
  // Sheet series: released a week before it is due. Two are out already, one is not released yet.
  timetableEvents.push(event("ev-rel-0", 0, "sheet-release", "task-0-0", "Exercise Sheet 1", -4, "08:00", null, false));
  timetableEvents.push(event("ev-due-0", 0, "sheet-deadline", "task-0-0", "Exercise Sheet 1", 3, "23:59", null, false));
  timetableEvents.push(event("ev-rel-1", 3, "sheet-release", "task-3-0", "Exercise Sheet 1", -7, "08:00", null, false));
  timetableEvents.push(event("ev-due-1", 3, "sheet-deadline", "task-3-0", "Exercise Sheet 1", 0, "18:00", null, false));
  timetableEvents.push(event("ev-rel-2", 4, "sheet-release", "task-4-0", "Exercise Sheet 1", 2, "08:00", null, false));
  timetableEvents.push(event("ev-due-2", 4, "sheet-deadline", "task-4-0", "Exercise Sheet 1", 9, "12:00", null, false));
  timetableEvents.push(event("ev-due-3", 2, "sheet-deadline", "task-2-0", "Exercise Sheet 1", 12, null, null, false));
  const todo = (id, title, dateOff, time, endTime, completed, repeatWeekly = false, completedOccurrences = []) => ({
    id, date: iso(dayOffset(dateOff)), time, endTime, title, notes: "", completed,
    completedAt: completed ? at(0, 8, 0).toISOString() : null, createdAt, repeatWeekly,
    completedOccurrences: completedOccurrences.map((o) => iso(dayOffset(o))), recurrenceEndDate: null,
    skippedOccurrences: [], occurrenceTimes: {},
  });
  dailyTodos.push(todo("todo-0", "Email the tutor", 0, "09:30", null, false));
  dailyTodos.push(todo("todo-1", "Library books back", 0, null, null, true));
  dailyTodos.push(todo("todo-2", "Weekly review", -14, "17:00", "17:30", false, true));
  dailyTodos.push(todo("todo-3", "Buy notebook", 1, null, null, false));
  tasks.push({
    id: "task-prep-0", semesterId: "sem-active", courseId: "course-1", title: "Exercise Sheet 1", subtype: "Sheet", unitLabel: "Sheet",
    totalUnits: 6, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt, prep: true, prepOf: "task-1-0",
  });
  exams.forEach((exam, i) => { exam.kind = ["midterm", "session", "project", "endterm"][i]; });
}

const lifetimeStudyMinutes = sessions.reduce((s, x) => s + x.minutes, 0);
const state = {
  semesters, courses, tasks, exams, sessions,
  timetableEvents, holidays: [], dailyTodos, calendarEntries,
  lifetimeStudyMinutes, lifetimeStudySessions: sessions.length,
  activeTab: "dashboard",
  settings: { accent: "#8fb4ff", userName: "", dailyGoalMinutes: 120, telemetryEnabled: false },
  social: { userId: "fixture-user-id-not-real", deviceSecret: "fixture-device-secret-not-real", friendCode: "FIX-0000" },
  timer: null,
};
delete state.timer;
const backup = { app: "study-tracker", backupVersion: 2, exportedAt: new Date(now).toISOString(), state, preferences: {} };
writeFileSync(outPath, JSON.stringify(backup, null, 2));
console.log(`${scenarioArg}: ${sessions.length} sessions, ${courses.length} courses, ${lifetimeStudyMinutes} min -> ${outPath} (today=${iso(now)})`);
