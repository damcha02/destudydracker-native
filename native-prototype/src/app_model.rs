use crate::academic_controller::{
    AcademicController, AcademicPersistencePort, NullAcademicPersistencePort,
};
use crate::dashboard::{DashboardScenario, DashboardSnapshot};
use crate::timer_controller::{
    NullPersistencePort, TimerApplicationEffect, TimerController, TimerPersistencePort,
};
use chrono::FixedOffset;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use study_tracker_core::academic::{
    Course, CourseId, Semester, SemesterId, SessionKind, StudySession,
};
use study_tracker_core::timer::{
    ClockObservation, RestoreInput, TimerCommand, TimerConfig, TimerMode, TimerPhase, TimerSnapshot,
};

const DEFAULT_HISTORY_POINTS: usize = 30;
/// How many recent sessions the session-notes card surfaces - `AcademicState::sessions` is
/// already newest-first (see `study-tracker-core`'s own doc comments), so this is simply "the
/// first N"; production's own equivalent panel shows a scrollable full list ("scroll for more" -
/// see `App.tsx`), which this Slint surface does not yet replicate (see
/// `docs/stage16-academic-domain.md` section 22, "UI integration").
const RECENT_SESSIONS_SHOWN: usize = 8;

#[derive(Debug, Clone, PartialEq)]
pub struct AppModel {
    title: String,
    status: String,
    timer: AppTimer,
    academic: AcademicController,
    /// Captured once per `AppModel` (not read fresh per Timer completion) - see
    /// `session_service`'s module docs for why this is a deliberate simplification rather than a
    /// per-instant timezone lookup.
    local_offset: FixedOffset,
    dashboard: DashboardSnapshot,
    dashboard_scenario: DashboardScenario,
    dashboard_points: usize,
    plot_size: (f32, f32),
    modes: Vec<TimerModeConfig>,
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
    /// Stage 16's minimal real CRUD surface (see `docs/stage16-academic-domain.md` section 22,
    /// "UI integration", for why this is intentionally not yet a full interactive Planner form).
    AddSemester {
        id: SemesterId,
        name: String,
        created_at: Instant,
    },
    AddCourse {
        id: CourseId,
        semester_id: SemesterId,
        name: String,
        color: String,
        created_at: Instant,
    },
    RemoveCourse(CourseId),
    RemoveSemester(SemesterId),
}

impl AppModel {
    /// The Stage 4-14 demo/test entry point: always starts fresh idle with `NullPersistencePort`
    /// (no real storage). `main.rs`'s real runtime uses `with_timer_persistence` instead as of
    /// Stage 15; this constructor is kept, `#[allow(dead_code)]`-marked, purely because every
    /// existing test in this module (and `main.rs`'s own dashboard/map/text-view regression
    /// checks - see docs/stage14-timer-productionization.md section 28) still deliberately builds its
    /// model this way, since none of them need real storage.
    #[allow(dead_code)]
    pub fn stage_four_timer_preview() -> Self {
        Self::stage_four_timer_preview_with_clock(Instant::now(), system_unix_millis())
    }

    fn default_modes() -> Vec<TimerModeConfig> {
        vec![
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
        ]
    }

    #[allow(dead_code)] // only reachable via stage_four_timer_preview - see its own doc comment
    fn stage_four_timer_preview_with_clock(
        clock_origin: Instant,
        wall_origin_unix_millis: i64,
    ) -> Self {
        let modes = Self::default_modes();
        let selected_mode = 1;

        Self {
            title: crate::platform::identity::window_title(),
            status: "Stage 8: Slint adapter driving renderer-independent timer core".to_string(),
            timer: AppTimer::new(selected_mode, &modes[selected_mode]),
            academic: AcademicController::new(Box::new(NullAcademicPersistencePort)),
            // A deterministic, host-independent offset for the demo/test path - see the
            // `local_offset` field's own doc comment. Real runtime uses the actual OS offset
            // (`with_timer_persistence_and_clock`, below).
            local_offset: FixedOffset::east_opt(0).expect("UTC is always a valid fixed offset"),
            dashboard: DashboardSnapshot::build(DashboardScenario::Typical, DEFAULT_HISTORY_POINTS),
            dashboard_scenario: DashboardScenario::Typical,
            dashboard_points: DEFAULT_HISTORY_POINTS,
            plot_size: (600.0, 300.0),
            modes,
            clock_origin,
            wall_origin_unix_millis,
        }
    }

