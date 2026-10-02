# Stage 20: Break Room, Wabi-Sabi Rest, games and achievements

Status: portable implementation complete and committed as `836a49b` (already on `origin/main`; the Linux pass handed it to the Windows VM that way). Linux verification: PASS WITH CONCERNS (portable). **Windows VM verification (2026-10-02, §43): PASS WITH CONCERNS — WINDOWS FUNCTIONALLY VERIFIED**; the Windows fixes from that pass are committed separately on top of `836a49b` ("native: verify stage 20 on windows and fix parity issues"). Former NOT RUN items now point to §43; only physical-hardware qualification remains (Stage 24).
Stage 19 is committed separately as `e2e1a56`.
Production reference: Study Tracker **v0.1.67**, upstream `fe2f7a60704aa77b3d72a1c86912e2d2a60274b9`. `desktop/` is read-only and unchanged.

## 1. Scope

In scope and implemented:
- **Field Notebook Break Room**: the page, tokens/XP, game cards, unlock/play, pet rock, water, quote, stretch idea, badge strip.
- **Wabi-Sabi Rest**: the sidebar "Rest" nav and its rooms menu. The rooms are Games, Meditation (rest timer and 4-7-8 breathing) and Achievements, which is the 3-D open-book album with page turns.
- **The four local games**: Wordle, Geodle, Flaggle and Daily Durak, fully playable in both styles and all themes.
- **The unlock-token economy, XP and pet rock.** All 43 achievements are evaluated, including the 7 Garden achievements, which are derived from existing data.
- Persistence in a native `break_room` section; production backup import of every Break Room key.

At the stage boundary only:
- **Travle** is in the catalog and can be unlocked, played (logged) and counted. It opens a placeholder that says the map game arrives in Stage 21. No route game was invented (§17).
- **Daily Skribbl** behaves the same way. It opens the production modal shell with a "needs the online service (Stage 22)" state; no drawing, upload, gallery or votes (§18).

Not touched: the Garden UI, Modern dashboards (Stage 23), any network/social code (Stage 22) and Travle gameplay (Stage 21).

## 2. Production provenance

- `damcha02/destudydracker` `main` was re-fetched on 2026-10-01. The head is `fe2f7a6` (v0.1.67). All 400 `desktop/` files are identical to upstream, so no resync was needed.
- Semantics were read directly from the production source:
  - `App.tsx`: `studyBreakGames`, `unlockGame`, `logPlayedBreak`, the badge effects, `profileBadgeGroups`, the Wabi album, the rest room and the game handlers;
  - `lib/wordle.ts`, `wordleWords.ts`, `geodle.ts`, `flaggle.ts`, `countries.ts`, `durak*.ts`, `storage.ts`;
  - `index.css` and `App.css`.
- Production was observed running: a scratch `vite build` of `desktop/` was served to a throw-away headless Chrome profile (`scripts/visual-parity/capture-prod.mjs`, plus `--math-random` for deterministic Durak picks).
- Golden fixtures were computed by **production's own TypeScript**, copied read-only to a temp dir and run under Node 25 type stripping (`scripts/stage20-goldens.mjs`).

## 3. Exact game inventory (v0.1.67 `studyBreakGames`, production order)

| # | Name (= persisted id) | Description | Native Stage 20 |
|---|---|---|---|
| 1 | Daily Durak | Solve today's Durak endgame puzzle | **Playable** (full port: rules, CPU, solver, daily puzzle search) |
| 2 | Wordle | Guess the 5-letter word in 6 tries | **Playable** (hard mode, keyboard, 2 300 answers / 16 172 guesses) |
| 3 | Travle | Build a border route between countries | Catalog, unlock and play logging; **placeholder screen, gameplay Stage 21** |
| 4 | Flaggle | Guess the flag from shared colors | **Playable** (pixel-mask reveal, similarity, 195 flags) |
| 5 | Daily Skribbl | Draw today's theme, then vote on the gallery | Catalog, unlock and play logging; **network shell, gameplay Stage 22** |
| 6 | Geodle | Guess the country from geography clues | **Playable** (six clues, tooltips, 195 countries) |

`STUDY_BREAK_GAME_COUNT` is 6, so Full House, Explorer and Perfectionist require all six, including Travle and Skribbl, exactly as in production. Those two can still be unlocked and played (logged) natively, so the badges stay reachable.

## 4. Field Notebook Break Room inventory

The layout was measured in production at 1520×980:
- head: "BREAK ROOM", date, rule;
- token pill "n/6 unlocked today" with an XP bar "x/45 min to next";
- streak pill (`streakEmoji`);
- six game cards with title, description, state and Unlock/Play. The unlock glow is static. Puzzle cards show `n/3`-style progress;
- pet rock: stage emoji and name, pat counter, a celebration at 1 000 pats;
- water glasses;
- daily quote (100 production quotes, daily pick);
- stretch idea (10);
- the badge strip of the Break Room achievements.

## 5. Wabi-Sabi Rest inventory

- **Sidebar**: "Rest 休み" is enabled. A rooms submenu holds Games, Meditation and Achievements; Wabi Today/Timer leave the room.
- **Games**: six Wabi game cards (paper, sage/vermilion), the token ring and the same unlock/play actions.
- **Meditation**:
  - rest timer: ring and editable `MM:SS` face (`parseTimerFaceInput`), default 5 min, START/PAUSE/RESET;
  - 4-7-8 breathing: Breathe in 4 s / Hold 7 s / Breathe out 8 s, four rounds, stopping after 75 s. The ring size animates over 1 s per phase;
  - guide column.
- **Achievements**: production's open-book album. Its parts:
  - desk scene with cup and pen, cloth cover with a paper slip and seal;
  - a click on the cover opens the book (780 ms cover hinge);
  - two-page spreads of achievement rows with art, Japanese name, how-to and earned date;
  - page turns by click zones and ←/→ (520 ms leaf hinge), Escape closes;
  - 14 pages for the full set, with a blank right page for odd counts.

## 6. Unlock-token economy

`studyBreakTokens = min(6, floor(todayMinutes / 45))`. `todayMinutes` is `getTodayMinutes`: all session kinds that ended today, break sessions included, as in production.
- Unlocks are per local day (`unlockedGames` + `unlockedGamesDate`). A new day resets the list but not the counters.
- `unlockGame` is guarded the way production's UI is: the Unlock button exists only while `canUnlockMore` and the game is locked, so a repeated click never unlocks twice.
- `totalUnlocks` +1 per unlock.
- `unlockStreak` reproduces production's quirk exactly. It compares *local* midnight with `new Date(lastUnlockDate)`, which JS parses as UTC midnight, so outside UTC the streak restarts at 1 on every unlock (golden-tested).
- `speedrunnerToday`: the first unlock of the day after a single ≥ 45-minute study or exam session.

## 7. XP system

