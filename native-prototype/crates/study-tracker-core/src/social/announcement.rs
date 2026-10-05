//! App announcements (Stage 22b): production's `GET /announcements/current` banner and the
//! owner's "Notify users below <version>" (`POST /announcements/update-notice`).

use serde::{Deserialize, Serialize};

/// `ANNOUNCEMENT_POLL_INTERVAL_MS`.
pub const POLL_INTERVAL_MS: i64 = 2 * 60 * 1000;
/// `saveDismissedAnnouncementIds`: the newest 100 dismissed ids are kept.
pub const MAX_DISMISSED: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// The dismissed set (`study-tracker-dismissed-announcements`), oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Dismissed(Vec<String>);

impl Dismissed {
    pub fn from_ids(ids: Vec<String>) -> Self {
        let mut d = Self(Vec::new());
        for id in ids {
            d.add(&id);
        }
        d
    }

    pub fn contains(&self, id: &str) -> bool {
        self.0.iter().any(|x| x == id)
    }

    /// Adds `id` (a `Set`, so a repeat does not move it) and keeps the newest 100.
    pub fn add(&mut self, id: &str) {
        if !self.contains(id) {
            self.0.push(id.to_string());
        }
        if self.0.len() > MAX_DISMISSED {
            let extra = self.0.len() - MAX_DISMISSED;
            self.0.drain(..extra);
        }
    }

    pub fn ids(&self) -> &[String] {
        &self.0
    }
}

/// `isValidAppVersion`: `^\d+(?:\.\d+)*$` after trimming.
pub fn is_valid_app_version(version: &str) -> bool {
    let v = version.trim();
    !v.is_empty()
        && v.split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}
