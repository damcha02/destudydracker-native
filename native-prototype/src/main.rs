mod app_model;

use app_model::{format_clock, AppCommand, AppModel, TimerStatus};
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
    apply_model_to_window(&window, &model.borrow(), Instant::now());
    bind_model_callbacks(&window, Rc::clone(&model), Rc::clone(&refresh_timer));
    window.run()
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
    let remaining = timer.remaining(now);
    let elapsed = timer.elapsed(now);
    let duration = timer.duration();
    let selected_mode = timer.selected_mode();
    let selected = &model.modes()[selected_mode];

    window.set_app_title(model.title().into());
    window.set_status_text(model.status().into());
    window.set_timer_text(format_clock(remaining).into());
    window.set_status_label(timer.status().as_str().into());
    window.set_context_title("General focus".into());
    window.set_context_detail("Analysis II problem set · Next: exercise 7".into());
    window.set_selected_mode(selected_mode as i32);
    window.set_mode_label(selected.label().into());
    window.set_mode_detail(selected.detail().into());
    window.set_phase_tone(selected.tone().accent_index());
    window.set_running(timer.is_running());
    window.set_completed(timer.status() == TimerStatus::Completed);
    window.set_timer_progress(timer.progress_fraction(now));
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