    /// The real Stage 15/16 runtime entry point: builds the same modes/dashboard as
    /// [`AppModel::stage_four_timer_preview`], but wires the given (real, durable) persistence
    /// ports into the timer and academic domain instead of their `Null*` equivalents, and - if
    /// the timer port already has a snapshot to offer - restores from it instead of starting
    /// fresh idle. A restored/recovered session range is routed into a real, persisted
    /// `StudySession` exactly like a live completion would be (see
    /// `AcademicController::route_timer_effects`) before this function returns, so recovery
    /// creates its session before the first frame is ever shown - not on some later tick.
    ///
    /// Not used by any test in this module - every existing test deliberately keeps using
    /// `stage_four_timer_preview`'s `Null*Port`s, since none of them need real storage and the
    /// trait-object indirection would only make them harder to read. This function's own storage
    /// behavior is exercised by `persistence::timer_port`'s, `persistence::academic_port`'s, and
    /// `persistence::migration`'s tests, and by `TimerController::restore`'s tests directly; the
    /// startup-recovery-creates-a-session behavior is exercised by this module's own
    /// `with_timer_persistence_*` tests, below.
    pub fn with_timer_persistence(
        timer_persistence: Box<dyn TimerPersistencePort>,
        academic_persistence: Box<dyn AcademicPersistencePort>,
    ) -> (Self, Vec<TimerApplicationEffect>) {
        Self::with_timer_persistence_and_clock(
            timer_persistence,
            academic_persistence,
            Instant::now(),
            system_unix_millis(),
            *chrono::Local::now().offset(),
        )
    }

