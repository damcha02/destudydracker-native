use super::*;

const SEC: i64 = 1000;
const MIN: i64 = 60 * SEC;
const HOUR: i64 = 60 * MIN;

fn clock(seconds: u64) -> ClockObservation {
    ClockObservation::new(seconds * 1000, seconds as i64 * SEC)
}

fn wall(seconds: i64) -> WallTimestamp {
    WallTimestamp::from_unix_millis(seconds * SEC)
}

fn focus_timer() -> TimerState {
    TimerState::default()
}

fn exam_timer() -> TimerState {
    let mut timer = TimerState::default();
    timer.apply(TimerCommand::SetMode(TimerMode::Exam), clock(0));
    timer
}

fn endless_timer() -> TimerState {
    let mut timer = TimerState::default();
    timer.apply(TimerCommand::SetMode(TimerMode::Endless), clock(0));
    timer
}

#[test]
fn initial_state_matches_production_defaults() {
    let timer = TimerState::default();

    assert_eq!(timer.phase, TimerPhase::Idle);
    assert_eq!(timer.config.mode, TimerMode::Focus);
    assert!(!timer.running);
    assert_eq!(timer.remaining_seconds, 25 * 60);
    assert_eq!(timer.config.break_seconds, 5 * 60);
    assert_eq!(timer.config.exam_seconds, 90 * 60);
}

#[test]
fn start_focus_creates_running_study_countdown() {
    let mut timer = focus_timer();
    let events = timer.apply(TimerCommand::Start, clock(10));

    assert_eq!(timer.phase, TimerPhase::Study);
    assert!(timer.running);
    assert_eq!(timer.remaining_seconds, 25 * 60);
    assert_eq!(timer.started_at, Some(wall(10)));
    assert_eq!(timer.ends_at, Some(wall(10 + 25 * 60)));
    assert_eq!(timer.active_segments, vec![ActiveSegment::open(wall(10))]);
    assert!(events.contains(&TimerEvent::Started {
        phase: TimerPhase::Study
    }));
}

#[test]
fn start_exam_creates_running_exam_countdown() {
    let mut timer = exam_timer();
    timer.apply(TimerCommand::Start, clock(2));

    assert_eq!(timer.phase, TimerPhase::Exam);
    assert_eq!(timer.remaining_seconds, 90 * 60);
    assert_eq!(timer.ends_at, Some(wall(2 + 90 * 60)));
}

#[test]
fn start_endless_creates_stopwatch_without_end_time() {
    let mut timer = endless_timer();
    timer.apply(TimerCommand::Start, clock(4));

    assert_eq!(timer.phase, TimerPhase::Stopwatch);
    assert!(timer.running);
    assert_eq!(timer.remaining_seconds, 0);
    assert_eq!(timer.ends_at, None);
    assert_eq!(timer.display_seconds(clock(94)), 90);
}

#[test]
fn countdown_pause_and_resume_excludes_paused_gap() {
    let mut timer = focus_timer();
    timer.apply(TimerCommand::Start, clock(0));

    timer.apply(TimerCommand::Pause, clock(10 * 60));
    assert!(!timer.running);
    assert_eq!(timer.remaining_seconds, 15 * 60);
    assert_eq!(timer.ends_at, None);
    assert_eq!(timer.active_segments[0].ended_at, Some(wall(10 * 60)));
    assert_eq!(timer.display_seconds(clock(15 * 60)), 15 * 60);

    timer.apply(TimerCommand::Resume, clock(15 * 60));
    assert!(timer.running);
    assert_eq!(timer.ends_at, Some(wall(30 * 60)));
    assert_eq!(timer.display_seconds(clock(25 * 60)), 5 * 60);
    assert_eq!(timer.active_segments.len(), 2);
    assert_eq!(timer.active_segments[1].started_at, wall(15 * 60));
}

#[test]
fn stopwatch_pause_and_resume_excludes_paused_gap() {
    let mut timer = endless_timer();
    timer.apply(TimerCommand::Start, clock(0));
    timer.apply(TimerCommand::Pause, clock(90));

    assert_eq!(timer.remaining_seconds, 90);
    assert_eq!(timer.display_seconds(clock(5 * 60)), 90);

    timer.apply(TimerCommand::Resume, clock(5 * 60));
    assert_eq!(timer.remaining_seconds, 90);
    assert_eq!(timer.display_seconds(clock(5 * 60 + 30)), 120);
    assert_eq!(timer.active_segments.len(), 2);
    assert_eq!(timer.active_segments[0].ended_at, Some(wall(90)));
}

