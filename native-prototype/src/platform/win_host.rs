//! Windows platform host (Stage 18): the hidden message-only window that owns the tray icon,
//! receives second-instance activation requests, and the small set of main-window controls
//! (hide / show / restore / foreground) the application layer needs.
//!
//! Everything runs on the UI thread, driven by the message pump Slint/winit already runs: there is
//! no extra thread, no timer and no polling loop. Commands leave this module only as
//! [`PlatformCommand`] values handed to a handler the application installs; nothing here knows
//! about the Timer, the Dashboard or Slint (architecture freeze, section 5).
//!
//! Production reference (`desktop/src-tauri/src/lib.rs`): tray id `study-tracker-timer`, menu
//! "Show Study Tracker" / "Quit", menu on right click only, left-click-up restores the window
//! (unminimize + show + focus), tooltip `Study Tracker - <timer title>`, icon generated per timer
//! phase.

use std::cell::RefCell;
use std::sync::OnceLock;

use super::single_instance::{InstanceNames, WM_ACTIVATE_REQUEST};
use super::tray_model::{tray_icon_rgba, tray_tooltip, TrayTimerState, ICON_SIZE};
use super::win_util::{copy_to_fixed, wide};
use study_tracker_core::timer::TimerPhase;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, BringWindowToTop, CreateIconIndirect, CreatePopupMenu, CreateWindowExW,
    DefWindowProcW, DestroyIcon, DestroyMenu, DestroyWindow, EnumThreadWindows, GetCursorPos,
    GetParent, GetWindowTextW, IsIconic, PostMessageW, RegisterClassW, RegisterWindowMessageW,
    SetForegroundWindow, ShowWindow, TrackPopupMenu, HICON, HWND_MESSAGE, ICONINFO, MF_STRING,
    SW_HIDE, SW_RESTORE, SW_SHOW, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
    WM_CONTEXTMENU, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
};

/// What the platform layer asks the application to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformCommand {
    /// Tray left click, tray menu "Show Study Tracker", or a second launch: bring the window up.
    ShowWindow,
    /// Tray menu "Quit": terminate the process for real.
    Quit,
}

type Handler = Box<dyn Fn(PlatformCommand) + Send + Sync>;
static HANDLER: OnceLock<Handler> = OnceLock::new();

const WM_TRAY: u32 = WM_APP + 2;
const TRAY_UID: u32 = 1;
const MENU_SHOW: usize = 1;
const MENU_QUIT: usize = 2;

struct TrayInner {
    hwnd: HWND,
    icon: HICON,
    phase: TimerPhase,
    tooltip: String,
    taskbar_created: u32,
    tray_visible: bool,
}

thread_local! {
    static TRAY: RefCell<Option<TrayInner>> = const { RefCell::new(None) };
}

/// Owns the message window and the tray icon; dropping it removes the tray icon and destroys the
/// window (called explicitly on the way out of `run()`, so the icon never outlives the process).
pub struct PlatformHost {
    _not_send: std::marker::PhantomData<*const ()>,
}

fn send_command(command: PlatformCommand) {
    if let Some(handler) = HANDLER.get() {
        handler(command);
    }
}

unsafe extern "system" fn platform_window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ACTIVATE_REQUEST => {
            send_command(PlatformCommand::ShowWindow);
            0
        }
        WM_TRAY => {
            // NOTIFYICON_VERSION_4: the event is in the low word of lParam.
            let event = (lparam as u32) & 0xffff;
            match event {
                WM_LBUTTONUP => send_command(PlatformCommand::ShowWindow),
                WM_CONTEXTMENU | WM_RBUTTONUP => {
                    if let Some(command) = show_tray_menu(hwnd) {
                        send_command(command);
                    }
                }
                _ => {}
            }
            0
        }
        other => {
            let taskbar_created = TRAY.with(|t| t.borrow().as_ref().map(|t| t.taskbar_created));
            if Some(other) == taskbar_created && other != 0 {
                // Explorer restarted: its notification area forgot our icon; add it again.
                TRAY.with(|t| {
                    if let Some(inner) = t.borrow_mut().as_mut() {
                        inner.tray_visible = false;
                        add_tray_icon(inner);
                    }
                });
                return 0;
            }
            // SAFETY: forwarding an unhandled message with the original arguments.
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
    }
}

