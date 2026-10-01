//! Daily Wordle, ported from `desktop/src/lib/wordle.ts` and its `App.tsx` handlers
//! (`initWordlePuzzle`, `submitWordleGuess`, `toggleWordleHardMode`).
//!
//! Word source: production's own `wordleWords.ts` ("generated from the local system dictionary plus
//! dwyl/english-words"), extracted verbatim. The answer list and its order are part of the daily
//! semantics; accepted guesses are `answers ∪ accepted`.

use std::collections::HashSet;
use std::sync::OnceLock;

use super::daily::{daily_index, puzzle_id};

pub const WORD_LENGTH: usize = 5;
pub const MAX_GUESSES: usize = 6;

const ANSWERS_DATA: &str = include_str!("../../data/break_room/wordle-answers.txt");
const GUESSES_DATA: &str = include_str!("../../data/break_room/wordle-guesses.txt");

/// `WORDLE_ANSWERS` in production order.
pub fn answers() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| ANSWERS_DATA.lines().filter(|l| !l.is_empty()).collect())
}

fn accepted() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        GUESSES_DATA
            .lines()
            .filter(|l| !l.is_empty())
            .chain(answers().iter().copied())
            .collect()
    })
}

/// `WORDLE_ANSWER_COUNT`.
pub fn answer_count() -> usize {
    answers().len()
}

/// `WORDLE_ACCEPTED_GUESS_COUNT` (the size of the union).
pub fn accepted_guess_count() -> usize {
    accepted().len()
}

/// `normalizeWordleGuess`: trim, lower-case, keep only `a-z`, at most five letters.
pub fn normalize_guess(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .filter(char::is_ascii_lowercase)
        .take(WORD_LENGTH)
        .collect()
}

/// `isAcceptedWordleGuess` (case-insensitive).
pub fn is_accepted_guess(value: &str) -> bool {
    accepted().contains(value.to_lowercase().as_str())
}

/// `getWordleAnswerForDate`.
pub fn answer_for_date(date: &str, seed_salt: &str) -> &'static str {
    let list = answers();
    list[daily_index(list.iter().copied(), seed_salt, date)]
}

