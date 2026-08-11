# Timer Compatibility Specification

This document records the production timer semantics inspected for Stage 7. Production source remains reference-only.

## Production Files Inspected

- `desktop/src/App.tsx`
- `desktop/src/types.ts`
- `desktop/src/lib/timerTransitions.ts`
- `desktop/src/lib/timerDisplay.ts`
- `desktop/src/lib/timerPersistence.ts`
- `desktop/src/lib/storage.ts`
- `desktop/src/hooks/useTimerTick.ts`
- `desktop/src/lib/timerTransitions.test.ts`
- `desktop/src/lib/timerDisplay.test.ts`
- `desktop/src/lib/timerRestore.test.ts`
- `desktop/src/lib/timerPersistence.test.ts`
- `desktop/src-tauri/src/lib.rs`

## Production State Shape

Production timer state contains:

- `phase`: `idle`, `study`, `break`, `exam`, `stopwatch`.
- `mode`: `focus`, `exam`, `endless`.
- `remainingSeconds`.
- `loggedSplitSeconds`.
- `activeSegments` with `startedAt` and nullable `endedAt`.
- `running`.
- `studyMinutes`, `breakMinutes`, `examMinutes`.
- `startedAt`, `endsAt`, `lastAliveAt` wall-clock timestamps.
- Course/task/session context fields and reflection fields.
- `presetLabel`.

## Behavior Contract

| Scenario | Initial state | Action | Expected behavior |
|---|---|---|---|
| Start focus | Idle, mode focus | Start | Phase becomes study, running true, `endsAt` set, one open active segment. |
| Start exam | Idle, mode exam | Start | Phase becomes exam, running true, `endsAt` set from exam duration. |
| Start endless | Idle, mode endless | Start | Phase becomes stopwatch, running true, no `endsAt`, elapsed starts at zero, one open segment. |
| Pause countdown | Running study/exam | Pause | Running false, `endsAt` cleared, remaining snapshot derived from current time, open segment closed. |
| Resume countdown | Paused study/exam | Resume | Running true, `endsAt` recomputed from frozen remaining, new open segment starts. |
| Pause stopwatch | Running stopwatch | Pause | Running false, elapsed snapshot stored in `remainingSeconds`, open segment closed. |
| Resume stopwatch | Paused stopwatch | Resume | Running true, elapsed snapshot preserved, new open segment starts. |
| Focus completion with break | Running study, break > 0 | Observe time past end | Study session range ready, then running break starts. |
| Focus completion without break | Running study, break = 0 | Observe time past end | Study session range ready, then timer resets idle. |
| Exam completion | Running exam | Observe time past end | Exam session range ready, then timer resets idle. |
| Break completion | Running break | Observe time past end | Timer resets idle; no study session range. |
| Manual save | Study/exam/stopwatch with positive active time | Save | Session ranges are finalized; session IDs are assigned outside the core; timer resets idle. |
| Reset | Any active or paused phase | Reset | Timer resets idle while retaining mode, durations, context, confidence, and preset label. |
| Repeated commands | Any state | Repeat Start/Pause/Reset/ObserveTime | Deterministic no-op where the transition is already complete. |
| Mode change while active | Running or paused | Set mode | No-op. UI disables this, but domain must also protect it. |

## Persistence And Restore Contract

| Scenario | Expected behavior |
|---|---|
| Running timer persisted | Persistence derives fresh display seconds and writes `lastAliveAt`. |
| Running countdown with recent heartbeat | Restore remains running and derives remaining from `endsAt`. |
| Running countdown expired while closed | Restore emits recovered session range and resets idle. |
| Stale open segment | Restore closes the open segment at `lastAliveAt` when available. |
| Running endless while app closed | Restore paused, not running; elapsed is capped at `lastAliveAt`; no session is auto-created. |
| Abandoned study/exam | If last activity is more than six hours old, recovered session range is emitted and timer resets idle. |
| Duplicate recovery | Re-applying recovery with the same recovered key must not emit duplicate recovered session ranges. |

## Preserved Ambiguities

- `loggedSplitSeconds` is retained in calculations and snapshots, but the inspected production code does not clearly show a current mutation path beyond resets and custom edits.
- Endless inactivity prompt/grace state is React-local and is not persisted. The core does not make it durable in Stage 7.
- Production uses wall-clock `Date` for live countdown expiry; the native core uses monotonic runtime elapsed for live display while preserving wall-clock restore semantics. This is an implementation improvement at the clock boundary, not a change to ordinary user-visible transitions.
- Session ID generation remains outside the core. Production normal sessions use generated IDs; recovered sessions use deterministic recovered IDs. Stage 7 emits ranges only.
