//! The Stage 22a Social / Daily Skribbl operations: request builders, typed wire DTOs and
//! validation into `study-tracker-core` types (brief §4, §9, §10, §12, §13).
//!
//! Every shape here is taken from production: the client (`desktop/src/lib/social.ts`,
//! `skribbl.ts`) for what is sent, the Worker (`cloudflare/src/index.ts`) for what comes back.
//!
//! Untrusted bytes -> DTO -> validation -> domain value:
//! - DTO fields are [`Lenient`]: a missing field, `null` or a wrong JSON type becomes `None`
//!   instead of failing the whole response (production's JavaScript tolerates the same);
//!   unknown fields are ignored (forward compatible).
//! - Lists are [`Rows`]: an invalid row is skipped, and at most a defensive cap of rows is kept
//!   (the rest is consumed without being stored).
//! - Required values (ids, codes, dates) that do not validate drop the row - or, for a
//!   single-object response, make it `NetError::Malformed`. Strings shown to the user pass
//!   `display_text` (no controls / bidi overrides, bounded). Numbers are clamped to plausible
//!   ranges; `NaN`/negative never reach the UI.
//! - Unknown enum values (avatar kinds, vote values) fall back to the safe default or are refused;
//!   nothing panics.

use std::fmt;
use std::marker::PhantomData;
use std::time::Duration;

use serde::de::{DeserializeOwned, IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use study_tracker_core::break_room::skribbl::{
    Drawing, GalleryPage, ThemeInfo, Winner, MAX_GALLERY_ROWS, MAX_SUBMIT_BYTES,
};
use study_tracker_core::social::avatar::Avatar;
use study_tracker_core::social::friends::{Friend, FriendRequest, FriendResponse, FriendsSnapshot};
use study_tracker_core::social::ids::{display_text, DrawingId, FriendCode, RequestId, UserId};
use study_tracker_core::social::leaderboard::{
    LeaderboardEntry, LeaderboardPeriod, LeaderboardScope,
};
use study_tracker_core::social::limits::{
    MAX_FRIENDS, MAX_FRIEND_REQUESTS, MAX_LEADERBOARD_ENTRIES, MAX_SYNC_BODY_BYTES,
    MAX_TOTAL_MINUTES, MAX_TOTAL_SESSIONS,
};
use study_tracker_core::social::profile::SocialProfile;
use study_tracker_core::social::stats::DailyStat;
use study_tracker_core::social::time::SocialTimestamp;
use study_tracker_core::social::SocialIdentity;

use super::device::{AppMetadata, DeviceIdentity};
use super::http::{ApiPath, ApiRequest, Body, HttpResponse, Method, NetError, Priority, Target};
use super::multipart::Multipart;

/// Total time for an API call (production has none; conservative, documented).
pub const API_TIMEOUT: Duration = Duration::from_secs(20);
/// A 1.5 MB upload on a slow uplink.
pub const UPLOAD_TIMEOUT: Duration = Duration::from_secs(45);
/// Largest JSON response accepted. The Worker's largest 22a reply (a friends snapshot with
/// leaderboard caches) is tens of KiB; this is defensive.
pub const MAX_API_RESPONSE: u64 = 2 * 1024 * 1024;
/// Display strings: names (48 in production), themes (~40), server messages.
const MAX_NAME_CHARS: usize = 64;
const MAX_THEME_CHARS: usize = 200;
const MAX_URL_LEN: usize = 512;

// ------------------------------------------------------------------------------- DTO helpers

/// A field that is `None` when missing, `null` or of the wrong type.
#[derive(Debug, Clone, PartialEq)]
pub struct Lenient<T>(pub Option<T>);

impl<T> Default for Lenient<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Lenient<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        Ok(Self(T::deserialize(value).ok()))
    }
}

/// A list that keeps the rows that parse, at most `CAP` of them.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows<T, const CAP: usize> {
    pub rows: Vec<T>,
    pub skipped: usize,
}

impl<T, const CAP: usize> Default for Rows<T, CAP> {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            skipped: 0,
        }
    }
}

