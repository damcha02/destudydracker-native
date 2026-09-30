//! Timer notifications (Stage 18): *what* is announced and *when*, independent of how Windows
//! displays it.
//!
//! ```text
//! TimerApplicationEffect (application layer)
//!     └─ notifications_for_effects  (pure mapping, production v0.1.67 strings)
//!          └─ NotificationLedger    (exactly-once guard at the application boundary)
//!               └─ NotificationSink (Windows toast adapter | test recorder)
//! ```
//!
//! `study-tracker-core` never sees any of this, and a failing sink can never affect the Timer or
//! the `StudySession` it just produced: delivery happens *after* the domain has already
//! transitioned and persisted, and [`deliver`] swallows (and logs) sink errors.
//!
//! Production reference (`desktop/src/App.tsx`, the timer tick effect): notifications are sent from
//! the **live** completion transitions only - never on startup recovery, manual save or reset:
//!
//! | Event | Title | Body |
//! |---|---|---|
//! | Focus block ends and a break follows | Focus session finished | Nice work. Time for a break. |
//! | Focus block ends, no break | Focus session finished | Your study timer is complete. |
//! | Exam ends | Exam timer finished | Your exam timer is complete. |
//! | Break ends | Break finished | Break is over. Ready for the next focus session? |
//! | First hide-to-tray of a run | Study Tracker is still running | Your session keeps tracking in the tray. Right-click the tray icon to quit. |
//!
//! (Production also notifies "Are you still here?" / "Timer stopped" for the Endless-mode
//! inactivity prompt; the native Timer has no inactivity prompt yet - Stage 14 "Production
//! divergences" - so those two are deferred with it.)

