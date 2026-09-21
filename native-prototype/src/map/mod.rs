//! Stage 11 map / geometry lab (presentation layer; nothing here belongs in `study-tracker-core`).
//!
//! ```text
//! assets/map/world-countries.tsv  ->  dataset.rs (parse, stress generators)
//!                                     viewport.rs (world <-> screen, zoom/pan)
//!                                     hit.rs      (bbox filter + point-in-polygon)
//!                                     mod.rs      (MapModel: hover/selection/drag + labels)
//! ```
//!
//! World space is the production Travle projection: x 0..1000, y 0..500 (y down). Geometry stays
//! in world space; only the viewport changes on pan/zoom. Slint draws the prepared path strings
//! once and moves/scales the whole layer, so pan/zoom never regenerates geometry.

pub mod dataset;
pub mod hit;
pub mod viewport;

use dataset::{MapDataset, StressLevel};
use viewport::Viewport;

pub type Point = (f64, f64);

pub const WORLD_WIDTH: f64 = 1000.0;
pub const WORLD_HEIGHT: f64 = 500.0;

/// Screen-space tolerance (px) for picking point markers and regions smaller than this.
pub const HIT_TOLERANCE_PX: f64 = 7.0;
/// Pointer travel (px) beyond which a press is a drag rather than a click.
const CLICK_SLOP_PX: f64 = 4.0;
const MAX_LABELS: usize = 48;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bounds {
    pub const EMPTY: Bounds = Bounds {
        min_x: f64::INFINITY,
        min_y: f64::INFINITY,
        max_x: f64::NEG_INFINITY,
        max_y: f64::NEG_INFINITY,
    };

    pub fn from_points(points: &[Point]) -> Self {
        let mut b = Self::EMPTY;
        for p in points {
            b.extend(*p);
        }
        b
    }

    pub fn extend(&mut self, p: Point) {
        self.min_x = self.min_x.min(p.0);
        self.min_y = self.min_y.min(p.1);
        self.max_x = self.max_x.max(p.0);
        self.max_y = self.max_y.max(p.1);
    }

    pub fn union(&mut self, other: &Bounds) {
        if other.is_valid() {
            self.extend((other.min_x, other.min_y));
            self.extend((other.max_x, other.max_y));
        }
    }

    pub fn is_valid(&self) -> bool {
        self.min_x <= self.max_x && self.min_y <= self.max_y
    }

    pub fn width(&self) -> f64 {
        (self.max_x - self.min_x).max(0.0)
    }

    pub fn height(&self) -> f64 {
        (self.max_y - self.min_y).max(0.0)
    }

    pub fn center(&self) -> Point {
        (
            (self.min_x + self.max_x) / 2.0,
            (self.min_y + self.max_y) / 2.0,
        )
    }

    pub fn contains(&self, p: Point) -> bool {
        p.0 >= self.min_x && p.0 <= self.max_x && p.1 >= self.min_y && p.1 <= self.max_y
    }

    pub fn expanded(&self, by: f64) -> Bounds {
        Bounds {
            min_x: self.min_x - by,
            min_y: self.min_y - by,
            max_x: self.max_x + by,
            max_y: self.max_y + by,
        }
    }

    #[cfg(test)]
    pub fn intersects(&self, other: &Bounds) -> bool {
        self.min_x <= other.max_x
            && self.max_x >= other.min_x
            && self.min_y <= other.max_y
            && self.max_y >= other.min_y
    }
}

/// One closed polygon boundary. The closing edge is implicit (first point is not repeated).
/// A region's rings may be disjoint parts (islands) or holes; see `hit::region_contains`.
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub points: Vec<Point>,
    pub bounds: Bounds,
}

