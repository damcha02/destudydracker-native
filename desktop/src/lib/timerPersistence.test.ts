import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defaultState } from "./storage";
import {
  startTimerPersistenceHeartbeat,
  TIMER_PERSISTENCE_HEARTBEAT_MS,
} from "./timerPersistence";

describe("timer persistence heartbeat", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("persists immediately when the timer starts or resumes", () => {
    const persist = vi.fn();
    const stop = startTimerPersistenceHeartbeat(() => defaultState, persist);

    expect(persist).toHaveBeenCalledOnce();
    expect(persist).toHaveBeenLastCalledWith(defaultState);

    stop();
  });

  it("persists the latest state every heartbeat interval", () => {
    const persist = vi.fn();
    let current = defaultState;
    const stop = startTimerPersistenceHeartbeat(() => current, persist);

    current = {
      ...defaultState,
      timer: {
        ...defaultState.timer,
        running: true,
        remainingSeconds: 123,
      },
    };

    vi.advanceTimersByTime(TIMER_PERSISTENCE_HEARTBEAT_MS);

    expect(persist).toHaveBeenCalledTimes(2);
    expect(persist).toHaveBeenLastCalledWith(current);

    stop();
  });

  it("stops persisting after cleanup", () => {
    const persist = vi.fn();
    const stop = startTimerPersistenceHeartbeat(() => defaultState, persist);

    stop();
    vi.advanceTimersByTime(TIMER_PERSISTENCE_HEARTBEAT_MS * 2);

    expect(persist).toHaveBeenCalledOnce();
  });
});
