//! Application glue for appearance (Stage 19): the persisted style/palette/theme preference, the
//! colour tokens it resolves to, the Wabi-Sabi surfaces' data and actions, and the Sakura clock's
//! run/stop rule. Same shape as Stage 18's `app_platform`: one UI-thread runtime in a thread-local,
//! entered from Slint callbacks and from the few places `main.rs` already refreshes the window.
//!
//! ```text
//! menu / panel / toggles ──► PreferencesController::set (writes only on change)
//!                                     │
//!                                     ▼ core::appearance::resolve
//!                 apply_appearance: FN.t / WS.t / Chrome.t, wabi flag, cards, Sakura rule
//!
//! academic change (refresh_dashboard) ──► WabiController::sync (cached by revision) ──► wabi-*
//! 100 ms Timer tick (apply_model_to_window) ──► wabi-timer (cards model reused, Rule A)
//! WM_SIZE / WM_SHOWWINDOW / tray hide-show / surface change ──► sync_sakura
//! ```

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Global, ModelRc, VecModel};
use study_tracker_core::academic::{CalendarEntryId, DailyTodoId, TimetableEventId};
use study_tracker_core::appearance::{
    AppStyle, AppearancePrefs, Palette, RenderedStyle, ThemeMode,
};
use study_tracker_core::dashboard::wabi::MarkTarget;

use crate::app_model::AppModel;
use crate::appearance_view::{
    cards_model, chrome_tokens, fn_tokens, palette_choices, style_choices, ws_tokens,
};
use crate::dashboard_view::{ChronoLocalClock, DashboardController};
use crate::persistence::PreferencesController;
use crate::sakura_controller::{SakuraController, SakuraState, SakuraWant};
use crate::wabi_view::{timer_data, WabiAction, WabiController};
use crate::{Chrome, MainWindow, WabiModeCard, FN, WS};

struct Runtime {
    window: slint::Weak<MainWindow>,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
    prefs: PreferencesController,
    wabi: WabiController,
    sakura: Rc<RefCell<SakuraController>>,
    /// Frozen animation time for screenshots (`STUDY_NATIVE_SAKURA_TIME`), else `None`.
    frozen_sakura_ms: Option<f64>,
    timer_cards: ModelRc<WabiModeCard>,
    switches: u64,
}

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

fn with_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    RUNTIME.with(|r| r.borrow_mut().as_mut().map(f))
}

/// Startup: loads nothing itself (the caller passes the loaded preference controller), pushes the
/// first appearance, binds every appearance/Wabi-Sabi callback. Call once, before the first frame.
pub fn install(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
    prefs: PreferencesController,
) {
    let sakura = SakuraController::new();
    {
        let assets = crate::SakuraAssets::get(window);
        sakura.borrow_mut().state_mut_for_freeze().set_images(
            crate::sakura_controller::PetalImages::from_sources([
                assets.get_petal_0(),
                assets.get_petal_1(),
            ]),
        );
    }
    window.set_petals(sakura.borrow().state().model());
    let frozen_sakura_ms = std::env::var("STUDY_NATIVE_SAKURA_TIME")
        .ok()
        .and_then(|v| v.parse::<f64>().ok());
    RUNTIME.with(|r| {
        *r.borrow_mut() = Some(Runtime {
            window: window.as_weak(),
            model,
            dashboard,
            prefs,
            wabi: WabiController::new(twelve_hour_clock()),
            sakura,
            frozen_sakura_ms,
            timer_cards: ModelRc::new(VecModel::<WabiModeCard>::default()),
            switches: 0,
        })
    });
    if std::env::var_os("STUDY_NATIVE_WABI_QUIET").is_some() {
        with_runtime(|rt| rt.wabi.ui_mut().quiet = true);
    }
    bind_callbacks(window);
    apply_appearance(window);
}

/// The current preference (for diagnostics and tests).
pub fn prefs() -> Option<AppearancePrefs> {
    with_runtime(|rt| rt.prefs.prefs())
}

pub fn preference_writes() -> u64 {
    with_runtime(|rt| rt.prefs.writes()).unwrap_or(0)
}

