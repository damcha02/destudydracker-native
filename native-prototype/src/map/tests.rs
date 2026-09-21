use super::dataset::{parse_path, MapDataset, MapError, StressLevel};
use super::hit::{hit_test, hit_test_stats, point_in_ring, region_contains};
use super::viewport::{Viewport, MAX_ZOOM, MIN_ZOOM};
use super::*;
use std::time::Instant;

fn square(x: f64, y: f64, s: f64) -> Ring {
    Ring::new(vec![(x, y), (x + s, y), (x + s, y + s), (x, y + s)]).unwrap()
}

fn region(rings: Vec<Ring>) -> Region {
    Region::build(
        "T".into(),
        "Test".into(),
        "X".into(),
        String::new(),
        1,
        1,
        rings,
        None,
        0,
    )
}

fn by_code<'a>(d: &'a MapDataset, code: &str) -> (usize, &'a Region) {
    d.regions
        .iter()
        .enumerate()
        .find(|(_, r)| r.code == code)
        .unwrap()
}

fn assert_finite_geometry(d: &MapDataset) {
    for r in &d.regions {
        assert!(
            !r.path.contains("NaN") && !r.path.contains("inf"),
            "{}",
            r.code
        );
        for ring in &r.rings {
            assert!(ring.points.len() >= 3);
            for (x, y) in &ring.points {
                assert!(x.is_finite() && y.is_finite());
                assert!(
                    (-1.0..=1001.0).contains(x) && (-1.0..=501.0).contains(y),
                    "{} ({x},{y})",
                    r.code
                );
            }
        }
        assert!((0..5).contains(&r.tone));
    }
}

// --- parsing ---------------------------------------------------------------------------------

#[test]
fn bundled_world_dataset_parses_with_expected_shape() {
    let d = MapDataset::world();
    let s = d.stats();
    assert_eq!(s.regions, 195);
    assert_eq!(s.marker_regions, 28);
    assert_eq!(s.regions - s.marker_regions, 167);
    assert_eq!(s.rings, 273, "rings {} stats {s:?}", s.rings);
    assert_eq!(s.vertices, 9_405, "vertices {} stats {s:?}", s.vertices);
    assert_finite_geometry(&d);
    let multi = d.regions.iter().filter(|r| r.rings.len() > 1).count();
    assert!(multi >= 25, "multipart countries {multi}");
}

#[test]
fn parse_path_handles_multi_subpaths_and_glued_commands() {
    let rings = parse_path("M0 0 L10 0 L10 10 L0 10Z M20 20 L30 20 L30 30Z").unwrap();
    assert_eq!(rings.len(), 2);
    assert_eq!(rings[0].points.len(), 4);
    assert_eq!(rings[1].points.len(), 3);
    // Closing point repeated, unterminated ring, comma separators.
    let rings = parse_path("M0,0 L5,0 L5,5 L0,0").unwrap();
    assert_eq!(rings.len(), 1);
    assert_eq!(rings[0].points.len(), 3);
}

#[test]
fn parse_path_rejects_garbage_and_drops_degenerate_rings() {
    assert!(parse_path("M0 0 C1 1 2 2 3 3").is_err());
    assert!(parse_path("M0 abc").is_err());
    assert!(parse_path("M0").is_err());
    assert!(parse_path("").unwrap().is_empty());
    assert!(parse_path("M0 0 L1 1Z").unwrap().is_empty());
    assert!(parse_path("M0 0 L0 0 L0 0Z").unwrap().is_empty());
}

#[test]
fn tsv_errors_report_the_line() {
    let err = MapDataset::from_tsv("A\tB\tC").unwrap_err();
    assert!(matches!(err, MapError::BadLine { line: 1, .. }));
    let ok = "AAA\tA\tEurope\tR\t1\t1\t-\t-\tM0 0 L10 0 L10 10Z\n\nBBB\tB\tAsia\t\t1\t1\t5\t6\t";
    let d = MapDataset::from_tsv(ok).unwrap();
    assert_eq!(d.regions.len(), 2);
    assert_eq!(d.regions[1].marker, Some((5.0, 6.0)));
    assert!(MapDataset::from_tsv("AAA\tA\tEurope\tR\t1\t1\t-\t-\t").is_err());
}

