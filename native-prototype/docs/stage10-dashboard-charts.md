# Stage 10 — Realistic Dashboard / Statistics / Charts Spike

## Question

Can the native architecture (Slint 1.17.1 + Winit + FemtoVG + renderer-independent Rust) produce a polished, data-dense Study Tracker dashboard with useful interactive charts at acceptable memory, CPU, rendering quality, responsiveness and maintainability?

**Verdict: PASS WITH CONCERNS** (see "Unresolved issues").

## Starting state

The task brief said the worktree would be clean. It was not: the previous agent had left ~870 lines of *uncommitted, partial* Stage 10 work in `app_model.rs`, `main.rs` and `main.slint`. It had a fixed 100×48 path viewbox, no history interaction, model rebuilds on every timer tick, a stray `history_total_label` text and no meaningful tests. That work was reviewed and **replaced** by the design below (the old diff was kept only in a scratch file, not in the repo). Nothing was committed.

## Production files inspected (read-only)

- `desktop/src/lib/metrics.ts` — `getWeeklyActivity`, `getStreakDays`, `getTodayMinutes`, `getFocusMomentum`, `getCourseHealthMap`, `getCourseMinutesMap`. Source of the streak semantics reproduced in `current_streak`.
- `desktop/src/App.tsx` — `renderStatCard`, `renderStatsWidget` (today / week / streak / open tasks), `renderWeeklyChart` (range toggle week/7/14/30/60/365, per-column hover tip `focusTip`), `renderCourseRadar` (dot + health bar + detail line per course), `dash-stats-grid` / `dash-analyst-grid` layout.
- `desktop/src/App.css` — `.dash-stats-grid` (4 columns, 16px gap), `.design-weekly-bars` (flex columns, 110–170px tall), `.design-stats-mini-grid`.

Findings that shaped the spike: production has **no line/time-series chart** and no SVG chart library; its weekly chart is HTML/CSS columns; there is no donut. The time-series chart is therefore net-new native work, not a port. Nothing under `desktop/` was modified.

## Architecture

```text
src/dashboard.rs   (plain Rust, no Slint)      deterministic mock data + chart preparation + tests
src/app_model.rs   (plain Rust, no Slint)      owns DashboardSnapshot + selection state via AppCommand
src/main.rs        (Slint adapter)             pushes prepared values into Slint properties
ui/dashboard.slint + ui/components/*           layout and drawing only
crates/study-tracker-core                      UNCHANGED (no chart/presentation concepts)
```

Rust owns: mock data, nice axis scale, normalized geometry, path command strings, x-axis label selection, percentages (largest-remainder so they sum to 100), streak, selection state, nearest-point lookup, all text ("Sat 19 Sep", "2h 35m").
Slint owns: layout, responsive breakpoints, colors/animation, hit areas, focus rings.
No statistics are computed in `.slint`.

### Files

New: `src/dashboard.rs`, `ui/dashboard.slint`, `ui/dashboard-types.slint`, `ui/components/{stat-card,bar-chart,line-chart,chart-tooltip,course-breakdown,session-list,segmented-control}.slint`, `scripts/sample-linux.sh`, this doc.
Modified: `src/app_model.rs` (dashboard state + commands; `Eq` dropped from `AppModel`/`AppCommand` because chart data holds `f32`), `src/main.rs` (dashboard adapter, env startup options), `ui/main.slint` (view switcher, dashboard surface, resizable window), `ui/theme.slint` (chart tokens + `Theme.tone()`; existing tokens untouched).

### Adapter update paths

- Timer tick → only timer properties (the dashboard is no longer re-pushed at 10 Hz).
- Hover / arrow keys → `apply_dashboard_selection`: ~8 scalar properties, no model rebuild.
- Range / data-set change → `apply_dashboard`: rebuilds models and paths.
- Plot resize → only the two path strings.

## Chart rendering approach

Investigated: Slint primitives (rectangles), `Path` with SVG-style commands, Rust-precomputed geometry. No chart dependency was added (`Cargo.toml` unchanged); a chart crate would have added a second rendering path or forced an SVG/bitmap route for little gain at this scale.

