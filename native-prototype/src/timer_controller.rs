//! Application-layer timer controller (Stage 14).
//!
//! This sits between the Slint-facing adapter (`app_model.rs`/`main.rs`) and
//! `study_tracker_core::timer`, exactly where the architecture freeze puts it:
//!
//! ```text
//! Slint UI -> native application adapter -> TimerController -> study-tracker-core
//!                                                  |
//!                                                  v
//!                                       TimerPersistencePort (Stage 15 implements the real one)
//! ```
//!
//! It owns no timer *domain* logic of its own - every state transition still goes through
//! `study_tracker_core::timer::TimerState::apply`/`restore_timer`. What it adds is the boundary
//! Stage 14 is required to establish without building a throwaway persistence implementation:
//! translating the core's untyped `PersistenceRequested` event into an actual call on a real
//! (trait-typed) port, and translating `SessionRangeReady`/`Completed` into an explicit,
//! typed application effect a future session subsystem (Stage 16) can consume, instead of a raw
//! `TimerEvent` leaking further up the stack.
//!
//! **What is real and final here**: the port trait's shape, the effect enum, the controller's
//! command/restore/query API, and every test in this module (including all of them exercised
//! against *injected* snapshots, per the Stage 14 brief - recovery correctness does not wait for
//! Stage 15's durable adapter to be tested).
//!
//! **What is explicitly not final**: [`NullPersistencePort`], the only port wired into `main.rs`
//! today. It performs no I/O and does not survive a restart. That is intentional - Stage 15 owns
//! the real, durable adapter (a file-backed store, per `docs/stage12_5-architecture-freeze.md`
//! section 13-section 14), and it must implement [`TimerPersistencePort`] and replace `NullPersistencePort` in
//! `main.rs`, not introduce a second, parallel persistence architecture.

#[cfg(test)]
use study_tracker_core::timer::TimerMode;
use study_tracker_core::timer::{
    restore_timer, ClockObservation, CompletionReason, RestoreInput, TimerCommand, TimerConfig,
    TimerContext, TimerEvent, TimerPhase, TimerSnapshot, TimerState as CoreTimerState,
};

/// The real persistence boundary Stage 14 establishes. `study-tracker-core` never sees this
/// trait (it stays storage-agnostic, per the frozen dependency direction); only this
/// application-layer module and its caller do.
///
/// A port's `persist` is called once per [`TimerEvent::PersistenceRequested`] the core emits
/// (start/pause/resume/reset/completion) - never on every tick, since `ObserveTime` while nothing
/// has changed emits no events at all. `load` is provided for symmetry and for a future adapter
/// that reads its own last-written snapshot on startup; `TimerController::restore` does not call
/// it itself; the caller decides when a restore should happen and supplies the snapshot to
/// recover from (this keeps recovery fully unit-testable with injected snapshots, per the brief).
pub trait TimerPersistencePort {
    fn persist(&mut self, snapshot: TimerSnapshot);
    // Not called by anything in Stage 14 (see the doc comment above); real callers arrive with
    // Stage 15's durable adapter and, later, `main.rs` calling `TimerController::restore` at
    // startup. This is a `bin`-only crate, so rustc's dead-code pass sees the whole program and
    // flags an uncalled trait method even with real implementations; kept deliberately rather
    // than deleted.
    #[allow(dead_code)]
    fn load(&self) -> Option<TimerSnapshot>;
}

/// The production-runtime default until Stage 15. Explicitly a no-op, not a disguised durable
/// store: `persist` drops the snapshot, `load` always returns `None` (so the app always starts
/// from a fresh Idle timer, exactly like it does today). This is deliberately distinct from an
/// in-memory port that merely *looks* persistent within one process run - such a port would
/// invite a future reader to assume it does something a fresh process restart would immediately
/// disprove. Restart persistence is Stage 15's job, not simulated here.
#[derive(Debug, Default)]
pub struct NullPersistencePort;

