//! Leaderboards (Stage 22a).
//!
//! The Worker is authoritative for every row (`getLeaderboard`: `competitive_daily_stats` /
//! `competitive_user_totals`, the latter corrected by migration 0022). The client never recomputes
//! another user's totals; it only filters its cached rows to the scope, re-sorts and re-ranks them
//! (`getLeaderboardWithLocalSelf`) - which, despite its name, does not insert a local self row.

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use super::avatar::Avatar;
use super::friends::FriendsSnapshot;
use super::ids::{FriendCode, UserId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaderboardScope {
    Global,
    Friends,
    Squad,
}

impl LeaderboardScope {
    /// Production's button order: Friends, Squad, Global.
    pub const UI_ORDER: [Self; 3] = [Self::Friends, Self::Squad, Self::Global];

    pub fn wire(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Friends => "friends",
            Self::Squad => "squad",
        }
    }

    pub fn arena_label(self) -> &'static str {
        match self {
            Self::Global => "World Arena",
            Self::Squad => "Squad Arena",
            Self::Friends => "Friends Arena",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaderboardPeriod {
    Daily,
    Weekly,
    Overall,
}

impl LeaderboardPeriod {
    pub const ALL: [Self; 3] = [Self::Daily, Self::Weekly, Self::Overall];

    pub fn wire(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Overall => "overall",
        }
    }

    /// The period chips / arena subtitle ("Daily Sprint", "Weekly League", "Hall of Focus").
    pub fn label(self) -> &'static str {
        match self {
            Self::Daily => "Daily Sprint",
            Self::Weekly => "Weekly League",
            Self::Overall => "Hall of Focus",
        }
    }

    /// Field Notebook table column ("Today", "This week", "All time").
    pub fn fn_column(self) -> &'static str {
        match self {
            Self::Daily => "Today",
            Self::Weekly => "This week",
            Self::Overall => "All time",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: FriendCode,
    pub avatar: Avatar,
    pub minutes: u64,
    pub sessions: u64,
    pub rank: u32,
    /// `YYYY-MM-DD` as the Worker sends it (display only).
    pub last_active_date: Option<String>,
    pub is_self: bool,
}

/// `a.displayName.localeCompare(b.displayName)` for the tie-break. An approximation of the
/// default ICU collation (case-insensitive first, then exact), which only matters for equal
/// minutes.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

/// `getLeaderboardWithLocalSelf`: the cached rows kept for the scope (global: all; friends: self
/// and friends; squad: self and current squad members), sorted by minutes descending then name,
/// re-ranked from 1, `isSelf` recomputed from the user id.
pub fn ranked_for_scope(
    rows: &[LeaderboardEntry],
    scope: LeaderboardScope,
    self_id: &UserId,
    friends: &FriendsSnapshot,
    squad_members: &[UserId],
) -> Vec<LeaderboardEntry> {
    let mut kept: Vec<LeaderboardEntry> = rows
        .iter()
        .filter(|e| match scope {
            LeaderboardScope::Global => true,
            _ if e.is_self => true,
            LeaderboardScope::Squad => squad_members.contains(&e.user_id),
            LeaderboardScope::Friends => friends.is_friend(&e.user_id),
        })
        .cloned()
        .collect();
    kept.sort_by(|a, b| {
        b.minutes
            .cmp(&a.minutes)
            .then_with(|| locale_compare(&a.display_name, &b.display_name))
    });
    for (i, e) in kept.iter_mut().enumerate() {
        e.rank = u32::try_from(i + 1).unwrap_or(u32::MAX);
        e.is_self = &e.user_id == self_id;
    }
    kept
}

/// `Math.max(1, ...entries.map(minutes))`: the Field Notebook bar scale.
pub fn top_minutes(rows: &[LeaderboardEntry]) -> u64 {
    rows.iter().map(|e| e.minutes).max().unwrap_or(0).max(1)
}

/// The Field Notebook bar width in percent: `minutes > 0 ? max(4, round(minutes / top * 100)) : 0`.
pub fn bar_percent(minutes: u64, top: u64) -> u32 {
    if minutes == 0 {
        return 0;
    }
    let pct = crate::dashboard::format::js_round(minutes as f64 / top.max(1) as f64 * 100.0);
    (pct as u32).max(4)
}