- **Bars**: `Rectangle`s; height = `fraction × plot height` (fraction prepared in Rust against a nice axis top). Gridlines/labels from `y_ticks`.
- **Line/area**: two `Path` elements with Rust-generated `M x y L …` commands.
- **Course share**: stacked rounded bar from cumulative `start`/`fraction`; the rows double as the legend. A donut was **not** built (Slint has no arc primitive short of hand-rolled path arcs; disproportionate for the value).

### Important renderer finding: Slint `Path` viewbox scales uniformly

The first implementation emitted paths in a fixed 1000×1000 viewbox so resizing would be free. Screenshot showed the line occupying only the middle ~45% of a wide plot: **Slint's `Path` preserves aspect ratio and centres the viewbox; it cannot be stretched non-uniformly.** Fix: Rust emits path commands in the plot's real pixel size, Slint reports the plot size through a `plot-resized` callback (`init` / `changed width` / `changed height`), and Rust re-emits only the path strings (`HistoryChart::set_plot_size`). Strokes stay a true 2px. Cost is small (below). This is a genuine architectural constraint for any future native chart work.

## Interaction model

| Control | Mouse | Keyboard | Accessible |
| --- | --- | --- | --- |
| Weekly bars | hover or click selects a bar | Tab to focus, ←/→, Home/End | `slider` role with value text `Mon · 2h 35m`; each bar a `button` with label |
| History line | hover/drag moves nearest-point selection | ←/→ (Shift = 7 points), Home/End | `slider` role, value text, summary description |
| Range 30/365/1000 | click | Tab, Enter/Space | `tab-list` / `tab` with `checked` |
| Data-set (edge cases) | click | Tab, Enter/Space | same |
| Sessions | wheel scrolls virtualized `ListView` | — | rows have `list-item` role + full text label |

Nothing is hover-only: the selected value is always shown in a text line under each chart ("Selected: Sat 19 Sep — 1h 58m") plus the tooltip band. Animations are finite (120–180 ms opacity/height/x); no timers or tickers were added.

Timer shortcuts (Space / R) are suppressed while the dashboard is shown so keyboard use of charts cannot start the timer.

## Edge cases

Deterministic tests plus a runtime "Data set" control (Typical / Empty / 1 point / Zeros / Same / Outlier):

- Empty: no path, empty-state text, default 0–1.5h axis, zero cards, no sessions (visually verified).
- One point: centred marker, no path, single x label.
- All-zero and identical values: on baseline / flat and inside the plot (5% axis headroom).
- Outlier: axis stretches, neighbours stay visible (visually verified on the weekly chart).
- Invalid plot sizes (0, negative, NaN, ∞) → empty paths. NaN/∞ fractions to selection are clamped.
- Long course name ("Seminar: Computational Models of Cognition and Learning") elides in course rows and fits in session rows; Japanese names (`日本語 (Japanese)`, `第4章`) render.
- Percentages sum to exactly 100; zero total → all 0.

## Dataset scaling (Rust-side, release build, this machine)

| Points | Full snapshot build | Path re-emit on resize | Path bytes |
| --- | --- | --- | --- |
| 30 | 58 µs | 13 µs | ~1 KB |
| 365 | 247 µs | 118 µs | ~11 KB |
| 1,000 | 515 µs | 313 µs | ~31 KB |
| 10,000 | 5.2 ms | 3.2 ms | ~316 KB |

(`cargo test --release preparation_cost_report -- --ignored --nocapture`.) Hover lookup is O(1) arithmetic; the micro-benchmark for it was optimized away, so no figure is claimed. Preparation is not a bottleneck at any tested size.

Runtime: 30, 365 and 1,000 points all rendered and were hover-interactive with no visible lag; CPU during hover sweeps was ~1.3% at every size and PSS grew only 0.4 MB from 30 → 1,000 points. **Readability, not speed, is the 1,000-point problem**: 1,000 daily points at ~1.3 points/px of high-variance mock data collapses into a solid band. A real product would aggregate (weekly means / rolling average / min-max decimation) above roughly 1 point per pixel. That was not built.

## Performance (Linux, release build, PSS primary)

