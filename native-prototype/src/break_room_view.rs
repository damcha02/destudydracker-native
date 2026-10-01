//! Break Room view data (Stage 20): turns the controller's state into the Slint structs, with
//! production's exact strings. No rules here - every number comes from `core::break_room`.

use slint::{ModelRc, SharedString, VecModel};
use study_tracker_core::break_room::achievements::rock_stage;
use study_tracker_core::break_room::catalog::{quotes, stretch_ideas, GAMES, GAME_COUNT};
use study_tracker_core::break_room::state::streak_emoji;

use study_tracker_core::break_room::achievements::{
    album_entries, album_pages, japanese_name, AlbumPage,
};
use study_tracker_core::break_room::rest::{breath_frame, BREATH_PHASES};

use crate::break_room_controller::{AlbumAnim, BreakRoomController, Room};
use crate::{AlbumData, AlbumPageData, AlbumRow, BrBadge, BrCard, FnBreakData, WabiRestData};

fn ss(s: impl Into<SharedString>) -> SharedString {
    s.into()
}

/// The six cards in production order.
pub fn cards(c: &BreakRoomController) -> Vec<BrCard> {
    let p = c.progression();
    GAMES
        .iter()
        .map(|g| BrCard {
            name: ss(g.name),
            desc: ss(g.desc),
            unlocked: p.is_unlocked(g.id),
            status: ss(c.card_status(g.id)),
            can_unlock: p.can_unlock_more,
            wait: ss(format!("~{} min", p.mins_until_next)),
            celebrating: c.ui.celebrating == Some(g.id),
        })
        .collect()
}

/// `rockStage.label · N pat(s)`.
pub fn rock_label(pats: u64) -> String {
    let (_, label) = rock_stage(pats);
    format!(
        "{label} \u{b7} {pats} pat{}",
        if pats == 1 { "" } else { "s" }
    )
}

/// `“text” — author`.
pub fn quote(index: usize) -> String {
    let list = quotes();
    let (text, author) = list[index % list.len()];
    format!("\u{201c}{text}\u{201d} \u{2014} {author}")
}

pub fn fn_break_data(c: &BreakRoomController) -> FnBreakData {
    let p = c.progression();
    let state = &c.record().state;
    let flames = streak_emoji(state.unlock_streak);
    let badges: Vec<BrBadge> = c
        .achievements()
        .iter()
        .take(10)
        .map(|a| BrBadge {
            icon: ss(a.icon),
            name: ss(a.name),
            earned: a.earned,
        })
        .collect();
    let water = p.water_today;
    FnBreakData {
        note: ss(format!(
            "{} of {GAME_COUNT} breaks available",
            p.unlocked.len()
        )),
        xp_fraction: p.xp_percent as f32 / 100.0,
        xp_label: ss(format!(
            "XP: {} / 45 \u{2014} ~{} min",
            p.xp_progress, p.mins_until_next
        )),
        streak: ss(if flames.is_empty() {
            String::new()
        } else {
            format!("{}-day streak", state.unlock_streak)
        }),
        streak_icon: ss(flames),
        quote: ss(quote(c.ui.quote_index)),
        cards: ModelRc::new(VecModel::from(cards(c))),
        stats_unlocked: ss(format!("Unlocked {}/{GAME_COUNT}", p.unlocked.len())),
        stats_played: ss(format!("Played {} today", p.played_today.len())),
        full_house: p.unlocked.len() == GAME_COUNT,
        water: ss(format!(
            "{water} glass{} today",
            if water == 1 { "" } else { "es" }
        )),
        badges: ModelRc::new(VecModel::from(badges)),
        stretch: ss(stretch_ideas()[c.ui.stretch_index % stretch_ideas().len()]),
        rock_plant: ss(rock_stage(state.pet_rock_pats).0),
        rock_label: ss(rock_label(state.pet_rock_pats)),
    }
}

