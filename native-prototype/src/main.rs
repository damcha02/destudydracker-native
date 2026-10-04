// Release builds get the Windows GUI subsystem (no console window); debug builds keep the
// console subsystem so `cargo run`/`cargo test` and any eprintln!/panic output stay visible
// during development. This is the standard Rust idiom for the split (the same pattern Tauri's
// own generated `main.rs` uses in `desktop/src-tauri/src/main.rs`). No effect on non-Windows
// targets. See docs/stage13-production-shell.md, "Windows GUI-subsystem handling".
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod academic_controller;
mod app_appearance;
mod app_break_games;
mod app_break_room;
mod app_model;
mod app_net;
#[cfg(windows)]
mod app_platform;
mod app_skribbl;
mod app_social;
#[cfg(windows)]
mod app_updater;
mod appearance_view;
mod break_room_art;
mod break_room_controller;
mod break_room_flags;
mod break_room_view;
mod dashboard_view;
mod game_tokens;
mod image_cache;
mod map;
mod map_adapter;
mod net;
mod net_jobs;
mod persistence;
mod platform;
mod sakura_controller;
mod session_service;
mod skribbl_canvas;
mod skribbl_controller;
mod social_controller;
mod synthetic_dataset;
mod timer_controller;
mod travle_map;
mod wabi_view;

