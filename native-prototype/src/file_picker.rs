//! "Choose an image" (Stage 22b): production's `<input type="file" accept="image/...">` for the
//! feed composer, the post editor and the avatar photo, as the platform's own open dialog.
//!
//! No toolkit and no new crate:
//! - **Linux:** the XDG desktop portal (`org.freedesktop.portal.FileChooser.OpenFile`) over the
//!   `zbus` that accesskit already links (its blocking API is a feature flag, not a crate). The
//!   portal shows the desktop's own dialog (GTK/KDE/...); the call waits on a short-lived thread
//!   so the UI keeps rendering, and only one dialog can be open. Without a portal, `zenity` or
//!   `kdialog` are tried; without those, the action reports that no dialog is available.
//! - **Windows:** the shell's `IFileOpenDialog` (COM, the `windows` crate already linked), modal
//!   to the app window on the UI thread, as every Windows app does.
//! - **macOS:** Stage 24 (not qualified yet): reports that no dialog is available.
//! - **Tests/automation:** `STUDY_NATIVE_PICK_FILE=<path>` answers without any dialog.
//!
//! The chosen path is handed to the caller only; it is never logged.

use std::cell::Cell;
use std::path::PathBuf;

thread_local! {
    static OPEN: Cell<bool> = const { Cell::new(false) };
}

/// Image types production's inputs accept (`image/png,image/jpeg,image/webp,image/gif`).
pub const IMAGE_PATTERNS: [&str; 5] = ["*.png", "*.jpg", "*.jpeg", "*.webp", "*.gif"];

/// Shows the dialog; `done(None)` when cancelled or unavailable. At most one at a time.
pub fn pick_image(title: &'static str, done: impl FnOnce(Option<PathBuf>) + 'static) {
    if OPEN.with(Cell::get) {
        return;
    }
    if let Ok(p) = std::env::var("STUDY_NATIVE_PICK_FILE") {
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            done(Some(PathBuf::from(p)))
        });
        return;
    }
    OPEN.with(|o| o.set(true));
    platform::pick(
        title,
        Box::new(move |path| {
            OPEN.with(|o| o.set(false));
            done(path);
        }),
    );
}

#[cfg(target_os = "linux")]
mod platform {
    use std::collections::HashMap;
    use std::path::PathBuf;

    use zbus::zvariant::{OwnedValue, Value};

    type Done = Box<dyn FnOnce(Option<PathBuf>)>;

    thread_local! {
        static DONE: std::cell::RefCell<Option<Done>> = const { std::cell::RefCell::new(None) };
    }

