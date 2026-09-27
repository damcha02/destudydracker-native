# Stage 12 — Windows platform feasibility

## Question

Is the native architecture (Rust + Slint 1.17.1 + Winit + FemtoVG, renderer-independent `study-tracker-core`) genuinely suitable for a **Windows-first** Study Tracker?

**Verdict: PASS WITH CONCERNS.** Everything below was measured on one real Windows PC (specs next section). Linux figures from Stages 9–11 appear only as historical context and are never used as Windows baselines. Windows memory is reported with Windows metrics, never called PSS.

The shape of the result: the everyday workload (timer running for hours, dashboard, idle) is excellent once one real defect found in this stage is fixed; the map is good; the concerns are emoji sequences, untested IME, incomplete accessibility, missing window icon / console subsystem, wheel tuning, and an open renderer decision for heavy vector data.

## Environment

| Item | Value |
| --- | --- |
| OS | Windows 11 Pro 10.0.26200 (build 26200) |
| CPU | Intel Core i9-14900KF, 24 cores / 32 threads |
| RAM | 63.8 GB |
| GPU / driver | NVIDIA GeForce RTX 5070 Ti, driver 32.0.15.9186 |
| Display | 3440×1440 physical, 164 Hz, 125 % scaling (120 DPI; 2752×1152 logical), **one monitor** |
| Input | USB mouse only (**no touchpad**), Swiss-German keyboard layout |
| Languages | de-DE, de-CH, fr-CH, en-US (**Japanese IME not installed**) |
| Rust | rustc 1.98.1 (2026-09-01), cargo 1.98.1, `stable-x86_64-pc-windows-msvc` |
| Other | VS 2022 Build Tools (C++ workload); WebView2 runtime 153.0.4234.48 present |

Rust and the MSVC Build Tools were **not installed** at the start; they were installed with the user's permission via winget (rustup 1.29.1, `Microsoft.VisualStudio.2022.BuildTools`). The machine is very fast: absolute numbers will not transfer to a weak laptop or integrated GPU, only the relative behaviour and the shape of the curves should.

## Build portability

- `cargo fmt --check`, `cargo check`, `cargo test --workspace`, `cargo test -p study-tracker-core`, `cargo build --release`, `git diff --check`: all pass. **No Windows portability fix was needed in existing source.**
- Tests: 19 core + 63 prototype pass (62 existing + 1 new), 2 ignored manual benchmark reports. Only warning: `function region is never used` in the *test* build (already present before this stage).
- Release executable: `target\release\study-tracker-native-prototype.exe`, **16,826,368 bytes** (Linux: 32 MB).
- `scripts/check.sh` only runs fmt/check/test/build; those five checks were reproduced natively (above).
- Platform assumptions found: the exe is a **console-subsystem** binary (PE subsystem 3), so a console window opens behind the app when launched normally; and **no window icon** is set (`WM_GETICON` returns 0/0).

## Files changed (all under `native-prototype/`, uncommitted)

| File | Change |
| --- | --- |
| `src/main.rs` | (1) diagnostics `STUDY_NATIVE_STARTUP_REPORT` and `STUDY_NATIVE_FRAME_STATS` (env-gated, off by default); (2) **timer fix**: `model_matches` guard so the 100 ms refresh no longer replaces unchanged models; (3) unit test `model_matches_only_when_rows_are_identical` |
| `scripts/win-metrics.ps1` | shared helpers: Private Working Set / Private Bytes / Working Set / CPU / threads / process tree; parameter-less on purpose (see "Tooling pitfalls") |
| `scripts/benchmark-windows.ps1` | `-Mode sample\|startup\|launch` |
| `scripts/win-input.ps1` | real mouse/keyboard/Unicode-text injection and screenshots (DPI-aware) |
| `scripts/map-stress-windows.ps1` | map stress matrix with Slint's own fps counter |
| `scripts/memory-retention-windows.ps1` | extreme-geometry retention experiment |
| `scripts/long-run-windows.ps1` | multi-phase long-running timer time series |
| `scripts/renderer-everyday-windows.ps1` | per-renderer startup / memory / timer CPU |
| `docs/stage12-windows-platform.md` | this document |

Production `desktop/` was not touched. Nothing was committed or pushed. The Skia experiment lives in a scratch copy outside the repository; the repository's `Cargo.toml` / `Cargo.lock` are unchanged.

## Windows renderer / backend (identified)

**Winit + FemtoVG renderer + OpenGL through WGL on the NVIDIA driver.** Evidence:

