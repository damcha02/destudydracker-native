import { describe, expect, it } from "vitest";
import { defaultTimer, recoverExpiredTimer } from "./storage";
import type { StudySession, TimerState } from "../types";

const MIN = 60 * 1000;
const HOUR = 60 * MIN;

function isoAgo(msAgo: number) {
  return new Date(Date.now() - msAgo).toISOString();
}

function endlessTimerRunningSince(startedMsAgo: number, lastAliveMsAgo: number, elapsedSecondsAtLastSave: number): TimerState {
  const startedAt = isoAgo(startedMsAgo);
  return {
    ...defaultTimer,
    phase: "stopwatch",
    mode: "endless",
    running: true,
    startedAt,
    endsAt: null,
    remainingSeconds: elapsedSecondsAtLastSave,
    activeSegments: [{ startedAt, endedAt: null }],
    lastAliveAt: isoAgo(lastAliveMsAgo),
  };
}

describe("endless timer restore", () => {
  it("1. restores paused instead of running", () => {
    const timer = endlessTimerRunningSince(2 * HOUR, 90 * MIN, 30 * 60);
    const result = recoverExpiredTimer(timer, []);
    expect(result.timer.running).toBe(false);
    expect(result.timer.phase).toBe("stopwatch");
  });

  it("2. preserves the persisted elapsed time instead of zeroing it", () => {
    // Force-closed 40 real minutes into the session; last heartbeat was right before close.
    const timer = endlessTimerRunningSince(40 * MIN, 0, 40 * 60);
    const result = recoverExpiredTimer(timer, []);
    expect(result.timer.remainingSeconds).toBeGreaterThan(39 * 60);
    expect(result.timer.remainingSeconds).toBeLessThanOrEqual(40 * 60 + 1);
  });

  it("3. does not count time while the app was closed", () => {
    // Studied for 20 real minutes, then the app was closed for 11 hours before reopening.
    const startedMsAgo = 11 * HOUR + 20 * MIN;
    const lastAliveMsAgo = 11 * HOUR; // last heartbeat was 20 minutes after start
    const timer = endlessTimerRunningSince(startedMsAgo, lastAliveMsAgo, 20 * 60);
    const result = recoverExpiredTimer(timer, []);
    // Must reflect ~20 minutes studied, never the full ~11h20m wall-clock gap.
    expect(result.timer.remainingSeconds).toBeGreaterThan(19 * 60);
    expect(result.timer.remainingSeconds).toBeLessThan(21 * 60);
  });

  it("4. closes the open segment at lastAliveAt, not at restore time", () => {
    const startedAt = isoAgo(11 * HOUR + 20 * MIN);
    const lastAliveAt = isoAgo(11 * HOUR);
    const timer: TimerState = {
      ...defaultTimer,
      phase: "stopwatch",
      mode: "endless",
      running: true,
      startedAt,
      endsAt: null,
      remainingSeconds: 20 * 60,
      activeSegments: [{ startedAt, endedAt: null }],
      lastAliveAt,
    };
    const result = recoverExpiredTimer(timer, []);
    const segment = result.timer.activeSegments[0];
    expect(segment.endedAt).not.toBeNull();
    expect(new Date(segment.endedAt as string).getTime()).toBe(new Date(lastAliveAt).getTime());
  });

  it("5. does not create a duplicate session across repeated restores", () => {
    const timer = endlessTimerRunningSince(11 * HOUR + 20 * MIN, 11 * HOUR, 20 * 60);
    const existingSessions: StudySession[] = [];

    const first = recoverExpiredTimer(timer, existingSessions);
    // Endless timers are never auto-packaged into a session on restore - they stay paused.
    expect(first.sessions.length).toBe(existingSessions.length);

    // Restoring again from the already-repaired (now paused, closed-segment) timer must
    // still not fabricate a session or double-count time.
    const second = recoverExpiredTimer(first.timer, first.sessions);
    expect(second.sessions.length).toBe(first.sessions.length);
    expect(second.timer.remainingSeconds).toBe(first.timer.remainingSeconds);
  });
});
