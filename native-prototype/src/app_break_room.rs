//! Application glue for the Break Room / Rest (Stage 20). Same shape as `app_appearance`: one
//! UI-thread runtime in a thread-local, entered from Slint callbacks and from `refresh_dashboard`.
//!
//! ```text
//! academic change (refresh_dashboard) ──► BreakRoomController::sync (cached by revision + day)
//! card / rock / water / game callbacks ──► controller action ──► push (only what changed)
//! rest timer / breathing ──► one 1 Hz slint::Timer, alive only while one of them runs
//! unlock glow / rock bounce ──► single-shot timers that flip the property back (bounded)
//! ```
//!
//! Nothing here ticks while the Break Room is idle: no repeating timer exists unless the rest
//! timer or the breathing exercise is running.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, Global, Model, ModelRc, Timer, TimerMode, VecModel};
use study_tracker_core::break_room::catalog::GAMES;
use study_tracker_core::dashboard::civil::{CivilDate, LocalClock};

use crate::app_model::AppModel;
use crate::break_room_controller::{AlbumAnim, Room};
use crate::break_room_controller::{BreakRoomController, Rng};
use crate::break_room_view::{album_data, album_pages_now, fn_break_data, wabi_rest_data};
use crate::dashboard_view::ChronoLocalClock;
use crate::persistence::break_room_port::FileBreakRoomPort;
use crate::persistence::NativeStore;
use crate::{Emoji, MainWindow};

struct Runtime {
    // persistent list models: a repeater keeps its items (and their running animations) when the
    // same model is updated in place, and rebuilds them when it is handed a new one
    fn_cards: Rc<VecModel<crate::BrCard>>,
    fn_badges: Rc<VecModel<crate::BrBadge>>,
    wabi_cards: Rc<VecModel<crate::BrCard>>,
    model: Rc<RefCell<AppModel>>,
    controller: BreakRoomController,
    seconds: Timer,
    effects: Vec<Timer>,
    pushes: u64,
}

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

fn with_runtime<R>(f: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    RUNTIME.with(|r| r.borrow_mut().as_mut().map(f))
}

fn today() -> CivilDate {
    ChronoLocalClock.local_date(crate::wall_now())
}

/// The platform's colour-emoji face (what the browser uses for production's emoji).
fn emoji_family() -> &'static str {
    if cfg!(windows) {
        "Segoe UI Emoji"
    } else if cfg!(target_os = "macos") {
        "Apple Color Emoji"
    } else {
        "Noto Color Emoji"
    }
}

/// `STUDY_NATIVE_BREAK_PICK=<0..1>` pins every random pick (quote, stretch idea, Durak hint) like a
/// parity capture's pinned `Math.random`; otherwise the picks are seeded from the clock.
fn rng_from_env() -> Rng {
    match std::env::var("STUDY_NATIVE_BREAK_PICK")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
    {
        Some(pick) => Rng::pinned(pick),
        None => Rng::seeded(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x5eed, |d| d.as_nanos() as u64),
        ),
    }
}

/// Production's CSS stacks end in the generic families (`serif`, `sans-serif`, `monospace`), which
/// Chromium resolves through fontconfig on Linux (Liberation Serif / Liberation Sans / the user's
/// monospace on a stock Arch install), while on Windows they land on Georgia / Arial / Consolas.
/// Native names the Windows faces in its tokens; on Linux it asks fontconfig once for the same
/// generic families so both apps render the same faces. Windows and macOS are unchanged.
fn apply_platform_fonts(window: &MainWindow) {
    if !cfg!(target_os = "linux") {
        return;
    }
    // the three lookups run concurrently (~9 ms each)
    let spawn = |generic: &str| {
        std::process::Command::new("fc-match")
            .args(["-f", "%{family[0]}", generic])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()
    };
    let children = [spawn("serif"), spawn("sans-serif"), spawn("monospace")];
    let fonts = children.map(|child| -> Option<String> {
        let out = child?.wait_with_output().ok()?;
        let name = String::from_utf8(out.stdout).ok()?.trim().to_string();
        (!name.is_empty()).then_some(name)
    });
    log::info!("platform fonts (fontconfig): {fonts:?}");
    let [serif, sans, mono] = fonts;
    let (fn_, ws) = (crate::FN::get(window), crate::WS::get(window));
    if let Some(f) = serif {
        fn_.set_serif(f.clone().into());
        ws.set_serif(f.into());
    }
    if let Some(f) = sans {
        fn_.set_sans(f.clone().into());
        ws.set_sans(f.into());
    }
    if let Some(f) = mono {
        fn_.set_mono(f.clone().into());
        ws.set_mono(f.into());
    }
}

