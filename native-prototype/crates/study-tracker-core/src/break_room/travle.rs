//! Daily Travle (Stage 21), ported from `desktop/src/lib/travle.ts` and its `App.tsx` handlers
//! (`initTravlePuzzle`, `submitTravleGuess`) and `storage.ts` (`normalizeTravlePuzzle`).
//!
//! The game: build a chain of land-bordering countries from a start to a destination in at most
//! seven guesses. Everything is a pure function of production's two tables, `COUNTRIES`
//! (`countries.rs`) and `COUNTRY_BORDERS` (`data/break_room/borders.tsv`, extracted verbatim, key
//! and neighbour order kept, because breadth-first search visits neighbours in that order and so
//! decides which of several equally short routes production shows).
//!
//! Identity: a [`CountryId`] is the country's position in `COUNTRIES` (canonically its ISO 3166
//! alpha-3 `code`). Production stores and compares display **names** (`start`, `target`,
//! `guesses`), so the public API takes and returns names exactly like `travle.ts`, and resolves
//! them through production's `normalizeCountryName` (no extra aliases).
//!
//! Production quirks kept on purpose (see `docs/stage21-travle.md`):
//! - The graph is not symmetric: Sri Lanka lists India, India does not list Sri Lanka. Every
//!   search walks outgoing edges, so Sri Lanka can start a route but never be reached.
//! - A guess is any country in `COUNTRIES`, including the 39 without land borders that the
//!   dropdown hides (typing "Australia" is accepted; it can only ever be off route).
//! - Names are matched after stripping everything outside `[a-z0-9]`, so "Türkiye" must be typed
//!   with its "ü" ("Turkiye" and "Turkey" are not countries).
//! - The guess-state "shortest length" is the number of countries on the shortest route, not the
//!   number of steps, which widens production's "could help" band by one.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::countries::{countries, normalize_country_name, Country};
use super::daily::{daily_index, puzzle_id};

/// `TRAVLE_MAX_GUESSES`.
pub const MAX_GUESSES: usize = 7;

const BORDERS: &str = include_str!("../../data/break_room/borders.tsv");

/// A country by its position in production's `COUNTRIES` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CountryId(u16);

impl CountryId {
    pub fn index(self) -> usize {
        usize::from(self.0)
    }

    pub fn country(self) -> &'static Country {
        &countries()[self.index()]
    }

    /// ISO alpha-3, production's graph and map key.
    pub fn code(self) -> &'static str {
        self.country().code
    }

    /// Production's display name (also the persisted form).
    pub fn name(self) -> &'static str {
        self.country().name
    }

    /// The country with this exact ISO alpha-3 code.
    pub fn from_code(code: &str) -> Option<Self> {
        ids().by_code.get(code).copied()
    }

    /// Every country, in production order.
    pub fn all() -> impl ExactSizeIterator<Item = CountryId> {
        (0..countries().len()).map(|i| CountryId(i as u16))
    }
}

struct Ids {
    by_code: HashMap<&'static str, CountryId>,
    /// normalized name -> first country with it (`COUNTRIES.find`)
    by_normalized_name: HashMap<String, CountryId>,
}

fn ids() -> &'static Ids {
    static IDS: OnceLock<Ids> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut by_code = HashMap::new();
        let mut by_normalized_name = HashMap::new();
        for id in CountryId::all() {
            by_code.entry(id.code()).or_insert(id);
            by_normalized_name
                .entry(normalize_country_name(id.name()))
                .or_insert(id);
        }
        Ids {
            by_code,
            by_normalized_name,
        }
    })
}

/// `findTravleCountry`: the country whose normalized name equals the normalized input, among
/// *all* countries (bordered or not).
pub fn find_travle_country(value: &str) -> Option<CountryId> {
    ids()
        .by_normalized_name
        .get(&normalize_country_name(value))
        .copied()
}

// ------------------------------------------------------------------------------- border graph

