//! Stage 22a protocol tests. The goldens are real responses of the production Worker code
//! (`scripts/stage22-worker-goldens/`: `cloudflare/src/index.ts` in a local, offline workerd),
//! so these assertions are "production server -> fixture -> Rust client", not hand-written.

use super::*;
use crate::net::endpoint::Origin;
use crate::net::images::{allow, ImageKind};
use study_tracker_core::social::avatar::AvatarStyle;
use study_tracker_core::social::friends::FriendsSnapshot;
use study_tracker_core::social::leaderboard::ranked_for_scope;
use study_tracker_core::social::profile::SocialProfile;
use study_tracker_core::social::{DeviceSecret, SocialIdentity};

const GOLDENS: &str = include_str!("../../tests/fixtures/social/worker-goldens.jsonl");
const SECRET: &str = "TEST_SECRET_MUST_NOT_APPEAR";

fn golden(name: &str) -> HttpResponse {
    for line in GOLDENS.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        if v["name"] == name {
            let body = match (&v["body"], &v["bodyBase64"]) {
                (serde_json::Value::String(s), _) => s.as_bytes().to_vec(),
                (_, serde_json::Value::String(b)) => base64(b),
                _ => Vec::new(),
            };
            return HttpResponse {
                status: v["status"].as_u64().unwrap() as u16,
                content_type: v["contentType"].as_str().map(str::to_string),
                body,
            };
        }
    }
    panic!("no golden named {name}");
}

