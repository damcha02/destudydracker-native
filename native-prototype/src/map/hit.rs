//! CPU hit testing: bounding-box filter, then even-odd point-in-polygon per ring.
//!
//! Even-odd parity across *all* rings of a region handles islands (disjoint rings), holes (ring
//! nested in a ring) and multipolygons without needing ring orientation. The same rule is used
//! for drawing (`fill-rule: even-odd`), so what you see is what you can click.

use super::dataset::MapDataset;
use super::{Bounds, Point, Region, Ring};

/// Work counters for hit-test scaling measurements.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HitStats {
    /// Regions whose bounding box contained the point.
    pub bbox_candidates: usize,
    /// Rings that were actually tested (ring bbox contained the point).
    pub rings_tested: usize,
    /// Polygon edges visited by the ray-casting loops.
    pub edges_visited: usize,
}

/// Ray casting. Points exactly on an edge may fall either way (irrelevant for picking).
pub fn point_in_ring(p: Point, ring: &Ring) -> bool {
    point_in_points(p, &ring.points, &mut 0)
}

fn point_in_points(p: Point, pts: &[Point], edges: &mut usize) -> bool {
    let mut inside = false;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        let (xi, yi) = pts[i];
        let (xj, yj) = pts[j];
        if (yi > p.1) != (yj > p.1) && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    *edges += pts.len();
    inside
}

/// True when the point lies inside the region (odd number of enclosing rings).
#[cfg(test)]
pub fn region_contains(region: &Region, p: Point) -> bool {
    region_contains_counted(region, p, &mut HitStats::default())
}

fn region_contains_counted(region: &Region, p: Point, stats: &mut HitStats) -> bool {
    if !region.bounds.contains(p) {
        return false;
    }
    stats.bbox_candidates += 1;
    let mut parity = false;
    for ring in &region.rings {
        if ring.bounds.contains(p) {
            stats.rings_tested += 1;
            if point_in_points(p, &ring.points, &mut stats.edges_visited) {
                parity = !parity;
            }
        }
    }
    parity
}

/// Topmost (last drawn) region containing `world`. `tolerance` (world units) lets markers and
/// regions smaller than the tolerance be picked despite being a few pixels wide.
pub fn hit_test(dataset: &MapDataset, world: Point, tolerance: f64) -> Option<usize> {
    hit_test_stats(dataset, world, tolerance).0
}

pub fn hit_test_stats(
    dataset: &MapDataset,
    world: Point,
    tolerance: f64,
) -> (Option<usize>, HitStats) {
    let mut stats = HitStats::default();
    if !(world.0.is_finite() && world.1.is_finite()) {
        return (None, stats);
    }
    // Markers first: microstate markers sit on or beside larger countries and must stay pickable.
    let mut nearest: Option<(usize, f64)> = None;
    for (i, region) in dataset.regions.iter().enumerate() {
        if let Some(marker) = region.marker {
            let d = (marker.0 - world.0).hypot(marker.1 - world.1);
            if d <= tolerance && nearest.map(|(_, best)| d < best).unwrap_or(true) {
                nearest = Some((i, d));
            }
        }
    }
    if let Some((i, _)) = nearest {
        return (Some(i), stats);
    }
    for (i, region) in dataset.regions.iter().enumerate().rev() {
        if region.marker.is_none() && region_contains_counted(region, world, &mut stats) {
            return (Some(i), stats);
        }
    }
    // Fallback: tiny regions (smaller than the tolerance) are pickable by proximity.
    let tiny = dataset.regions.iter().enumerate().rev().find(|(_, r)| {
        r.marker.is_none()
            && r.bounds.is_valid()
            && r.bounds.width().max(r.bounds.height()) < tolerance * 2.0
            && Bounds::expanded(&r.bounds, tolerance).contains(world)
    });
    (tiny.map(|(i, _)| i), stats)
}