/// A malformed `borders.tsv` (checked by tests; the bundled table is valid).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    BadLine(usize),
    UnknownCountry { line: usize, code: String },
    SelfEdge(String),
    DuplicateEdge { from: String, to: String },
    DuplicateCountry(String),
}

/// `COUNTRY_BORDERS` as adjacency lists over [`CountryId`], neighbour order kept. A country
/// production lists without borders (or not at all) has an empty list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorderGraph {
    neighbours: Vec<Vec<CountryId>>,
    /// countries in `COUNTRY_BORDERS` key order
    listed: Vec<CountryId>,
}

impl BorderGraph {
    pub fn parse(text: &str) -> Result<Self, GraphError> {
        let mut neighbours = vec![Vec::new(); countries().len()];
        let mut listed = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line_no = n + 1;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (code, rest) = line.split_once('\t').ok_or(GraphError::BadLine(line_no))?;
            let unknown = |c: &str| GraphError::UnknownCountry {
                line: line_no,
                code: c.to_string(),
            };
            let from = CountryId::from_code(code).ok_or_else(|| unknown(code))?;
            if listed.contains(&from) {
                return Err(GraphError::DuplicateCountry(code.to_string()));
            }
            listed.push(from);
            let list: &mut Vec<CountryId> = &mut neighbours[from.index()];
            for to_code in rest.split(' ').filter(|s| !s.is_empty()) {
                let to = CountryId::from_code(to_code).ok_or_else(|| unknown(to_code))?;
                if to == from {
                    return Err(GraphError::SelfEdge(code.to_string()));
                }
                if list.contains(&to) {
                    return Err(GraphError::DuplicateEdge {
                        from: code.to_string(),
                        to: to_code.to_string(),
                    });
                }
                list.push(to);
            }
        }
        Ok(Self { neighbours, listed })
    }

    /// `COUNTRY_BORDERS[code] ?? []`.
    pub fn neighbours(&self, id: CountryId) -> &[CountryId] {
        &self.neighbours[id.index()]
    }

    /// Countries in `COUNTRY_BORDERS` key order.
    pub fn listed(&self) -> &[CountryId] {
        &self.listed
    }

    /// Directed edges whose reverse is missing (production: only Sri Lanka -> India).
    pub fn one_way_edges(&self) -> Vec<(CountryId, CountryId)> {
        let mut out = Vec::new();
        for from in CountryId::all() {
            for &to in self.neighbours(from) {
                if !self.neighbours(to).contains(&from) {
                    out.push((from, to));
                }
            }
        }
        out
    }

    /// Breadth-first search from `start` along outgoing edges, visiting neighbours in production
    /// order and marking a country seen when it is queued: exactly the route production's
    /// path-array queue returns. `allowed` restricts the countries that may be entered (the start
    /// is always allowed). Empty when `target` cannot be reached.
    fn path(
        &self,
        start: CountryId,
        target: CountryId,
        allowed: Option<&dyn Fn(CountryId) -> bool>,
    ) -> Vec<CountryId> {
        let parents = self.search(start, Some(target), allowed);
        route_from_tree(&parents, start, target)
    }

    /// The breadth-first parent tree from `start` (stopping once `stop` is dequeued): `parents[i]`
    /// is the country `i` was first reached from (`Some(start)`'s own entry is itself).
    fn search(
        &self,
        start: CountryId,
        stop: Option<CountryId>,
        allowed: Option<&dyn Fn(CountryId) -> bool>,
    ) -> Vec<Option<CountryId>> {
        let mut parents: Vec<Option<CountryId>> = vec![None; self.neighbours.len()];
        parents[start.index()] = Some(start);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(current) = queue.pop_front() {
            if Some(current) == stop {
                break;
            }
            for &next in self.neighbours(current) {
                if allowed.is_some_and(|ok| !ok(next)) || parents[next.index()].is_some() {
                    continue;
                }
                parents[next.index()] = Some(current);
                queue.push_back(next);
            }
        }
        parents
    }

    /// The full breadth-first parent tree from `start` (every route production returns from it).
    fn search_tree(&self, start: CountryId) -> Vec<Option<CountryId>> {
        self.search(start, None, None)
    }

    /// `distanceMapFrom`: steps from `from` along outgoing edges (`None` = unreachable).
    pub fn distances_from(&self, from: CountryId) -> Vec<Option<u32>> {
        let mut distance = vec![None; self.neighbours.len()];
        distance[from.index()] = Some(0);
        let mut queue = std::collections::VecDeque::from([from]);
        while let Some(current) = queue.pop_front() {
            let d = distance[current.index()].unwrap_or(0);
            for &next in self.neighbours(current) {
                if distance[next.index()].is_none() {
                    distance[next.index()] = Some(d + 1);
                    queue.push_back(next);
                }
            }
        }
        distance
    }

    /// `getTravleShortestPath` on ids.
    pub fn shortest_path(&self, start: CountryId, target: CountryId) -> Vec<CountryId> {
        self.path(start, target, None)
    }
}

