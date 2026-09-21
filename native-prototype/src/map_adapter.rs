//! Slint adapter for the Stage 11 map lab: forwards pointer/keyboard input to `MapModel` and
//! pushes the prepared view (layer transform, labels, markers, overlays, texts) back to Slint.
//!
//! Update paths (mirrors Stage 10): a data-set change rebuilds the regions model; every pan/zoom
//! only sets the layer transform and rebuilds the small label/marker models; hover/selection
//! only swaps two overlay path strings.

use crate::map::dataset::StressLevel;
use crate::map::MapModel;
use crate::{MainWindow, MapLabelData, MapMarkerData, MapRegionData};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

/// Adapter-side bookkeeping so unchanged overlays are not re-sent and timings can be shown.
#[derive(Default)]
struct Diagnostics {
    hovered: Option<usize>,
    selected: Option<usize>,
    last_hit_us: f64,
    last_view_ms: f64,
}

pub struct MapController {
    model: MapModel,
    diag: Diagnostics,
}

pub type SharedMap = Rc<RefCell<MapController>>;

pub fn new_controller(level: StressLevel) -> SharedMap {
    Rc::new(RefCell::new(MapController {
        model: MapModel::new(level),
        diag: Diagnostics::default(),
    }))
}

/// Full push: regions model + everything else. Called at startup and on data-set change.
pub fn apply_all(window: &MainWindow, map: &mut MapController) {
    let regions: Vec<MapRegionData> = map
        .model
        .dataset()
        .regions
        .iter()
        .filter(|r| !r.rings.is_empty())
        .map(|r| MapRegionData {
            commands: SharedString::from(r.path.as_str()),
            tone: r.tone,
        })
        .collect();
    window.set_map_regions(ModelRc::new(Rc::new(VecModel::from(regions))));
    window.set_map_level_index(map.model.level().index() as i32);
    window.set_map_level_name(map.model.level().label().into());
    let s = map.model.dataset().stats();
    window.set_map_stats_text(
        format!(
            "{} regions ({} point markers) · {} parts · {} points · {} KB path data",
            s.regions,
            s.marker_regions,
            s.rings,
            crate::map::group_thousands(s.vertices as u64),
            s.path_bytes / 1024
        )
        .into(),
    );
    map.diag.hovered = None;
    map.diag.selected = None;
    window.set_map_hover_commands("".into());
    window.set_map_selected_commands("".into());
    window.set_map_hover_text(map.model.describe(None).into());
    window.set_map_selected_text(map.model.describe(None).into());
    apply_view(window, map);
    apply_overlays(window, map);
}

/// Pan/zoom/resize update: layer transform, labels, markers and the readout line.
pub fn apply_view(window: &MainWindow, map: &mut MapController) {
    let start = Instant::now();
    let layer = map.model.layer();
    window.set_map_layer_x(layer.x as f32);
    window.set_map_layer_y(layer.y as f32);
    window.set_map_layer_scale(layer.scale as f32);

    let labels: Vec<MapLabelData> = map
        .model
        .labels()
        .into_iter()
        .map(|l| MapLabelData {
            text: l.text.into(),
            x: l.x as f32,
            y: l.y as f32,
            width: l.width as f32,
        })
        .collect();
    window.set_map_labels(ModelRc::new(Rc::new(VecModel::from(labels))));
    let markers: Vec<MapMarkerData> = map
        .model
        .markers()
        .into_iter()
        .map(|m| MapMarkerData {
            x: m.x as f32,
            y: m.y as f32,
            tone: m.tone,
        })
        .collect();
    window.set_map_markers(ModelRc::new(Rc::new(VecModel::from(markers))));

    map.diag.last_view_ms = start.elapsed().as_secs_f64() * 1000.0;
    window.set_map_zoom_text(format!("Zoom {:.1}×", map.model.viewport().zoom()).into());
    window.set_map_perf_text(
        format!(
            "last hit-test {:.1} µs · view update {:.2} ms",
            map.diag.last_hit_us, map.diag.last_view_ms
        )
        .into(),
    );
}

/// Hover / selection change: swaps the two overlay paths and the description texts.
pub fn apply_overlays(window: &MainWindow, map: &mut MapController) {
    let (hovered, selected) = (map.model.hovered(), map.model.selected());
    if hovered != map.diag.hovered {
        map.diag.hovered = hovered;
        let commands = hovered
            .and_then(|i| map.model.dataset().regions.get(i))
            .map(|r| r.path.as_str())
            .unwrap_or("");
        window.set_map_hover_commands(commands.into());
        window.set_map_hover_text(map.model.describe(hovered).into());
    }
    if selected != map.diag.selected {
        map.diag.selected = selected;
        let commands = selected
            .and_then(|i| map.model.dataset().regions.get(i))
            .map(|r| r.path.as_str())
            .unwrap_or("");
        window.set_map_selected_commands(commands.into());
        window.set_map_selected_text(map.model.describe(selected).into());
    }
}

