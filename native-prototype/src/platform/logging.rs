//! Minimal file logging (Stage 13). Deliberately small: `log`'s facade plus a hand-written
//! `Log` impl that appends timestamped lines to one file, rather than pulling in an
//! observability framework. This is the one place startup/backend/fatal-error diagnostics go
//! once the release build has no console (see the `windows_subsystem` attribute in `main.rs`).
//!
//! Rules this module exists to enforce (Stage 13 brief §8):
//! - no high-frequency logging: callers must not log per-frame or per-timer-tick (nothing in
//!   this codebase does; `install_diagnostics`'s existing `STUDY_NATIVE_FRAME_STATS`/
//!   `STUDY_NATIVE_STARTUP_REPORT` hooks print to stdout directly and are unrelated to this
//!   logger — see the diagnostics-hook classification in `docs/stage13-production-shell.md`);
//! - no personal/sensitive data — only version/backend/renderer/lifecycle facts are logged
//!   anywhere in this codebase today;
//! - bounded growth: a simple one-generation rotation (`study-tracker.log` /
//!   `study-tracker.log.old`) runs at startup, not a full rotation subsystem.

use crate::platform::error::StartupError;
use crate::platform::paths::AppPaths;
use chrono::Local;
use log::{LevelFilter, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

/// Above this size, the previous log is dropped and the current one becomes `.old` before a
/// fresh file is opened. 2 MB comfortably covers many ordinary sessions' worth of startup/
/// lifecycle lines at this logger's volume (a handful of lines per run, not per frame).
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

const LOG_FILE_NAME: &str = "study-tracker.log";
const LOG_FILE_NAME_OLD: &str = "study-tracker.log.old";

struct FileLogger {
    file: Mutex<File>,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= LevelFilter::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!(
            "{} [{:5}] {}: {}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            record.level(),
            record.target(),
            record.args()
        );
        if let Ok(mut file) = self.file.lock() {
            let _ = file.write_all(line.as_bytes());
        }
        // Debug builds still have a console (see `windows_subsystem` in main.rs), so mirror
        // warnings/errors there too; release builds skip this (nothing would be attached to it).
        #[cfg(debug_assertions)]
        if record.level() <= log::Level::Warn {
            eprint!("{line}");
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// Rotates the log file if it has grown past [`MAX_LOG_BYTES`]. Best-effort: a failure here is
/// not fatal to startup, it just means the old file grows a bit further this run.
fn rotate_if_large(paths: &AppPaths) {
    let path = paths.log_dir.join(LOG_FILE_NAME);
    let Ok(metadata) = fs::metadata(&path) else {
        return; // no existing file yet — nothing to rotate
    };
    if metadata.len() < MAX_LOG_BYTES {
        return;
    }
    let _ = fs::rename(&path, paths.log_dir.join(LOG_FILE_NAME_OLD));
}

/// Installs the process-wide logger. Call once, as early as possible in `main`, after
/// `AppPaths::ensure_created`. Returns the resolved log file path for a startup log line.
pub fn init(paths: &AppPaths) -> Result<std::path::PathBuf, StartupError> {
    rotate_if_large(paths);
    let log_path = paths.log_dir.join(LOG_FILE_NAME);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|source| StartupError::Io {
            what: "log file",
            source,
        })?;
    log::set_boxed_logger(Box::new(FileLogger {
        file: Mutex::new(file),
    }))?;
    log::set_max_level(LevelFilter::Info);
    Ok(log_path)
}

/// Installs a panic hook that logs the panic (message + location) before the default hook runs,
/// so a panic during/after startup leaves a trace in the log file even with no console attached.
/// Deliberately not a crash reporter: no telemetry, no dialog beyond what `startup_error` already
/// provides for the specific startup-failure path.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!(target: "panic", "{info}");
        default_hook(info);
    }));
}
