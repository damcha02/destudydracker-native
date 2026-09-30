//! The real [`AcademicPersistencePort`] implementation (Stage 16), backed by [`NativeStore`] -
//! exactly analogous to `timer_port.rs`'s `FileTimerPersistencePort`, reading/writing the store's
//! `academic` section instead of its `timer` section.

use crate::academic_controller::AcademicPersistencePort;
use crate::persistence::store::{NativeStore, StoreEnvelope};
use study_tracker_core::academic::AcademicState;

pub struct FileAcademicPersistencePort {
    store: NativeStore,
}

impl FileAcademicPersistencePort {
    pub fn new(store: NativeStore) -> Self {
        Self { store }
    }
}

impl AcademicPersistencePort for FileAcademicPersistencePort {
    /// Read-modify-write, same reasoning as `FileTimerPersistencePort::persist`: never clobber the
    /// `timer` section (or any other) this port doesn't itself own.
    fn persist(&mut self, state: &AcademicState) {
        let mut envelope = match self.store.load() {
            Ok((envelope, warnings)) => {
                for warning in warnings {
                    log::warn!("academic persistence: {warning}");
                }
                envelope
            }
            Err(err) => {
                log::warn!(
                    "academic persistence: existing store unreadable, starting a fresh envelope: {err}"
                );
                StoreEnvelope::default()
            }
        };
        envelope.academic = Some(state.clone());
        if let Err(err) = self.store.save(&envelope) {
            log::warn!("academic persistence: failed to save: {err}");
        }
    }

    fn load(&self) -> Option<AcademicState> {
        match self.store.load() {
            Ok((envelope, warnings)) => {
                for warning in warnings {
                    log::warn!("academic persistence: {warning}");
                }
                envelope.academic
            }
            Err(err) => {
                log::warn!("academic persistence: could not load store: {err}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::academic::{Course, CourseId, Semester, SemesterId};
    use study_tracker_core::timer::WallTimestamp;

    struct TempDirGuard(std::path::PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_port() -> (TempDirGuard, FileAcademicPersistencePort) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "study-tracker-academic-port-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (
            TempDirGuard(dir.clone()),
            FileAcademicPersistencePort::new(NativeStore::new(dir.join("store.json"))),
        )
    }

    fn sample() -> AcademicState {
        let mut state = AcademicState::new();
        state.add_semester(Semester::new(
            SemesterId::from("s1"),
            "Fall".to_string(),
            WallTimestamp::from_unix_millis(0),
        ));
        state.add_course(Course::new(
            CourseId::from("c1"),
            SemesterId::from("s1"),
            "Analysis II".to_string(),
            "blue".to_string(),
            WallTimestamp::from_unix_millis(0),
        ));
        state
    }

    #[test]
    fn load_before_any_persist_returns_none() {
        let (_dir, port) = temp_port();
        assert_eq!(port.load(), None);
    }

    #[test]
    fn persist_then_load_round_trips() {
        let (_dir, mut port) = temp_port();
        port.persist(&sample());
        assert_eq!(port.load(), Some(sample()));
    }

    #[test]
    fn persisting_academic_data_never_discards_an_existing_timer_section() {
        use study_tracker_core::timer::{
            TimerConfig, TimerContext, TimerMode, TimerPhase, TimerSnapshot,
        };
        let (_dir, mut port) = temp_port();
        let timer_snapshot = TimerSnapshot {
            phase: TimerPhase::Study,
            mode: TimerMode::Focus,
            remaining_seconds: 900,
            logged_split_seconds: 0,
            active_segments: Vec::new(),
            running: true,
            config: TimerConfig::default(),
            context: TimerContext::default(),
            started_at: None,
            ends_at: None,
            last_alive_at: None,
        };
        port.store
            .save(&StoreEnvelope {
                timer: Some(timer_snapshot.clone()),
                academic: None,
                other: Default::default(),
            })
            .unwrap();

        port.persist(&sample());

        let (envelope, _warnings) = port.store.load().unwrap();
        assert_eq!(envelope.timer, Some(timer_snapshot));
        assert_eq!(envelope.academic, Some(sample()));
    }
}