use app_model::{format_clock, AppCommand, AppModel, TimerStatus};
use dashboard_view::{ChronoLocalClock, DashboardController, DashboardUiState};
use platform::error::StartupError;
use platform::{config, identity, logging, paths::AppPaths};
use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use study_tracker_core::academic::CalendarEntryId;
use study_tracker_core::dashboard::FocusRange;
use study_tracker_core::timer::WallTimestamp;

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
    // Stage 18: exactly one primary instance per profile (data directory), decided *before* the
    // log file, the store or anything else that could be written is opened - a second launch must
    // never become a second writer. A secondary asks the primary to show itself and exits.
    #[cfg(windows)]
    let instance_names =
        platform::single_instance::names_for(identity::APP_ID, &app_paths.data_dir);
    #[cfg(windows)]
    let _instance_guard = match platform::single_instance::acquire(&instance_names) {
        platform::single_instance::Acquisition::Primary(guard) => Some(guard),
        platform::single_instance::Acquisition::Unguarded => None,
        platform::single_instance::Acquisition::Secondary { activated } => {
            if std::env::var_os("STUDY_NATIVE_INSTANCE_REPORT").is_some() {
                println!("SECONDARY activated={activated}");
            }
            return Ok(());
        }
    };
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
    let timer_port: Box<dyn timer_controller::TimerPersistencePort> =
        Box::new(persistence::FileTimerPersistencePort::new(
            persistence::NativeStore::new(store_path.clone()),
        ));
    // Stage 16: the real, durable academic/planner persistence adapter, replacing
    // `NullAcademicPersistencePort`. Reads/writes the same store file's `academic` section - see
    // docs/stage16-academic-domain.md, "Persistence schema".
    let academic_port: Box<dyn academic_controller::AcademicPersistencePort> =
        Box::new(persistence::FileAcademicPersistencePort::new(
            persistence::NativeStore::new(store_path.clone()),
        ));
    // Stage 19: the persisted style / palette / light-dark preference (same store file).
    let preferences = persistence::PreferencesController::load(Box::new(
        persistence::FilePreferencesPort::new(persistence::NativeStore::new(store_path.clone())),
    ));
    log::info!("appearance preference loaded: {:?}", preferences.prefs());
    let (mut model, startup_recovery_effects) =
        AppModel::with_timer_persistence(timer_port, academic_port);
    if !startup_recovery_effects.is_empty() {
        // A recovered session range was already routed into a real, persisted StudySession by
        // `with_timer_persistence` itself before this line ever runs (see that function's own
        // doc comment) - this log line just reports that it happened.
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
    let dashboard = Rc::new(RefCell::new(DashboardController::new(
        dashboard_ui_from_env(),
    )));
    app_appearance::install(
        &window,
        Rc::clone(&model),
        Rc::clone(&dashboard),
        preferences,
    );
    app_appearance::apply_env_overrides();
    // Stage 20: the Break Room (its own section of the same store file).
    // Stage 22a: Social + Daily Skribbl. Configuring the network starts no thread and opens no
    // connection; nothing is requested without an explicit account (see app_social.rs).
    app_social::set_store_path(&store_path);
    app_social::install(&window, Rc::clone(&model), &store_path, &app_paths.data_dir);
    app_skribbl::install(&window);
    app_break_room::install(&window, Rc::clone(&model), &store_path);
    apply_model_to_window(&window, &model.borrow(), Instant::now());
    refresh_dashboard(&window, &model.borrow(), &mut dashboard.borrow_mut());
    dashboard_report_if_requested(&dashboard.borrow());
    // The Stage 11 map lab (a diagnostic surface) builds its data set only once it is shown
    // (Stage 21: no map geometry is parsed at startup).
    let _bench_timer = std::env::var("STUDY_NATIVE_MAP_BENCH")
        .ok()
        .and_then(|spec| {
            let map = map_adapter::ensure(&window, true)?;
            Some(map_adapter::start_bench(&window, &map, &spec))
        });
    map_adapter::ensure(&window, false);
    bind_model_callbacks(
        &window,
        Rc::clone(&model),
        Rc::clone(&refresh_timer),
        Rc::clone(&dashboard),
    );
    bind_dashboard_callbacks(&window, Rc::clone(&model), Rc::clone(&dashboard));
    let _rollover_timer =
        start_date_rollover_check(&window, Rc::clone(&model), Rc::clone(&dashboard));
    let _nav_stress = start_nav_stress(&window, Rc::clone(&model), Rc::clone(&dashboard));
    let _theme_stress = app_appearance::start_theme_stress();
    let _input_script = install_input_script(&window);
    let _break_stress = app_break_room::start_stress(&window);
    let _travle_stress = app_break_games::start_travle_stress(&window);
    let _diagnostics = install_diagnostics(&window);

    log::info!("first window created; entering the event loop");
    #[cfg(windows)]
    {
        match app_platform::install(
            &window,
            Rc::clone(&model),
            Rc::clone(&dashboard),
            &instance_names,
            &app_paths.data_dir,
        ) {
            Ok(()) => {}
            Err(error) => {
                log::warn!("platform integration unavailable (tray/notifications): {error}")
            }
        }
        app_updater::install(&window, app_paths.data_dir.clone());
        app_platform::after_timer_activity(); // tray starts in sync with a restored timer
                                              // The tray keeps the process alive with the window hidden, so the loop must run until an
                                              // explicit quit instead of until the last window closes (`window.run()`).
        window.show()?;
        app_appearance::after_window_shown();
        slint::run_event_loop_until_quit()?;
        app_platform::shutdown();
    }
    #[cfg(not(windows))]
    window.run()?;
    app_skribbl::close();
    app_social::shutdown();
    app_net::shutdown();
    if std::env::var_os("STUDY_NATIVE_NET_AUDIT").is_some() {
        println!("NET {}", app_net::report());
    }
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
/// Sakura clock ticks that wrote a new frame (Stage 19 diagnostics).
pub static SAKURA_FRAMES: AtomicU64 = AtomicU64::new(0);

/// Diagnostic hooks (off by default; used by the Windows benchmark scripts):
/// - `STUDY_NATIVE_INPUT` also takes `drag:x1,y1,x2,y2,steps` (Stage 22a Skribbl stress).
/// - `STUDY_NATIVE_STARTUP_REPORT=1` prints `FIRST_FRAME <ms since main>` once, after the first rendered frame.
/// - `STUDY_NATIVE_FRAME_STATS=1` prints `STATS <secs> frames=<n> ticks=<n>` every 10 s: frames actually rendered
///   and Rust timer-tick callbacks in that interval (shows how often the UI redraws / the model refreshes).
///
/// The returned timer must stay alive for the duration of the event loop.
static INPUT_SCRIPT_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// `STUDY_NATIVE_INPUT="click:X,Y;key:a;key:Return;text:slate;wait:500"` (Stage 20 diagnostics,
/// off by default): replays pointer clicks and key presses through Slint's own event dispatch -
/// hit testing, focus and key routing included - one step every 120 ms after a 1.5 s settle, so
/// interaction paths can be checked on a machine with no input-synthesis tool. A pending
/// `STUDY_NATIVE_SNAPSHOT` waits for the script to finish.
fn install_input_script(window: &MainWindow) -> Option<Timer> {
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    let script = std::env::var("STUDY_NATIVE_INPUT").ok()?;
    INPUT_SCRIPT_DONE.store(false, Ordering::Relaxed);
    let mut steps: std::collections::VecDeque<String> =
        script.split(';').map(str::to_string).collect();
    let weak = window.as_weak();
    let started = Instant::now();
    let mut wait_until = Instant::now();
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(120), move || {
        let now = Instant::now();
        if now.duration_since(started) < Duration::from_millis(1500) || now < wait_until {
            return;
        }
        let Some(w) = weak.upgrade() else { return };
        let Some(step) = steps.pop_front() else {
            INPUT_SCRIPT_DONE.store(true, Ordering::Relaxed);
            w.window().request_redraw();
            return;
        };
        let (kind, arg) = step.split_once(':').unwrap_or((step.as_str(), ""));
        let win = w.window();
        let key = |text: slint::SharedString| {
            win.dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
            win.dispatch_event(WindowEvent::KeyReleased { text });
        };
        match kind {
            "click" => {
                if let Some((x, y)) = arg
                    .split_once(',')
                    .and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)))
                {
                    let position = slint::LogicalPosition::new(x, y);
                    win.dispatch_event(WindowEvent::PointerMoved { position });
                    win.dispatch_event(WindowEvent::PointerPressed {
                        position,
                        button: PointerEventButton::Left,
                    });
                    win.dispatch_event(WindowEvent::PointerReleased {
                        position,
                        button: PointerEventButton::Left,
                    });
                }
            }
            // `drag:x1,y1,x2,y2,steps` (Stage 22a Skribbl stress): press, `steps` moves, release
            "drag" => {
                let v: Vec<f32> = arg.split(',').filter_map(|n| n.parse().ok()).collect();
                if let [x1, y1, x2, y2, steps] = v[..] {
                    let steps = steps.max(1.0) as u32;
                    let at = |t: f32| {
                        slint::LogicalPosition::new(x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
                    };
                    win.dispatch_event(WindowEvent::PointerMoved { position: at(0.0) });
                    win.dispatch_event(WindowEvent::PointerPressed {
                        position: at(0.0),
                        button: PointerEventButton::Left,
                    });
                    for i in 1..=steps {
                        win.dispatch_event(WindowEvent::PointerMoved {
                            position: at(i as f32 / steps as f32),
                        });
                    }
                    win.dispatch_event(WindowEvent::PointerReleased {
                        position: at(1.0),
                        button: PointerEventButton::Left,
                    });
                }
            }
            "key" => key(match arg {
                "Return" => Key::Return.into(),
                "Backspace" => Key::Backspace.into(),
                "Escape" => Key::Escape.into(),
                "Right" => Key::RightArrow.into(),
                "Left" => Key::LeftArrow.into(),
                other => other.into(),
            }),
            "text" => arg.chars().for_each(|c| key(c.to_string().into())),
            "wait" => wait_until = now + Duration::from_millis(arg.parse().unwrap_or(0)),
            _ => {}
        }
        println!("INPUT {step}");
    });
    Some(timer)
}

