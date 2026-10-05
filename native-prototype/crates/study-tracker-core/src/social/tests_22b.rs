//! Stage 22b domain tests: feed, squads, verified sessions, telemetry ids, announcements.

use std::collections::BTreeSet;

use super::announcement::{is_valid_app_version, Dismissed, MAX_DISMISSED};
use super::feed::*;
use super::ids::{display_paragraph, truncate_utf16, utf16_len};
use super::squad::*;
use super::telemetry::InstallId;
use super::verified::*;
use super::*;
use crate::timer::{ActiveSegment, TimerPhase, WallTimestamp};

fn uid(s: &str) -> UserId {
    UserId::parse(s).unwrap()
}

fn opt(id: &str, votes: u64, selected: bool) -> PollOption {
    PollOption {
        id: PollOptionId::parse(id).unwrap(),
        text: id.to_uppercase(),
        votes,
        selected,
    }
}

fn poll(multiple: bool, options: Vec<PollOption>) -> Poll {
    let total_votes = options.iter().map(|o| o.votes).sum();
    Poll {
        question: "Q?".into(),
        multiple,
        options,
        total_votes,
    }
}

fn post(id: &str, user: &str) -> FeedPost {
    FeedPost {
        id: PostId::parse(id).unwrap(),
        user_id: uid(user),
        display_name: "Ana".into(),
        friend_code: None,
        avatar: Avatar::default_for("Ana"),
        kind: PostKind::Session,
        subject: "Math".into(),
        detail: "1h · Focus".into(),
        note: "n".into(),
        icon: "✦".into(),
        minutes: 60,
        preset_label: "Focus".into(),
        created_at: None,
        is_self: false,
        image: None,
        image_expired: false,
        poll: None,
        reactions: BASE_REACTIONS.iter().map(|k| (k.to_string(), 0)).collect(),
        reacted: vec![],
        reacted_by: vec![],
        comments: vec![],
    }
}

fn pid(s: &str) -> PollOptionId {
    PollOptionId::parse(s).unwrap()
}

#[test]
fn single_choice_vote_moves_the_selection() {
    let p = poll(false, vec![opt("a", 2, true), opt("b", 1, false)]);
    let v = p.optimistic_vote(&pid("b")).unwrap();
    assert_eq!(
        v.options
            .iter()
            .map(|o| (o.votes, o.selected))
            .collect::<Vec<_>>(),
        vec![(1, false), (2, true)]
    );
    assert_eq!(v.total_votes, 3);
    // clicking the selected option removes the vote
    let v2 = v.optimistic_vote(&pid("b")).unwrap();
    assert_eq!(v2.options[1].votes, 1);
    assert!(!v2.options[1].selected);
    assert_eq!(v2.total_votes, 2);
    assert!(p.optimistic_vote(&pid("zz")).is_none());
}

#[test]
fn multiple_choice_vote_toggles_independently() {
    let p = poll(true, vec![opt("a", 1, true), opt("b", 0, false)]);
    let v = p.optimistic_vote(&pid("b")).unwrap();
    assert!(v.options[0].selected && v.options[1].selected);
    assert_eq!(v.total_votes, 2);
    assert_eq!(v.percent(&v.options[0]), 50);
    // never below zero, even from a stale count
    let stale = poll(true, vec![opt("a", 0, true)]);
    assert_eq!(
        stale.optimistic_vote(&pid("a")).unwrap().options[0].votes,
        0
    );
    assert_eq!(
        poll(false, vec![opt("a", 0, false)]).percent(&opt("a", 0, false)),
        0
    );
    // Math.round(2/3*100) = 67, Math.round(1/3*100) = 33
    let thirds = poll(false, vec![opt("a", 2, false), opt("b", 1, false)]);
    assert_eq!(thirds.percent(&thirds.options[0]), 67);
    assert_eq!(thirds.percent(&thirds.options[1]), 33);
}