use crate::timer_controller::TimerApplicationEffect;
use study_tracker_core::timer::{CompletionReason, TimerPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    FocusFinishedBreakNext,
    FocusFinished,
    ExamFinished,
    BreakFinished,
    StillRunningInTray,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRequest {
    pub kind: NotificationKind,
    pub title: &'static str,
    pub body: &'static str,
}

impl NotificationRequest {
    pub fn of(kind: NotificationKind) -> Self {
        let (title, body) = match kind {
            NotificationKind::FocusFinishedBreakNext => {
                ("Focus session finished", "Nice work. Time for a break.")
            }
            NotificationKind::FocusFinished => {
                ("Focus session finished", "Your study timer is complete.")
            }
            NotificationKind::ExamFinished => {
                ("Exam timer finished", "Your exam timer is complete.")
            }
            NotificationKind::BreakFinished => (
                "Break finished",
                "Break is over. Ready for the next focus session?",
            ),
            NotificationKind::StillRunningInTray => (
                super::tray_model::HIDE_NOTICE_TITLE,
                super::tray_model::HIDE_NOTICE_BODY,
            ),
        };
        Self { kind, title, body }
    }
}

/// Maps the effects of **one** Timer command to notifications. `phase_after` is the Timer's phase
/// once that command has been applied (a finished focus block followed by a running Break means
/// "time for a break"). Only natural countdown completions notify.
pub fn notifications_for_effects(
    effects: &[TimerApplicationEffect],
    phase_after: TimerPhase,
) -> Vec<NotificationRequest> {
    effects
        .iter()
        .filter_map(|effect| match effect {
            TimerApplicationEffect::Completed {
                phase,
                reason: CompletionReason::CountdownElapsed,
            } => match phase {
                TimerPhase::Study if phase_after == TimerPhase::Break => {
                    Some(NotificationKind::FocusFinishedBreakNext)
                }
                TimerPhase::Study => Some(NotificationKind::FocusFinished),
                TimerPhase::Exam => Some(NotificationKind::ExamFinished),
                TimerPhase::Break => Some(NotificationKind::BreakFinished),
                TimerPhase::Idle | TimerPhase::Stopwatch => None,
            },
            _ => None,
        })
        .map(NotificationRequest::of)
        .collect()
}

/// Exactly-once guard. The Timer emits each completion once, and the application drains its
/// outbox once, so duplicates should be impossible; this ledger makes that a checked property
/// rather than an assumption: a second request of the same kind stamped with the same completion
/// instant is dropped (covers a repeated observation of the same effect list).
#[derive(Debug, Default)]
pub struct NotificationLedger {
    last: Option<(NotificationKind, i64)>,
}

impl NotificationLedger {
    /// `true` if this (kind, completion instant) has not been delivered yet.
    pub fn admit(&mut self, kind: NotificationKind, completion_wall_millis: i64) -> bool {
        if self.last == Some((kind, completion_wall_millis)) {
            return false;
        }
        self.last = Some((kind, completion_wall_millis));
        true
    }
}

/// Where notifications finally go. Implemented by the Windows toast adapter and by test doubles.
pub trait NotificationSink {
    fn send(&mut self, request: &NotificationRequest) -> Result<(), String>;
}

/// Delivers `requests` (each stamped with its completion instant) through the ledger to `sink`.
/// Returns how many reached the sink successfully. **Never panics and never returns an error**:
/// a failing sink is logged and dropped because the domain already moved on (brief section 16).
pub fn deliver(
    sink: &mut dyn NotificationSink,
    ledger: &mut NotificationLedger,
    requests: &[(NotificationRequest, i64)],
) -> usize {
    let mut delivered = 0;
    for (request, completion_millis) in requests {
        if !ledger.admit(request.kind, *completion_millis) {
            log::debug!(
                "notification {:?} suppressed: already delivered",
                request.kind
            );
            continue;
        }
        match sink.send(request) {
            Ok(()) => {
                delivered += 1;
                log::info!("notification shown: {:?} ({})", request.kind, request.title);
            }
            Err(error) => log::warn!(
                "notification {:?} could not be shown: {error}",
                request.kind
            ),
        }
    }
    delivered
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completed(phase: TimerPhase, reason: CompletionReason) -> TimerApplicationEffect {
        TimerApplicationEffect::Completed { phase, reason }
    }

    #[derive(Default)]
    struct Recorder {
        sent: Vec<NotificationKind>,
        fail: bool,
    }
    impl NotificationSink for Recorder {
        fn send(&mut self, request: &NotificationRequest) -> Result<(), String> {
            if self.fail {
                return Err("toast platform unavailable".into());
            }
            self.sent.push(request.kind);
            Ok(())
        }
    }

    #[test]
    fn production_strings_and_event_mapping() {
        let n = notifications_for_effects(
            &[completed(
                TimerPhase::Study,
                CompletionReason::CountdownElapsed,
            )],
            TimerPhase::Break,
        );
        assert_eq!(n.len(), 1);
        assert_eq!(
            (n[0].title, n[0].body),
            ("Focus session finished", "Nice work. Time for a break.")
        );

        let n = notifications_for_effects(
            &[completed(
                TimerPhase::Study,
                CompletionReason::CountdownElapsed,
            )],
            TimerPhase::Idle,
        );
        assert_eq!(
            (n[0].title, n[0].body),
            ("Focus session finished", "Your study timer is complete.")
        );

        let n = notifications_for_effects(
            &[completed(
                TimerPhase::Exam,
                CompletionReason::CountdownElapsed,
            )],
            TimerPhase::Idle,
        );
        assert_eq!(
            (n[0].title, n[0].body),
            ("Exam timer finished", "Your exam timer is complete.")
        );

        let n = notifications_for_effects(
            &[completed(
                TimerPhase::Break,
                CompletionReason::CountdownElapsed,
            )],
            TimerPhase::Idle,
        );
        assert_eq!(
            (n[0].title, n[0].body),
            (
                "Break finished",
                "Break is over. Ready for the next focus session?"
            )
        );
        let tray = NotificationRequest::of(NotificationKind::StillRunningInTray);
        assert_eq!(tray.title, "Study Tracker is still running");
        assert_eq!(
            tray.body,
            "Your session keeps tracking in the tray. Right-click the tray icon to quit."
        );
    }

    #[test]
    fn only_live_countdown_completions_notify() {
        for reason in [
            CompletionReason::ManualSave,
            CompletionReason::AbandonedRecovery,
        ] {
            for phase in [
                TimerPhase::Study,
                TimerPhase::Exam,
                TimerPhase::Break,
                TimerPhase::Stopwatch,
            ] {
                assert!(
                    notifications_for_effects(&[completed(phase, reason)], TimerPhase::Idle).is_empty(),
                    "{phase:?}/{reason:?} must be silent (recovery and manual save never notify in production)"
                );
            }
        }
        assert!(
            notifications_for_effects(
                &[completed(
                    TimerPhase::Stopwatch,
                    CompletionReason::CountdownElapsed
                )],
                TimerPhase::Idle
            )
            .is_empty(),
            "an Endless timer has no countdown to finish"
        );
        assert!(notifications_for_effects(&[], TimerPhase::Idle).is_empty());
    }

    #[test]
    fn session_range_effects_alone_do_not_notify() {
        use study_tracker_core::timer::TimerContext;
        let effect = TimerApplicationEffect::SessionRangeReady {
            phase: TimerPhase::Study,
            reason: CompletionReason::CountdownElapsed,
            segments: vec![],
            context: TimerContext::default(),
            preset_label: String::new(),
        };
        assert!(
            notifications_for_effects(&[effect], TimerPhase::Idle).is_empty(),
            "the Completed effect carries the notification, not the session range"
        );
    }

    #[test]
    fn repeated_observation_of_the_same_completion_delivers_once() {
        let mut sink = Recorder::default();
        let mut ledger = NotificationLedger::default();
        let req = NotificationRequest::of(NotificationKind::FocusFinished);
        let batch = vec![
            (req.clone(), 1_000),
            (req.clone(), 1_000),
            (req.clone(), 1_000),
        ];
        assert_eq!(deliver(&mut sink, &mut ledger, &batch), 1);
        assert_eq!(
            deliver(&mut sink, &mut ledger, &batch),
            0,
            "a later replay of the same completion is dropped too"
        );
        assert_eq!(sink.sent, [NotificationKind::FocusFinished]);
        // A genuinely new completion (later instant) is delivered.
        assert_eq!(deliver(&mut sink, &mut ledger, &[(req, 2_000)]), 1);
        assert_eq!(sink.sent.len(), 2);
    }

    #[test]
    fn a_failing_sink_is_reported_nowhere_but_the_log_and_never_panics() {
        let mut sink = Recorder {
            fail: true,
            ..Recorder::default()
        };
        let mut ledger = NotificationLedger::default();
        let delivered = deliver(
            &mut sink,
            &mut ledger,
            &[(NotificationRequest::of(NotificationKind::ExamFinished), 5)],
        );
        assert_eq!(delivered, 0);
        assert!(sink.sent.is_empty());
    }
}