fn base64(s: &str) -> Vec<u8> {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes().filter(|c| *c != b'=') {
        buf = (buf << 6) | T.iter().position(|t| *t == c).unwrap() as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    out
}

fn ok(body: &str) -> HttpResponse {
    HttpResponse {
        status: 200,
        content_type: Some("application/json; charset=utf-8".into()),
        body: body.as_bytes().to_vec(),
    }
}

fn identity() -> SocialIdentity {
    SocialIdentity {
        user_id: UserId::parse("synthetic-user-0001").unwrap(),
        device_secret: DeviceSecret::parse(SECRET).unwrap(),
    }
}

fn test_origin() -> Origin {
    Origin {
        secure: true,
        host: "test.local".into(),
        port: 443,
    }
}

// --------------------------------------------------------------------------- golden: friends

#[test]
fn golden_friend_snapshots_parse_exactly() {
    let s = parse_snapshot(&golden("friends-status-ada-final")).unwrap();
    let names: Vec<&str> = s
        .friends
        .friends
        .iter()
        .map(|f| f.display_name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Bob", "Eve", "張偉 — Zoë 🦊"],
        "server order (display_name ASC)"
    );
    let cho = &s.friends.friends[2];
    assert!(
        matches!(&cho.avatar, Avatar::Photo { url, .. } if url == "https://test.local/profile/avatar/avatars%2Fgolden-cho%2F1.webp")
    );
    assert!(allow(
        cho.avatar.remote_photo_url().unwrap(),
        ImageKind::Avatar,
        &test_origin()
    )
    .is_some());
    assert_eq!(
        s.friends.friends[0].avatar,
        Avatar::Icon {
            icon: "🦊".into()
        }
    );
    assert!(
        s.friends.friends[0].last_seen_at.is_some(),
        "SQLite timestamps parse"
    );
    // incoming: the RTL and long-name requests, newest first
    let incoming: Vec<&str> = s
        .friends
        .incoming
        .iter()
        .map(|r| r.from_display_name.as_str())
        .collect();
    assert_eq!(incoming.len(), 2);
    assert!(incoming.contains(&"שלום עולם"));
    assert!(
        incoming.contains(&"A very long display name that goes on and on and"),
        "the Worker cut it to 48"
    );
    assert!(s.friends.outgoing.is_empty());
    assert!(
        s.leaderboards.is_empty(),
        "/friends/status/v2 sends no caches"
    );
}

#[test]
fn golden_request_and_respond_replies_carry_caches() {
    let s = parse_snapshot(&golden("friend-request-to-bob")).unwrap();
    assert_eq!(s.friends.outgoing.len(), 1);
    assert_eq!(s.friends.outgoing[0].to_friend_code.as_str(), "BOBB-2345");
    assert_eq!(
        s.leaderboards.len(),
        6,
        "global + friends x daily/weekly/overall"
    );
    let accepted = parse_snapshot(&golden("friend-respond-accept")).unwrap();
    assert_eq!(accepted.friends.friends.len(), 1);
    assert!(accepted.friends.incoming.is_empty());
    // the lower-case, padded code was normalised by the Worker (cleanCode) and accepted
    let s = parse_snapshot(&golden("friend-request-to-cho")).unwrap();
    assert!(s
        .friends
        .outgoing
        .iter()
        .any(|r| r.to_friend_code.as_str() == "CHOO-2345"));
    // a duplicate request is idempotent on the server (ON CONFLICT DO UPDATE): still one row
    let again = parse_snapshot(&golden("friend-request-to-cho-again")).unwrap();
    assert_eq!(
        again
            .friends
            .outgoing
            .iter()
            .filter(|r| r.to_friend_code.as_str() == "CHOO-2345")
            .count(),
        1
    );
}

#[test]
fn golden_friend_errors_map_to_productions_messages() {
    let cases = [
        (
            "friend-request-unknown-code",
            "No user with that friend code exists.",
        ),
        ("friend-request-self", "You cannot add yourself."),
        ("friend-request-already-friends", "You are already friends."),
        ("friend-respond-again", "Friend request not found."),
        ("friends-status-wrong-secret", "Invalid device secret."),
        ("sync-duplicate-code", "Friend code is already in use."),
        (
            "presence-unknown-user",
            "User has not synced a profile yet.",
        ),
    ];
    for (name, message) in cases {
        let err = parse_snapshot(&golden(name)).unwrap_err();
        assert_eq!(err.user_message("x"), message, "{name}");
    }
    assert!(matches!(
        parse_snapshot(&golden("friend-request-already-friends")),
        Err(NetError::Conflict(_))
    ));
    assert!(matches!(
        parse_snapshot(&golden("friends-status-wrong-secret")),
        Err(NetError::Unauthorized(_))
    ));
}

// ---------------------------------------------------------------------- golden: leaderboards

#[test]
fn golden_leaderboards_are_consumed_as_the_server_ranks_them() {
    let friends = parse_leaderboard(&golden("leaderboard-friends-daily")).unwrap();
    let rows: Vec<(&str, u64, bool)> = friends
        .iter()
        .map(|e| (e.display_name.as_str(), e.minutes, e.is_self))
        .collect();
    // Eve hides her hours from friends (show_hours_to_friends = 0): not listed
    assert_eq!(
        rows,
        [
            ("Bob", 120, false),
            ("Ada Lovelace", 50, true),
            ("張偉 — Zoë 🦊", 50, false)
        ]
    );
    let global = parse_leaderboard(&golden("leaderboard-global-daily")).unwrap();
    assert!(
        !global.iter().any(|e| e.display_name == "Dan Private"),
        "private users are hidden globally"
    );
    let overall = parse_leaderboard(&golden("leaderboard-global-overall")).unwrap();
    assert_eq!(
        overall[0].display_name,
        "A very long display name that goes on and on and"
    );
    assert_eq!(
        overall[0].minutes, 1000,
        "baseline totals (0022's corrected view) pass through unchanged"
    );
    assert!(parse_leaderboard(&golden("leaderboard-squad-daily"))
        .unwrap()
        .is_empty());
    assert!(matches!(
        parse_leaderboard(&golden("leaderboard-unknown-user")),
        Err(NetError::NotFound(_))
    ));
    // the client-side view of the friends scope keeps self and friends, re-ranks by minutes then name
    let me = UserId::parse("golden-ada").unwrap();
    let snapshot = parse_snapshot(&golden("friends-status-ada-final"))
        .unwrap()
        .friends;
    let ranked = ranked_for_scope(&friends, LeaderboardScope::Friends, &me, &snapshot, &[]);
    assert_eq!(
        ranked
            .iter()
            .map(|e| (e.rank, e.display_name.as_str()))
            .collect::<Vec<_>>(),
        [(1, "Bob"), (2, "Ada Lovelace"), (3, "張偉 — Zoë 🦊")]
    );
    assert!(ranked[1].is_self);
}

#[test]
fn golden_player_stats() {
    let bob = parse_player_stats(&golden("player-stats-friend")).unwrap();
    assert!(bob.hours_visible);
    let [d, w, o] = bob.periods.clone().unwrap();
    assert_eq!((d.minutes, w.minutes, o.minutes), (120, 180, 180));
    let eve = parse_player_stats(&golden("player-stats-hidden-hours")).unwrap();
    assert!(
        !eve.hours_visible && eve.periods.is_none(),
        "\"This friend has chosen not to share their study hours.\""
    );
    let me = parse_player_stats(&golden("player-stats-self")).unwrap();
    assert_eq!(
        me.avatar,
        Avatar::Letter {
            letter: "A".into(),
            style: AvatarStyle::Pixel
        }
    );
    assert_eq!(
        parse_player_stats(&golden("player-stats-not-friend"))
            .unwrap_err()
            .user_message("x"),
        "User is private."
    );
    assert!(matches!(
        parse_player_stats(&golden("player-stats-missing")),
        Err(NetError::NotFound(_))
    ));
}

#[test]
fn golden_sync_and_presence() {
    assert_eq!(parse_ok(&golden("sync-create-ada")), Ok(()));
    assert_eq!(parse_ok(&golden("presence-ok")), Ok(()));
    assert!(matches!(
        parse_ok(&golden("sync-wrong-secret")),
        Err(NetError::Unauthorized(_))
    ));
    assert!(matches!(
        parse_ok(&golden("sync-missing-user")),
        Err(NetError::Rejected(_))
    ));
}

// --------------------------------------------------------------------------- golden: Skribbl

#[test]
fn golden_skribbl_theme_and_submission() {
    let fresh = parse_theme(&golden("skribbl-theme-fresh")).unwrap();
    assert!(!fresh.submitted && fresh.image_url.is_none() && fresh.drawing_id.is_none());
    assert_eq!(fresh.date.len(), 10);
    assert!(!fresh.theme.is_empty());
    let done = parse_theme(&golden("skribbl-theme-submitted")).unwrap();
    assert!(done.submitted && done.drawing_id.is_some());
    let url = done.image_url.unwrap();
    assert!(
        allow(&url, ImageKind::SkribblDrawing, &test_origin()).is_some(),
        "{url}"
    );
    let submitted = parse_submit(&golden("skribbl-submit-ok")).unwrap();
    assert!(allow(&submitted, ImageKind::SkribblDrawing, &test_origin()).is_some());
    for (name, kind, message) in [
        (
            "skribbl-submit-duplicate",
            "conflict",
            "You already submitted a drawing today.",
        ),
        (
            "skribbl-submit-wrong-date",
            "rejected",
            "Drawings are only accepted for today's theme.",
        ),
        ("skribbl-submit-gif", "rejected", "Use PNG or WebP images."),
        (
            "skribbl-submit-too-large",
            "payload-too-large",
            "Drawing is too large. Keep it under 1.5 MB.",
        ),
    ] {
        let err = parse_submit(&golden(name)).unwrap_err();
        assert_eq!(
            (err.kind(), err.user_message("x").as_str()),
            (kind, message),
            "{name}"
        );
    }
    assert!(matches!(
        parse_theme(&golden("skribbl-theme-unknown-user")),
        Err(NetError::NotFound(Some(_)))
    ));
}

#[test]
fn golden_skribbl_gallery_votes_and_winner() {
    let empty = parse_gallery(&golden("skribbl-gallery-empty")).unwrap();
    assert!(empty.drawings.is_empty() && !empty.has_more && empty.next_offset.is_none());
    let p1 = parse_gallery(&golden("skribbl-gallery-page1")).unwrap();
    assert_eq!(p1.drawings.len(), 16);
    assert!(p1.has_more);
    assert_eq!(p1.next_offset, Some(16));
    assert_eq!(p1.drawings.iter().filter(|d| d.is_self).count(), 1);
    assert!(p1.drawings.iter().all(|d| allow(
        &d.image_url,
        ImageKind::SkribblDrawing,
        &test_origin()
    )
    .is_some()));
    let p2 = parse_gallery(&golden("skribbl-gallery-page2")).unwrap();
    assert_eq!(p2.drawings.len(), 4);
    assert!(!p2.has_more && p2.next_offset.is_none());
    assert_eq!(parse_vote(&golden("skribbl-vote-up")), Ok(1));
    assert_eq!(parse_vote(&golden("skribbl-vote-down")), Ok(-1));
    assert_eq!(parse_vote(&golden("skribbl-vote-down-bob")), Ok(-2));
    assert_eq!(parse_vote(&golden("skribbl-vote-clear")), Ok(-1));
    assert_eq!(
        parse_vote(&golden("skribbl-vote-own"))
            .unwrap_err()
            .user_message("x"),
        "You cannot vote on your own drawing."
    );
    assert!(matches!(
        parse_vote(&golden("skribbl-vote-invalid")),
        Err(NetError::Rejected(_))
    ));
    assert!(matches!(
        parse_vote(&golden("skribbl-vote-missing")),
        Err(NetError::NotFound(_))
    ));
    let after = parse_gallery(&golden("skribbl-gallery-after-votes")).unwrap();
    assert!(
        after.drawings.iter().any(|d| d.my_vote == 1),
        "ada's vote shows as myVote"
    );
    assert_eq!(
        parse_skribbl_leaderboard(&golden("skribbl-leaderboard-no-winner")),
        Ok(None)
    );
    let winner = parse_skribbl_leaderboard(&golden("skribbl-leaderboard-winner"))
        .unwrap()
        .unwrap();
    assert_eq!(
        (winner.display_name.as_str(), winner.score),
        ("張偉 — Zoë 🦊", 2)
    );
}

#[test]
fn golden_drawing_bytes_decode_and_wrong_routes_fail() {
    let png = golden("skribbl-drawing-png");
    assert_eq!(png.content_type.as_deref(), Some("image/png"));
    let img = crate::net::images::decode(
        ImageKind::SkribblDrawing,
        png.content_type.as_deref(),
        &png.body,
        360,
        240,
    )
    .unwrap();
    assert_eq!((img.width, img.height), (2, 2));
    let missing = golden("skribbl-drawing-missing");
    assert_eq!(missing.status, 404);
}

#[test]
fn no_golden_response_ever_echoes_a_credential() {
    for line in GOLDENS.lines() {
        assert!(!line.contains("secret-"), "{}", &line[..80.min(line.len())]);
    }
}

// --------------------------------------------------------------------------- request shapes

fn json_of(req: &ApiRequest) -> serde_json::Value {
    match &req.body {
        Body::Json(b) => serde_json::from_slice(b).unwrap(),
        _ => panic!("not JSON"),
    }
}

#[test]
fn requests_have_productions_exact_shapes() {
    let id = identity();
    let body = json_of(&friend_request_create(
        &id,
        &FriendCode::parse("ABCD-2345").unwrap(),
    ));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "friendCode": "ABCD-2345"})
    );
    let body = json_of(&friend_respond(
        &id,
        &RequestId::parse("r1").unwrap(),
        FriendResponse::Declined,
    ));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "requestId": "r1", "response": "declined"})
    );
    let body = json_of(&leaderboard(
        &id,
        LeaderboardScope::Friends,
        LeaderboardPeriod::Weekly,
    ));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "scope": "friends", "period": "weekly"})
    );
    let body = json_of(&player_stats(&id, &UserId::parse("u2").unwrap()));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "targetUserId": "u2"})
    );
    let body = json_of(&friends_status(&id));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET})
    );
    let body = json_of(&skribbl_gallery(&id, "2026-10-04", 16));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "date": "2026-10-04", "offset": 16, "limit": 16})
    );
    let body = json_of(&skribbl_vote(&id, &DrawingId::parse("d1").unwrap(), -1).unwrap());
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "drawingId": "d1", "vote": -1})
    );
    assert!(skribbl_vote(&id, &DrawingId::parse("d1").unwrap(), 5).is_none());
    let app = crate::net::device::AppMetadata {
        version: "0.1.0".into(),
        platform: "Linux x86_64".into(),
        runtime_channel: "development".into(),
    };
    let body = json_of(&presence(&id, &app));
    assert_eq!(
        body,
        serde_json::json!({"userId": "synthetic-user-0001", "deviceSecret": SECRET, "app": {"version": "0.1.0", "platform": "Linux x86_64", "runtimeChannel": "development"}})
    );
    // the two Skribbl GETs: credentials in the query (production), never in the log path
    let theme = skribbl_theme(&id);
    assert_eq!(theme.method, Method::Get);
    assert!(theme
        .path_and_query()
        .starts_with("/skribbl/theme?userId=synthetic-user-0001&deviceSecret="));
    assert_eq!(skribbl_leaderboard(&id).log_path(), "/skribbl/leaderboard");
}

