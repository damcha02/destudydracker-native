mod persistence;
mod state;

pub use persistence::{restore_timer, RestoreInput, RestoreOutcome, TimerSnapshot};
pub use state::{
    ActiveSegment, ClockObservation, CompletionReason, TimerCommand, TimerConfig, TimerContext,
    TimerEvent, TimerMode, TimerPhase, TimerState, WallTimestamp,
};

#[cfg(test)]
mod tests;
