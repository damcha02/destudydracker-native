#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppModel {
    title: String,
    status: String,
    demo_state: DemoState,
    progress_examples: Vec<ProgressExample>,
    scroll_rows: Vec<DemoRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoState {
    selected_mode: DemoMode,
    progress_percent: u8,
    controls_enabled: bool,
    activation_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DemoMode {
    Focus,
    Review,
    Rest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressExample {
    label: String,
    progress_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoRow {
    title: String,
    detail: String,
    progress_percent: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    MarkPresentationReady,
    SetProgress(u16),
    IncreaseProgress(u16),
    ToggleEnabled,
    SelectMode(DemoMode),
    RecordActivation,
}

impl AppModel {
    pub fn stage_three_widget_preview() -> Self {
        Self {
            title: "Study Tracker Native Prototype".to_string(),
            status: "Stage 3: widgets and interaction system".to_string(),
            demo_state: DemoState {
                selected_mode: DemoMode::Focus,
                progress_percent: 65,
                controls_enabled: true,
                activation_count: 0,
            },
            progress_examples: vec![
                ProgressExample::new("Warmup", 25),
                ProgressExample::new("Deep work", 65),
                ProgressExample::new("Almost done", 90),
            ],
            scroll_rows: (1..=14)
                .map(|index| {
                    DemoRow::new(
                        format!("Demo card {index:02}"),
                        "Scrollable bounded content row for clipping, wheel/touchpad, and resize checks.",
                        ((index * 7 + 18) % 100) as u16,
                    )
                })
                .collect(),
        }
    }

    pub fn apply(&mut self, command: AppCommand) {
        match command {
            AppCommand::MarkPresentationReady => {
                self.status = "Stage 3 model state loaded through Rust adapter".to_string();
            }
            AppCommand::SetProgress(progress) => {
                if self.demo_state.controls_enabled {
                    self.demo_state.progress_percent = progress.min(100) as u8;
                }
            }
            AppCommand::IncreaseProgress(amount) => {
                if self.demo_state.controls_enabled {
                    self.demo_state.progress_percent = self
                        .demo_state
                        .progress_percent
                        .saturating_add(amount.min(100) as u8)
                        .min(100);
                }
            }
            AppCommand::ToggleEnabled => {
                self.demo_state.controls_enabled = !self.demo_state.controls_enabled;
            }
            AppCommand::SelectMode(mode) => {
                if self.demo_state.controls_enabled {
                    self.demo_state.selected_mode = mode;
                }
            }
            AppCommand::RecordActivation => {
                if self.demo_state.controls_enabled {
                    self.demo_state.activation_count =
                        self.demo_state.activation_count.saturating_add(1);
                }
            }
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn demo_state(&self) -> &DemoState {
        &self.demo_state
    }

    pub fn progress_examples(&self) -> &[ProgressExample] {
        &self.progress_examples
    }

    pub fn scroll_rows(&self) -> &[DemoRow] {
        &self.scroll_rows
    }
}

impl DemoState {
    pub fn selected_mode(&self) -> DemoMode {
        self.selected_mode
    }

    pub fn progress_percent(&self) -> u8 {
        self.progress_percent
    }

    pub fn progress_fraction(&self) -> f32 {
        f32::from(self.progress_percent) / 100.0
    }

    pub fn controls_enabled(&self) -> bool {
        self.controls_enabled
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }
}

impl DemoMode {
    pub fn as_str(self) -> &'static str {
        match self {
            DemoMode::Focus => "Focus",
            DemoMode::Review => "Review",
            DemoMode::Rest => "Rest",
        }
    }
}

impl ProgressExample {
    pub fn new(label: impl Into<String>, progress_percent: u16) -> Self {
        Self {
            label: label.into(),
            progress_percent: progress_percent.min(100) as u8,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn progress_percent(&self) -> u8 {
        self.progress_percent
    }

    pub fn progress_fraction(&self) -> f32 {
        f32::from(self.progress_percent) / 100.0
    }
}

impl DemoRow {
    pub fn new(title: impl Into<String>, detail: impl Into<String>, progress_percent: u16) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            progress_percent: progress_percent.min(100) as u8,
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn progress_percent(&self) -> u8 {
        self.progress_percent
    }

    pub fn progress_fraction(&self) -> f32 {
        f32::from(self.progress_percent) / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, AppModel, DemoMode, DemoRow, ProgressExample};

    #[test]
    fn command_updates_status_without_ui_dependencies() {
        let mut model = AppModel::stage_three_widget_preview();

        assert_eq!(model.status(), "Stage 3: widgets and interaction system");

        model.apply(AppCommand::MarkPresentationReady);

        assert_eq!(
            model.status(),
            "Stage 3 model state loaded through Rust adapter"
        );
    }

    #[test]
    fn valid_state_transitions_update_demo_state() {
        let mut model = AppModel::stage_three_widget_preview();

        model.apply(AppCommand::SelectMode(DemoMode::Review));
        model.apply(AppCommand::SetProgress(25));
        model.apply(AppCommand::RecordActivation);

        assert_eq!(model.demo_state().selected_mode(), DemoMode::Review);
        assert_eq!(model.demo_state().progress_percent(), 25);
        assert_eq!(model.demo_state().activation_count(), 1);
    }

    #[test]
    fn progress_values_are_clamped() {
        let mut model = AppModel::stage_three_widget_preview();
        let example = ProgressExample::new("overflow", 250);
        let row = DemoRow::new("row", "detail", 250);

        model.apply(AppCommand::SetProgress(250));
        model.apply(AppCommand::IncreaseProgress(250));

        assert_eq!(model.demo_state().progress_percent(), 100);
        assert_eq!(example.progress_percent(), 100);
        assert_eq!(row.progress_percent(), 100);
    }

    #[test]
    fn disabled_state_blocks_activation_count() {
        let mut model = AppModel::stage_three_widget_preview();

        model.apply(AppCommand::ToggleEnabled);
        model.apply(AppCommand::RecordActivation);

        assert!(!model.demo_state().controls_enabled());
        assert_eq!(model.demo_state().activation_count(), 0);
    }

    #[test]
    fn disabled_state_blocks_governed_commands() {
        let mut model = AppModel::stage_three_widget_preview();

        model.apply(AppCommand::ToggleEnabled);
        model.apply(AppCommand::SelectMode(DemoMode::Review));
        model.apply(AppCommand::SetProgress(25));
        model.apply(AppCommand::IncreaseProgress(10));

        assert_eq!(model.demo_state().selected_mode(), DemoMode::Focus);
        assert_eq!(model.demo_state().progress_percent(), 65);
    }
}
