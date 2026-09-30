//! The real [`TimerPersistencePort`] implementation (Stage 15), backed by [`NativeStore`]. This
//! is what `main.rs` wires into `TimerController` at real startup in place of Stage 14's
//! `NullPersistencePort`; nothing in `timer_controller.rs` changed to make this possible - it was
//! built against the trait boundary from the start.

use crate::persistence::store::{NativeStore, StoreEnvelope};
use crate::timer_controller::TimerPersistencePort;
use study_tracker_core::timer::TimerSnapshot;

pub struct FileTimerPersistencePort {
    store: NativeStore,
}

impl FileTimerPersistencePort {
    pub fn new(store: NativeStore) -> Self {
        Self { store }
    }
}

impl TimerPersistencePort for FileTimerPersistencePort {
    /// Read-modify-write, so a save from this port never clobbers a section (a future domain's,
    /// or one preserved opaquely from a production import - see `migration.rs`) that this build
    /// doesn't itself touch. A pre-existing corrupt store is treated as unreadable-and-replaced
    /// (there is nothing else safe to do - the new timer state the caller is actively trying to
    /// save must not be lost because of an unrelated, already-broken file), logged rather than
    /// propagated: `TimerPersistencePort::persist` has no error return (matching
    /// `NullPersistencePort`'s signature - the core's `PersistenceRequested` call site is not
    /// designed to react to a persistence failure, by design; see `timer_controller.rs`).
    fn persist(&mut self, snapshot: TimerSnapshot) {
        let mut envelope = match self.store.load() {
            Ok((envelope, warnings)) => {
                for warning in warnings {
                    log::warn!("timer persistence: {warning}");
                }
                envelope
            }
            Err(err) => {
                log::warn!(
                    "timer persistence: existing store unreadable, starting a fresh envelope: {err}"
                );
                StoreEnvelope::default()
            }
        };
        envelope.timer = Some(snapshot);
        if let Err(err) = self.store.save(&envelope) {
            log::warn!("timer persistence: failed to save: {err}");
        }
    }

    fn load(&self) -> Option<TimerSnapshot> {
        match self.store.load() {
            Ok((envelope, warnings)) => {
                for warning in warnings {
                    log::warn!("timer persistence: {warning}");
                }
                envelope.timer
            }
            Err(err) => {
                log::warn!("timer persistence: could not load store: {err}");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::timer::{TimerConfig, TimerContext, TimerMode, TimerPhase};

    struct TempDirGuard(std::path::PathBuf);
    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn temp_port() -> (TempDirGuard, FileTimerPersistencePort) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "study-tracker-timer-port-test-{}-{}",
            std::process::id(),
            unique
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        (
            TempDirGuard(dir),
            FileTimerPersistencePort::new(NativeStore::new(path)),
        )
    }

    fn sample() -> TimerSnapshot {
        TimerSnapshot {
            phase: TimerPhase::Study,
            mode: TimerMode::Focus,
            remaining_seconds: 42,
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
    fn load_before_any_persist_returns_none() {
        let (_dir, port) = temp_port();
        assert_eq!(port.load(), None);
    }

    #[test]
    fn persist_then_load_round_trips() {
        let (_dir, mut port) = temp_port();
        port.persist(sample());
        assert_eq!(port.load(), Some(sample()));
    }

    #[test]
    fn a_later_persist_overwrites_the_earlier_snapshot() {
        let (_dir, mut port) = temp_port();
        port.persist(sample());
        let mut second = sample();
        second.remaining_seconds = 7;
        second.running = false;
        port.persist(second.clone());
        assert_eq!(port.load(), Some(second));
    }

    #[test]
    fn persisting_never_discards_an_unrelated_section_already_in_the_file() {
        use serde_json::{Map, Value};
        let (_dir, mut port) = temp_port();
        // Simulate a section a future stage (or an import) already wrote, that this build does
        // not itself model.
        let mut other = Map::new();
        other.insert("sessions".to_string(), Value::Array(vec![Value::from(1)]));
        port.store
            .save(&StoreEnvelope {
                timer: None,
                academic: None,
                other,
            })
            .unwrap();

        port.persist(sample());

        let (envelope, _warnings) = port.store.load().unwrap();
        assert_eq!(envelope.timer, Some(sample()));
        assert_eq!(
            envelope.other.get("sessions"),
            Some(&Value::Array(vec![Value::from(1)]))
        );
    }
}
