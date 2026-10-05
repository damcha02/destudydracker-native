//! Stage 22b end to end, headless: Feed, Squads, avatar upload, verified sessions, telemetry,
//! announcements and the owner actions - controller -> real `UreqTransport` -> `social-mock`.

use super::*;
use crate::net::images::DecodedImage;
use crate::net::social_ext::Upload;
use crate::social_controller::avatar::AvatarDraft;
use crate::social_controller::background::TimerView;
use crate::social_controller::feed::{LatestSession, PreparedImage};
use crate::social_controller::squad::Confirm;
use study_tracker_core::social::feed::FeedScope;
use study_tracker_core::social::squad::{SquadRole, SquadScorePeriod};
use study_tracker_core::social::verified::VerifiedAnchor;
use study_tracker_core::social::{PollOptionId, PostId, SquadId, VerifiedSessionId};
use study_tracker_core::timer::TimerPhase;

fn world_22b(in_squad: bool) -> social_mock::World {
    let mut w = seed::demo(NOW, false);
    seed::add_22b(&mut w, in_squad);
    w
}

fn post(id: &str) -> PostId {
    PostId::parse(id).unwrap()
}

fn open_feed(rig: &mut Rig, c: &mut SocialController) {
    let app = rig.env.app.clone();
    let ctx_out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(c, ctx_out);
    if c.subtab != Subtab::Feed {
        let out = c.set_subtab(Subtab::Feed, &app, now(), &rig.env.ctx());
        rig.drive(c, out);
    }
}

fn png_upload() -> PreparedImage {
    let bytes = seed::photo_png(64, 48, 1);
    PreparedImage {
        upload: Upload {
            bytes,
            mime: "image/png",
        },
        preview: DecodedImage {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
        },
    }
}

fn latest() -> LatestSession {
    LatestSession {
        id: "s1".into(),
        exam: false,
        goal: String::new(),
        minutes: 40,
        preset_label: String::new(),
        ended_at: WallTimestamp::from_unix_millis(NOW - 600_000),
        course_name: "Analysis".into(),
        minutes_label: "40m".into(),
    }
}

#[test]
fn no_identity_makes_zero_22b_requests() {
    let rig = Rig::new(world_22b(true));
    let mut c = none(&rig);
    let ctx = rig.env.ctx();
    let mut out = Vec::new();
    out.extend(c.refresh_feed());
    out.extend(c.on_feed_poll());
    out.extend(c.set_feed_scope(FeedScope::Global));
    out.extend(c.post_latest(
        Some(&latest()),
        || PollOptionId::parse("o").unwrap(),
        now(),
        &ctx,
    ));
    out.extend(c.toggle_reaction(&post("post-bob-1"), "fire"));
    out.extend(c.vote(
        &post("post-bob-1"),
        &PollOptionId::parse("opt-bob-1").unwrap(),
    ));
    out.extend(c.submit_comment(&post("post-bob-1")));
    out.extend(c.save_edit());
    out.extend(c.delete_post(&post("post-self-1")));
    out.extend(c.create_squad());
    out.extend(c.search_squads());
    out.extend(c.load_suggestions(true));
    out.extend(c.join_squad(&SquadId::parse("squad-owl").unwrap()));
    out.extend(c.send_chat());
    out.extend(c.leave_squad(true));
    out.extend(c.save_avatar(now(), &ctx));
    out.extend(c.observe_timer(
        TimerView {
            phase: TimerPhase::Study,
            running: true,
        },
        &ctx,
    ));
    out.extend(c.verified_tick(&ctx));
    out.extend(c.poll_announcement());
    out.extend(c.load_admin_usage());
    // telemetry is off by default: nothing at start-up or on the hourly tick
    out.extend(c.telemetry_startup([1; 16]));
    out.extend(c.telemetry_tick([1; 16]));
    assert!(out.is_empty(), "{out:?}");
    assert!(rig.server.log().is_empty(), "the server saw nothing");
}