#[test]
fn ring_normalisation() {
    assert!(Ring::new(vec![(0.0, 0.0), (1.0, 1.0)]).is_none());
    assert!(Ring::new(vec![
        (0.0, 0.0),
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
        (2.0, 2.0)
    ])
    .is_none());
    let r = Ring::new(vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 0.0)]).unwrap();
    assert_eq!(r.points.len(), 3);
    assert_eq!(
        r.bounds,
        Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 4.0,
            max_y: 4.0
        }
    );
}

#[test]
fn bounds_helpers() {
    let mut b = Bounds::EMPTY;
    assert!(!b.is_valid());
    assert_eq!(b.width(), 0.0);
    b.extend((1.0, 2.0));
    b.extend((5.0, 3.0));
    assert!(b.contains((3.0, 2.5)) && !b.contains((6.0, 2.5)));
    assert!(b.intersects(&Bounds {
        min_x: 4.0,
        min_y: 0.0,
        max_x: 9.0,
        max_y: 9.0
    }));
    assert!(!b.intersects(&Bounds {
        min_x: 6.0,
        min_y: 0.0,
        max_x: 9.0,
        max_y: 9.0
    }));
    assert_eq!(b.expanded(1.0).min_x, 0.0);
    let mut u = Bounds::EMPTY;
    u.union(&Bounds::EMPTY);
    assert!(!u.is_valid());
}

// --- hit testing -----------------------------------------------------------------------------

#[test]
fn point_in_ring_convex_and_concave() {
    let sq = square(0.0, 0.0, 10.0);
    assert!(point_in_ring((5.0, 5.0), &sq));
    assert!(!point_in_ring((11.0, 5.0), &sq));
    // "U" shape: the notch is outside.
    let u = Ring::new(vec![
        (0.0, 0.0),
        (9.0, 0.0),
        (9.0, 9.0),
        (6.0, 9.0),
        (6.0, 3.0),
        (3.0, 3.0),
        (3.0, 9.0),
        (0.0, 9.0),
    ])
    .unwrap();
    assert!(point_in_ring((1.0, 5.0), &u));
    assert!(!point_in_ring((4.5, 6.0), &u));
    assert!(point_in_ring((4.5, 1.0), &u));
}

#[test]
fn multipolygon_islands_and_holes_use_even_odd() {
    let d = MapDataset::for_level(StressLevel::MultiWithHole);
    let r = &d.regions[0];
    assert_eq!(r.rings.len(), 5);
    assert!(
        region_contains(r, (110.0, 110.0)),
        "solid part of the outer ring"
    );
    assert!(!region_contains(r, (200.0, 200.0)), "inside the hole");
    assert!(region_contains(r, (430.0, 150.0)), "disconnected island");
    assert!(region_contains(r, (704.0, 204.0)), "tiny island");
    assert!(!region_contains(r, (600.0, 400.0)), "water between parts");
    assert_eq!(hit_test(&d, (200.0, 200.0), 0.0), None);
    assert_eq!(hit_test(&d, (110.0, 110.0), 0.0), Some(0));
}

#[test]
fn real_data_hole_south_africa_contains_lesotho() {
    // The production dataset draws Lesotho as a ring nested inside South Africa's outline.
    let d = MapDataset::world();
    let (zaf_i, zaf) = by_code(&d, "ZAF");
    let (lso_i, lso) = by_code(&d, "LSO");
    assert!(zaf.rings.len() >= 2, "ZAF should carry the enclave ring");
    let p = lso.label_anchor;
    assert!(
        !region_contains(zaf, p),
        "even-odd: inside the nested ring = outside South Africa"
    );
    assert!(region_contains(lso, p));
    assert_eq!(hit_test(&d, p, 0.0), Some(lso_i));
    assert_ne!(hit_test(&d, p, 0.0), Some(zaf_i));
    // A point in South Africa proper still hits South Africa.
    let south_africa_proper = (zaf.bounds.min_x + 5.0, zaf.bounds.center().1);
    if region_contains(zaf, south_africa_proper) {
        assert_eq!(hit_test(&d, south_africa_proper, 0.0), Some(zaf_i));
    }
}

