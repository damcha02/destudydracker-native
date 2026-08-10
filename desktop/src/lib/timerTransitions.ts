import type { TimerState } from "../types";
import { getDisplayRemainingSeconds, getTimerActiveSeconds } from "./timerDisplay";

function closeOpenSegments(segments: TimerState["activeSegments"], pausedAt: string): TimerState["activeSegments"] {
  return (segments ?? []).map((segment) => (segment.endedAt === null ? { ...segment, endedAt: pausedAt } : segment));
}

/**
 * Pauses a running stopwatch/endless timer. Steady ticks no longer keep remainingSeconds
 * fresh in state (Phase 1 perf change), so this must explicitly derive the actual elapsed
 * value before flipping running to false - `{...timer, running: false}` alone would silently
 * carry over whatever stale value (e.g. 0 from startTimer) has been sitting in state since
 * the last transition. This is exactly the bug live-testing caught: pausing an endless timer
 * used to snap the display back to 0 instead of freezing at the elapsed time.
 */
export function pauseStopwatchTimer(timer: TimerState, now: Date = new Date()): TimerState {
  const pausedAt = now.toISOString();
  const elapsedAtPause = getDisplayRemainingSeconds(timer, now);
  return {
    ...timer,
    running: false,
    remainingSeconds: elapsedAtPause,
    activeSegments: closeOpenSegments(timer.activeSegments, pausedAt),
  };
}

/** Resumes a paused stopwatch/endless timer, continuing from its frozen elapsed value. */
export function resumeStopwatchTimer(timer: TimerState, now: Date = new Date()): TimerState {
  const elapsed = getTimerActiveSeconds(timer, now);
  const resumedAt = now.toISOString();
  return {
    ...timer,
    running: true,
    startedAt: resumedAt,
    remainingSeconds: elapsed,
    activeSegments: [...(timer.activeSegments ?? []), { startedAt: resumedAt, endedAt: null }],
  };
}

/**
 * Pauses a running countdown (study/exam) timer. Unlike the stopwatch branch, this was never
 * affected by the Phase 1 steady-tick change: it always recomputed remainingSeconds fresh
 * from the durable `endsAt` field rather than trusting whatever was last written to state.
 */
export function pauseCountdownTimer(timer: TimerState, endsAt: string, now: Date = new Date()): TimerState {
  const pausedAt = now.toISOString();
  const diff = Math.max(0, Math.ceil((new Date(endsAt).getTime() - now.getTime()) / 1000));
  return {
    ...timer,
    running: false,
    endsAt: null,
    remainingSeconds: diff,
    activeSegments: closeOpenSegments(timer.activeSegments, pausedAt),
  };
}

/** Resumes a paused countdown timer from its frozen remainingSeconds snapshot. */
export function resumeCountdownTimer(timer: TimerState, now: Date = new Date()): TimerState {
  const resumedAt = now.toISOString();
  return {
    ...timer,
    running: true,
    startedAt: resumedAt,
    endsAt: new Date(now.getTime() + timer.remainingSeconds * 1000).toISOString(),
    activeSegments: [...(timer.activeSegments ?? []), { startedAt: resumedAt, endedAt: null }],
  };
}
