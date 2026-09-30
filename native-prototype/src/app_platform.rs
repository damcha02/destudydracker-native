//! Application glue for the Windows platform layer (Stage 18): connects the tray, second-instance
//! activation, notifications and window lifecycle to the real application objects.
//!
//! Dependency direction (architecture freeze, section 5):
//!
//! ```text
//! AppModel (timer + academic state)            platform::{tray_model, notification}  pure rules
//!        |  tray_state(), take_notifications()        |
//!        v                                            v
//!   app_platform (this file: UI-thread only)  --> platform::{win_host, win_toast}   Win32 / WinRT
//! ```
//!
//! Everything here runs on the UI thread. Platform callbacks that arrive from the window procedure
//! are turned into `slint::invoke_from_event_loop` jobs, so no application state is ever touched
//! from inside a Win32 callback, and there is no thread, timer or polling loop of our own.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use slint::{CloseRequestResponse, ComponentHandle};

use crate::app_model::AppModel;
use crate::dashboard_view::DashboardController;
use crate::platform::identity;
use crate::platform::notification::{
    deliver, NotificationKind, NotificationLedger, NotificationRequest, NotificationSink,
};
use crate::platform::single_instance::InstanceNames;
use crate::platform::tray_model::{close_action, CloseAction, HideNoticeLatch};
use crate::platform::win_host::{
    find_main_window, hide_window, show_and_focus, PlatformCommand, PlatformHost,
};
use crate::platform::win_toast::{register_identity, ToastSink};
use crate::MainWindow;

/// True while the window is hidden to the tray: the 100 ms refresh skips property pushes exactly
/// like it does while minimized (the Stage 14 rule), so a hidden window renders nothing.
static HIDDEN_TO_TRAY: AtomicBool = AtomicBool::new(false);

pub fn is_hidden_to_tray() -> bool {
    HIDDEN_TO_TRAY.load(Ordering::Relaxed)
}

struct Runtime {
    window: slint::Weak<MainWindow>,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
    host: PlatformHost,
    sink: Box<dyn NotificationSink>,
    ledger: NotificationLedger,
    hide_notice: HideNoticeLatch,
}

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

/// Creates the tray/message window, registers the notification identity, installs the
/// close-request policy. Call once, on the UI thread, before the event loop starts.
pub fn install(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
    names: &InstanceNames,
    data_dir: &std::path::Path,
) -> Result<(), String> {
    // Notification identity: own AUMID + display name + icon (never production's).
    let icon_path = data_dir.join("branding").join("icon.png");
    if let Some(parent) = icon_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&icon_path, include_bytes!("../assets/branding/icon.png"));
    register_identity(
        identity::APP_ID,
        "Study Tracker (Native Preview)",
        &icon_path,
    );

    // The handler runs inside the window procedure: only enqueue.
    let host = PlatformHost::create(names, |command| {
        let _ = slint::invoke_from_event_loop(move || handle_command(command));
    })?;
    log::info!(
        "platform host ready (tray icon + activation window `{}`)",
        names.window_class
    );
    model.borrow_mut().take_notifications(); // nothing queued before we exist
    RUNTIME.with(|r| {
        *r.borrow_mut() = Some(Runtime {
            window: window.as_weak(),
            model,
            dashboard,
            host,
            sink: Box::new(ToastSink::new(identity::APP_ID)),
            ledger: NotificationLedger::default(),
            hide_notice: HideNoticeLatch::default(),
        });
    });

    window.window().on_close_requested(|| {
        let action = RUNTIME.with(|r| {
            r.borrow()
                .as_ref()
                .map(|rt| close_action(rt.model.borrow().tray_state(Instant::now())))
                .unwrap_or(CloseAction::Quit)
        });
        match action {
            CloseAction::Quit => {
                let _ = slint::quit_event_loop();
                CloseRequestResponse::HideWindow
            }
            CloseAction::HideToTray => {
                hide_to_tray();
                CloseRequestResponse::KeepWindowShown
            }
        }
    });
    Ok(())
}

/// Hide the window but keep the process (and the Timer) running.
fn hide_to_tray() {
    if let Some(hwnd) = find_main_window(&identity::window_title()) {
        HIDDEN_TO_TRAY.store(true, Ordering::Relaxed);
        hide_window(hwnd);
        log::info!("window hidden to the tray (a study session is running)");
    }
    // Production shows a notice the first time per run.
    RUNTIME.with(|r| {
        if let Some(rt) = r.borrow_mut().as_mut() {
            if rt.hide_notice.first_time() {
                let request = NotificationRequest::of(NotificationKind::StillRunningInTray);
                let now = crate::wall_now().unix_millis;
                deliver(rt.sink.as_mut(), &mut rt.ledger, &[(request, now)]);
            }
        }
    });
}

fn handle_command(command: PlatformCommand) {
    match command {
        PlatformCommand::Quit => {
            log::info!("tray: Quit requested");
            let _ = slint::quit_event_loop();
        }
        PlatformCommand::ShowWindow => show_main_window(),
    }
}

/// Tray click, tray menu, or a second launch: bring the existing window up, unchanged.
fn show_main_window() {
    let was_hidden = HIDDEN_TO_TRAY.swap(false, Ordering::Relaxed);
    if let Some(hwnd) = find_main_window(&identity::window_title()) {
        show_and_focus(hwnd);
    }
    // Properties were not pushed while hidden/minimized: bring the view up to date right now.
    let handles = RUNTIME.with(|r| {
        r.borrow().as_ref().map(|rt| {
            (
                rt.window.clone(),
                Rc::clone(&rt.model),
                Rc::clone(&rt.dashboard),
            )
        })
    });
    if let Some((weak, model, dashboard)) = handles {
        if let Some(window) = weak.upgrade() {
            crate::apply_model_to_window(&window, &model.borrow(), Instant::now());
            crate::refresh_dashboard_if_changed(
                &window,
                &model.borrow(),
                &mut dashboard.borrow_mut(),
            );
        }
    }
    let timer_report = RUNTIME.with(|r| {
        r.borrow()
            .as_ref()
            .map(|rt| rt.model.borrow().tray_state(Instant::now()))
    });
    log::info!("window shown (was hidden to tray: {was_hidden}); timer state: {timer_report:?}");
}

/// After any Timer command or tick: mirror the Timer into the tray and deliver whatever
/// notifications the command produced. Cheap when nothing changed (string compare, empty outbox).
pub fn after_timer_activity() {
    RUNTIME.with(|r| {
        let mut guard = r.borrow_mut();
        let Some(rt) = guard.as_mut() else { return };
        let (state, pending) = {
            let mut model = rt.model.borrow_mut();
            (model.tray_state(Instant::now()), model.take_notifications())
        };
        rt.host.update_tray(state);
        if !pending.is_empty() {
            deliver(rt.sink.as_mut(), &mut rt.ledger, &pending);
        }
    });
}

/// Explicit end of the process: removes the tray icon and closes the message window *before*
/// the event loop's owner returns, so no icon outlives the process.
pub fn shutdown() {
    RUNTIME.with(|r| drop(r.borrow_mut().take()));
}
