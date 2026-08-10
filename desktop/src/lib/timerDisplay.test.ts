import { describe, expect, it } from "vitest";
import { defaultTimer } from "./storage";
import { getDisplayRemainingSeconds, getTimerActiveSeconds, getTimerProgressPercent } from "./timerDisplay";
import type { TimerState } from "../types";

describe("getDisplayRemainingSeconds", () => {
  it("returns the stored snapshot when paused/idle, ignoring `now`", () => {
    const timer: TimerState = { ...defaultTimer, running: false, remainingSeconds: 742 };
    expect(getDisplayRemainingSeconds(timer, new Date("2026-01-01T00:00:00Z"))).toBe(742);
  });

  it("derives seconds left from endsAt for a running countdown", () => {
    const now = new Date("2026-08-01T12:00:00Z");
    const endsAt = new Date(now.getTime() + 37_000).toISOString();
    const timer: TimerState = { ...defaultTimer, phase: "study", running: true, endsAt, remainingSeconds: 999 };
    expect(getDisplayRemainingSeconds(timer, now)).toBe(37);
  });

  it("clamps to 0 once endsAt has passed", () => {
    const now = new Date("2026-08-01T12:00:00Z");
    const endsAt = new Date(now.getTime() - 5_000).toISOString();
    const timer: TimerState = { ...defaultTimer, phase: "study", running: true, endsAt, remainingSeconds: 999 };
    expect(getDisplayRemainingSeconds(timer, now)).toBe(0);
  });

  it("falls back to the stored snapshot when running with no endsAt", () => {
    const timer: TimerState = { ...defaultTimer, phase: "study", running: true, endsAt: null, remainingSeconds: 88 };
    expect(getDisplayRemainingSeconds(timer, new Date())).toBe(88);
  });

  it("delegates a running stopwatch to getTimerActiveSeconds using the same `now`", () => {
    const startedAt = "2026-08-01T12:00:00.000Z";
    const now = new Date("2026-08-01T12:05:30.000Z"); // 5m30s after start
    const timer: TimerState = {
      ...defaultTimer,
      phase: "stopwatch",
      mode: "endless",
      running: true,
      startedAt,
      endsAt: null,
      remainingSeconds: 0,
      activeSegments: [{ startedAt, endedAt: null }],
    };
    expect(getDisplayRemainingSeconds(timer, now)).toBe(330);
    expect(getDisplayRemainingSeconds(timer, now)).toBe(getTimerActiveSeconds(timer, now));
  });

  it("derives stopwatch display from the supplied `now`, not the real wall clock", () => {
    // `now` here is deliberately far from the actual current time. Before getTimerActiveSeconds
    // accepted a `now` parameter, the stopwatch branch called `new Date()` internally and ignored
    // whatever instant the caller intended, which would make this assertion fail (or return a
    // huge/unrelated value tied to real elapsed wall-clock time since `startedAt`).
    const startedAt = "2020-01-01T00:00:00.000Z";
    const now = new Date("2020-01-01T00:01:30.000Z"); // exactly 90s after start
    const timer: TimerState = {
      ...defaultTimer,
      phase: "stopwatch",
      mode: "endless",
      running: true,
      startedAt,
      endsAt: null,
      remainingSeconds: 0,
      activeSegments: [{ startedAt, endedAt: null }],
    };
    expect(getDisplayRemainingSeconds(timer, now)).toBe(90);
  });
});

describe("getTimerProgressPercent", () => {
  it("returns 50% at a countdown's midpoint", () => {
    const timer: TimerState = { ...defaultTimer, phase: "study", mode: "focus", studyMinutes: 20, running: false, remainingSeconds: 10 * 60 };
    expect(getTimerProgressPercent(timer)).toBe(50);
  });

  it("returns 100% once a countdown has fully elapsed", () => {
    const timer: TimerState = { ...defaultTimer, phase: "study", mode: "focus", studyMinutes: 20, running: false, remainingSeconds: 0 };
    expect(getTimerProgressPercent(timer)).toBe(100);
  });

  it("computes progress against breakMinutes during a break", () => {
    const timer: TimerState = { ...defaultTimer, phase: "break", breakMinutes: 10, running: false, remainingSeconds: 7 * 60 }; // 3 of 10 min elapsed
    expect(getTimerProgressPercent(timer)).toBe(30);
  });

  it("is always 100% for a stopwatch, regardless of elapsed time", () => {
    const timer: TimerState = { ...defaultTimer, phase: "stopwatch", mode: "endless", running: false, remainingSeconds: 5 };
    expect(getTimerProgressPercent(timer)).toBe(100);
  });

  it("clamps to 0% when remainingSeconds exceeds the configured duration", () => {
    const timer: TimerState = { ...defaultTimer, phase: "study", mode: "focus", studyMinutes: 10, running: false, remainingSeconds: 700 }; // > 600s configured
    expect(getTimerProgressPercent(timer)).toBe(0);
  });

  it("clamps to 100% when remainingSeconds is negative", () => {
    const timer: TimerState = { ...defaultTimer, phase: "study", mode: "focus", studyMinutes: 10, running: false, remainingSeconds: -50 };
    expect(getTimerProgressPercent(timer)).toBe(100);
  });
});