/// The Wabi-Sabi Rest room (games room + meditation).
pub fn wabi_rest_data(c: &BreakRoomController, dark: bool, card: slint::Color) -> WabiRestData {
    let p = c.progression();
    let state = &c.record().state;
    let rest = c.ui.rest;
    let b = c.ui.breathing;
    let frame = breath_frame(b.elapsed);
    WabiRestData {
        room: match c.ui.room {
            Room::Games => 0,
            Room::Meditation => 1,
            Room::Achievements => 2,
        },
        rest_title: ss(format!(
            "Rest, {} minute{}",
            rest.minutes,
            if rest.minutes == 1 { "" } else { "s" }
        )),
        clock: ss(rest.face()),
        running: rest.running,
        progress: rest.progress() as f32,
        dark,
        ring_bg: ring_vessel(card, dark),
        water_fraction: p.xp_percent as f32 / 100.0,
        water_label: ss(format!("{}/{GAME_COUNT}", p.unlocked.len())),
        games_note: ss(format!("{} of {GAME_COUNT} available", p.unlocked.len())),
        cards: ModelRc::new(VecModel::from(cards(c))),
        tree: c.record().rest_tree as i32,
        rock_plant: ss(rock_stage(state.pet_rock_pats).0),
        rock_label: ss(rock_label(state.pet_rock_pats)),
        breath_on: b.on,
        breath_size: frame.size as f32,
        breath_count: ss(if b.on {
            frame.remaining.to_string()
        } else {
            String::new()
        }),
        breath_title: ss(if b.on {
            BREATH_PHASES[frame.phase].0
        } else {
            "Four, seven, eight"
        }),
        breath_desc: ss(if b.on {
            "Follow the ring. Count with it, do not rush the hold."
        } else {
            "In for four, hold for seven, out for eight. Four rounds and the break is over."
        }),
        breath_button: ss(if b.on { "STOP" } else { "BEGIN" }),
        breath_round: ss(if b.on {
            format!("Round {} of 4", frame.round)
        } else {
            "Four rounds, about eighty seconds.".to_string()
        }),
        breath_phase: frame.phase as i32,
    }
}

/// The empty rest ring as production composites it: the fluid shader's display colour (black ink
/// rgb(.035,.035,.025), or paper ink rgb(.91,.89,.81) in dark) at alpha 0, premultiplied - so it is
/// added to the card behind it.
pub fn ring_vessel(card: slint::Color, dark: bool) -> slint::Color {
    let ink: [f32; 3] = if dark {
        [0.91, 0.89, 0.81]
    } else {
        [0.035, 0.035, 0.025]
    };
    let add = |c: u8, i: f32| (f32::from(c) + i * 255.0).round().min(255.0) as u8;
    slint::Color::from_rgb_u8(
        add(card.red(), ink[0]),
        add(card.green(), ink[1]),
        add(card.blue(), ink[2]),
    )
}

const MONTHS: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];

/// `formatEarnedDay` (`toLocaleDateString` month long, day, year; shown upper-case). Production
/// follows the browser's locale; native uses the en-US order production shows on an English system.
pub fn earned_day(iso: &str) -> String {
    match study_tracker_core::dashboard::civil::CivilDate::parse_iso(iso) {
        Some(d) => format!(
            "{} {}, {}",
            MONTHS[d.month() as usize - 1],
            d.day(),
            d.year()
        ),
        None => String::new(),
    }
}

fn page_data(page: Option<&AlbumPage>) -> AlbumPageData {
    let Some(page) = page else {
        return AlbumPageData::default();
    };
    let rows: Vec<AlbumRow> = page
        .rows
        .iter()
        .map(|r| AlbumRow {
            day: ss(r.earned_on.as_deref().map(earned_day).unwrap_or_default()),
            art: crate::break_room_art::art(&r.id),
            jp: ss(japanese_name(&r.id).unwrap_or_default()),
            en: ss(r.name),
            how: ss(r.how.clone()),
        })
        .collect();
    AlbumPageData {
        present: true,
        rows: ModelRc::new(VecModel::from(rows)),
        number: ss(page.number.to_string()),
        left: page.left,
        empty: page.empty,
    }
}

/// `pageStackShadow`'s layer count.
fn stack_layers(remaining: usize, total: usize) -> i32 {
    ((remaining as f64 / total.max(2) as f64 * 6.0).round() as i32).clamp(1, 6)
}

/// The album pages for the current achievements, and how many spreads they make.
pub fn album_pages_now(c: &BreakRoomController) -> Vec<AlbumPage> {
    album_pages(&album_entries(
        c.achievements(),
        &c.record().state.achievement_earned_on_dates,
    ))
}

