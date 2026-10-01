//! The Break Room controller (Stage 20): the single owner and single writer of the Break Room
//! record, plus the transient state production keeps in React (open game, drafts, messages, the
//! rest timer, the breathing exercise, the album's page). No Slint here: the view module turns
//! this into window properties, and every rule lives in `study_tracker_core::break_room`.
//!
//! ```text
//! academic revision / day change ──► sync(): AcademicInputs ─► progression + achievements
//!                                      ├─ daily badges counted once per day  ─┐
//!                                      └─ newly earned achievements dated     ├─► persist (only
//! user action (unlock, play, guess, pat ...) ──► core rule ──► state changed ─┘     on change)
//! ```
//!
//! Exactly-once: tokens and achievements are derived, never stored, so no Timer tick, restart,
//! recovery, import or second launch can award anything twice; the only counters (`badgeCounts`)
//! are guarded per day by `badgeCountDates`, unlocks by production's own UI rule.

use std::time::Instant;

use study_tracker_core::academic::AcademicState;
use study_tracker_core::break_room::achievements::{
    academic_inputs, daily_hits, evaluate, AcademicInputs, Achievement, AchievementInputs,
    EarnedDateTracker,
};
use study_tracker_core::break_room::catalog::GameId;
use study_tracker_core::break_room::durak::DurakSession;
use study_tracker_core::break_room::flaggle::PreviewRequest;
use study_tracker_core::break_room::geodle::CountrySubmit;
use study_tracker_core::break_room::rest::{Breathing, RestTimer};
use study_tracker_core::break_room::state::Progression;
use study_tracker_core::break_room::wordle::{WordleSubmit, WORD_LENGTH};
use study_tracker_core::dashboard::civil::{CivilDate, LocalClock};
use study_tracker_core::timer::WallTimestamp;

use crate::persistence::break_room_port::{BreakRoomPort, BreakRoomRecord, REST_TREE_COUNT};

/// Production's `Math.random()` stand-in: a small splitmix64 generator, seeded from the clock in
/// the app and fixed in tests / parity captures.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u64,
    /// When set, every draw returns this value (production captured with a pinned Math.random).
    pinned: Option<f64>,
}

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Self {
            state: seed,
            pinned: None,
        }
    }
    pub fn pinned(value: f64) -> Self {
        Self {
            state: 0,
            pinned: Some(value.clamp(0.0, 0.999_999_999)),
        }
    }
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// A value in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        if let Some(v) = self.pinned {
            return v;
        }
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// `Math.floor(Math.random() * n)`.
    pub fn index(&mut self, n: usize) -> usize {
        ((self.next_f64() * n as f64).floor() as usize).min(n.saturating_sub(1))
    }
    /// `crypto.randomUUID()` (v4 format) for a puzzle seed salt.
    pub fn uuid(&mut self) -> String {
        let (a, b) = (self.next_u64(), self.next_u64());
        format!(
            "{:08x}-{:04x}-4{:03x}-{:04x}-{:012x}",
            a >> 32,
            (a >> 16) & 0xffff,
            a & 0x0fff,
            ((b >> 48) & 0x3fff) | 0x8000,
            b & 0xffff_ffff_ffff
        )
    }
}

/// The Wabi-Sabi Rest room on show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Room {
    #[default]
    Games,
    Meditation,
    Achievements,
}

/// The album's motion in progress (`BookAnim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlbumAnim {
    #[default]
    None,
    Open,
    Close,
    Next,
    Prev,
}

impl AlbumAnim {
    /// Production's durations: 780 ms to open/close, 520 ms per page.
    pub fn duration_ms(self) -> u64 {
        match self {
            AlbumAnim::Open | AlbumAnim::Close => 780,
            AlbumAnim::Next | AlbumAnim::Prev => 520,
            AlbumAnim::None => 0,
        }
    }
}

/// The album's state machine (`BookGallery`): open/closed, the spread on show, and the one motion
/// in progress. The animation itself is Slint's; `finish` applies its result like production's
/// `finish()` does once the turn has played.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlbumUi {
    pub open: bool,
    pub spread: usize,
    pub anim: AlbumAnim,
    pub turn_from: usize,
    /// bumped once per cover / leaf motion (the view's animation phase)
    pub cover_seq: i32,
    pub leaf_seq: i32,
}

