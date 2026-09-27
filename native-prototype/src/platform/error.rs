//! One small error type for everything that can fail during startup, before the Slint event loop
//! is running (Stage 13). Kept boring on purpose: no error-handling crate, just `std::fmt` and a
//! handful of `From` impls, per the freeze's "no dependency without a concrete need" rule.

use std::fmt;

#[derive(Debug)]
pub enum StartupError {
    /// The OS did not report a usable data/cache directory (see `platform::paths`).
    NoKnownFolder,
    /// Could not create or write to an application directory.
    Io {
        what: &'static str,
        source: std::io::Error,
    },
    /// The logging backend could not be installed (e.g. installed twice — should not happen in
    /// normal operation, kept as a distinct variant so it's diagnosable if it ever does).
    Logging(log::SetLoggerError),
    /// Slint/Winit/the renderer backend failed to initialize or the window failed to run.
    Platform(slint::PlatformError),
}

impl fmt::Display for StartupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StartupError::NoKnownFolder => write!(
                f,
                "could not resolve a per-user application data directory on this system"
            ),
            StartupError::Io { what, source } => write!(f, "{what}: {source}"),
            StartupError::Logging(source) => write!(f, "could not start logging: {source}"),
            StartupError::Platform(source) => {
                write!(f, "could not start the application window: {source}")
            }
        }
    }
}

impl std::error::Error for StartupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StartupError::NoKnownFolder => None,
            StartupError::Io { source, .. } => Some(source),
            StartupError::Logging(source) => Some(source),
            StartupError::Platform(source) => Some(source),
        }
    }
}

impl From<log::SetLoggerError> for StartupError {
    fn from(source: log::SetLoggerError) -> Self {
        StartupError::Logging(source)
    }
}

impl From<slint::PlatformError> for StartupError {
    fn from(source: slint::PlatformError) -> Self {
        StartupError::Platform(source)
    }
}
