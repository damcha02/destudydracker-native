use super::avatar::{arena_hue, first_avatar_letter, initials, AVATAR_ICONS};
use super::friends::{check_friend_request, FriendResponse};
use super::ids::display_text;
use super::leaderboard::{bar_percent, ranked_for_scope, top_minutes};
use super::profile::{clean_display_name, SyncStatus};
use super::stats::{daily_stats, monthly_stat, period_stat, sync_stats};
use super::time::{is_recently_active, next_auto_sync_at, RECENTLY_ACTIVE_MS};
use super::*;
use crate::academic::{AcademicState, SessionId, SessionKind, StudySession};
use crate::dashboard::civil::{CivilDate, FixedOffsetClock};
use crate::timer::WallTimestamp;

const SECRET: &str = "TEST_SECRET_MUST_NOT_APPEAR";

fn uid(s: &str) -> UserId {
    UserId::parse(s).unwrap()
}

fn code(s: &str) -> FriendCode {
    FriendCode::parse(s).unwrap()
}

fn ts(s: &str) -> SocialTimestamp {
    SocialTimestamp::parse(s).unwrap()
}

fn friend(id: &str, name: &str, fc: &str) -> Friend {
    Friend {
        user_id: uid(id),
        display_name: name.into(),
        friend_code: code(fc),
        avatar: Avatar::default_for(name),
        friends_since: None,
        last_seen_at: None,
    }
}

fn request(id: &str, to_code: &str) -> FriendRequest {
    FriendRequest {
        id: RequestId::parse(id).unwrap(),
        from_user_id: uid("me"),
        to_user_id: uid("them"),
        from_display_name: "Me".into(),
        to_display_name: "Them".into(),
        from_friend_code: code("AAAA-AAAA"),
        to_friend_code: code(to_code),
        from_avatar: Avatar::default_for("Me"),
        to_avatar: Avatar::default_for("Them"),
        created_at: None,
    }
}

fn entry(id: &str, name: &str, minutes: u64) -> LeaderboardEntry {
    LeaderboardEntry {
        user_id: uid(id),
        display_name: name.into(),
        friend_code: code("ZZZZ-ZZZZ"),
        avatar: Avatar::default_for(name),
        minutes,
        sessions: 1,
        rank: 99,
        last_active_date: None,
        is_self: false,
    }
}

// ---------------------------------------------------------------------------------- identity

#[test]
fn device_secret_never_appears_in_debug_output() {
    let identity = SocialIdentity {
        user_id: uid("synthetic-user"),
        device_secret: DeviceSecret::parse(SECRET).unwrap(),
    };
    let debug = format!(
        "{identity:?} {:?} {:#?}",
        identity.device_secret,
        IdentityPhase::ExistingIdentity {
            identity: identity.clone()
        }
    );
    assert!(!debug.contains(SECRET), "{debug}");
    assert!(debug.contains("redacted"));
    assert_eq!(identity.device_secret.expose(), SECRET);
}

#[test]
fn device_secret_parsing_follows_the_worker_bounds() {
    assert!(DeviceSecret::parse("").is_none());
    assert!(DeviceSecret::parse("   ").is_none());
    assert!(DeviceSecret::parse(&"x".repeat(121)).is_none());
    assert!(DeviceSecret::parse(&"x".repeat(120)).is_some());
    assert!(DeviceSecret::parse("a b").is_none());
    assert!(DeviceSecret::parse("a\nb").is_none());
    assert_eq!(DeviceSecret::parse("  abc  ").unwrap().expose(), "abc");
}

#[test]
fn minted_identities_are_uuid_v4_and_distinct() {
    let a = SocialIdentity::mint([0; 16], [0xff; 16]);
    assert_eq!(a.user_id.as_str(), "00000000-0000-4000-8000-000000000000");
    assert_eq!(
        a.device_secret.expose(),
        "ffffffff-ffff-4fff-bfff-ffffffffffff"
    );
    assert_ne!(a.user_id.as_str(), a.device_secret.expose());
}

#[test]
fn identity_phases_only_run_background_work_when_established() {
    let identity = SocialIdentity::mint([1; 16], [2; 16]);
    assert!(IdentityPhase::NoIdentity.identity().is_none());
    assert!(!IdentityPhase::NoIdentity.is_established());
    let new = IdentityPhase::NewIdentity {
        candidate: identity.clone(),
    };
    assert!(new.identity().is_some());
    assert!(
        !new.is_established(),
        "a candidate never runs the background schedule"
    );
    assert!(IdentityPhase::ExistingIdentity { identity }.is_established());
}

// ------------------------------------------------------------------------------- identifiers

