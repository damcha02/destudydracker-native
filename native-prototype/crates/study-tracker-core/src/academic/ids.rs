//! Typed identifiers for the academic domain (Stage 16).
//!
//! Plain newtype wrappers around `String` - production's own IDs (`makeId()`, `desktop/src/lib/
//! storage.ts`) are opaque random strings (`crypto.randomUUID()`, falling back to a timestamp+
//! random string), never structured or parseable, so there is nothing more for a native type to
//! validate beyond "is a string, non-empty where required." These types exist purely so a
//! `SemesterId` and a `CourseId` cannot be silently swapped at a call site - not for any runtime
//! behavior difference from a plain `String`.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }
    };
}

typed_id!(SemesterId);
typed_id!(CourseId);
typed_id!(TaskId);
typed_id!(ExamId);
typed_id!(SessionId);
typed_id!(TimetableEventId);
typed_id!(HolidayId);
typed_id!(DailyTodoId);
typed_id!(CalendarEntryId);
