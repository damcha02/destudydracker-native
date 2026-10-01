//! Native persistence (Stage 15): the durable store, the real `TimerPersistencePort` adapter, and
//! the production-backup import pipeline. See `docs/stage15-persistence-migration.md` for the
//! full design writeup; `study-tracker-core` performs no filesystem I/O and knows nothing about
//! any of this (the frozen dependency-direction rule - see
//! `docs/stage12_5-architecture-freeze.md` section 5, section 13).

pub mod academic_port;
pub mod break_room_port;
pub mod migration;
pub mod migration_academic;
pub mod preferences_port;
pub mod store;
pub mod timer_port;

pub use academic_port::FileAcademicPersistencePort;
pub use preferences_port::{FilePreferencesPort, PreferencesController};
pub use store::NativeStore;
pub use timer_port::FileTimerPersistencePort;
