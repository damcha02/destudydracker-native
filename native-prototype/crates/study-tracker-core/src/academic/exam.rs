//! Exam (Stage 16) - academic exam records (a date/weight/preparedness tracked in the Planner),
//! **not** `TimerMode::Exam`/`TimerPhase::Exam` (a Timer preset for a timed study/exam block).
//! The two are unrelated production concepts that happen to share the English word "exam";
//! kept explicitly distinct here, matching production's own separation (`Exam` in `types.ts` has
//! no relationship to `TimerState` beyond a session's `kind: "exam"` tag being set when a Timer
//! block happened to run in Exam *mode* - see `session.rs`).
//!
//! Production reference: `desktop/src/types.ts`'s `Exam`, `App.tsx`'s `addExam`/`removeExam`/
//! `saveExamEdit`.

use serde::{Deserialize, Serialize};

use super::date::LocalDate;
use super::ids::{CourseId, ExamId, SemesterId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exam {
    pub id: ExamId,
    pub semester_id: SemesterId,
    pub course_id: CourseId,
    pub title: String,
    /// A local calendar date (see `date.rs`) - production's `examDate` carries no time-of-day.
    pub exam_date: LocalDate,
    pub weight: f64,
    pub preparedness: f64,
    pub location: String,
}

impl Exam {
    pub fn new(
        id: ExamId,
        semester_id: SemesterId,
        course_id: CourseId,
        title: String,
        exam_date: LocalDate,
    ) -> Self {
        Self {
            id,
            semester_id,
            course_id,
            title,
            exam_date,
            weight: 0.0,
            preparedness: 0.0,
            location: String::new(),
        }
    }
}