/// The route to `target` in a parent tree from `start` (empty when it was never reached).
fn route_from_tree(
    parents: &[Option<CountryId>],
    start: CountryId,
    target: CountryId,
) -> Vec<CountryId> {
    if parents[target.index()].is_none() {
        return Vec::new();
    }
    let mut route = vec![target];
    let mut at = target;
    while at != start {
        at = parents[at.index()].expect("a reached country has a parent");
        route.push(at);
    }
    route.reverse();
    route
}

/// Production's border graph, parsed once on first use (never at startup).
pub fn border_graph() -> &'static BorderGraph {
    static GRAPH: OnceLock<BorderGraph> = OnceLock::new();
    GRAPH.get_or_init(|| BorderGraph::parse(BORDERS).expect("bundled borders.tsv is valid"))
}

/// Whether production offers the country in the dropdown and the daily pairs (it has borders).
pub fn is_playable(id: CountryId) -> bool {
    !border_graph().neighbours(id).is_empty()
}

/// `filterTravleCountries`: countries with borders whose normalized name contains the normalized
/// query (all of them for an empty query), in production order.
pub fn filter_travle_countries(query: &str) -> Vec<&'static Country> {
    let normalized = normalize_country_name(query);
    CountryId::all()
        .filter(|&id| is_playable(id))
        .map(CountryId::country)
        .filter(|c| normalized.is_empty() || normalize_country_name(c.name).contains(&normalized))
        .collect()
}

/// `TRAVLE_COUNTRY_COUNT`: countries with at least one border.
pub fn country_count() -> usize {
    CountryId::all().filter(|&id| is_playable(id)).count()
}

/// `getTravleNeighbors`: the neighbours' names, sorted (`Array.prototype.sort`, UTF-16 order).
pub fn neighbours_of(country_name: &str) -> Vec<String> {
    let Some(id) = find_travle_country(country_name) else {
        return Vec::new();
    };
    let mut names: Vec<String> = border_graph()
        .neighbours(id)
        .iter()
        .map(|n| n.name().to_string())
        .collect();
    names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    names
}

/// `areTravleNeighbors`: `b` is in `a`'s list (directed, like production).
pub fn are_neighbours(a: &str, b: &str) -> bool {
    match (find_travle_country(a), find_travle_country(b)) {
        (Some(a), Some(b)) => border_graph().neighbours(a).contains(&b),
        _ => false,
    }
}

fn names(route: &[CountryId]) -> Vec<String> {
    route.iter().map(|id| id.name().to_string()).collect()
}

