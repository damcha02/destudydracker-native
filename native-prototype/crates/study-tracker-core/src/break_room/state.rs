//! The persisted Break Room state and its unlock economy (production `AppState` fields, v0.1.67).
//!
//! **Economy** (`App.tsx`, verified against source):
//! - `studyBreakTokens = min(6, floor(todayMinutes / 45))`, where `todayMinutes` is the sum of
//!   *every* session (study, exam **and** break kind; recovered and imported alike; zero-minute
//!   sessions add nothing) that ended on the local calendar day. Tokens are not stored and not
//!   spent: they are a daily allowance recomputed from the sessions.
//! - A game can be unlocked while `unlocked today < tokens`; unlocks are per day
//!   (`unlockedGamesDate`), so every game is locked again tomorrow.
//! - "XP" is only presentation: `todayMinutes % 45` of 45, and "~N min" to the next token
//!   (`max(1, 45 - todayMinutes % 45)`, 0 once all six are earned). There is no XP total, no
//!   level and no reward currency in production.
//! - `totalUnlocks`, `unlockStreak`, `lastUnlockDate`, `speedrunnerToday`, `playedGamesAllTime`,
//!   `badgeCounts`/`badgeCountDates`, water and pet-rock pats are the only progression state.

use std::collections::BTreeMap;

use super::catalog::{GameId, GAME_COUNT};
use super::daily::js_utc_day_number;
use super::durak::DurakPuzzle;
use super::flaggle::FlagglePuzzle;
use super::geodle::GeodlePuzzle;
use super::travle::TravlePuzzle;
use super::wordle::WordlePuzzle;
use crate::dashboard::civil::{CivilDate, LocalClock};
use crate::timer::WallTimestamp;

/// Minutes of study per unlock token.
pub const MINUTES_PER_TOKEN: u64 = 45;

/// `PlayedBreak`. `played_at` is `None` when an imported value was not a parsable date (production
/// then reads `NaN` hours, which is neither early nor late).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayedBreak {
    pub name: String,
    pub played_at: Option<WallTimestamp>,
}

/// Every persisted Break Room field production has (`achievementBoard`, a layout for an
/// achievement display production no longer mounts, is carried opaquely by the application layer).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BreakRoomState {
    pub unlocked_games: Vec<String>,
    pub unlocked_games_date: String,
    pub played_breaks: Vec<PlayedBreak>,
    pub played_breaks_date: String,
    pub total_unlocks: u64,
    pub unlock_streak: u64,
    pub last_unlock_date: String,
    pub speedrunner_today: bool,
    pub played_games_all_time: Vec<String>,
    pub badge_counts: BTreeMap<String, u64>,
    pub badge_count_dates: BTreeMap<String, String>,
    pub water_glasses: u64,
    pub water_date: String,
    pub pet_rock_pats: u64,
    pub achievement_earned_on_dates: BTreeMap<String, String>,
    pub durak: DurakPuzzle,
    pub wordle: WordlePuzzle,
    pub geodle: GeodlePuzzle,
    pub flaggle: FlagglePuzzle,
    pub travle: TravlePuzzle,
}

/// Everything the Break Room surfaces derive from the state for one local day.
#[derive(Debug, Clone, PartialEq)]
pub struct Progression {
    pub today_minutes: u64,
    /// `studyBreakTokens`.
    pub tokens: usize,
    /// `effectiveUnlocked`: today's unlocked game names.
    pub unlocked: Vec<String>,
    pub can_unlock_more: bool,
    /// `minsUntilNext`.
    pub mins_until_next: u64,
    /// `xpProgress` (0..45).
    pub xp_progress: u64,
    /// `xpPercent` (0..100).
    pub xp_percent: f64,
    /// `todayPlayedNames`: distinct names played today, first-played order.
    pub played_today: Vec<String>,
    /// `waterCount`.
    pub water_today: u64,
}

impl Progression {
    pub fn is_unlocked(&self, game: GameId) -> bool {
        self.unlocked.iter().any(|n| n == game.name())
    }
}

/// `streakEmoji`.
pub fn streak_emoji(unlock_streak: u64) -> &'static str {
    match unlock_streak {
        7.. => "\u{1F525}\u{1F525}\u{1F525}",
        3.. => "\u{1F525}\u{1F525}",
        1.. => "\u{1F525}",
        _ => "",
    }
}

