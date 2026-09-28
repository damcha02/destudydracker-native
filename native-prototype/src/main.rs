// Release builds get the Windows GUI subsystem (no console window); debug builds keep the
// console subsystem so `cargo run`/`cargo test` and any eprintln!/panic output stay visible
// during development. This is the standard Rust idiom for the split (the same pattern Tauri's
// own generated `main.rs` uses in `desktop/src-tauri/src/main.rs`). No effect on non-Windows
// targets. See docs/stage13-production-shell.md, "Windows GUI-subsystem handling".
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app_model;
mod dashboard;
mod map;
mod map_adapter;
mod persistence;
mod platform;
mod timer_controller;

use app_model::{format_clock, AppCommand, AppModel, TimerStatus};
use dashboard::{format_minutes, AxisMark, DashboardScenario};
use platform::error::StartupError;
use platform::{config, identity, logging, paths::AppPaths};
use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const RUNNING_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

slint::include_modules!();

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            report_fatal_startup_error(&error);
            std::process::ExitCode::FAILURE
        }
    }
}

/// All of startup up to and including the Slint event loop. Kept as one `Result`-returning
/// function (Stage 13 brief §9: "use `Result` propagation rather than widespread panics") so
/// every failure path — path resolution, logging, window/renderer creation — reports through the
/// same `StartupError` and the same fallback in `report_fatal_startup_error`, instead of a raw
/// panic that a release build's missing console would swallow.
fn run() -> Result<(), StartupError> {
    let app_paths = AppPaths::resolve()?;
    app_paths.ensure_created()?;
    let log_path = logging::init(&app_paths)?;
    logging::install_panic_hook();

    let runtime_config = config::RuntimeConfig::from_env();
    log::info!(
        "{} {} (by {}) starting; renderer={}; log file: {}",
        identity::DISPLAY_NAME,
        identity::version(),
        identity::ORGANIZATION,
        runtime_config.renderer.slint_backend_report(),
        log_path.display(),
    );
    config::log_active_benchmark_overrides();

    // Stage 15: the real, durable timer persistence adapter, replacing Stage 14's
    // `NullPersistencePort`. One small JSON file under the app's own data directory (never
    // production's - see `platform::paths`'s distinct app-id namespacing); if it holds a
    // snapshot from a previous run, the timer restores/recovers from it here, before the first
    // frame is ever shown. See docs/stage15-persistence-migration.md for the full design.
    let store_path = app_paths.data_dir.join("store.json");
    // Stage 15 diagnostic import: `STUDY_NATIVE_IMPORT_BACKUP=<path to a production backup.json>`
    // runs the inspect -> (optionally) commit pipeline against this app's own isolated native
    // store, before the timer restores from it - so an import that lands a timer section takes
    // effect on this same launch. Off unless set; never touches anything automatically. See
    // docs/stage15-persistence-migration.md, "Migration pipeline" and "Real production data
    // status" - this is the explicit, user-invoked action that section requires, not an automatic
    // migration.
    maybe_import_production_backup(
        &app_paths,
        &persistence::NativeStore::new(store_path.clone()),
    );
    let timer_port: Box<dyn timer_controller::TimerPersistencePort> = Box::new(
        persistence::FileTimerPersistencePort::new(persistence::NativeStore::new(store_path)),
    );
    let (mut model, startup_recovery_effects) = AppModel::with_timer_persistence(timer_port);
    if !startup_recovery_effects.is_empty() {
        // Stage 16 owns the session subsystem that will actually consume a recovered session
        // range; Stage 15 only guarantees the effect exists, is exactly-once, and is not lost -
        // logging it here is the honest current end of that pipeline, not a stand-in session UI.
        log::info!(
            "timer recovery on startup produced {} application effect(s): {startup_recovery_effects:?}",
            startup_recovery_effects.len()
        );
    }
    model.apply(AppCommand::MarkPresentationReady);
    let model = Rc::new(RefCell::new(model));
    let refresh_timer = Rc::new(Timer::default());

    let window = MainWindow::new()?;
    apply_startup_options(&window, &mut model.borrow_mut());
    // Stage 15 diagnostic hook, same family as STUDY_NATIVE_FRAME_STATS/STARTUP_REPORT: prints
    // the restored-then-startup-option-applied timer state to stdout once, with zero synthetic
    // input, so a real restart/kill-process persistence test can verify recovery from a launched-
    // and-exited process rather than from a screenshot. Deliberately placed after
    // `apply_startup_options` so it reflects any `STUDY_NATIVE_TIMER_MODE`/`_AUTOSTART` override
    // too, not just what was loaded from disk. See docs/stage15-persistence-migration.md,
    // "Restart/recovery" for how this was used.
    if std::env::var_os("STUDY_NATIVE_TIMER_STATE_REPORT").is_some() {
        let borrowed = model.borrow();
        let timer = borrowed.timer();
        let clock = borrowed.clock(Instant::now());
        println!(
            "TIMER_STATE mode={} status={:?} running={} remaining_secs={}",
            timer.selected_mode(),
            timer.status(),
            timer.is_running(),
            timer.remaining(clock).as_secs(),
        );
    }
    apply_model_to_window(&window, &model.borrow(), Instant::now());
    apply_dashboard(&window, &model.borrow());
    let map = map_adapter::new_controller(map_level_from_env());
    map_adapter::apply_all(&window, &mut map.borrow_mut());
    map_adapter::bind(&window, &map);
    let _bench_timer = std::env::var("STUDY_NATIVE_MAP_BENCH")
        .ok()
        .map(|spec| map_adapter::start_bench(&window, &map, &spec));
    bind_model_callbacks(&window, Rc::clone(&model), Rc::clone(&refresh_timer));
    let _diagnostics = install_diagnostics(&window);

    log::info!("first window created; entering the event loop");
    window.run()?;
    log::info!("event loop exited normally");
    Ok(())
}

