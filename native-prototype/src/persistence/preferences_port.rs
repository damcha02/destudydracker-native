//! Appearance preference persistence (Stage 19): the style / palette / light-dark choice, stored in
//! the same native store file as a `preferences` section.
//!
//! ```json
//! "preferences": { "appearance": { "style": "wabi-sabi", "palette": "sakura", "theme": "light" } }
//! ```
//!
//! * **Additive, no schema bump.** The store's existing `other` map already round-trips unknown
//!   top-level sections, so a Stage 15-18 build reading a Stage 19 store keeps this section
//!   untouched, and a Stage 15-18 store (no section) loads as production's defaults.
//! * **Production's own ids** are stored (`"wabi-sabi"`, not a Rust enum name), and every field is
//!   parsed on its own with production's fallbacks (`loadAppStyle`/`loadThemePalette`/theme), so an
//!   unknown or future value degrades to the default instead of failing the load.
//! * Unknown keys inside the section are preserved on write.
//! * Written **only when the preference actually changes** ([`PreferencesController::set`]); never
//!   by rendering, animation or navigation.

use serde_json::{Map, Value};
use study_tracker_core::appearance::AppearancePrefs;

use crate::persistence::store::{NativeStore, StoreEnvelope};

pub const SECTION: &str = "preferences";
const APPEARANCE: &str = "appearance";

/// Reads the appearance from a `preferences` section value (tolerant per field).
pub fn parse_appearance(section: Option<&Value>) -> AppearancePrefs {
    let appearance = section
        .and_then(|s| s.get(APPEARANCE))
        .and_then(Value::as_object);
    let field = |key: &str| appearance.and_then(|a| a.get(key)).and_then(Value::as_str);
    AppearancePrefs::from_production(field("style"), field("palette"), field("theme"))
}