#[test]
fn hit_test_prefers_topmost_and_handles_markers_tiny_and_invalid_input() {
    let d = MapDataset::for_level(StressLevel::TinyAndLarge);
    // Tiny polygon sits on top of the huge one.
    assert_eq!(
        d.regions[hit_test(&d, (500.03, 250.02), 0.0).unwrap()].code,
        "TNY"
    );
    // Just outside the tiny polygon but inside the huge one: huge wins unless tolerance picks tiny.
    assert_eq!(
        d.regions[hit_test(&d, (500.5, 250.5), 0.0).unwrap()].code,
        "BIG"
    );
    // Tiny fallback: a wide tolerance picks the tiny polygon via proximity.
    assert!(hit_test(&d, (500.5, 250.5), 2.0).is_some());
    assert_eq!(hit_test(&d, (f64::NAN, 1.0), 1.0), None);
    assert_eq!(
        hit_test(&MapDataset { regions: vec![] }, (1.0, 1.0), 1.0),
        None
    );

    let world = MapDataset::world();
    let (i, r) = world
        .regions
        .iter()
        .enumerate()
        .find(|(_, r)| r.marker.is_some())
        .unwrap();
    let m = r.marker.unwrap();
    assert_eq!(hit_test(&world, (m.0 + 1.0, m.1), 2.0), Some(i));
}

#[test]
fn real_countries_are_hit_at_their_anchor() {
    let d = MapDataset::world();
    let mut model = MapModel::new(StressLevel::World);
    model.resize(1000.0, 500.0); // scale 1: screen == world
    for code in ["DEU", "BRA", "AUS", "USA", "JPN", "ZAF"] {
        let (i, r) = by_code(&d, code);
        let hit = hit_test(&d, r.label_anchor, 0.0);
        assert_eq!(hit, Some(i), "{code} anchor {:?}", r.label_anchor);
        assert_eq!(
            model.pick(r.label_anchor.0, r.label_anchor.1),
            Some(i),
            "{code} via viewport"
        );
    }
}

#[test]
fn hit_stats_count_work_and_filter_by_bbox() {
    let d = MapDataset::world();
    let (hit, stats) = hit_test_stats(&d, (500.0, 490.0), 0.0); // open ocean / Antarctica edge
    let _ = hit;
    assert!(
        stats.bbox_candidates < 30,
        "bbox filter should prune most of 195 regions: {stats:?}"
    );
    let (_, ocean) = hit_test_stats(&d, (5.0, 5.0), 0.0);
    assert_eq!(ocean, Default::default());
}

// --- viewport --------------------------------------------------------------------------------

#[test]
fn viewport_round_trip_and_fit() {
    let v = Viewport::new(800.0, 400.0);
    assert_eq!(v.zoom(), MIN_ZOOM);
    assert!((v.scale() - 0.8).abs() < 1e-12);
    for p in [(0.0, 0.0), (500.0, 250.0), (999.0, 499.0), (123.4, 56.7)] {
        let back = v.screen_to_world(v.world_to_screen(p));
        assert!((back.0 - p.0).abs() < 1e-9 && (back.1 - p.1).abs() < 1e-9);
    }
    // Wider viewport: the world stays fully visible and centred at zoom 1.
    let wide = Viewport::new(1600.0, 400.0);
    assert!((wide.scale() - 0.8).abs() < 1e-12);
    assert_eq!(wide.world_to_screen((500.0, 250.0)), (800.0, 200.0));
}

