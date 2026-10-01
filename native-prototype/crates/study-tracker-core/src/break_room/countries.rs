//! Production's country table (`desktop/src/lib/countries.ts`, "objective fields from
//! mledoze/countries plus samayo/country-json"), shared by Geodle and Flaggle (Travle adds its own
//! border graph in Stage 21). Extracted verbatim, in production order, by
//! `scripts/stage20-extract-data.mjs`; parsed once, on first use, never at startup.

use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Country {
    pub code: &'static str,
    pub iso2: &'static str,
    pub name: &'static str,
    pub continent: &'static str,
    pub region: &'static str,
    pub population: u64,
    pub landlocked: bool,
    pub area_km2: u64,
    pub religion: &'static str,
    pub government: &'static str,
}

const DATA: &str = include_str!("../../data/break_room/countries.tsv");

/// `COUNTRIES`, in production order.
pub fn countries() -> &'static [Country] {
    static TABLE: OnceLock<Vec<Country>> = OnceLock::new();
    TABLE.get_or_init(|| {
        DATA.lines()
            .filter(|line| !line.starts_with('#') && !line.is_empty())
            .map(|line| {
                let cells: Vec<&'static str> = line.split('\t').collect();
                assert_eq!(cells.len(), 10, "countries.tsv row: {line}");
                Country {
                    code: cells[0],
                    iso2: cells[1],
                    name: cells[2],
                    continent: cells[3],
                    region: cells[4],
                    population: cells[5].parse().expect("population"),
                    landlocked: cells[6] == "true",
                    area_km2: cells[7].parse().expect("area"),
                    religion: cells[8],
                    government: cells[9],
                }
            })
            .collect()
    })
}

/// JavaScript `String.prototype.trim` (Rust's `trim` plus the BOM, which JS also strips).
fn js_trim(value: &str) -> &str {
    value.trim_matches(|c: char| c.is_whitespace() || c == '\u{feff}')
}

/// `normalizeCountryName`: trim, lower-case, every run of characters outside `[a-z0-9]` becomes
/// one space, then trim again. (So "Côte" matches as "c te", exactly like production.)
pub fn normalize_country_name(value: &str) -> String {
    let lower = js_trim(value).to_lowercase();
    let mut out = String::with_capacity(lower.len());
    let mut in_gap = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if in_gap && !out.is_empty() {
                out.push(' ');
            }
            in_gap = false;
            out.push(c);
        } else {
            in_gap = true;
        }
    }
    out
}

/// `findCountryByName` / `findFlaggleCountry`: an exact match after normalization.
pub fn find_country(value: &str) -> Option<&'static Country> {
    let normalized = normalize_country_name(value);
    countries()
        .iter()
        .find(|c| normalize_country_name(c.name) == normalized)
}

/// `filterCountries` / `filterFlaggleCountries`: every country whose normalized name contains the
/// normalized query (all of them for an empty query), in production order.
pub fn filter_countries(query: &str) -> Vec<&'static Country> {
    let normalized = normalize_country_name(query);
    countries()
        .iter()
        .filter(|c| normalized.is_empty() || normalize_country_name(c.name).contains(&normalized))
        .collect()
}