/// Writes the appearance into a (possibly existing) section, keeping any other keys in it.
pub fn write_appearance(section: Option<Value>, prefs: AppearancePrefs) -> Value {
    let mut map = match section {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    let mut appearance = Map::new();
    appearance.insert("style".into(), Value::from(prefs.style.production_id()));
    appearance.insert("palette".into(), Value::from(prefs.palette.production_id()));
    appearance.insert("theme".into(), Value::from(prefs.theme.production_id()));
    map.insert(APPEARANCE.into(), Value::Object(appearance));
    Value::Object(map)
}

/// Production's `restoreBackup` for the three appearance keys of a backup's `preferences` map:
/// each key that holds a string replaces that value, anything else leaves the current one. `None`
/// when the backup carries none of them (nothing to import).
pub fn merge_production_backup_preferences(
    current: AppearancePrefs,
    preferences: &Map<String, Value>,
) -> Option<AppearancePrefs> {
    let get = |key: &str| preferences.get(key).and_then(Value::as_str);
    let (style, palette, theme) = (
        get("study-tracker-style"),
        get("study-tracker-palette"),
        get("study-tracker-theme"),
    );
    if style.is_none() && palette.is_none() && theme.is_none() {
        return None;
    }
    // A present key goes through production's own loader for that key (unknown -> default).
    let parsed = AppearancePrefs::from_production(style, palette, theme);
    Some(AppearancePrefs {
        style: if style.is_some() {
            parsed.style
        } else {
            current.style
        },
        palette: if palette.is_some() {
            parsed.palette
        } else {
            current.palette
        },
        theme: if theme.is_some() {
            parsed.theme
        } else {
            current.theme
        },
    })
}

pub trait PreferencesPort {
    fn load(&self) -> AppearancePrefs;
    fn persist(&mut self, prefs: AppearancePrefs);
}

/// For tests and scratch callers: remembers nothing.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct NullPreferencesPort;

#[cfg(test)]
impl PreferencesPort for NullPreferencesPort {
    fn load(&self) -> AppearancePrefs {
        AppearancePrefs::default()
    }
    fn persist(&mut self, _prefs: AppearancePrefs) {}
}

pub struct FilePreferencesPort {
    store: NativeStore,
}

impl FilePreferencesPort {
    pub fn new(store: NativeStore) -> Self {
        Self { store }
    }
}

impl PreferencesPort for FilePreferencesPort {
    fn load(&self) -> AppearancePrefs {
        match self.store.load() {
            Ok((envelope, _warnings)) => parse_appearance(envelope.other.get(SECTION)),
            Err(err) => {
                log::warn!("preferences: could not load store, using defaults: {err}");
                AppearancePrefs::default()
            }
        }
    }

    /// Read-modify-write like the timer/academic ports, so no other section is ever clobbered.
    fn persist(&mut self, prefs: AppearancePrefs) {
        let mut envelope = match self.store.load() {
            Ok((envelope, _warnings)) => envelope,
            Err(err) => {
                log::warn!(
                    "preferences: existing store unreadable, starting a fresh envelope: {err}"
                );
                StoreEnvelope::default()
            }
        };
        let section = envelope.other.remove(SECTION);
        envelope
            .other
            .insert(SECTION.into(), write_appearance(section, prefs));
        if let Err(err) = self.store.save(&envelope) {
            log::warn!("preferences: failed to save: {err}");
        }
    }
}

/// Owns the current preference and is the only writer of it.
pub struct PreferencesController {
    prefs: AppearancePrefs,
    port: Box<dyn PreferencesPort>,
    writes: u64,
}

impl PreferencesController {
    pub fn load(port: Box<dyn PreferencesPort>) -> Self {
        Self {
            prefs: port.load(),
            port,
            writes: 0,
        }
    }

    pub fn prefs(&self) -> AppearancePrefs {
        self.prefs
    }

    /// Store writes performed by this controller (diagnostics/tests).
    pub fn writes(&self) -> u64 {
        self.writes
    }

    /// Applies a new preference; persists only if it differs. Returns whether it changed.
    pub fn set(&mut self, prefs: AppearancePrefs) -> bool {
        if prefs == self.prefs {
            return false;
        }
        self.prefs = prefs;
        self.port.persist(prefs);
        self.writes += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::appearance::{AppStyle, Palette, ThemeMode};

    struct TempDirGuard(std::path::PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn temp_store() -> (TempDirGuard, NativeStore) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "study-tracker-prefs-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        (TempDirGuard(dir), NativeStore::new(path))
    }

    fn wabi_sakura() -> AppearancePrefs {
        AppearancePrefs {
            style: AppStyle::WabiSabi,
            palette: Palette::Sakura,
            theme: ThemeMode::Light,
        }
    }

    #[test]
    fn the_null_port_starts_from_production_defaults() {
        let mut c = PreferencesController::load(Box::new(NullPreferencesPort));
        assert_eq!(c.prefs(), AppearancePrefs::default());
        assert!(c.set(wabi_sakura()));
        assert_eq!(c.writes(), 1);
    }

    #[test]
    fn a_fresh_or_pre_stage_19_store_loads_production_defaults() {
        let (_dir, store) = temp_store();
        assert_eq!(
            FilePreferencesPort::new(store).load(),
            AppearancePrefs::default()
        );
        let (_dir, store) = temp_store();
        std::fs::write(store.path(), br#"{"schema_version": 1, "timer": null}"#).unwrap();
        assert_eq!(
            FilePreferencesPort::new(store).load(),
            AppearancePrefs::default()
        );
    }

    #[test]
    fn a_choice_survives_a_restart_and_is_written_with_production_ids() {
        let (_dir, store) = temp_store();
        let path = store.path().to_path_buf();
        let mut controller = PreferencesController::load(Box::new(FilePreferencesPort::new(store)));
        assert!(controller.set(wabi_sakura()));
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["preferences"]["appearance"]["style"], "wabi-sabi");
        assert_eq!(raw["preferences"]["appearance"]["palette"], "sakura");
        assert_eq!(raw["preferences"]["appearance"]["theme"], "light");
        let restarted =
            PreferencesController::load(Box::new(FilePreferencesPort::new(NativeStore::new(path))));
        assert_eq!(restarted.prefs(), wabi_sakura());
    }

    #[test]
    fn an_unchanged_preference_is_never_written() {
        let (_dir, store) = temp_store();
        let path = store.path().to_path_buf();
        let mut controller = PreferencesController::load(Box::new(FilePreferencesPort::new(store)));
        assert!(!controller.set(AppearancePrefs::default()));
        assert!(!path.exists(), "no write for a no-op change");
        assert!(controller.set(wabi_sakura()));
        assert!(!controller.set(wabi_sakura()));
        assert_eq!(controller.writes(), 1);
    }

    #[test]
    fn unknown_or_future_values_fall_back_per_field() {
        let section: Value = serde_json::json!({
            "appearance": { "style": "brutalist-2030", "palette": "sakura", "theme": 7 },
            "future": { "keep": true }
        });
        let prefs = parse_appearance(Some(&section));
        assert_eq!(prefs.style, AppStyle::FieldNotebook);
        assert_eq!(prefs.palette, Palette::Sakura);
        assert_eq!(prefs.theme, ThemeMode::Dark);
        let written = write_appearance(Some(section), wabi_sakura());
        assert_eq!(
            written["future"]["keep"], true,
            "unknown keys in the section survive"
        );
    }

    #[test]
    fn saving_preferences_keeps_the_timer_and_academic_sections() {
        let (_dir, store) = temp_store();
        let path = store.path().to_path_buf();
        std::fs::write(
            &path,
            br#"{"schema_version": 1, "academic_like": {"sessions": [1, 2]}, "something": 3}"#,
        )
        .unwrap();
        FilePreferencesPort::new(NativeStore::new(path.clone())).persist(wabi_sakura());
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["academic_like"]["sessions"][1], 2);
        assert_eq!(raw["something"], 3);
    }

    #[test]
    fn backup_preferences_merge_key_by_key_like_restore_backup() {
        let current = AppearancePrefs::default();
        let mut prefs = Map::new();
        assert_eq!(merge_production_backup_preferences(current, &prefs), None);
        prefs.insert("study-tracker-style".into(), Value::from("wabi-sabi"));
        prefs.insert("study-tracker-palette".into(), Value::from("grove"));
        prefs.insert(
            "study-tracker-garden-variant".into(),
            Value::from("japanese"),
        );
        let merged = merge_production_backup_preferences(current, &prefs).unwrap();
        assert_eq!(merged.style, AppStyle::WabiSabi);
        assert_eq!(
            merged.palette,
            Palette::Forest,
            "legacy alias, like loadThemePalette"
        );
        assert_eq!(
            merged.theme, current.theme,
            "absent key keeps the current value"
        );
        // non-string values are ignored exactly like `typeof value === "string"`
        let mut odd = Map::new();
        odd.insert("study-tracker-theme".into(), Value::from(1));
        assert_eq!(merge_production_backup_preferences(current, &odd), None);
    }
}
