//! Daily Flaggle, ported from `desktop/src/lib/flaggle.ts` and its `App.tsx` handlers.
//!
//! Production rasterizes two 4:3 flag SVGs into 240x180 canvases and compares them pixel by pixel:
//! a target pixel "matches" when both pixels are opaque (alpha >= 128) and their RGB distance is at
//! most 52. Matched pixels of the target stay visible, the rest are painted `rgb(25, 25, 25)`;
//! `similarity` is matched / visible target pixels, rounded to one decimal. The preview shows the
//! target revealed by the union of all guesses (or the whole flag once solved/over).
//!
//! The rasterization itself (SVG -> RGBA) is the application's job; this module only defines the
//! pure pixel rules over RGBA8 buffers, so they are testable without any renderer.

use super::countries::{countries, find_country};
use super::daily::{daily_index, puzzle_id};
use super::geodle::CountrySubmit;

pub const MAX_GUESSES: usize = 7;
pub const FLAG_WIDTH: u32 = 240;
pub const FLAG_HEIGHT: u32 = 180;
pub const COLOR_TOLERANCE: f64 = 52.0;
/// The colour production paints over every pixel that is not revealed.
pub const HIDDEN_RGB: [u8; 3] = [25, 25, 25];

/// `getFlaggleAnswerForDate` (keyed by country code).
pub fn answer_for_date(date: &str, seed_salt: &str) -> &'static str {
    let list = countries();
    list[daily_index(list.iter().map(|c| c.code), seed_salt, date)].name
}

pub fn flaggle_puzzle_id(date: &str, seed_salt: &str) -> String {
    puzzle_id(date, seed_salt)
}

/// `flagPath`: the asset key of a country's flag (`<iso2 lower-case>.svg`), `None` for an unknown
/// name.
pub fn flag_asset_name(country_name: &str) -> Option<String> {
    find_country(country_name).map(|c| format!("{}.svg", c.iso2.to_ascii_lowercase()))
}

/// `pixelsMatchAt` for one RGBA pixel pair.
pub fn pixels_match(target: &[u8], guess: &[u8]) -> bool {
    if target[3] < 128 || guess[3] < 128 {
        return false;
    }
    let d = |i: usize| f64::from(target[i]) - f64::from(guess[i]);
    (d(0) * d(0) + d(1) * d(1) + d(2) * d(2)).sqrt() <= COLOR_TOLERANCE
}

/// `maskFlagByTargetColors`' similarity: `round(matched / visible * 1000) / 10`, 0 for a flag with
/// no opaque pixel. Both buffers are RGBA8 of the same size.
pub fn similarity(target: &[u8], guess: &[u8]) -> f64 {
    let (mut visible, mut matched) = (0u64, 0u64);
    for (t, g) in target.chunks_exact(4).zip(guess.chunks_exact(4)) {
        if t[3] < 128 {
            continue;
        }
        visible += 1;
        if pixels_match(t, g) {
            matched += 1;
        }
    }
    if visible == 0 {
        return 0.0;
    }
    // `Math.round` rounds .5 up (towards +infinity).
    (matched as f64 / visible as f64 * 1000.0 + 0.5).floor() / 10.0
}

/// `revealTargetFlagByGuesses`' masking, in place on a copy of the target: every opaque target
/// pixel no guess matches becomes [`HIDDEN_RGB`] (alpha untouched).
pub fn reveal(target: &[u8], guesses: &[&[u8]]) -> Vec<u8> {
    let mut out = target.to_vec();
    for (i, pixel) in out.chunks_exact_mut(4).enumerate() {
        if pixel[3] < 128 {
            continue;
        }
        let at = i * 4;
        let t = &target[at..at + 4];
        if guesses.iter().any(|g| pixels_match(t, &g[at..at + 4])) {
            continue;
        }
        pixel[..3].copy_from_slice(&HIDDEN_RGB);
    }
    out
}