impl<'de, T: DeserializeOwned, const CAP: usize> Deserialize<'de> for Rows<T, CAP> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V<T, const CAP: usize>(PhantomData<T>);
        impl<'de, T: DeserializeOwned, const CAP: usize> Visitor<'de> for V<T, CAP> {
            type Value = Rows<T, CAP>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a list")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut out = Rows::default();
                loop {
                    if out.rows.len() >= CAP {
                        // consume the rest without storing it
                        while seq.next_element::<IgnoredAny>()?.is_some() {
                            out.skipped += 1;
                        }
                        break;
                    }
                    match seq.next_element::<Lenient<T>>()? {
                        Some(Lenient(Some(row))) => out.rows.push(row),
                        Some(Lenient(None)) => out.skipped += 1,
                        None => break,
                    }
                }
                Ok(out)
            }
            // anything that is not a list is an empty list
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Rows::default())
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Rows::default())
            }
            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(Rows::default())
            }
            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Rows::default())
            }
            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Rows::default())
            }
            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Rows::default())
            }
            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Rows::default())
            }
        }
        d.deserialize_any(V::<T, CAP>(PhantomData))
    }
}

fn s(v: &Lenient<String>) -> Option<&str> {
    v.0.as_deref()
}

/// A non-negative count (`Number(row.minutes)`), clamped.
fn count(v: &Lenient<f64>, max: u64) -> u64 {
    match v.0 {
        Some(x) if x.is_finite() && x > 0.0 => (x.round() as u64).min(max),
        _ => 0,
    }
}

/// A signed score, clamped to a sane range.
fn score(v: &Lenient<f64>) -> i64 {
    match v.0 {
        Some(x) if x.is_finite() => x.round().clamp(-1e9, 1e9) as i64,
        _ => 0,
    }
}

fn name(raw: Option<&str>) -> String {
    let n = display_text(raw.unwrap_or(""), MAX_NAME_CHARS);
    if n.trim().is_empty() {
        "Student".to_string()
    } else {
        n
    }
}

fn url(raw: Option<&str>) -> Option<String> {
    raw.filter(|u| !u.is_empty() && u.len() <= MAX_URL_LEN)
        .map(str::to_string)
}

fn iso_date(raw: Option<&str>) -> Option<String> {
    let d = raw?;
    study_tracker_core::dashboard::civil::CivilDate::parse_iso(d)
        .filter(|_| d.len() == 10)
        .map(|_| d.to_string())
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AvatarDto {
    #[serde(default)]
    kind: Lenient<String>,
    #[serde(default)]
    letter: Lenient<String>,
    #[serde(default)]
    style: Lenient<String>,
    #[serde(default)]
    icon: Lenient<String>,
    #[serde(default)]
    name: Lenient<String>,
    #[serde(default)]
    url: Lenient<String>,
    #[serde(default, rename = "mimeType")]
    mime_type: Lenient<String>,
}

fn avatar(dto: &Lenient<AvatarDto>, display_name: &str) -> Avatar {
    let Some(a) = &dto.0 else {
        return Avatar::default_for(display_name);
    };
    let photo = s(&a.url).map(|u| {
        (
            s(&a.name).unwrap_or("photo"),
            u,
            s(&a.mime_type).unwrap_or(""),
        )
    });
    Avatar::normalized(
        s(&a.kind),
        s(&a.letter),
        s(&a.style),
        s(&a.icon),
        photo,
        display_name,
    )
}

fn check_status(resp: &HttpResponse) -> Result<(), NetError> {
    if (200..300).contains(&resp.status) {
        Ok(())
    } else {
        Err(NetError::from_status(resp.status, &resp.body))
    }
}

fn parse_json<T: DeserializeOwned>(resp: &HttpResponse) -> Result<T, NetError> {
    check_status(resp)?;
    serde_json::from_slice(&resp.body).map_err(|_| NetError::Malformed)
}

// ---------------------------------------------------------------------------- request basics

fn json_request<T: Serialize>(path: ApiPath, body: &T) -> ApiRequest {
    ApiRequest {
        method: Method::Post,
        target: Target::Api(path),
        query: Vec::new(),
        body: Body::Json(serde_json::to_vec(body).unwrap_or_default()),
        max_response: MAX_API_RESPONSE,
        timeout: API_TIMEOUT,
        priority: Priority::Api,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Auth<'a> {
    user_id: &'a str,
    device_secret: &'a str,
}

fn auth(id: &SocialIdentity) -> Auth<'_> {
    Auth {
        user_id: id.user_id.as_str(),
        device_secret: id.device_secret.expose(),
    }
}

/// `identityParams(social)`: production sends the credential in the query string of the two
/// Skribbl GETs. Kept for protocol compatibility; the query exists only inside the transport.
fn credential_query(id: &SocialIdentity) -> Vec<(&'static str, String)> {
    vec![
        ("userId", id.user_id.as_str().to_string()),
        ("deviceSecret", id.device_secret.expose().to_string()),
    ]
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDto<'a> {
    version: &'a str,
    platform: &'a str,
    runtime_channel: &'a str,
}

impl<'a> From<&'a AppMetadata> for AppDto<'a> {
    fn from(a: &'a AppMetadata) -> Self {
        Self {
            version: &a.version,
            platform: &a.platform,
            runtime_channel: &a.runtime_channel,
        }
    }
}