#[test]
fn zoom_is_bounded_and_anchored_at_the_cursor() {
    let mut v = Viewport::new(800.0, 400.0);
    let cursor = (300.0, 150.0);
    let anchor = v.screen_to_world(cursor);
    v.zoom_at(cursor, 4.0);
    let after = v.world_to_screen(anchor);
    assert!(
        (after.0 - cursor.0).abs() < 1e-6 && (after.1 - cursor.1).abs() < 1e-6,
        "{after:?}"
    );
    for _ in 0..50 {
        v.zoom_at(cursor, 3.0);
    }
    assert_eq!(v.zoom(), MAX_ZOOM);
    for _ in 0..100 {
        v.zoom_at(cursor, 0.2);
    }
    assert_eq!(v.zoom(), MIN_ZOOM);
    let c = v.center();
    assert_eq!(c, (500.0, 250.0), "fully zoomed out re-centres the world");
    // Invalid input is ignored.
    let before = v;
    for bad in [f64::NAN, f64::INFINITY, 0.0, -2.0] {
        v.zoom_at(cursor, bad);
    }
    v.zoom_at((f64::NAN, 1.0), 2.0);
    assert_eq!(v, before);
}

#[test]
fn pan_is_clamped_and_deterministic() {
    let mut v = Viewport::new(800.0, 400.0);
    v.zoom_at((400.0, 200.0), 8.0);
    let origin = v.center();
    v.pan_from(origin, -100.0, 60.0);
    let a = v.center();
    // Same total delta reached via a different path gives the identical centre (no accumulation).
    let mut w = Viewport::new(800.0, 400.0);
    w.zoom_at((400.0, 200.0), 8.0);
    for d in [(-20.0, 10.0), (-90.0, 70.0), (-100.0, 60.0)] {
        w.pan_from(origin, d.0, d.1);
    }
    assert_eq!(a, w.center());
    // Aggressive pans can never leave the world.
    v.pan_from(origin, 1.0e9, -1.0e9);
    let c = v.center();
    assert!((0.0..=1000.0).contains(&c.0) && (0.0..=500.0).contains(&c.1));
    v.pan_from(origin, f64::NAN, 1.0);
    assert_eq!(v.center(), c);
}

#[test]
fn long_pan_zoom_sessions_do_not_drift_or_explode() {
    let mut v = Viewport::new(640.0, 320.0);
    let mut seed = 1u64;
    let mut next = || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    for _ in 0..20_000 {
        let cursor = (next() * 640.0, next() * 320.0);
        v.zoom_at(cursor, 0.5 + next() * 1.5);
        let c = v.center();
        v.pan_from(c, (next() - 0.5) * 300.0, (next() - 0.5) * 300.0);
        let (x, y) = v.center();
        assert!(x.is_finite() && y.is_finite() && v.scale().is_finite() && v.scale() > 0.0);
        assert!((0.0..=1000.0).contains(&x) && (0.0..=500.0).contains(&y));
        assert!((MIN_ZOOM..=MAX_ZOOM).contains(&v.zoom()));
    }
    // Zooming all the way out always restores the canonical view.
    for _ in 0..200 {
        v.zoom_at((10.0, 10.0), 0.5);
    }
    assert_eq!((v.zoom(), v.center()), (1.0, (500.0, 250.0)));
}

#[test]
fn fit_bounds_and_resize() {
    let mut v = Viewport::new(800.0, 400.0);
    v.fit_bounds(&Bounds {
        min_x: 100.0,
        min_y: 100.0,
        max_x: 140.0,
        max_y: 120.0,
    });
    assert!(v.zoom() > 5.0);
    let s = v.world_to_screen((120.0, 110.0));
    assert!((s.0 - 400.0).abs() < 1.0 && (s.1 - 200.0).abs() < 1.0);
    v.fit_bounds(&Bounds::EMPTY);
    v.fit_bounds(&Bounds {
        min_x: 5.0,
        min_y: 5.0,
        max_x: 5.0,
        max_y: 5.0,
    }); // degenerate point
    assert!(v.zoom().is_finite() && v.zoom() <= MAX_ZOOM);
    v.resize(0.0, -5.0);
    assert!(v.size().0 >= 16.0 && v.scale() > 0.0);
    v.resize(f64::NAN, 100.0);
    assert!(v.size().0.is_finite());
}

