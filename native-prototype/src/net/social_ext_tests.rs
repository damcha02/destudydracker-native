//! Stage 22b wire tests: request shapes (exactly what production sends) and the validation of
//! every reply (bounded, sanitised, fail-safe on malformed or hostile data).

use super::*;
use study_tracker_core::social::feed::{FeedScope, NewPoll, PendingPost, PostKind};
use study_tracker_core::social::squad::{SquadAction, SquadRole, SquadScorePeriod};
use study_tracker_core::social::{DeviceSecret, SocialIdentity, SocialTimestamp, UserId};

const SECRET: &str = "TEST_SECRET_MUST_NOT_APPEAR";

fn id() -> SocialIdentity {
    SocialIdentity {
        user_id: UserId::parse("synthetic-user-0001").unwrap(),
        device_secret: DeviceSecret::parse(SECRET).unwrap(),
    }
}

fn ok(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: Some("application/json".into()),
        body: body.as_bytes().to_vec(),
    }
}

fn json_of(req: &ApiRequest) -> serde_json::Value {
    match &req.body {
        Body::Json(b) => serde_json::from_slice(b).unwrap(),
        _ => panic!("not JSON"),
    }
}

#[test]
fn request_bodies_are_productions() {
    let i = id();
    let post = PostId::parse("sess-1").unwrap();
    let b = json_of(&feed(&i, FeedScope::Friends));
    assert_eq!(
        b,
        serde_json::json!({"scope": "friends", "userId": "synthetic-user-0001", "deviceSecret": SECRET})
    );
    let b = json_of(&react(&i, &post, "🔥"));
    assert_eq!(b["postId"], "sess-1");
    assert_eq!(b["emoji"], "🔥");
    let b = json_of(&poll_vote(&i, &post, &PollOptionId::parse("o1").unwrap()));
    assert_eq!(b["optionId"], "o1");
    let b = json_of(&comment_create(&i, &post, "hi"));
    assert_eq!(
        (b["postId"].as_str(), b["body"].as_str()),
        (Some("sess-1"), Some("hi"))
    );
    assert_eq!(json_of(&post_update(&i, &post, "n"))["note"], "n");
    assert_eq!(json_of(&post_delete(&i, &post))["postId"], "sess-1");
    assert_eq!(json_of(&post_image_delete(&i, &post))["postId"], "sess-1");
    let b = json_of(&squad_create(&i, "Night Owls", true));
    assert_eq!(
        (b["name"].as_str(), b["isPrivate"].as_bool()),
        (Some("Night Owls"), Some(true))
    );
    assert_eq!(json_of(&squad_search(&i, ""))["query"], "");
    let s = SquadId::parse("squad-1").unwrap();
    assert_eq!(json_of(&squad_details(&i, &s))["squadId"], "squad-1");
    assert_eq!(json_of(&squad_join(&i, &s))["squadId"], "squad-1");
    let b = json_of(&squad_respond(&i, &RequestId::parse("r1").unwrap(), false));
    assert_eq!(
        (b["requestId"].as_str(), b["response"].as_str()),
        (Some("r1"), Some("declined"))
    );
    assert_eq!(
        json_of(&squad_leave(&i)),
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET})
    );
    assert_eq!(json_of(&squad_chat(&i, "yo"))["body"], "yo");
    assert_eq!(
        json_of(&squad_chat_delete(&i, &MessageId::parse("m1").unwrap()))["messageId"],
        "m1"
    );
    let t = UserId::parse("u2").unwrap();
    let b = json_of(&squad_role(&i, &t, SquadRole::CoLeader));
    assert_eq!(
        (b["targetUserId"].as_str(), b["role"].as_str()),
        (Some("u2"), Some("co_leader"))
    );
    assert_eq!(json_of(&squad_kick(&i, &t))["targetUserId"], "u2");
    assert_eq!(
        json_of(&squad_scoreboard(&i, SquadScorePeriod::Season))["period"],
        "season"
    );
    assert_eq!(json_of(&verified_start(&i)).as_object().unwrap().len(), 2);
    let v = VerifiedSessionId::parse("v1").unwrap();
    assert_eq!(json_of(&verified_heartbeat(&i, &v))["sessionId"], "v1");
    assert_eq!(json_of(&verified_finish(&i, &v))["sessionId"], "v1");
    let iv = [Interval {
        started_at: SocialTimestamp(0),
        ended_at: SocialTimestamp(60_000),
    }];
    let b = json_of(&verified_reconcile(&i, &v, &iv, "abc"));
    assert_eq!(b["anchorSessionId"], "v1");
    assert_eq!(b["chainTipHash"], "abc");
    assert_eq!(
        b["intervals"],
        serde_json::json!([{"startedAt": "1970-01-01T00:00:00.000Z", "endedAt": "1970-01-01T00:01:00.000Z"}])
    );
    assert_eq!(
        json_of(&update_notice(&i, "0.1.68"))["targetVersion"],
        "0.1.68"
    );
    assert_eq!(json_of(&admin_usage(&i)).as_object().unwrap().len(), 2);
}