/// `getWordlePuzzleId`.
pub fn wordle_puzzle_id(date: &str, seed_salt: &str) -> String {
    puzzle_id(date, seed_salt)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LetterState {
    // Ordered by production's keyboard strength: absent 1 < present 2 < correct 3.
    Absent,
    Present,
    Correct,
}

/// `scoreWordleGuess`: greens first, then yellows from the answer's *remaining* letter counts, so
/// a repeated guess letter is only yellow as many times as the answer still has it.
pub fn score_guess(guess: &str, answer: &str) -> Vec<(char, LetterState)> {
    let guess: Vec<char> = normalize_guess(guess).chars().collect();
    let answer: Vec<char> = normalize_guess(answer).chars().collect();
    let mut result: Vec<(char, LetterState)> =
        guess.iter().map(|&c| (c, LetterState::Absent)).collect();
    let mut remaining: Vec<(char, u32)> = Vec::new();
    let bump = |remaining: &mut Vec<(char, u32)>, c: char| match remaining
        .iter_mut()
        .find(|(k, _)| *k == c)
    {
        Some(entry) => entry.1 += 1,
        None => remaining.push((c, 1)),
    };
    // Production loops over all five positions; `undefined` slots of a short guess/answer
    // behave as non-matching, which `get` reproduces.
    for i in 0..WORD_LENGTH {
        match (guess.get(i), answer.get(i)) {
            (Some(g), Some(a)) if g == a => result[i].1 = LetterState::Correct,
            (_, Some(&a)) => bump(&mut remaining, a),
            _ => {}
        }
    }
    for (i, &g) in guess.iter().enumerate().take(WORD_LENGTH) {
        if result[i].1 == LetterState::Correct {
            continue;
        }
        if let Some(entry) = remaining.iter_mut().find(|(k, n)| *k == g && *n > 0) {
            result[i].1 = LetterState::Present;
            entry.1 -= 1;
        }
    }
    result
}

/// `getWordleKeyboardState`: the strongest state each letter has reached over all guesses.
pub fn keyboard_state(guesses: &[String], answer: &str) -> Vec<(char, LetterState)> {
    let mut states: Vec<(char, LetterState)> = Vec::new();
    for guess in guesses {
        for (letter, state) in score_guess(guess, answer) {
            match states.iter_mut().find(|(k, _)| *k == letter) {
                Some(entry) if state > entry.1 => entry.1 = state,
                Some(_) => {}
                None => states.push((letter, state)),
            }
        }
    }
    states
}

/// `getWordleHardModeViolation`: the first broken rule, as production's message, or `None`.
pub fn hard_mode_violation(guess: &str, previous: &[String], answer: &str) -> Option<String> {
    let guess: Vec<char> = normalize_guess(guess).chars().collect();
    // Maps keep insertion order like production's `Map`/`Set`, which decides which message wins.
    let mut required_positions: Vec<(usize, char)> = Vec::new();
    let mut required_counts: Vec<(char, usize)> = Vec::new();
    let mut eliminated: Vec<char> = Vec::new();
    let mut revealed: Vec<char> = Vec::new();
    for previous_guess in previous {
        let mut revealed_counts: Vec<(char, usize)> = Vec::new();
        for (index, (letter, state)) in score_guess(previous_guess, answer).into_iter().enumerate()
        {
            if state == LetterState::Correct {
                match required_positions.iter_mut().find(|(i, _)| *i == index) {
                    Some(entry) => entry.1 = letter,
                    None => required_positions.push((index, letter)),
                }
            }
            if state != LetterState::Absent {
                if !revealed.contains(&letter) {
                    revealed.push(letter);
                }
                match revealed_counts.iter_mut().find(|(k, _)| *k == letter) {
                    Some(entry) => entry.1 += 1,
                    None => revealed_counts.push((letter, 1)),
                }
            } else if !eliminated.contains(&letter) {
                eliminated.push(letter);
            }
        }
        for (letter, count) in revealed_counts {
            match required_counts.iter_mut().find(|(k, _)| *k == letter) {
                Some(entry) => entry.1 = entry.1.max(count),
                None => required_counts.push((letter, count)),
            }
        }
    }
    for (index, letter) in &required_positions {
        if guess.get(*index) != Some(letter) {
            return Some(format!(
                "{} must be in position {}.",
                letter.to_ascii_uppercase(),
                index + 1
            ));
        }
    }
    for (letter, count) in &required_counts {
        let actual = guess.iter().filter(|c| *c == letter).count();
        if actual < *count {
            let upper = letter.to_ascii_uppercase();
            return Some(if *count == 1 {
                format!("Guess must contain {upper}.")
            } else {
                format!("Guess must contain {count} {upper}s.")
            });
        }
    }
    for letter in &eliminated {
        if !revealed.contains(letter) && guess.contains(letter) {
            return Some(format!(
                "{} has been eliminated.",
                letter.to_ascii_uppercase()
            ));
        }
    }
    None
}

/// `WordlePuzzleState`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WordlePuzzle {
    pub seed_salt: String,
    pub active_date: String,
    pub puzzle_id: String,
    pub answer: String,
    pub guesses: Vec<String>,
    pub completed: bool,
    pub won: bool,
    pub hard_mode: bool,
}

/// What a guess submission did (`submitWordleGuess`). Rejections leave the puzzle untouched and
/// carry production's status message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WordleSubmit {
    Rejected(String),
    /// Accepted; the status line production shows next (empty while the game continues).
    Accepted {
        message: String,
    },
    /// Already completed, or the word was already guessed: production returns silently.
    Ignored,
}

