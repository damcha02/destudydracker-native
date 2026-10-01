use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde_json::Value;
use study_tracker_core::academic::{AcademicState, SessionId, SessionKind, StudySession};
use study_tracker_core::break_room::catalog::GameId;
use study_tracker_core::break_room::durak::DurakSession;
use study_tracker_core::dashboard::civil::{CivilDate, FixedOffsetClock, LocalClock};
use study_tracker_core::timer::WallTimestamp;

use super::*;
use crate::persistence::break_room_port::{parse_section, MemoryBreakRoomPort};

const CLOCK: FixedOffsetClock = FixedOffsetClock::new(2 * 3600);

fn today() -> CivilDate {
    CivilDate::parse_iso("2026-09-30").unwrap()
}

fn at(hour: i64, minute: i64) -> WallTimestamp {
    WallTimestamp::from_unix_millis(
        CLOCK.local_midnight(today()).unix_millis + hour * 3_600_000 + minute * 60_000,
    )
}

fn session(id: &str, kind: SessionKind, end_hour: i64, minutes: u32) -> StudySession {
    let end = at(end_hour, 0);
    StudySession {
        id: SessionId::new(id),
        semester_id: None,
        course_id: None,
        task_id: None,
        kind,
        goal: String::new(),
        learned: String::new(),
        blocker: String::new(),
        next_step: String::new(),
        confidence: 3,
        started_at: WallTimestamp::from_unix_millis(end.unix_millis - i64::from(minutes) * 60_000),
        ended_at: end,
        minutes,
        preset_label: String::new(),
    }
}

struct Harness {
    saved: Rc<RefCell<Option<Value>>>,
    controller: BreakRoomController,
    academic: AcademicState,
    revision: u64,
}

impl Harness {
    fn new() -> Self {
        let saved = Rc::new(RefCell::new(None));
        let controller = BreakRoomController::load(
            Box::new(MemoryBreakRoomPort {
                saved: Rc::clone(&saved),
            }),
            Rng::seeded(7),
            today(),
        );
        let mut h = Self {
            saved,
            controller,
            academic: AcademicState::new(),
            revision: 0,
        };
        h.sync();
        h
    }

    /// A restart: a new controller over what was saved.
    fn restart(&mut self) {
        self.controller = BreakRoomController::load(
            Box::new(MemoryBreakRoomPort {
                saved: Rc::clone(&self.saved),
            }),
            Rng::seeded(99),
            today(),
        );
        self.controller
            .sync(&self.academic, self.revision, today(), &CLOCK);
    }

    fn add(&mut self, sessions: Vec<StudySession>) -> usize {
        let inserted = self.academic.add_study_sessions(sessions, at(23, 0)).len();
        if inserted > 0 {
            self.revision += 1;
        }
        inserted
    }

    fn sync(&mut self) -> bool {
        self.controller
            .sync(&self.academic, self.revision, today(), &CLOCK)
    }
}

#[test]
fn a_fresh_profile_draws_its_puzzle_salts_once() {
    let h = Harness::new();
    assert_eq!(h.controller.writes(), 1, "the new salts are saved once");
    let salts = |c: &BreakRoomController| {
        let s = &c.record().state;
        (
            s.wordle.seed_salt.clone(),
            s.geodle.seed_salt.clone(),
            s.flaggle.seed_salt.clone(),
            s.travle.seed_salt.clone(),
        )
    };
    let first = salts(&h.controller);
    assert!(!first.0.is_empty() && first.0 != first.1);
    let mut h = h;
    h.restart();
    assert_eq!(salts(&h.controller), first, "a restart keeps them");
    assert_eq!(h.controller.writes(), 0, "and writes nothing");
}

#[test]
fn timer_sessions_feed_tokens_exactly_once() {
    let mut h = Harness::new();
    let writes = h.controller.writes();
    assert_eq!(h.controller.progression().tokens, 0);
    // a completed 45-minute Timer session
    assert_eq!(h.add(vec![session("s1", SessionKind::Study, 10, 45)]), 1);
    assert!(h.sync());
    assert_eq!(h.controller.progression().tokens, 1);
    // the session also makes "First Sprout" earned: dated once, one write
    assert_eq!(
        h.controller
            .record()
            .state
            .achievement_earned_on_dates
            .get("garden-first-sprout")
            .map(String::as_str),
        Some("2026-09-30")
    );
    assert_eq!(h.controller.writes(), writes + 1);
    let writes = h.controller.writes();
    let evaluations = h.controller.evaluations();
    // every later Timer tick / refresh with the same revision is a no-op
    for _ in 0..100 {
        assert!(!h.sync());
    }
    assert_eq!(
        h.controller.evaluations(),
        evaluations,
        "no per-tick evaluation"
    );
    // the same session recovered again (second launch, crash recovery) is deduplicated upstream
    assert_eq!(h.add(vec![session("s1", SessionKind::Study, 10, 45)]), 0);
    assert!(!h.sync());
    assert_eq!(h.controller.progression().tokens, 1);
    assert_eq!(
        h.controller.writes(),
        writes,
        "re-deriving never writes again"
    );
}