XP is not a stored currency in v0.1.67. `xpProgress = todayMinutes % 45`, `xpPercent = xpProgress / 45 × 100`, and `minsUntilNext = max(1, 45 − xpProgress)` while fewer than 6 tokens exist (otherwise 0). It is derived on every evaluation and **never persisted**, so it can never be awarded twice.

## 8. Pet rock

- `petRockPats` +1 per pat (persisted).
- The stage and name come from the 19 `petRockMilestones` (10, 50, 100, 250, 500, 666, 888, 1k, 5k, 10k, 50k, 100k, 500k, 666 666, 888 888, 1M, 6 666 666, …).
- The 1 000th pat starts the celebration, once.
- `rock-current` (the rock's current stage) is a Break Room achievement on the profile and is not shown in the album, as in production.

## 9. Achievement inventory (43)

| Group | Count | Ids |
|---|---|---|
| Break Room | 10 | full-house, first-break, on-fire, early-bird, night-owl, speedrunner, explorer, perfectionist, veteran, rock-current |
| Pet Rock | 19 | rock-sprouting … rock-guardian-angel |
| Focus Fossil | 7 | fossil-10/25/50/100/250/500/1000 (lifetime hours) |
| Garden of Knowledge | 7 | garden-first-sprout, -streak-bloom, -mushroom-ring, -cross-pollinator, -full-bloom, -harvest-season, -wise-tree |

How production stores them is reproduced exactly:
- Nothing stores "unlocked". Every achievement is a pure function of state, recomputed on change, so double awards are impossible by construction.
- The five *daily* badges (full-house, early-bird, night-owl, speedrunner, perfectionist) count once per local day (`badgeCounts` + `badgeCountDates`). Their "earned" is `earned today || count > 0`.
- `achievementEarnedOnDates`: the local day an achievement was *observed* turning earned while the app ran. Nothing is back-dated (`EarnedDateTracker`).
- The album shows 42 entries: everything except `rock-current`. Daily badges appear as copies per count. Art comes from production's 42 PNGs (`has_real_art`); the others use a wall icon and a Japanese name.

## 10. Garden-achievement handling

The seven `garden-*` achievements are functions of sessions, tasks and the rolling 7-day minutes: the same inputs the Stage 23 Garden derives from. They are evaluated in `achievements::evaluate` from `GardenInputs` (session count, streak days, weekly course count, completed tasks, `garden_stage` thresholds 0/30/90/210/420/720 min). No Garden state, UI or variant is built.

`getWeeklyCourseCount`'s UTC quirk is reproduced in `academic_inputs`.

## 11. Native domain architecture

- `crates/study-tracker-core/src/break_room/` is pure: no I/O, no Slint, no clock reads. Every function takes `today` and a `LocalClock`.

  | Module | Contents |
  |---|---|
  | `catalog` | games and availability |
  | `daily` | FNV-1a over UTF-16, base36 puzzle ids, daily index |
  | `countries` | 195 production countries |
  | `wordle` | |
  | `geodle` | |
  | `flaggle` | pixel similarity and reveal |
  | `durak` | 1 335 lines: rules, CPU, solver and daily puzzle |
  | `state` | `BreakRoomState`, `Progression`, unlock/play/water/rock/daily badges |
  | `achievements` | evaluation, album entries/pages, earned-date tracker |
  | `rest` | breathing, rest timer, face parser |

- App layer:
  - `break_room_controller.rs`: owns the state, derives on academic change or a new day (cached by revision + day), persists **only on change**, and holds the album's open/turn/close state machine.
  - `break_room_view.rs`: domain → Slint structs.
  - `app_break_room.rs` / `app_break_games.rs`: callbacks and pushes. Persistent `VecModel`s let repeaters keep their items and running animations.
- UI:
  - `ui/break/{types,fn-break,games}.slint`;
  - `ui/wabi/{rest,album}.slint`;
  - wiring in `ui/main.slint`, `ui/wabi/sidebar.slint`/`page.slint` and `ui/fn/shell.slint`/`page.slint`.

## 12. Persistence

- The native store gains a `break_room` section that uses production's field names:
  - `unlockedGames`, `unlockedGamesDate`, `playedBreaks`, `playedBreaksDate`, `playedGamesAllTime`;
  - `totalUnlocks`, `unlockStreak`, `lastUnlockDate`, `speedrunnerToday`;
  - `badgeCounts`, `badgeCountDates`, `achievementEarnedOnDates`;
  - `petRockPats`, water;
  - `wordlePuzzle`, `geodlePuzzle`, `flagglePuzzle`, `durakPuzzle`, `travlePuzzle`, `restTree`, `achievementBoard` (opaque).
- Parsing is tolerant, like production's loaders (`js_truthy`, defaults on garbage).
- `flagglePuzzle.maskedFlagDataUrl` is written as `""`: native recomputes the mask, and production regenerates it.
- Unknown keys and every other section are preserved (`FileBreakRoomPort`).
- Stores without the section (Stage ≤ 19) load production defaults. Tested.

## 13. Production import

- `migration.rs` classifies the 21 Break Room keys as **Consumed**.
- `commit_import` writes the `break_room` section and verifies it by read-back. A failure rolls back the whole destination (tested).
- Replace semantics match `restoreBackup`. An old backup without Break Room keys resets the section, as production does.
- Importing twice is idempotent.
- `ImportReport.break_room_imported` is logged.

## 14. Game architecture

- Each game is a core state machine plus `BreakRoomController` actions (`wordle_*`, `geodle_*`, `flaggle_*`, `durak_action`) and one `GamesData` push of the open game only.
- **No game timers and no frame loop.** Every game is turn-based and event-driven.
- The only animations are short, bounded Slint `animate` transitions: card lift, tile state change, modal fade, and the album hinge/slide. An open, idle game renders **0 frames** (§25).
- Durak's CPU replies are computed synchronously in the action, as in production.
- Flaggle replaces production's `<canvas>` pixel comparison with `resvg` rasterization (240×180, demultiplied) of the same SVG flags. The mask is computed in Rust and handed to Slint as an image (brief §29).

## 15. Randomness/date architecture

- Daily puzzles: `puzzle_id = date + ":" + base36(fnv1a_utf16(salt))`, and the daily pick is the n-th smallest `(hash, index)`.
- V8 date rollover is reproduced ("2026-02-30" → 2 Mar).
- The per-profile seed salts are drawn once (pinned splitmix `Rng`) and persisted.
- Durak: `mulberry32` (shared with Sakura) plus production's Java-style abs date hash, and `find_daily_puzzle(seed, pick)`. `STUDY_NATIVE_BREAK_PICK` pins `Math.random` for parity runs.
- All of it is golden-tested against production's TS output (`tests/fixtures/break_room/*.json`).

## 16. Each game implementation

- **Wordle**: `score_guess` handles duplicate letters like production; keyboard state, hard-mode violations in production's order and the toggle (only before the first guess).
  - Messages: "Not in word list", "Solved in N.", "Answer: X."
  - Typed input comes from the physical keyboard and the on-screen keys. Golden-tested over many date/salt pairs.
- **Geodle**: six clues (continent, population, landlocked, religion, area, government). Numeric clues are match, close (±20 %) or higher/lower.
  - `Intl` compact formatting is reproduced exactly (half-expand rounding, unit promotion).
  - Hint tooltips, a searchable country combo (type, filter, pick, Enter), and production's "Select a country from the list." / "You already guessed that country."
- **Flaggle**: the guess flag's pixels that match the answer flag are revealed on the mask; similarity %, guess rows with thumbnails, the dropdown with flag thumbnails, 6 guesses.
- **Daily Durak**: the full endgame solver and CPU. Controls: attack, throw, pass, defend, slide (perevodnoy), pick up, retry. The hand lift-select, the table and the log follow production; the daily puzzle survives restarts (tested).
- **Skribbl/Travle**: §17–18.

## 17. Travle Stage 21 boundary

Travle is in the catalog, can be unlocked and played (logged), and counts for Explorer, Perfectionist and Full House. The stored `travlePuzzle` is carried verbatim (`TravlePuzzle`) and re-written unchanged. Opening it shows a placeholder modal. It has **no** border graph, route logic, map rendering or guesses, which are all Stage 21.

## 18. Network-game Stage 22 boundary

Daily Skribbl can be unlocked and played (logged). Opening it shows production's modal shell with a note that drawing, upload, gallery and votes need the online service (Stage 22). It makes **no** network calls, adds no canvas and adds no social code. The `social` section stays withheld.

## 19. Meditation/breathing

- The rest timer and breathing exercise follow **elapsed monotonic time** (`Instant` anchors), not tick counts.
- A 250 ms seconds clock runs **only while** a rest timer or breathing exercise is running (`needs_seconds_clock`). It stops when they finish or when the user leaves the room.
- The breathing ring's size animates once per phase (production's `transition: width 1s linear`), with no per-frame loop.
- Reduced motion follows production: the album skips its cover/leaf animation (production checks `prefers-reduced-motion` in JS), and the rest-room liquid/splash are already static natively. Production does not disable the breathing transition, so native keeps it.