pub fn install(window: &MainWindow, model: Rc<RefCell<AppModel>>, store_path: &Path) {
    Emoji::get(window).set_family(emoji_family().into());
    apply_platform_fonts(window);
    let started = Instant::now();
    let controller = BreakRoomController::load(
        Box::new(FileBreakRoomPort::new(NativeStore::new(
            store_path.to_path_buf(),
        ))),
        rng_from_env(),
        today(),
    );
    log::info!(
        "break room: loaded in {:?} ({} pats, {} total unlocks)",
        started.elapsed(),
        controller.record().state.pet_rock_pats,
        controller.record().state.total_unlocks
    );
    RUNTIME.with(|r| {
        *r.borrow_mut() = Some(Runtime {
            fn_cards: Rc::new(VecModel::default()),
            fn_badges: Rc::new(VecModel::default()),
            wabi_cards: Rc::new(VecModel::default()),
            model,
            controller,
            seconds: Timer::default(),
            effects: Vec::new(),
            pushes: 0,
        })
    });
    if std::env::var("STUDY_NATIVE_VIEW").as_deref() == Ok("break") {
        window.set_show_break(true);
        window.set_rest_menu_open(true);
    }
    // `STUDY_NATIVE_REST_ROOM=games|meditation|achievements|album-open` (screenshots/benchmarks)
    match std::env::var("STUDY_NATIVE_REST_ROOM").as_deref() {
        Ok("meditation") => with_runtime(|rt| rt.controller.select_room(Room::Meditation)),
        Ok("achievements") => with_runtime(|rt| rt.controller.select_room(Room::Achievements)),
        Ok("album-open") => with_runtime(|rt| {
            rt.controller.select_room(Room::Achievements);
            rt.controller.ui.album.open_book(true);
        }),
        _ => None,
    };
    bind_callbacks(window);
    crate::app_break_games::install(window);
    sync_and_push(window);
    // `STUDY_NATIVE_OPEN_GAME=<card index>` (screenshots/benchmarks): plays that card like a click
    if let Some(i) = std::env::var("STUDY_NATIVE_OPEN_GAME")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        if let Some(game) = GAMES.get(i).map(|g| g.id) {
            with_runtime(|rt| {
                rt.controller
                    .play(game, crate::wall_now(), &ChronoLocalClock)
            });
            push(window);
            crate::app_break_games::opened(window);
        }
    }
}

/// Called after every Dashboard refresh (academic revision or day changed): the Timer ->
/// StudySession -> Break Room path. A cached no-op unless the revision or the day moved.
pub fn after_academic_change(window: &MainWindow, model: &AppModel) {
    let changed = with_runtime(|rt| {
        rt.controller.sync(
            model.academic().state(),
            model.academic().revision(),
            today(),
            &ChronoLocalClock,
        )
    })
    .unwrap_or(false);
    if changed {
        push(window);
    }
}

fn sync_and_push(window: &MainWindow) {
    let Some(model) = with_runtime(|rt| Rc::clone(&rt.model)) else {
        return;
    };
    let model = model.borrow();
    with_runtime(|rt| {
        rt.controller.sync(
            model.academic().state(),
            model.academic().revision(),
            today(),
            &ChronoLocalClock,
        )
    });
    push(window);
}

/// Updates a persistent model to `fresh`'s rows in place (only changed rows notify) and returns it.
pub fn sync_model<T: Clone + PartialEq + 'static>(
    cache: &Rc<VecModel<T>>,
    fresh: &ModelRc<T>,
) -> ModelRc<T> {
    let rows: Vec<T> = fresh.iter().collect();
    if cache.row_count() == rows.len() {
        for (i, row) in rows.into_iter().enumerate() {
            if cache.row_data(i).as_ref() != Some(&row) {
                cache.set_row_data(i, row);
            }
        }
    } else {
        cache.set_vec(rows);
    }
    ModelRc::from(cache.clone())
}

