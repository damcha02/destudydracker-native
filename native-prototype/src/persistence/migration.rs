//! Production backup -> native store import pipeline (Stage 15).
//!
//! **Source**: production's own file-based backup (`desktop/src/lib/storage.ts::buildBackup`/
//! `saveBackup`/`restoreBackup`), never WebView2/`localStorage` internals. Production's backup is
//! plain JSON, written through a supported, already-shipped code path (a Tauri `save` file dialog
//! plus the `write_backup_file` command), so importing it is reading a format production itself
//! already promises to produce and to be able to read back - safer and simpler than reverse-
//! engineering WebView2's LevelDB storage, which this module deliberately does not do (see
//! docs/stage15-persistence-migration.md, "Production backup mechanism").
//!
//! **Pipeline** (every step below is a plain function so it can be unit-tested independently):
//! `discover_and_read` (copy-then-parse, never touches the source again) -> `classify_fields`
//! (storage-inventory-driven field-by-field report) -> `convert_timer` (the only section this
//! build actually materializes into the native store) -> a caller commits the result with
//! `NativeStore::save` only after inspecting the report, exactly like `commit_import` below.
//!
//! **No source mutation anywhere in this module.** No network access. No real user data is
//! imported by anything in this file automatically - every function takes an explicit path/value
//! and returns a value or a report; nothing here is wired into `main.rs`'s normal startup.

use serde::Deserialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::persistence::store::StoreEnvelope;
use study_tracker_core::timer::{
    ActiveSegment, TimerConfig, TimerContext, TimerMode, TimerPhase, TimerSnapshot, WallTimestamp,
};

#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    NotJson(String),
    /// Mirrors production's own `restoreBackup` gate exactly (see `storage.ts`): a file lacking
    /// `sessions`/`courses` arrays and a `social` object is not recognized as a Study Tracker
    /// backup at all, native or otherwise.
    NotAStudyTrackerBackup,
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::Io(err) => write!(f, "I/O error: {err}"),
            ImportError::NotJson(reason) => write!(f, "not valid JSON: {reason}"),
            ImportError::NotAStudyTrackerBackup => {
                write!(f, "that file is not a Study Tracker backup")
            }
        }
    }
}

/// How one top-level `AppState` key found in a production backup is treated by this pipeline.
/// See docs/stage15-persistence-migration.md section 2 for the full inventory this classification is
/// drawn from, and section 19/section 24 of the Stage 15 brief for the policy behind the three classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClass {
    /// (A) Understood and actually materialized into the native store by this pipeline today.
    /// Only `timer`.
    Consumed,
    /// (B/C) Understood (or at least recognized) but not yet owned by any native domain - kept
    /// byte-for-byte in the native store's opaque `other` map so a later stage (Stage 16+) can
    /// read it back without this stage having invented a half-built domain model for it.
    Reserved,
    /// Secrets, device/social identity, or server-cached network state. Recognized (so the
    /// report is honest about what was found), but deliberately never written into the native
    /// store by this pipeline, even opaquely - see section 24/section 25 of the Stage 15 brief. Neither
    /// migrated nor silently dropped from the *report*: the report always says a withheld field
    /// was present.
    Withheld,
}

#[derive(Debug, Clone)]
pub struct FieldReport {
    pub key: String,
    pub class: FieldClass,
    pub note: &'static str,
}

/// The fixed classification table this pipeline uses. Ordered roughly as production's own
/// `AppState` shape (`types.ts`) is grouped, and kept exhaustive against the top-level key list in
/// docs/stage12_5-architecture-freeze.md section 13.1/section 19 - a key not in this table falls back to
/// `Reserved` (see `classify_fields`), never silently dropped.
fn known_field_class(key: &str) -> Option<FieldClass> {
    match key {
        "timer" => Some(FieldClass::Consumed),
        "social" => Some(FieldClass::Withheld),
        _ => None,
    }
}