impl Ring {
    /// Returns `None` for degenerate input (fewer than 3 distinct finite points).
    pub fn new(mut points: Vec<Point>) -> Option<Self> {
        points.retain(|p| p.0.is_finite() && p.1.is_finite());
        points.dedup();
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if points.len() < 3 {
            return None;
        }
        let bounds = Bounds::from_points(&points);
        Some(Self { points, bounds })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Region {
    pub code: String,
    pub name: String,
    pub continent: String,
    pub subregion: String,
    pub population: u64,
    pub area_km2: u64,
    pub rings: Vec<Ring>,
    /// Point-only regions (microstates) have no rings.
    pub marker: Option<Point>,
    pub bounds: Bounds,
    pub label_anchor: Point,
    /// Colour category (0..5) used by the UI; derived from continent or index.
    pub tone: i32,
    /// SVG-style `M x y L x y … Z` commands in world units (multiple sub-paths for multi-part).
    pub path: String,
}

impl Region {
    pub fn vertex_count(&self) -> usize {
        self.rings.iter().map(|r| r.points.len()).sum()
    }
}

/// Geometry counts reported next to every benchmark number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GeometryStats {
    pub regions: usize,
    pub marker_regions: usize,
    pub rings: usize,
    pub vertices: usize,
    pub path_bytes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    pub x: f64,
    pub y: f64,
    pub tone: i32,
}

/// Placement of the 1000x500 world layer inside the viewport: Slint positions the layer's
/// top-left at (`x`, `y`) and scales it by `scale` about its centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerTransform {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

#[derive(Debug, Clone, Copy)]
struct Drag {
    origin_center: Point,
    max_travel: f64,
}

/// Interactive map state: dataset + viewport + hover/selection + drag.
pub struct MapModel {
    dataset: MapDataset,
    level: StressLevel,
    viewport: Viewport,
    hovered: Option<usize>,
    selected: Option<usize>,
    drag: Option<Drag>,
    /// Alphabetical order of region indices (keyboard selection stepping).
    order: Vec<usize>,
}

impl MapModel {
    pub fn new(level: StressLevel) -> Self {
        let dataset = MapDataset::for_level(level);
        let order = alphabetical_order(&dataset);
        Self {
            dataset,
            level,
            viewport: Viewport::new(640.0, 320.0),
            hovered: None,
            selected: None,
            drag: None,
            order,
        }
    }

    pub fn set_level(&mut self, level: StressLevel) {
        self.dataset = MapDataset::for_level(level);
        self.order = alphabetical_order(&self.dataset);
        self.level = level;
        self.hovered = None;
        self.selected = None;
        self.drag = None;
        self.viewport.reset();
    }

    pub fn level(&self) -> StressLevel {
        self.level
    }

    pub fn dataset(&self) -> &MapDataset {
        &self.dataset
    }

    pub fn viewport(&self) -> &Viewport {
        &self.viewport
    }

    pub fn hovered(&self) -> Option<usize> {
        self.hovered
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn resize(&mut self, width: f64, height: f64) {
        self.viewport.resize(width, height);
    }

    pub fn layer(&self) -> LayerTransform {
        let s = self.viewport.scale();
        let (cx, cy) = self
            .viewport
            .world_to_screen((WORLD_WIDTH / 2.0, WORLD_HEIGHT / 2.0));
        LayerTransform {
            x: cx - WORLD_WIDTH / 2.0,
            y: cy - WORLD_HEIGHT / 2.0,
            scale: s,
        }
    }

    // --- pointer interaction -------------------------------------------------------------

    pub fn press(&mut self) {
        self.drag = Some(Drag {
            origin_center: self.viewport.center(),
            max_travel: 0.0,
        });
    }

    /// Pan relative to the press origin (deltas are total pointer travel since press, so
    /// repeated events never accumulate rounding error).
    pub fn drag(&mut self, dx: f64, dy: f64) {
        if let Some(drag) = self.drag.as_mut() {
            if !(dx.is_finite() && dy.is_finite()) {
                return;
            }
            drag.max_travel = drag.max_travel.max(dx.hypot(dy));
            let origin = drag.origin_center;
            self.viewport.pan_from(origin, dx, dy);
        }
    }

    /// Ends a press. Returns true when the press was a click (little movement) rather than a drag.
    pub fn release(&mut self) -> bool {
        let was_click = self
            .drag
            .map(|d| d.max_travel <= CLICK_SLOP_PX)
            .unwrap_or(false);
        self.drag = None;
        was_click
    }

    pub fn hover(&mut self, x: f64, y: f64) {
        self.hovered = self.pick(x, y);
    }

    pub fn leave(&mut self) {
        self.hovered = None;
    }

    pub fn click(&mut self, x: f64, y: f64) {
        self.selected = self.pick(x, y);
    }

    /// Region under a screen point, or `None` (outside map, water, invalid input).
    pub fn pick(&self, x: f64, y: f64) -> Option<usize> {
        if !(x.is_finite() && y.is_finite()) {
            return None;
        }
        let world = self.viewport.screen_to_world((x, y));
        if !(0.0..=WORLD_WIDTH).contains(&world.0) || !(0.0..=WORLD_HEIGHT).contains(&world.1) {
            return None;
        }
        hit::hit_test(
            &self.dataset,
            world,
            HIT_TOLERANCE_PX / self.viewport.scale(),
        )
    }

    pub fn zoom_wheel(&mut self, delta: f64, x: f64, y: f64) {
        if delta.is_finite() && delta != 0.0 {
            // Wheel notch (~ +/-120 logical px) -> ~15% zoom; smooth for touchpads too.
            self.viewport.zoom_at((x, y), (delta / 120.0 * 0.14).exp());
        }
    }

    pub fn zoom_step(&mut self, direction: i32) {
        let centre = self.viewport.screen_centre();
        self.viewport
            .zoom_at(centre, if direction >= 0 { 1.5 } else { 1.0 / 1.5 });
    }

    pub fn pan_pixels(&mut self, dx: f64, dy: f64) {
        let c = self.viewport.center();
        self.viewport.pan_from(c, -dx, -dy);
    }

    pub fn reset_view(&mut self) {
        self.viewport.reset();
    }

    /// Select the next/previous region alphabetically (keyboard navigation).
    pub fn step_selection(&mut self, delta: i32) {
        if self.order.is_empty() {
            self.selected = None;
            return;
        }
        let pos = self
            .selected
            .and_then(|s| self.order.iter().position(|o| *o == s));
        let next = match pos {
            Some(p) => (p as i64 + delta as i64).clamp(0, self.order.len() as i64 - 1) as usize,
            None if delta >= 0 => 0,
            None => self.order.len() - 1,
        };
        self.selected = Some(self.order[next]);
    }

    /// Zoom/pan so the selected region fills most of the viewport (works when it is offscreen).
    pub fn focus_selected(&mut self) {
        if let Some(region) = self.selected.and_then(|i| self.dataset.regions.get(i)) {
            let mut b = region.bounds;
            if let Some(m) = region.marker {
                b = Bounds::from_points(&[m]).expanded(15.0);
            }
            if b.is_valid() {
                self.viewport.fit_bounds(&b);
            }
        }
    }

    // --- derived presentation data --------------------------------------------------------

    pub fn markers(&self) -> Vec<Marker> {
        let (w, h) = self.viewport.size();
        self.dataset
            .regions
            .iter()
            .filter_map(|r| {
                let (x, y) = self.viewport.world_to_screen(r.marker?);
                (x >= -10.0 && y >= -10.0 && x <= w + 10.0 && y <= h + 10.0).then_some(Marker {
                    x,
                    y,
                    tone: r.tone,
                })
            })
            .collect()
    }

    /// Greedy, collision-avoiding labels for regions large enough on screen.
    pub fn labels(&self) -> Vec<Label> {
        let (vw, vh) = self.viewport.size();
        let s = self.viewport.scale();
        let mut candidates: Vec<(f64, &Region, f64)> = self
            .dataset
            .regions
            .iter()
            .filter(|r| r.marker.is_none() && r.bounds.is_valid())
            .filter_map(|r| {
                let text_w = r.name.chars().count() as f64 * 6.4 + 12.0;
                let (sw, sh) = (r.bounds.width() * s, r.bounds.height() * s);
                (sw >= text_w * 0.9 && sh >= 14.0).then_some((sw * sh, r, text_w))
            })
            .collect();
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0));

