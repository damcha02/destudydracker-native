# Stage 19: Wabi-Sabi, Sakura and the appearance system (Knowledge Garden deferred)

Status: implemented and verified, **uncommitted**. Stage 18 is committed separately as `031f943`.
Production reference: Study Tracker **v0.1.67**, upstream `fe2f7a60704aa77b3d72a1c86912e2d2a60274b9`. `desktop/` is read-only and unchanged.

## 1. Scope (corrected)

Inspecting production overturned the brief's premise that the Knowledge Garden belongs with Wabi-Sabi and Sakura (§3). The user decided (2026-10-01) to match production exactly:

| Item | Production reality | Stage 19 decision |
|---|---|---|
| Knowledge Garden (4 variants + "next style · n/4" cycle) | only a card in the **Modern** dashboards (Cockpit, Focus, Custom) | **deferred entirely** (domain, UI, all 4 variants) to the Modern-dashboard migration. No substitute host. This is a scope correction, not missing Stage 19 functionality. |
| Wabi-Sabi | a whole alternative app shell: Kokoro sidebar, own Dashboard, own Timer, Quiet mode | **implemented**: shell, sidebar, Dashboard, Timer, Quiet mode, menu and Theme/Style panel |
| Sakura | a light-only **palette** for Field Notebook and Wabi-Sabi | **implemented** on both styles, with the strict animation lifecycle |
| Appearance system | three independent persisted values (style, palette, theme) | **implemented**: centralized tokens, persistence, backup import |

Acceptance criteria 3, 6–8 and 13–14 (Garden) of the original brief are therefore **not applicable** to this stage.

Not absorbed: the Modern dashboards and their Focus, Cockpit, Analyst and Custom layouts; the "Something else" picker; Personal, Options and Settings; Planner, Notes, Rest and Circle surfaces; Timer task linking; the default-style Timer parity (Stage 23).

## 2. Production baseline and provenance

`git fetch` of `damcha02/destudydracker` `main` on 2026-10-01: head is still `fe2f7a6` (2026-09-30, "exam in wabi sabi", v0.1.67). The `desktop/` tree hashes are identical to upstream for all 400 files. **Fresh, no resync needed.**

Production was inspected from source (`App.tsx`, `index.css`, `App.css`, `SakuraScatter.tsx`, `ModernGarden.tsx`, `storage.ts`, `plannerSchedule.ts`, `examPhase.ts`, `timeInput.ts`). It was also observed running: a scratch `vite build` served to a throw-away headless Chrome profile, with the packaged CSP's font blocking reproduced (`scripts/visual-parity/capture-prod.mjs`, extended with `--style/--palette/--quiet/--clicks/--anim-time/--platform-fonts`).

## 3. Production Knowledge Garden inventory (for the later stage)

- **Where it is drawn:** `renderGardenCard` is called only from the Modern dashboard (Cockpit column, Focus hero, Custom widget `garden`).
- **Variants:**
  - `western` (default, `KnowledgeGardenWidget`, 12 procedural SVG species);
  - `japanese` (`JapaneseGardenWidget`, 9 stages, PNG trees, water and koi);
  - `terraces` and `stems` (`ModernGarden.tsx`).
  - "next style · n/4" cycles through them; the choice is persisted under `study-tracker-garden-variant`.
- **State:** none of its own. Everything derives on every render from:
  - the rolling last-7-days minutes (stages: 0/30/90/210/420/720 min, "Dormant" → "Flourishing");
  - the streak;
  - the most recent study/exam sessions of active courses;
  - completed tasks;
  - placement and species from FNV-hash-seeded `mulberry32`.
- **Garden achievements** (`garden-*`) are derived from the same stage and streak. They belong to Stage 20 (Break Room achievements).

The backup's `study-tracker-garden-variant` is not read yet (§11).

## 4. Garden domain semantics

Deferred with the Garden (§1). The summary in §3 is what the later stage must port.

## 5. Production Wabi-Sabi inventory

