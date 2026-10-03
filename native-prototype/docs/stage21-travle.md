# Stage 21: Travle

## 1. Stage 21 verdict

**PASS WITH CONCERNS — PENDING WINDOWS VERIFICATION** (Linux portable pass, uncommitted for review).

- Travle is a real game now: a pure-Rust domain in `study-tracker-core`, golden-tested against production's own code, plus a native map and modal in Field Notebook and Wabi-Sabi.
- Concerns (none blocking): keyboard focus of buttons/dropdown rows (shared with Stage 20's combos); startup could not be timed cleanly in this session; the serif line-gap needs a Windows check; +0.86 MB stripped binary (+2.3 %). See §44.

## 2. Starting checkpoint

- `git status --short`: clean. `git log --oneline -8`: `7f1f533` (Stage 20 Windows verification) on top of `836a49b`, `e2e1a56`, `031f943`, `e23871f`, `713bdf9`, `5093b36`, `62ff25f`.
- `git diff --check`: clean. `git diff -- desktop`: empty.
- HEAD == `7f1f533`. Stage 22 not begun.

## 3. Production freshness

- Upstream `damcha02/destudydracker` `main` (read-only clone in a scratch directory, no remote added here): **`e5c5a94`** ("stuff", 2026-10-02), one commit after `fe2f7a6`. `git describe`: `v0.1.67-2-ge5c5a94`; `package.json` still `0.1.67`.
- `desktop/` here: tree hash `d293c2c` = exactly `fe2f7a6:desktop`.
- `e5c5a94` touches 8 files: exam `note`/`releaseDate` (types, storage normalization), `examPhase.ts` calendar markers, planner/Wabi calendar UI, the Wabi nav becoming scrollable, two `--wabi-vermilion` fallbacks. **Nothing** in Travle, Break Room, games, achievements, countries/map data, or game modals (the one rest-room CSS line in the diff is unchanged context).
- Decision: unrelated; Stage 21 is built against `fe2f7a6`; `desktop/` not modified. The new exam fields belong to a later sync (academic domain / Wabi planner).

## 4. Production Travle inventory

Traced from `App.tsx` imports, recursively:

| Item | Where (production) |
|---|---|
| Rules, daily pairs, routes, guess states | `src/lib/travle.ts` (264 lines) |
| Border graph `COUNTRY_BORDERS` | `src/lib/countryBorders.ts` (156 keys) |
| Countries `COUNTRIES` (names, codes, region) | `src/lib/countries.ts` (shared with Geodle/Flaggle) |
| Map `TRAVLE_MAP_COUNTRIES`, viewBox `0 0 1000 500` | `src/lib/travleMapData.ts` (195 entries, 155 KB) |
| Hash, PUZZLE id, date math | copies inside `travle.ts` (same as Wordle/Geodle/Flaggle) |
| Persisted `travlePuzzle` / `TravlePuzzleState` | `types.ts`, `storage.ts` (`makeDefaultTravlePuzzle`, `normalizeTravlePuzzle`) |
| Open-time reset `initTravlePuzzle` | `App.tsx` 7455–7482 |
| Submit `submitTravleGuess`, select | `App.tsx` 8554–8610 |
| Render-time derivations (route, states, viewBox, route points) | `App.tsx` 5115–5172 |
| Modal JSX (head, cards, map SVG, zoom, chips, combo, result card) | `App.tsx` 14940–15085 |
| Card status Solved/Failed, Play button, unlock | `App.tsx` 11603, 14850 (+ Stage 20 economy) |
| Styles | `App.css` 12737–13300 (`.travle-*`), FN `.travle-card`, global FN/Wabi `input`/`h2` rules |
| Chunking | `vite.config.ts` (`game-travle`, `game-travle-map`) |

There is no Travle-specific achievement: Travle counts only through unlock/play (Explorer, Full House, Perfectionist, daily badges). No network, no assets beyond the data files, no animation besides a 120 ms fill/stroke transition, no hover/tooltip on the map, no keyboard shortcut (no Escape close), no map click.

## 5. Exact production rules

- **Valid country (a guess):** any of the 195 `COUNTRIES` whose `normalizeCountryName` equals the input's. The dropdown lists only the **156 with borders**, but typing an isolated one (e.g. "Australia") is accepted.
- **Border:** `COUNTRY_BORDERS[a]` contains `b` (land borders as production lists them; no maritime links; directed lists).
- **Playable / daily candidates:** countries with ≥1 border, codes sorted (`Array.sort`).
- **Daily pairs:** every ordered pair (start ≠ target) whose BFS shortest path needs **3–5 guesses** (path length 4–6). 5,414 pairs.
- **Daily pick:** pairs sorted (stable) by `hashString("<salt>:<startName>:<targetName>:<index>")` (FNV-1a over UTF-16), index `daysSinceFirstPuzzle(date) % 5414`, days from 2026-01-01 UTC-parsed (V8 roll-over, invalid/early dates → 0).
- **Salt:** random UUID per profile (`crypto.randomUUID`), stored. **Puzzle id:** `"<date>:<base36(hash(salt))>"`.
- **Shortest path:** BFS on outgoing edges in list order, first-found route wins.
- **Guess limit:** 7 (`TRAVLE_MAX_GUESSES`). The start counts as on the route but not as a guess.
- **Submit:** finished → ignored; not a country → "Select a country from the list." + dropdown opens; the start or a repeat → "That country is already in your route." (draft kept); else appended.
- **Win:** a route exists through start, target and guessed countries only (BFS restricted to that set). Message "Route complete: A -> B -> …." (solved path).
- **Loss:** 7 guesses without a route → "Route closed. Shortest path: …."
- **Otherwise feedback:** "X is on the route." / "X could help connect the route." / "X is off route." from the guess state:
  - *route*: on the solved path, the display path or the shortest path;
  - *possible*: `dist(start,x) + dist(target,x) ≤ |shortest path nodes| + 2` (distances along outgoing edges);
  - *miss*: otherwise.
- **Display path (map line):** the solved route, else the longest simple chain from the start through guesses (ties: closest to the target, then first found).
- **Completion state:** `completed`, `won`; the result card replaces the input.
- **New day:** load normalization and `initTravlePuzzle` replace a non-today puzzle; the card's Solved/Failed only shows for today's puzzle.

## 6. Production quirks (preserved, documented)

1. **One-way edge:** Sri Lanka lists India, India does not list Sri Lanka. Sri Lanka can start a route; it can never be reached (so it is never a daily target).
2. **Isolated countries are valid guesses** (always off route, and on the map they get no state colour — the map looks states up through the dropdown's table — only the `current` outline).
3. **No aliases:** "Turkey"/"Turkiye" are rejected; "Türkiye" works because non-ASCII letters become word gaps. "Sao Tome" etc. likewise (São Tomé is isolated anyway).
4. **"possible" band uses node count, not step count**, so it is one step wider than it reads.
5. **Graph components:** Afro-Eurasia 130, the Americas 22, Haiti/Dominican Republic, UK/Ireland. Pairs never cross components.
6. **Load normalization does not validate names:** a stored unknown start ("Atlantis") stays; non-string guesses are dropped *before* the 7-cap; truthy `completed` strings count.
7. **"1 steps"** (no singular) in the step counters.
8. **The typed text is `#1f2933` in every style**, i.e. nearly invisible on Field Notebook dark's surface (the FN input rule overrides background but not colour). Placeholder `#757575`.
9. Draft, message, dropdown and **zoom persist across closing/reopening** (App-level React state) and reset only when a new puzzle is drawn.
10. A one-point route "polyline" draws nothing.

## 7. Data sources / provenance

| Data | Production file | Stated source | Status |
|---|---|---|---|
| Names, ISO codes, region | `countries.ts` | "mledoze/countries plus samayo/country-json" (comment) | already in the repo since Stage 20; licence confirmation → Stage 24 |
| Border lists | `countryBorders.ts` | none stated | **unknown** → Stage 24 |
| Map geometry | `travleMapData.ts` | none stated (likely Natural Earth-derived, unverified) | **unknown** → Stage 24 |
| Aliases | none exist | — | — |
| Flags | not used by Travle | — | — |

No new third-party data was added: both native files are verbatim extracts of production files (`scripts/stage21-extract-data.mjs`), and the map file **replaces** Stage 11's copy of the same geometry.

## 8. Country identity

- `CountryId(u16)` = position in production's `COUNTRIES`; canonical key = ISO alpha-3 `code` (graph and map key). Map regions resolve by code (`MapCountry.id`); flags are not involved.
- Display name = production `name` = the **persisted** form (production stores names in `start`/`target`/`guesses`), so the public API takes/returns names exactly like `travle.ts`.
- Normalized input → `CountryId` through one prebuilt table (`find_travle_country`), first match wins (no collisions exist; tested).
- Tested: codes unique, names unique, every country resolves by name and by code, every map region is a game country and vice versa.

## 9. Normalization

Production's `normalizeCountryName`, shared with Stage 20 (`countries::normalize_country_name`): trim (incl. BOM) → lowercase → every run outside `[a-z0-9]` becomes one space → trim. Golden-tested on 465 inputs (all names, upper-cased/padded/hyphenated variants, diacritics, apostrophes, abbreviations, alternate names, junk). No aliases added.

## 10. Border graph

`data/break_room/borders.tsv` (verbatim; key and neighbour order kept, since BFS order decides ties). `BorderGraph::parse` rejects unknown codes, self-edges, duplicate edges, duplicate rows and malformed lines (tested). Parsed once on first use (`OnceLock`), never at startup.

Invariant tests: 156 listed countries, all with ≥1 neighbour; the 39 isolated are exactly production's; no self-edge; no duplicates; the **only** one-way edge is `LKA→IND` (encoded as the documented exception); component sizes `[130, 22, 2, 2]`.

## 11. Route algorithm

- Plain BFS (equal weights), parent tree with "seen when queued" — identical routes to production's path-array queue (proved by 4,018 golden paths).
- Restricted BFS for the solved route; full BFS distances for guess states; the display path is ported literally (enumerates simple chains over ≤ 8 countries).
- Daily pairs: one full BFS tree per start (156) instead of production's 24k early-exit searches — same parents, same routes — and a route is materialized only for kept pairs. Build ≈ 4 ms (release, one-time, lazy); a daily pick ≈ 0.4 ms.
- Separation: graph queries (`BorderGraph`), puzzle semantics (`Routes`, `TravlePuzzle`), presentation roles (`map_roles` — class flags only, no colours).

## 12. Daily puzzle generation

`puzzle_for_date(date, salt)` reuses Stage 20's `daily_index` (n-th smallest `(hash, index)`), now hashing the shared `"<salt>:"` prefix once instead of formatting a string per item (FNV is sequential; all Stage 20 daily goldens still pass). Golden-tested: 141 dates × 5 salts = 705 rows (120 consecutive days, year boundary, leap day 2028-02-29, EU/US DST days, pre-epoch, invalid, roll-over dates) plus 5 timezone probes (the same instant is a different local day in Zurich, Los Angeles, Tokyo, Auckland, São Paulo — production feeds its local day; native injects the local date, so only date → puzzle is domain logic).

## 13. Golden fixture methodology

```
desktop/src/lib/*.ts (copied read-only to a temp dir, Node 25 type stripping)
App.tsx handler/derivation TEXT cut by marker, types stripped (node:module), run verbatim with state shims
        ↓  scripts/stage21-goldens.mjs (fixed mulberry32)
crates/study-tracker-core/tests/fixtures/break_room/travle.json   (668 KB)
tests/fixtures/travle-view.json                                    (127 KB)
        ↓
Rust tests compare exactly
```

| Fixture | Count |
|---|---|
| daily puzzles | 705 + 5 timezone probes |
| name lookups / normalization | 465 |
| filter queries (full result lists) | 19 |
| neighbour lists (all countries) | 195 |
| directed adjacency pairs | 603 |
| shortest paths | 4,018 |
| games through the real handlers | 205 games, 1,262 submits (messages, draft/dropdown effects, state) |
| raw route queries (corrupt/unknown/isolated/duplicate guesses) | 300 |
| load normalization through `loadAppState` | 17 stored shapes |
| open-time init / rollover | 7 |
| map views (viewBox incl. 6 zoom levels, route points, classes) | 323 |
| zoom-button rounding | 12 |

Only Vite-only module features unused by Travle/storage are shimmed (`import.meta.glob` for flag URLs, the social env URL).

## 14. Core architecture

`crates/study-tracker-core/src/break_room/travle.rs` (pure; no I/O, clock, Slint, colours, pixels): `CountryId`, lookup/normalization, `BorderGraph`, `Routes`, daily pairs/pick/id, `TravlePuzzle` (`fresh`, `normalized`, `ensure_today`, `submit`, `is_today`), `TravleSubmit`, `GuessState`, `MapRole` (`map_roles`, `display_route`). `TravlePuzzle` moved here from `state.rs` (same fields). Catalog: Travle `Availability::Local`; `MapStage21` removed.

## 15. State machine

Production has no explicit state enum; native keeps its exact shape: `completed`/`won` flags on `TravlePuzzle` (InProgress = `!completed`; Won; Lost = `completed && !won`; "Ready" is just zero guesses). Commands: `submit(draft)`, `ensure_today(today)` (open), `normalized(today)` (load). Outcomes: `TravleSubmit::{NotACountry, AlreadyInRoute, Accepted{message}, Ignored}`. View state (draft, message, dropdown, zoom) lives in the app controller like production's React state.

## 16. Persistence

- Same `travlePuzzle` fields and names in the native `break_room` section (`seedSalt, activeDate, puzzleId, start, target, guesses, completed, won`); Stage 20's tolerant parser/writer unchanged.
- Load: `normalizeTravlePuzzle` now runs (Stage 20 only filled a missing salt). Old Stage 20 stores (empty or placeholder puzzle) load into today's puzzle; production backups import as before; import stays idempotent; rollback unchanged.
- Like production, unknown keys **inside** `travlePuzzle` are dropped (production rebuilds the object); unknown keys elsewhere in the section are preserved (Stage 20).
- Writes: only an accepted guess (and the Play log, as in production) persists. Load normalization of an old day does not write (it is re-derived on every launch, like the other games).
- Tests: fresh, mid-game restart, won restart, lost/old-day/wrong-id/placeholder/missing-salt/numeric-salt/impossible names/too many guesses/non-array/array/string/null/extra-keys/absent — all against production's `loadAppState` output.

## 17. Stage 20 integration

Reused unchanged: unlock economy, play log, card status (`today_done`), achievements, `BreakRoomController` (new `travle_*` actions only), `FileBreakRoomPort`, FN page and Wabi Rest cards, `GamesData` push path, `game_tokens`, shared `Panel`/`Backdrop`/`CloseButton`/`CountryCombo`/`SubmitButton`. Travle → Explorer/Full House/Perfectionist and daily badges tested exactly once (`travle_open_play_counts_for_explorer_and_full_house_exactly_once`).

## 18. Stage 11 reuse

| Stage 11 piece | Fate |
|---|---|
| Production geometry copy (`world-countries.tsv`) + `extract-map-data.py` | **Replaced** by `assets/map/travle-map.tsv` (adds production bounds/points; one copy in the binary) |
| Static world-space paths + one scaled layer (`transform-scale`, offset for centre scaling), stroke ÷ scale | **Reused** (the Travle map is exactly this) |
| `map/dataset.rs` stress generators (Dense×10/×50, Cells, Giant) | **Reused**; World level now built from Travle's data + core country table |
| `map/viewport.rs` free pan/zoom, `map/hit.rs` hit testing, labels/markers | **Kept in the lab only** — production Travle has no pan, wheel, click or hover |
| Map lab wiring | **Adapted**: built lazily on first show (no geometry parsed or 195 rows pushed at startup any more) |

## 19. Map data architecture

`travle-map.tsv` (embedded) → `travle_map::countries()` (parsed on first Travle open, `OnceLock`) → per-thread `SharedString` paths (made once) → a persistent 195-row `VecModel<TravleShape>` updated in place (`sync_model`: a guess changes a handful of rows; paths never re-sent) → Slint `Path` per country. Per push: role lookup + style; no reparsing, no per-frame work.

## 20. Rendering architecture

- Water rect, graticule (under the countries, so land hides it — production's visual), **all 195 countries in production order** (plain ones are the water colour but cover the graticule and earlier strokes exactly like the SVG), route polyline (round caps/joins, 0.88 opacity), markers as r = 3.6 circles.
- Styles: `travle_map::style` reproduces the CSS cascade (route, possible, miss, start, target, current, marker; last rule wins per property), verified against `getComputedStyle` values. Non-scaling strokes = width ÷ scale.
- `animate fill, stroke 120ms ease` = production's transition (bounded).
- Map colours are fixed in production (theme-independent); the frame around them uses theme tokens.

## 21. Viewport behaviour

Production's: viewBox = bounds of the route's countries + destination, min 120 × 90, +40/+35 margins, ÷ zoom, clamped into the world; `+`/`−` step 0.35 within 1–2.5 with production's rounding; `preserveAspectRatio` meet (centred). Ported exactly (323 golden views). Map height `clamp(230px, 34vh, 320px)` (≤ 820 px tall: `clamp(190px, 30vh, 260px)`), width = card. No pan/wheel/recenter (production has none). Resize/DPI: positions are logical, recomputed from the window size.

## 22. Hit testing

None needed: production's map has no pointer handlers. Not implemented (the Stage 11 lab keeps its hit testing for diagnostics).

## 23. Input / autocomplete

Stage 20's `CountryCombo` reused, extended with optional properties (defaults = Geodle/Flaggle unchanged): field height/background/border/radius/ink, two-line rows (name over region, 54 px), empty text "No connected countries found.", and a `cleared` callback (production's × does not clear the message). Typing opens and filters the list (first 80, playable countries, production order), focus opens it, ▾ toggles, a row picks, Enter or STEP submits.

## 24. Field Notebook UI

Measured production boxes (1520 × 980): modal 880 × 860 at y 60, padding 18, gap 12, radius 22, glow 24 %, head (kicker / 36 px serif heading / counts), Start/Destination cards 64 px, map card 322 px, chips, Latest guess card 82 px, input 46 px + STEP, status, Shortest card; result card (kicker, 32 px serif, route block, 3 stats, legend, Close). FN input rule: 1 px border, radius 2, surface background. Line boxes follow Chromium's `line-height: normal` = round(ascent) + round(descent) + round(gap) (from Slint `font-metrics`), which fixed a 1–3 px drift.

## 25. Wabi-Sabi UI

Same modal (production shares it) with Wabi's differences: tokens from Wabi variables (paper, ink, sage accent), heading letter-spacing 0, 0.005 em text tracking, **white square input with a 2 px `color-mix(border 80 %, text)` border**, and the 10 px `scrollbar-gutter` that keeps every fixed overlay out of the right edge (§bugs).

## 26. Themes

FN light/dark, FN + Sakura, Wabi light/dark, Wabi + Sakura captured. Travle has no theme state of its own (reads `Game` tokens/flags set by the appearance controller). New tokens: map-card border mix, won/lost card backgrounds, stat background.

## 27. Visual parity

Production: scratch `vite build` of a copy of `desktop/` + `design/` in headless Chromium (DPR 1, frozen 2026-09-30 12:00 Zurich, CSP font blocking). Native: Xwayland, scale 1, `STUDY_NATIVE_SNAPSHOT`. Both import the same synthetic backup whose `travlePuzzle` is today's production puzzle in a fixed state (`gen-break-fixture.mjs --travle fresh|mid|won|lost`); Syria → Bhutan; mid = Iraq, Iran, Albania. Script: `scripts/visual-parity/travle-pair-linux.sh`. Mean abs per-channel difference (0–255), whole window:

| # | Fixture | Diff |
|---|---|---|
| 1 | FN dark — fresh | **1.22** |
| 2 | FN dark — mid-game | **1.30** |
| 3 | FN dark — won (modal scrolls) | **1.92** |
| 3b | FN dark — lost | **1.85** |
| — | FN light — mid | **1.64** |
| 4 | Wabi light — fresh | **1.60** |
| 5 | Wabi dark — mid | **1.05** |
| 6 | Wabi light — won | **2.26** |
| 6b | Wabi dark — lost | **1.54** |
| 7 | FN dark + Sakura — mid | **1.88** |
| 7b | Wabi light + Sakura — mid | **1.72** |
| — | Wabi light 1700 × 1100 | **1.57** |
| — | FN dark 1100 × 760 (compact) mid / won | **2.32 / 2.98** |
| — | FN dark scale 1.25 (1216 × 784 logical) | **2.78** |

Region classification:

| Region | Status |
|---|---|
| Modal geometry, glow, shadow, cards, borders | MATCH |
| Map: water, graticule, country fills/strokes, route line, zoom controls, watermark, viewBox | MATCH (edge antialiasing only) |
| Route chips + arrows | CLOSE (dash pattern approximated; arrow glyph ±1 px) |
| Input, STEP, status, shortest | MATCH / CLOSE (text antialiasing) |
| Result card (route, stats, legend, Close) | CLOSE |
| Typography | CLOSE (femtovg vs Chromium antialiasing; Linux fallback faces as Stage 20) |
| Close ✕ glyph | DIFFERENT on Linux (Chromium falls back to a face drawing "X"; same as Stage 20) |
| Dropdown open | CLOSE (structure measured; not separately scored) |

## 28. Static rendering

Every static Travle state renders **0 frames, 0.00 % CPU** (FN and Wabi; open untouched, mid-game, completed). No Travle timer or polling exists; only input or the bounded 120 ms colour fade draws.

## 29. Performance (Linux, release, 1520 × 980, 20 s samples)

| Scenario | CPU | RSS | Anon | Threads | Frames |
|---|---|---|---|---|---|
| Stage 20 HEAD: Break page | 0.00 % | 88.1 MiB | 24.5 | 10 | 0 |
| Stage 20 HEAD: Travle placeholder open | 0.00 % | 94.8 | 25.5 | 10 | 0 |
| P21-0 Break page, Travle closed | 0.00 % | 88.2 | 23.8 | 10 | 0 |
| P21-1 Travle open, untouched | 0.00 % | 96.0 | 26.1 | 10 | 0 |
| P21-2 Travle, 3 guesses | 0.00 % | 95.9 | 26.0 | 10 | 0 |
| P21-2w same, Wabi dark | 0.00 % | 97.1 | 26.7 | 10 | 0 |
| P21-3 Travle completed | 0.00 % | 91.3 | 26.1 | 10 | 0 |
| P21-2s FN + Sakura (petal clock; HEAD Wordle + Sakura: 28.9 %, 973) | 28.5 % | 95.0 | 29.0 | 10 | 974 |
| P21-4 typing 25/s (dropdown re-filter) | 68.0 % | 92.5 | 26.6 | 10 | 670 |
| P21-5 resize 25/s | 83.8 % | 92.8 | 26.8 | 10 | 1,033 |
| P21-6 open/close 25/s | 51.0 % | 96.6 | 26.5 | 10 | 659 |

Persistence writes per scenario: §35. Model counts: the shapes model stays at 195 rows; the options model ≤ 80.

## 30. Map stress (Stage 11 levels, map lab pan benchmark, 8 s)

| Level | Pan bench CPU / frames | Idle after load anon |
|---|---|---|
| World (production) | 79.7 % / 590 | 28.0 MiB |
| Dense ×10 | 100 % / 404 | 49.9 |
| Dense ×50 | 100 % / 78 | 141.8 |
| Cells (2,000) | 108 % / 551 | 38.1 |
| Giant (50k path) | 99.7 % / 226 | 82.2 |

No crash; idle 0.00 % CPU (2–3 frames per 10 s window from the lab's own UI). Same shape as Stage 11 (cost ∝ vertices; Dense ×50 diagnostic only). Travle itself only ever draws the production map.

## 31. Memory

| Point | Anon |
|---|---|
| Baseline (Break page) | 23.8 MiB |
| First Travle open / several guesses | 26.1 / 26.0 |
| During/after 500 open/close cycles | 26.4 → 26.6 (plateau) |
| 400 played puzzles | 28.0 → 28.2 (plateau) |
| Stress-map high-water (Dense ×50, lab only) | 141.8 |

Live state ≈ +2.3 MiB while Travle is open (shapes, map strings, Slint item tree); the daily-pair table (~0.5 MB) is one-time. RSS swings of ±4.5 MiB between samples are GL/shared pages, not anon. No unbounded growth.

## 32. Open/close stress

`STUDY_NATIVE_TRAVLE_STRESS=open:500` (open/close via the real push path, zoom and dropdown toggled on the way): anon plateau 26.4–26.6 MiB, threads 10 throughout, **0 extra writes**, no duplicate models (shapes reuse one `VecModel`), no timers (none exist).

## 33. Guess stress

`guess:100` / `guess:400`: each puzzle = a new day's puzzle, two off-route guesses, then the shortest route, through the real submit path. Writes = accepted guesses exactly (498 / 1,990). Memory plateau 28.1–28.2 MiB. Stored state stays bounded (≤ 7 guesses per puzzle; one puzzle stored). Map rows restyle in place; no stale highlights (a new puzzle recomputes every role).

## 34. Date rollover

Tested in the controller (`travle_rolls_over_at_local_midnight_without_polling_or_double_counting`): the old puzzle stays stored until opened, the card stops showing Solved/Failed, unlocks reset per day, nothing is counted twice; a launch on the new day normalizes to the new day's puzzle with the same salt and **no write**. Driven by the existing day-change path (`sync` with the new date), no Travle polling.

## 35. Persistence churn

| Action | Writes |
|---|---|
| Opening/viewing, reopening, closing | 0 (Play itself logs the play once, as production) |
| Typing, picking, dropdown, ×, zoom | 0 |
| Resize, theme rendering, idle | 0 |
| Not a country / start / repeat / finished | 0 (production: state unchanged) |
| Accepted guess (incl. the completing one) | 1 |
| Load normalization to a new day | 0 |

## 36. Startup

- Nothing Travle-specific runs at startup unless the stored puzzle is from an older day: then load normalization draws today's puzzle and builds the 5,414-pair table (≈ 4 ms release, once). The map file is not parsed until Travle opens.
- The map lab no longer parses the world and pushes 195 region rows at startup (lazy), which offsets that.
- Measured first frame (7 launches, Xwayland with the capture display on a hidden workspace): medians 266–293 ms for both Stage 20 and Stage 21, mins S20 241.7–242.3 vs S21 236.8–240.1 ms. The session's frame-callback throttling dominated (Stage 20's documented 86–92 ms was taken with a visible display), so **no regression is detectable, but a clean number is pending** (Windows pass / visible display).

## 37. Binary size (Linux release)

| | Stage 20 (`7f1f533`) | Stage 21 | Δ |
|---|---|---|---|
| unstripped | 55,745,432 | 57,471,752 | +1.73 MB |
| stripped | 37,206,928 | 38,070,632 | **+0.86 MB (+2.3 %)** |
| `.text` | 20,583,981 | 21,217,661 | +634 KB |
| `.rodata` | 9,800,092 | 9,808,900 | +9 KB |

Attribution: code, mostly Slint-generated Travle UI (modal, chips, result card bindings; Travle-named symbols +384 KB), plus the core module and diagnostics. Data ≈ 0 (`travle-map.tsv` 149 KB replaces Stage 11's 152 KB; borders 3 KB). No dependencies.

## 38. Accessibility

The map is **one** `image` node labelled "Map route from X to Y" (no per-country nodes; the 195 paths and graticule are not in the tree). Exposed: modal group "Daily Travle puzzle", Start/Destination/Latest guess/Shortest texts, the route as a list of chip texts, zoom buttons ("Zoom in"/"Zoom out", default actions), the combo (text input "Enter a country", Clear country, Show countries, list items), STEP button, status text, result group with the route, Close. Countries are not clickable in production, so none are.

## 39. Keyboard / input

Type → list filters; Enter submits; ▾/× by pointer; Escape does nothing (production binds none for Travle); Tab reaches the text field; no focus trap. Like Stage 20's Geodle/Flaggle combo, dropdown rows and the buttons are pointer/accessibility-action targets, not Tab stops (production's are Tab-focusable buttons) — known difference (§44).

## 40. Stage 20 regression

- All Stage 20 tests pass (core and app; the Travle placeholder test was replaced by real-game tests).
- Modal parity re-measured against production (FN dark, same fixture): Durak 1.24, Wordle 1.34, Flaggle 1.23, Geodle 1.67 (Stage 20 recorded 1.22 / 1.38 / ~1.5 / 1.46 on its own fixture).
- Wabi Wordle: 10.46 → **8.23** (gutter fix).
- Shared changes: `CountryCombo` (new optional props; defaults unchanged), `Panel` (scroll through the padding), `CornerGlow` (fade radius), game host (Wabi gutter). Economy, achievements, album, Durak, rest/breathing untouched.

## 41. Stage 19 regression

Sakura: one clock, 22 petals (`live_petals=22` in every STATS line), petal frame rate with a modal open identical to HEAD (974 vs 973 frames / 20 s); `prefs_writes=0` during every stress; FN/Wabi/Sakura captures match production. Hidden/minimized discipline untouched (no change to its code).

## 42. Dependencies

None added. Node scripts use only Node built-ins. Platform code (Stage 18 tray, single instance, notifications, updater) untouched.

## 43. Licensing / provenance

See §7. For Stage 24: confirm the licences of `countries.ts` sources (mledoze/countries, samayo/country-json), and identify the origin of `countryBorders.ts` and `travleMapData.ts` (unattributed in production). Nothing was deleted or replaced except Stage 11's duplicate copy of the same map.

## 44. Known differences

1. Keyboard: dropdown rows/buttons not Tab stops (shared with Stage 20 combos).
2. Dashed destination chip: Chromium's dash pattern approximated (3 px dashes); arrow glyph ±1 px.
3. Close ✕ draws as ✕ natively; Linux Chromium shows a fallback "X" (Windows uses Segoe UI Symbol in both, per Stage 20).
4. Serif line gap: Liberation Serif metrics assumed for the heading line box; Georgia (Windows) has none → possibly 1–2 px; verify on Windows.
5. Stage 20 observation (not changed in Stage 21): Geodle/Flaggle native input text uses the theme text colour while production uses `#1f2933`, and their Wabi field keeps the FN look.
6. Text antialiasing (femtovg vs Chromium), as every stage.

## 45. Deferred work

- Windows verification (§51).
- A clean startup measurement.
- Production sync to `e5c5a94`+ (exam notes/release dates, Wabi nav) — not Travle.
- Licensing/provenance (Stage 24). Keyboard Tab stops for combos (shared polish).

## 46. Files changed

New:
- `crates/study-tracker-core/src/break_room/travle.rs`, `travle_tests.rs`
- `crates/study-tracker-core/data/break_room/borders.tsv`
- `crates/study-tracker-core/tests/fixtures/break_room/travle.json`
- `assets/map/travle-map.tsv`
- `src/travle_map.rs`, `tests/fixtures/travle-view.json`
- `scripts/stage21-extract-data.mjs`, `scripts/stage21-goldens.mjs`
- `scripts/visual-parity/travle-pair-linux.sh`, `scripts/visual-parity/open-travle.js`
- `docs/stage21-travle.md`

Removed: `assets/map/world-countries.tsv`, `scripts/extract-map-data.py` (Stage 11 duplicate data and its extractor).

Modified:
- core: `break_room/{catalog,countries,daily,mod,state,tests}.rs`
- app: `app_appearance.rs`, `app_break_games.rs`, `break_room_controller.rs` (+ `tests.rs`), `break_room_view.rs`, `game_tokens.rs`, `main.rs`, `map/{dataset,mod}.rs`, `map_adapter.rs`, `persistence/break_room_port.rs`
- UI: `ui/break/{games,types}.slint`, `ui/main.slint`
- scripts: `stage20-perf-linux.sh`, `visual-parity/{capture-prod,gen-break-fixture,imgtool}.mjs`

## 47. Production integrity

`git diff -- desktop` is empty. Production was only read (copies in temp/scratch directories); the scratch `vite build` ran on a copy.

## 48. Real-data status

No real user data read or written: synthetic fixtures, isolated temp profiles (`STUDY_NATIVE_DATA_DIR`), throw-away Chromium profiles.

## 49. Git status

Uncommitted; not pushed. See §46 for the full list.

## 50. Stage 21 verdict

**PASS WITH CONCERNS — PENDING WINDOWS VERIFICATION.**

## 51. Windows verification plan

1. Build/tests in the VM (A).
2. Travle FN/Wabi captures vs headless Edge at 1520 × 980 and scale 1.25/1.5 (Georgia/Arial line boxes, ✕ glyph).
3. 0 frames idle, open/close and guess stress with `STUDY_NATIVE_TRAVLE_STRESS`, writes count.
4. Startup vs Stage 20 in the same VM (fresh, stored, old-day Travle).
5. UIA tree for the modal (one map node, labels).
6. Real input: typing, dropdown, Enter, zoom, close; tray/minimize while Travle is open.

## 52. Proposed Stage 22

Daily Skribbl and the social/network layer, as scoped earlier: the Worker API client (theme, upload, gallery, votes), the drawing canvas, identity/session, offline/error states and their security review — with no change to Travle.

## Bugs found and fixed

| Bug | Cause | Fix | Regression check |
|---|---|---|---|
| Hard-edged glow on game modals (Travle, Skribbl) | Slint's default radial radius is half the square's diagonal, so the fade never reached the square's edge | stop the gradient at 70.71 % | Travle captures (glow region diff ≈ 0) |
| Every Wabi game modal 5 px right, gutter dimmed | production's `scrollbar-gutter: stable` keeps fixed overlays out of the right 10 px; the native host spanned the window (and a narrower host without `x` is centred by Slint) | host width −10 px in Wabi, `x: 0` | Wabi Travle 4.96 → 1.60; Wabi Wordle 10.46 → 8.23 |
| Overflowing modal content clipped 18 px early | native scrolled inside the padding; CSS clips at the padding box | Flickable spans the inner box, content padded inside | won captures 2.18 → 1.92 |
| Text 1–3 px low in Travle | Slint text boxes ≠ Chromium `line-height: normal` | line boxes from `font-metrics` with Chromium's rounding | compact 4.16 → 2.32 |
| Linux perf harness always reported `frames=0` | `paste \| bc` with no `bc` installed | awk sum | static rows still 0; stress rows now non-zero |
| Daily-pair build 58 ms | 24k early-exit BFS + a route per pair | one BFS tree per start, routes only for kept pairs | goldens unchanged; 4 ms |
| Map lab parsed the world at startup | eager Stage 11 wiring | built on first show | map tests pass; startup path no longer parses geometry |

## Tests / checks (Linux)

Final run (Linux, 2026-10-03):

| Check | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo check --workspace` | ok; 56 warnings, all pre-existing (identical count at `7f1f533`: Windows-only platform/updater code unused on Linux) |
| `cargo test --workspace` | core **174 passed**, 1 ignored (new: `travle_timing_report`, a manual benchmark); app **240 passed**, 1 ignored (pre-existing); 0 failed |
| `cargo test -p study-tracker-core` | 174 passed, 1 ignored |
| `cargo build --release` | ok; 57,471,752 bytes (stripped 38,070,632) |
| `git diff --check` | clean |
| `git diff -- desktop` | empty |

Stage 20's recorded counts (158 core / 238 app) were taken on Windows; Stage 21 adds 16 core Travle tests (golden, invariant and rule tests) and, in the app crate, 4 controller tests (replacing the placeholder test), 1 production-normalization port test and 6 `travle_map` tests.
