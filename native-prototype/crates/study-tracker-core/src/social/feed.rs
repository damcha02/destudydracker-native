//! The Social feed (Stage 22b): posts, polls, reactions and comments as production's
//! `App.tsx` / `SocialScreen.tsx` / `lib/social.ts` model them, with the Worker
//! (`cloudflare/src/index.ts` `getFeed`, `handleFeedReaction`, `handleFeedPollVote`, ...) as the
//! authority on every row.
//!
//! Pure domain only: the application validates wire DTOs into these types, owns the requests and
//! decides what to show. Every rule here names its production original.

use serde::{Deserialize, Serialize};

use super::avatar::{arena_hue_units, Avatar};
use super::ids::{
    display_paragraph, display_text, truncate_utf16, CommentId, FriendCode, PollOptionId, PostId,
    UserId,
};
use super::time::SocialTimestamp;

/// `SocialFeedScope`, in production's button order (`["friends", "global"]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedScope {
    Friends,
    Global,
}

impl Default for FeedScope {
    /// `useState<SocialFeedScope>("friends")`.
    fn default() -> Self {
        Self::Friends
    }
}

impl FeedScope {
    pub const UI_ORDER: [Self; 2] = [Self::Friends, Self::Global];

    pub fn wire(self) -> &'static str {
        match self {
            Self::Friends => "friends",
            Self::Global => "global",
        }
    }

    /// The scope buttons: Field Notebook "Friends log"/"Global log", otherwise
    /// "Friends Feed"/"Global Feed".
    pub fn label(self, field_notebook: bool) -> &'static str {
        match (self, field_notebook) {
            (Self::Friends, true) => "Friends log",
            (Self::Global, true) => "Global log",
            (Self::Friends, false) => "Friends Feed",
            (Self::Global, false) => "Global Feed",
        }
    }
}

/// Production limits (the Worker's constants and the inputs' `maxLength`), in UTF-16 units.
pub const MAX_COMMENT_UTF16: usize = 220;
pub const MAX_NOTE_UTF16: usize = 220;
pub const MAX_POLL_QUESTION_UTF16: usize = 180;
pub const MAX_POLL_OPTION_UTF16: usize = 100;
/// `MAX_FEED_POLL_OPTIONS` (client) / `MAX_POLL_OPTIONS` (Worker).
pub const MAX_POLL_OPTIONS: usize = 12;
/// `pendingFeedPosts.slice(0, 25)` / `MAX_SYNC_FEED_POSTS`.
pub const MAX_PENDING_POSTS: usize = 25;
/// `cachedFeeds[scope].slice(0, 50)` when a post is queued locally.
pub const MAX_CACHED_POSTS: usize = 50;
/// `getFeed` returns `LIMIT 40` posts.
pub const SERVER_FEED_LIMIT: usize = 40;
/// `FEED_IMAGE_TTL_MS`: an image expires 5 days after upload.
pub const FEED_IMAGE_TTL_MS: i64 = 5 * 24 * 60 * 60 * 1000;

/// Defensive caps on server data (production has none client-side; far above anything real).
pub const MAX_FEED_ROWS: usize = 100;
pub const MAX_COMMENTS_PER_POST: usize = 500;
pub const MAX_REACTION_KINDS: usize = 64;
pub const MAX_REACTOR_NAMES: usize = 200;
pub const MAX_POLL_OPTIONS_RECEIVED: usize = 24;

/// The three reactions every card shows (`reactions: { fire: 0, brain: 0, clap: 0, ... }`).
pub const BASE_REACTIONS: [&str; 3] = ["fire", "brain", "clap"];

/// The "+" picker's 36 emoji, in production's order.
pub const PICKER_EMOJI: [&str; 36] = [
    "🔥", "🧠", "👏", "⭐", "🎯", "💪", "📚", "⚡", "🎉", "🏆", "✨", "💡", "🎓", "🚀", "💎", "🌟",
    "📖", "🕐", "💯", "🙌", "🤯", "😤", "👑", "🌊", "😂", "🤣", "😭", "🥲", "😅", "🥹", "❤️", "🙏",
    "👍", "👎", "😍", "😎",
];