#[test]
fn the_sync_payload_matches_sync_social_state() {
    let id = identity();
    let profile = SocialProfile::new_default(FriendCode::parse("ABCD-WXYZ").unwrap());
    let stats = [study_tracker_core::social::stats::DailyStat {
        date: study_tracker_core::dashboard::civil::CivilDate::from_ymd(2026, 10, 4).unwrap(),
        minutes: 50,
        sessions: 2,
    }];
    let device = crate::net::device::device_identity_for("synthetic-machine");
    let app = crate::net::device::AppMetadata {
        version: "0.1.0".into(),
        platform: "Linux x86_64".into(),
        runtime_channel: "development".into(),
    };
    let req = sync_v2(&SyncInput {
        identity: &id,
        profile: &profile,
        lifetime_minutes: 50,
        lifetime_sessions: 2,
        stats: &stats,
        device: &device,
        app: &app,
    })
    .unwrap();
    let body = json_of(&req);
    assert_eq!(body["user"]["userId"], "synthetic-user-0001");
    assert_eq!(body["user"]["friendCode"], "ABCD-WXYZ");
    assert_eq!(body["user"]["displayName"], "Student WXYZ");
    assert_eq!(
        body["user"]["avatar"],
        serde_json::json!({"kind": "letter", "letter": "S", "style": "classic"})
    );
    assert_eq!(body["user"]["isPrivate"], false);
    assert_eq!(body["user"]["showHoursToFriends"], true);
    assert_eq!(body["user"]["lifetimeStudyMinutes"], 50);
    assert_eq!(
        body["user"]["device"]["fingerprintHash"]
            .as_str()
            .unwrap()
            .len(),
        16
    );
    assert_eq!(body["user"]["app"]["runtimeChannel"], "development");
    assert_eq!(
        body["stats"],
        serde_json::json!([{"date": "2026-10-04", "minutes": 50, "sessions": 2}])
    );
    assert_eq!(body["feedPosts"], serde_json::json!([]));
    assert!(
        body["user"].get("referralCode").is_none(),
        "never the honeypot field"
    );
    assert_eq!(req.log_path(), "/sync/v2");
    assert!(!format!("{req:?}").contains(SECRET));
}