## 20. Theme integration

- FN light/dark/Sakura and Wabi light/dark/Sakura are all supported.
- Game modals read the seven production CSS variables (`Game` global: surface, surface-2, inset, border, accent, text, muted). Their derived colours are computed with **`color-mix(in oklch)` semantics** (`game_tokens.rs`), including Chromium's powerless-hue rule (chroma < 0.02). That rule is what produces production's teal-tinted Geodle and blue-tinted Flaggle backgrounds in FN dark. Unit-tested against production's computed values.
- FN uses square buttons and keys; Wabi keeps production's 9/10/16 px radii.
- The Wabi album decor has light and dark captures.

## 21. Visual parity methodology

- Production: headless Chrome (DPR 1, 1520×980, frozen date, packaged-CSP font blocking reproduced, `--math-random`) on a synthetic fixture (`gen-break-fixture.mjs`: early/unlocked/full/empty). The fixture's daily puzzles come from production's own answer functions.
- Native: `scripts/visual-parity/capture-native-linux.sh`. It runs the same fixture in an isolated profile on a private Xwayland display at scale 1 and saves femtovg pixels from inside the rendering notifier (`STUDY_NATIVE_SNAPSHOT`). `STUDY_NATIVE_INPUT` scripts clicks/keys/text for game states.
- Fonts: on Linux both apps resolve through fontconfig (Liberation Serif/Sans, the user's monospace), and emoji use Noto Color Emoji. On Windows, Georgia/Arial/Consolas and Segoe UI Emoji apply. **Windows captures: PASS — WINDOWS VM FUNCTIONAL (pixel metric VM-LIMITED), §43.10.**
- Metric: mean absolute per-channel difference (0–255) over the full window, plus side-by-side/diff review.

## 22. Visual parity results (Linux, mean abs diff)

| Surface | Diff |
|---|---|
| FN Break Room, full page (unlocked fixture, dark) | **1.02** |
| Wabi Rest: Games light / dark | **0.77 / 0.95** |
| Wabi Rest: Meditation | **0.73** |
| Wabi album: closed | **2.44** |
| Wabi album: open spread | **4.86** |
| FN modal: Durak | **1.22** |
| FN modal: Wordle | **1.38** |
| FN modal: Geodle | **1.46** |
| FN modal: Flaggle | ~1.5 after the panel-height fix (5.3 before) |
| FN modal: Travle / Skribbl | 12.7 / 8.6: deferred placeholders vs production's real game states (by design) |
| Wabi modal: Wordle | 10.5 before the radius fix; the remainder is heading/label wrap |

The open album's residual is text weight. femtovg renders glyphs inside the −0.8° rotated stage slightly heavier than Chrome does. The geometry, decor and page content line up.

## 23. Region/game parity table

| Region | Status |
|---|---|
| FN header, token pill, XP bar, streak | MATCH |
| FN game cards (locked/unlocked/played/progress) | MATCH |
| FN pet rock, water, quote, stretch | MATCH (quote line-height compensated) |
| FN badge strip | MATCH |
| Wabi sidebar Rest + rooms menu | MATCH |
| Wabi game cards | MATCH |
| Wabi meditation: timer ring | CLOSE (reproduces production's empty-vessel compositing tint) |
| Wabi breathing ring | CLOSE (1 s linear size transition, phase text exact) |
| Album: desk, cup, pen, cover, slip, seal | CLOSE (production-derived raster decor; slip glyph pitch tuned) |
| Album: open spread, rows, art, dates | CLOSE (rotated-text weight) |
| Album: cover/leaf 3-D hinge | CLOSE (orthographic cos-scale hinge instead of CSS perspective) |
| Wordle modal: board, keyboard, hard pill | MATCH (FN) / CLOSE (Wabi wraps) |
| Geodle modal: input, combo, clue table, tooltips | CLOSE |
| Flaggle modal: mask, rows, dropdown thumbnails | CLOSE |
| Durak modal: table, hand, controls, log | CLOSE |
| Travle | DEFERRED (Stage 21) |
| Daily Skribbl | DEFERRED (Stage 22) |
| Windows rendering of everything above | PASS — WINDOWS VM FUNCTIONAL after the §43.3 fixes; see §43.10 |

## 24. Behavior parity matrix

| Behavior | Evidence |
|---|---|
| Daily ids/picks/answers per date and salt | goldens vs production TS (daily, wordle, geodle, durak) |
| Wordle scoring, hard mode, messages, completion once | core tests + controller test + typed end-to-end solve ("Solved in 4.") |
| Geodle clues, compact numbers, hints, submit/Enter | goldens + controller test + end-to-end Enter submit |
| Flaggle similarity/reveal, duplicate/unknown guesses | flag tests (incl. non-rectangular Nepal) + controller test + dropdown e2e |
| Durak rules, CPU, slide, retry, daily puzzle | goldens + session tests + restart test + attack→slide→defend e2e |
| Tokens from timer sessions exactly once | `timer_sessions_feed_tokens_exactly_once` |
| Unlock/play/daily badges exactly once across restarts | `unlock_play_and_daily_badges_are_exactly_once_across_restarts` |
| Viewing never writes; each action writes once | `every_persisted_action_writes_once_and_viewing_never_writes` |
| 1 000th pat celebration once | `the_thousandth_pat_celebrates` |
| Album open/turn/close timing and page math | `the_album_opens_turns_and_closes_like_production` + album golden + e2e |
| Rest timer/breathing follow elapsed time | `rest_timer_and_breathing_follow_elapsed_time` |
| Travle/Skribbl are entries without a game | `travle_and_skribbl_are_unlockable_playable_entries_without_a_game` |
| Import field-by-field, old-backup reset, idempotent, rollback | port + migration tests |

## 25. Performance (B20)

Linux numbers come from a release build on a private Xwayland display (1520×980), with the `STUDY_NATIVE_FRAME_STATS` frame count and CPU from `/proc` (`scripts/stage20-perf-linux.sh`). Windows: frame behaviour PASS — WINDOWS VM FUNCTIONAL; CPU/fps PASS — VM DIAGNOSTIC (§43.5); authoritative CPU/GPU STILL PENDING PHYSICAL WINDOWS.

| Scenario (brief §33) | Linux result |
|---|---|
| B20-P0 fresh Break Room idle (FN and Wabi Games) | 0 frames, 0.00 % CPU |
| B20-P1 realistic unlocked state idle (`full` fixture) | 0 frames, 0.00 % CPU |
| B20-P2 Achievements: album closed / open spread, idle | 0 frames, 0.00 % CPU |
| B20-P3 pet rock (on the Break Room / Rest surfaces), idle | 0 frames, 0.00 % CPU (a pat is one push and one write) |
| B20-P4… Wordle / Geodle / Flaggle / Durak open, idle | 0 frames, 0.00 % CPU each (the Travle/Skribbl placeholders also 0) |
| B20-PA… per-game animation (tile/card/modal transitions, album hinge, breathing ring) | animations are bounded (≤ 1 s) and stop. Windows: lifecycle PASS — WINDOWS VM FUNCTIONAL (all stop; §43.5); frame rate/CPU PASS — VM DIAGNOSTIC only, STILL PENDING PHYSICAL WINDOWS. The hidden Xwayland window is throttled to ~1 fps, so Linux frame/CPU numbers during animation are invalid |
| B20-PM Meditation visible, idle | 0 frames, 0.00 % CPU. Running timer/breathing: 1 model update per second, drawn on change only |
| B20-PT Timer running while the Break Room is visible | 0 frames, 0.00 % CPU (the Break Room shows no Timer, and Timer ticks cause no Break Room work) |
| B20-PMIN minimized during game/animation | Windows: PASS — WINDOWS VM FUNCTIONAL, 0 frames, 0 Sakura ticks (§43.6) |
| B20-PTRAY tray-hidden during game/animation | Windows: PASS — WINDOWS VM FUNCTIONAL, 0 frames, 0 Sakura ticks (§43.6) |
| B20-PNAV navigation stress | see §27 |
| B20-PGAME game reset stress | see §27 |

RSS idle is 91–100 MiB for the Break Room and games. The open album is 120–125 MiB: its DPR-2 desk/board/page decor adds about 28 MB anon while it is shown, and that memory is released on leaving (§27).

## 26. Background rendering

- No surface added in Stage 20 has a free-running timer.
- The only Stage 20 timer is the 250 ms seconds clock, and only while the rest timer or breathing is running.
- Album motion ends with a single-shot finish timer (duration + 30 ms).
- Effect timers are bounded and dropped when finished.
- When minimized or hidden, Slint does not render, and the seconds clock only updates model text. **Windows: PASS — WINDOWS VM FUNCTIONAL (§43.6).**

## 27. Memory stability

`scripts/stage20-memory-linux.sh` samples VmRSS/RssAnon while a diagnostic stress hook runs.

- **B20-PNAV** (`STUDY_NATIVE_BREAK_STRESS`): every 40 ms, one step through every Break Room surface. That covers the FN page, each game screen (Flaggle with its 80-flag dropdown), Wabi Games/Meditation/Achievements and album open/turn/close (14 steps per cycle).
  - 120 cycles: anon **51.3 → 51.3 MiB**, RSS 128.6 → 128.6 MiB (transient album peak 158.5/81.1 MiB, released), threads 10 throughout.
  - 500 cycles (7 000 steps, 2 501 pushes): anon **51.3 → 51.6 → 51.6 → 51.6 MiB** (minutes 1, 3, 5, 6), RSS 133.4 → 129.4 MiB, one transient album peak (163.3/81.2 MiB) released, threads 10 throughout. `break_writes=1`, `break_evaluations=1` for the whole run, and only 6 of the 42 art PNGs decoded (lazy). **Plateau reached.**
- **B20-PGAME** (`STUDY_NATIVE_GAME_RESET_STRESS=200`): 200 Durak daily-puzzle searches (solver included, varied seeds) dealt one by one into the open Durak screen, the heaviest game reset. Result: RSS 94.0 → 94.0 MiB, anon 26.6 → 26.6 MiB, threads 10, 0 extra writes. The other games' resets are covered by the navigation stress, which re-opens every game screen 500 times.
- Windows Private WS/Bytes plateau: **PASS — VM DIAGNOSTIC (plateau; §43.7).**

## 28. Startup

Nothing game-specific is built at startup:
- word lists, countries, flags and art all decode lazily on first use (`LazyLock`/thread-local caches);
- the album decor decodes only when the Achievements room is shown;
- the Break Room controller loads its section and derives once.

Fresh profile, `STUDY_NATIVE_STARTUP_REPORT` first frame, 7 launches each, median, on Linux (Xwayland, release):

| Profile | Stage 19 (`e2e1a56`) | Stage 20 |
|---|---|---|
| fresh, default style | 75.2–77.8 ms | 89.4–92.4 ms |
| stored synthetic profile (imported fixture) | 78.5–79.0 ms | 91.1–92.3 ms |
| stored, opening directly on the Break Room | n/a | 85.8–86.3 ms |
| fresh, Stage 20 with `fc-match` unavailable | n/a | **76.6 ms** |

The whole Linux delta (about +13 ms) is the Linux-only font resolution: Stage 20 asks fontconfig for production's generic families and then loads the real Liberation faces. Without it, Stage 20 starts like Stage 19, so the Break Room itself adds no measurable startup. The three lookups run concurrently. Starting them before window creation was tried and gained nothing, so it was dropped. This path does not exist on Windows, where the faces are named directly.

**Windows startup: PASS — VM DIAGNOSTIC (no regression vs Stage 19 in the same VM; §43.8). Physical comparison with 161 ms: STILL PENDING PHYSICAL WINDOWS.**

## 29. Persistence churn

- The store is written **only** when a derived or action result differs from the stored record (`persist only on change`).
- Viewing, navigating, opening games, idling, animating and running the rest timer write nothing. In every stress run, `break_writes` stayed at the single initial write (the profile's first puzzle-salt draw/dating) through the entire run.
- Test: `every_persisted_action_writes_once_and_viewing_never_writes`.

## 30. Timer/recovery integration

- Timer completion → StudySession (Stage 14/16 path, unchanged) → `refresh_dashboard` → `app_break_room::after_academic_change` → one controller sync (cached by academic revision + day) → achievement evaluation → push.
- No polling and no per-tick scan: a Timer tick that creates no session does not change the revision, so the Break Room does nothing.
- Tokens are derived from sessions, so a recovered or abandoned session counts exactly as it is stored (test `timer_sessions_feed_tokens_exactly_once`, `break_and_zero_minute_sessions_count_like_production`).
- Notifications are unchanged (Stage 18 path); Stage 20 adds none.
- **Windows Timer → session → notification exactly-once: PASS — WINDOWS VM FUNCTIONAL (§43.9).**

## 31. Stage 19 regression

- Wabi-Sabi Today/Timer/Quiet and FN surfaces are unchanged apart from the Rest nav/menu and the Break tab being enabled.
- Appearance switching pushes the Game tokens and re-pushes the Break Room.
- The Stage 19 tests all pass (see §36). Sakura is still bounded (22 live petals in every stress STATS line, `sakura_running=false` when not Sakura).
- Palette font properties became `in-out` so the platform font resolution can set them.
- **Windows Sakura hide/minimize stop: PASS — WINDOWS VM FUNCTIONAL (§43.6, §43.9).**

## 32. Stage 18 regression

- Platform code (tray, single instance, updater, notification) is untouched by Stage 20, and its tests pass.
- **Windows lifecycle (single instance, tray cycles, second launch, Quit): PASS — WINDOWS VM FUNCTIONAL after fix W20-1 (§43.3, §43.9).**

## 33. Accessibility/input

- Game cards, buttons, keys, tiles, combos and album zones have accessible roles and labels. Wordle tiles expose letter and state.
- The keyboard works everywhere:
  - Wordle: physical keys, Enter, Backspace;
  - Geodle/Flaggle: type, Enter, the dropdown;
  - Durak: buttons;
  - album: ←/→/Escape (FocusScope).
- Decorative images use `accessible-role: none`.
- Reduced motion skips the album's hinge and slide, as production does.
- **Windows accessibility: PARTIAL (UIA tree inspected, Narrator speech not run); IME: NOT RUN — IME UNAVAILABLE (§43.12).**

## 34. Assets/data/licensing

| Asset | Size | Source | Licence / status |
|---|---|---|---|
| `assets/break/flags/*.svgz` (195) | 431 KB | production's flag SVGs, which are lipis/flag-icons | MIT |
| `data/break_room/wordle-answers.txt` (2 300), `wordle-guesses.txt` (16 172) | | production `wordleWords.ts` | production notes "local system dictionary plus dwyl/english-words" (Unlicense); **exact provenance to confirm, Stage 24** |
| `data/break_room/countries.tsv` (195) | | production `countries.ts` | derived from mledoze/countries (ODbL) + samayo/country-json (MIT); attribution to carry in Stage 24 |
| `quotes.tsv` (100), `stretches.txt` (10) | | production `App.tsx` | production-owned text; quote attribution to confirm (Stage 24) |
| `assets/break/achievements/*.png` (42) | 2.1 MB | production, byte-identical | production-owned art; provenance to confirm (Stage 24) |
| `assets/break/japanese-*.png` (4 trees) | 0.4 MB | production | production-owned |
| `assets/break/album/{light,dark}/*` | 1.4 MB | rendered from production's CSS album scene (`stage20-album-assets.mjs`, CDP element isolation) | derived from production CSS; no third-party art |

Embedded binary growth is about 4.4 MB of assets (see §35).

## 35. Dependencies

- No new crates enter the dependency tree.
  - `png 0.18` and `resvg 0.47` (`default-features = false`) became direct dependencies; Slint already links both.
  - `flate2` (SVGZ) was already in the tree.
  - `serde_json` is a core **dev**-dependency for goldens.
- There is no game engine, browser, WebView, JS runtime, async runtime or database.
- Binary (Linux release, loaded sections): Stage 19 had text 17.05 MB / rodata 5.39 MB (file 40.99 MB); Stage 20 has text 20.60 MB / rodata 9.82 MB (file 55.71 MB). The +9.3 MB breaks down as Slint-generated UI code +3.5 MB, embedded assets +4.4 MB and unwind +0.8 MB.

## 36. Tests

| Suite | Count |
|---|---|
| `study-tracker-core` (was 122) | **158** (+36): daily hash/ids/V8 dates, Wordle/Geodle/Durak goldens vs production TS, compact formatting, Flaggle reveal, Durak rules/CPU/solver/session, economy/streak quirk, daily badges, all 43 achievements, garden inputs, album pages golden, rest/breathing/face parser |
| native app (was 212) | **229** (+17), 1 ignored (pre-existing): Break Room port (5), import (2), controller (12, incl. exactly-once/no-churn/restart/album), OKLCH tokens, flags, art |
| Total | **387 passed**, 1 ignored, 0 failed |

- `cargo fmt --check`, `cargo check --workspace`, `cargo test --workspace`, `cargo test -p study-tracker-core` and `cargo build --release` pass.
- On Linux, the build shows the same **56 pre-existing** dead-code warnings as the Stage 19 tree built on Linux (Windows-only platform code: tray, updater, notification). Stage 20 adds **none**.

## 37. Production integrity

`git diff -- desktop` is empty. Production sources were only read or copied read-only to temp directories for golden and fixture generation and scratch builds.

## 38. Real-data status

No real production user data was read, imported, modified or used:
- every run used `STUDY_NATIVE_DATA_DIR` temp profiles and synthetic fixtures (`gen-fixture.mjs` + `gen-break-fixture.mjs`);
- production captures used throw-away Chrome profiles;
- no real AppData/`~/.config` profile was touched.

## 39. Known differences

- The album hinge is orthographic (cos-scaled) rather than CSS `perspective`. The leaf has no perspective foreshortening, and the shading is approximated.
- Text inside the −0.8° rotated album stage renders heavier in femtovg than in Chrome.
- Wabi modal headings/labels wrap slightly differently at some widths.
- Flaggle masks are rasterized by resvg, not by Chrome's canvas. Edge anti-aliasing can differ by a pixel, but the similarity percentages match on tested pairs.
- Emoji come from the platform colour font. Production on Windows also uses Segoe UI Emoji.
- The album decor is a raster capture (DPR 2 desk) and is not resolution-independent beyond 2×.

## 40. Deferred work

- Travle gameplay (Stage 21).
- Daily Skribbl and all social/network features (Stage 22).
- Garden UI and Modern dashboards (Stage 23).
- Licensing/provenance confirmation for the word lists, quotes and art (Stage 24).
- The Windows verification pass: every item marked NOT RUN above.

## 41. Risks

- Windows font metrics (Georgia/Arial/Consolas) may shift wraps in the album and modals.
- Windows animation frame pacing is unmeasured.
- The album decor costs about 28 MB while open. If Windows Private Bytes grows too much, the decor can drop to DPR 1.
- The binary grew 9.3 MB. That is acceptable but worth watching.
- The `unlockStreak` UTC quirk is reproduced faithfully. If production fixes it, the native port must follow.

## 42. Stage 20 verdict

The portable implementation is complete:
- the domain, the four local games, economy, achievements and album are all ported;
- import and persistence work;
- Linux parity is close;
- idle rendering is 0 frames;
- memory plateaus;
- there is no churn;
- the automated checks pass.

**Windows VM verification: PASS WITH CONCERNS — WINDOWS FUNCTIONALLY VERIFIED (§43.14).** Physical-Windows performance/release qualification remains a Stage 24 gate.

## 43. Windows VM Verification

Run 2026-10-02 on a fresh clone. **Unexpectedly, portable Stage 20 had already been committed as `836a49b` (on top of `e2e1a56`) and pushed to `origin/main` before this pass**; the brief expected it uncommitted. Its content matched the documented portable state, so it was verified as-is (user decision); `836a49b` was not amended and history was not rewritten. Every result is labelled **A** = functional Windows result (closes the gate in a VM), **B** = VM diagnostic performance (useful for regressions/leaks, not authoritative), **C** = physical Windows required. The Linux results above are kept unchanged.

### 43.1 Environment

| | |
|---|---|
| Machine | **Virtual machine**: QEMU Standard PC (Q35 + ICH9), EDK II firmware; host CPU i7-13700H |
| Windows | 11 Pro 25H2, build 26200.8037, timezone Pacific (fixtures generated with `--tz -07:00`, `STUDY_NATIVE_NOW=2026-09-30T12:00:00-07:00`) |
| vCPU / RAM | 8 / 8 GB |
| Display | 1280×800, 100 % scaling (96 DPI), "Red Hat VirtIO GPU DOD controller", refresh reported as 1 Hz (virtual) |
| GL | The VirtIO adapter offers only Windows' GDI OpenGL 1.1, so FemtoVG cannot start ("Could not locate glCreateShader"). With user approval, **Mesa 26.2.3 llvmpipe** (`pal1000/mesa-dist-win`, MIT) `opengl32.dll` + `libgallium_wgl.dll` were placed next to the exe in `target/release` only, run with `GALLIUM_DRIVER=llvmpipe` (Mesa's default d3d12-over-WARP path exits with `0x80070057` here, a VM/Mesa artifact). Same FemtoVG/GL code path, **software rasterized**: every CPU/fps number below is B. |
| Toolchain | rustc 1.99.0 / cargo 1.99.0 `stable-x86_64-pc-windows-msvc`, VS 2022 Build Tools (installed for this pass), Node 24.19 (fixtures, production scratch build) |
| Production reference | scratch `vite build` of a copy of `desktop/` + `design/`, rendered in headless Edge 145 (Chromium = WebView2's engine), throw-away profile |

### 43.2 Build and tests (A)

Before fixes: `cargo fmt --check` clean, `cargo check --workspace` 0 warnings, `cargo test --workspace` **158 core + 236 app = 394 passed, 0 failed, 1 ignored**, `cargo build --release` 0 warnings. The app count is 229 + 7 Windows-only tests (3 `single_instance`, 4 `updater::http` incl. the WinHTTP loopback client) that do not compile on Linux. The 56 Linux dead-code warnings do not exist on Windows (that code is used here). The test build has one pre-existing warning (`map/tests.rs` `region` unused).

After fixes: see §43.15.

### 43.3 Bugs found and fixed

| Id | Class | Cause | Fix | Regression test / verification |
|---|---|---|---|---|
| **W20-1** | Windows, pre-existing since Stage 18 (`031f943`) | `Drop for PlatformHost` held `TRAY.borrow_mut()` across `DestroyWindow`; Windows delivers `WM_DESTROY` synchronously to `platform_window_proc`, whose `TRAY.borrow()` panicked inside an `extern "system"` callback, which aborts. **Every normal exit** (close with no session, tray Quit, snapshot quit) ended with exit code `0xC0000409` and a logged panic. Stage 18's Quit test passed only because the icon is deleted before the abort. | Take the state out and drop the borrow before calling Win32; `platform_window_proc` uses `try_borrow`; `app_platform::shutdown` drops the runtime outside its borrow. | `win_host::tests::dropping_the_host_destroys_its_window_without_reentrancy_panic` and `…a_message_delivered_while_the_tray_state_is_borrowed_does_not_panic` (the first aborts with `0xc0000409` on the old code). Real exits: 3/3 exit 0, "event loop exited normally", 0 panics; tray Quit the same (§43.9). |
| **W20-2** | Windows layout | Break Room stat chips/streak pill: the `HorizontalLayout` inherited the pill width and shared the surplus by stretch, which with Segoe UI Emoji metrics pushed the label ~12 px right and clipped "1-day streak". | Rows pinned to `preferred-width`. | Windows parity capture (no clipping; region diffs 13.8 → 11.5 and 7.4). |
| **W20-3** | Windows glyphs | `✕` (close), `⌫` (Wordle key), `▾` (combo), `↻` (stretch) were drawn in `FN.sans` = Arial, which lacks them; Slint's fallback found nothing, so they were tofu. `▶` fell back to the colour emoji face. Chromium uses Segoe UI Symbol. | `Emoji.symbol` global (Segoe UI Symbol on Windows; empty, meaning the text family, on Linux) used for those glyphs. | Windows captures: all render; Wordle 7.63 → 1.97. |
| **W20-4** | Windows layout | Wordle heading at a fixed y on one line; with Georgia's Windows metrics it ran under the Hard-mode pill, while production's flex header wraps it to two lines. | The heading wraps in the width left of the pill; everything below moves by the extra height (`head-extra`). Unchanged where it fits (Linux). | Windows capture: modal geometry equals production's (top 107 / bottom 872). |
| **W20-5** | Windows layout | Durak `TRUMP` column hard-coded at Liberation Sans's 39.9 px, so it read "TRUMI" with Arial. | Column width `max(39.9px, preferred)`, anchored right. | Windows capture and real play. |
| **W20-6** | Portable | The album's **left** page-turn zone had a width but no `x`, so Slint centred it on the spine: clicking the left edge never turned back or closed (keyboard ← worked). | `x: 0px`. | `STUDY_NATIVE_INPUT` replay (in-app dispatch, platform independent): 5–6 → 3–4, and the first spread closes; real clicks too. No UI test harness exists to unit-test geometry. |
| **W20-7** | Portable | The rest-timer face `TextInput` had `input-type: number`, which drops `:`, so MM:SS could not be typed ("00:07" became 7 minutes). Production's field is plain text. | Removed. | Real keyboard: "00:07" → 00:07, "Rest, 1 minute" (production's rounding), counts down, "abc" discarded. |
| **W20-8** | Portable parity | "Full house!" chip not green (`.break-stats-chip.collection { color: #22c55e }`). | `tone` property on the chip. | Windows capture. |

W20-6/7/8 also affect Linux; the Linux parity numbers in §22 predate them (the W20-2/3/4/5 fixes are metric-driven and leave the Linux layout unchanged where text fits). A Linux re-capture is recommended at the next Linux session.

### 43.4 Functional results (real Win32 input unless noted)

Real-input runs used `SendInput` clicks and keys with guards: every click asserts that the app owns the point, and every key asserts that the app is foreground. One early unguarded Geodle run leaked keystrokes into another window and was discarded.

| Area | Result | Class |
|---|---|---|
| Smoke: release exe opens with no console; FN Break Room, Break tab, Wabi Rest submenu, Games, Meditation, Achievements, album, four local games, Travle/Skribbl placeholders | PASS | A |
| FN Break Room: heading, XP bar/label, streak, quote, six cards (locked/unlocked/Play), footer pills, pet rock | PASS after W20-2/8 | A |
| Wabi Rest: sidebar Rest + rooms, tree, rock, token column, cards, light/dark/Sakura | PASS | A |
| Album: closed cover, open (cover click), ←/→, right zone, left zone (after W20-6), Escape close; 30 open→turn→close cycles then **0 frames**; `break_writes` unchanged | PASS | A |
| Durak: select, Attack, CPU defence, Throw/Pass, CPU attack, Defend/Slide/Pick up, loss → "Try Again"; mid-game state **identical after restart** | PASS | A |
| Wordle: physical typing, Backspace, "Not in the word list.", on-screen key, Enter, solve ("Solved in 3.") persisted | PASS after W20-3/4 | A |
| Geodle: focus opens the list, filter, "No countries found.", duplicate rejected, mouse pick, Enter submit, "Solved in 4." persisted; dropdown anchored after a live resize | PASS | A |
| Flaggle: mask, dropdown thumbnails (wheel-scrolled panel), Nepal (0.0 %), Jamaica 100 % with full reveal | PASS | A |
| Travle / Skribbl: placeholders open; **0 TCP / 0 UDP endpoints** over 20 s each; no Stage 21/22 code | PASS | A |
| Tokens at 44/45/89/90/269/270/330 min: XP 44/0/44/0/44/0/15 of 45; one token at 45 (a double click unlocks once), two at 90, six at 270; the next day resets unlocks and plays | PASS | A |
| Pet rock: 998 → 1 000 stage change (Heavenly → Cosmic), each pat one write, 1 001 after restart, frames settle | PASS | A |
| Achievements: Break Room / Pet Rock / Fossil / Garden rows with dates in the album (Garden derived, no Garden UI) | PASS | A |
| Rest timer: edit (after W20-7), start, elapsed countdown, completion, reset, invalid input | PASS | A |
| Breathing: phases on elapsed time, round 4 of 4 at 74 s, finished at ~78 s, then 0 frames | PASS | A |

### 43.5 Frame behaviour (A) and CPU (B)

`scripts/stage20-perf.ps1` (stage19-perf methodology):
- **Static surfaces: 0 frames** over the measured interval, no store write, CPU 0.00–0.08 % (B). Covered: FN Break Room (full/unlocked), Wabi Games light/dark, Meditation, album closed/open, all six games in FN and Wabi, and the Break Room with a Timer running.
- **Animations are bounded and stop** (0 frames afterwards): album hinge/turn, the breathing ring, the pet-rock celebration and the rest-timer vessel. The breathing ring copies production's per-second 1 s linear size change, so it eases continuously while the exercise runs.
- **Sakura visible with the Break Room:** ~21.5 logical ticks/s against 24 Hz, with 2 frames per tick (the known Slint 1.17 redundant redraw). It costs ~105 % of one core under llvmpipe (B, not comparable to the physical 12.7–13.7 %).

### 43.6 Minimized / tray-hidden (A)

Album open + Sakura + Timer and Durak/Wordle open + Timer: minimized and tray-hidden windows render **0 frames, 0 Sakura ticks**; restore resumes one clock.

`scripts/stage20-lifecycle.ps1` first visits every Stage 20 surface via `STUDY_NATIVE_BREAK_STRESS`. Results:
- **200 tray hide/restore + 200 minimize/restore, 0 failures**;
- 1 process, tray icon present before and after;
- Sakura `starts − stops = 1`;
- Private WS 94.2 → 95.0 MB, flat;
- `break_writes=0`.

### 43.7 Memory (B)

Private WS / Private Bytes (MB). llvmpipe keeps textures in process memory, so these are not comparable with Linux RSS or physical Windows.

| State | Private WS | Private Bytes | Class |
|---|---|---|---|
| Fresh launch (Dashboard) | 70.9 | 94.8 | baseline |
| Stored profile, Dashboard | 70.6 | 96.3 | baseline |
| FN Break Room | 67.7–69.5 | 95.8–97.3 | plateau |
| Wabi Games / Meditation | 64.5–68.3 | 88.4–92.2 | plateau |
| Album closed / open | 117.7 / 158.3 | 145.2 / 204.1 | transient while shown |
| Games open (FN / Wabi) | 69.4–80.1 / 76.8–87.9 | 97–108 / 106–118 | plateau |
| Navigation stress, 500 cycles (7 000 steps) | 87–106; first 104.8 → last 99.3 | 117–139 | **plateau**; album peak released each cycle; `break_writes=1`; 6 PNGs decoded |
| Durak reset stress, 200 (solver) | 87.3 → 91.5, flat | 119–126 | **plateau**; writes 2 (initial + play log) |
| After lifecycle stress | 95.0 | 130.7 | plateau |

No suspected leak. Threads are 17–26, including Mesa's rasterizer threads (Linux: 10).

### 43.8 Startup (B)

`STUDY_NATIVE_STARTUP_REPORT`, 12 launches each, two interleaved rounds in the same VM:

| Build | Fresh | Stored |
|---|---|---|
| Stage 19 (`e2e1a56`, built from `git archive`) | 672–677 ms | 679–753 ms |
| Stage 20 | 672–676 ms | 676–678 ms |

Stage 20 opening directly on the Break Room: 778 ms. **No Stage 20 startup regression.** The absolute ~670 ms against the physical 161 ms comes from the VM and software GL; the physical comparison is C.

### 43.9 Platform regression (A)

- **Single instance** (`scripts/stage20-instance.ps1`, with a Break Room game open in the primary):
  - 8 simultaneous launches → **1 survivor**;
  - 20 second launches → 0 failures; the hidden primary was shown each time, with 1 process and one tray icon throughout;
  - `store.json` hash unchanged, so there was no second writer; playedBreaks 7 → 7;
  - median 1 013 ms per second launch (Stage 18 physical: 1 005 ms).
- **Tray:** icon present; left-click restores; close-with-session hides; menu → Quit exits and removes the message window and the icon. The log ends "event loop exited normally" with 0 panics (W20-1).
- **Notification** (Demo 10 s, with the Break Room and Sakura visited):
  - sessions 95 → 96; the dashboard recomputed once and the Break Room evaluated once;
  - **exactly one** `notification shown: FocusFinished`;
  - the VM shell **displayed** the toast once ("Study Tracker (Native Preview) — Focus session finished").
- **Updater:**
  - disabled for source builds by default;
  - 35 updater tests pass on Windows (valid stage, bad/foreign signature, corrupted artifact, production key id, install never executes);
  - release exe against a local test feed (`*.invalid`, no network): available → downloaded → **rejected by the production key**, nothing staged.
- **Stage 19 Sakura:** 500 style switches with Sakura on → `starts=1 stops=0`, 22 live petals, ~21.5 ticks/s afterwards (one clock).

### 43.10 Windows visual parity (A for layout; numeric B / VM-LIMITED)

Method: the same fixtures (`gen-break-fixture.mjs full`, tz −07:00), 1520×980, DPR 1, a frozen date, and `Math.random`/`BREAK_PICK` pinned at 0.25. Production renders in headless Edge; native renders via the in-process `STUDY_NATIVE_SNAPSHOT`. Both sides use the real Windows faces (Georgia/Arial/Consolas/Segoe UI Emoji/Symbol). The native side is rasterized by llvmpipe, so absolute numbers are VM-LIMITED.

| Surface | Windows mean abs diff (after fixes) | Linux (§22) |
|---|---|---|
| FN Break Room | 2.02 (2.05 before) | 1.02 |
| Wabi Games light / dark | 0.83 / 1.11 | 0.77 / 0.95 |
| Meditation | 0.63 | 0.73 |
| Album closed / open | 2.33 / 4.65 | 2.44 / 4.86 |
| Durak | 1.88 | 1.22 |
| Wordle | 1.97 (7.63 before) | 1.38 |
| Geodle | 5.15 | 1.46 |
| Flaggle | 2.47 | ~1.5 |
| Travle / Skribbl | DEFERRED | deferred |

Remaining Windows differences (accepted as CLOSE):
- **Geodle:** Chromium's clue chips are 55 px tall against native's 50 px, because its line box grows with the Segoe UI Symbol fallback for ✓/✕/▲. Production's table and modal are therefore 21 px taller, offsetting the whole modal. No clipping or overlap.
- **Hard-mode pill:** stays on one line natively, while production stacks "Hard / mode".
- **Rotated text:** the stamp and album text render heavier (already in §39, and the same on physical Windows in Stage 17).
- **Emoji:** small ±1–3 px offsets.

### 43.11 DPI / resize (A, partial)

- **100 %:** PASS.
- **125 %:** **emulated** with `SLINT_SCALE_FACTOR=1.25` on the FN Break Room, Wordle, album and Geodle: no clipping, Wordle wraps, panels scroll at 92 % height. A real 125 % session needs a sign-out and was not run (C).
- **Live resize** 1240×700 ↔ 900×600 with the Geodle dropdown open and the album open: the dropdown stays anchored and the album rescales.
- **Below production's 1180×760 minimum** (e.g. 600 px tall), the Wabi sidebar's "Circle" meets the footer. That's out of range. Native's own minimum (460×340) is looser than production's (pre-existing, Stage 24).

### 43.12 Accessibility / IME

UI Automation tree (what Narrator reads): **PARTIAL** — inspected, not spoken. Representative entries:
- game cards: "Play" buttons;
- Wordle: tiles "S, present" / "R, correct"; Hard mode; Close;
- Geodle: `Edit: Country`, SUBMIT, clue texts;
- Durak: hand cards "Q♦"…, "Hidden card", Attack;
- album: "Open the album" / "Close the album" / "Next pages";
- Meditation: "Begin breathing exercise";
- Wabi Games: "Break timer 05:00, click to edit", "Pet rock: Cosmic Rock · 1234 pats".

Decorative art does not flood the tree: 2–9 unnamed images per surface, and 15 on the open album (decor and art without names — minor). Narrator speech: NOT RUN. Japanese IME: **NOT RUN — IME UNAVAILABLE** (en-US only).

### 43.13 Still pending physical Windows (C)

- GPU/CPU performance and Sakura cost on real hardware;
- high-refresh behaviour;
- startup against the 161 ms baseline;
- **physical sleep/resume (PENDING STAGE 24)**; VM suspend was not used as a substitute;
- real 125 % scaling;
- Narrator speech;
- Japanese IME composition;
- release performance qualification.

### 43.14 Verdict

All portable tests pass on Windows. Every functional Windows gate a VM can close passes after fixes W20-1…8, no leak or lifecycle defect remains, and visual behaviour is acceptable. **Stage 20: PASS WITH CONCERNS — WINDOWS FUNCTIONALLY VERIFIED.**

Concerns:
- portable Stage 20 was committed and pushed (`836a49b`) before verification; the Windows fixes are a separate, unpushed commit on top;
- W20-1 is a Stage 18 defect that physical testing had masked;
- Linux parity should be re-captured for W20-6/7/8;
- the physical-hardware items above remain Stage 24 gates.

### 43.15 Final checks

After all fixes, on Windows: `cargo fmt --check` clean; `cargo check --workspace` 0 warnings; `cargo test --workspace` **158 core + 238 app = 396 passed, 0 failed, 1 ignored** (pre-existing ignore; +2 = the W20-1 regression tests); `cargo test -p study-tracker-core` 158 passed; `cargo build --release` 0 warnings; `git diff --check` clean; `git diff -- desktop` empty. No real production user data was read or written: every run used a throw-away `STUDY_NATIVE_DATA_DIR` and synthetic fixtures, production renders used throw-away Edge profiles, and no installed Study Tracker profile exists on the VM. New Windows scripts: `scripts/stage20-perf.ps1`, `stage20-lifecycle.ps1`, `stage20-memory.ps1`, `stage20-instance.ps1`.
