//! Request/response values, the application error model and redaction (Stage 22a).
//!
//! Redaction is structural, not a log filter:
//! - an [`ApiRequest`] keeps its path and its (possibly credential-bearing) query/body apart, and
//!   its `Debug` prints only the method, path and body *length*;
//! - the full URL with the query string exists only inside the transport, for the duration of
//!   the call, and is never formatted into a log line or an error;
//! - [`NetError`] carries no URL, header or body - at most a short, sanitised server message.

use std::fmt;
use std::time::Duration;

use study_tracker_core::social::ids::display_text;

/// Production's own paths for the Social operations (22a and 22b) and the image routes. Requests can only use
/// these; a server payload cannot add one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApiPath {
    SyncV2,
    Presence,
    FriendsStatusV2,
    FriendsRequest,
    FriendsRespond,
    Leaderboard,
    PlayerStats,
    SkribblTheme,
    SkribblGallery,
    SkribblSubmit,
    SkribblVote,
    SkribblLeaderboard,
    // Stage 22b
    Feed,
    FeedReact,
    FeedPollVote,
    FeedComment,
    FeedUpdate,
    FeedDelete,
    FeedImageUpload,
    FeedImageDelete,
    ProfileAvatar,
    SquadsCreate,
    SquadsSearch,
    SquadsDetails,
    SquadsJoin,
    SquadsRespond,
    SquadsLeave,
    SquadsChat,
    SquadsChatDelete,
    SquadsPromote,
    SquadsKick,
    SquadsSettings,
    SquadsScoreboard,
    VerifiedStart,
    VerifiedHeartbeat,
    VerifiedFinish,
    VerifiedReconcile,
    AnnouncementsCurrent,
    AnnouncementsUpdateNotice,
    TelemetryHeartbeat,
    AdminUsage,
}

impl ApiPath {
    pub fn path(self) -> &'static str {
        match self {
            Self::SyncV2 => "/sync/v2",
            Self::Presence => "/presence",
            Self::FriendsStatusV2 => "/friends/status/v2",
            Self::FriendsRequest => "/friends/request",
            Self::FriendsRespond => "/friends/respond",
            Self::Leaderboard => "/leaderboard",
            Self::PlayerStats => "/player-stats",
            Self::SkribblTheme => "/skribbl/theme",
            Self::SkribblGallery => "/skribbl/gallery",
            Self::SkribblSubmit => "/skribbl/submit",
            Self::SkribblVote => "/skribbl/vote",
            Self::SkribblLeaderboard => "/skribbl/leaderboard",
            Self::Feed => "/feed",
            Self::FeedReact => "/feed/react",
            Self::FeedPollVote => "/feed/poll/vote",
            Self::FeedComment => "/feed/comment",
            Self::FeedUpdate => "/feed/update",
            Self::FeedDelete => "/feed/delete",
            Self::FeedImageUpload => "/feed/image",
            Self::FeedImageDelete => "/feed/image/delete",
            Self::ProfileAvatar => "/profile/avatar",
            Self::SquadsCreate => "/squads/create",
            Self::SquadsSearch => "/squads/search",
            Self::SquadsDetails => "/squads/details",
            Self::SquadsJoin => "/squads/join",
            Self::SquadsRespond => "/squads/respond",
            Self::SquadsLeave => "/squads/leave",
            Self::SquadsChat => "/squads/chat",
            Self::SquadsChatDelete => "/squads/chat/delete",
            Self::SquadsPromote => "/squads/promote",
            Self::SquadsKick => "/squads/kick",
            Self::SquadsSettings => "/squads/settings",
            Self::SquadsScoreboard => "/squads/scoreboard",
            Self::VerifiedStart => "/verified-session/start",
            Self::VerifiedHeartbeat => "/verified-session/heartbeat",
            Self::VerifiedFinish => "/verified-session/finish",
            Self::VerifiedReconcile => "/verified-session/reconcile-offline",
            Self::AnnouncementsCurrent => "/announcements/current",
            Self::AnnouncementsUpdateNotice => "/announcements/update-notice",
            Self::TelemetryHeartbeat => "/telemetry/heartbeat",
            Self::AdminUsage => "/admin/usage",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
}

/// Where the request goes: an API route, or a server image path already checked against the
/// image allow-list (`images::ImagePath`).
#[derive(Clone, PartialEq, Eq)]
pub enum Target {
    Api(ApiPath),
    /// A path under an allowed image prefix, percent-encoded as the server sent it.
    Image(String),
}

#[derive(Clone, PartialEq, Eq)]
pub enum Body {
    None,
    Json(Vec<u8>),
    Multipart {
        content_type: String,
        bytes: Vec<u8>,
    },
}

impl Body {
    pub fn len(&self) -> usize {
        match self {
            Self::None => 0,
            Self::Json(b) | Self::Multipart { bytes: b, .. } => b.len(),
        }
    }
}

/// Which queue a request waits in (API calls always run before image fetches).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Api,
    Image,
}

