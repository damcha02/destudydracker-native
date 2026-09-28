use crate::dashboard::{DashboardScenario, DashboardSnapshot};
use crate::timer_controller::{NullPersistencePort, TimerApplicationEffect, TimerController};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use study_tracker_core::timer::{
    ClockObservation, TimerCommand, TimerConfig, TimerMode, TimerPhase,
};

const DEFAULT_HISTORY_POINTS: usize = 30;

#[derive(Debug, Clone, PartialEq)]
pub struct AppModel {
    title: String,
    status: String,
    timer: AppTimer,
    dashboard: DashboardSnapshot,
    dashboard_scenario: DashboardScenario,
    dashboard_points: usize,
    plot_size: (f32, f32),
    modes: Vec<TimerModeConfig>,
    session_notes: Vec<SessionNote>,
    clock_origin: Instant,
    wall_origin_unix_millis: i64,
}

/// The Slint-facing timer wrapper. As of Stage 14 this is a thin read/effect-forwarding shell
/// around [`TimerController`] (application layer) - it owns no transition logic of its own; see
/// `docs/stage14-timer-productionization.md` for the full architecture writeup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppTimer {
    controller: TimerController,
    /// Effects the most recent command produced, for callers (tests today; a future Stage 16
    /// session subsystem) that want to react to a completed session range. Cleared and replaced
    /// on every `apply_events`, never accumulated across commands.
    pending_effects: Vec<TimerApplicationEffect>,
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
    Endless,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionNote {
    title: String,
    detail: String,
    minutes: u16,
    confidence: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppCommand {
    MarkPresentationReady,
    Start(Instant),
    Pause(Instant),
    Reset,
    SetMode(usize),
    Refresh(Instant),
    SetDashboardPoints(usize),
    SetDashboardScenario(DashboardScenario),
    SelectWeekday(usize),
    StepWeekday(i32),
    SelectHistoryFraction(f32),
    StepHistory(i32),
    ResizeHistoryPlot(f32, f32),
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
            // Matches production's Endless mode (desktop/src/App.tsx: mode "endless" / phase
            // "stopwatch" - counts up, no end time). Appended last rather than in production's
            // own picker order so every existing index-based test/reference above stays valid;
            // Sprint and Custom (production's other two presets) are deliberately not
            // reproduced yet - see docs/stage14-timer-productionization.md, "Production
            // divergences".
            TimerModeConfig::new(
                "Endless",
                "Counts up",
                0,
                0,
                0,
                TimerMode::Endless,
                ModeTone::Endless,
            ),
        ];
        let selected_mode = 1;

        Self {
            title: crate::platform::identity::window_title(),
            status: "Stage 8: Slint adapter driving renderer-independent timer core".to_string(),
            timer: AppTimer::new(selected_mode, &modes[selected_mode]),
            dashboard: DashboardSnapshot::build(DashboardScenario::Typical, DEFAULT_HISTORY_POINTS),
            dashboard_scenario: DashboardScenario::Typical,
            dashboard_points: DEFAULT_HISTORY_POINTS,
            plot_size: (600.0, 300.0),
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
                let command = if self.timer.controller.core().phase == TimerPhase::Idle {
                    TimerCommand::Start
                } else {
                    TimerCommand::Resume
                };
                self.apply_timer_command(command, now);
            }
            AppCommand::Pause(now) => self.apply_timer_command(TimerCommand::Pause, now),
            AppCommand::Reset => self.apply_timer_command(TimerCommand::Reset, self.clock_origin),
            AppCommand::SetMode(index) => {
                if index < self.modes.len()
                    && self.timer.controller.core().phase == TimerPhase::Idle
                    && !self.timer.controller.core().running
                {
                    // Reinitialize in place rather than building a whole new `AppTimer`: this
                    // keeps the same persistence port instance across a mode switch instead of
                    // silently discarding it (relevant once Stage 15's real adapter replaces
                    // `NullPersistencePort` - a fresh port per mode switch would be a bug then).
                    self.timer
                        .controller
                        .reinitialize(index, self.modes[index].core_config());
                    self.timer.pending_effects.clear();
                }
            }
            AppCommand::Refresh(now) => self.apply_timer_command(TimerCommand::ObserveTime, now),
            AppCommand::SetDashboardPoints(count) => {
                self.dashboard_points = count;
                self.rebuild_dashboard();
            }
            AppCommand::SetDashboardScenario(scenario) => {
                self.dashboard_scenario = scenario;
                self.rebuild_dashboard();
            }
            AppCommand::SelectWeekday(index) => self.dashboard.weekly.select(index),
            AppCommand::StepWeekday(delta) => self.dashboard.weekly.step(delta),
            AppCommand::SelectHistoryFraction(fraction) => {
                self.dashboard.history.select_fraction(fraction)
            }
            AppCommand::StepHistory(delta) => self.dashboard.history.step(delta),
            AppCommand::ResizeHistoryPlot(width, height) => {
                self.plot_size = (width, height);
                self.dashboard.history.set_plot_size(width, height);
            }
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

    pub fn dashboard(&self) -> &DashboardSnapshot {
        &self.dashboard
    }

    pub fn dashboard_points(&self) -> usize {
        self.dashboard_points
    }

    fn rebuild_dashboard(&mut self) {
        self.dashboard = DashboardSnapshot::build(self.dashboard_scenario, self.dashboard_points);
        // Rebuilt charts must keep the plot size Slint last reported.
        self.dashboard
            .history
            .set_plot_size(self.plot_size.0, self.plot_size.1);
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
        self.timer.pending_effects = self.timer.controller.apply(command, clock);
    }

    /// Application effects (session ranges, completions) produced by the most recent timer
    /// command. Not yet consumed by anything (Stage 16 owns the session subsystem that will);
    /// exposed now so tests - and this crate's own future callers - can observe routing without
    /// waiting for that stage. Cleared and replaced on every command, never accumulated.
    /// Exercised by this module's own tests below; not yet called from `main.rs` (there is no
    /// Slint-facing surface for a session range until Stage 16 exists).
    #[allow(dead_code)]
    pub fn timer_effects(&self) -> &[TimerApplicationEffect] {
        &self.timer.pending_effects
    }
}

