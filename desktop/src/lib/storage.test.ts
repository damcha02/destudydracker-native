import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  APP_STATE_STORAGE_KEYS,
  applyPersistedSections,
  createInitialPersistenceBaselines,
  defaultState,
  defaultTimer,
  getChangedSections,
  buildBackup,
  loadAppState,
  restoreBackup,
  saveAppState,
} from "./storage";
import type { AppState, TimerState } from "../types";

// APP_STATE_STORAGE_KEYS is [v2, v1, v3-timer, v3-social, v3-core], in that order - reused here
// instead of hardcoding the literal strings, so the test can't silently drift from storage.ts.
const [V2_KEY, , TIMER_KEY, SOCIAL_KEY, CORE_KEY] = APP_STATE_STORAGE_KEYS;

interface FailableStorage extends Storage {
  /**
   * Makes the next `times` setItem calls for this key throw, then behave normally again.
   * The social-key write path retries once internally (a quota-exceeded fallback that strips
   * avatar payloads and writes again) - simulating a genuine social-write failure requires
   * failing both that primary attempt and the fallback attempt, i.e. `times: 2`.
   */
  failNextWriteFor(key: string, times?: number): void;
}

function createMemoryLocalStorage(): FailableStorage {
  const map = new Map<string, string>();
  const failing = new Map<string, number>();
  return {
    getItem: (key) => (map.has(key) ? (map.get(key) as string) : null),
    setItem: (key, value) => {
      const remaining = failing.get(key) ?? 0;
      if (remaining > 0) {
        failing.set(key, remaining - 1);
        throw new Error(`Simulated write failure for ${key}`);
      }
      map.set(key, value);
    },
    removeItem: (key) => { map.delete(key); },
    clear: () => map.clear(),
    key: (index) => [...map.keys()][index] ?? null,
    get length() { return map.size; },
    failNextWriteFor: (key: string, times = 1) => { failing.set(key, (failing.get(key) ?? 0) + times); },
  } as FailableStorage;
}

function runningCountdown(overrides: Partial<TimerState> = {}): TimerState {
  const startedAt = "2026-08-01T12:00:00.000Z";
  return {
    ...defaultTimer,
    phase: "study",
    running: true,
    startedAt,
    endsAt: "2026-08-01T12:25:00.000Z",
    remainingSeconds: 25 * 60,
    activeSegments: [{ startedAt, endedAt: null }],
    ...overrides,
  };
}

let storage: FailableStorage;

beforeEach(() => {
  storage = createMemoryLocalStorage();
  vi.stubGlobal("localStorage", storage);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.useRealTimers();
});

describe("getChangedSections", () => {
  it("treats every section as changed when there is no baseline yet", () => {
    expect(getChangedSections(defaultState, { timer: null, social: null, core: null })).toEqual(new Set(["timer", "social", "core"]));
  });

  it("detects only the section whose reference actually changed", () => {
    const baselines = createInitialPersistenceBaselines(defaultState);
    const withNewTimer = { ...defaultState, timer: { ...defaultState.timer, studyMinutes: 30 } };
    expect(getChangedSections(withNewTimer, baselines)).toEqual(new Set(["timer"]));

    const withNewSocial = { ...defaultState, social: { ...defaultState.social, displayName: "Ada" } };
    expect(getChangedSections(withNewSocial, baselines)).toEqual(new Set(["social"]));

    const withNewCore = { ...defaultState, waterGlasses: 3 };
    expect(getChangedSections(withNewCore, baselines)).toEqual(new Set(["core"]));
  });

  it("reports nothing changed when every section's reference is identical to the baseline", () => {
    const baselines = createInitialPersistenceBaselines(defaultState);
    expect(getChangedSections(defaultState, baselines)).toEqual(new Set());
  });
});

describe("applyPersistedSections", () => {
  it("advances only the baselines for sections that succeeded, leaving failed ones stale", () => {
    const baselines = createInitialPersistenceBaselines(defaultState);
    const next = { ...defaultState, waterGlasses: 5, timer: { ...defaultState.timer, studyMinutes: 30 } };
    const updated = applyPersistedSections(baselines, next, new Set(["timer"]));
    expect(updated.timer).toBe(next.timer);
    expect(updated.social).toBe(baselines.social); // social unchanged - not in succeeded set
    expect(updated.core).toBe(baselines.core); // core write "failed" (not in succeeded set) - baseline stays stale
  });
});