impl AlbumUi {
    /// `openBook`. Returns the motion started, if any (reduced motion applies it at once).
    pub fn open_book(&mut self, reduced_motion: bool) -> AlbumAnim {
        if self.open || self.anim != AlbumAnim::None {
            return AlbumAnim::None;
        }
        if reduced_motion {
            self.open = true;
            self.spread = 0;
            return AlbumAnim::None;
        }
        self.anim = AlbumAnim::Open;
        self.cover_seq += 1;
        AlbumAnim::Open
    }

    /// `turn("next" | "prev")` within `spread_count` spreads.
    pub fn turn(&mut self, forward: bool, spread_count: usize, reduced_motion: bool) -> AlbumAnim {
        let index = self.spread.min(spread_count.saturating_sub(1));
        let Some(target) = (if forward {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        }) else {
            return AlbumAnim::None;
        };
        if !self.open || self.anim != AlbumAnim::None || target >= spread_count {
            return AlbumAnim::None;
        }
        if reduced_motion {
            self.spread = target;
            return AlbumAnim::None;
        }
        self.turn_from = index;
        self.anim = if forward {
            AlbumAnim::Next
        } else {
            AlbumAnim::Prev
        };
        self.leaf_seq += 1;
        self.anim
    }

    /// `closeBook` (Escape, or turning past either end).
    pub fn close_book(&mut self, reduced_motion: bool) -> AlbumAnim {
        if !self.open || self.anim != AlbumAnim::None {
            return AlbumAnim::None;
        }
        if reduced_motion {
            self.open = false;
            self.spread = 0;
            return AlbumAnim::None;
        }
        self.spread = 0;
        self.anim = AlbumAnim::Close;
        self.cover_seq += 1;
        AlbumAnim::Close
    }

    /// `pageForward`: open a shut album, turn a page, or close past the last spread.
    pub fn page_forward(&mut self, spread_count: usize, reduced_motion: bool) -> AlbumAnim {
        if !self.open {
            return self.open_book(reduced_motion);
        }
        if self.spread.min(spread_count.saturating_sub(1)) + 1 >= spread_count {
            self.close_book(reduced_motion)
        } else {
            self.turn(true, spread_count, reduced_motion)
        }
    }

    /// `pageBack`: turn back, or close on the first spread.
    pub fn page_back(&mut self, spread_count: usize, reduced_motion: bool) -> AlbumAnim {
        if !self.open {
            return AlbumAnim::None;
        }
        if self.spread.min(spread_count.saturating_sub(1)) == 0 {
            self.close_book(reduced_motion)
        } else {
            self.turn(false, spread_count, reduced_motion)
        }
    }

    /// The motion has played: apply its result (`finish`).
    pub fn finish(&mut self) {
        match self.anim {
            AlbumAnim::Open => self.open = true,
            AlbumAnim::Close => {
                self.open = false;
                self.spread = 0;
            }
            AlbumAnim::Next => self.spread = self.turn_from + 1,
            AlbumAnim::Prev => self.spread = self.turn_from.saturating_sub(1),
            AlbumAnim::None => {}
        }
        self.anim = AlbumAnim::None;
    }
}

/// Transient UI state (production's React state; never persisted).
#[derive(Debug, Default)]
pub struct BreakUi {
    pub open_game: Option<GameId>,
    pub wordle_draft: String,
    pub wordle_message: String,
    pub geodle_draft: String,
    pub geodle_message: String,
    pub geodle_dropdown: bool,
    pub flaggle_draft: String,
    pub flaggle_message: String,
    pub flaggle_dropdown: bool,
    /// A game card mid unlock-celebration.
    pub celebrating: Option<GameId>,
    pub quote_index: usize,
    pub stretch_index: usize,
    pub room: Room,
    pub rest: RestTimer,
    rest_anchor: Option<(Instant, u32)>,
    pub breathing: Breathing,
    breath_anchor: Option<Instant>,
    pub album: AlbumUi,
    /// Pats since launch (drives the bounce; one per click).
    pub rock_bounces: u64,
    pub rock_celebrating: bool,
}

