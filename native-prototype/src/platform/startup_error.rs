//! Windows-only fatal-startup-error fallback (Stage 13). A release build has no console (see the
//! `windows_subsystem` attribute in `main.rs`), so a startup failure that only wrote to stderr
//! would be genuinely invisible to the user. This shows a native `MessageBoxW` as a last resort.
//!
//! Kept deliberately tiny: one FFI call, no crash-reporter, no telemetry, no window handle
//! dependency (fatal errors here happen *before* any Slint window exists). `windows-sys` is a
//! thin FFI-declarations crate (no codegen beyond what is referenced), gated to
//! `cfg(windows)`-only in `Cargo.toml`, so this never touches `study-tracker-core` or non-Windows
//! builds.

use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MB_TOPMOST};

/// Shows a blocking native message box with the given text. Best-effort: if the call itself
/// somehow fails there is nothing further we can do without a console, so the return value is
/// intentionally discarded by the caller.
pub fn show_fatal_error(title: &str, message: &str) {
    let title_wide = to_wide(title);
    let message_wide = to_wide(message);
    // SAFETY: both buffers are valid, NUL-terminated UTF-16 strings that outlive the call
    // (they're not dropped until after `MessageBoxW` returns); a null HWND is documented as
    // valid (the box simply has no owner window).
    let _ = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message_wide.as_ptr(),
            title_wide.as_ptr(),
            MB_OK | MB_ICONERROR | MB_TOPMOST,
        )
    };
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
