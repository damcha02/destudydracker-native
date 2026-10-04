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
    // Travle's 195 map countries: one model for the app's lifetime, so a guess only restyles the
    // rows that changed (their paths are never re-sent) and reopening reuses it
    static TRAVLE_SHAPES: std::rc::Rc<slint::VecModel<crate::TravleShape>> = std::rc::Rc::new(slint::VecModel::default());
}

/// The open Travle screen's map rows (the view builds them only while Travle is open).
const TRAVLE_INDEX: i32 = 2;

/// Writes the open game's screen (`open: -1` when none).
pub fn push_games(window: &MainWindow) {
    if let Some(mut data) = with_controller(|c| games_data(c)) {
        DURAK_HAND.with(|hand| {
            data.durak.hand = crate::app_break_room::sync_model(hand, &data.durak.hand)
        });
        if data.open == TRAVLE_INDEX {
            TRAVLE_SHAPES.with(|shapes| {
                data.travle_shapes = crate::app_break_room::sync_model(shapes, &data.travle_shapes)
            });
        }
        window.set_games(data);
    }
}

/// A game was opened from a card (the controller already ran its `init*Puzzle`).
pub fn opened(window: &MainWindow) {
    push_games(window);
    // Stage 22a: Daily Skribbl is a network game with its own controller (src/app_skribbl.rs)
    if window.get_games().open == SKRIBBL_INDEX && !crate::app_skribbl::is_open() {
        crate::app_skribbl::open();
    }
}

const SKRIBBL_INDEX: i32 = 4;

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
            if crate::app_skribbl::is_open() {
                crate::app_skribbl::close();
            }
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
    window.on_travle_edited(move |t| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.travle_set_draft(&t));
        }
    });
    let weak = window.as_weak();
    window.on_travle_toggle(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, BreakRoomController::travle_toggle_dropdown);
        }
    });
    let weak = window.as_weak();
    window.on_travle_pick(move |n| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.travle_select(&n));
        }
    });
    let weak = window.as_weak();
    window.on_travle_clear(move || {
        if let Some(w) = weak.upgrade() {
            game(&w, BreakRoomController::travle_clear);
        }
    });
    let weak = window.as_weak();
    window.on_travle_submit(move || {
        let Some(w) = weak.upgrade() else { return };
        let before = with_controller(|c| c.writes()).unwrap_or(0);
        game(&w, |c| c.travle_submit(&ChronoLocalClock));
        if with_controller(|c| c.writes()).unwrap_or(0) != before {
            push(&w); // Solved/Failed shows on the card
        }
    });
    let weak = window.as_weak();
    window.on_travle_zoom(move |zoom_in| {
        if let Some(w) = weak.upgrade() {
            game(&w, |c| c.travle_zoom(zoom_in));
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

/// `STUDY_NATIVE_TRAVLE_STRESS=<mode>:<n>` (diagnostic, off by default): one step every 40 ms
/// through the same controller actions and pushes user input takes, then prints
/// `TRAVLE_STRESS done <mode> <n>`.
/// - `open:n`   n open/close cycles of the Travle screen (no play log), zooming and toggling the
///   dropdown on the way;
/// - `type:n`   n edits of the draft (the dropdown re-filters every time);
/// - `guess:n`  n daily puzzles (2026-10-01 onwards) each played to the end: two off-route guesses,
///   then the shortest route (real submits, so they persist like a user's);
/// - `resize:n` n window resizes between three sizes with Travle open.
pub fn start_travle_stress(window: &MainWindow) -> Option<slint::Timer> {
    use study_tracker_core::break_room::catalog::GameId;
    use study_tracker_core::break_room::travle::TravlePuzzle;
    let spec = std::env::var("STUDY_NATIVE_TRAVLE_STRESS").ok()?;
    let (mode, n) = spec.split_once(':')?;
    let (mode, n): (String, u64) = (mode.to_string(), n.parse().ok()?);
    let weak = window.as_weak();
    let mut step = 0u64;
    // guess mode: the queue of drafts for the current puzzle
    let mut drafts: Vec<String> = Vec::new();
    let mut puzzle_no = 0u64;
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(40),
        move || {
            let Some(w) = weak.upgrade() else { return };
            let total = match mode.as_str() {
                "open" => n * 2,
                _ => n,
            };
            if step >= total {
                if step == total && (mode != "guess" || drafts.is_empty()) {
                    println!("TRAVLE_STRESS done {mode} {n}");
                    step += 1;
                }
                if mode != "guess" || drafts.is_empty() {
                    return;
                }
            }
            match mode.as_str() {
                "open" => {
                    let opening = step % 2 == 0;
                    with_controller(|c| {
                        c.ui.open_game = opening.then_some(GameId::Travle);
                        if opening && step % 20 == 0 {
                            c.travle_zoom(true);
                            c.travle_toggle_dropdown();
                        } else if opening && step % 20 == 10 {
                            c.travle_zoom(false);
                            c.travle_toggle_dropdown();
                        }
                    });
                    w.set_show_break(true);
                    push_games(&w);
                    step += 1;
                }
                "type" => {
                    const WORDS: [&str; 6] = ["a", "al", "alb", "united", "t", ""];
                    with_controller(|c| {
                        c.ui.open_game = Some(GameId::Travle);
                        c.travle_set_draft(WORDS[(step % WORDS.len() as u64) as usize]);
                    });
                    w.set_show_break(true);
                    push_games(&w);
                    step += 1;
                }
                "guess" => {
                    if drafts.is_empty() {
                        if step >= n {
                            return;
                        }
                        let date = study_tracker_core::dashboard::civil::CivilDate::parse_iso(
                            "2026-10-01",
                        )
                        .map(|d| d.add_days(puzzle_no as i64).to_iso())
                        .unwrap_or_default();
                        let fresh = TravlePuzzle::fresh(&date, "stress-salt");
                        let shortest = fresh.routes().shortest();
                        drafts = ["Australia".to_string(), "Japan".to_string()]
                            .into_iter()
                            .chain(
                                shortest[1..shortest.len().saturating_sub(1)]
                                    .iter()
                                    .map(|id| id.name().to_string()),
                            )
                            .collect();
                        drafts.reverse();
                        with_controller(|c| {
                            c.ui.open_game = Some(GameId::Travle);
                            c.stress_replace_travle(fresh);
                        });
                        puzzle_no += 1;
                        step += 1;
                    }
                    if let Some(draft) = drafts.pop() {
                        with_controller(|c| {
                            c.travle_set_draft(&draft);
                            c.travle_submit(&ChronoLocalClock);
                        });
                    }
                    w.set_show_break(true);
                    push_games(&w);
                }
                "resize" => {
                    const SIZES: [(f32, f32); 3] =
                        [(1520.0, 980.0), (1100.0, 760.0), (1700.0, 1100.0)];
                    with_controller(|c| c.ui.open_game = Some(GameId::Travle));
                    w.set_show_break(true);
                    push_games(&w);
                    let (sw, sh) = SIZES[(step % 3) as usize];
                    w.window().set_size(slint::LogicalSize::new(sw, sh));
                    step += 1;
                }
                _ => step = total,
            }
        },
    );
    Some(timer)
}