/// Rasterizes flags for Flaggle (the app's resvg adapter; a stub in tests).
pub trait FlagSimilarity {
    /// `maskFlagByTargetColors(guess, answer).similarity`, or `None` when a flag cannot be drawn
    /// (production: "Could not render that flag. Try another guess.").
    fn similarity(&mut self, guess: &str, answer: &str) -> Option<f64>;
}

pub struct BreakRoomController {
    record: BreakRoomRecord,
    port: Box<dyn BreakRoomPort>,
    writes: u64,
    tracker: EarnedDateTracker,
    pub durak: DurakSession,
    pub ui: BreakUi,
    rng: Rng,
    cache_key: Option<(u64, CivilDate)>,
    inputs: AcademicInputs,
    today: CivilDate,
    achievements: Vec<Achievement>,
    progression: Progression,
    /// Achievement evaluations (diagnostics: never per Timer tick).
    evaluations: u64,
}

pub const QUOTE_COUNT: usize = 100;
pub const STRETCH_COUNT: usize = 10;

impl BreakRoomController {
    /// Loads the record and runs production's load-time puzzle normalization for `today`. Writes
    /// only if a puzzle had no seed salt yet (otherwise every launch would draw a new one).
    pub fn load(port: Box<dyn BreakRoomPort>, mut rng: Rng, today: CivilDate) -> Self {
        let mut record = port.load();
        let iso = today.to_iso();
        let missing_salt = [
            &record.state.wordle.seed_salt,
            &record.state.geodle.seed_salt,
            &record.state.flaggle.seed_salt,
            &record.state.travle.seed_salt,
        ]
        .iter()
        .any(|salt| salt.is_empty());
        record.state.wordle =
            std::mem::take(&mut record.state.wordle).normalized(&iso, || rng.uuid());
        record.state.geodle =
            std::mem::take(&mut record.state.geodle).normalized(&iso, || rng.uuid());
        record.state.flaggle =
            std::mem::take(&mut record.state.flaggle).normalized(&iso, || rng.uuid());
        if record.state.travle.seed_salt.is_empty() {
            record.state.travle.seed_salt = rng.uuid();
        }
        let ui = BreakUi {
            quote_index: rng.index(QUOTE_COUNT),
            stretch_index: rng.index(STRETCH_COUNT),
            ..BreakUi::default()
        };
        let mut controller = Self {
            progression: record.state.progression(&iso, 0),
            record,
            port,
            writes: 0,
            tracker: EarnedDateTracker::default(),
            durak: DurakSession::default(),
            ui,
            rng,
            cache_key: None,
            inputs: AcademicInputs::default(),
            today,
            achievements: Vec::new(),
            evaluations: 0,
        };
        if missing_salt {
            controller.persist();
        }
        controller
    }

    pub fn record(&self) -> &BreakRoomRecord {
        &self.record
    }
    pub fn writes(&self) -> u64 {
        self.writes
    }
    pub fn evaluations(&self) -> u64 {
        self.evaluations
    }
    pub fn progression(&self) -> &Progression {
        &self.progression
    }
    pub fn achievements(&self) -> &[Achievement] {
        &self.achievements
    }
    #[cfg(test)]
    pub fn inputs(&self) -> AcademicInputs {
        self.inputs
    }

    fn persist(&mut self) {
        self.port.persist(&self.record);
        self.writes += 1;
    }

    /// Recomputes the academic inputs when the academic revision or the day changed (the Timer ->
    /// StudySession -> AcademicState path lands here exactly once per new session), then
    /// re-derives. Returns whether anything visible may have changed.
    pub fn sync(
        &mut self,
        academic: &AcademicState,
        revision: u64,
        today: CivilDate,
        clock: &dyn LocalClock,
    ) -> bool {
        if self.cache_key == Some((revision, today)) {
            return false;
        }
        self.cache_key = Some((revision, today));
        self.today = today;
        self.inputs = academic_inputs(academic, today, clock);
        if self.derive(clock) {
            self.persist();
        }
        true
    }