/// `getTravleShortestPath` (names in, names out; empty when either name is unknown or there is
/// no route).
pub fn shortest_path(start: &str, target: &str) -> Vec<String> {
    match (find_travle_country(start), find_travle_country(target)) {
        (Some(s), Some(t)) => names(&border_graph().shortest_path(s, t)),
        _ => Vec::new(),
    }
}

// ----------------------------------------------------------------------------- route semantics

/// `TravleGuessState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuessState {
    /// "route": on the solved, displayed or shortest route
    Route,
    /// "possible": within two steps of a shortest route
    Possible,
    /// "miss"
    Miss,
}

/// A puzzle's start, destination and guesses resolved to ids once, with every route query
/// production derives from them. Unknown names (only possible in imported or corrupt state)
/// resolve to nothing and are skipped, exactly like production's `countryCode(...) || ""`.
#[derive(Debug, Clone)]
pub struct Routes {
    pub start: Option<CountryId>,
    pub target: Option<CountryId>,
    /// `guesses` that are countries, in order
    pub guesses: Vec<CountryId>,
}

impl Routes {
    pub fn new(start: &str, target: &str, guesses: &[String]) -> Self {
        Self {
            start: find_travle_country(start),
            target: find_travle_country(target),
            guesses: guesses
                .iter()
                .filter_map(|g| find_travle_country(g))
                .collect(),
        }
    }

    fn ends(&self) -> Option<(CountryId, CountryId)> {
        Some((self.start?, self.target?))
    }

    /// `getTravleShortestPath(start, target)`.
    pub fn shortest(&self) -> Vec<CountryId> {
        self.ends()
            .map(|(s, t)| border_graph().shortest_path(s, t))
            .unwrap_or_default()
    }

    /// `getTravleSolvedPath`: the shortest route through the start, the destination and the
    /// guessed countries only; empty while they do not connect.
    pub fn solved(&self) -> Vec<CountryId> {
        let Some((s, t)) = self.ends() else {
            return Vec::new();
        };
        let allowed = |id: CountryId| id == s || id == t || self.guesses.contains(&id);
        border_graph().path(s, t, Some(&allowed))
    }

    /// `isTravleRouteSolved`.
    pub fn is_solved(&self) -> bool {
        !self.solved().is_empty()
    }

    /// `getTravleDisplayPath`: the solved route, or else the longest simple chain from the start
    /// through guessed countries (ties: the one ending closest to the destination; first found
    /// wins a full tie). Production enumerates the chains breadth-first with path arrays; the
    /// set is at most the start plus seven guesses, so this stays small.
    pub fn display(&self) -> Vec<CountryId> {
        let solved = self.solved();
        if !solved.is_empty() {
            return solved;
        }
        let Some((start, target)) = self.ends() else {
            return Vec::new();
        };
        let graph = border_graph();
        let allowed = |id: CountryId| id == start || self.guesses.contains(&id);
        let to_target = graph.distances_from(target);
        let distance = |id: CountryId| to_target[id.index()].map_or(u64::MAX, u64::from);
        let mut queue = std::collections::VecDeque::from([vec![start]]);
        let mut best = vec![start];
        while let Some(path) = queue.pop_front() {
            let current = *path.last().expect("paths are never empty");
            let best_distance = distance(*best.last().expect("never empty"));
            let current_distance = distance(current);
            if path.len() > best.len()
                || (path.len() == best.len() && current_distance < best_distance)
            {
                best = path.clone();
            }
            for &next in graph.neighbours(current) {
                if !allowed(next) || path.contains(&next) {
                    continue;
                }
                let mut longer = path.clone();
                longer.push(next);
                queue.push_back(longer);
            }
        }
        best
    }

