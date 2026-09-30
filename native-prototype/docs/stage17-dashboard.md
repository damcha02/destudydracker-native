# Stage 17 — Production Dashboard migration and visual parity

Status: implemented and verified, **uncommitted** (Stage 16 is committed separately as `5093b36`).
Production reference: Study Tracker **v0.1.66**, upstream `b095706994b6caaf18432d0d41b4534f4b85be98` (`desktop/`, read-only).

## 1. Scope

Replace the Stage 10 synthetic Dashboard (deleted: `src/dashboard.rs`, `ui/dashboard.slint`, `ui/dashboard-types.slint`, six chart components) with the real production Dashboard, backed by `AcademicState` and reproducing production's layout, metrics, typography, colours, charts and interactions.

**Which production Dashboard.** Production ships three app styles (`modern`, `field-notebook`, `wabi-sabi`). Its default — `loadAppStyle()` returns `"field-notebook"` when nothing is stored — is the **Field Notebook** style, with a *Quiet* layout (default) and a *Full* layout. That is what every new installation shows, and it is what this stage reproduces. The Modern style (Focus/Cockpit/Analyst/Custom layouts, Knowledge Garden) and the Wabi-Sabi style are non-default, selected through a style picker that has not been migrated; they are **deferred** (inventory §2, "Deferred").

Not in scope: Stage 18 (tray, notifications, updater, single instance), the Planner, the Timer surface's restyling, settings/preferences persistence.

## 2. Production Dashboard inventory

Read from the implementation (`App.tsx` `renderFieldNotebook*Dashboard`, `renderTodayCard`/`renderWeeklyChart`/`renderCourseRadar`/`renderExamRunway`/`renderUrgentTasks`, `lib/metrics.ts`, `lib/scheduleWorkload.ts`, `lib/scheduleHealth.ts`, `lib/plannerSchedule.ts`, `App.css`, `index.css`) **and measured from the running app** (computed styles/geometry via DevTools protocol, see §12).

| Region | Data shown | Calculation source | Native decision |
|---|---|---|---|
| Top bar: "STUDY TRACKER", tilted **overall-score stamp**, theme toggle, menu button | score + label (`Strong/Steady/Watch/Critical`) | `getScheduleHealth` over active semesters, else `getOverallHealth(activeTasks, activeExams)` | implemented; menu button inert (menu panel deferred) |
| Tab row (sticky): Dashboard · Planner · Timer · Vault · Break Room · Social · `?` | – | – | Dashboard + Timer work; the other four are drawn but inert (unavailable state); `?` inert |
| **Quiet**: stamp line, "On the desk" title, selected-block subtitle, view switch | date label, ISO-ish week number, selected queue entry | `getFieldDashboardData`, `todayCalendarEntries` | implemented |
| Quiet action row: *Start focus*, *Something else*, "53m of 2h done today." | today minutes, goal | `getTodayMinutes`, `settings.dailyGoalMinutes` | implemented; *Start focus* navigates to Timer (task→Timer context link deferred); *Something else* picker **deferred** (drawn dimmed, inert) |
| Quiet planned list (≤6 open entries, select row, tick checkbox, "+N more planned · open Full") | title, course, due/time | `todayCalendarEntries`, `toggleCalendarEntry` | implemented incl. real ticking (`AcademicState::toggle_calendar_entry`) |
| Quiet side: **Pace** (units left, units/day, open tasks, planned today), **Ahead** (≤4 exams + nearest deadline), **Focused today** ledger | see §3 | `calculateScheduledWorkload`, `getUpcomingExams`, … | implemented |
| **Full** head: "Today's desk", stamp, subtitle, focus ledger, view switch | see §3 | as above | implemented |
| Full **study queue**: selected-block sheet + planned list (≤5, incl. done) with chips, meta, checkbox, *Focus* | `getCalendarEntryUnitLabel`, priority chip | `todayCalendarEntries`, `formatUnitAmount` | implemented; *Focus* navigates to Timer (link deferred) |
| Full **course ledger** (active courses: health bar, semester • tasks • time • target • overdue) | `courseHealthByCourseId` (+schedule override) | `calculateWorkloadHealth`, `withScheduleHealth` | implemented |
| Full **pace ledger** | units left / open / units per day / "logged this week" | as above | implemented |
| Full **exam runway** (≤4: days, title, course • weight, preparedness bar) | `getUpcomingExams` | – | implemented |
| Full **margin note** (tilted slip) | earliest exam text | – | implemented |
| Full **weekly rhythm / focus history**: range toggle (week, 7, 14, 30, 60, 1y), stratified columns per course, hover tooltip, milestone ("fossil") markers + popover, legend, discovered, stats, empty state | `focusTimeline` | sessions, courses, lifetime minutes | implemented (except glow effects, §24) |
| Footer note ("83H 26M LOGGED ACROSS 46 DAYS SINCE JUL 23") | lifetime minutes, distinct session days, first session | `lifetimeStudyMinutes`, `getSessionDaySet`, `getFirstSessionDate` | implemented |
| Empty states (Quiet/Full/chart/courses/exams) | messages | – | implemented, text verified against production |
| Light/dark theme | palette | `data-theme` | both implemented; in-session toggle, **not persisted** |
| "Something else" picker modal, Welcome/tour/What's-new modals, menu panel, page-help tour, announcements/update toast | – | – | **deferred** (need Planner rows / Options / network) |
| Modern style Dashboard (Focus, Cockpit, Analyst, Custom, Knowledge Garden, momentum chip, "Study health"), Wabi-Sabi Dashboard | – | – | **deferred** — non-default style; needs the style picker (Options migration) |

