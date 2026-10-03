//! Break Room domain tests. The `golden_*` tests compare against fixtures computed by production's
//! own TypeScript (`scripts/stage20-goldens.mjs`); the rest pin the App-level semantics read from
//! `desktop/src/App.tsx` at their boundaries.

use std::collections::BTreeMap;

use serde_json::Value;

use super::achievements::*;
use super::catalog::*;
use super::countries::*;
use super::daily::*;
use super::durak::*;
use super::flaggle;
use super::geodle::{self, ClueState, CountrySubmit, GeodlePuzzle};
use super::rest::*;
use super::state::*;
use super::wordle::{self, LetterState, WordlePuzzle, WordleSubmit};
use crate::dashboard::civil::{CivilDate, FixedOffsetClock, LocalClock};
use crate::timer::WallTimestamp;

fn fixture(name: &str) -> Value {
    let text = match name {
        "daily" => include_str!("../../tests/fixtures/break_room/daily.json"),
        "wordle" => include_str!("../../tests/fixtures/break_room/wordle.json"),
        "geodle" => include_str!("../../tests/fixtures/break_room/geodle.json"),
        "durak" => include_str!("../../tests/fixtures/break_room/durak.json"),
        "album" => include_str!("../../tests/fixtures/break_room/album.json"),
        _ => unreachable!(),
    };
    serde_json::from_str(text).unwrap()
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn d(iso: &str) -> CivilDate {
    CivilDate::parse_iso(iso).unwrap()
}

const ZURICH_SUMMER: FixedOffsetClock = FixedOffsetClock::new(2 * 3600);

// ---------------------------------------------------------------- catalog / daily

#[test]
fn the_catalog_is_productions_six_games_in_order() {
    let names: Vec<&str> = GAMES.iter().map(|g| g.name).collect();
    assert_eq!(
        names,
        [
            "Daily Durak",
            "Wordle",
            "Travle",
            "Flaggle",
            "Daily Skribbl",
            "Geodle"
        ]
    );
    assert_eq!(GAME_COUNT, 6);
    assert_eq!(
        GameId::from_name("Daily Skribbl"),
        Some(GameId::DailySkribbl)
    );
    assert_eq!(GameId::from_name("daily skribbl"), None);
    assert_eq!(GameId::Travle.info().availability, Availability::Local);
    assert_eq!(
        GameId::DailySkribbl.info().availability,
        Availability::NetworkStage22
    );
    let local = GAMES
        .iter()
        .filter(|g| g.availability == Availability::Local)
        .count();
    assert_eq!(local, 5);
}

#[test]
fn days_since_first_puzzle_follows_v8_date_parsing() {
    assert_eq!(days_since_first_puzzle("2026-01-01"), 0);
    assert_eq!(days_since_first_puzzle("2025-12-31"), 0, "clamped at 0");
    assert_eq!(days_since_first_puzzle("2026-02-01"), 31);
    assert_eq!(
        days_since_first_puzzle("2026-02-30"),
        60,
        "V8 rolls Feb 30 to Mar 2"
    );
    assert_eq!(days_since_first_puzzle("2026-03-02"), 60);
    assert_eq!(days_since_first_puzzle("2027-01-01"), 365);
    assert_eq!(days_since_first_puzzle("2028-02-29"), 789);
    assert_eq!(days_since_first_puzzle("2028-03-01"), 790);
    for bad in ["", "garbage", "2026-13-01", "2026-02-32", "2026-1-01"] {
        assert_eq!(days_since_first_puzzle(bad), 0, "{bad}");
    }
    assert_eq!(to_base36(0), "0");
    assert_eq!(to_base36(35), "z");
    assert_eq!(to_base36(u32::MAX), "1z141z3");
}

#[test]
fn golden_daily_answers_and_ids_match_production_for_every_date_and_salt() {
    let g = fixture("daily");
    for row in g["wordle"].as_array().unwrap() {
        let (salt, date) = (s(&row["salt"]), s(&row["date"]));
        assert_eq!(
            wordle::answer_for_date(date, salt),
            s(&row["answer"]),
            "{salt} {date}"
        );
        assert_eq!(wordle::wordle_puzzle_id(date, salt), s(&row["id"]));
    }
    for row in g["geodle"].as_array().unwrap() {
        let (salt, date) = (s(&row["salt"]), s(&row["date"]));
        assert_eq!(
            geodle::answer_for_date(date, salt),
            s(&row["answer"]),
            "{salt} {date}"
        );
        // Flaggle's answer function is the identical code over the same table.
        assert_eq!(flaggle::answer_for_date(date, salt), s(&row["answer"]));
        assert_eq!(geodle::geodle_puzzle_id(date, salt), s(&row["id"]));
    }
}

#[test]
fn a_salted_daily_answer_is_stable_within_a_day_and_changes_across_boundaries() {
    let salt = "salt-a";
    let same = wordle::answer_for_date("2026-10-01", salt);
    assert_eq!(same, wordle::answer_for_date("2026-10-01", salt));
    // month, year and leap-day boundaries each start a new position in the order
    for (a, b) in [
        ("2026-10-31", "2026-11-01"),
        ("2026-12-31", "2027-01-01"),
        ("2028-02-28", "2028-02-29"),
        ("2028-02-29", "2028-03-01"),
    ] {
        assert_ne!(days_since_first_puzzle(a), days_since_first_puzzle(b));
    }
    // a different salt is a different order
    let others = (0..30)
        .filter(|i| {
            let date = CivilDate::parse_iso("2026-10-01")
                .unwrap()
                .add_days(*i)
                .to_iso();
            wordle::answer_for_date(&date, salt) != wordle::answer_for_date(&date, "salt-b")
        })
        .count();
    assert!(others > 20);
}

// ---------------------------------------------------------------- countries

#[test]
fn country_table_and_name_matching_match_production() {
    assert_eq!(countries().len(), 195);
    assert_eq!(countries()[0].name, "Afghanistan");
    let g = fixture("geodle");
    assert_eq!(g["countryCount"], 195);
    for row in g["find"].as_array().unwrap() {
        let q = s(&row["query"]);
        assert_eq!(
            find_country(q).map(|c| c.name),
            row["found"].as_str(),
            "find {q:?}"
        );
        let codes: Vec<&str> = filter_countries(q).iter().map(|c| c.code).collect();
        let expected: Vec<&str> = row["filtered"].as_array().unwrap().iter().map(s).collect();
        assert_eq!(codes, expected, "filter {q:?}");
    }
    assert_eq!(normalize_country_name("  Côte d'Ivoire "), "c te d ivoire");
}

// ---------------------------------------------------------------- wordle

#[test]
fn golden_wordle_word_lists_scores_keyboard_and_hard_mode() {
    let g = fixture("wordle");
    assert_eq!(
        wordle::answer_count(),
        g["answerCount"].as_u64().unwrap() as usize
    );
    assert_eq!(
        wordle::accepted_guess_count(),
        g["acceptedCount"].as_u64().unwrap() as usize
    );
    let letter = |st: LetterState| match st {
        LetterState::Correct => 'c',
        LetterState::Present => 'p',
        LetterState::Absent => 'a',
    };
    for row in g["scores"].as_array().unwrap() {
        let got: String = wordle::score_guess(s(&row["guess"]), s(&row["answer"]))
            .into_iter()
            .map(|(_, st)| letter(st))
            .collect();
        assert_eq!(
            got,
            s(&row["score"]),
            "{} vs {}",
            row["guess"],
            row["answer"]
        );
    }
    for row in g["keyboards"].as_array().unwrap() {
        let guesses: Vec<String> = row["guesses"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| s(v).to_string())
            .collect();
        let mut keys: Vec<String> = wordle::keyboard_state(&guesses, s(&row["answer"]))
            .into_iter()
            .map(|(k, st)| format!("{k}{}", letter(st)))
            .collect();
        keys.sort();
        assert_eq!(keys.concat(), s(&row["state"]));
    }
    for row in g["hard"].as_array().unwrap() {
        let previous: Vec<String> = row["previous"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| s(v).to_string())
            .collect();
        assert_eq!(
            wordle::hard_mode_violation(s(&row["guess"]), &previous, s(&row["answer"])),
            row["violation"].as_str().map(str::to_string),
            "{row}"
        );
    }
    for row in g["normalize"].as_array().unwrap() {
        assert_eq!(wordle::normalize_guess(s(&row["input"])), s(&row["output"]));
    }
    for row in g["accept"].as_array().unwrap() {
        assert_eq!(
            wordle::is_accepted_guess(s(&row["input"])),
            row["accepted"].as_bool().unwrap()
        );
    }
}

#[test]
fn wordle_duplicate_letters_canonical_cases() {
    let states = |g: &str, a: &str| -> String {
        wordle::score_guess(g, a)
            .into_iter()
            .map(|(_, st)| match st {
                LetterState::Correct => 'G',
                LetterState::Present => 'Y',
                LetterState::Absent => '.',
            })
            .collect()
    };
    // a doubled guess letter is yellow only as often as the answer still has it
    assert_eq!(states("speed", "abide"), "..Y.Y");
    // the green claims the letter before any yellow
    assert_eq!(states("eerie", "there"), "Y.Y.G");
    assert_eq!(states("abbey", "babes"), "YYGG.");
    assert_eq!(states("crane", "crane"), "GGGGG");
    assert_eq!(states("aaaaa", "abase"), "G.G..");
}

fn wordle_puzzle(answer: &str) -> WordlePuzzle {
    WordlePuzzle {
        seed_salt: "s".into(),
        active_date: "2026-10-01".into(),
        puzzle_id: wordle::wordle_puzzle_id("2026-10-01", "s"),
        answer: answer.into(),
        ..WordlePuzzle::default()
    }
}

#[test]
fn wordle_submission_rules_messages_and_completion() {
    let mut p = wordle_puzzle("crane");
    assert_eq!(
        p.submit("cra"),
        WordleSubmit::Rejected("Enter 5 letters.".into())
    );
    assert_eq!(
        p.submit("zzzzz"),
        WordleSubmit::Rejected("Not in the word list.".into())
    );
    assert_eq!(
        p.submit("slate"),
        WordleSubmit::Accepted {
            message: String::new()
        }
    );
    // the same word again changes nothing (production's state guard)
    assert_eq!(
        p.submit("SLATE"),
        WordleSubmit::Accepted {
            message: String::new()
        }
    );
    assert_eq!(p.guesses, ["slate"]);
    assert_eq!(
        p.submit("crane"),
        WordleSubmit::Accepted {
            message: "Solved in 2.".into()
        }
    );
    assert!(p.completed && p.won);
    assert_eq!(p.submit("slate"), WordleSubmit::Ignored);

    let mut lose = wordle_puzzle("crane");
    for (i, w) in ["slate", "bloat", "pious", "dumpy", "fight", "jerky"]
        .iter()
        .enumerate()
    {
        let r = lose.submit(w);
        if i == 5 {
            assert_eq!(
                r,
                WordleSubmit::Accepted {
                    message: "Answer: CRANE".into()
                }
            );
        }
    }
    assert!(lose.completed && !lose.won);
    assert_eq!(lose.guesses.len(), wordle::MAX_GUESSES);
}

#[test]
fn wordle_hard_mode_is_enforced_and_locked_mid_puzzle() {
    let mut p = wordle_puzzle("crane");
    assert!(p.toggle_hard_mode());
    assert!(p.hard_mode);
    p.submit("trace"); // r, a, c present; e correct
    assert!(p.hard_mode_locked());
    assert!(
        !p.toggle_hard_mode(),
        "locked while a puzzle is in progress"
    );
    // positions first, in the order they were revealed (r at 2, a at 3, e at 5)
    assert_eq!(
        p.submit("bloat"),
        WordleSubmit::Rejected("R must be in position 2.".into())
    );
    // then the counts of revealed letters (c is present in "trace")
    assert_eq!(
        p.submit("grape"),
        WordleSubmit::Rejected("Guess must contain C.".into())
    );
    // then letters ruled out (t)
    assert_eq!(
        p.submit("crate"),
        WordleSubmit::Rejected("T has been eliminated.".into())
    );
    assert_eq!(
        p.submit("brace"),
        WordleSubmit::Accepted {
            message: String::new()
        }
    );
}

#[test]
fn wordle_load_normalization_and_day_rollover_follow_production() {
    let today = "2026-10-01";
    let mut p = WordlePuzzle::fresh(today, "salt-a", true);
    p.submit(&p.answer.clone());
    let kept = p.clone().normalized(today, || unreachable!());
    assert_eq!(kept, p, "today's puzzle survives a reload unchanged");
    let next = p.clone().normalized("2026-10-02", || unreachable!());
    assert!(next.guesses.is_empty() && !next.completed && !next.won);
    assert!(next.hard_mode, "hard mode is a preference that survives");
    assert_eq!(next.answer, wordle::answer_for_date("2026-10-02", "salt-a"));
    // a missing salt gets one (and only one) new salt
    let fresh = WordlePuzzle::default().normalized(today, || "new".into());
    assert_eq!(fresh.seed_salt, "new");
    assert_eq!(fresh.puzzle_id, wordle::wordle_puzzle_id(today, "new"));
    // invalid stored guesses are dropped, at most six kept
    let mut junk = wordle_puzzle("crane");
    junk.guesses = vec!["CRANE".into(), "slate".into(), "abc".into()];
    assert_eq!(junk.normalized(today, || unreachable!()).guesses, ["slate"]);
    // ensure_today is a no-op for today's puzzle
    let mut again = p.clone();
    assert!(!again.ensure_today(today, || unreachable!()));
    assert!(again.ensure_today("2026-10-02", || unreachable!()));
}

// ---------------------------------------------------------------- geodle

#[test]
fn golden_geodle_clues_values_and_hints_match_production() {
    let g = fixture("geodle");
    let state = |c: ClueState| match c {
        ClueState::Match => "match",
        ClueState::Miss => "miss",
        ClueState::Higher => "higher",
        ClueState::Lower => "lower",
        ClueState::Close => "close",
    };
    for row in g["clues"].as_array().unwrap() {
        let got = geodle::score_guess(s(&row["guess"]), s(&row["answer"]));
        let want = row["clues"].as_array().unwrap();
        assert_eq!(got.len(), want.len());
        for (c, w) in got.iter().zip(want) {
            assert_eq!(c.label, s(&w["label"]));
            assert_eq!(c.value, s(&w["value"]), "{} {}", row["guess"], c.label);
            assert_eq!(
                state(c.state),
                s(&w["state"]),
                "{} {}",
                row["guess"],
                c.label
            );
            assert_eq!(c.hint, s(&w["hint"]));
        }
    }
    for row in g["compact"].as_array().unwrap() {
        assert_eq!(
            geodle::format_compact(row["value"].as_u64().unwrap()),
            s(&row["text"]),
            "{}",
            row["value"]
        );
    }
}

#[test]
fn geodle_submission_rules() {
    let mut p = GeodlePuzzle {
        answer: "Rwanda".into(),
        active_date: "2026-10-01".into(),
        ..GeodlePuzzle::default()
    };
    assert_eq!(p.submit("Atlantis"), CountrySubmit::NotACountry);
    assert_eq!(
        p.submit("  switzerland"),
        CountrySubmit::Accepted {
            message: String::new()
        }
    );
    assert_eq!(
        p.guesses,
        ["Switzerland"],
        "stored under production's own name"
    );
    assert_eq!(p.submit("Switzerland"), CountrySubmit::AlreadyGuessed);
    assert_eq!(
        p.submit("rwanda"),
        CountrySubmit::Accepted {
            message: "Solved in 2.".into()
        }
    );
    assert!(p.won && p.completed);
    assert_eq!(p.submit("Chad"), CountrySubmit::Ignored);

    let mut lose = GeodlePuzzle {
        answer: "Rwanda".into(),
        ..GeodlePuzzle::default()
    };
    for (i, c) in countries()
        .iter()
        .filter(|c| c.name != "Rwanda")
        .take(7)
        .enumerate()
    {
        let r = lose.submit(c.name);
        if i == 6 {
            assert_eq!(
                r,
                CountrySubmit::Accepted {
                    message: "Answer: Rwanda.".into()
                }
            );
        }
    }
    assert!(lose.completed && !lose.won);
}

// ---------------------------------------------------------------- flaggle

#[test]
fn flaggle_pixel_rules_match_production() {
    let opaque = |r, g, b| [r, g, b, 255u8];
    assert!(flaggle::pixels_match(
        &opaque(255, 0, 0),
        &opaque(255, 30, 30)
    ));
    // distance exactly 52 still matches, just above does not
    assert!(flaggle::pixels_match(
        &opaque(100, 100, 100),
        &opaque(152, 100, 100)
    ));
    assert!(!flaggle::pixels_match(
        &opaque(100, 100, 100),
        &opaque(153, 100, 100)
    ));
    // translucent pixels never match
    assert!(!flaggle::pixels_match(
        &[255, 0, 0, 127],
        &opaque(255, 0, 0)
    ));
    // similarity: 3 visible target pixels, 2 matched -> 66.7; transparent ones ignored
    let target = [
        opaque(255, 0, 0),
        opaque(255, 0, 0),
        opaque(0, 0, 255),
        [0, 0, 0, 0],
    ]
    .concat();
    let guess = [
        opaque(250, 0, 0),
        opaque(255, 10, 0),
        opaque(255, 255, 255),
        opaque(1, 1, 1),
    ]
    .concat();
    assert_eq!(flaggle::similarity(&target, &guess), 66.7);
    assert_eq!(flaggle::similarity(&[0, 0, 0, 0], &[0, 0, 0, 0]), 0.0);
    // reveal: the unmatched opaque pixel becomes rgb(25,25,25)
    let shown = flaggle::reveal(&target, &[&guess]);
    assert_eq!(&shown[8..12], &[25, 25, 25, 255]);
    assert_eq!(&shown[0..8], &target[0..8]);
    assert_eq!(
        flaggle::flag_asset_name("Switzerland").as_deref(),
        Some("ch.svg")
    );
}

#[test]
fn flaggle_flow_and_preview() {
    let mut p = flaggle::FlagglePuzzle {
        answer: "Japan".into(),
        ..Default::default()
    };
    assert_eq!(p.preview(), flaggle::PreviewRequest::Empty);
    assert_eq!(p.check("Narnia"), Err(CountrySubmit::NotACountry));
    let name = p.check("bangladesh").unwrap();
    p.record(name, 41.2);
    assert_eq!(p.check("Bangladesh"), Err(CountrySubmit::AlreadyGuessed));
    assert_eq!(
        p.preview(),
        flaggle::PreviewRequest::RevealedBy(vec!["Bangladesh".into()])
    );
    assert_eq!(
        p.record("Japan", 100.0),
        CountrySubmit::Accepted {
            message: "Solved in 2.".into()
        }
    );
    assert_eq!(p.preview(), flaggle::PreviewRequest::Full);
}

// ---------------------------------------------------------------- durak

fn stored_from_json(v: &Value) -> DurakPuzzle {
    let strings = |k: &str| {
        v[k].as_array()
            .unwrap()
            .iter()
            .map(|x| s(x).to_string())
            .collect::<Vec<_>>()
    };
    DurakPuzzle {
        player_hand: strings("playerHand"),
        cpu_hand: strings("cpuHand"),
        trump_suit: s(&v["trumpSuit"]).into(),
        table: v["table"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| StoredEntry {
                attack: s(&e["attack"]).into(),
                defense: e["defense"].as_str().map(Into::into),
                attack_by: e["attackBy"].as_str().map(Into::into),
                defense_by: e["defenseBy"].as_str().map(Into::into),
            })
            .collect(),
        discard_pile: strings("discardPile"),
        phase: s(&v["phase"]).into(),
        winner: v["winner"].as_str().map(Into::into),
        message: s(&v["message"]).into(),
        ..DurakPuzzle::default()
    }
}

fn game_from_json(v: &Value) -> GameState {
    stored_from_json(v).to_game_state().unwrap()
}

#[test]
fn golden_durak_daily_deals_solver_and_hints_match_production() {
    let g = fixture("durak");
    for p in g["puzzles"].as_array().unwrap() {
        let seed = s(&p["seed"]);
        let first = find_daily_puzzle(seed, 0.0);
        if p["state"].is_null() {
            assert!(first.is_none(), "{seed}");
            continue;
        }
        let first = first.expect(seed);
        assert_eq!(
            first.initial,
            game_from_json(&p["state"]),
            "deal for {seed}"
        );
        assert_eq!(first.hint, s(&p["hintFirst"]), "first hint for {seed}");
        let last = find_daily_puzzle(seed, 0.999_999).unwrap();
        assert_eq!(last.hint, s(&p["hintLast"]), "last hint for {seed}");
        assert_eq!(first.hint_candidates.first().unwrap(), s(&p["hintFirst"]));
        assert_eq!(first.hint_candidates.last().unwrap(), s(&p["hintLast"]));
    }
}

#[test]
fn golden_durak_play_trajectories_match_production_step_by_step() {
    let g = fixture("durak");
    let puzzles: BTreeMap<String, Value> = g["puzzles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (s(&p["seed"]).to_string(), p["state"].clone()))
        .collect();
    let mut steps = 0;
    for t in g["trajectories"].as_array().unwrap() {
        let mut state = game_from_json(&puzzles[s(&t["seed"])]);
        for step in t["steps"].as_array().unwrap() {
            let cards: Vec<Card> = step["cards"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| Card::parse(s(c)).unwrap())
                .collect();
            state = match s(&step["type"]) {
                "attack" => process_cpu_turn(&player_attack(&state, &cards)),
                "throw" => process_cpu_turn(&player_throw(&state, &cards)),
                "pass" => process_cpu_turn(&player_pass_throw(&state)),
                "defend" => process_cpu_turn(&defend_one_card(&state, cards[0])),
                "pickup" => process_cpu_turn(&player_pick_up(&state)),
                "slide" => process_cpu_turn(&execute_slide(&state, cards[0])),
                other => panic!("{other}"),
            };
            assert_eq!(
                state,
                game_from_json(&step["after"]),
                "{} step {step}",
                t["seed"]
            );
            steps += 1;
        }
    }
    assert!(steps > 1000);
}

fn c(key: &str) -> Card {
    Card::parse(key).unwrap()
}

#[test]
fn durak_rules_beat_slide_and_limits() {
    assert!(can_beat(c("7♥"), c("6♥"), Suit::Spades));
    assert!(!can_beat(c("6♥"), c("7♥"), Suit::Spades));
    assert!(
        can_beat(c("6♠"), c("A♥"), Suit::Spades),
        "any trump beats a non-trump"
    );
    assert!(!can_beat(c("A♥"), c("6♠"), Suit::Spades));
    assert!(
        !can_beat(c("K♦"), c("6♣"), Suit::Spades),
        "off-suit non-trump never beats"
    );
    assert_eq!(Card::parse("10♥"), Some(Card::new(Rank::Ten, Suit::Hearts)));
    assert_eq!(Card::parse("1♥"), None);
    assert_eq!(Card::parse("10x"), None);
    let state = GameState {
        player_hand: vec![c("7♣"), c("7♦")],
        cpu_hand: vec![c("9♠"), c("6♦")],
        table: vec![TableEntry {
            attack: c("7♥"),
            defense: None,
            attack_by: Side::Cpu,
            defense_by: None,
        }],
        trump: Suit::Spades,
        discard: vec![],
        phase: Phase::PlayerDefense,
        winner: None,
        message: String::new(),
    };
    // both sevens can slide (receiver holds 2 >= 2 undefended after the slide)
    assert_eq!(
        legal_slide_cards(&state, Side::Player),
        vec![c("7♣"), c("7♦")]
    );
    let slid = execute_slide(&state, c("7♣"));
    assert_eq!(slid.phase, Phase::CpuDefense);
    assert_eq!(slid.message, "You slide with 7♣! CPU must defend.");
    // never with the last card
    let mut last = state.clone();
    last.player_hand.truncate(1);
    assert!(legal_slide_cards(&last, Side::Player).is_empty());
    // the CPU's single step: its best defence is the lowest non-trump, else lowest trump
    assert_eq!(
        best_defense(&[c("A♠"), c("9♥"), c("8♥")], c("7♥"), Suit::Spades),
        Some(c("8♥"))
    );
    assert_eq!(
        best_defense(&[c("A♠"), c("7♠")], c("7♥"), Suit::Spades),
        Some(c("7♠"))
    );
    assert_eq!(best_defense(&[c("6♦")], c("7♥"), Suit::Spades), None);
}

#[test]
fn durak_session_three_puzzles_a_day_failures_and_reset() {
    let today = "2026-10-01";
    let mut puzzle = DurakPuzzle::default();
    let mut session = DurakSession::default();
    assert!(session.open(&mut puzzle, today, 0.0));
    assert_eq!(puzzle.seed.as_deref(), Some("2026-10-01_0"));
    assert_eq!(puzzle.failures, 0);
    // reopening is a no-op
    assert!(!session.open(&mut puzzle, today, 0.0));
    // a restart restores the stored game exactly
    let mut restarted = DurakSession::default();
    assert!(!restarted.open(&mut puzzle, today, 0.0));
    assert_eq!(restarted.game, session.game);

    // lose deliberately: always pick up / pass until the CPU wins, then retry
    let mut guard = 0;
    while session.game.as_ref().unwrap().phase != Phase::Finished && guard < 60 {
        let phase = session.game.as_ref().unwrap().phase;
        match phase {
            Phase::PlayerAttack => {
                session.click_card(0);
                session.attack(&mut puzzle, today);
            }
            Phase::PlayerThrow => {
                session.throw_or_pass(&mut puzzle, today, true);
            }
            Phase::PlayerDefense => {
                session.pick_up(&mut puzzle, today);
            }
            _ => unreachable!(),
        }
        guard += 1;
    }
    let finished = session.game.clone().unwrap();
    if finished.winner == Some(Side::Cpu) {
        assert!(!puzzle.completed);
        assert_eq!(puzzle.solved_count, 0);
        assert!(session.retry(&mut puzzle, today, 0.0));
        assert_eq!(puzzle.failures, 1);
        assert_eq!(session.game.as_ref().unwrap().phase, Phase::PlayerAttack);
    }

    // solving: simulate by marking the session solved through `finish`'s public path - win the
    // stored puzzle by searching production's solver line with the real handlers
    let mut solved = 0;
    for _ in 0..3 {
        let mut p = DurakSession::default();
        p.open(&mut puzzle, today, 0.0);
        let won = play_to_win(&mut p, &mut puzzle, today);
        assert!(won, "every daily deal is solver-proven winnable");
        solved += 1;
        assert_eq!(puzzle.solved_count, solved);
        assert!(puzzle.completed);
    }
    let mut after = DurakSession::default();
    after.open(&mut puzzle, today, 0.0);
    assert_eq!(after.view(&puzzle), DurakView::AllSolved);

    // the next day resets the count and the failures
    let mut next_day = DurakSession::default();
    assert!(next_day.open(&mut puzzle, "2026-10-02", 0.0));
    assert_eq!(puzzle.solved_count, 0);
    assert_eq!(puzzle.failures, 0);
    assert_eq!(puzzle.seed.as_deref(), Some("2026-10-02_0"));
}

/// Plays the stored puzzle to a win with the session's own handlers by depth-first search over
/// the player's choices (the CPU is deterministic, so this terminates quickly on these endgames).
fn play_to_win(session: &mut DurakSession, puzzle: &mut DurakPuzzle, today: &str) -> bool {
    fn search(state: &GameState, depth: u32, path: &mut Vec<(&'static str, Vec<Card>)>) -> bool {
        if state.winner == Some(Side::Player) {
            return true;
        }
        if state.winner.is_some() || depth > 24 {
            return false;
        }
        let mut options: Vec<(&'static str, Vec<Card>)> = Vec::new();
        match state.phase {
            Phase::PlayerAttack => {
                for g in valid_attacks(&state.player_hand, Some(6.min(state.cpu_hand.len()))) {
                    for k in 1..=g.len() {
                        options.push(("attack", g[..k].to_vec()));
                    }
                }
            }
            Phase::PlayerThrow => {
                options.push(("pass", vec![]));
                for g in valid_throws(
                    &state.player_hand,
                    &state.table,
                    attack_limit_against_cpu(state),
                ) {
                    for k in 1..=g.len() {
                        options.push(("throw", g[..k].to_vec()));
                    }
                }
            }
            Phase::PlayerDefense => {
                if let Some(t) = state.table.iter().find(|e| e.defense.is_none()) {
                    for card in defense_options(&state.player_hand, t.attack, state.trump) {
                        options.push(("defend", vec![card]));
                    }
                }
                for card in legal_slide_cards(state, Side::Player) {
                    options.push(("slide", vec![card]));
                }
                options.push(("pickup", vec![]));
            }
            _ => return false,
        }
        for (kind, cards) in options {
            let next = match kind {
                "attack" => process_cpu_turn(&player_attack(state, &cards)),
                "throw" => process_cpu_turn(&player_throw(state, &cards)),
                "pass" => process_cpu_turn(&player_pass_throw(state)),
                "defend" => process_cpu_turn(&defend_one_card(state, cards[0])),
                "slide" => process_cpu_turn(&execute_slide(state, cards[0])),
                _ => process_cpu_turn(&player_pick_up(state)),
            };
            path.push((kind, cards));
            if search(&next, depth + 1, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    if !search(session.game.as_ref().unwrap(), 0, &mut path) {
        return false;
    }
    for (kind, cards) in path {
        let hand = session.game.as_ref().unwrap().player_hand.clone();
        session.selected = cards
            .iter()
            .map(|c| hand.iter().position(|h| h == c).unwrap())
            .collect();
        match kind {
            "attack" => session.attack(puzzle, today),
            "throw" => session.throw_or_pass(puzzle, today, false),
            "pass" => session.throw_or_pass(puzzle, today, true),
            "defend" => session.defend(puzzle, today),
            "slide" => session.slide(puzzle, today),
            _ => session.pick_up(puzzle, today),
        };
    }
    session.game.as_ref().unwrap().winner == Some(Side::Player)
}

#[test]
fn durak_card_selection_follows_production() {
    let mut session = DurakSession {
        game: Some(GameState {
            player_hand: vec![c("7♣"), c("7♦"), c("9♥")],
            cpu_hand: vec![c("6♠")],
            table: vec![],
            trump: Suit::Spades,
            discard: vec![],
            phase: Phase::PlayerAttack,
            winner: None,
            message: String::new(),
        }),
        selected: vec![],
    };
    session.click_card(0);
    session.click_card(1);
    assert_eq!(
        session.selected,
        [0],
        "attack size capped by the CPU's one card"
    );
    session.click_card(2);
    assert_eq!(session.selected, [2], "another rank replaces the selection");
    session.click_card(2);
    assert!(session.selected.is_empty(), "clicking again deselects");
}

#[test]
fn durak_stored_puzzle_round_trips_and_rejects_bad_records() {
    let daily = find_daily_puzzle("2026-10-01_0", 0.0).unwrap();
    let mut stored = DurakPuzzle::default();
    stored.store(&daily.initial, 2, false, &daily.hint, "2026-10-01_0");
    assert_eq!(stored.to_game_state(), Some(daily.initial.clone()));
    assert_eq!(stored.failures, 2);
    let mut bad = stored.clone();
    bad.player_hand.push("11♥".into());
    assert_eq!(bad.to_game_state(), None);
    let mut bad_phase = stored.clone();
    bad_phase.phase = "dealing".into();
    assert_eq!(bad_phase.to_game_state(), None);
    // a legacy table entry without attackBy: the first one is the CPU's
    let mut legacy = stored.clone();
    legacy.table = vec![
        StoredEntry {
            attack: "7♥".into(),
            ..StoredEntry::default()
        },
        StoredEntry {
            attack: "7♦".into(),
            ..StoredEntry::default()
        },
    ];
    let state = legacy.to_game_state().unwrap();
    assert_eq!(state.table[0].attack_by, Side::Cpu);
    assert_eq!(state.table[1].attack_by, Side::Player);
}

// ---------------------------------------------------------------- economy

fn state() -> BreakRoomState {
    BreakRoomState::default()
}

#[test]
fn tokens_are_one_per_45_minutes_today_capped_at_six() {
    let st = state();
    let today = "2026-10-01";
    let cases = [
        (0, 0, 45, 0),
        (44, 0, 1, 44),
        (45, 1, 45, 0),
        (89, 1, 1, 44),
        (90, 2, 45, 0),
        (269, 5, 1, 44),
        (270, 6, 0, 0),
        (1000, 6, 0, 10),
    ];
    for (minutes, tokens, until, xp) in cases {
        let p = st.progression(today, minutes);
        assert_eq!(p.tokens, tokens, "{minutes} min");
        assert_eq!(p.mins_until_next, until, "{minutes} min");
        assert_eq!(p.xp_progress, xp, "{minutes} min");
        assert_eq!(p.can_unlock_more, tokens > 0);
    }
    assert!((st.progression(today, 30).xp_percent - 66.666_666).abs() < 1e-3);
}

#[test]
fn unlocking_needs_a_free_token_never_double_unlocks_and_resets_daily() {
    let mut st = state();
    let today = d("2026-10-01");
    let clock = FixedOffsetClock::UTC;
    assert!(
        !st.unlock_game(GameId::Wordle, today, 44, false, &clock),
        "no token yet"
    );
    assert!(st.unlock_game(GameId::Wordle, today, 45, false, &clock));
    assert!(
        !st.unlock_game(GameId::Wordle, today, 200, false, &clock),
        "already unlocked"
    );
    assert!(
        !st.unlock_game(GameId::Geodle, today, 89, false, &clock),
        "second token not earned"
    );
    assert!(st.unlock_game(GameId::Geodle, today, 90, false, &clock));
    assert_eq!(st.unlocked_games, ["Wordle", "Geodle"]);
    assert_eq!(st.total_unlocks, 2);
    // tomorrow everything is locked again, the counter is not
    let p = st.progression("2026-10-02", 0);
    assert!(p.unlocked.is_empty());
    assert!(st.unlock_game(GameId::Travle, d("2026-10-02"), 45, false, &clock));
    assert_eq!(st.unlocked_games, ["Travle"]);
    assert_eq!(st.total_unlocks, 3);
}

#[test]
fn the_unlock_streak_reproduces_productions_utc_parsing_quirk() {
    // In UTC the streak counts consecutive days...
    let mut utc = state();
    let clock = FixedOffsetClock::UTC;
    for (i, day) in ["2026-10-01", "2026-10-02", "2026-10-03"]
        .iter()
        .enumerate()
    {
        utc.unlock_game(GameId::Wordle, d(day), 45, false, &clock);
        assert_eq!(utc.unlock_streak, i as u64 + 1);
    }
    utc.unlock_game(GameId::Geodle, d("2026-10-03"), 90, false, &clock);
    assert_eq!(utc.unlock_streak, 3, "same day keeps it");
    utc.unlock_game(GameId::Wordle, d("2026-10-05"), 45, false, &clock);
    assert_eq!(utc.unlock_streak, 1, "a gap restarts it");
    // ...but anywhere else production compares local midnight with UTC midnight, so it is never
    // exactly 0 or 1 day and every unlock restarts the streak at 1 (On Fire is unreachable).
    for clock in [ZURICH_SUMMER, FixedOffsetClock::new(-5 * 3600)] {
        let mut st = state();
        for day in ["2026-10-01", "2026-10-02", "2026-10-03"] {
            st.unlock_game(GameId::Wordle, d(day), 45, false, &clock);
            assert_eq!(st.unlock_streak, 1);
        }
    }
}

#[test]
fn speedrunner_needs_the_first_unlock_after_a_45_minute_session() {
    let clock = FixedOffsetClock::UTC;
    let mut st = state();
    st.unlock_game(GameId::Wordle, d("2026-10-01"), 90, false, &clock);
    st.unlock_game(GameId::Geodle, d("2026-10-01"), 90, true, &clock);
    assert!(
        !st.speedrunner_today,
        "only the first unlock of the day counts"
    );
    let mut st = state();
    st.unlock_game(GameId::Wordle, d("2026-10-01"), 45, true, &clock);
    assert!(st.speedrunner_today);
    st.unlock_game(GameId::Geodle, d("2026-10-01"), 90, false, &clock);
    assert!(st.speedrunner_today, "kept for the rest of the day");
    st.unlock_game(GameId::Geodle, d("2026-10-02"), 45, false, &clock);
    assert!(
        !st.speedrunner_today,
        "a new day's first unlock decides again"
    );
}

#[test]
fn playing_water_and_the_pet_rock() {
    let mut st = state();
    let t = WallTimestamp::from_unix_millis(1_790_000_000_000);
    st.log_played(GameId::Wordle, t, "2026-10-01");
    st.log_played(GameId::Wordle, t, "2026-10-01");
    st.log_played(GameId::Geodle, t, "2026-10-01");
    assert_eq!(st.played_breaks.len(), 3);
    assert_eq!(
        st.progression("2026-10-01", 0).played_today,
        ["Wordle", "Geodle"]
    );
    assert_eq!(st.played_games_all_time, ["Wordle", "Geodle"]);
    st.log_played(GameId::Travle, t, "2026-10-02");
    assert_eq!(st.played_breaks.len(), 1, "a new day starts a new list");
    assert_eq!(st.played_games_all_time.len(), 3);

    st.add_water("2026-10-01");
    st.add_water("2026-10-01");
    assert_eq!(st.progression("2026-10-01", 0).water_today, 2);
    assert_eq!(st.progression("2026-10-02", 0).water_today, 0);
    st.add_water("2026-10-02");
    assert_eq!(st.water_glasses, 1);

    st.pet_rock_pats = 998;
    assert!(!st.pat_rock());
    assert!(st.pat_rock(), "the 1000th pat celebrates");
    assert!(!st.pat_rock());
    assert_eq!(streak_emoji(0), "");
    assert_eq!(streak_emoji(2), "\u{1F525}");
    assert_eq!(streak_emoji(3), "\u{1F525}\u{1F525}");
    assert_eq!(streak_emoji(7), "\u{1F525}\u{1F525}\u{1F525}");
}

// ---------------------------------------------------------------- achievements

fn inputs<'a>(
    state: &'a BreakRoomState,
    today: &str,
    clock: &'a dyn LocalClock,
) -> AchievementInputs<'a> {
    AchievementInputs {
        state,
        today: d(today),
        clock,
        lifetime_minutes: 0,
        garden: GardenInputs::default(),
    }
}

fn earned(list: &[Achievement], id: &str) -> bool {
    list.iter()
        .find(|a| a.id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .earned
}

#[test]
fn the_inventory_is_43_achievements_in_productions_groups() {
    let st = state();
    let list = evaluate(&inputs(&st, "2026-10-01", &FixedOffsetClock::UTC));
    assert_eq!(list.len(), 10 + 19 + 7 + 7);
    let ids: Vec<&str> = list.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        &ids[..10],
        [
            "full-house",
            "first-break",
            "on-fire",
            "early-bird",
            "night-owl",
            "speedrunner",
            "explorer",
            "perfectionist",
            "veteran",
            "rock-current"
        ]
    );
    assert!(list.iter().all(|a| !a.earned));
    let daily: Vec<&str> = list
        .iter()
        .filter(|a| a.daily)
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(
        daily,
        [
            "full-house",
            "early-bird",
            "night-owl",
            "speedrunner",
            "perfectionist"
        ]
    );
    // every earnable id has production's real art
    assert!(list
        .iter()
        .filter(|a| a.id != "rock-current")
        .all(|a| has_real_art(&a.id)));
    assert!(!has_real_art("rock-current"));
}

#[test]
fn break_room_achievement_boundaries() {
    let clock = FixedOffsetClock::UTC;
    let today = "2026-10-01";
    let mut st = state();
    st.total_unlocks = 0;
    assert!(!earned(
        &evaluate(&inputs(&st, today, &clock)),
        "first-break"
    ));
    st.total_unlocks = 1;
    assert!(earned(
        &evaluate(&inputs(&st, today, &clock)),
        "first-break"
    ));
    st.total_unlocks = 9;
    assert!(!earned(&evaluate(&inputs(&st, today, &clock)), "veteran"));
    st.total_unlocks = 10;
    assert!(earned(&evaluate(&inputs(&st, today, &clock)), "veteran"));
    st.unlock_streak = 2;
    assert!(!earned(&evaluate(&inputs(&st, today, &clock)), "on-fire"));
    st.unlock_streak = 3;
    assert!(earned(&evaluate(&inputs(&st, today, &clock)), "on-fire"));
    st.played_games_all_time = GAMES.iter().take(5).map(|g| g.name.to_string()).collect();
    assert!(!earned(&evaluate(&inputs(&st, today, &clock)), "explorer"));
    st.played_games_all_time.push("Geodle".into());
    assert!(earned(&evaluate(&inputs(&st, today, &clock)), "explorer"));

    // full house / perfectionist are today-only (or counted)
    st.unlocked_games = GAMES.iter().map(|g| g.name.to_string()).collect();
    st.unlocked_games_date = today.into();
    let list = evaluate(&inputs(&st, today, &clock));
    assert!(earned(&list, "full-house"));
    assert!(!earned(&list, "perfectionist"));
    let t = WallTimestamp::from_unix_millis(d(today).days() * 86_400_000 + 12 * 3_600_000);
    for g in GAMES {
        st.log_played(g.id, t, today);
    }
    assert!(earned(
        &evaluate(&inputs(&st, today, &clock)),
        "perfectionist"
    ));
    assert!(
        !earned(&evaluate(&inputs(&st, "2026-10-02", &clock)), "full-house"),
        "gone tomorrow when never counted"
    );
    st.badge_counts.insert("full-house".into(), 1);
    assert!(
        earned(&evaluate(&inputs(&st, "2026-10-02", &clock)), "full-house"),
        "a counted daily badge stays earned"
    );
}

#[test]
fn early_bird_and_night_owl_use_the_local_hour_of_play() {
    let today = "2026-10-01";
    let at = |clock: &FixedOffsetClock, h: i64, m: i64| {
        WallTimestamp::from_unix_millis(
            clock.local_midnight(d(today)).unix_millis + h * 3_600_000 + m * 60_000,
        )
    };
    for clock in [FixedOffsetClock::UTC, ZURICH_SUMMER] {
        for (h, m, early, night) in [
            (8, 59, true, false),
            (9, 0, false, false),
            (21, 59, false, false),
            (22, 0, false, true),
            (23, 59, false, true),
        ] {
            let mut st = state();
            st.log_played(GameId::Wordle, at(&clock, h, m), today);
            let list = evaluate(&inputs(&st, today, &clock));
            assert_eq!(earned(&list, "early-bird"), early, "{h}:{m}");
            assert_eq!(earned(&list, "night-owl"), night, "{h}:{m}");
        }
    }
    // an unparsable imported time is neither
    let mut st = state();
    st.played_breaks = vec![PlayedBreak {
        name: "Wordle".into(),
        played_at: None,
    }];
    st.played_breaks_date = today.into();
    let list = evaluate(&inputs(&st, today, &FixedOffsetClock::UTC));
    assert!(!earned(&list, "early-bird") && !earned(&list, "night-owl"));
}

#[test]
fn daily_badges_count_exactly_once_per_day() {
    let mut st = state();
    let hits = [("full-house", true), ("early-bird", false)];
    assert_eq!(st.count_daily_badges(&hits, "2026-10-01"), ["full-house"]);
    for _ in 0..100 {
        assert!(
            st.count_daily_badges(&hits, "2026-10-01").is_empty(),
            "every re-evaluation is a no-op"
        );
    }
    assert_eq!(st.badge_counts["full-house"], 1);
    assert_eq!(st.count_daily_badges(&hits, "2026-10-02"), ["full-house"]);
    assert_eq!(st.badge_counts["full-house"], 2);
    assert!(!st.badge_counts.contains_key("early-bird"));
}

#[test]
fn pet_rock_milestones_and_stage_ladder() {
    let st_at = |pats| {
        let mut st = state();
        st.pet_rock_pats = pats;
        st
    };
    for (id, _, name, threshold, _) in PET_ROCK_MILESTONES {
        let before = st_at(threshold - 1);
        let at = st_at(threshold);
        assert!(!earned(
            &evaluate(&inputs(&before, "2026-10-01", &FixedOffsetClock::UTC)),
            id
        ));
        assert!(earned(
            &evaluate(&inputs(&at, "2026-10-01", &FixedOffsetClock::UTC)),
            id
        ));
        assert_eq!(rock_stage(threshold).1, name);
    }
    assert_eq!(rock_stage(0), ("", "Pet Rock"));
    assert_eq!(rock_stage(9), ("", "Pet Rock"));
    assert_eq!(rock_stage(10), ("\u{1F331}", "Sprouting Rock"));
    assert_eq!(rock_stage(700), ("\u{1F525}", "Hellish Rock"));
    assert_eq!(rock_stage(u64::MAX).1, "Guardian Angel");
    let current = |pats| {
        let st = st_at(pats);
        evaluate(&inputs(&st, "2026-10-01", &FixedOffsetClock::UTC))
            .into_iter()
            .find(|a| a.id == "rock-current")
            .unwrap()
    };
    let none = current(0);
    assert!(!none.earned);
    assert_eq!(
        (none.name, none.how.as_str()),
        ("Pet Rock", "Pat the pet rock 10 times.")
    );
    let cosmic = current(1500);
    assert_eq!(
        (cosmic.name, cosmic.how.as_str()),
        ("Cosmic Rock", "Pat the pet rock 5k times.")
    );
    assert_eq!(current(8_888_888).how, "Reach the final pet rock form.");
}

#[test]
fn fossil_and_garden_achievements_are_derived_without_garden_state() {
    let st = state();
    let clock = FixedOffsetClock::UTC;
    let with = |lifetime: u64, garden: GardenInputs| {
        evaluate(&AchievementInputs {
            state: &st,
            today: d("2026-10-01"),
            clock: &clock,
            lifetime_minutes: lifetime,
            garden,
        })
    };
    assert!(!earned(&with(599, GardenInputs::default()), "fossil-10"));
    assert!(earned(&with(600, GardenInputs::default()), "fossil-10"));
    assert!(earned(
        &with(60_000, GardenInputs::default()),
        "fossil-1000"
    ));
    let g = |f: fn(&mut GardenInputs)| {
        let mut i = GardenInputs::default();
        f(&mut i);
        i
    };
    assert!(earned(
        &with(0, g(|i| i.study_session_count = 1)),
        "garden-first-sprout"
    ));
    assert!(!earned(
        &with(0, g(|i| i.study_session_count = 19)),
        "garden-mushroom-ring"
    ));
    assert!(earned(
        &with(0, g(|i| i.study_session_count = 20)),
        "garden-mushroom-ring"
    ));
    assert!(!earned(
        &with(0, g(|i| i.streak_days = 4)),
        "garden-streak-bloom"
    ));
    assert!(earned(
        &with(0, g(|i| i.streak_days = 5)),
        "garden-streak-bloom"
    ));
    assert!(earned(
        &with(0, g(|i| i.weekly_course_count = 3)),
        "garden-cross-pollinator"
    ));
    assert!(!earned(
        &with(0, g(|i| i.weekly_total_minutes = 419)),
        "garden-full-bloom"
    ));
    assert!(earned(
        &with(0, g(|i| i.weekly_total_minutes = 420)),
        "garden-full-bloom"
    ));
    assert!(!earned(
        &with(0, g(|i| i.weekly_total_minutes = 719)),
        "garden-wise-tree"
    ));
    assert!(earned(
        &with(0, g(|i| i.weekly_total_minutes = 720)),
        "garden-wise-tree"
    ));
    assert!(earned(
        &with(0, g(|i| i.streak_days = 10)),
        "garden-wise-tree"
    ));
    assert!(!earned(
        &with(0, g(|i| i.completed_task_count = 9)),
        "garden-harvest-season"
    ));
    assert!(earned(
        &with(0, g(|i| i.completed_task_count = 10)),
        "garden-harvest-season"
    ));
    assert_eq!(
        [0, 29, 30, 89, 90, 210, 420, 720, 9999].map(garden_stage),
        [0, 0, 1, 1, 2, 3, 4, 5, 5]
    );
}

#[test]
fn earned_dates_skip_the_baseline_and_are_recorded_once() {
    let mut tracker = EarnedDateTracker::default();
    let mut dates = BTreeMap::new();
    let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(
        tracker
            .observe(&ids(&["first-break"]), &mut dates, "2026-10-01")
            .is_empty(),
        "first run is the baseline"
    );
    assert!(dates.is_empty());
    assert_eq!(
        tracker.observe(&ids(&["first-break", "veteran"]), &mut dates, "2026-10-01"),
        ["veteran"]
    );
    assert!(tracker
        .observe(&ids(&["first-break", "veteran"]), &mut dates, "2026-10-01")
        .is_empty());
    // dropping out and earning again keeps the first date
    tracker.observe(&ids(&["first-break"]), &mut dates, "2026-10-02");
    assert!(tracker
        .observe(&ids(&["first-break", "veteran"]), &mut dates, "2026-10-03")
        .is_empty());
    assert_eq!(dates["veteran"], "2026-10-01");
    // a restart starts a new baseline: nothing earned in between gets a date
    let mut restarted = EarnedDateTracker::default();
    assert!(restarted
        .observe(
            &ids(&["first-break", "veteran", "explorer"]),
            &mut dates,
            "2026-10-04"
        )
        .is_empty());
    assert!(!dates.contains_key("explorer"));
}

#[test]
fn album_order_pages_and_icons_match_production() {
    let golden = fixture("album");
    let mut st = state();
    st.pet_rock_pats = 9_000_000;
    st.total_unlocks = 10;
    st.unlock_streak = 3;
    st.speedrunner_today = true;
    st.last_unlock_date = "2026-10-01".into();
    st.played_games_all_time = GAMES.iter().map(|g| g.name.to_string()).collect();
    for id in ["full-house", "early-bird", "night-owl", "perfectionist"] {
        st.badge_counts.insert(id.into(), 1);
    }
    let all = evaluate(&AchievementInputs {
        state: &st,
        today: d("2026-10-01"),
        clock: &FixedOffsetClock::UTC,
        lifetime_minutes: 100_000,
        garden: GardenInputs {
            study_session_count: 50,
            streak_days: 12,
            weekly_course_count: 4,
            weekly_total_minutes: 900,
            completed_task_count: 12,
        },
    });
    let entries = album_entries(&all, &BTreeMap::new());
    assert_eq!(entries.len(), 42, "everything but rock-current");
    let names: Vec<&str> = album_pages(&entries)
        .iter()
        .flat_map(|p| p.rows.iter().map(|r| r.name))
        .collect();
    let want: Vec<&str> = golden["sorted"].as_array().unwrap().iter().map(s).collect();
    assert_eq!(names, want, "undated entries sort like localeCompare");
    assert_eq!(
        entries.iter().find(|e| e.id == "fossil-10").unwrap().icon,
        "\u{1F330}"
    );
    // dated entries come first, oldest first
    let mut dates = BTreeMap::new();
    dates.insert("veteran".to_string(), "2026-09-02".to_string());
    dates.insert("explorer".to_string(), "2026-09-01".to_string());
    let pages = album_pages(&album_entries(&all, &dates));
    assert_eq!(pages.len(), 14);
    assert_eq!(pages[0].rows[0].id, "explorer");
    assert_eq!(pages[0].rows[1].id, "veteran");
    assert!(pages[0].left && !pages[1].left && pages[13].number == 14);
    let empty = album_pages(&[]);
    assert_eq!(empty.len(), 2);
    assert!(empty[0].empty && !empty[1].empty && empty[1].rows.is_empty());
    let one = album_pages(&album_entries(&all, &BTreeMap::new())[..4]);
    assert_eq!(one.len(), 2, "an odd page count gets a blank right page");
}

// ---------------------------------------------------------------- rest

#[test]
fn breathing_follows_the_4_7_8_cycle_and_stops_after_four_rounds() {
    let f = breath_frame(0);
    assert_eq!((f.phase, f.remaining, f.size, f.round), (0, 4, 60.0, 1));
    assert_eq!(breath_frame(2).size, 115.0);
    let hold = breath_frame(4);
    assert_eq!((hold.phase, hold.remaining, hold.size), (1, 7, 170.0));
    let out = breath_frame(15);
    assert_eq!((out.phase, out.remaining, out.size), (2, 4, 115.0));
    assert_eq!(breath_frame(19).round, 2);
    assert_eq!(breath_frame(75).round, 4);
    let mut b = Breathing::default();
    assert!(!b.advance_to(3), "off: nothing moves");
    b.toggle();
    assert!(b.advance_to(1));
    assert!(!b.advance_to(1));
    assert!(b.advance_to(75));
    assert!(b.on);
    assert!(b.advance_to(76));
    assert_eq!(
        b,
        Breathing::default(),
        "the tick after 75 s switches it off"
    );
}

#[test]
fn rest_timer_counts_down_by_elapsed_time() {
    let mut t = RestTimer::default();
    assert_eq!(t.face(), "05:00");
    assert!(!t.advance_to(100), "paused timers do not move");
    t.toggle();
    assert!(t.advance_to(299));
    assert_eq!(t.face(), "04:59");
    assert!((t.progress() - 1.0 / 300.0).abs() < 1e-9);
    assert!(t.advance_to(0));
    assert!(!t.running && t.remaining == 0);
    t.toggle();
    assert_eq!(
        (t.running, t.remaining),
        (true, 300),
        "starting a finished timer refills it"
    );
    t.set_seconds(89);
    assert_eq!((t.minutes, t.remaining), (1, 89));
    t.set_seconds(90);
    assert_eq!(t.minutes, 2, "Math.round(1.5) = 2");
    t.set_seconds(10);
    assert_eq!(t.minutes, 1, "at least one minute");
    t.reset();
    assert_eq!((t.running, t.remaining), (false, 60));
}

#[test]
fn the_rest_timer_face_accepts_minutes_or_m_ss_like_production() {
    assert_eq!(parse_timer_face(" 7 "), Some(420));
    assert_eq!(parse_timer_face("0"), Some(1), "Math.max(1, 0)");
    assert_eq!(parse_timer_face("4:30"), Some(270));
    assert_eq!(parse_timer_face("0:00"), Some(1));
    assert_eq!(parse_timer_face("1234:05"), Some(74_045));
    for bad in [
        "", "4:60", "12345:00", "4:5:6", "four", "4.5", "-3", ":30", "4:",
    ] {
        assert_eq!(parse_timer_face(bad), None, "{bad:?}");
    }
}