// --- model -----------------------------------------------------------------------------------

#[test]
fn click_selects_drag_pans_and_outside_clicks_clear() {
    let mut m = MapModel::new(StressLevel::World);
    m.resize(1000.0, 500.0);
    let (de, r) = by_code(m.dataset(), "DEU");
    let (x, y) = r.label_anchor;
    m.press();
    assert!(m.release(), "no movement = click");
    m.click(x, y);
    assert_eq!(m.selected(), Some(de));
    m.click(-50.0, -50.0);
    assert_eq!(m.selected(), None, "click outside the map clears selection");
    m.click(f64::NAN, 3.0);
    assert_eq!(m.selected(), None);

    m.zoom_step(1);
    m.zoom_step(1);
    let before = m.viewport().center();
    m.press();
    m.drag(60.0, 30.0);
    m.drag(-30.0, 15.0);
    assert!(!m.release(), "moved beyond the slop = drag, not click");
    let after = m.viewport().center();
    assert_ne!(before, after);
    // Drag after release does nothing.
    m.drag(500.0, 500.0);
    assert_eq!(m.viewport().center(), after);
}

#[test]
fn hover_and_leave() {
    let mut m = MapModel::new(StressLevel::World);
    m.resize(1000.0, 500.0);
    let (br, r) = by_code(m.dataset(), "BRA");
    m.hover(r.label_anchor.0, r.label_anchor.1);
    assert_eq!(m.hovered(), Some(br));
    m.leave();
    assert_eq!(m.hovered(), None);
    m.hover(1.0e9, 1.0e9);
    assert_eq!(m.hovered(), None);
}

#[test]
fn keyboard_selection_is_alphabetical_and_focus_reveals_offscreen_regions() {
    let mut m = MapModel::new(StressLevel::World);
    m.resize(800.0, 400.0);
    m.step_selection(1);
    let first = m.dataset().regions[m.selected().unwrap()].name.clone();
    assert_eq!(first, "Afghanistan");
    m.step_selection(-1);
    assert_eq!(
        m.dataset().regions[m.selected().unwrap()].name,
        "Afghanistan",
        "clamped at start"
    );
    for _ in 0..10 {
        m.step_selection(1);
    }
    // Zoom far into another part of the world, then focus the (now offscreen) selection.
    m.zoom_wheel(-120.0 * 30.0, 700.0, 300.0);
    m.reset_view();
    m.zoom_at_screen_for_test(700.0, 300.0);
    let sel = m.selected().unwrap();
    m.focus_selected();
    let b = m.dataset().regions[sel].bounds;
    let (cx, cy) = m.viewport().world_to_screen(b.center());
    assert!((0.0..=800.0).contains(&cx) && (0.0..=400.0).contains(&cy));
}

impl MapModel {
    fn zoom_at_screen_for_test(&mut self, x: f64, y: f64) {
        for _ in 0..8 {
            self.zoom_wheel(120.0 * 5.0, x, y);
        }
    }
}

#[test]
fn labels_are_bounded_visible_and_non_overlapping() {
    let mut m = MapModel::new(StressLevel::World);
    m.resize(1000.0, 500.0);
    let labels = m.labels();
    assert!(!labels.is_empty() && labels.len() <= 48);
    for l in &labels {
        assert!(l.x >= 0.0 && l.y >= 0.0 && l.x + l.width <= 1000.0 && l.y + 16.0 <= 500.0);
    }
    for (i, a) in labels.iter().enumerate() {
        for b in &labels[i + 1..] {
            let overlap =
                a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + 16.0 && a.y + 16.0 > b.y;
            assert!(!overlap, "{} overlaps {}", a.text, b.text);
        }
    }
    // Zooming in on Europe reveals more (smaller) names than the world view has room for there.
    let world_count = labels.len();
    m.zoom_wheel(120.0 * 20.0, 500.0, 120.0);
    let zoomed = m.labels();
    assert!(
        zoomed.len() >= 3 && zoomed.len() <= 48,
        "world {world_count}, zoomed {}",
        zoomed.len()
    );
    assert_eq!(m.markers().iter().filter(|k| k.x.is_nan()).count(), 0);
}