#[test]
fn reaction_toggle_and_button_order() {
    let mut p = post("p1", "u1");
    p.toggle_reaction("fire", "Me");
    assert_eq!(p.count("fire"), 1);
    assert!(p.has_reacted("fire"));
    assert_eq!(p.names("fire"), ["Me".to_string()]);
    p.toggle_reaction("🎉", "Me");
    assert_eq!(p.reaction_keys(false), vec!["fire", "brain", "clap", "🎉"]);
    assert_eq!(p.reaction_keys(true), vec!["fire"]);
    // second click removes
    p.toggle_reaction("🎉", "Me");
    assert_eq!(p.count("🎉"), 0);
    assert_eq!(p.reaction_keys(false), vec!["fire", "brain", "clap"]);
    assert!(p.names("🎉").is_empty());
    // a stale zero never goes negative
    let mut q = post("p2", "u1");
    q.reacted.push("brain".into());
    q.toggle_reaction("brain", "Me");
    assert_eq!(q.count("brain"), 0);
    assert_eq!(reaction_glyph("fire", false), "🔥");
    assert_eq!(reaction_glyph("fire", true), "Nod");
    assert_eq!(reaction_glyph("🎯", false), "🎯");
    assert_eq!(
        reactors_label(&["a".into(), "b".into(), "c".into(), "d".into(), "e".into()]),
        "a, b, c +2 more"
    );
    assert_eq!(reactors_label(&["a".into(), "b".into()]), "a, b");
    assert!(is_known_reaction("fire") && is_known_reaction("❤️") && !is_known_reaction("<b>"));
}

#[test]
fn poll_drafts_follow_prepare_feed_poll_draft() {
    let mut n = 0;
    let mut next = || {
        n += 1;
        PollOptionId::parse(&format!("o{n}")).unwrap()
    };
    // nothing typed: no poll
    assert_eq!(prepare_poll(&PollDraft::default(), &mut next), Ok(None));
    // a question without two options is refused
    let mut d = PollDraft {
        question: "  Which   one? ".into(),
        options: vec!["A".into(), " a ".into(), "".into()],
        ..Default::default()
    };
    assert_eq!(prepare_poll(&d, &mut next), Err(PollDraftError));
    // whitespace collapsed, duplicates (case-insensitive) and empties dropped
    d.options = vec![
        "Red\tpen".into(),
        "red PEN".into(),
        "Blue".into(),
        "  ".into(),
    ];
    let p = prepare_poll(&d, &mut next).unwrap().unwrap();
    assert_eq!(p.question, "Which one?");
    assert_eq!(
        p.options
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>(),
        vec!["Red pen", "Blue"]
    );
    // max 12 options, lengths cut to 180/100 UTF-16 units
    let big = PollDraft {
        question: "q".repeat(400),
        options: (0..20).map(|i| format!("{i}{}", "x".repeat(150))).collect(),
        ..Default::default()
    };
    let p = prepare_poll(&big, &mut next).unwrap().unwrap();
    assert_eq!(p.options.len(), MAX_POLL_OPTIONS);
    assert_eq!(utf16_len(&p.question), 180);
    assert!(p.options.iter().all(|(_, t)| utf16_len(t) == 100));
    // the editor keeps 2..=12 rows
    let mut e = PollDraft::default();
    e.remove_option(0);
    assert_eq!(e.options.len(), 2);
    for _ in 0..20 {
        e.add_option();
    }
    assert_eq!(e.options.len(), MAX_POLL_OPTIONS);
    assert!(!PollDraft::default().has_content());
}

