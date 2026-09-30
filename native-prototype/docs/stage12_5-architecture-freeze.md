# Stage 12.5 — Windows-first architecture freeze and production migration plan

This is a **decision and planning** document. No source code changed to produce it (see "Verify no implementation creep" at the end). It freezes the architecture for the real native Study Tracker and lays out the incremental migration from `desktop/` (Tauri + React + WebView2). See ADR `docs/adr/0001-native-windows-architecture.md` for the condensed decision record; this document is the detailed policy it references.

## 1. Decision summary

- **Language**: Rust, for the shared core, application layer, and native platform adapters. Frozen; not reopened without a concrete blocker (§4).
- **UI**: Slint + Winit. Frozen (§6).
- **Renderer**: FemtoVG/OpenGL is the default on Windows, kept swappable behind the UI, not renderer-aware application logic (§7).
- **Platforms**: Windows first, Linux second, macOS third; shared logic by default, platform adapters where justified (§8, §9).
- **Core boundary**: `study-tracker-core` stays UI/platform-independent (§5).
- **Timer**: deadline/timestamp-based correctness and the "don't repaint unchanged state" performance rule are frozen invariants (§10).
- **Migration**: incremental, `desktop/` stays the released reference until cutover criteria are met (§22–§26, §32).
- **This stage**: documentation only. No Stage 13 work, no persistence migration, no platform adapters, no emoji/accessibility/icon fixes were implemented here.

## 2. Evidence summary

From Stages 0–12 (full detail in the referenced documents, not repeated here):

- **Stage 7 / native-core-architecture.md, timer-compatibility-spec.md**: a renderer-independent `study-tracker-core` crate with a deterministic, timestamp-based timer state machine matching production's behaviour contract, proven with unit tests, no sleeps, no platform dependencies.
- **Stage 9 (text/IME/accessibility, Linux)**: Latin/German/Japanese/mixed-script/math text render correctly; emoji with variation selectors (`⏱️`, `❤️`) render as tofu; Japanese IME blocked by environment; accessibility semantics present in the Slint hierarchy but not screen-reader-verified.
- **Stage 10 (dashboard/charts)**: realistic data-dense dashboard achievable in Slint with no chart library; a real renderer constraint found (Slint `Path` viewboxes scale uniformly, requiring a Rust-computed-geometry pattern for any chart/map); memory flat across interaction, 1,000/10,000-point series remain cheap to prepare.
- **Stage 11 (maps, Linux)**: a realistic 195-region/9.4k-vertex world map renders and hit-tests well; extreme synthetic stress data (Dense ×50, one 50k-vertex path) finds a real rendering ceiling and a memory high-water plateau after unloading it (not proven a leak).
- **Stage 12 (Windows, real hardware)**: confirmed renderer identity (FemtoVG/OpenGL via WGL, NVIDIA driver); found and fixed a real defect (unchanged Slint models replaced every 100 ms timer tick, causing 20 fps of unnecessary repaint — see §10); measured idle/timer/dashboard/map memory and CPU, a 56-minute long-running timer with no memory growth, warm startup around 206 ms (aligned probe), HiDPI at three scale factors, real mouse-wheel behaviour (no touchpad hardware available), and a UI-Automation accessibility pass (buttons/edit fields/progress bars exposed; map viewport not focusable; one unnamed progress bar; no live regions; Narrator not exercised by ear). Emoji tofu reproduces on Windows. Japanese IME remains blocked (not installed; per explicit instruction, not installed in Stage 12 either).
- **Stage 12 production comparison (same machine)**: production's actual release build (`desktop/`, v0.1.58, isolated WebView2 profile, real user data never touched) measured as an 8-process whole tree. Native uses about 43–54 MB Private Working Set and near-zero CPU across idle/timer-visible/timer-minimized/timer-paused; production uses about 143–156 MB Private Working Set and, while its timer is visible, minimized-from-the-Timer-tab, or paused, about 13–19% of one CPU core, traced in the read-only source to an infinite CSS `aura-pulse` animation on the timer face and a similarly infinite default-Dashboard garden animation, neither gated by `prefers-reduced-motion` in the timer's case, neither stopped by minimizing. Startup is about 4× slower for production (about 835 ms vs 206 ms, warm). Both apps' timers remain deadline-correct after minimize/restore, and both showed a memory plateau (not a leak) over the observed run lengths.

**What this evidence does and does not prove**: it establishes relative overhead on one fast desktop PC and identifies where each architecture's cost actually lives (WebView2 multi-process baseline vs. a specific animation choice). It does not measure a low-power Windows laptop, is not a Tauri-is-inherently-slow claim, and is not a claim that the native prototype is feature-equivalent to production today.

## 3. Final architecture

```text
Windows-first native Study Tracker

Slint UI (presentation only)
        |
native application / presentation layer   (Rust; Slint-facing adapter, view state, commands)
        |
shared Rust application / domain layer     (use-case orchestration, cross-cutting state)
        |
study-tracker-core                          (deterministic domain logic, DTOs, no I/O)
        |
persistence / network / platform adapters   (Rust; behind explicit service interfaces)
```

Dependency direction is one-way, top to bottom. `study-tracker-core` never depends on anything below the "domain outputs" line in the original diagram from `native-core-architecture.md`; that document's dependency-direction rule is unchanged and this freeze extends it to persistence, network, and platform-service boundaries explicitly (§13–§16).

## 4. Language decision (frozen)

