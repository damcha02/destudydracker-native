//! The Stage 22b operations: Feed (posts, polls, reactions, comments, images), Squads (snapshot,
//! search, details, membership, roles, chat, the Squad Arena), the profile avatar upload,
//! verified sessions, announcements, opt-in telemetry and the owner-only usage view.
//!
//! Same rules as `social_api` (22a): production's client (`desktop/src/lib/social.ts`) defines
//! what is sent, the Worker (`cloudflare/src/index.ts`) what comes back, and every reply goes
//! wire DTO -> validation -> core domain value. Lists are bounded (`Rows`, `BoundedMap`), strings
//! pass `display_text`/`display_paragraph`, ids are parsed, unknown enum values fail safe, and
//! nothing here can panic on server data.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{DeserializeOwned, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use study_tracker_core::social::announcement::Announcement;
use study_tracker_core::social::avatar::Avatar;
use study_tracker_core::social::feed::{
    Comment, FeedImage, FeedPost, FeedScope, PendingPost, Poll, PollOption, PostKind,
    MAX_COMMENTS_PER_POST, MAX_FEED_ROWS, MAX_POLL_OPTIONS_RECEIVED, MAX_REACTION_KINDS,
    MAX_REACTOR_NAMES,
};
use study_tracker_core::social::ids::{
    display_paragraph, display_text, CommentId, FriendCode, MessageId, PollOptionId, PostId,
    RequestId, SquadId, UserId, VerifiedSessionId,
};
use study_tracker_core::social::limits::{MAX_TOTAL_MINUTES, MAX_TOTAL_SESSIONS};
use study_tracker_core::social::squad::{
    Squad, SquadAction, SquadDetails, SquadJoinRequest, SquadMember, SquadMessage, SquadRole,
    SquadScoreEntry, SquadScorePeriod, SquadSearchResult, SquadSnapshot, MAX_MEMBERS_RECEIVED,
    MAX_MESSAGES_RECEIVED, MAX_SCOREBOARD_ROWS, MAX_SEARCH_ROWS, MAX_SQUAD_REQUESTS,
};
use study_tracker_core::social::telemetry::InstallId;
use study_tracker_core::social::time::SocialTimestamp;
use study_tracker_core::social::verified::Interval;
use study_tracker_core::social::SocialIdentity;

use super::device::AppMetadata;
use super::http::{ApiPath, ApiRequest, Body, HttpResponse, Method, NetError, Priority, Target};
use super::multipart::Multipart;
use super::social_api::{
    auth, avatar, count, json_request, name, parse_json, s, score, url, AppDto, Auth, AvatarDto,
    Lenient, Rows, API_TIMEOUT, MAX_API_RESPONSE, UPLOAD_TIMEOUT,
};

/// Feed text bounds (display; the Worker's own limits are lower).
const MAX_SUBJECT_CHARS: usize = 120;
const MAX_ICON_CHARS: usize = 8;
const MAX_NOTE_CHARS: usize = 400;
const MAX_BODY_CHARS: usize = 1000;
const MAX_REACTION_KEY_CHARS: usize = 16;
/// `MAX_FEED_IMAGE_BYTES` (5 MiB) and `MAX_PROFILE_AVATAR_BYTES` (256 KiB): refused locally.
pub const MAX_FEED_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_AVATAR_UPLOAD_BYTES: usize = 256 * 1024;

fn ts(v: &Lenient<String>) -> Option<SocialTimestamp> {
    s(v).and_then(SocialTimestamp::parse)
}

fn text(v: &Lenient<String>, max: usize) -> String {
    display_text(s(v).unwrap_or(""), max)
}

fn code(v: &Lenient<String>) -> Option<FriendCode> {
    s(v).and_then(FriendCode::parse)
}

// ---------------------------------------------------------------------- a bounded JSON object

/// A JSON object kept in wire order (production reads it with `JSON.parse`, which keeps the
/// order), at most `CAP` entries; values that do not parse are skipped; anything that is not an
/// object is an empty map.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundedMap<V, const CAP: usize>(pub Vec<(String, V)>);

impl<V, const CAP: usize> Default for BoundedMap<V, CAP> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<'de, V: DeserializeOwned, const CAP: usize> Deserialize<'de> for BoundedMap<V, CAP> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Vis<V, const CAP: usize>(PhantomData<V>);
        impl<'de, V: DeserializeOwned, const CAP: usize> Visitor<'de> for Vis<V, CAP> {
            type Value = BoundedMap<V, CAP>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut out = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    if out.len() >= CAP {
                        map.next_value::<IgnoredAny>()?;
                        continue;
                    }
                    if let Lenient(Some(v)) = map.next_value::<Lenient<V>>()? {
                        out.push((key, v));
                    }
                }
                Ok(BoundedMap(out))
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(BoundedMap(Vec::new()))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(BoundedMap(Vec::new()))
            }
            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(BoundedMap(Vec::new()))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(BoundedMap(Vec::new()))
            }
            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(BoundedMap(Vec::new()))
            }
            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(BoundedMap(Vec::new()))
            }
            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(BoundedMap(Vec::new()))
            }
        }
        d.deserialize_any(Vis::<V, CAP>(PhantomData))
    }
}

const BASE_KEYS: [&str; 3] = study_tracker_core::social::feed::BASE_REACTIONS;

/// A reaction key as the server stores it: `cleanText(emoji, 8)`, at most 4 code points; a key
/// that cannot have come from the Worker is dropped.
fn reaction_key(raw: &str) -> Option<String> {
    let k = display_text(raw.trim(), MAX_REACTION_KEY_CHARS);
    (!k.is_empty() && k.chars().count() <= 8).then_some(k)
}

// ------------------------------------------------------------------------------- feed posts

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollOptionDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    text: Lenient<String>,
    #[serde(default)]
    votes: Lenient<f64>,
    #[serde(default)]
    selected: Lenient<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PollDto {
    #[serde(default)]
    question: Lenient<String>,
    #[serde(default)]
    multiple: Lenient<bool>,
    #[serde(default)]
    options: Rows<PollOptionDto, MAX_POLL_OPTIONS_RECEIVED>,
    #[serde(default)]
    total_votes: Lenient<f64>,
}