#[test]
fn session_posts_follow_build_feed_post_from_session() {
    let ended = SocialTimestamp(1_759_658_400_000);
    let facts = SessionFacts {
        id: "sess-1",
        exam: false,
        goal: "",
        minutes: 50,
        preset_label: "",
        ended_at: ended,
    };
    let p = build_session_post(&facts, "", "  ", "50m", None).unwrap();
    assert_eq!(p.subject, "Study session");
    assert_eq!(p.detail, "50m · Focus");
    assert_eq!(p.icon, "✦");
    assert_eq!(p.note, fallback_note("sess-1"));
    assert!(FALLBACK_NOTES.contains(&p.note.as_str()));
    let exam = SessionFacts {
        exam: true,
        goal: "Chapter 4",
        ..facts
    };
    let p = build_session_post(&exam, "", "great", "50m", None).unwrap();
    assert_eq!(
        (p.subject.as_str(), p.icon.as_str(), p.note.as_str()),
        ("Chapter 4", "⚔", "great")
    );
    assert_eq!(p.detail, "50m · Exam");
    let p = build_session_post(&exam, "Analysis", "", "50m", None).unwrap();
    assert_eq!(p.subject, "Analysis");
    let row = p.as_feed_post(
        &uid("me"),
        "Me",
        &FriendCode::parse("ABCD-2345").unwrap(),
        &Avatar::default_for("Me"),
    );
    assert!(row.is_self && row.count("fire") == 0 && row.reactions.len() == 3);
    assert!(already_posted(&p.id, std::slice::from_ref(&p), &[]));
    assert!(already_posted(&p.id, &[], &[&[row]]));
    // the fallback pick is the code-unit sum mod 28 (production's `pickFeedFallbackNote`)
    let sum: u64 = "abc".chars().map(|c| c as u64).sum();
    assert_eq!(fallback_note("abc"), FALLBACK_NOTES[(sum % 28) as usize]);
}

#[test]
fn comment_notes_and_paragraphs() {
    assert_eq!(clean_comment("   "), None);
    assert_eq!(clean_comment("  hi  ").as_deref(), Some("hi"));
    let long = "é".repeat(300);
    assert_eq!(utf16_len(&clean_comment(&long).unwrap()), 220);
    // an emoji never splits
    let emoji = "😀".repeat(200); // 2 units each
    assert_eq!(utf16_len(&clean_comment(&emoji).unwrap()), 220);
    assert_eq!(utf16_len(&truncate_utf16("a😀", 2)), 1);
    assert_eq!(clean_note(" x\n "), "x");
    // paragraphs: newlines/tabs as spaces, runs collapsed, bidi overrides and controls gone,
    // markup kept as text
    assert_eq!(
        display_paragraph("line1\r\nline2\t\tend\u{202E}x\u{0007}", 100),
        "line1 line2 endx"
    );
    assert_eq!(display_paragraph("<b>bold</b>", 100), "<b>bold</b>");
    assert_eq!(display_paragraph("   ", 100), "");
    assert_eq!(display_paragraph(&"w".repeat(50), 10).len(), 10);
    assert_eq!(
        notice_excerpt(&"x".repeat(80)),
        format!("{}...", "x".repeat(72))
    );
    assert_eq!(notice_excerpt("short"), "short");
}

#[test]
fn comment_notice_only_for_new_comments_by_others_on_own_posts() {
    let me = uid("me");
    let mk = |id: &str, by: &str, is_self: bool| Comment {
        id: CommentId::parse(id).unwrap(),
        post_id: PostId::parse("own").unwrap(),
        user_id: uid(by),
        display_name: by.into(),
        friend_code: None,
        avatar: Avatar::default_for(by),
        body: format!("hi from {by}"),
        created_at: None,
        is_self,
    };
    let mut own = post("own", "me");
    own.comments = vec![mk("c1", "me", true), mk("c2", "bob", false)];
    let mut other = post("theirs", "bob");
    other.comments = vec![mk("c3", "kim", false)];
    let mut seen = BTreeSet::new();
    seen.insert(CommentId::parse("c2").unwrap());
    let feed = [own.clone(), other];
    assert_eq!(
        new_comment_notice(&feed, FeedScope::Global, &me, &mut seen),
        None
    );
    assert_eq!(seen.len(), 3);
    own.comments.push(mk("c4", "zoe", false));
    let n = new_comment_notice(&[own], FeedScope::Friends, &me, &mut seen).unwrap();
    assert_eq!(
        (n.commenter_name.as_str(), n.scope),
        ("zoe", FeedScope::Friends)
    );
}