/// Applies a new preference through the single writer and re-renders.
pub fn set_prefs(prefs: AppearancePrefs) {
    let changed = with_runtime(|rt| {
        let changed = rt.prefs.set(prefs);
        if changed {
            rt.switches += 1;
        }
        changed
    })
    .unwrap_or(false);
    if changed {
        if let Some(window) = with_runtime(|rt| rt.window.upgrade()).flatten() {
            log::info!("appearance: preference changed to {prefs:?} (persisted)");
            apply_appearance(&window);
        }
    }
}

fn resolved() -> Option<(
    AppearancePrefs,
    study_tracker_core::appearance::ResolvedAppearance,
)> {
    with_runtime(|rt| {
        let prefs = rt.prefs.prefs();
        (prefs, prefs.resolve())
    })
}

/// Pushes tokens, the style flag, the picker cards, the Wabi-Sabi data (if that style is drawn) and
/// re-evaluates the Sakura rule. Runs on startup and on an actual preference change only.
pub fn apply_appearance(window: &MainWindow) {
    let Some((prefs, resolved)) = resolved() else {
        return;
    };
    FN::get(window).set_t(fn_tokens(resolved.scheme));
    FN::get(window).set_dark(resolved.dark);
    WS::get(window).set_t(ws_tokens(resolved.scheme));
    Chrome::get(window).set_t(chrome_tokens(&resolved));
    window.set_wabi(resolved.rendered == RenderedStyle::WabiSabi);
    window.set_theme_locked(resolved.theme_locked);
    window.set_appearance_dark(resolved.dark);
    window.set_sakura(resolved.sakura);
    window.set_palette_cards(cards_model(
        palette_choices(prefs).into_iter().map(|(_, c)| c).collect(),
    ));
    window.set_style_cards(cards_model(
        style_choices(prefs).into_iter().map(|(_, c)| c).collect(),
    ));
    if resolved.rendered == RenderedStyle::WabiSabi {
        refresh_wabi(window, true);
        refresh_wabi_timer(window, Instant::now());
    }
    sync_sakura();
}

fn is_wabi() -> bool {
    resolved().is_some_and(|(_, r)| r.rendered == RenderedStyle::WabiSabi)
}

/// After an academic change / date rollover / Dashboard refresh: recompute the Wabi-Sabi data if
/// that style is on screen (and only then - Field Notebook users pay nothing for it).
pub fn after_dashboard_refresh(
    window: &MainWindow,
    model: &AppModel,
    dashboard: &DashboardController,
) {
    if is_wabi() {
        refresh_wabi_with(window, model, dashboard, false);
    }
}

/// Re-derives from the runtime's own handles (callers that hold no borrow of them).
fn refresh_wabi(window: &MainWindow, force_push: bool) {
    let Some((model, dashboard)) =
        with_runtime(|rt| (Rc::clone(&rt.model), Rc::clone(&rt.dashboard)))
    else {
        return;
    };
    let model = model.borrow();
    {
        let clock = ChronoLocalClock;
        dashboard.borrow_mut().sync(
            model.academic().state(),
            model.academic().revision(),
            crate::wall_now(),
            &clock,
        );
    }
    refresh_wabi_with(window, &model, &dashboard.borrow(), force_push);
}

/// The Wabi-Sabi Dashboard shares `getFieldDashboardData` with Field Notebook, so the already
/// cached metrics feed it (no second metrics computation).
fn refresh_wabi_with(
    window: &MainWindow,
    model: &AppModel,
    dashboard: &DashboardController,
    force_push: bool,
) {
    let Some(metrics) = dashboard.metrics() else {
        return;
    };
    let clock = ChronoLocalClock;
    with_runtime(|rt| {
        let recomputed = rt.wabi.sync(
            model.academic().state(),
            metrics,
            model.academic().revision(),
            &clock,
            model.active_timer_goal(),
        );
        if recomputed || force_push {
            window.set_wabi_dash(rt.wabi.dash_data());
            window.set_wabi_sidebar(rt.wabi.sidebar_data());
            window.set_wabi_quiet_data(rt.wabi.quiet_data());
            window.set_wabi_quiet(rt.wabi.ui().quiet);
            window.set_wabi_timer_menu_open(rt.wabi.ui().timer_menu_open);
        }
    });
}

