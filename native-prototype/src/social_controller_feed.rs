//! The Feed part of the Social controller (Stage 22b): production's `refreshSocialFeedNow`,
//! `postLatestSessionToFeed`, `toggleLocalFeedReaction`, `voteFeedPoll`, `submitFeedComment`,
//! `saveFeedPostEdit`, `deleteOwnFeedPost` and the queued deletions of `runSocialSync`.
//!
//! Mutations are never retried automatically (production has no retry either); a failure is a
//! message. Two races production leaves open are closed: an older poll-vote reply never
//! overwrites a newer vote (sequence per post), and a feed reply only ever lands in the scope it
//! was requested for.

use std::collections::{BTreeSet, HashMap};

use study_tracker_core::social::feed::{
    already_posted, build_session_post, clean_comment, clean_note, new_comment_notice,
    prepare_poll, CommentNotice, FeedImage, FeedPost, FeedScope, PendingPost, Poll, PollDraft,
    SessionFacts, MAX_CACHED_POSTS, MAX_PENDING_POSTS,
};
use study_tracker_core::social::{CommentId, PollOptionId, PostId};
use study_tracker_core::timer::WallTimestamp;

use super::{AfterSync, Pending, SocialController, Subtab, SyncContext};
use crate::net::http::{HttpResponse, NetError};
use crate::net::images::DecodedImage;
use crate::net::social_ext::{self, R2Usage, Upload};
use crate::net_jobs::Outgoing;
use crate::persistence::social_port::{SocialRecord, MAX_OWN_POST_IDS, MAX_SEEN_COMMENT_IDS};

/// Production's owner gate for the R2 panels (`R2_OWNER_FRIEND_CODE`). Display only: the Worker
/// sends `r2Usage` and accepts the owner actions only for the owner's authenticated account.
pub const OWNER_FRIEND_CODE: &str = "ZRWL-WKNF";

/// An image the user picked, already validated, resized and encoded (off the UI thread).
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedImage {
    pub upload: Upload,
    /// A small preview for the composer / editor.
    pub preview: DecodedImage,
}

/// The post being edited (`editingFeedPostId` and its drafts).
#[derive(Debug, Clone, PartialEq)]
pub struct PostEdit {
    pub post_id: PostId,
    pub note: String,
    pub image: Option<PreparedImage>,
    pub remove_image: bool,
}

