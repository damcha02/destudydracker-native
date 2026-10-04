//! The player's own profile (Stage 22a): the non-secret part of production's `SocialState`
//! (`friendCode`, `displayName`, `avatar`, `isPrivate`, `autoPostSessions`,
//! `showHoursToFriends`) plus its sync bookkeeping (`lastSyncedAt`, `lastSyncError`,
//! `nextAutoSyncAt`).

use serde::{Deserialize, Serialize};

use super::avatar::Avatar;
use super::ids::FriendCode;
use super::limits::MAX_DISPLAY_NAME_UTF16;
use super::time::SocialTimestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SocialProfile {
    pub friend_code: FriendCode,
    pub display_name: String,
    pub avatar: Avatar,
    pub is_private: bool,
    pub auto_post_sessions: bool,
    pub show_hours_to_friends: bool,
}

impl SocialProfile {
    /// `makeDefaultSocialState()`'s profile for a freshly minted code: "Student " + the code's
    /// last four characters (dash removed), a classic letter avatar, public, auto-post off,
    /// hours shown to friends.
    pub fn new_default(friend_code: FriendCode) -> Self {
        let suffix: String = {
            let chars: Vec<char> = friend_code.as_str().chars().collect();
            chars[chars.len().saturating_sub(4)..]
                .iter()
                .filter(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
                .collect()
        };
        let display_name = format!("Student {suffix}");
        Self {
            avatar: Avatar::default_for(&display_name),
            friend_code,
            display_name,
            is_private: false,
            auto_post_sessions: false,
            show_hours_to_friends: true,
        }
    }
}

/// Why a display-name edit was refused (`saveSocialName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRejection {
    Empty,
}

impl NameRejection {
    pub fn message(self) -> &'static str {
        "Give your player a name first."
    }
}

/// `socialNameDraft.trim().slice(0, 48)`: trimmed, at most 48 UTF-16 code units (an astral
/// character that would be split at the boundary is dropped whole instead of leaving half a
/// surrogate pair, which production would send as a lone surrogate).
pub fn clean_display_name(draft: &str) -> Result<String, NameRejection> {
    let trimmed = draft.trim();
    let mut units = 0usize;
    let mut out = String::new();
    for c in trimmed.chars() {
        units += c.len_utf16();
        if units > MAX_DISPLAY_NAME_UTF16 {
            break;
        }
        out.push(c);
    }
    if out.is_empty() {
        Err(NameRejection::Empty)
    } else {
        Ok(out)
    }
}

/// Sync bookkeeping kept with the profile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncStatus {
    pub last_synced_at: Option<SocialTimestamp>,
    /// A short, user-facing message (never a raw server body; see the application's error model).
    pub last_sync_error: Option<String>,
    pub next_auto_sync_at: Option<SocialTimestamp>,
}

impl SyncStatus {
    /// `shouldAutoSyncSocial`: no schedule yet, or the scheduled time has come.
    pub fn should_auto_sync(&self, now: crate::timer::WallTimestamp) -> bool {
        self.next_auto_sync_at
            .map_or(true, |at| now.unix_millis >= at.0)
    }
}