    /// Re-derives progression and achievements from the current record and inputs, counts the
    /// daily badges and dates newly earned achievements. Returns whether either changed the
    /// record (the caller persists once).
    pub fn derive(&mut self, clock: &dyn LocalClock) -> bool {
        let iso = self.today.to_iso();
        self.progression = self
            .record
            .state
            .progression(&iso, self.inputs.today_minutes);
        let input = AchievementInputs {
            state: &self.record.state,
            today: self.today,
            clock,
            lifetime_minutes: self.inputs.lifetime_minutes,
            garden: self.inputs.garden,
        };
        let hits = daily_hits(&input);
        self.evaluations += 1;
        let counted = self.record.state.count_daily_badges(&hits, &iso);
        let input = AchievementInputs {
            state: &self.record.state,
            today: self.today,
            clock,
            lifetime_minutes: self.inputs.lifetime_minutes,
            garden: self.inputs.garden,
        };
        self.achievements = evaluate(&input);
        let earned: Vec<String> = self
            .achievements
            .iter()
            .filter(|a| a.earned)
            .map(|a| a.id.clone())
            .collect();
        let dated = self.tracker.observe(
            &earned,
            &mut self.record.state.achievement_earned_on_dates,
            &iso,
        );
        let changed = !counted.is_empty() || !dated.is_empty();
        if changed {
            log::info!(
                "break room: counted daily badges {counted:?}, dated achievements {dated:?}"
            );
        }
        changed
    }

    /// A state change from a user action: derive, then persist once.
    fn changed(&mut self, clock: &dyn LocalClock) {
        self.derive(clock);
        self.persist();
    }

    /// Re-derives without an academic change (e.g. a forced refresh); persists only if the
    /// derivation itself changed the record.
    // ------------------------------------------------------------------ cards

    /// "Unlock" on a locked card. Returns whether it unlocked.
    pub fn unlock(&mut self, game: GameId, clock: &dyn LocalClock) -> bool {
        let unlocked = self.record.state.unlock_game(
            game,
            self.today,
            self.inputs.today_minutes,
            self.inputs.earned_token_in_one_session,
            clock,
        );
        if unlocked {
            self.ui.celebrating = Some(game);
            self.changed(clock);
        }
        unlocked
    }

    /// "Play" on an unlocked card: logs the play (`logPlayedBreak`) and opens the game, running
    /// its `init*Puzzle`. A locked game cannot be played (production shows no Play button).
    pub fn play(&mut self, game: GameId, now: WallTimestamp, clock: &dyn LocalClock) -> bool {
        if !self.progression.is_unlocked(game) {
            return false;
        }
        let iso = self.today.to_iso();
        self.record.state.log_played(game, now, &iso);
        self.ui.open_game = Some(game);
        let rng = &mut self.rng;
        match game {
            GameId::Wordle => {
                if self.record.state.wordle.ensure_today(&iso, || rng.uuid()) {
                    self.ui.wordle_draft.clear();
                    self.ui.wordle_message.clear();
                }
            }
            GameId::Geodle => {
                if self.record.state.geodle.ensure_today(&iso, || rng.uuid()) {
                    self.ui.geodle_draft.clear();
                    self.ui.geodle_message.clear();
                    self.ui.geodle_dropdown = false;
                }
            }
            GameId::Flaggle => {
                if self.record.state.flaggle.ensure_today(&iso, || rng.uuid()) {
                    self.ui.flaggle_draft.clear();
                    self.ui.flaggle_message.clear();
                    self.ui.flaggle_dropdown = false;
                }
            }
            GameId::DailyDurak => {
                let pick = rng.next_f64();
                self.durak.open(&mut self.record.state.durak, &iso, pick);
            }
            // Stage 21 / Stage 22: the card and the play log behave like production; the surface
            // is the deferred shell (see the view).
            GameId::Travle | GameId::DailySkribbl => {}
        }
        self.changed(clock);
        true
    }

    pub fn close_game(&mut self) {
        self.ui.open_game = None;
    }