#[test]
fn the_feed_loads_reacts_votes_comments_edits_and_deletes() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    open_feed(&mut rig, &mut c);
    assert_eq!(
        rig.server.count("/feed"),
        1,
        "the Feed subtab loads its scope once"
    );
    let rows = c.feed.rows(FeedScope::Friends);
    assert!(rows.iter().any(|p| p.id.as_str() == "post-bob-1"));
    assert!(
        !rows.iter().any(|p| p.id.as_str() == "post-kenji-1"),
        "friends log: friends only"
    );
    // global
    let out = c.set_feed_scope(FeedScope::Global);
    rig.drive(&mut c, out);
    assert!(c
        .feed
        .rows(FeedScope::Global)
        .iter()
        .any(|p| p.id.as_str() == "post-kenji-1"));
    // reaction: optimistic + one request; the server toggles
    let bob = post("post-bob-1");
    let before = c.feed.find(&bob).unwrap().count("clap");
    let out = c.toggle_reaction(&bob, "clap");
    assert_eq!(c.feed.find(&bob).unwrap().count("clap"), before + 1);
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/feed/react"), 1);
    assert!(rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .reactions
        .iter()
        .any(|(p, u, e)| p == "post-bob-1" && u == seed::SELF_ID && e == "clap"));
    // production quirk, reproduced by the Worker (not by the client): "brain" is five code
    // points, above `handleFeedReaction`'s limit of four, so the server toggles "fire" instead
    let out = c.toggle_reaction(&bob, "brain");
    rig.drive(&mut c, out);
    let had_fire = rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .reactions
        .iter()
        .any(|(p, u, e)| p == "post-bob-1" && u == seed::SELF_ID && e == "fire");
    assert!(
        !had_fire,
        "the seeded own fire reaction was toggled off by a brain click"
    );
    // a picker emoji
    let out = c.toggle_reaction(&bob, "🎉");
    rig.drive(&mut c, out);
    assert!(c
        .feed
        .find(&bob)
        .unwrap()
        .reaction_keys(false)
        .contains(&"🎉".to_string()));
    // single-choice vote: the server's poll replaces the optimistic one
    let out = c.vote(&bob, &PollOptionId::parse("opt-bob-1").unwrap());
    rig.drive(&mut c, out);
    let poll = c.feed.find(&bob).unwrap().poll.clone().unwrap();
    assert!(poll.options[0].selected && poll.options[0].votes == 1);
    assert_eq!(poll.total_votes, 4);
    // comment
    c.feed
        .comment_drafts
        .insert(bob.clone(), "  great  ".into());
    let out = c.submit_comment(&bob);
    assert!(
        c.submit_comment(&bob).is_empty(),
        "no second submit while saving"
    );
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/feed/comment"), 1);
    let comments = &c.feed.find(&bob).unwrap().comments;
    assert_eq!(comments.last().unwrap().body, "great");
    assert!(c.feed.expanded_comments.contains(&bob));
    assert!(!c.feed.comment_drafts.contains_key(&bob));
    // empty comment refused locally
    c.feed.comment_drafts.insert(bob.clone(), "   ".into());
    assert!(c.submit_comment(&bob).is_empty());
    assert_eq!(c.message.as_deref(), Some("Write a comment first."));
    // edit own post, then delete it
    let own = post("post-self-1");
    c.start_edit(&own);
    c.feed.editing.as_mut().unwrap().note = "  edited note ".into();
    let out = c.save_edit();
    assert!(c.save_edit().is_empty(), "saving: no second save");
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Post updated."));
    assert_eq!(c.feed.find(&own).unwrap().note, "edited note");
    assert!(c.feed.editing.is_none());
    let out = c.delete_post(&own);
    rig.drive(&mut c, out);
    assert!(c.feed.find(&own).is_none());
    assert_eq!(
        c.message.as_deref(),
        Some("Post deleted. You can repost that session now.")
    );
    assert!(!rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .posts
        .iter()
        .any(|p| p.id == "post-self-1"));
}

#[test]
fn others_posts_are_refused_by_the_server_even_with_a_forged_local_state() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    open_feed(&mut rig, &mut c);
    let bob = post("post-bob-1");
    c.start_edit(&bob); // the UI would never offer this
    let out = c.save_edit();
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("You can only edit your own posts.")
    );
    let out = c.delete_post(&bob);
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("You can only delete your own posts.")
    );
    assert!(rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .posts
        .iter()
        .any(|p| p.id == "post-bob-1"));
}

#[test]
fn a_stale_vote_reply_never_overwrites_a_newer_vote() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    open_feed(&mut rig, &mut c);
    let bob = post("post-bob-1");
    let mut first = c.vote(&bob, &PollOptionId::parse("opt-bob-1").unwrap());
    let mut second = c.vote(&bob, &PollOptionId::parse("opt-bob-3").unwrap());
    // the server processes them in order, the replies arrive in reverse
    let a = first.pop().unwrap();
    let b = second.pop().unwrap();
    let ra = rig.transport.execute(&rig.endpoint.origin(), &a.request);
    let rb = rig.transport.execute(&rig.endpoint.origin(), &b.request);
    c.on_reply(b.token, NetReply::Http(rb), now(), &rig.env.ctx());
    let after_newest = c.feed.find(&bob).unwrap().poll.clone();
    c.on_reply(a.token, NetReply::Http(ra), now(), &rig.env.ctx());
    assert_eq!(
        c.feed.find(&bob).unwrap().poll,
        after_newest,
        "the older reply is ignored"
    );
    assert_eq!(c.feed.stale_votes, 1);
    let poll = after_newest.unwrap();
    assert!(poll.options[2].selected && !poll.options[0].selected);
}