- Slint prints `Backend: FemtoVG renderer with OpenGL backend`.
- Loaded modules: `OPENGL32.dll` + `nvoglv64.dll` (NVIDIA's native OpenGL driver); **no** D3D, ANGLE or EGL modules. `glutin` ships both WGL and EGL bindings, WGL is what runs here.
- Also loaded: `uiautomationcore.dll` (Windows accessibility), `MSCTF.dll` (Text Services Framework, i.e. IME plumbing), `dwrite.dll`.
- **Text**: shaping and glyph rasterisation are Slint's own (`rustybuzz`/`parley`, `swash`/`skrifa`). DirectWrite is used **only** by `fontique` for system font enumeration and fallback (`IDWriteFontFallback::MapCharacters`). Text is *not* DirectWrite-rendered.
- Hardware acceleration: the vendor GL driver is loaded and the stress results (below) scale with geometry the way GPU tessellation does; no separate "is it accelerated" query was made, so this is inferred, not proven.

## Memory methodology (Windows)

Collected with `K32GetProcessMemoryInfo` (`PROCESS_MEMORY_COUNTERS_EX2`) per process, summed over the process tree (`conhost.exe`, present only when stdio is redirected, is excluded). All in MB (1,048,576 bytes).

| Metric | Meaning |
| --- | --- |
| **Working Set** | physical RAM currently mapped, including shared pages (DLLs, driver mappings). Varies run to run (idle 77 vs 90 MB) because of shared/driver pages. |
| **Private Working Set** | working-set pages not shareable with other processes. **The main comparison metric.** |
| **Private Bytes** | committed private virtual memory (Task Manager "Commit size"). Includes GPU-driver commit and untouched pages; can be far larger than RAM use. |
| CPU% | delta of user+kernel processor time / wall interval; **100 % = one logical core** (32 cores, so 3.2 % = whole machine). Always an interval, never an instantaneous reading. |

Process count was **1** in every app measurement (no helper processes). Thread count 4–14.

## Baseline W0–W3 (single session, FemtoVG, first release build)

| Point | Working Set | Private WS | Private Bytes | CPU | Threads |
| --- | --- | --- | --- | --- | --- |
| W0 launch + 5 s (Text view) | 78.1 | 43.5 | 119.3 | 0 % | 14 |
| W1 idle 60 s | 77.3 | 42.6 | 118.4 | 0.03 % | 8 |
| W2a Timer | 83.9 | 48.8 | 126.5 | 0 % | 8 |
| W2b Dashboard | 89.5 | 53.6 | 132.3 | 0 % | 8 |
| W2c Map, idle | 91.3 | 55.2 | 145.6 | 0 % | 8 |
| W2d Map after hover/zoom | 104.2 | 68.1 | 185.6 | 0.31 % | 8 |
| W3 back to Timer | 105.0 | 68.9 | 181.4 | 0 % | 8 |

Settled CPU is 0 % on Timer, Dashboard and Map.

## Long-running everyday efficiency

This is the workload that matters most. Method: `scripts/long-run-windows.ps1` runs one app session through idle, timer running (Exam, 120 min, so it cannot complete) with Timer / Dashboard / Map visible, **minimized**, restored, and paused; samples every 30 s; display kept awake; real synthetic clicks. Frame counts come from the app's own diagnostic (`STUDY_NATIVE_FRAME_STATS`).

### Result on the fixed build (56 min, 112 samples)

| Phase | Duration | Private WS (first → last) | Private Bytes | CPU avg (max) | Frames / 10 s | Rust ticks / 10 s |
| --- | --- | --- | --- | --- | --- | --- |
| Idle, Timer ready | 3 min | 43.8 → 43.7 | 119 → 115 | 0 % | 0 | 0 |
| Timer running, Timer visible | 12 min | 48.4 → 47.4, then **one +5 MB step** → 52.4 | 119 → 124 | 0.26 % (0.42) | 20 | 100 |
| Timer running, Dashboard visible | 8 min | 53.2 flat | 132 | 0.07 % (0.22) | **0** | 100 |
| Timer running, Map visible | 8 min | 54.4 flat | 145 → 137 | 0.09 % (0.21) | **0** | 100 |
| Timer running, **minimized** | 15 min | 57.4 → 57.5 flat | 114 → 108 | 0.23 % (0.47) | **0** | 100 |
| Restored, Timer visible | 4 min | 57.5 flat | 134 | 0.24 % (0.47) | 20 | 100 |
| Paused | 6 min | 57.5 flat | 134 | 0.03 % (0.18) | 0 | 0 |

A second, independent **30-minute** run of the same fixed build reproduced it: idle 42.9; timer visible 47.3 → 47.8 (0.28 % CPU); dashboard 53.4 (0.09 %); map 54.8 (0.11 %); minimized 55.8 flat (0.27 %); restored 55.8 (0.24 %); paused 56.1 (0.03 %). The +5 MB step did not recur in that run.

### The defect this stage found, and the as-is comparison

The build as it stood at HEAD `456d0da` (with only diagnostics added) ran a **true 30-minute as-is run**:

| Phase | Private WS | CPU avg (max) | Frames / 10 s |
| --- | --- | --- | --- |
| Idle | 42.1 | 0 % | 0 |
| Timer running, Timer visible | 47.6 → 48.1 | **3.81 % (5.97)** | **~200** |
| Dashboard visible | 53.4 | 0.55 % | 0 |
| Map visible | 54.7 | 0.43 % | 0 |
| **Minimized** | 55.8 flat | **3.59 %** | **~200 (rendering while minimized!)** |
| Restored | 55.8 | 4.59 % | ~200 |
| Paused | 55.3 | 0.34 % | 0 |

**Cause.** `apply_model_to_window` runs on every 100 ms tick and did `set_modes(ModelRc::new(Rc::new(VecModel::from(..))))` and the same for `session_notes`. A brand-new model object is always "changed" to Slint, so the repeaters were rebuilt and the whole window repainted **twice per tick (20 fps)**, visible or minimized, for a countdown that changes once per second. **Fix** (`src/main.rs`): compare rows and only install a model when its content differs (`model_matches`). **Effect**: 20 fps → 2 fps visible, **0 fps minimized/hidden**, CPU roughly **15× lower** (3.81 % → 0.26 % visible, 3.59 % → 0.23 % minimized). The short A/B (60 s each, identical binaries launched directly) agrees: as-is 2.6–2.7 % / 200 frames, fixed 0.23–0.39 % / 20 frames. Memory is the same either way.

This very likely also explains the Linux disagreement between Stage 4 (~1.7 %) and Stage 10 (~8 %); not re-verified on Linux.

### Conclusions

- **Idle memory**: 43 MB Private WS (Working Set 77–90 MB, Private Bytes ~118 MB).
- **Running memory**: 47–48 MB with the Timer visible; 53–55 MB after visiting Dashboard and Map; ~57 MB after a full session. Views add memory once (first visit) and hold it.
- **Paused / minimized**: 55–57 MB, identical to running; no growth in 15 min minimized or 6 min paused.
- **Growth vs plateau**: stable allocation warm-up followed by a **plateau**. No monotonic growth in any of the three long runs, **including the as-is build with its per-tick allocation churn**. One +5 MB step in 1 of 3 runs is cache warm-up-sized noise, not a trend. Not a leak on this evidence. This covers ≤ 1 h of running; multi-hour runs were **not** performed.
- **Visible CPU**: 0.26 % of one core (0.008 % of the machine) with the Timer visible; 0.07–0.09 % with another view visible.
- **Background CPU**: 0.23 % minimized. This is the residual 10 Hz Rust tick (17 property sets per tick, see below); it is not rendering. Minimized rendering is 0 fps after the fix.
- **Timer correctness after minimize/restore**: verified. Started 21:52:32.06; 15 minutes minimized; restored at 22:35:43.2 (wall elapsed 43:11.2). The restored window showed **76:49 remaining, 43m 11s elapsed**: exact to the second.
- **Multi-hour suitability**: architecture is suitable. Correctness is timestamp-based (see next section), not tick-counting; memory is flat over ≥ 56 min; CPU is negligible. Untested: sleep/hibernate/wall-clock changes and runs over 1 h.

### Timer update behaviour (verified in code and at runtime)

- `TimerState` derives everything from a `ClockObservation` (monotonic elapsed millis since app start + wall clock) passed with each command; `display_seconds(clock)` computes remaining time. **Nothing depends on counting UI ticks**; an existing core test skips 11 s of ticks and still completes correctly. Display has 1 s granularity.
- Rust timer: a Slint `Timer` at **100 ms only while running** (100 ticks / 10 s measured); stopped when paused/ready/completed (0 ticks measured while paused).
- Each tick sets ~17 scalar properties (Slint ignores unchanged values) and, before the fix, two models. After the fix the only visible change is the clock text / progress ring once per second, giving **2 frames/s** (2 frames per change). No permanent animation loop, no vsync-rate repainting.
- **Possible further saving, not implemented (measure first)**: the 10 Hz tick could drop to 1 Hz (or stop updating) while the window is minimized or the Timer view is hidden, since correctness does not depend on ticks. Worth ~0.2 % of a core; low priority.

## Startup

Definition: process spawn until the first rendered frame.

- **FemtoVG, first-frame notifier** (`FIRST_FRAME` printed after the first `AfterRendering`, warm, repeated): default view min 162.8 / median 164.3 / mean 166.2 / max 188 ms (12 runs); Map view 156.9 / 159.8 / 160.0 / 164 ms (12 runs). App-side portion (main → first frame) ~145–150 ms. Private WS at first frame 43.5 MB (Text view), 52 MB (Map view).
- Cold-boot / first-ever launch and antivirus scan effects were **not** measured; the first run of a series was at most 25 ms slower.
- Renderer-neutral first-paint probe (screen pixel probe, adds ~50–70 ms of polling overhead, 8 runs, used only for the renderer comparison): FemtoVG 180 / 229 / 280 ms; Skia-OpenGL 161 / 172 / 258; Skia-D3D12 278 / 290 / 373; Skia-software 59 / 142 / 156 ms (min looks like a probe artefact). Read these as "all within ~150–300 ms".

## Timer, Stage 9 text, Dashboard, Map: functional regression

| View | Result |
| --- | --- |
| **Timer** | Opens; presets selectable; **Space** starts (51:58 "In session"), Space pauses ("Paused"), Space resumes, **R** resets to "Ready 52:00"; Start/Pause/Reset buttons by mouse; progress ring; Tab/Shift+Tab move a visible focus ring. Completion behaviour not exercised (Demo preset = 10 s would); cosmetic: at the default window size the ring's bottom slightly overlaps the Start/Reset buttons. |
| **Stage 9 text** | Opens; both edit fields work (see below). |
| **Dashboard** | Cards, weekly bars, history line chart, range chips (30/365/1000), course breakdown, session list; hover tooltip and selection work; resize works. |
| **Map** | Renders 195 regions; hover, click selection, drag pan, wheel zoom, labels, zoom buttons work; keyboard model unchanged. |

## Dashboard

Preparation cost (existing `preparation_cost_report`, release): 30 pts 25 µs, 365 pts 120 µs, 1,000 pts 316 µs, **10,000 pts 2.85 ms** full snapshot build (resize path re-emit 1.72 ms at 10,000).

Interactive (FemtoVG, real synthetic input; CPU in % of one core over the action):

| Points | Idle Private WS | Idle CPU | Hover sweep CPU | Arrow keys CPU | Resize CPU | Private WS after | Settled CPU |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 30 | 44.0 | 0 % | 4.6 % | 2.2 % | 21.2 % | 48.7 | 0 % |
| 365 | 46.4 | 0 % | 15.8 % | 3.7 % | 26.0 % | 49.3 | 0 % |
| 1,000 | 48.2 | 0 % | 17.0 % | 1.8 % | 21.3 % | 52.8 | 0.16 % |
| 10,000 | 58.6 | 0 % | 51.7 % | 4.3 % | 46.1 % | 61.4 | 0 % |

Hover CPU is a continuous synthetic sweep (~100 moves/s), not sustained use. 10,000 points is heavier but usable; the realistic range (30–365) is cheap. Settled CPU returns to ~0.

## Map: real workload and stress

Real World dataset (195 regions, 273 rings, 9,405 vertices): 156–160 fps while animating a changed transform on **every** frame (panel cap is 164 Hz, so this is at the cap; 164 fps is not a target), 47–50 % of one core, Private WS 70 MB. Settled CPU 0 %. Hover, click selection, drag pan, wheel zoom, labels and resize were exercised at the system 125 % scale; click-to-select accuracy was additionally verified at 1.0 / 1.5 / 2.0 effective scale (see HiDPI).

Stress matrix, Slint's own fps counter (`SLINT_DEBUG_PERFORMANCE`, first sample dropped), 12 s runs, CPU % of one core, FemtoVG (final re-run):

| Level | fps pan / zoom (min) | CPU pan | Private WS pan | Private Bytes pan / zoom |
| --- | --- | --- | --- | --- |
| World (9.4k) | 158 / 154 (152) | 47 % | 70 MB | 181 / 219 MB |
| Dense ×10 (94k) | 100 / 82 (54) | 117 % | 121 MB | 1,143 / 1,282 MB |
| Dense ×50 (470k) | **26 / 21** (11) | 110 % | 194 MB | 1,664 / 1,281 MB |
| Cells (2,000 regions) | 152 / 127 | 119 % | 80 MB | 190 / 411 MB |
| Giant (1 path, 50k pts) | 76 / 76 | 115 % | 115 MB | 222 / 823 MB |

The ceiling is Dense ×50; the purpose was not to hit 60 fps there. **Notable**: with the GPU renderers, **Private Bytes (commit) balloons far past Private WS** on heavy data (FemtoVG: 1.1–1.7 GB commit for ~120–190 MB Private WS). It is commit accounting, not RAM, and disappears with software rendering (see renderer comparison), so it is most likely NVIDIA user-mode driver / GL buffer commit; not proven.

Historical Linux context (not comparable): Linux FemtoVG Dense ×50 17–21 fps, Giant ~30 fps; Windows/NVIDIA Giant is far better (76 fps) and World is at the display cap.

### Extreme-geometry memory retention (reproduced)

Sequence per cycle in one session, keys `]`/`[` switch levels (Light, Dense ×3, Dense ×10 are passed through on the way): World → Dense ×50 → interact (hover, 12 wheel notches, drag, zoom out) → World → Dashboard → Timer → Map (World). **6 cycles**, Private WS in MB:

| Point | C0 | C1 | C2 | C3 | C4 | C5 | C6 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Dense ×50 loaded | – | 191 | 281 | 290 | 282 | 294 | 289 |
| back on World | 51 | 176 | 252 | 258 | 259 | 255 | 259 |
| final Map (World) | – | 175 | 243 | 245 | 252 | 243 | 248 |

Private Bytes over the same points: baseline ~141 MB; Dense ×50 loaded ~320–505 MB; back on World 305–448 MB. **Interpretation: high-water plateau, not a leak.** Memory does not return to baseline after extreme geometry (51 → ~245 MB), jumps on the first cycle, settles from cycle 2, and shows no upward trend through cycle 6. It reproduces the Linux observation (the glibc-only hypothesis does not apply here; cause on Windows is unproven: FemtoVG buffers at high-water capacity, the GL driver, or the allocator). Skia-software stays small on the same geometry (Private WS 28 MB on World, 59 MB on Dense ×50 during the stress run; retention after unloading was not measured for it), which is consistent with, but does not prove, a GPU-path cause. A realistic app never loads ×50 geometry.

## Renderer comparison

What Slint 1.17.1 offers on Windows (from the dependency source): `renderer-femtovg` (OpenGL, current), `renderer-femtovg-wgpu` (wgpu; DX12 on Windows), `renderer-skia` / `-skia-opengl` / `-skia-vulkan`, `renderer-software`. Finding: **plain `renderer-skia` on Windows picks Skia's *software* surface** (cfg-time default is Vulkan → OpenGL → Metal → software; Direct3D is not the default). A real **Direct3D 12** surface exists (`d3d_surface.rs`, `skia-safe` `d3d` feature) and is selected with `BackendSelector::new().backend_name("winit").renderer_name("skia").require_d3d()`. All comparisons used a scratch copy of the same sources (`renderer-skia` swapped in, an env-selected `BackendSelector` call added); the prebuilt Skia binaries were downloaded by the build (~1 minute build). Verified at runtime: `skia backend d3d` with `D3D12Core.dll` + NVIDIA `nvwgf2umx.dll`; `skia backend opengl` with `nvoglv64.dll`; `skia backend software` with neither.

Skia executable: 23.3 MB (FemtoVG 16.8 MB, +6.5 MB). `renderer-femtovg-wgpu` was **not** built (Skia-D3D12 already answers "is a D3D12 path worthwhile"); it remains an untested candidate.

### Map stress (fps pan / zoom, same procedure)

| Level | FemtoVG-GL | Skia-D3D12 | Skia-OpenGL | Skia-software |
| --- | --- | --- | --- | --- |
| World | 158 / 154 | 157 / 157 | 157 / 158 | 160 / 158 |
| Dense ×10 | 100 / 82 | 159 / 116 | 158 / 135 | 147 / 93 |
| Dense ×50 | 26 / 21 | **130 / 36** | **131 / 36** | 104 / 32 |
| Cells | 152 / 127 | 161 / 125 | 158 / 154 | 157 / 93 |
| Giant (one 50k path) | **76 / 76** | 4.8 / 29 | 4.7 / 28 | 4.8 / 4.2 |

CPU % of one core while panning World: FemtoVG 47, Skia-D3D12 30, Skia-OpenGL 26, Skia-software 54. Private Bytes on Dense ×50 (pan): FemtoVG 1,664; Skia-D3D12 749; Skia-OpenGL 591; Skia-software 65 MB.

### Everyday workloads (timer running, same script per renderer)

| Renderer | Startup (probe) | Idle Private WS | Timer visible | Dashboard visible | Map visible | Minimized | CPU visible / minimized |
| --- | --- | --- | --- | --- | --- | --- | --- |
| FemtoVG-GL | ~229 ms | **43.1** | 47.5 | 53.2 | 55.6 | 57.6 | 0.47 % / 0.47 % |
| Skia-D3D12 | ~290 ms | 93.9 | 96.0 | 97.7 | 98.7 | 98.7 | 0.68 % / 0.68 % |
| Skia-OpenGL | ~172 ms | 54.5 | 107.5 | 108.1 | 111.6 | 116.6 | 0.57 % / 0.31 % |
| Skia-software | ~142 ms | **26.8** | 26.9 | 27.3 | 27.6 | 27.5 | 0.73 % / 0.83 % |

(All numbers are Private WS in MB; short 15–30 s CPU windows, so ±0.3 % is noise.) Frame counts are available only for FemtoVG (Slint's rendering notifier is not available on the Skia D3D12 / software surfaces in this configuration, so `STUDY_NATIVE_FRAME_STATS` reads 0 there).

### Trade-offs

- **FemtoVG-GL (current)**: lowest memory of the GPU renderers for everyday use, best single-huge-path performance, fastest to build, smallest exe. Weak on very dense multi-ring vector geometry (Dense ×50) and shows large driver commit charge on heavy data.
- **Skia-D3D12**: 5× faster on Dense ×50 pan and ~35 % lower CPU on the real map, native Direct3D 12, but +50 MB idle memory, +60–100 ms startup, +6.5 MB exe, **catastrophic on one 50k-vertex path (4.8 fps)**, no frame diagnostics.
- **Skia-OpenGL**: similar to D3D12; memory jumps to ~107 MB once the first frames are drawn.
- **Skia-software** (what `renderer-skia` alone gives you on Windows): remarkably low memory (27 MB) and no GPU driver dependency, and fast on this CPU for the real map. **Not portable evidence**: this is a 24-core desktop CPU; a typical laptop CPU at HiDPI will be far slower. It is also 4–5 fps on the Giant path.
- No renderer is declared the winner. **Recommendation: keep FemtoVG as the default** (realistic Study Tracker workloads never approach the ceiling). Geometry-heavy features would justify a real Skia evaluation on low-end hardware, including the single-huge-path weakness.

## Text and emoji (Windows)

Static and editable text, FemtoVG:

- Latin, German (Größe, Prüfung, Zürich, naïve, café), Japanese (日本語の勉強を始めましょう), mixed Latin/Japanese (Quantum Mechanics — 第4章 — σ² = 2.35 × 10⁻⁴), math symbols (≤ ≥ ∑ ∫ →) and combining marks (café, ä, ñ): **all render correctly**, no clipping, consistent baselines, crisp at 100/125/150/200 %. Japanese uses the system CJK font through DirectWrite fallback.
- Emoji, per sample:

| Sample | Result |
| --- | --- |
| 📚 | colour |
| ✅ | colour |
| 🧪 | colour |
| 🚀 | colour |
| ⏱️ (U+23F1 + VS16) | **tofu (empty box)** |
| ❤️ (U+2764 + VS16) | **tofu (empty box)** |
| 👍 | colour |
| 👨‍💻 (ZWJ sequence) | renders as a joined developer glyph (colour); exact gender/variant not verified |

Same failure class as Linux: the base-character-plus-variation-selector sequences fail while astral emoji work. Colour-font rendering itself works. No custom emoji font was bundled. Cause not investigated (font fallback for VS16 sequences in `parley`/Slint is the suspect). **Unresolved.**

## Editing, clipboard, keyboard

Real input on the Stage 9 fields (with correct scan-code key injection):

- Typing, Backspace, Delete, Home/End, **Shift+Arrow selection**, Shift+End, **Ctrl+Arrow** word movement, double-click word select, drag select: **work**.
- **Ctrl+A / Ctrl+X / Ctrl+C / Ctrl+V**: work; cut from one field, paste into another preserved `Study Tracker - 日本語 note - cafe - sigma σ²`.
- **Unicode clipboard** (`Ünïcödé 日本語 ✅ 👍 ⏱️ ä̈`) pastes correctly; ⏱️ is tofu inside the edit field too.
- **Application shortcuts do not steal input**: typing ` r r ` in a field inserts the text and does not trigger the timer Reset (the view advertises that Space/R are suppressed in fields).
- Timer: Space/R shortcuts, Tab/Shift+Tab focus traversal with visible focus ring (see Regression). Map keyboard model unchanged from Stage 11. Multiline `TextEdit` was not separately exercised in this stage.

## Japanese IME

**BLOCKED by environment.** Microsoft Japanese IME is not installed (`Get-WinUserLanguageList`: de-DE, de-CH, fr-CH, en-US, no ja-JP). No composition, candidate window, preedit or caret-during-composition test was possible. Not silently skipped. Per the user's instruction, no IME detection/installation guidance was implemented in Stage 12. What *is* known: `MSCTF.dll` (Text Services Framework) is loaded by the process, which is necessary but not sufficient for IME. **A manual test remains an acceptance gap** (enable Japanese in Settings → Time & language → Language & region → Add a language → 日本語, install the "Microsoft IME" feature, then compose in the Stage 9 single-line and multiline fields).

## HiDPI / scaling

- System scaling was **125 %** (window DPI 120, client 1525×1025 physical = 1220×820 logical). System scaling was **not changed** (disruptive); instead Slint's `SLINT_SCALE_FACTOR` override was used at **1.0 / 1.5 / 2.0**. This validates layout and hit-testing at other scale factors but is **not** a real per-monitor DPI transition.
- At 1.0, 1.25, 1.5 and 2.0 the client is 1220×820 logical (1220×820, 1525×1025, 1830×1230, 2440×1640 physical); text is crisp and the logical layout identical (verified on the Map view; other views were not re-inspected at each factor); a click at the Brazil label selects **Brazil** at every scale (logical input matches rendered geometry).
- Not tested: real system scale change, dragging between monitors with different DPI (only one monitor), resize while dragging across DPI.

## Mouse wheel and touchpad

- **Wheel**: one classic notch (120) is delivered to the app as 60 logical units, so with the Stage 11 curve `exp(delta/120 × 0.14)` **5 notches = 1.40×** (design intent ×2.0); 5 tiny events of 24 (a touchpad-like 120 total) = 1.1×; one large event of 360 = 1.2×. Behaviour is linear, smooth, free of huge jumps, clamps at 1.0× when zooming out. Zoom per notch (~7 %) is slower than intended: a **shared-code tuning fix** (rescale by the actual delta unit).
- **Precision touchpad**: **no touchpad hardware present** (only a USB mouse). Untested. Synthetic small-delta events suggest proportional handling but do not prove precision-touchpad behaviour.

## Accessibility (Windows UI Automation)

Narrator was not listened to (no audio verification); instead the tree Windows exposes (which is what Narrator and NVDA consume) was read through the real UI Automation client API on Timer, Text and Map views. No third-party software was installed.

Exposed well: window name `Study Tracker Native Prototype`; **Buttons** with meaningful names and Invoke pattern (`Deep Work timer preset`, `Start`, `Reset`, `Zoom in/out`, `Reset view`, the four view buttons), all keyboard-focusable; **Text** elements (the clock `52:00` becomes `51:58` after Start, the primary button's name flips to `Pause`); **ProgressBar** with RangeValue pattern (0..1); **Edit** controls with Value pattern and names (`Stage 9 single-line mixed text field`, `Stage 9 mixed language line edit`); **Image** for the map with a descriptive name that includes statistics and the current selection.

Limitations found: the map **Image is not keyboard-focusable in UIA** (its arrow/PageUp/PageDown model is unreachable to a screen-reader user, the zoom buttons are); one of the two progress bars has an empty name; every button's inner label appears again as a separate Text (68 Text elements on the Timer view, noisy); view switcher is a set of Buttons, not tabs; no landmarks/headings; no live region for the timer or its completion (not testable via Slint API); accessible list/table alternative for the map does not exist yet (carried from Stage 11). **Full accessibility compliance is not claimed; Narrator speech and focus tracking still need a manual pass.**

## Windows shell behaviour

- Standard top-level window: caption, min/max/resize/close, system menu. Minimize/restore, **maximize (client 3440×1351, excludes the taskbar) / restore**, native resize (including scripted resize storms), and close (`WM_CLOSE` → exit code 0) all work; the process keeps responding.
- Taskbar entry present. **No window icon** (`WM_GETICON` = 0), so a generic icon appears in the taskbar and Alt+Tab. **Console-subsystem exe**: a console window opens behind the app when launched from Explorer.
- Alt+Tab was not tested interactively (standard HWND, no reason to differ).
- Not implemented on purpose (later stages): tray, notifications, updater, autostart, global shortcuts.

## Shared vs Windows-specific classification

| Observation | Classification |
| --- | --- |
| Timer repainted at 20 fps (per-tick model replacement) | **Shared solution** (done; also removes the same waste on every platform) |
| Residual 10 Hz tick while minimized | Shared solution (optional) |
| Wheel notch = 60 units, zoom too slow | Shared solution (tuning; may need a per-platform delta unit) |
| Emoji + VS16 sequences show tofu | Shared / font-fallback issue in Slint text stack (unresolved; not solved by Windows) |
| Console window; no window icon | **Windows adapter** (`#![windows_subsystem = "windows"]` for release builds, embedded `.ico`; tiny) |
| Map viewport not focusable in UIA; unnamed progress bar; no live region | Shared (Slint accessible properties) plus verification with Narrator; a possible **Windows adapter** only if Slint's AccessKit bridge cannot express it |
| IME | Unverified; nothing indicates a Windows-specific implementation is needed (TSF is loaded); needs the manual test |
| Renderer for heavy vector data | Open **Windows renderer/backend** question (Skia-D3D12 helps dense multi-ring data, hurts single huge paths); no evidence yet that one is needed |
| Large Private Bytes on heavy geometry, retained high-water | Likely GPU driver / FemtoVG buffer behaviour; monitored, not a blocker |
| Nothing found that is a **Fundamental Slint limitation** | – |

No Windows-specific code was written beyond measurement tooling. Rust has direct access to Win32, DirectWrite, Direct2D, DirectComposition, UI Automation and TSF via the `windows` crate (and Slint already depends on it); nothing in Stage 12 showed a need to use them.

## Production Tauri comparison

**Skipped**, deliberately. `desktop/` has no `node_modules` and no Tauri build output, and this machine has no Node/npm; building it would install dependencies inside the production directory, which must stay read-only. The WebView2 runtime (153.0.4234.48) is present, so a comparison is possible later from a separate clone or with explicit permission. No Windows comparison against Tauri/WebView2 exists yet; Linux WebKit numbers must not be used for it.

## Tooling pitfalls found (kept here so nobody repeats them)

1. **Dot-sourcing a script that has a `param()` block silently overwrites the caller's variables.** `long-run-windows.ps1` dot-sourced `benchmark-windows.ps1`, whose `$Exe` default (the current release exe) replaced the caller's `-Exe`. Two "as-is" long runs therefore actually ran the fixed build; the false "as-is = 20 fps" result was caught by re-running the A/B and bisecting. Shared helpers now live in the parameter-less `win-metrics.ps1`. The first map-stress and retention runs were affected only trivially (10 s instead of 12 s; `-Csv` ignored) and their results are unaffected.
2. PowerShell variable names are case-insensitive (`$s` clobbered `$S`).
3. Synthetic arrows need scan codes and the extended-key flag, otherwise Shift+Arrow does not extend a selection (an early "selection bug" was this artefact, not the app).
4. PowerShell 5.1 `Set-Content -Encoding utf8` writes a BOM and `Get-Content` without `-Encoding` mangles UTF-8: one edit corrupted `·` in `main.rs`; the file was restored from git and edited with the editor tool instead.
5. `ExecutionPolicy` blocks `.ps1` by default; scripts are run with `-ExecutionPolicy Bypass` per process (no system change).

## Test-only hooks

`STUDY_NATIVE_VIEW=timer|text|dashboard|map`, `STUDY_NATIVE_MAP_LEVEL`, `STUDY_NATIVE_MAP_BENCH`, `STUDY_NATIVE_POINTS`, `STUDY_NATIVE_SCENARIO`, `STUDY_NATIVE_SIZE`, and new: `STUDY_NATIVE_STARTUP_REPORT=1` (prints `FIRST_FRAME <ms>`), `STUDY_NATIVE_FRAME_STATS=1` (prints `STATS <s> frames=<n> ticks=<n>` every 10 s). Slint's `SLINT_DEBUG_PERFORMANCE=refresh_full_speed,console` and `SLINT_SCALE_FACTOR` were also used.

## Risks

1. **IME unverified** (blocked) on a product whose users study Japanese/CJK material.
2. **Emoji sequences** with variation selectors render as tofu.
3. **Accessibility** incomplete: map not focusable, no live regions, Narrator/NVDA not exercised by ear.
4. **Ship polish**: no icon, console window.
5. **Renderer decision** open for dense vector data; FemtoVG shows large driver commit and retained high-water memory on extreme geometry; Skia has a single-huge-path cliff.
6. **One fast machine**: no low-end / integrated-GPU / laptop-battery / HiDPI-laptop evidence; software rendering results especially do not transfer.
7. Untested: touchpad, multi-monitor DPI transitions, real DPI change, multiline IME, sleep/hibernate, sessions over 1 h, cold boot startup, Tauri/WebView2 baseline.
8. Map data provenance/licence still undocumented in production (Stage 11).

## Recommendation for Stage 12.5 (not implemented)

1. **Ship-critical hygiene**: release builds with `windows_subsystem = "windows"`, embed an application icon; keep the timer fix and its test.
2. **IME**: run the manual Japanese IME test (install the language pack on purpose, user decision) in single-line and multiline fields, including candidate window placement, Backspace during composition and IME toggling.
3. **Emoji**: root-cause the VS16 tofu (font fallback / cluster handling), try upstream-supported fixes first; still no bundled emoji font unless justified.
4. **Accessibility**: manual Narrator (and NVDA if the user agrees to install it) pass; make the map viewport focusable / provide the accessible country list; name the unnamed progress bar; decide on live-region substitutes for timer completion.
5. **Wheel tuning** in shared code, and a real precision-touchpad check on a laptop.
6. **Low-end validation**: repeat the everyday scripts on an integrated-GPU laptop (battery drain during a multi-hour timer, HiDPI, software fallback), then decide renderer strategy; evaluate Skia-D3D12 further only if geometry-heavy features are planned, including the single-path case.
7. **Extend the long run**: multi-hour session including sleep/resume and wall-clock change; optional 1 Hz tick while hidden.
8. **Baseline against production**: Windows Tauri/WebView2 process tree, Private Bytes, startup and idle CPU, built from a clean clone with the user's permission.
