//! The non-secret Social section of the native store (Stage 22a): what production persists in
//! its `study-tracker-desktop-v3-social` localStorage key *minus the credential*.
//!
//! ```json
//! "social": { "version": 1, "userId": "...", "profile": {...}, "sync": {...},
//!             "friends": {...}, "leaderboards": [...] }
//! ```
//!
//! - Durable: the profile (`friendCode`, display name, avatar, privacy toggles) and sync
//!   bookkeeping (`lastSyncedAt`, `lastSyncError`, `nextAutoSyncAt`).
//! - Cache: the last friends snapshot and leaderboard rows, shown while offline as production
//!   shows its cached `state.social` lists.
//! - Stage 22b (additive): the queued posts and queued deletions (`pendingFeedPosts`,
//!   `pendingFeedPostDeletions`: production's outbox, which must survive a restart), the
//!   verified-session anchor (`verifiedAnchor`: what offline reconciliation is measured from),
//!   the squad membership (squad, members, join requests: so a restart offline still shows the
//!   user's squad), the ids of the user's own published posts (so a restarted app never re-posts
//!   a session that is already on the feed) and of the comments already seen (the baseline of
//!   the "new comment on your post" notice). Ids only for the last two, bounded.
//! - Deliberately **not** here (Stage 22b, brief §50): feed rows, comments, chat messages,
//!   squad scoreboards, poll/reaction state and any downloaded image. They are server data that
//!   is fetched again when Social opens; production caches them in localStorage, native keeps
//!   them in memory only (documented deviation).
//! - Never here: `deviceSecret` (own file, `social_credentials`), Daily Skribbl (production keeps
//!   no local Skribbl state), presence (a server-side timestamp), or anything transient.
//! - `userId` binds the section to its identity: a section whose `userId` does not match the
//!   loaded credential is ignored (it belongs to another account), never mixed in.
//! - Additive, no schema bump; unknown keys inside the section survive a save; a malformed section
//!   loads as "no cache" with a warning and is replaced only by the next real change.
//! - Writes happen on real state changes only (sync result, a profile edit), never on viewing.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use study_tracker_core::social::feed::PendingPost;
use study_tracker_core::social::friends::FriendsSnapshot;
use study_tracker_core::social::leaderboard::{
    LeaderboardEntry, LeaderboardPeriod, LeaderboardScope,
};
use study_tracker_core::social::profile::{SocialProfile, SyncStatus};
use study_tracker_core::social::squad::{Squad, SquadJoinRequest};
use study_tracker_core::social::verified::VerifiedAnchor;
use study_tracker_core::social::{CommentId, PostId, UserId};

use crate::persistence::store::NativeStore;

