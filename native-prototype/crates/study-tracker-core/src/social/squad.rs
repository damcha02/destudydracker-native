//! Squads (Stage 22b): membership, roles and their permissions, join requests, squad chat, the
//! squad search results and the Squad Arena scoreboard, as production's `App.tsx` /
//! `SocialScreen.tsx` model them and the Worker (`getSquadSnapshot`, `getSquadDetails`,
//! `getSquadScoreLeaderboard`, `handleSquad*`) enforces them.
//!
//! The client-side permission rules below only decide which controls are *shown*; the Worker
//! re-checks every one (`canManageRequests`, `canChangeRole`, `canKick`, leader-only settings)
//! and a refusal is reported like any other server error.

use serde::{Deserialize, Serialize};

use super::avatar::Avatar;
use super::ids::{FriendCode, MessageId, RequestId, SquadId, UserId};
use super::time::SocialTimestamp;

/// `MAX_SQUAD_MEMBERS` (Worker) - "Squads hold up to 4 players."
pub const MAX_SQUAD_MEMBERS: u64 = 4;
/// `MAX_SQUAD_NAME_LENGTH` / the inputs' `maxLength={48}` (UTF-16 units).
pub const MAX_SQUAD_NAME_UTF16: usize = 48;
/// `MAX_SQUAD_MESSAGE_LENGTH` / the chat input's `maxLength={500}`.
pub const MAX_SQUAD_MESSAGE_UTF16: usize = 500;
/// `getSquadSnapshot` returns the newest 60 messages, oldest first.
pub const SERVER_CHAT_LIMIT: usize = 60;
/// The squad scoreboard client cache (`SQUAD_SCOREBOARD_CACHE_TTL_MS`).
pub const SCOREBOARD_CACHE_MS: i64 = 60 * 1000;
/// `pickSquadSuggestions`: four random squads from the pool.
pub const SUGGESTIONS: usize = 4;

/// The season labels production hard-codes (`SQUAD_SEASON_NAME`, `SQUAD_SEASON_RANGE_LABEL`,
/// `SQUAD_TRACKING_START_LABEL`). The server's own season dates decide the points.
pub const SEASON_NAME: &str = "Frostbound Semester 26";
pub const SEASON_RANGE_LABEL: &str = "15.09.2026 - 18.12.2026";
pub const TRACKING_START_LABEL: &str = "29.07.2026";

/// Defensive caps on server data.
pub const MAX_MEMBERS_RECEIVED: usize = 16;
pub const MAX_MESSAGES_RECEIVED: usize = 200;
pub const MAX_SQUAD_REQUESTS: usize = 200;
pub const MAX_SCOREBOARD_ROWS: usize = 200;
pub const MAX_SEARCH_ROWS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SquadRole {
    Leader,
    CoLeader,
    Elder,
    Member,
}

impl SquadRole {
    /// `squadRoles`, highest first.
    pub const ALL: [Self; 4] = [Self::Leader, Self::CoLeader, Self::Elder, Self::Member];

    /// The wire value. An unknown value reads as `member` (the Worker's own `cleanSquadRole`
    /// default), which is also the least-privileged choice for the controls shown.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "leader" => Self::Leader,
            "co_leader" => Self::CoLeader,
            "elder" => Self::Elder,
            _ => Self::Member,
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::Leader => "leader",
            Self::CoLeader => "co_leader",
            Self::Elder => "elder",
            Self::Member => "member",
        }
    }

    /// `squadRoleLabels`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Leader => "Leader",
            Self::CoLeader => "Co-leader",
            Self::Elder => "Elder",
            Self::Member => "Member",
        }
    }

    /// `squadRoleRank`.
    pub fn rank(self) -> u8 {
        match self {
            Self::Leader => 4,
            Self::CoLeader => 3,
            Self::Elder => 2,
            Self::Member => 1,
        }
    }
}

/// `canManageSquadRequests`: anyone above member sees the join requests (and the server only
/// sends them to those roles).
pub fn can_manage_requests(role: Option<SquadRole>) -> bool {
    role.is_some_and(|r| r.rank() > SquadRole::Member.rank())
}

