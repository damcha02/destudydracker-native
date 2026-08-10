use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppModel {
    title: String,
    status: String,
    timer: TimerState,
    modes: Vec<TimerModeConfig>,
    session_notes: Vec<SessionNote>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimerState {
    selected_mode: usize,
    duration: Duration,
    remaining_when_stopped: Duration,
    status: TimerStatus,
    started_at: Option<Instant>,
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
        let modes = vec![
            TimerModeConfig::new("Pomodoro", "25 / 5", 25, 0, ModeTone::Study),
            TimerModeConfig::new("Deep Work", "52 / 17", 52, 0, ModeTone::DeepWork),
            TimerModeConfig::new("Exam", "120 min", 120, 0, ModeTone::Exam),
            TimerModeConfig::new("Demo", "00:10", 0, 10, ModeTone::Demo),
        ];
        let selected_mode = 1;
        let duration = modes[selected_mode].duration();

        Self {
            title: "Study Tracker Native Prototype".to_string(),
            status: "Stage 4: representative native focus timer".to_string(),
            timer: TimerState::new(selected_mode, duration),
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
        }
    }

    pub fn apply(&mut self, command: AppCommand) {
        match command {
            AppCommand::MarkPresentationReady => {
                self.status = "Stage 4 timer model loaded through Rust adapter".to_string();
            }
            AppCommand::Start(now) => self.timer.start(now),
            AppCommand::Pause(now) => self.timer.pause(now),
            AppCommand::Reset => self
                .timer
                .reset(self.mode_duration(self.timer.selected_mode)),
            AppCommand::SetMode(index) => {
                if index < self.modes.len() && !self.timer.is_running() {
                    self.timer = TimerState::new(index, self.mode_duration(index));
                }
            }
            AppCommand::Refresh(now) => self.timer.refresh(now),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn timer(&self) -> &TimerState {
        &self.timer
    }

    pub fn modes(&self) -> &[TimerModeConfig] {
        &self.modes
    }

    pub fn session_notes(&self) -> &[SessionNote] {
        &self.session_notes
    }

    fn mode_duration(&self, index: usize) -> Duration {
        self.modes[index].duration()
    }
}

impl TimerState {
    fn new(selected_mode: usize, duration: Duration) -> Self {
        Self {
            selected_mode,
            duration,
            remaining_when_stopped: duration,
            status: TimerStatus::Ready,
            started_at: None,
        }
    }

    fn start(&mut self, now: Instant) {
        if self.status == TimerStatus::Completed {
            self.remaining_when_stopped = self.duration;
        }
        if self.remaining_when_stopped.is_zero() {
            self.remaining_when_stopped = self.duration;
        }
        if !self.is_running() {
            self.started_at = Some(now);
            self.status = TimerStatus::Running;
        }
    }

    fn pause(&mut self, now: Instant) {
        if self.is_running() {
            self.remaining_when_stopped = self.remaining(now);
            self.started_at = None;
            self.status = if self.remaining_when_stopped.is_zero() {
                TimerStatus::Completed
            } else {
                TimerStatus::Paused
            };
        }
    }

    fn reset(&mut self, duration: Duration) {
        self.duration = duration;
        self.remaining_when_stopped = duration;
        self.status = TimerStatus::Ready;
        self.started_at = None;
    }

    fn refresh(&mut self, now: Instant) {
        if self.is_running() && self.remaining(now).is_zero() {
            self.remaining_when_stopped = Duration::ZERO;
            self.status = TimerStatus::Completed;
            self.started_at = None;
        }
    }

    pub fn selected_mode(&self) -> usize {
        self.selected_mode
    }

    pub fn status(&self) -> TimerStatus {
        self.status
    }

    pub fn is_running(&self) -> bool {
        self.status == TimerStatus::Running
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        if let Some(started_at) = self.started_at {
            self.remaining_when_stopped
                .saturating_sub(now.saturating_duration_since(started_at))
        } else {
            self.remaining_when_stopped
        }
    }

    pub fn elapsed(&self, now: Instant) -> Duration {
        self.duration.saturating_sub(self.remaining(now))
    }

    pub fn progress_fraction(&self, now: Instant) -> f32 {
        let total = self.duration.as_secs_f32();
        if total <= 0.0 {
            1.0
        } else {
            (self.elapsed(now).as_secs_f32() / total).clamp(0.0, 1.0)
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
        tone: ModeTone,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            minutes,
            seconds,
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

#[cfg(test)]
mod tests {
    use super::{format_clock, AppCommand, AppModel, TimerStatus};
    use std::time::{Duration, Instant};

    #[test]
    fn initial_timer_state_is_ready() {
        let model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        assert_eq!(model.timer().status(), TimerStatus::Ready);
        assert_eq!(model.timer().selected_mode(), 1);
        assert_eq!(model.timer().remaining(now), Duration::from_secs(52 * 60));
        assert_eq!(model.timer().progress_fraction(now), 0.0);
    }

    #[test]
    fn start_marks_timer_running_without_consuming_time() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::Start(now));

        assert_eq!(model.timer().status(), TimerStatus::Running);
        assert_eq!(model.timer().remaining(now), Duration::from_secs(52 * 60));
    }

    #[test]
    fn elapsed_time_uses_supplied_monotonic_instant() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::Start(now));

        assert_eq!(
            model.timer().remaining(now + Duration::from_secs(75)),
            Duration::from_secs(3045)
        );
        assert!(
            (model
                .timer()
                .progress_fraction(now + Duration::from_secs(1560))
                - 0.5)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn pause_freezes_remaining_time() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Pause(now + Duration::from_secs(20)));

        assert_eq!(model.timer().status(), TimerStatus::Paused);
        assert_eq!(
            model.timer().remaining(now + Duration::from_secs(1000)),
            Duration::from_secs(3100)
        );
    }

    #[test]
    fn resume_continues_from_paused_remaining_time() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Pause(now + Duration::from_secs(20)));
        model.apply(AppCommand::Start(now + Duration::from_secs(80)));

        assert_eq!(
            model.timer().remaining(now + Duration::from_secs(90)),
            Duration::from_secs(3090)
        );
    }

    #[test]
    fn reset_restores_selected_mode_duration() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Reset);

        assert_eq!(model.timer().status(), TimerStatus::Ready);
        assert_eq!(
            model.timer().remaining(now + Duration::from_secs(500)),
            Duration::from_secs(52 * 60)
        );
    }

    #[test]
    fn completion_clamps_at_zero_and_stops_running() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::SetMode(3));
        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Refresh(now + Duration::from_secs(11)));

        assert_eq!(model.timer().status(), TimerStatus::Completed);
        assert_eq!(
            model.timer().remaining(now + Duration::from_secs(100)),
            Duration::ZERO
        );
        assert_eq!(
            model
                .timer()
                .progress_fraction(now + Duration::from_secs(100)),
            1.0
        );
    }

    #[test]
    fn repeated_commands_are_idempotent() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Start(now + Duration::from_secs(5)));
        model.apply(AppCommand::Pause(now + Duration::from_secs(10)));
        model.apply(AppCommand::Pause(now + Duration::from_secs(20)));

        assert_eq!(
            model.timer().remaining(now + Duration::from_secs(30)),
            Duration::from_secs(3110)
        );
    }

    #[test]
    fn mode_changes_are_blocked_while_running() {
        let mut model = AppModel::stage_four_timer_preview();
        let now = Instant::now();

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