#[test]
fn squad_permissions_match_client_and_server_rules() {
    use SquadRole::*;
    // getAssignableSquadRoles
    assert_eq!(
        assignable_roles(Leader, Member),
        vec![CoLeader, Elder, Member]
    );
    assert_eq!(
        assignable_roles(Leader, CoLeader),
        vec![CoLeader, Elder, Member]
    );
    assert!(assignable_roles(Leader, Leader).is_empty());
    assert_eq!(assignable_roles(CoLeader, Elder), vec![Elder, Member]);
    assert!(assignable_roles(CoLeader, CoLeader).is_empty());
    assert!(assignable_roles(Elder, Member).is_empty());
    assert!(assignable_roles(Member, Member).is_empty());
    // kick
    assert!(can_kick(Leader, CoLeader) && !can_kick(Leader, Leader));
    assert!(can_kick(CoLeader, Elder) && !can_kick(CoLeader, CoLeader));
    assert!(can_kick(Elder, Member) && !can_kick(Elder, Elder));
    assert!(!can_kick(Member, Member));
    // join requests
    assert!(can_manage_requests(Some(Elder)) && !can_manage_requests(Some(Member)));
    assert!(!can_manage_requests(None));
    // every role the client offers is one the server accepts
    for actor in SquadRole::ALL {
        for target in SquadRole::ALL {
            for next in assignable_roles(actor, target) {
                assert!(
                    server_can_change_role(actor, target, next),
                    "{actor:?} {target:?} {next:?}"
                );
            }
        }
    }
    // unknown roles and actions fail safe
    assert_eq!(SquadRole::parse("admin"), Member);
    assert_eq!(SquadRole::parse("co_leader").label(), "Co-leader");
    assert_eq!(SquadAction::parse("weird"), SquadAction::Unavailable);
    assert!(!SquadAction::parse("weird").can_join_or_request());
    assert_eq!(SquadAction::Join.details_badge(), "Open to join");
    assert_eq!(clean_squad_name("   "), None);
    assert_eq!(utf16_len(&clean_squad_name(&"n".repeat(99)).unwrap()), 48);
    assert_eq!(clean_message(" \n "), None);
    assert_eq!(utf16_len(&clean_message(&"m".repeat(900)).unwrap()), 500);
}

#[test]
fn suggestions_are_a_shuffle_of_at_most_four() {
    let pool: Vec<SquadSearchResult> = (0..7)
        .map(|i| SquadSearchResult {
            id: SquadId::parse(&format!("s{i}")).unwrap(),
            name: format!("S{i}"),
            is_private: false,
            member_count: 1,
            max_members: 4,
            total_minutes: 0,
            total_sessions: 0,
            action: SquadAction::Join,
        })
        .collect();
    let picked = pick_suggestions(&pool, |i| i); // identity: no swaps
    assert_eq!(picked.len(), 4);
    assert_eq!(picked[0].name, "S0");
    let picked = pick_suggestions(&pool, |_| 0);
    assert_eq!(picked.len(), 4);
    assert_ne!(picked[0].name, "S0");
    assert!(pick_suggestions(&pool[..2], |_| 0).len() == 2);
}

fn sid(s: &str) -> VerifiedSessionId {
    VerifiedSessionId::parse(s).unwrap()
}

#[test]
fn verified_machine_follows_the_production_effect() {
    let mut m = VerifiedMachine::new(None);
    // not eligible at first: nothing but the (no-op) interval clear
    assert_eq!(m.observe(false), vec![Intent::StopHeartbeats]);
    assert!(!eligible(TimerPhase::Study, false));
    assert!(!eligible(TimerPhase::Break, true));
    assert!(!eligible(TimerPhase::Idle, true));
    assert!(eligible(TimerPhase::Stopwatch, true) && eligible(TimerPhase::Exam, true));
    // start
    let out = m.observe(true);
    let gen = m.generation();
    assert_eq!(
        out,
        vec![Intent::StopHeartbeats, Intent::Start { generation: gen }]
    );
    // a heartbeat tick while starting does nothing
    assert!(m.heartbeat_due().is_empty());
    let out = m.start_succeeded(gen, sid("s1"), SocialTimestamp(1_000));
    assert_eq!(
        out,
        vec![
            Intent::SaveAnchor(VerifiedAnchor {
                session_id: sid("s1"),
                confirmed_at: SocialTimestamp(1_000)
            }),
            Intent::ScheduleHeartbeats
        ]
    );
    assert_eq!(
        m.heartbeat_due(),
        vec![Intent::Heartbeat {
            session_id: sid("s1")
        }]
    );
    // a phase change that stays eligible (study -> exam) reschedules, no new start
    assert_eq!(
        m.observe(true),
        vec![Intent::StopHeartbeats, Intent::ScheduleHeartbeats]
    );
    // pause: finish
    assert_eq!(
        m.observe(false),
        vec![
            Intent::StopHeartbeats,
            Intent::Finish {
                session_id: sid("s1")
            }
        ]
    );
    assert!(m.heartbeat_due().is_empty());
    assert_eq!(m.finishes, 1);
}