    fn with_timer_persistence_and_clock(
        timer_persistence: Box<dyn TimerPersistencePort>,
        academic_persistence: Box<dyn AcademicPersistencePort>,
        clock_origin: Instant,
        wall_origin_unix_millis: i64,
        local_offset: FixedOffset,
    ) -> (Self, Vec<TimerApplicationEffect>) {
        let modes = Self::default_modes();
        let default_selected_mode = 1;
        let loaded_snapshot = timer_persistence.load();
        let now = ClockObservation::new(0, wall_origin_unix_millis);

        let (timer, effects) = match loaded_snapshot {
            Some(snapshot) => {
                // Pick whichever preset tile the restored config actually matches, by label
                // first (exact preset match) and by mode second (same kind of timer, different
                // preset - e.g. a custom duration production doesn't have a native tile for
                // yet), falling back to the ordinary default so the UI never ends up with no
                // tile selected at all.
                let selected_mode = modes
                    .iter()
                    .position(|mode| mode.label() == snapshot.config.preset_label)
                    .or_else(|| modes.iter().position(|mode| mode.mode() == snapshot.mode))
                    .unwrap_or(default_selected_mode);
                AppTimer::restore(selected_mode, snapshot, timer_persistence, now)
            }
            None => (
                AppTimer::new_with_persistence(
                    default_selected_mode,
                    &modes[default_selected_mode],
                    timer_persistence,
                ),
                Vec::new(),
            ),
        };

        let mut academic = AcademicController::load_or_new(academic_persistence);
        academic.route_timer_effects(&effects, local_offset, now);

        (
            Self {
                title: crate::platform::identity::window_title(),
                status: "Stage 8: Slint adapter driving renderer-independent timer core"
                    .to_string(),
                timer,
                academic,
                local_offset,
                dashboard: DashboardSnapshot::build(
                    DashboardScenario::Typical,
                    DEFAULT_HISTORY_POINTS,
                ),
                dashboard_scenario: DashboardScenario::Typical,
                dashboard_points: DEFAULT_HISTORY_POINTS,
                plot_size: (600.0, 300.0),
                modes,
                clock_origin,
                wall_origin_unix_millis,
            },
            effects,
        )
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
            AppCommand::AddSemester {
                id,
                name,
                created_at,
            } => {
                let wall = self.clock(created_at).wall;
                self.academic.add_semester(Semester::new(id, name, wall));
            }
            AppCommand::AddCourse {
                id,
                semester_id,
                name,
                color,
                created_at,
            } => {
                let wall = self.clock(created_at).wall;
                self.academic
                    .add_course(Course::new(id, semester_id, name, color, wall));
            }
            AppCommand::RemoveCourse(id) => self.academic.remove_course(&id),
            AppCommand::RemoveSemester(id) => self.academic.remove_semester(&id),
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

    /// The most recent real, persisted study sessions (newest first - see
    /// `AcademicState::add_study_sessions`), mapped into the Slint-facing `SessionNote` shape.
    /// Replaces the Stage 4-15 hardcoded demo list: this is real domain data flowing all the way
    /// from a completed/recovered Timer session through `AcademicController`/`AcademicState`'s
    /// persistence to the same UI card that used to show three fixed placeholder rows (see
    /// `docs/stage16-academic-domain.md` section 22 for the before/after).
    pub fn session_notes(&self) -> Vec<SessionNote> {
        self.academic
            .state()
            .sessions
            .iter()
            .take(RECENT_SESSIONS_SHOWN)
            .map(SessionNote::from_study_session)
            .collect()
    }

    pub fn academic(&self) -> &AcademicController {
        &self.academic
    }

    /// Only used by the `STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA` diagnostic hook
    /// (`main.rs`) to bulk-load a synthetic performance-measurement dataset; ordinary CRUD goes
    /// through `AppCommand`, not this direct handle.
    pub fn academic_mut(&mut self) -> &mut AcademicController {
        &mut self.academic
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
        // The Timer -> StudySession bridge (Stage 16's "hard acceptance item"): every command
        // that might have produced a `SessionRangeReady` (completion, manual save, ...) is routed
        // through the same real, persisted path a startup recovery uses (see
        // `with_timer_persistence_and_clock`) - `route_timer_effects` itself is a no-op for any
        // effect that isn't a session range, so this is safe to call unconditionally after every
        // command rather than threading a "did this produce a session" check through here too.
        self.academic
            .route_timer_effects(&self.timer.pending_effects, self.local_offset, clock);
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
    #[allow(dead_code)] // only reachable via AppModel::stage_four_timer_preview (tests only)
    fn new(selected_mode: usize, mode: &TimerModeConfig) -> Self {
        Self::new_with_persistence(selected_mode, mode, Box::new(NullPersistencePort))
    }

    fn new_with_persistence(
        selected_mode: usize,
        mode: &TimerModeConfig,
        persistence: Box<dyn TimerPersistencePort>,
    ) -> Self {
        Self {
            controller: TimerController::new(selected_mode, mode.core_config(), persistence),
            pending_effects: Vec::new(),
        }
    }

    /// Builds an `AppTimer` from a previously persisted snapshot (Stage 15's real startup path).
    /// Immediately writes the resulting (recovered-and-reset, or merely time-adjusted) state back
    /// through the port - see `TimerController::force_persist`'s doc comment for exactly why: it
    /// closes the window where a crash right after this restart could otherwise see the same
    /// stale snapshot again on the *next* restart and recover the same abandoned session twice.
    fn restore(
        selected_mode: usize,
        snapshot: TimerSnapshot,
        persistence: Box<dyn TimerPersistencePort>,
        now: ClockObservation,
    ) -> (Self, Vec<TimerApplicationEffect>) {
        let (mut controller, effects) = TimerController::restore(
            selected_mode,
            RestoreInput {
                snapshot,
                existing_recovered_keys: Vec::new(),
                now,
            },
            persistence,
        );
        controller.force_persist(now);
        (
            Self {
                controller,
                pending_effects: effects.clone(),
            },
            effects,
        )
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

    pub fn mode(&self) -> TimerMode {
        self.mode
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
    /// Maps a real, persisted [`StudySession`] into this card's display shape. Pure presentation
    /// mapping - no domain logic lives here (matching this crate's usual boundary: `AppModel`'s
    /// read-side methods only ever format already-computed domain data for Slint).
    fn from_study_session(session: &StudySession) -> Self {
        let title = if session.goal.trim().is_empty() {
            session.preset_label.clone()
        } else {
            session.goal.clone()
        };
        let kind_label = match session.kind {
            SessionKind::Exam => "Exam",
            SessionKind::Break => "Break",
            SessionKind::Study => "Study",
        };
        let detail = format!(
            "{kind_label} · {} min · confidence {}/5",
            session.minutes, session.confidence
        );
        Self {
            title,
            detail,
            minutes: session.minutes.min(u32::from(u16::MAX)) as u16,
            confidence: session.confidence.min(5),
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
    use crate::academic_controller::NullAcademicPersistencePort;
    use crate::dashboard::DashboardScenario;
    use crate::timer_controller::TimerPersistencePort;
    use chrono::FixedOffset;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};
    use study_tracker_core::timer::{
        ActiveSegment, TimerConfig, TimerContext, TimerMode, TimerPhase, TimerSnapshot,
        WallTimestamp,
    };

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

        // Stage 16: that effect must have become a real, persisted StudySession, visible through
        // the same session-notes card `main.rs` renders (see `session_notes()`'s own doc comment)
        // - not just an ephemeral effect value nothing ever consumes.
        assert_eq!(model.academic().state().sessions.len(), 1);
        assert_eq!(model.academic().state().lifetime_study_sessions, 1);
        assert_eq!(model.session_notes().len(), 1);

        // The next, unrelated command's effects must not still carry the old completion...
        model.apply(AppCommand::Reset);
        assert!(model.timer_effects().is_empty());
        // ...but the session itself, already durably recorded, must not disappear with it.
        assert_eq!(
            model.academic().state().sessions.len(),
            1,
            "a session, once created, is durable domain state - not tied to pending_effects' lifetime"
        );
    }

    // --- Stage 15: real-persistence wiring (TimerController's own restore/recovery correctness
    // is exercised exhaustively in timer_controller.rs; these tests cover the one thing that
    // lives only here - that AppModel::with_timer_persistence wires a loaded snapshot into the
    // right preset tile and force-persists exactly once after a restore). ------------------------

    struct FixedLoadPort {
        initial: Option<TimerSnapshot>,
        persisted: Rc<RefCell<Vec<TimerSnapshot>>>,
    }
    impl TimerPersistencePort for FixedLoadPort {
        fn persist(&mut self, snapshot: TimerSnapshot) {
            self.persisted.borrow_mut().push(snapshot);
        }
        fn load(&self) -> Option<TimerSnapshot> {
            self.initial.clone()
        }
    }

    fn wall(seconds: i64) -> WallTimestamp {
        WallTimestamp::from_unix_millis(seconds * 1000)
    }

    #[test]
    fn with_timer_persistence_starts_fresh_and_writes_nothing_when_the_port_has_no_snapshot() {
        let persisted = Rc::new(RefCell::new(Vec::new()));
        let port = FixedLoadPort {
            initial: None,
            persisted: persisted.clone(),
        };
        let (model, effects) = AppModel::with_timer_persistence_and_clock(
            Box::new(port),
            Box::new(NullAcademicPersistencePort),
            Instant::now(),
            1_000_000,
            FixedOffset::east_opt(0).unwrap(),
        );
        assert!(effects.is_empty());
        assert_eq!(model.timer().status(), TimerStatus::Ready);
        assert_eq!(
            model.timer().selected_mode(),
            1,
            "the ordinary default preset"
        );
        assert!(
            persisted.borrow().is_empty(),
            "a fresh start with nothing to restore must not write anything on its own"
        );
    }

    #[test]
    fn with_timer_persistence_restores_a_running_deep_work_session_into_its_own_preset_tile() {
        let persisted = Rc::new(RefCell::new(Vec::new()));
        let now_wall = 1_000_000i64;
        let snapshot = TimerSnapshot {
            phase: TimerPhase::Study,
            mode: TimerMode::Focus,
            remaining_seconds: 3000,
            logged_split_seconds: 0,
            active_segments: vec![ActiveSegment {
                started_at: wall(now_wall - 100),
                ended_at: None,
            }],
            running: true,
            config: TimerConfig {
                mode: TimerMode::Focus,
                study_seconds: 52 * 60,
                break_seconds: 17 * 60,
                exam_seconds: 90 * 60,
                preset_label: "Deep Work".to_string(),
            },
            context: TimerContext::default(),
            started_at: Some(wall(now_wall - 100)),
            ends_at: Some(wall(now_wall + 3000)),
            last_alive_at: Some(wall(now_wall - 1)),
        };
        let port = FixedLoadPort {
            initial: Some(snapshot),
            persisted: persisted.clone(),
        };

        let (model, effects) = AppModel::with_timer_persistence_and_clock(
            Box::new(port),
            Box::new(NullAcademicPersistencePort),
            Instant::now(),
            now_wall * 1000,
            FixedOffset::east_opt(0).unwrap(),
        );

        assert!(
            effects.is_empty(),
            "a still-running, non-expired restore has nothing to report"
        );
        assert_eq!(
            model.timer().selected_mode(),
            1,
            "restores into the Deep Work tile by matching its preset_label"
        );
        assert_eq!(model.timer().status(), TimerStatus::Running);
        assert_eq!(
            persisted.borrow().len(),
            1,
            "restore must force-persist exactly once, even when nothing else changed"
        );
    }

    #[test]
    fn with_timer_persistence_recovers_an_expired_session_exactly_once_and_saves_the_reset_state() {
        let persisted = Rc::new(RefCell::new(Vec::new()));
        let now_wall = 1_000_000i64;
        let snapshot = TimerSnapshot {
            phase: TimerPhase::Study,
            mode: TimerMode::Focus,
            remaining_seconds: 0,
            logged_split_seconds: 0,
            active_segments: vec![ActiveSegment {
                started_at: wall(now_wall - 2000),
                ended_at: None,
            }],
            running: true,
            config: TimerConfig::default(),
            context: TimerContext::default(),
            started_at: Some(wall(now_wall - 2000)),
            ends_at: Some(wall(now_wall - 10)), // already expired before "now"
            last_alive_at: Some(wall(now_wall - 2000)),
        };
        let port = FixedLoadPort {
            initial: Some(snapshot),
            persisted: persisted.clone(),
        };

        let (model, effects) = AppModel::with_timer_persistence_and_clock(
            Box::new(port),
            Box::new(NullAcademicPersistencePort),
            Instant::now(),
            now_wall * 1000,
            FixedOffset::east_opt(0).unwrap(),
        );

        assert_eq!(
            model.timer().status(),
            TimerStatus::Ready,
            "recovered back to idle"
        );
        let session_ranges = effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    crate::timer_controller::TimerApplicationEffect::SessionRangeReady { .. }
                )
            })
            .count();
        assert_eq!(
            session_ranges, 1,
            "recovery must report exactly one session range"
        );
        assert_eq!(
            persisted.borrow().len(),
            1,
            "the recovered-and-reset state must be force-persisted so a repeat restart cannot recover the same session again"
        );
        assert_eq!(
            persisted.borrow()[0].phase,
            TimerPhase::Idle,
            "what gets saved back is the reset idle state, not the stale expired one"
        );
        assert_eq!(
            model.academic().state().sessions.len(),
            1,
            "the recovered session range must have become a real, persisted StudySession"
        );
        assert_eq!(
            model.session_notes().len(),
            1,
            "and the UI's session list reflects it"
        );
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