#[test]
fn a_failed_vote_rolls_back_and_mutations_are_never_retried() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    open_feed(&mut rig, &mut c);
    let bob = post("post-bob-1");
    let before = c.feed.find(&bob).unwrap().poll.clone();
    for (path, fault, msg) in [
        (
            "/feed/poll/vote",
            Fault::Status(500, b"x".to_vec()),
            "The Social server had a problem. Try again later.",
        ),
        (
            "/feed/poll/vote",
            Fault::Status(429, b"Slow down.".to_vec()),
            "Slow down.",
        ),
        ("/feed/poll/vote", Fault::Drop, ""),
    ] {
        rig.server.fault(path, fault);
        rig.server.clear_log();
        let out = c.vote(&bob, &PollOptionId::parse("opt-bob-1").unwrap());
        rig.drive(&mut c, out);
        assert_eq!(rig.server.count(path), 1, "exactly one request, no retry");
        assert_eq!(c.feed.find(&bob).unwrap().poll, before, "rolled back");
        if !msg.is_empty() {
            assert_eq!(c.message.as_deref(), Some(msg));
        }
    }
    // a failed reaction keeps production's optimistic toggle and says so; one request only
    rig.server.fault(
        "/feed/react",
        Fault::Status(403, b"You cannot react to this feed post.".to_vec()),
    );
    rig.server.clear_log();
    let out = c.toggle_reaction(&bob, "clap");
    rig.drive(&mut c, out);
    eprintln!(
        "DBG msg={:?} log={:?} reactions={:?}",
        c.message,
        rig.server.log(),
        rig.server.world.lock().unwrap().x.reactions
    );
    assert_eq!(rig.server.count("/feed/react"), 1);
    assert_eq!(
        c.message.as_deref(),
        Some("You cannot react to this feed post.")
    );
    // a failed comment keeps the draft
    c.feed.comment_drafts.insert(bob.clone(), "keep me".into());
    rig.server
        .fault("/feed/comment", Fault::Status(500, b"x".to_vec()));
    let out = c.submit_comment(&bob);
    rig.drive(&mut c, out);
    assert_eq!(
        c.feed.comment_drafts.get(&bob).map(String::as_str),
        Some("keep me")
    );
    assert!(c.feed.comment_saving.is_none());
}

#[test]
fn posting_queues_then_a_sync_publishes_with_poll_and_image() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    open_feed(&mut rig, &mut c);
    // a poll needs two options
    c.feed.poll_draft.question = "Next?".into();
    c.feed.poll_draft.options = vec!["Series".into(), "".into()];
    let mut n = 0;
    let mut ids = || {
        n += 1;
        PollOptionId::parse(&format!("new-opt-{n}")).unwrap()
    };
    assert!(c
        .post_latest(Some(&latest()), &mut ids, now(), &rig.env.ctx())
        .is_empty());
    assert_eq!(
        c.message.as_deref(),
        Some("A poll needs a question and at least two different options.")
    );
    c.feed.poll_draft.options = vec!["Series".into(), "Integrals".into()];
    c.feed.note_draft = "one line".into();
    let out = c.post_latest(Some(&latest()), &mut ids, now(), &rig.env.ctx());
    assert!(
        out.is_empty(),
        "without an image the post only waits for the next sync"
    );
    assert_eq!(
        c.message.as_deref(),
        Some("Post queued. Sync to publish it to the feed.")
    );
    assert_eq!(c.record.as_ref().unwrap().pending_posts.len(), 1);
    assert!(c.session_posted("s1"));
    assert!(c
        .post_latest(Some(&latest()), &mut ids, now(), &rig.env.ctx())
        .is_empty());
    assert_eq!(
        c.message.as_deref(),
        Some("That session is already queued for the feed.")
    );
    // the queued post shows at the top of both feeds, as production's cache does
    assert_eq!(c.feed.rows(FeedScope::Friends)[0].id.as_str(), "s1");
    // Refresh publishes it
    let out = c.manual_sync(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert!(
        c.record.as_ref().unwrap().pending_posts.is_empty(),
        "sent posts leave the outbox"
    );
    let w = rig.server.world.lock().unwrap();
    let p = w.x.posts.iter().find(|p| p.id == "s1").unwrap();
    assert_eq!(
        (p.note.as_str(), p.subject.as_str()),
        ("one line", "Analysis")
    );
    assert_eq!(w.x.polls["s1"].options.len(), 2);
    drop(w);
    // a second session with an image: sync, then the upload
    let mut second = latest();
    second.id = "s2".into();
    c.feed.image_draft = Some(png_upload());
    let out = c.post_latest(Some(&second), &mut ids, now(), &rig.env.ctx());
    assert_eq!(c.message.as_deref(), Some("Publishing post image..."));
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Post published with image."));
    assert!(c.feed.find(&post("s2")).unwrap().image.is_some());
    assert!(rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .posts
        .iter()
        .find(|p| p.id == "s2")
        .unwrap()
        .image_key
        .is_some());
    // a refused upload says so, the post stays
    let mut third = latest();
    third.id = "s3".into();
    c.feed.image_draft = Some(png_upload());
    rig.server.fault(
        "/feed/image",
        Fault::Status(
            413,
            b"Image is too large. Use an image under 5 MB.".to_vec(),
        ),
    );
    let out = c.post_latest(Some(&third), &mut ids, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Image is too large. Use an image under 5 MB.")
    );
    assert!(rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .posts
        .iter()
        .any(|p| p.id == "s3"));
    // a session deleted from the history: its published post is queued for deletion and the
    // next sync's queued deletions remove it from the server
    c.session_removed("s1");
    assert!(c.feed.find(&post("s1")).is_none());
    assert_eq!(
        c.record.as_ref().unwrap().pending_post_deletions,
        vec![post("s1")]
    );
    let out = c.manual_sync(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert!(c.record.as_ref().unwrap().pending_post_deletions.is_empty());
    assert!(!rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .posts
        .iter()
        .any(|p| p.id == "s1"));
}

#[test]
fn auto_post_queues_completed_sessions_only_when_enabled() {
    let rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    c.auto_post(&latest());
    assert!(
        c.record.as_ref().unwrap().pending_posts.is_empty(),
        "off by default"
    );
    c.toggle_auto_post();
    c.auto_post(&latest());
    c.auto_post(&latest());
    let pending = &c.record.as_ref().unwrap().pending_posts;
    assert_eq!(pending.len(), 1, "never twice");
    assert!(study_tracker_core::social::feed::FALLBACK_NOTES.contains(&pending[0].note.as_str()));
}

#[test]
fn a_new_comment_on_an_own_post_raises_one_notice() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    open_feed(&mut rig, &mut c);
    let first = c.feed.notice.take();
    assert!(
        first.is_some(),
        "a fresh install sees the seeded comments as new"
    );
    // nothing new: no notice
    let out = c.on_feed_poll();
    rig.drive(&mut c, out);
    assert!(c.feed.notice.is_none());
    // someone comments
    {
        let mut w = rig.server.world.lock().unwrap();
        let now_ms = w.now_ms;
        w.x.comments.push(social_mock::world_22b::CommentRow {
            id: "comment-new".into(),
            post: "post-self-1".into(),
            user: "synthetic-friend-zoe".into(),
            body: "张伟: 很好".into(),
            created_at: now_ms,
        });
    }
    let out = c.on_feed_poll();
    rig.drive(&mut c, out);
    let n = c.feed.notice.clone().unwrap();
    assert_eq!(n.commenter_name, "张伟 Zoë 🦊");
    assert_eq!(c.open_notice(), Some(FeedScope::Friends));
    assert!(c.feed.expanded_comments.contains(&post("post-self-1")));
    // the seen ids are persisted (bounded), so a restart does not repeat the notice
    assert!(c
        .record
        .as_ref()
        .unwrap()
        .seen_comment_ids
        .iter()
        .any(|i| i.as_str() == "comment-new"));
}

