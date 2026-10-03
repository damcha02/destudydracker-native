//! Travle's map (Stage 21): production's pre-projected world (`desktop/src/lib/travleMapData.ts`,
//! copied verbatim to `assets/map/travle-map.tsv`, viewBox `0 0 1000 500`) and the render-time
//! derivations `App.tsx` makes from it: the auto-fitted viewBox, the route line and each country's
//! `travle-map-country` style. Presentation only; the game itself is
//! `study_tracker_core::break_room::travle`.
//!
//! ```text
//! travle-map.tsv ──(first open, once)──► MapCountry { id, path, point, bounds }  (static, shared
//!                                                     with the Stage 11 map lab's World level)
//! TravlePuzzle ──► core map_roles / display_route ──► view_box + route_line + style per country
//! ```
//!
//! Production's map is not interactive: no hover, no click, no pan or wheel; only the `+`/`−`
//! buttons, which change a zoom factor the viewBox divides by. So there is no hit testing.

use std::sync::OnceLock;

use study_tracker_core::break_room::travle::{CountryId, MapRole};

/// The copied production geometry (also the map lab's World data set).
pub const MAP_TSV: &str = include_str!("../assets/map/travle-map.tsv");

pub const WORLD_WIDTH: f64 = 1000.0;
pub const WORLD_HEIGHT: f64 = 500.0;
/// `r="3.6"` of a point-only country's circle.
pub const MARKER_RADIUS: f64 = 3.6;

/// One `TRAVLE_MAP_COUNTRIES` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct MapCountry {
    pub code: String,
    /// The game country it draws (`None` would be a map-only region; production has none).
    pub id: Option<CountryId>,
    /// SVG path data in world units (`d`), verbatim; empty for point-only countries.
    pub path: String,
    /// `point`: the circle centre of a point-only country (and its route-line anchor).
    pub point: Option<(f64, f64)>,
    /// `bounds`: min x, min y, max x, max y.
    pub bounds: [f64; 4],
}

impl MapCountry {
    /// What Slint draws: the path, or a circle of radius 3.6 around the point.
    pub fn commands(&self) -> String {
        match (self.path.is_empty(), self.point) {
            (false, _) => self.path.clone(),
            (true, Some((x, y))) => {
                let r = MARKER_RADIUS;
                format!(
                    "M {} {y} A {r} {r} 0 1 0 {} {y} A {r} {r} 0 1 0 {} {y} Z",
                    x - r,
                    x + r,
                    x - r
                )
            }
            (true, None) => String::new(),
        }
    }

    pub fn is_marker(&self) -> bool {
        self.path.is_empty() && self.point.is_some()
    }

    /// The route line's anchor: `point`, else the bounds' centre.
    pub fn anchor(&self) -> (f64, f64) {
        self.point.unwrap_or((
            (self.bounds[0] + self.bounds[2]) / 2.0,
            (self.bounds[1] + self.bounds[3]) / 2.0,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapDataError {
    pub line: usize,
    pub reason: &'static str,
}

/// Parses `code, point_x, point_y, min_x, min_y, max_x, max_y, path` rows ("-" = absent).
pub fn parse(text: &str) -> Result<Vec<MapCountry>, MapDataError> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bad = |reason| MapDataError {
            line: n + 1,
            reason,
        };
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 8 {
            return Err(bad("expected 8 tab-separated columns"));
        }
        let num = |s: &str| -> Result<Option<f64>, MapDataError> {
            if s == "-" {
                return Ok(None);
            }
            let v: f64 = s.parse().map_err(|_| bad("bad number"))?;
            if v.is_finite() {
                Ok(Some(v))
            } else {
                Err(bad("non-finite number"))
            }
        };
        let point = match (num(cols[1])?, num(cols[2])?) {
            (Some(x), Some(y)) => Some((x, y)),
            (None, None) => None,
            _ => return Err(bad("half a point")),
        };
        let mut bounds = [0.0; 4];
        for (i, b) in bounds.iter_mut().enumerate() {
            *b = num(cols[3 + i])?.ok_or(bad("missing bounds"))?;
        }
        let path = if cols[7] == "-" { "" } else { cols[7] };
        if path.is_empty() && point.is_none() {
            return Err(bad("neither a path nor a point"));
        }
        out.push(MapCountry {
            code: cols[0].to_string(),
            id: CountryId::from_code(cols[0]),
            path: path.to_string(),
            point,
            bounds,
        });
    }
    Ok(out)
}

/// `TRAVLE_MAP_COUNTRIES`, parsed on the first Travle open (never at startup) and kept.
pub fn countries() -> &'static [MapCountry] {
    static MAP: OnceLock<Vec<MapCountry>> = OnceLock::new();
    MAP.get_or_init(|| match parse(MAP_TSV) {
        Ok(map) => map,
        Err(e) => {
            // Covered by a test; a corrupt build draws an empty map instead of failing.
            log::error!("travle map data unreadable: {e:?}");
            Vec::new()
        }
    })
}

/// `travleMapCountryByCode.get(code)` for a game country.
pub fn country(id: CountryId) -> Option<&'static MapCountry> {
    countries().iter().find(|c| c.id == Some(id))
}

