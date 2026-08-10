import { describe, expect, it } from "vitest";
import { defaultTimer } from "./storage";
import { getDisplayRemainingSeconds } from "./timerDisplay";
import { pauseCountdownTimer, pauseStopwatchTimer, resumeCountdownTimer, resumeStopwatchTimer } from "./timerTransitions";
import type { TimerState } from "../types";

const SEC = 1000;
const MIN = 60 * SEC;

function endlessTimerStartedAt(startedAt: string): TimerState {
  return {
    ...defaultTimer,
    mode: "endless",
    phase: "stopwatch",
    running: true,
    startedAt,
    endsAt: null,
    remainingSeconds: 0,
    activeSegments: [{ startedAt, endedAt: null }],
  };
}

describe("stopwatch/endless pause and resume", () => {
  const t0 = new Date("2026-08-01T12:00:00.000Z");

  it("1. running 90s then pausing sets remainingSeconds to 90", () => {
    const running = endlessTimerStartedAt(t0.toISOString());
    const pausedAt = new Date(t0.getTime() + 90 * SEC);

    const paused = pauseStopwatchTimer(running, pausedAt);

    expect(paused.running).toBe(false);
    expect(paused.remainingSeconds).toBe(90);
  });

  it("2. the paused timer displays 90s", () => {
    const running = endlessTimerStartedAt(t0.toISOString());
    const pausedAt = new Date(t0.getTime() + 90 * SEC);
    const paused = pauseStopwatchTimer(running, pausedAt);

    expect(getDisplayRemainingSeconds(paused, pausedAt)).toBe(90);
  });

  it("3. still displays 90s after waiting 5 minutes while paused", () => {
    const running = endlessTimerStartedAt(t0.toISOString());
    const pausedAt = new Date(t0.getTime() + 90 * SEC);
    const paused = pauseStopwatchTimer(running, pausedAt);

    const fiveMinutesLater = new Date(pausedAt.getTime() + 5 * MIN);
    expect(getDisplayRemainingSeconds(paused, fiveMinutesLater)).toBe(90);
  });

  it("4. resuming after the paused gap, then running 30 more seconds, displays 120s", () => {
    const running = endlessTimerStartedAt(t0.toISOString());
    const pausedAt = new Date(t0.getTime() + 90 * SEC);
    const paused = pauseStopwatchTimer(running, pausedAt);

    const resumedAt = new Date(pausedAt.getTime() + 5 * MIN);
    const resumed = resumeStopwatchTimer(paused, resumedAt);

    // Elapsed at the moment of resume must still be 90s - the paused gap isn't counted.
    expect(resumed.remainingSeconds).toBe(90);
    expect(resumed.running).toBe(true);

    const thirtySecondsLater = new Date(resumedAt.getTime() + 30 * SEC);
    expect(getDisplayRemainingSeconds(resumed, thirtySecondsLater)).toBe(120);
  });

  it("5. activeSegments exclude the paused interval", () => {
    const running = endlessTimerStartedAt(t0.toISOString());
    const pausedAt = new Date(t0.getTime() + 90 * SEC);
    const paused = pauseStopwatchTimer(running, pausedAt);

    const resumedAt = new Date(pausedAt.getTime() + 5 * MIN);
    const resumed = resumeStopwatchTimer(paused, resumedAt);

    expect(resumed.activeSegments).toHaveLength(2);
    // First segment closes at the pause instant, not extended into the paused gap.
    expect(resumed.activeSegments[0].startedAt).toBe(t0.toISOString());
    expect(resumed.activeSegments[0].endedAt).toBe(pausedAt.toISOString());
    // Second segment only starts at resume, leaving the 5-minute gap uncovered by either segment.
    expect(resumed.activeSegments[1].startedAt).toBe(resumedAt.toISOString());
    expect(resumed.activeSegments[1].endedAt).toBeNull();
  });
});

describe("countdown pause and resume", () => {
  const t0 = new Date("2026-08-01T12:00:00.000Z");

  it("6. countdown pause/resume still works correctly", () => {
    const startedAt = t0.toISOString();
    const endsAt = new Date(t0.getTime() + 25 * MIN).toISOString(); // 25 min countdown
    const running: TimerState = {
      ...defaultTimer,
      phase: "study",
      mode: "focus",
      running: true,
      startedAt,
      endsAt,
      remainingSeconds: 25 * 60,
      activeSegments: [{ startedAt, endedAt: null }],
    };

    // Pause 10 minutes in - 15 minutes should remain.
    const pausedAt = new Date(t0.getTime() + 10 * MIN);
    const paused = pauseCountdownTimer(running, running.endsAt as string, pausedAt);
    expect(paused.running).toBe(false);
    expect(paused.endsAt).toBeNull();
    expect(paused.remainingSeconds).toBe(15 * 60);
    expect(paused.activeSegments[0].endedAt).toBe(pausedAt.toISOString());
    expect(getDisplayRemainingSeconds(paused, new Date(pausedAt.getTime() + 5 * MIN))).toBe(15 * 60);

    // Resume 5 minutes later - the paused gap must not count against the countdown.
    const resumedAt = new Date(pausedAt.getTime() + 5 * MIN);
    const resumed = resumeCountdownTimer(paused, resumedAt);
    expect(resumed.running).toBe(true);
    expect(resumed.startedAt).toBe(resumedAt.toISOString());
    expect(resumed.endsAt).toBe(new Date(resumedAt.getTime() + 15 * MIN).toISOString());
    expect(resumed.activeSegments).toHaveLength(2);
    expect(resumed.activeSegments[1].startedAt).toBe(resumedAt.toISOString());

    // 10 minutes after resuming (5 of the original 15 remaining), and exactly at expiry.
    expect(getDisplayRemainingSeconds(resumed, new Date(resumedAt.getTime() + 10 * MIN))).toBe(5 * 60);
    expect(getDisplayRemainingSeconds(resumed, new Date(resumedAt.getTime() + 15 * MIN))).toBe(0);
  });
});