| Area | Production (computed values at 1520×980) |
|---|---|
| Shell | `.shell { width: min(1450px, 100vw − 234px); margin-left: 186px; padding: 22px 32px 56px }`. `scrollbar-gutter: stable` reserves 10 px on the right. |
| Sidebar (`renderWabiSidebar`) | 186 px wide, `--wabi-paper-1`. Contents: "Kokoro" brand with a sage ring; overall-score ring (r 15.5, stroke 4, tone colour) with OVERALL and label; toolbar (light/dark, `?`, menu); nav Today/Plan/Timer/Notes/Rest/Circle (Georgia 14.5, active = 2 px sage bar + vermilion kanji 今日 計画 時計 記録 休み 仲間); Timer submenu (current semester → courses → tasks, Session logs); foot "TENDED TODAY", minutes, goal bar, "QUIET MODE →". |
| Dashboard (`renderWabiSabiDashboard`) | TODAY + date, "N open · Xh Ym to today's goal". ONE THING card: planned unit → first open timeline row → nearest released sheet; START / MARK DONE / SOMETHING ELSE. COMING UP: released, unsolved sheet deadlines within 60 days, max 6, NEXT marker, FOCUS. PLANNED TODAY: daily timeline (lectures, to-dos) then planned units. A hint line closes the page. |
| Timer (`renderWabiTimer`) | head TIMER + task/goal/"General focus"; 128 px Georgia clock centred in `100vh − 150px`; START/PAUSE/RESUME/START TRACKING, RESET, LOG AND CLOSE, "SWITCH TO BREAK →"; "WORK · 25 MIN" line; six preset cards (Pomodoro, Deep Work, Sprint, Exam, ∞ Endless, Custom). |
| Quiet mode | full-height paper overlay right of the sidebar (z 950): NOW, focus title (dropdown of other open tasks), meta, 86 px clock, START + "DONE, LOG IT", LEAVE QUIET MODE. |
| Palette | light: desk `#e9e5d8`, paper `#f4f1e8`/`#efebe0`/`#e5e1d0`, ink `#1c1d19`, muted `#8d8b7d`, faint `#a8a494`, sage `#4f6b4a`, vermilion `#b0472e`, rules `#ddd8c8`/`#1c1d19`. Dark: desk `#17160f` … (all in `appearance_view.rs`). |
| Fonts | CSS names Shippori Mincho / Zen Kaku Gothic New / IBM Plex Mono. The packaged CSP blocks them, so CDP `getPlatformFontsForNode` confirms **Georgia / Arial / Consolas**, with **Microsoft YaHei** for the kanji. |

## 6. Production Sakura inventory

| Property | Production |
|---|---|
| What it is | palette `sakura`. Offered only to Field Notebook and Wabi-Sabi (the Modern picker hides it). Wabi-Sabi + Sakura forces light (`themeLocked`). |
| Petals (`SakuraScatter`) | **22**, parameters from `mulberry32(0x534b5552)`. Left 0–100 %, size 16–36 px, opacity 0.22–0.42, drift ±80 px, spin ±(180–540)°, fall 14–28 s with negative delay up to 28 s, sway 3–6 s. |
| Motion | CSS `sakura-fall`: linear and infinite; `translate3d(drift·p, (100vh+80px)·p)` and `rotate(spin·p)` from `top: −60px`; opacity 0 → peak at 8 % → peak at 92 % → 0. Inner `sakura-sway`: ease-in-out, alternate, ±14 px and ±18°. Images alternate between two blossom PNGs (`object-fit: contain`). |
| Layering | `position: fixed; inset: 0; z 40; overflow: hidden; pointer-events: none`. Above content, **below** the Wabi sidebar (z 900). Quiet overlay (z 950) hosts its own 22-petal scatter. Field Notebook has no sidebar, so petals cover the page. |
| Texture | `body::after`: animated 200×200 GIF (**20 frames × 130 ms**, disposal "restore to background"), tiled, opacity 0.14, z 0. |
| Field Notebook + Sakura | pink notebook tokens (`#f7eeed` desk, `#fae6e7` paper, `#3d2230` ink …), identical for dark and light, `color-scheme: light`. Not locked. |
| Reduced motion | `@media (prefers-reduced-motion: reduce)`: petals static at `top: 20%`, own opacity, no transform. The GIF keeps animating (Chromium does not stop GIFs). |
| Rendering in production | compositor animations at display refresh (164 Hz here), plus GIF repaints at ~7.7 Hz. |

## 7. Production theme architecture

| Key | Values | Default | Fallback |
|---|---|---|---|
| `study-tracker-style` | `modern`, `field-notebook`, `wabi-sabi` | `field-notebook` | unknown → `field-notebook` |
| `study-tracker-palette` | 16 ids (`default` … `sakura`); legacy `parchment`→`paper`, `cosmic`→`retrowave`, `grove`→`forest` | `default` | unknown → `default` |
| `study-tracker-theme` | `dark` / `light` | `dark` (`|| "dark"`) | any other non-empty string becomes `data-theme` and **acts light** |

- Style and light/dark are separate concepts.
- Palette tokens are out-ranked by the Field Notebook and Wabi-Sabi token rules, so a Modern-only palette on those styles renders as default.
- Backup v2 carries these values as raw strings in `preferences`. `restoreBackup` sets each key that holds a string and leaves the others untouched.

## 8. Native Garden architecture

Deferred (§1).

## 9. Native theme architecture

