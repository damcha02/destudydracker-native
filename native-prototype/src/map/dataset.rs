//! Map dataset: parsing of the copied production geometry plus deterministic stress generators.

use super::{Bounds, GeometryStats, Point, Region, Ring, WORLD_HEIGHT, WORLD_WIDTH};
use std::fmt::Write as _;

/// The copied production data (see `scripts/extract-map-data.py`).
const WORLD_TSV: &str = include_str!("../../assets/map/world-countries.tsv");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StressLevel {
    /// Real data, ~9.7k vertices, 167 path regions + 28 point markers.
    World,
    /// Real data decimated to ~1k vertices (M1).
    Light,
    /// Real data, edges subdivided x3 (~29k vertices, M3).
    Dense3,
    /// x10 (~97k vertices, M3+).
    Dense10,
    /// x50 (~484k vertices, M4 diagnostic).
    Dense50,
    /// 2,000 small regions x 16 vertices (many-paths shape).
    Cells,
    /// One region, one ring, 50,000 vertices (few-huge-paths shape).
    Giant,
    /// Edge cases.
    Empty,
    OneRegion,
    TinyAndLarge,
    MultiWithHole,
}

impl StressLevel {
    pub const ALL: [StressLevel; 11] = [
        StressLevel::World,
        StressLevel::Light,
        StressLevel::Dense3,
        StressLevel::Dense10,
        StressLevel::Dense50,
        StressLevel::Cells,
        StressLevel::Giant,
        StressLevel::Empty,
        StressLevel::OneRegion,
        StressLevel::TinyAndLarge,
        StressLevel::MultiWithHole,
    ];

    pub fn from_index(i: usize) -> Self {
        Self::ALL.get(i).copied().unwrap_or(StressLevel::World)
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }

