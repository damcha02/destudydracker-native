//! The Social application controller (Stage 22a): identity lifecycle, production's sync/presence
//! schedule, Friends, Leaderboards and the Profile subset. Slint-free and network-free like the
//! Skribbl controller: requests leave as tagged [`Outgoing`] values, replies come back through
//! [`SocialController::on_reply`], and the app glue owns timers and the UI.
//!
//! Production behaviour ported (`App.tsx`, `lib/social.ts`) - **only while an identity is
//! established** (decision D2; with no identity nothing below ever produces a request):
//! - startup: `/presence`; `/sync/v2` when `nextAutoSyncAt` is due; and 2 s after load whenever the
//!   session list is non-empty (production's `[socialConfigured, sessions.length]` effect, which
//!   also fires on mount);
//! - every hour: the auto-sync check; every session-count change: a sync 2 s later;
//! - Social tab visible: `/friends/status/v2` at once and every 2 minutes; opening the
//!   Leaderboard/Feed/Profile/Squad subtab: `/presence`, plus a sync (Leaderboard) or a status
//!   refresh (the others); a scope/period change on the Leaderboard: `/leaderboard`;
//! - friend request: local checks, `/friends/request`, then a silent sync;
//! - respond: `/friends/respond`, the reply's snapshot replaces the lists;
//! - name save / privacy / show-hours: saved locally, then a silent sync;
//! - player profile dialog: `/player-stats` (own profile computed locally).
//!
//! Identity (decision D1): `NoIdentity` makes no request; `NewIdentity` exists only after the
//! user confirmed account creation and is persisted only once the bootstrap `/sync/v2` succeeded
//! (rollback otherwise); `ExistingIdentity` comes from the credential store.

use std::collections::HashMap;

use study_tracker_core::academic::AcademicState;
use study_tracker_core::dashboard::civil::LocalClock;
use study_tracker_core::social::friends::{check_friend_request, FriendResponse, FriendsSnapshot};
use study_tracker_core::social::identity::IdentityPhase;
use study_tracker_core::social::leaderboard::{
    ranked_for_scope, LeaderboardEntry, LeaderboardPeriod, LeaderboardScope,
};
use study_tracker_core::social::profile::{clean_display_name, SocialProfile, SyncStatus};
use study_tracker_core::social::stats::sync_stats;
use study_tracker_core::social::time::{next_auto_sync_at, SocialTimestamp};
use study_tracker_core::social::{Avatar, FriendCode, RequestId, SocialIdentity, UserId};
use study_tracker_core::timer::WallTimestamp;

use crate::net::device::{AppMetadata, DeviceIdentity};
use crate::net::endpoint::{EndpointClass, Origin, SocialEndpoint};
use crate::net::http::NetError;
use crate::net::social_api::{self, PlayerStats, SocialSnapshot, SyncInput};
use crate::net::worker::CancelToken;
use crate::net_jobs::{NetReply, Outgoing, Post, Tokens};
use crate::persistence::social_credentials::{usable_for, CredentialStore, StoredCredential};
use crate::persistence::social_port::{CachedBoard, SocialPort, SocialRecord};

/// The production Social subtabs, in production's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subtab {
    Feed,
    Leaderboard,
    Friends,
    Squad,
    Profile,
}

impl Subtab {
    pub const ALL: [Self; 5] = [
        Self::Feed,
        Self::Leaderboard,
        Self::Friends,
        Self::Squad,
        Self::Profile,
    ];

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn label(self) -> &'static str {
        match self {
            Self::Feed => "Feed",
            Self::Leaderboard => "Leaderboard",
            Self::Friends => "Friends",
            Self::Squad => "Squad",
            Self::Profile => "Profile",
        }
    }

    /// Every subtab is implemented (Feed and Squad since Stage 22b).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn available(self) -> bool {
        true
    }
}

/// The friend profile dialog (`viewingFriend`).
#[derive(Debug, Clone, PartialEq)]
pub struct Viewing {
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: Option<FriendCode>,
    pub avatar: Avatar,
    pub loading: bool,
    /// `None` + not loading = "Could not load stats."
    pub stats: Option<PlayerStats>,
}

#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Bootstrap,
    Sync {
        silent: bool,
        /// `sentFeedPostIds`: removed from the outbox once the server accepted them.
        sent: Vec<study_tracker_core::social::PostId>,
    },
    StatusAfterSync,
    Status,
    Presence,
    Leaderboard {
        scope: LeaderboardScope,
        period: LeaderboardPeriod,
    },
    FriendRequest,
    Respond {
        response: FriendResponse,
    },
    PlayerStats {
        target: UserId,
    },
    RestoreProfile,
    // ---- Stage 22b (handled in the child modules) ----
    Feed(feed::FeedPending),
    Squad(squad::SquadPending),
    Profile(avatar::AvatarPending),
    Background(background::BackgroundPending),
}

