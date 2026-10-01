//! Break Room persistence (Stage 20): the `break_room` section of the native store.
//!
//! ```json
//! "break_room": { "unlockedGames": [...], "petRockPats": 12, "wordlePuzzle": {...}, ..., "restTree": 2 }
//! ```
//!
//! * **Production's own field names and shapes** (`AppState` in `desktop/src/types.ts`), so a
//!   production backup imports by copying its fields in and a future export is a copy out.
//! * **Additive, no schema bump**: a Stage 15-19 store has no section and loads as production's
//!   `defaultState`; an older build keeps the section untouched in its `other` map.
//! * **Tolerant per field**, like production (which spreads the raw object over its defaults): a
//!   malformed field falls back to its default without failing the rest. Truthiness follows
//!   JavaScript where production only tests truthiness (`speedrunnerToday`, `completed`, `won`).
//! * Unknown keys inside the section are preserved on write.
//! * `achievementBoard` (the layout of an achievement wall production no longer mounts) is kept
//!   as production normalizes it, and otherwise carried opaquely.
//! * Flaggle guesses are written with `maskedFlagDataUrl: ""`: production keeps a guess only when
//!   that field is a string, and the mask itself is derived (never displayed), so an empty string
//!   keeps the data production-compatible without storing pixels.
//! * The daily puzzles are stored as they are; their load-time "is this still today's puzzle"
//!   normalization needs today's date and runs in the controller, exactly like production's
//!   `normalize*Puzzle` at every load.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use study_tracker_core::break_room::durak::{DurakPuzzle, StoredEntry};
use study_tracker_core::break_room::flaggle::{FlaggleGuess, FlagglePuzzle};
use study_tracker_core::break_room::geodle::GeodlePuzzle;
use study_tracker_core::break_room::state::{BreakRoomState, PlayedBreak, TravlePuzzle};
use study_tracker_core::break_room::wordle::WordlePuzzle;
use study_tracker_core::dashboard::civil::to_iso_utc_string;

use crate::persistence::migration::parse_iso_wall_timestamp;
use crate::persistence::store::{NativeStore, StoreEnvelope};

pub const SECTION: &str = "break_room";

/// Every production `AppState` key this section owns, in production's order.
pub const PRODUCTION_KEYS: [&str; 21] = [
    "unlockedGames",
    "unlockedGamesDate",
    "playedBreaks",
    "playedBreaksDate",
    "totalUnlocks",
    "unlockStreak",
    "lastUnlockDate",
    "speedrunnerToday",
    "playedGamesAllTime",
    "badgeCounts",
    "badgeCountDates",
    "waterGlasses",
    "waterDate",
    "petRockPats",
    "achievementBoard",
    "achievementEarnedOnDates",
    "durakPuzzle",
    "wordlePuzzle",
    "geodlePuzzle",
    "flagglePuzzle",
    "travlePuzzle",
];

/// The device-local rest-room tree (`study-tracker-rest-tree`, a production preference).
pub const REST_TREE_KEY: &str = "restTree";
pub const REST_TREE_COUNT: u32 = 4;

/// What the section holds beyond the core state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BreakRoomRecord {
    pub state: BreakRoomState,
    pub rest_tree: u32,
    /// `achievementBoard`, normalized like production; not used by any mounted surface.
    pub achievement_board: Vec<Value>,
    /// Keys of the section this build does not model, written back untouched.
    pub unknown: Map<String, Value>,
}

/// JavaScript truthiness of a JSON value.
pub fn js_truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