Conditions: Hyprland/Wayland, HiDPI ×2. The compositor tiled the window at **584×750** during measurement (requested sizes are not honoured reliably; see limitations), so the dashboard was in its stacked/2×2 layout. CPU is sampled from `/proc/<pid>/stat` deltas over the stated window, not lifetime %CPU. Executable: 30,996,704 bytes (~29.6 MiB, release, unstripped).

| Stage | Setup | RSS | **PSS** | Priv_Clean | Priv_Dirty | Shared_Clean | Shared_Dirty | CPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D1 | dashboard, 30 pts, launch + 4 s | 79.0 | **40.7** | 13.5 | 22.0 | 43.6 | 0.0 | 0.20% (5 s) |
| D2 | same, ~1 min idle | 79.0 | **40.7** | 13.5 | 22.0 | 43.6 | 0.0 | 0.10% (10 s) |
| D3 | after interaction ×1, +15 s | 82.6 | **42.8** | 13.9 | 22.7 | 46.0 | 0.0 | 0.00% (10 s) |
| D3 | ×2 | 82.7 | 42.8 | 13.9 | 22.7 | 46.0 | 0.0 | 0.00% |
| D3 | ×3 | 82.7 | 42.8 | 13.9 | 22.7 | 46.0 | 0.0 | 0.00% |
| D4 | 1,000 pts, launch + 4 s | 79.4 | **41.0** | 13.7 | 22.1 | 43.7 | 0.0 | 0.00% (5 s) |
| D4 | 1,000 pts, interaction ×1 / ×2 / ×3, +15 s | 83.6 | **43.6** | 14.1 | 23.3 | 46.2 | 0.0 | 0.00% (10 s) each |
| D5 | click back to Timer, +15 s | 82.7 | **42.8** | 13.9 | 22.8 | 46.0 | 0.0 | 0.00% (10 s) |

Interaction script (real events via a scratch `/dev/uinput` virtual mouse/keyboard + `hyprctl movecursor`): wheel-scroll the whole view, hover sweeps across bars and the history chart, scroll the sessions list, Tab + arrow/Home/End keys. Hover-sweep CPU (event rate limited to ~50 moves/s by `hyprctl`): 1.25% / 1.38% / 1.25% at 30 / 365 / 1000 points.

Same-session controls (10 s window): Timer view 40.2 MB PSS, 0.00% CPU; Stage 9 text view 47.8 MB PSS, 0.10% CPU. Timer *running* (after Space): ~8% CPU (existing Stage 4/8 100 ms tick + progress ring; unchanged by Stage 10).

### Memory interpretation

The historical figures (Stage 4/5 timer ~54 MB, Stage 9 text ~66 MB) were **not reproduced today**: fresh controls read 40.2 and 47.8 MB. The difference is environmental (window size/GPU buffers/driver state), so cross-day absolute comparisons are unreliable. The valid comparison is within today's runs: the dashboard costs ≈ +0.5 MB over the Timer view at launch and ≈ +2.6 MB after its first interaction (FemtoVG/glyph caches), then is flat across three interaction rounds — no growth. 1,000 points adds ≈ +0.8 MB. The 60–80 MB PSS "encouraging" band was never approached. A 1220×820 window will use more (larger framebuffers); that was not re-measured because the compositor would not hold that size in the benchmark runs.

## Visual observations

Assessed from screenshots at 1220×820 (two-column) and 528–584 px wide (stacked).

Good: consistent with the timer prototype's theme; clear hierarchy (small caps label → large value → detail); tidy card radii/borders; bars, gridlines and axis labels are crisp at HiDPI; selected-bar highlight plus tooltip band works; line joins/caps are smooth, 2px stroke antialiased well; the area gradient renders (subtle); Japanese and long names render without layout breakage; tooltip stays clamped inside the card.

Weak / honest: the mock 365/1000-point series is very jagged (mock property, plus the aggregation point above); (the sidebar's old static "Today 2h 45m · streak 6" placeholder contradicted the dashboard; it now reads today's value and streak from the same `dashboard-cards` data); the weekly card fills width generously so bars get fat in the stacked layout; the chart focus ring was not visually confirmed (keyboard *navigation* was). It plausibly resembles part of a polished app but is not yet production polish.

