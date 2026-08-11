use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct WallTimestamp {
    pub unix_millis: i64,
}

impl WallTimestamp {
    pub const fn from_unix_millis(unix_millis: i64) -> Self {
        Self { unix_millis }
    }

    pub fn plus_seconds(self, seconds: u64) -> Self {
        let millis = i64::try_from(seconds.saturating_mul(1000)).unwrap_or(i64::MAX);
        Self {
            unix_millis: self.unix_millis.saturating_add(millis),
        }
    }

    pub fn seconds_until(self, now: Self) -> i64 {
        div_ceil_i64(self.unix_millis.saturating_sub(now.unix_millis), 1000)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockObservation {
    pub monotonic_millis: u64,
    pub wall: WallTimestamp,
}

impl ClockObservation {
    pub const fn new(monotonic_millis: u64, wall_unix_millis: i64) -> Self {
        Self {
            monotonic_millis,
            wall: WallTimestamp::from_unix_millis(wall_unix_millis),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerMode {
    Focus,
    Exam,
    Endless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerPhase {
    Idle,
    Study,
    Break,
    Exam,
    Stopwatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimerConfig {
    pub mode: TimerMode,
    pub study_seconds: u64,
    pub break_seconds: u64,
    pub exam_seconds: u64,
    pub preset_label: String,
}

impl Default for TimerConfig {
    fn default() -> Self {
        Self {
            mode: TimerMode::Focus,
            study_seconds: 25 * 60,
            break_seconds: 5 * 60,
            exam_seconds: 90 * 60,
            preset_label: "Pomodoro 25/5".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TimerContext {
    pub semester_id: Option<String>,
    pub course_id: Option<String>,
    pub task_id: Option<String>,
    pub goal: String,
    pub learned: String,
    pub blocker: String,
    pub next_step: String,
    pub confidence: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSegment {
    pub started_at: WallTimestamp,
    pub ended_at: Option<WallTimestamp>,
}

impl ActiveSegment {
    pub fn open(started_at: WallTimestamp) -> Self {
        Self {
            started_at,
            ended_at: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeAnchor {
    monotonic_millis: u64,
    remaining_or_elapsed_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerState {
    pub phase: TimerPhase,
    pub running: bool,
    pub remaining_seconds: u64,
    pub logged_split_seconds: u64,
    pub active_segments: Vec<ActiveSegment>,
    pub started_at: Option<WallTimestamp>,
    pub ends_at: Option<WallTimestamp>,
    pub last_alive_at: Option<WallTimestamp>,
    pub config: TimerConfig,
    pub context: TimerContext,
    runtime_anchor: Option<RuntimeAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerCommand {
    Start,
    Pause,
    Resume,
    TogglePaused,
    Reset,
    CompleteManually,
    ObserveTime,
    SetMode(TimerMode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionReason {
    CountdownElapsed,
    ManualSave,
    AbandonedRecovery,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimerEvent {
    Started {
        phase: TimerPhase,
    },
    Paused {
        phase: TimerPhase,
    },
    Resumed {
        phase: TimerPhase,
    },
    Reset,
    BreakStarted,
    Completed {
        phase: TimerPhase,
        reason: CompletionReason,
    },
    SessionRangeReady {
        phase: TimerPhase,
        reason: CompletionReason,
        segments: Vec<ActiveSegment>,
        context: TimerContext,
        preset_label: String,
    },
    PersistenceRequested,
}

impl Default for TimerState {
    fn default() -> Self {
        Self::new(TimerConfig::default(), TimerContext::default())
    }
}

impl TimerState {
    pub fn new(config: TimerConfig, context: TimerContext) -> Self {
        let remaining_seconds = idle_seconds(&config);
        Self {
            phase: TimerPhase::Idle,
            running: false,
            remaining_seconds,
            logged_split_seconds: 0,
            active_segments: Vec::new(),
            started_at: None,
            ends_at: None,
            last_alive_at: None,
            config,
            context,
            runtime_anchor: None,
        }
    }

    pub fn apply(&mut self, command: TimerCommand, clock: ClockObservation) -> Vec<TimerEvent> {
        match command {
            TimerCommand::Start => self.start(clock),
            TimerCommand::Pause => self.pause(clock),
            TimerCommand::Resume => self.resume(clock),
            TimerCommand::TogglePaused => {
                if self.running {
                    self.pause(clock)
                } else {
                    self.resume(clock)
                }
            }
            TimerCommand::Reset => self.reset(),
            TimerCommand::CompleteManually => self.complete_manually(clock),
            TimerCommand::ObserveTime => self.observe_time(clock),
            TimerCommand::SetMode(mode) => self.set_mode(mode),
        }
    }

    pub fn display_seconds(&self, clock: ClockObservation) -> u64 {
        if !self.running {
            return self.remaining_seconds;
        }
        if self.phase == TimerPhase::Stopwatch {
            return self.active_seconds(clock);
        }
        self.remaining_from_runtime(clock)
    }

    pub fn active_seconds(&self, clock: ClockObservation) -> u64 {
        if !matches!(
            self.phase,
            TimerPhase::Study | TimerPhase::Exam | TimerPhase::Stopwatch
        ) {
            return 0;
        }
        let mut total = 0_u64;
        for segment in &self.active_segments {
            let end = segment.ended_at.unwrap_or(clock.wall);
            total = total.saturating_add(segment_seconds(segment.started_at, end));
        }
        if total > 0 {
            return total;
        }

        if self.phase == TimerPhase::Stopwatch {
            return self.remaining_seconds;
        }
        configured_seconds(self)
            .saturating_sub(self.remaining_seconds)
            .saturating_sub(self.logged_split_seconds)
    }

    pub fn snapshot_for_persistence(&self, clock: ClockObservation) -> crate::timer::TimerSnapshot {
        let mut snapshot = crate::timer::TimerSnapshot::from_state(self);
        if self.running {
            snapshot.remaining_seconds = self.display_seconds(clock);
        }
        snapshot.last_alive_at = Some(clock.wall);
        snapshot
    }

    pub(crate) fn from_parts_without_runtime(
        phase: TimerPhase,
        running: bool,
        remaining_seconds: u64,
        logged_split_seconds: u64,
        active_segments: Vec<ActiveSegment>,
        started_at: Option<WallTimestamp>,
        ends_at: Option<WallTimestamp>,
        last_alive_at: Option<WallTimestamp>,
        config: TimerConfig,
        context: TimerContext,
    ) -> Self {
        Self {
            phase,
            running,
            remaining_seconds,
            logged_split_seconds,
            active_segments,
            started_at,
            ends_at,
            last_alive_at,
            config,
            context,
            runtime_anchor: None,
        }
    }

    pub(crate) fn reset_to_idle_preserving_context(&mut self) {
        self.phase = TimerPhase::Idle;
        self.running = false;
        self.remaining_seconds = idle_seconds(&self.config);
        self.logged_split_seconds = 0;
        self.active_segments.clear();
        self.started_at = None;
        self.ends_at = None;
        self.runtime_anchor = None;
    }

    fn start(&mut self, clock: ClockObservation) -> Vec<TimerEvent> {
        if self.running {
            return Vec::new();
        }
        if self.phase != TimerPhase::Idle {
            return self.resume(clock);
        }

        let mut events = vec![TimerEvent::PersistenceRequested];
        match self.config.mode {
            TimerMode::Endless => {
                self.phase = TimerPhase::Stopwatch;
                self.running = true;
                self.started_at = Some(clock.wall);
                self.ends_at = None;
                self.remaining_seconds = 0;
                self.logged_split_seconds = 0;
                self.active_segments = vec![ActiveSegment::open(clock.wall)];
                self.runtime_anchor = Some(RuntimeAnchor {
                    monotonic_millis: clock.monotonic_millis,
                    remaining_or_elapsed_seconds: 0,
                });
                events.insert(0, TimerEvent::Started { phase: self.phase });
            }
            TimerMode::Focus | TimerMode::Exam => {
                let total_seconds = self.remaining_seconds.max(1);
                self.phase = if self.config.mode == TimerMode::Exam {
                    TimerPhase::Exam
                } else {
                    TimerPhase::Study
                };
                self.running = true;
                self.started_at = Some(clock.wall);
                self.ends_at = Some(clock.wall.plus_seconds(total_seconds));
                self.remaining_seconds = total_seconds;
                self.logged_split_seconds = 0;
                self.active_segments = vec![ActiveSegment::open(clock.wall)];
                self.runtime_anchor = Some(RuntimeAnchor {
                    monotonic_millis: clock.monotonic_millis,
                    remaining_or_elapsed_seconds: total_seconds,
                });
                events.insert(0, TimerEvent::Started { phase: self.phase });
            }
        }
        events
    }

    fn pause(&mut self, clock: ClockObservation) -> Vec<TimerEvent> {
        if self.phase == TimerPhase::Idle || !self.running {
            return Vec::new();
        }

        self.remaining_seconds = self.display_seconds(clock);
        self.running = false;
        self.ends_at = None;
        close_open_segments(&mut self.active_segments, clock.wall);
        self.runtime_anchor = None;
        vec![
            TimerEvent::Paused { phase: self.phase },
            TimerEvent::PersistenceRequested,
        ]
    }

    fn resume(&mut self, clock: ClockObservation) -> Vec<TimerEvent> {
        if self.phase == TimerPhase::Idle || self.running {
            return Vec::new();
        }

        self.running = true;
        self.started_at = Some(clock.wall);
        if self.phase == TimerPhase::Stopwatch {
            let elapsed = self.active_seconds(clock).max(self.remaining_seconds);
            self.remaining_seconds = elapsed;
            self.ends_at = None;
            self.runtime_anchor = Some(RuntimeAnchor {
                monotonic_millis: clock.monotonic_millis,
                remaining_or_elapsed_seconds: elapsed,
            });
        } else {
            self.ends_at = Some(clock.wall.plus_seconds(self.remaining_seconds));
            self.runtime_anchor = Some(RuntimeAnchor {
                monotonic_millis: clock.monotonic_millis,
                remaining_or_elapsed_seconds: self.remaining_seconds,
            });
        }
        self.active_segments.push(ActiveSegment::open(clock.wall));
        vec![
            TimerEvent::Resumed { phase: self.phase },
            TimerEvent::PersistenceRequested,
        ]
    }

    fn reset(&mut self) -> Vec<TimerEvent> {
        let was_idle = self.phase == TimerPhase::Idle
            && !self.running
            && self.remaining_seconds == idle_seconds(&self.config)
            && self.active_segments.is_empty();
        self.reset_to_idle_preserving_context();
        if was_idle {
            Vec::new()
        } else {
            vec![TimerEvent::Reset, TimerEvent::PersistenceRequested]
        }
    }

    fn observe_time(&mut self, clock: ClockObservation) -> Vec<TimerEvent> {
        if !self.running || self.phase == TimerPhase::Stopwatch {
            return Vec::new();
        }
        if self.display_seconds(clock) > 0 {
            return Vec::new();
        }

        let ended_at = self.ends_at.unwrap_or(clock.wall);
        close_open_segments(&mut self.active_segments, ended_at);
        self.remaining_seconds = 0;
        self.running = false;
        self.ends_at = None;
        self.runtime_anchor = None;

        match self.phase {
            TimerPhase::Study => self.complete_study(CompletionReason::CountdownElapsed),
            TimerPhase::Exam => self.complete_exam(CompletionReason::CountdownElapsed),
            TimerPhase::Break => {
                self.reset_to_idle_preserving_context();
                vec![
                    TimerEvent::Completed {
                        phase: TimerPhase::Break,
                        reason: CompletionReason::CountdownElapsed,
                    },
                    TimerEvent::PersistenceRequested,
                ]
            }
            _ => Vec::new(),
        }
    }

    fn set_mode(&mut self, mode: TimerMode) -> Vec<TimerEvent> {
        if self.phase != TimerPhase::Idle || self.running {
            return Vec::new();
        }
        self.config.mode = mode;
        self.remaining_seconds = idle_seconds(&self.config);
        vec![TimerEvent::PersistenceRequested]
    }

    fn complete_manually(&mut self, clock: ClockObservation) -> Vec<TimerEvent> {
        if !matches!(
            self.phase,
            TimerPhase::Study | TimerPhase::Exam | TimerPhase::Stopwatch
        ) || self.active_seconds(clock) == 0
        {
            return Vec::new();
        }
        if self.running {
            close_open_segments(&mut self.active_segments, clock.wall);
        }
        self.running = false;
        self.ends_at = None;
        self.runtime_anchor = None;

        let phase = self.phase;
        let segments = closed_segments(&self.active_segments);
        let mut events = session_events(phase, CompletionReason::ManualSave, segments, self);
        self.reset_to_idle_preserving_context();
        events.push(TimerEvent::PersistenceRequested);
        events
    }

    fn complete_study(&mut self, reason: CompletionReason) -> Vec<TimerEvent> {
        let segments = closed_segments(&self.active_segments);
        let mut events = session_events(TimerPhase::Study, reason, segments, self);
        if self.config.mode == TimerMode::Focus && self.config.break_seconds > 0 {
            let started_at = self
                .active_segments
                .last()
                .and_then(|segment| segment.ended_at)
                .or(self.started_at)
                .unwrap_or(WallTimestamp::from_unix_millis(0));
            self.phase = TimerPhase::Break;
            self.running = true;
            self.started_at = Some(started_at);
            self.ends_at = Some(started_at.plus_seconds(self.config.break_seconds));
            self.remaining_seconds = self.config.break_seconds;
            self.logged_split_seconds = 0;
            self.active_segments.clear();
            self.runtime_anchor = None;
            events.push(TimerEvent::BreakStarted);
            events.push(TimerEvent::PersistenceRequested);
        } else {
            self.reset_to_idle_preserving_context();
            events.push(TimerEvent::PersistenceRequested);
        }
        events
    }

    fn complete_exam(&mut self, reason: CompletionReason) -> Vec<TimerEvent> {
        let segments = closed_segments(&self.active_segments);
        let mut events = session_events(TimerPhase::Exam, reason, segments, self);
        self.reset_to_idle_preserving_context();
        events.push(TimerEvent::PersistenceRequested);
        events
    }

    fn remaining_from_runtime(&self, clock: ClockObservation) -> u64 {
        let Some(anchor) = &self.runtime_anchor else {
            return self
                .ends_at
                .map(|ends_at| ends_at.seconds_until(clock.wall).max(0) as u64)
                .unwrap_or(self.remaining_seconds);
        };
        let elapsed = clock
            .monotonic_millis
            .saturating_sub(anchor.monotonic_millis)
            / 1000;
        anchor.remaining_or_elapsed_seconds.saturating_sub(elapsed)
    }
}

pub(crate) fn idle_seconds(config: &TimerConfig) -> u64 {
    match config.mode {
        TimerMode::Focus => config.study_seconds,
        TimerMode::Exam => config.exam_seconds,
        TimerMode::Endless => 0,
    }
}

pub(crate) fn configured_seconds(timer: &TimerState) -> u64 {
    match timer.phase {
        TimerPhase::Exam => timer.config.exam_seconds,
        TimerPhase::Break => timer.config.break_seconds,
        TimerPhase::Stopwatch => timer.remaining_seconds,
        _ => timer.config.study_seconds,
    }
}

pub(crate) fn close_open_segments(segments: &mut [ActiveSegment], ended_at: WallTimestamp) {
    for segment in segments {
        if segment.ended_at.is_none() {
            segment.ended_at = Some(ended_at);
        }
    }
}

pub(crate) fn closed_segments(segments: &[ActiveSegment]) -> Vec<ActiveSegment> {
    segments
        .iter()
        .filter(|segment| segment.ended_at.is_some())
        .cloned()
        .collect()
}

pub(crate) fn segment_seconds(started_at: WallTimestamp, ended_at: WallTimestamp) -> u64 {
    if ended_at.unix_millis <= started_at.unix_millis {
        return 0;
    }
    ((ended_at.unix_millis - started_at.unix_millis) / 1000) as u64
}

pub(crate) fn session_events(
    phase: TimerPhase,
    reason: CompletionReason,
    segments: Vec<ActiveSegment>,
    timer: &TimerState,
) -> Vec<TimerEvent> {
    if segments.is_empty() {
        return vec![TimerEvent::Completed { phase, reason }];
    }
    vec![
        TimerEvent::SessionRangeReady {
            phase,
            reason,
            segments,
            context: timer.context.clone(),
            preset_label: timer.config.preset_label.clone(),
        },
        TimerEvent::Completed { phase, reason },
    ]
}

fn div_ceil_i64(value: i64, divisor: i64) -> i64 {
    if value <= 0 {
        value / divisor
    } else {
        (value + divisor - 1) / divisor
    }
}