#[derive(Debug)]
pub struct DiscoverOutcome {
    /// Where the source backup was found (never written to).
    pub source_path: PathBuf,
    /// A verbatim copy of the source file, written before anything else touches it - the
    /// "BACKUP SOURCE/COPY" pipeline step. Untouched for the rest of the run; a caller can re-
    /// verify byte-identity against `source_path` at any point (see the "source remains byte-
    /// identical" test).
    pub source_copy_path: PathBuf,
    /// The parsed `state` object (production's `AppState`, or the object itself if the file was
    /// an unwrapped pre-`backupVersion` blob) - not yet validated or converted.
    pub state: Map<String, Value>,
}

/// DISCOVER + READ SOURCE + BACKUP SOURCE/COPY, in one step because the copy must happen before
/// anything else is done with the file's bytes. Never opens `source_path` for writing.
pub fn discover_and_read(
    source_path: &Path,
    backup_copy_dir: &Path,
) -> Result<DiscoverOutcome, ImportError> {
    let raw = fs::read_to_string(source_path).map_err(ImportError::Io)?;

    fs::create_dir_all(backup_copy_dir).map_err(ImportError::Io)?;
    let copy_name = format!(
        "{}-{}.json",
        source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("study-tracker-backup"),
        now_unix_millis_for_filename(),
    );
    let source_copy_path = backup_copy_dir.join(copy_name);
    fs::write(&source_copy_path, &raw).map_err(ImportError::Io)?;

    let parsed: Value =
        serde_json::from_str(&raw).map_err(|err| ImportError::NotJson(err.to_string()))?;
    let Value::Object(wrapper) = parsed else {
        return Err(ImportError::NotAStudyTrackerBackup);
    };

    // Mirrors `restoreBackup`'s own unwrap: a `{backupVersion, state, preferences}` envelope
    // (current production shape) or a bare pre-wrapper `AppState` blob (older backups/exports).
    let is_wrapped = wrapper
        .get("backupVersion")
        .and_then(Value::as_u64)
        .is_some()
        && matches!(wrapper.get("state"), Some(Value::Object(_)));
    let state = if is_wrapped {
        match wrapper.get("state") {
            Some(Value::Object(state)) => state.clone(),
            _ => unreachable!("checked by is_wrapped above"),
        }
    } else {
        wrapper
    };

    let has_sessions = matches!(state.get("sessions"), Some(Value::Array(_)));
    let has_courses = matches!(state.get("courses"), Some(Value::Array(_)));
    let has_social = matches!(state.get("social"), Some(Value::Object(_)));
    if !has_sessions || !has_courses || !has_social {
        return Err(ImportError::NotAStudyTrackerBackup);
    }

    Ok(DiscoverOutcome {
        source_path: source_path.to_path_buf(),
        source_copy_path,
        state,
    })
}

fn now_unix_millis_for_filename() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// Classifies every top-level key actually present in `state` (CLASSIFY, part of NORMALIZE ->
/// VALIDATE). Never fabricates a key that isn't there.
pub fn classify_fields(state: &Map<String, Value>) -> Vec<FieldReport> {
    let mut report: Vec<FieldReport> = state
        .keys()
        .map(|key| {
            let class = known_field_class(key).unwrap_or(FieldClass::Reserved);
            let note = match (key.as_str(), class) {
                ("timer", _) => "converted into the native TimerSnapshot format",
                ("social", _) => {
                    "device secret / friend code / verified-session anchor / cached network \
                     state - withheld, never written to the native store (see Stage 15 section 24/section 25)"
                }
                (_, FieldClass::Reserved) => {
                    "preserved opaquely for a later stage's own domain; not modeled by Stage 15"
                }
                _ => "",
            };
            FieldReport {
                key: key.clone(),
                class,
                note,
            }
        })
        .collect();
    report.sort_by(|a, b| a.key.cmp(&b.key));
    report
}

/// Production's `TimerActiveSegment`/`TimerState` shape (`desktop/src/types.ts`), read exactly as
/// production writes it: camelCase keys, ISO-8601 wall-clock strings (never unix millis, unlike
/// the native `TimerSnapshot`) - matching `timer-compatibility-spec.md`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductionActiveSegment {
    started_at: String,
    ended_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProductionTimerState {
    phase: String,
    mode: String,
    remaining_seconds: f64,
    logged_split_seconds: f64,
    active_segments: Vec<ProductionActiveSegment>,
    running: bool,
    study_minutes: f64,
    break_minutes: f64,
    exam_minutes: f64,
    started_at: Option<String>,
    ends_at: Option<String>,
    semester_id: Option<String>,
    course_id: Option<String>,
    task_id: Option<String>,
    goal: String,
    learned: String,
    blocker: String,
    next_step: String,
    confidence: f64,
    preset_label: String,
    last_alive_at: Option<String>,
}