/// One HTTP exchange the worker performs. Built only by `social_api` / `images`.
#[derive(Clone)]
pub struct ApiRequest {
    pub method: Method,
    pub target: Target,
    /// Query parameters (`/skribbl/theme`'s `userId`/`deviceSecret`). Never logged.
    pub query: Vec<(&'static str, String)>,
    pub body: Body,
    /// Response bodies above this are refused (`NetError::Malformed`) without being buffered.
    pub max_response: u64,
    /// Total time for this call (connect, send, receive).
    pub timeout: Duration,
    pub priority: Priority,
}

impl ApiRequest {
    /// The path for logs and diagnostics: never the query string.
    pub fn log_path(&self) -> String {
        match &self.target {
            Target::Api(p) => p.path().to_string(),
            Target::Image(p) => {
                // the image key is a server value: keep only the route prefix
                let prefix = p.split('/').take(3).collect::<Vec<_>>().join("/");
                format!("{prefix}/…")
            }
        }
    }

    /// The path plus the percent-encoded query (inside the transport only).
    pub fn path_and_query(&self) -> String {
        let path = match &self.target {
            Target::Api(p) => p.path().to_string(),
            Target::Image(p) => p.clone(),
        };
        if self.query.is_empty() {
            return path;
        }
        let q: Vec<String> = self
            .query
            .iter()
            .map(|(k, v)| format!("{k}={}", percent_encode(v)))
            .collect();
        format!("{path}?{}", q.join("&"))
    }
}

impl fmt::Debug for ApiRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ApiRequest({:?} {} query_params={} body_bytes={})",
            self.method,
            self.log_path(),
            self.query.len(),
            self.body.len()
        )
    }
}

/// `encodeURIComponent` / `URLSearchParams` style: unreserved characters kept, everything else
/// percent-encoded.
pub fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

/// The application-level error. Enough to tell the user what to do (offline / try later /
/// account problem) without a raw body, URL, header or credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetError {
    /// DNS failure, connection refused, network unreachable, TLS handshake impossible.
    Offline,
    Timeout,
    /// 401/403 (e.g. "Invalid device secret." / "User is private.").
    Unauthorized(Option<String>),
    NotFound(Option<String>),
    /// 409 (Skribbl "You already submitted a drawing today.", friends "You are already friends.").
    Conflict(Option<String>),
    /// 429 (new-account throttle, R2 budget).
    RateLimited(Option<String>),
    /// 413 or a local size check.
    PayloadTooLarge(Option<String>),
    /// Other 4xx with the server's message.
    Rejected(Option<String>),
    /// 5xx: never shown verbatim (it may carry internal detail).
    Server,
    /// Unparsable / invalid / oversized response.
    Malformed,
    /// Refused locally by policy (image allow-list, test-build loopback guard, bad request).
    Blocked,
    /// Dropped from the queue (superseded, navigation, shutdown, queue full).
    Cancelled,
}

/// The server's plain-text message, if it is safe and useful to show: a short single line, not
/// markup or JSON. Production shows `response.text()` verbatim; native keeps that for the short
/// user-facing 4xx messages the Worker writes ("No user with that friend code exists.") and
/// drops anything else.
pub fn server_message(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?.trim();
    if text.is_empty()
        || text.len() > 200
        || text.starts_with(['<', '{', '['])
        || text.contains('\n')
    {
        return None;
    }
    let clean = display_text(text, 200);
    (!clean.trim().is_empty()).then_some(clean)
}

impl NetError {
    /// Maps a non-2xx status to an error (body used only for a safe short message).
    pub fn from_status(status: u16, body: &[u8]) -> Self {
        let msg = server_message(body);
        match status {
            401 | 403 => Self::Unauthorized(msg),
            404 => Self::NotFound(msg),
            409 => Self::Conflict(msg),
            413 => Self::PayloadTooLarge(msg),
            429 => Self::RateLimited(msg),
            400..=499 => Self::Rejected(msg),
            _ => Self::Server,
        }
    }

    /// The server's own message when it is one of the 4xx kinds.
    pub fn server_text(&self) -> Option<&str> {
        match self {
            Self::Unauthorized(m)
            | Self::NotFound(m)
            | Self::Conflict(m)
            | Self::RateLimited(m)
            | Self::PayloadTooLarge(m)
            | Self::Rejected(m) => m.as_deref(),
            _ => None,
        }
    }

