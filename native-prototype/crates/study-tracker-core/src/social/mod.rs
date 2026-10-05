//! Social domain (Stage 22a, completed in 22b): identity states, player profile, friends, friend requests and
//! leaderboards, as production's `desktop/src/lib/social.ts` / `App.tsx` / `SocialScreen.tsx`
//! model them, with the Worker (`cloudflare/src/index.ts`) as the authority on server data.
//!
//! Pure domain only. No HTTP, URLs (beyond opaque avatar-URL strings carried as data), headers,
//! credential *storage*, threads or UI types live here: the application layer owns transport,
//! the credential file and the Slint models, and hands this module validated values.
//!
//! Everything that arrives from the server reaches these types only through the application's
//! wire-DTO validation (`src/net/social_api.rs`), which uses the constructors and limits below.

pub mod announcement;
pub mod avatar;
pub mod feed;
pub mod friends;
pub mod identity;
pub mod ids;
pub mod leaderboard;
pub mod limits;
pub mod profile;
pub mod squad;
pub mod stats;
pub mod telemetry;
pub mod time;
pub mod verified;

pub use avatar::{Avatar, AvatarStyle};
pub use friends::{Friend, FriendRequest, FriendRequestRejection, FriendsSnapshot};
pub use identity::{DeviceSecret, IdentityPhase, SocialIdentity};
pub use ids::{
    CommentId, FriendCode, MessageId, PollOptionId, PostId, RequestId, SquadId, UserId,
    VerifiedSessionId,
};
pub use leaderboard::{LeaderboardEntry, LeaderboardPeriod, LeaderboardScope};
pub use profile::SocialProfile;
pub use time::SocialTimestamp;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_22b;