/// `canEditSquadMember` (client).
fn can_edit_member(actor: SquadRole, target: SquadRole) -> bool {
    match actor {
        SquadRole::Leader => target != SquadRole::Leader,
        SquadRole::CoLeader => target.rank() < SquadRole::CoLeader.rank(),
        _ => false,
    }
}

/// `canKickSquadMember` (client; the Worker's `canKick` is the same rule).
pub fn can_kick(actor: SquadRole, target: SquadRole) -> bool {
    match actor {
        SquadRole::Leader => target != SquadRole::Leader,
        SquadRole::CoLeader => target.rank() < SquadRole::CoLeader.rank(),
        SquadRole::Elder => target == SquadRole::Member,
        SquadRole::Member => false,
    }
}

/// `getAssignableSquadRoles`: the "Make ..." buttons. Never "leader"; a co-leader can only hand
/// out elder or member.
pub fn assignable_roles(actor: SquadRole, target: SquadRole) -> Vec<SquadRole> {
    if !can_edit_member(actor, target) {
        return Vec::new();
    }
    SquadRole::ALL
        .into_iter()
        .filter(|r| {
            *r != SquadRole::Leader
                && (actor == SquadRole::Leader || r.rank() < SquadRole::CoLeader.rank())
        })
        .collect()
}

/// The Worker's `canChangeRole` (what it will accept), for tests and stale-state handling.
pub fn server_can_change_role(actor: SquadRole, target: SquadRole, next: SquadRole) -> bool {
    match actor {
        SquadRole::Leader => next != SquadRole::Leader,
        SquadRole::CoLeader => {
            target.rank() < SquadRole::CoLeader.rank() && next.rank() < SquadRole::CoLeader.rank()
        }
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadMember {
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: Option<FriendCode>,
    pub avatar: Avatar,
    pub role: SquadRole,
    pub joined_at: Option<SocialTimestamp>,
    pub last_seen_at: Option<SocialTimestamp>,
    pub minutes: u64,
    pub sessions: u64,
    pub is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squad {
    pub id: SquadId,
    pub name: String,
    pub is_private: bool,
    pub created_at: Option<SocialTimestamp>,
    pub total_minutes: u64,
    pub total_sessions: u64,
    pub member_count: u64,
    pub my_role: SquadRole,
    pub members: Vec<SquadMember>,
}

impl Squad {
    /// `isLastSquadMember`: leaving deletes the squad (confirmation first).
    pub fn is_last_member(&self) -> bool {
        self.member_count <= 1
    }

    pub fn member_ids(&self) -> Vec<UserId> {
        self.members.iter().map(|m| m.user_id.clone()).collect()
    }
}

/// A pending join request: incoming (to a squad the user manages: who asked) or outgoing (the
/// user's own: which squad).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadJoinRequest {
    pub id: RequestId,
    pub squad_id: SquadId,
    pub squad_name: Option<String>,
    pub user_id: Option<UserId>,
    pub display_name: Option<String>,
    pub friend_code: Option<FriendCode>,
    pub avatar: Option<Avatar>,
    pub created_at: Option<SocialTimestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadMessage {
    pub id: MessageId,
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: Option<FriendCode>,
    pub avatar: Avatar,
    pub role: SquadRole,
    pub body: String,
    pub created_at: Option<SocialTimestamp>,
    pub is_self: bool,
}

/// The squad part of a social snapshot (`getSquadSnapshot`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SquadSnapshot {
    pub squad: Option<Squad>,
    pub incoming: Vec<SquadJoinRequest>,
    pub outgoing: Vec<SquadJoinRequest>,
    pub messages: Vec<SquadMessage>,
}

/// `SocialSquadScorePeriod`, in production's chip order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SquadScorePeriod {
    Daily,
    Season,
    Overall,
}

impl Default for SquadScorePeriod {
    /// `useState<SocialSquadScorePeriod>("season")`.
    fn default() -> Self {
        Self::Season
    }
}

impl SquadScorePeriod {
    pub const ALL: [Self; 3] = [Self::Daily, Self::Season, Self::Overall];

    pub fn wire(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Season => "season",
            Self::Overall => "overall",
        }
    }

    /// The chips and the arena subtitle.
    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily",
            Self::Season => "Seasonal Points",
            Self::Overall => "Overall Points",
        }
    }

    /// The arena subtitle (`socialArenaSubtitle` for the squad scope).
    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Daily => "Daily Sprint",
            Self::Season => "Seasonal Points",
            Self::Overall => "Overall Points",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SquadScoreEntry {
    pub squad_id: SquadId,
    pub squad_name: String,
    pub is_private: bool,
    pub member_count: u64,
    pub total_minutes: u64,
    pub total_sessions: u64,
    pub average_minutes: f64,
    pub rank: u32,
    pub points: i64,
    pub scored_days: Option<u64>,
}