#[test]
fn submit_builds_productions_form_and_refuses_oversize() {
    let id = identity();
    let req = skribbl_submit(&id, vec![0x89, b'P', b'N', b'G'], "2026-10-04").unwrap();
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
        "name=\"date\"",
        "name=\"image\"; filename=\"drawing.png\"",
        "Content-Type: image/png",
    ] {
        assert!(text.contains(field), "{field}");
    }
    assert!(!format!("{req:?}").contains(SECRET));
    let err = skribbl_submit(
        &id,
        vec![0; study_tracker_core::break_room::skribbl::MAX_SUBMIT_BYTES + 1],
        "d",
    )
    .unwrap_err();
    assert_eq!(
        err.user_message("x"),
        "Drawing is too large. Keep it under 1.5 MB."
    );
}

// ------------------------------------------------------------------------ malformed matrix

#[test]
fn malformed_bodies_never_panic_and_fail_safely() {
    let bodies: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b"not json".to_vec(),
        b"{\"social\":{\"friends\":[".to_vec(),
        b"[]".to_vec(),
        b"null".to_vec(),
        b"42".to_vec(),
        b"\"str\"".to_vec(),
        b"{}".to_vec(),
        vec![0xff, 0xfe, 0xfd],
        b"{\"entries\":{}}".to_vec(),
        b"{\"social\":[]}".to_vec(),
    ];
    for body in &bodies {
        let resp = HttpResponse {
            status: 200,
            content_type: None,
            body: body.clone(),
        };
        assert!(parse_snapshot(&resp).is_err());
        let _ = parse_leaderboard(&resp);
        assert!(parse_player_stats(&resp).is_ok() || parse_player_stats(&resp).is_err());
        assert!(parse_theme(&resp).is_err());
        let _ = parse_gallery(&resp);
        assert!(parse_submit(&resp).is_err());
        assert!(parse_vote(&resp).is_err());
        let _ = parse_skribbl_leaderboard(&resp);
        assert!(parse_ok(&resp).is_err());
    }
}

