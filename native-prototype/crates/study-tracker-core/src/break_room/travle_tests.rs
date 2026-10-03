//! Travle domain tests. The `golden_*` tests compare against fixtures computed by production's own
//! code (`scripts/stage21-goldens.mjs`: `desktop/src/lib/travle.ts` imported as-is, and the
//! `App.tsx` handlers cut out of the source text and run verbatim); the rest pin graph invariants.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::countries::countries;
use super::daily::puzzle_id;
use super::travle::*;

fn fixture() -> &'static Value {
    static F: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        serde_json::from_str(include_str!("../../tests/fixtures/break_room/travle.json")).unwrap()
    })
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap()
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| s(x).to_string())
        .collect()
}

/// Production's `codes()` helper: names joined by ">" as codes, unknown names as `?name`.
fn codes(route: &[CountryId]) -> String {
    route
        .iter()
        .map(|id| id.code())
        .collect::<Vec<_>>()
        .join(">")
}

fn name_codes(names: &[String]) -> String {
    names
        .iter()
        .map(|n| {
            find_travle_country(n)
                .filter(|id| id.name() == n)
                .map_or(format!("?{n}"), |id| id.code().to_string())
        })
        .collect::<Vec<_>>()
        .join(">")
}

fn state_letter(state: GuessState) -> &'static str {
    match state {
        GuessState::Route => "route",
        GuessState::Possible => "possible",
        GuessState::Miss => "miss",
    }
}

fn states_map(states: &[(CountryId, GuessState)]) -> BTreeMap<String, String> {
    states
        .iter()
        .map(|(id, st)| (id.name().to_string(), state_letter(*st).to_string()))
        .collect()
}

fn fixture_states(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), s(v).to_string()))
        .collect()
}

// ------------------------------------------------------------------------------- invariants

#[test]
fn graph_matches_production_counts_and_invariants() {
    let g = border_graph();
    let meta = &fixture()["meta"];
    assert_eq!(
        country_count(),
        meta["countryCount"].as_u64().unwrap() as usize
    );
    assert_eq!(country_count(), 156);
    assert_eq!(MAX_GUESSES, meta["maxGuesses"].as_u64().unwrap() as usize);
    assert_eq!(g.listed().len(), 156, "every key of COUNTRY_BORDERS");
    // every listed country has at least one neighbour (playable == listed)
    for &id in g.listed() {
        assert!(!g.neighbours(id).is_empty(), "{}", id.code());
    }
    // isolated countries are exactly production's
    let isolated: Vec<String> = CountryId::all()
        .filter(|&id| !is_playable(id))
        .map(|id| id.name().to_string())
        .collect();
    assert_eq!(isolated, strs(&meta["isolated"]));
    // no self-edges, no duplicates (parse rejects them), every target a known country (typed ids)
    for id in CountryId::all() {
        let n = g.neighbours(id);
        assert!(!n.contains(&id));
        assert_eq!(n.iter().collect::<BTreeSet<_>>().len(), n.len());
    }
    // production's one intentional-or-not asymmetry, encoded as an exception
    let one_way: Vec<(&str, &str)> = g
        .one_way_edges()
        .into_iter()
        .map(|(a, b)| (a.code(), b.code()))
        .collect();
    assert_eq!(one_way, [("LKA", "IND")]);
    // canonical ids: codes and names unique, every country resolvable both ways
    let mut seen_codes = BTreeSet::new();
    let mut seen_names = BTreeSet::new();
    for id in CountryId::all() {
        assert!(seen_codes.insert(id.code()));
        assert!(seen_names.insert(id.name()));
        assert_eq!(CountryId::from_code(id.code()), Some(id));
        assert_eq!(find_travle_country(id.name()), Some(id));
        assert!(!id.country().region.is_empty(), "dropdown detail");
    }
    assert_eq!(CountryId::all().len(), countries().len());
}