fn string(obj: &Map<String, Value>, key: &str) -> String {
    obj.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// A non-negative whole count from a JSON number (fractions truncated, negatives/NaN -> 0).
fn count(value: Option<&Value>) -> u64 {
    value
        .and_then(Value::as_f64)
        .filter(|f| f.is_finite() && *f > 0.0)
        .map(|f| f as u64)
        .unwrap_or(0)
}

fn count_map(value: Option<&Value>) -> BTreeMap<String, u64> {
    value
        .and_then(Value::as_object)
        .map(|obj| {
            obj.iter()
                .filter(|(_, v)| v.is_number())
                .map(|(k, v)| (k.clone(), count(Some(v))))
                .collect()
        })
        .unwrap_or_default()
}

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn object(value: Option<&Value>) -> Map<String, Value> {
    value
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn parse_wordle(value: Option<&Value>) -> WordlePuzzle {
    let o = object(value);
    WordlePuzzle {
        seed_salt: string(&o, "seedSalt"),
        active_date: string(&o, "activeDate"),
        puzzle_id: string(&o, "puzzleId"),
        answer: string(&o, "answer"),
        guesses: strings(o.get("guesses")),
        completed: js_truthy(o.get("completed")),
        won: js_truthy(o.get("won")),
        hard_mode: js_truthy(o.get("hardMode")),
    }
}

fn parse_geodle(value: Option<&Value>) -> GeodlePuzzle {
    let o = object(value);
    GeodlePuzzle {
        seed_salt: string(&o, "seedSalt"),
        active_date: string(&o, "activeDate"),
        puzzle_id: string(&o, "puzzleId"),
        answer: string(&o, "answer"),
        guesses: strings(o.get("guesses")),
        completed: js_truthy(o.get("completed")),
        won: js_truthy(o.get("won")),
    }
}

fn parse_flaggle(value: Option<&Value>) -> FlagglePuzzle {
    let o = object(value);
    // `normalizeFlagglePuzzle` keeps a guess only with a string country, a number similarity and a
    // string `maskedFlagDataUrl`.
    let guesses = o
        .get("guesses")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|g| {
                    let g = g.as_object()?;
                    let country = g.get("country")?.as_str()?;
                    let similarity = g.get("similarity")?.as_f64()?;
                    g.get("maskedFlagDataUrl")?.as_str()?;
                    Some(FlaggleGuess {
                        country: country.to_string(),
                        similarity,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    FlagglePuzzle {
        seed_salt: string(&o, "seedSalt"),
        active_date: string(&o, "activeDate"),
        puzzle_id: string(&o, "puzzleId"),
        answer: string(&o, "answer"),
        guesses,
        completed: js_truthy(o.get("completed")),
        won: js_truthy(o.get("won")),
    }
}

fn parse_travle(value: Option<&Value>) -> TravlePuzzle {
    let o = object(value);
    TravlePuzzle {
        seed_salt: string(&o, "seedSalt"),
        active_date: string(&o, "activeDate"),
        puzzle_id: string(&o, "puzzleId"),
        start: string(&o, "start"),
        target: string(&o, "target"),
        guesses: strings(o.get("guesses")),
        completed: js_truthy(o.get("completed")),
        won: js_truthy(o.get("won")),
    }
}

fn parse_durak(value: Option<&Value>) -> DurakPuzzle {
    let Some(o) = value.and_then(Value::as_object) else {
        return DurakPuzzle::default();
    };
    let defaults = DurakPuzzle::default();
    let opt_string = |key: &str| o.get(key).and_then(Value::as_str).map(str::to_string);
    DurakPuzzle {
        seed: opt_string("seed").filter(|s| !s.is_empty()),
        hint: string(o, "hint"),
        player_hand: strings(o.get("playerHand")),
        cpu_hand: strings(o.get("cpuHand")),
        trump_suit: opt_string("trumpSuit").unwrap_or(defaults.trump_suit),
        table: o
            .get("table")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|e| {
                        let e = e.as_object()?;
                        Some(StoredEntry {
                            attack: e.get("attack")?.as_str()?.to_string(),
                            defense: e.get("defense").and_then(Value::as_str).map(str::to_string),
                            attack_by: e
                                .get("attackBy")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                            defense_by: e
                                .get("defenseBy")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
        discard_pile: strings(o.get("discardPile")),
        phase: opt_string("phase").unwrap_or(defaults.phase),
        winner: opt_string("winner").filter(|s| !s.is_empty()),
        message: string(o, "message"),
        failures: count(o.get("failures")) as u32,
        completed: js_truthy(o.get("completed")),
        solved_count: count(o.get("solvedCount")) as u32,
    }
}

/// `achievementBoard` filtered as production's loader does (an item needs a string `id` and
/// finite `x`/`y`); the item itself is kept verbatim.
fn parse_board(value: Option<&Value>) -> Vec<Value> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| {
                    item.get("id").is_some_and(Value::is_string)
                        && item
                            .get("x")
                            .and_then(Value::as_f64)
                            .is_some_and(f64::is_finite)
                        && item
                            .get("y")
                            .and_then(Value::as_f64)
                            .is_some_and(f64::is_finite)
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Production's `Math.max(0, Number(v) || 0) % 4` for the rest tree (stored as a string there).
pub fn parse_rest_tree(value: Option<&Value>) -> u32 {
    let n = match value {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    };
    if !n.is_finite() || n <= 0.0 {
        return 0;
    }
    // JS `%` on a float, then used as an array index (fractions never occur in practice)
    (n % f64::from(REST_TREE_COUNT)) as u32
}

/// Reads a section (or a production `AppState`, which has the same keys) field by field.
pub fn parse_section(section: Option<&Value>) -> BreakRoomRecord {
    let o = object(section);
    let played_breaks = o
        .get("playedBreaks")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|p| {
                    let p = p.as_object()?;
                    let name = p.get("name")?.as_str()?.to_string();
                    let played_at = p
                        .get("playedAt")
                        .and_then(Value::as_str)
                        .and_then(|t| parse_iso_wall_timestamp(t).ok());
                    Some(PlayedBreak { name, played_at })
                })
                .collect()
        })
        .unwrap_or_default();
    let state = BreakRoomState {
        unlocked_games: strings(o.get("unlockedGames")),
        unlocked_games_date: string(&o, "unlockedGamesDate"),
        played_breaks,
        played_breaks_date: string(&o, "playedBreaksDate"),
        total_unlocks: count(o.get("totalUnlocks")),
        unlock_streak: count(o.get("unlockStreak")),
        last_unlock_date: string(&o, "lastUnlockDate"),
        speedrunner_today: js_truthy(o.get("speedrunnerToday")),
        played_games_all_time: strings(o.get("playedGamesAllTime")),
        badge_counts: count_map(o.get("badgeCounts")),
        badge_count_dates: string_map(o.get("badgeCountDates")),
        water_glasses: count(o.get("waterGlasses")),
        water_date: string(&o, "waterDate"),
        pet_rock_pats: count(o.get("petRockPats")),
        achievement_earned_on_dates: string_map(o.get("achievementEarnedOnDates")),
        durak: parse_durak(o.get("durakPuzzle")),
        wordle: parse_wordle(o.get("wordlePuzzle")),
        geodle: parse_geodle(o.get("geodlePuzzle")),
        flaggle: parse_flaggle(o.get("flagglePuzzle")),
        travle: parse_travle(o.get("travlePuzzle")),
    };
    let unknown = o
        .iter()
        .filter(|(k, _)| !PRODUCTION_KEYS.contains(&k.as_str()) && k.as_str() != REST_TREE_KEY)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    BreakRoomRecord {
        state,
        rest_tree: parse_rest_tree(o.get(REST_TREE_KEY)),
        achievement_board: parse_board(o.get("achievementBoard")),
        unknown,
    }
}

fn puzzle_head(map: &mut Map<String, Value>, salt: &str, date: &str, id: &str) {
    map.insert("seedSalt".into(), Value::from(salt));
    map.insert("activeDate".into(), Value::from(date));
    map.insert("puzzleId".into(), Value::from(id));
}

/// Writes the record as the section value (production field names), keeping unknown keys.
pub fn write_section(record: &BreakRoomRecord) -> Value {
    let s = &record.state;
    let mut m = record.unknown.clone();
    let strs = |v: &[String]| Value::from(v.to_vec());
    m.insert("unlockedGames".into(), strs(&s.unlocked_games));
    m.insert(
        "unlockedGamesDate".into(),
        Value::from(s.unlocked_games_date.clone()),
    );
    m.insert(
        "playedBreaks".into(),
        Value::Array(
            s.played_breaks
                .iter()
                .map(|p| {
                    let mut o = Map::new();
                    o.insert("name".into(), Value::from(p.name.clone()));
                    o.insert(
                        "playedAt".into(),
                        Value::from(p.played_at.map(to_iso_utc_string).unwrap_or_default()),
                    );
                    Value::Object(o)
                })
                .collect(),
        ),
    );
    m.insert(
        "playedBreaksDate".into(),
        Value::from(s.played_breaks_date.clone()),
    );
    m.insert("totalUnlocks".into(), Value::from(s.total_unlocks));
    m.insert("unlockStreak".into(), Value::from(s.unlock_streak));
    m.insert(
        "lastUnlockDate".into(),
        Value::from(s.last_unlock_date.clone()),
    );
    m.insert("speedrunnerToday".into(), Value::from(s.speedrunner_today));
    m.insert("playedGamesAllTime".into(), strs(&s.played_games_all_time));
    m.insert(
        "badgeCounts".into(),
        Value::Object(
            s.badge_counts
                .iter()
                .map(|(k, v)| (k.clone(), Value::from(*v)))
                .collect(),
        ),
    );
    m.insert(
        "badgeCountDates".into(),
        Value::Object(
            s.badge_count_dates
                .iter()
                .map(|(k, v)| (k.clone(), Value::from(v.clone())))
                .collect(),
        ),
    );
    m.insert("waterGlasses".into(), Value::from(s.water_glasses));
    m.insert("waterDate".into(), Value::from(s.water_date.clone()));
    m.insert("petRockPats".into(), Value::from(s.pet_rock_pats));
    m.insert(
        "achievementBoard".into(),
        Value::Array(record.achievement_board.clone()),
    );
    m.insert(
        "achievementEarnedOnDates".into(),
        Value::Object(
            s.achievement_earned_on_dates
                .iter()
                .map(|(k, v)| (k.clone(), Value::from(v.clone())))
                .collect(),
        ),
    );
    let d = &s.durak;
    let mut durak = Map::new();
    durak.insert(
        "seed".into(),
        d.seed.clone().map_or(Value::Null, Value::from),
    );
    durak.insert("hint".into(), Value::from(d.hint.clone()));
    durak.insert("playerHand".into(), strs(&d.player_hand));
    durak.insert("cpuHand".into(), strs(&d.cpu_hand));
    durak.insert("trumpSuit".into(), Value::from(d.trump_suit.clone()));
    durak.insert(
        "table".into(),
        Value::Array(
            d.table
                .iter()
                .map(|e| {
                    let mut o = Map::new();
                    o.insert("attack".into(), Value::from(e.attack.clone()));
                    if let Some(def) = &e.defense {
                        o.insert("defense".into(), Value::from(def.clone()));
                    }
                    if let Some(by) = &e.attack_by {
                        o.insert("attackBy".into(), Value::from(by.clone()));
                    }
                    if let Some(by) = &e.defense_by {
                        o.insert("defenseBy".into(), Value::from(by.clone()));
                    }
                    Value::Object(o)
                })
                .collect(),
        ),
    );
    durak.insert("discardPile".into(), strs(&d.discard_pile));
    durak.insert("phase".into(), Value::from(d.phase.clone()));
    if let Some(w) = &d.winner {
        durak.insert("winner".into(), Value::from(w.clone()));
    }
    durak.insert("message".into(), Value::from(d.message.clone()));
    durak.insert("failures".into(), Value::from(d.failures));
    durak.insert("completed".into(), Value::from(d.completed));
    durak.insert("solvedCount".into(), Value::from(d.solved_count));
    m.insert("durakPuzzle".into(), Value::Object(durak));

    let w = &s.wordle;
    let mut wordle = Map::new();
    puzzle_head(&mut wordle, &w.seed_salt, &w.active_date, &w.puzzle_id);
    wordle.insert("answer".into(), Value::from(w.answer.clone()));
    wordle.insert("guesses".into(), strs(&w.guesses));
    wordle.insert("completed".into(), Value::from(w.completed));
    wordle.insert("won".into(), Value::from(w.won));
    wordle.insert("hardMode".into(), Value::from(w.hard_mode));
    m.insert("wordlePuzzle".into(), Value::Object(wordle));

    let g = &s.geodle;
    let mut geodle = Map::new();
    puzzle_head(&mut geodle, &g.seed_salt, &g.active_date, &g.puzzle_id);
    geodle.insert("answer".into(), Value::from(g.answer.clone()));
    geodle.insert("guesses".into(), strs(&g.guesses));
    geodle.insert("completed".into(), Value::from(g.completed));
    geodle.insert("won".into(), Value::from(g.won));
    m.insert("geodlePuzzle".into(), Value::Object(geodle));

    let f = &s.flaggle;
    let mut flaggle = Map::new();
    puzzle_head(&mut flaggle, &f.seed_salt, &f.active_date, &f.puzzle_id);
    flaggle.insert("answer".into(), Value::from(f.answer.clone()));
    flaggle.insert(
        "guesses".into(),
        Value::Array(
            f.guesses
                .iter()
                .map(|g| {
                    let mut o = Map::new();
                    o.insert("country".into(), Value::from(g.country.clone()));
                    o.insert("similarity".into(), Value::from(g.similarity));
                    o.insert("maskedFlagDataUrl".into(), Value::from(""));
                    Value::Object(o)
                })
                .collect(),
        ),
    );
    flaggle.insert("completed".into(), Value::from(f.completed));
    flaggle.insert("won".into(), Value::from(f.won));
    m.insert("flagglePuzzle".into(), Value::Object(flaggle));

    let t = &s.travle;
    let mut travle = Map::new();
    puzzle_head(&mut travle, &t.seed_salt, &t.active_date, &t.puzzle_id);
    travle.insert("start".into(), Value::from(t.start.clone()));
    travle.insert("target".into(), Value::from(t.target.clone()));
    travle.insert("guesses".into(), strs(&t.guesses));
    travle.insert("completed".into(), Value::from(t.completed));
    travle.insert("won".into(), Value::from(t.won));
    m.insert("travlePuzzle".into(), Value::Object(travle));

    m.insert(REST_TREE_KEY.into(), Value::from(record.rest_tree));
    Value::Object(m)
}

/// The Break Room part of a production backup, as `restoreBackup` would leave it: the backup's
/// `AppState` *replaces* the whole core state, so any of the 21 keys it lacks (an older backup)
/// comes back as production's default, never as the value the destination had. The rest tree is
/// a preference: replaced only when the backup carries it as a string, otherwise the current one
/// stays.
pub fn from_production_backup(
    state: &Map<String, Value>,
    preferences: &Map<String, Value>,
    current_rest_tree: u32,
) -> BreakRoomRecord {
    let section: Map<String, Value> = PRODUCTION_KEYS
        .iter()
        .filter_map(|k| state.get(*k).map(|v| ((*k).to_string(), v.clone())))
        .collect();
    let mut record = parse_section(Some(&Value::Object(section)));
    record.rest_tree = match preferences.get("study-tracker-rest-tree") {
        Some(value @ Value::String(_)) => parse_rest_tree(Some(value)),
        _ => current_rest_tree,
    };
    record
}

pub trait BreakRoomPort {
    fn load(&self) -> BreakRoomRecord;
    fn persist(&mut self, record: &BreakRoomRecord);
}

/// For tests: remembers the last write.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct MemoryBreakRoomPort {
    pub saved: std::rc::Rc<std::cell::RefCell<Option<Value>>>,
}

#[cfg(test)]
impl BreakRoomPort for MemoryBreakRoomPort {
    fn load(&self) -> BreakRoomRecord {
        parse_section(self.saved.borrow().as_ref())
    }
    fn persist(&mut self, record: &BreakRoomRecord) {
        *self.saved.borrow_mut() = Some(write_section(record));
    }
}

pub struct FileBreakRoomPort {
    store: NativeStore,
}

impl FileBreakRoomPort {
    pub fn new(store: NativeStore) -> Self {
        Self { store }
    }
}

impl BreakRoomPort for FileBreakRoomPort {
    fn load(&self) -> BreakRoomRecord {
        match self.store.load() {
            Ok((envelope, _warnings)) => parse_section(envelope.other.get(SECTION)),
            Err(err) => {
                log::warn!("break room: could not load store, using defaults: {err}");
                BreakRoomRecord::default()
            }
        }
    }

    /// Read-modify-write, like every other port, so no other section is ever clobbered.
    fn persist(&mut self, record: &BreakRoomRecord) {
        let mut envelope = match self.store.load() {
            Ok((envelope, _warnings)) => envelope,
            Err(err) => {
                log::warn!(
                    "break room: existing store unreadable, starting a fresh envelope: {err}"
                );
                StoreEnvelope::default()
            }
        };
        envelope.other.insert(SECTION.into(), write_section(record));
        if let Err(err) = self.store.save(&envelope) {
            log::warn!("break room: failed to save: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn production_state() -> Value {
        json!({
            "unlockedGames": ["Wordle", "Geodle", 7],
            "unlockedGamesDate": "2026-09-30",
            "playedBreaks": [
                {"name": "Wordle", "playedAt": "2026-09-30T06:30:00.000Z"},
                {"name": "Geodle", "playedAt": "not a date"},
                {"playedAt": "2026-09-30T06:30:00.000Z"}
            ],
            "playedBreaksDate": "2026-09-30",
            "totalUnlocks": 12,
            "unlockStreak": 1,
            "lastUnlockDate": "2026-09-30",
            "speedrunnerToday": 1,
            "playedGamesAllTime": ["Wordle", "Geodle", "Daily Durak", "Flaggle"],
            "badgeCounts": {"early-bird": 3, "speedrunner": "2"},
            "badgeCountDates": {"early-bird": "2026-09-30"},
            "waterGlasses": 2,
            "waterDate": "2026-09-30",
            "petRockPats": 1234,
            "achievementBoard": [{"id": "veteran", "x": 0.2, "y": 0.4}, {"id": 3, "x": 0, "y": 0}],
            "achievementEarnedOnDates": {"first-break": "2026-08-30", "bad": 4},
            "durakPuzzle": {"seed": "2026-09-30_0", "hint": "CPU holds A♠", "playerHand": ["10♥", "J♦"], "cpuHand": ["6♠"],
                "trumpSuit": "spades", "table": [{"attack": "7♥"}], "discardPile": [], "phase": "player_defense",
                "message": "m", "failures": 2, "completed": false, "solvedCount": 1},
            "wordlePuzzle": {"seedSalt": "s", "activeDate": "2026-09-30", "puzzleId": "x", "answer": "frost", "guesses": ["slate"], "completed": false, "won": false, "hardMode": true},
            "geodlePuzzle": {"seedSalt": "g", "activeDate": "2026-09-30", "puzzleId": "y", "answer": "Germany", "guesses": ["Brazil"], "completed": false, "won": false},
            "flagglePuzzle": {"seedSalt": "f", "activeDate": "2026-09-30", "puzzleId": "z", "answer": "Jamaica",
                "guesses": [{"country": "Italy", "similarity": 38.4, "maskedFlagDataUrl": "data:image/png;base64,AAAA"}, {"country": "Chad", "similarity": 1}],
                "completed": false, "won": false},
            "travlePuzzle": {"seedSalt": "t", "activeDate": "2026-09-30", "puzzleId": "w", "start": "France", "target": "Poland", "guesses": [], "completed": false, "won": false},
            "sessions": [], "social": {"userId": "u"}
        })
    }

    #[test]
    fn a_production_state_is_read_field_by_field_with_productions_tolerance() {
        let state = production_state();
        let mut prefs = Map::new();
        prefs.insert("study-tracker-rest-tree".into(), json!("6"));
        let record = from_production_backup(state.as_object().unwrap(), &prefs, 0);
        let s = &record.state;
        assert_eq!(
            s.unlocked_games,
            ["Wordle", "Geodle"],
            "non-strings dropped"
        );
        assert_eq!(
            s.played_breaks.len(),
            2,
            "an entry without a name is dropped"
        );
        assert!(s.played_breaks[0].played_at.is_some());
        assert!(
            s.played_breaks[1].played_at.is_none(),
            "an unparsable time is kept as unknown"
        );
        assert!(s.speedrunner_today, "truthy like production");
        assert_eq!(s.badge_counts.get("early-bird"), Some(&3));
        assert!(
            !s.badge_counts.contains_key("speedrunner"),
            "a non-number count is ignored"
        );
        assert_eq!(s.pet_rock_pats, 1234);
        assert_eq!(
            record.achievement_board.len(),
            1,
            "production's board filter"
        );
        assert_eq!(s.achievement_earned_on_dates.len(), 1);
        assert_eq!(s.durak.solved_count, 1);
        assert_eq!(s.durak.table[0].attack, "7♥");
        assert!(s.wordle.hard_mode);
        assert_eq!(
            s.flaggle.guesses.len(),
            1,
            "a guess without maskedFlagDataUrl is dropped like production"
        );
        assert_eq!(s.travle.start, "France");
        assert_eq!(record.rest_tree, 2, "6 % 4");
        assert!(
            record.unknown.is_empty(),
            "only the 21 keys are taken from AppState"
        );
    }

    #[test]
    fn write_then_parse_round_trips_and_keeps_unknown_keys() {
        let mut record =
            from_production_backup(production_state().as_object().unwrap(), &Map::new(), 3);
        assert_eq!(record.rest_tree, 3, "no preference keeps the current tree");
        record
            .unknown
            .insert("futureField".into(), json!({"keep": true}));
        let written = write_section(&record);
        assert_eq!(written["futureField"]["keep"], true);
        assert_eq!(
            written["flagglePuzzle"]["guesses"][0]["maskedFlagDataUrl"],
            ""
        );
        assert_eq!(
            written["playedBreaks"][0]["playedAt"],
            "2026-09-30T06:30:00.000Z"
        );
        assert_eq!(parse_section(Some(&written)), record);
        // idempotent
        assert_eq!(write_section(&parse_section(Some(&written))), written);
    }

    #[test]
    fn missing_or_garbage_sections_load_production_defaults() {
        let defaults = BreakRoomRecord::default();
        assert_eq!(parse_section(None), defaults);
        assert_eq!(parse_section(Some(&json!(42))), defaults);
        let odd = parse_section(Some(
            &json!({"petRockPats": -5, "totalUnlocks": "many", "durakPuzzle": [], "restTree": "abc"}),
        ));
        assert_eq!(odd.state.pet_rock_pats, 0);
        assert_eq!(odd.state.total_unlocks, 0);
        assert_eq!(odd.state.durak.phase, "player_attack");
        assert_eq!(odd.rest_tree, 0);
        assert!(
            js_truthy(Some(&json!("x")))
                && !js_truthy(Some(&json!("")))
                && !js_truthy(Some(&json!(0)))
        );
        assert!(js_truthy(Some(&json!([]))) && !js_truthy(None));
    }

    #[test]
    fn an_old_backup_without_break_room_keys_resets_like_restore_backup() {
        let record =
            from_production_backup(json!({"sessions": []}).as_object().unwrap(), &Map::new(), 1);
        assert_eq!(record.state, BreakRoomState::default());
        assert_eq!(record.rest_tree, 1);
    }

    #[test]
    fn the_file_port_keeps_every_other_section() {
        let dir = std::env::temp_dir().join(format!("st-break-port-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        std::fs::write(&path, br#"{"schema_version": 1, "preferences": {"appearance": {"style": "wabi-sabi"}}, "x": 1}"#).unwrap();
        let mut port = FileBreakRoomPort::new(NativeStore::new(path.clone()));
        assert_eq!(
            port.load(),
            BreakRoomRecord::default(),
            "a Stage 15-19 store has no section"
        );
        let mut record = BreakRoomRecord::default();
        record.state.pet_rock_pats = 5;
        port.persist(&record);
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["preferences"]["appearance"]["style"], "wabi-sabi");
        assert_eq!(raw["x"], 1);
        assert_eq!(raw["break_room"]["petRockPats"], 5);
        assert_eq!(port.load().state.pet_rock_pats, 5);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