    /// The card's status text (`Solved`/`Failed` for today's finished puzzle, `n/3` for Durak,
    /// `Daily` for Skribbl).
    pub fn card_status(&self, game: GameId) -> String {
        let s = &self.record.state;
        let iso = self.today.to_iso();
        let today_done = |active: &str, id: &str, salt: &str, completed: bool, won: bool| {
            let is_today = !salt.is_empty()
                && active == iso
                && id == study_tracker_core::break_room::daily::puzzle_id(&iso, salt);
            if is_today && completed {
                if won { "Solved" } else { "Failed" }.to_string()
            } else {
                String::new()
            }
        };
        match game {
            GameId::DailyDurak => format!("{}/3", s.durak.solved_count),
            GameId::Wordle => today_done(
                &s.wordle.active_date,
                &s.wordle.puzzle_id,
                &s.wordle.seed_salt,
                s.wordle.completed,
                s.wordle.won,
            ),
            GameId::Geodle => today_done(
                &s.geodle.active_date,
                &s.geodle.puzzle_id,
                &s.geodle.seed_salt,
                s.geodle.completed,
                s.geodle.won,
            ),
            GameId::Flaggle => today_done(
                &s.flaggle.active_date,
                &s.flaggle.puzzle_id,
                &s.flaggle.seed_salt,
                s.flaggle.completed,
                s.flaggle.won,
            ),
            GameId::Travle => today_done(
                &s.travle.active_date,
                &s.travle.puzzle_id,
                &s.travle.seed_salt,
                s.travle.completed,
                s.travle.won,
            ),
            GameId::DailySkribbl => "Daily".into(),
        }
    }

    // ------------------------------------------------------------------ small actions

    pub fn add_water(&mut self, clock: &dyn LocalClock) {
        let iso = self.today.to_iso();
        self.record.state.add_water(&iso);
        self.changed(clock);
    }

    pub fn pat_rock(&mut self, clock: &dyn LocalClock) {
        if self.record.state.pat_rock() {
            self.ui.rock_celebrating = true;
        }
        self.ui.rock_bounces += 1;
        self.changed(clock);
    }

    /// The stretch card's ↻.
    pub fn next_stretch(&mut self) {
        self.ui.stretch_index = (self.ui.stretch_index + 1) % STRETCH_COUNT;
    }

    /// The rest-room tree (a device preference in production; persisted with the section).
    pub fn next_tree(&mut self) {
        self.record.rest_tree = (self.record.rest_tree + 1) % REST_TREE_COUNT;
        self.persist();
    }

    // ------------------------------------------------------------------ wordle

    pub fn wordle_letter(&mut self, letter: char) {
        if self.record.state.wordle.completed {
            return;
        }
        let mut draft = self.ui.wordle_draft.clone();
        draft.push(letter);
        self.ui.wordle_draft = study_tracker_core::break_room::wordle::normalize_guess(&draft)
            .chars()
            .take(WORD_LENGTH)
            .collect();
        self.ui.wordle_message.clear();
    }

    pub fn wordle_backspace(&mut self) {
        if self.record.state.wordle.completed {
            return;
        }
        self.ui.wordle_draft.pop();
        self.ui.wordle_message.clear();
    }

    pub fn wordle_enter(&mut self, clock: &dyn LocalClock) {
        let before = self.record.state.wordle.clone();
        match self.record.state.wordle.submit(&self.ui.wordle_draft) {
            WordleSubmit::Ignored => {}
            WordleSubmit::Rejected(message) => self.ui.wordle_message = message,
            WordleSubmit::Accepted { message } => {
                self.ui.wordle_message = message;
                self.ui.wordle_draft.clear();
                if self.record.state.wordle != before {
                    self.changed(clock);
                }
            }
        }
    }

    pub fn wordle_toggle_hard(&mut self, clock: &dyn LocalClock) {
        if self.record.state.wordle.toggle_hard_mode() {
            self.ui.wordle_message.clear();
            self.changed(clock);
        }
    }

    // ------------------------------------------------------------------ geodle / flaggle

    pub fn geodle_set_draft(&mut self, text: &str) {
        self.ui.geodle_draft = text.to_string();
        self.ui.geodle_dropdown = true;
        self.ui.geodle_message.clear();
    }

    pub fn geodle_select(&mut self, name: &str) {
        self.ui.geodle_draft = name.to_string();
        self.ui.geodle_dropdown = false;
        self.ui.geodle_message.clear();
    }

    pub fn geodle_submit(&mut self, clock: &dyn LocalClock) {
        let result = self.record.state.geodle.submit(&self.ui.geodle_draft);
        self.ui.geodle_message = result.message().to_string();
        match result {
            CountrySubmit::NotACountry => self.ui.geodle_dropdown = true,
            CountrySubmit::Accepted { .. } => {
                self.ui.geodle_draft.clear();
                self.ui.geodle_dropdown = false;
                self.changed(clock);
            }
            _ => {}
        }
    }

