//! Runtime configuration boundary (Stage 13). Deliberately tiny: this is *application/runtime*
//! configuration, not user settings (that's Stage 15+) and not a generic configuration
//! framework — most constants in this codebase stay constants.

/// The renderer/backend a normal build actually has compiled in. `FemtoVg` is the only variant
/// today because it is the only renderer Cargo feature enabled in `Cargo.toml`
/// (`slint`'s `renderer-femtovg`), per the architecture freeze (ADR 0001, freeze §7): FemtoVG is
/// the default *and only* Windows renderer in the normal build. Skia/software comparisons stay
/// in the separate scratch-copy pattern used for the Stage 12 renderer comparison — bundling
/// every renderer into the normal binary was explicitly rejected there on memory/startup/size
/// grounds and that finding still holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererBackend {
    FemtoVg,
}

impl RendererBackend {
    pub fn compiled_default() -> Self {
        RendererBackend::FemtoVg
    }

    pub fn slint_backend_report(self) -> &'static str {
        match self {
            RendererBackend::FemtoVg => "FemtoVG renderer with OpenGL backend (Winit)",
        }
    }
}

/// Minimal runtime configuration, resolved once at startup and logged (never re-read per
/// frame/tick).
pub struct RuntimeConfig {
    pub renderer: RendererBackend,
}

impl RuntimeConfig {
    /// `SLINT_BACKEND`/`SLINT_RENDERER` (read directly by Slint's own backend selector, not by
    /// this struct) can still steer selection *among compiled-in backends* the way Stage 12's
    /// benchmark scripts already do; since only FemtoVG is compiled into a normal Stage 13
    /// build, there is nothing else to select, and this just logs the attempt so a developer who
    /// sets it and sees no effect isn't left guessing why.
    pub fn from_env() -> Self {
        if let Ok(requested) = std::env::var("SLINT_BACKEND") {
            log::info!(
                target: "config",
                "SLINT_BACKEND={requested} is set; only FemtoVG is compiled into this build (see Cargo.toml), so this has no effect unless the build is reconfigured"
            );
        }
        RuntimeConfig {
            renderer: RendererBackend::compiled_default(),
        }
    }
}

/// Every `STUDY_NATIVE_*`/`SLINT_*` environment variable Stages 9-12 already made meaningful,
/// kept supported (see `docs/stage13-production-shell.md`, "Diagnostic hook classification").
/// Logs which of them are actually set this run so a benchmark session is traceable from the log
/// file alone; changes nothing by itself.
pub fn log_active_benchmark_overrides() {
    const KNOWN_VARS: &[&str] = &[
        "STUDY_NATIVE_VIEW",
        "STUDY_NATIVE_POINTS",
        "STUDY_NATIVE_SCENARIO",
        "STUDY_NATIVE_SIZE",
        "STUDY_NATIVE_MAP_LEVEL",
        "STUDY_NATIVE_MAP_BENCH",
        "STUDY_NATIVE_STARTUP_REPORT",
        "STUDY_NATIVE_FRAME_STATS",
        "SLINT_SCALE_FACTOR",
        "SLINT_DEBUG_PERFORMANCE",
    ];
    let active: Vec<String> = KNOWN_VARS
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|v| format!("{name}={v}")))
        .collect();
    if !active.is_empty() {
        log::info!(target: "config", "benchmark/diagnostic overrides active: {}", active.join(", "));
    }
}