impl TimerPersistencePort for NullPersistencePort {
    fn persist(&mut self, _snapshot: TimerSnapshot) {}
    fn load(&self) -> Option<TimerSnapshot> {
        None
    }
}

/// A deterministic, in-memory port used only by tests (and available to any future caller that
/// genuinely wants an in-process-only store, e.g. a scratch/demo build) - never wired into
/// `main.rs`'s normal startup path.
#[allow(dead_code)] // used by this module's own tests; kept public for a future test/demo caller
#[derive(Debug, Default)]
pub struct InMemoryPersistencePort {
    last: Option<TimerSnapshot>,
}

impl TimerPersistencePort for InMemoryPersistencePort {
    fn persist(&mut self, snapshot: TimerSnapshot) {
        self.last = Some(snapshot);
    }
    fn load(&self) -> Option<TimerSnapshot> {
        self.last.clone()
    }
}

/// Typed output the application layer hands further up the stack, instead of a raw
/// [`TimerEvent`]. Session ranges carry no identity (matching the frozen rule: "the timer core
/// does not generate session IDs" - preserved here one layer up, not just in core); assigning an
/// ID and actually storing a session is Stage 16's job (the session/course domain), which is why
/// this effect is only *routed*, never consumed, by Stage 14.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerApplicationEffect {
    /// A Study/Exam/Endless block produced a completed range. `reason` distinguishes a natural
    /// countdown completion, a manual save, or a recovered (abandoned-timer) range - Stage 16
    /// needs this to decide how to label the resulting session.
    SessionRangeReady {
        phase: TimerPhase,
        reason: CompletionReason,
        segments: Vec<study_tracker_core::timer::ActiveSegment>,
        context: TimerContext,
        preset_label: String,
    },
    /// A phase transition or terminal state a UI may want to react to beyond the plain
    /// mode/phase/running fields already on `TimerController` (e.g. flashing a "completed"
    /// affordance). Carries no new information beyond the core's own `Completed` event; kept
    /// distinct from `SessionRangeReady` because a Break completion produces this without a
    /// session (breaks are never logged as study sessions, matching production).
    Completed {
        phase: TimerPhase,
        reason: CompletionReason,
    },
}

/// Application-layer timer state: the core plus the bookkeeping needed to answer UI-facing
/// questions (`status()`, `duration()`, ...) that the raw `TimerState` intentionally does not
/// expose (those are presentation concerns, not domain math). This replaces the old `AppTimer`
/// (Stage 4-13); its public read API is unchanged so `main.rs`'s existing Slint-facing code did
/// not need to change.
pub struct TimerController {
    selected_mode: usize,
    core: CoreTimerState,
    last_completion: Option<TimerPhase>,
    persistence: Box<dyn TimerPersistencePort>,
}

impl std::fmt::Debug for TimerController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimerController")
            .field("selected_mode", &self.selected_mode)
            .field("core", &self.core)
            .field("last_completion", &self.last_completion)
            .finish_non_exhaustive()
    }
}

// `Box<dyn TimerPersistencePort>` cannot derive `PartialEq`; the controller is compared by its
// observable domain state only, which is what every existing test (and the Slint adapter) cares
// about. Two controllers with different port instances but identical timer state are equal.
impl PartialEq for TimerController {
    fn eq(&self, other: &Self) -> bool {
        self.selected_mode == other.selected_mode
            && self.core == other.core
            && self.last_completion == other.last_completion
    }
}
impl Eq for TimerController {}
impl Clone for TimerController {
    fn clone(&self) -> Self {
        Self {
            selected_mode: self.selected_mode,
            core: self.core.clone(),
            last_completion: self.last_completion,
            persistence: Box::new(NullPersistencePort),
        }
    }
}

impl TimerController {
    pub fn new(
        selected_mode: usize,
        config: TimerConfig,
        persistence: Box<dyn TimerPersistencePort>,
    ) -> Self {
        Self {
            selected_mode,
            core: CoreTimerState::new(config, TimerContext::default()),
            last_completion: None,
            persistence,
        }
    }

