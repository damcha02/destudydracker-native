mod app_model;

use app_model::{AppCommand, AppModel, FontWeight, TextSize};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let mut model = AppModel::stage_two_text_input_preview();
    model.apply(AppCommand::MarkPresentationReady);
    let model = Rc::new(RefCell::new(model));

    let window = MainWindow::new()?;
    apply_model_to_window(&window, &model.borrow());
    bind_model_callbacks(&window, Rc::clone(&model));
    window.run()
}

fn bind_model_callbacks(window: &MainWindow, model: Rc<RefCell<AppModel>>) {
    let single_line_model = Rc::clone(&model);
    window.on_single_line_edited(move |text| {
        single_line_model
            .borrow_mut()
            .apply(AppCommand::UpdateSingleLine(text.to_string()));
    });

    window.on_multiline_edited(move |text| {
        model
            .borrow_mut()
            .apply(AppCommand::UpdateMultiline(text.to_string()));
    });
}

fn apply_model_to_window(window: &MainWindow, model: &AppModel) {
    window.set_app_title(model.title().into());
    window.set_status_text(model.status().into());
    window.set_wrapping_text(model.wrapping_text().into());
    window.set_single_line_text(model.single_line_input().into());
    window.set_multiline_text(model.multiline_input().into());

    let samples = model
        .text_samples()
        .iter()
        .map(|sample| TextSampleData {
            label: SharedString::from(sample.label()),
            value: SharedString::from(sample.value()),
            weight: slint_weight(sample.weight()),
            size: slint_size(sample.size()),
        })
        .collect::<Vec<_>>();
    window.set_text_samples(ModelRc::new(Rc::new(VecModel::from(samples))));

    let notes = model
        .notes()
        .iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    window.set_notes(ModelRc::new(Rc::new(VecModel::from(notes))));
}

fn slint_weight(weight: FontWeight) -> i32 {
    match weight {
        FontWeight::Normal => 400,
        FontWeight::SemiBold => 600,
        FontWeight::Bold => 700,
    }
}

fn slint_size(size: TextSize) -> f32 {
    match size {
        TextSize::Body => 16.0,
        TextSize::Large => 18.0,
        TextSize::Display => 22.0,
    }
}