    /// `getTravleGuessStates`, keyed by country (production keys the same thing by name).
    pub fn guess_states(&self) -> Vec<(CountryId, GuessState)> {
        let Some((start, target)) = self.ends() else {
            return Vec::new();
        };
        let graph = border_graph();
        let solved = self.solved();
        let display = self.display();
        let answer = self.shortest();
        // `answerPathCodes.size`: countries on the shortest route (a route never repeats one)
        let shortest_length = answer.len() as u64;
        let from_start = graph.distances_from(start);
        let from_target = graph.distances_from(target);
        let mut states: Vec<(CountryId, GuessState)> = Vec::new();
        for &id in &self.guesses {
            let state = if solved.contains(&id) || display.contains(&id) || answer.contains(&id) {
                GuessState::Route
            } else {
                match (from_start[id.index()], from_target[id.index()]) {
                    (Some(a), Some(b)) if u64::from(a) + u64::from(b) <= shortest_length + 2 => {
                        GuessState::Possible
                    }
                    _ => GuessState::Miss,
                }
            };
            match states.iter_mut().find(|(c, _)| *c == id) {
                Some(slot) => slot.1 = state,
                None => states.push((id, state)),
            }
        }
        states
    }
}

// --------------------------------------------------------------------------------- daily puzzle

/// `TravleDailyPuzzle` (by id; `start.name()`/`target.name()` are what production stores).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyPuzzle {
    pub start: CountryId,
    pub target: CountryId,
    pub shortest_path: Vec<CountryId>,
}

struct DailyPairs {
    pairs: Vec<DailyPuzzle>,
    /// "<start name>:<target name>", the middle of production's per-pair sort key
    keys: Vec<String>,
}

/// `buildDailyPairs`: every ordered pair of bordered countries (codes sorted) whose shortest
/// route needs 3 to 5 guesses; built once on first use, like production's `dailyPairsCache`.
fn daily_pairs() -> &'static DailyPairs {
    static PAIRS: OnceLock<DailyPairs> = OnceLock::new();
    PAIRS.get_or_init(|| {
        let graph = border_graph();
        let mut codes: Vec<CountryId> = CountryId::all().filter(|&id| is_playable(id)).collect();
        codes.sort_by(|a, b| a.code().encode_utf16().cmp(b.code().encode_utf16()));
        let mut pairs = Vec::new();
        for &start in &codes {
            // One full search per start: production's early-exit search for each target sets the
            // same parents before it reaches that target, so every route is the one it returns.
            let parents = graph.search_tree(start);
            let steps = graph.distances_from(start);
            for &target in &codes {
                // `guessesNeeded = path.length - 1`: the route's steps (no route: never kept)
                if start == target || !steps[target.index()].is_some_and(|n| (3..=5).contains(&n)) {
                    continue;
                }
                pairs.push(DailyPuzzle {
                    start,
                    target,
                    shortest_path: route_from_tree(&parents, start, target),
                });
            }
        }
        let keys = pairs
            .iter()
            .map(|p| format!("{}:{}", p.start.name(), p.target.name()))
            .collect();
        DailyPairs { pairs, keys }
    })
}

/// The number of candidate daily pairs.
pub fn daily_pair_count() -> usize {
    daily_pairs().pairs.len()
}

/// `getTravlePuzzleForDate`: the pairs ordered by `hashString("<salt>:<start>:<target>:<index>")`
/// (stable), position `daysSinceFirstPuzzle(date) % count`.
pub fn puzzle_for_date(date: &str, seed_salt: &str) -> &'static DailyPuzzle {
    let all = daily_pairs();
    &all.pairs[daily_index(all.keys.iter().map(String::as_str), seed_salt, date)]
}

/// `getTravlePuzzleId`.
pub fn travle_puzzle_id(date: &str, seed_salt: &str) -> String {
    puzzle_id(date, seed_salt)
}

// ------------------------------------------------------------------------------- puzzle state

/// `TravlePuzzleState`, the persisted `travlePuzzle`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TravlePuzzle {
    pub seed_salt: String,
    pub active_date: String,
    pub puzzle_id: String,
    pub start: String,
    pub target: String,
    pub guesses: Vec<String>,
    pub completed: bool,
    pub won: bool,
}