impl WordlePuzzle {
    /// `makeDefaultWordlePuzzle` / the fresh branch of `initWordlePuzzle`.
    pub fn fresh(date: &str, seed_salt: &str, hard_mode: bool) -> Self {
        Self {
            seed_salt: seed_salt.to_string(),
            active_date: date.to_string(),
            puzzle_id: wordle_puzzle_id(date, seed_salt),
            answer: answer_for_date(date, seed_salt).to_string(),
            guesses: Vec::new(),
            completed: false,
            won: false,
            hard_mode,
        }
    }

    /// `normalizeWordlePuzzle` (load time): keep the stored puzzle only if it is today's under its
    /// own salt, otherwise today's fresh puzzle; hard mode always survives. `new_salt` is only
    /// used when the stored salt is missing/empty.
    pub fn normalized(mut self, today: &str, new_salt: impl FnOnce() -> String) -> Self {
        if self.seed_salt.is_empty() {
            self.seed_salt = new_salt();
        }
        let id = wordle_puzzle_id(today, &self.seed_salt);
        let current = self.active_date == today && self.puzzle_id == id;
        let valid_word = |w: &String| w.len() == 5 && w.bytes().all(|b| b.is_ascii_lowercase());
        Self {
            answer: if current && valid_word(&self.answer) {
                self.answer.clone()
            } else {
                answer_for_date(today, &self.seed_salt).to_string()
            },
            guesses: if current {
                self.guesses
                    .iter()
                    .filter(|g| valid_word(g))
                    .take(MAX_GUESSES)
                    .cloned()
                    .collect()
            } else {
                Vec::new()
            },
            completed: current && self.completed,
            won: current && self.won,
            active_date: today.to_string(),
            puzzle_id: id,
            hard_mode: self.hard_mode,
            seed_salt: self.seed_salt,
        }
    }

    /// `initWordlePuzzle` (on opening the game): a no-op when today's puzzle is already loaded,
    /// otherwise a fresh one. Returns whether anything changed.
    pub fn ensure_today(&mut self, today: &str, new_salt: impl FnOnce() -> String) -> bool {
        let salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt.clone()
        };
        let id = wordle_puzzle_id(today, &salt);
        if self.active_date == today && self.puzzle_id == id && !self.answer.is_empty() {
            return false;
        }
        *self = Self::fresh(today, &salt, self.hard_mode);
        true
    }

    /// `wordleHardModeLocked`.
    pub fn hard_mode_locked(&self) -> bool {
        !self.guesses.is_empty() && !self.completed
    }

    /// `toggleWordleHardMode`. Returns whether it changed.
    pub fn toggle_hard_mode(&mut self) -> bool {
        if self.hard_mode_locked() {
            return false;
        }
        self.hard_mode = !self.hard_mode;
        true
    }

    /// `submitWordleGuess` for the typed draft.
    pub fn submit(&mut self, draft: &str) -> WordleSubmit {
        if self.completed {
            return WordleSubmit::Ignored;
        }
        let guess = normalize_guess(draft);
        if guess.len() != WORD_LENGTH {
            return WordleSubmit::Rejected("Enter 5 letters.".into());
        }
        if !is_accepted_guess(&guess) {
            return WordleSubmit::Rejected("Not in the word list.".into());
        }
        if self.hard_mode {
            if let Some(violation) = hard_mode_violation(&guess, &self.guesses, &self.answer) {
                return WordleSubmit::Rejected(violation);
            }
        }
        // The state update ignores a word already guessed, but production still sets the status
        // message from the pre-update state and clears the draft.
        let previous = self.guesses.len();
        let message = if guess == self.answer {
            format!("Solved in {}.", previous + 1)
        } else if previous + 1 >= MAX_GUESSES {
            format!("Answer: {}", self.answer.to_uppercase())
        } else {
            String::new()
        };
        if self.guesses.contains(&guess) {
            return WordleSubmit::Accepted { message };
        }
        self.guesses.push(guess.clone());
        self.won = guess == self.answer;
        self.completed = self.won || self.guesses.len() >= MAX_GUESSES;
        WordleSubmit::Accepted { message }
    }
}
