//! Windows toast notifications for an *unpackaged* desktop executable (Stage 18).
//!
//! A toast needs an AppUserModelID (AUMID) Windows can attribute it to. Unpackaged apps get one by
//! (a) setting it on the process (`SetCurrentProcessExplicitAppUserModelID`) and (b) registering a
//! display name and icon for it under `HKCU\Software\Classes\AppUserModelId\<AUMID>` - a per-user,
//! non-elevated registry key that this module owns and that contains nothing else. The AUMID is the
//! native prototype's own (`identity::APP_ID`), never production's `com.damcha.studytracker`, so
//! the installed Tauri app and the prototype keep separate notification identities (and separate
//! entries in Windows' per-app notification settings).
//!
//! If showing a toast fails, [`ToastSink`] falls back to the tray icon's balloon notification
//! (which needs no identity) and, failing that, reports an error that `notification::deliver`
//! logs and drops - a notification can never affect the Timer or persistence.

use super::notification::{NotificationRequest, NotificationSink};
use super::win_host;
use super::win_util::wide;
use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::UI::Notifications::{
    NotificationSetting, ToastNotification, ToastNotificationManager,
};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};
use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

fn registry_path(aumid: &str) -> String {
    format!("Software\\Classes\\AppUserModelId\\{aumid}")
}

fn set_string(key: HKEY, name: &str, value: &str) -> bool {
    let (name, value) = (wide(name), wide(value));
    // SAFETY: valid open key; NUL-terminated UTF-16 buffers; byte length includes the terminator.
    unsafe {
        RegSetValueExW(
            key,
            name.as_ptr(),
            0,
            REG_SZ,
            value.as_ptr() as *const u8,
            (value.len() * 2) as u32,
        ) == 0
    }
}

/// Registers the notification identity for this process (idempotent, per-user). Returns `false`
/// (and logs) if the registry write failed; toasts may then fall back to the balloon path.
pub fn register_identity(aumid: &str, display_name: &str, icon_png: &std::path::Path) -> bool {
    let path = wide(&registry_path(aumid));
    let mut key: HKEY = std::ptr::null_mut();
    // SAFETY: valid NUL-terminated key path, out-pointer to a local; the key is closed below.
    let created = unsafe {
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            path.as_ptr(),
            0,
            std::ptr::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        )
    };
    let mut ok = created == 0;
    if ok {
        ok &= set_string(key, "DisplayName", display_name);
        ok &= set_string(key, "IconUri", &icon_png.to_string_lossy());
        // SAFETY: key opened above.
        unsafe { RegCloseKey(key) };
    } else {
        log::warn!("notification identity could not be registered (registry error {created})");
    }
    let id = wide(aumid);
    // SAFETY: NUL-terminated id that outlives the call.
    let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(id.as_ptr()) };
    if hr < 0 {
        log::warn!("SetCurrentProcessExplicitAppUserModelID failed (HRESULT {hr:#x})");
        ok = false;
    }
    ok
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn show_toast(aumid: &str, request: &NotificationRequest) -> windows::core::Result<()> {
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        xml_escape(request.title),
        xml_escape(request.body)
    )))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(aumid))?;
    if notifier.Setting()? != NotificationSetting::Enabled {
        return Err(windows::core::Error::new(
            windows::core::HRESULT(0x8000_4005u32 as i32),
            "notifications are disabled for this app in Windows settings",
        ));
    }
    notifier.Show(&toast)
}

pub struct ToastSink {
    aumid: String,
    /// Set when WinRT refuses our identity (e.g. `0x80070490 Element not found`: an unpackaged
    /// executable with no Start-menu shortcut carrying the AUMID - the installer creates that
    /// shortcut in Stage 23). After the first such failure the sink goes straight to the tray
    /// balloon instead of failing the toast path on every notification.
    toast_unavailable: bool,
}

impl ToastSink {
    pub fn new(aumid: &str) -> Self {
        Self {
            aumid: aumid.to_string(),
            toast_unavailable: false,
        }
    }
}

impl NotificationSink for ToastSink {
    fn send(&mut self, request: &NotificationRequest) -> Result<(), String> {
        if !self.toast_unavailable {
            match show_toast(&self.aumid, request) {
                Ok(()) => return Ok(()),
                Err(toast_error) => {
                    log::info!("WinRT toast unavailable for this identity ({toast_error}); using the tray balloon notification from now on");
                    self.toast_unavailable = true;
                }
            }
        }
        if win_host::show_balloon(request.title, request.body) {
            Ok(())
        } else {
            Err("neither a toast nor the tray balloon could be shown".to_string())
        }
    }
}
