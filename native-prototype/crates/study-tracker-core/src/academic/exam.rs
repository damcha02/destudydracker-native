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

/// Production v0.1.67's `ExamKind`: everything except `Session` is a one-off dated item inside the
/// semester; `Session` exams fall in the exam session after it. An exam with no kind is a
/// `Session` exam (what every exam was before kinds existed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExamKind {
    Midterm,
    Endterm,
    SemesterEnd,
    Project,
    Session,
}

impl ExamKind {
    /// Parses production's JSON spelling; anything else is "absent", like its normalizer.
    pub fn from_production(value: &str) -> Option<Self> {
        match value {
            "midterm" => Some(Self::Midterm),
            "endterm" => Some(Self::Endterm),
            "semester-end" => Some(Self::SemesterEnd),
            "project" => Some(Self::Project),
            "session" => Some(Self::Session),
            _ => None,
        }
    }
}

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
    /// Added in production v0.1.67 (wabi exam kinds). `None` = `Session`. Stores written before
    /// Stage 18 have no such field and load as `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ExamKind>,
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
            kind: None,
        }
    }
}