// ----------------------------------------------------------------------------- /sync/v2

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceDto<'a> {
    fingerprint_hash: &'a str,
    label: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncUser<'a> {
    user_id: &'a str,
    device_secret: &'a str,
    friend_code: &'a str,
    display_name: &'a str,
    avatar: &'a Avatar,
    is_private: bool,
    show_hours_to_friends: bool,
    lifetime_study_minutes: u64,
    lifetime_study_sessions: u64,
    device: DeviceDto<'a>,
    app: AppDto<'a>,
}

#[derive(Serialize)]
struct StatDto {
    date: String,
    minutes: u64,
    sessions: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SyncBody<'a> {
    user: SyncUser<'a>,
    stats: Vec<StatDto>,
    /// Feed posts are Stage 22b; production sends its queue here, native sends none yet.
    feed_posts: [(); 0],
}

pub struct SyncInput<'a> {
    pub identity: &'a SocialIdentity,
    pub profile: &'a SocialProfile,
    pub lifetime_minutes: u64,
    pub lifetime_sessions: u64,
    pub stats: &'a [DailyStat],
    pub device: &'a DeviceIdentity,
    pub app: &'a AppMetadata,
}

/// `syncSocialState` -> `POST /sync/v2` (creates the account when the id is unknown).
/// `Err(PayloadTooLarge)` above production's 256 KiB client limit.
pub fn sync_v2(input: &SyncInput) -> Result<ApiRequest, NetError> {
    let body = SyncBody {
        user: SyncUser {
            user_id: input.identity.user_id.as_str(),
            device_secret: input.identity.device_secret.expose(),
            friend_code: input.profile.friend_code.as_str(),
            display_name: &input.profile.display_name,
            avatar: &input.profile.avatar,
            is_private: input.profile.is_private,
            show_hours_to_friends: input.profile.show_hours_to_friends,
            lifetime_study_minutes: input.lifetime_minutes,
            lifetime_study_sessions: input.lifetime_sessions,
            device: DeviceDto {
                fingerprint_hash: &input.device.fingerprint_hash,
                label: &input.device.label,
            },
            app: input.app.into(),
        },
        stats: input
            .stats
            .iter()
            .map(|s| StatDto {
                date: s.date.to_iso(),
                minutes: s.minutes,
                sessions: s.sessions,
            })
            .collect(),
        feed_posts: [],
    };
    let req = json_request(ApiPath::SyncV2, &body);
    if req.body.len() > MAX_SYNC_BODY_BYTES {
        return Err(NetError::PayloadTooLarge(Some(
            "Social sync payload is too large. Use a smaller profile photo, then sync again."
                .into(),
        )));
    }
    Ok(req)
}

#[derive(Debug, Deserialize)]
struct OkDto {
    #[serde(default)]
    ok: Lenient<bool>,
}