#[test]
fn graph_connectivity_is_what_production_ships() {
    // Weakly connected components of the playable graph: Afro-Eurasia (130), the Americas (22),
    // Haiti/Dominican Republic and the United Kingdom/Ireland. Recorded, not "fixed": a daily pair
    // never crosses components (no route), so these pairs only ever meet each other.
    let g = border_graph();
    let mut component: BTreeMap<CountryId, usize> = BTreeMap::new();
    let mut sizes = Vec::new();
    for &root in g.listed() {
        if component.contains_key(&root) {
            continue;
        }
        let mut stack = vec![root];
        let mut size = 0;
        component.insert(root, sizes.len());
        while let Some(at) = stack.pop() {
            size += 1;
            let reverse = g
                .listed()
                .iter()
                .copied()
                .filter(|&o| g.neighbours(o).contains(&at));
            let next: Vec<CountryId> = g.neighbours(at).iter().copied().chain(reverse).collect();
            for n in next {
                if let std::collections::btree_map::Entry::Vacant(e) = component.entry(n) {
                    e.insert(sizes.len());
                    stack.push(n);
                }
            }
        }
        sizes.push(size);
    }
    sizes.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(sizes.iter().sum::<usize>(), 156);
    assert_eq!(sizes, [130, 22, 2, 2]);
    let pair = |a: &str, b: &str| {
        component[&CountryId::from_code(a).unwrap()] == component[&CountryId::from_code(b).unwrap()]
    };
    assert!(pair("HTI", "DOM") && pair("GBR", "IRL") && pair("USA", "BRA") && pair("FRA", "ZAF"));
    assert!(!pair("FRA", "USA") && !pair("GBR", "FRA"));
}

#[test]
fn malformed_border_tables_are_rejected() {
    assert_eq!(
        BorderGraph::parse("FRA\tFRA").unwrap_err(),
        GraphError::SelfEdge("FRA".into())
    );
    assert!(matches!(
        BorderGraph::parse("FRA\tESP ESP").unwrap_err(),
        GraphError::DuplicateEdge { .. }
    ));
    assert!(matches!(
        BorderGraph::parse("FRA\tXXX").unwrap_err(),
        GraphError::UnknownCountry { line: 1, .. }
    ));
    assert!(matches!(
        BorderGraph::parse("# c\nZZZ\tFRA").unwrap_err(),
        GraphError::UnknownCountry { line: 2, .. }
    ));
    assert_eq!(
        BorderGraph::parse("FRA ESP").unwrap_err(),
        GraphError::BadLine(1)
    );
    assert_eq!(
        BorderGraph::parse("FRA\tESP\nFRA\tDEU").unwrap_err(),
        GraphError::DuplicateCountry("FRA".into())
    );
    let ok = BorderGraph::parse("FRA\tESP DEU\nESP\tFRA").unwrap();
    let fra = CountryId::from_code("FRA").unwrap();
    assert_eq!(
        ok.neighbours(fra)
            .iter()
            .map(|c| c.code())
            .collect::<Vec<_>>(),
        ["ESP", "DEU"],
        "neighbour order kept"
    );
}

// ----------------------------------------------------------------------------------- golden

#[test]
fn golden_daily_puzzles_match_production() {
    let rows = fixture()["daily"].as_array().unwrap();
    assert!(rows.len() >= 700);
    for row in rows {
        let (date, salt) = (s(&row["date"]), s(&row["salt"]));
        let p = puzzle_for_date(date, salt);
        assert_eq!(p.start.name(), s(&row["start"]), "{date} {salt}");
        assert_eq!(p.target.name(), s(&row["target"]), "{date} {salt}");
        assert_eq!(codes(&p.shortest_path), s(&row["path"]), "{date} {salt}");
        assert_eq!(travle_puzzle_id(date, salt), s(&row["id"]));
        assert_eq!(travle_puzzle_id(date, salt), puzzle_id(date, salt));
        let fresh = TravlePuzzle::fresh(date, salt);
        assert_eq!(
            (fresh.start.as_str(), fresh.target.as_str()),
            (s(&row["start"]), s(&row["target"]))
        );
        assert!((4..=6).contains(&p.shortest_path.len()), "3 to 5 guesses");
    }
}

