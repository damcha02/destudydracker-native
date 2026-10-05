//! Stage 22b view building: the Feed, Squads, the Squad Arena and the 22b dialogs, as the
//! strings production renders (`SocialScreen.tsx`, `App.tsx`). Nothing here decides anything;
//! it formats the controller's state for `SocialData`.
//!
//! Render discipline (W22a-6, brief §24/§42): every list keeps its current model when its rows are
//! equal (compared deeply, nested models included), so an unchanged poll writes no property and
//! renders no frame. The feed's post list is one persistent `VecModel` updated row by row: a
//! comment field keeps its focus while a poll refreshes a neighbour, and a row whose *picture*
//! changed is removed and re-inserted, never updated in place (W22a-2: FemtoVG kept drawing an
//! image that arrived after the card's first paint as blank).

use std::rc::Rc;

use slint::{Image, Model, ModelRc, SharedString, VecModel};
use study_tracker_core::dashboard::format::format_minutes;
use study_tracker_core::social::avatar::{crop, Avatar, AvatarStyle, AVATAR_ICONS};
use study_tracker_core::social::feed::{
    reaction_glyph, reactors_label, FeedPost, FeedScope, PostKind,
};
use study_tracker_core::social::leaderboard::{LeaderboardPeriod, LeaderboardScope};
use study_tracker_core::social::squad::{
    assignable_roles, can_kick, can_manage_requests, SquadAction, SquadRole, SquadScorePeriod,
    SEASON_NAME, SEASON_RANGE_LABEL, TRACKING_START_LABEL,
};
use study_tracker_core::social::time::{is_recently_active, SocialTimestamp, RECENTLY_ACTIVE_MS};
use study_tracker_core::timer::WallTimestamp;

use super::{no_photo, s_avatar, seen_label, stat, Runtime};
use crate::social_controller::avatar::AvatarDraft;
use crate::{
    SAdminRow, SArenaRow, SAvatar, SAvatarChoice, SBadge, SBadgeGroup, SBoardRow, SChatMessage,
    SComment, SDetailMember, SDialogsView, SFeedPost, SFeedView, SFriendRow, SJoinRequest,
    SPollOption, SReaction, SRoleAction, SSquadCard, SSquadMember, SSquadView, SWeekRow,
};

/// `formatFeedPostedAt`: `Intl.DateTimeFormat("en", { month: "short", day: "numeric", hour:
/// "numeric", minute: "2-digit" })` - "Oct 4, 11:35 AM".
pub fn posted_label(ts: Option<SocialTimestamp>) -> String {
    let Some(ts) = ts else { return String::new() };
    chrono::DateTime::from_timestamp_millis(ts.0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%b %-d, %-I:%M %p")
                .to_string()
        })
        .unwrap_or_default()
}

/// `formatBytes`.
pub fn format_bytes(v: u64) -> String {
    const G: u64 = 1024 * 1024 * 1024;
    const M: u64 = 1024 * 1024;
    if v >= G {
        format!("{:.2} GB", v as f64 / G as f64)
    } else if v >= M {
        format!("{} MB", (v as f64 / M as f64).round())
    } else if v >= 1024 {
        format!("{} KB", (v as f64 / 1024.0).round())
    } else {
        format!("{v} B")
    }
}

/// `Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 })`.
pub fn compact(v: u64) -> String {
    let f = |x: f64, s: &str| {
        let r = (x * 10.0).round() / 10.0;
        if r.fract() == 0.0 {
            format!("{}{s}", r as u64)
        } else {
            format!("{r:.1}{s}")
        }
    };
    match v {
        0..=999 => v.to_string(),
        1_000..=999_999 => f(v as f64 / 1e3, "K"),
        1_000_000..=999_999_999 => f(v as f64 / 1e6, "M"),
        _ => f(v as f64 / 1e9, "B"),
    }
}

fn model<T: Clone + 'static>(rows: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(rows))
}

/// Deep equality for rows that hold nested models (`ModelRc` itself compares by identity).
pub trait DeepEq {
    fn deep_eq(&self, other: &Self) -> bool;
}

fn same<T: Clone + PartialEq + 'static>(a: &ModelRc<T>, b: &ModelRc<T>) -> bool {
    a.row_count() == b.row_count() && a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

fn same_deep<T: Clone + DeepEq + 'static>(a: &ModelRc<T>, b: &ModelRc<T>) -> bool {
    a.row_count() == b.row_count() && a.iter().zip(b.iter()).all(|(x, y)| x.deep_eq(&y))
}

impl DeepEq for SComment {
    fn deep_eq(&self, o: &Self) -> bool {
        self == o
    }
}

impl DeepEq for SFeedPost {
    fn deep_eq(&self, o: &Self) -> bool {
        let mut a = self.clone();
        let mut b = o.clone();
        let nested = same(&a.poll_options, &b.poll_options)
            && same(&a.reactions, &b.reactions)
            && same(&a.comments, &b.comments);
        // compare the scalar fields with the nested models made identical
        a.poll_options = b.poll_options.clone();
        a.reactions = b.reactions.clone();
        a.comments = b.comments.clone();
        let _ = &mut b;
        nested && a == b
    }
}

impl DeepEq for SSquadMember {
    fn deep_eq(&self, o: &Self) -> bool {
        let mut a = self.clone();
        let nested = same(&a.roles, &o.roles);
        a.roles = o.roles.clone();
        nested && a == *o
    }
}

impl DeepEq for SBadgeGroup {
    fn deep_eq(&self, o: &Self) -> bool {
        let mut a = self.clone();
        let nested = same(&a.badges, &o.badges) && same(&a.sub_badges, &o.sub_badges);
        a.badges = o.badges.clone();
        a.sub_badges = o.sub_badges.clone();
        nested && a == *o
    }
}