    pub fn label(self) -> &'static str {
        match self {
            StressLevel::World => "World (real, 9.7k pts)",
            StressLevel::Light => "Light (1k pts)",
            StressLevel::Dense3 => "Dense ×3",
            StressLevel::Dense10 => "Dense ×10",
            StressLevel::Dense50 => "Dense ×50 (diagnostic)",
            StressLevel::Cells => "Cells (2,000 regions)",
            StressLevel::Giant => "Giant (1 path, 50k pts)",
            StressLevel::Empty => "Empty",
            StressLevel::OneRegion => "One region",
            StressLevel::TinyAndLarge => "Tiny + huge",
            StressLevel::MultiWithHole => "Multi-part + hole",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapDataset {
    pub regions: Vec<Region>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MapError {
    BadLine { line: usize, reason: String },
}

impl MapDataset {
    pub fn for_level(level: StressLevel) -> Self {
        match level {
            StressLevel::World => Self::world(),
            StressLevel::Light => Self::world().decimated(10),
            StressLevel::Dense3 => Self::world().densified(3),
            StressLevel::Dense10 => Self::world().densified(10),
            StressLevel::Dense50 => Self::world().densified(50),
            StressLevel::Cells => Self::cells(2_000, 16),
            StressLevel::Giant => Self::giant(50_000),
            StressLevel::Empty => Self {
                regions: Vec::new(),
            },
            StressLevel::OneRegion => Self::one_region(),
            StressLevel::TinyAndLarge => Self::tiny_and_large(),
            StressLevel::MultiWithHole => Self::multi_with_hole(),
        }
    }

    /// The bundled real dataset. Panics only if the bundled asset is corrupt (covered by a test).
    pub fn world() -> Self {
        Self::from_tsv(WORLD_TSV).expect("bundled world-countries.tsv must parse")
    }

    /// Parses `code, name, continent, region, population, area, point_x, point_y, path` rows.
    pub fn from_tsv(text: &str) -> Result<Self, MapError> {
        let mut regions = Vec::new();
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let bad = |reason: &str| MapError::BadLine {
                line: n + 1,
                reason: reason.to_string(),
            };
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() != 9 {
                return Err(bad("expected 9 tab-separated columns"));
            }
            let marker = match (cols[6], cols[7]) {
                ("-", _) | (_, "-") => None,
                (x, y) => Some((
                    x.parse::<f64>().map_err(|_| bad("bad point x"))?,
                    y.parse::<f64>().map_err(|_| bad("bad point y"))?,
                )),
            };
            let rings = parse_path(cols[8]).map_err(|e| bad(&e))?;
            if rings.is_empty() && marker.is_none() {
                return Err(bad("region has neither geometry nor a marker"));
            }
            regions.push(Region::build(
                cols[0].to_string(),
                cols[1].to_string(),
                cols[2].to_string(),
                cols[3].to_string(),
                cols[4].parse().map_err(|_| bad("bad population"))?,
                cols[5].parse().map_err(|_| bad("bad area"))?,
                rings,
                marker,
                continent_tone(cols[2]),
            ));
        }
        Ok(Self { regions })
    }

    pub fn stats(&self) -> GeometryStats {
        GeometryStats {
            regions: self.regions.len(),
            marker_regions: self.regions.iter().filter(|r| r.marker.is_some()).count(),
            rings: self.regions.iter().map(|r| r.rings.len()).sum(),
            vertices: self.regions.iter().map(|r| r.vertex_count()).sum(),
            path_bytes: self.regions.iter().map(|r| r.path.len()).sum(),
        }
    }

    /// Keep every `keep_every`-th vertex of each ring (at least 3); drops rings that collapse.
    pub fn decimated(&self, keep_every: usize) -> Self {
        let keep = keep_every.max(1);
        Self {
            regions: self
                .regions
                .iter()
                .map(|r| r.map_rings(|ring| decimate_ring(ring, keep)))
                .collect(),
        }
    }

    /// Subdivide every edge into `factor` pieces with deterministic sideways jitter, so the
    /// result has coastline-like detail rather than collinear points.
    pub fn densified(&self, factor: usize) -> Self {
        let f = factor.max(1);
        Self {
            regions: self
                .regions
                .iter()
                .enumerate()
                .map(|(ri, r)| {
                    let mut ring_index = 0;
                    r.map_rings(|ring| {
                        ring_index += 1;
                        densify_ring(ring, f, (ri as u64) << 20 | ring_index)
                    })
                })
                .collect(),
        }
    }

    /// `count` disjoint irregular polygons of `points` vertices tiled over the world.
    pub fn cells(count: usize, points: usize) -> Self {
        let points = points.max(3);
        let cols = ((count as f64 * 2.0).sqrt().ceil() as usize).max(1);
        let rows = count.div_ceil(cols).max(1);
        let (cw, ch) = (WORLD_WIDTH / cols as f64, WORLD_HEIGHT / rows as f64);
        let regions = (0..count)
            .map(|n| {
                let (cx, cy) = ((n % cols) as f64 + 0.5, (n / cols) as f64 + 0.5);
                let center = (cx * cw, cy * ch);
                let ring: Vec<Point> = (0..points)
                    .map(|k| {
                        let angle = k as f64 / points as f64 * std::f64::consts::TAU;
                        let r = 0.36 + 0.12 * unit_noise(n as u64, k as u64);
                        (
                            center.0 + angle.cos() * r * cw,
                            center.1 + angle.sin() * r * ch,
                        )
                    })
                    .collect();
                Region::build(
                    format!("C{n:04}"),
                    format!("Cell {n:04}"),
                    "Synthetic".into(),
                    String::new(),
                    1_000 + n as u64,
                    10 + n as u64,
                    Ring::new(ring).into_iter().collect(),
                    None,
                    (n % 5) as i32,
                )
            })
            .collect();
        Self { regions }
    }

    /// One star-shaped (hence simple) polygon with `points` vertices covering most of the world.
    pub fn giant(points: usize) -> Self {
        let points = points.max(3);
        let ring: Vec<Point> = (0..points)
            .map(|k| {
                let angle = k as f64 / points as f64 * std::f64::consts::TAU;
                let r = 0.9 + 0.08 * unit_noise(7, k as u64);
                (
                    500.0 + angle.cos() * r * 480.0,
                    250.0 + angle.sin() * r * 235.0,
                )
            })
            .collect();
        Self {
            regions: vec![Region::build(
                "GNT".into(),
                "Giant polygon".into(),
                "Synthetic".into(),
                String::new(),
                1,
                1,
                Ring::new(ring).into_iter().collect(),
                None,
                1,
            )],
        }
    }

    fn one_region() -> Self {
        let square = vec![
            (300.0, 150.0),
            (700.0, 150.0),
            (700.0, 350.0),
            (300.0, 350.0),
        ];
        Self {
            regions: vec![Region::build(
                "ONE".into(),
                "Single square".into(),
                "Synthetic".into(),
                String::new(),
                42,
                42,
                Ring::new(square).into_iter().collect(),
                None,
                0,
            )],
        }
    }

    fn tiny_and_large() -> Self {
        let whole = vec![(2.0, 2.0), (998.0, 2.0), (998.0, 498.0), (2.0, 498.0)];
        let tiny = vec![(500.0, 250.0), (500.08, 250.0), (500.04, 250.07)];
        let normal = vec![
            (100.0, 100.0),
            (220.0, 90.0),
            (240.0, 200.0),
            (120.0, 220.0),
        ];
        let mk = |code: &str, name: &str, pts: Vec<Point>, tone: i32| {
            Region::build(
                code.into(),
                name.into(),
                "Synthetic".into(),
                String::new(),
                1,
                1,
                Ring::new(pts).into_iter().collect(),
                None,
                tone,
            )
        };
        // Drawn back to front: the huge polygon first, so smaller regions sit on top of it.
        Self {
            regions: vec![
                mk("BIG", "Huge polygon", whole, 3),
                mk("NRM", "Normal polygon", normal, 1),
                mk("TNY", "Tiny polygon", tiny, 2),
            ],
        }
    }

    fn multi_with_hole() -> Self {
        let sq = |x: f64, y: f64, s: f64| vec![(x, y), (x + s, y), (x + s, y + s), (x, y + s)];
        let rings: Vec<Ring> = [
            sq(100.0, 100.0, 200.0),
            sq(150.0, 150.0, 100.0),
            sq(400.0, 120.0, 60.0),
            sq(520.0, 300.0, 40.0),
            sq(700.0, 200.0, 8.0),
        ]
        .into_iter()
        .filter_map(Ring::new)
        .collect();
        Self {
            regions: vec![Region::build(
                "MUL".into(),
                "Archipelago with lake".into(),
                "Synthetic".into(),
                String::new(),
                9,
                9,
                rings,
                None,
                2,
            )],
        }
    }
}

impl Region {
    /// Computes bounds, label anchor and the world-space path string.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        code: String,
        name: String,
        continent: String,
        subregion: String,
        population: u64,
        area_km2: u64,
        rings: Vec<Ring>,
        marker: Option<Point>,
        tone: i32,
    ) -> Self {
        let mut bounds = Bounds::EMPTY;
        for ring in &rings {
            bounds.union(&ring.bounds);
        }
        if let Some(m) = marker {
            bounds.extend(m);
        }
        let label_anchor = label_anchor(&rings).or(marker).unwrap_or((0.0, 0.0));
        let path = path_string(&rings);
        Self {
            code,
            name,
            continent,
            subregion,
            population,
            area_km2,
            rings,
            marker,
            bounds,
            label_anchor,
            tone,
            path,
        }
    }

    fn map_rings(&self, mut f: impl FnMut(&Ring) -> Option<Ring>) -> Self {
        let rings: Vec<Ring> = self.rings.iter().filter_map(&mut f).collect();
        Self::build(
            self.code.clone(),
            self.name.clone(),
            self.continent.clone(),
            self.subregion.clone(),
            self.population,
            self.area_km2,
            rings,
            self.marker,
            self.tone,
        )
    }
}

fn continent_tone(continent: &str) -> i32 {
    match continent {
        "Europe" => 0,
        "Americas" => 1,
        "Africa" => 2,
        "Asia" => 3,
        "Oceania" => 4,
        _ => 0,
    }
}

/// Parses absolute `M`/`L`/`Z` path data into rings. Errors on unknown commands or numbers.
/// Command letters may be glued to numbers on either side (`M1 2`, `3 4Z`), as in the source data.
pub fn parse_path(d: &str) -> Result<Vec<Ring>, String> {
    let mut spaced = String::with_capacity(d.len() + d.len() / 4);
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            spaced.push(' ');
            spaced.push(ch);
            spaced.push(' ');
        } else if ch == ',' {
            spaced.push(' ');
        } else {
            spaced.push(ch);
        }
    }

    let mut rings = Vec::new();
    let mut current: Vec<Point> = Vec::new();
    let mut tokens = spaced.split_whitespace();
    let flush = |current: &mut Vec<Point>, rings: &mut Vec<Ring>| {
        if let Some(ring) = Ring::new(std::mem::take(current)) {
            rings.push(ring);
        }
    };
    let number = |tokens: &mut std::str::SplitWhitespace| -> Result<f64, String> {
        let text = tokens.next().ok_or("missing coordinate")?;
        text.parse::<f64>()
            .map_err(|_| format!("bad number '{text}'"))
    };

    while let Some(cmd) = tokens.next() {
        match cmd {
            "M" | "L" => {
                if cmd == "M" {
                    flush(&mut current, &mut rings);
                }
                let x = number(&mut tokens)?;
                let y = number(&mut tokens)?;
                current.push((x, y));
            }
            "Z" | "z" => flush(&mut current, &mut rings),
            other => return Err(format!("unsupported path command or token '{other}'")),
        }
    }
    flush(&mut current, &mut rings);
    Ok(rings)
}