#[test]
fn squads_without_one_suggest_search_join_and_request() {
    let mut rig = Rig::new(world_22b(false));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let out = c.set_subtab(Subtab::Squad, &app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(
        rig.server.count("/squads/search"),
        1,
        "suggestions load once"
    );
    assert!(c.current_squad().is_none());
    assert!(!c.squad.suggestions.is_empty() && c.squad.suggestions.len() <= 4);
    assert_eq!(
        c.record.as_ref().unwrap().squad_outgoing.len(),
        1,
        "the pending request to Quiet Corner"
    );
    // reload reshuffles the pool it has (no request)
    rig.server.clear_log();
    assert!(c.load_suggestions(false).is_empty());
    // search
    c.squad.search_draft = "  library ".into();
    let out = c.search_squads();
    rig.drive(&mut c, out);
    assert_eq!(c.squad.results.len(), 1);
    assert_eq!(c.squad.results[0].name, "Library Ghosts");
    // request to a private squad: pending, and production searches again
    let lib = SquadId::parse("squad-lib").unwrap();
    let out = c.join_squad(&lib);
    assert!(c.join_squad(&lib).is_empty(), "no double click while busy");
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Join request sent."));
    assert!(c.current_squad().is_none());
    assert_eq!(rig.server.count("/squads/join"), 1);
    // a full squad answers 409 with the Worker's text
    c.squad.search_draft = "full".into();
    let out = c.search_squads();
    rig.drive(&mut c, out);
    let out = c.join_squad(&SquadId::parse("squad-full").unwrap());
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("That squad is already full."));
    // join a public squad
    c.squad.search_draft = "maths".into();
    let out = c.search_squads();
    rig.drive(&mut c, out);
    let out = c.join_squad(&SquadId::parse("squad-math").unwrap());
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Joined squad."));
    assert_eq!(c.current_squad().unwrap().name, "Maths Circle");
    assert_eq!(
        c.current_squad().map(|s| s.my_role),
        Some(SquadRole::Member)
    );
}