#[test]
fn rows_with_bad_fields_are_skipped_not_fatal() {
    let body = r#"{"social":{
        "friends":[
            {"userId":"ok","displayName":"Ok","friendCode":"OKOK-2345","avatar":{"kind":"letter","letter":"o","style":"wat"}},
            {"userId":"","displayName":"No id","friendCode":"X"},
            {"userId":"x2","displayName":123,"friendCode":"XXXX-2345"},
            {"userId":"x3","friendCode":null},
            "garbage", 7, null,
            {"userId":"ok","displayName":"Duplicate","friendCode":"OKOK-2345"},
            {"userId":"x4","displayName":"<b>bold</b>\u202Eevil","friendCode":"XXXY-2345","lastSeenAt":"2099-99-99","avatar":{"kind":"hologram"}, "unknownField":{"deep":[1,2,3]}}
        ],
        "incomingFriendRequests":[{"id":"r1"}],
        "outgoingFriendRequests":"not a list",
        "squad": {"weird": true}
    }}"#;
    let s = parse_snapshot(&ok(body)).unwrap();
    let names: Vec<&str> = s
        .friends
        .friends
        .iter()
        .map(|f| f.display_name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Ok", "Student", "<b>bold</b>evil"],
        "wrong type -> fallback name; markup kept as text; bidi override removed"
    );
    assert_eq!(
        s.friends.friends[0].avatar,
        Avatar::Letter {
            letter: "O".into(),
            style: AvatarStyle::Classic
        }
    );
    assert_eq!(
        s.friends.friends[2].last_seen_at, None,
        "impossible timestamp"
    );
    assert_eq!(
        s.friends.friends[2].avatar,
        Avatar::default_for("<b>bold</b>evil"),
        "unknown avatar kind"
    );
    assert!(
        s.friends.incoming.is_empty(),
        "a request missing required fields is dropped"
    );
    assert!(s.friends.outgoing.is_empty(), "a non-list is empty");
}