/// `{ ok, syncedAt }`. The client keeps its own `syncedAt` (`new Date().toISOString()`), as
/// production does.
pub fn parse_ok(resp: &HttpResponse) -> Result<(), NetError> {
    let dto: OkDto = parse_json(resp)?;
    match dto.ok.0 {
        Some(true) => Ok(()),
        _ => Err(NetError::Malformed),
    }
}

// ----------------------------------------------------------------------------- /presence

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PresenceBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    app: AppDto<'a>,
}

pub fn presence(id: &SocialIdentity, app: &AppMetadata) -> ApiRequest {
    json_request(
        ApiPath::Presence,
        &PresenceBody {
            auth: auth(id),
            app: app.into(),
        },
    )
}

// ------------------------------------------------------------------- friends / social snapshot

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendDto {
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    friends_since: Lenient<String>,
    #[serde(default)]
    last_seen_at: Lenient<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequestDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    from_user_id: Lenient<String>,
    #[serde(default)]
    to_user_id: Lenient<String>,
    #[serde(default)]
    from_display_name: Lenient<String>,
    #[serde(default)]
    to_display_name: Lenient<String>,
    #[serde(default)]
    from_friend_code: Lenient<String>,
    #[serde(default)]
    to_friend_code: Lenient<String>,
    #[serde(default)]
    from_avatar: Lenient<AvatarDto>,
    #[serde(default)]
    to_avatar: Lenient<AvatarDto>,
    #[serde(default)]
    created_at: Lenient<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntryDto {
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    minutes: Lenient<f64>,
    #[serde(default)]
    sessions: Lenient<f64>,
    #[serde(default)]
    rank: Lenient<f64>,
    #[serde(default)]
    last_active_date: Lenient<String>,
    #[serde(default)]
    is_self: Lenient<bool>,
}

type LeaderRows = Rows<LeaderboardEntryDto, MAX_LEADERBOARD_ENTRIES>;

#[derive(Debug, Clone, Default, Deserialize)]
struct PeriodRowsDto {
    #[serde(default)]
    daily: LeaderRows,
    #[serde(default)]
    weekly: LeaderRows,
    #[serde(default)]
    overall: LeaderRows,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct CachedLeaderboardsDto {
    #[serde(default)]
    global: Lenient<PeriodRowsDto>,
    #[serde(default)]
    friends: Lenient<PeriodRowsDto>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SocialSnapshotDto {
    #[serde(default)]
    friends: Option<Rows<FriendDto, MAX_FRIENDS>>,
    #[serde(default)]
    incoming_friend_requests: Option<Rows<FriendRequestDto, MAX_FRIEND_REQUESTS>>,
    #[serde(default)]
    outgoing_friend_requests: Option<Rows<FriendRequestDto, MAX_FRIEND_REQUESTS>>,
    #[serde(default)]
    cached_leaderboards: Lenient<CachedLeaderboardsDto>,
}

#[derive(Debug, Deserialize)]
struct SnapshotEnvelope {
    #[serde(default)]
    social: Lenient<SocialSnapshotDto>,
}

fn friend(dto: &FriendDto) -> Option<Friend> {
    let display_name = name(s(&dto.display_name));
    Some(Friend {
        user_id: UserId::parse(s(&dto.user_id)?)?,
        friend_code: FriendCode::parse(s(&dto.friend_code)?)?,
        avatar: avatar(&dto.avatar, &display_name),
        friends_since: s(&dto.friends_since).and_then(SocialTimestamp::parse),
        last_seen_at: s(&dto.last_seen_at).and_then(SocialTimestamp::parse),
        display_name,
    })
}

fn friend_request(dto: &FriendRequestDto) -> Option<FriendRequest> {
    let from_name = name(s(&dto.from_display_name));
    let to_name = name(s(&dto.to_display_name));
    Some(FriendRequest {
        id: RequestId::parse(s(&dto.id)?)?,
        from_user_id: UserId::parse(s(&dto.from_user_id)?)?,
        to_user_id: UserId::parse(s(&dto.to_user_id)?)?,
        from_friend_code: FriendCode::parse(s(&dto.from_friend_code)?)?,
        to_friend_code: FriendCode::parse(s(&dto.to_friend_code)?)?,
        from_avatar: avatar(&dto.from_avatar, &from_name),
        to_avatar: avatar(&dto.to_avatar, &to_name),
        created_at: s(&dto.created_at).and_then(SocialTimestamp::parse),
        from_display_name: from_name,
        to_display_name: to_name,
    })
}

pub fn leaderboard_entry(dto: &LeaderboardEntryDto) -> Option<LeaderboardEntry> {
    let display_name = name(s(&dto.display_name));
    Some(LeaderboardEntry {
        user_id: UserId::parse(s(&dto.user_id)?)?,
        friend_code: FriendCode::parse(s(&dto.friend_code)?)?,
        avatar: avatar(&dto.avatar, &display_name),
        minutes: count(&dto.minutes, MAX_TOTAL_MINUTES),
        sessions: count(&dto.sessions, MAX_TOTAL_SESSIONS),
        rank: count(&dto.rank, u64::from(u32::MAX)) as u32,
        last_active_date: iso_date(s(&dto.last_active_date)),
        is_self: dto.is_self.0.unwrap_or(false),
        display_name,
    })
}

fn leaderboard_rows(rows: &LeaderRows) -> Vec<LeaderboardEntry> {
    let mut seen = std::collections::HashSet::new();
    rows.rows
        .iter()
        .filter_map(leaderboard_entry)
        .filter(|e| seen.insert(e.user_id.clone()))
        .collect()
}

/// The 22a part of a `getSocialSnapshot` reply.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SocialSnapshot {
    pub friends: FriendsSnapshot,
    /// `cachedLeaderboards` when the reply carried them (`/friends/request`, `/friends/respond`
    /// send the full snapshot; `/friends/status/v2` does not), keyed by scope and period.
    pub leaderboards: Vec<(LeaderboardScope, LeaderboardPeriod, Vec<LeaderboardEntry>)>,
}

/// `{ social: {...} }` (getSocialSnapshot). A reply without the three lists is malformed.
pub fn parse_snapshot(resp: &HttpResponse) -> Result<SocialSnapshot, NetError> {
    let env: SnapshotEnvelope = parse_json(resp)?;
    let social = env.social.0.ok_or(NetError::Malformed)?;
    let (Some(friends), Some(incoming), Some(outgoing)) = (
        social.friends,
        social.incoming_friend_requests,
        social.outgoing_friend_requests,
    ) else {
        return Err(NetError::Malformed);
    };
    let friends = FriendsSnapshot {
        friends: friends.rows.iter().filter_map(friend).collect(),
        incoming: incoming.rows.iter().filter_map(friend_request).collect(),
        outgoing: outgoing.rows.iter().filter_map(friend_request).collect(),
    }
    .deduplicated();
    let mut leaderboards = Vec::new();
    if let Some(cached) = social.cached_leaderboards.0 {
        for (scope, rows) in [
            (LeaderboardScope::Global, cached.global.0),
            (LeaderboardScope::Friends, cached.friends.0),
        ] {
            if let Some(rows) = rows {
                leaderboards.push((
                    scope,
                    LeaderboardPeriod::Daily,
                    leaderboard_rows(&rows.daily),
                ));
                leaderboards.push((
                    scope,
                    LeaderboardPeriod::Weekly,
                    leaderboard_rows(&rows.weekly),
                ));
                leaderboards.push((
                    scope,
                    LeaderboardPeriod::Overall,
                    leaderboard_rows(&rows.overall),
                ));
            }
        }
    }
    Ok(SocialSnapshot {
        friends,
        leaderboards,
    })
}

/// `getFriendStatus` -> `POST /friends/status/v2`.
pub fn friends_status(id: &SocialIdentity) -> ApiRequest {
    json_request(ApiPath::FriendsStatusV2, &auth(id))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FriendRequestBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    friend_code: &'a str,
}

/// `createFriendRequest` -> `POST /friends/request`.
pub fn friend_request_create(id: &SocialIdentity, code: &FriendCode) -> ApiRequest {
    json_request(
        ApiPath::FriendsRequest,
        &FriendRequestBody {
            auth: auth(id),
            friend_code: code.as_str(),
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RespondBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    request_id: &'a str,
    response: &'static str,
}

/// `respondToFriendRequest` -> `POST /friends/respond`.
pub fn friend_respond(
    id: &SocialIdentity,
    request: &RequestId,
    response: FriendResponse,
) -> ApiRequest {
    json_request(
        ApiPath::FriendsRespond,
        &RespondBody {
            auth: auth(id),
            request_id: request.as_str(),
            response: response.wire(),
        },
    )
}

// ---------------------------------------------------------------------------- /leaderboard

#[derive(Serialize)]
struct LeaderboardBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    scope: &'static str,
    period: &'static str,
}

/// `getSocialLeaderboard` -> `POST /leaderboard`.
pub fn leaderboard(
    id: &SocialIdentity,
    scope: LeaderboardScope,
    period: LeaderboardPeriod,
) -> ApiRequest {
    json_request(
        ApiPath::Leaderboard,
        &LeaderboardBody {
            auth: auth(id),
            scope: scope.wire(),
            period: period.wire(),
        },
    )
}

#[derive(Debug, Deserialize)]
struct EntriesDto {
    #[serde(default)]
    entries: Option<LeaderRows>,
}

/// `{ entries: [...] }`.
pub fn parse_leaderboard(resp: &HttpResponse) -> Result<Vec<LeaderboardEntry>, NetError> {
    let dto: EntriesDto = parse_json(resp)?;
    let rows = dto.entries.ok_or(NetError::Malformed)?;
    Ok(leaderboard_rows(&rows))
}

// ---------------------------------------------------------------------------- /player-stats

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlayerStatsBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    target_user_id: &'a str,
}

/// `getPlayerStats` -> `POST /player-stats`.
pub fn player_stats(id: &SocialIdentity, target: &UserId) -> ApiRequest {
    json_request(
        ApiPath::PlayerStats,
        &PlayerStatsBody {
            auth: auth(id),
            target_user_id: target.as_str(),
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PeriodStatsDto {
    #[serde(default)]
    minutes: Lenient<f64>,
    #[serde(default)]
    sessions: Lenient<f64>,
    #[serde(default)]
    last_active_date: Lenient<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerStatsDto {
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    last_seen_at: Lenient<String>,
    #[serde(default)]
    hours_visible: Lenient<bool>,
    #[serde(default)]
    daily: Lenient<PeriodStatsDto>,
    #[serde(default)]
    weekly: Lenient<PeriodStatsDto>,
    #[serde(default)]
    overall: Lenient<PeriodStatsDto>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodStats {
    pub minutes: u64,
    pub sessions: u64,
    pub last_active_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerStats {
    pub display_name: String,
    pub friend_code: Option<FriendCode>,
    pub avatar: Avatar,
    pub last_seen_at: Option<SocialTimestamp>,
    pub hours_visible: bool,
    /// All three present only when hours are visible (production's own condition).
    pub periods: Option<[PeriodStats; 3]>,
}

fn period(dto: &PeriodStatsDto) -> PeriodStats {
    PeriodStats {
        minutes: count(&dto.minutes, MAX_TOTAL_MINUTES),
        sessions: count(&dto.sessions, MAX_TOTAL_SESSIONS),
        last_active_date: iso_date(s(&dto.last_active_date)),
    }
}

pub fn parse_player_stats(resp: &HttpResponse) -> Result<PlayerStats, NetError> {
    let dto: PlayerStatsDto = parse_json(resp)?;
    let display_name = name(s(&dto.display_name));
    let hours_visible = dto.hours_visible.0.unwrap_or(false);
    let periods = match (&dto.daily.0, &dto.weekly.0, &dto.overall.0) {
        (Some(d), Some(w), Some(o)) if hours_visible => Some([period(d), period(w), period(o)]),
        _ => None,
    };
    Ok(PlayerStats {
        friend_code: s(&dto.friend_code).and_then(FriendCode::parse),
        avatar: avatar(&dto.avatar, &display_name),
        last_seen_at: s(&dto.last_seen_at).and_then(SocialTimestamp::parse),
        hours_visible,
        periods,
        display_name,
    })
}

// -------------------------------------------------------------------------- Daily Skribbl

/// `getSkribblTheme` -> `GET /skribbl/theme?userId&deviceSecret`.
pub fn skribbl_theme(id: &SocialIdentity) -> ApiRequest {
    ApiRequest {
        method: Method::Get,
        target: Target::Api(ApiPath::SkribblTheme),
        query: credential_query(id),
        body: Body::None,
        max_response: MAX_API_RESPONSE,
        timeout: API_TIMEOUT,
        priority: Priority::Api,
    }
}

/// `getSkribblLeaderboard` -> `GET /skribbl/leaderboard?userId&deviceSecret`.
pub fn skribbl_leaderboard(id: &SocialIdentity) -> ApiRequest {
    ApiRequest {
        target: Target::Api(ApiPath::SkribblLeaderboard),
        ..skribbl_theme(id)
    }
}

#[derive(Serialize)]
struct GalleryBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    date: &'a str,
    offset: usize,
    limit: usize,
}

/// `getSkribblGallery` -> `POST /skribbl/gallery`.
pub fn skribbl_gallery(id: &SocialIdentity, date: &str, offset: usize) -> ApiRequest {
    json_request(
        ApiPath::SkribblGallery,
        &GalleryBody {
            auth: auth(id),
            date,
            offset,
            limit: study_tracker_core::break_room::skribbl::GALLERY_PAGE_SIZE,
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VoteBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    drawing_id: &'a str,
    vote: i8,
}

/// `voteSkribblDrawing` -> `POST /skribbl/vote` (`vote` is -1, 0 or 1).
pub fn skribbl_vote(id: &SocialIdentity, drawing: &DrawingId, vote: i8) -> Option<ApiRequest> {
    matches!(vote, -1..=1).then(|| {
        json_request(
            ApiPath::SkribblVote,
            &VoteBody {
                auth: auth(id),
                drawing_id: drawing.as_str(),
                vote,
            },
        )
    })
}

/// `submitSkribblDrawing` -> multipart `POST /skribbl/submit` (`userId`, `deviceSecret`, `date`,
/// `image`). Native uploads PNG (the Worker accepts `image/png` and `image/webp`). Refuses
/// locally above the Worker's 1.5 MB limit.
pub fn skribbl_submit(
    id: &SocialIdentity,
    png: Vec<u8>,
    date: &str,
) -> Result<ApiRequest, NetError> {
    if png.len() > MAX_SUBMIT_BYTES {
        return Err(NetError::PayloadTooLarge(Some(
            "Drawing is too large. Keep it under 1.5 MB.".into(),
        )));
    }
    // a fresh random boundary until one does not occur in the image (practically always the first)
    let (content_type, bytes) = (0..8)
        .find_map(|_| {
            Multipart::new()
                .text("userId", id.user_id.as_str())
                .text("deviceSecret", id.device_secret.expose())
                .text("date", date)
                .file("image", "drawing.png", "image/png", png.clone())
                .finish()
        })
        .ok_or(NetError::Blocked)?;
    Ok(ApiRequest {
        method: Method::Post,
        target: Target::Api(ApiPath::SkribblSubmit),
        query: Vec::new(),
        body: Body::Multipart {
            content_type,
            bytes,
        },
        max_response: MAX_API_RESPONSE,
        timeout: UPLOAD_TIMEOUT,
        priority: Priority::Api,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThemeDto {
    #[serde(default)]
    date: Lenient<String>,
    #[serde(default)]
    theme: Lenient<String>,
    #[serde(default)]
    submitted: Lenient<bool>,
    #[serde(default)]
    drawing_id: Lenient<String>,
    #[serde(default)]
    image_url: Lenient<String>,
}

pub fn parse_theme(resp: &HttpResponse) -> Result<ThemeInfo, NetError> {
    let dto: ThemeDto = parse_json(resp)?;
    let theme = display_text(s(&dto.theme).ok_or(NetError::Malformed)?, MAX_THEME_CHARS);
    let submitted = dto.submitted.0.unwrap_or(false);
    Ok(ThemeInfo {
        date: iso_date(s(&dto.date)).ok_or(NetError::Malformed)?,
        theme,
        submitted,
        drawing_id: s(&dto.drawing_id).and_then(DrawingId::parse),
        image_url: if submitted {
            url(s(&dto.image_url))
        } else {
            None
        },
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrawingDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    vote_score: Lenient<f64>,
    #[serde(default)]
    vote_count: Lenient<f64>,
    #[serde(default)]
    my_vote: Lenient<f64>,
    #[serde(default)]
    is_self: Lenient<bool>,
    #[serde(default)]
    image_url: Lenient<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GalleryDto {
    #[serde(default)]
    drawings: Option<Rows<DrawingDto, MAX_GALLERY_ROWS>>,
    #[serde(default)]
    next_offset: Lenient<f64>,
    #[serde(default)]
    has_more: Lenient<bool>,
}

fn drawing(dto: &DrawingDto) -> Option<Drawing> {
    let my_vote = match dto.my_vote.0 {
        Some(v) if v == 1.0 => 1,
        Some(v) if v == -1.0 => -1,
        _ => 0,
    };
    Some(Drawing {
        id: DrawingId::parse(s(&dto.id)?)?,
        user_id: UserId::parse(s(&dto.user_id)?)?,
        display_name: name(s(&dto.display_name)),
        vote_score: score(&dto.vote_score),
        vote_count: count(&dto.vote_count, MAX_TOTAL_SESSIONS),
        my_vote,
        is_self: dto.is_self.0.unwrap_or(false),
        image_url: url(s(&dto.image_url))?,
    })
}

pub fn parse_gallery(resp: &HttpResponse) -> Result<GalleryPage, NetError> {
    let dto: GalleryDto = parse_json(resp)?;
    let rows = dto.drawings.ok_or(NetError::Malformed)?;
    let mut seen = std::collections::HashSet::new();
    let drawings: Vec<Drawing> = rows
        .rows
        .iter()
        .filter_map(drawing)
        .filter(|d| seen.insert(d.id.clone()))
        .collect();
    let next_offset = match dto.next_offset.0 {
        Some(n) if n.is_finite() && n >= 0.0 && n < 1e7 => Some(n as usize),
        _ => None,
    };
    Ok(GalleryPage {
        drawings,
        next_offset,
        has_more: dto.has_more.0.unwrap_or(false) && next_offset.is_some(),
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    image_url: Lenient<String>,
}

/// `{ ok, drawingId, date, imageUrl }`: the new drawing's image URL.
pub fn parse_submit(resp: &HttpResponse) -> Result<String, NetError> {
    let dto: SubmitDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    url(s(&dto.image_url)).ok_or(NetError::Malformed)
}

#[derive(Debug, Deserialize)]
struct VoteDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    score: Lenient<f64>,
}

/// `{ ok, score }`.
pub fn parse_vote(resp: &HttpResponse) -> Result<i64, NetError> {
    let dto: VoteDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) || dto.score.0.is_none() {
        return Err(NetError::Malformed);
    }
    Ok(score(&dto.score))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WinnerDto {
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    score: Lenient<f64>,
    #[serde(default)]
    drawing_id: Lenient<String>,
}

#[derive(Debug, Deserialize)]
struct SkribblLeaderboardDto {
    #[serde(default)]
    winner: Lenient<WinnerDto>,
}

/// `{ date, winner | null }`. A winner without a drawing id or name is treated as none.
pub fn parse_skribbl_leaderboard(resp: &HttpResponse) -> Result<Option<Winner>, NetError> {
    let dto: SkribblLeaderboardDto = parse_json(resp)?;
    Ok(dto.winner.0.and_then(|w| {
        s(&w.drawing_id)?;
        Some(Winner {
            display_name: name(s(&w.display_name)),
            score: score(&w.score),
        })
    }))
}

#[cfg(test)]
#[path = "social_api_tests.rs"]
mod tests;