#[test]
fn suggestions_are_not_refetched_in_a_loop_when_the_server_fails() {
    let mut rig = Rig::new(world_22b(false));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    for _ in 0..5 {
        rig.server
            .fault("/squads/search", Fault::Status(500, b"x".to_vec()));
    }
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let out = c.set_subtab(Subtab::Squad, &app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    for _ in 0..5 {
        let out = c.on_status_poll();
        rig.drive(&mut c, out);
    }
    assert_eq!(
        rig.server.count("/squads/search"),
        1,
        "one attempt per visit (production loops)"
    );
}

#[test]
fn a_squad_leader_chats_manages_members_and_leaves() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let sq = c.current_squad().unwrap().clone();
    assert_eq!(sq.name, "Night Owls");
    assert_eq!(sq.my_role, SquadRole::Leader);
    assert_eq!(
        c.squad.messages.len(),
        4,
        "the status reply carries the chat"
    );
    assert_eq!(c.record.as_ref().unwrap().squad_incoming.len(), 1);
    // chat: the draft clears at once, a second Enter sends nothing
    c.squad.chat_draft = "  see you at 9  ".into();
    let out = c.send_chat();
    assert!(c.send_chat().is_empty());
    rig.drive(&mut c, out);
    assert_eq!(c.squad.messages.last().unwrap().body, "see you at 9");
    // a refused message comes back into the field
    c.squad.chat_draft = "nope".into();
    rig.server
        .fault("/squads/chat", Fault::Status(500, b"x".to_vec()));
    let out = c.send_chat();
    assert!(c.squad.chat_draft.is_empty());
    rig.drive(&mut c, out);
    assert_eq!(c.squad.chat_draft, "nope");
    // delete own message: asked first
    let mine = c
        .squad
        .messages
        .iter()
        .rev()
        .find(|m| m.is_self)
        .unwrap()
        .id
        .clone();
    assert!(c.delete_message(&mine, false).is_empty());
    assert!(matches!(
        c.squad.confirm,
        Some(Confirm::DeleteMessage { .. })
    ));
    assert!(c.answer_confirm(false).is_empty(), "Cancel sends nothing");
    c.delete_message(&mine, false);
    let out = c.answer_confirm(true);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Message deleted."));
    // accept the join request
    let rid = c.record.as_ref().unwrap().squad_incoming[0].id.clone();
    let out = c.answer_squad_request(&rid, true);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Squad request accepted."));
    assert_eq!(c.current_squad().unwrap().member_count, 4);
    // promote, then kick (asked first)
    let zoe = UserId::parse("synthetic-friend-zoe").unwrap();
    let out = c.change_role(&zoe, SquadRole::Elder);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Squad rank updated."));
    assert!(
        c.change_role(&zoe, SquadRole::Leader).is_empty(),
        "never offered"
    );
    assert!(c.kick(&zoe, "Zoë", false).is_empty());
    assert_eq!(
        c.squad.confirm.as_ref().unwrap().text(),
        "Kick Zoë from the squad?"
    );
    let out = c.answer_confirm(true);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Member kicked."));
    // settings (leader)
    c.start_squad_settings();
    c.squad.settings_name = "  Night   Owls 2 ".into();
    c.squad.settings_private = false;
    let out = c.save_squad_settings();
    rig.drive(&mut c, out);
    assert_eq!(c.current_squad().unwrap().name, "Night Owls 2");
    assert!(!c.current_squad().unwrap().is_private);
    // leave: not the last member, no question
    let out = c.leave_squad(false);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("You left the squad."));
    assert!(c.current_squad().is_none());
    let w = rig.server.world.lock().unwrap();
    assert!(
        w.x.members
            .iter()
            .any(|m| m.squad == "squad-owl" && m.role == "leader"),
        "the next member leads"
    );
}

#[test]
fn the_last_member_is_asked_before_the_squad_is_deleted() {
    let mut rig = Rig::new(world_22b(false));
    let mut c = existing(&rig);
    c.squad.name_draft = "   ".into();
    assert!(c.create_squad().is_empty());
    assert_eq!(c.message.as_deref(), Some("Name your squad first."));
    c.squad.name_draft = "Solo".into();
    let out = c.create_squad();
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Squad created."));
    assert!(c.squad.name_draft.is_empty());
    assert!(c.leave_squad(false).is_empty());
    assert_eq!(c.squad.confirm, Some(Confirm::LeaveLast));
    let out = c.answer_confirm(true);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Squad deleted."));
    assert!(!rig
        .server
        .world
        .lock()
        .unwrap()
        .x
        .squads
        .iter()
        .any(|s| s.name == "Solo"));
    // creating while already in one: the Worker's 409
    let out = c.create_squad_named("A");
    rig.drive(&mut c, out);
    let out = c.create_squad_named("B");
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Leave your current squad before creating a new one.")
    );
}

#[test]
fn stale_local_permissions_are_refused_by_the_server() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    // the server demotes the user behind the client's back
    {
        let mut w = rig.server.world.lock().unwrap();
        for m in w.x.members.iter_mut().filter(|m| m.user == seed::SELF_ID) {
            m.role = "member".into();
        }
    }
    assert_eq!(
        c.current_squad().map(|s| s.my_role),
        Some(SquadRole::Leader),
        "the local view is stale"
    );
    let bob = UserId::parse("synthetic-friend-bob").unwrap();
    c.kick(&bob, "Bob", false);
    let out = c.answer_confirm(true);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("You cannot kick that member."));
    let out = c.change_role(&bob, SquadRole::Member);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("You cannot assign that rank."));
    c.start_squad_settings();
    let out = c.save_squad_settings();
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Only the squad leader can edit squad settings.")
    );
    assert!(!c.syncing, "the busy state ends with the refusal");
    // the next status poll brings the truth
    let out = c.on_status_poll();
    rig.drive(&mut c, out);
    assert_eq!(
        c.current_squad().map(|s| s.my_role),
        Some(SquadRole::Member)
    );
}