#[test]
fn golden_local_day_drives_the_puzzle_in_every_timezone() {
    // production feeds its *local* calendar day; the native clock does the same (the date is
    // injected), so only the date -> puzzle step is the domain's
    for row in fixture()["tzProbes"].as_array().unwrap() {
        let p = puzzle_for_date(s(&row["today"]), "fixture-travle-salt");
        assert_eq!(p.start.name(), s(&row["start"]), "{}", s(&row["tz"]));
        assert_eq!(p.target.name(), s(&row["target"]), "{}", s(&row["tz"]));
    }
}

#[test]
fn golden_country_lookup_and_normalization_match_production() {
    let rows = fixture()["find"].as_array().unwrap();
    assert!(rows.len() > 400);
    for row in rows {
        let got = find_travle_country(s(&row["input"])).map(CountryId::name);
        assert_eq!(got, row["name"].as_str(), "{:?}", s(&row["input"]));
    }
    // the production spellings that are *not* aliases
    assert_eq!(find_travle_country("Turkey"), None);
    assert_eq!(find_travle_country("Turkiye"), None);
    assert_eq!(
        find_travle_country("Türkiye").map(CountryId::code),
        Some("TUR")
    );
    for row in fixture()["filter"].as_array().unwrap() {
        let got: Vec<&str> = filter_travle_countries(s(&row["query"]))
            .iter()
            .map(|c| c.code)
            .collect();
        assert_eq!(got.join(" "), s(&row["names"]), "{:?}", s(&row["query"]));
    }
}

#[test]
fn golden_adjacency_matches_production() {
    for row in fixture()["neighbours"].as_array().unwrap() {
        assert_eq!(neighbours_of(s(&row["name"])), strs(&row["neighbours"]));
    }
    for row in fixture()["neighbourPairs"].as_array().unwrap() {
        let r = row.as_array().unwrap();
        assert_eq!(
            are_neighbours(s(&r[0]), s(&r[1])),
            r[2].as_bool().unwrap(),
            "{r:?}"
        );
    }
}

#[test]
fn golden_shortest_paths_match_production() {
    let rows = fixture()["paths"].as_array().unwrap();
    assert!(rows.len() >= 4000);
    for row in rows {
        let r = row.as_array().unwrap();
        let (a, b) = (s(&r[0]), s(&r[1]));
        let name = |code: &str| {
            CountryId::from_code(code).map_or(code.to_string(), |id| id.name().to_string())
        };
        let got = shortest_path(&name(a), &name(b));
        assert_eq!(name_codes(&got), s(&r[2]), "{a} -> {b}");
    }
}

#[test]
fn golden_route_queries_match_production() {
    for row in fixture()["routeQueries"].as_array().unwrap() {
        let routes = Routes::new(s(&row["start"]), s(&row["target"]), &strs(&row["guesses"]));
        let ctx = format!(
            "{} -> {} via {:?}",
            s(&row["start"]),
            s(&row["target"]),
            strs(&row["guesses"])
        );
        assert_eq!(codes(&routes.solved()), s(&row["solved"]), "solved {ctx}");
        assert_eq!(
            codes(&routes.display()),
            s(&row["display"]),
            "display {ctx}"
        );
        assert_eq!(
            states_map(&routes.guess_states()),
            fixture_states(&row["states"]),
            "states {ctx}"
        );
        assert_eq!(routes.is_solved(), row["isSolved"].as_bool().unwrap());
    }
}

