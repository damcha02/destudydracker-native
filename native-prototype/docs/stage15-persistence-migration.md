# Stage 15 — Native persistence and safe production-data migration

**Production reference commit**: `b095706994b6caaf18432d0d41b4534f4b85be98` (upstream `destudydracker`, `main`, 2026-09-27, v0.1.66 - unchanged since Stage 14's re-audit; re-confirmed no Timer/storage-relevant commits landed upstream since).

## 1. Scope

Two related objectives, both delivered: (A) a real, durable native persistence architecture behind the `TimerPersistencePort` boundary Stage 14 established, replacing `NullPersistencePort` in normal runtime; (B) a safe, explicit, read-only-on-the-source production-backup import pipeline, exercised end-to-end against a sanitized fixture, but never against real user data automatically. Stage 16 (sessions/courses/planner) is not started; this stage only builds the storage/migration infrastructure Stage 16 will use.

## 2. Production v0.1.66 persistence inventory

Read fresh from `desktop/src/lib/storage.ts`, `desktop/src/types.ts`, `desktop/src/App.tsx`, `desktop/src-tauri/src/lib.rs` (no changes vs. Stage 14's own re-audit - confirmed by re-reading, not assumed).

| Top-level `AppState` key | Owning feature | Local-only? | Network-derived? | Sensitive? | Needed now? | Native DTO | This stage's handling |
|---|---|---|---|---|---|---|---|
| `timer` | Timer | yes | no | no | yes | `TimerSnapshot` (existing, Stage 7) | **Consumed** - converted and persisted |
| `sessions`, `lifetimeStudyMinutes`, `lifetimeStudySessions` | Session log | yes | no | no | not yet | none yet | Reserved - preserved opaquely |
| `semesters`, `courses`, `tasks`, `exams`, `calendarEntries` | Planner | yes | no | no | not yet | none yet | Reserved |
| `settings` | Preferences | yes | no | no (contains no secrets itself) | not yet | none yet | Reserved |
| `activeTab`, `holidays`, `dailyTodos`, `timetableEvents`, `exports` | Misc UI/derived | yes | no | no | not yet | none yet | Reserved |
| `wordlePuzzle`, `geodlePuzzle`, `flagglePuzzle`, `travlePuzzle`, `durakPuzzle`, `unlockedGames*`, `playedBreaks*`, `badgeCounts*`, `waterGlasses*`, `petRockPats`, `totalUnlocks`, `unlockStreak`, `lastUnlockDate`, `speedrunnerToday`, `playedGamesAllTime` | Break Room / achievements | yes | no | no | not yet | none yet | Reserved |
| `social` | Friends/squads/feed/leaderboards/verified-session anchor | **no** | **yes** (cached feed/leaderboard) | **yes** (`deviceSecret`, friend code, verified-session anchor) | not yet | none | **Withheld** - recognized, reported, never written to the native store, even opaquely |

Unknowns explicitly recorded: whether any `settings` sub-field is itself sensitive (e.g. a vault path revealing local filesystem structure) was not individually audited field-by-field this stage - `settings` as a whole is classified `Reserved`, not `Withheld`, on the basis that it contains no credentials/tokens/identity by construction (confirmed from `types.ts`'s `Settings` shape: accent, goal, telemetry opt-in, vault path, visible-tab flags - a local path, not a secret), but a future stage that actually consumes `settings` should re-audit `vaultPath` specifically before ever transmitting it anywhere.

Derived (not stored, not migrated): dashboard/statistics (computed from `sessions` at render time, per `desktop/src/lib/metrics.ts`). Ephemeral (not stored): in-flight UI state (`activeTab`'s *display* value is stored, but transient things like open-modal state are not).

## 3. Production backup mechanism

`desktop/src/lib/storage.ts`'s `buildBackup`/`saveBackup`/`restoreBackup` (re-inspected fresh this stage): a JSON file, `{app: "study-tracker", backupVersion: 2, exportedAt: <ISO>, state: <AppState>, preferences: <selected localStorage keys>}`, written via a Tauri `save` file-picker dialog plus the already-shipped `write_backup_file` Tauri command (`desktop/src-tauri/src/lib.rs:692`, a plain `fs::write(path, contents)`). `restoreBackup` validates a candidate file by checking `Array.isArray(state.sessions)`, `Array.isArray(state.courses)`, and a `social` object with non-empty `userId`/`deviceSecret` strings - this pipeline (`migration::discover_and_read`) mirrors that exact same gate, so "is this a real Study Tracker backup" uses production's own definition, not an invented one.

**Why this is the safe migration source** (per the brief's explicit preference): it is a supported, already-shipped, user-invoked export path production itself promises to produce and re-read - not WebView2/`localStorage`/LevelDB internals. This pipeline never touches WebView2 storage directly and never will unless a future stage finds a concrete reason the backup format itself is insufficient (none was found).

## 4. Native persistence architecture

```text
Slint UI -> AppModel/AppTimer -> TimerController -> TimerPersistencePort (trait, Stage 14)
                                                          |
                                                          v
                                          FileTimerPersistencePort (Stage 15, new)
                                                          |
                                                          v
                                                    NativeStore (Stage 15, new)
                                                          |
                                                          v
                                        one JSON file under AppPaths::data_dir
```

`study-tracker-core` performs no filesystem I/O and does not depend on `serde_json`, `dirs`, or anything else this stage added - verified by `crates/study-tracker-core/Cargo.toml` being unchanged (still only `serde`). `TimerPersistencePort` (the trait) was already frozen in Stage 14; nothing about its shape changed. New files: `src/persistence/store.rs` (the durable store), `src/persistence/timer_port.rs` (the real port implementation), `src/persistence/migration.rs` (the import pipeline), `src/persistence/mod.rs`.

## 5. Native storage format and schema

One JSON file, `{"schema_version": 1, "timer": {...TimerSnapshot fields...}, ...other top-level keys verbatim...}`. `schema_version` is checked on every load: missing -> `StoreError::MissingSchemaVersion`; higher than `CURRENT_SCHEMA_VERSION` -> `StoreError::UnsupportedFutureSchema`, refused rather than silently rewritten. Bumping `schema_version` is reserved for when an *existing* section's shape changes incompatibly; adding a brand-new section for a future domain (Stage 16+) does not require a bump, since every top-level key this build doesn't itself model round-trips untouched through an internal `other: serde_json::Map<String, Value>` field. Application version (`Cargo.toml`'s `0.1.0`), storage schema version (`1`), and production source version (`0.1.66`, tracked separately in `docs/production-reference.md`) are three independent numbers, never conflated.

## 6. DTO boundaries

Only `TimerSnapshot` (Stage 7/14, unchanged) crosses this boundary today. `study-tracker-core`'s own `Serialize`/`Deserialize` derives are what `serde_json` reads/writes directly - no separate "storage DTO" duplicate type was introduced, since `TimerSnapshot` was already designed exactly for this (a serializable DTO distinct from the runtime `TimerState`, per `native-core-architecture.md`).

## 7. File locations

`platform::paths::AppPaths::data_dir.join("store.json")` - i.e. the same per-user, namespaced (`com.damcha.studytracker.native-shell`, distinct from production's `com.damcha.studytracker`) directory Stage 13 already resolves and creates. Never beside the executable, never in the repository, never a literal username-bearing path in source. A new `STUDY_NATIVE_DATA_DIR` environment-variable override was added to `AppPaths::resolve()` purely as test/benchmark infrastructure (isolated temp directories for automated and manual verification - see section 19); it has no effect unless explicitly set, and every test and manual check in this stage used it, never the real per-OS location, except for one accidental real-machine launch caught and reported in section 25.

## 8. Atomic writes and backups

**Atomic replacement**: serialize the whole envelope, write it to a sibling `store.json.tmp`, then `std::fs::rename(tmp, target)`. **Verified empirically on this Windows machine** (not assumed from POSIX semantics, per the brief's explicit instruction) that `std::fs::rename` *does* replace an existing destination file on Windows - a small standalone Rust program confirmed a `rename` onto an existing `target.txt` succeeds and the target ends up holding the new contents (Rust's standard library calls `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` internally, unlike a bare Win32 `MoveFileW`). A process killed between the write and the rename leaves either the untouched old file or the fully-written new one - never a half-written target - since only the throwaway `.tmp` file can ever be partially written; a unit test (`a_leftover_tmp_file_from_an_interrupted_write_does_not_affect_loading_the_real_file`) locks this in.

**Backup policy**: the migration pipeline's `discover_and_read` always writes a timestamped, byte-identical copy of a source production backup to `<data_dir>/imported-backups/<name>-<unix_millis>.json` *before* anything else touches it - this is the "backup before migration" step, distinct from an ordinary native persistence write (which has no separate backup/retention policy of its own, since production's own backup mechanism is what's being read from, not duplicated). Retention: import-copy files accumulate (one per import attempt, including dry runs); no automatic pruning exists yet - acceptable at Stage 15's scale (a manual/diagnostic action, not a background process), flagged as a future cleanup in section 30 rather than solved speculatively.

## 9. `TimerPersistencePort` implementation

`FileTimerPersistencePort` (`src/persistence/timer_port.rs`) wraps a `NativeStore`. `persist`: read-modify-write (load the existing envelope, replace only the `timer` field, save) so a save never clobbers a section this build doesn't itself touch; a pre-existing corrupt file is treated as unreadable-and-replaced (logged, not propagated - `TimerPersistencePort::persist` has no error return, matching `NullPersistencePort`'s signature). `load`: returns `envelope.timer`, logging any non-fatal load warnings. `main.rs` now wires `FileTimerPersistencePort` as the real runtime default, replacing `NullPersistencePort` (which remains in `timer_controller.rs`'s test-only and `AppModel::stage_four_timer_preview` demo path - see section 24, "Files changed").

## 10. Write cadence

Unchanged from Stage 14's design, now proven against a real file: `TimerController::apply` calls `persistence.persist(...)` exactly when the core emits `TimerEvent::PersistenceRequested` - on Start/Pause/Resume/Reset/Completion, never on a no-op `ObserveTime`. **Measured on real hardware** (not just asserted by a unit test): a Focus timer left running and visible for 8 real seconds produced exactly one write, at Start, with the store file's modification time unchanged for the following 8 seconds; the same held while genuinely minimized (`IsIconic` confirmed `True`) for 12 seconds. No heartbeat-based persistence was added - the deadline/timestamp design means a running timer's remaining time is always correctly recomputable from `ends_at` regardless of how long ago the last write happened, so there is nothing a periodic heartbeat write would add beyond disk churn.

## 11. Crash/unclean-exit model

Tested with real `Stop-Process -Force` kills against an isolated `STUDY_NATIVE_DATA_DIR` (never the real production Study Tracker, never this project's own real dev profile except once, accidentally, and harmlessly - see section 25), not merely simulated with fixtures:

- A Focus ("Deep Work") timer started, left running, and killed 3 seconds later left a `store.json` with `running: true`, the correct `ends_at`, and the correct `preset_label`.
- Relaunching (a real second process, same isolated data directory, no mode/autostart override) correctly restored `status=Running`, the correct preset tile (matched by `preset_label`), and a `remaining_seconds` reduced by almost exactly the real wall-clock time that passed while the process was dead - proving the deadline-based design survives a genuine process restart, not just a unit-test-constructed `RestoreInput`.

`last_alive_at` and the existing `restore_timer` stale-segment logic are unchanged from Stage 7/14; this stage only proves them against a real killed-and-restarted process instead of an injected snapshot.

## 12. Timer restart behavior (real, measured results)

| Scenario | Real test | Result |
|---|---|---|
| Running Focus, unclean kill, restart | Deep Work autostarted, killed after 3 s, restarted with no override | Restored running, correct preset, `remaining_seconds` reduced by the real elapsed wall time |
| Paused, unclean kill, restart 5 s later | Exam autostarted+autopaused, killed, waited 5 real seconds, restarted | Restored paused, `remaining_seconds` identical both times (7200 s) - the paused gap was excluded across a real restart |
| Expired while away, restart | Demo (10 s duration) autostarted, killed after 0.8 s, **12 real seconds** allowed to pass (past the 10 s deadline), then restarted | Restored to `Ready`/`Idle`; log shows `timer recovery on startup produced 2 application effect(s): [SessionRangeReady { reason: AbandonedRecovery, ... }, Completed { ... }]` - exactly one recovered session, matching the core's design |
| Duplicate-recovery check, third launch | Same store, no new `Start`, launched again | No recovery log line at all - the earlier `force_persist` had already written the reset Idle state back, so the same abandoned session was **not** recovered a second time |

All four were real, separate Windows processes against isolated temp data directories - not simulated.

## 13. Production import pipeline

`DISCOVER + READ + BACKUP COPY` (`discover_and_read`) -> `CLASSIFY` (`classify_fields`) -> `CONVERT + VALIDATE` (`convert_timer`) -> `COMMIT` (`commit_import`, itself: write -> read back -> verify -> rollback on any mismatch). Exposed as a diagnostic startup hook, `STUDY_NATIVE_IMPORT_BACKUP=<path>` (+ optional `STUDY_NATIVE_IMPORT_DRY_RUN=1` to inspect without committing), matching the brief's explicit allowance ("a developer/diagnostic import command or isolated test harness is acceptable for Stage 15"). Never runs unless that variable is explicitly set; never touches the source file (verified - see section 19); commits only into this app's own isolated native store, never production's storage.

## 14. Data preservation (unported fields)

Every top-level key production's backup actually contains is classified (section 2's table) and reported; nothing is silently dropped from the *report*. Only `timer` is currently written into the native store (`Consumed`). `Reserved` fields (`sessions`, `courses`, `semesters`, `tasks`, `exams`, `calendarEntries`, `settings`, `activeTab`, and the rest of section 2's table) are **not yet written into the native store by `commit_import`** - a deliberate scope decision, not an oversight: writing large, real, unmodeled arrays into the store now would mean Stage 16 inherits a shape Stage 15 invented rather than one it designs on purpose, which is exactly what the brief's "do not pretend to migrate features we have not ported" (section 19) warns against. The mechanism to preserve them opaquely already exists and is tested (`preserve_reserved_sections`/`envelope_with_reserved_sections`, both `#[allow(dead_code)]`-marked since no caller wires them in yet) - Stage 16 can call `preserve_reserved_sections` itself once it knows what shape it actually wants, without this stage having guessed.

## 15. Validation

Field-aware, mirroring production's own tolerant-per-field philosophy: a numeric field that cannot represent a valid non-negative duration (a negative `remainingSeconds`, a non-finite `studyMinutes`) is a reported conversion error, not a silent clamp or a panic; an unrecognized `phase`/`mode` string is a reported error; a malformed ISO timestamp is a reported error. All of these fail the *whole* `timer` conversion (not partially import a corrupt timer), matching section 20's "do not silently accept structurally dangerous corruption" - a corrupt timer section blocks that section's import entirely rather than guessing. Structural validation at the backup-envelope level exactly mirrors production's own `restoreBackup` gate (`sessions`/`courses` arrays, a `social` object) - a file that fails this is rejected outright as "not a Study Tracker backup," before any field-level classification runs at all.

## 16. Idempotence

Both persistence layers were tested for repeat-safety: (a) native store saves fully replace the envelope (a second `save(StoreEnvelope::default())` leaves no trace of a first save's `other` sections - `a_second_save_fully_replaces_the_first_not_merges_stale_fields`); (b) importing the identical backup twice produces byte-identical native store contents both times (`importing_twice_is_idempotent_not_duplicating_anything`); (c) the real-restart duplicate-recovery test in section 12/section 19 proves the same at the process level, not just at the function level.

## 17. Rollback

`commit_import` writes, reads back, and compares field-for-field against what was written; any mismatch (or a write/read failure) restores the pre-import envelope exactly (`a_failed_commit_leaves_the_native_destination_completely_unchanged`; the complementary "never call commit" path is also tested). Because the migration source is always read-only and the native destination write is itself atomic (section 8), a rollback is simply "write back what was there before," never a partial-state repair.

## 18. Secrets/device/social handling

`social` (device secret, friend code, verified-session anchor, cached feed/leaderboard data) is classified `Withheld`: recognized and reported (the report always names it), but never written into the native store, not even opaquely into `other` - verified directly against the real import test (`grep`-checked the resulting `store.json` for `"deviceSecret"`/`"social"` and found neither), and by a dedicated unit test (`committing_never_writes_a_withheld_social_section_into_the_native_store`). No credential is migrated, and none is silently regenerated - device/social identity remains entirely Stage 22's concern (per the architecture freeze's own phase ordering), not touched here.

## 19. Fixtures

`native-prototype/tests/fixtures/sanitized-production-backup.json`: a hand-written, entirely synthetic backup in production's real `{app, backupVersion, state, preferences}` shape, with an obviously-fake `userId`/`deviceSecret`/`friendCode` and one placeholder course/session - **no personal data of any kind**. Used for one real, end-to-end manual run of the import diagnostic (`STUDY_NATIVE_IMPORT_BACKUP`), which confirmed: the fixture's SHA-256 hash was identical before and after the run (the source was never mutated); a timestamped verbatim copy was written to `imported-backups/`; all 12 top-level keys were classified correctly (10 Reserved, 1 Withheld = `social`, 1 Consumed = `timer`); the resulting native `store.json` contained the converted timer and, confirmed by direct inspection, no trace of `social`/`deviceSecret`. Every other fixture (corrupted/edge-case JSON) used in automated tests is constructed inline in the test source itself (`write_fixture` helpers in `migration.rs`'s and `store.rs`'s own test modules) rather than as separate checked-in files, since each is a one- or two-line JSON string specific to one test.

## 20. Corruption testing

All of the following are automated, deterministic, and confirmed not to panic (native store: `src/persistence/store.rs`; production import: `src/persistence/migration.rs`):

| Case | Store (`NativeStore::load`) | Import (`discover_and_read`/`convert_timer`) |
|---|---|---|
| Empty file | `Corrupt` | `NotJson` |
| Truncated JSON | `Corrupt` | `NotJson` |
| Invalid JSON | `Corrupt` | `NotJson` |
| JSON array as root | `Corrupt` | n/a (backup envelope must be an object) |
| Missing schema/version marker | `MissingSchemaVersion` | n/a (backups have no schema_version; gated by section/array checks instead) |
| Unsupported future schema | `UnsupportedFutureSchema(999)` | n/a |
| Missing `timer` section | `None`, no warning | `Ok(None)`, not an error |
| Malformed timer enum (`"levitating"` phase) | dropped with a warning, rest of file intact | reported conversion error |
| Negative duration | dropped with a warning (can't fit `u64`) | reported conversion error |
| Malformed timestamp | n/a (native format stores unix millis, not strings) | reported conversion error |
| Missing `sessions`/`courses`/`social` | n/a | `NotAStudyTrackerBackup` |
| Unexpected additional fields | round-trips via `other` | classified `Reserved`, conversion of `timer` unaffected |
| Interrupted/temp-file scenario | a leftover `.tmp` file never affects loading the real target | n/a (the copy step doesn't use a temp file - the source is read once, copied once, never re-read) |
| Very large but reasonable backup | not specifically stress-tested this stage (production's real data is small - a single JSON blob per section.13.1 of the freeze); no code path here scales worse than linearly in file size | same |

## 21. Performance

Measured with the existing `scripts/win-metrics.ps1` tooling (Stage 12's own methodology), real Windows process trees, isolated `STUDY_NATIVE_DATA_DIR`:

| Point | Private WS | CPU | Notes |
|---|---|---|---|
| P15-0 idle (timer never started) | 44.2 MB | 0.19% | `store.json` never created - Idle never emits `PersistenceRequested` |
| P15-1 Focus running, visible, 8 s | 48.4 MB | 0.19% | exactly one write (at Start); file mtime unchanged for the following 8 s |
| P15-2 Focus running, minimized, 12 s | 44.2 MB | 0.13% | `IsIconic` confirmed `True`; file mtime unchanged throughout - no extra churn from minimizing |
| P15-3 transition write | n/a (see P15-1) | n/a | confirmed a real write happens on Start and nothing between transitions |
| P15-4 restart with real recovery | n/a | n/a | first-frame time 153 ms (`STUDY_NATIVE_STARTUP_REPORT`), in the same class as Stage 12's ~206 ms warm baseline - loading and restoring a real snapshot adds no measurable startup cost |

No continuous idle disk churn at any point measured. All figures are in the same class as Stage 12-14's accepted native baseline; no regression from adding real persistence.

## 22. Memory

The persisted state is a few hundred bytes to a few kilobytes (one `TimerSnapshot`, plus whatever a future import's `Reserved` sections eventually add); `NativeStore::load`/`save` parse and discard the JSON `Value` each call rather than retaining it, and `FileTimerPersistencePort` holds no in-process cache of the store's contents at all (every `persist`/`load` does a fresh read - see section 10 for why this is cheap enough given the write cadence). No duplication beyond one transient `serde_json::Value` per load/save call was observed or is architecturally possible from this code's shape.

## 23. Windows-specific details

The one Windows-specific fact this stage depended on and verified rather than assumed: `std::fs::rename`'s replace-existing-destination semantics (section 8). Everything else (`AppPaths`, the store, the port, the migration pipeline) is plain, portable Rust with no `cfg(windows)` code - `persistence/` has zero platform-gated code, unlike `platform::startup_error`.

## 24. Cross-platform implications

Nothing in `persistence/` assumes Windows path syntax or Windows-only filesystem behavior; `AppPaths::resolve()` already resolves the correct per-OS directory via `dirs`, and `fs::rename`'s atomic-replace behavior is standard POSIX `rename(2)` semantics on Linux/macOS too (this stage's own verification was Windows-specific only because this stage's own testing hardware is Windows - no Linux/macOS runtime claim is made, per the frozen rule against claiming untested-platform verification).

## 25. Real production data status

**No real user data was read, imported, moved, deleted, or modified by this stage.** Every test and manual verification used either an in-memory-constructed fixture, a hand-written synthetic backup file (section 19, no personal data), or a real Windows process pointed at an isolated `STUDY_NATIVE_DATA_DIR` temp directory - never the real per-OS `%LOCALAPPDATA%\com.damcha.studytracker.native-shell` location, and never production's own `%LOCALAPPDATA%\com.damcha.studytracker` at all. **One accidental exception, caught and reported honestly rather than omitted**: a single manual-testing PowerShell call had its environment variables reset between tool calls (an artifact of how the shell tool works, not of the persistence code), causing one launch-and-immediately-exit cycle to run against this project's own real native-shell dev data directory instead of the intended isolated temp directory. Verified immediately afterward: no `store.json` was created there (the timer was never started, so nothing was ever persisted), and the directory's pre-existing `logs`/`cache` subdirectories (already present from Stages 12-14's own testing) were the only things touched, receiving one more harmless log-startup line. Production's own real data directory was never involved. `desktop/`'s `git diff` is empty (see section 27).

## 26. Dependencies

Two new **direct** dependencies on the bin crate (`native-prototype/Cargo.toml`), both already present *transitively* beforehand (confirmed via `cargo tree`, `Cargo.lock` diff is 2 lines):

- `serde_json = "1"` - the JSON reader/writer for the native store format and the production-backup parser. Not a database, not an ORM, not an async runtime.
- `serde = { version = "1", features = ["derive"] }` - already a direct dependency of `study-tracker-core`; added directly to the bin crate too only to use its `Deserialize` derive on the production-backup-shape struct in `migration.rs`.

No SQLite, no embedded database, no ORM, no networking stack, no LevelDB parser was added or considered necessary - production's actual persistence need (a small, single-user, single-machine JSON blob) remains exactly what a plain serialized file satisfies, per the architecture freeze's section 14 decision, re-confirmed rather than reopened.

## 27. Files changed

Modified: `Cargo.toml`, `Cargo.lock` (two new direct deps, both already transitive), `src/app_model.rs` (`with_timer_persistence`/`with_timer_persistence_and_clock`, `AppTimer::restore`/`new_with_persistence`, `TimerModeConfig::mode()`), `src/main.rs` (real port wiring, `maybe_import_production_backup`, `STUDY_NATIVE_TIMER_STATE_REPORT` diagnostic hook), `src/platform/paths.rs` (`STUDY_NATIVE_DATA_DIR` test-isolation override), `src/timer_controller.rs` (`TimerController::force_persist`), `docs/timer-compatibility-spec.md` and `docs/stage14-timer-productionization.md` (a mojibake fix to prior "section N" references - see section 30 for how it was caught - no content change).
New: `src/persistence/mod.rs`, `src/persistence/store.rs`, `src/persistence/timer_port.rs`, `src/persistence/migration.rs`, `tests/fixtures/sanitized-production-backup.json`, `docs/stage15-persistence-migration.md` (this file).

## 28. Tests/checks

```text
cargo fmt --check                 -> pass
cargo check --workspace           -> pass, 0 warnings
cargo test --workspace            -> 122 passed, 0 failed, 2 ignored (was 84 after Stage 14; +38 this stage)
cargo test -p study-tracker-core  -> 25 passed, 0 failed (unchanged - study-tracker-core was not touched)
cargo build --release             -> pass
```

New automated tests by area: `persistence::store` (14 tests - round-trip, schema versioning, every corruption case in section 20, atomic-write/leftover-tmp behavior), `persistence::timer_port` (4 tests - load/persist round-trip, overwrite, never-discards-unrelated-sections), `persistence::migration` (15 tests - discovery/copy/classification/conversion, every corruption case, commit/rollback/idempotence, the withheld-secret guarantee), `timer_controller::force_persist_writes_immediately_without_a_command`, `app_model` (3 new: fresh-start writes nothing, restores a running session into its correct preset tile, recovers-and-force-persists an expired session exactly once).

Real (non-unit-test) verification performed manually, not fabricated: the restart/kill-process sequences in section 12, the end-to-end import run in section 19, and the performance measurements in section 21 - all against isolated data directories, all using the project's zero-synthetic-input environment-variable hooks (no synthetic mouse/keyboard input anywhere in this stage's verification, consistent with the Stage 13 privacy-incident rule still governing this project).

## 29. Production integrity

`git diff -- desktop` is empty. `desktop/` was read (section 3) but never written. No real production or dev-profile user data was imported, modified, or deleted (section 25).

## 30. Risks/open issues

- `imported-backups/` accumulates one copy per import attempt with no retention/pruning policy yet - low risk at Stage 15's diagnostic-tool scale, worth a policy once a real UI import flow exists (Stage 16+).
- `Reserved` sections are classified and preservable (`preserve_reserved_sections`) but not yet wired into `commit_import` - intentional (section 14), but means a real import today only ever materializes the timer; a user expecting their full history migrated would need to wait for that wiring.
- The mojibake-artifact bug (a `§` section-symbol getting corrupted into two Thai codepoints via a particular tool write path) recurred in this stage's own new files despite being previously flagged in project history; caught via a systematic non-ASCII byte sweep before committing, not by chance - worth remembering as a recurring hazard of this specific tool environment, not a one-off.
- No stress test of a "very large but reasonable" backup was run (production's own real data is small enough that this is a low-priority gap, not a known defect).
- `settings`' sub-fields were not individually re-audited for sensitivity beyond the whole-object classification (section 2) - flagged for whichever future stage first actually consumes `settings`.

## 31. Stage 15 verdict

**PASS.** Every acceptance criterion in the brief's section 39 was met: Stage 14 was checkpointed separately and cleanly beforehand; the production persistence/backup mechanism was fully inventoried and understood; native storage has explicit, checked schema versioning; storage uses Stage 13's app-data paths with production/native directories provably isolated; `study-tracker-core` performs no filesystem I/O; a real `TimerPersistencePort` implementation is wired into normal runtime in place of `NullPersistencePort`; running/paused/expired timer recovery was verified against **real killed-and-restarted Windows processes**, not only injected snapshots; recovered session effects were shown exactly-once across a real third launch; writes are demonstrably transition-driven with zero measured idle/minimized disk churn; writes are atomic via a verified (not assumed) Windows `rename` behavior; a failed/uncommitted import leaves the native destination provably unchanged; the production migration source is read-only (byte-hash-verified before/after a real import run) and uses the supported backup format, never WebView2 internals; import is validated before commit with a read-back check; re-import is idempotent; unknown/unported production data is classified and never silently discarded from the report; secrets are classified `Withheld` and verifiably never written; corrupted inputs never panic across an extensive table of cases; every automated test uses an isolated temp directory; no real production or dev-profile user data was imported or altered (one harmless accidental touch of this project's own already-existing dev log directory is reported plainly in section 25, not concealed); Stage 14's performance/rendering guarantees were re-measured and hold with real persistence active; `desktop/` is untouched; and Stage 16 was not started.

## Ready for real migration test?

**NO** in the sense of "point this at your actual production profile today" - that is an explicit, separate, user-approved action this stage deliberately does not take or invite casually. **YES** in the sense the brief actually asks: the implementation is proven safe and ready for a *real, user-approved* migration test whenever you want to run one, with these exact remaining considerations, not blockers: (1) only `timer` will actually be imported today - your other production data (sessions, courses, planner, achievements, social) will be recognized and reported field-by-field but not yet written into the native store (section 14/section 30); (2) you would need to supply the path to a real production backup file yourself (`STUDY_NATIVE_IMPORT_BACKUP=<path>`) - nothing runs automatically; (3) I'd recommend doing it once, first, with `STUDY_NATIVE_IMPORT_DRY_RUN=1` to see the exact field report before committing anything, exactly as the diagnostic tool was designed to support.

## Proposed Stage 16

Sessions + courses + planner domain migration, using this stage's persistence architecture: define the DTOs, wire `preserve_reserved_sections`/`commit_import` (or their Stage 16 successors) to actually materialize those sections, and build the real Slint Planner screen against production's actual field names and shapes from `types.ts`, per the architecture freeze's own roadmap (Phase D).
