use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use study_tracker_core::timer::{
    ClockObservation, CompletionReason, TimerCommand, TimerConfig, TimerContext, TimerEvent,
    TimerMode, TimerPhase, TimerState as CoreTimerState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppModel {
    title: String,
    status: String,
    timer: AppTimer,
    modes: Vec<TimerModeConfig>,
    session_notes: Vec<SessionNote>,
    clock_origin: Instant,
    wall_origin_unix_millis: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTimer {
    selected_mode: usize,
    core: CoreTimerState,
    last_completion: Option<TimerPhase>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerStatus {
    Ready,
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerModeConfig {
    label: String,
    detail: String,
    minutes: u64,
    seconds: u64,
    break_minutes: u64,
    mode: TimerMode,
    tone: ModeTone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeTone {
    Study,
    DeepWork,
    Exam,
    Demo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNote {
    title: String,
    detail: String,
    minutes: u16,
    confidence: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    MarkPresentationReady,
    Start(Instant),
    Pause(Instant),
    Reset,
    SetMode(usize),
    Refresh(Instant),
}

impl AppModel {
    pub fn stage_four_timer_preview() -> Self {
        Self::stage_four_timer_preview_with_clock(Instant::now(), system_unix_millis())
    }

    fn stage_four_timer_preview_with_clock(
        clock_origin: Instant,
        wall_origin_unix_millis: i64,
    ) -> Self {
        let modes = vec![
            TimerModeConfig::new(
                "Pomodoro",
                "25 / 5",
                25,
                0,
                5,
                TimerMode::Focus,
                ModeTone::Study,
            ),
            TimerModeConfig::new(
                "Deep Work",
                "52 / 17",
                52,
                0,
                17,
                TimerMode::Focus,
                ModeTone::DeepWork,
            ),
            TimerModeConfig::new(
                "Exam",
                "120 min",
                120,
                0,
                0,
                TimerMode::Exam,
                ModeTone::Exam,
            ),
            TimerModeConfig::new("Demo", "00:10", 0, 10, 0, TimerMode::Focus, ModeTone::Demo),
        ];
        let selected_mode = 1;

        Self {
            title: "Study Tracker Native Prototype".to_string(),
            status: "Stage 8: Slint adapter driving renderer-independent timer core".to_string(),
            timer: AppTimer::new(selected_mode, &modes[selected_mode]),
            modes,
            session_notes: vec![
                SessionNote::new(
                    "Analysis problem set",
                    "General focus · 52 min · confidence 4/5",
                    52,
                    4,
                ),
                SessionNote::new(
                    "Physics derivation",
                    "Exam prep · 90 min · confidence 3/5",
                    90,
                    3,
                ),
                SessionNote::new(
                    "Linear algebra review",
                    "Pomodoro · 25 min · confidence 5/5",
                    25,
                    5,
                ),
            ],
            clock_origin,
            wall_origin_unix_millis,
        }
    }

    pub fn apply(&mut self, command: AppCommand) {
        match command {
            AppCommand::MarkPresentationReady => {
                self.status = "Stage 8 timer UI is backed by study-tracker-core".to_string();
            }
            AppCommand::Start(now) => {
                let command = if self.timer.core.phase == TimerPhase::Idle {
                    TimerCommand::Start
                } else {
                    TimerCommand::Resume
                };
                self.apply_timer_command(command, now);
            }
            AppCommand::Pause(now) => self.apply_timer_command(TimerCommand::Pause, now),
            AppCommand::Reset => {
                self.timer
                    .core
                    .apply(TimerCommand::Reset, self.clock(self.clock_origin));
                self.timer.last_completion = None;
            }
            AppCommand::SetMode(index) => {
                if index < self.modes.len()
                    && self.timer.core.phase == TimerPhase::Idle
                    && !self.timer.core.running
                {
                    self.timer = AppTimer::new(index, &self.modes[index]);
                }
            }
            AppCommand::Refresh(now) => self.apply_timer_command(TimerCommand::ObserveTime, now),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn timer(&self) -> &AppTimer {
        &self.timer
    }

    pub fn modes(&self) -> &[TimerModeConfig] {
        &self.modes
    }

    pub fn session_notes(&self) -> &[SessionNote] {
        &self.session_notes
    }

    pub fn clock(&self, now: Instant) -> ClockObservation {
        let elapsed = now.saturating_duration_since(self.clock_origin);
        let elapsed_millis = elapsed.as_millis().min(u128::from(u64::MAX)) as u64;
        let wall_unix_millis = self
            .wall_origin_unix_millis
            .saturating_add(i64::try_from(elapsed_millis).unwrap_or(i64::MAX));
        ClockObservation::new(elapsed_millis, wall_unix_millis)
    }

    fn apply_timer_command(&mut self, command: TimerCommand, now: Instant) {
        let clock = self.clock(now);
        let events = self.timer.core.apply(command, clock);
        self.timer.apply_events(&events);
    }
}

impl AppTimer {
    fn new(selected_mode: usize, mode: &TimerModeConfig) -> Self {
        Self {
            selected_mode,
            core: CoreTimerState::new(mode.core_config(), TimerContext::default()),
            last_completion: None,
        }
    }

    fn apply_events(&mut self, events: &[TimerEvent]) {
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

    pub fn selected_mode(&self) -> usize {
        self.selected_mode
    }

    pub fn status(&self) -> TimerStatus {
        if self.last_completion.is_some() && self.core.phase == TimerPhase::Idle {
            TimerStatus::Completed
        } else if self.core.running {
            TimerStatus::Running
        } else if self.core.phase == TimerPhase::Idle {
            TimerStatus::Ready
        } else {
            TimerStatus::Paused
        }
    }

    pub fn status_label(&self) -> &'static str {
        match self.core.phase {
            TimerPhase::Break if self.core.running => "Break",
            TimerPhase::Break => "Break paused",
            TimerPhase::Exam if self.core.running => "Exam",
            TimerPhase::Stopwatch if self.core.running => "Stopwatch",
            _ => self.status().as_str(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.core.running
    }

    pub fn duration(&self) -> Duration {
        let seconds = match self.core.phase {
            TimerPhase::Break => self.core.config.break_seconds,
            TimerPhase::Exam => self.core.config.exam_seconds,
            TimerPhase::Stopwatch => self
                .core
                .display_seconds(ClockObservation::new(0, 0))
                .max(1),
            _ => self.core.config.study_seconds,
        };
        Duration::from_secs(seconds.max(1))
    }

    pub fn remaining(&self, clock: ClockObservation) -> Duration {
        Duration::from_secs(self.core.display_seconds(clock))
    }

    pub fn elapsed(&self, clock: ClockObservation) -> Duration {
        if self.core.phase == TimerPhase::Stopwatch {
            Duration::from_secs(self.core.display_seconds(clock))
        } else {
            self.duration().saturating_sub(self.remaining(clock))
        }
    }

    pub fn progress_fraction(&self, clock: ClockObservation) -> f32 {
        if self.core.phase == TimerPhase::Stopwatch {
            return 1.0;
        }
        let total = self.duration().as_secs_f32();
        if total <= 0.0 {
            1.0
        } else {
            (self.elapsed(clock).as_secs_f32() / total).clamp(0.0, 1.0)
        }
    }
}

impl TimerStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TimerStatus::Ready => "Ready",
            TimerStatus::Running => "In session",
            TimerStatus::Paused => "Paused",
            TimerStatus::Completed => "Completed",
        }
    }
}

impl TimerModeConfig {
    fn new(
        label: impl Into<String>,
        detail: impl Into<String>,
        minutes: u64,
        seconds: u64,
        break_minutes: u64,
        mode: TimerMode,
        tone: ModeTone,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            minutes,
            seconds,
            break_minutes,
            mode,
            tone,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn tone(&self) -> ModeTone {
        self.tone
    }

    fn duration(&self) -> Duration {
        Duration::from_secs(self.minutes * 60 + self.seconds)
    }

    fn core_config(&self) -> TimerConfig {
        TimerConfig {
            mode: self.mode,
            study_seconds: self.duration().as_secs().max(1),
            break_seconds: self.break_minutes * 60,
            exam_seconds: self.duration().as_secs().max(1),
            preset_label: self.label.clone(),
        }
    }
}

impl ModeTone {
    pub fn accent_index(self) -> i32 {
        match self {
            ModeTone::Study => 0,
            ModeTone::DeepWork => 1,
            ModeTone::Exam => 2,
            ModeTone::Demo => 3,
        }
    }
}

impl SessionNote {
    fn new(
        title: impl Into<String>,
        detail: impl Into<String>,
        minutes: u16,
        confidence: u8,
    ) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            minutes,
            confidence: confidence.min(5),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn minutes(&self) -> u16 {
        self.minutes
    }

    pub fn confidence(&self) -> u8 {
        self.confidence
    }
}

pub fn format_clock(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn system_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{format_clock, AppCommand, AppModel, TimerStatus};
    use std::time::{Duration, Instant};

    fn model_at(now: Instant) -> AppModel {
        AppModel::stage_four_timer_preview_with_clock(now, 0)
    }

    #[test]
    fn initial_timer_state_is_ready() {
        let now = Instant::now();
        let model = model_at(now);
        let clock = model.clock(now);

        assert_eq!(model.timer().status(), TimerStatus::Ready);
        assert_eq!(model.timer().selected_mode(), 1);
        assert_eq!(model.timer().remaining(clock), Duration::from_secs(52 * 60));
        assert_eq!(model.timer().progress_fraction(clock), 0.0);
    }

    #[test]
    fn start_marks_timer_running_without_consuming_time() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));

        assert_eq!(model.timer().status(), TimerStatus::Running);
        assert_eq!(
            model.timer().remaining(model.clock(now)),
            Duration::from_secs(52 * 60)
        );
    }

    #[test]
    fn elapsed_time_uses_supplied_monotonic_instant() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));

        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(75))),
            Duration::from_secs(3045)
        );
        assert!(
            (model
                .timer()
                .progress_fraction(model.clock(now + Duration::from_secs(1560)))
                - 0.5)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn pause_freezes_remaining_time() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Pause(now + Duration::from_secs(20)));

        assert_eq!(model.timer().status(), TimerStatus::Paused);
        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(1000))),
            Duration::from_secs(3100)
        );
    }

    #[test]
    fn resume_continues_from_paused_remaining_time() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Pause(now + Duration::from_secs(20)));
        model.apply(AppCommand::Start(now + Duration::from_secs(80)));

        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(90))),
            Duration::from_secs(3090)
        );
    }

    #[test]
    fn reset_restores_selected_mode_duration() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Reset);

        assert_eq!(model.timer().status(), TimerStatus::Ready);
        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(500))),
            Duration::from_secs(52 * 60)
        );
    }

    #[test]
    fn completion_uses_core_focus_to_idle_semantics_for_demo_without_break() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::SetMode(3));
        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Refresh(now + Duration::from_secs(11)));

        assert_eq!(model.timer().status(), TimerStatus::Completed);
        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(100))),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn repeated_commands_are_idempotent() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Start(now + Duration::from_secs(5)));
        model.apply(AppCommand::Pause(now + Duration::from_secs(10)));
        model.apply(AppCommand::Pause(now + Duration::from_secs(20)));

        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(30))),
            Duration::from_secs(3110)
        );
    }

    #[test]
    fn mode_changes_are_blocked_while_running() {
        let now = Instant::now();
        let mut model = model_at(now);

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::SetMode(0));

        assert_eq!(model.timer().selected_mode(), 1);
    }

    #[test]
    fn clock_format_is_zero_padded() {
        assert_eq!(format_clock(Duration::from_secs(9)), "00:09");
        assert_eq!(format_clock(Duration::from_secs(125)), "02:05");
    }
}