    pub fn flaggle_set_draft(&mut self, text: &str) {
        self.ui.flaggle_draft = text.to_string();
        self.ui.flaggle_dropdown = true;
        self.ui.flaggle_message.clear();
    }

    pub fn flaggle_select(&mut self, name: &str) {
        self.ui.flaggle_draft = name.to_string();
        self.ui.flaggle_dropdown = false;
        self.ui.flaggle_message.clear();
    }

    pub fn flaggle_submit(&mut self, flags: &mut dyn FlagSimilarity, clock: &dyn LocalClock) {
        let country = match self.record.state.flaggle.check(&self.ui.flaggle_draft) {
            Ok(country) => country,
            Err(result) => {
                self.ui.flaggle_message = result.message().to_string();
                if result == CountrySubmit::NotACountry {
                    self.ui.flaggle_dropdown = true;
                }
                return;
            }
        };
        let answer = self.record.state.flaggle.answer.clone();
        let Some(similarity) = flags.similarity(country, &answer) else {
            self.ui.flaggle_message = "Could not render that flag. Try another guess.".into();
            return;
        };
        let result = self.record.state.flaggle.record(country, similarity);
        self.ui.flaggle_message = result.message().to_string();
        self.ui.flaggle_draft.clear();
        self.ui.flaggle_dropdown = false;
        self.changed(clock);
    }

    pub fn flaggle_preview(&self) -> PreviewRequest {
        self.record.state.flaggle.preview()
    }

    // ------------------------------------------------------------------ durak

    /// Runs one Durak handler against the stored puzzle; persists when it changed anything.
    pub fn durak_action(
        &mut self,
        clock: &dyn LocalClock,
        action: impl FnOnce(
            &mut DurakSession,
            &mut study_tracker_core::break_room::durak::DurakPuzzle,
            &str,
            f64,
        ) -> bool,
    ) {
        let iso = self.today.to_iso();
        let pick = self.rng.next_f64();
        if action(&mut self.durak, &mut self.record.state.durak, &iso, pick) {
            self.changed(clock);
        }
    }

    // ------------------------------------------------------------------ rest room

    pub fn select_room(&mut self, room: Room) {
        self.ui.room = room;
    }

    pub fn rest_toggle(&mut self, now: Instant) {
        self.ui.rest.toggle();
        self.ui.rest_anchor = self
            .ui
            .rest
            .running
            .then_some((now, self.ui.rest.remaining));
    }

    pub fn rest_reset(&mut self) {
        self.ui.rest.reset();
        self.ui.rest_anchor = None;
    }

    pub fn rest_set_seconds(&mut self, seconds: u32) {
        if self.ui.rest.running {
            return;
        }
        self.ui.rest.set_seconds(seconds);
    }

    /// Advances the rest timer from its monotonic anchor. Returns whether it changed.
    pub fn rest_tick(&mut self, now: Instant) -> bool {
        let Some((anchor, at_start)) = self.ui.rest_anchor else {
            return false;
        };
        let elapsed = now.saturating_duration_since(anchor).as_secs();
        let left = u64::from(at_start).saturating_sub(elapsed) as u32;
        let changed = self.ui.rest.advance_to(left);
        if !self.ui.rest.running {
            self.ui.rest_anchor = None;
        }
        changed
    }

    pub fn breath_toggle(&mut self, now: Instant) {
        self.ui.breathing.toggle();
        self.ui.breath_anchor = self.ui.breathing.on.then_some(now);
    }

    pub fn breath_tick(&mut self, now: Instant) -> bool {
        let Some(anchor) = self.ui.breath_anchor else {
            return false;
        };
        let changed = self
            .ui
            .breathing
            .advance_to(now.saturating_duration_since(anchor).as_secs());
        if !self.ui.breathing.on {
            self.ui.breath_anchor = None;
        }
        changed
    }

    /// Whether a 1 Hz clock is needed at all (rest timer running or breathing on).
    pub fn needs_seconds_clock(&self) -> bool {
        self.ui.rest_anchor.is_some() || self.ui.breath_anchor.is_some()
    }
}

#[cfg(test)]
mod tests;