#[test]
fn the_squad_arena_is_cached_a_minute_and_details_open() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let out = c.set_subtab(Subtab::Leaderboard, &app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let out = c.set_scope(LeaderboardScope::Squad);
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/squads/scoreboard"), 1);
    let season = c
        .squad
        .scores
        .get(&SquadScorePeriod::Season)
        .unwrap()
        .clone();
    // points desc, then average minutes per scored member-day desc (`getSquadScoreLeaderboard`)
    assert_eq!(
        (season[0].squad_name.as_str(), season[0].points),
        ("Library Ghosts", 5)
    );
    assert_eq!(
        (
            season[1].squad_name.as_str(),
            season[1].points,
            season[1].rank
        ),
        ("Night Owls", 5, 1),
        "ties share the rank"
    );
    // leaving and returning within a minute: no new request
    let out = c.set_scope(LeaderboardScope::Friends);
    rig.drive(&mut c, out);
    let out = c.set_scope(LeaderboardScope::Squad);
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/squads/scoreboard"), 1);
    let out = c.set_squad_score_period(SquadScorePeriod::Daily);
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/squads/scoreboard"), 2);
    // the squad members' board (Squad tab) comes from /leaderboard scope=squad
    let board = c.board(LeaderboardScope::Squad, LeaderboardPeriod::Weekly);
    assert!(board
        .iter()
        .all(|e| ["Sam Synthetic", "Bob", "张伟 Zoë 🦊"].contains(&e.display_name.as_str())));
    // details
    let owl = SquadId::parse("squad-owl").unwrap();
    let out = c.open_squad_details(&owl);
    assert!(c.squad.viewing.as_ref().unwrap().loading);
    rig.drive(&mut c, out);
    let v = c.squad.viewing.clone().unwrap();
    assert_eq!(
        v.details.unwrap().action,
        study_tracker_core::social::squad::SquadAction::Current
    );
    // a late details reply for a closed dialog changes nothing
    let lib = SquadId::parse("squad-lib").unwrap();
    let out = c.open_squad_details(&lib);
    c.close_squad_details();
    rig.drive(&mut c, out);
    assert!(c.squad.viewing.is_none());
}

#[test]
fn avatar_photo_upload_replaces_the_avatar_and_syncs() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    c.open_avatar_editor();
    c.avatar_mode(2);
    let out = c.save_avatar(now(), &rig.env.ctx());
    assert!(out.is_empty());
    assert_eq!(c.message.as_deref(), Some("Choose a photo before saving."));
    c.crop_done(Ok(("me.png".into(), png_upload())));
    assert!(matches!(
        c.avatar.draft,
        Some(AvatarDraft::Photo { local: Some(_), .. })
    ));
    // a refused upload keeps the old avatar and the editor
    rig.server.fault(
        "/profile/avatar",
        Fault::Status(413, b"Avatar image is too large.".to_vec()),
    );
    let out = c.save_avatar(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Avatar image is too large."));
    assert!(c.avatar.open);
    assert!(matches!(c.profile().unwrap().avatar, Avatar::Letter { .. }));
    let out = c.save_avatar(now(), &rig.env.ctx());
    assert!(
        c.save_avatar(now(), &rig.env.ctx()).is_empty(),
        "one upload at a time"
    );
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Profile avatar saved and synced.")
    );
    assert!(!c.avatar.open);
    let url = c
        .profile()
        .unwrap()
        .avatar
        .remote_photo_url()
        .unwrap()
        .to_string();
    assert!(url.starts_with(&rig.server.url()) && url.contains("/profile/avatar/"));
    assert_eq!(rig.server.count("/sync/v2"), 1, "the new avatar is synced");
    // a letter avatar saves without an upload
    c.open_avatar_editor();
    c.avatar_mode(0);
    c.avatar_style(study_tracker_core::social::avatar::AvatarStyle::Pixel);
    c.avatar_letter("Q");
    let out = c.save_avatar(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(
        c.profile().unwrap().avatar,
        Avatar::Letter {
            letter: "Q".into(),
            style: study_tracker_core::social::avatar::AvatarStyle::Pixel
        }
    );
}

fn study(running: bool) -> TimerView {
    TimerView {
        phase: TimerPhase::Study,
        running,
    }
}