```text
AppearancePrefs {style, palette, theme}  (core::appearance, pure; production ids and fallbacks)
      │ resolve()  → ResolvedAppearance {rendered style, ColorScheme, sakura, theme_locked, dark}
      ▼
appearance_view.rs  — the ONLY colour tables: fn_tokens / ws_tokens / chrome_tokens per ColorScheme
      ▼
Slint globals FN.t (FnTokens), WS.t (WsTokens), Chrome.t (ChromeTokens)  ← every surface reads only these
```

- `ui/fn/palette.slint` no longer holds `dark ? … : …` ternaries. Its defaults are the dark table, and the existing `FN.<token>` names are unchanged, so Stage 17 code needed only targeted fixes. Three tokens were added for Sakura: `ok/steady/critical` for the score pill, `rule-hard` (`--fn-rule-hard`) and `os-dark` (`color-scheme`).
- The menu and panel (`ui/chrome.slint`) are skinned by `ChromeTokens` derived from the active style's own tokens, not by `if wabi` branches.
- Semantic UI state (selection, quiet mode, submenu expansion) lives in Rust (`WabiUi`), separate from tokens.

## 10. Theme persistence

- **Storage:** a `preferences` section in the existing `store.json`: `{"appearance": {"style", "palette", "theme"}}`, written with production's ids.
- **Compatibility:** additive, with no schema bump. Stage 15–18 stores (no section) load production defaults, and older builds round-trip the section through `other`.
- **Tolerance:** each field is parsed independently with production's fallbacks; unknown keys in the section are preserved.
- **Single writer:** `PreferencesController` writes **only on an actual change**.
  - Measured: 1 write for an applied preference; 0 writes for navigation, animation, minimize/restore or Dashboard viewing.
  - The 500-switch stress writes exactly one store save per switch (§28).
- **Tests:** restart round-trip; unchanged → no write (no file created); unknown/future values; other sections kept; null port.

## 11. Production import

- `DiscoverOutcome` now carries the backup wrapper's `preferences`.
- `commit_import` merges `study-tracker-style`, `study-tracker-palette` and `study-tracker-theme` key by key, exactly like `restoreBackup` (strings only, absent keys keep the current value, legacy aliases mapped). It verifies the result on read-back and rolls back on mismatch. The log reports `appearance_imported`.
- The other preference keys (dashboard layouts, garden variant, rest tree, circle competitive) belong to unmigrated features. They are not imported, consistent with Stage 15's "reserved, not written" policy.

**Stage 16 importer fixes found by Wabi-Sabi parity:**

| Bug | Fix |
|---|---|
| timetable `completedOccurrences` and `occurrenceOverrides` were dropped | imported |
| to-do `completedOccurrences`, `skippedOccurrences` and `occurrenceTimes` were dropped | imported |
| time-less events were kept as 00:00 (production drops them) | dropped |
| events whose task does not exist were kept (production drops them) | dropped |
| unknown kinds were kept (production drops them) | dropped; legacy kinds mapped |
| `unitTypeId` was ignored | used as the `taskId` fallback |
| to-dos without a title were kept (production drops them) | dropped |

These fixes also correct the Field Notebook Dashboard's schedule-health score for anyone with a timetable.

## 12. Garden derivation

Deferred. However, porting the Wabi-Sabi Dashboard exposed an academic rule that was **missing since Stage 16**:

- **Production rule** (`App.tsx` effect over `timetableEvents/tasks/semesters/holidays`): a task with timetable events has its `totalUnits` derived from its non-release occurrences projected over the semester, and its `completedUnits` from the ticked dates.
  - Prep tasks keep their hand-set total.
  - Without semester dates, the event records are counted instead.
- **Native port:** `AcademicState::sync_task_units_from_schedule`, plus `count_event_occurrence_dates`.
- **Where it runs:** in `convert_academic`, on load (in memory, no write) and inside `AcademicController::persist` (every mutation).
- **Evidence:** this is why production shows "Sheet 1 of 1" for a task stored with 6 units.

Also ported: `toggleTimetableOccurrence` (with `unitDecrementFor`) and `toggleDailyTodoOccurrence`.

## 13. Sakura rendering architecture

```text
core::appearance::sakura (pure, golden-tested)        sakura_controller.rs (one slint::Timer)
 production_petals(): 22 PetalParams (mulberry32)  →   tick: t = now − origin
 petal_pose(params, t, vh): fall + sway + fades          poses → 22 fixed model rows (set in place,
 texture_frame(t): ⌊t/130⌋ mod 20                          unchanged rows skipped)
                                                          texture-frame property
 Slint ui/sakura.slint: SakuraPetals (repeater over 22 Images, rotation, clip), SakuraTexture (tiled)
```