fn path_string(rings: &[Ring]) -> String {
    let mut s = String::with_capacity(rings.iter().map(|r| r.points.len() * 16).sum());
    for ring in rings {
        for (i, (x, y)) in ring.points.iter().enumerate() {
            let _ = write!(s, "{}{x:.3} {y:.3}", if i == 0 { "M " } else { " L " });
        }
        s.push_str(" Z ");
    }
    s
}

/// Centroid of the largest ring (by bounding-box area), falling back to its bbox centre when the
/// centroid is not inside the ring (concave shapes).
fn label_anchor(rings: &[Ring]) -> Option<Point> {
    let ring = rings.iter().max_by(|a, b| {
        (a.bounds.width() * a.bounds.height()).total_cmp(&(b.bounds.width() * b.bounds.height()))
    })?;
    let (mut area2, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let n = ring.points.len();
    for i in 0..n {
        let (x0, y0) = ring.points[i];
        let (x1, y1) = ring.points[(i + 1) % n];
        let cross = x0 * y1 - x1 * y0;
        area2 += cross;
        cx += (x0 + x1) * cross;
        cy += (y0 + y1) * cross;
    }
    let fallback = ring.bounds.center();
    if area2.abs() < 1e-9 {
        return Some(fallback);
    }
    let centroid = (cx / (3.0 * area2), cy / (3.0 * area2));
    Some(
        if centroid.0.is_finite()
            && centroid.1.is_finite()
            && super::hit::point_in_ring(centroid, ring)
        {
            centroid
        } else {
            fallback
        },
    )
}

fn decimate_ring(ring: &Ring, keep: usize) -> Option<Ring> {
    let mut pts: Vec<Point> = ring.points.iter().step_by(keep).copied().collect();
    if pts.len() < 3 {
        pts = ring.points.iter().copied().take(3).collect();
    }
    Ring::new(pts)
}

fn densify_ring(ring: &Ring, factor: usize, seed: u64) -> Option<Ring> {
    let n = ring.points.len();
    let mut out = Vec::with_capacity(n * factor);
    for i in 0..n {
        let a = ring.points[i];
        let b = ring.points[(i + 1) % n];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy);
        let (nx, ny) = if len > 0.0 {
            (-dy / len, dx / len)
        } else {
            (0.0, 0.0)
        };
        out.push(a);
        for k in 1..factor {
            let t = k as f64 / factor as f64;
            let jitter = unit_noise(seed.wrapping_add(i as u64 * 131), k as u64) * 0.10 * len
                / factor as f64;
            out.push((a.0 + dx * t + nx * jitter, a.1 + dy * t + ny * jitter));
        }
    }
    Ring::new(out)
}

/// Deterministic value in -1.0..1.0 (splitmix64 of the two keys).
fn unit_noise(a: u64, b: u64) -> f64 {
    let mut z = a.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ b.wrapping_add(0x1234_5678_9ABC_DEF1);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z as f64 / u64::MAX as f64) * 2.0 - 1.0
}
