# Stage 11 — Maps / Difficult Rendering / Geometry Stress Spike

## Question

Can the current native architecture (Rust + Slint 1.17.1 + Winit + FemtoVG/OpenGL, renderer-independent core) handle Study Tracker's map / vector-geometry workloads with acceptable visual quality, interaction latency, memory, CPU behaviour and maintainability?

**Verdict: PASS WITH CONCERNS.** Everything here was measured on the Linux development machine (Arch, Hyprland/Wayland, HiDPI ×2, 90 Hz panel). **Nothing in this document is a Windows or macOS result.** The strategy since Stage 10 is "shared Rust core, Windows-first product, platform specialization only where measurements justify it"; Stage 12 must re-measure on Windows.

## Repository state note

The Stage 10 work was **not committed** when Stage 11 started (`HEAD` was still `764ad77`, Stage 9), so the worktree contained the whole Stage 10 change set (plus the sidebar fix) and Stage 11 is layered on top of it in the same uncommitted worktree. Nothing was committed or pushed.

## Production functionality inspected (read-only)

- `desktop/src/lib/travleMapData.ts` (155 KB) — `TRAVLE_MAP_COUNTRIES`: 195 entries `{ code, d?, point?, bounds }`; `d` is absolute-command SVG path data in a pre-projected 1000×500 viewBox (`TRAVLE_MAP_VIEWBOX`); `point` for 28 point-only microstates; `bounds` per country.
- `desktop/src/lib/countries.ts` (52 KB) — `COUNTRIES`: code, iso2, name, continent, region, population, area, etc.
- `desktop/src/lib/countryBorders.ts` — `COUNTRY_BORDERS`: adjacency lists used by Travle's route logic (not needed for rendering; not copied).
- `desktop/src/lib/travle.ts`, `geodle.ts` — game logic (daily puzzle, guess scoring); not ported.
- `desktop/src/App.tsx` (~lines 5247–5285, 15553–15600) — Travle map card: an inline `<svg viewBox>` with one `<path>` per country (class per guess state: route/possible/miss/start/target/current), `<circle r=3.6>` for point-only countries, a graticule (3 lines each way), a route `<polyline>` between country centres, an auto-fit viewBox around the route countries, and `+`/`−` buttons that only change a CSS zoom state (1 … 2.5). **Production has no drag-pan, no wheel zoom, no hit-testing on the map** (country picking is via a text field). This spike is therefore a superset of what production does today, chosen to stress the renderer.
- `desktop/src/App.css` (`.travle-map*`) — water/graticule/country/marker styles.

Nothing under `desktop/` was modified.

## Dataset