- **One clock** for every petal layer: the window layer and the Quiet-overlay layer share one model.
- **Bounded:** 22 rows created once. The `live_petals` stat is 22 throughout every stress run.
- **Production-exact motion:** a port of the CSS keyframes, ease-in-out, alternate and negative delays. Against Chromium frozen at `currentTime = 9000 ms`, 8 sampled petals match within **1 px and 1°** and opacity within 0.002 (unit test `poses_match_chromium_frozen_at_nine_seconds`).
- **Opacity without layers:** each petal's opacity is applied by picking a pre-multiplied alpha variant of its image (22 levels, step 0.02, generated once at runtime). The texture's 0.14 is baked into the frame PNGs.
  - Measured: per-petal `opacity` layers cost **~7 % of a core and ~5 MB** at 30 Hz.
- **No allocation per frame** beyond the 22 refcounted image handles.
- **Assets:** `assets/sakura/petal-{0,1}.png` (production's blossoms scaled to fit 72 px) and `leaves-00..19.png` (production's GIF frames), derived by `scripts/stage19-sakura-assets.mjs` with no npm packages.

## 14. Animation lifecycle

| State | Behaviour (measured, §26) |
|---|---|
| A. Wabi-Sabi (or Field Notebook) + Sakura, visible | clock runs at 24 Hz |
| B. Wabi-Sabi or Field Notebook, default palette | no clock; static (0 frames) |
| C. Modern stored (not drawn natively) | default style drawn, no Sakura (production never shows Sakura on Modern) |
| D. Minimized | clock stopped: **0 Sakura ticks, 0 frames**, 0.08 % CPU (only the Timer's own 100 ms tick, which pushes nothing while minimized) |
| E. Hidden to tray | clock stopped: **0 ticks, 0 frames, 0 % CPU** |
| F. Debug lab on screen (text spike / map lab) | clock stopped, 0 frames (production has no such surfaces; every production surface shows petals, like its app-level scatter) |
| G. Shutdown | clock owned by the runtime, dropped with it |
| Reduced motion (Windows "Show animations" off = Chromium's `prefers-reduced-motion`) | static petals at 20 %; texture still animates at its own 130 ms, as in production |

- **Time origin:** set when the effect is switched on, like a fresh `SakuraScatter` mount. Hide/show does **not** reset it: a restore evaluates the pose at the current time once, with no catch-up frames. A palette or style switch restarts it, like a remount.

## 15. Visibility, minimize and tray

The rule `enabled && production surface && window visible` is re-evaluated on:
- preference changes;
- surface changes (`changed show-*` in Slint);
- Stage 18's hide-to-tray and show paths;
- `WM_SIZE`, `WM_SHOWWINDOW` and `WM_SETTINGCHANGE`, received through a comctl32 subclass of the main window (`win_host::watch_main_window`).

There is no polling. Window state is read with `IsWindowVisible && !IsIconic` plus the tray flag. The watcher is installed from inside the event loop because winit creates the HWND lazily (a first attempt before the loop found no window; fixed with a bounded retry).

## 16. Frame invalidation

- Static Wabi-Sabi Dashboard, static Field Notebook, Timer view: **0 frames** in 20 s windows.
- `WabiController` recomputes only on academic revision, day, selection or quiet-pick change. 5,000 simulated ticks cause 0 recomputations (test).
- The Timer tick reuses the preset-cards model (Rule A, test).
- Theme switching neither rebuilds Sakura state nor creates controllers (clock starts minus stops ≤ 1 at every point).

**Finding (Slint 1.17 backend, not Study Tracker code):** on Windows, `i-slint-backend-winit`'s `TimerBasedFrameThrottle` re-requests one more redraw after every change, and femtovg fully re-renders it. Every UI update therefore costs **two** rendered frames:
- 300 Sakura ticks gave 600 frames.
- With no property write per tick: 0 frames, 0.08 % CPU.

This affects all Slint UI updates. A Slint upgrade or upstream fix should be checked in Stage 23.

**Also found:** `scripts/stage17-perf.ps1`'s STATS regex had lost its backslashes (`STATS (d+)`), so its "frames in window" column could never count anything. The Stage 17 "0 frames" figures were therefore not measurements. Stage 19's script parses correctly, and confirms that static views really render 0 frames.

## 17. Assets and fonts

- **Reused production files:**
  - `desktop/public/196-…-cherry-blossom….png` and `…sakura-flower-png-kawaii-….png` → `assets/sakura/petal-0/1.png` (downscaled);
  - `sakura-leaves-ezgif.com-gif-maker.gif` → 20 PNG frames.
- **Licensing risk:** these come from production's repository, but their file names suggest third-party web downloads. Provenance and licence should be confirmed before shipping, as for production itself.
- **Existing project SVGs reused:** sun/moon and the dotted rule.
- **Fonts:** no fonts bundled. Georgia, Arial, Consolas and Microsoft YaHei are what production actually renders (§5).

## 18. Visual parity methodology

1. **Fixtures:** `gen-fixture.mjs` gained a `wabi` scenario (realistic plus weekly lectures, sheet release/deadline pairs, timed, any-time and repeating to-dos, and an exam-prep task). The three Stage 17 fixtures regenerate byte-identically. There are two derived variants: `wabi-noplan` (no planned units) and `wabi-sheets` (sheets only). All data is synthetic.
2. **Production capture:** `capture-prod.mjs --style wabi-sabi [--palette sakura] [--quiet 1] [--tab timer] [--clicks …] [--anim-time 9000]`, with frozen `Date`, `Europe/Zurich`, 1520×980, DPR 1. `--anim-time` pauses every animation at one time.
3. **Native capture:** `capture-native.ps1 -Style -Palette -Quiet -SakuraTime 9000 -Extra`. It uses an isolated data dir, the same fixture imported, `STUDY_NATIVE_NOW` and the same size. `STUDY_NATIVE_SAKURA_TIME` renders the petals at exactly that animation time with no clock.
4. **Comparison:** `imgtool.mjs diff/side` (mean absolute difference 0–255; a similarity indicator, not a pixel-perfect claim).
5. **Golden text:** production `innerText` of the sidebar, Dashboard and Quiet mode for four fixtures (`tests/fixtures/dashboard/production-text/wabi-sabi-*.txt`) is compared literally in `wabi_view` tests.
6. **Sakura caveat:** only the GIF texture's frame phase cannot be frozen in Chromium (`getAnimations()` does not include GIFs). The petals are frozen and compared at t = 9 s.

## 19. Visual parity results (1520×980, DPR 1)

| Configuration | Mean abs diff |
|---|---|
| Wabi-Sabi Dashboard, light | **0.68** |
| Wabi-Sabi Dashboard, dark | **0.98** |
| Wabi-Sabi Timer, light (Pomodoro selected) | **0.69** |
| Wabi-Sabi Quiet mode, light | **0.67** |
| Wabi-Sabi + Sakura, petals frozen at 9 s | **0.81** (petal region crop 0.24) |
| Field Notebook + Sakura, frozen at 9 s | **1.48** |
| Field Notebook default, realistic Quiet (Stage 17 regression) | 1.56, unchanged |
| Wabi-Sabi menu | 0.99 |
| Field Notebook menu | 1.68 |
| Field Notebook Theme panel | 2.35 |
| Wabi-Sabi Style panel | 6.30 (production blurs the backdrop) |

## 20. Region table

| Region | Verdict | Notes |
|---|---|---|
| Sidebar: brand, score ring, toolbar | MATCH | |
| Sidebar: navigation + kanji | MATCH | kanji in Microsoft YaHei, as in production |
| Sidebar: Timer submenu (semester → courses → tasks, Session logs) | CLOSE | task rows never show "selected" (no Timer task linking); Session logs inert |
| Sidebar: foot (tended today, goal bar, Quiet link) | MATCH | |
| Dashboard: TODAY head + remaining | MATCH | |
| Dashboard: ONE THING (all three fallbacks) | MATCH | title box height corrected for Slint's taller Georgia line |
| Dashboard: COMING UP | MATCH | |
| Dashboard: PLANNED TODAY (+ done strike, selected bar) | MATCH | strike-through drawn as a hairline |
| Dashboard: SOMETHING ELSE | DIFFERENT | drawn, inert (picker not migrated, as in Stage 17) |
| Timer: head, clock, actions, phase line, cards | CLOSE | Sprint and Custom drawn unavailable (not in the native Timer); "SWITCH TO BREAK →" inert; the clock is not editable; native default preset is Deep Work (Stage 14 divergence) |
| Quiet mode | MATCH | dropdown implemented (other open tasks) |
| Sakura petals (positions, rotation, opacity, layering, clipping) | MATCH | frozen-frame comparison; 24 Hz instead of display refresh (§26) |
| Sakura texture | CLOSE | same frames, timing and opacity; frame phase arbitrary in both |
| Field Notebook + Sakura palette | CLOSE | "Something else" dim (Stage 17 inert); the Full layout's 2 px head rule kept ink (unverified token) |
| Menu dropdowns | MATCH | Personal, Options and Settings inert |
| Theme / Style panels | CLOSE | no backdrop blur (Slint cannot); Modern listed but not selectable |
| Wabi-Sabi + Sakura texture under the sidebar | MATCH | |
| Scrollbar | DIFFERENT | thin native indicator in the reserved 10 px gutter; production shows WebView2's scrollbar (hidden in headless captures) |
| Knowledge Garden | N/A | deferred to Modern (§1) |

## 21–25. Comparisons

| Comparison | Files | Result |
|---|---|---|
| Empty | `wabi-sabi-empty.txt` | golden text MATCH: "2h to today's goal", placeholder ONE THING, no actions, empty PLANNED TODAY; score 0 "Critical", as in production |
| Populated | `wabi`, `wabi-noplan`, `wabi-sheets` | golden text MATCH, including ONE THING "Email the tutor · To-do · 09:30" (timeline fallback) and "Exercise Sheet 1 1 · Programming · due Sep 30" (deadline fallback, production's doubled number preserved) |
| Wabi-Sabi Dashboard | §19 | 0.68 light, 0.98 dark |
| Wabi-Sabi Timer | §19 | 0.69 |
| Sakura | §13, §19 | Chromium-pose unit test plus frozen frame 0.81 |

Screenshots (synthetic data only) are in `docs/stage19-screenshots/`.

## 26. Performance (release, this machine, 20 s windows)

The machine has a 164 Hz monitor. Frames are counted by `STUDY_NATIVE_FRAME_STATS`.

| Scenario | Procs / threads | Private WS / Private Bytes | CPU (1 core) | Frames/s | Sakura ticks/s | Store written |
|---|---|---|---|---|---|---|
| G19-P0 fresh default style, idle | 1 / 14 | 47.5 / – MB | 0.08 % | 0 | 0 | no |
| G19-P1 Field Notebook realistic | 1 | 47.4 | 0.00 % | 0 | 0 | no |
| G19-P2 Wabi-Sabi static | 1 | 47.0 | 0.00 % | 0 | 0 | no |
| G19-P3 Wabi-Sabi + Sakura (24 Hz) | 1 / 14 | 64.8 | **12.7 %** | 48 | 24 | no |
| G19-P3b Field Notebook + Sakura | 1 | ~65 | ≈ P3 | 48 | 24 | no |
| G19-P3r reduced motion | 1 | 65 | 4.1 % | 15 | 7.7 | no |
| G19-P4 Sakura + Timer running | 1 | ~70 | ≈ P3 + 0.1 | 48 | 24 | no |
| G19-P5 same, minimized | 1 | 72.8 | **0.16 %** | **0** | **0** | no |
| G19-P6 same, hidden to tray | 1 | 69.4 | **0.00 %** | **0** | **0** | no |
| G19-P7 Sakura palette, debug lab | 1 | 47.3 | 0.00 % | 0 | 0 | no |
| G19-P8 500 style switches | 1 / 14 | 68.4 → 69.8–70.1 | 31 % while switching 25×/s | – | – | yes, 1 per switch (§28) |
| G19-P9 200+200 hide/show cycles | 1 / 14 | 63.3–63.6 | §29 | 0 hidden | 0 hidden | no |
| G19-P10 30-minute soak | 1 / 14 | 68.5 → 69.0 | 12.9–13.7 % | 49 | 24 | no |
| G19-P11 Wabi-Sabi, stress dataset | 1 | 47.3 | 0.00 % | 0 | 0 | no |

**Frame rate.** All rows are Wabi-Sabi + Sakura, measured after the alpha-variant optimisation. Frames are always twice the ticks because of Slint's throttle (§16).

| Cadence | CPU (1 core) | Frames/s |
|---|---|---|
| 165 Hz (≈ display refresh) | 30.3 % | 160 (vsync-bound) |
| 60 Hz | 20.8 % | 123 |
| 30 Hz | 14.5 % | 60 |
| **24 Hz (default)** | **12.7 %** | 48 |
| 20 Hz | 10.9 % | 40 |
| 15 Hz | 7.5 % | 30 |

- At 24 Hz a petal moves at most 3.2 px and 1.6° per frame. Petals are 16–36 px at 22–42 % opacity, and look the same as the 60 and 165 Hz runs.
- Rendering at monitor refresh would cost 2.4× the CPU for no visible gain, so the high-refresh display is deliberately **not** followed.
- GPU utilisation was not measured with sufficient precision to report.
- Before optimisation, the 22 opacity layers added about 7 points at 30 Hz (19.8 % → 12.7 % with alpha variants removed).

## 27. 30-minute soak (G19-P10)

Setup: Wabi-Sabi + Sakura visible, Timer running (120 min exam) on the Wabi-Sabi Timer, release build at 24 Hz. Memory and CPU are averaged over each 5-minute interval.

| t (min) | 0 | 5 | 10 | 15 | 20 | 25 | 30 |
|---|---|---|---|---|---|---|---|
| Private WS (MB) | 68.5 | 69.7 | 69.9 | 70.2 | 69.2 | 68.7 | 69.0 |
| CPU (1 core) | 12.9 % | 13.0 % | 13.5 % | 13.5 % | 13.7 % | 13.5 % | 13.4 % |

- Private Bytes 154 MB; 14 threads.
- 49 frames/s (24 ticks); 22 live petals; store not written in the measured window.
- **Plateau, no growth.**
- Switching away afterwards stops the clock: G19-P5/P6/P7 and the lifecycle holds (§29) show 0 frames and 0 ticks.

## 28. Theme-switch stress (G19-P8)

- **Run:** `STUDY_NATIVE_THEME_STRESS=250` performs **500 switches** Field Notebook ↔ Wabi-Sabi with Sakura on, every 40 ms, through the panel's own `set_prefs` path.
- **Writes:** `THEME_STRESS done switches=500 prefs_writes=501` (500 switches plus the initial palette override), one store write per real change. The final choice persisted.
- **Memory:** Private WS 68.4 MB during the run, then 69.8 / 69.9 / 70.1 / 70.0 / 70.0 MB over the next 75 s. Plateau.
- **No duplicate loops:**
  - CPU afterwards settles at the single-clock level (10.9–12.0 %, Sakura on Field Notebook).
  - 22 live petals.
  - Clock starts minus stops ≤ 1 throughout.
  - The Sakura model is created once; switching never reallocates it.

## 29. Hide/show stress (`scripts/stage19-lifecycle.ps1`, Wabi-Sabi + Sakura, Timer running)

- **200 tray hide/show cycles** (real `WM_CLOSE` plus the tray icon's click message) and **200 minimize/restore cycles** (`ShowWindow`): 0 failures, 1 process.
- **Clock:** 401 starts and 400 stops after 400 transitions, so the clock restarts exactly once per show and never duplicates. 22 live petals throughout.
- **Memory:** Private WS 63.3 → 63.4 → 63.3 → 63.6 MB across the tray phase, and 63.6 MB flat across the minimize phase. Private Bytes 141.4–141.8 MB, with a transient 158.2 during the minimize phase that returned to 141.9 MB. Threads 14 → 9 at the end.
- **Holds:**
  - visible: 14.3 % CPU (24 Hz);
  - hidden to tray: **0 % CPU, 0 frames, 0 Sakura ticks**;
  - minimized: **0.08 % CPU, 0 frames, 0 ticks**.
- **Timer:** 98–100 ticks every 10 s throughout (the Timer keeps running logically).

## 30. Startup (release, `STUDY_NATIVE_STARTUP_REPORT`, 5 launches each, median)

| Profile | First frame |
|---|---|
| fresh, default style | **161.2 ms** (158.4–167.5) |
| realistic Field Notebook, stored | **163.0 ms** |
| Wabi-Sabi + Sakura, stored | **150.7 ms** |

Stage 16 was 160.6 ms and Stage 17 160–190 ms, so startup is unchanged.
- Wabi-Sabi data is computed only when that style is drawn; Field Notebook users pay nothing for it.
- The petal alpha variants are built once at startup: 2 × 22 images of at most 72 × 72 px.

## 31. Persistence churn

- Animation, navigation, viewing, minimize and tray hold: store `mtime` unchanged in every window of §26.
- A preference change writes exactly once. An unchanged selection writes nothing (test: no file created).
- Wabi-Sabi marks write through the academic controller once per toggle (test).

## 32. Accessibility and input

- Petals and texture are `Image { accessible-role: none }` with no `TouchArea`, so clicks pass through to the controls beneath.
- The Wabi-Sabi controls underneath stay clickable; Timer buttons were exercised under Sakura during the stress runs.
- Task marks expose `accessible-role: checkbox` with checked state and a label. Nav items are `tab`, toolbar buttons are `button`.
- Reduced motion follows Windows "Show animations" (`SPI_GETCLIENTAREAANIMATION`, which is what Chromium maps to `prefers-reduced-motion`) and reproduces production's media query. It is re-read on `WM_SETTINGCHANGE`. `STUDY_NATIVE_REDUCED_MOTION=1` forces it for tests.
- `displayTime` 12/24 h follows the Windows display language's time format, approximating Chromium's locale-derived `hour12` (`STUDY_NATIVE_12H` overrides it).

## 33a. Timer integration under Sakura

Setup: Demo preset (10 s) running on the Wabi-Sabi Timer with Sakura visible. It completed inside the run.
- Sessions 93 → **94**, exactly one; lifetime minutes +1.
- **One** notification ("FocusFinished").
- **One** Dashboard recompute (revision 0 → 1).
- No duplicate StudySession.
- Sakura kept running (22 petals); the Timer kept ticking (98–100 per 10 s) through every hide/show and minimize cycle (§29).
- The relaunch restored the Timer idle with the preference persisted.

The Timer stays timestamp-based: Sakura never touches Timer state, and Timer ticks never touch Sakura state.

## 33. Stage 18 regression

- Tray hide/show: 200 cycles, 0 failures (§29).
- Hidden window: 0 frames.
- `stage18-lifecycle.ps1` re-run with Wabi-Sabi + Sakura active:
  - 8 simultaneous launches leave **1 survivor**;
  - 50 tray cycles with 0 failures;
  - **20 second launches** all restore the primary (median 1,006 ms) with one process;
  - tray **Quit** exits the process and removes the icon.
- Single process throughout. The tray window was found by the same per-profile class name.
- Updater, notification and single-instance code are unchanged; their tests pass (§34).
- Completion notification paths are unchanged: `CompleteManually` (LOG AND CLOSE) produces no notification, as before for manual saves.

## 34. Tests

| Suite | Count |
|---|---|
| `study-tracker-core` (was 90) | **122** (+32): appearance prefs/resolve/fallbacks (11); Sakura mulberry32 bit-exact vs Node, ranges, Chromium poses, fades, periodicity, ease-in-out, reduced motion, texture frames (7); Wabi-Sabi Dashboard/sidebar/Quiet semantics, prep expansion, semester stage, schedule sync, toggles, `unitDecrementFor`, moved-occurrence quirk, 12 h time (15) |
| native app (was 187) | **212** (+25): preferences persistence/import (7); appearance tokens/cards/arc (4); Sakura controller rule/bounded/origin/no-replay/reduced motion (5); Wabi-Sabi goldens ×4, importer drops/keeps, caching, row actions, controller toggle → Dashboard, timer cards Rule A (9) |
| Total | **334 passed**, 1 ignored (pre-existing), 0 failed |

Checks:
- `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo test -p study-tracker-core` and `cargo build --release` all pass.
- The release build has no warnings.
- The test build shows one **pre-existing** warning (unused helper `region` in `src/map/tests.rs`, from the map lab). It is not introduced by this stage.
- `scripts/stage17-perf.ps1`'s broken STATS regex (§16) is fixed.

## 35. Dependencies

**Zero new crates.** One feature flag was added to the already-present `windows-sys` (`Win32_Globalization`, display-language time format). The subclass, `SystemParametersInfo` and window queries use already-enabled features. No animation engine, WebView or GPU framework.

**Release exe: 20,233,216 B (Stage 18) → 23,001,088 B, +2,767,872 B (+13.7 %).**
- The growth is dominated by Slint's generated code for the new surfaces (the generated `main.rs` is 8.7 MB of source).
- The Sakura assets are 19 KB.
- If size becomes a concern, Slint's code generation is the lever, not the assets.

## 36. Production integrity

`git diff -- desktop` is empty. The production web build was made into a scratch directory, and the headless browser used throw-away profiles.

## 37. Known differences

- No backdrop blur behind the panels.
- Timer: Sprint and Custom unavailable; "Switch to break" inert; clock not editable; native default preset is Deep Work; no task linking, so the heading is always the goal or "General focus".
- Petals render at 24 Hz instead of display refresh.
- The scrollbar is a thin native indicator.
- Hover micro-transitions are instant.
- SOMETHING ELSE, Plan/Notes/Rest/Circle, Session logs, the help `?` and Personal/Options/Settings are inert.
- The Modern style can be stored and imported but renders the default style.
- The Full layout's 2 px head rule stays ink under Sakura (token unverified).
- 12/24 h is derived from the display language rather than Chromium's ICU locale.

## 38. Deferred work

- **Knowledge Garden** (all four variants, persisted cycle, achievements input) and the **Modern** dashboards that host it.
- **Timer (Stage 23):** task linking, Sprint/Custom presets, switch-to-break, editable clock, default-style Timer parity.
- The "Something else" picker; Planner, Notes, Rest, Circle; Session logs; Personal/Options/Settings.
- A Slint upgrade or check for the double-redraw throttle (§16).
- Asset licence confirmation (§17).
- The manual Windows-sleep Timer check carried over from Stage 18. It was not performed in this session and remains a Stage 23 verification item.

## 39. Risks

- Slint's double redraw doubles the cost of every UI update until it is fixed upstream.
- Sakura costs ~12.7 % of one core while visible (about 0.8 % of this 16-thread machine). It is bounded and stops completely when invisible, but it is not free.
- Asset provenance (§17).
- Real-profile edge cases (DST, many timetable overrides) are untested because no real data may be used.
- The importer fixes change Field Notebook numbers for timetable users. This is the correct production behaviour, but it is a visible change.

## 40. Verdict

**PASS WITH CONCERNS.** The corrected Stage 19 scope is complete:
- Wabi-Sabi shell, Dashboard, Timer and Quiet mode, verified against production text (exact) and pixels (0.7–1.0).
- Sakura on both styles, production-exact motion, one bounded clock, zero frames when invisible.
- Centralized, persisted and importable appearance system.

Concerns:
- Sakura's visible CPU cost, inflated by Slint's redundant redraw.
- Inert extras (picker, menus, timer extras) and the absence of panel blur.
- Asset licensing to confirm.
- The deferred sleep test.