/// Last-resort reporting for a startup failure. Always logs (if logging made it far enough to
/// initialize) and always writes to stderr (harmless even with no console attached); on Windows
/// also shows a native message box, since a release build's missing console means stderr alone
/// would otherwise be genuinely invisible to the user. Kept out of `study-tracker-core` and out
/// of the normal `run()` path — this only runs once, at the very end, on the way out.
fn report_fatal_startup_error(error: &StartupError) {
    let message = format!("{} failed to start:\n\n{error}", identity::DISPLAY_NAME);
    log::error!("{message}");
    eprintln!("{message}");
    #[cfg(windows)]
    platform::startup_error::show_fatal_error(identity::DISPLAY_NAME, &message);
}

static FRAMES_RENDERED: AtomicU64 = AtomicU64::new(0);
static TIMER_TICKS: AtomicU64 = AtomicU64::new(0);

/// Diagnostic hooks (off by default; used by the Windows benchmark scripts):
/// - `STUDY_NATIVE_STARTUP_REPORT=1` prints `FIRST_FRAME <ms since main>` once, after the first rendered frame.
/// - `STUDY_NATIVE_FRAME_STATS=1` prints `STATS <secs> frames=<n> ticks=<n>` every 10 s: frames actually rendered
///   and Rust timer-tick callbacks in that interval (shows how often the UI redraws / the model refreshes).
///
/// The returned timer must stay alive for the duration of the event loop.
fn install_diagnostics(window: &MainWindow) -> Option<Timer> {
    let startup = std::env::var_os("STUDY_NATIVE_STARTUP_REPORT").is_some();
    let stats = std::env::var_os("STUDY_NATIVE_FRAME_STATS").is_some();
    if !startup && !stats {
        return None;
    }
    let started = Instant::now();
    let mut reported = false;
    let result = window.window().set_rendering_notifier(move |state, _| {
        if matches!(state, slint::RenderingState::AfterRendering) {
            FRAMES_RENDERED.fetch_add(1, Ordering::Relaxed);
            if startup && !reported {
                reported = true;
                println!(
                    "FIRST_FRAME {:.1}",
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
        }
    });
    if let Err(error) = result {
        eprintln!("rendering notifier unavailable: {error}");
    }
    if !stats {
        return None;
    }
    let timer = Timer::default();
    let mut last = (0u64, 0u64);
    timer.start(TimerMode::Repeated, Duration::from_secs(10), move || {
        let now = (
            FRAMES_RENDERED.load(Ordering::Relaxed),
            TIMER_TICKS.load(Ordering::Relaxed),
        );
        println!(
            "STATS {:.0} frames={} ticks={}",
            started.elapsed().as_secs_f64(),
            now.0 - last.0,
            now.1 - last.1
        );
        last = now;
    });
    Some(timer)
}

/// Optional environment overrides so benchmarks and screenshots can start in a known state
/// without synthetic input: `STUDY_NATIVE_VIEW=timer|text|dashboard`,
/// `STUDY_NATIVE_POINTS=30|365|1000` (any count), `STUDY_NATIVE_SCENARIO=0..5`,
/// `STUDY_NATIVE_SIZE=WIDTHxHEIGHT` (logical pixels).
fn apply_startup_options(window: &MainWindow, model: &mut AppModel) {
    if let Ok(view) = std::env::var("STUDY_NATIVE_VIEW") {
        window.set_show_dashboard(view == "dashboard");
        window.set_show_text_spike(view == "text");
        window.set_show_map(view == "map");
    }
    if let Some(points) = std::env::var("STUDY_NATIVE_POINTS")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        model.apply(AppCommand::SetDashboardPoints(points));
    }
    if let Some(index) = std::env::var("STUDY_NATIVE_SCENARIO")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        model.apply(AppCommand::SetDashboardScenario(
            DashboardScenario::from_index(index),
        ));
    }
    if let Some((w, h)) = std::env::var("STUDY_NATIVE_SIZE").ok().and_then(|v| {
        let (w, h) = v.split_once('x')?;
        Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
    }) {
        window.window().set_size(slint::LogicalSize::new(w, h));
    }
    // Stage 14 benchmark hooks: let the Windows perf scripts (T14-0..T14-6, see
    // docs/stage14-timer-productionization.md) put the timer into a running state without any
    // synthetic mouse/keyboard input at all - starting the app is enough. Both are no-ops unless
    // set; `STUDY_NATIVE_TIMER_AUTOSTART` alone starts whatever mode is already selected.
    if let Some(index) = std::env::var("STUDY_NATIVE_TIMER_MODE")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        model.apply(AppCommand::SetMode(index));
    }
    if std::env::var_os("STUDY_NATIVE_TIMER_AUTOSTART").is_some() {
        let now = Instant::now();
        model.apply(AppCommand::Start(now));
        if std::env::var_os("STUDY_NATIVE_TIMER_AUTOPAUSE").is_some() {
            model.apply(AppCommand::Pause(now));
        }
    }
}

