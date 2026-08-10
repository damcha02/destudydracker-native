mod app_model;

use app_model::{AppCommand, AppModel, DemoMode};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let mut model = AppModel::stage_three_widget_preview();
    model.apply(AppCommand::MarkPresentationReady);
    let model = Rc::new(RefCell::new(model));

    let window = MainWindow::new()?;
    apply_model_to_window(&window, &model.borrow());
    bind_model_callbacks(&window, Rc::clone(&model));
    window.run()
}

fn bind_model_callbacks(window: &MainWindow, model: Rc<RefCell<AppModel>>) {
    let weak_window = window.as_weak();
    let dispatch = move |command: AppCommand| {
        model.borrow_mut().apply(command);
        if let Some(window) = weak_window.upgrade() {
            apply_model_to_window(&window, &model.borrow());
        }
    };
    let dispatch = Rc::new(dispatch);

    {
        let dispatch = Rc::clone(&dispatch);
        window.on_activate_primary(move || {
            dispatch(AppCommand::RecordActivation);
        });
    }
    {
        let dispatch = Rc::clone(&dispatch);
        window.on_toggle_enabled(move || {
            dispatch(AppCommand::ToggleEnabled);
        });
    }
    {
        let dispatch = Rc::clone(&dispatch);
        window.on_select_mode(move |mode| {
            dispatch(AppCommand::SelectMode(mode_from_index(mode)));
        });
    }
    {
        let dispatch = Rc::clone(&dispatch);
        window.on_set_progress(move |progress| {
            dispatch(AppCommand::SetProgress(progress.max(0) as u16));
        });
    }
    window.on_increase_progress(move || {
        dispatch(AppCommand::IncreaseProgress(10));
    });
}

fn apply_model_to_window(window: &MainWindow, model: &AppModel) {
    let state = model.demo_state();

    window.set_app_title(model.title().into());
    window.set_status_text(model.status().into());
    window.set_selected_mode(mode_to_index(state.selected_mode()));
    window.set_selected_mode_label(state.selected_mode().as_str().into());
    window.set_demo_progress(state.progress_fraction());
    window.set_demo_progress_label(format!("{}%", state.progress_percent()).into());
    window.set_controls_enabled(state.controls_enabled());
    window.set_activation_count(state.activation_count() as i32);

    let progress_examples = model
        .progress_examples()
        .iter()
        .map(|example| ProgressExampleData {
            label: SharedString::from(example.label()),
            progress: example.progress_fraction(),
            percent: example.progress_percent() as i32,
        })
        .collect::<Vec<_>>();
    window.set_progress_examples(ModelRc::new(Rc::new(VecModel::from(progress_examples))));

    let scroll_rows = model
        .scroll_rows()
        .iter()
        .map(|row| DemoRowData {
            title: SharedString::from(row.title()),
            detail: SharedString::from(row.detail()),
            progress: row.progress_fraction(),
            percent: row.progress_percent() as i32,
        })
        .collect::<Vec<_>>();
    window.set_scroll_rows(ModelRc::new(Rc::new(VecModel::from(scroll_rows))));
}

fn mode_to_index(mode: DemoMode) -> i32 {
    match mode {
        DemoMode::Focus => 0,
        DemoMode::Review => 1,
        DemoMode::Rest => 2,
    }
}

fn mode_from_index(index: i32) -> DemoMode {
    match index {
        1 => DemoMode::Review,
        2 => DemoMode::Rest,
        _ => DemoMode::Focus,
    }
}