#[test]
fn layer_transform_matches_viewport() {
    let mut m = MapModel::new(StressLevel::World);
    m.resize(1000.0, 500.0);
    let l = m.layer();
    assert_eq!((l.x, l.y, l.scale), (0.0, 0.0, 1.0));
    m.zoom_step(1);
    let l = m.layer();
    // The layer's centre lands where the viewport puts world (500, 250).
    let c = m.viewport().world_to_screen((500.0, 250.0));
    assert!((l.x + 500.0 - c.0).abs() < 1e-9 && (l.y + 250.0 - c.1).abs() < 1e-9);
    assert!((l.scale - 1.5).abs() < 1e-9);
}

#[test]
fn edge_case_levels_do_not_panic() {
    for level in StressLevel::ALL {
        if level == StressLevel::Dense50 {
            continue; // covered by the ignored benchmark (slow in debug builds)
        }
        let mut m = MapModel::new(level);
        m.resize(640.0, 320.0);
        assert_finite_geometry(m.dataset());
        m.hover(320.0, 160.0);
        m.click(320.0, 160.0);
        m.step_selection(1);
        m.focus_selected();
        m.zoom_wheel(-1.0e6, 10.0, 10.0);
        m.zoom_wheel(1.0e6, 10.0, 10.0);
        m.pan_pixels(1.0e12, -1.0e12);
        let _ = (
            m.labels(),
            m.markers(),
            m.describe(m.selected()),
            m.describe(m.hovered()),
        );
        assert!(m.layer().scale.is_finite() && m.layer().x.is_finite());
    }
    let empty = MapModel::new(StressLevel::Empty);
    assert_eq!(empty.describe(None), "No region");
    assert!(empty.dataset().stats() == Default::default());
}

// --- generators ------------------------------------------------------------------------------

#[test]
fn stress_generators_have_documented_counts() {
    let world = MapDataset::world().stats();
    let cells = MapDataset::cells(2_000, 16).stats();
    assert_eq!(
        (cells.regions, cells.rings, cells.vertices),
        (2_000, 2_000, 32_000)
    );
    let giant = MapDataset::giant(50_000).stats();
    assert_eq!((giant.regions, giant.rings, giant.vertices), (1, 1, 50_000));
    let light = MapDataset::world().decimated(10).stats();
    assert!(
        light.vertices < world.vertices / 6 && light.vertices > 900,
        "light {}",
        light.vertices
    );
    let d3 = MapDataset::world().densified(3).stats();
    assert!(
        d3.vertices >= world.vertices * 29 / 10 && d3.vertices <= world.vertices * 3,
        "{}",
        d3.vertices
    );
    let d10 = MapDataset::for_level(StressLevel::Dense10);
    assert!(d10.stats().vertices > world.vertices * 9);
    assert_finite_geometry(&d10);
    assert_finite_geometry(&MapDataset::cells(2_000, 16));
    assert_finite_geometry(&MapDataset::giant(50_000));
    assert_eq!(
        MapDataset::for_level(StressLevel::Dense3),
        MapDataset::for_level(StressLevel::Dense3),
        "deterministic"
    );
}