/// What `uploadImageForFeedPost` is part of.
#[derive(Debug, Clone, PartialEq)]
pub enum UploadPurpose {
    /// `postLatestSessionToFeed`.
    NewPost,
    /// `saveFeedPostEdit` with a replacement image (the note was already saved).
    Edit { note: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostImageJob {
    pub post_id: PostId,
    pub image: PreparedImage,
    pub purpose: UploadPurpose,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EditThen {
    Done,
    Upload(PreparedImage),
    RemoveImage,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FeedPending {
    Refresh {
        scope: FeedScope,
    },
    React {
        post: PostId,
    },
    Vote {
        post: PostId,
        seq: u64,
        previous: Option<Poll>,
    },
    Comment {
        post: PostId,
    },
    Update {
        post: PostId,
        note: String,
        then: EditThen,
    },
    ImageUpload(PostImageJob),
    ImageDelete {
        post: PostId,
        note: String,
    },
    Delete {
        post: PostId,
    },
    QueuedDeletion {
        post: PostId,
    },
}

/// The latest study/exam session as the composer needs it (`latestFeedSession`).
#[derive(Debug, Clone, PartialEq)]
pub struct LatestSession {
    pub id: String,
    pub exam: bool,
    pub goal: String,
    pub minutes: u64,
    pub preset_label: String,
    pub ended_at: WallTimestamp,
    pub course_name: String,
    /// `formatMinutes(session.minutes)`.
    pub minutes_label: String,
}

#[derive(Debug, Default)]
pub struct FeedState {
    pub scope: FeedScope,
    friends: Vec<FeedPost>,
    global: Vec<FeedPost>,
    pub loading: bool,
    pub r2: Option<R2Usage>,
    pub note_draft: String,
    pub poll_draft: PollDraft,
    pub poll_open: bool,
    pub image_draft: Option<PreparedImage>,
    /// A picked image is being prepared off the UI thread.
    pub preparing_image: bool,
    pub comment_drafts: HashMap<PostId, String>,
    pub expanded_comments: BTreeSet<PostId>,
    pub comment_saving: Option<PostId>,
    pub emoji_picker: Option<PostId>,
    pub editing: Option<PostEdit>,
    pub saving: bool,
    /// Posts whose image could not be shown ("Image could not load").
    pub failed_images: BTreeSet<PostId>,
    pub expanded_image: Option<PostId>,
    pub notice: Option<CommentNotice>,
    pub(super) seen_comments: BTreeSet<CommentId>,
    pub(super) seen_initialized: bool,
    vote_seq: HashMap<PostId, u64>,
    next_seq: u64,
    /// Diagnostics: replies dropped because a newer one superseded them.
    pub stale_votes: u64,
}

impl FeedState {
    pub fn rows(&self, scope: FeedScope) -> &[FeedPost] {
        match scope {
            FeedScope::Friends => &self.friends,
            FeedScope::Global => &self.global,
        }
    }

    fn rows_mut(&mut self, scope: FeedScope) -> &mut Vec<FeedPost> {
        match scope {
            FeedScope::Friends => &mut self.friends,
            FeedScope::Global => &mut self.global,
        }
    }

    /// Applies `f` to the post in both cached feeds (production maps `cachedFeeds.global` and
    /// `cachedFeeds.friends` alike).
    fn update_post(&mut self, id: &PostId, mut f: impl FnMut(&mut FeedPost)) {
        for rows in [&mut self.friends, &mut self.global] {
            for p in rows.iter_mut().filter(|p| &p.id == id) {
                f(p);
            }
        }
    }

    fn remove_post(&mut self, id: &PostId) {
        self.friends.retain(|p| &p.id != id);
        self.global.retain(|p| &p.id != id);
    }

    pub fn find(&self, id: &PostId) -> Option<&FeedPost> {
        self.rows(self.scope)
            .iter()
            .chain(self.friends.iter())
            .chain(self.global.iter())
            .find(|p| &p.id == id)
    }

    /// Start-up: the queued posts are what the cache shows until the first feed arrives.
    pub(super) fn reset_caches_from_pending(&mut self, record: Option<&SocialRecord>) {
        self.friends.clear();
        self.global.clear();
        let Some(r) = record else { return };
        let rows: Vec<FeedPost> = r
            .pending_posts
            .iter()
            .map(|p| {
                p.as_feed_post(
                    &r.user_id,
                    &r.profile.display_name,
                    &r.profile.friend_code,
                    &r.profile.avatar,
                )
            })
            .collect();
        self.friends = rows.clone();
        self.global = rows;
    }

    /// `cachedFeeds[scope] = feed`; the user's own post ids are remembered (persisted) so a
    /// restarted app knows which sessions are already published. Returns whether the persisted
    /// part changed.
    pub(super) fn store_feed(
        &mut self,
        scope: FeedScope,
        rows: Vec<FeedPost>,
        record: &mut SocialRecord,
    ) -> bool {
        let mut changed = false;
        for p in rows
            .iter()
            .filter(|p| p.is_self || p.user_id == record.user_id)
        {
            if !record.own_post_ids.contains(&p.id) {
                record.own_post_ids.push(p.id.clone());
                changed = true;
            }
        }
        if record.own_post_ids.len() > MAX_OWN_POST_IDS {
            let extra = record.own_post_ids.len() - MAX_OWN_POST_IDS;
            record.own_post_ids.drain(..extra);
        }
        *self.rows_mut(scope) = rows;
        changed
    }

    fn next_vote(&mut self, post: &PostId) -> u64 {
        self.next_seq += 1;
        self.vote_seq.insert(post.clone(), self.next_seq);
        self.next_seq
    }
}

impl SocialController {
    pub fn is_owner(&self) -> bool {
        self.profile()
            .is_some_and(|p| p.friend_code.as_str() == OWNER_FRIEND_CODE)
    }

    fn pending_post(&self, id: &PostId) -> bool {
        self.record
            .as_ref()
            .is_some_and(|r| r.pending_posts.iter().any(|p| &p.id == id))
    }

    /// `latestFeedSessionPosted`.
    pub fn session_posted(&self, session_id: &str) -> bool {
        let Some(id) = PostId::parse(session_id) else {
            return false;
        };
        let Some(r) = &self.record else { return false };
        already_posted(
            &id,
            &r.pending_posts,
            &[&self.feed.friends, &self.feed.global],
        ) || r.own_post_ids.contains(&id)
    }

    fn send_feed(
        &mut self,
        pending: FeedPending,
        request: crate::net::http::ApiRequest,
    ) -> Outgoing {
        self.send(Pending::Feed(pending), request)
    }

    fn feed_in_flight(&self, scope: FeedScope) -> bool {
        self.pending.values().any(
            |(p, _)| matches!(p, Pending::Feed(FeedPending::Refresh { scope: s }) if *s == scope),
        )
    }

    /// `refreshSocialFeedNow`: only while Social shows the Feed subtab. An identical refresh
    /// still in flight answers for both.
    pub fn refresh_feed(&mut self) -> Vec<Outgoing> {
        if !self.active() || !self.tab_visible || self.subtab != Subtab::Feed {
            return Vec::new();
        }
        let scope = self.feed.scope;
        if self.feed_in_flight(scope) {
            self.coalesced += 1;
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.feed.loading = true;
        vec![self.send_feed(FeedPending::Refresh { scope }, social_ext::feed(&id, scope))]
    }

    /// The 2-minute feed refresh while the Feed subtab is visible.
    pub fn on_feed_poll(&mut self) -> Vec<Outgoing> {
        self.refresh_feed()
    }

    pub fn set_feed_scope(&mut self, scope: FeedScope) -> Vec<Outgoing> {
        if scope == self.feed.scope {
            return Vec::new();
        }
        self.feed.scope = scope;
        self.refresh_feed()
    }

    // ------------------------------------------------------------------------- composing

    /// A picked image finished preparing (or failed: the message production shows).
    pub fn image_draft_ready(&mut self, result: Result<PreparedImage, String>, for_edit: bool) {
        self.feed.preparing_image = false;
        match result {
            Ok(image) => {
                if for_edit {
                    if let Some(e) = self.feed.editing.as_mut() {
                        e.image = Some(image);
                        e.remove_image = false;
                    }
                } else {
                    self.feed.image_draft = Some(image);
                }
            }
            Err(message) => self.say(message),
        }
    }

    /// `canViewR2Usage && r2UsageStatus?.paused`: owner uploads are paused (the file input is
    /// disabled and a message explains why).
    pub fn uploads_paused(&self) -> bool {
        self.is_owner() && self.feed.r2.as_ref().is_some_and(|u| u.paused)
    }

    /// `postLatestSessionToFeed`.
    pub fn post_latest(
        &mut self,
        latest: Option<&LatestSession>,
        mut new_option_id: impl FnMut() -> PollOptionId,
        now: WallTimestamp,
        ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let Some(latest) = latest else {
            self.say("Finish a study session before posting to the feed.");
            return Vec::new();
        };
        if self.session_posted(&latest.id) {
            self.say("That session is already queued for the feed.");
            return Vec::new();
        }
        let poll = match prepare_poll(&self.feed.poll_draft, &mut new_option_id) {
            Ok(p) => p,
            Err(e) => {
                self.say(e.message());
                return Vec::new();
            }
        };
        let facts = SessionFacts {
            id: &latest.id,
            exam: latest.exam,
            goal: &latest.goal,
            minutes: latest.minutes,
            preset_label: &latest.preset_label,
            ended_at: study_tracker_core::social::SocialTimestamp::from_wall(latest.ended_at),
        };
        let Some(post) = build_session_post(
            &facts,
            &latest.course_name,
            &self.feed.note_draft,
            &latest.minutes_label,
            poll,
        ) else {
            return Vec::new();
        };
        let post_id = post.id.clone();
        self.queue_post(post);
        let image = self.feed.image_draft.take();
        self.feed.note_draft.clear();
        self.feed.poll_draft = PollDraft::default();
        self.feed.poll_open = false;
        let Some(image) = image else {
            self.say("Post queued. Sync to publish it to the feed.");
            return Vec::new();
        };
        self.say("Publishing post image...");
        let job = PostImageJob {
            post_id,
            image,
            purpose: UploadPurpose::NewPost,
        };
        let out = self.sync(true, now, ctx);
        if out.is_empty() {
            // `runSocialSync` returned at once (already running / refused): production goes on
            // to the upload straight away
            return self.upload_post_image(job);
        }
        self.after_sync.push(AfterSync::UploadPostImage(job));
        out
    }

    /// `queueFeedPost`: into the outbox (newest first, 25) and at the top of both cached feeds.
    fn queue_post(&mut self, post: PendingPost) {
        let Some(r) = self.record.as_mut() else {
            return;
        };
        if r.pending_posts.iter().any(|p| p.id == post.id) {
            return;
        }
        let row = post.as_feed_post(
            &r.user_id,
            &r.profile.display_name,
            &r.profile.friend_code,
            &r.profile.avatar,
        );
        if !r.own_post_ids.contains(&post.id) {
            r.own_post_ids.push(post.id.clone());
        }
        r.pending_posts.insert(0, post);
        r.pending_posts.truncate(MAX_PENDING_POSTS);
        for rows in [&mut self.feed.friends, &mut self.feed.global] {
            rows.insert(0, row.clone());
            rows.truncate(MAX_CACHED_POSTS);
        }
        self.persist();
    }

    /// Auto-post (`prependSessionsToState` with `autoPostSessions`): the Timer logged a session.
    pub fn auto_post(&mut self, session: &LatestSession) {
        let auto = self.profile().is_some_and(|p| p.auto_post_sessions);
        if !auto || !self.active() || self.session_posted(&session.id) {
            return;
        }
        let facts = SessionFacts {
            id: &session.id,
            exam: session.exam,
            goal: &session.goal,
            minutes: session.minutes,
            preset_label: &session.preset_label,
            ended_at: study_tracker_core::social::SocialTimestamp::from_wall(session.ended_at),
        };
        if let Some(post) = build_session_post(
            &facts,
            &session.course_name,
            "",
            &session.minutes_label,
            None,
        ) {
            self.queue_post(post);
        }
    }

    /// A session deleted from the history (production's `removeSession`): its published post is
    /// queued for deletion at the next sync. (Native has no session-deletion UI until Stage 23;
    /// the mechanism is complete for it.)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn session_removed(&mut self, session_id: &str) {
        let Some(id) = PostId::parse(session_id) else {
            return;
        };
        let published = self
            .feed
            .friends
            .iter()
            .chain(self.feed.global.iter())
            .any(|p| p.id == id);
        self.feed.remove_post(&id);
        let Some(r) = self.record.as_mut() else {
            return;
        };
        r.pending_posts.retain(|p| p.id != id);
        if published && !r.pending_post_deletions.contains(&id) {
            r.pending_post_deletions.push(id);
        }
        self.persist();
    }

    pub(super) fn send_queued_deletions(&mut self) -> Vec<Outgoing> {
        let (Some(id), Some(r)) = (self.identity().cloned(), self.record.as_ref()) else {
            return Vec::new();
        };
        let queued: Vec<PostId> = r
            .pending_post_deletions
            .iter()
            .filter(|p| {
                !self.pending.values().any(
                    |(x, _)| matches!(x, Pending::Feed(FeedPending::QueuedDeletion { post }) if post == *p),
                )
            })
            .cloned()
            .collect();
        queued
            .into_iter()
            .map(|post| {
                let req = social_ext::post_delete(&id, &post);
                self.send_feed(FeedPending::QueuedDeletion { post }, req)
            })
            .collect()
    }

    pub(super) fn upload_post_image(&mut self, job: PostImageJob) -> Vec<Outgoing> {
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        match social_ext::post_image_upload(&id, &job.post_id, &job.image.upload) {
            Ok(req) => vec![self.send_feed(FeedPending::ImageUpload(job), req)],
            Err(err) => {
                self.feed.saving = false;
                self.say(err.user_message("Post queued, but image upload failed."));
                Vec::new()
            }
        }
    }

    // ------------------------------------------------------------------------ interactions

    /// `toggleLocalFeedReaction`: the local toggle first; a queued post stays local.
    pub fn toggle_reaction(&mut self, post: &PostId, key: &str) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let my_name = self
            .profile()
            .map(|p| p.display_name.clone())
            .unwrap_or_default();
        self.feed
            .update_post(post, |p| p.toggle_reaction(key, &my_name));
        if self.pending_post(post) {
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        let req = social_ext::react(&id, post, key);
        vec![self.send_feed(FeedPending::React { post: post.clone() }, req)]
    }

    pub fn toggle_emoji_picker(&mut self, post: &PostId) {
        self.feed.emoji_picker = if self.feed.emoji_picker.as_ref() == Some(post) {
            None
        } else {
            Some(post.clone())
        };
    }

    /// `voteFeedPoll`.
    pub fn vote(&mut self, post: &PostId, option: &PollOptionId) -> Vec<Outgoing> {
        if !self.active() {
            return Vec::new();
        }
        let Some(previous) = self.feed.find(post).and_then(|p| p.poll.clone()) else {
            return Vec::new();
        };
        if self.pending_post(post) {
            self.say("Sync this post before voting on its poll.");
            return Vec::new();
        }
        let Some(optimistic) = previous.optimistic_vote(option) else {
            return Vec::new();
        };
        self.feed
            .update_post(post, |p| p.poll = Some(optimistic.clone()));
        let seq = self.feed.next_vote(post);
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        let req = social_ext::poll_vote(&id, post, option);
        vec![self.send_feed(
            FeedPending::Vote {
                post: post.clone(),
                seq,
                previous: Some(previous),
            },
            req,
        )]
    }

    pub fn toggle_comments(&mut self, post: &PostId) {
        if !self.feed.expanded_comments.remove(post) {
            self.feed.expanded_comments.insert(post.clone());
        }
    }

    /// `submitFeedComment`.
    pub fn submit_comment(&mut self, post: &PostId) -> Vec<Outgoing> {
        if !self.active() || self.feed.comment_saving.as_ref() == Some(post) {
            return Vec::new();
        }
        let draft = self
            .feed
            .comment_drafts
            .get(post)
            .cloned()
            .unwrap_or_default();
        let Some(body) = clean_comment(&draft) else {
            self.say("Write a comment first.");
            return Vec::new();
        };
        if self.pending_post(post) {
            self.say("Sync this post before adding comments.");
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.feed.comment_saving = Some(post.clone());
        let req = social_ext::comment_create(&id, post, &body);
        vec![self.send_feed(FeedPending::Comment { post: post.clone() }, req)]
    }

    /// `startEditingFeedPost`.
    pub fn start_edit(&mut self, post: &PostId) {
        let note = self
            .feed
            .find(post)
            .map(|p| p.note.clone())
            .unwrap_or_default();
        self.feed.editing = Some(PostEdit {
            post_id: post.clone(),
            note,
            image: None,
            remove_image: false,
        });
    }

    pub fn cancel_edit(&mut self) {
        self.feed.editing = None;
    }

    /// `saveFeedPostEdit`: the note, then the replacement image or the removal; a queued post is
    /// only edited locally.
    pub fn save_edit(&mut self) -> Vec<Outgoing> {
        let Some(edit) = self.feed.editing.clone() else {
            return Vec::new();
        };
        if self.feed.saving || !self.active() {
            return Vec::new();
        }
        let note = clean_note(&edit.note);
        if self.pending_post(&edit.post_id) {
            self.finish_edit(&edit.post_id, &note, edit.remove_image);
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.feed.saving = true;
        let then = match (edit.image, edit.remove_image) {
            (Some(img), _) => EditThen::Upload(img),
            (None, true) => EditThen::RemoveImage,
            _ => EditThen::Done,
        };
        let req = social_ext::post_update(&id, &edit.post_id, &note);
        vec![self.send_feed(
            FeedPending::Update {
                post: edit.post_id,
                note,
                then,
            },
            req,
        )]
    }

    fn finish_edit(&mut self, post: &PostId, note: &str, remove_image: bool) {
        let note = note.to_string();
        self.feed.update_post(post, |p| {
            p.note = note.clone();
            if remove_image {
                p.image = None;
                p.image_expired = false;
            }
        });
        if let Some(r) = self.record.as_mut() {
            let mut changed = false;
            for p in r.pending_posts.iter_mut().filter(|p| &p.id == post) {
                p.note = note.clone();
                changed = true;
            }
            if changed {
                self.persist();
            }
        }
        self.feed.saving = false;
        self.feed.editing = None;
        self.say("Post updated.");
    }

    /// `deleteOwnFeedPost`.
    pub fn delete_post(&mut self, post: &PostId) -> Vec<Outgoing> {
        if self.feed.saving || !self.active() {
            return Vec::new();
        }
        if self.pending_post(post) {
            self.post_deleted(post);
            return Vec::new();
        }
        let Some(id) = self.identity().cloned() else {
            return Vec::new();
        };
        self.feed.saving = true;
        let req = social_ext::post_delete(&id, post);
        vec![self.send_feed(FeedPending::Delete { post: post.clone() }, req)]
    }

    fn post_deleted(&mut self, post: &PostId) {
        self.feed.remove_post(post);
        if let Some(r) = self.record.as_mut() {
            r.pending_posts.retain(|p| &p.id != post);
            r.own_post_ids.retain(|p| p != post);
        }
        self.persist();
        self.feed.saving = false;
        self.feed.editing = None;
        self.say("Post deleted. You can repost that session now.");
    }

    /// The comment notice's "View": the Feed, its scope, the post's comments open.
    pub fn open_notice(&mut self) -> Option<FeedScope> {
        let n = self.feed.notice.take()?;
        self.subtab = Subtab::Feed;
        self.feed.scope = n.scope;
        self.feed.expanded_comments.insert(n.post_id);
        Some(n.scope)
    }

    pub fn image_failed(&mut self, post: &PostId) {
        self.feed.failed_images.insert(post.clone());
    }

    pub fn image_loaded(&mut self, post: &PostId) {
        self.feed.failed_images.remove(post);
    }

    // ------------------------------------------------------------------------------ replies

    pub(super) fn feed_cancelled(&mut self, p: FeedPending) {
        match p {
            FeedPending::Refresh { .. } => self.feed.loading = self.any_feed_refresh(),
            FeedPending::Comment { .. } => self.feed.comment_saving = None,
            FeedPending::Update { .. }
            | FeedPending::ImageUpload(_)
            | FeedPending::ImageDelete { .. }
            | FeedPending::Delete { .. } => self.feed.saving = false,
            _ => {}
        }
    }

    fn any_feed_refresh(&self) -> bool {
        self.pending
            .values()
            .any(|(p, _)| matches!(p, Pending::Feed(FeedPending::Refresh { .. })))
    }

    fn apply_image(&mut self, post: &PostId, image: Option<FeedImage>) {
        self.feed.update_post(post, |p| {
            p.image = image.clone();
            p.image_expired = false;
        });
        self.feed.failed_images.remove(post);
    }

    fn set_r2(&mut self, usage: Option<R2Usage>) {
        if self.is_owner() {
            if usage.is_some() {
                self.feed.r2 = usage;
            }
        } else {
            self.feed.r2 = None;
        }
    }

    pub(super) fn feed_reply(
        &mut self,
        p: FeedPending,
        result: Result<HttpResponse, NetError>,
        _now: WallTimestamp,
        _ctx: &SyncContext,
    ) -> Vec<Outgoing> {
        match p {
            FeedPending::Refresh { scope } => {
                self.feed.loading = self.any_feed_refresh();
                match result.and_then(|r| social_ext::parse_feed(&r)) {
                    Ok((rows, usage)) => {
                        let mut persist = false;
                        if let Some(r) = self.record.as_ref() {
                            let self_id = r.user_id.clone();
                            let notice = new_comment_notice(
                                &rows,
                                scope,
                                &self_id,
                                &mut self.feed.seen_comments,
                            );
                            if notice.is_some() {
                                self.feed.notice = notice;
                            }
                        }
                        if self.is_owner() {
                            if usage.is_some() {
                                self.feed.r2 = usage;
                            }
                        } else {
                            self.feed.r2 = None;
                        }
                        let live: BTreeSet<PostId> = rows
                            .iter()
                            .filter(|p| p.image.is_some())
                            .map(|p| p.id.clone())
                            .collect();
                        self.feed.failed_images.retain(|id| live.contains(id));
                        if let Some(mut r) = self.record.take() {
                            persist |= self.feed.store_feed(scope, rows, &mut r);
                            let seen: Vec<CommentId> =
                                self.feed.seen_comments.iter().cloned().collect();
                            if seen != r.seen_comment_ids {
                                let skip = seen.len().saturating_sub(MAX_SEEN_COMMENT_IDS);
                                r.seen_comment_ids = seen[skip..].to_vec();
                                persist = true;
                            }
                            persist |= r.sync.last_sync_error.take().is_some();
                            self.record = Some(r);
                        }
                        if persist {
                            self.persist();
                        }
                    }
                    Err(err) => {
                        let text = self.scrub(err.user_message("Could not refresh the feed."));
                        let changed = self.record.as_mut().is_some_and(|r| {
                            let changed = r.sync.last_sync_error.as_deref() != Some(text.as_str());
                            r.sync.last_sync_error = Some(text);
                            changed
                        });
                        if changed {
                            self.persist();
                        }
                    }
                }
                Vec::new()
            }
            FeedPending::React { .. } => {
                if let Err(err) = result.and_then(|r| crate::net::social_api::parse_ok(&r)) {
                    // production keeps its optimistic toggle and says so
                    self.say(err.user_message("Could not sync reaction."));
                }
                Vec::new()
            }
            FeedPending::Vote {
                post,
                seq,
                previous,
            } => {
                if self.feed.vote_seq.get(&post) != Some(&seq) {
                    // a newer vote on this poll is on its way: its reply decides
                    self.feed.stale_votes += 1;
                    return Vec::new();
                }
                self.feed.vote_seq.remove(&post);
                match result.and_then(|r| social_ext::parse_poll_vote(&r)) {
                    Ok(poll) => self.feed.update_post(&post, |p| p.poll = poll.clone()),
                    Err(err) => {
                        self.feed.update_post(&post, |p| p.poll = previous.clone());
                        self.say(err.user_message("Could not sync poll vote."));
                    }
                }
                Vec::new()
            }
            FeedPending::Comment { post } => {
                self.feed.comment_saving = None;
                match result.and_then(|r| social_ext::parse_comment(&r, &post)) {
                    Ok(comment) => {
                        self.feed.update_post(&post, |p| {
                            if !p.comments.iter().any(|c| c.id == comment.id) {
                                p.comments.push(comment.clone());
                            }
                        });
                        // the own comment is not a "new comment" for the notice
                        self.feed.seen_comments.insert(comment.id.clone());
                        self.feed.comment_drafts.remove(&post);
                        self.feed.expanded_comments.insert(post);
                    }
                    Err(err) => self.say(err.user_message("Could not post comment.")),
                }
                Vec::new()
            }
            FeedPending::Update { post, note, then } => {
                match result.and_then(|r| crate::net::social_api::parse_ok(&r)) {
                    Ok(()) => match then {
                        EditThen::Done => {
                            self.finish_edit(&post, &note, false);
                            Vec::new()
                        }
                        EditThen::Upload(image) => self.upload_post_image(PostImageJob {
                            post_id: post,
                            image,
                            purpose: UploadPurpose::Edit { note },
                        }),
                        EditThen::RemoveImage => {
                            let Some(id) = self.identity().cloned() else {
                                return Vec::new();
                            };
                            let req = social_ext::post_image_delete(&id, &post);
                            vec![self.send_feed(FeedPending::ImageDelete { post, note }, req)]
                        }
                    },
                    Err(err) => {
                        self.feed.saving = false;
                        self.say(err.user_message("Could not update feed post."));
                        Vec::new()
                    }
                }
            }
            FeedPending::ImageUpload(job) => {
                match result.and_then(|r| social_ext::parse_post_image(&r)) {
                    Ok((image, usage)) => {
                        self.set_r2(usage);
                        self.apply_image(&job.post_id, Some(image));
                        match job.purpose {
                            UploadPurpose::NewPost => self.say("Post published with image."),
                            UploadPurpose::Edit { note } => {
                                self.finish_edit(&job.post_id, &note, false)
                            }
                        }
                    }
                    Err(err) => {
                        self.feed.saving = false;
                        let fallback = match job.purpose {
                            UploadPurpose::NewPost => "Post queued, but image upload failed.",
                            UploadPurpose::Edit { .. } => "Could not update feed post.",
                        };
                        self.say(err.user_message(fallback));
                    }
                }
                Vec::new()
            }
            FeedPending::ImageDelete { post, note } => {
                match result.and_then(|r| social_ext::parse_ok_usage(&r)) {
                    Ok(usage) => {
                        self.set_r2(usage);
                        self.finish_edit(&post, &note, true);
                    }
                    Err(err) => {
                        self.feed.saving = false;
                        self.say(err.user_message("Could not update feed post."));
                    }
                }
                Vec::new()
            }
            FeedPending::Delete { post } => {
                match result.and_then(|r| crate::net::social_api::parse_ok(&r)) {
                    Ok(()) => self.post_deleted(&post),
                    Err(err) => {
                        self.feed.saving = false;
                        self.say(err.user_message("Could not delete feed post."));
                    }
                }
                Vec::new()
            }
            FeedPending::QueuedDeletion { post } => {
                // failures stay queued for the next sync (production's loop does the same)
                if result
                    .and_then(|r| crate::net::social_api::parse_ok(&r))
                    .is_ok()
                {
                    if let Some(r) = self.record.as_mut() {
                        r.pending_post_deletions.retain(|p| p != &post);
                    }
                    self.persist();
                }
                Vec::new()
            }
        }
    }
}