/// Inputs the controller needs from the rest of the app at sync time.
pub struct SyncContext<'a> {
    pub academic: &'a AcademicState,
    pub clock: &'a dyn LocalClock,
    pub device: &'a DeviceIdentity,
    pub app: &'a AppMetadata,
    /// The Timer's phase and segments, read-only (offline reconciliation's live intervals).
    pub timer_phase: study_tracker_core::timer::TimerPhase,
    pub timer_segments: &'a [study_tracker_core::timer::ActiveSegment],
}

#[path = "social_controller_avatar.rs"]
pub mod avatar;
#[path = "social_controller_background.rs"]
pub mod background;
#[path = "social_controller_feed.rs"]
pub mod feed;
#[path = "social_controller_squad.rs"]
pub mod squad;

/// What happens when a sync finishes (production chains these after `runSocialSync`).
#[derive(Debug, Clone, PartialEq)]
enum AfterSync {
    /// `postLatestSessionToFeed` with an image: upload it once the post is on the server.
    UploadPostImage(feed::PostImageJob),
}

pub struct SocialController {
    endpoint: Option<SocialEndpoint>,
    credentials: Box<dyn CredentialStore>,
    port: Box<dyn SocialPort>,
    pub phase: IdentityPhase,
    pub record: Option<SocialRecord>,
    tokens: Tokens,
    pending: HashMap<u64, (Pending, CancelToken)>,
    // ---- UI state (production's React state) ----
    pub subtab: Subtab,
    pub scope: LeaderboardScope,
    pub period: LeaderboardPeriod,
    pub tab_visible: bool,
    pub syncing: bool,
    sync_in_progress: bool,
    pub friend_code_draft: String,
    pub name_editing: bool,
    pub name_draft: String,
    pub name_prompt_open: bool,
    pub has_unread: bool,
    pub viewing: Option<Viewing>,
    pub confirm_new_account: bool,
    pub creating_account: bool,
    pub message: Option<String>,
    /// bumped whenever `message` is set, so the banner restarts its 3.2 s timer
    pub message_seq: u64,
    pub wabi_competitive: bool,
    pub restoring_profile: bool,
    /// Why the last profile restore failed (shown with "Try again").
    pub restore_error: Option<String>,
    /// Diagnostics.
    pub requests_made: u64,
    /// Refreshes not sent because an identical one was already in flight.
    pub coalesced: u64,
    pub stale_replies: u64,
    // ---- Stage 22b ----
    pub feed: feed::FeedState,
    pub squad: squad::SquadState,
    pub avatar: avatar::AvatarState,
    pub bg: background::BackgroundState,
    after_sync: Vec<AfterSync>,
    /// Network-class failures seen since the last success (drives "connectivity restored").
    offline_failures: u64,
}

impl SocialController {
    /// Loads the credential and the cached profile. No network.
    pub fn new(
        endpoint: Option<SocialEndpoint>,
        credentials: Box<dyn CredentialStore>,
        port: Box<dyn SocialPort>,
        wabi_competitive: bool,
    ) -> Self {
        let class = endpoint.as_ref().map(SocialEndpoint::class);
        let stored = match credentials.load() {
            Ok(c) => c,
            Err(err) => {
                log::warn!("social: {err}; starting without a Social account");
                None
            }
        };
        let identity = class.and_then(|c| usable_for(stored, c));
        let record = identity
            .as_ref()
            .and_then(|id| port.load().filter(|r| r.user_id == id.user_id));
        let restoring = identity.is_some() && record.is_none();
        let prefs = port.load_prefs();
        let phase = match identity {
            Some(identity) => IdentityPhase::ExistingIdentity { identity },
            None => IdentityPhase::NoIdentity,
        };
        log::info!(
            "social: endpoint {}, account {}",
            endpoint
                .as_ref()
                .map_or_else(|| "not configured".to_string(), SocialEndpoint::describe),
            match &phase {
                IdentityPhase::NoIdentity => "none",
                _ if restoring => "present (profile to restore)",
                _ => "present",
            }
        );
        Self {
            endpoint,
            credentials,
            port,
            phase,
            record,
            tokens: Tokens::default(),
            pending: HashMap::new(),
            subtab: Subtab::Feed,
            scope: LeaderboardScope::Friends,
            period: LeaderboardPeriod::Weekly,
            tab_visible: false,
            syncing: false,
            sync_in_progress: false,
            friend_code_draft: String::new(),
            name_editing: false,
            name_draft: String::new(),
            name_prompt_open: false,
            has_unread: false,
            viewing: None,
            confirm_new_account: false,
            creating_account: false,
            message: None,
            message_seq: 0,
            wabi_competitive,
            restoring_profile: restoring,
            restore_error: None,
            requests_made: 0,
            coalesced: 0,
            stale_replies: 0,
            feed: feed::FeedState::default(),
            squad: squad::SquadState::default(),
            avatar: avatar::AvatarState::default(),
            bg: background::BackgroundState::new(prefs),
            after_sync: Vec::new(),
            offline_failures: 0,
        }
        .with_restored_state()
    }