#[test]
fn break_and_zero_minute_sessions_count_like_production() {
    let mut h = Harness::new();
    h.add(vec![
        session("b", SessionKind::Break, 9, 30),
        session("z", SessionKind::Study, 9, 0),
    ]);
    h.sync();
    let p = h.controller.progression();
    assert_eq!(
        (p.today_minutes, p.tokens, p.xp_progress),
        (30, 0, 30),
        "break minutes count, zero adds nothing"
    );
    h.add(vec![session("e", SessionKind::Exam, 11, 15)]);
    h.sync();
    assert_eq!(h.controller.progression().tokens, 1);
    assert!(
        !h.controller.inputs().earned_token_in_one_session,
        "no single session reached 45"
    );
}

#[test]
fn unlock_play_and_daily_badges_are_exactly_once_across_restarts() {
    let mut h = Harness::new();
    h.add(vec![session("s1", SessionKind::Study, 8, 50)]);
    h.sync();
    assert!(
        !h.controller.play(GameId::Wordle, at(8, 30), &CLOCK),
        "locked games cannot be played"
    );
    assert!(h.controller.unlock(GameId::Wordle, &CLOCK));
    assert!(
        !h.controller.unlock(GameId::Wordle, &CLOCK),
        "no second unlock"
    );
    assert!(
        !h.controller.unlock(GameId::Geodle, &CLOCK),
        "only one token"
    );
    assert!(h.controller.play(GameId::Wordle, at(8, 30), &CLOCK));
    let state = &h.controller.record().state;
    assert_eq!(state.total_unlocks, 1);
    assert!(
        state.speedrunner_today,
        "first unlock after a 50-minute session"
    );
    assert_eq!(state.badge_counts.get("early-bird"), Some(&1));
    assert_eq!(state.badge_counts.get("speedrunner"), Some(&1));
    let earned =
        |c: &BreakRoomController, id: &str| c.achievements().iter().any(|a| a.id == id && a.earned);
    assert!(earned(&h.controller, "first-break") && earned(&h.controller, "early-bird"));
    // dated once, today (the first sync was the baseline)
    assert_eq!(
        h.controller
            .record()
            .state
            .achievement_earned_on_dates
            .get("first-break")
            .map(String::as_str),
        Some("2026-09-30")
    );
    let snapshot = h.controller.record().state.clone();
    // a restart, more syncs, and a recovered duplicate session change nothing
    h.restart();
    for _ in 0..10 {
        h.sync();
    }
    h.add(vec![session("s1", SessionKind::Study, 8, 50)]);
    h.sync();
    assert_eq!(h.controller.record().state, snapshot);
    assert_eq!(h.controller.writes(), 0, "nothing to save after a restart");
}

#[test]
fn every_persisted_action_writes_once_and_viewing_never_writes() {
    let mut h = Harness::new();
    let base = h.controller.writes();
    h.controller.select_room(Room::Meditation);
    h.controller.next_stretch();
    h.controller.close_game();
    let _ = h.controller.card_status(GameId::Wordle);
    assert_eq!(
        h.controller.writes(),
        base,
        "navigation and viewing do not write"
    );
    h.controller.pat_rock(&CLOCK);
    h.controller.add_water(&CLOCK);
    assert_eq!(h.controller.writes(), base + 2);
    assert_eq!(h.controller.record().state.pet_rock_pats, 1);
    h.controller.next_tree();
    assert_eq!(h.controller.record().rest_tree, 1);
    assert_eq!(h.controller.writes(), base + 3);
    let saved = parse_section(h.saved.borrow().as_ref());
    assert_eq!(saved.state.pet_rock_pats, 1);
    assert_eq!(saved.rest_tree, 1);
}

#[test]
fn the_thousandth_pat_celebrates() {
    let mut h = Harness::new();
    for _ in 0..999 {
        h.controller.pat_rock(&CLOCK);
    }
    assert!(!h.controller.ui.rock_celebrating);
    h.controller.pat_rock(&CLOCK);
    assert!(h.controller.ui.rock_celebrating);
    let rock = h
        .controller
        .achievements()
        .iter()
        .find(|a| a.id == "rock-cosmic")
        .unwrap();
    assert!(rock.earned);
}