/// Writes the Break Room properties (the only writer). The album is rebuilt only while its room
/// is the one on show.
pub fn push(window: &MainWindow) {
    let dark = window.get_appearance_dark();
    let card = crate::WS::get(window).get_paper_1();
    let data = with_runtime(|rt| {
        rt.pushes += 1;
        let album =
            (rt.controller.ui.room == Room::Achievements).then(|| album_data(&rt.controller, dark));
        (
            fn_break_data(&rt.controller),
            wabi_rest_data(&rt.controller, dark, card),
            album,
        )
    });
    if let Some((mut fn_data, mut rest, album)) = data {
        with_runtime(|rt| {
            fn_data.cards = sync_model(&rt.fn_cards, &fn_data.cards);
            fn_data.badges = sync_model(&rt.fn_badges, &fn_data.badges);
            rest.cards = sync_model(&rt.wabi_cards, &rest.cards);
        });
        window.set_rest_room(rest.room);
        window.set_breath_ring(if rest.breath_on {
            rest.breath_size
        } else {
            60.0
        });
        window.set_fn_break(fn_data);
        window.set_wabi_rest(rest);
        if let Some(album) = album {
            window.set_album_cover_seq(album.cover_seq);
            window.set_album_leaf_seq(album.leaf_seq);
            window.set_album_slide_open(album.slide_open);
            window.set_album(album);
        }
    }
}

fn on_seconds_tick(window: &MainWindow) {
    push(window);
}

/// Starts an album motion's finish (production's `finish()` after the WAAPI animation).
fn album_motion(window: &MainWindow, started: AlbumAnim) {
    push(window);
    if started == AlbumAnim::None {
        return;
    }
    let weak = window.as_weak();
    after(started.duration_ms() + 30, move || {
        with_runtime(|rt| rt.controller.ui.album.finish());
        if let Some(w) = weak.upgrade() {
            push(&w);
        }
    });
}

fn spread_count() -> usize {
    with_runtime(|rt| (album_pages_now(&rt.controller).len() / 2).max(1)).unwrap_or(1)
}

pub fn report() -> String {
    with_runtime(|rt| {
        format!(
            "break_writes={} break_pushes={} break_evaluations={} break_art_decoded={}",
            rt.controller.writes(),
            rt.pushes,
            rt.controller.evaluations(),
            crate::break_room_art::decoded()
        )
    })
    .unwrap_or_default()
}

/// Runs `f` after `ms` once (bounded effect timers; finished ones are dropped on the next call).
fn after(ms: u64, f: impl FnOnce() + 'static) {
    let timer = Timer::default();
    let mut f = Some(f);
    timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(ms),
        move || {
            if let Some(f) = f.take() {
                f();
            }
        },
    );
    with_runtime(|rt| {
        rt.effects.retain(Timer::running);
        rt.effects.push(timer);
    });
}

fn action(window: &MainWindow, f: impl FnOnce(&mut BreakRoomController)) {
    with_runtime(|rt| f(&mut rt.controller));
    push(window);
}