Anything `Stage 16` does not hold yet: `settings` (`dailyGoalMinutes`, `userName`, accent, `visibleTabs`) — native uses production's defaults (goal 120 min; accent `#8fb4ff`). No schema change was made (§22).

## 3. Production metric semantics (ported, not re-derived)

Pure Rust in `study_tracker_core::dashboard` (`metrics.rs`, `schedule.rs`, `focus.rs`, `format.rs`, `civil.rs`). Each function is a transliteration of the named TypeScript function.

| Displayed | Production source | Native |
|---|---|---|
| Focused today | `getTodayMinutes`: sum of `minutes` of **every** session (any kind) whose *local* `endedAt` date is today | `SessionDays::minutes_on` |
| Goal % | `clamp(round(today/goal*100),0,100)`, goal = `max(1, dailyGoalMinutes ?? 120)` | `goal_progress_percent` |
| Streak | `getStreakDays` | `SessionDays::streak_days` (§6) |
| "13h 35m logged this week" / "… this week" | `weeklyActivity` sum: **rolling last 7 days**, not the Monday week | `weekly_total_minutes` |
| Lifetime / footer | `state.lifetimeStudyMinutes` (survives pruning), `sessionDays.size`, first `startedAt` | kept verbatim |
| Week N | `ceil(((now − Jan1_local)/86400000 + Jan1.getDay() + 1)/7)` with the *current instant* | `week_number` |
| Open tasks / units left | all `state.tasks` with remaining>0 (archived semesters **included**) | `open_task_count`, `total_units_left` |
| Units/day | `calculateScheduledWorkload(activeTasks, scheduledUnits, today).unitsPerDay`: calendar units/7 over overdue+next 7 days, legacy due-date pace for unscheduled tasks | `schedule.rs` |
| Course score | `calculateWorkloadHealth(courseTasks, courseExams)` then `getScheduleHealth` override | `workload_health`, `with_schedule_health` |
| Overall score | schedule health (active semesters) else completion health; `0` for no tasks | `overall_score` |
| Exams | all exams with `daysUntil ≥ 0`, soonest first, 4 max (archived included) | `upcoming_exams` |
| Nearest deadline | smallest `dueDate` string among unfinished dated tasks (may be overdue); else next exam | `nearest_deadline_label` |
| Queue | entries dated today with a task or ad-hoc title; open first, then `(startTime ?? createdAt)` string order | `DashboardMetrics::queue` |
| Chart | `focusTimeline` (§8) | `focus_timeline` |

**Production quirks found and their treatment** (Stage brief: document before deciding; nothing silently "fixed"):