## Responsive behaviour

Breakpoints on content width: ≥ 820 px two-column chart/list rows; < 820 stacked; < 620 summary cards 2×2; whole view scrolls vertically. Verified visually at ~1220×820 (two-column) and 528 / 584 px wide (stacked, 2×2 cards, no clipping or negative sizes). The window is now resizable (`preferred-*`/`min-*` instead of fixed `width`/`height` in `main.slint`; the fixed size had made it non-resizable).

Not verified: a genuinely **wide** window (screen is 1280×800 logical) and a genuinely **short** window (the tiling compositor overrode requested sizes; `STUDY_NATIVE_SIZE` and `hyprctl resizewindowpixel` were both unreliable). Short-window safety rests on construction (everything is inside a vertical `ScrollView`; chart heights derive from fixed card heights, `max()` guards on plot sizes), not on observation. A 168 px-wide accident showed clipping below the 460 px minimum, which the OS/compositor did not enforce.

## Renderer (FemtoVG) observations

- Path stroke quality, joins, caps and gradients were fine at 2× scale; **1× scale was not tested**.
- No clipping defects observed; Path is not clipped by its parent, so markers at plot edges render whole.
- Hover updates repaint promptly; settled CPU returns to 0.00%.
- Constraint found: `Path` viewbox scaling is uniform (above).
- Stage 9 emoji tofu (`⏱️`, `❤️`) still present; not made worse. No renderer switch was made or needed.

## Accessibility

Represented: roles/labels/values on both charts (`slider`), each bar (`button`), range/data-set pickers (`tab-list`/`tab`), stat cards and course rows (`groupbox` with full sentence labels), session rows (`list-item`), share bar (`image` with label). Visible text equivalents for every selectable value. Keyboard: Tab focus, arrows/Home/End verified with real key events; Enter/Space on pickers by construction.

Not verified: any real screen reader (Orca/NVDA/VoiceOver), Tab order across the whole page, focus-ring visuals on the charts, Windows/macOS accessibility trees.

## Regression (Linux, real input)

Timer: view switch by clicking Timer from the dashboard; mode selection (Pomodoro → 25:00); Start via Space (52:00 → 51:57 after ~3 s); Pause ("Paused 51:51"); Resume (24:56 after ~4.4 s run time); Reset via R ("Ready 52:00"); progress ring updates. Stage 9 view opens with the same text (and same emoji tofu) as before. Windows/macOS: unverified.

## Verification commands

`cargo fmt --check` OK · `cargo check` OK · `cargo test --workspace` 19 core + 35 prototype passed, 1 ignored (timing report) · `cargo test -p study-tracker-core` 19 passed · `cargo build --release` OK · `./scripts/check.sh` exit 0 · `git diff --check` clean.

## Unresolved issues

1. Large series need aggregation/decimation to be readable (~1 pt/px and above).
2. Slint `Path` uniform viewbox forces a Rust↔Slint size round-trip; any new native chart needs that pattern.
3. Wide/short windows and 1× DPI not verified; benchmark ran at 584×750, not a typical 1220×820.
4. No real screen-reader test; chart focus ring visuals unconfirmed.
5. Donut chart, zoom/pan, date-range picker, per-course filtering not attempted.
6. ~~Sidebar static text contradicted dashboard numbers~~ — fixed: sidebar shows today's value and streak from the dashboard cards (its daily-goal line was dropped).
7. Windows/macOS untested.
8. Timer-running CPU (~8%) is pre-existing and worth revisiting independently.
9. `AppModel`/`AppCommand` lost `Eq` (f32 payloads); harmless now, note if `Eq` is wanted later.

## Startup hooks (test-only conveniences)

`STUDY_NATIVE_VIEW=timer|text|dashboard`, `STUDY_NATIVE_POINTS=<n>`, `STUDY_NATIVE_SCENARIO=0..5`, `STUDY_NATIVE_SIZE=WxH` (unreliable under tiling compositors). `scripts/sample-linux.sh <pid> [seconds] [label]` prints one memory + sampled-CPU line.