fn bind_callbacks(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_break_opened(move || {
        if let Some(w) = weak.upgrade() {
            sync_and_push(&w);
            w.invoke_surface_changed();
        }
    });
    let weak = window.as_weak();
    window.on_break_unlock(move |i| {
        let Some(w) = weak.upgrade() else { return };
        let Some(game) = GAMES.get(i as usize).map(|g| g.id) else {
            return;
        };
        let unlocked =
            with_runtime(|rt| rt.controller.unlock(game, &ChronoLocalClock)).unwrap_or(false);
        push(&w);
        if unlocked {
            let weak = w.as_weak();
            after(700, move || {
                with_runtime(|rt| rt.controller.ui.celebrating = None);
                if let Some(w) = weak.upgrade() {
                    push(&w);
                }
            });
        }
    });
    let weak = window.as_weak();
    window.on_break_play(move |i| {
        let Some(w) = weak.upgrade() else { return };
        let Some(game) = GAMES.get(i as usize).map(|g| g.id) else {
            return;
        };
        action(&w, |c| {
            c.play(game, crate::wall_now(), &ChronoLocalClock);
        });
        crate::app_break_games::opened(&w);
    });
    let weak = window.as_weak();
    window.on_rest_pick_room(move |i| {
        let Some(w) = weak.upgrade() else { return };
        let room = match i {
            1 => Room::Meditation,
            2 => Room::Achievements,
            _ => Room::Games,
        };
        with_runtime(|rt| rt.controller.select_room(room));
        if !w.get_show_break() {
            w.set_show_break(true);
            w.invoke_break_opened();
        }
        push(&w);
    });
    let weak = window.as_weak();
    window.on_rest_toggle(move || {
        let Some(w) = weak.upgrade() else { return };
        with_runtime(|rt| rt.controller.rest_toggle(Instant::now()));
        ensure_seconds_clock(&w, on_seconds_tick);
        push(&w);
    });
    let weak = window.as_weak();
    window.on_rest_reset(move || {
        let Some(w) = weak.upgrade() else { return };
        with_runtime(|rt| rt.controller.rest_reset());
        ensure_seconds_clock(&w, on_seconds_tick);
        push(&w);
    });
    let weak = window.as_weak();
    window.on_rest_edit(move |text| {
        let Some(w) = weak.upgrade() else { return };
        if let Some(seconds) = study_tracker_core::break_room::rest::parse_timer_face(&text) {
            with_runtime(|rt| rt.controller.rest_set_seconds(seconds));
        }
        push(&w);
    });
    let weak = window.as_weak();
    window.on_rest_tree(move || {
        if let Some(w) = weak.upgrade() {
            action(&w, BreakRoomController::next_tree);
        }
    });
    let weak = window.as_weak();
    window.on_rest_breath(move || {
        let Some(w) = weak.upgrade() else { return };
        with_runtime(|rt| rt.controller.breath_toggle(Instant::now()));
        ensure_seconds_clock(&w, on_seconds_tick);
        push(&w);
    });
    let weak = window.as_weak();
    window.on_album_open(move || {
        let Some(w) = weak.upgrade() else { return };
        let reduced = crate::app_appearance::reduced_motion();
        let started =
            with_runtime(|rt| rt.controller.ui.album.open_book(reduced)).unwrap_or(AlbumAnim::None);
        album_motion(&w, started);
    });
    let weak = window.as_weak();
    window.on_album_turn(move |dir| {
        let Some(w) = weak.upgrade() else { return };
        let reduced = crate::app_appearance::reduced_motion();
        let count = spread_count();
        let started = with_runtime(|rt| {
            if dir > 0 {
                rt.controller.ui.album.page_forward(count, reduced)
            } else {
                rt.controller.ui.album.page_back(count, reduced)
            }
        })
        .unwrap_or(AlbumAnim::None);
        album_motion(&w, started);
    });
    let weak = window.as_weak();
    window.on_album_close(move || {
        let Some(w) = weak.upgrade() else { return };
        let reduced = crate::app_appearance::reduced_motion();
        let started = with_runtime(|rt| rt.controller.ui.album.close_book(reduced))
            .unwrap_or(AlbumAnim::None);
        album_motion(&w, started);
    });
    let weak = window.as_weak();
    window.on_break_water(move || {
        if let Some(w) = weak.upgrade() {
            action(&w, |c| c.add_water(&ChronoLocalClock));
        }
    });
    let weak = window.as_weak();
    window.on_break_stretch(move || {
        if let Some(w) = weak.upgrade() {
            action(&w, BreakRoomController::next_stretch);
        }
    });
    let weak = window.as_weak();
    window.on_break_pat(move || {
        let Some(w) = weak.upgrade() else { return };
        action(&w, |c| c.pat_rock(&ChronoLocalClock));
        let celebrating = with_runtime(|rt| rt.controller.ui.rock_celebrating).unwrap_or(false);
        w.set_rock_bounce(true);
        w.set_rock_celebrate(celebrating);
        let weak = w.as_weak();
        after(if celebrating { 400 } else { 170 }, move || {
            with_runtime(|rt| rt.controller.ui.rock_celebrating = false);
            if let Some(w) = weak.upgrade() {
                w.set_rock_bounce(false);
                w.set_rock_celebrate(false);
            }
        });
    });
}

/// Gives the game layer access to the controller (the games live in `app_break_games`).
pub fn with_controller<R>(f: impl FnOnce(&mut BreakRoomController) -> R) -> Option<R> {
    with_runtime(|rt| f(&mut rt.controller))
}