#[test]
fn ids_are_trimmed_bounded_and_control_free() {
    assert_eq!(uid("  abc ").as_str(), "abc");
    assert!(UserId::parse("").is_none());
    assert!(UserId::parse(&"a".repeat(81)).is_none());
    assert!(UserId::parse("a\u{0}b").is_none());
    assert!(UserId::parse("a b").is_none());
}

#[test]
fn friend_codes_normalise_like_production() {
    let c = FriendCode::from_user_input("  abcd-2345 ").unwrap();
    assert_eq!(c.as_str(), "ABCD-2345");
    assert!(c.is_standard());
    assert!(!code("ABCD-1234").is_standard(), "1 is not in the alphabet");
    assert!(FriendCode::from_user_input("   ").is_none());
    assert!(FriendCode::parse(&"A".repeat(33)).is_none());
    let generated = FriendCode::generate([0, 1, 2, 3, 31, 32, 255, 64]);
    assert!(generated.is_standard());
    assert_eq!(
        generated.as_str(),
        "ABCD-9A9A",
        "byte % 32 picks the letter"
    );
}

#[test]
fn display_text_strips_controls_and_bidi_overrides_but_keeps_unicode() {
    assert_eq!(display_text("Zoë 学生 🦊", 100), "Zoë 学生 🦊");
    assert_eq!(display_text("שלום", 100), "שלום", "RTL text itself is kept");
    assert_eq!(display_text("a\u{202E}evil\u{202C}b", 100), "aevilb");
    assert_eq!(display_text("a\u{2066}b\u{2069}", 100), "ab");
    assert_eq!(display_text("line\nbreak\t\u{7}", 100), "linebreak");
    assert_eq!(
        display_text("\u{200F}mark", 100),
        "\u{200F}mark",
        "RLM is harmless"
    );
    assert_eq!(display_text(&"x".repeat(500), 10).len(), 10);
    assert_eq!(
        display_text("<script>alert(1)</script>", 100),
        "<script>alert(1)</script>",
        "plain text, never markup"
    );
}

// ---------------------------------------------------------------------------------- timestamps

#[test]
fn social_timestamps_parse_production_forms() {
    assert_eq!(
        ts("2026-09-30 12:00:00").0,
        ts("2026-09-30T12:00:00Z").0,
        "SQLite form is UTC"
    );
    assert_eq!(
        ts("2026-09-30T14:00:00+02:00").0,
        ts("2026-09-30T12:00:00Z").0
    );
    assert_eq!(
        ts("2026-09-30T12:00:00.250Z").0 - ts("2026-09-30T12:00:00Z").0,
        250
    );
    assert_eq!(
        ts("2026-09-30").0,
        CivilDate::from_ymd(2026, 9, 30).unwrap().days() * 86_400_000
    );
    assert_eq!(
        ts("2026-09-30T12:00:00Z").to_iso(),
        "2026-09-30T12:00:00.000Z"
    );
    for bad in [
        "",
        "yesterday",
        "2026-02-31 12:00:00",
        "2026-13-01",
        "2026-09-30 25:00:00",
        "2026-09-30T12:00:00",
        "2026-09-30 12:00:00Zjunk",
        "２０２６-09-30",
        &"9".repeat(60),
    ] {
        assert!(SocialTimestamp::parse(bad).is_none(), "{bad:?}");
    }
}

#[test]
fn recently_active_matches_production_window() {
    let now = ts("2026-09-30T12:00:00Z").wall();
    let at = |s: &str| Some(ts(s));
    assert!(is_recently_active(
        at("2026-09-30T11:16:00Z"),
        now,
        RECENTLY_ACTIVE_MS
    ));
    assert!(
        !is_recently_active(at("2026-09-30T11:15:00Z"), now, RECENTLY_ACTIVE_MS),
        "exactly 45 min is not < 45 min"
    );
    assert!(
        is_recently_active(at("2026-10-01T00:00:00Z"), now, RECENTLY_ACTIVE_MS),
        "future counts, as in production"
    );
    assert!(!is_recently_active(None, now, RECENTLY_ACTIVE_MS));
}

