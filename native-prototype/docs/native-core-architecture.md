# Native Core Architecture

Stage 7 establishes the renderer-independent Rust core boundary for a future native Study Tracker. The timer is the first bounded domain used to prove the architecture.

## Dependency Direction

```text
Slint UI
  -> native application adapter
    -> study-tracker-core
      -> domain outputs
        -> persistence / network / platform adapters
```

The reverse direction is forbidden. `study-tracker-core` must not depend on Slint, Winit, FemtoVG, Tauri, React, WebView, async runtimes, networking clients, or OS GUI/platform APIs.

The current repository structure is:

```text
native-prototype/
  Cargo.toml                         workspace root and Slint prototype package
  crates/study-tracker-core/         renderer-independent domain crate
  src/                               current Slint/application adapter prototype
  ui/                                Slint presentation files
```

`study-tracker-core` is intentionally usable in headless unit tests. It currently depends only on `serde` for serializable DTOs.

## Ownership

- `crates/study-tracker-core/src/timer/` owns timer domain state, commands, transitions, persistence snapshots, restoration policy, and deterministic tests.
- `native-prototype/src/` remains the Slint-facing application adapter for the Stage 4 visual prototype. It may depend on `study-tracker-core`, but the existing screen is not rewired in Stage 7.
- `native-prototype/ui/` owns presentation only.
- Future persistence adapters will read/write `TimerSnapshot` but must not live in the core crate.
- Future network and platform adapters will translate domain outputs into Cloudflare calls, tray updates, notifications, file dialogs, updater actions, and OS integration.

## Timer State Machine

The core models production timer concepts rather than the simpler Stage 4 demo timer:

- Modes: `Focus`, `Exam`, `Endless`.
- Phases: `Idle`, `Study`, `Break`, `Exam`, `Stopwatch`.
- Active segments record wall-clock ranges where study/exam/stopwatch work was active.
- Pause closes the open segment.
- Resume opens a new segment and excludes the paused gap.
- Focus completion emits a session range and starts a break when a break duration is configured.
- Exam completion emits an exam session range and resets idle.
- Break completion resets idle without emitting a study session range.
- Manual completion emits finalized study/exam/stopwatch ranges and resets idle; session IDs remain outside the core.
- Endless/stopwatch has no natural completion and restores paused after restart.

Legal transitions are deterministic and do not rely on UI controls to prevent invalid commands:

```text
Idle -> Running
Running -> Paused
Paused -> Running
Running -> Completed
Running -> Idle on reset
Paused -> Idle on reset
Idle -> Idle on repeated reset
Running -> Running on repeated start
Paused -> Paused on repeated pause
```

## Clock Strategy

Live runtime behavior uses `ClockObservation`, which contains both monotonic and wall-clock observations. Countdown display while the process is alive is derived from monotonic elapsed time, so correctness does not depend on UI callback frequency or wall-clock drift during one run.

Persistence and restoration use wall-clock timestamps because monotonic anchors cannot survive process restart. `TimerSnapshot` stores wall timestamps for `started_at`, `ends_at`, active segments, and `last_alive_at`.

Tests construct `ClockObservation` values directly. No timer-domain tests sleep or read the real clock.

## Events And Effects

The core returns explicit `TimerEvent` values. It does not perform side effects.

Events include:

- `Started`
- `Paused`
- `Resumed`
- `Reset`
- `BreakStarted`
- `Completed`
- `SessionRangeReady`
- `PersistenceRequested`

Adapters are responsible for acting on these outputs. For example, `SessionRangeReady` contains ranges and context; an application service, not the core, creates session IDs, writes persistence, posts social updates, plays audio, or shows notifications.

## Persistence Boundary

`TimerState` is runtime/domain state. `TimerSnapshot` is the serializable DTO. The core can convert state to a snapshot and restore state from a snapshot, but it cannot read or write storage.

Production-compatible restart behavior is preserved:

- Running countdown with a recent heartbeat remains running and derives remaining time from `ends_at`.
- Expired study/exam countdown emits a recovered session range and resets idle.
- Stale study/exam state closes open segments at `last_alive_at` when available.
- Endless/stopwatch restores paused, caps elapsed time at `last_alive_at`, and does not auto-create a session.
- Abandoned study/exam timers use the production six-hour inactivity threshold.

Future persistence adapters may map production ISO strings to `WallTimestamp` and back. Stage 7 does not migrate production data or alter production localStorage.

## Networking Boundary

The core does not know Cloudflare, HTTP, auth, social sync, telemetry, or leaderboards. Future online functionality should receive domain outputs or state snapshots from an application service and return explicit service results.

## Platform Integration Boundary

Tray, notifications, audio, updater, file dialogs, clipboard, drag/drop, and vault integration are platform adapters. They must not enter `study-tracker-core`. The application adapter decides which platform effects to trigger from domain events.

## Testing Strategy

The core has deterministic unit tests for:

- Initial state.
- Start/pause/resume/reset.
- Countdown and stopwatch elapsed calculations.
- No negative countdown display.
- Completion and break transitions.
- Repeated commands.
- Mode-change blocking while active.
- Persistence snapshots.
- Restart/recovery behavior.
- Duplicate recovered-session prevention.

Future domains should follow the same pattern: plain Rust state, explicit commands, explicit outputs, no sleeps, and no platform dependencies.

## Migration Strategy

Future native migration should extract renderer-independent behavior before UI parity work:

1. Timer domain and persistence snapshot compatibility.
2. Course/task/semester/exam domain model.
3. Statistics computation.
4. API/social client state model.
5. Slint screens bound through thin adapters.

Do not mechanically translate React components one by one. Keep production source as behavioral reference until native parity is proven.
