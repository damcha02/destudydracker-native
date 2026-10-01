//! Daily Geodle, ported from `desktop/src/lib/geodle.ts` and its `App.tsx` handlers
//! (`initGeodlePuzzle`, `submitGeodleGuess`). Seven guesses; every guess is scored on six clues
//! against the answer from production's own country table.

use super::countries::{countries, find_country, Country};
use super::daily::{daily_index, puzzle_id};

pub const MAX_GUESSES: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClueState {
    Match,
    Miss,
    Higher,
    Lower,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clue {
    pub label: &'static str,
    pub value: String,
    pub state: ClueState,
    pub hint: String,
}

/// `numericClue`: equal is a match, within 20 % of the answer is close, otherwise the direction
/// the answer lies in.
fn numeric_clue(guess: u64, answer: u64) -> ClueState {
    if guess == answer {
        return ClueState::Match;
    }
    let ratio = guess.abs_diff(answer) as f64 / (answer.max(1)) as f64;
    if ratio <= 0.2 {
        ClueState::Close
    } else if guess < answer {
        ClueState::Higher
    } else {
        ClueState::Lower
    }
}

/// `new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: v >= 1e6 ? 1 : 0 })`
/// for non-negative integers: English short scale (K, M, B, T), the default `halfExpand` rounding,
/// trailing zeros dropped, and a value that rounds up to 1000 of a unit promoted to the next unit.
pub fn format_compact(value: u64) -> String {
    let fraction_digits: u32 = if value >= 1_000_000 { 1 } else { 0 };
    if value < 1000 {
        return value.to_string();
    }
    const UNITS: [(u64, &str); 4] = [
        (1_000, "K"),
        (1_000_000, "M"),
        (1_000_000_000, "B"),
        (1_000_000_000_000, "T"),
    ];
    let mut unit_index = UNITS
        .iter()
        .rposition(|(scale, _)| value >= *scale)
        .unwrap_or(0);
    loop {
        let (scale, suffix) = UNITS[unit_index];
        let step = scale / 10u64.pow(fraction_digits);
        // round-half-up in exact integer arithmetic: value / step, to an integer
        let scaled = (u128::from(value) * 2 + u128::from(step)) / (2 * u128::from(step));
        let limit = 1000u128 * 10u128.pow(fraction_digits);
        if scaled >= limit && unit_index + 1 < UNITS.len() {
            unit_index += 1;
            continue;
        }
        let divisor = 10u128.pow(fraction_digits);
        let whole = scaled / divisor;
        let frac = scaled % divisor;
        return if frac == 0 {
            format!("{whole}{suffix}")
        } else {
            format!("{whole}.{frac}{suffix}")
        };
    }
}

fn numeric_hint(population: bool, state: ClueState, guess: &Country) -> String {
    let noun = if population { "people" } else { "area" };
    let name = guess.name;
    match state {
        ClueState::Match => format!("The answer has the same {noun} as {name}."),
        ClueState::Close => format!("The answer is close to {name}'s {noun}."),
        ClueState::Higher if population => format!("The answer has more people than {name}."),
        ClueState::Higher => format!("The answer is larger than {name}."),
        ClueState::Lower if population => format!("The answer has fewer people than {name}."),
        ClueState::Lower => format!("The answer is smaller than {name}."),
        ClueState::Miss => String::new(),
    }
}

/// `getGeodleAnswerForDate` (keyed by country code).
pub fn answer_for_date(date: &str, seed_salt: &str) -> &'static str {
    let list = countries();
    list[daily_index(list.iter().map(|c| c.code), seed_salt, date)].name
}

/// `getGeodlePuzzleId`.
pub fn geodle_puzzle_id(date: &str, seed_salt: &str) -> String {
    puzzle_id(date, seed_salt)
}

