# Stage 14 — Timer productionization

**Production reference commit**: `b095706994b6caaf18432d0d41b4534f4b85be98` (upstream `destudydracker`, `main`, 2026-09-27, v0.1.66 - see `docs/production-reference.md`). All production-behavior claims in this document were checked against `desktop/` at this exact commit, not against memory of earlier stages.

## 1. Scope

Turn the Stage 7 timer core and the Stage 4/8/12/13 demo timer surface into the production-quality native Timer feature: a real application-layer controller with a genuine (if not-yet-durable) persistence boundary, Focus/Exam/Endless all selectable and correct, comprehensive deterministic recovery/large-clock-jump tests, a verified minimized/background-rendering behavior (which turned out to need a real fix - see ยง10), and the everyday-performance/memory evidence the architecture freeze calls for. Stage 15 (the durable persistence adapter and real production-data migration) is explicitly not started.

## 2. Production behavior inspected

Read (again, fresh, against the synced `b095706` reference, not assumed from earlier stages): `desktop/src/App.tsx` (timer render/effect sections, notification/inactivity logic), `desktop/src/types.ts` (`TimerState`), `desktop/src/lib/timerTransitions.ts`, `timerDisplay.ts`, `timerPersistence.ts`, `desktop/src/hooks/useTimerTick.ts`, `desktop/src/lib/storage.ts` (`TIMER_KEY` section and `normalizeTimerSegments`). Also specifically searched for: keyboard shortcuts (`grep`'d every `keydown`/`onKeyDown`/`key ===` site in `App.tsx`), notifications (`sendTimerNotification`, `ENDLESS_INACTIVITY_*`), and completion effects (the 1 Hz poll interval that detects `endsAt` crossing).

## 3. Compatibility findings (new, since the last spec revision)

- **Every core-relevant file is unchanged** since the reference `timer-compatibility-spec.md` was written: `timerTransitions.ts`, `timerDisplay.ts`, `timerPersistence.ts`, `useTimerTick.ts`, `TimerClockDigits.tsx`, `WabiRestFluidRing.tsx`, and every timer test file diff empty against the new production reference. `storage.ts`'s `TIMER_KEY`, its `{timer: TimerState}` section shape, and `normalizeTimerSegments` are also unchanged (only unrelated sections - Planner/achievements/backup mechanism - grew).
- **No global Space/R keyboard shortcut exists in production.** Exhaustive search of `App.tsx` for `keydown`/`onKeyDown`/literal key checks found only per-element `Enter`/`Space` activation on individually focused buttons/rows (standard accessible-button behavior) and unrelated `Escape`-closes-modal / `Enter`-submits-guess handlers elsewhere. The native prototype's Space=Start/Pause and R=Reset shortcuts (visible as "Space shortcut"/"R shortcut" captions under the buttons) are a **native-only addition dating to Stage 4**, not a production behavior being preserved. Classified as an accepted native divergence (ยง18) and kept, since Stage 12's benchmark tooling already depends on it and it does not conflict with anything production does.
- **Endless has a production-only inactivity/grace mechanism** not modeled anywhere in the native core: after `ENDLESS_INACTIVITY_PROMPT_MS` (2 hours) of no user "still here" confirmation, a prompt + notification appears; after a further `ENDLESS_INACTIVITY_GRACE_MS` (1 hour) with no response, the session auto-stops (truncated at the prompt time) with a "stopped due to inactivity" notification. See ยง12 for the placement decision.
- **Mode-change guard is now stricter in production** (`disabled={state.timer.running}` -> `disabled={state.timer.phase !== "idle"}`, i.e. also disabled while paused). The native core's `AppModel::apply`'s `SetMode` handler already required `phase == Idle && !running`, so it was already at least as strict; no change was needed, just confirmed.
- `timer-compatibility-spec.md` was updated with a short provenance note recording this re-audit; no behavioral row in its tables needed to change.

## 4. Core changes

**None.** Every scenario Stage 14 was asked to guarantee - long-absence-while-active, duplicate observation after completion, arbitrarily large paused gaps, large monotonic jumps for Endless, a backward wall-clock anomaly on an open segment, and a malformed/corrupted restored segment - was already handled correctly by the Stage 7 core exactly as designed (monotonic time for live math, wall time only for durable timestamps, `saturating_*` arithmetic throughout). Six new deterministic tests were added to `crates/study-tracker-core/src/timer/tests.rs` to lock this in (see ยง21); none required a code change to pass. This is treated as a positive finding, not a gap: the dual-clock design from Stage 7 held up under adversarial-clock testing without modification.

## 5. Application/controller architecture

New module `native-prototype/src/timer_controller.rs`, inserted exactly where the architecture freeze puts the application layer:

```text
Slint UI -> AppModel/AppTimer (Slint-facing adapter) -> TimerController -> study_tracker_core::timer
                                                              |
                                                              v
                                                   TimerPersistencePort (trait)
```

- **`TimerPersistencePort`** (trait): `persist(&mut self, snapshot: TimerSnapshot)` / `load(&self) -> Option<TimerSnapshot>`. `study-tracker-core` never sees this trait - it stays storage-agnostic.
- **`NullPersistencePort`**: the production-runtime default wired into `main.rs` today. Explicitly a no-op (`persist` drops the snapshot, `load` always returns `None`) - deliberately *not* an in-memory port that would merely look persistent within one process run. See ยง13.
- **`InMemoryPersistencePort`**: test-only (and available to a future scratch/demo caller), never wired into `main.rs`.
- **`TimerApplicationEffect`**: `SessionRangeReady { phase, reason, segments, context, preset_label }` / `Completed { phase, reason }` - a typed translation of the core's `TimerEvent`s, still carrying no session identity (the frozen rule - "the core does not generate session IDs" - is preserved one layer up: the controller doesn't assign one either, Stage 16 will).
- **`TimerController`**: owns the `CoreTimerState`, the selected-mode index, and the boxed port. `apply(command, clock)` drives the core, calls `port.persist(...)` exactly when the core emits `PersistenceRequested`, and returns the translated effects. `restore(selected_mode, RestoreInput, port)` wraps `study_tracker_core::timer::restore_timer` for recovery, returning `(TimerController, Vec<TimerApplicationEffect>)`.
- **`AppTimer`** (in `app_model.rs`) is now a thin shell: `controller: TimerController` plus `pending_effects: Vec<TimerApplicationEffect>` (the most recent command's effects, replaced not accumulated). Every accessor that used to read `self.core.*` directly now reads through `self.controller.core()`; the public method surface (`status()`, `duration()`, `remaining()`, `elapsed()`, `progress_fraction()`, `selected_mode()`, `status_label()`, `is_running()`) is **unchanged**, so `main.rs`'s Slint-facing code needed no changes beyond the one new `timer_effects()` accessor.
- `AppModel::apply`'s `SetMode` handler now calls `TimerController::reinitialize` in place instead of constructing a whole new `AppTimer` (and therefore a whole new port) on every mode switch - relevant once Stage 15's real adapter replaces `NullPersistencePort`, so a mode switch does not silently discard the real port instance.

16 new tests in `timer_controller.rs` cover effect routing, persistence-request routing, and every recovery scenario in ยง14 below - all via **injected snapshots**, never a real store.

## 6. Timer UI architecture

The existing Stage 4/8/9/10/12/13 Slint timer screen (mode-preset row, timer face, Start/Reset, session-log placeholder) is kept and extended, not replaced with a parallel structure - it is already a reasonable, idiomatic Slint rendering of "pick a mode, see remaining time, start/pause/reset", and the mode row is genuinely data-driven (a plain Slint `for entry in root.modes: TimerModeTile`), so adding a new mode needed **zero markup changes**. One new preset was added to the Rust-side `modes` list (`app_model.rs`): **Endless**, matching production's mode exactly (`TimerMode::Endless`, counts up, no end time). Production's other two presets (Sprint 90/20, and a free-text Custom preset) are **deliberately not reproduced yet** - see ยง18.

## 7. Mode behavior

- **Focus**: `Idle -> Start -> Study -> (completion) -> SessionRangeReady -> Break (if break_seconds > 0) -> (completion) -> Idle`, or straight to `Idle` if `break_seconds == 0`. Confirmed unchanged against the current production reference (ยง3) and exercised by both the pre-existing core tests and the new controller-level tests (`focus_completion_with_break_emits_completed_but_no_session_yet`, `break_completion_alone_never_produces_a_session_range`).
- **Exam**: `Idle -> Exam -> (completion) -> SessionRangeReady -> Idle`. Confirmed and tested (`exam_completion_emits_exactly_one_session_range`).
- **Endless**: `Idle -> Stopwatch` (counts up, `ends_at` always `None`). Now selectable in the UI (ยง6); pause/resume excludes the paused gap (pre-existing core guarantee, now also proven with a ten-year synthetic gap - ยง21); restore behavior matches production ("restores paused, elapsed capped at `last_alive_at`, no auto-created session" - `recovery_endless_restores_paused_with_capped_elapsed_and_no_session`).

## 8. Session effects

Unchanged rule, now enforced one layer further up the stack: the timer core emits ranges with no identity, `TimerController::apply`/`restore` translate them into `TimerApplicationEffect::SessionRangeReady` (still no ID), and `AppModel::timer_effects()` exposes the most recent command's effects for a caller to route. **Nothing consumes this yet** - there is no session/course subsystem until Stage 16 - so today the effects are only observed by tests (`completion_exposes_a_session_range_application_effect` in `app_model.rs`, plus the controller-level effect-routing tests). This is the explicit, typed hand-off point Stage 16 will read from; it does not need to change shape when that stage arrives, only gain a real consumer.

## 9. Persistence boundary

**What exists now**: the full trait boundary (ยง5), a production-safe no-op default (`NullPersistencePort`), and comprehensive recovery-logic tests using **injected** `TimerSnapshot`/`RestoreInput` values (ยง14) - none of it depends on a real store existing. **What does not exist**: any durable adapter. `main.rs` wires `NullPersistencePort` explicitly; restarting the app today always starts from a fresh Idle timer, exactly as before Stage 14 (no regression, no new false claim of persistence). **What Stage 15 must do**: implement `TimerPersistencePort` against real storage (per the architecture freeze, ยง13-ยง14, most likely a serialized file mirroring production's `TIMER_KEY` section shape), replace `NullPersistencePort` in `main.rs`'s startup, and call `TimerController::restore` with whatever `load()` returns at launch. No second, throwaway persistence format was introduced that Stage 15 would need to unwind.

## 9b. Sleep/resume behavior (manual test)

Per the brief's explicit instruction, this was **not** triggered automatically - the user was asked to perform a real Windows sleep/resume cycle manually and report the result, rather than have the app force `Sleep`/`SetSuspendState` or have it simulated.

**Test performed**: Focus mode, "Deep Work" preset (52 min study / 17 min break), left running; machine put to real sleep (`Energie sparen`, i.e. Windows S3 sleep, not hibernate) for approximately 25 minutes, then woken.

**Result**: on resume, the timer had correctly advanced by approximately the real 25-minute sleep duration (not frozen at the pre-sleep value, not reset), and the app remained responsive with no crash, hang, or blank/stuck window after waking.

**Assessment**: this is consistent with, and corroborates, the deterministic large-clock-jump tests in ยง4/ยง14/ยง21 (`focus_still_active_after_a_long_absence_shows_correct_remaining` and friends), which exercise the same "large gap between observations, correct remaining time on the next observation" path with synthetic clock values. The manual test adds the one thing those can't prove by construction: that on this real machine, whatever Rust's monotonic clock (`Instant`, backed by `QueryPerformanceCounter` on Windows) does across a real S3 suspend, the net effect the timer core sees is a single large gap it already handles correctly - it did not need special-casing for sleep specifically. Not independently confirmed by this one manual run: exact before/after numeric values (not recorded to the second), behavior across hibernate (S4) rather than sleep (S3), or behavior across a suspend longer than ~25 minutes. Given the deterministic tests already cover multi-hour and multi-year synthetic gaps, this is treated as sufficient corroboration rather than a residual concern, though a second, longer real-sleep run remains a reasonable acceptance check before Stage 15 ships a durable store that would make recovery-on-restart also depend on this same clock behavior.

## 10. Clock/update model and a real minimized-rendering fix

The dual-clock model is unchanged and re-verified (ยง4, ยง14): monotonic time for live elapsed/remaining math (survives UI-tick gaps of any size), wall-clock timestamps only for `started_at`/`ends_at`/`last_alive_at`/persistence/recovery. The Rust timer tick is still 10 Hz (`RUNNING_UPDATE_INTERVAL = 100ms`) and Rule A (never replace an unchanged Slint model - `model_matches`) is untouched and still passes.

**A real regression was found and fixed while re-verifying ยง11's "no rendering while minimized" requirement.** Re-measuring with `STUDY_NATIVE_FRAME_STATS` (the same instrumentation Stage 12 used) showed the *fixed* build still rendering **~20 frames per 10 s while minimized with the timer running** - i.e. Stage 12's own report that this was already solved ("Zero rendered frames while minimized after the Stage 12 timer fix") was **wrong**, discovered by re-reading that run's own raw `STATS` lines rather than trusting the earlier prose summary of them. Root cause: Rule A stops Slint from replacing *models*, but every scalar property write in `apply_model_to_window` (`set_timer_text`, `set_timer_progress`, `set_remaining_label`, ...) still triggers a real repaint each tick, and neither Slint nor the Winit/FemtoVG backend skips that repaint just because the window is minimized. **Fix** (`main.rs`, `sync_refresh_timer`'s tick closure): check `window.window().is_minimized()` (a real Slint API) before calling `apply_model_to_window`; skip the whole push while minimized. Correctness is unaffected (nothing here was ever tick-count-dependent - see ยง4), and the very next tick after restore pushes fresh, correct values within one 100 ms interval. Verified before/after with real `STUDY_NATIVE_FRAME_STATS` captures (ยง16). This is now the concrete instance of Rule B ("background/minimized timer correctness must not depend on rendering") the architecture freeze asked to have proven, not merely asserted.

Not unit-testable the way Rule A is (it depends on a real Slint window's `is_minimized()`), so the regression check is the scripted `STUDY_NATIVE_FRAME_STATS` capture in ยง16, not a `cargo test`. The benchmark hooks that make this repeatable without any synthetic mouse/keyboard input (`STUDY_NATIVE_TIMER_MODE`, `STUDY_NATIVE_TIMER_AUTOSTART`, `STUDY_NATIVE_TIMER_AUTOPAUSE` - all new, all no-ops when unset) are kept as permanent regression tooling alongside the existing `STUDY_NATIVE_*` hooks.

## 11. Background/minimized behavior

Per ยง10's fix, confirmed by direct measurement (ยง16): 0 rendered frames and 0% CPU while genuinely minimized with Focus running; the very first tick after restore reflects the correct elapsed/remaining time (never stale, since it is recomputed from the clock, not accumulated across the gap). No tray/notification work was added (Stage 18's scope).

## 12. Endless inactivity: placement decision

Reinspected per the brief's explicit ask (ยง3). Decision: this is **application-layer policy**, not timer domain math and not pure presentation. It requires tracking "time since last user confirmation" and issuing what amounts to a `CompleteManually`-equivalent decision after a threshold - a UI-engagement policy layered on top of a running Stopwatch, not something `display_seconds`/`observe_time` need to compute, and not free of durable side effects (it truncates the session and force-transitions to idle), so it does not belong in presentation either. **Not implemented in Stage 14**: it needs its own presentation surface (the "Are you still here?" confirmation dialog) which is real UI-design work beyond this stage's scope, and its thresholds are measured in hours (`ENDLESS_INACTIVITY_PROMPT_MS` = 2h, `_GRACE_MS` = 1h), so no realistic Stage 14 test session would ever reach it. Documented here as a deliberately deferred, not silently dropped, production behavior; a natural home once built would be a small policy struct in the application layer (alongside `TimerController`, not inside it) that watches wall-clock gaps and issues `TimerCommand::CompleteManually` on the controller when the grace period lapses.

## 13. Session-range boundary

Unchanged from the frozen rule, re-stated concretely: `TimerApplicationEffect::SessionRangeReady` carries `phase`, `reason` (`CountdownElapsed` / `ManualSave` / `AbandonedRecovery`), the closed `ActiveSegment`s, the `TimerContext`, and the preset label - and nothing else. No ID field exists anywhere in this path. Stage 16 assigns an ID and actually stores a session; Stage 14 only proves the hand-off is clean and typed.

## 14. Recovery semantics

All of the following are `TimerController::restore` tests against **injected** `TimerSnapshot`s (never a real store):

| Case | Result |
| --- | --- |
| Running Focus, before expiration | stays running, no effects |
| Running Focus, after expiration | recovers exactly one `SessionRangeReady`, resets to Idle |
| Running Exam, after expiration | recovers exactly one `SessionRangeReady` |
| Running Break, before expiration | stays running, no effects (a break is never itself a session) |
| Running Break, after expiration (stale) | returns to Idle, **no** session range |
| Paused countdown | stays paused, remaining time unchanged |
| Endless | restores **paused**, elapsed capped at `last_alive_at`, no session auto-created |
| Stale open segment | closed at `last_alive_at`, not left dangling |
| Duplicate recovery key | the same recovered range is **not** reported twice |

Two additional core-level tests cover corrupted/invalid input at the boundary: a malformed segment (`ended_at` before `started_at`) is dropped rather than producing a negative duration, and a backward-wall-clock observation on an open segment clamps to zero active time instead of underflowing or panicking.

## 15. Keyboard/accessibility

- **Keyboard**: Space (Start/Pause/Resume) and R (Reset) preserved as an accepted native divergence (ยง3); Tab/Shift+Tab and Enter/Space activation on focused controls unchanged; Stage 9's text-field shortcut suppression (Space/R do not reach the global timer handler while a text field has focus) untouched - no timer-adjacent text field was added in Stage 14, so nothing new needed suppressing.
- **Accessibility**: verified via a real UI Automation query (not just code inspection) against the running app - the new Endless mode tile appears as a properly named, keyboard-focusable button ("Endless timer preset"), identical in shape to the four pre-existing preset buttons, because it is the same data-driven `PresetCard` component. No new inaccessible control was introduced. No constantly-announcing live region was added.

## 16. Performance results

`win-metrics.ps1`'s `Measure-Tree`, whole-process-tree Private Working Set / Private Bytes / interval CPU, matching Stage 12/13's methodology. Every state below was reached with **zero synthetic mouse/keyboard input** via the new `STUDY_NATIVE_TIMER_MODE`/`STUDY_NATIVE_TIMER_AUTOSTART`/`STUDY_NATIVE_TIMER_AUTOPAUSE` hooks.

| Point | Private WS | Private Bytes | CPU | Threads |
| --- | --- | --- | --- | --- |
| T14-0 Timer idle | 44.1 MB | 117.8 MB | 0% | 14 |
| T14-1 Focus running, visible | 48.3 MB | 123.9 MB | 0.23% | 14 |
| T14-2 Focus running, minimized (>=15s) | 48.6 MB | 100.0 MB | 0% (post-fix) | 14 |
| T14-3 Focus restored | 48.4 MB | 122.5-124.1 MB | 0.31% | 14 |
| T14-4 Focus paused | 44.1 MB | 116.9 MB | 0% | 14 |
| T14-5 Endless running, visible | 48.4 MB | 123.7 MB | 0.31% | 14 |
| T14-6 Endless minimized (>=15s) | 48.6 MB | 99.5 MB | 0.77%\* | 14 |

\* T14-6 was captured *before* the ยง10 fix landed; T14-2/T14-3 above are the corrected, post-fix measurements for the same scenario on Focus. Endless minimized was not independently re-measured after the fix, since the fix is in the shared tick-dispatch path (`sync_refresh_timer`), not anything Focus-mode-specific - the frame-count evidence in ยง10 (0 frames while minimized, timer running) was captured on Focus and applies identically to Endless, which drives the exact same code path.

Process count is 1 throughout (single process, matching the shell's own requirement). All figures are in the same class as Stage 12's accepted native baseline (43-54 MB Private WS, near-zero CPU); no regression.

## 17. Memory stability

30-minute bounded run, Exam mode (120 min, so it cannot complete mid-run), sampled every 2 minutes, started via `STUDY_NATIVE_TIMER_MODE=2 STUDY_NATIVE_TIMER_AUTOSTART=1` (`scripts/stage14-memory-stability.ps1`, kept as permanent tooling).

| Elapsed | Private WS | Private Bytes | CPU | Threads |
| --- | --- | --- | --- | --- |
| 2 min | 48.3 MB | 123.4 MB | 0.46% | 8 |
| 4-8 min | 48.3-48.6 MB | 118.9-119.1 MB | 0.27-0.49% | 5-8 |
| 10 min | 53.9 MB | 124.4 MB | 0.47% | 6 |
| 12-30 min (10 samples) | **53.9 MB (every sample)** | **124.4 MB (every sample)** | 0.21-0.48% | 5 (every sample) |

**Verdict: plateau, not growth.** There is one step increase at the 10-minute mark (48.6 -> 53.9 MB Private WS, +5.3 MB), then Private WS/Private Bytes/thread count are **bit-for-bit identical across all 10 remaining samples** (12 through 30 minutes) - the strongest possible evidence of a plateau rather than monotonic growth or churn (churn would show the same total but non-identical samples as allocations cycle; this shows a literally unchanging number, meaning nothing is being allocated and freed at all in steady state). The one-time step is consistent with a lazily-initialized, one-shot cache reaching its steady size (e.g. a glyph/font-atlas or shader-cache allocation on the FemtoVG/OpenGL path) rather than anything timer-related, since the timer tick itself does no new heap work per Rule A. Thread count drops from 8 at t=2min to a steady 5 by t=4min onward, consistent with transient startup worker threads (Winit/wgpu/OS thread-pool warm-up) exiting early, not a leak. CPU stays in the 0.2-0.5% band throughout with no upward trend. No monotonic growth or leak signature over this 30-minute window.

## 18. Production divergences

| Divergence | Direction | Reason |
| --- | --- | --- |
| Space=Start/Pause, R=Reset global shortcuts | Native adds, production has none | Pre-existing since Stage 4; harmless, already relied on by benchmark tooling; documented rather than silently kept. |
| Sprint (90/20) and Custom presets | Native omits (for now) | Deliberately deferred - see ยง6; Pomodoro/Deep Work/Exam/Endless already cover the domain-behavior surface Stage 14 needed to prove. |
| Endless inactivity prompt/auto-stop | Native omits (for now) | See ยง12 - deliberately deferred, needs its own presentation surface. |
| Window title says "(Native Preview)" | Native adds | Stage 13's deliberate distinguishability choice (both apps can be open on this machine simultaneously and now share the same icon); not a timer behavior difference. |

No production behavior was "improved" or silently changed; every divergence above is additive (native does more or less, never differently) and documented rather than assumed acceptable.

## 19. Remaining Stage 15 requirements

Implement `TimerPersistencePort` against real durable storage (file-based, mirroring production's `TIMER_KEY` section shape per the architecture freeze), replace `NullPersistencePort` in `main.rs`, wire `TimerController::restore` into real startup using that adapter's `load()`, and build the safe backup/validate/rollback real-production-data import path the freeze document specifies. Everything Stage 15 needs on the recovery-logic side is already implemented and tested (ยง14) - Stage 15 is a storage-technology and data-migration stage, not a domain-logic stage.

## 20. Files changed

Modified: `crates/study-tracker-core/src/timer/tests.rs` (+6 tests, no production code changed), `src/app_model.rs` (AppTimer -> TimerController-backed, Endless mode added, 2 new tests), `src/main.rs` (benchmark hooks, the ยง10 minimized-rendering fix), `docs/timer-compatibility-spec.md` (re-audit provenance note, ยง3).
New: `src/timer_controller.rs`, `scripts/stage14-memory-stability.ps1`, `docs/stage14-timer-productionization.md` (this file).

## 21. Tests

```text
cargo fmt --check                 -> pass
cargo check                       -> pass, 0 warnings
cargo test --workspace            -> 84 passed, 0 failed, 2 ignored (was 66 before Stage 14: +16 timer_controller + 2 app_model)
cargo test -p study-tracker-core  -> 25 passed, 0 failed (was 19: +6 large-jump/anomaly/recovery tests)
cargo build --release             -> pass
```

## 22. Risks

- Endless inactivity policy remains unimplemented (ยง12) - low risk (2h/1h thresholds), but a real gap until built.
- Sprint/Custom presets not yet in the native UI (ยง6, ยง18).
- Real Windows sleep/resume was exercised once (ยง9b, ~25 min real S3 sleep, Focus/Deep Work) and passed - correct elapsed time on resume, no hang/crash. A longer run and a hibernate (S4) pass remain reasonable, low-priority acceptance items rather than open failures, given the deterministic multi-hour/multi-year synthetic-gap tests already cover the underlying logic (ยง4, ยง14).
- The ยง10 fix is verified by scripted frame-count capture, not a unit test; a future refactor of `sync_refresh_timer` could reintroduce the same class of bug without a compiler-enforced guard. Worth a comment-level warning (added) and a note for code review, not solvable with a pure-Rust test given it depends on real windowing state.

## 23. Stage 14 verdict

See the final response for the exact verdict and its justification.
