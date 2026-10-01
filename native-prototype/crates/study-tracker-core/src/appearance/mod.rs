//! App style / colour palette / light-dark theme (Stage 19): the user's appearance preference and
//! the pure rules that turn it into what is actually rendered.
//!
//! Production (`desktop/src/App.tsx`, `index.css`) has three **independent** persisted values:
//!
//! | localStorage key        | values                                          | default          |
//! |-------------------------|-------------------------------------------------|------------------|
//! | `study-tracker-style`   | `modern`, `field-notebook`, `wabi-sabi`         | `field-notebook` |
//! | `study-tracker-palette` | 16 palettes (`default` ... `sakura`), 3 aliases | `default`        |
//! | `study-tracker-theme`   | `dark` / `light` (any other string acts light)  | `dark`           |
//!
//! and one derived lock: Wabi-Sabi + Sakura is light-only ("Sakura is light-only in Wabi-Sabi").
//! Nothing here renders anything; the app layer maps a [`ResolvedAppearance`] to colour tokens.

pub mod sakura;

use serde::{Deserialize, Serialize};

/// `AppStyle` (`App.tsx`). Modern is a real production style that this native build cannot draw
/// yet (its dashboards host the Knowledge Garden; deferred), so it is kept as a value - a restored
/// or imported "modern" preference survives - but renders as the default style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AppStyle {
    Modern,
    FieldNotebook,
    WabiSabi,
}

impl AppStyle {
    /// `loadAppStyle`: anything unrecognized falls back to Field Notebook.
    pub const DEFAULT: AppStyle = AppStyle::FieldNotebook;

    pub fn from_production(value: &str) -> Option<Self> {
        match value {
            "modern" => Some(AppStyle::Modern),
            "field-notebook" => Some(AppStyle::FieldNotebook),
            "wabi-sabi" => Some(AppStyle::WabiSabi),
            _ => None,
        }
    }

    pub fn production_id(self) -> &'static str {
        match self {
            AppStyle::Modern => "modern",
            AppStyle::FieldNotebook => "field-notebook",
            AppStyle::WabiSabi => "wabi-sabi",
        }
    }

    /// Whether this build can draw the style's surfaces.
    pub fn is_supported_natively(self) -> bool {
        !matches!(self, AppStyle::Modern)
    }
}

/// `ThemePalette` (`App.tsx`), in production's own picker order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Palette {
    Default,
    Original,
    Midnight,
    Paper,
    Cyberpunk,
    Retrowave,
    Forest,
    Ocean,
    Ume,
    Copper,
    Organs,
    Lavender,
    Gpt,
    Claude,
    Cute,
    Sakura,
}

impl Palette {
    pub const ALL: [Palette; 16] = [
        Palette::Default,
        Palette::Original,
        Palette::Midnight,
        Palette::Paper,
        Palette::Cyberpunk,
        Palette::Retrowave,
        Palette::Forest,
        Palette::Ocean,
        Palette::Ume,
        Palette::Copper,
        Palette::Organs,
        Palette::Lavender,
        Palette::Gpt,
        Palette::Claude,
        Palette::Cute,
        Palette::Sakura,
    ];

    pub fn production_id(self) -> &'static str {
        match self {
            Palette::Default => "default",
            Palette::Original => "original",
            Palette::Midnight => "midnight",
            Palette::Paper => "paper",
            Palette::Cyberpunk => "cyberpunk",
            Palette::Retrowave => "retrowave",
            Palette::Forest => "forest",
            Palette::Ocean => "ocean",
            Palette::Ume => "ume",
            Palette::Copper => "copper",
            Palette::Organs => "organs",
            Palette::Lavender => "lavender",
            Palette::Gpt => "gpt",
            Palette::Claude => "claude",
            Palette::Cute => "cute",
            Palette::Sakura => "sakura",
        }
    }

    /// `loadThemePalette`: exact ids, then the three renamed legacy ids, else `None` (the caller
    /// falls back to `Default`, like production).
    pub fn from_production(value: &str) -> Option<Self> {
        if let Some(found) = Self::ALL.iter().find(|p| p.production_id() == value) {
            return Some(*found);
        }
        match value {
            "parchment" => Some(Palette::Paper),
            "cosmic" => Some(Palette::Retrowave),
            "grove" => Some(Palette::Forest),
            _ => None,
        }
    }

    /// The palettes production's theme picker offers for a style: Field Notebook and Wabi-Sabi get
    /// exactly Default + Sakura, Modern gets every palette except Sakura.
    pub fn offered_for(style: AppStyle) -> Vec<Palette> {
        match style {
            AppStyle::FieldNotebook | AppStyle::WabiSabi => vec![Palette::Default, Palette::Sakura],
            AppStyle::Modern => Self::ALL
                .iter()
                .copied()
                .filter(|p| *p != Palette::Sakura)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    /// `localStorage.getItem("study-tracker-theme") || "dark"` becomes `data-theme` verbatim. The
    /// style sheets only define explicit `dark` overrides on top of light base tokens for Field
    /// Notebook and Wabi-Sabi, and the toggle tests `=== "dark"`, so any other non-empty string
    /// behaves as light.
    pub fn from_production(value: Option<&str>) -> Self {
        match value {
            None | Some("") | Some("dark") => ThemeMode::Dark,
            Some(_) => ThemeMode::Light,
        }
    }

    pub fn production_id(self) -> &'static str {
        match self {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
        }
    }

    pub fn toggled(self) -> Self {
        match self {
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::Dark,
        }
    }
}

