//! Per-user application directories (Stage 13). Resolves real per-OS known folders through the
//! `dirs` crate — chosen because it is small, has no async runtime or heavyweight transitive
//! dependencies, and is a widely used, actively maintained way to ask the OS for these locations
//! rather than hand-rolling `%LOCALAPPDATA%`/`XDG_*`/`~/Library` lookups per platform.
//!
//! This module only *resolves and creates directories*. It does not read or write any
//! application/user data yet — that is Stage 15 (persistence). The only file this stage actually
//! writes here is the log file (`platform::logging`).

use crate::platform::error::StartupError;
use crate::platform::identity::APP_ID;
use std::fs;
use std::path::PathBuf;

/// Resolved application directories for this run. All three live under the OS's per-user
/// local/roaming-app-data root, namespaced by [`APP_ID`] — never beside the executable, and
/// never a literal `C:\Users\...` path baked into source (Requirement §7 of the Stage 13 brief).
pub struct AppPaths {
    /// Durable application data (unused until Stage 15; created now so the directory exists and
    /// is a known, inspectable location from the first shell build onward).
    pub data_dir: PathBuf,
    /// Non-durable cache data (unused by Stage 13; reserved for future use, e.g. a font/asset
    /// cache). On Windows `dirs::cache_dir()` and `dirs::data_local_dir()` resolve to the same
    /// `%LOCALAPPDATA%` root (Windows has no separate cache-folder convention), so this is a
    /// dedicated subdirectory of it, not a literal alias.
    pub cache_dir: PathBuf,
    /// Where `platform::logging` writes `study-tracker.log` (and its one rotated backup).
    pub log_dir: PathBuf,
}

impl AppPaths {
    /// Resolves the three directories without creating them. Fails only if the OS cannot report
    /// a local-app-data-equivalent folder at all (practically: a badly broken user profile).
    ///
    /// `STUDY_NATIVE_DATA_DIR`, if set, overrides `data_dir` (and, beneath it, `log_dir`)
    /// directly instead of resolving the real per-OS known folder. This exists purely as test/
    /// benchmark infrastructure (Stage 15's real-restart and kill-process persistence checks, and
    /// any future automated fixture-based test that must never touch a real user-data directory
    /// - see docs/stage15-persistence-migration.md, "Fixtures"). It has no effect unless
    /// explicitly set; normal launches are unaffected and still resolve the real OS location.
    pub fn resolve() -> Result<Self, StartupError> {
        if let Some(override_dir) = std::env::var_os("STUDY_NATIVE_DATA_DIR") {
            let data_dir = PathBuf::from(override_dir);
            return Ok(Self {
                cache_dir: data_dir.join("cache"),
                log_dir: data_dir.join("logs"),
                data_dir,
            });
        }
        let data_root = dirs::data_local_dir().ok_or(StartupError::NoKnownFolder)?;
        let cache_root = dirs::cache_dir().unwrap_or_else(|| data_root.clone());
        let data_dir = data_root.join(APP_ID);
        Ok(Self {
            cache_dir: cache_root.join(APP_ID).join("cache"),
            log_dir: data_dir.join("logs"),
            data_dir,
        })
    }

    /// Creates all three directories (idempotent). Call once, early in startup, before logging.
    pub fn ensure_created(&self) -> Result<(), StartupError> {
        for (what, dir) in [
            ("application data directory", &self.data_dir),
            ("cache directory", &self.cache_dir),
            ("log directory", &self.log_dir),
        ] {
            fs::create_dir_all(dir).map_err(|source| StartupError::Io { what, source })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests deliberately assert relationships between the resolved paths, never an
    // absolute path or username, so they pass on any machine/CI user account.

    #[test]
    fn log_dir_is_nested_under_data_dir() {
        let paths = AppPaths::resolve().expect("this test environment has a known-folder root");
        assert!(paths.log_dir.starts_with(&paths.data_dir));
        assert_eq!(paths.log_dir.file_name().unwrap(), "logs");
    }

    #[test]
    fn every_directory_is_namespaced_by_app_id() {
        let paths = AppPaths::resolve().expect("this test environment has a known-folder root");
        for dir in [&paths.data_dir, &paths.cache_dir, &paths.log_dir] {
            assert!(
                dir.components().any(|c| c.as_os_str() == APP_ID),
                "{dir:?} does not contain the app-id namespace component {APP_ID:?}"
            );
        }
    }

    #[test]
    fn app_id_does_not_collide_with_production() {
        // The whole point of a distinct identifier (see identity.rs) is that this never equals
        // production's own Tauri identifier.
        assert_ne!(APP_ID, "com.damcha.studytracker");
    }
}