    /// Fills the in-memory caches from the persisted record (queued posts show in the feed as
    /// production's cache shows them; the verified anchor survives restarts).
    fn with_restored_state(mut self) -> Self {
        if let Some(r) = &self.record {
            self.bg.verified = study_tracker_core::social::verified::VerifiedMachine::new(
                r.verified_anchor.clone(),
            );
            self.feed.seen_comments = r.seen_comment_ids.iter().cloned().collect();
            self.feed.seen_initialized = !r.seen_comment_ids.is_empty();
        }
        self.feed.reset_caches_from_pending(self.record.as_ref());
        self
    }

    pub fn configured(&self) -> bool {
        self.endpoint.is_some()
    }

    pub fn origin(&self) -> Option<Origin> {
        self.endpoint.as_ref().map(SocialEndpoint::origin)
    }

    /// The established identity (what Skribbl and every background request use).
    pub fn identity(&self) -> Option<&SocialIdentity> {
        match &self.phase {
            IdentityPhase::ExistingIdentity { identity } => Some(identity),
            _ => None,
        }
    }

    /// An established identity *with* its profile: what the Social screens and sync need.
    pub fn active(&self) -> bool {
        self.identity().is_some() && self.record.is_some()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn port_writes(&self) -> u64 {
        self.port.writes()
    }

    /// Defence in depth: a server message that echoes the credential is never shown or kept.
    pub(crate) fn scrub(&self, text: String) -> String {
        let secret = match &self.phase {
            IdentityPhase::ExistingIdentity { identity } => identity.device_secret.expose(),
            IdentityPhase::NewIdentity { candidate } => candidate.device_secret.expose(),
            IdentityPhase::NoIdentity => return text,
        };
        if !secret.is_empty() && text.contains(secret) {
            text.replace(secret, "[redacted]")
        } else {
            text
        }
    }

    fn say(&mut self, text: impl Into<String>) {
        let text = self.scrub(text.into());
        self.message = Some(text);
        self.message_seq += 1;
    }

    fn persist(&mut self) {
        self.port.persist(self.record.as_ref());
    }

    /// `Post::DecodeImage` etc. are not used by controller requests; uploads are plain HTTP.
    fn send(&mut self, pending: Pending, request: crate::net::http::ApiRequest) -> Outgoing {
        let token = self.tokens.next();
        let cancel = CancelToken::new();
        self.pending.insert(token, (pending, cancel.clone()));
        self.requests_made += 1;
        Outgoing {
            token,
            request,
            cancel,
            post: Post::None,
        }
    }

    fn with_identity(
        &mut self,
        pending: Pending,
        build: impl FnOnce(&SocialIdentity) -> crate::net::http::ApiRequest,
    ) -> Vec<Outgoing> {
        let Some(identity) = self.identity().cloned() else {
            return Vec::new();
        };
        // read-only refreshes are coalesced: an identical one still in flight answers for both
        // (rapid tab/scope switching against a slow server must not flood the request queue)
        if self.same_refresh_in_flight(&pending) {
            self.coalesced += 1;
            return Vec::new();
        }
        let request = build(&identity);
        vec![self.send(pending, request)]
    }

    fn same_refresh_in_flight(&self, pending: &Pending) -> bool {
        let same = |p: &Pending| match (pending, p) {
            (Pending::Presence, Pending::Presence) => true,
            (Pending::Status, Pending::Status | Pending::StatusAfterSync) => true,
            (
                Pending::Leaderboard { scope, period },
                Pending::Leaderboard {
                    scope: s,
                    period: q,
                },
            ) => scope == s && period == q,
            _ => false,
        };
        self.pending.values().any(|(p, _)| same(p))
    }

    // ------------------------------------------------------------------- schedule hooks

    /// App start (after the window is up). Presence and the due auto-sync - or, with no local
    /// profile for the stored identity, the profile restore first.
    pub fn on_startup(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        if self.identity().is_none() {
            return Vec::new();
        }
        if self.restoring_profile {
            return self.restore_profile();
        }
        let app = ctx.app.clone();
        let mut out = self.presence(&app);
        let due = self
            .record
            .as_ref()
            .is_some_and(|r| r.sync.should_auto_sync(now));
        if due {
            out.extend(self.sync(true, now, ctx));
        }
        out
    }

    /// The hourly check (`runAutomaticSocialSync`).
    pub fn on_hourly(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        let due = self
            .record
            .as_ref()
            .is_some_and(|r| r.sync.should_auto_sync(now));
        if due && self.identity().is_some() {
            self.sync(true, now, ctx)
        } else {
            Vec::new()
        }
    }

    /// 2 s after the session count changed (and after load when sessions exist).
    pub fn on_sessions_changed(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        if ctx.academic.sessions.is_empty() {
            return Vec::new();
        }
        self.sync(true, now, ctx)
    }

    /// The 2-minute friend-status poll while the Social tab is visible.
    pub fn on_status_poll(&mut self) -> Vec<Outgoing> {
        if !self.tab_visible || !self.active() {
            return Vec::new();
        }
        self.refresh_status()
    }

    fn presence(&mut self, app: &AppMetadata) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let app = app.clone();
        self.with_identity(Pending::Presence, |id| social_api::presence(id, &app))
    }