/// Stage 15's diagnostic production-backup import (see the call site in `run()` for the safety
/// reasoning). Reads and reports on `STUDY_NATIVE_IMPORT_BACKUP`'s file; commits the converted
/// timer into `native_store` unless `STUDY_NATIVE_IMPORT_DRY_RUN` is also set. Never touches the
/// source file, never runs unless explicitly requested, never does anything with `social` or any
/// other withheld/reserved section beyond reporting that it saw them (see
/// `persistence::migration`'s module docs).
fn maybe_import_production_backup(
    app_paths: &platform::paths::AppPaths,
    native_store: &persistence::NativeStore,
) {
    let Some(backup_path) = std::env::var_os("STUDY_NATIVE_IMPORT_BACKUP") else {
        return;
    };
    let backup_path = std::path::PathBuf::from(backup_path);
    let dry_run = std::env::var_os("STUDY_NATIVE_IMPORT_DRY_RUN").is_some();
    let backup_copy_dir = app_paths.data_dir.join("imported-backups");
    log::info!(
        "timer backup import requested: {} (dry_run={dry_run})",
        backup_path.display()
    );
    let (discovered, fields, timer_result) =
        match persistence::migration::inspect_import(&backup_path, &backup_copy_dir) {
            Ok(outcome) => outcome,
            Err(err) => {
                log::error!("import: could not read backup: {err}");
                return;
            }
        };
    log::info!(
        "import: source backed up (unmodified) to {}",
        discovered.source_copy_path.display()
    );
    for field in &fields {
        log::info!(
            "import: field {:?} classified as {:?} ({})",
            field.key,
            field.class,
            field.note
        );
    }
    let timer = match timer_result {
        Ok(timer) => timer,
        Err(err) => {
            log::error!("import: timer section could not be converted, nothing imported: {err}");
            return;
        }
    };
    log::info!(
        "import: timer section {}",
        if timer.is_some() {
            "converted successfully"
        } else {
            "absent from this backup"
        }
    );
    if dry_run {
        log::info!("import: STUDY_NATIVE_IMPORT_DRY_RUN is set, not committing anything");
        return;
    }
    match persistence::migration::commit_import(native_store, &discovered, fields, timer) {
        Ok(report) => log::info!(
            "import: committed = {}, timer_imported = {}, source = {}, source copy = {}, {} field(s) classified, {} warning(s)",
            report.committed,
            report.timer_imported,
            report.source_path.display(),
            report.source_copy_path.display(),
            report.fields.len(),
            report.warnings.len(),
        ),
        Err(err) => log::error!("import: commit failed, native destination left unchanged: {err}"),
    }
}

