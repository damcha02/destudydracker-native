import { useState } from "react";
import { useTimerTick } from "../hooks/useTimerTick";
import { getDisplayRemainingSeconds } from "../lib/timerDisplay";
import type { TimerState } from "../types";

/**
 * Renders the live "MM:SS" clock text for a timer. Ticks locally (via useTimerTick) instead
 * of reading a per-tick-written root AppState field, so only this leaf re-renders while the
 * timer runs - not the whole App() tree. `formatClock` is intentionally not imported from
 * App.tsx to avoid a reverse dependency; callers pass it in.
 */
export function TimerClockDigits({ timer, formatClock }: { timer: TimerState; formatClock: (totalSeconds: number) => string }) {
  const [seconds, setSeconds] = useState(() => getDisplayRemainingSeconds(timer));

  useTimerTick(timer, (latestTimer) => {
    setSeconds(getDisplayRemainingSeconds(latestTimer));
  });

  return <>{formatClock(seconds)}</>;
}
