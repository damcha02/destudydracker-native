//! Friends and friend requests (Stage 22a).
//!
//! Production's Friends surface: add by player tag (`/friends/request`), the incoming list with
//! Accept/Decline (`/friends/respond`), the pending (outgoing) list, and the friends list, all
//! replaced wholesale from `/friends/status/v2` snapshots. There is **no** remove-friend or
//! cancel-request operation in production (neither in the client nor in the Worker), so none
//! exists here.

use serde::{Deserialize, Serialize};

use super::avatar::Avatar;
use super::ids::{FriendCode, RequestId, UserId};
use super::time::SocialTimestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: FriendCode,
    pub avatar: Avatar,
    pub friends_since: Option<SocialTimestamp>,
    pub last_seen_at: Option<SocialTimestamp>,
}

/// A pending request, from either side. Production keeps both ends' names, codes and avatars.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: RequestId,
    pub from_user_id: UserId,
    pub to_user_id: UserId,
    pub from_display_name: String,
    pub to_display_name: String,
    pub from_friend_code: FriendCode,
    pub to_friend_code: FriendCode,
    pub from_avatar: Avatar,
    pub to_avatar: Avatar,
    pub created_at: Option<SocialTimestamp>,
}

/// The friends part of a `getSocialSnapshot` (`friends`, `incomingFriendRequests`,
/// `outgoingFriendRequests`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendsSnapshot {
    pub friends: Vec<Friend>,
    pub incoming: Vec<FriendRequest>,
    pub outgoing: Vec<FriendRequest>,
}

impl FriendsSnapshot {
    /// Drops repeated ids (first occurrence wins). The Worker never sends duplicates; this only
    /// keeps a malformed or replayed payload from producing duplicate rows.
    pub fn deduplicated(mut self) -> Self {
        let mut seen = std::collections::HashSet::new();
        self.friends.retain(|f| seen.insert(f.user_id.clone()));
        let mut seen = std::collections::HashSet::new();
        self.incoming.retain(|r| seen.insert(r.id.clone()));
        let mut seen = std::collections::HashSet::new();
        self.outgoing.retain(|r| seen.insert(r.id.clone()));
        self
    }

    pub fn is_friend(&self, user_id: &UserId) -> bool {
        self.friends.iter().any(|f| &f.user_id == user_id)
    }

    /// `outgoingFriendRequestCodes.has(code)`.
    pub fn has_outgoing_to(&self, code: &FriendCode) -> bool {
        self.outgoing.iter().any(|r| &r.to_friend_code == code)
    }

    /// `incomingFriendRequestCount` (the nav badge).
    pub fn incoming_count(&self) -> usize {
        self.incoming.len()
    }
}

/// Why `sendFriendRequestToCode` refused before contacting the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendRequestRejection {
    Empty,
    OwnCode,
    AlreadyFriends,
    AlreadyPending,
}

impl FriendRequestRejection {
    /// Production's exact `setMessage` texts.
    pub fn message(self) -> &'static str {
        match self {
            Self::Empty => "Enter a friend code first.",
            Self::OwnCode => "That is your own friend code.",
            Self::AlreadyFriends => "You are already friends.",
            Self::AlreadyPending => "Friend request already pending.",
        }
    }
}

/// `sendFriendRequestToCode`'s local checks, in production's order. Returns the normalised code
/// to send.
pub fn check_friend_request(
    draft: &str,
    own_code: &FriendCode,
    snapshot: &FriendsSnapshot,
) -> Result<FriendCode, FriendRequestRejection> {
    let code = FriendCode::from_user_input(draft).ok_or(FriendRequestRejection::Empty)?;
    if &code == own_code {
        return Err(FriendRequestRejection::OwnCode);
    }
    if snapshot.friends.iter().any(|f| f.friend_code == code) {
        return Err(FriendRequestRejection::AlreadyFriends);
    }
    if snapshot.has_outgoing_to(&code) {
        return Err(FriendRequestRejection::AlreadyPending);
    }
    Ok(code)
}

/// The friend-request answer (`"accepted" | "declined"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendResponse {
    Accepted,
    Declined,
}

impl FriendResponse {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Declined => "declined",
        }
    }

    /// `answerFriendRequest`'s success message.
    pub fn done_message(self) -> &'static str {
        match self {
            Self::Accepted => "Friend request accepted.",
            Self::Declined => "Friend request declined.",
        }
    }
}