/// `submitTravleGuess` outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TravleSubmit {
    /// Not a country: production shows the message and opens the dropdown.
    NotACountry,
    /// The start or an earlier guess (state unchanged).
    AlreadyInRoute,
    /// Appended; the message production shows.
    Accepted { message: String },
    /// The puzzle is finished.
    Ignored,
}

impl TravleSubmit {
    pub fn message(&self) -> &str {
        match self {
            TravleSubmit::NotACountry => "Select a country from the list.",
            TravleSubmit::AlreadyInRoute => "That country is already in your route.",
            TravleSubmit::Accepted { message } => message,
            TravleSubmit::Ignored => "",
        }
    }
}

fn joined(route: &[CountryId]) -> String {
    route
        .iter()
        .map(|id| id.name())
        .collect::<Vec<_>>()
        .join(" -> ")
}

impl TravlePuzzle {
    /// A fresh puzzle for `date` (`makeDefaultTravlePuzzle` / the reset in `initTravlePuzzle`).
    pub fn fresh(date: &str, seed_salt: &str) -> Self {
        let puzzle = puzzle_for_date(date, seed_salt);
        Self {
            seed_salt: seed_salt.to_string(),
            active_date: date.to_string(),
            puzzle_id: travle_puzzle_id(date, seed_salt),
            start: puzzle.start.name().to_string(),
            target: puzzle.target.name().to_string(),
            guesses: Vec::new(),
            completed: false,
            won: false,
        }
    }

    /// `normalizeTravlePuzzle` (load time): keeps today's puzzle as stored (names unchecked,
    /// guesses capped at seven), replaces any other day's with today's.
    pub fn normalized(self, today: &str, new_salt: impl FnOnce() -> String) -> Self {
        let seed_salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt
        };
        let puzzle_id = travle_puzzle_id(today, &seed_salt);
        let current = self.active_date == today && self.puzzle_id == puzzle_id;
        let daily = || puzzle_for_date(today, &seed_salt);
        let start = if current && !self.start.is_empty() {
            self.start
        } else {
            daily().start.name().to_string()
        };
        let target = if current && !self.target.is_empty() {
            self.target
        } else {
            daily().target.name().to_string()
        };
        Self {
            guesses: if current {
                self.guesses.into_iter().take(MAX_GUESSES).collect()
            } else {
                Vec::new()
            },
            completed: current && self.completed,
            won: current && self.won,
            active_date: today.to_string(),
            puzzle_id,
            seed_salt,
            start,
            target,
        }
    }

    /// `travlePuzzleIsToday` (the card's Solved/Failed badge).
    pub fn is_today(&self, today: &str) -> bool {
        !self.seed_salt.is_empty()
            && self.active_date == today
            && self.puzzle_id == travle_puzzle_id(today, &self.seed_salt)
    }

    /// `initTravlePuzzle` (opening the game): nothing if today's puzzle is in place with a start
    /// and a destination, otherwise a fresh one. Returns whether it reset.
    pub fn ensure_today(&mut self, today: &str, new_salt: impl FnOnce() -> String) -> bool {
        let seed_salt = if self.seed_salt.is_empty() {
            new_salt()
        } else {
            self.seed_salt.clone()
        };
        let id = travle_puzzle_id(today, &seed_salt);
        if self.active_date == today
            && self.puzzle_id == id
            && !self.start.is_empty()
            && !self.target.is_empty()
        {
            return false;
        }
        *self = Self::fresh(today, &seed_salt);
        true
    }

    pub fn routes(&self) -> Routes {
        Routes::new(&self.start, &self.target, &self.guesses)
    }

    /// `travleRoute`: the start, then every guess.
    pub fn route(&self) -> Vec<&str> {
        std::iter::once(self.start.as_str())
            .chain(self.guesses.iter().map(String::as_str))
            .collect()
    }

    /// `travleCurrent`: the latest guess, else the start.
    pub fn current(&self) -> &str {
        self.guesses
            .last()
            .map_or(self.start.as_str(), String::as_str)
    }

    /// `travleGuessesLeft`.
    pub fn guesses_left(&self) -> usize {
        MAX_GUESSES.saturating_sub(self.guesses.len())
    }

    /// `submitTravleGuess` for the typed draft.
    pub fn submit(&mut self, draft: &str) -> TravleSubmit {
        if self.completed {
            return TravleSubmit::Ignored;
        }
        let Some(country) = find_travle_country(draft) else {
            return TravleSubmit::NotACountry;
        };
        let name = country.name();
        if name == self.start || self.guesses.iter().any(|g| g == name) {
            return TravleSubmit::AlreadyInRoute;
        }
        self.guesses.push(name.to_string());
        let routes = self.routes();
        let solved = routes.solved();
        let won = !solved.is_empty();
        let message = if won {
            format!("Route complete: {}.", joined(&solved))
        } else if self.guesses.len() >= MAX_GUESSES {
            format!(
                "Route closed. Shortest path: {}.",
                joined(&routes.shortest())
            )
        } else {
            let state = routes
                .guess_states()
                .into_iter()
                .find(|(id, _)| *id == country)
                .map(|(_, s)| s);
            match state {
                Some(GuessState::Route) => format!("{name} is on the route."),
                Some(GuessState::Possible) => format!("{name} could help connect the route."),
                _ => format!("{name} is off route."),
            }
        };
        self.won = won;
        self.completed = won || self.guesses.len() >= MAX_GUESSES;
        TravleSubmit::Accepted { message }
    }
}