Rust for the shared core, the application layer, and native host/platform adapters. Platform APIs are accessed through Rust crates/bindings (the `windows` crate family, already a transitive dependency via Slint's Windows backend, is available if a Windows adapter needs direct Win32/DirectWrite/UI Automation/TSF access). No C or C++ alternative was investigated in this or any prior stage; none is planned. This decision is not reopened without a concrete, measured blocker that no available Rust crate or binding can address.

## 5. Core boundary (frozen)

`study-tracker-core` must remain independent of: Slint, Winit, FemtoVG, Skia, Win32, Tauri, React, WebView, filesystem implementation details, network implementation details, notification APIs, tray APIs, and updater APIs. It contains deterministic domain logic and serializable DTOs where appropriate (following the existing `TimerState`/`TimerSnapshot`/`TimerEvent` pattern: runtime state, a separate serializable snapshot, and explicit output events with no side effects performed by the core itself). Effects cross explicit boundaries via adapters that live outside the core crate. UI types (Slint models, `ModelRc`, accessible-role enums, etc.) must never appear in core signatures; platform types (file handles, HTTP clients, notification builders) must never appear in core signatures either. Future domains (courses/tasks/semesters/statistics/social state) should follow the same command-in/event-out shape already proven by the timer domain.

## 6. Windows UI direction (frozen)

Slint + Winit + FemtoVG/OpenGL is the default Windows UI stack. This is a default, not an irreversible renderer lock (§7). Stage 12 evidence for keeping it as the default: single-process architecture, low ordinary idle/timer memory and CPU, fast startup, good realistic-map performance, workable text/CJK rendering, and a same-machine comparison that favors it over the current production WebView2 architecture on every everyday metric measured. Skia/D3D12 is not adopted merely because Direct3D sounds more Windows-native; see §7 for what would justify switching.

## 7. Renderer policy

**Default**: FemtoVG over OpenGL (current). **Keep the renderer swappable**: application/domain code must never become renderer-aware; renderer selection is a presentation-adapter/build concern only (the existing Slint `BackendSelector`/Cargo-feature mechanism already supports this without touching `study-tracker-core` or the application layer). No dynamic runtime renderer switching is implemented now.

Trade-offs found in Stage 12 (full tables in `stage12-windows-platform.md`):

| | FemtoVG/OpenGL (default) | Skia/D3D12 |
|---|---|---|
| Ordinary idle/timer memory | lower (~43 MB) | higher (~94 MB) |
| Startup | faster (~206 ms) | slower (~290–370 ms) |
| Realistic map (9.4k vertices) | excellent | comparable |
| Dense synthetic stress (Dense ×50) | weak (~26 fps) | much better (~130 fps) |
| Single huge vector path (Giant) | good (~76 fps) | poor (~5 fps) |

**Revisit conditions** (any one is sufficient to reopen the renderer question; a synthetic Dense ×50 result alone is explicitly not sufficient):

1. A real (shipped, not synthetic-stress) production feature cannot meet a measured responsiveness target on the default renderer.
2. A text or emoji rendering defect is traced specifically to the renderer/text-stack combination, not to font fallback data alone.
3. A GPU-driver compatibility or reliability problem appears (crashes, corrupt frames, driver-specific hangs) that is renderer-specific.
4. A platform (not one benchmark) genuinely requires a different renderer to function at all.

## 8. Platform strategy

Priority: **Windows first, Linux second, macOS third.** Three categories:

- **Shared** (default): domain logic, application logic, persistence format/schema where practical, network protocol, most Slint presentation, timer behaviour, statistics calculations. Used across platforms unless evidence requires otherwise.
- **Platform adapter**: OS-specific implementation behind a shared interface — notifications, tray, autostart, file locations, updater, power/sleep integration, accessibility enhancements, OS-specific window behaviour (§16 lists each with its current status).
- **Platform-specialized presentation**: allowed only where an OS materially benefits from a different UI/presentation implementation. None exist today and none are created speculatively; a real, measured need must appear first.

## 9. Windows-specific implementation policy (frozen rule)

> A Windows-specific implementation is justified only when at least one holds: (1) the shared implementation cannot provide the required behavior; (2) native Windows integration materially improves UX; (3) measurements demonstrate a meaningful performance problem; (4) accessibility requires it; (5) reliability or security requires it; (6) Windows lifecycle/power behavior requires it. **"Windows has an API for this" is not sufficient justification on its own.**

## 10. Timer architecture (frozen invariants)

The Stage 7/8 core design is preserved unchanged:

- Deadline/timestamp-based: live display derives from monotonic elapsed time; persistence/recovery uses wall-clock timestamps (`started_at`, `ends_at`, `last_alive_at`) because monotonic anchors cannot survive a process restart.
- Correctness does not depend on receiving every UI tick — an existing core test skips 11 seconds of ticks and still completes correctly.
- Paused gaps are excluded from active-segment accounting; pause closes the open segment, resume opens a new one.
- Commands and events are deterministic and explicit (`TimerCommand` in, `TimerEvent` out); the core performs no side effects.
- Session identity (IDs) stays outside the timer core.

Two performance/correctness rules, discovered and fixed in Stage 12, are now frozen and must not be silently reintroduced during migration:

> **Rule A**: A timer tick must not cause unrelated Slint models or expensive UI state to be replaced when their values have not changed. (Root cause found in Stage 12: `apply_model_to_window` installed a fresh `ModelRc` for the mode list and session notes on every 100 ms tick regardless of content, causing Slint to rebuild repeaters and repaint at 20 fps even while minimized. Fixed with a `model_matches` content-equality guard before any `set_*` model call; kept as a unit test, `model_matches_only_when_rows_are_identical`, in `native-prototype/src/main.rs`.)

> **Rule B**: Background/minimized timer correctness must not depend on rendering, and decorative animation must not be necessary for timer state progression. (Verified for the native prototype: zero rendered frames while minimized after Rule A's fix, with correct elapsed/remaining time on restore. Production's timer is also deadline-correct across minimize/restore, but its decorative aura animation is not gated by visibility — see §12 — which is the concrete counter-example this rule exists to prevent in the native migration.)

## 11. Everyday performance principles (behavioral budgets, not frozen numbers)

Do not promise the prototype's exact numbers for the final application; different feature load, persistence, and platform adapters will change them. These are principles a reviewer can check a migrated feature against:

- **Idle**: settles close to zero CPU; no continuous repainting without visible animation; stable memory across repeated idling.
- **Timer visible**: very low CPU; a bounded UI update rate (the current design ticks at 10 Hz internally but only repaints on an actual value change, about 1–2 repaints per second); stable memory over multi-hour sessions.
- **Timer background/minimized**: no unnecessary rendering; near-idle CPU; the timer remains correct; stable memory.
- **Static Dashboard**: settles near idle after interaction (no residual per-frame cost once the user stops interacting).
- **Optional animation** (garden, Wabi-Sabi, sakura, break-room effects): higher CPU/GPU is acceptable while genuinely visible; must be reducible/optional where a reduce-motion preference exists; must stop or strongly throttle when not visible or minimized (this is the rule production's current implementation violates for the timer aura — see §12).
- **Heavy features (maps, games)**: temporary additional CPU/GPU/RAM is acceptable while in use; resources should stabilize (a plateau, matching the Stage 11/12 retention-experiment finding), and repeated open/close cycles must not show unbounded growth.

## 12. Animation policy (Wabi-Sabi, sakura, decorative effects)

Production's Wabi-Sabi style, Japanese-garden dashboard variant, and sakura petal animation are **not removed and not migrated in this stage**. Policy for their eventual migration (Phase G, §23):

- Decorative animations are optional presentation effects layered on top of, never required by, domain state progression.
- They may use more CPU/GPU while genuinely visible on screen.
- They must not compromise timer correctness (Rule B, §10).
- They must stop or strongly throttle when minimized or not visible — this is the one concrete behavior change relative to the current production implementation that this freeze calls for, based on the measured 13–19% CPU cost of the timer's own aura animation continuing while minimized and even while paused.
- Animation state must not create unbounded allocation over repeated activation cycles.
- They should respect a future reduce-motion/animation setting (production has no such setting today for the timer aura; one should exist in the native version).
- Benchmark Wabi-Sabi/sakura/garden effects **separately** from baseline application performance; never fold their CPU into a "timer overhead" or "idle" figure, in either direction.

## 13. Persistence architecture

### 13.1 What production actually stores (inspected, read-only, from `desktop/src/types.ts`, `desktop/src/lib/storage.ts`)

Production state is one `AppState` object with these top-level areas: `semesters`, `courses`, `tasks`, `exams`, `calendarEntries` (planner), `sessions` (study session log) plus `lifetimeStudyMinutes`/`lifetimeStudySessions`, `exports` (vault daily-note export records), `settings` (accent, daily goal, telemetry opt-in, vault path, visible-tab flags, etc.), `social` (full `SocialState`: friends, friend requests, squad, squad messages, cached feed/leaderboard data, a device secret and friend code, a verified-session anchor), `timer` (`TimerState`, matching `timer-compatibility-spec.md`), `activeTab`, and a large set of engagement/progression fields: `unlockedGames`/`unlockedGamesDate`, `playedBreaks`/`playedBreaksDate`, `totalUnlocks`, `unlockStreak`, `lastUnlockDate`, `speedrunnerToday`, `playedGamesAllTime`, `badgeCounts`/`badgeCountDates`, `waterGlasses`/`waterDate`, `petRockPats`, and five daily-puzzle-game states (`durakPuzzle`, `wordlePuzzle`, `geodlePuzzle`, `flagglePuzzle`, `travlePuzzle`).

Storage mechanism: **browser `localStorage`** inside the WebView2 profile, under a set of versioned keys (`storage.ts`): a legacy `study-tracker-desktop-v1` key, a `study-tracker-desktop-v2` key, and a split-out `v3` generation (`-timer`, `-social`, `-core` keys). `APP_STATE_STORAGE_KEYS` lists every key the app has ever used, read on load. Loading runs the parsed JSON through per-field `normalize*`/`migrate*` functions (`normalizeAvatar`, `normalizeSquad`, `normalizeTimerSegments`, `migrateSemesters`, `normalizeCalendarEntries`, `normalizeVisibleTabs`, `normalizeActiveTab`, `normalizeLifetimeTotals`, per-game `normalize*Puzzle`, and more) rather than a single numbered schema migration — each field independently tolerates missing/old-shaped data and is repaired on load. There is no separate database file and no SQLite; this is exactly what a browser `localStorage`-based app looks like, and it works because WebView2's storage backing survives across app versions on the same machine.

Timer recovery specifically: covered already by `timer-compatibility-spec.md` and the `study-tracker-core` persistence module (`crates/study-tracker-core/src/timer/persistence.rs`), which already models the restore contract (`TimerSnapshot`, running/expired/stale/endless/abandoned recovery cases) independent of storage technology.

### 13.2 Native persistence boundary (design, not implemented)

- **Canonical domain DTOs** live in or alongside `study-tracker-core` (or sibling domain crates as they're added), one per bounded domain (timer already has `TimerSnapshot`; courses/tasks/sessions/social/progression get their own as those domains are built). DTOs are plain serializable Rust types — never Slint models, never platform storage types.
- **Persistence adapter** sits below the application layer, translates DTOs to/from durable storage, and is the only code that knows the storage technology. Application/domain code calls a narrow trait (`load`, `save`, maybe `migrate`) and never touches a file path, a connection string, or a serialization format directly.
- **Schema/version migration strategy**: continue production's proven approach — versioned keys/sections plus per-field tolerant normalization on load — rather than inventing a rigid numbered-migration system with no current justification. Each DTO module owns its own "accept an older/partial shape" logic, mirroring `storage.ts`'s per-field `normalize*` functions.
- **Backup before migration**: any one-time production-data import step (Phase C, §23) must write a timestamped backup of the source `localStorage` export (or the reachable WebView2 storage) before writing anything in the native format, and must never modify production's storage in place.
- **Validation after migration**: round-trip every migrated record back through the native domain types and compare field-for-field against the parsed production record before considering that record migrated; report (not silently skip) anything that fails to validate.
- **Rollback/recovery strategy**: the native app must be able to run against an empty/fresh store (as production does on first launch) so a failed migration is recoverable by falling back to "start fresh, keep the backup," never by silently discarding data.

**No data is migrated in Stage 12.5.** No real user data was or will be modified by this stage.

## 14. Persistence technology decision

**No database technology change is justified by current evidence, and none is adopted now.** Production's actual requirements — a single-user, single-machine, moderate-sized JSON-shaped state blob (semesters/courses/tasks/sessions/settings/social cache/progression/games), read in full on load and written on change/heartbeat — are exactly what simple serialized local storage already satisfies today, and nothing inspected in `desktop/` needs relational queries, concurrent multi-writer access, or partial/streamed reads. Introducing SQLite "because native apps often use it" would be exactly the kind of speculative architecture §33 (below) rules out. **This remains an explicit, revisitable migration decision** with its own criteria, not a default:

- Adopt a structured local database (e.g., SQLite via `rusqlite`/`sqlx`) only if a concrete need appears — for example, a dataset large enough that whole-blob load/save becomes measurably slow, a need for partial/incremental reads, or genuine relational queries across sessions/courses/tasks that a Rust domain layer cannot answer cheaply from an in-memory structure.
- Until then, a serialized file (JSON, or a Rust-native format like `serde`-driven binary) per the DTO boundary in §13.2 is sufficient and simplest.

## 15. Network/backend boundary

Production's network surface (`desktop/src/lib/social.ts`, proxied on Windows through a `native_social_sync` Tauri command that just relays to the Cloudflare Worker) covers: social sync (`syncSocialState`), feed (`getSocialFeed`, reactions/polls/comments/images), friends (`createFriendRequest`, `respondToFriendRequest`, `getFriendStatus`), squads (`createSquad`, `searchSquads`, `getSquadDetails`, squad messages, squad scoreboards), leaderboards, verified-session anchoring (`startVerifiedSession`/`heartbeatVerifiedSession`/`finishVerifiedSession`/offline-credit reconciliation), profile avatar upload, and an update-announcement check. All of it is HTTP JSON against a single configurable `SOCIAL_API_URL` (a Cloudflare Worker), gated behind `isSocialApiConfigured()`; the feature is fully optional and off when unconfigured.

Boundary for the native migration (not implemented now):

```text
domain/application layer
        |
service interface (e.g. a SocialService trait: sync, feed, friends, squad, leaderboard, verified-session methods)
        |
network adapter (HTTP client, JSON wire format, Cloudflare-specific request/response shapes, auth/device-secret handling)
```

`study-tracker-core` and the application layer must never see HTTP client types, raw JSON `Value`s, or Cloudflare-specific request shapes directly — only boundary DTOs the service interface returns. Networking is not migrated in this stage.

## 16. Platform services boundary

| Service | Production behavior (read-only inspection) | Native status |
|---|---|---|
| **Notifications** | `tauri-plugin-notification`; used for update-available and hide-to-tray messages | Platform adapter expected; shared trigger logic, Windows-native notification call. |
| **Tray** | `tauri::tray` with a phase-colored generated icon reflecting timer state (`tray_icon_for_phase`, `set_timer_tray_state`); closing the main window hides to tray instead of quitting while a verified session is active | Platform adapter expected; the icon-generation and title/tooltip logic can be shared, the OS tray call is platform-specific. |
| **Updater** | `tauri-plugin-updater`, GitHub Releases `latest.json` endpoint, passive install mode on Windows, user-initiated `downloadAndInstall`; a separate Linux-only manual-download code path exists in `lib.rs` | Platform adapter expected; Windows-first, needs its own investigation (Squirrel/MSIX/manual — not decided here). |
| **Autostart** | Not found in the inspected production source | Not present in production; do not add speculatively. |
| **Global shortcuts** | Not found in the inspected production source (only in-window keyboard shortcuts) | Not present in production; do not add speculatively. |
| **Power/sleep events** | Not explicitly handled in production source; correctness instead comes from wall-clock timestamps surviving a suspend | Shared design already sufficient (see §17); explicit OS sleep/resume event hooks remain a platform investigation only if timestamp-based recovery alone proves insufficient. |
| **Filesystem paths** | Production uses a user-chosen Obsidian vault path plus Tauri's app-data directory; Windows paths (`%LOCALAPPDATA%\com.damcha.studytracker`) confirmed to exist from Stage 12's isolation testing | Platform adapter expected (standard per-OS app-data/user-data directory resolution). |
| **Clipboard** | Standard WebView2 clipboard via the browser text-input stack; Slint's `TextInput`/`LineEdit` already expose cut/copy/paste (verified working on Windows in Stage 12) | Likely shared-sufficient; Slint's built-in behavior already covers this. |
| **File dialogs** | `tauri-plugin-dialog`, used for the Obsidian vault picker | Platform adapter expected; Windows-native (or Slint/`rfd`-crate) file dialog. |
| **OS integration (single instance)** | `tauri-plugin-single-instance` | Platform adapter expected. |

## 17. Sleep/hibernate correctness

**Not verified in Stage 12** and **not implemented or tested in Stage 12.5.** Added as an explicit migration acceptance requirement (tracked as a gate, §18, and scheduled at Phase B, §23):

- System sleep must not make countdown state drift: the architecture already stores `started_at`/`ends_at`/`last_alive_at` as wall-clock timestamps (§10), so a countdown's remaining time recomputes correctly from `ends_at` regardless of how long the process was suspended, exactly as the existing recovery-on-restart logic already does for a closed/reopened app.
- Resume must derive correct elapsed/remaining time from timestamps, not from having accumulated periodic ticks while asleep (Windows suspends timers/threads during sleep, so a tick-accumulation approach would silently lose time; the deadline-based design in §10 avoids this by construction, but it must be verified against a real sleep/resume cycle, not just against a closed/reopened process).
- Session active-segment ranges must remain correct across a sleep (an open segment that was "running" going into sleep should either close at the correct wall-clock instant or be recomputed consistently, mirroring the existing stale-open-segment recovery logic in `timer-compatibility-spec.md`).

This is a real Windows-lifecycle behavior (§9 condition 6) worth a dedicated manual verification pass once a real native window/timer exists in Phase B; it is not implemented now.

## 18. Known gates (explicit, not blockers to starting migration)

These remain open from Stage 12 and are **not resolved in Stage 12.5**:

- **Japanese IME**: blocked by environment (not installed); genuine composition (preedit, candidate selection, commit, backspace-during-composition) remains unverified on Windows. Gate: must pass before release; not a migration blocker (do not install the IME in this stage).
- **Emoji variation-selector tofu** (`⏱️`, `❤️` and likely others): reproduces on Windows exactly as on Linux; a presentation/text-stack issue, not solved here. Acceptance requirement: common Study Tracker emoji/assets must render predictably on Windows before release; possible fixes (font-fallback correction, text-stack change, replacing UI emoji with controlled vector/icon assets) are investigated at the appropriate later stage, not chosen now.
- **Accessibility**: UI-Automation plumbing exists and is largely correct (named, focusable buttons and edit fields; a progress bar with range values), but the map viewport is not keyboard-focusable through UIA, at least one progress bar has no name, there are no live regions, and Narrator speech has not been manually verified by ear. Not a compliance claim of any kind (WCAG or otherwise) until verified. Accessibility checks are placed throughout migration (§28), not deferred to the end.
- **Sleep/hibernate**: see §17; unverified, scheduled as a Phase B acceptance check.
- **Low-power/laptop hardware**: all Windows measurements are from one very fast desktop (24 cores, RTX 5070 Ti); this is a **pre-release validation gate** (§29), not a migration blocker, because the relative architecture ranking (native vs. WebView2 multi-process baseline) is unlikely to invert on weaker hardware, even though absolute numbers, thermal, and battery impact would differ and are not known.

## 19. Production feature inventory

Inspected read-only from `desktop/src/types.ts`, `desktop/src/App.tsx`, `desktop/src/lib/*`, `desktop/src-tauri/src/lib.rs`, `desktop/package.json`. Six top-level tabs exist: Dashboard, Planner, Timer, Vault, Break Room, Social (Wabi-Sabi style relabels these Today/Plan/Timer/Notes/Rest/Circle but they are the same six areas).

| Feature | Domain | Application | Persistence | Network | Presentation | Platform |
|---|:-:|:-:|:-:|:-:|:-:|:-:|
| Timer (modes, phases, active segments, recovery) | ✓ | ✓ | ✓ | | ✓ | |
| Semesters / Courses / Tasks / Exams (planner data model) | ✓ | ✓ | ✓ | | ✓ | |
| Calendar entries (planner scheduling) | ✓ | ✓ | ✓ | | ✓ | |
| Study sessions log + lifetime totals | ✓ | ✓ | ✓ | | ✓ | |
| Dashboard / statistics (weekly focus, history, course breakdown, streak) | ✓ | ✓ | | | ✓ | |
| Settings (accent, goal, telemetry opt-in, visible tabs, vault path) | | ✓ | ✓ | | ✓ | |
| Vault / Obsidian integration (create/link vault, notes, summaries, PDF import) | | ✓ | ✓ (files) | | ✓ | ✓ (filesystem, `tauri::command`s) |
| Knowledge Garden progression (western + Japanese variants) | ✓ | ✓ | ✓ | | ✓ | |
| Wabi-Sabi style + sakura petal animation | | | | | ✓ | |
| Themes (color palettes, Modern/Field Notebook/Wabi-Sabi styles) | | | ✓ (preference) | | ✓ | |
| Break Room games: Durak, Wordle, Geodle, Flaggle, Travle (daily puzzles) | ✓ (per-game logic) | ✓ | ✓ | | ✓ | |
| Travle's map (country-guessing route game; uses `travleMapData`/`countries`/`countryBorders`) | ✓ | ✓ | ✓ | | ✓ | |
| Achievements / progression micro-features (badges, unlock streak, water glasses, pet rock pats, speedrunner flag) | ✓ | ✓ | ✓ | | ✓ | |
| Social: friends, friend requests, squads, squad messages/scoreboard, feed (posts/reactions/polls/comments/images), leaderboards | ✓ (partial local calc) | ✓ | ✓ (cache) | ✓ | ✓ | |
| Verified-session anchoring / offline-credit reconciliation | ✓ | ✓ | ✓ | ✓ | | |
| Telemetry (opt-in install-id heartbeat) | | ✓ | ✓ | ✓ | ✓ (settings toggle) | |
| Tray icon reflecting timer phase, hide-to-tray on close during a verified session | | ✓ | | | | ✓ |
| Notifications (update available, hide-to-tray notice) | | ✓ | | | | ✓ |
| Updater (GitHub Releases, passive install on Windows, manual Linux path) | | ✓ | | ✓ | ✓ (settings UI) | ✓ |
| Single-instance enforcement | | | | | | ✓ |
| Device identity (fingerprint hash + label) | | | ✓ | | | ✓ |

**Important scoping note for the map (Phase I, §23)**: production's only real geographic feature is Travle's inline SVG map card — a static per-guess-colored country map with `+`/`−` CSS-zoom buttons and no drag-pan, no wheel-zoom, and no map-based hit-testing (country picking is a text field). The Stage 11 native map spike deliberately built a **superset** of this (full pan/zoom/hit-testing) to stress the renderer, not because production needs it. Phase I should target what Travle actually needs; the fuller interactive map remains proven-feasible infrastructure, not a requirement to migrate as-is.

## 20. Shared vs. platform-specific (explicit boundaries)

Restating §8 with the concrete inventory: everything in the Domain/Application/Persistence/Network/Presentation columns of §19 is **shared** by default. Only the Platform column items — vault filesystem access, tray, notifications, updater, single-instance, device identity — are platform adapters, and per §16 each has its own current status. No feature in the inventory currently needs a platform-specialized *presentation* (a different Slint UI per OS); none is created speculatively.

## 21. Performance regression suite (future, compact)

Turn the useful Stage 12 findings into a permanent but small regression suite once real migrated features exist, run manually or via the existing `native-prototype/scripts/*.ps1` tooling (kept, see §31). Do not preserve the prototype's exact numbers as hard pass/fail thresholds; use them as a reference point for "did this regress."

Scenarios: startup; idle; timer visible; timer minimized; timer paused; 30+ minute timer memory stability; Dashboard idle; optional animation (measured separately, per §12); one heavy feature (map or a game); repeated open/close cycles. Metrics recorded for each: Private Working Set, Private Bytes, CPU (interval, not instantaneous), process count, and startup time where relevant. A normal/low-power Windows laptop run remains a **pre-release validation gate** (§18, §29), not a per-PR requirement.

## 22. Migration strategy

Not a big-bang rewrite. `desktop/` remains the released, read-only-during-migration behavioral reference (§27) until each subsystem is migrated and accepted per the parity matrix (§30). For every migrated subsystem, follow this sequence: (1) inspect the actual production behavior from source; (2) write a compatibility specification (the timer's `timer-compatibility-spec.md` is the template); (3) implement the shared/domain behavior in Rust with deterministic tests; (4) implement the native adapter/UI; (5) write deterministic tests; (6) manually verify; (7) compare against production; (8) benchmark if performance-sensitive; (9) only then mark it migrated in the parity matrix.

## 23. Migration roadmap (phases)

Derived from the actual production inventory in §19, not assumed:

- **Phase A — Production shell foundation**: turn the prototype into a production-quality native application shell. GUI subsystem (no console window), an application icon, stable navigation across the real six areas, an error-reporting/logging strategy, platform paths, configuration, and the renderer-selection mechanism made a deliberate build-time choice rather than a prototype default.
- **Phase B — Timer productionization**: the timer core already exists and is frozen (§10). Build the real native timer UI end to end: production-mode semantics (Focus/Exam/Endless matching `timer-compatibility-spec.md`), persistence integration, recovery, session creation, and the sleep/resume verification from §17, plus background/minimized-behavior verification.
- **Phase C — Persistence and existing-data migration**: implement the storage adapter (§13–§14) and a safe one-time migration path for real production data, including backup, per-field validation, and rollback, exactly as §13.2 specifies. No data is touched until this phase, and even then only with explicit backup-first tooling.
- **Phase D — Sessions / courses / planner / core application data**: semesters, courses, tasks, exams, calendar entries, and the study-session log, using production's actual field names and shapes from `types.ts` (§19), not invented ones.
- **Phase E — Dashboard/statistics**: Stage 10 already proves the charting approach; replace its demo data with real domain/application data from Phase D.
- **Phase F — Platform integration (Windows first)**: tray (with the phase-reflecting icon behavior), notifications, updater, single-instance, device identity — per the §16 table, Windows first, Linux/macOS adapters only as needed later.
- **Phase G — Garden / progression / themes**, including Wabi-Sabi and the sakura animation, under the animation policy in §12.
- **Phase H — Break Room games and achievements**: Durak, Wordle, Geodle, Flaggle, Travle, plus the badge/streak/water-glass/pet-rock-pat micro-features, using the actual production inventory (§19), not a reduced guess.
- **Phase I — Travle's map**: scoped to what Travle actually needs (§19's scoping note), not the full Stage 11 interactive map spike, unless a later product decision explicitly wants the richer interaction.
- **Phase J — Parity audit and cutover**: feature-by-feature comparison against the parity matrix (§30) and the cutover criteria (§32); only then retire Tauri.

This is a starting template; adjust ordering within it if a later stage's own inspection finds production evidence that contradicts an assumption here.

## 24. Numbered future stages (after 12.5)

Each stage below is scoped to be reviewable on its own; none is started by this document.

### Stage 13 — Production shell foundation (Phase A)
- **Objective**: make the existing native prototype window/process look and behave like a real application shell, without adding new domain features.
- **Production reference**: `desktop/src-tauri/tauri.conf.json` (window title/icon/metadata), `desktop/src-tauri/icons/`.
- **Native files/modules**: `native-prototype/Cargo.toml` (GUI subsystem attribute, icon embedding), `native-prototype/src/main.rs`, a new `native-prototype/src/platform/` (or similar) module for paths/logging/config.
- **Scope**: no console window on release builds; an embedded application icon; a basic logging setup; resolving a per-OS app-data directory (without writing production-shaped data yet); making the renderer choice an explicit build/config decision instead of a hard-coded feature.
- **Non-goals**: no persistence, no platform services (tray/notifications/updater), no new UI screens, no data migration.
- **Tests**: unit tests for any new path-resolution/config-parsing logic.
- **Manual verification**: launch the release build on Windows; confirm no console window, a real icon in the taskbar/Alt+Tab, and unchanged Timer/Dashboard/Map/Text-spike behavior.
- **Performance checks**: re-run the existing Stage 12 baseline/startup scripts to confirm no regression.
- **Acceptance criteria**: no console window; a real icon; existing Stage 12 regression checks still pass; production untouched.
- **Commit expected**: yes, after review.

### Stage 14 — Timer productionization (Phase B)
- **Objective**: a real native Timer screen matching production's Focus/Exam/Endless semantics end to end, with sleep/resume verified.
- **Production reference**: `desktop/src/App.tsx` (Timer tab), `desktop/src/lib/timerTransitions.ts`, `timerDisplay.ts`, `timerPersistence.ts`, `desktop/src/hooks/useTimerTick.ts`, `timer-compatibility-spec.md`.
- **Native files/modules**: `crates/study-tracker-core/src/timer/*` (already exists; extend only if a genuine gap is found against the compatibility spec), `native-prototype/src/app_model.rs`, `native-prototype/src/main.rs`, `native-prototype/ui/` timer screen.
- **Scope**: production-mode semantics (course/task linking fields can be stubbed if Phase D hasn't landed yet, but the timer's own behavior must be complete); persistence integration (using the Phase C boundary once it exists, or a minimal file-backed stand-in if sequenced before Phase C — see §25 dependency note); recovery on restart; the sleep/resume manual verification from §17.
- **Non-goals**: no course/task/session domain beyond what the timer needs to reference; no dashboard integration; no network/social integration.
- **Tests**: core unit tests (already largely present) extended for any new gap; persistence round-trip tests once the storage adapter exists.
- **Manual verification**: start/pause/resume/reset by mouse and keyboard; minimize during a run; sleep the machine during a run and verify correct resume; verify Rule A/Rule B (§10) hold.
- **Performance checks**: repeat the Stage 12 long-run timer benchmark against the productionized timer.
- **Acceptance criteria**: matches the behavior contract in `timer-compatibility-spec.md`; sleep/resume verified; Rules A and B hold; no regression vs. Stage 12 numbers.
- **Commit expected**: yes.

### Stage 15 — Persistence adapter and safe production-data migration (Phase C)
- **Objective**: implement the storage adapter (§13.2) and a safe, backed-up, validated one-time import of real production data into the native format — without touching production's storage.
- **Production reference**: `desktop/src/lib/storage.ts` (full read shape and normalize/migrate functions), the real `%LOCALAPPDATA%\com.damcha.studytracker` WebView2 profile (read-only, backed up before any native import test).
- **Native files/modules**: a new persistence-adapter crate/module (location decided during the stage, following §13.2's trait boundary), DTOs alongside the domains that exist by then (timer's already exist).
- **Scope**: the storage trait and its first real backing (per §14, a serialized file unless this stage finds concrete evidence for something else); backup-then-import tooling; per-field validation reporting.
- **Non-goals**: no UI change; no new domain features; no deletion of any production data ever.
- **Tests**: round-trip tests per DTO; migration-validation tests against representative (synthetic, not real-user) production-shaped JSON fixtures.
- **Manual verification**: run the import tool against a backed-up copy of real data (never the live profile) and inspect the validation report.
- **Acceptance criteria**: every DTO round-trips; a representative real-data backup imports with a clean validation report or clearly reported discrepancies; production storage is provably untouched (same fingerprint-verification technique used in the Stage 12 production comparison).
- **Commit expected**: yes, tooling and adapter code; imported data itself is never committed.

### Stage 16 — Sessions / courses / planner data (Phase D)
- **Objective**: real semester/course/task/exam/calendar-entry domain and its native UI, backed by the Stage 15 persistence adapter.
- **Production reference**: `desktop/src/types.ts` (`Semester`, `Course`, `Task`, `Exam`, `CalendarEntry`), `desktop/src/App.tsx` Planner tab.
- **Native files/modules**: new domain module(s) alongside `study-tracker-core` or as sibling crates; new Planner Slint screen.
- **Non-goals**: no dashboard/statistics wiring yet (Phase E); no social/network features.
- **Tests**: domain unit tests; persistence round-trip via Stage 15's adapter.
- **Manual verification**: create/edit/complete tasks, schedule calendar entries, compare against production's Planner behavior.
- **Acceptance criteria**: field-for-field shape match with production's types; deterministic tests pass; manual comparison against production shows equivalent behavior.
- **Commit expected**: yes.

### Stage 17 — Dashboard/statistics with real data (Phase E)
- **Objective**: replace Stage 10's demo dataset with real data from Stage 16's domain.
- **Production reference**: `desktop/src/lib/metrics.ts`, `desktop/src/App.tsx` dashboard rendering.
- **Native files/modules**: `native-prototype/src/dashboard.rs` (already exists; rewire its data source).
- **Non-goals**: no new chart types beyond Stage 10's proven set unless production evidence demands one.
- **Tests**: statistics-calculation unit tests matching `metrics.ts` semantics (streak, weekly totals, course health).
- **Manual verification**: compare computed statistics against production for the same underlying session data.
- **Acceptance criteria**: statistics match production's calculations; existing Stage 10 performance characteristics hold with real (not mock) data volumes.
- **Commit expected**: yes.

### Stage 18 — Windows platform integration (Phase F)
- **Objective**: tray, notifications, updater, single-instance, and device identity on Windows, per the §16 adapter boundary.
- **Production reference**: `desktop/src-tauri/src/lib.rs` (tray icon generation, `set_timer_tray_state`, notification calls, updater plugin config, single-instance plugin, `get_device_identity`).
- **Native files/modules**: a new `native-prototype/src/platform/windows/` (or similar) module set, behind the shared service-interface pattern from §16.
- **Non-goals**: no Linux/macOS adapters yet unless trivially shared.
- **Tests**: unit tests for any shareable logic (e.g., icon-phase selection); platform calls themselves are manually verified.
- **Manual verification**: tray icon reflects timer phase; hide-to-tray on close during an active session; a real Windows notification appears; update check works against the real GitHub Releases endpoint (without auto-installing).
- **Acceptance criteria**: each platform service in the §16 table that was marked "adapter expected" is implemented and manually verified on Windows.
- **Commit expected**: yes.

### Stage 19 — Garden / progression / themes, including Wabi-Sabi (Phase G)
- **Objective**: migrate the Knowledge Garden progression (both variants), the theme system, and Wabi-Sabi including sakura, under the animation policy (§12).
- **Production reference**: `desktop/src/App.css` (garden/sakura/aura CSS), `desktop/src/App.tsx` garden rendering, theme/style selection UI.
- **Non-goals**: no change to timer behavior; the animation-policy visibility-throttling rule (§12) must be implemented here even though production's current implementation doesn't have it — this is a deliberate, documented improvement, not a defect being copied forward.
- **Tests**: unit tests for garden-state progression logic.
- **Manual verification**: visual comparison against production; verify animations throttle/stop when minimized.
- **Performance checks**: benchmark garden/sakura CPU separately from baseline (§12, §21).
- **Acceptance criteria**: visual/behavioral parity with production; animation-visibility rule holds; no baseline-CPU contamination.
- **Commit expected**: yes.

### Stage 20 — Break Room games and achievements (Phase H)
- **Objective**: migrate Durak, Wordle, Geodle, Flaggle, Travle, and the achievement/streak/water-glass/pet-rock-pat micro-features.
- **Production reference**: `desktop/src/lib/{durak,wordle,geodle,flaggle,travle}.ts`, `desktop/src/App.tsx` Break Room rendering, badge/streak logic in `App.tsx`/`storage.ts`.
- **Non-goals**: Travle's map is scoped to Stage 21, not built here (its game logic — puzzle generation, guess scoring — can be migrated here; the map card rendering is Stage 21).
- **Tests**: deterministic puzzle-generation/scoring unit tests per game, matching production's seeded daily-puzzle determinism.
- **Manual verification**: play each game; compare daily puzzle determinism against production for the same date/seed.
- **Acceptance criteria**: each game is deterministically equivalent to production for the same seed; achievement counters match.
- **Commit expected**: yes.

### Stage 21 — Travle's map (Phase I)
- **Objective**: the map card Travle actually needs (§19 scoping note), reusing Stage 11's proven rendering/hit-testing infrastructure only as far as production's real requirement goes.
- **Production reference**: `desktop/src/lib/travleMapData.ts`, `countries.ts`, `countryBorders.ts`, `desktop/src/App.tsx` Travle map card, `desktop/src/App.css` `.travle-map*`.
- **Native files/modules**: `native-prototype/src/map/*`, `native-prototype/src/map_adapter.rs` (already exist from Stage 11; scope down or reuse as appropriate).
- **Non-goals**: do not ship the fuller pan/zoom/hit-test interaction model merely because it exists, unless this stage's own product decision explicitly wants it.
- **Tests**: existing Stage 11 map unit tests remain the base; extend only for Travle-specific behavior (route rendering, guess-state coloring).
- **Manual verification**: visual/behavioral comparison against production's Travle map card.
- **Acceptance criteria**: Travle plays correctly with the native map card; no unused interaction surface shipped without a product reason.
- **Commit expected**: yes.

### Stage 22 — Social and network integration
- **Objective**: implement the network adapter boundary (§15) and migrate friends/squads/feed/leaderboards/verified-session anchoring.
- **Production reference**: `desktop/src/lib/social.ts`, `desktop/src-tauri/src/lib.rs` (`native_social_sync`).
- **Non-goals**: no changes to the Cloudflare Worker backend itself.
- **Tests**: service-interface unit tests with a fake network adapter; verified-session/offline-credit reconciliation logic tests.
- **Manual verification**: sync against the real backend from a test account; compare behavior against production.
- **Acceptance criteria**: functional parity for friends/squads/feed/leaderboards/verified sessions; core/application layers remain network-adapter-agnostic per §15.
- **Commit expected**: yes.

### Stage 23 — Parity audit and cutover (Phase J)
- **Objective**: full feature-by-feature comparison against the parity matrix (§30) and a go/no-go decision against the cutover criteria (§32).
- **Production reference**: the complete inventory in §19.
- **Non-goals**: no new features; this stage only audits and decides.
- **Tests**: the full test suite across all layers (§28).
- **Manual verification**: side-by-side comparison of every migrated feature against production, plus the pre-release low-power-hardware validation (§29).
- **Acceptance criteria**: every cutover criterion in §32 is met.
- **Commit expected**: yes, plus the actual cutover (retiring Tauri) as a separate, explicitly approved step — not silently bundled into this stage's commit.

Stages beyond 23 (packaging/installer work, Linux/macOS adapter parity, etc.) are intentionally not enumerated yet; define them once Stage 23's audit shows what's actually left.

## 25. Migration dependencies (critical ordering)

- Stage 15 (persistence adapter) should exist, at least in a minimal form, before Stage 14 needs real persistence integration; Stage 14 may proceed with a temporary minimal file-backed stand-in if sequencing pressure requires it, but that stand-in must be replaced by Stage 15's real adapter, not left in place.
- Stage 16 (sessions/courses/planner data) must exist before Stage 17 (dashboard with real data) — the dashboard has nothing real to show otherwise.
- Stage 16 should exist before Stage 19 (garden/progression), since garden growth is driven by session data.
- Stage 20 (achievements/games) depends on Stage 16's session/progression data existing for achievement counters to mean anything.
- Stage 18 (tray) depends on Stage 14's timer application state (the tray icon reflects timer phase).
- Stage 22 (social/network) can proceed in parallel with Stages 16–21 once Stage 13's shell exists, since it's largely independent of local domain data except for verified-session anchoring, which depends on Stage 14's timer.
- Do not sequence UI screens in an order that forces a fake temporary architecture everywhere merely to look further along; where real dependency ordering above conflicts with a desire to demo a later screen early, prefer correct ordering.

## 26. Testing strategy

- **Core unit tests**: deterministic domain behavior (already the pattern for the timer; extend to every new domain).
- **Application tests**: use-case orchestration (command-in/event-out, matching the core's own pattern one layer up).
- **Persistence tests**: round-trip, migration-validation, and recovery tests (§13.2, §15).
- **Adapter tests**: platform/network boundaries where practical (e.g., a fake network adapter for social-service tests; icon-phase-selection logic tested directly even though the OS tray call itself is manual-only).
- **UI/model tests**: presentation mapping (e.g., the existing `model_matches` test pattern — content equality before a Slint model replacement — should be the template for any future adapter that pushes data into Slint models).
- **Manual platform verification**: real Windows behavior for anything that cannot be meaningfully unit-tested (tray, notifications, IME, screen readers, sleep/resume, real file dialogs).
- **Performance regression checks**: the compact suite in §21, run manually against key everyday workloads, not turned into a CI gate requiring hardware this project doesn't have.

Avoid making pixel-perfect screenshot tests the primary UI correctness mechanism; screenshots remain useful for manual visual verification (as used throughout Stages 9–12) but are not the test suite.

## 27. Production reference policy (frozen rule)

> `desktop/` remains READ-ONLY behavioral reference during migration unless the user explicitly requests production maintenance. Native migration must not require production source modifications. Production is not deleted until parity/cutover (§32) is accepted.

## 28. Accessibility policy

Accessibility is placed throughout migration, not deferred to the end. At minimum, every migrated screen must define: semantic names/roles (following the existing `DemoButton`/`PresetCard`/progress-indicator pattern already proven in Stages 3–10); keyboard operability; a sensible focus order; state/value exposure (matching what UI Automation already surfaces for the timer's progress ring and buttons, per Stage 12); and, where relevant, a reduced-motion respect (§12) and textual alternatives for chart/map content (the Stage 10 dashboard's "always show the selected value as text, never hover-only" pattern is the template). Each phase in §23–§24 that introduces a new screen should include a Narrator verification pass for that screen before it's marked accepted in the parity matrix (§30) — not bundled into one final accessibility stage. No compliance claim (WCAG or otherwise) is made without direct verification.

## 29. Pre-release hardware validation

A normal or low-power Windows laptop (integrated GPU, battery-powered) run of the compact performance suite (§21) is a **pre-release validation gate**: required before public release, **not** a blocker for starting or continuing migration on the current desktop hardware. Rationale: the relative architecture ranking found in Stage 12 (native's multi-process-free, low-idle-CPU behavior vs. production's WebView2 multi-process baseline) is unlikely to invert on weaker hardware — if anything, WebView2's per-process baseline cost matters more, not less, on a constrained machine — while the *absolute* battery/thermal impact genuinely is unknown and must be measured before shipping.

## 30. Feature parity matrix

States: `NOT STARTED`, `DOMAIN COMPLETE`, `NATIVE UI COMPLETE`, `PERSISTENCE COMPLETE`, `PLATFORM COMPLETE`, `VERIFIED`, `ACCEPTED`. A feature reaches `ACCEPTED` only after a manual comparison against production (§22 step 7) and, where performance-sensitive, a benchmark (§22 step 8) — never merely because a visual mock exists.

| Feature (from §19) | Status |
|---|---|
| Timer | `DOMAIN COMPLETE` (core + Stage 12 UI fix); native UI/persistence/platform work is Stage 14 |
| Semesters/Courses/Tasks/Exams/Calendar | `PERSISTENCE COMPLETE` (Stage 16: native domain, cascade-delete, persistence, import; no interactive Planner UI yet) |
| Study sessions log / lifetime totals | `PERSISTENCE COMPLETE` (Stage 16: real Timer->StudySession bridge verified on real hardware, retention/dedup, session-notes UI card now real data) |
| Dashboard/statistics | `NATIVE UI COMPLETE` with demo data (Stage 10); real-data wiring is Stage 17 |
| Settings | `NOT STARTED` |
| Vault/Obsidian integration | `NOT STARTED` |
| Knowledge Garden progression | `NOT STARTED` |
| Wabi-Sabi/sakura | `NOT STARTED` |
| Themes | `NOT STARTED` |
| Break Room games (5) | `NOT STARTED` |
| Travle map | `NOT STARTED` (Stage 11's superset spike is proven-feasible infrastructure, not itself a migrated feature) |
| Achievements/progression micro-features | `NOT STARTED` |
| Social (friends/squads/feed/leaderboards) | `NOT STARTED` |
| Verified-session anchoring | `NOT STARTED` |
| Telemetry | `NOT STARTED` |
| Tray | `NOT STARTED` |
| Notifications | `NOT STARTED` |
| Updater | `NOT STARTED` |
| Single-instance | `NOT STARTED` |
| Device identity | `NOT STARTED` |

Update this table as each stage in §24 completes; do not mark anything `ACCEPTED` prematurely.

## 31. Stage 12 tooling retained

The Windows benchmark scripts added in Stage 12 are **kept as permanent regression tooling**, not deleted: `native-prototype/scripts/win-metrics.ps1`, `win-input.ps1`, `win-tree.ps1`, `benchmark-windows.ps1`, `map-stress-windows.ps1`, `memory-retention-windows.ps1`, `long-run-windows.ps1`, `renderer-everyday-windows.ps1`, `ab-everyday-windows.ps1`, `ab-longrun-windows.ps1`, `ab-startup-windows.ps1`. These back the performance regression suite in §21. The Stage 12 timer-fix rationale (§10, Rule A) is recorded here specifically so a later migration stage does not accidentally reintroduce per-tick model replacement while wiring in real data (Stage 17 in particular, since it touches the same `apply_model_to_window`-style code path pattern).

## 32. Cutover criteria

Tauri may be retired only when **all** of the following hold:

1. Every production feature in the §19 inventory that is still part of the product is `ACCEPTED` in the parity matrix (§30) — optional/experimental features not actually in current production are not required.
2. Existing user data migrates safely per §13.2's backup/validate/rollback process, verified against real (backed-up) production data.
3. Timer recovery, including sleep/resume (§17), is validated.
4. Windows packaging (installer/updater) works end to end.
5. Tray/notifications/updater requirements from §16/Stage 18 are resolved for Windows.
6. An accessibility acceptance pass (§28) is complete for every migrated screen.
7. The Japanese IME acceptance gate (§18) passes.
8. The ordinary-workload performance regression suite (§21) passes with no unexplained regression vs. the Stage 12 baseline.
9. The low-power Windows hardware validation (§29) is complete.
10. No critical data-loss bug exists.
11. A rollback/backup strategy exists and has been exercised at least once.

## 33. Rules future agents must not casually reopen

- Do not reopen the Rust-vs-other-language decision (§4) without a concrete, measured blocker.
- Do not reopen the Slint-vs-other-UI-framework decision (§6) without a concrete, measured blocker.
- Do not switch the default renderer away from FemtoVG (§7) without one of the four listed revisit conditions; a synthetic stress benchmark alone is not sufficient.
- Do not let UI or platform types leak into `study-tracker-core` (§5) "just this once" for convenience.
- Do not reintroduce per-tick unconditional Slint model replacement (§10, Rule A) — check new adapter code against the `model_matches` pattern.
- Do not make decorative animation required for timer or domain-state progression (§10, Rule B; §12).
- Do not introduce a database, an ECS, an actor system, microservices, a custom rendering engine, a plugin system, a generic DI framework, or async-everywhere without a concrete current need (§14, and this section generally) — the simplest architecture justified by Study Tracker's actual requirements wins.
- Do not perform a big-bang rewrite or delete `desktop/` before cutover criteria (§32) are met.
- Do not treat a low-power-hardware validation gap as a reason to halt migration (§29) — it's a pre-release gate, not a blocker.
- Do not mark a parity-matrix feature `ACCEPTED` (§30) without a manual comparison against production.

## 34. Verify no implementation creep

This stage changed **documentation only**. No Rust or Slint source was modified to produce this freeze; the timer-fix rationale it records (§10) refers to work already committed in Stage 12 (`5538032`), not new work done here. If a future reviewer believes source code must change to "support" this architecture freeze, that change belongs to Stage 13 or later, scoped and reviewed on its own — not folded into this document's commit.