/// From the 100 ms Timer refresh (and after every Timer command): the Wabi-Sabi Timer/Quiet values.
/// Cheap; the cards model is only replaced when the selected preset or availability changed.
pub fn refresh_wabi_timer(window: &MainWindow, now: Instant) {
    if !is_wabi() {
        return;
    }
    with_runtime(|rt| {
        let facts = rt.model.borrow().wabi_timer_facts(now);
        let (data, replaced) = timer_data(&facts, &rt.timer_cards);
        if let Some(model) = replaced {
            rt.timer_cards = model;
        }
        window.set_wabi_timer(data);
    });
}

fn act(action: WabiAction) {
    let Some(window) = with_runtime(|rt| rt.window.upgrade()).flatten() else {
        return;
    };
    match action {
        WabiAction::Toggle(target) => {
            let handles = with_runtime(|rt| (Rc::clone(&rt.model), Rc::clone(&rt.dashboard)));
            let Some((model, dashboard)) = handles else {
                return;
            };
            let now = crate::wall_now();
            let changed = {
                let mut model = model.borrow_mut();
                let academic = model.academic_mut();
                match &target {
                    MarkTarget::CalendarEntry(id) => {
                        academic.toggle_calendar_entry(&CalendarEntryId::new(id.clone()), now)
                    }
                    MarkTarget::TimetableOccurrence { event_id, date } => academic
                        .toggle_timetable_occurrence(
                            &TimetableEventId::new(event_id.clone()),
                            date,
                        ),
                    MarkTarget::Todo { todo_id, date } => academic.toggle_daily_todo_occurrence(
                        &DailyTodoId::new(todo_id.clone()),
                        date,
                        now,
                    ),
                }
            };
            log::info!("wabi: toggled {target:?} (changed: {changed})");
            // The same refresh path every academic change takes (revision bump -> recompute).
            crate::refresh_dashboard(&window, &model.borrow(), &mut dashboard.borrow_mut());
        }
        WabiAction::Start(target) => {
            log::info!(
                "wabi: start requested for {target:?} (Timer task linking is not migrated yet)"
            );
            window.set_show_dashboard(false);
            with_runtime(|rt| rt.wabi.ui_mut().quiet = false);
            window.set_wabi_quiet(false);
        }
        WabiAction::Select(task) => {
            with_runtime(|rt| {
                rt.wabi.ui_mut().selected_task = Some(task);
            });
            refresh_wabi(&window, true);
        }
    }
}

