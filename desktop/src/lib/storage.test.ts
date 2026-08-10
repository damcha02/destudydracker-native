import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  APP_STATE_STORAGE_KEYS,
  applyPersistedSections,
  createInitialPersistenceBaselines,
  defaultState,
  defaultTimer,
  getChangedSections,
  loadAppState,
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

    const afterEdit: AppState = { ...state, tasks: [{ id: "t1", semesterId: "s", courseId: "c", title: "Read", unitLabel: "Unit", totalUnits: 1, completedUnits: 0, dueDate: null, priority: "medium", notes: "", createdAt: "2026-08-01T00:00:00.000Z" }] };
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
});