impl DeepEq for SAdminRow {
    fn deep_eq(&self, o: &Self) -> bool {
        same(&self.cells, &o.cells)
    }
}

/// Keeps `old` when `new` has the same rows (so the property does not change).
fn keep<T: Clone + PartialEq + 'static>(new: &mut ModelRc<T>, old: &ModelRc<T>) {
    if same(new, old) {
        *new = old.clone();
    }
}

fn keep_deep<T: Clone + DeepEq + 'static>(new: &mut ModelRc<T>, old: &ModelRc<T>) {
    if same_deep(new, old) {
        *new = old.clone();
    }
}

/// The persistent post list: rows updated in place, a changed picture means a new row.
pub fn sync_posts(model: &VecModel<SFeedPost>, rows: Vec<SFeedPost>) -> bool {
    let same_ids =
        model.row_count() == rows.len() && model.iter().zip(rows.iter()).all(|(a, b)| a.id == b.id);
    if !same_ids {
        model.set_vec(rows);
        return true;
    }
    let mut changed = false;
    for (i, row) in rows.into_iter().enumerate() {
        let Some(old) = model.row_data(i) else {
            continue;
        };
        if old.deep_eq(&row) {
            continue;
        }
        changed = true;
        if old.image_state != row.image_state || old.image != row.image {
            model.remove(i);
            model.insert(i, row);
        } else {
            model.set_row_data(i, row);
        }
    }
    changed
}

// ------------------------------------------------------------------------------------ feed

fn reaction_rows(p: &FeedPost, wabi: bool) -> Vec<SReaction> {
    p.reaction_keys(wabi)
        .into_iter()
        .filter_map(|k| {
            let n = p.count(&k);
            if n == 0 && !study_tracker_core::social::feed::BASE_REACTIONS.contains(&k.as_str()) {
                return None;
            }
            Some(SReaction {
                label: format!("{} {n}", reaction_glyph(&k, wabi)).into(),
                active: p.has_reacted(&k),
                tooltip: reactors_label(p.names(&k)).into(),
                key: k.into(),
            })
        })
        .collect()
}

