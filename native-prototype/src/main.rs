mod app_model;
mod dashboard;
mod map;
mod map_adapter;

use app_model::{format_clock, AppCommand, AppModel, TimerStatus};
use dashboard::{format_minutes, AxisMark, DashboardScenario};
use slint::{ComponentHandle, ModelRc, SharedString, Timer, TimerMode, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

const RUNNING_UPDATE_INTERVAL: Duration = Duration::from_millis(100);

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let mut model = AppModel::stage_four_timer_preview();
    model.apply(AppCommand::MarkPresentationReady);
    let model = Rc::new(RefCell::new(model));
    let refresh_timer = Rc::new(Timer::default());

    let window = MainWindow::new()?;
    apply_startup_options(&window, &mut model.borrow_mut());
    apply_model_to_window(&window, &model.borrow(), Instant::now());
    apply_dashboard(&window, &model.borrow());
    let map = map_adapter::new_controller(map_level_from_env());
    map_adapter::apply_all(&window, &mut map.borrow_mut());
    map_adapter::bind(&window, &map);
    let _bench_timer = std::env::var("STUDY_NATIVE_MAP_BENCH")
        .ok()
        .map(|spec| map_adapter::start_bench(&window, &map, &spec));
    bind_model_callbacks(&window, Rc::clone(&model), Rc::clone(&refresh_timer));
    window.run()
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
        let now = Instant::now();
        model.borrow_mut().apply(AppCommand::Refresh(now));
        if let Some(window) = weak_window.upgrade() {
            apply_model_to_window(&window, &model.borrow(), now);
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
    window.set_modes(ModelRc::new(Rc::new(VecModel::from(modes))));

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
    window.set_session_notes(ModelRc::new(Rc::new(VecModel::from(notes))));
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