pub const SECTION: &str = "social";
const VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedBoard {
    pub scope: LeaderboardScope,
    pub period: LeaderboardPeriod,
    pub entries: Vec<LeaderboardEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialRecord {
    pub user_id: UserId,
    pub profile: SocialProfile,
    #[serde(default)]
    pub sync: SyncStatus,
    #[serde(default)]
    pub friends: FriendsSnapshot,
    #[serde(default)]
    pub leaderboards: Vec<CachedBoard>,
    // ---- Stage 22b ----
    #[serde(default)]
    pub pending_posts: Vec<PendingPost>,
    #[serde(default)]
    pub pending_post_deletions: Vec<PostId>,
    #[serde(default)]
    pub verified_anchor: Option<VerifiedAnchor>,
    #[serde(default)]
    pub squad: Option<Squad>,
    #[serde(default)]
    pub squad_incoming: Vec<SquadJoinRequest>,
    #[serde(default)]
    pub squad_outgoing: Vec<SquadJoinRequest>,
    #[serde(default)]
    pub own_post_ids: Vec<PostId>,
    #[serde(default)]
    pub seen_comment_ids: Vec<CommentId>,
}

/// Bounds for the id lists (newest kept).
pub const MAX_OWN_POST_IDS: usize = 100;
pub const MAX_SEEN_COMMENT_IDS: usize = 1000;

const KNOWN: [&str; 14] = [
    "version",
    "userId",
    "profile",
    "sync",
    "friends",
    "leaderboards",
    "pendingPosts",
    "pendingPostDeletions",
    "verifiedAnchor",
    "squad",
    "squadIncoming",
    "squadOutgoing",
    "ownPostIds",
    "seenCommentIds",
];

impl SocialRecord {
    /// A fresh record for an identity (no cache, no outbox).
    pub fn new(user_id: UserId, profile: SocialProfile) -> Self {
        Self {
            user_id,
            profile,
            sync: SyncStatus::default(),
            friends: FriendsSnapshot::default(),
            leaderboards: Vec::new(),
            pending_posts: Vec::new(),
            pending_post_deletions: Vec::new(),
            verified_anchor: None,
            squad: None,
            squad_incoming: Vec::new(),
            squad_outgoing: Vec::new(),
            own_post_ids: Vec::new(),
            seen_comment_ids: Vec::new(),
        }
    }
}

/// Parses the section; `None` (with a log line) when absent or unreadable.
pub fn parse_section(section: Option<&Value>) -> Option<SocialRecord> {
    let value = section?;
    if value.get("version").and_then(Value::as_u64) != Some(VERSION) {
        log::warn!("social: store section has an unknown version; ignoring the cached profile");
        return None;
    }
    match serde_json::from_value::<SocialRecord>(value.clone()) {
        Ok(record) => Some(record),
        Err(_) => {
            log::warn!("social: store section is unreadable; starting without cached Social data");
            None
        }
    }
}

/// Writes the section, keeping keys of the existing section this build does not model.
pub fn write_section(record: &SocialRecord, existing: Option<&Value>) -> Value {
    let mut out = Map::new();
    if let Some(Value::Object(old)) = existing {
        for (k, v) in old {
            if !KNOWN.contains(&k.as_str()) {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    if let Ok(Value::Object(fields)) = serde_json::to_value(record) {
        out.extend(fields);
    }
    out.insert("version".into(), Value::from(VERSION));
    Value::Object(out)
}

/// Per-install Social preferences (Stage 22b), stored in the `preferences` section because
/// production keeps them outside the account: the opt-in telemetry switch
/// (`settings.telemetryEnabled`, default **off**), its install id
/// (`study-tracker-telemetry-install-id`) and the dismissed announcement ids
/// (`study-tracker-dismissed-announcements`). None of them is a credential.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SocialPrefs {
    pub telemetry_enabled: bool,
    pub install_id: Option<String>,
    pub dismissed_announcements: Vec<String>,
}

const PREFS: &str = "preferences";
const TELEMETRY: &str = "telemetry";
const DISMISSED: &str = "dismissedAnnouncements";

pub fn parse_prefs(section: Option<&Value>) -> SocialPrefs {
    let t = section.and_then(|s| s.get(TELEMETRY));
    SocialPrefs {
        telemetry_enabled: t
            .and_then(|t| t.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        install_id: t
            .and_then(|t| t.get("installId"))
            .and_then(Value::as_str)
            .map(str::to_string),
        dismissed_announcements: section
            .and_then(|s| s.get(DISMISSED))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn write_prefs(section: Option<Value>, prefs: &SocialPrefs) -> Value {
    let mut map = match section {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    let mut t = Map::new();
    t.insert("enabled".into(), Value::Bool(prefs.telemetry_enabled));
    if let Some(id) = &prefs.install_id {
        t.insert("installId".into(), Value::from(id.as_str()));
    }
    map.insert(TELEMETRY.into(), Value::Object(t));
    map.insert(
        DISMISSED.into(),
        Value::Array(
            prefs
                .dismissed_announcements
                .iter()
                .map(|s| Value::from(s.as_str()))
                .collect(),
        ),
    );
    Value::Object(map)
}

pub trait SocialPort {
    fn load(&self) -> Option<SocialRecord>;
    /// `None` removes the section (the identity was cleared).
    fn persist(&mut self, record: Option<&SocialRecord>);
    fn writes(&self) -> u64;
    fn load_prefs(&self) -> SocialPrefs;
    fn save_prefs(&mut self, prefs: &SocialPrefs);
}

pub struct FileSocialPort {
    store: NativeStore,
    writes: u64,
}

impl FileSocialPort {
    pub fn new(store: NativeStore) -> Self {
        Self { store, writes: 0 }
    }
}

impl SocialPort for FileSocialPort {
    fn load(&self) -> Option<SocialRecord> {
        match self.store.load() {
            Ok((envelope, _)) => parse_section(envelope.other.get(SECTION)),
            Err(err) => {
                log::warn!("social: could not load store: {err}");
                None
            }
        }
    }

    fn persist(&mut self, record: Option<&SocialRecord>) {
        let mut envelope = match self.store.load() {
            Ok((envelope, _)) => envelope,
            Err(err) => {
                // the same policy as every other section (the shared quarantine policy is deferred)
                log::warn!(
                    "social: existing store unreadable, not writing the Social section: {err}"
                );
                return;
            }
        };
        match record {
            Some(r) => {
                let value = write_section(r, envelope.other.get(SECTION));
                envelope.other.insert(SECTION.into(), value);
            }
            None => {
                envelope.other.remove(SECTION);
            }
        }
        if let Err(err) = self.store.save(&envelope) {
            log::warn!("social: failed to save: {err}");
            return;
        }
        self.writes += 1;
    }

    fn writes(&self) -> u64 {
        self.writes
    }

    fn load_prefs(&self) -> SocialPrefs {
        match self.store.load() {
            Ok((envelope, _)) => parse_prefs(envelope.other.get(PREFS)),
            Err(_) => SocialPrefs::default(),
        }
    }

    fn save_prefs(&mut self, prefs: &SocialPrefs) {
        let mut envelope = match self.store.load() {
            Ok((envelope, _)) => envelope,
            Err(err) => {
                log::warn!("social: existing store unreadable, not writing preferences: {err}");
                return;
            }
        };
        let value = write_prefs(envelope.other.remove(PREFS), prefs);
        envelope.other.insert(PREFS.into(), value);
        if let Err(err) = self.store.save(&envelope) {
            log::warn!("social: failed to save preferences: {err}");
            return;
        }
        self.writes += 1;
    }
}

/// For tests and headless stress runs.
#[derive(Default)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct MemorySocialPort {
    pub record: Option<SocialRecord>,
    pub writes: u64,
    pub prefs: SocialPrefs,
}

impl SocialPort for MemorySocialPort {
    fn load(&self) -> Option<SocialRecord> {
        self.record.clone()
    }

    fn persist(&mut self, record: Option<&SocialRecord>) {
        self.record = record.cloned();
        self.writes += 1;
    }

    fn writes(&self) -> u64 {
        self.writes
    }

    fn load_prefs(&self) -> SocialPrefs {
        self.prefs.clone()
    }

    fn save_prefs(&mut self, prefs: &SocialPrefs) {
        self.prefs = prefs.clone();
        self.writes += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::social::avatar::Avatar;
    use study_tracker_core::social::friends::Friend;
    use study_tracker_core::social::FriendCode;

    fn record() -> SocialRecord {
        SocialRecord {
            user_id: UserId::parse("synthetic-user-0001").unwrap(),
            profile: SocialProfile::new_default(FriendCode::parse("ABCD-WXYZ").unwrap()),
            sync: SyncStatus::default(),
            friends: FriendsSnapshot {
                friends: vec![Friend {
                    user_id: UserId::parse("f1").unwrap(),
                    display_name: "Zoë 学生".into(),
                    friend_code: FriendCode::parse("FRND-2345").unwrap(),
                    avatar: Avatar::default_for("Zoë"),
                    friends_since: None,
                    last_seen_at: None,
                }],
                ..Default::default()
            },
            leaderboards: vec![],
            ..SocialRecord::new(
                UserId::parse("synthetic-user-0001").unwrap(),
                SocialProfile::new_default(FriendCode::parse("ABCD-WXYZ").unwrap()),
            )
        }
    }

    #[test]
    fn round_trip_keeps_unknown_keys_and_never_contains_a_secret() {
        let existing = serde_json::json!({"version": 1, "futureField": [1, 2], "userId": "old"});
        let written = write_section(&record(), Some(&existing));
        assert_eq!(written["futureField"], serde_json::json!([1, 2]));
        assert_eq!(parse_section(Some(&written)), Some(record()));
        let text = written.to_string();
        assert!(!text.contains("deviceSecret") && !text.contains("device_secret"));
    }

    #[test]
    fn malformed_or_foreign_sections_load_as_nothing() {
        for bad in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"version": 2}),
            serde_json::json!({"version": 1, "userId": 5}),
            serde_json::json!({"version": 1, "userId": "u"}),
        ] {
            assert_eq!(parse_section(Some(&bad)), None, "{bad}");
        }
        assert_eq!(parse_section(None), None);
    }

    #[test]
    fn file_port_writes_only_its_section() {
        let dir = std::env::temp_dir().join(format!("st22-social-port-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store.json");
        std::fs::write(
            &path,
            r#"{"schema_version":1,"break_room":{"petRockPats":3}}"#,
        )
        .unwrap();
        let mut port = FileSocialPort::new(NativeStore::new(path.clone()));
        assert_eq!(port.load(), None);
        port.persist(Some(&record()));
        assert_eq!(port.load(), Some(record()));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("petRockPats"), "other sections untouched");
        port.persist(None);
        assert_eq!(port.load(), None);
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("petRockPats"));
        assert_eq!(port.writes(), 2);
        // an unreadable store is never overwritten by the Social port
        std::fs::write(&path, "garbage").unwrap();
        port.persist(Some(&record()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "garbage");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
