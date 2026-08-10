import type { AppState } from "../types";

export const TIMER_PERSISTENCE_HEARTBEAT_MS = 30_000;

type IntervalId = ReturnType<typeof setInterval>;

interface TimerScheduler {
  setInterval(callback: () => void, delay: number): IntervalId;
  clearInterval(intervalId: IntervalId): void;
}

export function startTimerPersistenceHeartbeat(
  getState: () => AppState,
  persist: (state: AppState) => void,
  scheduler: TimerScheduler = globalThis,
  intervalMs = TIMER_PERSISTENCE_HEARTBEAT_MS,
): () => void {
  const persistLatestState = (): void => {
    persist(getState());
  };

  // Persist immediately when a timer starts or resumes. The normal 700 ms
  // debounce can otherwise be reset continuously by the timer's 500 ms ticks.
  persistLatestState();

  const intervalId = scheduler.setInterval(persistLatestState, intervalMs);
  return () => scheduler.clearInterval(intervalId);
}