    /// A user-facing message: the Worker's own short message when it sent one, else a restrained
    /// generic line for the class (`fallback` for the operation's own wording).
    pub fn user_message(&self, fallback: &str) -> String {
        if let Some(m) = self.server_text() {
            return m.to_string();
        }
        match self {
            Self::Offline => "You're offline or the Social server can't be reached. Try again when you're connected.".into(),
            Self::Timeout => "The Social server took too long to answer. Try again in a moment.".into(),
            Self::Unauthorized(_) => "This device's Social account was not accepted by the server.".into(),
            Self::RateLimited(_) => "The Social server is busy. Try again later.".into(),
            Self::PayloadTooLarge(_) => "That is too large to send.".into(),
            Self::Server => "The Social server had a problem. Try again later.".into(),
            _ => fallback.to_string(),
        }
    }

    /// A short, credential-free label for logs.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Timeout => "timeout",
            Self::Unauthorized(_) => "unauthorized",
            Self::NotFound(_) => "not-found",
            Self::Conflict(_) => "conflict",
            Self::RateLimited(_) => "rate-limited",
            Self::PayloadTooLarge(_) => "payload-too-large",
            Self::Rejected(_) => "rejected",
            Self::Server => "server",
            Self::Malformed => "malformed",
            Self::Blocked => "blocked",
            Self::Cancelled => "cancelled",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "TEST_SECRET_MUST_NOT_APPEAR";

    #[test]
    fn requests_never_print_their_query_or_body() {
        let req = ApiRequest {
            method: Method::Get,
            target: Target::Api(ApiPath::SkribblTheme),
            query: vec![("userId", "u".into()), ("deviceSecret", SECRET.into())],
            body: Body::Json(format!("{{\"deviceSecret\":\"{SECRET}\"}}").into_bytes()),
            max_response: 1024,
            timeout: Duration::from_secs(1),
            priority: Priority::Api,
        };
        let shown = format!("{req:?} {:#?} {}", req, req.log_path());
        assert!(!shown.contains(SECRET), "{shown}");
        assert!(shown.contains("/skribbl/theme"));
        // the transport-only form does carry it, percent-encoded
        assert!(req
            .path_and_query()
            .contains("deviceSecret=TEST_SECRET_MUST_NOT_APPEAR"));
    }

    #[test]
    fn image_paths_log_only_their_route() {
        let req = ApiRequest {
            method: Method::Get,
            target: Target::Image("/skribbl/drawing/drawings%2F2026-10-04%2Fuser.png".into()),
            query: vec![],
            body: Body::None,
            max_response: 1,
            timeout: Duration::from_secs(1),
            priority: Priority::Image,
        };
        assert_eq!(req.log_path(), "/skribbl/drawing/…");
    }

    #[test]
    fn percent_encoding_matches_url_search_params_for_ids() {
        assert_eq!(percent_encode("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(percent_encode("9b2c-AF_.~"), "9b2c-AF_.~");
    }

    #[test]
    fn status_mapping_and_safe_server_messages() {
        assert_eq!(
            NetError::from_status(409, b"You already submitted a drawing today."),
            NetError::Conflict(Some("You already submitted a drawing today.".into()))
        );
        assert_eq!(
            NetError::from_status(404, b"No user with that friend code exists.").user_message("x"),
            "No user with that friend code exists."
        );
        assert_eq!(
            NetError::from_status(403, b""),
            NetError::Unauthorized(None)
        );
        assert_eq!(
            NetError::from_status(429, b"Too many").kind(),
            "rate-limited"
        );
        assert_eq!(
            NetError::from_status(413, b"Drawing is too large. Keep it under 1.5 MB."),
            NetError::PayloadTooLarge(Some("Drawing is too large. Keep it under 1.5 MB.".into()))
        );
        assert_eq!(
            NetError::from_status(400, b"Invalid vote value."),
            NetError::Rejected(Some("Invalid vote value.".into()))
        );
        // 5xx text is never shown
        assert_eq!(
            NetError::from_status(500, b"D1_ERROR: no such table: users"),
            NetError::Server
        );
        assert_eq!(
            NetError::from_status(502, b"<html>bad gateway</html>"),
            NetError::Server
        );
        // markup, JSON, multi-line, overlong and control-laden bodies are dropped or cleaned
        assert_eq!(server_message(b"<script>alert(1)</script>"), None);
        assert_eq!(server_message(b"{\"error\":\"x\"}"), None);
        assert_eq!(server_message(b"line1\nline2"), None);
        assert_eq!(server_message(&[b'x'; 201]), None);
        assert_eq!(server_message(b"\xff\xfe"), None, "invalid UTF-8");
        assert_eq!(
            server_message("bad\u{202E}gnirts".as_bytes()),
            Some("badgnirts".into())
        );
        assert_eq!(NetError::Offline.user_message("x"), "You're offline or the Social server can't be reached. Try again when you're connected.");
        assert_eq!(
            NetError::Malformed.user_message("Could not load."),
            "Could not load."
        );
    }
}