#[test]
fn next_auto_sync_is_the_earliest_of_interval_midnight_and_monday() {
    let zurich = FixedOffsetClock::new(2 * 3600);
    // Wednesday 2026-09-30 10:00 local: midnight (14 h away) loses to the 12 h interval
    let now = ts("2026-09-30T08:00:00Z").wall();
    assert_eq!(
        next_auto_sync_at(now, &zurich).unix_millis,
        now.unix_millis + 12 * 3_600_000
    );
    // 20:00 local: local midnight comes first
    let now = ts("2026-09-30T18:00:00Z").wall();
    assert_eq!(
        SocialTimestamp::from_wall(next_auto_sync_at(now, &zurich)).to_iso(),
        "2026-09-30T22:00:00.000Z"
    );
    // Sunday 23:00 local: Monday 00:00 is also the next midnight
    let now = ts("2026-10-04T21:00:00Z").wall();
    assert_eq!(
        SocialTimestamp::from_wall(next_auto_sync_at(now, &zurich)).to_iso(),
        "2026-10-04T22:00:00.000Z"
    );
    let status = SyncStatus {
        next_auto_sync_at: Some(ts("2026-09-30T12:00:00Z")),
        ..Default::default()
    };
    assert!(!status.should_auto_sync(ts("2026-09-30T11:59:59Z").wall()));
    assert!(status.should_auto_sync(ts("2026-09-30T12:00:00Z").wall()));
    assert!(SyncStatus::default().should_auto_sync(now));
}

// ------------------------------------------------------------------------------------ avatars

#[test]
fn avatars_normalise_like_storage_and_worker() {
    let n = |kind, letter, style, icon, photo| {
        Avatar::normalized(kind, letter, style, icon, photo, "zoë")
    };
    assert_eq!(
        n(Some("letter"), Some("q"), Some("pixel"), None, None),
        Avatar::Letter {
            letter: "Q".into(),
            style: AvatarStyle::Pixel
        }
    );
    assert_eq!(
        n(Some("letter"), Some("QQ"), Some("nope"), None, None),
        Avatar::Letter {
            letter: "Z".into(),
            style: AvatarStyle::Classic
        }
    );
    assert_eq!(
        n(Some("letter"), Some("é"), None, None, None),
        Avatar::Letter {
            letter: "Z".into(),
            style: AvatarStyle::Classic
        },
        "only A-Z"
    );
    assert_eq!(
        n(Some("icon"), None, None, Some("🦊"), None),
        Avatar::Icon {
            icon: "🦊".into()
        }
    );
    assert_eq!(
        n(Some("icon"), None, None, Some("💩"), None),
        Avatar::default_for("zoë"),
        "icon outside the set"
    );
    let photo = n(
        Some("photo"),
        None,
        None,
        None,
        Some((
            "me.webp",
            "https://w.example/profile/avatar/u/1.webp",
            "text/html",
        )),
    );
    assert_eq!(
        photo,
        Avatar::Photo {
            name: "me.webp".into(),
            url: "https://w.example/profile/avatar/u/1.webp".into(),
            mime_type: "image/webp".into()
        }
    );
    for bad in [
        "http://w.example/profile/avatar/a",
        "https://w.example/feed/image/a",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https:///profile/avatar/x",
    ] {
        assert_eq!(
            n(
                Some("photo"),
                None,
                None,
                None,
                Some(("x", bad, "image/png"))
            ),
            Avatar::default_for("zoë"),
            "{bad}"
        );
    }
    assert_eq!(
        n(Some("hologram"), None, None, None, None),
        Avatar::default_for("zoë")
    );
    assert_eq!(n(None, None, None, None, None), Avatar::default_for("zoë"));
    assert_eq!(AVATAR_ICONS.len(), 40);
}

#[test]
fn avatar_presentation_helpers() {
    assert_eq!(first_avatar_letter("  zoë"), "Z");
    assert_eq!(first_avatar_letter(""), "S");
    assert_eq!(
        first_avatar_letter("ßtudent"),
        "SS",
        "toUpperCase semantics"
    );
    assert_eq!(initials("ada lovelace"), "AL");
    assert_eq!(initials("one_two-three four"), "OT");
    assert_eq!(initials("   "), "ST");
    assert_eq!(arena_hue("Ada"), (65 + 100 + 97) % 360);
    // astral: the high surrogate counts (0xD83E for 🦊)
    assert_eq!(arena_hue("🦊"), 0xD83E % 360);
    assert_eq!(Avatar::Icon { icon: "★".into() }.display_text("x"), "★");
    assert_eq!(Avatar::default_for("bob").display_text("bob"), "B");
    let photo = Avatar::Photo {
        name: String::new(),
        url: "https://x/profile/avatar/a".into(),
        mime_type: "image/webp".into(),
    };
    assert_eq!(photo.display_text("Ada L"), "");
    assert_eq!(photo.remote_photo_url(), Some("https://x/profile/avatar/a"));
    let data = Avatar::Photo {
        name: String::new(),
        url: "data:image/png;base64,AA".into(),
        mime_type: "image/png".into(),
    };
    assert_eq!(
        data.remote_photo_url(),
        None,
        "a data: URI is never fetched"
    );
}