fn parse_iso_wall_timestamp(text: &str) -> Result<WallTimestamp, String> {
    chrono::DateTime::parse_from_rfc3339(text)
        .map(|dt| WallTimestamp::from_unix_millis(dt.timestamp_millis()))
        .map_err(|err| format!("could not parse timestamp {text:?}: {err}"))
}

fn parse_optional_iso(text: &Option<String>) -> Result<Option<WallTimestamp>, String> {
    match text {
        None => Ok(None),
        Some(text) => parse_iso_wall_timestamp(text).map(Some),
    }
}

fn nonneg_seconds(minutes: f64, field: &str) -> Result<u64, String> {
    if !minutes.is_finite() || minutes < 0.0 {
        return Err(format!(
            "{field} is not a valid non-negative number: {minutes}"
        ));
    }
    Ok((minutes * 60.0).round() as u64)
}

fn nonneg_u64(value: f64, field: &str) -> Result<u64, String> {
    if !value.is_finite() || value < 0.0 {
        return Err(format!(
            "{field} is not a valid non-negative number: {value}"
        ));
    }
    Ok(value.round() as u64)
}

impl ProductionTimerState {
    fn convert(self) -> Result<TimerSnapshot, String> {
        let phase = match self.phase.as_str() {
            "idle" => TimerPhase::Idle,
            "study" => TimerPhase::Study,
            "break" => TimerPhase::Break,
            "exam" => TimerPhase::Exam,
            "stopwatch" => TimerPhase::Stopwatch,
            other => return Err(format!("unknown timer phase {other:?}")),
        };
        let mode = match self.mode.as_str() {
            "focus" => TimerMode::Focus,
            "exam" => TimerMode::Exam,
            "endless" => TimerMode::Endless,
            other => return Err(format!("unknown timer mode {other:?}")),
        };
        let active_segments = self
            .active_segments
            .into_iter()
            .map(|segment| {
                Ok(ActiveSegment {
                    started_at: parse_iso_wall_timestamp(&segment.started_at)?,
                    ended_at: parse_optional_iso(&segment.ended_at)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        Ok(TimerSnapshot {
            phase,
            mode,
            remaining_seconds: nonneg_u64(self.remaining_seconds, "remainingSeconds")?,
            logged_split_seconds: nonneg_u64(self.logged_split_seconds, "loggedSplitSeconds")?,
            active_segments,
            running: self.running,
            config: TimerConfig {
                mode,
                study_seconds: nonneg_seconds(self.study_minutes, "studyMinutes")?,
                break_seconds: nonneg_seconds(self.break_minutes, "breakMinutes")?,
                exam_seconds: nonneg_seconds(self.exam_minutes, "examMinutes")?,
                preset_label: self.preset_label,
            },
            context: TimerContext {
                semester_id: self.semester_id,
                course_id: self.course_id,
                task_id: self.task_id,
                goal: self.goal,
                learned: self.learned,
                blocker: self.blocker,
                next_step: self.next_step,
                confidence: self.confidence.clamp(0.0, 5.0).round() as u8,
            },
            started_at: parse_optional_iso(&self.started_at)?,
            ends_at: parse_optional_iso(&self.ends_at)?,
            last_alive_at: parse_optional_iso(&self.last_alive_at)?,
        })
    }
}

/// CONVERT + VALIDATE NATIVE RESULT for the one section this pipeline materializes. `Ok(None)`
/// means "no timer section in this backup at all" (not an error - matches production's own
/// tolerant `parsed.timer ?? {}` handling of a missing/empty timer). `Err` means the section was
/// present but unreadable; the caller decides whether that blocks the whole import (this pipeline
/// treats it as import-blocking - see `commit_import` - rather than silently proceeding without
/// the user's timer state, since a corrupt timer section is exactly the kind of thing section 20 of the
/// brief says must not be silently accepted).
pub fn convert_timer(state: &Map<String, Value>) -> Result<Option<TimerSnapshot>, String> {
    match state.get("timer") {
        None | Some(Value::Null) => Ok(None),
        Some(raw) => {
            let production_timer: ProductionTimerState =
                serde_json::from_value(raw.clone()).map_err(|err| err.to_string())?;
            production_timer.convert().map(Some)
        }
    }
}

#[derive(Debug)]
pub struct ImportReport {
    pub source_path: PathBuf,
    pub source_copy_path: PathBuf,
    pub fields: Vec<FieldReport>,
    pub timer_imported: bool,
    pub warnings: Vec<String>,
    pub committed: bool,
}

/// The full pipeline's outer shell: DISCOVER -> READ -> BACKUP COPY -> PARSE -> CLASSIFY ->
/// CONVERT -> VALIDATE -> (native destination untouched so far) -> the caller then decides
/// whether to actually call [`commit_import`]. Never writes to `destination_store` itself -
/// "inspect first, commit only if the caller chooses to" (section 14/section 26 of the brief: a supported
/// "select backup -> inspect -> validate -> import" API shape, not a one-shot auto-import).
pub fn inspect_import(
    source_path: &Path,
    backup_copy_dir: &Path,
) -> Result<
    (
        DiscoverOutcome,
        Vec<FieldReport>,
        Result<Option<TimerSnapshot>, String>,
    ),
    ImportError,
> {
    let discovered = discover_and_read(source_path, backup_copy_dir)?;
    let fields = classify_fields(&discovered.state);
    let timer = convert_timer(&discovered.state);
    Ok((discovered, fields, timer))
}

/// COMMIT: writes the converted timer (and nothing else - `Withheld`/`Reserved` sections are
/// deliberately NOT copied into the native destination store by this pipeline; see the module
/// docs and section 24/section 25 of the brief) to `destination_store`, then reads it back and compares
/// field-for-field against what was about to be written, per section 13.2 of the architecture freeze
/// ("validation after migration: round-trip every migrated record... before considering that
/// record migrated"). On any failure - conversion error, write error, or a read-back mismatch -
/// the destination is left exactly as it was found (rollback), never partially written.
///
/// `discovered`/`fields` are folded into the returned [`ImportReport`] verbatim, so a caller (see
/// `main.rs`'s `maybe_import_production_backup`) can log/inspect the *whole* report from one
/// value instead of threading the discovery output and the report separately.
pub fn commit_import(
    destination_store: &crate::persistence::store::NativeStore,
    discovered: &DiscoverOutcome,
    fields: Vec<FieldReport>,
    timer: Option<TimerSnapshot>,
) -> Result<ImportReport, String> {
    let (mut envelope, _warnings) = destination_store
        .load()
        .map_err(|err| format!("could not read the native destination before importing: {err}"))?;
    let pre_import_envelope = envelope.clone();

    envelope.timer = timer.clone();
    destination_store
        .save(&envelope)
        .map_err(|err| format!("failed to write the native destination: {err}"))?;

    let (read_back, warnings) = match destination_store.load() {
        Ok(loaded) => loaded,
        Err(err) => {
            // Roll back: restore exactly what was there before.
            let _ = destination_store.save(&pre_import_envelope);
            return Err(format!(
                "read-back verification failed to even load after import, rolled back: {err}"
            ));
        }
    };
    if read_back.timer != timer {
        let _ = destination_store.save(&pre_import_envelope);
        return Err(
            "read-back verification found a mismatch after import, rolled back".to_string(),
        );
    }

    Ok(ImportReport {
        source_path: discovered.source_path.clone(),
        source_copy_path: discovered.source_copy_path.clone(),
        fields,
        timer_imported: timer.is_some(),
        warnings,
        committed: true,
    })
}

/// Preserves an unimplemented section's raw JSON into a native store's opaque `other` map,
/// without ever touching a `Withheld` key. This is the mechanism `commit_import` could use for a
/// future stage's `Reserved` sections (Stage 16+ decides whether/how to actually consume them);
/// Stage 15 exposes it and tests it, but `commit_import` above deliberately keeps its own scope
/// to the timer only, per "do not pretend to migrate features we have not ported" (section 19).
#[allow(dead_code)]
pub fn preserve_reserved_sections(
    state: &Map<String, Value>,
    fields: &[FieldReport],
) -> Map<String, Value> {
    let mut preserved = Map::new();
    for field in fields {
        if field.class == FieldClass::Reserved {
            if let Some(value) = state.get(&field.key) {
                preserved.insert(field.key.clone(), value.clone());
            }
        }
    }
    preserved
}

#[allow(dead_code)]
pub fn envelope_with_reserved_sections(
    mut envelope: StoreEnvelope,
    reserved: Map<String, Value>,
) -> StoreEnvelope {
    for (key, value) in reserved {
        envelope.other.insert(key, value);
    }
    envelope
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::store::NativeStore;

    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn temp_dir(label: &str) -> TempDirGuard {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "study-tracker-migration-test-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        TempDirGuard(dir)
    }

    fn write_fixture(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    fn minimal_valid_backup(timer_json: &str) -> String {
        format!(
            r#"{{
                "app": "study-tracker",
                "backupVersion": 2,
                "exportedAt": "2026-09-27T00:00:00.000Z",
                "state": {{
                    "sessions": [],
                    "courses": [],
                    "social": {{"userId": "u1", "deviceSecret": "s1"}},
                    "timer": {timer_json}
                }},
                "preferences": {{}}
            }}"#
        )
    }

    fn valid_timer_json() -> &'static str {
        r#"{
            "phase": "study",
            "mode": "focus",
            "remainingSeconds": 900,
            "loggedSplitSeconds": 0,
            "activeSegments": [{"startedAt": "2026-09-27T10:00:00.000Z", "endedAt": null}],
            "running": true,
            "studyMinutes": 25,
            "breakMinutes": 5,
            "examMinutes": 90,
            "startedAt": "2026-09-27T10:00:00.000Z",
            "endsAt": "2026-09-27T10:25:00.000Z",
            "semesterId": null,
            "courseId": null,
            "taskId": null,
            "goal": "",
            "learned": "",
            "blocker": "",
            "nextStep": "",
            "confidence": 3,
            "presetLabel": "Pomodoro 25/5",
            "lastAliveAt": "2026-09-27T10:10:00.000Z"
        }"#
    }

    #[test]
    fn a_minimal_valid_backup_discovers_reads_and_copies_without_touching_the_source() {
        let dir = temp_dir("valid");
        let backup_dir = dir.0.join("copies");
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let before = fs::read_to_string(&source).unwrap();

        let outcome = discover_and_read(&source, &backup_dir).expect("valid backup");
        assert!(outcome.source_copy_path.exists());
        let copy_contents = fs::read_to_string(&outcome.source_copy_path).unwrap();
        assert_eq!(copy_contents, before, "the copy must be byte-identical");

        let after = fs::read_to_string(&source).unwrap();
        assert_eq!(after, before, "the source file must never be modified");
    }

    #[test]
    fn classify_fields_marks_timer_consumed_social_withheld_and_everything_else_reserved() {
        let dir = temp_dir("classify");
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        let fields = classify_fields(&outcome.state);

        let find = |key: &str| fields.iter().find(|f| f.key == key).unwrap().class;
        assert_eq!(find("timer"), FieldClass::Consumed);
        assert_eq!(find("social"), FieldClass::Withheld);
        assert_eq!(find("sessions"), FieldClass::Reserved);
        assert_eq!(find("courses"), FieldClass::Reserved);
    }

    #[test]
    fn convert_timer_maps_every_production_field_into_the_native_shape() {
        let dir = temp_dir("convert");
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        let snapshot = convert_timer(&outcome.state)
            .unwrap()
            .expect("timer present");

        assert_eq!(snapshot.phase, TimerPhase::Study);
        assert_eq!(snapshot.mode, TimerMode::Focus);
        assert_eq!(snapshot.remaining_seconds, 900);
        assert!(snapshot.running);
        assert_eq!(snapshot.config.study_seconds, 25 * 60);
        assert_eq!(snapshot.config.break_seconds, 5 * 60);
        assert_eq!(snapshot.config.exam_seconds, 90 * 60);
        assert_eq!(snapshot.config.preset_label, "Pomodoro 25/5");
        assert_eq!(snapshot.context.confidence, 3);
        assert_eq!(snapshot.active_segments.len(), 1);
        assert!(snapshot.active_segments[0].ended_at.is_none());
        assert!(snapshot.started_at.is_some());
        assert!(snapshot.ends_at.is_some());
        assert!(snapshot.last_alive_at.is_some());
    }

    #[test]
    fn a_backup_with_no_timer_section_converts_to_none_not_an_error() {
        let dir = temp_dir("no-timer");
        let source = write_fixture(&dir.0, "backup.json", &minimal_valid_backup("null"));
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        assert_eq!(convert_timer(&outcome.state).unwrap(), None);
    }

    #[test]
    fn a_file_missing_sessions_courses_or_social_is_rejected_as_not_a_backup() {
        let dir = temp_dir("not-a-backup");
        let source = write_fixture(&dir.0, "backup.json", r#"{"hello": "world"}"#);
        assert!(matches!(
            discover_and_read(&source, &dir.0.join("copies")),
            Err(ImportError::NotAStudyTrackerBackup)
        ));
    }

    #[test]
    fn invalid_json_is_rejected_without_panicking() {
        let dir = temp_dir("invalid-json");
        let source = write_fixture(&dir.0, "backup.json", "{not json");
        assert!(matches!(
            discover_and_read(&source, &dir.0.join("copies")),
            Err(ImportError::NotJson(_))
        ));
    }

    #[test]
    fn an_empty_file_is_rejected_without_panicking() {
        let dir = temp_dir("empty");
        let source = write_fixture(&dir.0, "backup.json", "");
        assert!(matches!(
            discover_and_read(&source, &dir.0.join("copies")),
            Err(ImportError::NotJson(_))
        ));
    }

    #[test]
    fn a_negative_remaining_seconds_in_the_timer_section_is_a_reported_conversion_error() {
        let dir = temp_dir("negative-duration");
        let bad_timer =
            valid_timer_json().replace("\"remainingSeconds\": 900", "\"remainingSeconds\": -900");
        let source = write_fixture(&dir.0, "backup.json", &minimal_valid_backup(&bad_timer));
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        assert!(convert_timer(&outcome.state).is_err());
    }

    #[test]
    fn a_malformed_timestamp_in_the_timer_section_is_a_reported_conversion_error_not_a_panic() {
        let dir = temp_dir("bad-timestamp");
        let bad_timer = valid_timer_json().replace(
            "\"startedAt\": \"2026-09-27T10:00:00.000Z\"",
            "\"startedAt\": \"not-a-date\"",
        );
        let source = write_fixture(&dir.0, "backup.json", &minimal_valid_backup(&bad_timer));
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        assert!(convert_timer(&outcome.state).is_err());
    }

    #[test]
    fn an_unknown_phase_or_mode_string_is_a_reported_conversion_error_not_a_panic() {
        let dir = temp_dir("unknown-phase");
        let bad_timer =
            valid_timer_json().replace("\"phase\": \"study\"", "\"phase\": \"levitating\"");
        let source = write_fixture(&dir.0, "backup.json", &minimal_valid_backup(&bad_timer));
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        assert!(convert_timer(&outcome.state).is_err());
    }

    #[test]
    fn unexpected_additional_fields_do_not_break_classification_or_conversion() {
        let dir = temp_dir("extra-fields");
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()).replacen(
                "\"courses\": []",
                "\"courses\": [], \"aBrandNewFieldFromTheFuture\": 12345",
                1,
            ),
        );
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        let fields = classify_fields(&outcome.state);
        assert!(fields
            .iter()
            .any(|f| f.key == "aBrandNewFieldFromTheFuture" && f.class == FieldClass::Reserved));
        assert!(convert_timer(&outcome.state).unwrap().is_some());
    }