#[test]
fn countdown_completion_never_goes_negative_and_starts_break() {
    let mut timer = focus_timer();
    timer.apply(TimerCommand::Start, clock(0));

    let events = timer.apply(TimerCommand::ObserveTime, clock(25 * 60 + 5));

    assert_eq!(timer.phase, TimerPhase::Break);
    assert!(timer.running);
    assert_eq!(timer.remaining_seconds, 5 * 60);
    assert!(events.iter().any(|event| matches!(
        event,
        TimerEvent::SessionRangeReady {
            phase: TimerPhase::Study,
            ..
        }
    )));
    assert!(events.contains(&TimerEvent::BreakStarted));
}

#[test]
fn break_completion_returns_idle_without_session() {
    let mut timer = focus_timer();
    timer.apply(TimerCommand::Start, clock(0));
    timer.apply(TimerCommand::ObserveTime, clock(25 * 60));

    let events = timer.apply(TimerCommand::ObserveTime, clock(30 * 60));

    assert_eq!(timer.phase, TimerPhase::Idle);
    assert!(!events
        .iter()
        .any(|event| matches!(event, TimerEvent::SessionRangeReady { .. })));
}

#[test]
fn exam_completion_returns_idle_with_exam_session_range() {
    let mut timer = exam_timer();
    timer.apply(TimerCommand::Start, clock(0));

    let events = timer.apply(TimerCommand::ObserveTime, clock(90 * 60));

    assert_eq!(timer.phase, TimerPhase::Idle);
    assert!(events.iter().any(|event| matches!(
        event,
        TimerEvent::SessionRangeReady {
            phase: TimerPhase::Exam,
            ..
        }
    )));
}

#[test]
fn focus_completion_without_break_returns_idle() {
    let mut timer = TimerState::new(
        TimerConfig {
            break_seconds: 0,
            ..TimerConfig::default()
        },
        TimerContext::default(),
    );
    timer.apply(TimerCommand::Start, clock(0));

    let events = timer.apply(TimerCommand::ObserveTime, clock(25 * 60));

    assert_eq!(timer.phase, TimerPhase::Idle);
    assert!(events.iter().any(|event| matches!(
        event,
        TimerEvent::SessionRangeReady {
            phase: TimerPhase::Study,
            ..
        }
    )));
    assert!(!events.contains(&TimerEvent::BreakStarted));
}

#[test]
fn manual_save_emits_range_and_resets_without_session_identity() {
    let mut timer = endless_timer();
    timer.apply(TimerCommand::Start, clock(0));

    let events = timer.apply(TimerCommand::CompleteManually, clock(90));

    assert_eq!(timer.phase, TimerPhase::Idle);
    assert!(events.iter().any(|event| matches!(
        event,
        TimerEvent::SessionRangeReady {
            phase: TimerPhase::Stopwatch,
            reason: CompletionReason::ManualSave,
            segments,
            ..
        } if segments[0].started_at == wall(0) && segments[0].ended_at == Some(wall(90))
    )));
}

#[test]
fn repeated_commands_are_deterministic_noops() {
    let mut timer = focus_timer();
    timer.apply(TimerCommand::Start, clock(0));
    let started = timer.clone();

    assert!(timer.apply(TimerCommand::Start, clock(5)).is_empty());
    assert_eq!(timer, started);

    timer.apply(TimerCommand::Pause, clock(60));
    let paused = timer.clone();
    assert!(timer.apply(TimerCommand::Pause, clock(120)).is_empty());
    assert_eq!(timer, paused);

    timer.apply(TimerCommand::Reset, clock(130));
    let reset = timer.clone();
    assert!(timer.apply(TimerCommand::Reset, clock(140)).is_empty());
    assert_eq!(timer, reset);
}

#[test]
fn mode_changes_are_blocked_while_active() {
    let mut timer = focus_timer();
    timer.apply(TimerCommand::Start, clock(0));

    let events = timer.apply(TimerCommand::SetMode(TimerMode::Exam), clock(1));

    assert!(events.is_empty());
    assert_eq!(timer.config.mode, TimerMode::Focus);
    assert_eq!(timer.phase, TimerPhase::Study);
}

#[test]
fn snapshot_derives_running_remaining_and_last_alive() {
    let mut timer = focus_timer();
    timer.apply(TimerCommand::Start, clock(0));

    let snapshot = timer.snapshot_for_persistence(clock(90));

    assert_eq!(snapshot.remaining_seconds, 25 * 60 - 90);
    assert_eq!(snapshot.last_alive_at, Some(wall(90)));
}