/// `feedFallbackNotes`: the note a post gets when the composer is left blank.
pub const FALLBACK_NOTES: [&str; 28] = [
    "only 5 billion things to go...",
    "keeping up with the deadline",
    "not wasting time",
    "deleted instagram",
    "none of it is real...",
    "hustling",
    "grinding",
    "workaholic",
    "need a breather",
    "one more sesh",
    "fantasizing about my next break",
    "spending too much time in the breakroom",
    "cannot break into the vault",
    "slaying demons",
    "training dragons",
    "living in delusion",
    "code never sleeps",
    "brain.exe running",
    "fueled by caffeine",
    "in the zone",
    "closing tabs, opening minds",
    "debugging my life",
    "on the grindset",
    "minimum viable student",
    "late to the party, early to the library",
    "ctrl+s my sanity",
    "segfault in real life",
    "stack overflow of assignments",
];

/// `pickFeedFallbackNote(seed)`: the sum of every code point's first UTF-16 unit, modulo the list.
pub fn fallback_note(seed: &str) -> &'static str {
    FALLBACK_NOTES[(arena_hue_units(seed) % FALLBACK_NOTES.len() as u64) as usize]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostKind {
    Session,
    Milestone,
}

impl PostKind {
    pub fn wire(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Milestone => "milestone",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollOption {
    pub id: PollOptionId,
    pub text: String,
    pub votes: u64,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Poll {
    pub question: String,
    pub multiple: bool,
    pub options: Vec<PollOption>,
    pub total_votes: u64,
}

impl Poll {
    /// `Math.round(option.votes / poll.totalVotes * 100)`, 0 without votes.
    pub fn percent(&self, option: &PollOption) -> u32 {
        if self.total_votes == 0 {
            return 0;
        }
        crate::dashboard::format::js_round(option.votes as f64 / self.total_votes as f64 * 100.0)
            .clamp(0.0, 100.0) as u32
    }

    /// `voteFeedPoll`'s optimistic update: toggles `option_id`; in a one-answer poll choosing an
    /// option also clears the previously selected one. `totalVotes` is recomputed as the sum.
    /// `None` when the option is not in the poll (nothing to do).
    pub fn optimistic_vote(&self, option_id: &PollOptionId) -> Option<Poll> {
        let was = self.options.iter().find(|o| &o.id == option_id)?.selected;
        let options: Vec<PollOption> = self
            .options
            .iter()
            .map(|o| {
                let mut o = o.clone();
                if &o.id == option_id {
                    o.selected = !was;
                    o.votes = if was {
                        o.votes.saturating_sub(1)
                    } else {
                        o.votes + 1
                    };
                } else if !self.multiple && o.selected && !was {
                    o.selected = false;
                    o.votes = o.votes.saturating_sub(1);
                }
                o
            })
            .collect();
        let total_votes = options.iter().map(|o| o.votes).sum();
        Some(Poll {
            question: self.question.clone(),
            multiple: self.multiple,
            options,
            total_votes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub id: CommentId,
    pub post_id: PostId,
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: Option<FriendCode>,
    pub avatar: Avatar,
    pub body: String,
    pub created_at: Option<SocialTimestamp>,
    pub is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedImage {
    /// The Worker's `${origin}/feed/image/<key>`; only a candidate for the app's image policy.
    pub url: String,
    pub mime_type: Option<String>,
    pub expires_at: Option<SocialTimestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedPost {
    pub id: PostId,
    pub user_id: UserId,
    pub display_name: String,
    pub friend_code: Option<FriendCode>,
    pub avatar: Avatar,
    pub kind: PostKind,
    pub subject: String,
    pub detail: String,
    pub note: String,
    pub icon: String,
    pub minutes: u64,
    pub preset_label: String,
    pub created_at: Option<SocialTimestamp>,
    pub is_self: bool,
    pub image: Option<FeedImage>,
    /// `imageExpiredAt` set: "Image expired".
    pub image_expired: bool,
    pub poll: Option<Poll>,
    /// `reactions`, in the server's key order (`fire`, `brain`, `clap` first).
    pub reactions: Vec<(String, u64)>,
    /// The keys of `reacted` that are true.
    pub reacted: Vec<String>,
    /// `reactedBy`: names per key (the server hides private non-friends).
    pub reacted_by: Vec<(String, Vec<String>)>,
    pub comments: Vec<Comment>,
}

impl FeedPost {
    pub fn count(&self, key: &str) -> u64 {
        self.reactions
            .iter()
            .find(|(k, _)| k == key)
            .map_or(0, |(_, n)| *n)
    }

    pub fn has_reacted(&self, key: &str) -> bool {
        self.reacted.iter().any(|k| k == key)
    }

    pub fn names(&self, key: &str) -> &[String] {
        self.reacted_by
            .iter()
            .find(|(k, _)| k == key)
            .map_or(&[], |(_, n)| n.as_slice())
    }

    /// `toggleLocalFeedReaction`'s local update: the count moves by one (never below 0), the own
    /// name is appended or removed from `reactedBy`, and `reacted` flips.
    pub fn toggle_reaction(&mut self, key: &str, my_name: &str) {
        let was = self.has_reacted(key);
        match self.reactions.iter_mut().find(|(k, _)| k == key) {
            Some((_, n)) => *n = if was { n.saturating_sub(1) } else { *n + 1 },
            None => self
                .reactions
                .push((key.to_string(), if was { 0 } else { 1 })),
        }
        if was {
            self.reacted.retain(|k| k != key);
        } else {
            self.reacted.push(key.to_string());
        }
        let names = match self.reacted_by.iter_mut().find(|(k, _)| k == key) {
            Some((_, names)) => names,
            None => {
                self.reacted_by.push((key.to_string(), Vec::new()));
                &mut self.reacted_by.last_mut().expect("just pushed").1
            }
        };
        if was {
            if let Some(i) = names.iter().position(|n| n == my_name) {
                names.remove(i);
            }
        } else {
            names.push(my_name.to_string());
        }
    }

    /// The reaction buttons, in order: Wabi-Sabi shows only `fire` ("Nod"); otherwise `fire`,
    /// `brain`, `clap`, then every other key with a count above zero, each once.
    pub fn reaction_keys(&self, wabi: bool) -> Vec<String> {
        if wabi {
            return vec!["fire".into()];
        }
        let mut keys: Vec<String> = BASE_REACTIONS.iter().map(|k| k.to_string()).collect();
        for (k, n) in &self.reactions {
            if *n > 0 && !keys.contains(k) {
                keys.push(k.clone());
            }
        }
        keys
    }
}

/// The button text for a key: Wabi "Nod", the three named keys as their emoji, anything else
/// as sent (it is the emoji itself).
pub fn reaction_glyph(key: &str, wabi: bool) -> String {
    if wabi {
        return "Nod".into();
    }
    match key {
        "fire" => "🔥".into(),
        "brain" => "🧠".into(),
        "clap" => "👏".into(),
        other => other.to_string(),
    }
}

/// The hover tooltip: up to three names, then "+N more".
pub fn reactors_label(names: &[String]) -> String {
    if names.len() <= 3 {
        names.join(", ")
    } else {
        format!("{} +{} more", names[..3].join(", "), names.len() - 3)
    }
}

/// What production accepts as a reaction key on the server (`cleanText(emoji, 8)` and at most 4
/// code points; anything else becomes "fire"). The client only ever sends the three named keys
/// or one of [`PICKER_EMOJI`].
pub fn is_known_reaction(key: &str) -> bool {
    BASE_REACTIONS.contains(&key) || PICKER_EMOJI.contains(&key)
}

// ------------------------------------------------------------------------------- composing

/// The poll popover's draft (`FeedPollDraft`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollDraft {
    pub question: String,
    pub multiple: bool,
    pub options: Vec<String>,
}

impl Default for PollDraft {
    /// `emptyFeedPollDraft()`: two empty options.
    fn default() -> Self {
        Self {
            question: String::new(),
            multiple: false,
            options: vec![String::new(), String::new()],
        }
    }
}

impl PollDraft {
    /// `feedPollHasDraft`: a question or any option typed.
    pub fn has_content(&self) -> bool {
        !self.question.trim().is_empty() || self.options.iter().any(|o| !o.trim().is_empty())
    }

    /// `addFeedPollOption`: up to 12.
    pub fn add_option(&mut self) {
        if self.options.len() < MAX_POLL_OPTIONS {
            self.options.push(String::new());
        }
    }

    /// `removeFeedPollOption`: never below two.
    pub fn remove_option(&mut self, index: usize) {
        if self.options.len() > 2 && index < self.options.len() {
            self.options.remove(index);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollDraftError;

impl PollDraftError {
    pub fn message(self) -> &'static str {
        "A poll needs a question and at least two different options."
    }
}

/// A poll as it is sent in `/sync/v2` (`{ question, multiple, options: [{ id, text }] }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewPoll {
    pub question: String,
    pub multiple: bool,
    pub options: Vec<(PollOptionId, String)>,
}

impl NewPoll {
    /// The poll as the own (not yet synced) post shows it: no votes.
    pub fn as_poll(&self) -> Poll {
        Poll {
            question: self.question.clone(),
            multiple: self.multiple,
            options: self
                .options
                .iter()
                .map(|(id, text)| PollOption {
                    id: id.clone(),
                    text: text.clone(),
                    votes: 0,
                    selected: false,
                })
                .collect(),
            total_votes: 0,
        }
    }
}

/// `trim().replace(/\s+/g, " ")`.
fn collapse_ws(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `prepareFeedPollDraft`: question and options trimmed, whitespace collapsed and cut to
/// 180/100 units; empty and case-insensitively duplicate options dropped; at most 12. No question
/// and no option: no poll (`Ok(None)`). A question without two options (or options without a
/// question) is refused. `new_id` supplies each option's id (`makeId()`).
pub fn prepare_poll(
    draft: &PollDraft,
    mut new_id: impl FnMut() -> PollOptionId,
) -> Result<Option<NewPoll>, PollDraftError> {
    let question = truncate_utf16(&collapse_ws(&draft.question), MAX_POLL_QUESTION_UTF16);
    let mut seen: Vec<String> = Vec::new();
    let mut options = Vec::new();
    for raw in &draft.options {
        let text = truncate_utf16(&collapse_ws(raw), MAX_POLL_OPTION_UTF16);
        let key = text.to_lowercase();
        if text.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.push(key);
        options.push((new_id(), text));
        if options.len() == MAX_POLL_OPTIONS {
            break;
        }
    }
    if question.is_empty() && options.is_empty() {
        return Ok(None);
    }
    if question.is_empty() || options.len() < 2 {
        return Err(PollDraftError);
    }
    Ok(Some(NewPoll {
        question,
        multiple: draft.multiple,
        options,
    }))
}

/// A post waiting in `pendingFeedPosts` until the next `/sync/v2` carries it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingPost {
    pub id: PostId,
    pub kind: PostKind,
    pub subject: String,
    pub detail: String,
    pub note: String,
    pub icon: String,
    pub minutes: u64,
    pub preset_label: String,
    /// The session's `endedAt` (ISO 8601).
    pub created_at: SocialTimestamp,
    pub poll: Option<NewPoll>,
}

/// What `buildFeedPostFromSession` needs from a study session.
pub struct SessionFacts<'a> {
    pub id: &'a str,
    pub exam: bool,
    pub goal: &'a str,
    pub minutes: u64,
    pub preset_label: &'a str,
    pub ended_at: SocialTimestamp,
}

/// `buildFeedPostFromSession` (+ the poll `queueFeedPost` attaches). `minutes_label` is
/// `formatMinutes(session.minutes)` (the dashboard formatter, owned by the caller).
pub fn build_session_post(
    session: &SessionFacts<'_>,
    course_name: &str,
    note: &str,
    minutes_label: &str,
    poll: Option<NewPoll>,
) -> Option<PendingPost> {
    let subject = if !course_name.is_empty() {
        course_name.to_string()
    } else if !session.goal.is_empty() {
        session.goal.to_string()
    } else if session.exam {
        "Exam session".to_string()
    } else {
        "Study session".to_string()
    };
    let preset = if !session.preset_label.is_empty() {
        session.preset_label
    } else if session.exam {
        "Exam"
    } else {
        "Focus"
    };
    let note = note.trim();
    Some(PendingPost {
        id: PostId::parse(session.id)?,
        kind: PostKind::Session,
        subject,
        detail: format!("{minutes_label} · {preset}"),
        note: if note.is_empty() {
            fallback_note(session.id).to_string()
        } else {
            note.to_string()
        },
        icon: if session.exam { "⚔" } else { "✦" }.to_string(),
        minutes: session.minutes,
        preset_label: session.preset_label.to_string(),
        created_at: session.ended_at,
        poll,
    })
}

impl PendingPost {
    /// The row `queueFeedPost` puts at the top of both cached feeds.
    pub fn as_feed_post(
        &self,
        user_id: &UserId,
        display_name: &str,
        friend_code: &FriendCode,
        avatar: &Avatar,
    ) -> FeedPost {
        FeedPost {
            id: self.id.clone(),
            user_id: user_id.clone(),
            display_name: display_name.to_string(),
            friend_code: Some(friend_code.clone()),
            avatar: avatar.clone(),
            kind: self.kind,
            subject: self.subject.clone(),
            detail: self.detail.clone(),
            note: self.note.clone(),
            icon: self.icon.clone(),
            minutes: self.minutes,
            preset_label: self.preset_label.clone(),
            created_at: Some(self.created_at),
            is_self: true,
            image: None,
            image_expired: false,
            poll: self.poll.as_ref().map(NewPoll::as_poll),
            reactions: BASE_REACTIONS.iter().map(|k| (k.to_string(), 0)).collect(),
            reacted: Vec::new(),
            reacted_by: Vec::new(),
            comments: Vec::new(),
        }
    }
}

/// `queueFeedPost`'s duplicate rule: a post already queued or already in either cached feed is
/// not queued again (`latestFeedSessionPosted` reads the same lists).
pub fn already_posted(id: &PostId, pending: &[PendingPost], cached: &[&[FeedPost]]) -> bool {
    pending.iter().any(|p| &p.id == id) || cached.iter().any(|f| f.iter().any(|p| &p.id == id))
}

/// A comment as the user typed it: trimmed; empty is refused (`"Write a comment first."`);
/// production's input stops at 220 units, the Worker cuts at 220 too.
pub fn clean_comment(draft: &str) -> Option<String> {
    let body = draft.trim();
    (!body.is_empty()).then(|| truncate_utf16(body, MAX_COMMENT_UTF16))
}

/// A post note as edited (`editingFeedPostNote.trim()`), cut at 220 units like the textarea.
pub fn clean_note(draft: &str) -> String {
    truncate_utf16(draft.trim(), MAX_NOTE_UTF16)
}

/// Display forms of untrusted feed text.
pub fn display_note(raw: &str) -> String {
    display_paragraph(raw, 400)
}

pub fn display_short(raw: &str, max: usize) -> String {
    display_text(raw, max)
}

/// The own-post comment notice (`updateFeedCommentNoticeFromFeeds`): the first comment, by
/// someone else, on one of the user's own posts, that has not been seen before. Every comment
/// in `feed` is marked seen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentNotice {
    pub post_id: PostId,
    pub scope: FeedScope,
    pub commenter_name: String,
    pub body: String,
}

pub fn new_comment_notice(
    feed: &[FeedPost],
    scope: FeedScope,
    self_id: &UserId,
    seen: &mut std::collections::BTreeSet<CommentId>,
) -> Option<CommentNotice> {
    let mut notice = None;
    for post in feed {
        let own = &post.user_id == self_id || post.is_self;
        for c in &post.comments {
            let fresh = !seen.contains(&c.id);
            if fresh && own && &c.user_id != self_id && !c.is_self && notice.is_none() {
                notice = Some(CommentNotice {
                    post_id: post.id.clone(),
                    scope,
                    commenter_name: c.display_name.clone(),
                    body: c.body.clone(),
                });
            }
            seen.insert(c.id.clone());
        }
    }
    notice
}

/// The banner text: the body cut at 72 characters with "...".
pub fn notice_excerpt(body: &str) -> String {
    if super::ids::utf16_len(body) > 72 {
        format!("{}...", truncate_utf16(body, 72))
    } else {
        body.to_string()
    }
}