/// What the viewer can do with a squad (`action` from `/squads/search` and `/squads/details`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SquadAction {
    Join,
    Request,
    Pending,
    Full,
    Unavailable,
    Current,
}

impl SquadAction {
    /// An unknown value is `Unavailable`: no join button for a state the client does not know.
    pub fn parse(raw: &str) -> Self {
        match raw {
            "join" => Self::Join,
            "request" => Self::Request,
            "pending" => Self::Pending,
            "full" => Self::Full,
            "current" => Self::Current,
            _ => Self::Unavailable,
        }
    }

    pub fn can_join_or_request(self) -> bool {
        matches!(self, Self::Join | Self::Request)
    }

    /// The badge on a search card when there is no button.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Full => "Full",
            _ => "Unavailable",
        }
    }

    /// The badge in the squad details dialog.
    pub fn details_badge(self) -> &'static str {
        match self {
            Self::Current => "Your squad",
            Self::Pending => "Request pending",
            Self::Full => "Full",
            Self::Unavailable => "Unavailable",
            Self::Request => "Request to join",
            Self::Join => "Open to join",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquadSearchResult {
    pub id: SquadId,
    pub name: String,
    pub is_private: bool,
    pub member_count: u64,
    pub max_members: u64,
    pub total_minutes: u64,
    pub total_sessions: u64,
    pub action: SquadAction,
}

impl SquadSearchResult {
    /// After a successful request to a private squad production marks the card "pending".
    pub fn mark_pending(&mut self) {
        self.action = SquadAction::Pending;
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SquadDetails {
    pub id: SquadId,
    pub name: String,
    pub is_private: bool,
    pub total_minutes: u64,
    pub total_sessions: u64,
    pub member_count: u64,
    pub max_members: u64,
    pub previous_day_average_minutes: f64,
    pub action: SquadAction,
    pub members: Vec<SquadMember>,
}

/// `pickSquadSuggestions`: a Fisher-Yates shuffle of the pool, the first four. `random(i)` must
/// return an index in `0..=i` (the caller owns the RNG; the core never touches one).
pub fn pick_suggestions(
    pool: &[SquadSearchResult],
    mut random: impl FnMut(usize) -> usize,
) -> Vec<SquadSearchResult> {
    let mut pool = pool.to_vec();
    for i in (1..pool.len()).rev() {
        let j = random(i).min(i);
        pool.swap(i, j);
    }
    pool.truncate(SUGGESTIONS);
    pool
}

/// A squad name as typed (`squadNameDraft.trim()`), refused when empty ("Name your squad
/// first."), cut to the input's 48 units.
pub fn clean_squad_name(draft: &str) -> Option<String> {
    let name = draft.trim();
    (!name.is_empty()).then(|| super::ids::truncate_utf16(name, MAX_SQUAD_NAME_UTF16))
}

/// A chat message as typed (`squadChatDraft.trim()`): empty sends nothing; cut to 500 units.
pub fn clean_message(draft: &str) -> Option<String> {
    let body = draft.trim();
    (!body.is_empty()).then(|| super::ids::truncate_utf16(body, MAX_SQUAD_MESSAGE_UTF16))
}