#[test]
fn golden_games_follow_production_handlers() {
    let games = fixture()["games"].as_array().unwrap();
    let submits: usize = games
        .iter()
        .map(|g| g["steps"].as_array().unwrap().len())
        .sum();
    assert!(submits > 1000);
    for game in games {
        let (start, target) = (s(&game["start"]), s(&game["target"]));
        let mut p = TravlePuzzle {
            seed_salt: "game".into(),
            active_date: "2026-10-02".into(),
            puzzle_id: travle_puzzle_id("2026-10-02", "game"),
            start: start.into(),
            target: target.into(),
            ..TravlePuzzle::default()
        };
        // `initTravlePuzzle` keeps today's puzzle
        assert!(!p.ensure_today("2026-10-02", || unreachable!()));
        for step in game["steps"].as_array().unwrap() {
            let draft = s(&step["draft"]);
            let before = p.clone();
            let result = p.submit(draft);
            let ctx = format!("{start} -> {target}, {draft:?} after {:?}", before.guesses);
            // the shown message: production keeps the old one on Ignored (setTravleMessage not called)
            if result != TravleSubmit::Ignored {
                assert_eq!(result.message(), s(&step["message"]), "{ctx}");
            }
            assert_eq!(p.guesses, strs(&step["guesses"]), "{ctx}");
            assert_eq!(p.completed, step["completed"].as_bool().unwrap(), "{ctx}");
            assert_eq!(p.won, step["won"].as_bool().unwrap(), "{ctx}");
            // the draft clears and the dropdown closes only on an accepted guess; a non-country
            // opens it; a repeat leaves both
            match &result {
                TravleSubmit::Accepted { .. } => {
                    assert_eq!(s(&step["draftAfter"]), "");
                    assert!(!step["dropdown"].as_bool().unwrap());
                }
                TravleSubmit::NotACountry => assert!(step["dropdown"].as_bool().unwrap()),
                TravleSubmit::AlreadyInRoute | TravleSubmit::Ignored => {
                    assert_eq!(s(&step["draftAfter"]), draft);
                    assert_eq!(p, before, "no state change, so nothing to persist");
                }
            }
        }
        let routes = p.routes();
        let f = &game["final"];
        assert_eq!(codes(&routes.shortest()), s(&f["shortest"]));
        assert_eq!(codes(&routes.solved()), s(&f["solved"]));
        assert_eq!(codes(&routes.display()), s(&f["display"]));
        assert_eq!(
            states_map(&routes.guess_states()),
            fixture_states(&f["states"])
        );
    }
}

