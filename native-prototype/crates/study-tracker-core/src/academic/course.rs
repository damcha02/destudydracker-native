//! Course (Stage 16). Production reference: `desktop/src/types.ts`'s `Course`, `App.tsx`'s
//! `addCourse`/`removeCourse` and the load-time `targetGrade` clamp in `storage.ts`.

use serde::{Deserialize, Serialize};

use super::ids::{CourseId, SemesterId};
use crate::timer::WallTimestamp;

/// Swiss-style 4.0-6.0 grade scale, matching production's own load-time clamp
/// (`targetGrade >= 4 && targetGrade <= 6 ? targetGrade : 4`, `storage.ts`). Production's
/// *creation*-time validation is actually looser (`Number(courseDraft.targetGrade) || 4`, no
/// range clamp) - this constant documents the stricter, load-bearing invariant this domain
/// enforces uniformly instead of reproducing that inconsistency.
pub const DEFAULT_TARGET_GRADE: f64 = 4.0;
pub const MIN_TARGET_GRADE: f64 = 4.0;
pub const MAX_TARGET_GRADE: f64 = 6.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Course {
    pub id: CourseId,
    pub semester_id: SemesterId,
    pub name: String,
    /// An opaque color token (production stores a CSS color string); never interpreted here.
    pub color: String,
    pub target_grade: f64,
    pub created_at: WallTimestamp,
    pub external_url: Option<String>,
}

impl Course {
    pub fn new(
        id: CourseId,
        semester_id: SemesterId,
        name: String,
        color: String,
        created_at: WallTimestamp,
    ) -> Self {
        Self {
            id,
            semester_id,
            name,
            color,
            target_grade: DEFAULT_TARGET_GRADE,
            created_at,
            external_url: None,
        }
    }
}

/// The one uniform validation rule this domain applies (see the constant docs above): out-of-
/// range or non-finite input becomes the default rather than being silently accepted or panicking.
pub fn clamp_target_grade(value: f64) -> f64 {
    if !value.is_finite() || value < MIN_TARGET_GRADE || value > MAX_TARGET_GRADE {
        DEFAULT_TARGET_GRADE
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_accepts_the_valid_range() {
        assert_eq!(clamp_target_grade(5.5), 5.5);
        assert_eq!(clamp_target_grade(4.0), 4.0);
        assert_eq!(clamp_target_grade(6.0), 6.0);
    }

    #[test]
    fn clamp_rejects_out_of_range_and_non_finite_values() {
        assert_eq!(clamp_target_grade(3.9), DEFAULT_TARGET_GRADE);
        assert_eq!(clamp_target_grade(6.1), DEFAULT_TARGET_GRADE);
        assert_eq!(clamp_target_grade(f64::NAN), DEFAULT_TARGET_GRADE);
        assert_eq!(clamp_target_grade(f64::INFINITY), DEFAULT_TARGET_GRADE);
    }
}