| # | Quirk | Decision |
|---|---|---|
| PQ-1 | `daysUntil`/`formatDate` parse date-only strings as **UTC midnight** then read them in local time: correct east of UTC (Zurich), **one day early west of UTC** | **preserved** (`js_date_only_as_local`), unit-tested both ways; candidate fix after cutover |
| PQ-2 | Empty profile shows "Overall score **Critical 0**" (health 0 → "Critical") | preserved (verified against production) |
| PQ-3 | "This week" is Monday-based in the chart but *rolling 7 days* in the pace note/section caption | preserved |
| PQ-4 | Exams, nearest deadline, open-task count and "units left" span **all** semesters; pace, radar and health use **active** ones only | preserved |
| PQ-5 | Streak counts any session (break kind, 0 minutes, recovered) | preserved, tested |
| PQ-6 | Queue order compares `"HH:MM"` with an ISO timestamp as plain strings | preserved (`to_iso_utc_string` reproduces `toISOString`) |
| PQ-7 | Week number uses the real current instant (may tick over mid-day) | preserved |
| PQ-8 | "Start focus" renders as an *unstyled* OS button (only `.ghost-button` is styled) | preserved visually |
| PQ-9 | Chart uses real `now`, other metrics use the day-boundary `calendarToday` | equivalent here (one clock) |
| PQ-10 | Invalid date strings produce `NaN` in production | defensive divergence: such a task is ignored in the pace (a date picker cannot produce them) |
| PQ-11 | The packaged app's CSP (`font-src 'self' data:`) blocks the web fonts named first in the CSS; real rendering is **Georgia / Consolas / Arial** | reproduced (§14) |

## 4. Native metrics architecture

```text
AcademicController (revision counter, persistence)   study_tracker_core::dashboard (pure, no I/O, no clock)
        │ state()                                        civil.rs    CivilDate + LocalClock trait (only timezone boundary)
        ▼                                                format.rs   formatMinutes, toFixed, Intl formats …
DashboardController (src/dashboard_view.rs)              metrics.rs  ported metrics.ts
  cache key (revision, local date, goal) ──────────────► schedule.rs ported plannerSchedule/scheduleWorkload/scheduleHealth
  DashboardMetrics + FocusTimeline                       focus.rs    focusTimeline
        │ build_data()                                   mod.rs      DashboardMetrics::compute
        ▼
  FnDashboardData (Slint struct) ──► ui/fn/*.slint (draw only)
```

No statistic is computed in Slint, from persistence JSON, or by widgets; nothing is persisted; `AcademicState` is not duplicated. The clock boundary is a trait: production wiring uses `ChronoLocalClock` (per-instant OS timezone rules, like JS `Date`); tests use `FixedOffsetClock` so every date is deterministic.

## 5. Date / week semantics

* Days are local calendar days of the **instant** (`clock.local_date`), matching `isoDate(new Date(endedAt))`.
* Chart "This week" = Monday…Sunday containing today (`getDay()==0` → previous Monday); "7d…1y" = the N days ending today.
* Rolling "last 7 days" = today−6 … today.
* Verified boundaries (unit tests): midnight (session attributed to the local day it *ends* on, tested at UTC+2 and UTC), seven-day edge (day −7 excluded), month/year boundary, leap day (2028-02-29) in streak and in the 365-day range, 2100 not a leap year, Sunday belongs to the previous Monday's week, date rollover makes the cache stale. DST-adjacent dates: per-instant conversion via chrono; a true DST-transition test was not built (needs a timezone database dependency) — documented gap, affects at most which day a session near a DST change is counted on.

## 6. Streak

Active day = any session (study/break/exam, any duration, imported or recovered) ending that local day. If today has none, counting starts from yesterday; if neither has one the streak is 0; it stops at the first missing day. Tests: today+yesterday run, yesterday-only, two-days-ago (broken → 0), gap, zero-minute break session, month/year/leap boundaries, and the production value **9** for the realistic fixture.

## 7. Course breakdown