#[test]
fn telemetry_carries_only_the_install_id_and_app_metadata() {
    let app = AppMetadata {
        version: "0.1.68".into(),
        platform: "linux".into(),
        runtime_channel: "development".into(),
    };
    let install = InstallId::from_random([1; 16]);
    let req = telemetry_heartbeat(&install, &app);
    let b = json_of(&req);
    let keys: Vec<&String> = b.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["app", "installId"]);
    assert_eq!(
        b["app"],
        serde_json::json!({"version": "0.1.68", "platform": "linux", "runtimeChannel": "development"})
    );
    let text = String::from_utf8(match &req.body {
        Body::Json(b) => b.clone(),
        _ => unreachable!(),
    })
    .unwrap();
    assert!(
        !text.contains("userId") && !text.contains("deviceSecret") && !text.contains("fingerprint")
    );
    // the announcement read is anonymous: a query, no body, no credential
    let a = announcement_current(&app);
    assert!(matches!(a.method, Method::Get) && a.body.len() == 0);
    assert_eq!(a.query, vec![("appVersion", "0.1.68".to_string())]);
}

#[test]
fn uploads_are_multipart_with_productions_fields_and_local_size_caps() {
    let i = id();
    let post = PostId::parse("sess-1").unwrap();
    let png = Upload {
        bytes: vec![0x89, b'P', b'N', b'G', 1, 2, 3],
        mime: "image/png",
    };
    let req = post_image_upload(&i, &post, &png).unwrap();
    let Body::Multipart {
        content_type,
        bytes,
    } = &req.body
    else {
        panic!()
    };
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let text = String::from_utf8_lossy(bytes);
    for field in [
        "name=\"userId\"",
        "name=\"deviceSecret\"",
        "name=\"postId\"",
        "name=\"image\"; filename=\"feed-image.png\"",
        "Content-Type: image/png",
    ] {
        assert!(text.contains(field), "{field}");
    }
    // the Debug form never shows the body
    assert!(!format!("{req:?}").contains(SECRET));
    let big = Upload {
        bytes: vec![0; MAX_FEED_UPLOAD_BYTES + 1],
        mime: "image/jpeg",
    };
    assert!(matches!(
        post_image_upload(&i, &post, &big),
        Err(NetError::PayloadTooLarge(_))
    ));
    let req = avatar_upload(&i, &png, "me.png").unwrap();
    let Body::Multipart { bytes, .. } = &req.body else {
        panic!()
    };
    let text = String::from_utf8_lossy(bytes);
    assert!(text.contains("name=\"name\"") && text.contains("filename=\"me.png\""));
    let big = Upload {
        bytes: vec![0; MAX_AVATAR_UPLOAD_BYTES + 1],
        mime: "image/png",
    };
    assert!(matches!(
        avatar_upload(&i, &big, ""),
        Err(NetError::PayloadTooLarge(_))
    ));
}

#[test]
fn queued_posts_travel_in_sync_v2_like_production() {
    let p = PendingPost {
        id: PostId::parse("sess-1").unwrap(),
        kind: PostKind::Session,
        subject: "Math".into(),
        detail: "50m · Focus".into(),
        note: "x".into(),
        icon: "✦".into(),
        minutes: 50,
        preset_label: String::new(),
        created_at: SocialTimestamp(1_000),
        poll: Some(NewPoll {
            question: "Q?".into(),
            multiple: true,
            options: vec![
                (PollOptionId::parse("a").unwrap(), "A".into()),
                (PollOptionId::parse("b").unwrap(), "B".into()),
            ],
        }),
    };
    let wire = serde_json::to_value(FeedPostWire::from(&p)).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({
            "id": "sess-1", "type": "session", "subject": "Math", "detail": "50m · Focus",
            "note": "x", "icon": "✦", "minutes": 50, "presetLabel": "", "createdAt": "1970-01-01T00:00:01.000Z",
            "poll": {"question": "Q?", "multiple": true, "options": [{"id": "a", "text": "A"}, {"id": "b", "text": "B"}]}
        })
    );
}