fn poll(dto: &PollDto) -> Poll {
    let mut seen = std::collections::HashSet::new();
    let options: Vec<PollOption> = dto
        .options
        .rows
        .iter()
        .filter_map(|o| {
            Some(PollOption {
                id: PollOptionId::parse(s(&o.id)?)?,
                text: text(&o.text, 200),
                votes: count(&o.votes, MAX_TOTAL_SESSIONS),
                selected: o.selected.0.unwrap_or(false),
            })
        })
        .filter(|o| seen.insert(o.id.clone()))
        .collect();
    let sum: u64 = options.iter().map(|o| o.votes).sum();
    Poll {
        question: text(&dto.question, 300),
        multiple: dto.multiple.0.unwrap_or(false),
        // the Worker sends the sum; a reply that disagrees is corrected (percentages <= 100)
        total_votes: match dto.total_votes.0 {
            Some(_) => count(&dto.total_votes, MAX_TOTAL_SESSIONS).max(sum),
            None => sum,
        },
        options,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    post_id: Lenient<String>,
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    body: Lenient<String>,
    #[serde(default)]
    created_at: Lenient<String>,
    #[serde(default)]
    is_self: Lenient<bool>,
}

fn comment(dto: &CommentDto, post: &PostId) -> Option<Comment> {
    let display_name = name(s(&dto.display_name));
    let post_id = s(&dto.post_id)
        .and_then(PostId::parse)
        .unwrap_or_else(|| post.clone());
    Some(Comment {
        id: CommentId::parse(s(&dto.id)?)?,
        post_id,
        user_id: UserId::parse(s(&dto.user_id)?)?,
        friend_code: code(&dto.friend_code),
        avatar: avatar(&dto.avatar, &display_name),
        body: display_paragraph(s(&dto.body).unwrap_or(""), MAX_NOTE_CHARS),
        created_at: ts(&dto.created_at),
        is_self: dto.is_self.0.unwrap_or(false),
        display_name,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPostDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default, rename = "type")]
    kind: Lenient<String>,
    #[serde(default)]
    subject: Lenient<String>,
    #[serde(default)]
    detail: Lenient<String>,
    #[serde(default)]
    note: Lenient<String>,
    #[serde(default)]
    icon: Lenient<String>,
    #[serde(default)]
    minutes: Lenient<f64>,
    #[serde(default)]
    preset_label: Lenient<String>,
    #[serde(default)]
    created_at: Lenient<String>,
    #[serde(default)]
    is_self: Lenient<bool>,
    #[serde(default)]
    image_url: Lenient<String>,
    #[serde(default)]
    image_mime_type: Lenient<String>,
    #[serde(default)]
    image_expires_at: Lenient<String>,
    #[serde(default)]
    image_expired_at: Lenient<String>,
    #[serde(default)]
    poll: Lenient<PollDto>,
    #[serde(default)]
    reactions: BoundedMap<f64, MAX_REACTION_KINDS>,
    #[serde(default)]
    reacted: BoundedMap<bool, MAX_REACTION_KINDS>,
    #[serde(default)]
    reacted_by: BoundedMap<Rows<String, MAX_REACTOR_NAMES>, MAX_REACTION_KINDS>,
    #[serde(default)]
    comments: Rows<CommentDto, MAX_COMMENTS_PER_POST>,
}

pub fn feed_post(dto: &FeedPostDto) -> Option<FeedPost> {
    let id = PostId::parse(s(&dto.id)?)?;
    let display_name = name(s(&dto.display_name));
    let mut reactions: Vec<(String, u64)> = Vec::new();
    for (k, v) in &dto.reactions.0 {
        if let Some(k) = reaction_key(k) {
            if !reactions.iter().any(|(x, _)| *x == k) {
                reactions.push((k, count(&Lenient(Some(*v)), MAX_TOTAL_SESSIONS)));
            }
        }
    }
    // production's key order: `{ fire, brain, clap, ...counts }` where the counts come from
    // `GROUP BY post_id, emoji` (SQLite's byte order) - reconstructed here, independent of how
    // the JSON object was ordered on the wire
    let rank = |k: &str| {
        BASE_KEYS
            .iter()
            .position(|b| *b == k)
            .unwrap_or(BASE_KEYS.len())
    };
    reactions.sort_by(|(a, _), (b, _)| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.as_bytes().cmp(b.as_bytes()))
    });
    let reacted = dto
        .reacted
        .0
        .iter()
        .filter(|(_, on)| *on)
        .filter_map(|(k, _)| reaction_key(k))
        .collect();
    let reacted_by = dto
        .reacted_by
        .0
        .iter()
        .filter_map(|(k, names)| {
            Some((
                reaction_key(k)?,
                names
                    .rows
                    .iter()
                    .map(|n| display_text(n, super::social_api::MAX_NAME_CHARS))
                    .collect(),
            ))
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    let comments = dto
        .comments
        .rows
        .iter()
        .filter_map(|c| comment(c, &id))
        .filter(|c| seen.insert(c.id.clone()))
        .collect();
    let image = url(s(&dto.image_url)).map(|u| FeedImage {
        url: u,
        mime_type: s(&dto.image_mime_type).map(|m| display_text(m, 40)),
        expires_at: ts(&dto.image_expires_at),
    });
    Some(FeedPost {
        id,
        user_id: UserId::parse(s(&dto.user_id)?)?,
        friend_code: code(&dto.friend_code),
        avatar: avatar(&dto.avatar, &display_name),
        display_name,
        kind: if s(&dto.kind) == Some("milestone") {
            PostKind::Milestone
        } else {
            PostKind::Session
        },
        subject: text(&dto.subject, MAX_SUBJECT_CHARS),
        detail: text(&dto.detail, MAX_SUBJECT_CHARS),
        note: display_paragraph(s(&dto.note).unwrap_or(""), MAX_NOTE_CHARS),
        icon: text(&dto.icon, MAX_ICON_CHARS),
        minutes: count(&dto.minutes, MAX_TOTAL_MINUTES),
        preset_label: text(&dto.preset_label, MAX_SUBJECT_CHARS),
        created_at: ts(&dto.created_at),
        is_self: dto.is_self.0.unwrap_or(false),
        image_expired: s(&dto.image_expired_at).is_some(),
        image,
        poll: dto.poll.0.as_ref().map(poll),
        reactions,
        reacted,
        reacted_by,
        comments,
    })
}

fn feed_rows(rows: &Rows<FeedPostDto, MAX_FEED_ROWS>) -> Vec<FeedPost> {
    let mut seen = std::collections::HashSet::new();
    rows.rows
        .iter()
        .filter_map(feed_post)
        .filter(|p| seen.insert(p.id.clone()))
        .collect()
}

/// `{ month, storageBytes, classAOps, classBOps, warning, paused, limits }` (owner only).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct R2Usage {
    pub storage_bytes: u64,
    pub class_a_ops: u64,
    pub class_b_ops: u64,
    pub warning: bool,
    pub paused: bool,
    pub storage_hard_bytes: u64,
    pub class_a_hard: u64,
    pub class_b_hard: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct R2LimitsDto {
    #[serde(default)]
    storage_hard_bytes: Lenient<f64>,
    #[serde(default)]
    class_a_hard_monthly: Lenient<f64>,
    #[serde(default)]
    class_b_hard_monthly: Lenient<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct R2UsageDto {
    #[serde(default)]
    storage_bytes: Lenient<f64>,
    #[serde(default)]
    class_a_ops: Lenient<f64>,
    #[serde(default)]
    class_b_ops: Lenient<f64>,
    #[serde(default)]
    warning: Lenient<bool>,
    #[serde(default)]
    paused: Lenient<bool>,
    #[serde(default)]
    limits: Lenient<R2LimitsDto>,
}

const BIG: u64 = 1 << 50;

fn r2(dto: &R2UsageDto) -> R2Usage {
    let l = dto.limits.0.as_ref();
    R2Usage {
        storage_bytes: count(&dto.storage_bytes, BIG),
        class_a_ops: count(&dto.class_a_ops, BIG),
        class_b_ops: count(&dto.class_b_ops, BIG),
        warning: dto.warning.0.unwrap_or(false),
        paused: dto.paused.0.unwrap_or(false),
        storage_hard_bytes: l.map_or(0, |l| count(&l.storage_hard_bytes, BIG)),
        class_a_hard: l.map_or(0, |l| count(&l.class_a_hard_monthly, BIG)),
        class_b_hard: l.map_or(0, |l| count(&l.class_b_hard_monthly, BIG)),
    }
}

#[derive(Serialize)]
struct FeedBody<'a> {
    scope: &'static str,
    #[serde(flatten)]
    auth: Auth<'a>,
}

/// `getSocialFeed` -> `POST /feed` `{ scope, userId, deviceSecret }`.
pub fn feed(id: &SocialIdentity, scope: FeedScope) -> ApiRequest {
    json_request(
        ApiPath::Feed,
        &FeedBody {
            scope: scope.wire(),
            auth: auth(id),
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedReplyDto {
    #[serde(default)]
    feed: Option<Rows<FeedPostDto, MAX_FEED_ROWS>>,
    #[serde(default)]
    r2_usage: Lenient<R2UsageDto>,
}

/// `{ feed, r2Usage? }`. A reply without `feed` is malformed.
pub fn parse_feed(resp: &HttpResponse) -> Result<(Vec<FeedPost>, Option<R2Usage>), NetError> {
    let dto: FeedReplyDto = parse_json(resp)?;
    let rows = dto.feed.ok_or(NetError::Malformed)?;
    Ok((feed_rows(&rows), dto.r2_usage.0.as_ref().map(r2)))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PostBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    post_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReactBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    post_id: &'a str,
    emoji: &'a str,
}

/// `reactToFeedPost` -> `POST /feed/react` (the server toggles). `{ ok }`.
pub fn react(id: &SocialIdentity, post: &PostId, emoji: &str) -> ApiRequest {
    json_request(
        ApiPath::FeedReact,
        &ReactBody {
            auth: auth(id),
            post_id: post.as_str(),
            emoji,
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VoteBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    post_id: &'a str,
    option_id: &'a str,
}

/// `voteOnFeedPoll` -> `POST /feed/poll/vote` (the server toggles; one-answer polls replace).
pub fn poll_vote(id: &SocialIdentity, post: &PostId, option: &PollOptionId) -> ApiRequest {
    json_request(
        ApiPath::FeedPollVote,
        &VoteBody {
            auth: auth(id),
            post_id: post.as_str(),
            option_id: option.as_str(),
        },
    )
}

#[derive(Debug, Deserialize)]
struct VoteReplyDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    poll: Lenient<PollDto>,
}

/// `{ ok, poll | null }`: the authoritative poll after the vote (`None` = the post has none).
pub fn parse_poll_vote(resp: &HttpResponse) -> Result<Option<Poll>, NetError> {
    let dto: VoteReplyDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    Ok(dto.poll.0.as_ref().map(poll))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommentBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    post_id: &'a str,
    body: &'a str,
}

/// `commentOnFeedPost` -> `POST /feed/comment`.
pub fn comment_create(id: &SocialIdentity, post: &PostId, body: &str) -> ApiRequest {
    json_request(
        ApiPath::FeedComment,
        &CommentBody {
            auth: auth(id),
            post_id: post.as_str(),
            body,
        },
    )
}

#[derive(Debug, Deserialize)]
struct CommentReplyDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    comment: Lenient<CommentDto>,
}

/// `{ ok, comment }`.
pub fn parse_comment(resp: &HttpResponse, post: &PostId) -> Result<Comment, NetError> {
    let dto: CommentReplyDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    dto.comment
        .0
        .as_ref()
        .and_then(|c| comment(c, post))
        .ok_or(NetError::Malformed)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    post_id: &'a str,
    note: &'a str,
}

/// `updateFeedPost` -> `POST /feed/update` `{ postId, note }`. `{ ok }`.
pub fn post_update(id: &SocialIdentity, post: &PostId, note: &str) -> ApiRequest {
    json_request(
        ApiPath::FeedUpdate,
        &UpdateBody {
            auth: auth(id),
            post_id: post.as_str(),
            note,
        },
    )
}

/// `deleteFeedPost` -> `POST /feed/delete`. `{ ok }`.
pub fn post_delete(id: &SocialIdentity, post: &PostId) -> ApiRequest {
    json_request(
        ApiPath::FeedDelete,
        &PostBody {
            auth: auth(id),
            post_id: post.as_str(),
        },
    )
}

/// `deleteFeedPostImage` -> `POST /feed/image/delete`. `{ ok, r2Usage? }`.
pub fn post_image_delete(id: &SocialIdentity, post: &PostId) -> ApiRequest {
    json_request(
        ApiPath::FeedImageDelete,
        &PostBody {
            auth: auth(id),
            post_id: post.as_str(),
        },
    )
}

fn multipart_request(
    path: ApiPath,
    fields: &[(&str, &str)],
    file: (&str, &str, &str, &[u8]),
) -> Result<ApiRequest, NetError> {
    let (content_type, bytes) = (0..8)
        .find_map(|_| {
            fields
                .iter()
                .fold(Multipart::new(), |m, (k, v)| m.text(k, v))
                .file(file.0, file.1, file.2, file.3.to_vec())
                .finish()
        })
        .ok_or(NetError::Blocked)?;
    Ok(ApiRequest {
        method: Method::Post,
        target: Target::Api(path),
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

/// A prepared upload: the encoded bytes and their type (the Worker accepts PNG, JPEG, WebP and
/// GIF for feed images and avatars).
#[derive(Clone, PartialEq, Eq)]
pub struct Upload {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

impl fmt::Debug for Upload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Upload({} {} bytes)", self.mime, self.bytes.len())
    }
}

fn extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        _ => "webp",
    }
}

/// `uploadFeedPostImage` -> multipart `POST /feed/image` (`userId`, `deviceSecret`, `postId`,
/// `image`). Refused locally above the Worker's 5 MB.
pub fn post_image_upload(
    id: &SocialIdentity,
    post: &PostId,
    upload: &Upload,
) -> Result<ApiRequest, NetError> {
    if upload.bytes.len() > MAX_FEED_UPLOAD_BYTES {
        return Err(NetError::PayloadTooLarge(Some(
            "Image is too large. Use an image under 5 MB.".into(),
        )));
    }
    multipart_request(
        ApiPath::FeedImageUpload,
        &[
            ("userId", id.user_id.as_str()),
            ("deviceSecret", id.device_secret.expose()),
            ("postId", post.as_str()),
        ],
        (
            "image",
            &format!("feed-image.{}", extension(upload.mime)),
            upload.mime,
            &upload.bytes,
        ),
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageReplyDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    image_url: Lenient<String>,
    #[serde(default)]
    image_mime_type: Lenient<String>,
    #[serde(default)]
    image_expires_at: Lenient<String>,
    #[serde(default)]
    r2_usage: Lenient<R2UsageDto>,
}

/// `{ ok, imageUrl, imageMimeType, imageExpiresAt, imageExpiredAt: null, r2Usage? }`.
pub fn parse_post_image(resp: &HttpResponse) -> Result<(FeedImage, Option<R2Usage>), NetError> {
    let dto: ImageReplyDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    let image = FeedImage {
        url: url(s(&dto.image_url)).ok_or(NetError::Malformed)?,
        mime_type: s(&dto.image_mime_type).map(|m| display_text(m, 40)),
        expires_at: ts(&dto.image_expires_at),
    };
    Ok((image, dto.r2_usage.0.as_ref().map(r2)))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OkUsageDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    r2_usage: Lenient<R2UsageDto>,
}

/// `{ ok, r2Usage? }`.
pub fn parse_ok_usage(resp: &HttpResponse) -> Result<Option<R2Usage>, NetError> {
    let dto: OkUsageDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    Ok(dto.r2_usage.0.as_ref().map(r2))
}

/// `uploadProfileAvatar` -> multipart `POST /profile/avatar` (`userId`, `deviceSecret`, `name`,
/// `image`). Refused locally above the Worker's 256 KiB.
pub fn avatar_upload(
    id: &SocialIdentity,
    upload: &Upload,
    file_name: &str,
) -> Result<ApiRequest, NetError> {
    if upload.bytes.len() > MAX_AVATAR_UPLOAD_BYTES {
        return Err(NetError::PayloadTooLarge(Some(
            "Avatar image is too large.".into(),
        )));
    }
    let name = if file_name.is_empty() {
        format!("avatar.{}", extension(upload.mime))
    } else {
        file_name.to_string()
    };
    multipart_request(
        ApiPath::ProfileAvatar,
        &[
            ("userId", id.user_id.as_str()),
            ("deviceSecret", id.device_secret.expose()),
            ("name", &name),
        ],
        ("image", &name, upload.mime, &upload.bytes),
    )
}

#[derive(Debug, Deserialize)]
struct AvatarReplyDto {
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
}

/// `{ avatar }`: the stored photo avatar (anything but a photo is malformed here).
pub fn parse_avatar(resp: &HttpResponse, display_name: &str) -> Result<Avatar, NetError> {
    let dto: AvatarReplyDto = parse_json(resp)?;
    if dto.avatar.0.is_none() {
        return Err(NetError::Malformed);
    }
    match avatar(&dto.avatar, display_name) {
        a @ Avatar::Photo { .. } if a.remote_photo_url().is_some() => Ok(a),
        _ => Err(NetError::Malformed),
    }
}

/// One queued post as `/sync/v2` sends it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPostWire<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    kind: &'static str,
    subject: &'a str,
    detail: &'a str,
    note: &'a str,
    icon: &'a str,
    minutes: u64,
    preset_label: &'a str,
    created_at: String,
    poll: Option<PollWire<'a>>,
}

#[derive(Serialize)]
struct PollWire<'a> {
    question: &'a str,
    multiple: bool,
    options: Vec<PollOptionWire<'a>>,
}

#[derive(Serialize)]
struct PollOptionWire<'a> {
    id: &'a str,
    text: &'a str,
}

impl<'a> From<&'a PendingPost> for FeedPostWire<'a> {
    fn from(p: &'a PendingPost) -> Self {
        Self {
            id: p.id.as_str(),
            kind: p.kind.wire(),
            subject: &p.subject,
            detail: &p.detail,
            note: &p.note,
            icon: &p.icon,
            minutes: p.minutes,
            preset_label: &p.preset_label,
            created_at: p.created_at.to_iso(),
            poll: p.poll.as_ref().map(|poll| PollWire {
                question: &poll.question,
                multiple: poll.multiple,
                options: poll
                    .options
                    .iter()
                    .map(|(id, text)| PollOptionWire {
                        id: id.as_str(),
                        text,
                    })
                    .collect(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------------- squads

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadMemberDto {
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    role: Lenient<String>,
    #[serde(default)]
    joined_at: Lenient<String>,
    #[serde(default)]
    last_seen_at: Lenient<String>,
    #[serde(default)]
    minutes: Lenient<f64>,
    #[serde(default)]
    sessions: Lenient<f64>,
    #[serde(default)]
    is_self: Lenient<bool>,
}

fn member(dto: &SquadMemberDto) -> Option<SquadMember> {
    let display_name = name(s(&dto.display_name));
    Some(SquadMember {
        user_id: UserId::parse(s(&dto.user_id)?)?,
        friend_code: code(&dto.friend_code),
        avatar: avatar(&dto.avatar, &display_name),
        role: SquadRole::parse(s(&dto.role).unwrap_or("")),
        joined_at: ts(&dto.joined_at),
        last_seen_at: ts(&dto.last_seen_at),
        minutes: count(&dto.minutes, MAX_TOTAL_MINUTES),
        sessions: count(&dto.sessions, MAX_TOTAL_SESSIONS),
        is_self: dto.is_self.0.unwrap_or(false),
        display_name,
    })
}

fn members(rows: &Rows<SquadMemberDto, MAX_MEMBERS_RECEIVED>) -> Vec<SquadMember> {
    let mut seen = std::collections::HashSet::new();
    rows.rows
        .iter()
        .filter_map(member)
        .filter(|m| seen.insert(m.user_id.clone()))
        .collect()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    name: Lenient<String>,
    #[serde(default)]
    is_private: Lenient<bool>,
    #[serde(default)]
    created_at: Lenient<String>,
    #[serde(default)]
    total_minutes: Lenient<f64>,
    #[serde(default)]
    total_sessions: Lenient<f64>,
    #[serde(default)]
    member_count: Lenient<f64>,
    #[serde(default)]
    my_role: Lenient<String>,
    #[serde(default)]
    members: Rows<SquadMemberDto, MAX_MEMBERS_RECEIVED>,
}

fn squad_name(raw: &Lenient<String>) -> String {
    let n = text(raw, 64);
    if n.trim().is_empty() {
        "Squad".into()
    } else {
        n
    }
}

fn squad(dto: &SquadDto) -> Option<Squad> {
    Some(Squad {
        id: SquadId::parse(s(&dto.id)?)?,
        name: squad_name(&dto.name),
        is_private: dto.is_private.0.unwrap_or(false),
        created_at: ts(&dto.created_at),
        total_minutes: count(&dto.total_minutes, MAX_TOTAL_MINUTES),
        total_sessions: count(&dto.total_sessions, MAX_TOTAL_SESSIONS),
        member_count: count(&dto.member_count, 1000),
        my_role: SquadRole::parse(s(&dto.my_role).unwrap_or("")),
        members: members(&dto.members),
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadRequestDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    squad_id: Lenient<String>,
    #[serde(default)]
    squad_name: Lenient<String>,
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    created_at: Lenient<String>,
}

fn squad_request(dto: &SquadRequestDto) -> Option<SquadJoinRequest> {
    let display_name = s(&dto.display_name).map(|n| name(Some(n)));
    Some(SquadJoinRequest {
        id: RequestId::parse(s(&dto.id)?)?,
        squad_id: SquadId::parse(s(&dto.squad_id)?)?,
        squad_name: s(&dto.squad_name).map(|n| display_text(n, 64)),
        user_id: s(&dto.user_id).and_then(UserId::parse),
        friend_code: code(&dto.friend_code),
        avatar: dto
            .avatar
            .0
            .as_ref()
            .map(|_| avatar(&dto.avatar, display_name.as_deref().unwrap_or("Student"))),
        display_name,
        created_at: ts(&dto.created_at),
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadMessageDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    avatar: Lenient<AvatarDto>,
    #[serde(default)]
    role: Lenient<String>,
    #[serde(default)]
    body: Lenient<String>,
    #[serde(default)]
    created_at: Lenient<String>,
    #[serde(default)]
    is_self: Lenient<bool>,
}

fn message(dto: &SquadMessageDto) -> Option<SquadMessage> {
    let display_name = name(s(&dto.display_name));
    Some(SquadMessage {
        id: MessageId::parse(s(&dto.id)?)?,
        user_id: UserId::parse(s(&dto.user_id)?)?,
        friend_code: code(&dto.friend_code),
        avatar: avatar(&dto.avatar, &display_name),
        role: SquadRole::parse(s(&dto.role).unwrap_or("")),
        body: display_paragraph(s(&dto.body).unwrap_or(""), MAX_BODY_CHARS),
        created_at: ts(&dto.created_at),
        is_self: dto.is_self.0.unwrap_or(false),
        display_name,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadScoreDto {
    #[serde(default)]
    squad_id: Lenient<String>,
    #[serde(default)]
    squad_name: Lenient<String>,
    #[serde(default)]
    is_private: Lenient<bool>,
    #[serde(default)]
    member_count: Lenient<f64>,
    #[serde(default)]
    total_minutes: Lenient<f64>,
    #[serde(default)]
    total_sessions: Lenient<f64>,
    #[serde(default)]
    average_minutes: Lenient<f64>,
    #[serde(default)]
    rank: Lenient<f64>,
    #[serde(default)]
    points: Lenient<f64>,
    #[serde(default)]
    scored_days: Lenient<f64>,
}

fn finite_nonneg(v: &Lenient<f64>, max: f64) -> f64 {
    match v.0 {
        Some(x) if x.is_finite() && x > 0.0 => x.min(max),
        _ => 0.0,
    }
}

fn score_entry(dto: &SquadScoreDto) -> Option<SquadScoreEntry> {
    Some(SquadScoreEntry {
        squad_id: SquadId::parse(s(&dto.squad_id)?)?,
        squad_name: squad_name(&dto.squad_name),
        is_private: dto.is_private.0.unwrap_or(false),
        member_count: count(&dto.member_count, 1000),
        total_minutes: count(&dto.total_minutes, MAX_TOTAL_MINUTES),
        total_sessions: count(&dto.total_sessions, MAX_TOTAL_SESSIONS),
        average_minutes: finite_nonneg(&dto.average_minutes, MAX_TOTAL_MINUTES as f64),
        rank: count(&dto.rank, u64::from(u32::MAX)) as u32,
        points: score(&dto.points),
        scored_days: dto.scored_days.0.map(|_| count(&dto.scored_days, 100_000)),
    })
}

fn score_rows(rows: &Rows<SquadScoreDto, MAX_SCOREBOARD_ROWS>) -> Vec<SquadScoreEntry> {
    let mut seen = std::collections::HashSet::new();
    rows.rows
        .iter()
        .filter_map(score_entry)
        .filter(|e| seen.insert(e.squad_id.clone()))
        .collect()
}

#[derive(Debug, Clone, Default, Deserialize)]
struct FeedsDto {
    #[serde(default)]
    global: Option<Rows<FeedPostDto, MAX_FEED_ROWS>>,
    #[serde(default)]
    friends: Option<Rows<FeedPostDto, MAX_FEED_ROWS>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ScoresDto {
    #[serde(default)]
    daily: Option<Rows<SquadScoreDto, MAX_SCOREBOARD_ROWS>>,
    #[serde(default)]
    season: Option<Rows<SquadScoreDto, MAX_SCOREBOARD_ROWS>>,
    #[serde(default)]
    overall: Option<Rows<SquadScoreDto, MAX_SCOREBOARD_ROWS>>,
}

/// The 22b keys of a social snapshot (flattened into the 22a DTO). `squad` is `null` for a user
/// without one; a missing `squad` key means the reply did not carry the squad part at all.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotMoreDto {
    #[serde(default, deserialize_with = "present")]
    squad: Option<Lenient<SquadDto>>,
    #[serde(default)]
    incoming_squad_requests: Option<Rows<SquadRequestDto, MAX_SQUAD_REQUESTS>>,
    #[serde(default)]
    outgoing_squad_requests: Option<Rows<SquadRequestDto, MAX_SQUAD_REQUESTS>>,
    #[serde(default)]
    squad_messages: Option<Rows<SquadMessageDto, MAX_MESSAGES_RECEIVED>>,
    #[serde(default)]
    cached_feeds: Lenient<FeedsDto>,
    #[serde(default)]
    cached_squad_score_leaderboards: Lenient<ScoresDto>,
}

/// A field that is `Some` whenever the key is present (even with `null`).
fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Lenient<SquadDto>>, D::Error> {
    Lenient::<SquadDto>::deserialize(d).map(Some)
}

type MoreParts = (
    Option<SquadSnapshot>,
    Vec<(FeedScope, Vec<FeedPost>)>,
    Vec<(SquadScorePeriod, Vec<SquadScoreEntry>)>,
);

pub(super) fn snapshot_more(dto: SnapshotMoreDto) -> MoreParts {
    // production spreads `getSquadSnapshot`'s four keys together; all four or none
    let squad_part = match (
        dto.squad,
        dto.incoming_squad_requests,
        dto.outgoing_squad_requests,
        dto.squad_messages,
    ) {
        (Some(squad_dto), Some(inc), Some(out), Some(msgs)) => {
            let squad_value = squad_dto.0.as_ref().and_then(squad);
            let mut seen = std::collections::HashSet::new();
            let messages = msgs
                .rows
                .iter()
                .filter_map(message)
                .filter(|m| seen.insert(m.id.clone()))
                .collect();
            Some(SquadSnapshot {
                squad: squad_value,
                incoming: inc.rows.iter().filter_map(squad_request).collect(),
                outgoing: out.rows.iter().filter_map(squad_request).collect(),
                messages,
            })
        }
        _ => None,
    };
    let mut feeds = Vec::new();
    if let Some(f) = dto.cached_feeds.0 {
        if let Some(rows) = f.friends {
            feeds.push((FeedScope::Friends, feed_rows(&rows)));
        }
        if let Some(rows) = f.global {
            feeds.push((FeedScope::Global, feed_rows(&rows)));
        }
    }
    let mut scores = Vec::new();
    if let Some(sc) = dto.cached_squad_score_leaderboards.0 {
        for (period, rows) in [
            (SquadScorePeriod::Daily, sc.daily),
            (SquadScorePeriod::Season, sc.season),
            (SquadScorePeriod::Overall, sc.overall),
        ] {
            if let Some(rows) = rows {
                scores.push((period, score_rows(&rows)));
            }
        }
    }
    (squad_part, feeds, scores)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SquadCreateBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    name: &'a str,
    is_private: bool,
}

/// `createSquad` -> `POST /squads/create` (409 when already in one). Full snapshot.
pub fn squad_create(id: &SocialIdentity, name: &str, is_private: bool) -> ApiRequest {
    json_request(
        ApiPath::SquadsCreate,
        &SquadCreateBody {
            auth: auth(id),
            name,
            is_private,
        },
    )
}

/// `updateSquadSettings` -> `POST /squads/settings` (leader only). Full snapshot.
pub fn squad_settings(id: &SocialIdentity, name: &str, is_private: bool) -> ApiRequest {
    json_request(
        ApiPath::SquadsSettings,
        &SquadCreateBody {
            auth: auth(id),
            name,
            is_private,
        },
    )
}

#[derive(Serialize)]
struct QueryBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    query: &'a str,
}

/// `searchSquads` -> `POST /squads/search` (`""` lists the suggestion pool).
pub fn squad_search(id: &SocialIdentity, query: &str) -> ApiRequest {
    json_request(
        ApiPath::SquadsSearch,
        &QueryBody {
            auth: auth(id),
            query,
        },
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SquadSearchDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    name: Lenient<String>,
    #[serde(default)]
    is_private: Lenient<bool>,
    #[serde(default)]
    member_count: Lenient<f64>,
    #[serde(default)]
    max_members: Lenient<f64>,
    #[serde(default)]
    total_minutes: Lenient<f64>,
    #[serde(default)]
    total_sessions: Lenient<f64>,
    #[serde(default)]
    action: Lenient<String>,
}

#[derive(Debug, Deserialize)]
struct SearchReplyDto {
    #[serde(default)]
    squads: Option<Rows<SquadSearchDto, MAX_SEARCH_ROWS>>,
}

/// `{ squads: [...] }`.
pub fn parse_squad_search(resp: &HttpResponse) -> Result<Vec<SquadSearchResult>, NetError> {
    let dto: SearchReplyDto = parse_json(resp)?;
    let rows = dto.squads.ok_or(NetError::Malformed)?;
    let mut seen = std::collections::HashSet::new();
    Ok(rows
        .rows
        .iter()
        .filter_map(|r| {
            Some(SquadSearchResult {
                id: SquadId::parse(s(&r.id)?)?,
                name: squad_name(&r.name),
                is_private: r.is_private.0.unwrap_or(false),
                member_count: count(&r.member_count, 1000),
                max_members: count(&r.max_members, 1000),
                total_minutes: count(&r.total_minutes, MAX_TOTAL_MINUTES),
                total_sessions: count(&r.total_sessions, MAX_TOTAL_SESSIONS),
                action: SquadAction::parse(s(&r.action).unwrap_or("")),
            })
        })
        .filter(|r| seen.insert(r.id.clone()))
        .collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SquadIdBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    squad_id: &'a str,
}

/// `getSquadDetails` -> `POST /squads/details`.
pub fn squad_details(id: &SocialIdentity, squad: &SquadId) -> ApiRequest {
    json_request(
        ApiPath::SquadsDetails,
        &SquadIdBody {
            auth: auth(id),
            squad_id: squad.as_str(),
        },
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SquadDetailsDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    name: Lenient<String>,
    #[serde(default)]
    is_private: Lenient<bool>,
    #[serde(default)]
    total_minutes: Lenient<f64>,
    #[serde(default)]
    total_sessions: Lenient<f64>,
    #[serde(default)]
    member_count: Lenient<f64>,
    #[serde(default)]
    max_members: Lenient<f64>,
    #[serde(default)]
    previous_day_average_minutes: Lenient<f64>,
    #[serde(default)]
    action: Lenient<String>,
    #[serde(default)]
    members: Rows<SquadMemberDto, MAX_MEMBERS_RECEIVED>,
}

#[derive(Debug, Deserialize)]
struct DetailsReplyDto {
    #[serde(default)]
    squad: Lenient<SquadDetailsDto>,
}

/// `{ squad }`.
pub fn parse_squad_details(resp: &HttpResponse) -> Result<SquadDetails, NetError> {
    let dto: DetailsReplyDto = parse_json(resp)?;
    let d = dto.squad.0.ok_or(NetError::Malformed)?;
    Ok(SquadDetails {
        id: s(&d.id)
            .and_then(SquadId::parse)
            .ok_or(NetError::Malformed)?,
        name: squad_name(&d.name),
        is_private: d.is_private.0.unwrap_or(false),
        total_minutes: count(&d.total_minutes, MAX_TOTAL_MINUTES),
        total_sessions: count(&d.total_sessions, MAX_TOTAL_SESSIONS),
        member_count: count(&d.member_count, 1000),
        max_members: count(&d.max_members, 1000),
        previous_day_average_minutes: finite_nonneg(
            &d.previous_day_average_minutes,
            MAX_TOTAL_MINUTES as f64,
        ),
        action: SquadAction::parse(s(&d.action).unwrap_or("")),
        members: members(&d.members),
    })
}

/// `joinSquad` -> `POST /squads/join` (a private squad records a request). Full snapshot.
pub fn squad_join(id: &SocialIdentity, squad: &SquadId) -> ApiRequest {
    json_request(
        ApiPath::SquadsJoin,
        &SquadIdBody {
            auth: auth(id),
            squad_id: squad.as_str(),
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SquadRespondBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    request_id: &'a str,
    response: &'static str,
}

/// `respondToSquadRequest` -> `POST /squads/respond`. Full snapshot.
pub fn squad_respond(id: &SocialIdentity, request: &RequestId, accept: bool) -> ApiRequest {
    json_request(
        ApiPath::SquadsRespond,
        &SquadRespondBody {
            auth: auth(id),
            request_id: request.as_str(),
            response: if accept { "accepted" } else { "declined" },
        },
    )
}

/// `leaveSquad` -> `POST /squads/leave` (the last member deletes the squad). Full snapshot.
pub fn squad_leave(id: &SocialIdentity) -> ApiRequest {
    json_request(ApiPath::SquadsLeave, &auth(id))
}

#[derive(Serialize)]
struct ChatBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    body: &'a str,
}

/// `sendSquadMessage` -> `POST /squads/chat`. Full snapshot.
pub fn squad_chat(id: &SocialIdentity, body: &str) -> ApiRequest {
    json_request(
        ApiPath::SquadsChat,
        &ChatBody {
            auth: auth(id),
            body,
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MessageBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    message_id: &'a str,
}

/// `deleteSquadMessage` -> `POST /squads/chat/delete` (own messages only). Full snapshot.
pub fn squad_chat_delete(id: &SocialIdentity, message: &MessageId) -> ApiRequest {
    json_request(
        ApiPath::SquadsChatDelete,
        &MessageBody {
            auth: auth(id),
            message_id: message.as_str(),
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RoleBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    target_user_id: &'a str,
    role: &'static str,
}

/// `setSquadMemberRole` -> `POST /squads/promote`. Full snapshot.
pub fn squad_role(id: &SocialIdentity, target: &UserId, role: SquadRole) -> ApiRequest {
    json_request(
        ApiPath::SquadsPromote,
        &RoleBody {
            auth: auth(id),
            target_user_id: target.as_str(),
            role: role.wire(),
        },
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    target_user_id: &'a str,
}

/// `kickSquadMember` -> `POST /squads/kick`. Full snapshot.
pub fn squad_kick(id: &SocialIdentity, target: &UserId) -> ApiRequest {
    json_request(
        ApiPath::SquadsKick,
        &TargetBody {
            auth: auth(id),
            target_user_id: target.as_str(),
        },
    )
}

#[derive(Serialize)]
struct PeriodBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    period: &'static str,
}

/// `getSquadScoreboard` -> `POST /squads/scoreboard`.
pub fn squad_scoreboard(id: &SocialIdentity, period: SquadScorePeriod) -> ApiRequest {
    json_request(
        ApiPath::SquadsScoreboard,
        &PeriodBody {
            auth: auth(id),
            period: period.wire(),
        },
    )
}

#[derive(Debug, Deserialize)]
struct EntriesDto {
    #[serde(default)]
    entries: Option<Rows<SquadScoreDto, MAX_SCOREBOARD_ROWS>>,
}

/// `{ entries: [...] }`.
pub fn parse_squad_scoreboard(resp: &HttpResponse) -> Result<Vec<SquadScoreEntry>, NetError> {
    let dto: EntriesDto = parse_json(resp)?;
    Ok(score_rows(&dto.entries.ok_or(NetError::Malformed)?))
}

// ------------------------------------------------------------------------- verified sessions

/// `startVerifiedSession` -> `POST /verified-session/start` `{ userId, deviceSecret }`.
pub fn verified_start(id: &SocialIdentity) -> ApiRequest {
    json_request(ApiPath::VerifiedStart, &auth(id))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    session_id: &'a str,
}

/// `heartbeatVerifiedSession` -> `POST /verified-session/heartbeat`.
pub fn verified_heartbeat(id: &SocialIdentity, session: &VerifiedSessionId) -> ApiRequest {
    json_request(
        ApiPath::VerifiedHeartbeat,
        &SessionBody {
            auth: auth(id),
            session_id: session.as_str(),
        },
    )
}

/// `finishVerifiedSession` -> `POST /verified-session/finish`.
pub fn verified_finish(id: &SocialIdentity, session: &VerifiedSessionId) -> ApiRequest {
    json_request(
        ApiPath::VerifiedFinish,
        &SessionBody {
            auth: auth(id),
            session_id: session.as_str(),
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartReplyDto {
    #[serde(default)]
    session_id: Lenient<String>,
}

/// `{ sessionId, startedAt, resumed }`: the session id.
pub fn parse_verified_start(resp: &HttpResponse) -> Result<VerifiedSessionId, NetError> {
    let dto: StartReplyDto = parse_json(resp)?;
    s(&dto.session_id)
        .and_then(VerifiedSessionId::parse)
        .ok_or(NetError::Malformed)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IntervalWire {
    started_at: String,
    ended_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconcileBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    anchor_session_id: &'a str,
    intervals: Vec<IntervalWire>,
    chain_tip_hash: &'a str,
}

/// `reconcileOfflineVerifiedCredit` -> `POST /verified-session/reconcile-offline`. The caller
/// passes the SHA-256 (hex) of `verified::canonical_intervals(intervals)`.
pub fn verified_reconcile(
    id: &SocialIdentity,
    anchor: &VerifiedSessionId,
    intervals: &[Interval],
    chain_tip_hash: &str,
) -> ApiRequest {
    json_request(
        ApiPath::VerifiedReconcile,
        &ReconcileBody {
            auth: auth(id),
            anchor_session_id: anchor.as_str(),
            intervals: intervals
                .iter()
                .map(|i| IntervalWire {
                    started_at: i.started_at.to_iso(),
                    ended_at: i.ended_at.to_iso(),
                })
                .collect(),
            chain_tip_hash,
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReconcileReplyDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    capped_from_claimed_minutes: Lenient<f64>,
}

/// `{ ok, creditedMinutes, cappedFromClaimedMinutes, flagged }`: the capped minutes.
pub fn parse_reconcile(resp: &HttpResponse) -> Result<u64, NetError> {
    let dto: ReconcileReplyDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    Ok(count(&dto.capped_from_claimed_minutes, 24 * 60 * 1000))
}

// ------------------------------------------------------------------ announcements, telemetry

/// `getCurrentAnnouncement` -> `GET /announcements/current?appVersion=` (anonymous).
pub fn announcement_current(app: &AppMetadata) -> ApiRequest {
    ApiRequest {
        method: Method::Get,
        target: Target::Api(ApiPath::AnnouncementsCurrent),
        query: vec![("appVersion", app.version.clone())],
        body: Body::None,
        max_response: MAX_API_RESPONSE,
        timeout: API_TIMEOUT,
        priority: Priority::Api,
    }
}

#[derive(Debug, Deserialize)]
struct AnnouncementDto {
    #[serde(default)]
    id: Lenient<String>,
    #[serde(default)]
    title: Lenient<String>,
    #[serde(default)]
    body: Lenient<String>,
}

#[derive(Debug, Deserialize)]
struct AnnouncementReplyDto {
    #[serde(default)]
    announcement: Lenient<AnnouncementDto>,
}

/// `{ announcement | null }`.
pub fn parse_announcement(resp: &HttpResponse) -> Result<Option<Announcement>, NetError> {
    let dto: AnnouncementReplyDto = parse_json(resp)?;
    Ok(dto.announcement.0.and_then(|a| {
        let id = display_text(s(&a.id)?, 120);
        (!id.trim().is_empty()).then(|| Announcement {
            id,
            title: text(&a.title, 200),
            body: display_paragraph(s(&a.body).unwrap_or(""), 600),
        })
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NoticeBody<'a> {
    #[serde(flatten)]
    auth: Auth<'a>,
    target_version: &'a str,
}

/// `notifyUsersAboutUpdate` -> `POST /announcements/update-notice` (owner only).
pub fn update_notice(id: &SocialIdentity, target_version: &str) -> ApiRequest {
    json_request(
        ApiPath::AnnouncementsUpdateNotice,
        &NoticeBody {
            auth: auth(id),
            target_version,
        },
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NoticeReplyDto {
    #[serde(default)]
    ok: Lenient<bool>,
    #[serde(default)]
    target_version: Lenient<String>,
}

/// `{ ok, id, targetVersion }`: the version the notice targets.
pub fn parse_update_notice(resp: &HttpResponse) -> Result<String, NetError> {
    let dto: NoticeReplyDto = parse_json(resp)?;
    if dto.ok.0 != Some(true) {
        return Err(NetError::Malformed);
    }
    Ok(text(&dto.target_version, 40))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TelemetryBody<'a> {
    install_id: &'a str,
    app: AppDto<'a>,
}

/// `sendTelemetryHeartbeat` -> `POST /telemetry/heartbeat` `{ installId, app }`. No account,
/// no credential, no device fingerprint.
pub fn telemetry_heartbeat(install: &InstallId, app: &AppMetadata) -> ApiRequest {
    json_request(
        ApiPath::TelemetryHeartbeat,
        &TelemetryBody {
            install_id: install.as_str(),
            app: app.into(),
        },
    )
}

// ---------------------------------------------------------------------- owner: usage view

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdminSummary {
    pub user_count: u64,
    pub active_24h: u64,
    pub active_7d: u64,
    pub users_with_app_version: u64,
    pub flagged_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminUser {
    pub display_name: String,
    pub friend_code: String,
    pub last_seen_at: String,
    pub device_label: Option<String>,
    pub device_fingerprint_hash: Option<String>,
    pub app_version: Option<String>,
    pub app_platform: Option<String>,
    pub app_runtime_channel: Option<String>,
    pub app_seen_at: Option<String>,
    pub is_flagged: bool,
    pub flagged_reason: Option<String>,
    pub signup_country: Option<String>,
    pub signup_asn: Option<u64>,
    pub signup_as_organization: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminAbuseEvent {
    pub event_type: String,
    pub country: Option<String>,
    pub as_organization: Option<String>,
    pub path: Option<String>,
    pub user_agent: Option<String>,
    pub user_id: Option<String>,
    pub detail: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdminInstall {
    pub install_id: String,
    pub app_version: String,
    pub app_platform: String,
    pub app_runtime_channel: String,
    pub created_at: String,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AdminUsage {
    pub summary: Option<AdminSummary>,
    pub users: Vec<AdminUser>,
    pub flagged: Vec<AdminUser>,
    pub abuse: Vec<AdminAbuseEvent>,
    pub telemetry: Vec<AdminInstall>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSummaryDto {
    #[serde(default)]
    user_count: Lenient<f64>,
    #[serde(default, rename = "active24h")]
    active_24h: Lenient<f64>,
    #[serde(default, rename = "active7d")]
    active_7d: Lenient<f64>,
    #[serde(default)]
    users_with_app_version: Lenient<f64>,
    #[serde(default)]
    flagged_count: Lenient<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminUserDto {
    #[serde(default)]
    display_name: Lenient<String>,
    #[serde(default)]
    friend_code: Lenient<String>,
    #[serde(default)]
    last_seen_at: Lenient<String>,
    #[serde(default)]
    device_label: Lenient<String>,
    #[serde(default)]
    device_fingerprint_hash: Lenient<String>,
    #[serde(default)]
    app_version: Lenient<String>,
    #[serde(default)]
    app_platform: Lenient<String>,
    #[serde(default)]
    app_runtime_channel: Lenient<String>,
    #[serde(default)]
    app_seen_at: Lenient<String>,
    #[serde(default)]
    is_flagged: Lenient<f64>,
    #[serde(default)]
    flagged_reason: Lenient<String>,
    #[serde(default)]
    signup_country: Lenient<String>,
    #[serde(default)]
    signup_asn: Lenient<f64>,
    #[serde(default)]
    signup_as_organization: Lenient<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminAbuseDto {
    #[serde(default)]
    event_type: Lenient<String>,
    #[serde(default)]
    country: Lenient<String>,
    #[serde(default)]
    as_organization: Lenient<String>,
    #[serde(default)]
    path: Lenient<String>,
    #[serde(default)]
    user_agent: Lenient<String>,
    #[serde(default)]
    user_id: Lenient<String>,
    #[serde(default)]
    detail: Lenient<String>,
    #[serde(default)]
    created_at: Lenient<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminInstallDto {
    #[serde(default)]
    install_id: Lenient<String>,
    #[serde(default)]
    app_version: Lenient<String>,
    #[serde(default)]
    app_platform: Lenient<String>,
    #[serde(default)]
    app_runtime_channel: Lenient<String>,
    #[serde(default)]
    created_at: Lenient<String>,
    #[serde(default)]
    last_seen_at: Lenient<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminUsageDto {
    #[serde(default)]
    summary: Lenient<AdminSummaryDto>,
    #[serde(default)]
    users: Rows<AdminUserDto, 200>,
    #[serde(default)]
    flagged_users: Rows<AdminUserDto, 200>,
    #[serde(default)]
    abuse_events: Rows<AdminAbuseDto, 200>,
    #[serde(default)]
    telemetry: Rows<AdminInstallDto, 200>,
}

fn opt_text(v: &Lenient<String>, max: usize) -> Option<String> {
    s(v).map(|x| display_text(x, max)).filter(|x| !x.is_empty())
}

fn admin_user(d: &AdminUserDto) -> AdminUser {
    AdminUser {
        display_name: name(s(&d.display_name)),
        friend_code: text(&d.friend_code, 32),
        last_seen_at: text(&d.last_seen_at, 40),
        device_label: opt_text(&d.device_label, 120),
        device_fingerprint_hash: opt_text(&d.device_fingerprint_hash, 120),
        app_version: opt_text(&d.app_version, 40),
        app_platform: opt_text(&d.app_platform, 120),
        app_runtime_channel: opt_text(&d.app_runtime_channel, 80),
        app_seen_at: opt_text(&d.app_seen_at, 40),
        is_flagged: d.is_flagged.0.is_some_and(|f| f != 0.0),
        flagged_reason: opt_text(&d.flagged_reason, 300),
        signup_country: opt_text(&d.signup_country, 8),
        signup_asn: d
            .signup_asn
            .0
            .map(|_| count(&d.signup_asn, u64::from(u32::MAX))),
        signup_as_organization: opt_text(&d.signup_as_organization, 160),
    }
}

/// `getAdminUsage` -> `POST /admin/usage` (403 for anyone but the owner).
pub fn admin_usage(id: &SocialIdentity) -> ApiRequest {
    json_request(ApiPath::AdminUsage, &auth(id))
}

pub fn parse_admin_usage(resp: &HttpResponse) -> Result<AdminUsage, NetError> {
    let dto: AdminUsageDto = parse_json(resp)?;
    Ok(AdminUsage {
        summary: dto.summary.0.as_ref().map(|s| AdminSummary {
            user_count: count(&s.user_count, BIG),
            active_24h: count(&s.active_24h, BIG),
            active_7d: count(&s.active_7d, BIG),
            users_with_app_version: count(&s.users_with_app_version, BIG),
            flagged_count: count(&s.flagged_count, BIG),
        }),
        users: dto.users.rows.iter().map(admin_user).collect(),
        flagged: dto.flagged_users.rows.iter().map(admin_user).collect(),
        abuse: dto
            .abuse_events
            .rows
            .iter()
            .map(|e| AdminAbuseEvent {
                event_type: text(&e.event_type, 60),
                country: opt_text(&e.country, 8),
                as_organization: opt_text(&e.as_organization, 160),
                path: opt_text(&e.path, 200),
                user_agent: opt_text(&e.user_agent, 300),
                user_id: opt_text(&e.user_id, 80),
                detail: text(&e.detail, 300),
                created_at: text(&e.created_at, 40),
            })
            .collect(),
        telemetry: dto
            .telemetry
            .rows
            .iter()
            .map(|t| AdminInstall {
                install_id: text(&t.install_id, 80),
                app_version: text(&t.app_version, 40),
                app_platform: text(&t.app_platform, 120),
                app_runtime_channel: text(&t.app_runtime_channel, 80),
                created_at: text(&t.created_at, 40),
                last_seen_at: text(&t.last_seen_at, 40),
            })
            .collect(),
    })
}

#[cfg(test)]
#[path = "social_ext_tests.rs"]
mod tests;
