//! Semester (Stage 16). Production reference: `desktop/src/types.ts`'s `Semester`/`SemesterPhase`,
//! `desktop/src/App.tsx`'s `addSemester`/`updateSemester`/`removeSemester`.

use serde::{Deserialize, Serialize};

use super::date::LocalDate;
use super::ids::SemesterId;
use crate::timer::WallTimestamp;

/// Matches production's `SemesterPhase` exactly (`"semester" | "exam-prep"`) - a manually-toggled
/// label for whether a semester's coursework or its exam period is currently the focus. Not
/// derived from dates; production never infers it from `startDate`/`endDate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemesterPhase {
    Semester,
    ExamPrep,
}

impl Default for SemesterPhase {
    fn default() -> Self {
        Self::Semester
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Semester {
    pub id: SemesterId,
    pub name: String,
    pub created_at: WallTimestamp,
    /// Optional academic-calendar dates - a local *day*, never a timezone-attached instant (see
    /// `date.rs`). Production allows either to be null/absent (a semester can exist before its
    /// dates are known) and allows them to be set in either order or even to overlap another
    /// semester's range - it never validates this at creation, so neither does this type.
    pub start_date: Option<LocalDate>,
    pub end_date: Option<LocalDate>,
    pub phase: SemesterPhase,
    pub archived: bool,
    pub archived_at: Option<WallTimestamp>,
}

impl Semester {
    /// Mirrors `addSemester`'s defaults exactly: a fresh semester has no dates, is in the
    /// ordinary "semester" phase, and is not archived.
    pub fn new(id: SemesterId, name: String, created_at: WallTimestamp) -> Self {
        Self {
            id,
            name,
            created_at,
            start_date: None,
            end_date: None,
            phase: SemesterPhase::Semester,
            archived: false,
            archived_at: None,
        }
    }
}
