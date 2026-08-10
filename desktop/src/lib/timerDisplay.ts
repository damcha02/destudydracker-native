import type { TimerActiveSegment, TimerState } from "../types";
import { clamp } from "./metrics";

export function getConfiguredTimerSeconds(timer: Pick<TimerState, "phase" | "mode" | "studyMinutes" | "examMinutes">) {
  return timer.phase === "exam" || timer.mode === "exam" ? timer.examMinutes * 60 : timer.studyMinutes * 60;
}

export function closeTimerSegments(timer: TimerState, endedAt: string): Array<{ startedAt: string; endedAt: string }> {
  const fallbackSegments: TimerActiveSegment[] = [];
  if (timer.startedAt && (timer.phase === "study" || timer.phase === "exam" || timer.phase === "stopwatch")) {
    const startedAtMs = new Date(timer.startedAt).getTime();
    if (Number.isFinite(startedAtMs)) {
      const configuredSeconds = timer.phase === "stopwatch" ? Math.max(0, Math.floor(timer.remainingSeconds)) : getConfiguredTimerSeconds(timer);
      const fallbackActiveSeconds = timer.phase === "stopwatch"
        ? configuredSeconds
        : clamp(configuredSeconds - timer.remainingSeconds - (timer.loggedSplitSeconds ?? 0), 0, configuredSeconds);
      fallbackSegments.push({
        startedAt: timer.startedAt,
        endedAt: timer.running ? null : new Date(startedAtMs + fallbackActiveSeconds * 1000).toISOString(),
      });
    }
  }
  const segments = timer.activeSegments?.length ? timer.activeSegments : fallbackSegments;
  const finalEndMs = new Date(endedAt).getTime();
  return segments.flatMap((segment) => {
    const startMs = new Date(segment.startedAt).getTime();
    const segmentEndMs = new Date(segment.endedAt ?? endedAt).getTime();
    const boundedEndMs = Math.min(segmentEndMs, finalEndMs);
    if (!Number.isFinite(startMs) || !Number.isFinite(boundedEndMs) || boundedEndMs <= startMs) return [];
    return [{ startedAt: segment.startedAt, endedAt: new Date(boundedEndMs).toISOString() }];
  });
}

/**
 * `now` defaults to `new Date()` so every existing call site (which never passed a second
 * argument) is byte-for-byte unaffected. A caller that needs a specific instant — e.g. a
 * display derivation ticking off its own clock — can pass it explicitly instead of relying
 * on the real wall clock at call time.
 */
export function getTimerActiveSeconds(timer: TimerState, now: Date = new Date()) {
  if (timer.phase !== "study" && timer.phase !== "exam" && timer.phase !== "stopwatch") return 0;

  const activeSegments = closeTimerSegments(timer, now.toISOString());
  if (activeSegments.length) {
    return activeSegments.reduce((sum, segment) => {
      const startMs = new Date(segment.startedAt).getTime();
      const endMs = new Date(segment.endedAt).getTime();
      return sum + Math.max(0, Math.floor((endMs - startMs) / 1000));
    }, 0);
  }

  const configuredSeconds = getConfiguredTimerSeconds(timer);
  return clamp(configuredSeconds - timer.remainingSeconds - (timer.loggedSplitSeconds ?? 0), 0, configuredSeconds);
}

export function getIdleTimerSeconds(timer: Pick<TimerState, "mode" | "studyMinutes" | "examMinutes">) {
  if (timer.mode === "endless") return 0;
  return (timer.mode === "exam" ? timer.examMinutes : timer.studyMinutes) * 60;
}

/**
 * Derives "seconds left to show right now" from durable timer fields instead of a
 * per-tick-written state value. Mirrors the tick loop's own math exactly (see App.tsx's
 * 500ms timer effect) so it can be called from a leaf component's own interval without
 * requiring the root AppState to be updated every tick. `now` is threaded through to both
 * branches so a caller ticking off its own clock gets a consistent instant either way.
 */
export function getDisplayRemainingSeconds(timer: TimerState, now: Date = new Date()): number {
  if (!timer.running) return timer.remainingSeconds;
  if (timer.phase === "stopwatch") return getTimerActiveSeconds(timer, now);
  if (!timer.endsAt) return timer.remainingSeconds;
  return Math.max(0, Math.ceil((new Date(timer.endsAt).getTime() - now.getTime()) / 1000));
}

/** Mirrors the formula that used to live inline in App() as the timerConfiguredSeconds const. */
export function getTimerConfiguredSecondsForProgress(timer: Pick<TimerState, "phase" | "mode" | "studyMinutes" | "examMinutes" | "breakMinutes">): number {
  if (timer.phase === "stopwatch") return 1;
  if (timer.phase === "break") return Math.max(1, timer.breakMinutes * 60);
  return Math.max(1, (timer.mode === "exam" ? timer.examMinutes : timer.studyMinutes) * 60);
}

/**
 * Progress-ring percentage (0-100), mirroring the formula that used to live inline in App()
 * as the timerProgress const, but deriving "seconds left" instead of reading a per-tick-
 * written state field.
 */
export function getTimerProgressPercent(timer: TimerState, now: Date = new Date()): number {
  if (timer.phase === "stopwatch") return 100;
  const configuredSeconds = getTimerConfiguredSecondsForProgress(timer);
  const remaining = getDisplayRemainingSeconds(timer, now);
  return clamp(((configuredSeconds - remaining) / configuredSeconds) * 100, 0, 100);
}