        let mut placed: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut labels = Vec::new();
        for (_, region, text_w) in candidates {
            if labels.len() >= MAX_LABELS {
                break;
            }
            let (x, y) = self.viewport.world_to_screen(region.label_anchor);
            let rect = (x - text_w / 2.0, y - 8.0, x + text_w / 2.0, y + 8.0);
            if rect.0 < 0.0 || rect.1 < 0.0 || rect.2 > vw || rect.3 > vh {
                continue;
            }
            let overlaps = placed
                .iter()
                .any(|p| rect.0 < p.2 && rect.2 > p.0 && rect.1 < p.3 && rect.3 > p.1);
            if !overlaps {
                placed.push(rect);
                labels.push(Label {
                    text: region.name.clone(),
                    x: rect.0,
                    y: rect.1,
                    width: text_w,
                });
            }
        }
        labels
    }

    /// Human-readable description of a region (the textual equivalent of the colour/geometry).
    pub fn describe(&self, index: Option<usize>) -> String {
        match index.and_then(|i| self.dataset.regions.get(i)) {
            None => "No region".to_string(),
            Some(r) => {
                let geometry = match (r.marker, r.rings.len()) {
                    (Some(_), _) => "point marker".to_string(),
                    (None, 1) => format!("1 part, {} points", r.vertex_count()),
                    (None, n) => format!("{n} parts, {} points", r.vertex_count()),
                };
                format!(
                    "{} ({}) · {}{} · pop. {} · {} km² · {}",
                    r.name,
                    r.code,
                    r.continent,
                    if r.subregion.is_empty() {
                        String::new()
                    } else {
                        format!(", {}", r.subregion)
                    },
                    group_thousands(r.population),
                    group_thousands(r.area_km2),
                    geometry
                )
            }
        }
    }
}

fn alphabetical_order(dataset: &MapDataset) -> Vec<usize> {
    let mut order: Vec<usize> = (0..dataset.regions.len()).collect();
    order.sort_by(|a, b| dataset.regions[*a].name.cmp(&dataset.regions[*b].name));
    order
}

pub fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('\'');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests;