/// The persisted appearance preference: three independent values, exactly like production.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppearancePrefs {
    pub style: AppStyle,
    pub palette: Palette,
    pub theme: ThemeMode,
}

impl Default for AppearancePrefs {
    fn default() -> Self {
        Self {
            style: AppStyle::DEFAULT,
            palette: Palette::Default,
            theme: ThemeMode::Dark,
        }
    }
}

impl AppearancePrefs {
    /// Production's three `load*` functions over raw stored strings (`None` = key absent).
    pub fn from_production(
        style: Option<&str>,
        palette: Option<&str>,
        theme: Option<&str>,
    ) -> Self {
        Self {
            style: style
                .and_then(AppStyle::from_production)
                .unwrap_or(AppStyle::DEFAULT),
            palette: palette
                .and_then(Palette::from_production)
                .unwrap_or(Palette::Default),
            theme: ThemeMode::from_production(theme),
        }
    }

    pub fn resolve(self) -> ResolvedAppearance {
        ResolvedAppearance::from_prefs(self)
    }
}

/// Which surface set is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderedStyle {
    FieldNotebook,
    WabiSabi,
}

/// Which token table the surfaces use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    FieldNotebookDark,
    FieldNotebookLight,
    /// `:root[data-app-style="field-notebook"][data-palette="sakura"]` - the same pink tokens
    /// whether `data-theme` is dark or light.
    FieldNotebookSakura,
    WabiLight,
    WabiDark,
}

/// What a preference actually looks like on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedAppearance {
    pub rendered: RenderedStyle,
    pub scheme: ColorScheme,
    /// The falling petals + animated petal texture (`SakuraScatter`, `body::after`): only for the
    /// Sakura palette on Field Notebook or Wabi-Sabi (the two styles whose picker offers it).
    pub sakura: bool,
    /// Wabi-Sabi + Sakura: the light/dark toggle is disabled ("Sakura is light-only").
    pub theme_locked: bool,
    /// The effective `data-theme` (what the sun/moon toggle icon shows).
    pub dark: bool,
}

impl ResolvedAppearance {
    pub fn from_prefs(prefs: AppearancePrefs) -> Self {
        let sakura_style = matches!(prefs.style, AppStyle::FieldNotebook | AppStyle::WabiSabi);
        let sakura = sakura_style && prefs.palette == Palette::Sakura;
        let theme_locked = prefs.style == AppStyle::WabiSabi && prefs.palette == Palette::Sakura;
        let dark = !theme_locked && prefs.theme == ThemeMode::Dark;
        let rendered = match prefs.style {
            AppStyle::WabiSabi => RenderedStyle::WabiSabi,
            AppStyle::FieldNotebook | AppStyle::Modern => RenderedStyle::FieldNotebook,
        };
        let scheme = match rendered {
            RenderedStyle::WabiSabi if dark => ColorScheme::WabiDark,
            RenderedStyle::WabiSabi => ColorScheme::WabiLight,
            RenderedStyle::FieldNotebook if sakura => ColorScheme::FieldNotebookSakura,
            RenderedStyle::FieldNotebook if dark => ColorScheme::FieldNotebookDark,
            RenderedStyle::FieldNotebook => ColorScheme::FieldNotebookLight,
        };
        Self {
            rendered,
            scheme,
            sakura,
            theme_locked,
            dark,
        }
    }
}

#[cfg(test)]
mod tests;