#[test]
fn oversized_lists_strings_and_numbers_are_bounded() {
    let many: Vec<String> = (0..MAX_LEADERBOARD_ENTRIES + 500)
        .map(|i| format!(r#"{{"userId":"u{i}","displayName":"{}","friendCode":"C{i}","minutes":1e300,"sessions":-5,"rank":"x"}}"#, "n".repeat(10_000)))
        .collect();
    let body = format!(r#"{{"entries":[{}]}}"#, many.join(","));
    let rows = parse_leaderboard(&ok(&body)).unwrap();
    assert_eq!(rows.len(), MAX_LEADERBOARD_ENTRIES);
    assert_eq!(
        rows[0].display_name.chars().count(),
        64,
        "display strings are bounded"
    );
    assert_eq!(rows[0].minutes, MAX_TOTAL_MINUTES);
    assert_eq!(rows[0].sessions, 0, "negative counts become 0");
    assert_eq!(rows[0].rank, 0);
    // NaN-like / duplicates / unknown my_vote
    let g = r#"{"drawings":[
        {"id":"a","userId":"u","displayName":"A","voteScore":"NaN","voteCount":3,"myVote":7,"isSelf":false,"imageUrl":"https://test.local/skribbl/drawing/a"},
        {"id":"a","userId":"u","displayName":"A again","voteScore":1,"voteCount":1,"myVote":1,"isSelf":false,"imageUrl":"https://test.local/skribbl/drawing/a"},
        {"id":"b","userId":"u","displayName":"B","imageUrl":null}
    ],"nextOffset":-1,"hasMore":true}"#;
    let page = parse_gallery(&ok(g)).unwrap();
    assert_eq!(
        page.drawings.len(),
        1,
        "duplicate ids and rows without an image are dropped"
    );
    assert_eq!(
        (page.drawings[0].vote_score, page.drawings[0].my_vote),
        (0, 0)
    );
    assert_eq!(
        (page.next_offset, page.has_more),
        (None, false),
        "no next page without a valid offset"
    );
}

#[test]
fn single_object_responses_need_their_required_fields() {
    assert!(
        parse_theme(&ok(r#"{"date":"2026-10-04"}"#)).is_err(),
        "no theme"
    );
    assert!(
        parse_theme(&ok(r#"{"date":"04.10.2026","theme":"x"}"#)).is_err(),
        "bad date"
    );
    assert!(
        parse_theme(&ok(
            r#"{"date":"2026-10-04","theme":"x","submitted":"yes"}"#
        ))
        .is_ok(),
        "wrong-typed optional is tolerated"
    );
    let t = parse_theme(&ok(r#"{"date":"2026-10-04","theme":"Cat\u0000 wearing a hat\u202E","submitted":false,"imageUrl":"https://evil.example/x"}"#)).unwrap();
    assert_eq!(t.theme, "Cat wearing a hat");
    assert_eq!(
        t.image_url, None,
        "an image URL without a submission is ignored"
    );
    assert!(parse_submit(&ok(r#"{"ok":true}"#)).is_err());
    assert!(parse_submit(&ok(
        r#"{"ok":false,"imageUrl":"https://x/skribbl/drawing/a"}"#
    ))
    .is_err());
    assert!(parse_vote(&ok(r#"{"ok":true}"#)).is_err());
    assert_eq!(
        parse_skribbl_leaderboard(&ok(r#"{"winner":{"displayName":"X","score":3}}"#)),
        Ok(None),
        "a winner without a drawing is no winner"
    );
    assert_eq!(
        parse_skribbl_leaderboard(&ok(r#"{"winner":{"drawingId":"d","score":1}}"#))
            .unwrap()
            .unwrap()
            .display_name,
        "Student"
    );
}

#[test]
fn http_status_matrix() {
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
        let resp = HttpResponse {
            status,
            content_type: None,
            body: b"<html>stack trace at line 1</html>".to_vec(),
        };
        let err = parse_theme(&resp).unwrap_err();
        assert_eq!(err.kind(), kind, "{status}");
        assert!(
            !err.user_message("x").contains("stack trace"),
            "raw bodies never reach the UI"
        );
    }
    // 3xx is never followed (transport) and is not success
    let resp = HttpResponse {
        status: 302,
        content_type: None,
        body: vec![],
    };
    assert!(parse_ok(&resp).is_err());
}

#[test]
fn snapshots_are_not_duplicated_by_replay() {
    let reply = golden("friends-status-ada-final");
    let a = parse_snapshot(&reply).unwrap();
    let b = parse_snapshot(&reply).unwrap();
    assert_eq!(
        a, b,
        "applying the same reply twice yields the same state (snapshots replace)"
    );
    let merged = FriendsSnapshot {
        friends: [a.friends.friends.clone(), b.friends.friends.clone()].concat(),
        ..Default::default()
    }
    .deduplicated();
    assert_eq!(merged.friends.len(), a.friends.friends.len());
}