fn bind_callbacks(window: &MainWindow) {
    window.on_appearance_pick_palette(|index| {
        let Some(prefs) = prefs() else { return };
        if let Some((palette, _)) = palette_choices(prefs).get(index.max(0) as usize) {
            set_prefs(AppearancePrefs {
                palette: *palette,
                ..prefs
            });
        }
    });
    window.on_appearance_pick_style(|index| {
        let Some(prefs) = prefs() else { return };
        if let Some((style, card)) = style_choices(prefs).get(index.max(0) as usize) {
            if card.enabled {
                set_prefs(AppearancePrefs {
                    style: *style,
                    ..prefs
                });
            }
        }
    });
    window.on_appearance_set_dark(|dark| {
        let Some((prefs, resolved)) = resolved() else {
            return;
        };
        if dark && resolved.theme_locked {
            return; // "Sakura is light-only" (the button is disabled too)
        }
        set_prefs(AppearancePrefs {
            theme: if dark {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            },
            ..prefs
        });
    });
    window.on_appearance_toggle_dark(|| {
        let Some((prefs, resolved)) = resolved() else {
            return;
        };
        if resolved.theme_locked {
            return;
        }
        set_prefs(AppearancePrefs {
            theme: prefs.theme.toggled(),
            ..prefs
        });
    });
    // Navigation (`setActiveTab`): the sidebar's Today/Timer, which also leave Quiet mode.
    {
        let weak = window.as_weak();
        window.on_wabi_open_dashboard(move || {
            let Some(window) = weak.upgrade() else { return };
            with_runtime(|rt| rt.wabi.ui_mut().quiet = false);
            window.set_wabi_quiet(false);
            window.set_show_dashboard(true);
        });
    }
    {
        let weak = window.as_weak();
        window.on_wabi_open_timer(move || {
            let Some(window) = weak.upgrade() else { return };
            let already = !window.get_show_dashboard() && !window.get_wabi_quiet();
            // "Already on Timer: toggle the submenu. Coming from elsewhere: open it."
            let open = with_runtime(|rt| {
                let ui = rt.wabi.ui_mut();
                ui.quiet = false;
                ui.timer_menu_open = if already { !ui.timer_menu_open } else { true };
                ui.timer_menu_open
            })
            .unwrap_or(true);
            window.set_wabi_quiet(false);
            window.set_wabi_timer_menu_open(open);
            window.set_show_dashboard(false);
        });
    }
    {
        let weak = window.as_weak();
        window.on_wabi_toggle_quiet(move || {
            let Some(window) = weak.upgrade() else { return };
            let quiet = with_runtime(|rt| {
                let ui = rt.wabi.ui_mut();
                if !ui.quiet {
                    // entering: start from the Timer's own task (none natively) and fold menus
                    ui.quiet_task = None;
                    ui.timer_menu_open = false;
                }
                ui.quiet = !ui.quiet;
                ui.quiet
            })
            .unwrap_or(false);
            window.set_wabi_quiet(quiet);
            refresh_wabi(&window, true);
            sync_sakura();
        });
    }
    {
        let weak = window.as_weak();
        window.on_wabi_toggle_semester(move || {
            let Some(window) = weak.upgrade() else { return };
            with_runtime(|rt| {
                let ui = rt.wabi.ui_mut();
                ui.semester_open = !ui.semester_open;
                rt.wabi.invalidate_ui();
            });
            refresh_wabi(&window, true);
        });
    }
    {
        let weak = window.as_weak();
        window.on_wabi_toggle_course(move |index| {
            let Some(window) = weak.upgrade() else { return };
            with_runtime(|rt| {
                if let Some(id) = rt.wabi.course_id(index.max(0) as usize) {
                    let open = &mut rt.wabi.ui_mut().open_courses;
                    if !open.remove(&id) {
                        open.insert(id);
                    }
                    rt.wabi.invalidate_ui();
                }
            });
            refresh_wabi(&window, true);
        });
    }
    window.on_wabi_start_one(|| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.one_thing_start()) {
            act(action);
        }
    });
    window.on_wabi_mark_one(|| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.one_thing_mark()) {
            act(action);
        }
    });
    window.on_wabi_deadline_mark(|i| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.deadline_mark(i.max(0) as usize)) {
            act(action);
        }
    });
    window.on_wabi_deadline_focus(|i| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.deadline_focus(i.max(0) as usize)) {
            act(action);
        }
    });
    window.on_wabi_deadline_select(|i| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.deadline_select(i.max(0) as usize)) {
            act(action);
        }
    });
    window.on_wabi_planned_mark(|i| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.planned_mark(i.max(0) as usize)) {
            act(action);
        }
    });
    window.on_wabi_planned_select(|i| {
        if let Some(Some(action)) = with_runtime(|rt| rt.wabi.planned_select(i.max(0) as usize)) {
            act(action);
        }
    });
    {
        let weak = window.as_weak();
        window.on_wabi_quiet_pick(move |i| {
            let Some(window) = weak.upgrade() else { return };
            with_runtime(|rt| {
                if let Some(task) = rt.wabi.quiet_pick(i.max(0) as usize) {
                    rt.wabi.ui_mut().quiet_task = Some(task);
                }
            });
            refresh_wabi(&window, true);
        });
    }
    window.on_surface_changed(sync_sakura);
}

// --- Sakura ----------------------------------------------------------------------------------------