#[test]
fn restore_running_countdown_with_recent_heartbeat_keeps_running() {
    let snapshot = TimerSnapshot {
        phase: TimerPhase::Study,
        mode: TimerMode::Focus,
        remaining_seconds: 20 * 60,
        logged_split_seconds: 0,
        active_segments: vec![ActiveSegment::open(wall(0))],
        running: true,
        config: TimerConfig::default(),
        context: TimerContext::default(),
        started_at: Some(wall(0)),
        ends_at: Some(wall(25 * 60)),
        last_alive_at: Some(wall(60)),
    };

    let outcome = restore_timer(RestoreInput {
        snapshot,
        existing_recovered_keys: Vec::new(),
        now: clock(90),
    });

    assert!(outcome.timer.running);
    assert_eq!(outcome.timer.remaining_seconds, 25 * 60 - 90);
    assert!(outcome.events.is_empty());
}

#[test]
fn restore_expired_countdown_recovers_session_and_resets() {
    let snapshot = TimerSnapshot {
        phase: TimerPhase::Study,
        mode: TimerMode::Focus,
        remaining_seconds: 0,
        logged_split_seconds: 0,
        active_segments: vec![ActiveSegment::open(wall(0))],
        running: true,
        config: TimerConfig::default(),
        context: TimerContext::default(),
        started_at: Some(wall(0)),
        ends_at: Some(wall(25 * 60)),
        last_alive_at: Some(wall(24 * 60)),
    };

    let outcome = restore_timer(RestoreInput {
        snapshot,
        existing_recovered_keys: Vec::new(),
        now: clock(30 * 60),
    });

    assert_eq!(outcome.timer.phase, TimerPhase::Idle);
    assert!(outcome.events.iter().any(|event| matches!(
        event,
        TimerEvent::SessionRangeReady {
            reason: CompletionReason::AbandonedRecovery,
            ..
        }
    )));
    assert_eq!(outcome.recovered_keys.len(), 1);
}

#[test]
fn restore_endless_closes_at_last_alive_and_does_not_create_session() {
    let snapshot = TimerSnapshot {
        phase: TimerPhase::Stopwatch,
        mode: TimerMode::Endless,
        remaining_seconds: 20 * 60,
        logged_split_seconds: 0,
        active_segments: vec![ActiveSegment::open(WallTimestamp::from_unix_millis(
            -11 * HOUR - 20 * MIN,
        ))],
        running: true,
        config: TimerConfig {
            mode: TimerMode::Endless,
            ..TimerConfig::default()
        },
        context: TimerContext::default(),
        started_at: Some(WallTimestamp::from_unix_millis(-11 * HOUR - 20 * MIN)),
        ends_at: None,
        last_alive_at: Some(WallTimestamp::from_unix_millis(-11 * HOUR)),
    };

    let outcome = restore_timer(RestoreInput {
        snapshot,
        existing_recovered_keys: Vec::new(),
        now: clock(0),
    });

    assert!(!outcome.timer.running);
    assert_eq!(outcome.timer.phase, TimerPhase::Stopwatch);
    assert_eq!(outcome.timer.remaining_seconds, 20 * 60);
    assert_eq!(
        outcome.timer.active_segments[0].ended_at,
        Some(WallTimestamp::from_unix_millis(-11 * HOUR))
    );
    assert!(outcome.events.is_empty());
}

#[test]
fn abandoned_paused_study_recovers_once_and_not_twice() {
    let snapshot = TimerSnapshot {
        phase: TimerPhase::Study,
        mode: TimerMode::Focus,
        remaining_seconds: 20 * 60,
        logged_split_seconds: 0,
        active_segments: vec![ActiveSegment {
            started_at: WallTimestamp::from_unix_millis(-8 * HOUR),
            ended_at: Some(WallTimestamp::from_unix_millis(-7 * HOUR)),
        }],
        running: false,
        config: TimerConfig::default(),
        context: TimerContext::default(),
        started_at: Some(WallTimestamp::from_unix_millis(-8 * HOUR)),
        ends_at: None,
        last_alive_at: Some(WallTimestamp::from_unix_millis(-7 * HOUR)),
    };

    let first = restore_timer(RestoreInput {
        snapshot: snapshot.clone(),
        existing_recovered_keys: Vec::new(),
        now: clock(0),
    });
    assert_eq!(first.recovered_keys.len(), 1);
    assert!(first
        .events
        .iter()
        .any(|event| matches!(event, TimerEvent::SessionRangeReady { .. })));

    let second = restore_timer(RestoreInput {
        snapshot,
        existing_recovered_keys: first.recovered_keys,
        now: clock(0),
    });
    assert!(second.events.is_empty());
}

#[test]
fn manual_save_event_keeps_session_identity_outside_core() {
    let mut timer = endless_timer();
    timer.apply(TimerCommand::Start, clock(0));
    timer.apply(TimerCommand::Pause, clock(90));

    let segments = timer.active_segments.clone();
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].started_at, wall(0));
    assert_eq!(segments[0].ended_at, Some(wall(90)));
    // The domain exposes ranges/context only. A persistence or app service must assign IDs.
    assert_eq!(timer.context, TimerContext::default());
}