describe("saveAppState - section-aware writes", () => {
  it("first save writes all three sections and completes migration (removes v2)", () => {
    storage.setItem(V2_KEY, JSON.stringify(defaultState));
    const baselines = createInitialPersistenceBaselines(defaultState);

    const succeeded = saveAppState(defaultState, baselines);

    expect(succeeded).toEqual(new Set(["timer", "social", "core"]));
    expect(storage.getItem(TIMER_KEY)).not.toBeNull();
    expect(storage.getItem(SOCIAL_KEY)).not.toBeNull();
    expect(storage.getItem(CORE_KEY)).not.toBeNull();
    expect(storage.getItem(V2_KEY)).toBeNull();
  });

  it("during 5 minutes of an untouched running timer, only the timer section is written on each heartbeat tick", () => {
    const state: AppState = { ...defaultState, timer: runningCountdown() };
    let baselines = createInitialPersistenceBaselines(state);

    // First save completes migration (all 3 sections, matches real app behavior on first launch).
    let succeeded = saveAppState(state, baselines);
    baselines = applyPersistedSections(baselines, state, succeeded);
    expect(succeeded).toEqual(new Set(["timer", "social", "core"]));

    // 10 heartbeat ticks (30s * 10 = 5 minutes), state reference never changes because Phase 1
    // no longer writes remainingSeconds on steady ticks.
    for (let tick = 0; tick < 10; tick++) {
      succeeded = saveAppState(state, baselines, { forceSections: new Set(["timer"]) });
      expect(succeeded).toEqual(new Set(["timer"]));
      baselines = applyPersistedSections(baselines, state, succeeded);
    }
  });

  it("writes only the social section after a social sync", () => {
    const state: AppState = { ...defaultState, timer: runningCountdown() };
    let baselines = createInitialPersistenceBaselines(state);
    baselines = applyPersistedSections(baselines, state, saveAppState(state, baselines));

    const afterSync: AppState = { ...state, social: { ...state.social, displayName: "Ada" } };
    const succeeded = saveAppState(afterSync, baselines);

    expect(succeeded).toEqual(new Set(["social"]));
  });

  it("writes only the core section after editing a task", () => {
    const state: AppState = { ...defaultState, timer: runningCountdown() };
    let baselines = createInitialPersistenceBaselines(state);
    baselines = applyPersistedSections(baselines, state, saveAppState(state, baselines));

    const afterEdit: AppState = { ...state, tasks: [{ id: "t1", semesterId: "s", courseId: "c", title: "Read", subtype: "Other", unitLabel: "Unit", totalUnits: 1, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-08-01T00:00:00.000Z" }] };
    const succeeded = saveAppState(afterEdit, baselines);

    expect(succeeded).toEqual(new Set(["core"]));
  });

  it("persists a freshly-derived remainingSeconds for a running countdown, not the stale in-memory value", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-01T12:10:00.000Z")); // 10 minutes into a 25-minute countdown
    const state: AppState = { ...defaultState, timer: runningCountdown() };

    saveAppState(state, createInitialPersistenceBaselines(defaultState));

    const persisted = JSON.parse(storage.getItem(TIMER_KEY) as string) as { timer: TimerState };
    expect(persisted.timer.remainingSeconds).toBe(15 * 60);
  });

  it("persists freshly-derived elapsed seconds for a running endless timer", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-08-01T12:05:00.000Z"));
    const startedAt = "2026-08-01T12:00:00.000Z";
    const state: AppState = {
      ...defaultState,
      timer: { ...defaultTimer, mode: "endless", phase: "stopwatch", running: true, startedAt, endsAt: null, remainingSeconds: 0, activeSegments: [{ startedAt, endedAt: null }] },
    };

    saveAppState(state, createInitialPersistenceBaselines(defaultState));

    const persisted = JSON.parse(storage.getItem(TIMER_KEY) as string) as { timer: TimerState };
    expect(persisted.timer.remainingSeconds).toBe(300);
  });

  it("leaves remainingSeconds untouched when the timer is not running", () => {
    const state: AppState = { ...defaultState, timer: { ...defaultTimer, running: false, remainingSeconds: 742 } };
    saveAppState(state, createInitialPersistenceBaselines(defaultState));
    const persisted = JSON.parse(storage.getItem(TIMER_KEY) as string) as { timer: TimerState };
    expect(persisted.timer.remainingSeconds).toBe(742);
  });
});

describe("saveAppState - retry on failure", () => {
  it("1. core write fails once -> next save retries core", () => {
    const state: AppState = { ...defaultState, waterGlasses: 3 };
    let baselines = createInitialPersistenceBaselines(defaultState);

    storage.failNextWriteFor(CORE_KEY);
    const first = saveAppState(state, baselines);
    expect(first.has("core")).toBe(false);
    expect(first.has("timer")).toBe(true);
    expect(first.has("social")).toBe(true);
    baselines = applyPersistedSections(baselines, state, first);

    const second = saveAppState(state, baselines);
    expect(second.has("core")).toBe(true);
  });

  it("2. social write fails once -> next save retries social", () => {
    const state: AppState = { ...defaultState, social: { ...defaultState.social, displayName: "Ada" } };
    let baselines = createInitialPersistenceBaselines(defaultState);

    storage.failNextWriteFor(SOCIAL_KEY, 2); // primary attempt + the avatar-stripped fallback attempt
    const first = saveAppState(state, baselines);
    expect(first.has("social")).toBe(false);
    baselines = applyPersistedSections(baselines, state, first);

    const second = saveAppState(state, baselines);
    expect(second.has("social")).toBe(true);
  });

  it("3. timer write fails once -> next heartbeat retries timer", () => {
    const state: AppState = { ...defaultState, timer: runningCountdown() };
    let baselines = createInitialPersistenceBaselines(state);
    baselines = applyPersistedSections(baselines, state, saveAppState(state, baselines)); // establish migration

    storage.failNextWriteFor(TIMER_KEY);
    const heartbeat1 = saveAppState(state, baselines, { forceSections: new Set(["timer"]) });
    expect(heartbeat1.has("timer")).toBe(false);
    baselines = applyPersistedSections(baselines, state, heartbeat1);

    const heartbeat2 = saveAppState(state, baselines, { forceSections: new Set(["timer"]) });
    expect(heartbeat2.has("timer")).toBe(true);
  });

  it("4. one section fails while two succeed -> only the failed section remains dirty", () => {
    const state: AppState = { ...defaultState, waterGlasses: 3, social: { ...defaultState.social, displayName: "Ada" } };
    let baselines = createInitialPersistenceBaselines(defaultState);

    storage.failNextWriteFor(SOCIAL_KEY, 2); // primary attempt + the avatar-stripped fallback attempt
    const result = saveAppState(state, baselines);
    expect(result).toEqual(new Set(["timer", "core"]));
    baselines = applyPersistedSections(baselines, state, result);

    expect(getChangedSections(state, baselines)).toEqual(new Set(["social"]));
  });

  it("5. v2 is retained after a partial migration failure and deleted only once all three v3 sections have succeeded", () => {
    storage.setItem(V2_KEY, JSON.stringify(defaultState));
    let baselines = createInitialPersistenceBaselines(defaultState);

    storage.failNextWriteFor(CORE_KEY);
    const attempt1 = saveAppState(defaultState, baselines);
    expect(attempt1.has("core")).toBe(false);
    expect(storage.getItem(V2_KEY)).not.toBeNull();
    baselines = applyPersistedSections(baselines, defaultState, attempt1);

    const attempt2 = saveAppState(defaultState, baselines);
    expect(attempt2).toEqual(new Set(["timer", "social", "core"]));
    expect(storage.getItem(V2_KEY)).toBeNull();
  });
});

describe("loadAppState - migration and corruption", () => {
  it("loads correctly from a legacy-only (pre-migration) blob", () => {
    storage.setItem(V2_KEY, JSON.stringify({ ...defaultState, waterGlasses: 4 }));
    const result = loadAppState();
    expect(result.waterGlasses).toBe(4);
  });

  it("round-trips losslessly through save then load", () => {
    const state: AppState = { ...defaultState, waterGlasses: 2, timer: runningCountdown() };
    saveAppState(state, createInitialPersistenceBaselines(defaultState));
    const loaded = loadAppState();
    expect(loaded.waterGlasses).toBe(2);
    expect(loaded.timer.studyMinutes).toBe(state.timer.studyMinutes);
  });

  it("loads correctly with only the timer section migrated (social/core still legacy)", () => {
    storage.setItem(V2_KEY, JSON.stringify({ ...defaultState, social: { ...defaultState.social, displayName: "LegacySocial" }, waterGlasses: 9 }));
    storage.setItem(TIMER_KEY, JSON.stringify({ timer: { ...defaultTimer, studyMinutes: 55 } }));
    const result = loadAppState();
    expect(result.timer.studyMinutes).toBe(55);
    expect(result.social.displayName).toBe("LegacySocial");
    expect(result.waterGlasses).toBe(9);
  });

  it("loads correctly with only the social section migrated (timer/core still legacy)", () => {
    storage.setItem(V2_KEY, JSON.stringify({ ...defaultState, timer: { ...defaultTimer, studyMinutes: 55 }, waterGlasses: 9 }));
    storage.setItem(SOCIAL_KEY, JSON.stringify({ social: { ...defaultState.social, displayName: "NewSocial" } }));
    const result = loadAppState();
    expect(result.social.displayName).toBe("NewSocial");
    expect(result.timer.studyMinutes).toBe(55);
    expect(result.waterGlasses).toBe(9);
  });

  it("loads correctly with only the core section migrated (timer/social still legacy)", () => {
    storage.setItem(V2_KEY, JSON.stringify({ ...defaultState, timer: { ...defaultTimer, studyMinutes: 55 }, social: { ...defaultState.social, displayName: "LegacySocial" } }));
    storage.setItem(CORE_KEY, JSON.stringify({ waterGlasses: 9 }));
    const result = loadAppState();
    expect(result.waterGlasses).toBe(9);
    expect(result.timer.studyMinutes).toBe(55);
    expect(result.social.displayName).toBe("LegacySocial");
  });

  it("a corrupted individual v3 key does not lose data from the other sections", () => {
    storage.setItem(TIMER_KEY, "{not valid json");
    storage.setItem(SOCIAL_KEY, JSON.stringify({ social: { ...defaultState.social, displayName: "Ada" } }));
    storage.setItem(CORE_KEY, JSON.stringify({ waterGlasses: 7 }));
    const result = loadAppState();
    expect(result.social.displayName).toBe("Ada");
    expect(result.waterGlasses).toBe(7);
    expect(result.timer).toEqual(defaultTimer); // corrupted timer section falls back to default, doesn't throw
  });

  it("6. loads a valid v3 state when the legacy v2 blob is corrupt", () => {
    storage.setItem(V2_KEY, "{not valid json");
    storage.setItem(TIMER_KEY, JSON.stringify({ timer: { ...defaultTimer, studyMinutes: 42 } }));
    storage.setItem(SOCIAL_KEY, JSON.stringify({ social: { ...defaultState.social, displayName: "Ada" } }));
    storage.setItem(CORE_KEY, JSON.stringify({ waterGlasses: 3 }));

    const result = loadAppState();

    expect(result.timer.studyMinutes).toBe(42);
    expect(result.social.displayName).toBe("Ada");
    expect(result.waterGlasses).toBe(3);
  });

  it("7. ignores bogus timer/social properties embedded in the core section when dedicated v3 keys are absent", () => {
    const legacyTimer = { ...defaultTimer, studyMinutes: 77 };
    const legacySocial = { ...defaultState.social, displayName: "Legacy" };
    storage.setItem(V2_KEY, JSON.stringify({ ...defaultState, timer: legacyTimer, social: legacySocial }));
    // No TIMER_KEY / SOCIAL_KEY written - only a core blob that (as if corrupted) contains
    // timer/social properties it should never legitimately have.
    storage.setItem(CORE_KEY, JSON.stringify({
      waterGlasses: 1,
      timer: { ...defaultTimer, studyMinutes: 999 },
      social: { ...defaultState.social, displayName: "Bogus" },
    }));

    const result = loadAppState();

    expect(result.timer.studyMinutes).toBe(77);
    expect(result.social.displayName).toBe("Legacy");
    expect(result.waterGlasses).toBe(1);
  });

  it("returns defaultState when nothing is persisted at all", () => {
    expect(loadAppState()).toEqual(defaultState);
  });

  it("backfills phase/archived/date defaults on an old-shape semester and course", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      semesters: [{ id: "sem1", name: "Old Semester", createdAt: "2026-01-01T00:00:00.000Z" }],
      courses: [{ id: "course1", semesterId: "sem1", name: "Old Course", color: "#fff", targetGrade: 5, createdAt: "2026-01-01T00:00:00.000Z" }],
    }));
    const result = loadAppState();
    expect(result.semesters[0]).toMatchObject({ startDate: null, endDate: null, phase: "semester", archived: false, archivedAt: null });
    expect(result.courses[0]).toMatchObject({ externalUrl: null });
  });

  it("defaults a legacy daily todo's missing time/endTime/notes fields", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      dailyTodos: [{ id: "todo1", date: "2026-09-07", title: "Buy pens", completed: false, completedAt: null, createdAt: "2026-01-01T00:00:00.000Z" }],
    }));
    const result = loadAppState();
    expect(result.dailyTodos[0]).toMatchObject({ time: null, endTime: null, notes: "", repeatWeekly: false, completedOccurrences: [], recurrenceEndDate: null, skippedOccurrences: [], occurrenceTimes: {} });
  });

  it("defaults the new planner arrays to [] when the stored blob predates them", () => {
    storage.setItem(CORE_KEY, JSON.stringify({ waterGlasses: 1 }));
    const result = loadAppState();
    expect(result.timetableEvents).toEqual([]);
    expect(result.holidays).toEqual([]);
    expect(result.dailyTodos).toEqual([]);
  });

  it("round-trips populated timetable events, holidays, and todos", () => {
    const state: AppState = {
      ...defaultState,
      tasks: [{
        id: "task1", semesterId: "sem1", courseId: "course1", title: "Lectures", subtype: "Lecture", unitLabel: "unit",
        totalUnits: 14, completedUnits: 3, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z",
      }],
      timetableEvents: [{
        id: "ev1", semesterId: "sem1", courseId: "course1", kind: "occurrence", taskId: "task1", label: "Lecture",
        date: "2026-09-07", time: "10:00", endTime: "12:00", repeatWeekly: true, recurrenceEndDate: null, occurrenceOverrides: {}, url: null,
        completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z",
      }],
      holidays: [{ id: "holiday1", semesterId: "sem1", startDate: "2026-12-20", endDate: "2027-01-05", label: "Winter break", createdAt: "2026-01-01T00:00:00.000Z" }],
      dailyTodos: [{ id: "todo1", date: "2026-09-07", time: null, endTime: null, title: "Buy pens", notes: "", completed: false, completedAt: null, createdAt: "2026-01-01T00:00:00.000Z", repeatWeekly: false, completedOccurrences: [], recurrenceEndDate: null, skippedOccurrences: [], occurrenceTimes: {} }],
    };
    saveAppState(state, createInitialPersistenceBaselines(defaultState));
    const loaded = loadAppState();
    expect(loaded.timetableEvents).toEqual(state.timetableEvents);
    expect(loaded.holidays).toEqual(state.holidays);
    expect(loaded.dailyTodos).toEqual(state.dailyTodos);
  });

  it("migrates a legacy recurring-class-event and exercise-sheet-series blob into unified timetable events, creating fallback tasks for them", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      semesters: [{ id: "sem1", name: "WS", createdAt: "2026-01-01T00:00:00.000Z", startDate: "2026-09-07", endDate: "2026-12-31", phase: "semester", archived: false, archivedAt: null }],
      recurringClassEvents: [{ id: "ev1", semesterId: "sem1", courseId: "course1", label: "Lecture", weekday: 1, startTime: "10:00", endTime: "12:00", createdAt: "2026-01-01T00:00:00.000Z" }],
      exerciseSheetSeries: [{ id: "series1", semesterId: "sem1", courseId: "course1", label: "Sheet", releaseWeekday: 1, releaseTime: "20:00", deadlineWeekday: 0, deadlineTime: "23:59", url: "https://example.com", createdAt: "2026-01-01T00:00:00.000Z" }],
    }));
    const result = loadAppState();
    expect(result.timetableEvents).toEqual([
      { id: "ev1", semesterId: "sem1", courseId: "course1", kind: "occurrence", taskId: "course1:legacy-lecture-task", label: "Lecture", date: "2026-09-07", time: "10:00", endTime: "12:00", repeatWeekly: true, recurrenceEndDate: null, occurrenceOverrides: {}, url: null, completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
      { id: "series1-release", semesterId: "sem1", courseId: "course1", kind: "sheet-release", taskId: "course1:legacy-sheet-task", label: "Sheet", date: "2026-09-07", time: "20:00", endTime: null, repeatWeekly: true, recurrenceEndDate: null, occurrenceOverrides: {}, url: "https://example.com", completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
      { id: "series1-deadline", semesterId: "sem1", courseId: "course1", kind: "sheet-deadline", taskId: "course1:legacy-sheet-task", label: "Sheet", date: "2026-09-13", time: "23:59", endTime: null, repeatWeekly: true, recurrenceEndDate: null, occurrenceOverrides: {}, url: "https://example.com", completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
    ]);
    expect(result.tasks.map((task) => task.id)).toEqual(expect.arrayContaining(["course1:legacy-lecture-task", "course1:legacy-sheet-task"]));
  });

  it("migrates old kind='class'/'lecture'/'exercise-session' timetable events (with no taskId or unitTypeId at all) onto a fallback task", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      courses: [{ id: "course1", semesterId: "sem1", name: "Old Course", color: "#fff", targetGrade: 5, createdAt: "2026-01-01T00:00:00.000Z" }],
      timetableEvents: [
        { id: "ev1", semesterId: "sem1", courseId: "course1", kind: "class", label: "Lecture", date: "2026-09-07", time: "10:00", endTime: "12:00", repeatWeekly: true, url: null, completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
        { id: "ev2", semesterId: "sem1", courseId: "course1", kind: "exercise-session", label: "Exercise", date: "2026-09-08", time: "14:00", endTime: "16:00", repeatWeekly: true, url: null, completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
      ],
    }));
    const result = loadAppState();
    expect(result.timetableEvents[0]).toMatchObject({ kind: "occurrence", taskId: "course1:legacy-lecture-task" });
    expect(result.timetableEvents[1]).toMatchObject({ kind: "occurrence", taskId: "course1:legacy-lecture-task" });
    expect(result.tasks.some((task) => task.id === "course1:legacy-lecture-task")).toBe(true);
  });

  it("converts a pre-unification course.unitTypes entry into a Task, reusing the unit type's id, and drops a timetable event whose taskId no longer resolves to any task", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      courses: [{
        id: "course1", semesterId: "sem1", name: "Old Course", color: "#fff", targetGrade: 5, createdAt: "2026-01-01T00:00:00.000Z",
        unitTypes: [{ id: "unit1", label: "Lectures", behavior: "single", repeatWeeklyDefault: true, completedCount: 4, createdAt: "2026-01-01T00:00:00.000Z" }],
      }],
      timetableEvents: [
        { id: "ev1", semesterId: "sem1", courseId: "course1", kind: "occurrence", unitTypeId: "unit1", label: "Lecture", date: "2026-09-07", time: "10:00", endTime: "12:00", repeatWeekly: true, url: null, completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
        { id: "ev2", semesterId: "sem1", courseId: "course1", kind: "occurrence", unitTypeId: "no-such-unit", label: "Orphaned", date: "2026-09-08", time: "10:00", endTime: "12:00", repeatWeekly: true, url: null, completedOccurrences: [], createdAt: "2026-01-01T00:00:00.000Z" },
      ],
    }));
    const result = loadAppState();
    const migratedTask = result.tasks.find((task) => task.id === "unit1");
    expect(migratedTask).toMatchObject({ courseId: "course1", title: "Lectures", subtype: "Lecture", completedUnits: 4, totalUnits: 4 });
    expect(result.timetableEvents.map((event) => event.id)).toEqual(["ev1"]);
    expect(result.timetableEvents[0].taskId).toBe("unit1");
  });

  it("defaults a task's subtype to 'Other' when missing or invalid, and preserves a valid explicit subtype", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      tasks: [
        { id: "t1", semesterId: "sem1", courseId: "course1", title: "Old task", unitLabel: "Unit", totalUnits: 5, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" },
        { id: "t2", semesterId: "sem1", courseId: "course1", title: "Bogus subtype", subtype: "Homework", unitLabel: "Unit", totalUnits: 5, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" },
        { id: "t3", semesterId: "sem1", courseId: "course1", title: "Weekly lecture", subtype: "Lecture", unitLabel: "unit", totalUnits: 0, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" },
      ],
    }));
    const result = loadAppState();
    expect(result.tasks.find((task) => task.id === "t1")?.subtype).toBe("Other");
    expect(result.tasks.find((task) => task.id === "t2")?.subtype).toBe("Other");
    expect(result.tasks.find((task) => task.id === "t3")?.subtype).toBe("Lecture");
  });

  it("infers a subtype from the title for a task that predates the subtype field entirely, matching 'Sheet' over 'Session' for a title that contains both 'exercise' and 'sheet'", () => {
    storage.setItem(CORE_KEY, JSON.stringify({
      tasks: [
        { id: "t1", semesterId: "sem1", courseId: "course1", title: "Exercise Sheets", unitLabel: "unit", totalUnits: 1, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" },
        { id: "t2", semesterId: "sem1", courseId: "course1", title: "Lectures", unitLabel: "unit", totalUnits: 1, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" },
        { id: "t3", semesterId: "sem1", courseId: "course1", title: "Exercise Sessions", unitLabel: "unit", totalUnits: 1, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-01-01T00:00:00.000Z" },
      ],
    }));
    const result = loadAppState();
    expect(result.tasks.find((task) => task.id === "t1")?.subtype).toBe("Sheet");
    expect(result.tasks.find((task) => task.id === "t2")?.subtype).toBe("Lecture");
    expect(result.tasks.find((task) => task.id === "t3")?.subtype).toBe("Session");
  });
});

describe("restoreBackup", () => {
  it("rejects files that are not backups", async () => {
    const { restoreBackup } = await import("./storage");
    expect(() => restoreBackup("nope")).toThrow(/valid JSON/);
    expect(() => restoreBackup("{}")).toThrow(/not a Study Tracker backup/);
    expect(() => restoreBackup(JSON.stringify({ sessions: [], courses: [], social: {} }))).toThrow(/account identity/);
  });
});

describe("backup round trip", () => {
  it("restores study time, semesters, courses, deadlines, achievements and the account on a blank device", () => {
    const original = {
      ...defaultState,
      semesters: [{ id: "sem1", name: "HS26", createdAt: "2026-09-01T00:00:00.000Z", startDate: "2026-09-14", endDate: "2026-12-23", phase: "active", archived: false, archivedAt: null }],
      courses: [{ id: "c1", semesterId: "sem1", name: "Analysis", color: "#123456", targetGrade: 5.5, createdAt: "2026-09-01T00:00:00.000Z", externalUrl: null }],
      tasks: [{ id: "t1", semesterId: "sem1", courseId: "c1", title: "Sheet 3", subtype: "Sheet", unitLabel: "sheet", totalUnits: 4, completedUnits: 1, dueDate: "2026-10-05", priority: "high", notes: "", createdAt: "2026-09-02T00:00:00.000Z" }],
      exams: [{ id: "e1", semesterId: "sem1", courseId: "c1", title: "Final", examDate: "2027-01-20", weight: 100, preparedness: 40, location: "HG E 5" }],
      sessions: [{ id: "s1", semesterId: "sem1", courseId: "c1", taskId: "t1", kind: "study", goal: "g", learned: "l", blocker: "", nextStep: "", confidence: 3, startedAt: "2026-09-20T09:00:00.000Z", endedAt: "2026-09-20T10:30:00.000Z", minutes: 90, presetLabel: "Deep" }],
      lifetimeStudyMinutes: 4321,
      lifetimeStudySessions: 77,
      petRockPats: 1234,
      achievementEarnedOnDates: { "early-bird": "2026-09-10" },
      social: { ...defaultState.social, userId: "user-abc", deviceSecret: "secret-xyz", friendCode: "ABCD-2345", displayName: "Dani" },
    } as unknown as AppState;

    const file = JSON.stringify(buildBackup(original));
    storage.clear();
    restoreBackup(file);
    const loaded = loadAppState();

    expect(loaded.semesters).toMatchObject([{ id: "sem1", name: "HS26", startDate: "2026-09-14", endDate: "2026-12-23" }]);
    expect(loaded.courses).toMatchObject([{ id: "c1", name: "Analysis", targetGrade: 5.5 }]);
    expect(loaded.tasks).toMatchObject([{ id: "t1", title: "Sheet 3", dueDate: "2026-10-05" }]);
    expect(loaded.exams).toMatchObject([{ id: "e1", title: "Final", examDate: "2027-01-20" }]);
    expect(loaded.sessions).toMatchObject([{ id: "s1", minutes: 90 }]);
    expect(loaded.lifetimeStudyMinutes).toBe(4321);
    expect(loaded.lifetimeStudySessions).toBe(77);
    expect(loaded.petRockPats).toBe(1234);
    expect(loaded.achievementEarnedOnDates).toEqual({ "early-bird": "2026-09-10" });
    expect(loaded.social.userId).toBe("user-abc");
    expect(loaded.social.deviceSecret).toBe("secret-xyz");
  });
});
