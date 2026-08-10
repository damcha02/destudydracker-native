import { useEffect } from "react";
import type { RefObject } from "react";
import { useTimerTick } from "./useTimerTick";
import { getTimerProgressPercent } from "../lib/timerDisplay";
import type { TimerState } from "../types";

function applyProgress(ref: RefObject<HTMLElement | null>, timer: TimerState) {
  const percent = getTimerProgressPercent(timer);
  ref.current?.style.setProperty("--timer-progress", `${percent * 3.6}deg`);
}

/**
 * Mutates the `--timer-progress` CSS variable directly on the ref'd element via its own
 * interval (useTimerTick), instead of flowing a per-tick-written root AppState field through
 * a declarative style prop - so the ring updates every tick without causing any React
 * re-render. Reuses the exact percentage formula that used to live inline in App() as the
 * timerConfiguredSeconds/timerProgress consts (see lib/timerDisplay.ts).
 *
 * useTimerTick's own deps (running/phase/startedAt/endsAt/remainingSeconds) don't cover
 * every field the percentage formula reads - mode/studyMinutes/examMinutes/breakMinutes can
 * all change the configured-seconds denominator (e.g. editing "Break minutes" while paused
 * mid-break changes breakMinutes without touching any of useTimerTick's deps). Rather than
 * broadening useTimerTick's dependency list for every consumer, this hook adds its own
 * immediate (non-interval) resync effect scoped to exactly the fields this formula needs.
 */
export function useTimerProgressRing(ref: RefObject<HTMLElement | null>, timer: TimerState) {
  useTimerTick(timer, (latestTimer) => {
    applyProgress(ref, latestTimer);
  });

  useEffect(() => {
    applyProgress(ref, timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [timer.mode, timer.phase, timer.studyMinutes, timer.examMinutes, timer.breakMinutes, timer.remainingSeconds]);
}