/// `STUDY_NATIVE_MAP_LEVEL=0..10` selects the initial map stress level (see `StressLevel::ALL`).
fn map_level_from_env() -> map::dataset::StressLevel {
    std::env::var("STUDY_NATIVE_MAP_LEVEL")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(map::dataset::StressLevel::from_index)
        .unwrap_or(map::dataset::StressLevel::World)
}

fn bind_model_callbacks(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    refresh_timer: Rc<Timer>,
) {
    let weak_window = window.as_weak();
    let window_handle = window.as_weak();
    let dispatch_model = Rc::clone(&model);
    let dispatch_refresh_timer = Rc::clone(&refresh_timer);
    let dispatch = move |command: AppCommand| {
        dispatch_model.borrow_mut().apply(command);
        let now = Instant::now();
        if let Some(window) = weak_window.upgrade() {
            apply_model_to_window(&window, &dispatch_model.borrow(), now);
            sync_refresh_timer(
                &window,
                Rc::clone(&dispatch_model),
                Rc::clone(&dispatch_refresh_timer),
            );
        }
    };
    let dispatch = Rc::new(dispatch);

    {
        let dispatch = Rc::clone(&dispatch);
        let model = Rc::clone(&model);
        window.on_toggle_timer(move || {
            let now = Instant::now();
            let command = if model.borrow().timer().is_running() {
                AppCommand::Pause(now)
            } else {
                AppCommand::Start(now)
            };
            dispatch(command);
        });
    }
    {
        let dispatch = Rc::clone(&dispatch);
        window.on_reset_timer(move || {
            dispatch(AppCommand::Reset);
        });
    }
    {
        let dispatch = Rc::clone(&dispatch);
        window.on_select_mode(move |index| {
            if index >= 0 {
                dispatch(AppCommand::SetMode(index as usize));
            }
        });
    }
    bind_dashboard_callbacks(window, Rc::clone(&model));
    if let Some(window) = window_handle.upgrade() {
        sync_refresh_timer(&window, model, refresh_timer);
    }
}

fn sync_refresh_timer(window: &MainWindow, model: Rc<RefCell<AppModel>>, refresh_timer: Rc<Timer>) {
    if !model.borrow().timer().is_running() {
        refresh_timer.stop();
        return;
    }
    if refresh_timer.running() {
        return;
    }

    let weak_window = window.as_weak();
    let timer_for_callback = Rc::clone(&refresh_timer);
    refresh_timer.start(TimerMode::Repeated, RUNNING_UPDATE_INTERVAL, move || {
        TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
        let now = Instant::now();
        model.borrow_mut().apply(AppCommand::Refresh(now));
        if let Some(window) = weak_window.upgrade() {
            // Stage 14 finding: Slint/the FemtoVG-Winit backend does not itself skip a repaint
            // just because the window is minimized - every scalar property write below
            // (`set_timer_text`, `set_timer_progress`, ...) still triggered a real render pass
            // each tick even while minimized, at ~2 fps and a small but real CPU cost, exactly
            // the class of waste Rule A already eliminated for unchanged *models*. Correctness
            // never depended on this push (it's already timestamp-derived - see `AppModel::clock`
            // and `study_tracker_core::timer`), so it's safe to skip entirely while minimized;
            // the very next tick after restore pushes fresh, correct values within one
            // `RUNNING_UPDATE_INTERVAL` (100 ms), matching Rule B in
            // docs/stage12_5-architecture-freeze.md, section 10.
            if !window.window().is_minimized() {
                apply_model_to_window(&window, &model.borrow(), now);
            }
            if !model.borrow().timer().is_running() {
                timer_for_callback.stop();
            }
        }
    });
}