// ------------------------------------------------------------------------------ map semantics

/// What the map shows for one country: production's `travle-map-country` class list, minus any
/// colour (that is the renderer's business).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MapRole {
    /// `possible`: a guessed bordered country that could help
    pub possible: bool,
    /// `miss`
    pub miss: bool,
    /// `route`: a guessed bordered country on a route, or any country of the solved route
    pub route: bool,
    /// `start`: the first country of the route
    pub start: bool,
    /// `target`
    pub target: bool,
    /// `current`: the latest guess (else the start)
    pub current: bool,
}

impl MapRole {
    pub fn is_plain(&self) -> bool {
        *self == MapRole::default()
    }
}

/// The map classes of every country that has one (the rest are plain), in production order.
///
/// Production looks a country's guess state up by its name in the *dropdown's* table, so a
/// guessed country without borders gets no route/possible/miss class: only `current` while it is
/// the latest guess.
pub fn map_roles(puzzle: &TravlePuzzle) -> Vec<(CountryId, MapRole)> {
    let routes = puzzle.routes();
    let route_ids: Vec<CountryId> = puzzle
        .route()
        .iter()
        .filter_map(|n| find_travle_country(n))
        .collect();
    let target = find_travle_country(&puzzle.target);
    let current = find_travle_country(puzzle.current());
    let solved = routes.solved();
    let states = routes.guess_states();
    let mut out = Vec::new();
    for id in CountryId::all() {
        let in_route = route_ids.contains(&id);
        let state = is_playable(id)
            .then(|| states.iter().find(|(c, _)| *c == id).map(|(_, s)| *s))
            .flatten();
        let role = MapRole {
            possible: in_route && state == Some(GuessState::Possible),
            miss: in_route && state == Some(GuessState::Miss),
            route: (in_route && state == Some(GuessState::Route)) || solved.contains(&id),
            start: route_ids.first() == Some(&id),
            target: target == Some(id),
            current: current == Some(id),
        };
        if !role.is_plain() {
            out.push((id, role));
        }
    }
    out
}

/// The countries the map's route line joins (`travleDisplayPathCodes`).
pub fn display_route(puzzle: &TravlePuzzle) -> Vec<CountryId> {
    puzzle.routes().display()
}