fn unlocked(h: &mut Harness, game: GameId) {
    h.add(vec![session(
        &format!("s-{game:?}"),
        SessionKind::Study,
        7,
        270,
    )]);
    h.sync();
    assert!(h.controller.unlock(game, &CLOCK));
    assert!(h.controller.play(game, at(12, 0), &CLOCK));
}

#[test]
fn wordle_through_the_controller() {
    let mut h = Harness::new();
    unlocked(&mut h, GameId::Wordle);
    let answer = h.controller.record().state.wordle.answer.clone();
    assert_eq!(
        answer,
        study_tracker_core::break_room::wordle::answer_for_date(
            "2026-09-30",
            &h.controller.record().state.wordle.seed_salt
        )
    );
    for c in "zzzzz".chars() {
        h.controller.wordle_letter(c);
    }
    h.controller.wordle_letter('q');
    assert_eq!(
        h.controller.ui.wordle_draft, "zzzzz",
        "at most five letters"
    );
    h.controller.wordle_enter(&CLOCK);
    assert_eq!(h.controller.ui.wordle_message, "Not in the word list.");
    for _ in 0..5 {
        h.controller.wordle_backspace();
    }
    let writes = h.controller.writes();
    for c in answer.chars() {
        h.controller.wordle_letter(c);
    }
    h.controller.wordle_enter(&CLOCK);
    assert_eq!(h.controller.ui.wordle_message, "Solved in 1.");
    assert_eq!(h.controller.writes(), writes + 1);
    assert_eq!(h.controller.card_status(GameId::Wordle), "Solved");
    // tomorrow the card label is gone again
    h.controller.sync(
        &h.academic,
        h.revision,
        CivilDate::parse_iso("2026-10-01").unwrap(),
        &CLOCK,
    );
    assert_eq!(h.controller.card_status(GameId::Wordle), "");
}

struct StubFlags;
impl FlagSimilarity for StubFlags {
    fn similarity(&mut self, guess: &str, answer: &str) -> Option<f64> {
        if guess == "Nepal" {
            None
        } else if guess == answer {
            Some(100.0)
        } else {
            Some(42.5)
        }
    }
}

#[test]
fn geodle_and_flaggle_through_the_controller() {
    let mut h = Harness::new();
    unlocked(&mut h, GameId::Geodle);
    h.controller.geodle_set_draft("Atlantis");
    h.controller.geodle_submit(&CLOCK);
    assert_eq!(
        h.controller.ui.geodle_message,
        "Select a country from the list."
    );
    assert!(h.controller.ui.geodle_dropdown);
    let answer = h.controller.record().state.geodle.answer.clone();
    h.controller.geodle_select(&answer);
    h.controller.geodle_submit(&CLOCK);
    assert_eq!(h.controller.ui.geodle_message, "Solved in 1.");

    let mut h = Harness::new();
    unlocked(&mut h, GameId::Flaggle);
    h.controller.flaggle_select("Nepal");
    h.controller.flaggle_submit(&mut StubFlags, &CLOCK);
    assert_eq!(
        h.controller.ui.flaggle_message,
        "Could not render that flag. Try another guess."
    );
    assert!(h.controller.record().state.flaggle.guesses.is_empty());
    let answer = h.controller.record().state.flaggle.answer.clone();
    let other = if answer == "Chad" { "Mali" } else { "Chad" };
    h.controller.flaggle_select(other);
    h.controller.flaggle_submit(&mut StubFlags, &CLOCK);
    assert_eq!(
        h.controller.record().state.flaggle.guesses[0].similarity,
        42.5
    );
    assert_eq!(
        h.controller.flaggle_preview(),
        PreviewRequest::RevealedBy(vec![other.to_string()])
    );
    // the saved section keeps the guess in production's shape
    let saved = h.saved.borrow().clone().unwrap();
    assert_eq!(
        saved["flagglePuzzle"]["guesses"][0]["maskedFlagDataUrl"],
        ""
    );
}