/// The 1 Hz clock for the rest timer / breathing, started on demand and dropped when idle.
pub fn ensure_seconds_clock(window: &MainWindow, on_tick: fn(&MainWindow)) {
    let needed = with_runtime(|rt| rt.controller.needs_seconds_clock()).unwrap_or(false);
    let running = with_runtime(|rt| rt.seconds.running()).unwrap_or(false);
    if needed && !running {
        let weak = window.as_weak();
        with_runtime(|rt| {
            rt.seconds
                .start(TimerMode::Repeated, Duration::from_millis(250), move || {
                    let Some(w) = weak.upgrade() else { return };
                    let now = Instant::now();
                    let (changed, still) = with_runtime(|rt| {
                        let a = rt.controller.rest_tick(now);
                        let b = rt.controller.breath_tick(now);
                        (a || b, rt.controller.needs_seconds_clock())
                    })
                    .unwrap_or((false, false));
                    if changed {
                        on_tick(&w);
                    }
                    if !still {
                        with_runtime(|rt| rt.seconds.stop());
                    }
                })
        });
    } else if !needed && running {
        with_runtime(|rt| rt.seconds.stop());
    }
}

/// `STUDY_NATIVE_BREAK_STRESS=<cycles>` (diagnostic, off by default): every 40 ms one step of a
/// cycle through every Break Room surface - the page, each game's screen (Flaggle with its 80-flag
/// dropdown open), the three Rest rooms and the album - through the same properties/models user
/// navigation uses. Nothing is persisted (games are opened without a play log). Prints
/// `BREAK_STRESS done <cycles>` at the end.
///
/// `STUDY_NATIVE_GAME_RESET_STRESS=<n>`: deals n Durak puzzles (`<date>_<i>`, solver included) into
/// the open Durak screen one by one, in memory only. Prints `GAME_RESET_STRESS done <n>`.
pub fn start_stress(window: &MainWindow) -> Option<Timer> {
    use study_tracker_core::break_room::catalog::GameId;
    let cycles: u64 = std::env::var("STUDY_NATIVE_BREAK_STRESS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let resets: u64 = std::env::var("STUDY_NATIVE_GAME_RESET_STRESS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if cycles == 0 && resets == 0 {
        return None;
    }
    let weak = window.as_weak();
    let mut step = 0u64;
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_millis(40), move || {
        let Some(w) = weak.upgrade() else { return };
        if resets > 0 {
            if step >= resets {
                if step == resets {
                    println!("GAME_RESET_STRESS done {resets}");
                    step += 1;
                }
                return;
            }
            let seed = format!("2026-09-{:02}_{}", 1 + step % 28, step % 3);
            with_runtime(|rt| {
                rt.controller.ui.open_game = Some(GameId::DailyDurak);
                rt.controller.durak.game =
                    study_tracker_core::break_room::durak::find_daily_puzzle(&seed, 0.0)
                        .map(|p| p.initial);
                rt.controller.durak.selected.clear();
            });
            w.set_show_break(true);
            crate::app_break_games::push_games(&w);
            step += 1;
            return;
        }
        const STEPS: u64 = 14;
        if step >= cycles * STEPS {
            if step == cycles * STEPS {
                println!("BREAK_STRESS done {cycles}");
                step += 1;
            }
            return;
        }
        let open = |g: Option<GameId>, dropdown: bool| {
            with_runtime(|rt| {
                rt.controller.ui.open_game = g;
                rt.controller.ui.flaggle_dropdown = dropdown;
                if g == Some(GameId::DailyDurak) && rt.controller.durak.game.is_none() {
                    rt.controller.durak.game =
                        study_tracker_core::break_room::durak::find_daily_puzzle(
                            "2026-09-30_0",
                            0.0,
                        )
                        .map(|p| p.initial);
                }
            });
            crate::app_break_games::push_games(&w);
        };
        let room = |r: Room| {
            with_runtime(|rt| rt.controller.select_room(r));
            push(&w);
        };
        match step % STEPS {
            0 => w.set_show_break(true),
            1 => open(Some(GameId::DailyDurak), false),
            2 => open(Some(GameId::Wordle), false),
            3 => open(Some(GameId::Geodle), false),
            4 => open(Some(GameId::Flaggle), true),
            5 => open(Some(GameId::DailySkribbl), false),
            6 => open(Some(GameId::Travle), false),
            7 => open(None, false),
            8 => room(Room::Meditation),
            9 => room(Room::Achievements),
            10 => {
                with_runtime(|rt| rt.controller.ui.album.open_book(true));
                push(&w);
            }
            11 => {
                with_runtime(|rt| rt.controller.ui.album.close_book(true));
                push(&w);
            }
            12 => room(Room::Games),
            _ => {
                w.set_show_break(false);
                w.set_show_dashboard(true);
            }
        }
        step += 1;
    });
    Some(timer)
}
