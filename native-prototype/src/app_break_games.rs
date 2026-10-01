//! The Break Room game screens' glue (Stage 20): Slint callbacks -> `BreakRoomController` game
//! actions -> one push of the open game's data. No game logic here; no timers (every game is
//! turn-based and event-driven, so an open, idle board renders nothing).

use slint::ComponentHandle;
use study_tracker_core::break_room::durak::DurakSession;

use crate::app_break_room::{push, with_controller};
use crate::break_room_controller::BreakRoomController;
use crate::break_room_flags::ResvgFlags;
use crate::break_room_view::games_data;
use crate::dashboard_view::ChronoLocalClock;
use crate::MainWindow;

thread_local! {
    // the Durak hand keeps its items (and their lift animation) across pushes
    static DURAK_HAND: std::rc::Rc<slint::VecModel<crate::DCard>> = std::rc::Rc::new(slint::VecModel::default());
}

/// Writes the open game's screen (`open: -1` when none).
pub fn push_games(window: &MainWindow) {
    if let Some(mut data) = with_controller(|c| games_data(c)) {
        DURAK_HAND.with(|hand| {
            data.durak.hand = crate::app_break_room::sync_model(hand, &data.durak.hand)
        });
        window.set_games(data);
    }
}

/// A game was opened from a card (the controller already ran its `init*Puzzle`).
pub fn opened(window: &MainWindow) {
    push_games(window);
}

fn game(window: &MainWindow, f: impl FnOnce(&mut BreakRoomController)) {
    with_controller(f);
    push_games(window);
}

fn durak(
    window: &MainWindow,
    action: impl FnOnce(
        &mut DurakSession,
        &mut study_tracker_core::break_room::durak::DurakPuzzle,
        &str,
        f64,
    ) -> bool,
) {
    with_controller(|c| c.durak_action(&ChronoLocalClock, action));
    push_games(window);
    // a solved puzzle changes the card's `n/3`
    push(window);
}

pub fn install(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_game_close(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, BreakRoomController::close_game);
            push(&w);
        }
    });
    let weak = window.as_weak();
    window.on_wordle_key(move |key| {
        let Some(w) = weak.upgrade() else { return };
        let before = with_controller(|c| c.writes()).unwrap_or(0);
        game(&w, |c| match key.as_str() {
            "enter" => c.wordle_enter(&ChronoLocalClock),
            "back" => c.wordle_backspace(),
            k => {
                let mut chars = k.chars();
                if let (Some(l), None) = (chars.next(), chars.next()) {
                    if l.is_ascii_alphabetic() {
                        c.wordle_letter(l.to_ascii_lowercase());
                    }
                }
            }
        });
        if with_controller(|c| c.writes()).unwrap_or(0) != before {
            push(&w); // solved/failed shows on the card
        }
    });
    let weak = window.as_weak();
    window.on_wordle_hard(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.wordle_toggle_hard(&ChronoLocalClock));
        }
    });
    let weak = window.as_weak();
    window.on_geodle_edited(move |t| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.geodle_set_draft(&t));
        }
    });
    let weak = window.as_weak();
    window.on_geodle_toggle(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.ui.geodle_dropdown = !c.ui.geodle_dropdown);
        }
    });
    let weak = window.as_weak();
    window.on_geodle_pick(move |n| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.geodle_select(&n));
        }
    });
    let weak = window.as_weak();
    window.on_geodle_submit(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.geodle_submit(&ChronoLocalClock));
            push(&w);
        }
    });
    let weak = window.as_weak();
    window.on_flaggle_edited(move |t| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.flaggle_set_draft(&t));
        }
    });
    let weak = window.as_weak();
    window.on_flaggle_toggle(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.ui.flaggle_dropdown = !c.ui.flaggle_dropdown);
        }
    });
    let weak = window.as_weak();
    window.on_flaggle_pick(move |n| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.flaggle_select(&n));
        }
    });
    let weak = window.as_weak();
    window.on_flaggle_submit(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.flaggle_submit(&mut ResvgFlags, &ChronoLocalClock));
            push(&w);
        }
    });
    let weak = window.as_weak();
    window.on_durak_card(move |i| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.durak.click_card(i.max(0) as usize));
        }
    });
    let weak = window.as_weak();
    window.on_durak_act(move |a| {
        let Some(w) = weak.upgrade() else { return };
        match a.as_str() {
            "attack" => durak(&w, |s, p, t, _| s.attack(p, t)),
            "throw" => durak(&w, |s, p, t, _| s.throw_or_pass(p, t, false)),
            "pass" => durak(&w, |s, p, t, _| s.throw_or_pass(p, t, true)),
            "defend" => durak(&w, |s, p, t, _| s.defend(p, t)),
            "slide" => durak(&w, |s, p, t, _| s.slide(p, t)),
            "pickup" => durak(&w, |s, p, t, _| s.pick_up(p, t)),
            "retry" => durak(&w, |s, p, t, pick| s.retry(p, t, pick)),
            _ => {}
        }
    });
}