impl AppTimer {
    fn new(selected_mode: usize, mode: &TimerModeConfig) -> Self {
        Self {
            controller: TimerController::new(
                selected_mode,
                mode.core_config(),
                Box::new(NullPersistencePort),
            ),
            pending_effects: Vec::new(),
        }
    }

    pub fn selected_mode(&self) -> usize {
        self.controller.selected_mode()
    }

    pub fn status(&self) -> TimerStatus {
        let core = self.controller.core();
        if self.controller.last_completion().is_some() && core.phase == TimerPhase::Idle {
            TimerStatus::Completed
        } else if core.running {
            TimerStatus::Running
        } else if core.phase == TimerPhase::Idle {
            TimerStatus::Ready
        } else {
            TimerStatus::Paused
        }
    }

    pub fn status_label(&self) -> &'static str {
        let core = self.controller.core();
        match core.phase {
            TimerPhase::Break if core.running => "Break",
            TimerPhase::Break => "Break paused",
            TimerPhase::Exam if core.running => "Exam",
            TimerPhase::Stopwatch if core.running => "Stopwatch",
            _ => self.status().as_str(),
        }
    }

    pub fn is_running(&self) -> bool {
        self.controller.core().running
    }

    pub fn duration(&self) -> Duration {
        let core = self.controller.core();
        let seconds = match core.phase {
            TimerPhase::Break => core.config.break_seconds,
            TimerPhase::Exam => core.config.exam_seconds,
            TimerPhase::Stopwatch => core.display_seconds(ClockObservation::new(0, 0)).max(1),
            _ => core.config.study_seconds,
        };
        Duration::from_secs(seconds.max(1))
    }

    pub fn remaining(&self, clock: ClockObservation) -> Duration {
        Duration::from_secs(self.controller.core().display_seconds(clock))
    }

    pub fn elapsed(&self, clock: ClockObservation) -> Duration {
        if self.controller.core().phase == TimerPhase::Stopwatch {
            Duration::from_secs(self.controller.core().display_seconds(clock))
        } else {
            self.duration().saturating_sub(self.remaining(clock))
        }
    }

    pub fn progress_fraction(&self, clock: ClockObservation) -> f32 {
        if self.controller.core().phase == TimerPhase::Stopwatch {
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
            // Falls through Theme.tone()'s default branch (ui/theme.slint) to Theme.primary -
            // deliberately reused rather than adding a 5th Slint accent color for one preset.
            ModeTone::Endless => 4,
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
    use crate::dashboard::DashboardScenario;
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
    fn endless_mode_is_selectable_and_counts_up_without_an_end_time() {
        let now = Instant::now();
        let mut model = model_at(now);
        let endless_index = model
            .modes()
            .iter()
            .position(|m| m.label() == "Endless")
            .expect("Endless must be one of the selectable modes");

        model.apply(AppCommand::SetMode(endless_index));
        assert_eq!(model.timer().selected_mode(), endless_index);
        assert_eq!(model.timer().status(), TimerStatus::Ready);

        model.apply(AppCommand::Start(now));
        assert_eq!(model.timer().status(), TimerStatus::Running);
        assert_eq!(
            model
                .timer()
                .remaining(model.clock(now + Duration::from_secs(90))),
            Duration::from_secs(90),
            "Endless counts elapsed time up, not down"
        );
    }

    #[test]
    fn completion_exposes_a_session_range_application_effect() {
        let now = Instant::now();
        let mut model = model_at(now);

        // Demo preset (index 3): 10s study, no break, so completion goes straight to idle with
        // exactly one recorded session range - the same effect a real UI/Stage 16 would consume.
        model.apply(AppCommand::SetMode(3));
        model.apply(AppCommand::Start(now));
        model.apply(AppCommand::Refresh(now + Duration::from_secs(11)));

        let ranges = model
            .timer_effects()
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    crate::timer_controller::TimerApplicationEffect::SessionRangeReady { .. }
                )
            })
            .count();
        assert_eq!(ranges, 1);

        // The next, unrelated command's effects must not still carry the old completion.
        model.apply(AppCommand::Reset);
        assert!(model.timer_effects().is_empty());
    }

    #[test]
    fn clock_format_is_zero_padded() {
        assert_eq!(format_clock(Duration::from_secs(9)), "00:09");
        assert_eq!(format_clock(Duration::from_secs(125)), "02:05");
    }

    #[test]
    fn dashboard_commands_rebuild_and_select_without_touching_the_timer() {
        let mut model = AppModel::stage_four_timer_preview();
        let timer_before = model.timer().clone();

        model.apply(AppCommand::SetDashboardPoints(1_000));
        assert_eq!(model.dashboard().history.points.len(), 1_000);
        assert_eq!(model.dashboard_points(), 1_000);

        model.apply(AppCommand::SelectHistoryFraction(0.0));
        assert_eq!(model.dashboard().history.selected, Some(0));
        model.apply(AppCommand::StepHistory(2));
        assert_eq!(model.dashboard().history.selected, Some(2));

        model.apply(AppCommand::SelectWeekday(1));
        model.apply(AppCommand::StepWeekday(-5));
        assert_eq!(model.dashboard().weekly.selected, 0);

        model.apply(AppCommand::SetDashboardScenario(DashboardScenario::Empty));
        assert!(model.dashboard().history.points.is_empty());
        model.apply(AppCommand::SelectHistoryFraction(0.5));
        model.apply(AppCommand::StepHistory(1));
        assert_eq!(model.dashboard().history.selected, None);

        assert_eq!(model.timer(), &timer_before);
    }
}
