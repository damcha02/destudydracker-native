import { useEffect, useEffectEvent } from "react";
import type { TimerState } from "../types";

/**
 * Runs `onTick` immediately and then every `intervalMs` while `timer.running` is true.
 * The effect depends only on the durable fields that actually bound or seed the displayed
 * value (running/phase/startedAt/endsAt/remainingSeconds), not the whole `timer` object's
 * identity, so edits to unrelated timer metadata (goal/learned/mode/studyMinutes/presetLabel/
 * ...) don't tear down and restart the interval. `remainingSeconds` is included even though
 * running ticks no longer write it (Phase 1) because a paused/idle timer has no interval at
 * all here - a reset/preset/custom-minutes edit while idle changes remainingSeconds without
 * touching running/phase/startedAt/endsAt, and without this dep the immediate resync on
 * mount/dep-change wouldn't fire, leaving consumers stale. `onTick` still always sees the
 * latest `timer` value via useEffectEvent, even on ticks between those restarts.
 *
 * Shared by every Phase 1 live-timer consumer (TimerClockDigits, the progress-ring ref
 * mutation, the tray IPC sync) so the interval/durable-deps pattern lives in exactly one
 * place instead of being re-implemented per consumer.
 */
export function useTimerTick(timer: TimerState, onTick: (timer: TimerState) => void, intervalMs = 500) {
  const tick = useEffectEvent(() => {
    onTick(timer);
  });

  useEffect(() => {
    tick();
    if (!timer.running) return undefined;
    const id = window.setInterval(tick, intervalMs);
    return () => window.clearInterval(id);
  }, [timer.running, timer.phase, timer.startedAt, timer.endsAt, timer.remainingSeconds, intervalMs]);
}