fn show_tray_menu(hwnd: HWND) -> Option<PlatformCommand> {
    // SAFETY: standard popup-menu sequence on the UI thread with a window we own.
    unsafe {
        let menu = CreatePopupMenu();
        if menu.is_null() {
            return None;
        }
        let show = wide("Show Study Tracker");
        let quit = wide("Quit");
        AppendMenuW(menu, MF_STRING, MENU_SHOW, show.as_ptr());
        AppendMenuW(menu, MF_STRING, MENU_QUIT, quit.as_ptr());
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point);
        // Required so the menu closes when the user clicks elsewhere.
        SetForegroundWindow(hwnd);
        let choice = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            0,
            hwnd,
            std::ptr::null(),
        );
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);
        match choice as usize {
            MENU_SHOW => Some(PlatformCommand::ShowWindow),
            MENU_QUIT => Some(PlatformCommand::Quit),
            _ => None,
        }
    }
}

/// Builds an `HICON` from straight RGBA pixels (production draws the same 32x32 bitmap).
fn icon_from_rgba(rgba: &[u8]) -> Option<HICON> {
    let size = ICON_SIZE as i32;
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [unsafe { std::mem::zeroed() }],
    };
    // SAFETY: GDI object creation/cleanup with valid arguments; the DIB section memory is written
    // within its `size*size*4` bounds before the bitmap is handed to CreateIconIndirect, which
    // copies it; all temporary GDI objects are released on every path.
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let color = CreateDIBSection(
            hdc,
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        );
        ReleaseDC(std::ptr::null_mut(), hdc);
        if color.is_null() || bits.is_null() {
            return None;
        }
        let target = std::slice::from_raw_parts_mut(bits as *mut u8, rgba.len());
        for (src, dst) in rgba.chunks_exact(4).zip(target.chunks_exact_mut(4)) {
            // RGBA -> BGRA. Icons use straight alpha, exactly what the generator produces.
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
        let mask = CreateBitmap(size, size, 1, 1, std::ptr::null());
        let icon_info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let icon = CreateIconIndirect(&icon_info);
        DeleteObject(color);
        DeleteObject(mask);
        if icon.is_null() {
            None
        } else {
            Some(icon)
        }
    }
}

fn notify_data(inner: &TrayInner, flags: u32) -> NOTIFYICONDATAW {
    // SAFETY: NOTIFYICONDATAW is plain old data; all-zero is a valid starting state.
    let mut data: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    data.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = inner.hwnd;
    data.uID = TRAY_UID;
    data.uFlags = flags;
    data.uCallbackMessage = WM_TRAY;
    data.hIcon = inner.icon;
    copy_to_fixed(&inner.tooltip, &mut data.szTip);
    data
}

fn add_tray_icon(inner: &mut TrayInner) {
    let data = notify_data(inner, NIF_MESSAGE | NIF_ICON | NIF_TIP);
    // SAFETY: valid, fully initialized NOTIFYICONDATAW.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) } != 0;
    if added {
        let mut versioned = notify_data(inner, 0);
        versioned.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        // SAFETY: as above.
        unsafe {
            Shell_NotifyIconW(NIM_SETVERSION, &versioned);
        }
        inner.tray_visible = true;
    } else {
        log::warn!("tray icon could not be added to the notification area");
    }
}

impl PlatformHost {
    /// Creates the message window and the tray icon. `handler` receives every
    /// [`PlatformCommand`]; it is invoked on the UI thread from the window procedure, so it should
    /// only *enqueue* work (e.g. `slint::invoke_from_event_loop`).
    pub fn create(
        names: &InstanceNames,
        handler: impl Fn(PlatformCommand) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        if HANDLER.set(Box::new(handler)).is_err() {
            return Err("platform host already created".into());
        }
        let class_name = wide(&names.window_class);
        // SAFETY: ordinary window-class registration and window creation on the UI thread; all
        // pointers reference locals that outlive the calls.
        let hwnd = unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null());
            let class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(platform_window_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };
            RegisterClassW(&class);
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                HWND_MESSAGE,
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err("message window could not be created".into());
        }
        let phase = TimerPhase::Idle;
        let icon =
            icon_from_rgba(&tray_icon_rgba(phase)).ok_or("tray icon could not be created")?;
        let taskbar_created = {
            let name = wide("TaskbarCreated");
            // SAFETY: NUL-terminated name.
            unsafe { RegisterWindowMessageW(name.as_ptr()) }
        };
        let mut inner = TrayInner {
            hwnd,
            icon,
            phase,
            tooltip: tray_tooltip(TrayTimerState::IDLE),
            taskbar_created,
            tray_visible: false,
        };
        add_tray_icon(&mut inner);
        TRAY.with(|t| *t.borrow_mut() = Some(inner));
        Ok(Self {
            _not_send: std::marker::PhantomData,
        })
    }

    /// Reflects the Timer in the tray. Writes to the notification area only when the tooltip text
    /// or the phase glyph actually changed (the caller may call this every refresh).
    pub fn update_tray(&self, state: TrayTimerState) {
        TRAY.with(|t| {
            let mut guard = t.borrow_mut();
            let Some(inner) = guard.as_mut() else { return };
            let tooltip = tray_tooltip(state);
            let phase_changed = inner.phase != state.phase;
            if !phase_changed && inner.tooltip == tooltip {
                return;
            }
            let mut flags = NIF_TIP;
            if phase_changed {
                if let Some(icon) = icon_from_rgba(&tray_icon_rgba(state.phase)) {
                    let old = std::mem::replace(&mut inner.icon, icon);
                    inner.phase = state.phase;
                    // SAFETY: `old` was created by CreateIconIndirect and is no longer referenced
                    // by us; the shell holds its own copy after NIM_MODIFY below.
                    unsafe { DestroyIcon(old) };
                    flags |= NIF_ICON;
                }
            }
            inner.tooltip = tooltip;
            let data = notify_data(inner, flags);
            // SAFETY: valid, fully initialized NOTIFYICONDATAW.
            unsafe {
                Shell_NotifyIconW(NIM_MODIFY, &data);
            }
        });
    }
}