const POST: &str = r#"{"id":"p1","userId":"u1","displayName":"Ana","friendCode":"ANAA-2345",
 "avatar":{"kind":"icon","icon":"🦊"},"type":"session","subject":"Math","detail":"1h · Focus",
 "note":"line1\nline2","icon":"✦","minutes":60,"presetLabel":"Focus","createdAt":"2026-10-04T10:00:00.000Z",
 "isSelf":false,"imageUrl":"http://127.0.0.1:1/feed/image/k","imageMimeType":"image/png","imageExpiresAt":"2026-10-09T10:00:00.000Z",
 "imageExpiredAt":null,
 "poll":{"question":"Q?","multiple":false,"options":[{"id":"o1","text":"A","votes":2,"selected":true},{"id":"o2","text":"B","votes":1,"selected":false}],"totalVotes":3},
 "reactions":{"fire":2,"brain":0,"clap":0,"🎯":1},"reacted":{"fire":true,"brain":false},
 "reactedBy":{"fire":["Bob","Zoë"],"🎯":["Kenji"]},
 "comments":[{"id":"c1","postId":"p1","userId":"u2","displayName":"Bob","friendCode":"BOBB-2345","body":"hi","createdAt":"2026-10-04 10:05:00","isSelf":false}],
 "futureField":{"x":1}}"#;

