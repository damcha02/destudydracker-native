//! Central application identity (Stage 13). One place other Stage 13+ code reads name/id/version
//! from, instead of scattering string literals across `main.rs`/`app_model.rs`/the `.rc` file.

/// User-facing product name. Matches production's display name deliberately (both are
/// "Study Tracker"; the architecture freeze does not treat the native shell as a different
/// product). Anything that must stay visually distinguishable from the real, released app while
/// this is still a prototype/shell (window title, executable metadata) appends [`SHELL_LABEL`].
pub const DISPLAY_NAME: &str = "Study Tracker";

/// Appended wherever the native build must not be mistaken for the real production app — most
/// importantly the window title (both can be open at once during A/B measurement, as in the
/// Stage 12 production comparison) and the embedded executable metadata. Drop this once the
/// native app is production per the freeze's cutover criteria (§32), not before.
pub const SHELL_LABEL: &str = "Native Preview";

/// **Intentionally different from production's Tauri identifier**
/// (`com.damcha.studytracker`, see `desktop/src-tauri/tauri.conf.json`).
///
/// Reusing production's exact identifier here would make [`crate::platform::paths::AppPaths`]
/// resolve to the *same* per-OS app-data directory production uses — the directory Stage 12's
/// production comparison went to deliberate lengths to never write into
/// (`native-prototype/docs/stage12-windows-platform.md`, "Production data / profile
/// conditions"). Stage 13 writes only a log file, but persistence lands here too from Stage 15
/// onward, so the separation starts now rather than being retrofitted later.
///
/// This is a namespaced child of production's own reverse-DNS owner (`com.damcha`), not an
/// unrelated identifier, so it doesn't collide with anything else on a user's machine.
///
/// **Open question, not resolved here** (per the Stage 12.5 freeze's instruction to document
/// rather than silently choose when an identifier is unsuitable): whether the *eventual*
/// production-native app adopts `com.damcha.studytracker` itself (likely, so users keep their
/// data/updater history) or keeps a distinct identifier is a Stage 15 decision.
pub const APP_ID: &str = "com.damcha.studytracker.native-shell";

/// Owner/publisher namespace, taken from production's identifier's second segment
/// (`com.[damcha].studytracker`). Not itself used for path resolution; kept for anything that
/// wants a human-readable "by damcha" string (e.g. the `.rc` `CompanyName` field).
pub const ORGANIZATION: &str = "damcha";

/// The native shell's own Cargo version. Deliberately **not** kept in sync with production's
/// version track (`desktop/package.json` is at 0.1.58 in the repository, 0.1.65 as installed on
/// this machine) — they are different build artifacts on different release cadences until
/// cutover. Do not "fix" this to match production's number.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Window title: distinguishable from production's plain "Study Tracker" title (confirmed via
/// screenshot in Stage 12) so Alt-Tab/taskbar stays unambiguous now that both apps can share the
/// same icon (see `ui/main.slint`'s `icon:` binding).
pub fn window_title() -> String {
    format!("{DISPLAY_NAME} ({SHELL_LABEL})")
}