fn apply_model_to_window(window: &MainWindow, model: &AppModel, now: Instant) {
    let timer = model.timer();
    let clock = model.clock(now);
    let remaining = timer.remaining(clock);
    let elapsed = timer.elapsed(clock);
    let duration = timer.duration();
    let selected_mode = timer.selected_mode();
    let selected = &model.modes()[selected_mode];

    window.set_app_title(model.title().into());
    window.set_status_text(model.status().into());
    window.set_timer_text(format_clock(remaining).into());
    window.set_status_label(timer.status_label().into());
    window.set_context_title("General focus".into());
    window.set_context_detail("Analysis II problem set · Next: exercise 7".into());
    window.set_selected_mode(selected_mode as i32);
    window.set_mode_label(selected.label().into());
    window.set_mode_detail(selected.detail().into());
    window.set_phase_tone(selected.tone().accent_index());
    window.set_running(timer.is_running());
    window.set_completed(timer.status() == TimerStatus::Completed);
    window.set_timer_progress(timer.progress_fraction(clock));
    window.set_remaining_label(format_duration_label(remaining).into());
    window.set_elapsed_label(format_duration_label(elapsed).into());
    window.set_total_label(format_duration_label(duration).into());

    let modes = model
        .modes()
        .iter()
        .enumerate()
        .map(|(index, mode)| TimerModeData {
            label: SharedString::from(mode.label()),
            detail: SharedString::from(mode.detail()),
            selected: index == selected_mode,
            enabled: !timer.is_running(),
            tone: mode.tone().accent_index(),
            index: index as i32,
        })
        .collect::<Vec<_>>();
    if !model_matches(&window.get_modes(), &modes) {
        window.set_modes(ModelRc::new(Rc::new(VecModel::from(modes))));
    }

    let notes = model
        .session_notes()
        .iter()
        .map(|note| SessionNoteData {
            title: SharedString::from(note.title()),
            detail: SharedString::from(note.detail()),
            minutes: note.minutes() as i32,
            confidence: note.confidence() as i32,
        })
        .collect::<Vec<_>>();
    if !model_matches(&window.get_session_notes(), &notes) {
        window.set_session_notes(ModelRc::new(Rc::new(VecModel::from(notes))));
    }
}

/// True when `current` already holds exactly `rows`. `apply_model_to_window` runs every 100 ms while the timer
/// runs; installing a fresh model object each time makes Slint rebuild the repeaters and repaint (2 frames per
/// tick, even minimized) although nothing changed (Stage 12 finding), so unchanged models are left alone.
fn model_matches<T: Clone + PartialEq + 'static>(current: &ModelRc<T>, rows: &[T]) -> bool {
    current.row_count() == rows.len()
        && rows
            .iter()
            .enumerate()
            .all(|(i, row)| current.row_data(i).as_ref() == Some(row))
}

const HISTORY_RANGES: [usize; 3] = [30, 365, 1_000];