/// The album scene for the current state and motion, laid out exactly as `BookGallery` renders.
pub fn album_data(c: &BreakRoomController, dark: bool) -> AlbumData {
    let pages = album_pages_now(c);
    let a = c.ui.album;
    let spread_count = (pages.len() / 2).max(1);
    let index = a.spread.min(spread_count - 1);
    let turning = matches!(a.anim, AlbumAnim::Next | AlbumAnim::Prev);
    let base = if turning { a.turn_from } else { index };
    let (l, r) = (base * 2, base * 2 + 1);
    let at = |i: isize| if i < 0 { None } else { pages.get(i as usize) };
    let (mut left, mut right, mut front, mut back) = (at(l as isize), at(r as isize), None, None);
    match a.anim {
        AlbumAnim::Next => {
            right = at(r as isize + 2);
            front = at(r as isize);
            back = at(r as isize + 1);
        }
        AlbumAnim::Prev => {
            left = at(l as isize - 2);
            front = at(l as isize);
            back = at(l as isize - 1);
        }
        AlbumAnim::Open => right = at(1),
        _ => {}
    }
    let opening = a.anim == AlbumAnim::Open;
    let closing = a.anim == AlbumAnim::Close;
    AlbumData {
        dark,
        open: a.open,
        show_left: a.open && !closing,
        show_right: a.open || opening,
        show_closed: !a.open || closing,
        show_cover: !a.open || closing || opening,
        slide_open: (a.open && !closing) || opening,
        anim: match a.anim {
            AlbumAnim::None => 0,
            AlbumAnim::Open => 1,
            AlbumAnim::Close => 2,
            AlbumAnim::Next => 3,
            AlbumAnim::Prev => 4,
        },
        cover_seq: a.cover_seq,
        leaf_seq: a.leaf_seq,
        left_page: page_data(left),
        right_page: page_data(right),
        leaf_front: page_data(front),
        leaf_back: page_data(back),
        first_page: page_data(pages.first()),
        left_layers: stack_layers(l + 1, pages.len()),
        right_layers: stack_layers(pages.len().saturating_sub(r), pages.len()),
        at_first: index == 0,
        at_last: index + 1 >= spread_count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_strings() {
        assert_eq!(rock_label(1), "Pet Rock \u{b7} 1 pat");
        assert_eq!(rock_label(1234), "Cosmic Rock \u{b7} 1234 pats");
        assert_eq!(quotes().len(), 100);
        assert_eq!(stretch_ideas().len(), 10);
        // a pinned Math.random() of 0.25 picks production's index 25 / 2
        assert!(quote(25).starts_with("\u{201c}Within you, there is a stillness"));
        assert_eq!(
            stretch_ideas()[2],
            "Stand up, reach arms to the ceiling, side bend"
        );
        assert_eq!(earned_day("2026-09-28"), "SEPTEMBER 28, 2026");
        assert_eq!(earned_day("2026-08-30"), "AUGUST 30, 2026");
        assert_eq!(earned_day("junk"), "");
        // production pixels: light card (239,235,224) -> (248,244,230); dark (38,36,24) -> (255,255,231)
        let light = ring_vessel(slint::Color::from_rgb_u8(239, 235, 224), false);
        assert_eq!((light.red(), light.green(), light.blue()), (248, 244, 230));
        let dark = ring_vessel(slint::Color::from_rgb_u8(38, 36, 24), true);
        assert_eq!((dark.red(), dark.green(), dark.blue()), (255, 255, 231));
        assert_eq!(stack_layers(1, 14), 1);
        assert_eq!(stack_layers(13, 14), 6);
        assert_eq!(stack_layers(7, 14), 3);
    }
}

// ------------------------------------------------------------------------------ game screens

use study_tracker_core::break_room::catalog::GameId;
use study_tracker_core::break_room::countries::filter_countries;
use study_tracker_core::break_room::durak::{attack_on_top, Card, DurakView, Phase, Side};
use study_tracker_core::break_room::flaggle::PreviewRequest;
use study_tracker_core::break_room::geodle::{self, ClueState};
use study_tracker_core::break_room::wordle::{self, LetterState, MAX_GUESSES, WORD_LENGTH};

use crate::{
    CountryOption, DCard, DCol, DurakData, FlagRow, FlaggleData, GamesData, GeoClue, GeoRow,
    GeodleData, WKey, WTile, WordleData,
};

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

/// `toLocaleString()` in English: thousands separated by commas.
fn grouped(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn letter_state(s: LetterState) -> i32 {
    match s {
        LetterState::Correct => 2,
        LetterState::Present => 3,
        LetterState::Absent => 4,
    }
}

pub fn wordle_data(c: &BreakRoomController) -> WordleData {
    let p = &c.record().state.wordle;
    let mut tiles: Vec<WTile> = Vec::with_capacity(30);
    for guess in &p.guesses {
        for (l, st) in wordle::score_guess(guess, &p.answer) {
            tiles.push(WTile {
                letter: ss(l.to_ascii_uppercase().to_string()),
                state: letter_state(st),
            });
        }
    }
    if !p.completed && tiles.len() < 30 {
        let draft: Vec<char> = c.ui.wordle_draft.chars().collect();
        for i in 0..WORD_LENGTH {
            let letter = draft
                .get(i)
                .map(|l| l.to_ascii_uppercase().to_string())
                .unwrap_or_default();
            let state = if letter.is_empty() { 0 } else { 1 };
            tiles.push(WTile {
                letter: ss(letter),
                state,
            });
        }
    }
    while tiles.len() < MAX_GUESSES * WORD_LENGTH {
        tiles.push(WTile {
            letter: ss(""),
            state: 0,
        });
    }
    tiles.truncate(30);
    let keyboard = wordle::keyboard_state(&p.guesses, &p.answer);
    let key = |l: char| WKey {
        label: ss(l.to_ascii_uppercase().to_string()),
        state: keyboard
            .iter()
            .find(|(k, _)| *k == l)
            .map_or(0, |(_, s)| letter_state(*s)),
        row: 0,
        wide: false,
        key: ss(l.to_string()),
    };
    let row = |letters: &str| letters.chars().map(key).collect::<Vec<_>>();
    let mut bottom = vec![WKey {
        label: ss("Enter"),
        state: 0,
        row: 2,
        wide: true,
        key: ss("enter"),
    }];
    bottom.extend(row("zxcvbnm"));
    bottom.push(WKey {
        label: ss("⌫"),
        state: 0,
        row: 2,
        wide: true,
        key: ss("back"),
    });
    WordleData {
        tiles: model(tiles),
        keys0: model(row("qwertyuiop")),
        keys1: model(row("asdfghjkl")),
        keys2: model(bottom),
        status: ss(if p.completed {
            if p.won {
                format!("Solved in {}.", p.guesses.len())
            } else {
                format!("The word was {}.", p.answer.to_uppercase())
            }
        } else if c.ui.wordle_message.is_empty() {
            "Type a guess, then press Enter.".into()
        } else {
            c.ui.wordle_message.clone()
        }),
        counts: ss(format!(
            "{} answers \u{b7} {} accepted guesses",
            grouped(wordle::answer_count()),
            grouped(wordle::accepted_guess_count())
        )),
        hard: p.hard_mode,
        hard_locked: p.hard_mode_locked(),
        completed: p.completed,
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn geodle_data(c: &BreakRoomController) -> GeodleData {
    let p = &c.record().state.geodle;
    let left = geodle::MAX_GUESSES.saturating_sub(p.guesses.len());
    let rows: Vec<GeoRow> = p
        .guesses
        .iter()
        .rev()
        .map(|g| GeoRow {
            country: ss(g.as_str()),
            clues: model(
                geodle::score_guess(g, &p.answer)
                    .into_iter()
                    .map(|cl| GeoClue {
                        glyph: ss(match cl.state {
                            ClueState::Higher => "▲",
                            ClueState::Lower => "▼",
                            ClueState::Match => "✓",
                            ClueState::Close => "≈",
                            ClueState::Miss => "×",
                        }),
                        value: ss(cl.value),
                        state: match cl.state {
                            ClueState::Match => 0,
                            ClueState::Miss => 1,
                            ClueState::Higher | ClueState::Lower => 2,
                            ClueState::Close => 3,
                        },
                        hint: ss(cl.hint),
                    })
                    .collect(),
            ),
        })
        .collect();
    GeodleData {
        header: ss(format!(
            "{} countries \u{b7} {} left",
            geodle::countries_count(),
            plural(left, "guess", "guesses")
        )),
        draft: ss(c.ui.geodle_draft.as_str()),
        dropdown: c.ui.geodle_dropdown,
        options: model(
            filter_countries(&c.ui.geodle_draft)
                .into_iter()
                .take(80)
                .map(|co| CountryOption {
                    name: ss(co.name),
                    detail: ss(co.continent),
                    flag: Default::default(),
                })
                .collect(),
        ),
        status: ss(if p.completed {
            if p.won {
                format!("Solved in {}.", p.guesses.len())
            } else {
                format!("The country was {}.", p.answer)
            }
        } else if c.ui.geodle_message.is_empty() {
            "Use the clues after each guess to narrow it down.".into()
        } else {
            c.ui.geodle_message.clone()
        }),
        rows: model(rows),
        completed: p.completed,
    }
}

pub fn flaggle_data(c: &BreakRoomController) -> FlaggleData {
    let p = &c.record().state.flaggle;
    let left = study_tracker_core::break_room::flaggle::MAX_GUESSES.saturating_sub(p.guesses.len());
    let preview = match c.flaggle_preview() {
        PreviewRequest::Empty => None,
        PreviewRequest::Full => crate::break_room_flags::preview(&p.answer, &[], true),
        PreviewRequest::RevealedBy(names) => {
            crate::break_room_flags::preview(&p.answer, &names, false)
        }
    };
    FlaggleData {
        header: ss(format!(
            "{} countries \u{b7} {} left",
            geodle::countries_count(),
            plural(left, "guess", "guesses")
        )),
        has_preview: preview.is_some(),
        preview: preview.unwrap_or_default(),
        draft: ss(c.ui.flaggle_draft.as_str()),
        dropdown: c.ui.flaggle_dropdown,
        options: model(if c.ui.flaggle_dropdown {
            filter_countries(&c.ui.flaggle_draft)
                .into_iter()
                .take(80)
                .map(|co| CountryOption {
                    name: ss(co.name),
                    detail: ss(""),
                    flag: crate::break_room_flags::thumbnail(co.name),
                })
                .collect()
        } else {
            Vec::new()
        }),
        status: ss(if p.completed {
            if p.won {
                format!("Solved in {}.", p.guesses.len())
            } else {
                format!("The flag was {}.", p.answer)
            }
        } else if c.ui.flaggle_message.is_empty() {
            "Only colors shared with the target flag stay visible.".into()
        } else {
            c.ui.flaggle_message.clone()
        }),
        rows: model(
            p.guesses
                .iter()
                .rev()
                .map(|g| FlagRow {
                    country: ss(g.country.as_str()),
                    similarity: ss(format!("{:.1}%", g.similarity)),
                    flag: crate::break_room_flags::thumbnail(&g.country),
                })
                .collect(),
        ),
        completed: p.completed,
    }
}

fn face(card: Card) -> DCard {
    DCard {
        rank: ss(card.rank.label()),
        suit: ss(card.suit.symbol().to_string()),
        red: card.suit.is_red(),
        kind: 0,
        selected: false,
        playable: false,
    }
}

pub fn durak_data(c: &BreakRoomController) -> DurakData {
    let puzzle = &c.record().state.durak;
    let session = &c.durak;
    let game = match session.view(puzzle) {
        DurakView::AllSolved => {
            return DurakData {
                view: 1,
                ..Default::default()
            }
        }
        DurakView::Empty => {
            return DurakData {
                view: 2,
                ..Default::default()
            }
        }
        DurakView::Game(g) => g,
    };
    let slot = DCard {
        kind: 2,
        ..Default::default()
    };
    let table: Vec<DCol> = game
        .table
        .iter()
        .map(|e| {
            let attack = face(e.attack);
            let defense = e.defense.map(face).unwrap_or_else(|| slot.clone());
            let (top, bottom) = if attack_on_top(game.phase, e) {
                (attack, defense)
            } else {
                (defense, attack)
            };
            DCol {
                top,
                bottom,
                active: e.defense.is_none(),
            }
        })
        .collect();
    let hand: Vec<DCard> = game
        .player_hand
        .iter()
        .enumerate()
        .map(|(i, &card)| DCard {
            selected: session.selected.contains(&i),
            playable: session.playable(i),
            ..face(card)
        })
        .collect();
    let (can_defend, can_slide) = session.can_defend_and_slide();
    DurakData {
        view: 0,
        hint: ss(if puzzle.hint.is_empty() {
            "CPU has some strong cards...".to_string()
        } else {
            puzzle.hint.clone()
        }),
        trump: ss(game.trump.symbol().to_string()),
        cpu_cards: game.cpu_hand.len() as i32,
        message: ss(game.message.as_str()),
        table: model(table),
        hand: model(hand),
        phase: match game.phase {
            Phase::PlayerAttack => 0,
            Phase::PlayerDefense => 1,
            Phase::PlayerThrow => 2,
            _ => 3,
        },
        finished: match (game.phase, game.winner) {
            (Phase::Finished, Some(Side::Player)) => 1,
            (Phase::Finished, _) => 2,
            _ => 0,
        },
        failures: ss(format!("Failures: {}", puzzle.failures)),
        has_selection: !session.selected.is_empty(),
        can_defend,
        can_slide,
    }
}

/// The open game's screen (only the open one is built).
pub fn games_data(c: &BreakRoomController) -> GamesData {
    let Some(game) = c.ui.open_game else {
        return GamesData {
            open: -1,
            ..Default::default()
        };
    };
    let mut d = GamesData {
        open: game.index() as i32,
        ..Default::default()
    };
    match game {
        GameId::Wordle => d.wordle = wordle_data(c),
        GameId::Geodle => d.geodle = geodle_data(c),
        GameId::Flaggle => d.flaggle = flaggle_data(c),
        GameId::DailyDurak => d.durak = durak_data(c),
        GameId::Travle | GameId::DailySkribbl => {}
    }
    d
}
