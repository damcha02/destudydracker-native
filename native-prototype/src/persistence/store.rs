//! The native durable store (Stage 15): one small versioned JSON file under the app's per-user
//! data directory (`platform::paths::AppPaths::data_dir`), atomically written and tolerant to
//! read. See `docs/stage15-persistence-migration.md` for the full design writeup; this module is
//! deliberately small - production's own persistence needs (a single-user, single-machine,
//! moderate-sized JSON-shaped state blob, per the architecture freeze's section 14) do not justify a
//! database, and none is added here.
//!
//! Section design mirrors production's own "one blob, several independently-tolerant sections"
//! approach (`desktop/src/lib/storage.ts`'s `TIMER_KEY`/`SOCIAL_KEY`/`CORE_KEY` split), just as
//! one file with named top-level keys instead of several `localStorage` keys - a single JSON
//! file has no size/query pressure that would justify anything more. Only `timer` is modeled by
//! this build; every other top-level key found in the file (a section a future native build adds,
//! or one imported-but-not-yet-consumed from a production backup - see `migration.rs`) round-trips
//! through `other` untouched, so a save from this build never discards data it doesn't understand.

use serde_json::{Map, Value};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use study_tracker_core::timer::TimerSnapshot;

/// The current native storage schema version. Bump this, and add an explicit upgrade step, only
/// when an *existing* section's shape changes incompatibly - adding a brand-new top-level section
/// for a future domain does not require a bump, since `other` already preserves and round-trips
/// anything this build doesn't model.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// The whole native store, deserialized. `timer` is the only section this build actually reads
/// and writes; `other` is every other top-level key found in the file, verbatim, as unparsed
/// `serde_json::Value`s.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StoreEnvelope {
    pub timer: Option<TimerSnapshot>,
    pub other: Map<String, Value>,
}

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    /// The file exists but is not readable as a JSON object at all (empty, truncated, garbage,
    /// or a valid JSON value that isn't an object).
    Corrupt(String),
    /// The file is valid JSON but has no `schema_version` field.
    MissingSchemaVersion,
    /// `schema_version` is higher than `CURRENT_SCHEMA_VERSION` - refuse to touch the file rather
    /// than silently rewriting it in a shape an older/newer build might not round-trip correctly.
    UnsupportedFutureSchema(u32),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(err) => write!(f, "I/O error: {err}"),
            StoreError::Corrupt(reason) => write!(f, "store file is corrupt: {reason}"),
            StoreError::MissingSchemaVersion => {
                write!(f, "store file has no schema_version field")
            }
            StoreError::UnsupportedFutureSchema(version) => write!(
                f,
                "store file has schema_version {version}, which is newer than this build ({CURRENT_SCHEMA_VERSION}) understands"
            ),
        }
    }
}

impl std::error::Error for StoreError {}

/// Non-fatal, reported-not-silent problems found while loading (e.g. the `timer` section itself
/// was unreadable but the rest of the file parsed fine).
pub type LoadWarnings = Vec<String>;

/// A native store bound to one file path. Cheap to construct; does no I/O until `load`/`save`.
pub struct NativeStore {
    path: PathBuf,
}

impl NativeStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    #[allow(dead_code)] // used by this module's and timer_port's tests to inspect the file directly
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Loads the store. A missing file is not an error - it is a fresh profile - and returns an
    /// empty envelope with no warnings, exactly like production's first-launch `defaultState`.
    pub fn load(&self) -> Result<(StoreEnvelope, LoadWarnings), StoreError> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok((StoreEnvelope::default(), Vec::new()))
            }
            Err(err) => return Err(StoreError::Io(err)),
        };
        parse_envelope(&text)
    }

    /// Serializes and atomically replaces the store file: write the full contents to a sibling
    /// `.tmp` file, then `fs::rename` it over the real target. Verified empirically on this
    /// Windows machine (not assumed from POSIX semantics - see
    /// docs/stage15-persistence-migration.md, "Atomic writes and backups") that `std::fs::rename`
    /// DOES replace an existing destination file on Windows (the standard library calls
    /// `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING` internally, unlike a bare `MoveFileW`), so
    /// no separate delete-then-rename dance is needed. A process killed between the two steps
    /// leaves either the old file (rename never ran) or the new one (rename completed) - never a
    /// half-written target, since only the throwaway `.tmp` file could ever be left partially
    /// written.
    pub fn save(&self, envelope: &StoreEnvelope) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(StoreError::Io)?;
        }
        let json = serialize_envelope(envelope)
            .map_err(|err| StoreError::Corrupt(format!("failed to serialize: {err}")))?;
        let tmp_path = tmp_path_for(&self.path);
        fs::write(&tmp_path, json.as_bytes()).map_err(StoreError::Io)?;
        fs::rename(&tmp_path, &self.path).map_err(StoreError::Io)?;
        Ok(())
    }
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("store")
        .to_string();
    name.push_str(".tmp");
    path.with_file_name(name)
}

