#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppModel {
    title: String,
    status: String,
    text_samples: Vec<TextSample>,
    wrapping_text: String,
    single_line_input: String,
    multiline_input: String,
    notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSample {
    label: String,
    value: String,
    weight: FontWeight,
    size: TextSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    SemiBold,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSize {
    Body,
    Large,
    Display,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    MarkPresentationReady,
    UpdateSingleLine(String),
    UpdateMultiline(String),
}

impl AppModel {
    pub fn stage_two_text_input_preview() -> Self {
        Self {
            title: "Study Tracker Native Prototype".to_string(),
            status: "Stage 2: text, layout, and input feasibility".to_string(),
            text_samples: vec![
                TextSample::new(
                    "Latin / bold",
                    "Study Tracker - Focus Session",
                    FontWeight::Bold,
                    TextSize::Display,
                ),
                TextSample::new(
                    "European Unicode / normal",
                    "Größe · Prüfung · Zürich · naïve · café",
                    FontWeight::Normal,
                    TextSize::Body,
                ),
                TextSample::new(
                    "Japanese / semibold",
                    "日本語の勉強を始めましょう",
                    FontWeight::SemiBold,
                    TextSize::Large,
                ),
                TextSample::new(
                    "Mixed script + math / bold",
                    "Quantum Mechanics - 第4章 - σ² = 2.35 × 10⁻⁴",
                    FontWeight::Bold,
                    TextSize::Large,
                ),
                TextSample::new(
                    "Emoji fallback / normal",
                    "📚 ⏱️ ✅ 🧪 🚀",
                    FontWeight::Normal,
                    TextSize::Display,
                ),
            ],
            wrapping_text: "Wrapping test: Study Tracker needs long notes that mix English, 日本語, symbols, and math. This paragraph intentionally includes Größe, naïve café, 第4章, σ² = 2.35 × 10⁻⁴, and emoji 📚 ✅ 🚀 so resizing can reveal clipping, fallback, shaping, and line-break behavior without using browser layout.".to_string(),
            single_line_input: "Edit me: café 日本語 σ² 📚".to_string(),
            multiline_input: "Multiline edit test\n日本語の入力候補をここで確認\nResize the window and try selection, copy, paste, arrows, Home/End, and Ctrl+A.".to_string(),
            notes: vec![
                "Use mouse, Tab, and Shift+Tab to move focus between fields.".to_string(),
                "Clipboard and IME behavior are runtime/environment checks, not model logic.".to_string(),
                "No timer, persistence, network, tray, updater, or production feature code is present.".to_string(),
            ],
        }
    }

    pub fn apply(&mut self, command: AppCommand) {
        match command {
            AppCommand::MarkPresentationReady => {
                self.status = "Stage 2 model state loaded through Rust adapter".to_string();
            }
            AppCommand::UpdateSingleLine(text) => {
                self.single_line_input = text;
            }
            AppCommand::UpdateMultiline(text) => {
                self.multiline_input = text;
            }
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub fn text_samples(&self) -> &[TextSample] {
        &self.text_samples
    }

    pub fn wrapping_text(&self) -> &str {
        &self.wrapping_text
    }

    pub fn single_line_input(&self) -> &str {
        &self.single_line_input
    }

    pub fn multiline_input(&self) -> &str {
        &self.multiline_input
    }

    pub fn notes(&self) -> &[String] {
        &self.notes
    }
}

impl TextSample {
    pub fn new(
        label: impl Into<String>,
        value: impl Into<String>,
        weight: FontWeight,
        size: TextSize,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            weight,
            size,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn weight(&self) -> FontWeight {
        self.weight
    }

    pub fn size(&self) -> TextSize {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, AppModel, FontWeight, TextSize};

    #[test]
    fn command_updates_status_without_ui_dependencies() {
        let mut model = AppModel::stage_two_text_input_preview();

        assert_eq!(
            model.status(),
            "Stage 2: text, layout, and input feasibility"
        );

        model.apply(AppCommand::MarkPresentationReady);

        assert_eq!(
            model.status(),
            "Stage 2 model state loaded through Rust adapter"
        );
    }

    #[test]
    fn stores_unicode_samples_as_plain_rust_strings() {
        let model = AppModel::stage_two_text_input_preview();

        assert!(model
            .text_samples()
            .iter()
            .any(|sample| sample.value().contains("日本語")));
        assert!(model
            .text_samples()
            .iter()
            .any(|sample| sample.value().contains("📚")));
        assert!(model
            .text_samples()
            .iter()
            .any(|sample| sample.value().contains("σ²")));
        assert!(model
            .text_samples()
            .iter()
            .any(|sample| sample.weight() == FontWeight::SemiBold
                && sample.size() == TextSize::Large));
    }

    #[test]
    fn text_edit_commands_update_model_state() {
        let mut model = AppModel::stage_two_text_input_preview();

        model.apply(AppCommand::UpdateSingleLine(
            "typed 日本語 café".to_string(),
        ));
        model.apply(AppCommand::UpdateMultiline("line 1\nline 2 ✅".to_string()));

        assert_eq!(model.single_line_input(), "typed 日本語 café");
        assert_eq!(model.multiline_input(), "line 1\nline 2 ✅");
    }
}
