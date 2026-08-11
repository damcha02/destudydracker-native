use serde::{Deserialize, Serialize};

use super::state::{
    close_open_segments, closed_segments, configured_seconds, idle_seconds, segment_seconds,
    session_events, ActiveSegment, ClockObservation, CompletionReason, TimerConfig, TimerContext,
    TimerMode, TimerPhase, TimerState, WallTimestamp,
};

pub const TIMER_ALIVE_GAP_SECONDS: u64 = 5 * 60;
pub const ABANDONED_TIMER_INACTIVITY_SECONDS: u64 = 6 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerSnapshot {
    pub phase: TimerPhase,
    pub mode: TimerMode,
    pub remaining_seconds: u64,
    pub logged_split_seconds: u64,
    pub active_segments: Vec<ActiveSegment>,
    pub running: bool,
    pub config: TimerConfig,
    pub context: TimerContext,
    pub started_at: Option<WallTimestamp>,
    pub ends_at: Option<WallTimestamp>,
    pub last_alive_at: Option<WallTimestamp>,
}

impl TimerSnapshot {
    pub fn from_state(timer: &TimerState) -> Self {
        Self {
            phase: timer.phase,
            mode: timer.config.mode,
            remaining_seconds: timer.remaining_seconds,
            logged_split_seconds: timer.logged_split_seconds,
            active_segments: timer.active_segments.clone(),
            running: timer.running,
            config: timer.config.clone(),
            context: timer.context.clone(),
            started_at: timer.started_at,
            ends_at: timer.ends_at,
            last_alive_at: timer.last_alive_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreInput {
    pub snapshot: TimerSnapshot,
    pub existing_recovered_keys: Vec<String>,
    pub now: ClockObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreOutcome {
    pub timer: TimerState,
    pub events: Vec<super::state::TimerEvent>,
    pub recovered_keys: Vec<String>,
}

pub fn restore_timer(input: RestoreInput) -> RestoreOutcome {
    let mut timer = snapshot_to_state(input.snapshot);

    let mut events = Vec::new();
    let mut recovered_keys = input.existing_recovered_keys;

    if timer.running
        && timer.ends_at.is_some()
        && matches!(timer.phase, TimerPhase::Study | TimerPhase::Exam)
    {
        let ends_at = timer.ends_at.expect("checked above");
        if ends_at.unix_millis <= input.now.wall.unix_millis {
            let mut recovery_timer = timer.clone();
            close_open_segments(&mut recovery_timer.active_segments, ends_at);
            let segments = closed_segments(&recovery_timer.active_segments);
            let key = recovery_key(recovery_timer.phase, &segments);
            if !key.is_empty() && !recovered_keys.iter().any(|existing| existing == &key) {
                events.extend(session_events(
                    recovery_timer.phase,
                    CompletionReason::AbandonedRecovery,
                    segments,
                    &recovery_timer,
                ));
                recovered_keys.push(key);
            }
            recovery_timer.reset_to_idle_preserving_context();
            return RestoreOutcome {
                timer: recovery_timer,
                events,
                recovered_keys,
            };
        }

        let is_stale = timer
            .last_alive_at
            .map(|last_alive| seconds_between(last_alive, input.now.wall) > TIMER_ALIVE_GAP_SECONDS)
            .unwrap_or(false);
        if !is_stale {
            timer.remaining_seconds = ends_at.seconds_until(input.now.wall).max(0) as u64;
            return RestoreOutcome {
                timer,
                events,
                recovered_keys,
            };
        }

        timer.running = false;
        timer.ends_at = None;
    } else if !timer.running || timer.ends_at.is_none() {
        timer.running = false;
        timer.ends_at = None;
    }

    if timer.running || timer.phase == TimerPhase::Idle {
        return RestoreOutcome {
            timer,
            events,
            recovered_keys,
        };
    }

    let repaired_segments = close_stale_open_segments(&timer);
    timer.active_segments = repaired_segments;
    if timer.phase == TimerPhase::Stopwatch {
        timer.remaining_seconds = timer
            .active_segments
            .iter()
            .filter_map(|segment| {
                segment
                    .ended_at
                    .map(|end| segment_seconds(segment.started_at, end))
            })
            .sum();
        return RestoreOutcome {
            timer,
            events,
            recovered_keys,
        };
    }

    let last_activity = last_timer_activity(&timer);
    if last_activity
        .map(|activity| {
            seconds_between(activity, input.now.wall) > ABANDONED_TIMER_INACTIVITY_SECONDS
        })
        .unwrap_or(false)
    {
        let segments = closed_segments(&timer.active_segments);
        let key = recovery_key(timer.phase, &segments);
        if !key.is_empty() && !recovered_keys.iter().any(|existing| existing == &key) {
            events.extend(session_events(
                timer.phase,
                CompletionReason::AbandonedRecovery,
                segments,
                &timer,
            ));
            recovered_keys.push(key);
        }
        timer.reset_to_idle_preserving_context();
    }

    RestoreOutcome {
        timer,
        events,
        recovered_keys,
    }
}

fn snapshot_to_state(snapshot: TimerSnapshot) -> TimerState {
    let mut config = snapshot.config;
    config.mode = snapshot.mode;
    TimerState::from_parts_without_runtime(
        snapshot.phase,
        snapshot.running,
        snapshot.remaining_seconds,
        snapshot.logged_split_seconds,
        normalize_segments(snapshot.active_segments),
        snapshot.started_at,
        snapshot.ends_at,
        snapshot.last_alive_at,
        config,
        snapshot.context,
    )
}

fn normalize_segments(segments: Vec<ActiveSegment>) -> Vec<ActiveSegment> {
    segments
        .into_iter()
        .filter(|segment| {
            segment
                .ended_at
                .map(|ended_at| ended_at.unix_millis >= segment.started_at.unix_millis)
                .unwrap_or(true)
        })
        .collect()
}

fn close_stale_open_segments(timer: &TimerState) -> Vec<ActiveSegment> {
    let Some(open_index) = timer
        .active_segments
        .iter()
        .position(|segment| segment.ended_at.is_none())
    else {
        return timer.active_segments.clone();
    };

    let open = &timer.active_segments[open_index];
    let end = if let Some(last_alive_at) = timer.last_alive_at {
        WallTimestamp::from_unix_millis(last_alive_at.unix_millis.max(open.started_at.unix_millis))
    } else {
        let closed_seconds: u64 = timer
            .active_segments
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != open_index)
            .filter_map(|(_, segment)| {
                segment
                    .ended_at
                    .map(|end| segment_seconds(segment.started_at, end))
            })
            .sum();
        let elapsed_seconds = if timer.phase == TimerPhase::Stopwatch {
            timer.remaining_seconds.saturating_sub(closed_seconds)
        } else {
            configured_seconds(timer)
                .saturating_sub(timer.remaining_seconds)
                .saturating_sub(timer.logged_split_seconds)
                .saturating_sub(closed_seconds)
        };
        open.started_at.plus_seconds(elapsed_seconds)
    };

    let mut segments = timer.active_segments.clone();
    segments[open_index].ended_at = Some(end);
    segments
}

fn last_timer_activity(timer: &TimerState) -> Option<WallTimestamp> {
    timer
        .active_segments
        .iter()
        .filter_map(|segment| segment.ended_at.or(Some(segment.started_at)))
        .chain(timer.started_at)
        .max()
}

fn recovery_key(phase: TimerPhase, segments: &[ActiveSegment]) -> String {
    let Some(first) = segments.first() else {
        return String::new();
    };
    let Some(last_end) = segments.last().and_then(|segment| segment.ended_at) else {
        return String::new();
    };
    format!(
        "recovered-{phase:?}-{}-{}",
        first.started_at.unix_millis, last_end.unix_millis
    )
}

fn seconds_between(start: WallTimestamp, end: WallTimestamp) -> u64 {
    if end.unix_millis <= start.unix_millis {
        return 0;
    }
    ((end.unix_millis - start.unix_millis) / 1000) as u64
}

#[allow(dead_code)]
fn _restore_idle_seconds(config: &TimerConfig) -> u64 {
    idle_seconds(config)
}