Radar rows = courses of non-archived semesters, in `state.courses` order, score per §3, time = all sessions with that `courseId`. Sessions referencing deleted courses or none render as the **"General"** layer in the chart (`--ink-4`), never crash, never vanish (tested with the fixture's `course-deleted-gone` and a course-less session). Colours are production's CSS strings (`#rrggbb`; a few named colours and a fallback are understood).

## 8. Chart architecture

Rust owns data (`focus_timeline`: days, per-course layers sorted largest first, milestone hits, legend, stats) **and geometry** (`dashboard_view::build_fossil`: column height `max(6, √(total/max(30,maxDay))·100)`, layer heights `max(4, share·height−1)`, 1 px stack gap, per-layer irregular corner radii from a **bit-exact port of `fossilRand`** — checked against Node for 7 seeds — column gap by range, date labels, milestone glyph index, tooltip content). Slint only draws rectangles/gradients/SVG glyphs from that data; hover tooltip and milestone popover state is purely visual and lives in Slint. Production's `grayscale(.25) contrast(.92)` card filter is applied to chart colours in Rust (`chart_filter`, unit-tested). Up to 365 columns render from ~1 500 plain elements; no chart/WebView library, no new dependency.

## 9. Invalidation / recomputation policy

`DashboardController::sync` recomputes when `(AcademicController::revision(), local date, daily goal)` changes; the chart timeline additionally when the range changes. The academic revision is bumped by `persist()`, the single choke point of every mutation. Measured/tested:

* 5 000 simulated 100 ms timer ticks → 0 recomputations (`unchanged_revision_never_recomputes…`).
* Quiet↔Full, row selection: metrics untouched; range change: chart only; Quiet never builds the chart.
* Timer-tick path cost: one integer comparison (`is_stale`). A 60 s Slint timer checks only the local date (writes nothing unless the day changed).
* Runtime: `D17-P3/P4` show exactly **1** `dashboard: recomputed` log line over 27 s of a running timer.

## 10. Timer → Dashboard flow

`TimerController` completion → `AcademicController::route_timer_effects` adds the `StudySession` (Stage 16) → revision bump → the existing 100 ms refresh path sees `is_stale` → `refresh_dashboard` → new `FnDashboardData`. `TimerController` never touches a widget. Verified on the release exe (`D17-P5`): a 10 s Demo completion produced `today_minutes 53→54`, `lifetime 5006→5007`, exactly 2 recompute lines (startup, completion), one store write, all without restart. Unit test `a_timer_completion_session_updates_the_dashboard_without_a_restart` covers the same path.

## 11. Production visual inventory (measured)

| Token | Dark | Light |
|---|---|---|
| desk / surface / surface-2 | `#191814` / `#24221e` / `#2b2823` | `#e6dfd1` / `#fbf8f0` / `#f4efe3` |
| ink / ink-3 / ink-4 | `#eee6d6` / `#aaa08d` / `#817867` | `#23211d` / `#6d6656` / `#8b8474` |
| line / line-soft / rule-dot | `#58503f` / `#3f392e` / `#584f3e` | `#cec3ae` / `#ded5c3` / `#d5cbb7` |
| stamp (warn) / steady / ok | `#c47a4d` / `#7187b4` / `#73956a` | `#9c5a34` / `#4a5f86` / `#4f6b4a` |

Layout: page width `min(1720, vw−32)`, desk padding 26/34/30 (18/16/24 ≤780), Quiet grid `minmax(0,760) | 1fr` gap 64, Full grid `1.45fr .92fr .72fr` (min 280/260) gap 24, single column ≤1180, stacked head ≤780. All in `ui/fn/palette.slint`, `full.slint`, `quiet.slint` with the measured pixel values.

## 12. Visual parity methodology (repeatable)

`scripts/visual-parity/`:

1. `gen-fixture.mjs` – deterministic **synthetic** production backup (empty / small / realistic), dates relative to 2026-09-30, fixed `+02:00`; committed under `tests/fixtures/dashboard/`. No personal data.
2. `capture-prod.mjs` – builds nothing in `desktop/` (a scratch `vite build --outDir <scratch>`), serves it, and renders it in a **throw-away headless Chrome profile** with: seeded `localStorage`, frozen `Date`, `Europe/Zurich`, Google-Fonts/update hosts blocked (the packaged CSP), DPR 1/1.25. Also dumps computed styles/geometry/CSS rules (`probe-*.js`) and the visible text (`tests/fixtures/dashboard/production-text/*.txt`, used as golden values).
3. `capture-native.ps1` – starts the native exe with an isolated `STUDY_NATIVE_DATA_DIR`, imports the same fixture, pins `STUDY_NATIVE_NOW`, `SLINT_SCALE_FACTOR`, size and layout, and saves the client-area PNG.
4. `imgtool.mjs` – dependency-free PNG decode, region crop/zoom, side-by-side + diff, mean-absolute-difference and tile grid.
5. `pair.sh` ties 2–4 together. Screenshots (synthetic only) are committed in `docs/stage17-screenshots/` as *production | native*.

Nothing here touches the installed production app, its WebView2 profile, its localStorage or its app-data.

**Numbers are mean absolute pixel difference (0–255) — a similarity indicator, not a "pixel perfect" claim.** Text antialiasing (DirectWrite/Chromium vs femtovg) guarantees non-zero differences on every glyph.

| Config | Mean abs diff |
|---|---|
| realistic Quiet 1520×980 | 1.56 |
| realistic Quiet 1920×1080 | 1.15 |
| realistic Quiet 1180×760 (production min window) | 2.42 |
| empty Quiet | 1.21 · small Quiet 1.27 |
| realistic Quiet light | 1.83 |
| realistic Quiet @125 % (this machine's real scaling; 1900×1225) | 1.77 |
| realistic Full 1520×1320 (whole page) | 5.00 |
| realistic Full 1180×980 (one column) | 3.77 |
| realistic Full 1920×1080 | 6.47 |
| empty Full 3.67 · small Full 3.63 | |
| realistic Full light 4.86 · @125 % 4.34 | |

## 13. Region-by-region parity

Rubric: **MATCH** = same structure, geometry within ~1 px, same fonts/colours, diff ≲ 2; **CLOSE** = same structure/fonts/colours, small offsets or rendering differences; **DIFFERENT** = visibly different by design/limitation (explained); **MISSING** = not built (explained).

| Region | Verdict | Mean diff | Notes |
|---|---|---|---|
| Quiet: stamp line, title, subtitle | MATCH | 1.4 | |
| Quiet: view switch | MATCH | | |
| Quiet: *Start focus* | CLOSE | | production's unstyled OS button reproduced (grey/outset, light variant in light theme) |
| Quiet: *Something else* | DIFFERENT | | drawn, dimmed and inert — picker modal deferred |
| Quiet: planned list / selection / hover | CLOSE | 3.3 | ±1 px vertical; hover tint + inset reproduced |
| Quiet: italic note | CLOSE | 7.0 | Slint has no `line-height`, lines sit ~5 px tighter |
| Quiet: Pace / Ahead / Focused today | MATCH | 1.6 | |
| Top bar + score stamp | CLOSE | 2.0 | frame tilt reproduced; rotated text renders heavier (femtovg), letter-spacing less visible |
| Tab row (sticky) | CLOSE | 3.2 | sticky behaviour matches; no horizontal scrolling when narrower than the tabs |
| Full: head (title, ledger, switch) | MATCH | 1.9 | |
| Full: selected-block sheet | CLOSE | 7.0 | |
| Full: queue list (chips, meta, checkbox, *Focus*) | CLOSE | 6.7 | |
| Full: course ledger | CLOSE | 5.1 | |
| Full: pace ledger | CLOSE | 3.8 | |
| Full: exam runway | CLOSE | 7.8 | |
| Full: margin note | CLOSE | 6.9 | box tilt reproduced; text not rotated (rotated Slint text was illegibly heavy), tighter line spacing |
| Full: weekly chart (columns, legend, discovered, stats, toggle, tooltip, popover) | CLOSE | ~8 | no glow/blur shadows, survey flag grey not amber-tinted |
| Full: footer note | CLOSE | | |
| Empty states (all) | MATCH | 1.2 (Quiet) / 3.7 (Full) | text identical to production |
| Light theme | CLOSE | 1.8 / 4.9 | |
| Tabs Planner/Vault/Break Room/Social, `?`, menu button | DIFFERENT | | drawn, inert (unavailable surfaces / deferred panels) |
| Menu panel, modals, tours, update toast | MISSING | | deferred (§2) |
| Modern / Wabi-Sabi Dashboards, Garden, Focus/Cockpit/Analyst/Custom layouts | MISSING | | non-default style, deferred |

No *major* region of the default Dashboard is MISSING; the MISSING rows are separate features (menu, modals) or a different, non-default style.

## 14. Typography

Production CSS names `Newsreader` (serif), `Courier Prime` (mono), `Hanken Grotesk`; its packaged CSP (`font-src 'self' data:`) blocks their Google-hosted files, so the shipped app actually renders **Georgia**, **Consolas** (Chromium's default monospace on Windows) and **Arial** (`Helvetica Neue`→Arial). Native uses the same system fonts by name; **no font is bundled or licensed**. Sizes, weights, letter-spacing and colours were read from computed styles. Remaining differences: Slint has no `line-height` or `text-transform` (line spacing approximated with explicit boxes, uppercase applied in Rust), glyph rasterization differs (DirectWrite ClearType vs femtovg).

## 15. Icons / assets

Project-owned SVGs under `assets/dashboard/` copied from production's inline SVG path data (six tab glyphs, sun/moon, five milestone glyphs, strata, survey flag) plus a 2×1 dot tile for production's `1px dotted` rules. Tinted with `colorize`; no Unicode/emoji substitutes. Production's `⌖`/`⚑`-style glyph fonts are not used. App icon: Stage 13's.

## 16. Responsive behaviour

Implemented breakpoints: ≤1180 (Full → one column), ≤780 (reduced padding, stacked head/view switch/next-task), column minimums. Production itself cannot be narrower than its `minWidth` 1180 (Tauri window config), so ≤780 rendering is reached only by restore-down edge cases there; native allows 460. Checked at 1920, 1520, 1180 (vs production, table above) and at 900/600 (functional: no overlap or clipping of text, vertical scrolling only; layout is *not* identical to production's stacked 600 px rendering: native keeps the topbar on one row and clips the tab row instead of scrolling it).

## 17. Empty state

Brand-new profile (0 sessions/courses/exams): every metric is 0, no NaN/∞, "Critical 0" stamp (PQ-2), empty-state strings identical to production (tested against captured text), no placeholders.

## 18. Synthetic datasets

`empty`, `small` (1 course, 1 session today — every number hand-verified: 45m, 38 %, streak 1, 15 units, 2.4/day, score 57, 18 days), `realistic` (5 active courses + 1 archived, 93 sessions over 70 days, streak 9 with a gap, a dangling and a course-less session, 3 upcoming + 1 past exam, 4 planned entries) and the Stage 16 stress dataset (~2 000 sessions, 24 courses, 360 tasks, 72 exams, 120 timetable events). Expected numbers come from production's own rendering of the same files (`production-text/*.txt`), not from the Rust code.

## 19. Performance (release build, this machine, 27 s windows)

| Scenario | Procs | Private WS | Private Bytes | CPU (1 core) | Frames in steady-state 10 s | Store written | Recomputes |
|---|---|---|---|---|---|---|---|
| D17-P0 empty, Dashboard | 1 | 43.6 MB | 122.7 | 0 % | 0 | no | 1 (startup) |
| D17-P1 realistic, Quiet | 1 | 45.2 | 126.2 | 0 % | 0 | no | 1 |
| D17-P1b realistic, Full | 1 | 47.9 | 129.6 | 0 % | 0 | no | 1 |
| D17-P2 stress, Full, 1 y chart | 1 | 50.4 | 133.3 | 0 % | 0 | no | 1 (2.6 ms) |
| D17-P3 timer running, Dashboard visible | 1 | 47.7 | 128.0 | 0.06 % | 0 | no | 1 |
| D17-P4 timer running, **minimized** (`IsIconic`) | 1 | 47.8 | 130.2 | 0.06 % | 0 | no | 1 |
| D17-P5 Demo completion → refresh | 1 | 51.6 | 135.4 | 0.11 % | – | yes (the session) | 2 |
| D17-P6 600 navigation cycles | 1 | 59.8 (see §20) | 140.5 | 20.8 % (intentional 25 flips/s) | – | no | 1 |

Dashboard compute: 85–200 µs (empty/realistic), 2.6 ms (stress, first load), 100–230 µs per chart rebuild. First frame (release, `STUDY_NATIVE_STARTUP_REPORT`): empty ≈161–171 ms, realistic Quiet ≈160–165, realistic Full ≈188, stress Full ≈176 — vs Stage 16's 160.6 ms baseline (Full adds ≈25 ms for the chart). Timer ticks verified separately: 98–100 ticks per 10 s with **0 frames** in steady state, visible Full layout and minimized alike. Viewing the Dashboard writes nothing (store mtime and log size unchanged in every window). One process in all cases; no browser/helper processes. Scripts: `scripts/stage17-perf.ps1`, `scripts/stage17-nav-check.ps1`.

## 20. Memory stability

`STUDY_NATIVE_NAV_STRESS=600`: 600 × (Dashboard ↔ Timer, Quiet ↔ Full, all six chart ranges) through the real property/callback paths. Private WS sampled every 15 s after the first 10 s: **60.6, 64.3, 64.0, 64.3, 64.4, 62.3 MB** — a plateau (allocator high-water from building the chart models), no growth; the last sample is lower. Models are replaced wholesale per refresh, not appended.

## 21. Dependencies

**Zero new crates** (no chart framework, WebView, database, async runtime or network stack). `chrono` (already present) provides the local-timezone clock; `serde_json` (already present) only in tests.

## 22. Persistence

The Dashboard is derived state; nothing Dashboard-specific is stored. Schema stays v1. Stage 15/16 stores load unchanged. Not persisted (session-only): theme, Quiet/Full, chart range, selected row — production persists the first two in `localStorage`; native needs a preferences store (Stage 18+/Options), listed in §26.

## 23. Tests

| Suite | Count |
|---|---|
| `study-tracker-core` (was 48) | **90** (+42: civil/clock, formatting incl. JS `toFixed`/`Math.round`, metrics, schedule expansion/health/workload, focus timeline, `toggle_calendar_entry`) |
| native bin (was 142 − removed Stage 10 tests) | **134** (+17 `dashboard_view`: golden production values for realistic/small/empty, Quiet/Full text, recompute policy, Timer-completion refresh, date rollover, stress dataset, dangling course, geometry, `fossilRand` bit-exactness, `chart_filter`, colour parsing) |
| Total workspace | **224 passed**, 1 ignored (pre-existing), 0 failed |

Cross-check against production: the golden assertions use values printed by the running production app (score 62, streak 9, course scores 55/71/78/86/93, 13h 35m, 83h 26m/46 days/Jul 23, …).

## 24. Remaining visual differences

Rotated text (score stamp) heavier than Chromium's; margin-note text unrotated and tighter; no glow/blur shadows on chart columns, health fills and dots; chart survey-flag tone; disabled look of *Something else*; scrollbar is a thin native overlay (production's WebView scrollbar); inert tabs; no horizontal tab-row scroll; ≤780 px layout not identical; hover micro-transitions (production animates 0.15–0.18 s) are instantaneous.

## 25. Risks / open issues

* The surrounding Timer surface is still the dark Stage 4–14 design; switching Dashboard ↔ Timer crosses two visual languages until the Timer/shell migrate.
* Daily goal and accent are production defaults (settings not migrated); a user with another goal would see a different goal %.
* Schedule-dependent numbers (units/day, scores) were verified against production for the fixtures (no timetable events in them) and unit-tested for timetable expansion; a real imported profile with recurring events should be spot-checked against production side by side before cutover.
* DST-transition correctness untested (§5).
* Real-user data was never imported (by rule), so real-data edge cases are untested.
* Font rasterization parity is bounded by the renderer.

## 26. Stage 18 requirements

Tray, notifications, updater and single-instance as planned, plus (carried from this stage): a preferences store (theme, layout, settings incl. daily goal), Timer↔task context linking for *Start focus*/*Focus*, the "Something else" picker, the menu panel, and restyling the Timer surface so the shell stops mixing two designs.

## 27. Stage 17 verdict

**PASS WITH CONCERNS.** The default production Dashboard (Field Notebook, Quiet + Full, dark + light) is rebuilt with production's metrics (verified against the running app), layout, fonts, colours and charts, backed by real `AcademicState`, updating on Timer completion without restart, with zero new dependencies and no idle/minimized CPU or viewing-induced writes. Concerns: non-default styles/layouts and several interactive extras (picker, menu, modals) are deferred; preferences are not persisted; the Timer surface is not yet restyled; narrow layouts and rotated text are approximations.

## Production integrity / real data

`git diff -- desktop` is empty. The production web build was made into a scratch directory; the headless browser used a throw-away profile; no production backup, localStorage, installed app or app-data directory was read or written; every fixture is synthetic; no screenshot contains personal data.
