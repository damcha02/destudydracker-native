//! StudySession (Stage 16). Production reference: `desktop/src/types.ts`'s `StudySession`/
//! `SessionKind`, `App.tsx`'s `buildSessionFromTimer`/`buildSessionsFromTimerRange`.
//!
//! The record itself is a plain, storage-agnostic value type - `study-tracker-core` still knows
//! nothing about the Timer's *creation* of one (no dependency in either direction between
//! `academic` and `timer`). The actual Timer-completion -> `StudySession` bridge, including
//! production's local-midnight day-splitting behavior, lives in the application layer
//! (`native-prototype/src/session_service.rs`) because it depends on the OS's local timezone,
//! which `study-tracker-core` must never depend on (see that module's docs for why, and for the
//! splitting algorithm itself).

use serde::{Deserialize, Serialize};

use super::ids::{CourseId, SemesterId, SessionId, TaskId};
use crate::timer::WallTimestamp;

/// Matches production's `SessionKind` (`"study" | "break" | "exam"`) exactly. Production's own
/// `buildSessionFromTimer` only ever produces `"study"` or `"exam"` (`timer.phase === "exam" ?
/// "exam" : "study"`, i.e. Break phase never reaches session creation at all - see
/// `session_service.rs`); the `Break` variant exists because the *type* allows it and a future
/// stage's data (or an imported production backup) could contain one, not because this domain
/// ever constructs one itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionKind {
    Study,
    Break,
    Exam,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudySession {
    pub id: SessionId,
    pub semester_id: Option<SemesterId>,
    pub course_id: Option<CourseId>,
    pub task_id: Option<TaskId>,
    pub kind: SessionKind,
    pub goal: String,
    pub learned: String,
    pub blocker: String,
    pub next_step: String,
    pub confidence: u8,
    pub started_at: WallTimestamp,
    pub ended_at: WallTimestamp,
    pub minutes: u32,
    pub preset_label: String,
}