#[test]
fn verified_sessions_start_heartbeat_finish_and_persist_the_anchor() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    let out = c.observe_timer(study(true), &rig.env.ctx());
    assert_eq!(out.len(), 1);
    rig.drive(&mut c, out);
    assert!(c.bg.heartbeats_on);
    let anchor = c.record.as_ref().unwrap().verified_anchor.clone().unwrap();
    assert_eq!(anchor.confirmed_at.0, NOW);
    let out = c.verified_tick(&rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/verified-session/heartbeat"), 1);
    // break: finish
    let out = c.observe_timer(
        TimerView {
            phase: TimerPhase::Break,
            running: true,
        },
        &rig.env.ctx(),
    );
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/verified-session/finish"), 1);
    assert!(!c.bg.heartbeats_on);
    assert!(
        c.verified_tick(&rig.env.ctx()).is_empty(),
        "no heartbeat after the session ended"
    );
    // "not found" restarts once, with one schedule
    let out = c.observe_timer(study(true), &rig.env.ctx());
    rig.drive(&mut c, out);
    {
        let mut w = rig.server.world.lock().unwrap();
        for v in w.x.verified.iter_mut() {
            v.finished = true;
        }
    }
    let gen = c.bg.heartbeat_generation;
    let out = c.verified_tick(&rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/verified-session/start"), 3);
    assert!(c.bg.heartbeats_on && c.bg.heartbeat_generation > gen);
}

#[test]
fn verified_traffic_follows_the_identity_and_only_the_loopback_mock() {
    // the production endpoint and no identity: nothing, whatever the Timer does
    let mut c = SocialController::new(
        Some(SocialEndpoint::Production),
        Box::new(MemoryCredentialStore::default()),
        Box::new(MemorySocialPort::default()),
        false,
    );
    let env = Env::new();
    for running in [true, false, true] {
        assert!(c.observe_timer(study(running), &env.ctx()).is_empty());
        assert!(c.verified_tick(&env.ctx()).is_empty());
    }
    assert_eq!(c.requests_made, 0);
}

#[test]
fn a_gap_longer_than_the_grace_window_is_reconciled_once_with_the_chain_hash() {
    let mut rig = Rig::new(world_22b(true));
    // a previous anchor three hours old, for a session the mock knows
    {
        let mut w = rig.server.world.lock().unwrap();
        w.x.verified.push(social_mock::world_22b::VerifiedRow {
            id: "verified-old".into(),
            user: seed::SELF_ID.into(),
            started: NOW - 4 * 3_600_000,
            last_heartbeat: NOW - 3 * 3_600_000,
            finished: true,
            credited: 60,
        });
    }
    let mut record = synthetic_record();
    record.verified_anchor = Some(VerifiedAnchor {
        session_id: VerifiedSessionId::parse("verified-old").unwrap(),
        confirmed_at: study_tracker_core::social::SocialTimestamp(NOW - 3 * 3_600_000),
    });
    let creds = MemoryCredentialStore {
        stored: Some(StoredCredential {
            identity: synthetic_identity(),
            endpoint: EndpointClass::LocalTest,
        }),
        fail_saves: false,
    };
    let mut c = SocialController::new(
        Some(rig.endpoint.clone()),
        Box::new(creds),
        Box::new(MemorySocialPort {
            record: Some(record),
            writes: 0,
            prefs: Default::default(),
        }),
        false,
    );
    // an offline study session two hours ago is in the history
    rig.env.academic.sessions[0].started_at = WallTimestamp::from_unix_millis(NOW - 2 * 3_600_000);
    rig.env.academic.sessions[0].ended_at = WallTimestamp::from_unix_millis(NOW - 3_600_000);
    let mut out = c.observe_timer(study(true), &rig.env.ctx());
    let start = out.pop().unwrap();
    let r = rig
        .transport
        .execute(&rig.endpoint.origin(), &start.request);
    let follow = c.on_reply(start.token, NetReply::Http(r), now(), &rig.env.ctx());
    assert_eq!(follow.len(), 1, "one reconcile");
    let body: serde_json::Value = match &follow[0].request.body {
        crate::net::http::Body::Json(b) => serde_json::from_slice(b).unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(body["anchorSessionId"], "verified-old");
    // production's canonical form: `JSON.stringify` with startedAt before endedAt
    let canonical = format!(
        "[{}]",
        body["intervals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| format!(
                r#"{{"startedAt":"{}","endedAt":"{}"}}"#,
                i["startedAt"].as_str().unwrap(),
                i["endedAt"].as_str().unwrap()
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    let digest = ring::digest::digest(&ring::digest::SHA256, canonical.as_bytes());
    let hex: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        body["chainTipHash"], hex,
        "the hash covers exactly what is sent"
    );
    rig.drive(&mut c, follow);
    assert_eq!(rig.server.count("/verified-session/reconcile-offline"), 1);
    // the next confirmation is fresh: no second reconcile
    let out = c.verified_tick(&rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/verified-session/reconcile-offline"), 1);
}

#[test]
fn telemetry_is_opt_in_and_carries_no_identity() {
    let mut rig = Rig::new(world_22b(true));
    // NoIdentity: production sends telemetry without an account once the user opts in
    let mut c = none(&rig);
    assert!(c.telemetry_startup([3; 16]).is_empty(), "off by default");
    let out = c.set_telemetry(true, [3; 16]);
    assert_eq!(out.len(), 1);
    assert!(!format!("{out:?}").contains(seed::SELF_SECRET));
    rig.drive(&mut c, out);
    let hits = rig.server.world.lock().unwrap().x.telemetry.clone();
    assert_eq!(hits.len(), 1);
    assert_eq!(
        hits[0].keys,
        vec!["app".to_string(), "installId".to_string()]
    );
    // the install id is made once and kept
    let out = c.telemetry_tick([9; 16]);
    rig.drive(&mut c, out);
    let hits = rig.server.world.lock().unwrap().x.telemetry.clone();
    assert_eq!(hits[0].install_id, hits[1].install_id);
    // off: nothing more
    assert!(c.set_telemetry(false, [3; 16]).is_empty());
    assert!(c.telemetry_tick([3; 16]).is_empty());
    assert_eq!(rig.server.count("/telemetry/heartbeat"), 2);
    // with an identity, the payload still never carries the account
    let mut c = existing(&rig);
    let out = c.set_telemetry(true, [4; 16]);
    let text = format!("{:?}", out[0].request.body.len());
    let body = match &out[0].request.body {
        crate::net::http::Body::Json(b) => String::from_utf8(b.clone()).unwrap(),
        _ => unreachable!(),
    };
    assert!(
        !body.contains(seed::SELF_ID) && !body.contains(seed::SELF_SECRET),
        "{text}"
    );
}

#[test]
fn announcements_need_an_account_and_stay_dismissed() {
    let mut rig = Rig::new(world_22b(true));
    rig.server.world.lock().unwrap().x.announcements.push((
        "a-1".into(),
        "Hello".into(),
        "Body".into(),
        None,
        true,
    ));
    let mut nobody = none(&rig);
    assert!(
        nobody.poll_announcement().is_empty(),
        "D2: no account, no connection"
    );
    let mut c = existing(&rig);
    let out = c.poll_announcement();
    assert!(c.poll_announcement().is_empty(), "one in flight");
    rig.drive(&mut c, out);
    assert_eq!(c.bg.announcement.as_ref().unwrap().title, "Hello");
    c.dismiss_announcement();
    let out = c.poll_announcement();
    rig.drive(&mut c, out);
    assert!(
        c.bg.announcement.is_none(),
        "a dismissed announcement stays hidden"
    );
}

#[test]
fn owner_actions_are_shown_for_the_owner_tag_and_enforced_by_the_server() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    assert!(!c.is_owner());
    assert!(c.load_admin_usage().is_empty(), "not offered");
    // a forged local owner tag: the server still refuses
    c.record.as_mut().unwrap().profile.friend_code =
        FriendCode::parse(crate::social_controller::feed::OWNER_FRIEND_CODE).unwrap();
    assert!(c.is_owner());
    let out = c.load_admin_usage();
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Only the app owner can view usage data.")
    );
    assert!(c.bg.admin_usage.is_none());
    assert!(c.send_update_notice("0.1.", false).is_empty());
    assert_eq!(
        c.message.as_deref(),
        Some("Wait until the current app version is loaded before notifying users.")
    );
    c.send_update_notice("0.1.68", false);
    assert!(matches!(
        c.squad.confirm,
        Some(Confirm::UpdateNotice { .. })
    ));
    let out = c.answer_confirm(true);
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Only the app owner can notify users about updates.")
    );
    // the real owner (server agrees)
    rig.server
        .world
        .lock()
        .unwrap()
        .users
        .get_mut(seed::SELF_ID)
        .unwrap()
        .friend_code = social_mock::world_22b::OWNER_CODE.into();
    let out = c.load_admin_usage();
    rig.drive(&mut c, out);
    assert!(c.bg.admin_open);
    assert!(c.bg.admin_usage.as_ref().unwrap().summary.is_some());
    c.send_update_notice("0.1.68", false);
    let out = c.answer_confirm(true);
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("Update notice sent to users below 0.1.68.")
    );
    // the owner sees R2 usage with the feed
    open_feed(&mut rig, &mut c);
    assert!(c.feed.r2.is_some());
}

