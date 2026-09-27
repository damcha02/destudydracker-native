//! Native application shell: identity, platform paths, logging, runtime configuration, and
//! fatal-startup-error reporting (Stage 13, "production shell foundation").
//!
//! Everything here is Slint/Winit-facing application-adapter code, one layer *above*
//! `study-tracker-core` in the frozen dependency direction (see
//! `docs/stage12_5-architecture-freeze.md`, sections 3 and 5). `study-tracker-core` must never
//! import from this module or from any of its dependencies (`dirs`, `log`, `chrono`,
//! `windows-sys`).
//!
//! Platform-specific pieces are gated with `#[cfg(windows)]`/`#[cfg(not(windows))]` at the
//! smallest possible granularity so Linux/macOS keep building (Stage 13 does not add CI or
//! runtime-verify either, per the freeze's cross-platform discipline).

pub mod config;
pub mod error;
pub mod identity;
pub mod logging;
pub mod paths;

#[cfg(windows)]
pub mod startup_error;