    pub fn pick(title: &'static str, done: Done) {
        DONE.with(|d| *d.borrow_mut() = Some(done));
        // the dialog runs out of process (the portal or zenity); this short-lived thread only
        // waits for the answer and hands it to the UI thread
        let spawned = std::thread::Builder::new()
            .name("file-dialog".into())
            .spawn(move || {
                let path = portal(title).unwrap_or_else(|| fallback(title));
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(d) = DONE.with(|d| d.borrow_mut().take()) {
                        d(path);
                    }
                });
            });
        if spawned.is_err() {
            if let Some(d) = DONE.with(|d| d.borrow_mut().take()) {
                d(None);
            }
        }
    }

    fn portal(title: &str) -> Option<Option<PathBuf>> {
        let conn = zbus::blocking::Connection::session().ok()?;
        let sender = conn
            .unique_name()?
            .trim_start_matches(':')
            .replace('.', "_");
        let mut token_bytes = [0u8; 8];
        let _ = getrandom::fill(&mut token_bytes);
        let token: String = format!(
            "st{}",
            token_bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let request_path = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
        let request = zbus::blocking::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            request_path.as_str(),
            "org.freedesktop.portal.Request",
        )
        .ok()?;
        let mut responses = request.receive_signal("Response").ok()?;
        let chooser = zbus::blocking::Proxy::new(
            &conn,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.FileChooser",
        )
        .ok()?;
        let patterns: Vec<(u32, &str)> = super::IMAGE_PATTERNS.iter().map(|p| (0u32, *p)).collect();
        let filters = vec![("Images", patterns)];
        let mut options: HashMap<&str, Value> = HashMap::new();
        options.insert("handle_token", Value::from(token.as_str()));
        options.insert("modal", Value::from(true));
        options.insert("multiple", Value::from(false));
        options.insert("filters", Value::from(filters));
        let _: zbus::zvariant::OwnedObjectPath =
            chooser.call("OpenFile", &("", title, options)).ok()?;
        let message = responses.next()?;
        let (code, results): (u32, HashMap<String, OwnedValue>) =
            message.body().deserialize().ok()?;
        if code != 0 {
            return Some(None);
        }
        let uris: Vec<String> = results
            .get("uris")
            .and_then(|v| Vec::<String>::try_from(v.clone()).ok())
            .unwrap_or_default();
        Some(uris.first().and_then(|u| file_uri_to_path(u)))
    }

    /// `file:///a%20b.png` -> `/a b.png` (only local files are accepted).
    fn file_uri_to_path(uri: &str) -> Option<PathBuf> {
        let rest = uri.strip_prefix("file://")?;
        let bytes = rest.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
    }

    fn fallback(title: &str) -> Option<PathBuf> {
        let filter = format!("Images | {}", super::IMAGE_PATTERNS.join(" "));
        let tries: [(&str, Vec<String>); 2] = [
            (
                "zenity",
                vec![
                    "--file-selection".into(),
                    format!("--title={title}"),
                    format!("--file-filter={filter}"),
                ],
            ),
            (
                "kdialog",
                vec![
                    "--getopenfilename".into(),
                    ".".into(),
                    super::IMAGE_PATTERNS.join(" "),
                    "--title".into(),
                    title.into(),
                ],
            ),
        ];
        for (cmd, args) in tries {
            match std::process::Command::new(cmd).args(&args).output() {
                Ok(out) if out.status.success() => {
                    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    return (!p.is_empty()).then(|| PathBuf::from(p));
                }
                Ok(_) => return None, // cancelled
                Err(_) => continue,   // not installed
            }
        }
        log::warn!("file dialog: no XDG portal, zenity or kdialog is available");
        None
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn file_uris_decode() {
            assert_eq!(
                super::file_uri_to_path("file:///home/x/my%20pic.png").unwrap(),
                std::path::PathBuf::from("/home/x/my pic.png")
            );
            assert!(super::file_uri_to_path("https://x/y.png").is_none());
        }
    }
}

#[cfg(windows)]
mod platform {
    use std::path::PathBuf;

    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, SIGDN_FILESYSPATH,
    };

    type Done = Box<dyn FnOnce(Option<PathBuf>)>;

    pub fn pick(title: &'static str, done: Done) {
        // deferred one turn so the click that opened it has finished
        let _ = slint::invoke_from_event_loop(move || {
            let path = unsafe { show(title) };
            done(path);
        });
    }

    unsafe fn show(title: &str) -> Option<PathBuf> {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let name = HSTRING::from("Images");
        let spec = HSTRING::from(super::IMAGE_PATTERNS.join(";"));
        let filters = [COMDLG_FILTERSPEC {
            pszName: PCWSTR(name.as_ptr()),
            pszSpec: PCWSTR(spec.as_ptr()),
        }];
        dialog.SetFileTypes(&filters).ok()?;
        dialog.SetTitle(&HSTRING::from(title)).ok()?;
        let opts = dialog.GetOptions().ok()?;
        dialog
            .SetOptions(opts | FOS_FILEMUSTEXIST | FOS_FORCEFILESYSTEM)
            .ok()?;
        let owner =
            crate::platform::win_host::find_main_window(&crate::platform::identity::window_title())
                .map(|h| windows::Win32::Foundation::HWND(h as _));
        dialog.Show(owner).ok()?;
        let item = dialog.GetResult().ok()?;
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path = raw.to_string().ok();
        CoTaskMemFree(Some(raw.0 as *const _));
        path.map(PathBuf::from)
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod platform {
    pub fn pick(_title: &'static str, done: Box<dyn FnOnce(Option<std::path::PathBuf>)>) {
        log::warn!("file dialog: not available on this platform yet (Stage 24)");
        done(None);
    }
}