fn serialize_envelope(envelope: &StoreEnvelope) -> serde_json::Result<String> {
    let mut root = Map::new();
    root.insert(
        "schema_version".to_string(),
        Value::from(CURRENT_SCHEMA_VERSION),
    );
    if let Some(timer) = &envelope.timer {
        root.insert("timer".to_string(), serde_json::to_value(timer)?);
    }
    for (key, value) in &envelope.other {
        root.insert(key.clone(), value.clone());
    }
    serde_json::to_string_pretty(&Value::Object(root))
}

fn parse_envelope(text: &str) -> Result<(StoreEnvelope, LoadWarnings), StoreError> {
    if text.trim().is_empty() {
        return Err(StoreError::Corrupt("file is empty".to_string()));
    }
    let value: Value =
        serde_json::from_str(text).map_err(|err| StoreError::Corrupt(err.to_string()))?;
    let Value::Object(mut obj) = value else {
        return Err(StoreError::Corrupt(
            "root of the store file is not a JSON object".to_string(),
        ));
    };
    let schema_version = obj
        .remove("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or(StoreError::MissingSchemaVersion)? as u32;
    if schema_version > CURRENT_SCHEMA_VERSION {
        return Err(StoreError::UnsupportedFutureSchema(schema_version));
    }

    let mut warnings = Vec::new();
    let timer = match obj.remove("timer") {
        None | Some(Value::Null) => None,
        Some(raw) => match serde_json::from_value::<TimerSnapshot>(raw) {
            Ok(snapshot) => Some(snapshot),
            Err(err) => {
                warnings.push(format!(
                    "timer section could not be read, starting fresh: {err}"
                ));
                None
            }
        },
    };

    Ok((StoreEnvelope { timer, other: obj }, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::timer::{TimerConfig, TimerContext, TimerPhase};

    /// A directory under the OS temp root, unique per test, removed when the guard drops. Never
    /// a real user-data directory (acceptance criterion: "tests use isolated temp directories/
    /// fixtures, never real user-data directories" - see docs/stage15-persistence-migration.md).
    /// Hand-rolled instead of adding a `tempfile`/`tempdir` crate dependency purely for tests.
    struct TempDirGuard(PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp_store() -> (TempDirGuard, NativeStore) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "study-tracker-store-test-{}-{}-{}",
            std::process::id(),
            unique,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("store.json");
        (TempDirGuard(dir), NativeStore::new(path))
    }

    fn sample_snapshot() -> TimerSnapshot {
        TimerSnapshot {
            phase: TimerPhase::Study,
            mode: study_tracker_core::timer::TimerMode::Focus,
            remaining_seconds: 900,
            logged_split_seconds: 0,
            active_segments: Vec::new(),
            running: true,
            config: TimerConfig::default(),
            context: TimerContext::default(),
            started_at: None,
            ends_at: None,
            last_alive_at: None,
        }
    }

    #[test]
    fn a_missing_file_loads_as_an_empty_envelope_with_no_warnings() {
        let (_dir, store) = temp_store();
        let (envelope, warnings) = store.load().expect("missing file is not an error");
        assert_eq!(envelope, StoreEnvelope::default());
        assert!(warnings.is_empty());
    }

    #[test]
    fn save_then_load_round_trips_the_timer_section() {
        let (_dir, store) = temp_store();
        let envelope = StoreEnvelope {
            timer: Some(sample_snapshot()),
            other: Map::new(),
        };
        store.save(&envelope).expect("save succeeds");
        let (loaded, warnings) = store.load().expect("load succeeds");
        assert!(warnings.is_empty());
        assert_eq!(loaded.timer, Some(sample_snapshot()));
    }

    #[test]
    fn saved_file_carries_the_current_schema_version() {
        let (_dir, store) = temp_store();
        store.save(&StoreEnvelope::default()).unwrap();
        let raw = fs::read_to_string(store.path()).unwrap();
        let value: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            value["schema_version"].as_u64(),
            Some(CURRENT_SCHEMA_VERSION as u64)
        );
    }

    #[test]
    fn unknown_top_level_sections_round_trip_untouched() {
        let (_dir, store) = temp_store();
        let mut other = Map::new();
        other.insert(
            "sessions".to_string(),
            Value::Array(vec![Value::String("placeholder".into())]),
        );
        let envelope = StoreEnvelope {
            timer: Some(sample_snapshot()),
            other,
        };
        store.save(&envelope).unwrap();
        let (loaded, _warnings) = store.load().unwrap();
        assert_eq!(
            loaded.other.get("sessions"),
            Some(&Value::Array(vec![Value::String("placeholder".into())]))
        );
    }

    #[test]
    fn an_empty_file_is_reported_as_corrupt_not_a_panic() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), b"").unwrap();
        assert!(matches!(store.load(), Err(StoreError::Corrupt(_))));
    }

    #[test]
    fn truncated_json_is_reported_as_corrupt() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), br#"{"schema_version": 1, "timer": {"phase"#).unwrap();
        assert!(matches!(store.load(), Err(StoreError::Corrupt(_))));
    }

    #[test]
    fn invalid_json_is_reported_as_corrupt() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), b"not json at all").unwrap();
        assert!(matches!(store.load(), Err(StoreError::Corrupt(_))));
    }

    #[test]
    fn a_json_array_root_is_reported_as_corrupt_not_a_panic() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), b"[1, 2, 3]").unwrap();
        assert!(matches!(store.load(), Err(StoreError::Corrupt(_))));
    }

    #[test]
    fn missing_schema_version_is_reported_explicitly() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), br#"{"timer": null}"#).unwrap();
        assert!(matches!(
            store.load(),
            Err(StoreError::MissingSchemaVersion)
        ));
    }

    #[test]
    fn a_schema_version_newer_than_this_build_is_rejected_not_silently_accepted() {
        let (_dir, store) = temp_store();
        fs::write(store.path(), br#"{"schema_version": 999}"#).unwrap();
        assert!(matches!(
            store.load(),
            Err(StoreError::UnsupportedFutureSchema(999))
        ));
    }

    #[test]
    fn a_malformed_timer_section_is_dropped_with_a_warning_not_a_hard_failure() {
        let (_dir, store) = temp_store();
        // "phase" holds a value the TimerPhase enum has no variant for - a real-world stand-in
        // for a negative-duration/malformed-timestamp/unknown-enum production glitch. The rest
        // of the file (a hypothetical sibling section) must still come back.
        fs::write(
            store.path(),
            br#"{"schema_version": 1, "timer": {"phase": "not-a-real-phase"}, "sessions": [1,2]}"#,
        )
        .unwrap();
        let (envelope, warnings) = store.load().expect("a corrupt timer section is not fatal");
        assert_eq!(envelope.timer, None);
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            envelope.other.get("sessions"),
            Some(&Value::Array(vec![Value::from(1), Value::from(2)]))
        );
    }

    #[test]
    fn a_negative_remaining_seconds_is_treated_as_a_malformed_timer_section() {
        let (_dir, store) = temp_store();
        fs::write(
            store.path(),
            br#"{"schema_version": 1, "timer": {"phase": "Study", "mode": "Focus", "remaining_seconds": -5, "logged_split_seconds": 0, "active_segments": [], "running": true, "config": {"mode":"Focus","study_seconds":1500,"break_seconds":300,"exam_seconds":5400,"preset_label":"x"}, "context": {"semester_id":null,"course_id":null,"task_id":null,"goal":"","learned":"","blocker":"","next_step":"","confidence":0}, "started_at": null, "ends_at": null, "last_alive_at": null}}"#,
        )
        .unwrap();
        let (envelope, warnings) = store.load().expect("negative duration must not panic");
        assert_eq!(
            envelope.timer, None,
            "a negative duration cannot fit u64 and is rejected, not clamped silently"
        );
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn a_leftover_tmp_file_from_an_interrupted_write_does_not_affect_loading_the_real_file() {
        let (_dir, store) = temp_store();
        let good = StoreEnvelope {
            timer: Some(sample_snapshot()),
            other: Map::new(),
        };
        store.save(&good).unwrap();
        // Simulate a process that died mid-write on a *later* save: it managed to write the
        // sibling .tmp file but never got to the rename. The previously-saved real file must
        // still be exactly what it was.
        fs::write(tmp_path_for(store.path()), b"not even valid json").unwrap();
        let (loaded, warnings) = store.load().expect("the leftover .tmp file is never read");
        assert!(warnings.is_empty());
        assert_eq!(loaded.timer, Some(sample_snapshot()));
    }

    #[test]
    fn a_second_save_fully_replaces_the_first_not_merges_stale_fields() {
        let (_dir, store) = temp_store();
        let mut first_other = Map::new();
        first_other.insert("stale".to_string(), Value::from(true));
        store
            .save(&StoreEnvelope {
                timer: Some(sample_snapshot()),
                other: first_other,
            })
            .unwrap();
        store.save(&StoreEnvelope::default()).unwrap();
        let (loaded, _warnings) = store.load().unwrap();
        assert_eq!(loaded, StoreEnvelope::default());
    }
}