/// Legacy balloon notification through the tray icon (works without any app identity
/// registration; Windows renders it as a toast). Used as a fallback only.
pub fn show_balloon(title: &str, body: &str) -> bool {
    TRAY.with(|t| {
        let guard = t.borrow();
        let Some(inner) = guard.as_ref() else {
            return false;
        };
        let mut data = notify_data(inner, NIF_INFO);
        copy_to_fixed(body, &mut data.szInfo);
        copy_to_fixed(title, &mut data.szInfoTitle);
        data.dwInfoFlags = NIIF_INFO;
        // SAFETY: valid, fully initialized NOTIFYICONDATAW.
        unsafe { Shell_NotifyIconW(NIM_MODIFY, &data) != 0 }
    })
}

impl Drop for PlatformHost {
    fn drop(&mut self) {
        TRAY.with(|t| {
            if let Some(inner) = t.borrow_mut().take() {
                let data = notify_data(&inner, 0);
                // SAFETY: removing our own icon, then releasing the icon and window we created.
                unsafe {
                    Shell_NotifyIconW(NIM_DELETE, &data);
                    DestroyIcon(inner.icon);
                    DestroyWindow(inner.hwnd);
                }
            }
        });
    }
}

// --- main-window controls -------------------------------------------------------------------

struct FindState {
    title: Vec<u16>,
    found: HWND,
}

unsafe extern "system" fn enum_thread_window(hwnd: HWND, lparam: LPARAM) -> i32 {
    // SAFETY: `lparam` is the address of the `FindState` passed by `find_main_window`, alive for
    // the whole synchronous enumeration.
    let state = unsafe { &mut *(lparam as *mut FindState) };
    let mut buffer = [0u16; 256];
    // SAFETY: buffer has room for the length passed.
    let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) } as usize;
    // Top-level (un-owned) windows only, matched by exact title.
    // SAFETY: plain query.
    let top_level = unsafe { GetParent(hwnd) }.is_null();
    if top_level && buffer[..len] == state.title[..state.title.len() - 1] {
        state.found = hwnd;
        return 0; // stop
    }
    1
}

/// The application's main window (a top-level window of the *current* thread with `title`).
pub fn find_main_window(title: &str) -> Option<HWND> {
    let mut state = FindState {
        title: wide(title),
        found: std::ptr::null_mut(),
    };
    // SAFETY: the callback only runs during this call and receives the address of `state`.
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(enum_thread_window),
            &mut state as *mut _ as LPARAM,
        );
    }
    (!state.found.is_null()).then_some(state.found)
}

/// Hides the window (it stays alive; the process keeps running).
pub fn hide_window(hwnd: HWND) {
    // SAFETY: valid window handle owned by this process.
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// Makes the window visible, restores it if minimized and asks Windows to bring it to the
/// foreground (subject to Windows' foreground-lock rules; a secondary instance grants permission
/// with `AllowSetForegroundWindow` before posting its request).
pub fn show_and_focus(hwnd: HWND) {
    // SAFETY: valid window handle owned by this process.
    unsafe {
        ShowWindow(hwnd, SW_SHOW);
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
    }
}