pub fn feed_view(
    rt: &mut Runtime,
    wabi: bool,
    now: WallTimestamp,
    posts_model: &Rc<VecModel<SFeedPost>>,
) -> SFeedView {
    let c = &rt.controller;
    let f = &c.feed;
    let scope = f.scope;
    let record = c.record.clone();
    let self_id = c.identity().map(|i| i.user_id.clone());
    let profile = c.profile().cloned();
    let friends = c.friends().cloned().unwrap_or_default();
    let rows: Vec<FeedPost> = f.rows(scope).to_vec();
    let editing = f.editing.clone();
    let expanded = f.expanded_comments.clone();
    let picker = f.emoji_picker.clone();
    let failed = f.failed_images.clone();
    let comment_saving = f.comment_saving.clone();
    let drafts = f.comment_drafts.clone();
    let pending_ids: Vec<_> = record
        .as_ref()
        .map(|r| r.pending_posts.iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default();
    let mut posts = Vec::new();
    for p in &rows {
        let own = self_id.as_ref() == Some(&p.user_id) || p.is_self;
        let avatar_src = if own {
            profile
                .as_ref()
                .map(|pr| pr.avatar.clone())
                .unwrap_or(p.avatar.clone())
        } else {
            p.avatar.clone()
        };
        let avatar = s_avatar(rt, &avatar_src, &p.display_name, own);
        let (image_state, image, aspect) = match (&p.image, p.image_expired) {
            (Some(img), _) if failed.contains(&p.id) => {
                let _ = img;
                (3, no_photo(), 1.0)
            }
            (Some(img), _) => match rt.feed_pictures.get(&img.url) {
                Some((pic, a)) => (2, pic.clone(), *a),
                None => {
                    super::request_feed_image(rt, &img.url, &p.id);
                    if rt.feed_images.state(&img.url)
                        == Some(&crate::image_cache::ImageState::Failed)
                    {
                        (3, no_photo(), 1.0)
                    } else {
                        (1, no_photo(), 1.0)
                    }
                }
            },
            (None, true) => (4, no_photo(), 1.0),
            _ => (0, no_photo(), 1.0),
        };
        let poll = p.poll.clone();
        let comments: Vec<SComment> = p
            .comments
            .iter()
            .map(|cm| {
                let cav = if cm.is_self {
                    profile
                        .as_ref()
                        .map(|pr| pr.avatar.clone())
                        .unwrap_or(cm.avatar.clone())
                } else {
                    cm.avatar.clone()
                };
                SComment {
                    id: cm.id.as_str().into(),
                    person: cm.user_id.as_str().into(),
                    name: format!(
                        "{}{}",
                        cm.display_name,
                        if cm.is_self { " (You)" } else { "" }
                    )
                    .into(),
                    is_self: cm.is_self,
                    avatar: s_avatar(rt, &cav, &cm.display_name, cm.is_self),
                    time: posted_label(cm.created_at).into(),
                    body: cm.body.clone().into(),
                }
            })
            .collect();
        let is_editing = editing.as_ref().is_some_and(|e| e.post_id == p.id);
        let edit_has_new = editing
            .as_ref()
            .is_some_and(|e| e.post_id == p.id && e.image.is_some());
        posts.push(SFeedPost {
            id: p.id.as_str().into(),
            kind: if p.kind == PostKind::Milestone { 1 } else { 0 },
            person: p.user_id.as_str().into(),
            name: if p.kind == PostKind::Milestone {
                p.display_name.clone().into()
            } else {
                format!("{}{}", p.display_name, if own { " (You)" } else { "" }).into()
            },
            is_self: own,
            pending: pending_ids.contains(&p.id),
            avatar,
            time: posted_label(p.created_at).into(),
            subject: if p.subject.is_empty() {
                "Study session".into()
            } else {
                p.subject.clone().into()
            },
            detail: if p.detail.is_empty() {
                format!(
                    "{} · {}",
                    format_minutes(p.minutes),
                    if p.preset_label.is_empty() {
                        "Focus"
                    } else {
                        &p.preset_label
                    }
                )
                .into()
            } else {
                p.detail.clone().into()
            },
            icon: if p.icon.is_empty() {
                if p.kind == PostKind::Milestone {
                    "🏆".into()
                } else {
                    "✦".into()
                }
            } else {
                p.icon.clone().into()
            },
            note: if p.kind == PostKind::Milestone && p.note.is_empty() {
                p.detail.clone().into()
            } else {
                p.note.clone().into()
            },
            has_poll: poll.is_some(),
            poll_question: poll
                .as_ref()
                .map(|q| q.question.clone())
                .unwrap_or_default()
                .into(),
            poll_multiple: poll.as_ref().is_some_and(|q| q.multiple),
            poll_options: model(
                poll.as_ref()
                    .map(|q| {
                        q.options
                            .iter()
                            .map(|o| {
                                let pct = q.percent(o);
                                SPollOption {
                                    id: o.id.as_str().into(),
                                    text: o.text.clone().into(),
                                    selected: o.selected,
                                    fill: pct as f32 / 100.0,
                                    count: format!("{} · {pct}%", o.votes).into(),
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
            poll_total: poll
                .as_ref()
                .map(|q| {
                    format!(
                        "{} vote{}",
                        q.total_votes,
                        if q.total_votes == 1 { "" } else { "s" }
                    )
                })
                .unwrap_or_default()
                .into(),
            image_state,
            image,
            image_aspect: aspect,
            reactions: model(reaction_rows(p, wabi)),
            picker: picker.as_ref() == Some(&p.id),
            comments_open: expanded.contains(&p.id),
            comment_count: p.comments.len() as i32,
            comments: model(comments),
            draft: drafts.get(&p.id).cloned().unwrap_or_default().into(),
            draft_rev: rt.draft_revs.get(p.id.as_str()).copied().unwrap_or(0),
            saving_comment: comment_saving.as_ref() == Some(&p.id),
            editing: is_editing,
            edit_image_label: if p.image.is_some() || p.image_expired || edit_has_new {
                "Replace image".into()
            } else {
                "Add image".into()
            },
            edit_can_remove: p.image.is_some() && !edit_has_new,
        });
    }
    sync_posts(posts_model, posts);
    let c = &rt.controller;
    let f = &c.feed;
    let live: Vec<String> = friends
        .friends
        .iter()
        .filter(|fr| is_recently_active(fr.last_seen_at, now, RECENTLY_ACTIVE_MS))
        .map(|fr| fr.display_name.clone())
        .collect();
    let latest = rt.latest_session.clone();
    let posted = latest.as_ref().is_some_and(|l| c.session_posted(&l.id));
    let r2 = if c.is_owner() { f.r2.clone() } else { None };
    let week_rows = c.board(LeaderboardScope::Friends, LeaderboardPeriod::Weekly);
    let week_rows: Vec<_> = week_rows.into_iter().take(6).collect();
    let max = week_rows
        .iter()
        .map(|e| e.minutes)
        .max()
        .unwrap_or(0)
        .max(1);
    let prof_avatar = profile.as_ref().map(|p| p.avatar.clone());
    let edit = f.editing.clone();
    let mut week = Vec::new();
    for e in &week_rows {
        let av = if e.is_self {
            prof_avatar.clone().unwrap_or(e.avatar.clone())
        } else {
            e.avatar.clone()
        };
        week.push(SWeekRow {
            person: e.user_id.as_str().into(),
            name: format!(
                "{}{}",
                e.display_name,
                if e.is_self { " (You)" } else { "" }
            )
            .into(),
            is_self: e.is_self,
            avatar: s_avatar(rt, &av, &e.display_name, e.is_self),
            minutes: format_minutes(e.minutes).into(),
            bar: e.minutes as f32 / max as f32,
        });
    }
    let stories: Vec<SFriendRow> = friends
        .friends
        .iter()
        .take(10)
        .map(|fr| SFriendRow {
            id: fr.user_id.as_str().into(),
            name: fr.display_name.clone().into(),
            code: fr.friend_code.as_str().into(),
            detail: Default::default(),
            avatar: SAvatar {
                photo: no_photo(),
                ..Default::default()
            },
            live: is_recently_active(fr.last_seen_at, now, RECENTLY_ACTIVE_MS),
        })
        .collect();
    let c = &rt.controller;
    let f = &c.feed;
    let image_preview = f
        .image_draft
        .as_ref()
        .map(|i| picture(&i.preview))
        .unwrap_or_else(no_photo);
    let edit_preview = edit
        .as_ref()
        .and_then(|e| e.image.as_ref())
        .map(|i| picture(&i.preview));
    let lightbox_post = f
        .expanded_image
        .as_ref()
        .and_then(|id| f.find(id))
        .filter(|p| p.image.is_some())
        .cloned();
    let (lightbox, lightbox_image, lightbox_aspect, lightbox_label) = match &lightbox_post {
        Some(p) => {
            let url = &p.image.as_ref().expect("filtered").url;
            match rt.feed_pictures.get(url) {
                Some((pic, a)) => (
                    true,
                    pic.clone(),
                    *a,
                    format!("{}'s feed post image", p.display_name),
                ),
                None => (
                    true,
                    no_photo(),
                    1.0,
                    format!("{}'s feed post image", p.display_name),
                ),
            }
        }
        None => (false, no_photo(), 1.0, String::new()),
    };
    SFeedView {
        scope: if scope == FeedScope::Friends { 0 } else { 1 },
        loading: f.loading,
        syncing: c.syncing,
        live_count: live.len() as i32,
        live_names: live
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ")
            .into(),
        has_session: latest.is_some(),
        posted,
        session_title: latest
            .as_ref()
            .map(|l| {
                format!(
                    "{} {} block",
                    l.minutes_label,
                    if l.exam { "exam" } else { "study" }
                )
            })
            .unwrap_or_default()
            .into(),
        poll_open: f.poll_open,
        poll_has_draft: f.poll_draft.has_content(),
        poll_multiple: f.poll_draft.multiple,
        poll_question: f.poll_draft.question.clone().into(),
        poll_options: model(
            f.poll_draft
                .options
                .iter()
                .map(|o| SharedString::from(o.as_str()))
                .collect(),
        ),
        poll_rev: rt.poll_rev,
        image_draft: f.image_draft.is_some(),
        image_preview,
        preparing_image: f.preparing_image,
        uploads_paused: c.uploads_paused(),
        r2_banner: r2.as_ref().is_some_and(|u| u.warning || u.paused),
        r2_paused: r2.as_ref().is_some_and(|u| u.paused),
        r2_text: r2
            .as_ref()
            .map(|u| {
                format!(
                    "Storage {} / {} · Writes {} / {} · Reads {} / {}",
                    format_bytes(u.storage_bytes),
                    format_bytes(u.storage_hard_bytes),
                    compact(u.class_a_ops),
                    compact(u.class_a_hard),
                    compact(u.class_b_ops),
                    compact(u.class_b_hard)
                )
            })
            .unwrap_or_default()
            .into(),
        posts: ModelRc::from(posts_model.clone()),
        week: model(week),
        edit_saving: f.saving,
        edit_has_preview: edit_preview.is_some(),
        edit_preview: edit_preview.unwrap_or_else(no_photo),
        edit_remove_hint: edit
            .as_ref()
            .is_some_and(|e| e.remove_image && e.image.is_none()),
        edit_pending_hint: edit
            .as_ref()
            .is_some_and(|e| pending_ids.contains(&e.post_id)),
        edit_rev: rt.edit_rev,
        edit_note: edit
            .as_ref()
            .map(|e| e.note.clone())
            .unwrap_or_default()
            .into(),
        lightbox,
        lightbox_image,
        lightbox_aspect,
        lightbox_label: lightbox_label.into(),
        stories: model(stories),
    }
}

pub fn picture(img: &crate::net::images::DecodedImage) -> Image {
    Image::from_rgba8(
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &img.rgba, img.width, img.height,
        ),
    )
}

pub fn keep_feed(new: &mut SFeedView, old: &SFeedView) {
    keep(&mut new.week, &old.week);
    keep(&mut new.poll_options, &old.poll_options);
    keep(&mut new.stories, &old.stories);
    for n in [
        &mut new.image_preview,
        &mut new.edit_preview,
        &mut new.lightbox_image,
    ] {
        unset_to_placeholder(n);
    }
}

/// Slint compares two empty `Image`s as *different* (`ImageInner::None` has no equality), so a
/// view holding one never equals its previous copy and every push would rewrite it (and redraw).
/// Unset pictures therefore all share the one 1x1 placeholder, which compares equal.
fn unset_to_placeholder(i: &mut Image) {
    if !img_set(i) {
        *i = no_photo();
    }
}

fn img_set(i: &Image) -> bool {
    let s = i.size();
    !(s.width <= 1 && s.height <= 1)
}

// ---------------------------------------------------------------------------------- squads

fn role_key(r: SquadRole) -> i32 {
    match r {
        SquadRole::Leader => 0,
        SquadRole::CoLeader => 1,
        SquadRole::Elder => 2,
        SquadRole::Member => 3,
    }
}

pub fn role_from_key(k: i32) -> SquadRole {
    match k {
        0 => SquadRole::Leader,
        1 => SquadRole::CoLeader,
        2 => SquadRole::Elder,
        _ => SquadRole::Member,
    }
}

fn card(s: &study_tracker_core::social::squad::SquadSearchResult) -> SSquadCard {
    SSquadCard {
        id: s.id.as_str().into(),
        name: s.name.clone().into(),
        meta: format!(
            "{} · {}/{} members · {}",
            if s.is_private { "Private" } else { "Public" },
            s.member_count,
            s.max_members,
            format_minutes(s.total_minutes)
        )
        .into(),
        action: match s.action {
            SquadAction::Join => 0,
            SquadAction::Request => 1,
            _ => 2,
        },
        badge: s.action.badge().into(),
    }
}

pub fn squad_view(rt: &mut Runtime, now: WallTimestamp) -> SSquadView {
    let c = &rt.controller;
    let sq = &c.squad;
    let record = c.record.clone();
    let squad = record.as_ref().and_then(|r| r.squad.clone());
    let profile = c.profile().cloned();
    let my_role = squad.as_ref().map(|s| s.my_role);
    let mut v = SSquadView {
        has_squad: squad.is_some(),
        busy: c.syncing,
        private_draft: sq.private_draft,
        searching: sq.searching,
        suggestions: model(sq.suggestions.iter().map(card).collect()),
        suggestions_loading: sq.suggestions_loading,
        can_reload: sq.has_pool_to_reload(),
        results: model(sq.results.iter().map(card).collect()),
        pending_note: record
            .as_ref()
            .filter(|r| !r.squad_outgoing.is_empty())
            .map(|r| {
                format!(
                    "Pending request: {}",
                    r.squad_outgoing
                        .iter()
                        .map(|q| q.squad_name.clone().unwrap_or_else(|| "Squad".into()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .unwrap_or_default()
            .into(),
        name_rev: rt.squad_name_rev,
        period: match c.period {
            LeaderboardPeriod::Daily => 0,
            LeaderboardPeriod::Weekly => 1,
            LeaderboardPeriod::Overall => 2,
        },
        chat_draft: sq.chat_draft.clone().into(),
        chat_rev: rt.chat_rev,
        settings_editing: sq.settings_editing,
        settings_name: sq.settings_name.clone().into(),
        settings_private: sq.settings_private,
        settings_rev: rt.settings_rev,
        arena_period: match sq.score_period {
            SquadScorePeriod::Daily => 0,
            SquadScorePeriod::Season => 1,
            SquadScorePeriod::Overall => 2,
        },
        season_name: SEASON_NAME.into(),
        season_text: format!(
            "Season: {SEASON_RANGE_LABEL} · Overall tracking started: {TRACKING_START_LABEL}"
        )
        .into(),
        can_manage_requests: can_manage_requests(my_role),
        ..Default::default()
    };
    if let Some(s) = &squad {
        v.kicker = if s.is_private {
            "Private squad"
        } else {
            "Public squad"
        }
        .into();
        v.name = s.name.clone().into();
        v.meta = format!(
            "{}/4 members · Your rank: {}",
            s.member_count,
            s.my_role.label()
        )
        .into();
        v.is_leader = s.my_role == SquadRole::Leader;
        v.stats = model(vec![
            stat(
                format_minutes(s.total_minutes),
                "Total squad focus".into(),
                "",
                0,
            ),
            stat(s.total_sessions.to_string(), "Sessions".into(), "", 0),
            stat(
                record
                    .as_ref()
                    .map_or(0, |r| r.squad_incoming.len())
                    .to_string(),
                "Pending requests".into(),
                "",
                0,
            ),
        ]);
        let mut members = Vec::new();
        for m in &s.members {
            let roles = if m.is_self {
                Vec::new()
            } else {
                assignable_roles(s.my_role, m.role)
            };
            let kick = !m.is_self && can_kick(s.my_role, m.role);
            let manage = !roles.is_empty() || kick;
            let av = if m.is_self {
                profile
                    .as_ref()
                    .map(|p| p.avatar.clone())
                    .unwrap_or(m.avatar.clone())
            } else {
                m.avatar.clone()
            };
            members.push(SSquadMember {
                id: m.user_id.as_str().into(),
                name: format!(
                    "{}{}",
                    m.display_name,
                    if m.is_self { " (You)" } else { "" }
                )
                .into(),
                is_self: m.is_self,
                avatar: s_avatar(rt, &av, &m.display_name, m.is_self),
                role: m.role.label().into(),
                role_key: role_key(m.role),
                detail: format!(
                    "{} · {} · {} sessions",
                    m.friend_code.as_ref().map(|c| c.as_str()).unwrap_or(""),
                    format_minutes(m.minutes),
                    m.sessions
                )
                .into(),
                expanded: rt.controller.squad.expanded_member.as_ref() == Some(&m.user_id),
                manage_title: if manage {
                    format!("Manage {}", m.display_name)
                } else if m.is_self {
                    "This is you".into()
                } else {
                    "No actions available".into()
                }
                .into(),
                manage_detail: format!(
                    "{} · joined {}",
                    m.role.label(),
                    m.joined_at.map(|t| seen_label(t, now)).unwrap_or_default()
                )
                .into(),
                roles: model(
                    roles
                        .into_iter()
                        .map(|r| SRoleAction {
                            label: format!("Make {}", r.label()).into(),
                            role: role_key(r),
                            current: r == m.role,
                        })
                        .collect(),
                ),
                can_kick: kick,
            });
        }
        v.members = model(members);
        let board = rt
            .controller
            .board(LeaderboardScope::Squad, rt.controller.period);
        let prof_avatar = profile.as_ref().map(|p| p.avatar.clone());
        let mut rows = Vec::new();
        for e in &board {
            let av = if e.is_self {
                prof_avatar.clone().unwrap_or(e.avatar.clone())
            } else {
                e.avatar.clone()
            };
            rows.push(SBoardRow {
                id: e.user_id.as_str().into(),
                rank: e.rank as i32,
                name: e.display_name.clone().into(),
                code: e.friend_code.as_str().into(),
                hours: format_minutes(e.minutes).into(),
                sessions: e.sessions.to_string().into(),
                bar: 0.0,
                is_self: e.is_self,
                avatar: s_avatar(rt, &av, &e.display_name, e.is_self),
            });
        }
        v.board = model(rows);
        let msgs = rt.controller.squad.messages.clone();
        let mut chat = Vec::new();
        for m in &msgs {
            let av = if m.is_self {
                prof_avatar.clone().unwrap_or(m.avatar.clone())
            } else {
                m.avatar.clone()
            };
            chat.push(SChatMessage {
                id: m.id.as_str().into(),
                name: m.display_name.clone().into(),
                role: m.role.label().into(),
                body: m.body.clone().into(),
                is_self: m.is_self,
                avatar: s_avatar(rt, &av, &m.display_name, m.is_self),
            });
        }
        v.messages = model(chat);
        let reqs = record
            .as_ref()
            .map(|r| r.squad_incoming.clone())
            .unwrap_or_default();
        let mut requests = Vec::new();
        for q in &reqs {
            let name = q.display_name.clone().unwrap_or_else(|| "Student".into());
            let av = q
                .avatar
                .clone()
                .unwrap_or_else(|| Avatar::default_for(&name));
            requests.push(SJoinRequest {
                id: q.id.as_str().into(),
                code: q
                    .friend_code
                    .as_ref()
                    .map(|c| c.as_str().to_string())
                    .unwrap_or_default()
                    .into(),
                avatar: s_avatar(rt, &av, &name, false),
                name: name.into(),
            });
        }
        v.requests = model(requests);
    }
    // the Squad Arena
    let c = &rt.controller;
    let period = c.squad.score_period;
    let my_squad = squad.as_ref().map(|s| s.id.clone());
    let arena: Vec<SArenaRow> = c
        .squad
        .scores
        .get(&period)
        .map(|rows| {
            rows.iter()
                .map(|e| {
                    let mine = my_squad.as_ref() == Some(&e.squad_id);
                    let avg = format_minutes(e.average_minutes.round() as u64);
                    SArenaRow {
                        id: e.squad_id.as_str().into(),
                        rank: e.rank as i32,
                        name: format!(
                            "{}{}",
                            e.squad_name,
                            if mine { " (Your squad)" } else { "" }
                        )
                        .into(),
                        is_self: mine,
                        meta: format!(
                            "{} · {} members · avg {avg}",
                            if e.is_private { "Private" } else { "Public" },
                            e.member_count
                        )
                        .into(),
                        points: if period == SquadScorePeriod::Daily {
                            avg.clone().into()
                        } else {
                            format!("{} pts", e.points).into()
                        },
                        sub: if period == SquadScorePeriod::Daily {
                            format!("{} pts if day ends now", e.points).into()
                        } else {
                            format!("{} scored days", e.scored_days.unwrap_or(0)).into()
                        },
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    v.arena = model(arena);
    // the details dialog
    if let Some(view) = c.squad.viewing.clone() {
        v.details_open = true;
        v.d_rank = view.entry.rank as i32;
        let private = view
            .details
            .as_ref()
            .map_or(view.entry.is_private, |d| d.is_private);
        v.d_kicker = if private {
            "Private squad"
        } else {
            "Public squad"
        }
        .into();
        v.d_name = view
            .details
            .as_ref()
            .map_or(view.entry.squad_name.clone(), |d| d.name.clone())
            .into();
        let action = view.details.as_ref().map(|d| d.action);
        v.d_badge = action
            .map_or("Open to join", SquadAction::details_badge)
            .into();
        v.d_can_join = action.is_some_and(SquadAction::can_join_or_request);
        v.d_join_label = if action == Some(SquadAction::Join) {
            "Join squad"
        } else {
            "Request to join"
        }
        .into();
        v.d_state = if view.loading {
            0
        } else if view.details.is_some() {
            1
        } else {
            2
        };
        if let Some(d) = &view.details {
            let label = match period {
                SquadScorePeriod::Season => "Season",
                SquadScorePeriod::Overall => "Overall",
                SquadScorePeriod::Daily => "Today if held",
            };
            v.d_stats = model(vec![
                stat(
                    format!("{}/{}", d.member_count, d.max_members),
                    "Members".into(),
                    "#",
                    0,
                ),
                stat(format_minutes(d.total_minutes), "Today".into(), "◆", 0),
                stat(format!("{} pts", view.entry.points), label.into(), "★", 0),
                stat(
                    format_minutes(d.previous_day_average_minutes.round() as u64),
                    "Yesterday avg".into(),
                    "↯",
                    0,
                ),
            ]);
            let prof_avatar = profile.as_ref().map(|p| p.avatar.clone());
            let mut members = Vec::new();
            for m in &d.members {
                let av = if m.is_self {
                    prof_avatar.clone().unwrap_or(m.avatar.clone())
                } else {
                    m.avatar.clone()
                };
                members.push(SDetailMember {
                    name: format!(
                        "{}{}",
                        m.display_name,
                        if m.is_self { " (You)" } else { "" }
                    )
                    .into(),
                    detail: format!(
                        "{} · today {} · {} sessions",
                        m.role.label(),
                        format_minutes(m.minutes),
                        m.sessions
                    )
                    .into(),
                    avatar: s_avatar(rt, &av, &m.display_name, m.is_self),
                });
            }
            v.d_members = model(members);
        }
    }
    v
}

pub fn keep_squad(new: &mut SSquadView, old: &SSquadView) {
    keep(&mut new.suggestions, &old.suggestions);
    keep(&mut new.results, &old.results);
    keep(&mut new.stats, &old.stats);
    keep_deep(&mut new.members, &old.members);
    keep(&mut new.board, &old.board);
    keep(&mut new.messages, &old.messages);
    keep(&mut new.requests, &old.requests);
    keep(&mut new.arena, &old.arena);
    keep(&mut new.d_stats, &old.d_stats);
    keep(&mut new.d_members, &old.d_members);
}

// --------------------------------------------------------------------------------- dialogs

fn choice(
    rt: &mut Runtime,
    a: Avatar,
    name: &str,
    label: &str,
    selected: bool,
    key: &str,
) -> SAvatarChoice {
    SAvatarChoice {
        avatar: s_avatar(rt, &a, name, true),
        label: label.into(),
        selected,
        key: key.into(),
    }
}

pub fn dialogs_view(rt: &mut Runtime) -> SDialogsView {
    let c = &rt.controller;
    let name = c
        .profile()
        .map(|p| p.display_name.clone())
        .unwrap_or_default();
    let draft = c.avatar.draft.clone();
    let mut v = SDialogsView {
        avatar_open: c.avatar.open,
        letter_picker: c.avatar.letter_picker,
        avatar_busy: c.avatar.uploading || c.avatar.preparing || c.syncing,
        badges_open: c.avatar.badges_open,
        telemetry_on: c.bg.telemetry_enabled,
        is_owner: c.is_owner(),
        admin_loading: c.bg.admin_loading,
        admin_open: c.bg.admin_open,
        ..Default::default()
    };
    if let Some(conf) = &c.squad.confirm {
        v.confirm_open = true;
        v.confirm_text = conf.text().into();
    }
    if let Some(n) = &c.feed.notice {
        v.notice_open = true;
        v.notice_name = n.commenter_name.clone().into();
        v.notice_body = study_tracker_core::social::feed::notice_excerpt(&n.body).into();
    }
    if let Some(a) = &c.bg.announcement {
        v.announcement_open = true;
        v.announcement_title = a.title.clone().into();
        v.announcement_body = a.body.clone().into();
    }
    let version = crate::net::device::app_metadata().version;
    let valid = study_tracker_core::social::announcement::is_valid_app_version(&version);
    v.notice_enabled = !c.bg.notice_sending && valid;
    v.notice_label = if c.bg.notice_sending {
        "Notifying...".to_string()
    } else if valid {
        format!("Notify users below {version}")
    } else {
        "Version not ready".to_string()
    }
    .into();
    if let Some(u) = &c.bg.admin_usage {
        let num =
            |f: fn(&crate::net::social_ext::AdminSummary) -> u64| u.summary.as_ref().map_or(0, f);
        if u.summary.is_some() {
            v.admin_summary = format!(
                "{} synced users · {} active 24h · {} active 7d · {} opt-in installs",
                num(|s| s.user_count),
                num(|s| s.active_24h),
                num(|s| s.active_7d),
                u.telemetry.len()
            )
            .into();
        }
        // `.admin-usage-summary-grid`
        v.admin_counts = model(vec![
            num(|s| s.user_count).to_string().into(),
            num(|s| s.active_24h).to_string().into(),
            num(|s| s.active_7d).to_string().into(),
            u.telemetry.len().to_string().into(),
            num(|s| s.flagged_count).to_string().into(),
        ]);
        let row = |cells: Vec<String>| SAdminRow {
            cells: model(cells.into_iter().map(SharedString::from).collect()),
        };
        let or = |s: &Option<String>, d: &str| {
            s.clone()
                .filter(|x| !x.is_empty())
                .unwrap_or_else(|| d.to_string())
        };
        let short = |s: &Option<String>| {
            s.as_deref()
                .map(|x| x.chars().take(8).collect::<String>())
                .unwrap_or_else(|| "unknown".into())
        };
        let user = |name: &str, code: &str, flagged: bool| {
            let name = if name.is_empty() { "Unknown" } else { name };
            if flagged {
                format!("{name} {code} · Flagged")
            } else {
                format!("{name} {code}")
            }
        };
        v.admin_users = model(
            u.users
                .iter()
                .map(|x| {
                    row(vec![
                        user(&x.display_name, &x.friend_code, x.is_flagged),
                        admin_timestamp(Some(&x.last_seen_at)),
                        or(&x.app_version, "unknown"),
                        format!(
                            "{} · device {}",
                            x.device_label
                                .clone()
                                .or(x.app_platform.clone())
                                .unwrap_or_else(|| "unknown device".into()),
                            short(&x.device_fingerprint_hash)
                        ),
                        or(&x.app_runtime_channel, "unknown"),
                    ])
                })
                .collect(),
        );
        v.admin_flagged = model(
            u.flagged
                .iter()
                .map(|x| {
                    row(vec![
                        user(&x.display_name, &x.friend_code, false),
                        or(&x.flagged_reason, "—"),
                        or(&x.signup_country, "unknown"),
                        or(&x.signup_as_organization, "unknown"),
                        admin_timestamp(Some(&x.last_seen_at)),
                    ])
                })
                .collect(),
        );
        v.admin_installs = model(
            u.telemetry
                .iter()
                .map(|t| {
                    row(vec![
                        short(&Some(t.install_id.clone())),
                        admin_timestamp(Some(&t.last_seen_at)),
                        admin_timestamp(Some(&t.created_at)),
                        if t.app_version.is_empty() {
                            "unknown".into()
                        } else {
                            t.app_version.clone()
                        },
                        if t.app_platform.is_empty() {
                            "unknown".into()
                        } else {
                            t.app_platform.clone()
                        },
                        if t.app_runtime_channel.is_empty() {
                            "unknown".into()
                        } else {
                            t.app_runtime_channel.clone()
                        },
                    ])
                })
                .collect(),
        );
        v.admin_abuse = model(
            u.abuse
                .iter()
                .map(|e| {
                    row(vec![
                        e.event_type.clone(),
                        or(&e.path, "—"),
                        or(&e.country, "unknown"),
                        or(&e.as_organization, "unknown"),
                        or(&e.user_id, "—"),
                        admin_timestamp(Some(&e.created_at)),
                    ])
                })
                .collect(),
        );
    }
    // the avatar editor
    if let Some(d) = draft {
        let (mode, avatar, letter, has_photo) = match &d {
            AvatarDraft::Letter { letter, style } => (
                0,
                Avatar::Letter {
                    letter: letter.clone(),
                    style: *style,
                },
                letter.clone(),
                false,
            ),
            AvatarDraft::Icon { icon } => {
                (1, Avatar::Icon { icon: icon.clone() }, String::new(), false)
            }
            AvatarDraft::Photo {
                remote,
                local,
                name: n,
            } => {
                let a = match remote {
                    Some(url) => Avatar::Photo {
                        name: n.clone(),
                        url: url.clone(),
                        mime_type: "image/webp".into(),
                    },
                    None => Avatar::Photo {
                        name: n.clone(),
                        url: String::new(),
                        mime_type: "image/webp".into(),
                    },
                };
                (2, a, String::new(), remote.is_some() || local.is_some())
            }
        };
        v.avatar_mode = mode;
        v.letter = letter.clone().into();
        v.avatar_has_photo = has_photo;
        let mut draft_view = s_avatar(rt, &avatar, &name, true);
        if let AvatarDraft::Photo {
            local: Some(img), ..
        } = &d
        {
            draft_view.photo = picture(&img.preview);
            draft_view.has_photo = true;
            draft_view.kind = 7;
        }
        v.avatar_draft = draft_view;
        let mut styles = Vec::new();
        for s in AvatarStyle::ALL {
            let selected = matches!(&d, AvatarDraft::Letter { style, .. } if *style == s);
            let l = if letter.is_empty() {
                study_tracker_core::social::avatar::first_avatar_letter(&name)
            } else {
                letter.clone()
            };
            styles.push(choice(
                rt,
                Avatar::Letter {
                    letter: l,
                    style: s,
                },
                &name,
                s.label(),
                selected,
                s.id(),
            ));
        }
        v.avatar_styles = model(styles);
        let mut icons = Vec::new();
        for i in AVATAR_ICONS {
            let selected = matches!(&d, AvatarDraft::Icon { icon } if icon == i);
            icons.push(choice(
                rt,
                Avatar::Icon {
                    icon: i.to_string(),
                },
                &name,
                "",
                selected,
                i,
            ));
        }
        v.avatar_icons = model(icons);
    }
    // the crop editor
    let c = &rt.controller;
    if let Some(cr) = &c.avatar.crop {
        let (w, h) = (
            f64::from(cr.source.image.width),
            f64::from(cr.source.image.height),
        );
        let (left, top, dw, dh) = crop::image_rect(cr.crop, w, h);
        v.crop_open = true;
        if rt.crop_picture.as_ref().map(|(k, _)| *k)
            != Some(std::sync::Arc::as_ptr(&cr.source) as usize)
        {
            rt.crop_picture = Some((
                std::sync::Arc::as_ptr(&cr.source) as usize,
                picture(&cr.source.image),
            ));
        }
        v.crop_image = rt
            .crop_picture
            .as_ref()
            .map(|(_, p)| p.clone())
            .unwrap_or_else(no_photo);
        v.crop_x = left as f32;
        v.crop_y = top as f32;
        v.crop_w = dw as f32;
        v.crop_h = dh as f32;
        v.crop_zoom = cr.crop.zoom as f32;
        v.crop_dragging = cr.dragging();
    } else {
        rt.crop_picture = None;
    }
    if rt.controller.avatar.badges_open {
        v.badge_groups = model(badge_groups());
    }
    v
}

fn badge(a: &study_tracker_core::break_room::achievements::Achievement) -> SBadge {
    SBadge {
        icon: a.icon.into(),
        name: a.name.into(),
        how: a.how.clone().into(),
        earned: a.earned,
        count: if a.daily {
            format!("×{}", a.count).into()
        } else {
            SharedString::new()
        },
    }
}

/// `formatAdminTimestamp`: "Never" when absent; a SQLite `YYYY-MM-DD HH:MM:SS` (UTC) or ISO time
/// as "Oct 4 02:05 PM" in local time; anything else verbatim.
pub fn admin_timestamp(value: Option<&str>) -> String {
    use chrono::{DateTime, Local, NaiveDateTime, Utc};
    let Some(v) = value.filter(|v| !v.is_empty()) else {
        return "Never".into();
    };
    let parsed = if v.contains('T') {
        DateTime::parse_from_rfc3339(v)
            .ok()
            .map(|d| d.with_timezone(&Utc))
    } else {
        NaiveDateTime::parse_from_str(v, "%Y-%m-%d %H:%M:%S")
            .ok()
            .map(|n| n.and_utc())
    };
    match parsed {
        Some(d) => d
            .with_timezone(&Local)
            .format("%b %-d %I:%M %p")
            .to_string(),
        None => v.to_string(),
    }
}

/// `profileBadgeGroups`, from the Break Room's evaluated achievements (one source of truth).
fn badge_groups() -> Vec<SBadgeGroup> {
    use study_tracker_core::break_room::achievements::Category;
    let all =
        crate::app_break_room::with_controller(|c| c.achievements().to_vec()).unwrap_or_default();
    let of = |cat: Category| -> Vec<SBadge> {
        all.iter()
            .filter(|a| a.category == cat)
            .map(badge)
            .collect()
    };
    vec![
        SBadgeGroup {
            title: Category::BreakRoom.title().into(),
            source: Category::BreakRoom.source().into(),
            badges: model(of(Category::BreakRoom)),
            sub_title: Category::PetRock.title().into(),
            sub_source: Category::PetRock.source().into(),
            sub_badges: model(of(Category::PetRock)),
        },
        SBadgeGroup {
            title: Category::FocusFossil.title().into(),
            source: Category::FocusFossil.source().into(),
            badges: model(of(Category::FocusFossil)),
            ..Default::default()
        },
        SBadgeGroup {
            title: Category::Garden.title().into(),
            source: Category::Garden.source().into(),
            badges: model(of(Category::Garden)),
            ..Default::default()
        },
    ]
}

pub fn keep_dialogs(new: &mut SDialogsView, old: &SDialogsView) {
    keep(&mut new.avatar_styles, &old.avatar_styles);
    keep(&mut new.avatar_icons, &old.avatar_icons);
    keep_deep(&mut new.badge_groups, &old.badge_groups);
    keep_deep(&mut new.admin_users, &old.admin_users);
    keep_deep(&mut new.admin_flagged, &old.admin_flagged);
    keep_deep(&mut new.admin_abuse, &old.admin_abuse);
    keep_deep(&mut new.admin_installs, &old.admin_installs);
    unset_to_placeholder(&mut new.avatar_draft.photo);
    unset_to_placeholder(&mut new.crop_image);
    keep(&mut new.admin_counts, &old.admin_counts);
}