fn install_diagnostics(window: &MainWindow) -> Option<Timer> {
    if let Ok(path) = std::env::var("STUDY_NATIVE_SNAPSHOT") {
        install_snapshot(window, path);
        return None;
    }
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
    let mut last = (0u64, 0u64, 0u64);
    timer.start(TimerMode::Repeated, Duration::from_secs(10), move || {
        let now = (
            FRAMES_RENDERED.load(Ordering::Relaxed),
            TIMER_TICKS.load(Ordering::Relaxed),
            SAKURA_FRAMES.load(Ordering::Relaxed),
        );
        println!(
            "STATS {:.0} frames={} ticks={} sakura_ticks={} {} {} {} {} {}",
            started.elapsed().as_secs_f64(),
            now.0 - last.0,
            now.1 - last.1,
            now.2 - last.2,
            app_appearance::sakura_report(),
            app_break_room::report(),
            app_net::report(),
            app_social::report(),
            app_skribbl::report(),
        );
        last = now;
    });
    Some(timer)
}

/// `STUDY_NATIVE_SNAPSHOT=<file.png>` (Stage 20 parity captures, off by default): once the window
/// has rendered `STUDY_NATIVE_SNAPSHOT_FRAMES` frames (default 3) after
/// `STUDY_NATIVE_SNAPSHOT_DELAY_MS` (default 1500), saves the window's pixels as a PNG and quits.
/// The snapshot is taken inside the rendering notifier: on Wayland femtovg's back buffer is only
/// valid there.
fn install_snapshot(window: &MainWindow, path: String) {
    let delay = std::env::var("STUDY_NATIVE_SNAPSHOT_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1500u64);
    let started = Instant::now();
    let weak = window.as_weak();
    let mut armed_frames = 0u32;
    let result = window.window().set_rendering_notifier(move |state, _| {
        if !matches!(state, slint::RenderingState::AfterRendering) {
            return;
        }
        let Some(w) = weak.upgrade() else { return };
        if started.elapsed() < Duration::from_millis(delay)
            || !INPUT_SCRIPT_DONE.load(Ordering::Relaxed)
        {
            w.window().request_redraw();
            return;
        }
        armed_frames += 1;
        if armed_frames < 3 {
            w.window().request_redraw();
            return;
        }
        if armed_frames > 3 {
            return;
        }
        match w.window().take_snapshot() {
            Ok(shot) => {
                let written = std::fs::File::create(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|file| {
                        let mut enc = png::Encoder::new(
                            std::io::BufWriter::new(file),
                            shot.width(),
                            shot.height(),
                        );
                        enc.set_color(png::ColorType::Rgba);
                        enc.set_depth(png::BitDepth::Eight);
                        enc.write_header()
                            .and_then(|mut wr| wr.write_image_data(shot.as_bytes()))
                            .map_err(|e| e.to_string())
                    });
                println!(
                    "SNAPSHOT {path} {}x{} {written:?}",
                    shot.width(),
                    shot.height()
                );
            }
            Err(error) => println!("SNAPSHOT failed: {error}"),
        }
        let _ = slint::quit_event_loop();
    });
    if let Err(error) = result {
        eprintln!("rendering notifier unavailable: {error}");
    }
}

/// Optional environment overrides so benchmarks and screenshots can start in a known state
/// without synthetic input: `STUDY_NATIVE_VIEW=timer|text|dashboard`,
/// `STUDY_NATIVE_SIZE=WIDTHxHEIGHT` (logical pixels). Dashboard-specific ones (Stage 17):
/// `STUDY_NATIVE_DASHBOARD_LAYOUT=quiet|full`, `STUDY_NATIVE_DASHBOARD_RANGE=week|7|14|30|60|365`.
fn apply_startup_options(window: &MainWindow, model: &mut AppModel) {
    if let Ok(view) = std::env::var("STUDY_NATIVE_VIEW") {
        window.set_show_dashboard(view == "dashboard");
        window.set_show_text_spike(view == "text");
        window.set_show_map(view == "map");
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
    // Stage 16 diagnostic hooks: exercise the real Course/Semester/Task/Exam CRUD command path
    // (the same one a future interactive Planner form would call) through real, persisted
    // `AcademicState` - without adding synthetic mouse/keyboard input or a new interactive form
    // yet (see docs/stage16-academic-domain.md, "UI integration"). Fixed, well-known ids so
    // `STUDY_NATIVE_REMOVE_DEMO_COURSE` can target exactly what `_ADD_` created; both off unless
    // set.
    if std::env::var_os("STUDY_NATIVE_ADD_DEMO_COURSE").is_some() {
        let now = Instant::now();
        let semester_id = study_tracker_core::academic::SemesterId::new("demo-semester");
        let course_id = study_tracker_core::academic::CourseId::new("demo-course");
        model.apply(AppCommand::AddSemester {
            id: semester_id.clone(),
            name: "Fall 2026".to_string(),
            created_at: now,
        });
        model.apply(AppCommand::AddCourse {
            id: course_id,
            semester_id,
            name: "Analysis II".to_string(),
            color: "blue".to_string(),
            created_at: now,
        });
    }
    if std::env::var_os("STUDY_NATIVE_REMOVE_DEMO_COURSE").is_some() {
        model.apply(AppCommand::RemoveCourse(
            study_tracker_core::academic::CourseId::new("demo-course"),
        ));
    }
    if std::env::var_os("STUDY_NATIVE_REMOVE_DEMO_SEMESTER").is_some() {
        model.apply(AppCommand::RemoveSemester(
            study_tracker_core::academic::SemesterId::new("demo-semester"),
        ));
    }
    // Stage 16 performance-measurement hook: loads a large, deterministic, entirely fabricated
    // "heavy but plausible student profile" (see synthetic_dataset.rs for exact counts and
    // rationale) and persists it, so a *later*, separate launch measures real cold-start
    // load/parse cost at that scale (S16-P5 - see docs/stage16-academic-domain.md). Off unless
    // set; never runs automatically.
    if std::env::var_os("STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA").is_some() {
        let state = synthetic_dataset::build_synthetic_academic_state();
        log::info!(
            "synthetic dataset: generated {} semester(s), {} course(s), {} task(s), {} exam(s), {} session(s), {} timetable event(s), {} holiday(s), {} daily todo(s), {} calendar entr(y/ies)",
            state.semesters.len(),
            state.courses.len(),
            state.tasks.len(),
            state.exams.len(),
            state.sessions.len(),
            state.timetable_events.len(),
            state.holidays.len(),
            state.daily_todos.len(),
            state.calendar_entries.len(),
        );
        model.academic_mut().replace_all(state);
    }
    // Stage 16 diagnostic hook, same family as STUDY_NATIVE_TIMER_STATE_REPORT: prints real
    // academic-domain counts to stdout once, with zero synthetic input.
    if std::env::var_os("STUDY_NATIVE_ACADEMIC_STATE_REPORT").is_some() {
        let state = model.academic().state();
        println!(
            "ACADEMIC_STATE semesters={} courses={} tasks={} exams={} sessions={} lifetime_minutes={}",
            state.semesters.len(),
            state.courses.len(),
            state.tasks.len(),
            state.exams.len(),
            state.sessions.len(),
            state.lifetime_study_minutes,
        );
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
    let (discovered, fields, timer_result, academic, academic_warnings) =
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
    for warning in &academic_warnings {
        log::warn!("import: academic conversion - {warning}");
    }
    log::info!(
        "import: academic section converted: {} semester(s), {} course(s), {} task(s), {} exam(s), {} session(s), {} timetable event(s), {} holiday(s), {} daily todo(s), {} calendar entr(y/ies)",
        academic.semesters.len(),
        academic.courses.len(),
        academic.tasks.len(),
        academic.exams.len(),
        academic.sessions.len(),
        academic.timetable_events.len(),
        academic.holidays.len(),
        academic.daily_todos.len(),
        academic.calendar_entries.len(),
    );
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
    match persistence::migration::commit_import(native_store, &discovered, fields, timer, academic)
    {
        Ok(report) => log::info!(
            "import: committed = {}, timer_imported = {}, appearance_imported = {}, break_room_imported = {}, academic = {:?}, source = {}, source copy = {}, {} field(s) classified, {} warning(s)",
            report.committed,
            report.timer_imported,
            report.appearance_imported,
            report.break_room_imported,
            report.academic_summary,
            report.source_path.display(),
            report.source_copy_path.display(),
            report.fields.len(),
            report.warnings.len(),
        ),
        Err(err) => log::error!("import: commit failed, native destination left unchanged: {err}"),
    }
}

/// `STUDY_NATIVE_MAP_LEVEL=0..10` selects the initial map stress level (see `StressLevel::ALL`).
pub fn map_level_from_env() -> map::dataset::StressLevel {
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
    dashboard: Rc<RefCell<DashboardController>>,
) {
    let weak_window = window.as_weak();
    let window_handle = window.as_weak();
    let dispatch_model = Rc::clone(&model);
    let dispatch_dashboard = Rc::clone(&dashboard);
    let dispatch_refresh_timer = Rc::clone(&refresh_timer);
    let dispatch = move |command: AppCommand| {
        dispatch_model.borrow_mut().apply(command);
        let now = Instant::now();
        if let Some(window) = weak_window.upgrade() {
            apply_model_to_window(&window, &dispatch_model.borrow(), now);
            refresh_dashboard_if_changed(
                &window,
                &dispatch_model.borrow(),
                &mut dispatch_dashboard.borrow_mut(),
            );
            sync_refresh_timer(
                &window,
                Rc::clone(&dispatch_model),
                Rc::clone(&dispatch_refresh_timer),
                Rc::clone(&dispatch_dashboard),
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
        // Stage 19: Wabi-Sabi's "LOG AND CLOSE" / Quiet mode's "DONE, LOG IT".
        let dispatch = Rc::clone(&dispatch);
        window.on_wabi_timer_log(move || dispatch(AppCommand::CompleteManually(Instant::now())));
    }
    {
        let dispatch = Rc::clone(&dispatch);
        window.on_select_mode(move |index| {
            if index >= 0 {
                dispatch(AppCommand::SetMode(index as usize));
            }
        });
    }
    if let Some(window) = window_handle.upgrade() {
        sync_refresh_timer(&window, model, refresh_timer, dashboard);
    }
}

fn sync_refresh_timer(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    refresh_timer: Rc<Timer>,
    dashboard: Rc<RefCell<DashboardController>>,
) {
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
            #[cfg(windows)]
            let hidden = app_platform::is_hidden_to_tray();
            #[cfg(not(windows))]
            let hidden = false;
            if !window.window().is_minimized() && !hidden {
                apply_model_to_window(&window, &model.borrow(), now);
            }
            // A Timer completion inside this tick adds a StudySession (academic revision bump);
            // the Dashboard must reflect it without a restart - even while minimized, since the
            // first frame after restore must already be correct. One integer comparison per tick;
            // nothing is recomputed unless the revision really changed.
            refresh_dashboard_if_changed(&window, &model.borrow(), &mut dashboard.borrow_mut());
            #[cfg(windows)]
            app_platform::after_timer_activity();
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
    app_appearance::refresh_wabi_timer(window, now);
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

/// Initial Dashboard UI state, with diagnostic overrides so screenshots/benchmarks can start in a
/// known layout without synthetic input (Stage 17).
fn dashboard_ui_from_env() -> DashboardUiState {
    let mut ui = DashboardUiState::default();
    if let Ok(layout) = std::env::var("STUDY_NATIVE_DASHBOARD_LAYOUT") {
        ui.full = layout.eq_ignore_ascii_case("full");
    }
    if let Ok(range) = std::env::var("STUDY_NATIVE_DASHBOARD_RANGE") {
        ui.range = match range.as_str() {
            "7" => FocusRange::Days(7),
            "14" => FocusRange::Days(14),
            "30" => FocusRange::Days(30),
            "60" => FocusRange::Days(60),
            "365" => FocusRange::Days(365),
            _ => FocusRange::Week,
        };
    }
    ui
}

/// The wall clock the Dashboard derives "today" from. `STUDY_NATIVE_NOW=<RFC 3339 instant>` (a
/// diagnostic/screenshot hook, off by default) pins it so a synthetic fixture dated relative to a
/// fixed day renders identically no matter when the comparison is run.
fn wall_now() -> WallTimestamp {
    static PINNED: std::sync::OnceLock<Option<i64>> = std::sync::OnceLock::new();
    let pinned = PINNED.get_or_init(|| {
        std::env::var("STUDY_NATIVE_NOW")
            .ok()
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(&v).ok())
            .map(|dt| dt.timestamp_millis())
    });
    if let Some(millis) = pinned {
        return WallTimestamp::from_unix_millis(*millis);
    }
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64);
    WallTimestamp::from_unix_millis(millis)
}

/// Recomputes (if the academic revision/date/range changed) and pushes the Dashboard data. The
/// only place that writes the Dashboard properties, so a view that is merely *shown* never
/// writes anything.
fn refresh_dashboard(window: &MainWindow, model: &AppModel, dashboard: &mut DashboardController) {
    let clock = ChronoLocalClock;
    let started = Instant::now();
    let metrics_before = dashboard.stats().metrics;
    dashboard.sync(
        model.academic().state(),
        model.academic().revision(),
        wall_now(),
        &clock,
    );
    push_dashboard(window, dashboard, &clock);
    app_appearance::after_dashboard_refresh(window, model, dashboard);
    app_break_room::after_academic_change(window, model);
    app_social::after_academic_change(window, model);
    if dashboard.stats().metrics != metrics_before {
        // One line per real metrics recomputation (never per Timer tick): the evidence the Stage 17 runtime
        // checks use for "no continuous recomputation" and "Timer completion refreshes the Dashboard".
        if let Some(m) = dashboard.metrics() {
            log::info!(
                "dashboard: recomputed (academic revision {}, today_minutes={}, streak={}, sessions_minutes_lifetime={}) in {:?}; totals so far {:?}",
                model.academic().revision(),
                m.today_minutes,
                m.streak_days,
                m.lifetime_minutes,
                started.elapsed(),
                dashboard.stats(),
            );
        }
    }
}

fn push_dashboard(window: &MainWindow, dashboard: &DashboardController, clock: &ChronoLocalClock) {
    window.set_fn_data(dashboard.data(clock));
    let (value, detail) = dashboard.sidebar_text();
    window.set_sidebar_today_value(value.into());
    window.set_sidebar_today_detail(detail.into());
}

/// Called after every Timer command/tick: a single integer comparison unless a session was just
/// added (or another academic mutation happened), in which case the Dashboard refreshes - the
/// "Timer -> StudySession -> AcademicState -> Dashboard" path, with no widget notified directly.
fn refresh_dashboard_if_changed(
    window: &MainWindow,
    model: &AppModel,
    dashboard: &mut DashboardController,
) {
    if !dashboard.is_stale(model.academic().revision()) {
        return;
    }
    refresh_dashboard(window, model, dashboard);
}

/// Dashboard callbacks. None of them touch the Timer; the only academic mutation is ticking a
/// planned unit (`toggle_calendar_entry`), which goes through `AcademicController` like every
/// other change and therefore bumps the revision the cache is keyed on.
fn bind_dashboard_callbacks(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
) {
    {
        let (weak, model, dashboard) = (window.as_weak(), Rc::clone(&model), Rc::clone(&dashboard));
        window.on_fn_select_entry(move |id| {
            dashboard.borrow_mut().ui_mut().selected_entry = Some(id.to_string());
            if let Some(window) = weak.upgrade() {
                refresh_dashboard_view(&window, &model.borrow(), &mut dashboard.borrow_mut());
            }
        });
    }
    {
        let (weak, model, dashboard) = (window.as_weak(), Rc::clone(&model), Rc::clone(&dashboard));
        window.on_fn_toggle_entry(move |id| {
            model
                .borrow_mut()
                .academic_mut()
                .toggle_calendar_entry(&CalendarEntryId::new(id.to_string()), wall_now());
            if let Some(window) = weak.upgrade() {
                refresh_dashboard(&window, &model.borrow(), &mut dashboard.borrow_mut());
            }
        });
    }
    {
        let (weak, model, dashboard) = (window.as_weak(), Rc::clone(&model), Rc::clone(&dashboard));
        window.on_fn_set_full(move |full| {
            dashboard.borrow_mut().ui_mut().full = full;
            if let Some(window) = weak.upgrade() {
                refresh_dashboard_view(&window, &model.borrow(), &mut dashboard.borrow_mut());
            }
        });
    }
    {
        let (weak, model, dashboard) = (window.as_weak(), Rc::clone(&model), Rc::clone(&dashboard));
        window.on_fn_set_range(move |index| {
            if let Some(range) = FocusRange::ALL.get(index.max(0) as usize) {
                dashboard.borrow_mut().ui_mut().range = *range;
            }
            if let Some(window) = weak.upgrade() {
                refresh_dashboard_view(&window, &model.borrow(), &mut dashboard.borrow_mut());
            }
        });
    }
    // Stage 19: the light/dark toggle is a persisted preference now (`app_appearance`); the old
    // session-only `fn-toggle-theme` callback is no longer used by the page.
    // "Focus" on a row/"Start focus": navigation to the Timer surface happens in Slint; linking
    // the task/course to the Timer session needs the Timer surface's context UI (Stage 18+), so
    // the native callback only records the intent for diagnostics.
    window.on_fn_focus_task(|id| {
        log::info!(
            "dashboard: focus requested for entry {id} (Timer context linking is not migrated yet)"
        )
    });
    window.on_fn_start_focus(|| {
        log::info!("dashboard: start focus requested (Timer context linking is not migrated yet)")
    });
}

/// UI-only change (layout, range, selection): the cached metrics stay valid, only the view data
/// (and, for a new range, the chart timeline) is rebuilt.
fn refresh_dashboard_view(
    window: &MainWindow,
    model: &AppModel,
    dashboard: &mut DashboardController,
) {
    refresh_dashboard(window, model, dashboard);
}

/// A 60-second check that notices the local date rolling over (Dashboard numbers are day-relative
/// but no academic change happens at midnight). It compares one date; it writes properties only
/// when the day actually changed. The returned timer must stay alive with the event loop.
fn start_date_rollover_check(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
) -> Timer {
    let timer = Timer::default();
    let weak = window.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_secs(60), move || {
        let clock = ChronoLocalClock;
        let academic = model.borrow();
        let stale =
            dashboard
                .borrow()
                .is_stale_for(academic.academic().revision(), wall_now(), &clock);
        if stale {
            if let Some(window) = weak.upgrade() {
                refresh_dashboard(&window, &academic, &mut dashboard.borrow_mut());
            }
        }
    });
    timer
}

/// `STUDY_NATIVE_NAV_STRESS=<cycles>` (diagnostic, off by default): flips Dashboard <-> Timer and
/// Quiet <-> Full through the same window properties/callback paths user clicks use, every 40 ms,
/// for memory-stability measurements (Stage 17, D17-P6). Logs `NAV_STRESS done` when finished.
fn start_nav_stress(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    dashboard: Rc<RefCell<DashboardController>>,
) -> Option<Timer> {
    let cycles: u32 = std::env::var("STUDY_NATIVE_NAV_STRESS")
        .ok()?
        .parse()
        .ok()?;
    let timer = Timer::default();
    let weak = window.as_weak();
    let mut step = 0u32;
    timer.start(TimerMode::Repeated, Duration::from_millis(40), move || {
        let Some(window) = weak.upgrade() else { return };
        if step >= cycles * 4 {
            if step == cycles * 4 {
                log::info!("NAV_STRESS done after {cycles} cycles");
                println!("NAV_STRESS done");
                step += 1;
            }
            return;
        }
        match step % 4 {
            0 => window.set_show_dashboard(true),
            1 => {
                dashboard.borrow_mut().ui_mut().full = (step / 4) % 2 == 0;
                dashboard.borrow_mut().ui_mut().range =
                    FocusRange::ALL[((step / 4) as usize) % FocusRange::ALL.len()];
                refresh_dashboard(&window, &model.borrow(), &mut dashboard.borrow_mut());
            }
            2 => window.set_show_dashboard(false),
            _ => {}
        }
        step += 1;
    });
    Some(timer)
}

/// `STUDY_NATIVE_DASHBOARD_REPORT=1`: prints the Dashboard's headline numbers once, so a scripted
/// run can compare them with production's own rendering of the same fixture.
fn dashboard_report_if_requested(dashboard: &DashboardController) {
    if std::env::var_os("STUDY_NATIVE_DASHBOARD_REPORT").is_none() {
        return;
    }
    if let Some(m) = dashboard.metrics() {
        println!(
            "DASHBOARD today={} today_min={} goal={} streak={} week_min={} lifetime_min={} open_tasks={} units_left={} units_per_day={:.4} queue={} exams={} courses={} score={} {} computations={:?}",
            m.today.to_iso(),
            m.today_minutes,
            m.daily_goal_minutes,
            m.streak_days,
            m.weekly_total_minutes,
            m.lifetime_minutes,
            m.open_task_count,
            m.total_units_left,
            m.units_per_day,
            m.queue.len(),
            m.exams.len(),
            m.courses.len(),
            m.overall_score,
            m.overall_label,
            dashboard.stats(),
        );
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