/// Dashboard callbacks bypass the timer refresh path: they only touch dashboard properties,
/// and pure selection changes (hover / arrow keys) push a handful of scalars, never the models.
fn bind_dashboard_callbacks(window: &MainWindow, model: Rc<RefCell<AppModel>>) {
    fn selection(window: &MainWindow, model: &Rc<RefCell<AppModel>>, command: AppCommand) {
        model.borrow_mut().apply(command);
        apply_dashboard_selection(window, &model.borrow());
    }
    fn rebuild(window: &MainWindow, model: &Rc<RefCell<AppModel>>, command: AppCommand) {
        model.borrow_mut().apply(command);
        apply_dashboard(window, &model.borrow());
    }

    macro_rules! bind {
        ($setter:ident, $handler:expr) => {{
            let weak = window.as_weak();
            let model = Rc::clone(&model);
            window.$setter(move |arg| {
                if let Some(window) = weak.upgrade() {
                    #[allow(clippy::redundant_closure_call)]
                    ($handler)(&window, &model, arg);
                }
            });
        }};
    }

    bind!(on_weekly_select, |w: &MainWindow,
                             m: &Rc<RefCell<AppModel>>,
                             i: i32| {
        selection(w, m, AppCommand::SelectWeekday(i.max(0) as usize))
    });
    bind!(on_weekly_step, |w: &MainWindow,
                           m: &Rc<RefCell<AppModel>>,
                           d: i32| {
        selection(w, m, AppCommand::StepWeekday(d))
    });
    bind!(on_history_hover, |w: &MainWindow,
                             m: &Rc<RefCell<AppModel>>,
                             f: f32| {
        selection(w, m, AppCommand::SelectHistoryFraction(f))
    });
    bind!(on_history_step, |w: &MainWindow,
                            m: &Rc<RefCell<AppModel>>,
                            d: i32| {
        selection(w, m, AppCommand::StepHistory(d))
    });
    {
        let weak = window.as_weak();
        let model = Rc::clone(&model);
        window.on_history_plot_resized(move |width, height| {
            if let Some(window) = weak.upgrade() {
                model
                    .borrow_mut()
                    .apply(AppCommand::ResizeHistoryPlot(width, height));
                let model = model.borrow();
                let history = &model.dashboard().history;
                window.set_history_line(history.line_commands.as_str().into());
                window.set_history_area(history.area_commands.as_str().into());
            }
        });
    }
    bind!(
        on_set_history_range,
        |w: &MainWindow, m: &Rc<RefCell<AppModel>>, i: i32| {
            let points = HISTORY_RANGES[(i.max(0) as usize).min(HISTORY_RANGES.len() - 1)];
            rebuild(w, m, AppCommand::SetDashboardPoints(points))
        }
    );
    bind!(on_set_scenario, |w: &MainWindow,
                            m: &Rc<RefCell<AppModel>>,
                            i: i32| {
        rebuild(
            w,
            m,
            AppCommand::SetDashboardScenario(DashboardScenario::from_index(i.max(0) as usize)),
        )
    });
}

fn model_rc<T: Clone + 'static>(items: Vec<T>) -> ModelRc<T> {
    ModelRc::new(Rc::new(VecModel::from(items)))
}

fn axis_marks(marks: &[AxisMark]) -> ModelRc<AxisMarkData> {
    model_rc(
        marks
            .iter()
            .map(|mark| AxisMarkData {
                position: mark.position,
                label: SharedString::from(mark.label.as_str()),
            })
            .collect(),
    )
}