/// One stored guess. Production also persists the masked PNG as a data URL; it is never displayed
/// (the history shows the guessed country's own flag) and is a pure function of the two flags, so
/// native does not keep it.
#[derive(Debug, Clone, PartialEq)]
pub struct FlaggleGuess {
    pub country: String,
    pub similarity: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FlagglePuzzle {
    pub seed_salt: String,
    pub active_date: String,
    pub puzzle_id: String,
    pub answer: String,
    pub guesses: Vec<FlaggleGuess>,
    pub completed: bool,
    pub won: bool,
}

/// What the preview should show (`refreshFlagglePreview`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewRequest {
    /// "Guess a country to reveal matching flag colors."
    Empty,
    /// The whole target flag (solved, out of guesses, or the answer was guessed).
    Full,
    /// The target revealed by these guessed countries.
    RevealedBy(Vec<String>),
}

impl FlagglePuzzle {
    pub fn fresh(date: &str, seed_salt: &str) -> Self {
        Self {
            seed_salt: seed_salt.to_string(),
            active_date: date.to_string(),
            puzzle_id: flaggle_puzzle_id(date, seed_salt),
            answer: answer_for_date(date, seed_salt).to_string(),
            guesses: Vec::new(),
            completed: false,
            won: false,
        }
    }

    /// `normalizeFlagglePuzzle`.
    pub fn normalized(self, today: &str, new_salt: impl FnOnce() -> String) -> Self {
        let salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt
        };
        let id = flaggle_puzzle_id(today, &salt);
        let current = self.active_date == today && self.puzzle_id == id;
        Self {
            answer: if current && !self.answer.is_empty() {
                self.answer
            } else {
                answer_for_date(today, &salt).to_string()
            },
            guesses: if current {
                self.guesses.into_iter().take(MAX_GUESSES).collect()
            } else {
                Vec::new()
            },
            completed: current && self.completed,
            won: current && self.won,
            active_date: today.to_string(),
            puzzle_id: id,
            seed_salt: salt,
        }
    }

    /// `initFlagglePuzzle`.
    pub fn ensure_today(&mut self, today: &str, new_salt: impl FnOnce() -> String) -> bool {
        let salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt.clone()
        };
        let id = flaggle_puzzle_id(today, &salt);
        if self.active_date == today && self.puzzle_id == id && !self.answer.is_empty() {
            return false;
        }
        *self = Self::fresh(today, &salt);
        true
    }

    /// The validation half of `submitFlaggleGuess`; `Ok(country name)` means the caller should
    /// compute the similarity and then call [`FlagglePuzzle::record`].
    pub fn check(&self, draft: &str) -> Result<&'static str, CountrySubmit> {
        if self.completed {
            return Err(CountrySubmit::Ignored);
        }
        let Some(country) = find_country(draft) else {
            return Err(CountrySubmit::NotACountry);
        };
        if self.guesses.iter().any(|g| g.country == country.name) {
            return Err(CountrySubmit::AlreadyGuessed);
        }
        Ok(country.name)
    }

    /// The state half of `submitFlaggleGuess`, once the similarity is known.
    pub fn record(&mut self, country: &str, similarity: f64) -> CountrySubmit {
        if self.completed || self.guesses.iter().any(|g| g.country == country) {
            return CountrySubmit::Ignored;
        }
        let previous = self.guesses.len();
        let message = if country == self.answer {
            format!("Solved in {}.", previous + 1)
        } else if previous + 1 >= MAX_GUESSES {
            format!("Answer: {}.", self.answer)
        } else {
            String::new()
        };
        self.guesses.push(FlaggleGuess {
            country: country.to_string(),
            similarity,
        });
        self.won = country == self.answer;
        self.completed = self.won || self.guesses.len() >= MAX_GUESSES;
        CountrySubmit::Accepted { message }
    }

    /// What the preview card shows for the current state. Production refreshes it after each guess
    /// with `revealFull = solved || out of guesses`, and on open from the stored guesses.
    pub fn preview(&self) -> PreviewRequest {
        let names: Vec<String> = self.guesses.iter().map(|g| g.country.clone()).collect();
        if self.completed || names.iter().any(|n| *n == self.answer) {
            return PreviewRequest::Full;
        }
        if names.is_empty() {
            return PreviewRequest::Empty;
        }
        PreviewRequest::RevealedBy(names)
    }
}