/// A viewBox `x y width height` in world units.
pub type ViewBox = [f64; 4];

/// `travleMapViewBox`: fit the route's countries and the destination (at least 120 x 90 world
/// units, plus a margin of 40 x 35 each side), divided by the zoom factor, clamped inside the world.
/// The whole world when none of them has map data.
pub fn view_box(focus: &[CountryId], zoom: f64) -> ViewBox {
    let mut seen: Vec<CountryId> = Vec::new();
    let mut boxes = Vec::new();
    for &id in focus {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        if let Some(c) = country(id) {
            boxes.push(c.bounds);
        }
    }
    if boxes.is_empty() {
        return [0.0, 0.0, WORLD_WIDTH, WORLD_HEIGHT];
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for b in boxes {
        min_x = min_x.min(b[0]);
        min_y = min_y.min(b[1]);
        max_x = max_x.max(b[2]);
        max_y = max_y.max(b[3]);
    }
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;
    let fit_width = 120f64.max(max_x - min_x + 80.0);
    let fit_height = 90f64.max(max_y - min_y + 70.0);
    let width = WORLD_WIDTH.min(fit_width / zoom);
    let height = WORLD_HEIGHT.min(fit_height / zoom);
    let x = 0f64.max((WORLD_WIDTH - width).min(center_x - width / 2.0));
    let y = 0f64.max((WORLD_HEIGHT - height).min(center_y - height / 2.0));
    [x, y, width, height]
}

/// `travleMapRoutePoints`: the display route's anchors (countries without map data skipped).
pub fn route_points(route: &[CountryId]) -> Vec<(f64, f64)> {
    route
        .iter()
        .filter_map(|&id| country(id))
        .map(MapCountry::anchor)
        .collect()
}

/// The polyline as path commands; empty below two points (an SVG polyline of one point draws
/// nothing).
pub fn route_commands(points: &[(f64, f64)]) -> String {
    if points.len() < 2 {
        return String::new();
    }
    let mut out = String::new();
    for (i, (x, y)) in points.iter().enumerate() {
        use std::fmt::Write as _;
        let _ = write!(out, "{}{x} {y}", if i == 0 { "M " } else { " L " });
    }
    out
}

/// The zoom buttons (`Math.round((zoom ± 0.35) * 100) / 100`, between 1 and 2.5).
pub fn zoom_in(zoom: f64) -> f64 {
    2.5f64.min(((zoom + 0.35) * 100.0).round() / 100.0)
}

pub fn zoom_out(zoom: f64) -> f64 {
    1f64.max(((zoom - 0.35) * 100.0).round() / 100.0)
}

/// RGBA (non-premultiplied, 0-255 alpha).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

const fn rgb(v: u32) -> Rgba {
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

/// `rgba(r, g, b, a)` with CSS's alpha rounding to 8 bits.
fn rgba(r: u8, g: u8, b: u8, a: f32) -> Rgba {
    Rgba(r, g, b, (a * 255.0).round() as u8)
}

/// A country's resolved `.travle-map-country` style: fill, stroke and its on-screen width
/// (`vector-effect: non-scaling-stroke`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CountryStyle {
    pub fill: Rgba,
    pub stroke: Rgba,
    pub width: f32,
}

/// `.travle-map-water` / a plain country (the card, graticule and route line are fixed colours
/// in `ui/break/games.slint`).
pub const WATER: Rgba = rgb(0x302e2b);

/// The CSS cascade of `App.css`'s `.travle-map-country` rules: every rule has the same
/// specificity, so for each property the last matching rule in source order wins (route,
/// possible, miss, start, target, current, marker).
pub fn style(role: MapRole, marker: bool) -> CountryStyle {
    let mut s = CountryStyle {
        fill: WATER,
        stroke: Rgba(0, 0, 0, 0),
        width: 0.0,
    };
    if role.route {
        s = CountryStyle {
            fill: rgb(0xb6d983),
            stroke: rgba(247, 239, 214, 0.68),
            width: 1.25,
        };
    }
    if role.possible {
        s = CountryStyle {
            fill: rgb(0xe6d46f),
            stroke: rgba(247, 239, 214, 0.62),
            width: 1.15,
        };
    }
    if role.miss {
        s = CountryStyle {
            fill: rgb(0x25231f),
            stroke: rgba(232, 220, 199, 0.24),
            width: 1.0,
        };
    }
    if role.start {
        s.fill = rgb(0x82b9ad);
    }
    if role.target {
        s = CountryStyle {
            fill: rgb(0xd887a1),
            stroke: rgba(247, 239, 214, 0.76),
            width: 1.35,
        };
    }
    if role.current {
        s.stroke = rgb(0xf2df9f);
        s.width = 2.2;
    }
    if marker {
        s.width = 1.5;
    }
    s
}

/// Production's class list for a role (tests compare it with the fixtures).
#[cfg(test)]
pub fn class_name(role: MapRole) -> String {
    let mut parts = Vec::new();
    if role.possible {
        parts.push("possible");
    }
    if role.miss {
        parts.push("miss");
    }
    if role.route {
        parts.push("route");
    }
    if role.start {
        parts.push("start");
    }
    if role.target {
        parts.push("target");
    }
    if role.current {
        parts.push("current");
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use study_tracker_core::break_room::travle::{display_route, map_roles, TravlePuzzle};

    fn fixture() -> Value {
        serde_json::from_str(include_str!("../tests/fixtures/travle-view.json")).unwrap()
    }

    fn puzzle(v: &Value) -> TravlePuzzle {
        TravlePuzzle {
            start: v["start"].as_str().unwrap().into(),
            target: v["target"].as_str().unwrap().into(),
            guesses: v["guesses"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g.as_str().unwrap().to_string())
                .collect(),
            ..TravlePuzzle::default()
        }
    }

    /// `${x} ${y} ${w} ${h}` / `${x},${y} ...` parsed back to numbers.
    fn numbers(s: &str) -> Vec<f64> {
        s.split([' ', ','])
            .filter(|t| !t.is_empty())
            .map(|t| t.parse().unwrap())
            .collect()
    }

    #[test]
    fn bundled_map_data_is_production_and_complete() {
        let map = countries();
        assert_eq!(map.len(), 195);
        assert_eq!(map.iter().filter(|c| c.is_marker()).count(), 28);
        assert_eq!(map.iter().filter(|c| !c.path.is_empty()).count(), 167);
        // every map region is a game country and every game country has map data
        let ids: Vec<CountryId> = map.iter().map(|c| c.id.expect(&c.code)).collect();
        for id in CountryId::all() {
            assert!(ids.contains(&id), "{} has no map region", id.code());
        }
        for c in map {
            assert!(c.bounds[0] <= c.bounds[2] && c.bounds[1] <= c.bounds[3]);
            assert!(!c.commands().is_empty());
        }
        // production's file order (alphabetical by code) is the draw order
        let codes: Vec<&str> = map.iter().map(|c| c.code.as_str()).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted);
    }

    #[test]
    fn malformed_map_rows_fail_safely() {
        assert!(parse("FRA\t-\t-\t1\t2\t3\t4\t-").is_err(), "no geometry");
        assert!(parse("FRA\t1\t-\t1\t2\t3\t4\tM0 0Z").is_err(), "half point");
        assert!(parse("FRA\t-\t-\tx\t2\t3\t4\tM0 0Z").is_err());
        assert!(parse("FRA\t-\t-\tinf\t2\t3\t4\tM0 0Z").is_err());
        assert!(parse("FRA\t-\t-").is_err());
        let ok = parse("# h\nZZZ\t-\t-\t1\t2\t3\t4\tM1 2 L3 4Z").unwrap();
        assert_eq!(ok[0].id, None, "unknown code: drawn, never styled");
    }

    #[test]
    fn golden_viewbox_route_line_and_classes_match_production() {
        let f = fixture();
        let views = f["views"].as_array().unwrap();
        assert!(views.len() > 300);
        for v in views {
            let p = puzzle(v);
            let zoom = v["zoom"].as_f64().unwrap();
            let ctx = format!(
                "{:?} -> {:?} via {:?} @{zoom}",
                p.start, p.target, p.guesses
            );
            // focus: route codes + target code (unknown names skipped)
            let find = study_tracker_core::break_room::travle::find_travle_country;
            let mut focus: Vec<CountryId> = p.route().iter().filter_map(|n| find(n)).collect();
            focus.extend(find(&p.target));
            let vb = view_box(&focus, zoom);
            assert_eq!(
                vb.to_vec(),
                numbers(v["viewBox"].as_str().unwrap()),
                "{ctx}"
            );
            let pts: Vec<f64> = route_points(&display_route(&p))
                .into_iter()
                .flat_map(|(x, y)| [x, y])
                .collect();
            assert_eq!(pts, numbers(v["points"].as_str().unwrap()), "{ctx}");
            let classes: std::collections::BTreeMap<String, String> = map_roles(&p)
                .into_iter()
                .map(|(id, role)| (id.code().to_string(), class_name(role)))
                .collect();
            let want: std::collections::BTreeMap<String, String> = v["classes"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, c)| (k.clone(), c.as_str().unwrap().replace(" marker", "")))
                .collect();
            assert_eq!(classes, want, "{ctx}");
        }
    }

    #[test]
    fn golden_zoom_buttons_round_like_production() {
        let f = fixture();
        let mut z = 1.0;
        for step in f["zoomSteps"].as_array().unwrap() {
            let s = step.as_array().unwrap();
            z = if s[0] == "in" {
                zoom_in(z)
            } else {
                zoom_out(z)
            };
            assert_eq!(z, s[1].as_f64().unwrap());
        }
    }

    #[test]
    fn styles_follow_the_css_cascade() {
        let plain = style(MapRole::default(), false);
        assert_eq!(plain.fill, WATER, "a plain country is the sea's colour");
        assert_eq!(plain.stroke.3, 0);
        let start = style(
            MapRole {
                start: true,
                current: true,
                ..MapRole::default()
            },
            false,
        );
        assert_eq!(
            (start.fill, start.stroke, start.width),
            (rgb(0x82b9ad), rgb(0xf2df9f), 2.2)
        );
        // the measured production values (getComputedStyle on the mid-game map)
        let miss_current = style(
            MapRole {
                miss: true,
                current: true,
                ..MapRole::default()
            },
            false,
        );
        assert_eq!(miss_current.fill, Rgba(37, 35, 31, 255));
        let target = style(
            MapRole {
                target: true,
                route: true,
                ..MapRole::default()
            },
            false,
        );
        assert_eq!(
            (target.fill, target.stroke, target.width),
            (Rgba(216, 135, 161, 255), Rgba(247, 239, 214, 194), 1.35)
        );
        assert_eq!(style(MapRole::default(), true).width, 1.5);
    }

    #[test]
    fn route_line_needs_two_points() {
        assert_eq!(route_commands(&[]), "");
        assert_eq!(route_commands(&[(1.0, 2.0)]), "");
        assert_eq!(route_commands(&[(1.0, 2.0), (3.5, 4.0)]), "M 1 2 L 3.5 4");
    }
}