/// Pushes the whole prepared dashboard snapshot. Called at startup and when the data set changes.
fn apply_dashboard(window: &MainWindow, model: &AppModel) {
    let dashboard = model.dashboard();

    window.set_dashboard_cards(model_rc(
        dashboard
            .summary_cards
            .iter()
            .map(|card| SummaryCardData {
                label: card.label.as_str().into(),
                value: card.value.as_str().into(),
                detail: card.detail.as_str().into(),
                tone: card.tone,
            })
            .collect(),
    ));

    let weekly = &dashboard.weekly;
    window.set_weekly_bars(model_rc(
        weekly
            .bars
            .iter()
            .map(|bar| WeeklyBarData {
                day: bar.day.as_str().into(),
                detail: bar.detail.as_str().into(),
                value: bar.value.as_str().into(),
                fraction: bar.fraction,
                tone: bar.tone,
                is_today: bar.is_today,
            })
            .collect(),
    ));
    window.set_weekly_ticks(axis_marks(&weekly.y_ticks));
    window.set_weekly_subtitle(
        format!(
            "Last 7 days · {} total",
            format_minutes(weekly.total_minutes)
        )
        .into(),
    );
    window.set_weekly_summary(
        format!(
            "Seven days, {} in total. Use left and right arrow keys to inspect a day.",
            format_minutes(weekly.total_minutes)
        )
        .into(),
    );

    let history = &dashboard.history;
    window.set_history_line(history.line_commands.as_str().into());
    window.set_history_area(history.area_commands.as_str().into());
    window.set_history_y_ticks(axis_marks(&history.y_ticks));
    window.set_history_x_marks(axis_marks(&history.x_marks));
    window.set_history_count(history.points.len() as i32);
    window.set_history_subtitle(
        format!(
            "{} days · {} total · peak {}",
            history.points.len(),
            format_minutes(history.total_minutes),
            format_minutes(history.peak_minutes)
        )
        .into(),
    );
    window.set_history_summary(
        format!(
            "{} points, {} active days, average {} per active day",
            history.points.len(),
            history.active_days,
            format_minutes(
                history
                    .total_minutes
                    .checked_div(history.active_days)
                    .unwrap_or(0)
            ),
        )
        .into(),
    );
    window.set_history_range_index(
        HISTORY_RANGES
            .iter()
            .position(|n| *n == model.dashboard_points())
            .unwrap_or(0) as i32,
    );
    window.set_scenario_index(dashboard.scenario.index() as i32);

    let courses = &dashboard.courses;
    window.set_dashboard_courses(model_rc(
        courses
            .courses
            .iter()
            .map(|course| CourseRowData {
                name: course.name.as_str().into(),
                detail: course.detail.as_str().into(),
                minutes_label: format_minutes(course.minutes).into(),
                percent: course.percent as i32,
                fraction: course.fraction,
                start: course.start,
                health: course.health as i32,
                tone: course.tone,
            })
            .collect(),
    ));
    window.set_courses_subtitle(
        format!(
            "{} courses · {} tracked",
            courses.courses.len(),
            format_minutes(courses.total_minutes)
        )
        .into(),
    );
    window.set_courses_total_label(format_minutes(courses.total_minutes).into());

    window.set_dashboard_sessions(model_rc(
        dashboard
            .sessions
            .iter()
            .map(|session| SessionRowData {
                course: session.course.as_str().into(),
                kind: session.kind.as_str().into(),
                when: session.when.as_str().into(),
                duration: format_minutes(session.minutes).into(),
                tone: session.tone,
            })
            .collect(),
    ));
    window.set_sessions_subtitle(
        format!("{} sessions · scroll for more", dashboard.sessions.len()).into(),
    );

    apply_dashboard_selection(window, model);
}

/// Cheap update for hover / keyboard selection: a few scalar properties, no model rebuilds.
fn apply_dashboard_selection(window: &MainWindow, model: &AppModel) {
    let dashboard = model.dashboard();

    let weekly = &dashboard.weekly;
    window.set_weekly_selected(weekly.selected as i32);
    window.set_weekly_selected_text(
        weekly
            .bars
            .get(weekly.selected)
            .map(|bar| format!("Selected: {}", bar.detail))
            .unwrap_or_default()
            .into(),
    );

    let history = &dashboard.history;
    match history
        .selected
        .and_then(|i| Some((history.points.get(i)?, history.positions.get(i)?)))
    {
        Some((point, (x, y))) => {
            window.set_history_has_selection(true);
            window.set_history_selected_x(*x);
            window.set_history_selected_y(*y);
            window.set_history_selected_title(point.label.as_str().into());
            window.set_history_selected_value(format_minutes(point.minutes).into());
        }
        None => {
            window.set_history_has_selection(false);
            window.set_history_selected_title("".into());
            window.set_history_selected_value("".into());
        }
    }
}

fn format_duration_label(duration: Duration) -> String {
    let seconds = duration.as_secs();
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    if minutes >= 60 {
        format!("{}h {:02}m", minutes / 60, minutes % 60)
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_of(rows: &[i32]) -> ModelRc<i32> {
        ModelRc::new(Rc::new(VecModel::from(rows.to_vec())))
    }

    /// The 100 ms timer refresh must not replace unchanged models (Stage 12: doing so repainted at 20 fps).
    #[test]
    fn model_matches_only_when_rows_are_identical() {
        let current = model_of(&[1, 2, 3]);
        assert!(model_matches(&current, &[1, 2, 3]));
        assert!(!model_matches(&current, &[1, 2, 4]), "changed row");
        assert!(!model_matches(&current, &[1, 2]), "shorter");
        assert!(!model_matches(&current, &[1, 2, 3, 4]), "longer");
        assert!(model_matches(&model_of(&[]), &[]), "empty equals empty");
    }
}