    /// Builds a controller from a previously persisted snapshot (Stage 14's recovery boundary -
    /// see the module docs: the snapshot is *injected* here, never read from a real store yet).
    /// Returns the resulting effects alongside the controller so a caller can route any
    /// `SessionRangeReady` recovered from an abandoned timer exactly like a live completion.
    /// Exercised extensively by this module's tests; not yet called from `main.rs`, since there
    /// is no durable snapshot to restore from until Stage 15 - real startup wiring is one line
    /// once that adapter's `load()` returns something.
    #[allow(dead_code)]
    pub fn restore(
        selected_mode: usize,
        input: RestoreInput,
        persistence: Box<dyn TimerPersistencePort>,
    ) -> (Self, Vec<TimerApplicationEffect>) {
        let outcome = restore_timer(input);
        let controller = Self {
            selected_mode,
            core: outcome.timer,
            last_completion: None,
            persistence,
        };
        (controller, translate(&outcome.events))
    }

    /// Applies one command, drives the core, routes `PersistenceRequested` to the port, and
    /// returns the typed application effects (everything else the core emitted).
    pub fn apply(
        &mut self,
        command: TimerCommand,
        clock: ClockObservation,
    ) -> Vec<TimerApplicationEffect> {
        let events = self.core.apply(command, clock);
        self.track_completion(&events);
        for event in &events {
            if matches!(event, TimerEvent::PersistenceRequested) {
                self.persistence
                    .persist(self.core.snapshot_for_persistence(clock));
            }
        }
        translate(&events)
    }

    fn track_completion(&mut self, events: &[TimerEvent]) {
        for event in events {
            if let TimerEvent::Completed { phase, reason } = event {
                if *reason == CompletionReason::CountdownElapsed {
                    self.last_completion = Some(*phase);
                }
            }
            if matches!(
                event,
                TimerEvent::Started { .. } | TimerEvent::Resumed { .. } | TimerEvent::Reset
            ) {
                self.last_completion = None;
            }
        }
    }

    /// Writes the controller's current state to the port immediately, bypassing the usual "only
    /// on a core-emitted `PersistenceRequested` event" rule. Used exactly once by the Stage 15
    /// adapter layer, right after [`TimerController::restore`]: writing the recovered/reset (or
    /// merely time-adjusted) state back immediately closes the window where an unclean exit right
    /// after a restart could otherwise see the exact same stale snapshot on the *next* restart and
    /// recover the exact same abandoned session a second time (see
    /// `docs/stage15-persistence-migration.md`, "Idempotence"). Not called anywhere else -
    /// ordinary running/paused ticks must keep going through `apply`, never this.
    pub fn force_persist(&mut self, clock: ClockObservation) {
        self.persistence
            .persist(self.core.snapshot_for_persistence(clock));
    }

    pub fn selected_mode(&self) -> usize {
        self.selected_mode
    }

    pub fn core(&self) -> &CoreTimerState {
        &self.core
    }

    pub fn last_completion(&self) -> Option<TimerPhase> {
        self.last_completion
    }

    /// Replaces the whole controller in place for a `SetMode`-style mode switch (mirrors the old
    /// `AppTimer::new` used by `app_model.rs`'s `SetMode` handler, which already only fires while
    /// the core is Idle and not running - the core's own `set_mode` guard is the real authority;
    /// this is just how the adapter picks a different starting `TimerConfig`/preset).
    pub fn reinitialize(&mut self, selected_mode: usize, config: TimerConfig) {
        self.selected_mode = selected_mode;
        self.core = CoreTimerState::new(config, TimerContext::default());
        self.last_completion = None;
    }
}

