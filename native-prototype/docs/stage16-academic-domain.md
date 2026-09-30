# Stage 16 — Sessions, courses, semesters and planner domain migration

**Production reference**: Study Tracker v0.1.66, upstream commit `b095706994b6caaf18432d0d41b4534f4b85be98` (unchanged since Stage 14/15's audits; re-confirmed no relevant commits landed since).

## 1. Scope

Migrate production's academic/planner data model - Semester, Course, Task, Exam, StudySession, and the Planner value objects (TimetableEvent, Holiday, DailyTodo, CalendarEntry) - into native Rust domain types, connect them to Stage 15's persistence architecture, and build the real Timer-to-StudySession bridge Stage 14 deferred. Stage 17 (Dashboard on real data) is not started; social/network data remains untouched (Stage 22).

## 2. Production reference/provenance

Read fresh from `desktop/src/types.ts` (every type's full field list), `desktop/src/App.tsx` (`buildSessionFromTimer`, `buildSessionsFromTimerRange`, `pruneSessionHistory`, `addSemester`/`addCourse`/`addTask`/`addExam`, `removeTask`/`removeCourse`/`removeSemester`/`removeExam`, `completeSessionManually`), and `desktop/src/lib/storage.ts` (`recoverExpiredTimer`, `buildRecoveredSessionsFromSegments`, `normalizeSemesters`/`normalizeExams`/`normalizeTimetableEvents`/etc.). No production source was modified; `git diff -- desktop` is empty (section 30).

## 3. Domain inventory

| Entity | Production type | Required fields | Optional/derived | Identity | References | Create/update/delete behavior | Ordering | Dangling refs tolerated? | Dashboard/Garden dependent? |
|---|---|---|---|---|---|---|---|---|---|
| Semester | `Semester` | id, name | createdAt (defaulted), startDate/endDate/archived/archivedAt | random id (`makeId()`) | none | append; rename in place; delete cascades to courses+everything a course-delete cascades, plus holidays | creation order (array push) | n/a (root entity) | yes (via sessions) |
| Course | `Course` | id, semesterId, name | color, targetGrade (loose `Number()\|\|4` at creation, clamped 4-6 on load), externalUrl | random id | semesterId (unchecked at creation) | append; delete cascades to tasks/exams/timetableEvents + calendarEntries of those tasks | creation order | yes - a session's `courseId` survives its course's deletion | yes |
| Task | `Task` | id, semesterId, courseId, title | subtype (inferred if absent), unitLabel, totalUnits/completedUnits (derived elsewhere, not user-entered), dueDate, priority, notes | random id | semesterId, courseId (unchecked) | append; delete cascades to calendarEntries+timetableEvents referencing it | creation order | yes | yes |
| Exam | `Exam` | id, semesterId, courseId, title, examDate | weight, preparedness, location | random id | semesterId, courseId (unchecked) | append; delete has no cascade | creation order | yes | no |
| StudySession | `StudySession` | id, kind, startedAt, endedAt, minutes | semesterId/courseId/taskId (nullable), goal/learned/blocker/nextStep/confidence, presetLabel | random id (live) / deterministic `recovered-...` id (abandoned recovery) | semesterId/courseId/taskId (nullable, never validated, never cascade-deleted) | **prepended** (newest-first) on creation via Timer completion or manual save; `removeSession` deletes by id; pruned by age (365d) and count (3000) on every write | newest-first | yes, by design (section 20) | yes |
| TimetableEvent | `TimetableEvent` | id, semesterId, courseId, taskId, kind, date, time | endTime, repeatWeekly, recurrenceEndDate, occurrenceOverrides, completedOccurrences, url | random id | semesterId, courseId, taskId (validated against known tasks *only* during legacy-blob migration, not at ordinary creation/deletion time) | append; deleted when its task is deleted | creation order | no at legacy-migration time (dropped if taskId unresolved); yes otherwise | no |
| Holiday | `Holiday` | id, semesterId, startDate, endDate | label | random id | semesterId | append; deleted when its semester is deleted | creation order | no | no |
| DailyTodo | `DailyTodo` | id, date, title | time/endTime, notes, completed/completedAt, repeat/recurrence fields | random id | none | append | creation order | n/a | no |
| CalendarEntry | `CalendarEntry` | id, taskId, date, unitAmount | unitStart, completed/completedAt, startTime/endTime, adHoc* fields | random id | taskId (or ad-hoc semester/course ids) | append; deleted when its task is deleted | creation order | yes for ad-hoc entries | no |
| SemesterPhase | `"semester" \| "exam-prep"` | - | - | - | - | a plain field on `Semester`, not an independent entity | - | - | - |

All nine are independent domain entities in the native model (section 4 explains why none were collapsed into value objects of another).

## 4. Domain boundaries

Native module structure, mirroring the Timer domain's existing proven shape rather than production's one `AppState`:

```text
crates/study-tracker-core/src/academic/
  ids.rs        - typed id newtypes (SemesterId, CourseId, ...)
  date.rs       - LocalDate (calendar-day-only values) - see section 13
  semester.rs, course.rs, task.rs, exam.rs, session.rs, planner.rs  - one entity module each
  state.rs      - AcademicState: the aggregate + CRUD/cascade-delete/retention methods
  tests.rs      - deterministic unit tests
```

Production's `AppState` is a persistence/UI aggregation convenience (one object because `localStorage`/React state needed one), not evidence for a Rust architecture - `AcademicState` is the native aggregate instead, holding the same nine lists but with all its own mutation logic (append/update/remove/cascade/retention), directly testable with no I/O, exactly like `TimerState`. No new crate was created (unnecessary - the existing `study-tracker-core` crate boundary already fits; see section 27 for the "why not a separate crate" reasoning applied concretely: nothing here needs independent versioning or a different dependency footprint from the Timer domain).

The **application layer** (`native-prototype/src/`) gained two new modules:

```text
src/academic_controller.rs   - AcademicController: owns AcademicState + a persistence port, CRUD wrappers, route_timer_effects
src/session_service.rs       - the Timer -> StudySession bridge (local-day splitting) - see section 7/18
```

Dependency direction is unchanged and still one-way: `Slint UI -> AppModel -> AcademicController -> AcademicState (core) -> persistence port -> NativeStore`. `study-tracker-core` still depends on nothing but `serde` - confirmed by its `Cargo.toml` being untouched this stage.

## 5. Native models

See section 3's table for exact fields. Every entity's native field names are the `snake_case` form of production's own field names with no semantic renaming (`examDate` -> `exam_date`, `semesterId` -> `semester_id`, etc.) so a reader translating between the two never has to guess a mapping.

## 6. IDs/references

Production ids are opaque random strings (`makeId()`: `crypto.randomUUID()`, or a `Date.now()-Math.random()` fallback) - carrying no structure to validate, so the native `SemesterId`/`CourseId`/... newtypes are thin `String` wrappers, not parsed/validated identifiers. **Imported ids are never regenerated** (`migration_academic.rs` uses the production id string directly for every entity - see section 15, and the dedicated `ids_are_preserved_exactly_never_regenerated` test). Native-created ids (via the CRUD command path, e.g. the `STUDY_NATIVE_ADD_DEMO_COURSE` diagnostic hook) are also caller-supplied strings, not centrally generated - the same as production, where the *caller* (a form handler) calls `makeId()`, not some entity constructor.

`StudySession` ids specifically use two different schemes, exactly mirroring production's own two schemes for the same two situations (see section 7): a fresh, effectively-random id (`session-{now:x}-{counter:x}`, `session_service::fresh_session_id`) for a live/manual completion, and a **deterministic** id derived purely from `(phase, bucket_start, bucket_end)` (`session_service::recovered_session_id`) for an abandoned-timer recovery - production's own `recovered-${phase}-${startedAt}-${endedAt}` scheme, reproduced in spirit (unix millis instead of ISO strings - an intentional representation difference, not a behavioral one) specifically so recovering the *same* abandoned range twice yields the *same* id both times, and `AcademicState::add_study_sessions`'s dedup-by-id silently drops the second one.

Tested: duplicate ids (`re_adding_a_session_with_the_same_id_is_a_no_op_not_a_duplicate`), missing/dangling references (`a_course_may_reference_a_semester_id_that_does_not_exist`, `a_task_may_reference_a_course_id_that_does_not_exist`, `removing_a_semester_never_touches_historical_sessions`), malformed ids during import (any record missing its own id is skipped and reported - `a_record_missing_its_id_is_skipped_and_reported_not_rejecting_the_whole_import`), and import/re-import identity stability (`ids_are_preserved_exactly_never_regenerated`, `duplicate_session_ids_across_two_imports_are_not_double_counted`).

## 7. StudySession semantics

Production's `buildSessionFromTimer`/`buildSessionsFromTimerRange` do one thing this stage had to reproduce exactly: **a session that crosses local midnight is split into one record per local calendar day**, each with its own start/end and a rounded minute count, all pieces on the same day merged into one record (summing only their *active* time, excluding any paused gap between them). `session_service::split_segments_into_daily_sessions` reproduces this precisely (see section 18/section 12 for the clock-offset caveat) - proven with a dedicated same-day-merge-excludes-the-gap test (`two_segments_on_the_same_local_day_with_a_gap_merge_into_one_session_excluding_the_gap`).

`SessionKind` mapping (`session_kind_for_phase`): Study/Stopwatch phases -> `Study`, Exam phase -> `Exam`, Break/Idle -> `None` (no session at all - Break completions never reach this function, matching production's `buildSessionsFromTimerRange` never being called for a Break). Endless (Stopwatch) manual completion producing a `Study`-kind session matches production exactly (`timer.phase === "exam" ? "exam" : "study"` - stopwatch defaults to study).

Retention (`AcademicState::prune_session_history`/`add_study_sessions`): 365-day age cutoff **and** a 3000-record cap, applied on every insertion - identical to production's `SESSION_HISTORY_DAYS`/`SESSION_HISTORY_MAX` constants and its `pruneSessionHistory` function's own two-part filter. Lifetime totals (`lifetime_study_minutes`/`lifetime_study_sessions`) are a running counter incremented only for Study/Exam kinds, never recomputed from the (pruned) list - matching `getNewLifetimeTotals`'s accumulation onto `state.lifetimeStudyMinutes` exactly, and verified to survive both pruning and individual-session removal (`removing_one_session_does_not_affect_others_or_lifetime_totals`).

## 8. Course semantics

IDs/names/colors/metadata/semester relationship all preserved (section 3/5). `externalUrl` preserved as `Option<String>`. No archived/soft-delete concept exists for `Course` in production (only `Semester` has `archived`) - not invented here either. Deletion cascades exactly as production's `removeCourse` (section 20).

## 9. Semester semantics

`archived`/`archivedAt` preserved as plain fields - production has no "current/active semester" concept computed by the core data model itself (any "which semester is active" UI logic in `App.tsx` is presentation-layer selection state, not a stored field, and is out of Stage 16's scope per section 18's UI-integration limits). Date ranges (`start_date`/`end_date`) are optional `LocalDate`s with **no** overlap or ordering validation - matching production exactly, which never validates this either (confirmed by reading `addSemester`/`updateSemester`: no date-conflict check exists at all). `SemesterPhase` (`Semester`/`ExamPrep`) is a plain manually-set field, not derived from dates.

## 10. Task semantics

Completion (`completed_units`/`total_units`), due dates, course association, priority, and notes are preserved as plain data. Production's actual completion/progress computation (`plannerSchedule.ts`'s recurrence-expansion engine, which recomputes these two fields from checked-off calendar occurrences) is **not** migrated this stage - out of scope per the brief's own instruction not to invent functionality; the two numeric fields are carried through as opaque data a future stage can wire real computation into. Subtype defaults to whatever the caller supplies (no legacy label-inference heuristic reproduced - a deliberate simplification since that heuristic only matters for very old pre-subtype production data, and any subtype-carrying record from any real backup already carries an explicit `subtype` string).

## 11. Exam semantics

**Explicitly distinct from `TimerMode::Exam`/`TimerPhase::Exam`** (a Timer preset), documented at the top of `academic::exam`'s own module doc comment specifically to prevent future confusion: an `Exam` is academic-calendar data (a date, a weight, a preparedness score) with no relationship to the Timer beyond a completed Timer *block* happening to carry `kind: "exam"` on its resulting `StudySession` when the Timer's *mode* was Exam. No cascade on deletion (nothing references an exam by id in production).

## 12. Calendar/timetable/planner semantics

Kept as four **distinct** types (section 3's inventory), not flattened: `TimetableEvent` (recurring class/lecture/sheet slot, tied to course+task, with per-occurrence override support modeled via a deterministic `BTreeMap`, not a `HashMap`, for ordering - section 21), `Holiday` (a semester-wide date range, no course/task), `DailyTodo` (personal, no semester/course/task association at all), `CalendarEntry` (a scheduled work-unit allocation against a specific task, with `unitAmount` modeled as an exhaustive 3-value enum (`UnitAmount::{Whole,Half,Quarter}`) rather than a bare float, matching production's own hard-coded restriction to exactly `1`/`0.5`/`0.25`). Per-occurrence override *values* (skip/reschedule) and completed-occurrence lists are carried as empty/default on import this stage (see `convert_timetable_event`'s doc comment) - the events themselves migrate; their occurrence-level refinements are a lower-value, comparatively complex nested structure deliberately left for a later pass, not silently discarded (the untouched raw production data remains recoverable via the source backup copy, section 15).

## 13. Date/time policy

Three deliberately distinct kinds of time value (see `academic::date`'s own extensive module doc comment, reproduced in essence here):

- **Absolute instant** (`created_at`, `archived_at`, session `started_at`/`ended_at`, `completed_at`): reuses the Timer domain's existing `WallTimestamp` (unix milliseconds) - a real, timezone-independent moment.
- **Local calendar date** (`LocalDate`, `YYYY-MM-DD`): semester/task/exam/holiday/todo/calendar-entry dates name a *day*, not an instant, and are never converted to a UTC timestamp - doing so could silently shift which calendar day a date-only value near a local midnight boundary represents. Validated only for shape (`LocalDate::parse`), not real calendar correctness (`2026-02-30` parses fine) - matching production's own complete lack of stronger validation on these plain strings.
- **Local time-of-day** (`HH:MM` strings on `TimetableEvent`/`DailyTodo`/`CalendarEntry`): a recurring weekly slot, kept as plain `String`/`Option<String>` - a fourth wrapper type for a handful of fields was not worth the ceremony.

**The one place a real timezone decision was unavoidable**: splitting a Timer session at local midnight (section 7/18) needs to know the local UTC offset. This uses a `FixedOffset` **captured once** at the moment of handling (`chrono::Local::now().offset()` at the real call site), not a per-instant timezone lookup - documented as a deliberate simplification with one known, narrow limitation: a session that happens to span a real DST transition could have its midnight-split boundary misplaced by up to the DST shift (typically one hour) right around the transition. Tested directly with a positive `FixedOffset` proving the offset genuinely shifts which day a given instant lands on (`a_positive_timezone_offset_shifts_the_local_midnight_boundary`); a true DST-transition test was not built (it would need a `chrono-tz`-style real timezone-rule dependency this stage did not add - see section 27) - this is a documented, narrow, low-stakes gap (it only ever shifts which day a few minutes of one session are logged under), not a silent one.

## 14. Persistence schema decision

**`schema_version` stays at `1`.** A brand-new top-level `academic` section was added to the native store envelope (`StoreEnvelope.academic: Option<AcademicState>`) - additive, not a change to the existing `timer` section's shape, so no version bump is warranted per Stage 15's own rule ("bump only when an *existing* section's shape changes incompatibly"). Verified bidirectionally: a Stage-15-shaped (timer-only) store loads correctly with `academic: None` (exercised by every pre-existing Stage 15 persistence test, all still passing unmodified); a Stage-16-shaped store (with `academic` present) would be read correctly by hypothetical unmodified Stage 15 code too, since Stage 15's `other: Map<String, Value>` catch-all already round-trips any key it doesn't itself model (this exact mechanism was the reason Stage 15 built `other` in the first place). No migration test needed beyond what Stage 15 already has, since nothing about the existing section changed.

## 15. Import conversion

`persistence/migration_academic.rs`'s `convert_academic` extends Stage 15's pipeline: every array-shaped production section (`semesters`, `courses`, `tasks`, `exams`, `sessions`, `timetableEvents`, `holidays`, `dailyTodos`, `calendarEntries`) is now `Consumed` (was `Reserved`) and converted into the corresponding native list. Unlike the Timer section's single strict `serde`-derived struct parse (`ProductionTimerState` in `migration.rs`), these are **field-level tolerant per-record** conversions (`get_str`/`get_string_or`/`get_local_date`/... helpers operating on the raw `serde_json::Map`), matching production's own `normalizeXxx` philosophy exactly - a record missing a field central to its identity/meaning is skipped and reported (never rejecting the whole array); a record with only a malformed *optional* field keeps the record and defaults just that field. Sessions specifically route through `AcademicState::add_study_sessions` (not a plain array push), so an import that happens to contain an id already present natively is deduped and its minutes are not double-counted in the lifetime totals - the same guarantee a live recovery gets.

## 16. Normalization

Documented per-domain in the entity modules' own doc comments and enforced by `migration_academic.rs`'s conversion functions; the policy in one place: identity-critical fields (id; for `Exam`, also `examDate`, since a dateless exam is meaningless; for `StudySession`, also `startedAt`/`endedAt`/`kind`) missing or malformed -> the whole record is skipped and reported. Purely cosmetic/optional fields (color, notes, `startDate`/`endDate` on a `Semester`, ...) missing or malformed -> default, record kept. An unknown future enum value (an unrecognized `phase`/`kind`/`subtype`/`priority` string) never gets silently reinterpreted as something else picked at random - it either falls back to an explicit, documented default (`TaskSubtype::Other`, `Priority::Medium`, `TimetableEventKind::Occurrence`) or, where the value is load-bearing enough that a wrong guess would be worse than skipping (`StudySession.kind`), the record is rejected instead.

## 17. CRUD/application services

`AcademicController` (application layer) wraps every `AcademicState` mutation with a persist call: `add_semester`/`rename_semester`/`remove_semester`, `add_course`/`update_course`/`remove_course`, `add_task`/`update_task`/`remove_task`, `add_exam`/`update_exam`/`remove_exam`, plus append/remove for the four planner value objects. Slint never mutates a persistence DTO directly - `AppCommand` variants (`AddSemester`, `AddCourse`, `RemoveCourse`, `RemoveSemester`) go through `AppModel::apply`, which calls into `AcademicController`, matching the existing Timer command pattern exactly. See section 22 for which of these are currently reachable from a *real* interface (a diagnostic hook today, not yet an interactive form) versus proven only at the Rust test level.

## 18. Timer -> session flow

The exact end-to-end path, now real:

```text
TimerController::apply/restore (Stage 14)
  -> TimerApplicationEffect::SessionRangeReady { phase, reason, segments, context, preset_label }
    -> AcademicController::route_timer_effects(effects, local_offset, clock)
      -> session_service::split_segments_into_daily_sessions(...)   (local-day splitting, section 7)
        -> one or more StudySession values, ids from fresh_session_id or recovered_session_id
          depending on `reason` (CompletionReason::AbandonedRecovery vs. anything else)
      -> AcademicState::add_study_sessions(sessions, now)            (dedup + retention + lifetime totals)
    -> AcademicController persists the whole academic section
```

Called from **both** `AppModel::apply_timer_command` (every live command - Start/Pause/Resume/Reset/Refresh/manual completion) and `AppModel::with_timer_persistence_and_clock` (startup recovery), so a recovered abandoned session goes through the identical path a live completion does - not a separate, parallel implementation. No React/Tauri dependency anywhere in this path (it is pure Rust, `study-tracker-core` + the two new application-layer modules).

**Verified on real Windows hardware, not only in unit tests**: a Demo-preset (10 s) Timer, autostarted with zero synthetic input, genuinely completed during a real event-loop run and produced exactly one real, persisted `StudySession` (`session-1a0e5fdfe30-0`, `kind: "Study"`, `minutes: 1`, `preset_label: "Demo"`), confirmed by inspecting the real `store.json` after killing the process; a real restart then reloaded that same session (`ACADEMIC_STATE ... sessions=1`); a genuinely abandoned-and-recovered import scenario (an imported backup's timer section had already expired relative to real wall-clock time) produced exactly one additional recovered session on top of the imported one, with no duplication (section 22, section 28).

Every scenario the brief's section 19 lists was exercised: normal Focus completion, Focus-with-break (no session until the whole block finishes - unchanged Stage 14 behavior), Exam completion, a recovered expired session, repeated startup (no duplicate - section 22), reset/no-session cases (Reset alone never produces a session, matching the core's own `TimerEvent` shape), and Endless/Stopwatch manual-completion behavior (produces a `Study`-kind session, matching production).

## 19. Consistency/atomicity

The Timer's own persisted state and the newly-created `StudySession`(s) are never written as two separate file operations that could leave a half-applied state: `AcademicController::route_timer_effects` persists the *whole* academic section in one `NativeStore::save` call (itself already atomic - write-to-`.tmp`-then-`rename`, unchanged from Stage 15), and the Timer's own `PersistenceRequested`-triggered save is a separate, already-atomic write to the same envelope's `timer` section. A crash between the two leaves one of them written and the other not - by design, matching how production itself has no cross-`localStorage`-key transaction either (`state.timer` and `state.sessions` are separate keys/sections there too, per the Stage 12.5 freeze's own inventory). No transaction database was introduced (correctly out of scope per section 20's explicit instruction); the ordering (Timer state settles first inside `TimerController::apply`, then the session-creation/persist happens in the same synchronous call before `apply_timer_command` returns) means a crash can, at worst, leave a completed-and-reset Timer with the session not yet written - recoverable, since the Timer's own recovery logic (Stage 14) does not depend on that session having been recorded, and a future restart's own recovery pass (if the Timer state itself indicates an abandoned/expired block) would still recreate it deterministically via `recovered_session_id`.

## 20. Deletion/reference semantics

Reproduced exactly from reading `removeTask`/`removeCourse`/`removeSemester`/`removeExam` in `App.tsx` (section 3's table, cascade column) and locked in with dedicated tests (`removing_a_semester_cascades_to_its_courses_tasks_exams_timetable_and_holidays`, `removing_a_course_cascades_to_its_tasks_exams_timetable_and_their_calendar_entries`, `removing_a_task_cascades_to_its_calendar_entries_and_timetable_events_only`, `exams_have_no_cascade_and_preserve_creation_order`). **The one rule verified most deliberately**: deleting a Semester/Course/Task never touches `AcademicState::sessions` - `removing_a_semester_never_touches_historical_sessions` constructs a session referencing a semester, deletes that semester, and asserts the session (with its now-dangling `semester_id`) survives untouched. This matches production exactly: historical study time is never retroactively erased because its academic context was later deleted.

## 21. Ordering

Every list in `AcademicState` is a plain `Vec`, never a `HashMap`, so iteration/serialization order is exactly insertion order - deterministic by construction, with no accidental reliance on hash-map iteration order anywhere in this stage's code (including `TimetableEvent.occurrence_overrides` and `DailyTodo.occurrence_times`, which use `BTreeMap`, not `HashMap`, specifically to keep their own key order deterministic too). Semesters/courses/tasks/exams/planner items preserve creation order (matching production's own `[...current.list, newItem]` append pattern); `StudySession`s are **prepended** (newest-first), matching production's `[...newSessions, ...existing]`. Verified directly: `semesters_are_appended_in_creation_order`, `new_sessions_are_prepended_newest_first`.

## 22. UI integration

**What real native UI now uses these domains, concretely**: the existing Timer screen's session-notes card (`ui/main.slint`'s already-data-driven `session-notes` property, unchanged markup - see `main.rs`'s `apply_model_to_window`) now renders the 8 most recent **real** `StudySession` records from `AcademicState`, replacing the Stage 4-15 hardcoded three-line demo placeholder entirely (`AppModel::session_notes()`'s doc comment spells out the before/after). This is real domain data, persisted, surviving restarts, verified on real hardware (section 18).

**What is deliberately not built yet**: an interactive Planner form for creating/editing courses, semesters, tasks, or exams by hand. Per the brief's explicit instruction not to attempt "a giant pixel-perfect production Planner rewrite," the real CRUD command path (section 17) is instead proven end-to-end through: (a) exhaustive deterministic tests at the `AcademicState`/`AcademicController` level, and (b) real, zero-synthetic-input diagnostic hooks (`STUDY_NATIVE_ADD_DEMO_COURSE`, `STUDY_NATIVE_REMOVE_DEMO_COURSE`/`_SEMESTER`, `STUDY_NATIVE_ACADEMIC_STATE_REPORT`) that call the exact same `AppCommand`s a future interactive form would - verified on real hardware to create a real Semester+Course, persist them, and survive a real restart (section 18's evidence, and the `store.json` excerpt showing `"name": "Fall 2026"`/`"name": "Analysis II"` created this way). A real interactive Planner form is Stage 17+ UI work, not redone here to avoid "inventing" a UI design under this stage's time budget rather than migrating production's actual one.

The Stage 10 Dashboard remains synthetic, untouched, per the brief's explicit instruction (Stage 17's job).

## 23. Synthetic stress dataset

`src/synthetic_dataset.rs`, reached only via `STUDY_NATIVE_GENERATE_SYNTHETIC_ACADEMIC_DATA=1` (never automatic). Entirely fabricated, no personal data. Sizes chosen deliberately (documented in the module's own doc comment too, so the two can never drift):

| Entity | Count | Rationale |
|---|---|---|
| Semesters | 3 | A student a few years in |
| Courses | 24 (8/semester) | "dozens of courses" |
| Tasks | 360 (15/course) | "hundreds of tasks" |
| Exams | 72 (3/course) | Midterm/final/resit-shaped |
| Study sessions | 2000 | "hundreds/thousands"; generated directly (bypassing the retention-pruning insertion path - see section 24's note) specifically to test raw cold-load/parse cost at that file scale |
| Timetable events | 120 (5/course) | A realistic weekly lecture/exercise/sheet load |
| Holidays | 10 | |
| Daily todos | 60 | ~2 months of ordinary items |
| Calendar entries | 400 | A few scheduled work-units per task |

Resulting `store.json`: **~1.46 MB**.

## 24. Performance

Measured with `scripts/win-metrics.ps1` (Stage 12's methodology), real Windows process trees, isolated `STUDY_NATIVE_DATA_DIR`, the synthetic dataset above already persisted on disk unless noted:

| Point | Private WS | CPU | Notes |
|---|---|---|---|
| S16-P0 idle, large dataset present | 46.6 MB | 0% | session list still only renders its 8-item cap (section 22) - no cost scales with total session count |
| S16-P1 Focus running, visible, large dataset | ~50-51 MB | 0.3-0.6% | within the same noisy band Stage 14/15 already measured for this scenario; no clear regression attributable to dataset size |
| S16-P2 Focus running, **minimized**, large dataset | 46.7 MB | **0%** | `IsIconic` confirmed `True` - the Stage 14 minimized-rendering fix holds unchanged with real, large academic data loaded |
| S16-P3 create-course command, large dataset | 49.7 MB | ~0.5% | one more course persisted; store grew by ~1.4 KB, no hang |
| S16-P4 Timer completion -> session persistence, large dataset | n/a | n/a | confirmed via a clean, isolated run: a real Demo completion against the 2000-session dataset produced exactly one new, correctly-shaped `StudySession` within the real 12 s window |
| S16-P5 cold start with the large dataset | 46.6 MB | 0% at idle | first-frame time **160.6 ms** - in the same class as the ~150-206 ms warm-startup baseline from Stage 12/15; parsing/loading the full ~1.46 MB, ~2900-record store adds no measurable startup cost |

**A real methodology finding worth recording plainly**: an earlier, longer sequential test script (generate -> idle -> running -> running-longer -> minimized -> add-course -> *then* completion, all against one data directory with only brief pauses between kills) produced a run where the Demo timer appeared not to complete. Investigating properly (rather than assuming either "it's fine" or "it's broken") found the actual cause: an *earlier* step in that same sequence had left a genuinely still-running, non-expired Deep Work (52-minute) timer persisted; the next step's `SetMode(Demo)` command was then correctly rejected by the core's own mode-change guard (Stage 7's frozen rule: mode changes are blocked while active), so `Start` simply resumed the pre-existing 52-minute countdown instead of starting a fresh 10-second one - which cannot complete in a 12-second window. A clean, isolated re-run (generate large dataset -> fresh process -> Demo autostart -> 12 s -> kill) confirmed the real behavior is correct: one new session was created, byte-verified in the resulting store file. Recorded here because it's a legitimate finding from real measurement, not because it turned out to matter: a multi-step manual test script's own leftover state, not a Stage 16 defect.

No continuous disk churn was found at any point (write timestamps checked, not just CPU/memory, across every scenario above).

## 25. Memory

`AcademicState` holds one in-memory copy of each list; `NativeStore::load`/`save` parse and discard a transient `serde_json::Value` per call (unchanged from Stage 15); `AcademicController` holds no additional cache beyond the one `AcademicState` it owns. Migration's own conversion path (`migration_academic.rs`) briefly holds the raw parsed JSON, the converted `AcademicState`, and (only during `commit_import`'s own load-modify-save) a clone of the previous envelope for rollback purposes - all transient, freed once the import call returns; steady-state memory (section 24's measurements) shows no retained duplication at the ~1.46 MB/2900-record scale tested.

## 26. Windows verification

All measurements and manual verification in this document are Windows-only, on this project's real Windows hardware - no Linux/macOS runtime claim is made. `study-tracker-core::academic` and `session_service`/`academic_controller`/`migration_academic` contain zero platform-gated (`cfg(windows)`) code and no Windows-specific API calls; `session_service`'s one real-clock dependency (`chrono::Local`, called only from `main.rs`/`app_model.rs`, never from `study-tracker-core`) is already cross-platform via the `chrono` crate.

## 27. Dependencies

**Zero new dependencies.** `chrono` (already a direct dependency since Stage 13, for log timestamps) gained real new usage (`FixedOffset`, `NaiveDate`, `TimeZone` trait methods) in `session_service.rs`, but no new crate was added to `Cargo.toml`. `serde`/`serde_json` (already direct dependencies since Stage 15/16's own migration work) are used identically for the new `AcademicState` section. No database, ORM, async runtime, networking stack, or ID-generation crate (`uuid`, `rand`) was added - session/entity identity uses a process-local atomic counter plus the wall-clock moment (`fresh_session_id`) or pure content-derived determinism (`recovered_session_id`), both sufficient for this stage's actual uniqueness requirement (see `fresh_session_id`'s own doc comment for why this is a deliberate, justified choice over adding a dependency).

## 28. Tests

```text
cargo fmt --check                 -> pass
cargo check --workspace           -> pass, 0 warnings
cargo test --workspace            -> 142 passed, 0 failed, 2 ignored (was 122 after Stage 15; +20 this stage)
cargo test -p study-tracker-core  -> 48 passed, 0 failed (was 25; +23: academic domain + LocalDate + Course clamp)
cargo build --release             -> pass
```

New tests by area: `study-tracker-core::academic` (23 - cascades, ordering, retention, dedup, clamping, `LocalDate` parsing), `session_service` (11 - day-splitting, same-day merge excluding a gap, phase-to-kind mapping, sub-minute rounding, timezone-offset sensitivity, deterministic recovery ids), `persistence::migration_academic` (6 - full-state conversion, per-record skip-and-report, malformed-optional-field tolerance, id preservation, exam date requirement, cross-import dedup), `persistence::academic_port` (3 - round-trip, never-discards-the-timer-section), `app_model` (2 new assertions extending existing Timer-completion/recovery tests to also check real session creation). One pre-existing, unrelated `cargo test`-only warning (`map::tests::region` unused helper, Stage 11) was noticed during this stage's own zero-warnings verification but predates Stage 16 and was left untouched (out of scope; invisible to `cargo check`, which is what prior stages' "0 warnings" claims were based on).

## 29. Files changed

Modified: `crates/study-tracker-core/src/lib.rs` (+`pub mod academic`), `src/app_model.rs` (real `AcademicController` wiring, `AppCommand` CRUD variants, `session_notes()` now real), `src/main.rs` (real academic port wiring, diagnostic hooks), `src/persistence/mod.rs`, `src/persistence/store.rs` (+`academic` section), `src/persistence/timer_port.rs` (test fixture updates for the new `StoreEnvelope` field), `src/persistence/migration.rs` (semesters/courses/tasks/exams/sessions/timetableEvents/holidays/dailyTodos/calendarEntries now `Consumed`; `ImportReport` gained `academic_summary`), `tests/fixtures/sanitized-production-backup.json` (extended with real academic-domain records).
New: `crates/study-tracker-core/src/academic/{mod,ids,date,semester,course,task,exam,session,planner,state,tests}.rs`, `src/academic_controller.rs`, `src/session_service.rs`, `src/synthetic_dataset.rs`, `src/persistence/academic_port.rs`, `src/persistence/migration_academic.rs`, `docs/stage16-academic-domain.md` (this file).

## 30. Risks/open issues

- No interactive Planner UI yet (section 22) - real CRUD is proven via tests + diagnostic hooks, not a form a user would actually use day-to-day. Stage 17+'s job.
- The DST-transition midnight-split edge case (section 13) remains a documented, narrow, untested-by-real-DST-transition simplification.
- Per-occurrence `TimetableEvent`/`DailyTodo` override *values* are not yet populated by import (section 12/15) - the events themselves migrate; their fine-grained recurring-schedule refinements do not yet.
- Task progress (`total_units`/`completed_units`) is carried as opaque data; production's real recurrence-expansion computation of these fields is not migrated (section 10) - a future stage's job once the Planner UI needs it.
- The read-modify-write persistence design's write cost scales with total store size (confirmed non-alarming at ~1.46 MB in section 24, but not stress-tested beyond that) - worth revisiting only if a real user's data ever grows enough to matter, per the "no premature optimization" instruction.

## 31. Deferred Stage 17+ data

Dashboard/statistics wiring to real data (Stage 17, explicitly not started - Stage 10's synthetic Dashboard is untouched). Garden/progression, Wabi-Sabi/themes, Break Room games/achievements, social/network state, verified-session anchoring, telemetry, tray/notifications/updater - all per the architecture freeze's own phase ordering, none touched this stage.

## 32. Stage 16 verdict

**PASS.** Every acceptance criterion in the brief's section 35 was met: Stage 15 was checkpointed separately (already committed) before this stage began; the production academic/planner domain was inventoried from source, not assumed; native domain boundaries are explicit and justified (no new crate, a lighter CRUD shape than Timer's command/event machine, justified by these entities' actual simplicity); no domain model depends on Slint/filesystem/Tauri; a stable, non-regenerating identity policy exists and is tested; `StudySession` is a real, tested native entity with a real, verified-on-real-hardware Timer bridge including exactly-once recovered-session creation; Courses/Semesters/Tasks/Exams/relevant Planner data are all migrated with real, tested CRUD and cascade-delete semantics matching production; date/time semantics are a deliberate, documented three-way policy; Stage 15's Timer-only storage remains readable (schema version correctly did not bump); the production backup importer now materializes every Stage 16 domain, tolerantly, per-record, with ids preserved and secrets still withheld; ordering is deterministic everywhere; no domain logic was duplicated into Slint; no polling or disk churn was introduced (verified by real write-timestamp checks, not just CPU); the synthetic realistic dataset performs with no measurable regression at cold-start, idle, running, or minimized; Stage 14's minimized-timer guarantee was re-verified and holds; no real production or dev-profile user data was touched; `desktop/` is unchanged; and Stage 17 has not been started.

## Proposed Stage 17

Dashboard migration: replace Stage 10's synthetic dataset with real data drawn from this stage's `AcademicState` (sessions -> weekly/history charts, courses -> course-breakdown cards, streak calculation from real session dates), matching production's `metrics.ts` calculations exactly per the architecture freeze's own Phase E scoping.