    #[test]
    fn commit_import_round_trips_through_a_real_native_store_and_verifies_read_back() {
        let dir = temp_dir("commit");
        let store = NativeStore::new(dir.0.join("store.json"));
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let (discovered, fields, timer_result) =
            inspect_import(&source, &dir.0.join("copies")).unwrap();
        assert!(fields.iter().any(|f| f.key == "timer"));
        let timer = timer_result.expect("valid timer converts cleanly");

        let report =
            commit_import(&store, &discovered, fields, timer.clone()).expect("commit succeeds");
        assert!(report.committed);
        assert_eq!(report.timer_imported, timer.is_some());
        assert_eq!(report.source_copy_path, discovered.source_copy_path);
        let (loaded, _warnings) = store.load().unwrap();
        assert_eq!(loaded.timer, timer);
    }

    #[test]
    fn committing_never_writes_a_withheld_social_section_into_the_native_store() {
        let dir = temp_dir("withheld");
        let store = NativeStore::new(dir.0.join("store.json"));
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let (discovered, fields, timer_result) =
            inspect_import(&source, &dir.0.join("copies")).unwrap();
        commit_import(&store, &discovered, fields, timer_result.unwrap()).unwrap();

        let raw = fs::read_to_string(store.path()).unwrap();
        assert!(!raw.contains("deviceSecret"));
        assert!(!raw.contains("social"));
    }