#[test]
fn durak_through_the_controller_survives_a_restart() {
    let mut h = Harness::new();
    unlocked(&mut h, GameId::DailyDurak);
    let game = h
        .controller
        .durak
        .game
        .clone()
        .expect("today's puzzle is dealt on open");
    assert_eq!(
        h.controller.record().state.durak.seed.as_deref(),
        Some("2026-09-30_0")
    );
    let writes = h.controller.writes();
    h.controller.durak.click_card(0);
    h.controller
        .durak_action(&CLOCK, |s, p, t, _| s.attack(p, t));
    assert_eq!(h.controller.writes(), writes + 1);
    let after = h.controller.durak.game.clone().unwrap();
    assert_ne!(after, game);
    h.restart();
    assert!(
        h.controller.durak.game.is_none(),
        "the live game is React-like state"
    );
    assert!(h.controller.play(GameId::DailyDurak, at(13, 0), &CLOCK));
    assert_eq!(
        h.controller.durak.game.as_ref(),
        Some(&after),
        "reopening restores the stored game"
    );
    let _ = DurakSession::default();
}

#[test]
fn rest_timer_and_breathing_follow_elapsed_time() {
    let mut h = Harness::new();
    let t0 = Instant::now();
    assert!(!h.controller.needs_seconds_clock());
    h.controller.rest_toggle(t0);
    assert!(h.controller.needs_seconds_clock());
    assert!(h.controller.rest_tick(t0 + Duration::from_millis(1500)));
    assert_eq!(h.controller.ui.rest.remaining, 299);
    assert!(
        !h.controller.rest_tick(t0 + Duration::from_millis(1900)),
        "same whole second"
    );
    // a late tick (e.g. after the window was hidden) jumps, never drifts
    assert!(h.controller.rest_tick(t0 + Duration::from_secs(120)));
    assert_eq!(h.controller.ui.rest.remaining, 180);
    assert!(h.controller.rest_tick(t0 + Duration::from_secs(400)));
    assert!(!h.controller.ui.rest.running);
    assert!(
        !h.controller.needs_seconds_clock(),
        "a finished timer stops the clock"
    );

    h.controller.breath_toggle(t0);
    assert!(h.controller.breath_tick(t0 + Duration::from_secs(5)));
    assert_eq!(h.controller.ui.breathing.elapsed, 5);
    assert!(h.controller.breath_tick(t0 + Duration::from_secs(80)));
    assert!(!h.controller.ui.breathing.on, "stops after four rounds");
    assert!(!h.controller.needs_seconds_clock());
    assert_eq!(
        h.controller.writes(),
        1,
        "only the initial salt save: rest clocks never persist"
    );
}

#[test]
fn travle_and_skribbl_are_unlockable_playable_entries_without_a_game() {
    let mut h = Harness::new();
    unlocked(&mut h, GameId::Travle);
    assert_eq!(h.controller.ui.open_game, Some(GameId::Travle));
    assert!(h
        .controller
        .record()
        .state
        .played_games_all_time
        .contains(&"Travle".to_string()));
    assert_eq!(h.controller.card_status(GameId::DailySkribbl), "Daily");
}

#[test]
fn the_album_opens_turns_and_closes_like_production() {
    let mut a = AlbumUi::default();
    // turning a shut album does nothing; forward opens it
    assert_eq!(a.turn(true, 3, false), AlbumAnim::None);
    assert_eq!(a.page_back(3, false), AlbumAnim::None);
    assert_eq!(a.page_forward(3, false), AlbumAnim::Open);
    assert_eq!(a.cover_seq, 1);
    assert_eq!(
        a.page_forward(3, false),
        AlbumAnim::None,
        "no input while a motion plays"
    );
    a.finish();
    assert!(a.open && a.spread == 0);
    assert_eq!(a.page_forward(3, false), AlbumAnim::Next);
    a.finish();
    assert_eq!(a.spread, 1);
    assert_eq!(a.page_forward(3, false), AlbumAnim::Next);
    a.finish();
    assert_eq!(a.spread, 2);
    // past the last spread closes, and a close returns to the first spread
    assert_eq!(a.page_forward(3, false), AlbumAnim::Close);
    assert_eq!(a.spread, 0, "spread reset as the close starts");
    a.finish();
    assert!(!a.open);
    // back on the first spread closes too
    a.page_forward(3, false);
    a.finish();
    assert_eq!(a.page_back(3, false), AlbumAnim::Close);
    a.finish();
    // reduced motion applies everything at once
    let mut r = AlbumUi::default();
    assert_eq!(r.open_book(true), AlbumAnim::None);
    assert!(r.open);
    r.turn(true, 2, true);
    assert_eq!(r.spread, 1);
    assert_eq!((r.cover_seq, r.leaf_seq), (0, 0));
    assert_eq!(AlbumAnim::Open.duration_ms(), 780);
    assert_eq!(AlbumAnim::Next.duration_ms(), 520);
}