#[test]
fn a_feed_row_is_validated_into_the_domain() {
    let (rows, usage) = parse_feed(&ok(&format!(r#"{{"feed":[{POST}]}}"#))).unwrap();
    assert!(usage.is_none());
    let p = &rows[0];
    assert_eq!(p.note, "line1 line2", "paragraph text, never markup");
    assert_eq!(
        p.reactions,
        vec![
            ("fire".into(), 2),
            ("brain".into(), 0),
            ("clap".into(), 0),
            ("🎯".into(), 1)
        ]
    );
    assert_eq!(p.reacted, vec!["fire".to_string()]);
    assert_eq!(p.names("fire"), ["Bob".to_string(), "Zoë".to_string()]);
    assert_eq!(p.reaction_keys(false), vec!["fire", "brain", "clap", "🎯"]);
    let poll = p.poll.as_ref().unwrap();
    assert_eq!(
        (
            poll.total_votes,
            poll.options.len(),
            poll.percent(&poll.options[0])
        ),
        (3, 2, 67)
    );
    assert_eq!(p.comments.len(), 1);
    assert!(p.image.is_some() && !p.image_expired);
    assert_eq!(
        p.created_at,
        SocialTimestamp::parse("2026-10-04T10:00:00.000Z")
    );
}

#[test]
fn hostile_and_malformed_feed_data_fails_safe() {
    // not JSON, wrong root, missing feed
    for body in [
        "",
        "{",
        "[]",
        "null",
        r#"{"feed":5}"#.replace("5", "null").as_str(),
        r#"{"nofeed":[]}"#,
    ] {
        let r = parse_feed(&ok(body));
        assert!(
            matches!(r, Err(NetError::Malformed)) || r.as_ref().is_ok_and(|(f, _)| f.is_empty()),
            "{body}"
        );
    }
    assert!(
        parse_feed(&ok(r#"{"feed":{}}"#)).unwrap().0.is_empty(),
        "wrong type: empty list"
    );
    // rows without ids are dropped, duplicates collapse, overlong strings are bounded,
    // controls and bidi overrides removed, markup kept as text
    let long = "x".repeat(10_000);
    let body = format!(
        r#"{{"feed":[{{"userId":"u"}},{{"id":"a","userId":"u","subject":"{long}","note":"<script>\u202Ealert(1)</script>","displayName":"\u0007Bob"}},{{"id":"a","userId":"u"}},
        {{"id":"b","userId":"u","reactions":{{"fire":-5,"brain":1e300,"{long}":1,"x":"str"}},"poll":{{"options":[{{"id":"o","votes":-1}},{{"id":"o","votes":2}},{{"text":"no id"}}],"totalVotes":1}},
          "comments":[{{"id":"c"}},{{"id":"c","userId":"u","body":"dup"}},{{"id":"d","userId":"u","body":"ok"}}]}}]}}"#
    );
    let rows = parse_feed(&ok(&body)).unwrap().0;
    assert_eq!(rows.len(), 2);
    assert!(rows[0].subject.chars().count() <= 120);
    assert_eq!(rows[0].note, "<script>alert(1)</script>");
    assert_eq!(rows[0].display_name, "Bob");
    let b = &rows[1];
    assert_eq!(b.count("fire"), 0, "negative counts are zero");
    assert!(
        b.reactions.iter().all(|(k, _)| k.chars().count() <= 8),
        "a key the Worker cannot store is dropped"
    );
    let poll = b.poll.as_ref().unwrap();
    assert_eq!(
        poll.options.len(),
        1,
        "duplicate and id-less options dropped"
    );
    assert!(
        poll.total_votes >= poll.options.iter().map(|o| o.votes).sum::<u64>(),
        "percent never above 100"
    );
    assert_eq!(
        b.comments.len(),
        2,
        "id-less comment dropped, duplicates collapse"
    );
    // too many rows: bounded
    let many = format!(
        r#"{{"feed":[{}]}}"#,
        (0..500)
            .map(|i| format!(r#"{{"id":"p{i}","userId":"u"}}"#))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        parse_feed(&ok(&many)).unwrap().0.len(),
        study_tracker_core::social::feed::MAX_FEED_ROWS
    );
    // status codes map to typed errors, 5xx text is never shown
    for (status, kind) in [
        (400, "rejected"),
        (401, "unauthorized"),
        (403, "unauthorized"),
        (404, "not-found"),
        (409, "conflict"),
        (413, "payload-too-large"),
        (429, "rate-limited"),
        (500, "server"),
        (503, "server"),
    ] {
        let r = HttpResponse {
            status,
            content_type: None,
            body: b"D1_ERROR internal".to_vec(),
        };
        let e = parse_feed(&r).unwrap_err();
        assert_eq!(e.kind(), kind, "{status}");
    }
}

#[test]
fn squad_snapshots_search_details_and_scores_validate() {
    let snap = r#"{"social":{"friends":[],"incomingFriendRequests":[],"outgoingFriendRequests":[],
      "squad":{"id":"s1","name":"Night\u202E Owls","isPrivate":true,"memberCount":2,"myRole":"wizard","totalMinutes":-3,
               "members":[{"userId":"u1","displayName":"Sam","role":"leader","minutes":10,"sessions":1,"isSelf":true},
                          {"userId":"u1","displayName":"dup"},{"displayName":"no id"},
                          {"userId":"u2","displayName":"Bob","role":"co_leader"}]},
      "incomingSquadRequests":[{"id":"r1","squadId":"s1","userId":"u9","displayName":"Kim"},{"id":"bad"}],
      "outgoingSquadRequests":[],
      "squadMessages":[{"id":"m1","userId":"u2","displayName":"Bob","role":"co_leader","body":"hi\r\nthere","isSelf":false},{"id":"m1","userId":"u2"},{"body":"no id"}]}}"#;
    let s = crate::net::social_api::parse_snapshot(&ok(snap)).unwrap();
    let sq = s.squad.unwrap();
    let squad = sq.squad.unwrap();
    assert_eq!(squad.name, "Night Owls");
    assert_eq!(
        squad.my_role,
        SquadRole::Member,
        "an unknown role is the least privileged"
    );
    assert_eq!(squad.total_minutes, 0);
    assert_eq!(squad.members.len(), 2);
    assert_eq!(sq.incoming.len(), 1);
    assert_eq!(sq.messages.len(), 1);
    assert_eq!(sq.messages[0].body, "hi there");
    // a status reply without the squad keys carries no squad part (production spreads nothing)
    let bare =
        r#"{"social":{"friends":[],"incomingFriendRequests":[],"outgoingFriendRequests":[]}}"#;
    assert!(crate::net::social_api::parse_snapshot(&ok(bare))
        .unwrap()
        .squad
        .is_none());
    // `squad: null` is "no squad", not "no squad part"
    let none = r#"{"social":{"friends":[],"incomingFriendRequests":[],"outgoingFriendRequests":[],"squad":null,"incomingSquadRequests":[],"outgoingSquadRequests":[],"squadMessages":[]}}"#;
    let s = crate::net::social_api::parse_snapshot(&ok(none)).unwrap();
    assert!(s.squad.is_some_and(|p| p.squad.is_none()));
    let search = parse_squad_search(&ok(r#"{"squads":[{"id":"a","name":"A","action":"join","memberCount":2,"maxMembers":4},{"id":"b","name":"","action":"hack"},{"id":"a","name":"dup"}]}"#)).unwrap();
    assert_eq!(search.len(), 2);
    assert_eq!(search[1].action, SquadAction::Unavailable);
    assert_eq!(search[1].name, "Squad");
    assert!(parse_squad_search(&ok(r#"{"nope":1}"#)).is_err());
    let d = parse_squad_details(&ok(r#"{"squad":{"id":"a","name":"A","action":"request","maxMembers":4,"previousDayAverageMinutes":"NaN","members":[]}}"#)).unwrap();
    assert_eq!(d.action, SquadAction::Request);
    assert_eq!(d.previous_day_average_minutes, 0.0);
    assert!(parse_squad_details(&ok(r#"{"squad":null}"#)).is_err());
    let scores = parse_squad_scoreboard(&ok(r#"{"entries":[{"squadId":"a","squadName":"A","rank":1,"points":3,"averageMinutes":120.5,"scoredDays":2},{"squadId":"a"},{"squadName":"x"}]}"#)).unwrap();
    assert_eq!(scores.len(), 1);
    assert_eq!((scores[0].points, scores[0].scored_days), (3, Some(2)));
}

#[test]
fn verified_announcement_admin_and_upload_replies_validate() {
    assert_eq!(
        parse_verified_start(&ok(r#"{"sessionId":"v1","startedAt":"x","resumed":true}"#))
            .unwrap()
            .as_str(),
        "v1"
    );
    assert!(parse_verified_start(&ok(r#"{"sessionId":""}"#)).is_err());
    assert_eq!(
        parse_reconcile(&ok(
            r#"{"ok":true,"creditedMinutes":5,"cappedFromClaimedMinutes":3}"#
        ))
        .unwrap(),
        3
    );
    assert!(parse_reconcile(&ok(r#"{"ok":false}"#)).is_err());
    let a = parse_announcement(&ok(
        r#"{"announcement":{"id":"a1","title":"T\u0000","body":"<b>x</b>\nline"}}"#,
    ))
    .unwrap()
    .unwrap();
    assert_eq!((a.title.as_str(), a.body.as_str()), ("T", "<b>x</b> line"));
    assert!(parse_announcement(&ok(r#"{"announcement":null}"#))
        .unwrap()
        .is_none());
    assert!(parse_announcement(&ok(r#"{"announcement":{"id":""}}"#))
        .unwrap()
        .is_none());
    let usage = parse_admin_usage(&ok(r#"{"summary":{"userCount":3,"active24h":2,"active7d":3},"users":[{"displayName":"A","isFlagged":1}],"telemetry":[{"installId":"i"}]}"#)).unwrap();
    assert_eq!(usage.summary.unwrap().user_count, 3);
    assert!(usage.users[0].is_flagged);
    let (img, _) = parse_post_image(&ok(
        r#"{"ok":true,"imageUrl":"http://127.0.0.1:1/feed/image/k","imageMimeType":"image/png"}"#,
    ))
    .unwrap();
    assert!(img.url.ends_with("/feed/image/k"));
    assert!(parse_post_image(&ok(r#"{"ok":true}"#)).is_err());
    // only a stored photo is a valid avatar reply
    assert!(parse_avatar(&ok(r#"{"avatar":{"kind":"letter","letter":"S"}}"#), "S").is_err());
    assert!(parse_avatar(&ok(r#"{"avatar":{"kind":"photo","name":"p","url":"http://127.0.0.1:1/profile/avatar/k","mimeType":"image/png"}}"#), "S").is_ok());
    let p = parse_poll_vote(&ok(r#"{"ok":true,"poll":null}"#)).unwrap();
    assert!(p.is_none());
    let c = parse_comment(
        &ok(r#"{"ok":true,"comment":{"id":"c","userId":"u","body":"x"}}"#),
        &PostId::parse("p").unwrap(),
    )
    .unwrap();
    assert_eq!(c.post_id.as_str(), "p");
}