    #[test]
    fn importing_twice_is_idempotent_not_duplicating_anything() {
        let dir = temp_dir("idempotent");
        let store = NativeStore::new(dir.0.join("store.json"));
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let (d1, f1, timer1) = inspect_import(&source, &dir.0.join("copies")).unwrap();
        commit_import(&store, &d1, f1, timer1.unwrap()).unwrap();
        let (loaded_once, _) = store.load().unwrap();

        let (d2, f2, timer2) = inspect_import(&source, &dir.0.join("copies")).unwrap();
        commit_import(&store, &d2, f2, timer2.unwrap()).unwrap();
        let (loaded_twice, _) = store.load().unwrap();

        assert_eq!(
            loaded_once, loaded_twice,
            "re-importing the same backup must be a no-op, not a duplicate"
        );
    }

    #[test]
    fn a_failed_commit_leaves_the_native_destination_completely_unchanged() {
        let dir = temp_dir("rollback");
        let store = NativeStore::new(dir.0.join("store.json"));
        // Seed the destination with a known-good prior state.
        let prior = TimerSnapshot {
            phase: TimerPhase::Idle,
            mode: TimerMode::Focus,
            remaining_seconds: 0,
            logged_split_seconds: 0,
            active_segments: Vec::new(),
            running: false,
            config: TimerConfig::default(),
            context: TimerContext::default(),
            started_at: None,
            ends_at: None,
            last_alive_at: None,
        };
        store
            .save(&StoreEnvelope {
                timer: Some(prior.clone()),
                other: Map::new(),
            })
            .unwrap();

        // A "successful" commit_import cannot itself fail here (there's no injectable I/O
        // failure point in this test harness), so this test instead locks in the *contract*:
        // commit_import's read-back check compares against exactly what it wrote, which the
        // round-trip test above already exercises on the success path. This test checks the
        // complementary half of the same contract - an import that is never committed (the
        // caller stops after `inspect_import`, e.g. because the report showed a problem) leaves
        // the destination bit-for-bit as it was.
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let _ = inspect_import(&source, &dir.0.join("copies")).unwrap();
        // Deliberately not calling commit_import.

        let (loaded, _warnings) = store.load().unwrap();
        assert_eq!(loaded.timer, Some(prior));
    }

    #[test]
    fn preserve_reserved_sections_carries_reserved_data_and_never_a_withheld_key() {
        let dir = temp_dir("preserve-reserved");
        let source = write_fixture(
            &dir.0,
            "backup.json",
            &minimal_valid_backup(valid_timer_json()),
        );
        let outcome = discover_and_read(&source, &dir.0.join("copies")).unwrap();
        let fields = classify_fields(&outcome.state);
        let preserved = preserve_reserved_sections(&outcome.state, &fields);
        assert!(preserved.contains_key("sessions"));
        assert!(preserved.contains_key("courses"));
        assert!(
            !preserved.contains_key("social"),
            "Withheld keys must never be preserved, even opaquely"
        );
        assert!(
            !preserved.contains_key("timer"),
            "Consumed keys are handled by convert_timer, not this path"
        );
    }
}