#[test]
fn every_22b_route_keeps_the_secret_out_of_messages_and_debug_output() {
    let mut rig = Rig::new(world_22b(true));
    let mut c = existing(&rig);
    let mut seen = String::new();
    let mut run = |c: &mut SocialController, rig: &mut Rig, out: Vec<Outgoing>| {
        seen.push_str(&format!("{out:?}"));
        rig.drive(c, out);
        seen.push_str(&format!("{:?}", c.message));
    };
    open_feed(&mut rig, &mut c);
    let bob = post("post-bob-1");
    let o = c.toggle_reaction(&bob, "fire");
    run(&mut c, &mut rig, o);
    let o = c.vote(&bob, &PollOptionId::parse("opt-bob-2").unwrap());
    run(&mut c, &mut rig, o);
    c.feed.comment_drafts.insert(bob.clone(), "x".into());
    let o = c.submit_comment(&bob);
    run(&mut c, &mut rig, o);
    let o = c.observe_timer(study(true), &rig.env.ctx());
    run(&mut c, &mut rig, o);
    let o = c.verified_tick(&rig.env.ctx());
    run(&mut c, &mut rig, o);
    let o = c.poll_announcement();
    run(&mut c, &mut rig, o);
    c.squad.chat_draft = "hi".into();
    let o = c.send_chat();
    run(&mut c, &mut rig, o);
    for fault in ["/feed", "/squads/chat"] {
        rig.server.fault(
            fault,
            Fault::Status(
                403,
                format!("Invalid device secret. {}", seed::SELF_SECRET).into_bytes(),
            ),
        );
    }
    let o = c.refresh_feed();
    run(&mut c, &mut rig, o);
    c.squad.chat_draft = "hi".into();
    let o = c.send_chat();
    run(&mut c, &mut rig, o);
    assert!(!seen.contains(seed::SELF_SECRET), "{seen}");
    assert!(!format!("{:?}", c.record).contains(seed::SELF_SECRET));
}

impl SocialController {
    /// Test helper: create a squad with a name in one step.
    fn create_squad_named(&mut self, name: &str) -> Vec<Outgoing> {
        self.squad.name_draft = name.into();
        self.create_squad()
    }
}