impl BreakRoomState {
    /// Today's derived economy. `today_minutes` is `getTodayMinutes` (all session kinds).
    pub fn progression(&self, today: &str, today_minutes: u64) -> Progression {
        let tokens = ((today_minutes / MINUTES_PER_TOKEN) as usize).min(GAME_COUNT);
        let unlocked = if self.unlocked_games_date == today {
            self.unlocked_games.clone()
        } else {
            Vec::new()
        };
        let mut played_today: Vec<String> = Vec::new();
        for played in self.played_today(today) {
            if !played_today.contains(&played.name) {
                played_today.push(played.name.clone());
            }
        }
        let xp_progress = today_minutes % MINUTES_PER_TOKEN;
        Progression {
            today_minutes,
            tokens,
            can_unlock_more: unlocked.len() < tokens,
            unlocked,
            mins_until_next: if tokens < GAME_COUNT {
                (MINUTES_PER_TOKEN - xp_progress).max(1)
            } else {
                0
            },
            xp_progress,
            xp_percent: xp_progress as f64 / MINUTES_PER_TOKEN as f64 * 100.0,
            played_today,
            water_today: if self.water_date == today {
                self.water_glasses
            } else {
                0
            },
        }
    }

    /// `effectivePlayed`.
    pub fn played_today(&self, today: &str) -> &[PlayedBreak] {
        if self.played_breaks_date == today {
            &self.played_breaks
        } else {
            &[]
        }
    }

    /// `unlockGame(name)`, guarded the way production's UI guards it (the Unlock button exists
    /// only for a locked game while `canUnlockMore`), so a repeated click can never unlock twice.
    ///
    /// `earned_token_in_one_session`: some study/exam session that ended today lasted >= 45
    /// minutes (production's Speedrunner condition). `clock` reproduces production's streak
    /// arithmetic exactly, including its quirk: it compares *local* midnight of today with
    /// `new Date(lastUnlockDate)`, which JavaScript parses as *UTC* midnight, so outside UTC the
    /// difference is never exactly 0 or 1 day and the streak restarts at 1 on every unlock.
    pub fn unlock_game(
        &mut self,
        game: GameId,
        today: CivilDate,
        today_minutes: u64,
        earned_token_in_one_session: bool,
        clock: &dyn LocalClock,
    ) -> bool {
        let t = today.to_iso();
        let progression = self.progression(&t, today_minutes);
        if !progression.can_unlock_more || progression.is_unlocked(game) {
            return false;
        }
        let fresh = progression.unlocked;
        let first_today = fresh.is_empty();
        let already_speedrunner = self.last_unlock_date == t && self.speedrunner_today;
        let new_streak = if self.last_unlock_date.is_empty() {
            1
        } else {
            match js_utc_day_number(&self.last_unlock_date) {
                Some(day) => {
                    let previous_ms = day * 86_400_000;
                    let today_ms = clock.local_midnight(today).unix_millis;
                    let diff = today_ms - previous_ms;
                    if diff == 86_400_000 {
                        self.unlock_streak + 1
                    } else if diff == 0 {
                        self.unlock_streak
                    } else {
                        1
                    }
                }
                None => 1,
            }
        };
        let mut unlocked = fresh;
        unlocked.push(game.name().to_string());
        self.unlocked_games = unlocked;
        self.unlocked_games_date = t.clone();
        self.total_unlocks += 1;
        self.speedrunner_today =
            already_speedrunner || (first_today && earned_token_in_one_session);
        self.unlock_streak = new_streak;
        self.last_unlock_date = t;
        true
    }

    /// `logPlayedBreak(name)` (pressing Play).
    pub fn log_played(&mut self, game: GameId, now: WallTimestamp, today: &str) {
        if self.played_breaks_date != today {
            self.played_breaks.clear();
        }
        self.played_breaks_date = today.to_string();
        self.played_breaks.push(PlayedBreak {
            name: game.name().to_string(),
            played_at: Some(now),
        });
        if !self.played_games_all_time.iter().any(|n| n == game.name()) {
            self.played_games_all_time.push(game.name().to_string());
        }
    }

    /// `addWater`.
    pub fn add_water(&mut self, today: &str) {
        let current = if self.water_date == today {
            self.water_glasses
        } else {
            0
        };
        self.water_date = today.to_string();
        self.water_glasses = current + 1;
    }

    /// `patRock`; returns whether this pat starts the 1000-pat celebration.
    pub fn pat_rock(&mut self) -> bool {
        self.pet_rock_pats += 1;
        self.pet_rock_pats == 1000
    }

    /// The daily-badge counter effect: each of the five daily badges counts at most once per local
    /// day, the first time it is seen earned that day. Returns the ids counted now.
    pub fn count_daily_badges(&mut self, hits: &[(&str, bool)], today: &str) -> Vec<String> {
        let mut counted = Vec::new();
        for (id, earned) in hits {
            if !*earned || self.badge_count_dates.get(*id).map(String::as_str) == Some(today) {
                continue;
            }
            *self.badge_counts.entry((*id).to_string()).or_insert(0) += 1;
            self.badge_count_dates
                .insert((*id).to_string(), today.to_string());
            counted.push((*id).to_string());
        }
        counted
    }
}