// ------------------------------------------------------------------------------------ profile

#[test]
fn default_profile_matches_make_default_social_state() {
    let p = SocialProfile::new_default(code("ABCD-WXYZ"));
    assert_eq!(p.display_name, "Student WXYZ");
    assert_eq!(
        p.avatar,
        Avatar::Letter {
            letter: "S".into(),
            style: AvatarStyle::Classic
        }
    );
    assert!(!p.is_private && !p.auto_post_sessions && p.show_hours_to_friends);
}

#[test]
fn display_names_trim_and_cut_at_48_utf16_units() {
    assert_eq!(clean_display_name("  Ada  ").unwrap(), "Ada");
    assert!(clean_display_name("   ").is_err());
    assert_eq!(clean_display_name(&"a".repeat(60)).unwrap().len(), 48);
    let emoji = "🦊".repeat(30); // 60 UTF-16 units
    assert_eq!(clean_display_name(&emoji).unwrap().chars().count(), 24);
    let edge = format!("{}🦊", "a".repeat(47)); // the fox would straddle unit 48
    assert_eq!(clean_display_name(&edge).unwrap(), "a".repeat(47));
}

// ------------------------------------------------------------------------------------ friends

#[test]
fn friend_request_preflight_follows_production_order_and_messages() {
    let own = code("SELF-CODE");
    let snapshot = FriendsSnapshot {
        friends: vec![friend("f1", "Friend", "FRND-AAAA")],
        incoming: vec![],
        outgoing: vec![request("r1", "PEND-AAAA")],
    };
    let check = |d: &str| check_friend_request(d, &own, &snapshot);
    assert_eq!(check("  "), Err(FriendRequestRejection::Empty));
    assert_eq!(check("self-code"), Err(FriendRequestRejection::OwnCode));
    assert_eq!(
        check("frnd-aaaa"),
        Err(FriendRequestRejection::AlreadyFriends)
    );
    assert_eq!(
        check("PEND-AAAA"),
        Err(FriendRequestRejection::AlreadyPending)
    );
    assert_eq!(check(" new2-code ").unwrap().as_str(), "NEW2-CODE");
    assert_eq!(
        FriendRequestRejection::AlreadyPending.message(),
        "Friend request already pending."
    );
    assert_eq!(FriendResponse::Accepted.wire(), "accepted");
    assert_eq!(
        FriendResponse::Declined.done_message(),
        "Friend request declined."
    );
}

#[test]
fn snapshots_drop_duplicate_ids() {
    let s = FriendsSnapshot {
        friends: vec![
            friend("f1", "A", "AAAA-AAAA"),
            friend("f1", "A again", "AAAA-AAAA"),
            friend("f2", "B", "BBBB-BBBB"),
        ],
        incoming: vec![request("r1", "X"), request("r1", "X")],
        outgoing: vec![],
    }
    .deduplicated();
    assert_eq!(s.friends.len(), 2);
    assert_eq!(s.friends[0].display_name, "A", "first wins");
    assert_eq!(s.incoming.len(), 1);
    assert_eq!(s.incoming_count(), 1);
}

// -------------------------------------------------------------------------------- leaderboards

#[test]
fn leaderboards_filter_sort_rank_and_mark_self() {
    let me = uid("me");
    let friends = FriendsSnapshot {
        friends: vec![friend("f1", "Friend", "FRND-AAAA")],
        ..Default::default()
    };
    let mut self_row = entry("me", "Me", 30);
    self_row.is_self = true;
    let rows = vec![
        entry("stranger", "Zed", 500),
        entry("f1", "beth", 30),
        self_row,
        entry("f2", "Ann", 30),
    ];
    let friends_view = ranked_for_scope(&rows, LeaderboardScope::Friends, &me, &friends, &[]);
    assert_eq!(
        friends_view
            .iter()
            .map(|e| e.display_name.as_str())
            .collect::<Vec<_>>(),
        ["beth", "Me"],
        "tie broken by name; strangers dropped"
    );
    assert_eq!(
        friends_view.iter().map(|e| e.rank).collect::<Vec<_>>(),
        [1, 2]
    );
    assert!(friends_view[1].is_self && !friends_view[0].is_self);
    let global = ranked_for_scope(&rows, LeaderboardScope::Global, &me, &friends, &[]);
    assert_eq!(
        global
            .iter()
            .map(|e| (e.display_name.as_str(), e.rank))
            .collect::<Vec<_>>(),
        [("Zed", 1), ("Ann", 2), ("beth", 3), ("Me", 4)]
    );
    let squad = ranked_for_scope(&rows, LeaderboardScope::Squad, &me, &friends, &[uid("f2")]);
    assert_eq!(
        squad
            .iter()
            .map(|e| e.display_name.as_str())
            .collect::<Vec<_>>(),
        ["Ann", "Me"]
    );
    // server minutes are used as-is (no recomputation): 0022's corrected totals pass through
    assert_eq!(global[0].minutes, 500);
}