pub fn bind(window: &MainWindow, map: &SharedMap) {
    // Wraps a callback so it upgrades the window, borrows the controller and repaints the view.
    macro_rules! on {
        ($setter:ident, |$w:ident, $m:ident $(, $arg:ident)*| $body:block) => {{
            let weak = window.as_weak();
            let map = Rc::clone(map);
            window.$setter(move |$($arg),*| {
                if let Some($w) = weak.upgrade() {
                    let mut guard = map.borrow_mut();
                    let $m: &mut MapController = &mut guard;
                    $body
                }
            });
        }};
    }

    on!(on_map_viewport_resized, |w, m, width, height| {
        m.model.resize(width as f64, height as f64);
        apply_view(&w, m);
    });
    on!(on_map_press, |w, m| {
        let _ = &w;
        m.model.press();
    });
    on!(on_map_drag, |w, m, dx, dy| {
        m.model.drag(dx as f64, dy as f64);
        apply_view(&w, m);
    });
    on!(on_map_release, |w, m, x, y| {
        if m.model.release() {
            m.model.click(x as f64, y as f64);
            apply_overlays(&w, m);
        }
    });
    on!(on_map_hover, |w, m, x, y| {
        let start = Instant::now();
        m.model.hover(x as f64, y as f64);
        m.diag.last_hit_us = start.elapsed().as_secs_f64() * 1e6;
        apply_overlays(&w, m);
    });
    on!(on_map_leave, |w, m| {
        m.model.leave();
        apply_overlays(&w, m);
    });
    on!(on_map_wheel, |w, m, delta, x, y| {
        m.model.zoom_wheel(delta as f64, x as f64, y as f64);
        apply_view(&w, m);
    });
    on!(on_map_zoom_step, |w, m, direction| {
        m.model.zoom_step(direction);
        apply_view(&w, m);
    });
    on!(on_map_reset_view, |w, m| {
        m.model.reset_view();
        apply_view(&w, m);
    });
    on!(on_map_pan_key, |w, m, dx, dy| {
        m.model.pan_pixels(dx as f64, dy as f64);
        apply_view(&w, m);
    });
    on!(on_map_step_selection, |w, m, delta| {
        m.model.step_selection(delta);
        apply_overlays(&w, m);
    });
    on!(on_map_focus_selected, |w, m| {
        m.model.focus_selected();
        apply_view(&w, m);
    });
    on!(on_map_cycle_level, |w, m, delta| {
        // Cycles the seven stress data sets (World..Giant); edge cases are reached via the picker.
        let next = (m.model.level().index() as i32 + delta).rem_euclid(7);
        m.model.set_level(StressLevel::from_index(next as usize));
        apply_all(&w, m);
    });
    on!(on_map_set_level, |w, m, index| {
        m.model
            .set_level(StressLevel::from_index(index.max(0) as usize));
        apply_all(&w, m);
    });
}

/// Opt-in benchmark driver (`STUDY_NATIVE_MAP_BENCH=pan|zoom[:seconds[:zoom-notches]]`): applies one pan or zoom
/// step per timer tick for a bounded time so every frame has a changed transform. It exists only to
/// measure render cost per frame (pair it with `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console`)
/// and stops itself; there is no permanent animation loop.
pub fn start_bench(window: &MainWindow, map: &SharedMap, spec: &str) -> slint::Timer {
    // spec = mode[:seconds[:initial-zoom-wheel-notches]]
    let mut parts = spec.split(':');
    let mode = parts.next().unwrap_or("pan").to_string();
    let seconds = parts
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(12);
    let notches = parts
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(8.0);
    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let map = Rc::clone(map);
    let started = Instant::now();
    let mut tick = 0u32;
    let mut work_ns = 0u128;
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(4),
        move || {
            let Some(window) = weak.upgrade() else { return };
            let mut guard = map.borrow_mut();
            let m: &mut MapController = &mut guard;
            let t0 = Instant::now();
            if tick == 0 {
                m.model.zoom_wheel(120.0 * notches, 300.0, 200.0); // default ~3x zoomed
            }
            let phase = tick as f64 / 60.0;
            match mode.as_str() {
                "zoom" => {
                    let dir = if (tick / 90) % 2 == 0 { 1.0 } else { -1.0 };
                    m.model
                        .zoom_wheel(dir * 60.0, 300.0 + 100.0 * phase.sin(), 200.0);
                }
                _ => m
                    .model
                    .pan_pixels(9.0 * phase.cos(), 5.0 * (phase * 0.7).sin()),
            }
            apply_view(&window, m);
            work_ns += t0.elapsed().as_nanos();
            tick += 1;
            if started.elapsed().as_secs() >= seconds {
                eprintln!(
                "bench {mode}: {tick} ticks in {seconds}s, Rust+property work avg {:.3} ms/tick",
                work_ns as f64 / tick as f64 / 1.0e6
            );
                window.set_map_hover_text(format!("bench finished: {tick} ticks").into());
                std::process::exit(0);
            }
        },
    );
    timer
}