#[test]
fn giant_and_cells_are_hit_testable() {
    let giant = MapDataset::giant(50_000);
    assert_eq!(hit_test(&giant, (500.0, 250.0), 0.0), Some(0));
    assert_eq!(hit_test(&giant, (5.0, 5.0), 0.0), None);
    let (hit, stats) = hit_test_stats(&giant, (500.0, 250.0), 0.0);
    assert_eq!(hit, Some(0));
    assert_eq!(stats.edges_visited, 50_000);
    let cells = MapDataset::cells(2_000, 16);
    let center = cells.regions[777].label_anchor;
    assert_eq!(hit_test(&cells, center, 0.0), Some(777));
}

#[test]
fn stress_level_index_round_trips() {
    for l in StressLevel::ALL {
        assert_eq!(StressLevel::from_index(l.index()), l);
        assert!(!l.label().is_empty());
    }
    assert_eq!(StressLevel::from_index(99), StressLevel::World);
}

#[test]
fn describe_and_group_thousands() {
    assert_eq!(group_thousands(0), "0");
    assert_eq!(group_thousands(999), "999");
    assert_eq!(group_thousands(1_000), "1'000");
    assert_eq!(group_thousands(83_240_525), "83'240'525");
    let m = MapModel::new(StressLevel::World);
    let (i, _) = by_code(m.dataset(), "DEU");
    let text = m.describe(Some(i));
    assert!(
        text.contains("Germany") && text.contains("Europe") && text.contains("DEU"),
        "{text}"
    );
}

// --- manual benchmark ------------------------------------------------------------------------

/// `cargo test --release map_benchmark_report -- --ignored --nocapture`
#[test]
#[ignore]
fn map_benchmark_report() {
    fn time<T>(f: impl FnOnce() -> T) -> (T, f64) {
        let t = Instant::now();
        let v = f();
        (v, t.elapsed().as_secs_f64() * 1000.0)
    }
    let (world, parse_ms) = time(MapDataset::world);
    println!(
        "parse+bbox+anchor+path (real world): {parse_ms:.3} ms  {:?}",
        world.stats()
    );
    for level in StressLevel::ALL {
        let (d, build_ms) = time(|| MapDataset::for_level(level));
        let s = d.stats();
        // Cost of re-projecting every vertex through the viewport (what per-frame Rust regeneration would pay).
        let mut vp = Viewport::new(1000.0, 500.0);
        vp.zoom_at((300.0, 200.0), 3.0);
        let (sum, project_ms) = time(|| {
            let mut acc = 0.0;
            for r in &d.regions {
                for ring in &r.rings {
                    for p in &ring.points {
                        let q = vp.world_to_screen(*p);
                        acc += q.0 + q.1;
                    }
                }
            }
            acc
        });
        std::hint::black_box(sum);
        // Hit testing over a deterministic 100x50 grid of points.
        let mut total_edges = 0usize;
        let mut total_candidates = 0usize;
        let mut worst_us = 0.0f64;
        let mut hits = 0usize;
        let start = Instant::now();
        for gy in 0..50 {
            for gx in 0..100 {
                let p = (gx as f64 * 10.0 + 5.0, gy as f64 * 10.0 + 5.0);
                let t = Instant::now();
                let (hit, st) = hit_test_stats(&d, p, 7.0);
                worst_us = worst_us.max(t.elapsed().as_secs_f64() * 1e6);
                total_edges += st.edges_visited;
                total_candidates += st.bbox_candidates;
                hits += hit.is_some() as usize;
            }
        }
        let avg_us = start.elapsed().as_secs_f64() * 1e6 / 5000.0;
        let mut m = MapModel::new(StressLevel::Empty);
        std::hint::black_box(&mut m);
        println!(
            "{:<26} regions={:>5} rings={:>5} verts={:>7} path={:>8}B | build {build_ms:>9.3} ms | project-all {project_ms:>8.3} ms | hit avg {avg_us:>8.2} µs worst {worst_us:>9.1} µs (avg bbox cand {:.1}, edges {:.0}, hit-rate {:.0}%)",
            level.label(), s.regions, s.rings, s.vertices, s.path_bytes,
            total_candidates as f64 / 5000.0, total_edges as f64 / 5000.0, hits as f64 / 50.0
        );
    }
}