#[test]
fn field_notebook_bars_follow_production_formula() {
    let rows = [entry("a", "A", 300), entry("b", "B", 6), entry("c", "C", 0)];
    let top = top_minutes(&rows);
    assert_eq!(top, 300);
    assert_eq!(bar_percent(300, top), 100);
    assert_eq!(bar_percent(6, top), 4, "at least 4 %");
    assert_eq!(bar_percent(0, top), 0);
    assert_eq!(top_minutes(&[]), 1);
    assert_eq!(bar_percent(150, 300), 50);
}

// ---------------------------------------------------------------------------------- own stats

fn session(id: &str, end: &str, minutes: u32, kind: SessionKind) -> StudySession {
    let ended = ts(end).wall();
    StudySession {
        id: SessionId::new(id),
        semester_id: None,
        course_id: None,
        task_id: None,
        kind,
        goal: String::new(),
        learned: String::new(),
        blocker: String::new(),
        next_step: String::new(),
        confidence: 3,
        started_at: WallTimestamp::from_unix_millis(
            ended.unix_millis - i64::from(minutes) * 60_000,
        ),
        ended_at: ended,
        minutes,
        preset_label: String::new(),
    }
}

#[test]
fn own_stats_match_production_derivations() {
    let clock = FixedOffsetClock::new(2 * 3600);
    let mut state = AcademicState::new();
    state.sessions = vec![
        session("a", "2026-09-30T08:00:00Z", 50, SessionKind::Study), // Wed
        session("b", "2026-09-29T21:30:00Z", 25, SessionKind::Exam),  // Tue 23:30 local
        session("c", "2026-09-29T22:30:00Z", 10, SessionKind::Study), // Wed 00:30 local
        session("d", "2026-09-27T10:00:00Z", 40, SessionKind::Study), // Sun: last week
        session("e", "2026-09-30T09:00:00Z", 15, SessionKind::Break), // never counted, but is "last active"
        session("f", "2026-08-31T21:59:00Z", 5, SessionKind::Study),  // Aug 31 23:59 local
    ];
    state.lifetime_study_minutes = 9999;
    state.lifetime_study_sessions = 77;
    let now = ts("2026-09-30T10:00:00Z").wall();
    let daily = period_stat(&state, LeaderboardPeriod::Daily, now, &clock);
    assert_eq!((daily.minutes, daily.sessions), (60, 2));
    assert_eq!(daily.last_active, CivilDate::from_ymd(2026, 9, 30));
    let weekly = period_stat(&state, LeaderboardPeriod::Weekly, now, &clock);
    assert_eq!(
        (weekly.minutes, weekly.sessions),
        (85, 3),
        "Monday 00:00 local onwards"
    );
    let overall = period_stat(&state, LeaderboardPeriod::Overall, now, &clock);
    assert_eq!(
        (overall.minutes, overall.sessions),
        (9999, 77),
        "lifetime running totals"
    );
    let month = monthly_stat(&state, now, &clock);
    assert_eq!((month.minutes, month.sessions), (125, 4));
    let rows = daily_stats(&state, &clock);
    assert_eq!(
        rows.iter()
            .map(|r| (r.date.to_iso(), r.minutes, r.sessions))
            .collect::<Vec<_>>(),
        [
            ("2026-08-31".to_string(), 5, 1),
            ("2026-09-27".to_string(), 40, 1),
            ("2026-09-29".to_string(), 25, 1),
            ("2026-09-30".to_string(), 60, 2),
        ]
    );
    // the sync payload keeps the newest 370 days
    let mut big = AcademicState::new();
    big.sessions = (0..400)
        .map(|i| {
            session(
                &format!("s{i}"),
                &CivilDate::from_ymd(2025, 1, 1)
                    .unwrap()
                    .add_days(i)
                    .to_iso(),
                1,
                SessionKind::Study,
            )
        })
        .collect();
    let synced = sync_stats(&big, &FixedOffsetClock::UTC);
    assert_eq!(synced.len(), 370);
    assert_eq!(
        synced.last().unwrap().date,
        CivilDate::from_ymd(2025, 1, 1).unwrap().add_days(399)
    );
}