    fn refresh_status(&mut self) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        self.with_identity(Pending::Status, social_api::friends_status)
    }

    fn refresh_leaderboard(&mut self) -> Vec<Outgoing> {
        if !self.active() || !self.tab_visible || self.subtab != Subtab::Leaderboard {
            return Vec::new();
        }
        // `refreshSocialLeaderboard` also runs for the squad scope (the members' board the Squad
        // tab shows); `refreshSquadScoreboard` adds the Squad Arena alongside
        let (scope, period) = (self.scope, self.period);
        let mut out = self.with_identity(Pending::Leaderboard { scope, period }, |id| {
            social_api::leaderboard(id, scope, period)
        });
        out.extend(self.refresh_squad_scoreboard(false));
        out
    }

    /// `runSocialSync({ silent })`.
    pub fn sync(&mut self, silent: bool, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        let (Some(identity), Some(record)) = (self.identity().cloned(), self.record.clone()) else {
            return Vec::new();
        };
        if self.sync_in_progress {
            return Vec::new();
        }
        let stats = sync_stats(ctx.academic, ctx.clock);
        let input = SyncInput {
            identity: &identity,
            profile: &record.profile,
            lifetime_minutes: ctx.academic.lifetime_study_minutes,
            lifetime_sessions: ctx.academic.lifetime_study_sessions,
            stats: &stats,
            device: ctx.device,
            app: ctx.app,
            feed_posts: &record.pending_posts,
        };
        let sent = social_api::sent_post_ids(&input);
        match social_api::sync_v2(&input) {
            Ok(request) => {
                self.sync_in_progress = true;
                self.syncing = true;
                vec![self.send(Pending::Sync { silent, sent }, request)]
            }
            Err(err) => {
                let text = err.user_message("Could not sync social data.");
                self.record_sync_failure(&text, now, ctx.clock);
                if !silent {
                    self.say(text);
                }
                Vec::new()
            }
        }
    }

    fn record_sync_failure(&mut self, text: &str, now: WallTimestamp, clock: &dyn LocalClock) {
        let text = self.scrub(text.to_string());
        if let Some(r) = self.record.as_mut() {
            r.sync.last_sync_error = Some(text);
            r.sync.next_auto_sync_at =
                Some(SocialTimestamp::from_wall(next_auto_sync_at(now, clock)));
        }
        self.persist();
    }

    // ------------------------------------------------------------------- UI actions

    /// The Social tab became visible (`setActiveTab("friends")` + the activeTab effect).
    pub fn tab_opened(
        &mut self,
        app: &AppMetadata,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        self.tab_visible = true;
        // a failed restore is retried whenever Social is opened again
        if self.restoring_profile && self.restore_error.is_some() {
            return self.restore_profile();
        }
        self.has_unread = false;
        if let Some(r) = &self.record {
            if is_default_name(&r.profile.display_name) {
                self.name_draft.clear();
                self.name_prompt_open = true;
            }
        }
        let mut out = self.refresh_status();
        out.extend(self.subtab_effect(app, now, ctx));
        out.extend(self.refresh_leaderboard());
        out.extend(self.refresh_feed());
        out.extend(self.maybe_load_suggestions());
        out
    }

    pub fn tab_closed(&mut self) {
        self.tab_visible = false;
    }

    /// The subtab effect: presence plus a sync (Leaderboard) or a status refresh.
    fn subtab_effect(
        &mut self,
        app: &AppMetadata,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        if !self.tab_visible || self.subtab == Subtab::Friends {
            return Vec::new();
        }
        let mut out = self.presence(app);
        if self.subtab == Subtab::Leaderboard {
            out.extend(self.sync(true, now, ctx));
        } else {
            out.extend(self.refresh_status());
        }
        out
    }

    pub fn set_subtab(
        &mut self,
        subtab: Subtab,
        app: &AppMetadata,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        if subtab == self.subtab {
            return Vec::new();
        }
        self.subtab = subtab;
        let mut out = self.subtab_effect(app, now, ctx);
        out.extend(self.refresh_leaderboard());
        out.extend(self.refresh_feed());
        out.extend(self.maybe_load_suggestions());
        out
    }

    pub fn set_scope(&mut self, scope: LeaderboardScope) -> Vec<Outgoing> {
        if scope == self.scope {
            return Vec::new();
        }
        self.scope = scope;
        self.refresh_leaderboard()
    }

    pub fn set_period(&mut self, period: LeaderboardPeriod) -> Vec<Outgoing> {
        if period == self.period {
            return Vec::new();
        }
        self.period = period;
        self.refresh_leaderboard()
    }

    /// "Refresh" / "Sync Arena" (not silent: "Social data synced.").
    pub fn manual_sync(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        if self.restoring_profile {
            // "Try again" on the restore notice
            return self.restore_profile();
        }
        self.sync(false, now, ctx)
    }

    /// `sendFriendRequestToCode(friendCodeDraft)`.
    pub fn send_friend_request(&mut self, code_draft: &str) -> Vec<Outgoing> {
        let Some(record) = &self.record else {
            return Vec::new();
        };
        match check_friend_request(code_draft, &record.profile.friend_code, &record.friends) {
            Err(rejection) => {
                self.say(rejection.message());
                Vec::new()
            }
            Ok(code) => {
                self.syncing = true;
                self.with_identity(Pending::FriendRequest, |id| {
                    social_api::friend_request_create(id, &code)
                })
            }
        }
    }

    pub fn respond(&mut self, request: &RequestId, response: FriendResponse) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        self.syncing = true;
        let request = request.clone();
        self.with_identity(Pending::Respond { response }, |id| {
            social_api::friend_respond(id, &request, response)
        })
    }

    pub fn start_name_edit(&mut self) {
        if let Some(r) = &self.record {
            self.name_draft = r.profile.display_name.clone();
            self.name_editing = true;
        }
    }

    pub fn cancel_name_edit(&mut self) {
        if let Some(r) = &self.record {
            self.name_draft = r.profile.display_name.clone();
        }
        self.name_editing = false;
    }

    /// `saveSocialName` (from the Profile form or the name prompt).
    pub fn save_name(
        &mut self,
        draft: &str,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        match clean_display_name(draft) {
            Err(rejection) => {
                self.say(rejection.message());
                Vec::new()
            }
            Ok(name) => {
                if let Some(r) = self.record.as_mut() {
                    r.profile.display_name = name;
                }
                self.persist();
                self.name_editing = false;
                self.name_prompt_open = false;
                self.say("Player name saved.");
                self.sync(true, now, ctx)
            }
        }
    }

    pub fn toggle_private(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        let Some(r) = self.record.as_mut() else {
            return Vec::new();
        };
        r.profile.is_private = !r.profile.is_private;
        let private = r.profile.is_private;
        self.persist();
        self.say(if private {
            "Profile set to private."
        } else {
            "Profile set to public."
        });
        self.sync(true, now, ctx)
    }

    pub fn toggle_auto_post(&mut self) {
        let Some(r) = self.record.as_mut() else {
            return;
        };
        let was = r.profile.auto_post_sessions;
        r.profile.auto_post_sessions = !was;
        self.persist();
        self.say(if was {
            "Auto-post disabled."
        } else {
            "Auto-post enabled."
        });
    }

    pub fn toggle_show_hours(&mut self, now: WallTimestamp, ctx: &SyncContext) -> Vec<Outgoing> {
        let Some(r) = self.record.as_mut() else {
            return Vec::new();
        };
        r.profile.show_hours_to_friends = !r.profile.show_hours_to_friends;
        let shown = r.profile.show_hours_to_friends;
        self.persist();
        self.say(if shown {
            "Friends can see your study hours."
        } else {
            "Your study hours are hidden from friends."
        });
        self.sync(true, now, ctx)
    }

    /// `openFriendProfile`: own profile from local numbers, others from `/player-stats`.
    pub fn open_profile(
        &mut self,
        user_id: UserId,
        display_name: String,
        friend_code: Option<FriendCode>,
        avatar: Avatar,
    ) -> Vec<Outgoing> {
        let is_self = self.identity().is_some_and(|id| id.user_id == user_id);
        let avatar = if is_self {
            self.record
                .as_ref()
                .map_or(avatar.clone(), |r| r.profile.avatar.clone())
        } else {
            avatar
        };
        self.viewing = Some(Viewing {
            user_id: user_id.clone(),
            display_name,
            friend_code,
            avatar,
            loading: !is_self,
            stats: None,
        });
        if is_self {
            return Vec::new();
        }
        let target = user_id.clone();
        self.with_identity(Pending::PlayerStats { target: user_id }, |id| {
            social_api::player_stats(id, &target)
        })
    }

    pub fn close_profile(&mut self) {
        self.viewing = None;
    }

    pub fn dismiss_message(&mut self) {
        self.message = None;
    }

    pub fn set_wabi_competitive(&mut self, on: bool) {
        self.wabi_competitive = on;
        if !on && self.subtab == Subtab::Leaderboard {
            self.subtab = Subtab::Feed;
        }
    }

    // ------------------------------------------------------------------- identity

    /// "Create a new Social account" -> the confirmation dialog.
    pub fn ask_new_account(&mut self) {
        if matches!(self.phase, IdentityPhase::NoIdentity) && self.configured() {
            self.confirm_new_account = true;
        }
    }

    pub fn cancel_new_account(&mut self) {
        self.confirm_new_account = false;
    }

    /// Confirmed: mint a candidate identity (OS CSPRNG) and bootstrap it with `/sync/v2`.
    /// Nothing is persisted until the server accepted it.
    pub fn create_account(
        &mut self,
        random: [u8; 40],
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        if !matches!(self.phase, IdentityPhase::NoIdentity)
            || !self.configured()
            || self.creating_account
        {
            return Vec::new();
        }
        self.confirm_new_account = false;
        let mut user = [0u8; 16];
        let mut secret = [0u8; 16];
        let mut code = [0u8; 8];
        user.copy_from_slice(&random[0..16]);
        secret.copy_from_slice(&random[16..32]);
        code.copy_from_slice(&random[32..40]);
        let candidate = SocialIdentity::mint(user, secret);
        let profile = SocialProfile::new_default(FriendCode::generate(code));
        let stats = sync_stats(ctx.academic, ctx.clock);
        let input = SyncInput {
            identity: &candidate,
            profile: &profile,
            lifetime_minutes: ctx.academic.lifetime_study_minutes,
            lifetime_sessions: ctx.academic.lifetime_study_sessions,
            stats: &stats,
            device: ctx.device,
            app: ctx.app,
            feed_posts: &[],
        };
        let request = match social_api::sync_v2(&input) {
            Ok(r) => r,
            Err(err) => {
                self.say(err.user_message("Could not create the Social account."));
                return Vec::new();
            }
        };
        self.record = Some(SocialRecord::new(candidate.user_id.clone(), profile));
        self.phase = IdentityPhase::NewIdentity { candidate };
        self.creating_account = true;
        let _ = now;
        vec![self.send(Pending::Bootstrap, request)]
    }

    fn bootstrap_result(
        &mut self,
        result: Result<(), NetError>,
        now: WallTimestamp,
        clock: &dyn LocalClock,
    ) -> Vec<Outgoing> {
        self.creating_account = false;
        let IdentityPhase::NewIdentity { candidate } =
            std::mem::replace(&mut self.phase, IdentityPhase::NoIdentity)
        else {
            return Vec::new();
        };
        match result {
            Ok(()) => {
                let Some(class) = self.endpoint.as_ref().map(SocialEndpoint::class) else {
                    self.record = None;
                    return Vec::new();
                };
                let stored = StoredCredential {
                    identity: candidate.clone(),
                    endpoint: class,
                };
                if let Err(err) = self.credentials.save(&stored) {
                    // the account exists server-side but could not be kept: say so, keep nothing
                    log::warn!("social: {err}");
                    self.record = None;
                    self.say(
                        "The Social account was created but could not be saved on this device.",
                    );
                    return Vec::new();
                }
                if let Some(r) = self.record.as_mut() {
                    r.sync.last_synced_at = Some(SocialTimestamp::from_wall(now));
                    r.sync.next_auto_sync_at =
                        Some(SocialTimestamp::from_wall(next_auto_sync_at(now, clock)));
                }
                self.phase = IdentityPhase::ExistingIdentity {
                    identity: candidate,
                };
                self.persist();
                log::info!("social: a new Social account was created on request");
                self.say("Social account created.");
                // the Social tab is open with production's default "Student XXXX" name: ask now
                // (production asks whenever the tab is opened with such a name)
                if self.tab_visible
                    && self
                        .record
                        .as_ref()
                        .is_some_and(|r| is_default_name(&r.profile.display_name))
                {
                    self.name_draft.clear();
                    self.name_prompt_open = true;
                }
                let mut out = self.refresh_status();
                out.extend(self.refresh_leaderboard());
                out
            }
            Err(err) => {
                // rollback: no credential, no record, back to NoIdentity
                self.record = None;
                self.say(err.user_message("Could not create the Social account."));
                Vec::new()
            }
        }
    }

    /// Existing identity with no local profile (a credential without its profile): read the
    /// server's view of this account before anything else.
    fn restore_profile(&mut self) -> Vec<Outgoing> {
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        if self
            .pending
            .values()
            .any(|(p, _)| *p == Pending::RestoreProfile)
        {
            return Vec::new();
        }
        self.restore_error = None;
        let target = id.user_id.clone();
        self.with_identity(Pending::RestoreProfile, |i| {
            social_api::player_stats(i, &target)
        })
    }

    // ------------------------------------------------------------------- replies

    /// `{ ...current.social, ...result.social }`: every list the reply carried replaces the
    /// local one (the outbox is kept). Persists only when the persisted part really changed.
    fn apply_snapshot(&mut self, snapshot: SocialSnapshot) {
        let Some(r) = self.record.as_mut() else {
            return;
        };
        let mut changed = r.friends != snapshot.friends;
        r.friends = snapshot.friends;
        for (scope, period, entries) in snapshot.leaderboards {
            changed |= set_board(&mut r.leaderboards, scope, period, entries);
        }
        if let Some(sq) = snapshot.squad {
            changed |= r.squad != sq.squad
                || r.squad_incoming != sq.incoming
                || r.squad_outgoing != sq.outgoing;
            r.squad = sq.squad;
            r.squad_incoming = sq.incoming;
            r.squad_outgoing = sq.outgoing;
            self.squad.messages = sq.messages;
        }
        for (scope, rows) in snapshot.feeds {
            changed |= self.feed.store_feed(scope, rows, r);
        }
        for (period, rows) in snapshot.squad_scores {
            self.squad.scores.insert(period, rows);
        }
        if changed {
            self.persist();
        }
    }

    pub fn on_reply(
        &mut self,
        token: u64,
        reply: NetReply,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        let Some((pending, _)) = self.pending.remove(&token) else {
            self.stale_replies += 1;
            return Vec::new();
        };
        let result = reply.http();
        let mut out = self.track_connectivity(&result);
        if matches!(result, Err(NetError::Cancelled)) {
            match pending {
                Pending::Sync { .. } => {
                    self.sync_in_progress = false;
                    self.after_sync.clear();
                }
                Pending::Bootstrap => {
                    return self.bootstrap_result(Err(NetError::Cancelled), now, ctx.clock)
                }
                Pending::Feed(p) => self.feed_cancelled(p),
                Pending::Squad(p) => self.squad_cancelled(p),
                Pending::Profile(p) => self.avatar_cancelled(p),
                Pending::Background(p) => self.background_cancelled(p),
                _ => {}
            }
            self.syncing = self.sync_in_progress;
            return out;
        }
        out.extend(self.on_reply_inner(pending, result, now, ctx));
        out
    }

    /// A network-class failure, then a success: connectivity came back (production's `online`
    /// event, which re-attempts the verified session).
    fn track_connectivity(
        &mut self,
        result: &Result<crate::net::http::HttpResponse, NetError>,
    ) -> Vec<Outgoing> {
        match result {
            Err(NetError::Offline | NetError::Timeout) => {
                self.offline_failures += 1;
                Vec::new()
            }
            Ok(_) if self.offline_failures > 0 => {
                self.offline_failures = 0;
                self.verified_connectivity_restored()
            }
            _ => Vec::new(),
        }
    }

    fn on_reply_inner(
        &mut self,
        pending: Pending,
        result: Result<crate::net::http::HttpResponse, NetError>,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        match pending {
            Pending::Feed(p) => self.feed_reply(p, result, now, ctx),
            Pending::Squad(p) => self.squad_reply(p, result, now, ctx),
            Pending::Profile(p) => self.avatar_reply(p, result, now, ctx),
            Pending::Background(p) => self.background_reply(p, result, now, ctx),
            Pending::Bootstrap => self.bootstrap_result(
                result.and_then(|r| social_api::parse_ok(&r)),
                now,
                ctx.clock,
            ),
            Pending::Sync { silent, sent } => {
                self.sync_in_progress = false;
                self.syncing = false;
                let after = std::mem::take(&mut self.after_sync);
                let mut out = match result.and_then(|r| social_api::parse_ok(&r)) {
                    Ok(()) => {
                        if !silent {
                            self.say("Social data synced.");
                        }
                        if let Some(r) = self.record.as_mut() {
                            r.sync.last_synced_at = Some(SocialTimestamp::from_wall(now));
                            r.sync.last_sync_error = None;
                            r.sync.next_auto_sync_at = Some(SocialTimestamp::from_wall(
                                next_auto_sync_at(now, ctx.clock),
                            ));
                            // `pendingFeedPosts.filter(post => !sentFeedPostIds.includes(post.id))`
                            r.pending_posts.retain(|p| !sent.contains(&p.id));
                        }
                        self.persist();
                        // production sets the dot after every successful sync, also while the
                        // Social tab is open (it clears only when the tab is opened again)
                        self.has_unread = true;
                        let mut out = self
                            .with_identity(Pending::StatusAfterSync, social_api::friends_status);
                        // queued deletions go out after the sync, one request each
                        out.extend(self.send_queued_deletions());
                        out
                    }
                    Err(err) => {
                        let text = err.user_message("Could not sync social data.");
                        self.record_sync_failure(&text, now, ctx.clock);
                        if !silent {
                            self.say(text);
                        }
                        Vec::new()
                    }
                };
                // production continues its chain whatever the sync's outcome
                for AfterSync::UploadPostImage(job) in after {
                    out.extend(self.upload_post_image(job));
                }
                out
            }
            Pending::StatusAfterSync | Pending::Status => {
                if let Ok(snapshot) = result.and_then(|r| social_api::parse_snapshot(&r)) {
                    if matches!(pending, Pending::Status) {
                        let cleared = self
                            .record
                            .as_mut()
                            .is_some_and(|r| r.sync.last_sync_error.take().is_some());
                        if cleared {
                            self.persist();
                        }
                    }
                    self.apply_snapshot(snapshot);
                }
                self.maybe_load_suggestions()
            }
            Pending::Presence => Vec::new(),
            Pending::Leaderboard { scope, period } => {
                if let Ok(entries) = result.and_then(|r| social_api::parse_leaderboard(&r)) {
                    let changed = self
                        .record
                        .as_mut()
                        .is_some_and(|r| set_board(&mut r.leaderboards, scope, period, entries));
                    if changed {
                        self.persist();
                    }
                }
                Vec::new()
            }
            Pending::FriendRequest => {
                self.syncing = false;
                match result.and_then(|r| social_api::parse_snapshot(&r)) {
                    Ok(_) => {
                        self.friend_code_draft.clear();
                        self.say("Friend request sent.");
                        self.sync(true, now, ctx)
                    }
                    Err(err) => {
                        self.say(err.user_message("Could not send friend request."));
                        Vec::new()
                    }
                }
            }
            Pending::Respond { response } => {
                self.syncing = false;
                match result.and_then(|r| social_api::parse_snapshot(&r)) {
                    Ok(snapshot) => {
                        self.apply_snapshot(snapshot);
                        if let Some(r) = self.record.as_mut() {
                            r.sync.last_synced_at = Some(SocialTimestamp::from_wall(now));
                            r.sync.last_sync_error = None;
                            r.sync.next_auto_sync_at = Some(SocialTimestamp::from_wall(
                                next_auto_sync_at(now, ctx.clock),
                            ));
                        }
                        self.persist();
                        self.say(response.done_message());
                    }
                    Err(err) => self.say(err.user_message("Could not update friend request.")),
                }
                Vec::new()
            }
            Pending::PlayerStats { target } => {
                let parsed = result.and_then(|r| social_api::parse_player_stats(&r));
                if let Some(v) = self.viewing.as_mut().filter(|v| v.user_id == target) {
                    v.loading = false;
                    match parsed {
                        Ok(stats) => {
                            v.avatar = stats.avatar.clone();
                            v.stats = Some(stats);
                        }
                        Err(err) => {
                            let text = err.user_message("Could not load profile.");
                            self.say(text);
                        }
                    }
                }
                Vec::new()
            }
            Pending::RestoreProfile => {
                match result.and_then(|r| social_api::parse_player_stats(&r)) {
                    Ok(stats) => {
                        let (Some(id), Some(code)) =
                            (self.identity().cloned(), stats.friend_code.clone())
                        else {
                            return Vec::new();
                        };
                        // Only a dev/test credential reaches this path in 22a: the server does not
                        // return the privacy toggles, so production's defaults apply until changed.
                        let mut profile = SocialProfile::new_default(code);
                        profile.display_name = stats.display_name.clone();
                        profile.avatar = stats.avatar.clone();
                        self.record = Some(SocialRecord::new(id.user_id, profile));
                        self.restoring_profile = false;
                        self.persist();
                        let mut out = self.presence(ctx.app);
                        out.extend(self.sync(true, now, ctx));
                        // the Social tab was opened while restoring: its effects run now
                        if self.tab_visible {
                            if is_default_name(&stats.display_name) {
                                self.name_draft.clear();
                                self.name_prompt_open = true;
                            }
                            out.extend(self.refresh_status());
                            out.extend(self.refresh_leaderboard());
                            out.extend(self.refresh_feed());
                            out.extend(self.maybe_load_suggestions());
                        }
                        out
                    }
                    Err(err) => {
                        log::warn!(
                            "social: the account's profile could not be restored ({})",
                            err.kind()
                        );
                        self.restore_error =
                            Some(err.user_message("Could not load your Social profile."));
                        Vec::new()
                    }
                }
            }
        }
    }

    /// Cancels everything in flight (shutdown).
    pub fn shutdown(&mut self) {
        for (_, (_, cancel)) in self.pending.drain() {
            cancel.cancel();
        }
        self.sync_in_progress = false;
        self.syncing = false;
        self.after_sync.clear();
        // production sends nothing at exit; the Worker settles a silent verified session
        self.bg.verified.reset();
    }

    // ------------------------------------------------------------------- view helpers

    /// `getLeaderboardWithLocalSelf(state, scope, period)`.
    pub fn board(
        &self,
        scope: LeaderboardScope,
        period: LeaderboardPeriod,
    ) -> Vec<LeaderboardEntry> {
        let (Some(r), Some(id)) = (&self.record, self.identity()) else {
            return Vec::new();
        };
        let rows = r
            .leaderboards
            .iter()
            .find(|b| b.scope == scope && b.period == period)
            .map(|b| b.entries.as_slice())
            .unwrap_or(&[]);
        let members = r.squad.as_ref().map(|s| s.member_ids()).unwrap_or_default();
        ranked_for_scope(rows, scope, &id.user_id, &r.friends, &members)
    }

    pub fn friends(&self) -> Option<&FriendsSnapshot> {
        self.record.as_ref().map(|r| &r.friends)
    }

    pub fn profile(&self) -> Option<&SocialProfile> {
        self.record.as_ref().map(|r| &r.profile)
    }

    pub fn sync_status(&self) -> Option<&SyncStatus> {
        self.record.as_ref().map(|r| &r.sync)
    }

    pub fn incoming_count(&self) -> usize {
        self.friends().map_or(0, FriendsSnapshot::incoming_count)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn class(&self) -> Option<EndpointClass> {
        self.endpoint.as_ref().map(SocialEndpoint::class)
    }
}

fn set_board(
    boards: &mut Vec<CachedBoard>,
    scope: LeaderboardScope,
    period: LeaderboardPeriod,
    entries: Vec<LeaderboardEntry>,
) -> bool {
    match boards
        .iter_mut()
        .find(|b| b.scope == scope && b.period == period)
    {
        Some(b) if b.entries == entries => false,
        Some(b) => {
            b.entries = entries;
            true
        }
        None => {
            boards.push(CachedBoard {
                scope,
                period,
                entries,
            });
            true
        }
    }
}

/// `/^Student [A-Z0-9]{0,4}$/` on the trimmed name.
pub fn is_default_name(name: &str) -> bool {
    name.trim().strip_prefix("Student ").is_some_and(|rest| {
        rest.len() <= 4
            && rest
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}

#[cfg(test)]
#[path = "social_controller_tests.rs"]
mod tests;