fn puzzle_from(v: &Value) -> TravlePuzzle {
    let o = v.as_object().unwrap();
    let st = |k: &str| o.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    TravlePuzzle {
        seed_salt: st("seedSalt"),
        active_date: st("activeDate"),
        puzzle_id: st("puzzleId"),
        start: st("start"),
        target: st("target"),
        guesses: o["guesses"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        completed: o["completed"].as_bool().unwrap(),
        won: o["won"].as_bool().unwrap(),
    }
}

#[test]
fn golden_open_time_init_and_rollover_match_production() {
    for row in fixture()["init"].as_array().unwrap() {
        let mut p = puzzle_from(&row["before"]);
        let reset = p.ensure_today(s(&row["today"]), || "drawn-salt".into());
        assert_eq!(p, puzzle_from(&row["after"]), "{}", s(&row["name"]));
        // production clears draft/message/dropdown and the zoom only when it resets
        assert_eq!(reset, s(&row["draft"]).is_empty(), "{}", s(&row["name"]));
        assert_eq!(reset, row["zoom"].as_f64().unwrap() == 1.0);
    }
}

#[test]
fn golden_well_formed_load_normalization_matches_production() {
    // the shapes a native store can hold (typed fields); malformed JSON shapes are covered by the
    // app crate's parser tests against the same fixture rows
    for row in fixture()["normalize"].as_array().unwrap() {
        let name = s(&row["name"]);
        let Some(stored) = row["stored"].as_object() else {
            continue;
        };
        let typed = ["seedSalt", "activeDate", "puzzleId", "start", "target"]
            .iter()
            .all(|k| stored.get(*k).map_or(true, Value::is_string))
            && stored
                .get("guesses")
                .and_then(Value::as_array)
                .is_some_and(|g| g.iter().all(Value::is_string))
            && stored.get("completed").is_some_and(Value::is_boolean)
            && stored.get("won").is_some_and(Value::is_boolean);
        if !typed {
            continue;
        }
        let got = puzzle_from(&row["stored"]).normalized("2026-10-02", || {
            "11111111-2222-4333-8444-555555555555".into()
        });
        assert_eq!(got, puzzle_from(&row["loaded"]), "{name}");
    }
}

// ----------------------------------------------------------------------------- semantics

#[test]
fn submit_rules() {
    let mut p = TravlePuzzle::fresh("2026-10-02", "keep-salt");
    assert_eq!(
        (p.start.as_str(), p.target.as_str()),
        ("Belgium", "Turkmenistan")
    );
    assert_eq!(p.submit("Atlantis"), TravleSubmit::NotACountry);
    assert_eq!(p.submit(" belgium "), TravleSubmit::AlreadyInRoute);
    assert!(matches!(p.submit("germany"), TravleSubmit::Accepted { .. }));
    assert_eq!(p.guesses, ["Germany"], "stored as production's name");
    assert_eq!(p.submit("Germany"), TravleSubmit::AlreadyInRoute);
    // an island with no borders is a valid (useless) guess, exactly as in production
    assert_eq!(p.submit("Australia").message(), "Australia is off route.");
    for c in ["Poland", "Russia", "Kazakhstan"] {
        p.submit(c);
    }
    assert!(p.won && p.completed);
    assert_eq!(p.submit("France"), TravleSubmit::Ignored);
    assert_eq!(p.guesses_left(), 2);
    assert_eq!(p.current(), "Kazakhstan");
    assert_eq!(
        p.route(),
        [
            "Belgium",
            "Germany",
            "Australia",
            "Poland",
            "Russia",
            "Kazakhstan"
        ]
    );
}

#[test]
fn sri_lanka_can_start_but_never_be_reached() {
    assert_eq!(shortest_path("Sri Lanka", "India"), ["Sri Lanka", "India"]);
    assert!(shortest_path("India", "Sri Lanka").is_empty());
    assert!(are_neighbours("Sri Lanka", "India"));
    assert!(!are_neighbours("India", "Sri Lanka"));
}

#[test]
fn unknown_and_corrupt_names_never_panic() {
    let p = TravlePuzzle {
        start: "Atlantis".into(),
        target: "".into(),
        guesses: vec!["Mordor".into(), "France".into(), "France".into(), "".into()],
        ..TravlePuzzle::default()
    };
    let r = p.routes();
    assert!(r.shortest().is_empty() && r.solved().is_empty() && r.display().is_empty());
    assert!(r.guess_states().is_empty());
    let roles = map_roles(&p);
    // only France (first route country found, and the... no: current is "" -> none) is marked
    assert!(roles.iter().all(|(id, _)| id.code() == "FRA"));
    assert!(display_route(&p).is_empty());
}

#[test]
fn map_roles_follow_production_classes() {
    let mut p = TravlePuzzle::fresh("2026-10-02", "keep-salt");
    let role = |p: &TravlePuzzle, code: &str| {
        map_roles(p)
            .into_iter()
            .find(|(id, _)| id.code() == code)
            .map(|(_, r)| r)
            .unwrap_or_default()
    };
    // fresh: start (also current) and target
    assert_eq!(map_roles(&p).len(), 2);
    assert!(role(&p, "BEL").start && role(&p, "BEL").current);
    assert!(role(&p, "TKM").target && !role(&p, "TKM").route);
    p.submit("Australia");
    // a guessed country without borders: no state class, only `current`
    assert_eq!(
        role(&p, "AUS"),
        MapRole {
            current: true,
            ..MapRole::default()
        }
    );
    assert!(!role(&p, "BEL").current);
    for c in ["Germany", "Poland", "Russia", "Kazakhstan"] {
        p.submit(c);
    }
    // won: the destination joins the solved route
    assert!(role(&p, "TKM").route && role(&p, "TKM").target);
    assert!(role(&p, "BEL").route, "the start is on the solved route");
}

#[test]
#[ignore]
fn travle_timing_report() {
    let t = std::time::Instant::now();
    countries();
    println!("countries {:?}", t.elapsed());
    let t = std::time::Instant::now();
    border_graph();
    find_travle_country("x");
    println!("graph+ids {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let n = daily_pair_count();
    println!("pairs {n}, build {:?}", t.elapsed());
    let t = std::time::Instant::now();
    for i in 0..100 {
        puzzle_for_date(&format!("2026-10-{:02}", 1 + i % 28), "salt");
    }
    println!("puzzle_for_date x100 {:?}", t.elapsed());
}