/// `scoreGeodleGuess`; empty when either name is not a known country.
pub fn score_guess(guess_name: &str, answer_name: &str) -> Vec<Clue> {
    let (Some(guess), Some(answer)) = (find_country(guess_name), find_country(answer_name)) else {
        return Vec::new();
    };
    let population = numeric_clue(guess.population, answer.population);
    let area = numeric_clue(guess.area_km2, answer.area_km2);
    let coast = |c: &Country| {
        if c.landlocked {
            "is landlocked"
        } else {
            "touches water"
        }
    };
    let same = |a: &str, b: &str| {
        if a == b {
            ClueState::Match
        } else {
            ClueState::Miss
        }
    };
    vec![
        Clue {
            label: "Continent",
            value: guess.continent.to_string(),
            state: same(guess.continent, answer.continent),
            hint: if guess.continent == answer.continent {
                format!("Both countries are in {}.", guess.continent)
            } else {
                format!("The answer is not in {}.", guess.continent)
            },
        },
        Clue {
            label: "Population",
            value: format_compact(guess.population),
            state: population,
            hint: numeric_hint(true, population, guess),
        },
        Clue {
            label: "Landlocked",
            value: if guess.landlocked { "Yes" } else { "No" }.to_string(),
            state: if guess.landlocked == answer.landlocked {
                ClueState::Match
            } else {
                ClueState::Miss
            },
            hint: if guess.landlocked == answer.landlocked {
                format!("Both countries {}.", coast(guess))
            } else {
                format!(
                    "{} {}, but the answer {}.",
                    guess.name,
                    coast(guess),
                    coast(answer)
                )
            },
        },
        Clue {
            label: "Religion",
            value: guess.religion.to_string(),
            state: same(guess.religion, answer.religion),
            hint: if guess.religion == answer.religion {
                format!(
                    "Both countries have {} as the dominant religion.",
                    guess.religion
                )
            } else {
                format!(
                    "The answer does not have {} as the dominant religion.",
                    guess.religion
                )
            },
        },
        Clue {
            label: "Area",
            value: format!("{} km²", format_compact(guess.area_km2)),
            state: area,
            hint: numeric_hint(false, area, guess),
        },
        Clue {
            label: "Gov.",
            value: guess.government.to_string(),
            state: same(guess.government, answer.government),
            hint: if guess.government == answer.government {
                format!("Both countries are {}.", guess.government)
            } else {
                format!("The answer is not {}.", guess.government)
            },
        },
    ]
}

/// `GeodlePuzzleState` (also Flaggle's shape minus the guess payload).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GeodlePuzzle {
    pub seed_salt: String,
    pub active_date: String,
    pub puzzle_id: String,
    pub answer: String,
    pub guesses: Vec<String>,
    pub completed: bool,
    pub won: bool,
}

/// `submitGeodleGuess` / `submitFlaggleGuess` outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountrySubmit {
    /// Not a country from the list: production shows the message and opens the dropdown.
    NotACountry,
    AlreadyGuessed,
    Accepted {
        message: String,
    },
    Ignored,
}

impl CountrySubmit {
    pub fn message(&self) -> &str {
        match self {
            CountrySubmit::NotACountry => "Select a country from the list.",
            CountrySubmit::AlreadyGuessed => "You already guessed that country.",
            CountrySubmit::Accepted { message } => message,
            CountrySubmit::Ignored => "",
        }
    }
}

impl GeodlePuzzle {
    pub fn fresh(date: &str, seed_salt: &str) -> Self {
        Self {
            seed_salt: seed_salt.to_string(),
            active_date: date.to_string(),
            puzzle_id: geodle_puzzle_id(date, seed_salt),
            answer: answer_for_date(date, seed_salt).to_string(),
            guesses: Vec::new(),
            completed: false,
            won: false,
        }
    }

    /// `normalizeGeodlePuzzle`.
    pub fn normalized(self, today: &str, new_salt: impl FnOnce() -> String) -> Self {
        let salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt
        };
        let id = geodle_puzzle_id(today, &salt);
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

    /// `initGeodlePuzzle`.
    pub fn ensure_today(&mut self, today: &str, new_salt: impl FnOnce() -> String) -> bool {
        let salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt.clone()
        };
        let id = geodle_puzzle_id(today, &salt);
        if self.active_date == today && self.puzzle_id == id && !self.answer.is_empty() {
            return false;
        }
        *self = Self::fresh(today, &salt);
        true
    }

    /// `submitGeodleGuess` for the typed draft.
    pub fn submit(&mut self, draft: &str) -> CountrySubmit {
        if self.completed {
            return CountrySubmit::Ignored;
        }
        let Some(country) = find_country(draft) else {
            return CountrySubmit::NotACountry;
        };
        if self.guesses.iter().any(|g| g == country.name) {
            return CountrySubmit::AlreadyGuessed;
        }
        let previous = self.guesses.len();
        let message = if country.name == self.answer {
            format!("Solved in {}.", previous + 1)
        } else if previous + 1 >= MAX_GUESSES {
            format!("Answer: {}.", self.answer)
        } else {
            String::new()
        };
        self.guesses.push(country.name.to_string());
        self.won = country.name == self.answer;
        self.completed = self.won || self.guesses.len() >= MAX_GUESSES;
        CountrySubmit::Accepted { message }
    }
}

/// `GEODLE_COUNTRY_COUNT` / `FLAGGLE_COUNTRY_COUNT`.
pub fn countries_count() -> usize {
    countries().len()
}