fn translate(events: &[TimerEvent]) -> Vec<TimerApplicationEffect> {
    events
        .iter()
        .filter_map(|event| match event {
            TimerEvent::SessionRangeReady {
                phase,
                reason,
                segments,
                context,
                preset_label,
            } => Some(TimerApplicationEffect::SessionRangeReady {
                phase: *phase,
                reason: *reason,
                segments: segments.clone(),
                context: context.clone(),
                preset_label: preset_label.clone(),
            }),
            TimerEvent::Completed { phase, reason } => Some(TimerApplicationEffect::Completed {
                phase: *phase,
                reason: *reason,
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::timer::{ActiveSegment, WallTimestamp};

    fn clock(seconds: u64) -> ClockObservation {
        ClockObservation::new(seconds * 1000, seconds as i64 * 1000)
    }
    fn wall(seconds: i64) -> WallTimestamp {
        WallTimestamp::from_unix_millis(seconds * 1000)
    }

    fn focus_config() -> TimerConfig {
        TimerConfig::default()
    }
    fn exam_config() -> TimerConfig {
        let mut config = TimerConfig::default();
        config.mode = TimerMode::Exam;
        config
    }
    fn endless_config() -> TimerConfig {
        let mut config = TimerConfig::default();
        config.mode = TimerMode::Endless;
        config
    }

    fn controller(config: TimerConfig) -> TimerController {
        TimerController::new(0, config, Box::new(NullPersistencePort))
    }

    // --- Effect routing --------------------------------------------------------------------

    #[test]
    fn starting_and_pausing_produce_no_application_effects() {
        let mut ctl = controller(focus_config());
        assert!(ctl.apply(TimerCommand::Start, clock(0)).is_empty());
        assert!(ctl.apply(TimerCommand::Pause, clock(10)).is_empty());
        assert!(ctl.apply(TimerCommand::Resume, clock(20)).is_empty());
    }

    #[test]
    fn focus_completion_with_break_emits_completed_but_no_session_yet() {
        // Study -> Break is not a session boundary; the session is only ready once the whole
        // Focus block (study, optionally followed by break) finishes.
        let mut ctl = controller(focus_config()); // 25 min study, 5 min break
        ctl.apply(TimerCommand::Start, clock(0));
        let effects = ctl.apply(TimerCommand::ObserveTime, clock(25 * 60));
        assert_eq!(
            effects,
            vec![
                TimerApplicationEffect::SessionRangeReady {
                    phase: TimerPhase::Study,
                    reason: CompletionReason::CountdownElapsed,
                    segments: vec![ActiveSegment {
                        started_at: wall(0),
                        ended_at: Some(wall(25 * 60)),
                    }],
                    context: TimerContext::default(),
                    preset_label: "Pomodoro 25/5".into(),
                },
                TimerApplicationEffect::Completed {
                    phase: TimerPhase::Study,
                    reason: CompletionReason::CountdownElapsed
                }
            ],
        );
        assert_eq!(ctl.core().phase, TimerPhase::Break);
    }

    #[test]
    fn break_completion_alone_never_produces_a_session_range() {
        let mut ctl = controller(focus_config());
        ctl.apply(TimerCommand::Start, clock(0));
        ctl.apply(TimerCommand::ObserveTime, clock(25 * 60));
        let effects = ctl.apply(TimerCommand::ObserveTime, clock(25 * 60 + 5 * 60));
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. })),
            "a break finishing must never itself be reported as a study session"
        );
        assert_eq!(ctl.core().phase, TimerPhase::Idle);
    }

    #[test]
    fn exam_completion_emits_exactly_one_session_range() {
        let mut ctl = controller(exam_config());
        ctl.apply(TimerCommand::Start, clock(0));
        let effects = ctl.apply(TimerCommand::ObserveTime, clock(90 * 60));
        let ranges = effects
            .iter()
            .filter(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. }))
            .count();
        assert_eq!(ranges, 1);
    }

    // --- Persistence-request routing --------------------------------------------------------

    /// A port that records every call, so tests can assert exactly how many times (and with
    /// what) `persist` was invoked - the real thing this application layer must get right, since
    /// Stage 15's durable adapter will pay a real I/O cost per call.
    #[derive(Default)]
    struct RecordingPort {
        calls: Vec<TimerSnapshot>,
    }
    impl TimerPersistencePort for RecordingPort {
        fn persist(&mut self, snapshot: TimerSnapshot) {
            self.calls.push(snapshot);
        }
        fn load(&self) -> Option<TimerSnapshot> {
            self.calls.last().cloned()
        }
    }

    #[test]
    fn persistence_port_is_called_once_per_mutating_command_and_reflects_the_latest_state() {
        let mut recorder = RecordingPort::default();
        // `TimerController` owns the port as a trait object, so a plain struct can't observe the
        // calls from outside; drive the port directly (its own `persist`) the same way
        // `TimerController::apply` does, proving the routing logic in isolation from the
        // core-generated events it is fed. `core_events_request_persistence_exactly_when_expected`
        // below proves those events are generated only on real transitions.
        let mut core = CoreTimerState::new(focus_config(), TimerContext::default());
        for (command, at) in [
            (TimerCommand::Start, 0),
            (TimerCommand::Pause, 10),
            (TimerCommand::Resume, 20),
            (TimerCommand::Reset, 30),
        ] {
            let events = core.apply(command, clock(at));
            for event in &events {
                if matches!(event, TimerEvent::PersistenceRequested) {
                    recorder.persist(core.snapshot_for_persistence(clock(at)));
                }
            }
        }
        assert_eq!(recorder.calls.len(), 4, "one persist per mutating command");
        assert_eq!(
            recorder.calls.last().unwrap().phase,
            TimerPhase::Idle,
            "reset was the last command"
        );
    }

    #[test]
    fn core_events_request_persistence_exactly_when_expected() {
        // The actual guarantee `TimerController::apply` depends on: `PersistenceRequested` is
        // emitted alongside every real transition and is absent from a no-op `ObserveTime`.
        let mut core = CoreTimerState::new(focus_config(), TimerContext::default());
        let start = core.apply(TimerCommand::Start, clock(0));
        assert!(start.contains(&TimerEvent::PersistenceRequested));
        let idle_observe = core.apply(TimerCommand::ObserveTime, clock(60));
        assert!(
            !idle_observe.contains(&TimerEvent::PersistenceRequested),
            "an ObserveTime with time remaining must not request persistence"
        );
    }

    #[test]
    fn controller_apply_forwards_persistence_requests_into_the_boxed_port() {
        use std::cell::RefCell;
        use std::rc::Rc;

        // Shared interior-mutable log so the test can observe calls made through the
        // `Box<dyn TimerPersistencePort>` `TimerController` owns, without needing `Any`-downcast.
        struct SharedLogPort(Rc<RefCell<Vec<TimerSnapshot>>>);
        impl TimerPersistencePort for SharedLogPort {
            fn persist(&mut self, snapshot: TimerSnapshot) {
                self.0.borrow_mut().push(snapshot);
            }
            fn load(&self) -> Option<TimerSnapshot> {
                self.0.borrow().last().cloned()
            }
        }

        let log = Rc::new(RefCell::new(Vec::new()));
        let mut ctl = TimerController::new(0, focus_config(), Box::new(SharedLogPort(log.clone())));
        ctl.apply(TimerCommand::Start, clock(0));
        ctl.apply(TimerCommand::ObserveTime, clock(30)); // no-op: nothing pending, no state change
        ctl.apply(TimerCommand::Pause, clock(40));

        let calls = log.borrow();
        assert_eq!(
            calls.len(),
            2,
            "Start and Pause each persist once; the ObserveTime must not"
        );
        assert_eq!(calls[1].phase, TimerPhase::Study);
        assert!(!calls[1].running);
    }

    #[test]
    fn force_persist_writes_immediately_without_a_command() {
        let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        struct SharedLogPort(std::rc::Rc<std::cell::RefCell<Vec<TimerSnapshot>>>);
        impl TimerPersistencePort for SharedLogPort {
            fn persist(&mut self, snapshot: TimerSnapshot) {
                self.0.borrow_mut().push(snapshot);
            }
            fn load(&self) -> Option<TimerSnapshot> {
                self.0.borrow().last().cloned()
            }
        }
        let mut ctl = TimerController::new(0, focus_config(), Box::new(SharedLogPort(log.clone())));
        assert!(
            log.borrow().is_empty(),
            "construction alone must not persist"
        );
        ctl.force_persist(clock(0));
        assert_eq!(log.borrow().len(), 1);
    }

    // --- Recovery: every case fed as an injected snapshot, per the Stage 14 brief -----------

    fn snapshot_running_countdown(
        phase: TimerPhase,
        config: TimerConfig,
        started: i64,
        ends: i64,
    ) -> TimerSnapshot {
        TimerSnapshot {
            phase,
            mode: config.mode,
            remaining_seconds: (ends - started) as u64,
            logged_split_seconds: 0,
            active_segments: vec![ActiveSegment {
                started_at: wall(started),
                ended_at: None,
            }],
            running: true,
            config,
            context: TimerContext::default(),
            started_at: Some(wall(started)),
            ends_at: Some(wall(ends)),
            last_alive_at: Some(wall(started)),
        }
    }

    #[test]
    fn recovery_running_focus_before_expiration_stays_running() {
        let snapshot = snapshot_running_countdown(TimerPhase::Study, focus_config(), 0, 1500);
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(100), // heartbeat still fresh, well before the 1500s deadline
            },
            Box::new(NullPersistencePort),
        );
        assert!(effects.is_empty());
        assert!(ctl.core().running);
        assert_eq!(ctl.core().phase, TimerPhase::Study);
    }

    #[test]
    fn recovery_running_focus_after_expiration_recovers_a_session_and_resets() {
        let snapshot = snapshot_running_countdown(TimerPhase::Study, focus_config(), 0, 1500);
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(2000), // long past the 1500s deadline
            },
            Box::new(NullPersistencePort),
        );
        assert!(effects
            .iter()
            .any(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. })));
        assert_eq!(ctl.core().phase, TimerPhase::Idle);
        assert!(!ctl.core().running);
    }

    #[test]
    fn recovery_running_exam_after_expiration_recovers_a_session() {
        let snapshot = snapshot_running_countdown(TimerPhase::Exam, exam_config(), 0, 5400);
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(10_000),
            },
            Box::new(NullPersistencePort),
        );
        assert!(effects
            .iter()
            .any(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. })));
        assert_eq!(ctl.core().phase, TimerPhase::Idle);
    }

    #[test]
    fn recovery_running_break_before_expiration_stays_running() {
        let mut config = focus_config();
        let snapshot = snapshot_running_countdown(
            TimerPhase::Break,
            {
                config.mode = TimerMode::Focus;
                config
            },
            0,
            300,
        );
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(100),
            },
            Box::new(NullPersistencePort),
        );
        assert!(effects.is_empty());
        assert_eq!(ctl.core().phase, TimerPhase::Break);
    }

    #[test]
    fn recovery_running_break_after_expiration_returns_idle_without_a_session() {
        // Break is not running/ends_at-guarded the same way as Study/Exam in `restore_timer`
        // (only Study/Exam get the "recover a session" branch); a Break simply falls through to
        // idle via the stale-inactivity path once its own last-activity look-back applies. This
        // test locks in that a Break recovery never fabricates a study session either way.
        let mut config = focus_config();
        config.mode = TimerMode::Focus;
        let snapshot = TimerSnapshot {
            phase: TimerPhase::Break,
            mode: TimerMode::Focus,
            remaining_seconds: 0,
            logged_split_seconds: 0,
            active_segments: Vec::new(),
            running: false,
            config,
            context: TimerContext::default(),
            started_at: Some(wall(0)),
            ends_at: None,
            last_alive_at: Some(wall(0)),
        };
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(7 * 3600), // past the 6-hour abandoned threshold
            },
            Box::new(NullPersistencePort),
        );
        assert!(!effects
            .iter()
            .any(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. })));
        assert_eq!(ctl.core().phase, TimerPhase::Idle);
    }

    #[test]
    fn recovery_paused_countdown_is_left_paused() {
        let snapshot = TimerSnapshot {
            phase: TimerPhase::Study,
            mode: TimerMode::Focus,
            remaining_seconds: 900,
            logged_split_seconds: 0,
            active_segments: vec![ActiveSegment {
                started_at: wall(0),
                ended_at: Some(wall(600)),
            }],
            running: false,
            config: focus_config(),
            context: TimerContext::default(),
            started_at: Some(wall(0)),
            ends_at: None,
            last_alive_at: Some(wall(600)),
        };
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(700),
            },
            Box::new(NullPersistencePort),
        );
        assert!(effects.is_empty());
        assert!(!ctl.core().running);
        assert_eq!(ctl.core().phase, TimerPhase::Study);
        assert_eq!(ctl.core().remaining_seconds, 900);
    }

    #[test]
    fn recovery_endless_restores_paused_with_capped_elapsed_and_no_session() {
        let snapshot = TimerSnapshot {
            phase: TimerPhase::Stopwatch,
            mode: TimerMode::Endless,
            remaining_seconds: 0,
            logged_split_seconds: 0,
            active_segments: vec![ActiveSegment {
                started_at: wall(0),
                ended_at: None,
            }],
            running: true,
            config: endless_config(),
            context: TimerContext::default(),
            started_at: Some(wall(0)),
            ends_at: None,
            last_alive_at: Some(wall(120)),
        };
        let (ctl, effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(999_999),
            },
            Box::new(NullPersistencePort),
        );
        assert!(
            effects.is_empty(),
            "endless never auto-creates a session on restore"
        );
        assert!(
            !ctl.core().running,
            "endless restores paused, matching production"
        );
        assert_eq!(
            ctl.core().remaining_seconds,
            120,
            "elapsed time is capped at last_alive_at"
        );
    }

    #[test]
    fn recovery_stale_open_segment_is_closed_at_last_alive_at() {
        let snapshot = TimerSnapshot {
            phase: TimerPhase::Study,
            mode: TimerMode::Focus,
            remaining_seconds: 1200,
            logged_split_seconds: 0,
            active_segments: vec![ActiveSegment {
                started_at: wall(0),
                ended_at: None,
            }],
            running: false,
            config: focus_config(),
            context: TimerContext::default(),
            started_at: Some(wall(0)),
            ends_at: None,
            last_alive_at: Some(wall(300)),
        };
        let (ctl, _effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now: clock(400),
            },
            Box::new(NullPersistencePort),
        );
        assert_eq!(ctl.core().active_segments[0].ended_at, Some(wall(300)));
    }

    #[test]
    fn recovery_duplicate_key_does_not_recover_the_same_session_twice() {
        let snapshot = snapshot_running_countdown(TimerPhase::Study, focus_config(), 0, 1500);
        let (first_ctl, first_effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot: snapshot.clone(),
                existing_recovered_keys: Vec::new(),
                now: clock(2000),
            },
            Box::new(NullPersistencePort),
        );
        assert!(first_effects
            .iter()
            .any(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. })));

        // Simulate re-applying recovery with the same snapshot and the recovered key carried
        // forward - the exact "duplicate recovery" scenario the compatibility spec calls out.
        let recovered_keys = restore_timer(RestoreInput {
            snapshot: snapshot.clone(),
            existing_recovered_keys: Vec::new(),
            now: clock(2000),
        })
        .recovered_keys;
        let (_second_ctl, second_effects) = TimerController::restore(
            0,
            RestoreInput {
                snapshot,
                existing_recovered_keys: recovered_keys,
                now: clock(2000),
            },
            Box::new(NullPersistencePort),
        );
        assert!(
            !second_effects
                .iter()
                .any(|e| matches!(e, TimerApplicationEffect::SessionRangeReady { .. })),
            "the same recovered range must not be reported twice"
        );
        let _ = first_ctl;
    }
}