fn window_visible(window: &MainWindow) -> bool {
    if window.window().is_minimized() {
        return false;
    }
    #[cfg(windows)]
    {
        if crate::app_platform::is_hidden_to_tray() {
            return false;
        }
        if let Some(hwnd) =
            crate::platform::win_host::find_main_window(&crate::platform::identity::window_title())
        {
            return crate::platform::win_host::is_window_showing(hwnd);
        }
    }
    true
}

fn reduced_motion() -> bool {
    if std::env::var_os("STUDY_NATIVE_REDUCED_MOTION").is_some() {
        return true;
    }
    #[cfg(windows)]
    {
        crate::platform::win_host::prefers_reduced_motion()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Re-evaluates the Sakura run/stop rule. Cheap; called on every visibility-relevant event.
pub fn sync_sakura() {
    let Some((window, sakura, frozen, enabled)) = with_runtime(|rt| {
        (
            rt.window.clone(),
            Rc::clone(&rt.sakura),
            rt.frozen_sakura_ms,
            rt.prefs.prefs().resolve().sakura,
        )
    }) else {
        return;
    };
    let Some(win) = window.upgrade() else { return };
    let surface = !win.get_show_text_spike() && !win.get_show_map();
    let want = SakuraWant {
        enabled,
        surface,
        window_visible: window_visible(&win),
    };
    if let Some(t) = frozen {
        // Screenshot mode: one deterministic frame at a fixed animation time, no clock at all.
        let mut s = sakura.borrow_mut();
        s.stop();
        let h = win
            .window()
            .size()
            .to_logical(win.window().scale_factor())
            .height;
        freeze(&mut s, t, h, &win);
        return;
    }
    let weak = window.clone();
    SakuraController::sync(
        &sakura,
        want,
        reduced_motion(),
        move |state: &mut SakuraState| {
            let Some(win) = weak.upgrade() else { return };
            let h = win
                .window()
                .size()
                .to_logical(win.window().scale_factor())
                .height;
            state.update(Instant::now(), h);
            win.set_texture_frame(state.texture_frame());
            crate::SAKURA_FRAMES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
    );
}

fn freeze(s: &mut SakuraController, t_ms: f64, h: f32, win: &MainWindow) {
    // Evaluate the poses as if the effect had been running for exactly `t_ms`.
    let origin = Instant::now()
        .checked_sub(Duration::from_secs_f64(t_ms / 1000.0))
        .unwrap_or_else(Instant::now);
    let state = s.state_mut_for_freeze();
    state.set_want(
        SakuraWant {
            enabled: false,
            ..SakuraWant::default()
        },
        origin,
    );
    state.set_want(
        SakuraWant {
            enabled: true,
            surface: true,
            window_visible: true,
        },
        origin,
    );
    state.set_reduced_motion(reduced_motion());
    state.update(origin + Duration::from_secs_f64(t_ms / 1000.0), h);
    win.set_texture_frame(state.texture_frame());
}

#[cfg(windows)]
fn install_window_watch() {
    let Some(hwnd) =
        crate::platform::win_host::find_main_window(&crate::platform::identity::window_title())
    else {
        log::warn!(
            "sakura: main window not found yet; visibility is re-checked on tray/surface events"
        );
        return;
    };
    let installed = crate::platform::win_host::watch_main_window(hwnd, || {
        // Inside the window procedure: only enqueue.
        let _ = slint::invoke_from_event_loop(sync_sakura);
    });
    log::info!("sakura: window state watch installed: {installed}");
}

/// Must be called once the window exists on screen (after `show()`), so the HWND can be found.
/// winit creates the native window lazily, so the watch is installed from inside the event loop,
/// retrying briefly until the HWND exists.
pub fn after_window_shown() {
    fn attempt(tries_left: u32) {
        #[cfg(windows)]
        {
            let found = crate::platform::win_host::find_main_window(
                &crate::platform::identity::window_title(),
            )
            .is_some();
            if !found && tries_left > 0 {
                slint::Timer::single_shot(Duration::from_millis(50), move || {
                    attempt(tries_left - 1)
                });
                return;
            }
            install_window_watch();
        }
        let _ = tries_left;
        sync_sakura();
    }
    let _ = slint::invoke_from_event_loop(|| attempt(40));
}

/// Stats line for `STUDY_NATIVE_FRAME_STATS`.
pub fn sakura_report() -> String {
    with_runtime(|rt| {
        let s = rt.sakura.borrow();
        let st = s.state().stats();
        format!(
            "sakura_running={} live_petals={} starts={} stops={} prefs_writes={} style_switches={} wabi_recomputes={}",
            s.is_running(),
            st.live_petals,
            st.starts,
            st.stops,
            rt.prefs.writes(),
            rt.switches,
            rt.wabi.recomputes()
        )
    })
    .unwrap_or_default()
}

/// `STUDY_NATIVE_THEME_STRESS=<n>`: n style switches Field Notebook <-> Wabi-Sabi through the same
/// path the panel uses (every 40 ms), keeping the current palette (so Sakura stays on when chosen).
pub fn start_theme_stress() -> Option<slint::Timer> {
    let cycles: u32 = std::env::var("STUDY_NATIVE_THEME_STRESS")
        .ok()?
        .parse()
        .ok()?;
    let timer = slint::Timer::default();
    let mut done = 0u32;
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(40),
        move || {
            if done >= cycles * 2 {
                if done == cycles * 2 {
                    log::info!("THEME_STRESS done after {cycles} round trips");
                    println!(
                        "THEME_STRESS done switches={} prefs_writes={}",
                        cycles * 2,
                        preference_writes()
                    );
                    done += 1;
                }
                return;
            }
            let Some(prefs) = prefs() else { return };
            let style = if prefs.style == AppStyle::WabiSabi {
                AppStyle::FieldNotebook
            } else {
                AppStyle::WabiSabi
            };
            set_prefs(AppearancePrefs { style, ..prefs });
            done += 1;
        },
    );
    Some(timer)
}

/// Diagnostics/screenshot overrides: `STUDY_NATIVE_STYLE`, `STUDY_NATIVE_PALETTE`,
/// `STUDY_NATIVE_THEME` (production ids). Applied through the normal writer (so they persist into
/// the isolated profile the harness uses, exactly as a user's choice would).
pub fn apply_env_overrides() {
    let Some(mut prefs) = prefs() else { return };
    if let Some(style) = std::env::var("STUDY_NATIVE_STYLE")
        .ok()
        .and_then(|v| AppStyle::from_production(&v))
    {
        prefs.style = style;
    }
    if let Some(palette) = std::env::var("STUDY_NATIVE_PALETTE")
        .ok()
        .and_then(|v| Palette::from_production(&v))
    {
        prefs.palette = palette;
    }
    if let Ok(theme) = std::env::var("STUDY_NATIVE_THEME") {
        prefs.theme = ThemeMode::from_production(Some(&theme));
    }
    // Stage 17's screenshot hook, kept working: `STUDY_NATIVE_DASHBOARD_DARK=0` = light.
    if std::env::var("STUDY_NATIVE_DASHBOARD_DARK").is_ok_and(|v| v == "0") {
        prefs.theme = ThemeMode::Light;
    }
    set_prefs(prefs);
    // `STUDY_NATIVE_PANEL=menu|themes|styles`: open the menu or the Theme/Style panel (screenshots).
    if let (Ok(panel), Some(Some(window))) = (
        std::env::var("STUDY_NATIVE_PANEL"),
        with_runtime(|rt| rt.window.upgrade()),
    ) {
        match panel.as_str() {
            "menu" => window.set_menu_open(true),
            "themes" => window.set_panel_view(0),
            "styles" => window.set_panel_view(1),
            _ => {}
        }
    }
}

/// `displayTime`'s 12/24-hour choice: what the Windows display language writes by default
/// (Chromium derives `hour12` from the browser locale, which follows the display language).
fn twelve_hour_clock() -> bool {
    if let Ok(v) = std::env::var("STUDY_NATIVE_12H") {
        return v == "1";
    }
    #[cfg(windows)]
    {
        crate::platform::win_util::ui_locale_prefers_twelve_hour()
    }
    #[cfg(not(windows))]
    {
        false
    }
}