#[test]
fn a_late_start_is_finished_as_an_orphan() {
    let mut m = VerifiedMachine::new(None);
    m.observe(true);
    let g1 = m.generation();
    // the timer is paused before the start answered
    m.observe(false);
    assert_eq!(
        m.start_succeeded(g1, sid("late"), SocialTimestamp(5)),
        vec![Intent::Finish {
            session_id: sid("late")
        }]
    );
    assert!(m.session().is_none());
    // a superseded generation while still eligible is also finished (production's `disposed`)
    m.observe(true);
    let g2 = m.generation();
    m.observe(true); // e.g. study -> exam before the start landed: a second start goes out
    let g3 = m.generation();
    assert_eq!(
        m.start_succeeded(g2, sid("a"), SocialTimestamp(6)),
        vec![Intent::Finish {
            session_id: sid("a")
        }]
    );
    assert!(matches!(
        m.start_succeeded(g3, sid("b"), SocialTimestamp(7))[0],
        Intent::SaveAnchor(_)
    ));
    assert_eq!(m.session(), Some(&sid("b")));
}

#[test]
fn not_found_restarts_with_exactly_one_schedule() {
    let mut m = VerifiedMachine::new(None);
    m.observe(true);
    let g = m.generation();
    m.start_succeeded(g, sid("s1"), SocialTimestamp(0));
    // another failure is ignored
    assert!(m.heartbeat_failed(&sid("s1"), Failure::Other).is_empty());
    assert!(m.heartbeat_failed(&sid("s1"), Failure::Network).is_empty());
    assert_eq!(m.session(), Some(&sid("s1")));
    // not found: forget, stop the old schedule, start again
    let out = m.heartbeat_failed(&sid("s1"), Failure::NotFound);
    assert_eq!(
        out,
        vec![Intent::StopHeartbeats, Intent::Start { generation: g }]
    );
    // a stale not-found for an older session does nothing
    assert!(m
        .heartbeat_failed(&sid("old"), Failure::NotFound)
        .is_empty());
    let out = m.start_succeeded(g, sid("s2"), SocialTimestamp(10));
    assert_eq!(out.last(), Some(&Intent::ScheduleHeartbeats));
}

#[test]
fn offline_start_is_retried_on_the_cadence_or_on_reconnect() {
    let mut m = VerifiedMachine::new(None);
    m.observe(true);
    let g = m.generation();
    m.start_failed(g, Failure::Other);
    assert!(
        m.heartbeat_due().is_empty(),
        "a server refusal is not retried"
    );
    m.observe(false);
    m.observe(true);
    let g = m.generation();
    m.start_failed(g, Failure::Network);
    assert_eq!(m.heartbeat_due(), vec![Intent::Start { generation: g }]);
    assert!(m.heartbeat_due().is_empty(), "never two starts in flight");
    m.start_failed(g, Failure::Network);
    assert_eq!(
        m.connectivity_restored(),
        vec![Intent::Start { generation: g }]
    );
    m.start_succeeded(g, sid("s"), SocialTimestamp(1));
    assert_eq!(
        m.connectivity_restored(),
        vec![Intent::Heartbeat {
            session_id: sid("s")
        }]
    );
    m.observe(false);
    assert!(m.connectivity_restored().is_empty());
}