- **Source:** copied (via `scripts/extract-map-data.py`, which only *reads* production) from the two production files above into `assets/map/world-countries.tsv` (152,064 bytes, 195 rows, tab-separated: code, name, continent, subregion, population, area km², point x, point y, path).
- **Format:** original absolute `M x y L x y … Z` path data (multi-part countries are several `M…Z` sub-paths) in the production 1000×500 projected space; embedded with `include_str!`.
- **Provenance / licence: unknown.** The production files carry no attribution or licence header; the shapes are probably derived from a public-domain source such as Natural Earth, but that is **unverified**. Treat the copy as project-internal until the origin is confirmed before any distribution.
- **Preprocessing:** parse to rings (drop repeated closing points and degenerate rings), per-ring and per-region bounding boxes, label anchor (centroid of the largest ring, falling back to its bbox centre), and a world-space path string per region.
- **Counts (real data):** 195 regions = 167 polygon regions + 28 point markers; 273 rings (parts); 9,405 vertices (production's 9,682 "M/L tokens" includes repeated closing points); path text 168,146 bytes; 30 countries are multi-part; largest ring set is Canada (762 vertices); **exactly one nested ring: Lesotho inside South Africa** (a real enclave; used as the hole test).

## Architecture

```text
assets/map/world-countries.tsv
        ↓ include_str!
src/map/dataset.rs   parse, bounds, anchors, path strings, stress generators   (plain Rust)
src/map/viewport.rs  world↔screen, bounded zoom, clamped pan                   (plain Rust)
src/map/hit.rs       bbox filter + even-odd point-in-polygon                    (plain Rust)
src/map/mod.rs       MapModel: hover/selection/drag, labels, markers, layer     (plain Rust)
src/map_adapter.rs   Slint adapter: input → MapModel → properties               (Slint-facing)
ui/map-lab.slint     drawing + input forwarding only
crates/study-tracker-core   UNCHANGED (no geography or rendering concepts)
```

`MapModel` is owned by the adapter (`Rc<RefCell<MapController>>`), **not** by `AppModel`, so the big datasets are never cloned and `AppModel` keeps its `Clone/PartialEq` derive.

### Geometry representation

- `Region { code, name, continent, subregion, population, area_km2, rings: Vec<Ring>, marker: Option<Point>, bounds, label_anchor, tone, path }`.
- `Ring { points: Vec<(f64,f64)>, bounds }` — closed implicitly, no repeated last point, finite coordinates only, ≥ 3 points.
- **Multipolygons / islands:** one region holds several disjoint rings. **Holes / enclaves:** a ring nested in another ring. Both are handled by **even-odd parity across all rings of a region** (no orientation needed). Slint draws with `fill-rule: evenodd`, so what is drawn is what can be clicked.
- Point-only regions (28 microstates) have no rings; they are drawn as fixed-size screen-space dots and hit by distance.

## Rendering strategy

Path strings are **static in world space** and drawn once as one Slint `Path` per region inside a single 1000×500 "layer" `Rectangle`. Pan/zoom only change three properties of that layer (`x`, `y`, `transform-scale`); geometry is never regenerated. Hover and selection are two extra overlay `Path`s (their `commands` swap to the region's path string), so hovering never touches the per-region models. Labels and microstate markers are **screen-space** overlays computed in Rust (≤ 48 labels, greedy collision avoidance) so text stays crisp and un-scaled at every zoom. Border stroke width is `0.7px / scale` so borders keep a constant on-screen width.

### Where should the transform happen? (decision)

| Option | Cost per pan event | Verdict |
| --- | --- | --- |
| Rust re-projects every vertex and re-emits path strings | projecting is trivial (0.5 ms for 470k vertices), but building and re-parsing multi-MB path strings in Slint every event is not | rejected |
| **Slint layer transform (chosen)** | 3 property sets (~0.03 ms Rust+property work per tick, measured) | chosen |
| Custom renderer transform | not available through Slint's public API | n/a |

Note the lesson from Stage 10 still applies: Slint's `Path` scales its viewbox *uniformly*; here the layer is exactly 1000×500 with a 1000×500 viewbox so nothing is stretched.

## Pan / zoom model

- Viewport state is `(zoom, centre)` in `f64`; the pixel offset is always recomputed, never accumulated, so nothing drifts.
- Pan: `press` records the origin centre; each `move` sets `centre = origin − total_delta/scale`, so the result depends only on the total drag, not on how many events arrived. A press whose pointer travel stays ≤ 4 px is a click (selection); larger is a drag.
- Zoom: wheel (`exp(delta/120 × 0.14)` per notch) anchored at the cursor, buttons (+/−/⌂, ×1.5), keys `+ − 0`. Bounds 1× … 64× (relative to "world fits"). The centre is clamped inside the world so the map can never be lost off-screen; zooming fully out always restores the canonical view.
- Tests cover round trips, anchor stability, bounds, invalid input (NaN/∞/0/negative ignored), deterministic drag, and a 20,000-step randomized pan/zoom session (no drift, no NaN, always in bounds).

## Hit testing

Cursor → inverse viewport transform → world point → *markers first* (nearest within 7 px / scale) → regions in reverse draw order: region bbox test, per-ring bbox test, even-odd ray casting → fallback for regions smaller than the tolerance (bbox proximity). No spatial index: a linear scan over ≤ 2,000 bboxes is far below anything measurable, so none was built.

| Data set (release build, 5,000 grid probes) | avg | worst | avg bbox candidates | avg edges visited |
| --- | --- | --- | --- | --- |
| World (9.4k vertices) | 0.54 µs | 5.6 µs | 0.6 | 82 |
| Dense ×10 (94k) | 1.27 µs | 13.7 µs | 0.6 | 818 |
| Dense ×50 (470k) | 3.92 µs | 107.6 µs | 0.6 | 4,089 |
| Cells (2,000 regions) | 3.36 µs | 16.6 µs | 0.6 | 10 |
| Giant (1 ring, 50k vertices) | 34.9 µs | 83.1 µs | 0.9 | 43,240 |

Hover is limited by rendering, not by picking.

## Labels

Regions whose on-screen bbox is wide/tall enough for their name (`chars × 6.4 + 12` px estimated width) are candidates, sorted by on-screen area, placed greedily with rectangle collision rejection, clipped to the viewport, capped at 48. Zooming in reveals more names (Europe/Africa at ~4× shows ~35). Limitations: text width is estimated (not shaped), no curved/leader labels, anchors are area centroids so narrow or crescent countries (Norway, Chile, Italy) can look off-centre, labels for point markers are not shown (name appears in the hover/selected text).

## Stress levels and geometry counts

| Level | Regions | Rings | Vertices | Path bytes | Note |
| --- | --- | --- | --- | --- | --- |
| World (real) | 195 (28 markers) | 273 | 9,405 | 168 KB | the realistic workload |
| M1 Light | 195 | 273 | 1,273 | 23 KB | real data decimated ×10 |
| Dense ×3 | 195 | 273 | 28,215 | 503 KB | M2/M3 |
| Dense ×10 | 195 | 273 | 94,050 | 1.7 MB | M3 |
| Dense ×50 | 195 | 273 | 470,250 | 8.4 MB | M4 diagnostic |
| Cells | 2,000 | 2,000 | 32,000 | 569 KB | many small paths |
| Giant | 1 | 1 | 50,000 | 882 KB | one huge path |
| Empty / One / Tiny+huge / Multi+hole | 0 / 1 / 3 / 1 | — | — | — | edge cases |

Dense levels subdivide every edge with deterministic sideways jitter (coastline-like, not collinear).

## Geometry preparation (release build, single run each)

| Step | World | Dense ×3 | Dense ×10 | Dense ×50 | Cells | Giant |
| --- | --- | --- | --- | --- | --- | --- |
| Parse + bbox + anchors + path strings (or generate) | 2.2 ms | 8.2 ms | 22.0 ms | 100.7 ms | 8.0 ms | 11.0 ms |
| Project every vertex through the viewport | 0.009 ms | 0.034 ms | 0.171 ms | 0.532 ms | 0.116 ms | 0.046 ms |

`cargo test --release map_benchmark_report -- --ignored --nocapture` reproduces these. Plain wall-clock timings, single run; treat them as orders of magnitude.

## Interaction performance

### Continuous worst case: a changed transform on *every* frame

A test-only driver (`STUDY_NATIVE_MAP_BENCH=pan|zoom[:seconds[:zoom-notches]]`, finite, stops itself and exits) applies one pan or zoom step per 4 ms timer tick; frame rate comes from `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console` (the panel caps at ~90 fps). Sampled CPU is one core's share.

| Level (vertices) | fps pan / zoom | CPU pan / zoom | Rust+property work per tick |
| --- | --- | --- | --- |
| World (9.4k) | 90 / 90 (vsync cap) | 30.5% / 30.0% | 0.03 ms |
| Dense ×3 (28k) | 81 / 77 | 41% / 39% | 0.03 ms |
| Dense ×10 (94k) | 50 / 63 | 57% / 64% | 0.03 ms |
| Dense ×50 (470k) | **17 / 21** | 83% / 91% | 0.03 ms |
| Cells (2,000 paths, 32k) | 76 / 66 | 66% / 59% | 0.07–0.11 ms |
| Giant (1 path, 50k) | **30 / 29** | 45% / 34% | 0.01 ms |

Findings:
- The realistic dataset holds the display's full refresh rate while animating the transform every frame.
- Cost is dominated by FemtoVG per-frame tessellation, roughly linear in vertices. Static (unchanged-transform) frames are cheap at every level (90 fps even at 470k vertices), so the cost is paid only while the view changes.
- **One huge path is markedly worse per vertex than many paths** (50k-vertex single polygon ≈ 30 fps vs 94k vertices over 273 rings ≈ 50–63 fps).
- More of the geometry offscreen is cheaper: Dense ×10 pan gave 46 → 60 → 87 fps at zoom notches 8 → 24 → 32. FemtoVG therefore skips part of the offscreen work, but not all of it. **Rust-side viewport culling / zoom-dependent LOD is the obvious optimization if heavy data is ever required; it was not built** (real data never needs it).

### Real input (concurrent sampling, real mouse events through a virtual input device)

| Input | World | Dense ×10 |
| --- | --- | --- |
| Hover sweeps (rate limited by `hyprctl`, ~20–50 moves/s) | 0.67% | 0.83% |
| Drag-pan | 0.50% | 1.00% |
| Wheel zoom (~20 notches/s) | 7.8% | 25.3% |
| Settled after all of the above | **0.00%** | **0.00%** |

There is no permanent timer or animation loop; CPU returns to 0.00% as soon as input stops.

## Memory (release build, `/proc/<pid>/smaps_rollup`, MB)

Window 1150×700 floating; real-input rounds = hover sweeps, 4 click selections, 25-notch zoom, two drag pans, zoom out, keyboard PageDown ×3 + Enter, reset; executable 32,065,600 bytes (release, unstripped).

### World (realistic) — M0, M1, M2, M5 (same session)

| Point | RSS | **PSS** | Priv_Clean | Priv_Dirty | Shared_Clean | Shared_Dirty | CPU |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M0 open +5 s | 83.7 | **43.3** | 13.8 | 25.1 | 44.8 | 0.0 | 0.00% |
| M1 idle ~1 min | 83.7 | **43.3** | 13.8 | 25.1 | 44.8 | 0.0 | 0.00% |
| M2 after interaction ×1 | 84.4 | **43.9** | 13.8 | 25.7 | 44.8 | 0.0 | 0.00% |
| M2 ×2 | 84.5 | 44.0 | 13.8 | 25.8 | 44.8 | 0.0 | 0.00% |
| M2 ×3 | 84.5 | 44.0 | 13.8 | 25.8 | 44.8 | 0.0 | 0.00% |
| M5 → Dashboard (visits 1/2/3) | 85.6 / 85.7 / 85.8 | 44.6 / 44.6 / 44.8 | 14.0 | 25.8–26.0 | 45.9 | 0.0 | 0.00% |
| M5 back on Map (1/2/3) | 84.7 / 84.8 / 84.9 | 44.2 / 44.3 / 44.4 | 14.0 | 25.8–26.0 | 44.9 | 0.0 | 0.00% |
| M5 → Timer | 84.9 | 44.4 | 14.0 | 26.0 | 44.9 | 0.0 | 0.00% |

### Heavy data sets — M3 and M4

| Level | M3 open +5 s PSS | M4 after interaction ×1 / ×2 / ×3 PSS | Priv_Dirty (M4) | Settled CPU |
| --- | --- | --- | --- | --- |
| Dense ×10 (94k vertices) | 66.8 | 69.3 / 69.3 / 69.4 | 51.0 | 0.00% |
| Dense ×50 (470k vertices) | 159.6 | 163.9 / 164.0 / 164.0 | 145.8 | 0.00% |

Same-session comparison with the earlier views (Stage 10 numbers were taken on a different day; not mixed in): the map at open (43.3 MB) sits about where the Stage 10 dashboard and Timer controls did (40–44 MB) in comparable window sizes.

### Memory stability

- Repeated interaction (3 rounds), Map → Dashboard → Map ×3 and Timer: **flat** (±0.4 MB), no monotonic growth.
- Cycling all seven stress levels up to Dense ×50 and back to World, three full cycles: PSS after returning to World **175.4 → 173.4 → 173.5 MB** (baseline 44 MB). **Plateau, not a leak** — but it does not fall back after leaving the heavy data set; with `MALLOC_MMAP_THRESHOLD_`/`MALLOC_TRIM_THRESHOLD_=131072` it still settled at 141.2 MB (stable over 3 cycles). So glibc's dynamic mmap threshold explains only part of it. Working hypothesis (unverified): FemtoVG keeps its per-frame tessellation/vertex buffers at their high-water capacity. A realistic app that never loads ×50-class geometry never sees this, but a Stage 12 Windows run should check whether the same retention occurs there.

## Rendering quality (FemtoVG / OpenGL, HiDPI ×2)

Good: filled country polygons with even-odd fill, crisp constant-width borders at all zooms (compensated stroke width), no visible seams between neighbouring countries at 1×–8× zoom, smooth antialiased outlines, thin 0.7 px borders stay visible at fit zoom, overlays (hover, selection, markers, labels) crisp because they are drawn in screen space, no clipping errors at the viewport edge (rounded card clip works), no visible tearing or flicker while dragging.

Observed weaknesses: (a) tessellation cost grows linearly with vertices while the view changes (above); (b) a single very large path is disproportionately slow; (c) the selected-region fill is a translucent overlay, so on some continent colours it reads greyer than intended (a colour-tuning issue, not a renderer defect); (d) 1× (non-HiDPI) rendering was **not** tested; (e) extremely thin lines at maximum zoom were not exhaustively inspected; (f) Stage 9's emoji tofu is unchanged and irrelevant here.

## Renderer comparison

One bounded comparison was performed; two candidates were assessed and skipped. **No renderer winner is declared, and none of this transfers to Windows.**

What Slint 1.17.1 offers here (from its `Cargo.toml`): `renderer-femtovg` (current), `renderer-femtovg-wgpu`, `renderer-skia` / `-skia-opengl` / `-skia-vulkan`, `renderer-software`.

| Candidate | Decision | Reason |
| --- | --- | --- |
| **Software renderer** (`renderer-software`, `SLINT_BACKEND=winit-software`) | **Tested** (in a throwaway copy of the project under the scratchpad; the repository's `Cargo.toml`/`Cargo.lock` were not changed) | No new heavy crates; informative as a GPU-less baseline |
| FemtoVG on wgpu (`renderer-femtovg-wgpu`) | Skipped | Pulls the whole `wgpu` tree (dozens of new crates, long builds) and, on this Linux machine, would exercise Vulkan/GL, not the D3D12 backend that would matter for Windows. Better evaluated directly on Windows in Stage 12 |
| Skia (`renderer-skia*`) | Skipped | `skia-bindings` downloads a prebuilt multi-tens-of-MB Skia archive at build time (network dependency, large executable growth); again a Linux result would not inform Windows. Candidate for Stage 12 on Windows |

Software-renderer results (same machine, same map view, release build; the debug env var `SLINT_DEBUG_PERFORMANCE=refresh_full_speed` was used only for the fps figure):

| Metric | FemtoVG / OpenGL | Software |
| --- | --- | --- |
| Window mapped after launch (median of 3) | 0.32 s | 0.32 s |
| PSS at idle (map view, World) | 43.3 MB | **29.4 MB** (−14 MB; no GL driver state; 4 threads fewer) |
| Idle CPU | 0.00% | 0.00% |
| Executable | 32.1 MB | 33.6 MB (+1.6 MB) |
| Frame rate, full redraw of the map view | 90 fps (vsync cap) | **4–7 fps** (full-window dirty rect) |
| Visual result | correct | **map layer not scaled**: the paths draw, but `transform-scale` is ignored, so zoom breaks and the map no longer lines up with the Rust-placed labels/markers |

Conclusion: the software renderer is **not viable for the map workload as designed** (no item scale transform, ~15× slower full redraws). It is interesting only as a possible GPU-less/remote-desktop fallback for the ordinary (non-map) views on Windows, which is a Stage 12 question, and it would force a different map implementation (Rust-side re-projection of geometry) if ever used for maps.

**Candidates to evaluate in Stage 12 on real Windows hardware:** current FemtoVG/OpenGL (baseline on Windows drivers), FemtoVG-on-wgpu (D3D12), and Skia (D3D/ANGLE). Measure the same things as here: geometry-heavy pan/zoom fps with a per-frame changing transform, memory plateau after heavy geometry, HiDPI quality, startup, and executable size.

## Accessibility

- The map viewport is a focusable element (`accessible-role: image`) whose label states the data-set statistics and the currently selected region, and whose description lists the keyboard model.
- Every important value exists as text: the *Hover* and *Selected* lines under the map give name, ISO code, continent/subregion, population, area and geometry summary (`Germany (DEU) · Europe, Western Europe · pop. 82'905'782 · 357'022 km² · 1 part, 57 points`). Colour is only a continent hint; nothing depends on colour or geometry alone.
- Keyboard: arrows pan, `+`/`−` zoom, `0` reset, PageUp/PageDown select previous/next country alphabetically, Enter zooms to the selection (works when the selection is offscreen), `[` / `]` cycle stress data sets. Buttons (+, −, ⌂) and both pickers have labels/roles.
- Limitations: no real screen-reader test (Orca/NVDA/VoiceOver); the map itself is not a navigable structure; the label overlay is not exposed to accessibility; keyboard selection has no visible "cursor" apart from the selection highlight.
- A future accessible alternative would be a sortable/searchable **country list or table** bound to the same `MapModel` selection (name, continent, population, area), with selection synchronised both ways and live-region announcements of the selected country, plus adjacency ("borders: …") from `countryBorders.ts`.

## Windows-first implications

Shared Rust (keep): dataset parsing and preprocessing, `Region/Ring/Bounds`, `Viewport`, hit testing, `MapModel`, label placement, stress generators, all tests. These are platform-independent and were designed so.

Plausible platform specialisation (decide from Stage 12 measurements, not from this document): the renderer backend and GPU API (FemtoVG/OpenGL here; on Windows candidates include FemtoVG-on-wgpu (D3D12), Skia (D3D/ANGLE), or plain FemtoVG/OpenGL through ANGLE/WGL); font and text rasterisation (DirectWrite vs Fontconfig; matters for CJK labels and emoji, cf. Stage 9); window composition and HiDPI/fractional scaling; high-precision touchpad and wheel input (Windows precision touchpads produce different scroll deltas than the ×120 notches assumed here); accessibility (UI Automation vs AT-SPI); IME (TSF). Nothing was written for Windows and no `cfg(target_os)` branches were added.

## Unresolved issues and risks

1. **Heavy geometry is expensive while the view changes** (17–30 fps at 470k vertices or one 50k-vertex path). Real data is fine; production geography beyond ~100k vertices needs Rust-side culling/LOD.
2. **Memory does not return to baseline** after loading extreme geometry (plateau, not growth). Cause unverified.
3. Everything is Linux + OpenGL + Hyprland. Windows/macOS unmeasured; GPU driver behaviour and memory attribution will differ.
4. Only one renderer measured; the comparison below is limited.
5. The dataset's licence/provenance is undocumented in production.
6. Label placement is approximate; no curved labels or leaders.
7. Wheel/touchpad behaviour assumes wheel deltas of ~120 per notch; precision touchpads unverified.
8. No screen-reader verification; no non-map accessible alternative built.
9. 1× DPI rendering untested; map view window sizes below ~600 px not visually reviewed.
10. The map card's fixed 200 px of chrome makes the map short in very small windows (it scrolls, but the map area shrinks).

## Verification

Automated (Linux): `cargo fmt --check`, `cargo check`, `cargo test --workspace` (**19 core + 62 prototype tests pass**, 2 ignored manual benchmark reports: `preparation_cost_report` from Stage 10 and `map_benchmark_report`), `cargo test -p study-tracker-core` (19), `cargo build --release`, `./scripts/check.sh`, `git diff --check` — all pass. 27 of the prototype tests are new map tests: parsing (glued `Z`, multi-subpaths, garbage, degenerate rings, TSV errors), ring/bounds helpers, point-in-polygon (convex/concave), islands/holes/multipolygon parity, the real South Africa/Lesotho enclave, topmost/marker/tiny hit priority, invalid input, viewport round trips, cursor-anchored zoom and bounds, deterministic drag, a 20,000-step pan/zoom soak, `fit_bounds`, model click-vs-drag, hover/leave, alphabetical keyboard selection and focus of an offscreen selection, labels (bounded, visible, non-overlapping), layer transform, all edge-case levels, generator counts, and determinism.

Regression on Linux with real input events (virtual mouse/keyboard via `/dev/uinput`, a scratch tool kept outside the repository): Timer view opens; mode selection (Pomodoro → 25:00), Start (24:55 after ~5 s), Pause, Resume, Reset ("Ready 25:00"), progress ring; Stage 9 text view opens (same emoji tofu as before); Stage 10 dashboard opens with charts and cards intact; Map lab opens. One regression was found and fixed during this pass: adding the fourth view button made the switcher wider than the timer column and clipped "Map lab"; the buttons were narrowed (90/118/118/100 px). Windows/macOS: unverified.

## Test-only hooks

`STUDY_NATIVE_VIEW=map`, `STUDY_NATIVE_MAP_LEVEL=0..10`, `STUDY_NATIVE_MAP_BENCH=pan|zoom[:seconds[:zoom-notches]]` (self-terminating benchmark driver), `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console` (Slint's own frame counter), `scripts/extract-map-data.py`, `scripts/sample-linux.sh`.