#[test]
fn anchors_reconcile_only_after_the_grace_window_and_one_at_a_time() {
    let anchor = VerifiedAnchor {
        session_id: sid("prev"),
        confirmed_at: SocialTimestamp(0),
    };
    let mut m = VerifiedMachine::new(Some(anchor));
    m.observe(true);
    let g = m.generation();
    // exactly at the grace boundary: no reconcile (`staleMs <= GRACE` returns)
    let out = m.start_succeeded(g, sid("s"), SocialTimestamp(NORMAL_CREDIT_GRACE_MS));
    assert!(!out.iter().any(|i| matches!(i, Intent::Reconcile { .. })));
    // more than the grace later: reconcile from the previous anchor
    let later = SocialTimestamp(NORMAL_CREDIT_GRACE_MS * 3);
    let out = m.heartbeat_succeeded(sid("s"), later);
    assert!(out.contains(&Intent::Reconcile {
        anchor_session_id: sid("s"),
        since: SocialTimestamp(NORMAL_CREDIT_GRACE_MS),
        until: later
    }));
    // while it runs, another stale confirmation does not send a second one
    let out = m.heartbeat_succeeded(sid("s"), SocialTimestamp(NORMAL_CREDIT_GRACE_MS * 6));
    assert!(!out.iter().any(|i| matches!(i, Intent::Reconcile { .. })));
    m.reconcile_finished();
    assert_eq!(m.reconciles, 1);
    assert_eq!(
        capped_message(1).as_deref(),
        Some("1 offline minute could not be verified.")
    );
    assert_eq!(
        capped_message(3).as_deref(),
        Some("3 offline minutes could not be verified.")
    );
    assert_eq!(capped_message(0), None);
}

#[test]
fn offline_intervals_and_the_canonical_hash_input() {
    let w = WallTimestamp::from_unix_millis;
    let history = [
        HistorySession {
            study_or_exam: true,
            started_at: w(0),
            ended_at: w(1_000),
        },
        HistorySession {
            study_or_exam: true,
            started_at: w(5_000),
            ended_at: w(9_000),
        },
        HistorySession {
            study_or_exam: false,
            started_at: w(5_000),
            ended_at: w(9_000),
        },
    ];
    let segs = [
        ActiveSegment {
            started_at: w(10_000),
            ended_at: Some(w(11_000)),
        },
        ActiveSegment {
            started_at: w(12_000),
            ended_at: None,
        },
    ];
    let iv = offline_intervals(
        &history,
        TimerPhase::Study,
        &segs,
        SocialTimestamp(2_000),
        SocialTimestamp(20_000),
    );
    assert_eq!(
        iv.iter()
            .map(|i| (i.started_at.0, i.ended_at.0))
            .collect::<Vec<_>>(),
        vec![(5_000, 9_000), (10_000, 11_000), (12_000, 20_000)]
    );
    // a break phase contributes no live segment
    assert_eq!(
        offline_intervals(
            &history,
            TimerPhase::Break,
            &segs,
            SocialTimestamp(2_000),
            SocialTimestamp(20_000)
        )
        .len(),
        1
    );
    assert_eq!(
        canonical_intervals(&iv[..1]),
        "[{\"startedAt\":\"1970-01-01T00:00:05.000Z\",\"endedAt\":\"1970-01-01T00:00:09.000Z\"}]"
    );
    assert_eq!(canonical_intervals(&[]), "[]");
}

#[test]
fn telemetry_install_ids_and_announcements() {
    let id = InstallId::from_random([0xff; 16]);
    assert_eq!(id.as_str().len(), 36);
    assert_eq!(&id.as_str()[14..15], "4");
    assert!(InstallId::parse(id.as_str()).is_some());
    assert!(InstallId::parse("bad id").is_none());
    assert!(InstallId::parse(&"a".repeat(81)).is_none());
    let mut d = Dismissed::default();
    for i in 0..150 {
        d.add(&format!("a{i}"));
    }
    assert_eq!(d.ids().len(), MAX_DISMISSED);
    assert!(!d.contains("a0") && d.contains("a149"));
    d.add("a149");
    assert_eq!(d.ids().len(), MAX_DISMISSED);
    assert!(is_valid_app_version("0.1.68") && is_valid_app_version(" 2 "));
    assert!(
        !is_valid_app_version("0.1.") && !is_valid_app_version("v1") && !is_valid_app_version("")
    );
}
