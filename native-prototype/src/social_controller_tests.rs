//! Social end to end, headless: controller -> real `UreqTransport` -> `social-mock` -> controller.

use social_mock::{seed, Fault, MockServer};
use study_tracker_core::academic::{AcademicState, SessionId, SessionKind, StudySession};
use study_tracker_core::dashboard::civil::FixedOffsetClock;
use study_tracker_core::social::friends::FriendResponse;
use study_tracker_core::social::{DeviceSecret, FriendCode, SocialIdentity, UserId};
use study_tracker_core::timer::WallTimestamp;

use super::*;
use crate::net::device::{device_identity_for, AppMetadata};
use crate::net::endpoint::{LocalEndpoint, PRODUCTION_ORIGIN};
use crate::net::transport::{Transport, UreqTransport};
use crate::persistence::social_credentials::MemoryCredentialStore;
use crate::persistence::social_port::MemorySocialPort;

const NOW: i64 = 1_791_108_000_000; // 2026-10-04 12:00 Zurich

struct Env {
    academic: AcademicState,
    clock: FixedOffsetClock,
    device: DeviceIdentity,
    app: AppMetadata,
}

impl Env {
    fn new() -> Self {
        let mut academic = AcademicState::new();
        academic.sessions.push(StudySession {
            id: SessionId::new("s1"),
            semester_id: None,
            course_id: None,
            task_id: None,
            kind: SessionKind::Study,
            goal: String::new(),
            learned: String::new(),
            blocker: String::new(),
            next_step: String::new(),
            confidence: 3,
            started_at: WallTimestamp::from_unix_millis(NOW - 3_000_000),
            ended_at: WallTimestamp::from_unix_millis(NOW - 600_000),
            minutes: 40,
            preset_label: String::new(),
        });
        academic.lifetime_study_minutes = 40;
        academic.lifetime_study_sessions = 1;
        Self {
            academic,
            clock: FixedOffsetClock::new(7200),
            device: device_identity_for("synthetic-machine"),
            app: AppMetadata {
                version: "0.1.0".into(),
                platform: "Linux x86_64".into(),
                runtime_channel: "development".into(),
            },
        }
    }

    fn ctx(&self) -> SyncContext<'_> {
        SyncContext {
            academic: &self.academic,
            clock: &self.clock,
            device: &self.device,
            app: &self.app,
        }
    }
}

struct Rig {
    server: MockServer,
    transport: UreqTransport,
    endpoint: SocialEndpoint,
    env: Env,
}

impl Rig {
    fn new(world: social_mock::World) -> Self {
        let server = MockServer::start(0, world).unwrap();
        seed::bind_origin(&mut server.world.lock().unwrap());
        let endpoint =
            SocialEndpoint::Test(LocalEndpoint::new("127.0.0.1", server.port()).unwrap());
        Self {
            server,
            transport: UreqTransport::new(),
            endpoint,
            env: Env::new(),
        }
    }

    fn drive(&mut self, c: &mut SocialController, mut out: Vec<Outgoing>) -> usize {
        let mut n = 0;
        while let Some(o) = out.pop() {
            n += 1;
            assert!(n < 200, "runaway request loop");
            let r = if o.cancel.is_cancelled() {
                Err(NetError::Cancelled)
            } else {
                self.transport.execute(&self.endpoint.origin(), &o.request)
            };
            out.extend(c.on_reply(
                o.token,
                NetReply::from_result(r, o.post),
                WallTimestamp::from_unix_millis(NOW),
                &self.env.ctx(),
            ));
        }
        n
    }
}

fn synthetic_identity() -> SocialIdentity {
    SocialIdentity {
        user_id: UserId::parse(seed::SELF_ID).unwrap(),
        device_secret: DeviceSecret::parse(seed::SELF_SECRET).unwrap(),
    }
}

fn synthetic_record() -> SocialRecord {
    let mut profile = SocialProfile::new_default(FriendCode::parse(seed::SELF_CODE).unwrap());
    profile.display_name = seed::SELF_NAME.into();
    SocialRecord {
        user_id: synthetic_identity().user_id,
        profile,
        sync: SyncStatus::default(),
        friends: FriendsSnapshot::default(),
        leaderboards: vec![],
    }
}

fn existing(rig: &Rig) -> SocialController {
    let creds = MemoryCredentialStore {
        stored: Some(StoredCredential {
            identity: synthetic_identity(),
            endpoint: EndpointClass::LocalTest,
        }),
        fail_saves: false,
    };
    let port = MemorySocialPort {
        record: Some(synthetic_record()),
        writes: 0,
    };
    SocialController::new(
        Some(rig.endpoint.clone()),
        Box::new(creds),
        Box::new(port),
        false,
    )
}

fn none(rig: &Rig) -> SocialController {
    SocialController::new(
        Some(rig.endpoint.clone()),
        Box::new(MemoryCredentialStore::default()),
        Box::new(MemorySocialPort::default()),
        false,
    )
}

fn now() -> WallTimestamp {
    WallTimestamp::from_unix_millis(NOW)
}

#[test]
fn no_identity_makes_zero_requests_from_every_trigger() {
    let rig = Rig::new(seed::demo(NOW, false));
    let mut c = none(&rig);
    let ctx = rig.env.ctx();
    let app = rig.env.app.clone();
    let mut out = Vec::new();
    out.extend(c.on_startup(now(), &ctx));
    out.extend(c.on_hourly(now(), &ctx));
    out.extend(c.on_sessions_changed(now(), &ctx));
    out.extend(c.tab_opened(&app, now(), &ctx));
    out.extend(c.on_status_poll());
    for sub in Subtab::ALL {
        out.extend(c.set_subtab(sub, &app, now(), &ctx));
    }
    out.extend(c.set_scope(LeaderboardScope::Global));
    out.extend(c.set_period(LeaderboardPeriod::Daily));
    out.extend(c.manual_sync(now(), &ctx));
    out.extend(c.send_friend_request("ABCD-2345"));
    out.extend(c.save_name("New name", now(), &ctx));
    out.extend(c.toggle_private(now(), &ctx));
    out.extend(c.open_profile(
        UserId::parse("x").unwrap(),
        "X".into(),
        None,
        Avatar::default_for("X"),
    ));
    assert!(out.is_empty(), "{out:?}");
    assert_eq!(c.requests_made, 0);
    assert!(rig.server.log().is_empty(), "the server saw nothing");
    assert!(!c.active() && c.identity().is_none());
}

#[test]
fn creating_an_account_is_explicit_bootstraps_and_persists_only_on_success() {
    let mut rig = Rig::new(seed::empty(NOW));
    let mut c = none(&rig);
    assert!(
        c.create_account([7; 40], now(), &rig.env.ctx()).len() == 1,
        "confirmed creation sends the bootstrap"
    );
    assert!(matches!(c.phase, IdentityPhase::NewIdentity { .. }));
    assert!(
        c.identity().is_none(),
        "a candidate is not an established identity"
    );
    let out = c.create_account([7; 40], now(), &rig.env.ctx());
    assert!(out.is_empty(), "no second bootstrap while one is running");
    // drive the first one: re-create to get its Outgoing
    let mut c = none(&rig);
    c.ask_new_account();
    assert!(c.confirm_new_account);
    let out = c.create_account([9; 40], now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert!(c.active(), "{:?}", c.message);
    assert_eq!(c.message.as_deref(), Some("Social account created."));
    let profile = c.profile().unwrap();
    assert!(profile.friend_code.is_standard());
    assert!(is_default_name(&profile.display_name));
    assert_eq!(
        rig.server.world.lock().unwrap().users.len(),
        1,
        "the account exists on the (mock) server"
    );
    // the default name opens production's "Set your name" prompt on the Social tab
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    assert!(c.name_prompt_open);
    rig.drive(&mut c, out);
}

#[test]
fn a_failed_bootstrap_rolls_back_completely() {
    for fault in [
        Fault::Status(500, b"boom".to_vec()),
        Fault::Status(409, b"Friend code is already in use.".to_vec()),
        Fault::Drop,
        Fault::Body(b"not json".to_vec()),
    ] {
        let mut rig = Rig::new(seed::empty(NOW));
        rig.server.fault("/sync/v2", fault.clone());
        let mut c = none(&rig);
        let out = c.create_account([3; 40], now(), &rig.env.ctx());
        rig.drive(&mut c, out);
        assert!(matches!(c.phase, IdentityPhase::NoIdentity), "{fault:?}");
        assert!(c.record.is_none());
        assert!(
            c.credentials.load().unwrap().is_none(),
            "nothing half-created is kept"
        );
        assert!(c.message.is_some());
        assert_eq!(c.port_writes(), 0);
    }
}

#[test]
fn an_unsavable_credential_is_not_kept_half_created() {
    let mut rig = Rig::new(seed::empty(NOW));
    let creds = MemoryCredentialStore {
        stored: None,
        fail_saves: true,
    };
    let mut c = SocialController::new(
        Some(rig.endpoint.clone()),
        Box::new(creds),
        Box::new(MemorySocialPort::default()),
        false,
    );
    let out = c.create_account([5; 40], now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert!(matches!(c.phase, IdentityPhase::NoIdentity));
    assert!(c.record.is_none());
}

#[test]
fn credentials_for_another_endpoint_class_are_not_used() {
    let rig = Rig::new(seed::demo(NOW, false));
    let creds = MemoryCredentialStore {
        stored: Some(StoredCredential {
            identity: synthetic_identity(),
            endpoint: EndpointClass::LocalTest,
        }),
        fail_saves: false,
    };
    let mut c = SocialController::new(
        Some(SocialEndpoint::Production),
        Box::new(creds),
        Box::new(MemorySocialPort {
            record: Some(synthetic_record()),
            writes: 0,
        }),
        false,
    );
    assert!(
        c.identity().is_none(),
        "a local-test identity never talks to production"
    );
    assert!(c.on_startup(now(), &rig.env.ctx()).is_empty());
    assert_eq!(c.origin().unwrap().to_string(), PRODUCTION_ORIGIN);
}

#[test]
fn startup_follows_productions_schedule() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let out = c.on_startup(now(), &rig.env.ctx());
    assert_eq!(out.len(), 2, "presence + the due auto-sync");
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/presence"), 1);
    assert_eq!(rig.server.count("/sync/v2"), 1);
    assert_eq!(
        rig.server.count("/friends/status/v2"),
        1,
        "the status refresh after a sync"
    );
    let sync = c.sync_status().unwrap().clone();
    assert!(sync.last_synced_at.is_some() && sync.last_sync_error.is_none());
    assert!(sync.next_auto_sync_at.unwrap().0 > NOW);
    assert_eq!(c.friends().unwrap().friends.len(), 4);
    assert_eq!(c.incoming_count(), 1);
    assert!(
        c.has_unread,
        "a sync while Social is not open shows the dot"
    );
    // the next start before the auto-sync is due: presence only
    let out = c.on_startup(now(), &rig.env.ctx());
    assert_eq!(out.len(), 1);
    rig.drive(&mut c, out);
    // the hourly check before the due time: nothing
    assert!(c.on_hourly(now(), &rig.env.ctx()).is_empty());
}

#[test]
fn the_social_tab_refreshes_and_the_leaderboard_follows_scope_and_period() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    assert!(
        !c.has_unread && !c.name_prompt_open,
        "a real name: no prompt"
    );
    rig.drive(&mut c, out);
    assert_eq!(
        rig.server.count("/friends/status/v2"),
        1,
        "status at once; the Feed subtab effect's identical refresh is coalesced into it"
    );
    assert_eq!(c.coalesced, 1);
    assert_eq!(rig.server.count("/presence"), 1);
    let out = c.set_subtab(Subtab::Leaderboard, &app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(rig.server.count("/leaderboard"), 1);
    assert!(
        c.has_unread,
        "production's quirk: a sync sets the dot even with Social open"
    );
    assert_eq!(
        rig.server.count("/sync/v2"),
        1,
        "opening the Leaderboard syncs"
    );
    let board = c.board(LeaderboardScope::Friends, LeaderboardPeriod::Weekly);
    assert_eq!(
        board
            .iter()
            .map(|e| e.display_name.as_str())
            .collect::<Vec<_>>(),
        ["Bob", "Sam Synthetic", "张伟 Zoë 🦊", "Amélie", "שלום עולם"]
    );
    assert!(board[1].is_self && board[1].rank == 2);
    let out = c.set_scope(LeaderboardScope::Global);
    rig.drive(&mut c, out);
    let global = c.board(LeaderboardScope::Global, LeaderboardPeriod::Weekly);
    assert!(global.iter().any(|e| e.display_name == "Kenji 健二"));
    assert!(!global.iter().any(|e| e.display_name == "Dan Private"));
    assert!(
        c.set_scope(LeaderboardScope::Squad).is_empty(),
        "the squad scoreboard is Stage 22b"
    );
    let out = c.set_period(LeaderboardPeriod::Overall);
    assert!(out.is_empty(), "squad scope makes no leaderboard request");
    let out = c.set_scope(LeaderboardScope::Global);
    rig.drive(&mut c, out);
    assert_eq!(
        c.board(LeaderboardScope::Global, LeaderboardPeriod::Overall)[0].display_name,
        "A very long display name that goes on and on and"
    );
    // polling only while the tab is visible
    assert_eq!(c.on_status_poll().len(), 1);
    c.tab_closed();
    assert!(c.on_status_poll().is_empty());
}

#[test]
fn friend_requests_respond_and_errors() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let out = c.on_startup(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    // local checks never reach the server
    for (draft, message) in [
        ("", "Enter a friend code first."),
        ("synt-2345", "That is your own friend code."),
        ("BOBB-2345", "You are already friends."),
        ("PRYA-2345", "Friend request already pending."),
    ] {
        assert!(c.send_friend_request(draft).is_empty());
        assert_eq!(c.message.as_deref(), Some(message));
    }
    let out = c.send_friend_request("dann-2345");
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Friend request sent."));
    assert!(c
        .friends()
        .unwrap()
        .outgoing
        .iter()
        .any(|r| r.to_friend_code.as_str() == "DANN-2345"));
    let out = c.send_friend_request("ZZZZ-9999");
    rig.drive(&mut c, out);
    assert_eq!(
        c.message.as_deref(),
        Some("No user with that friend code exists.")
    );
    // accept Kenji's request; the reply's snapshot replaces the lists
    let id = c.friends().unwrap().incoming[0].id.clone();
    let out = c.respond(&id, FriendResponse::Accepted);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Friend request accepted."));
    assert_eq!(c.friends().unwrap().friends.len(), 5);
    assert!(c.friends().unwrap().incoming.is_empty());
    // answering it again: the server's 404, no duplicate friend
    let out = c.respond(&id, FriendResponse::Accepted);
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("Friend request not found."));
    assert_eq!(c.friends().unwrap().friends.len(), 5);
}

#[test]
fn profile_edits_persist_and_sync_with_productions_messages() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let out = c.save_name("   ", now(), &rig.env.ctx());
    assert!(out.is_empty());
    assert_eq!(c.message.as_deref(), Some("Give your player a name first."));
    let out = c.save_name("  Zoë 学生 🦊  ", now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert_eq!(c.profile().unwrap().display_name, "Zoë 学生 🦊");
    assert_eq!(
        rig.server.world.lock().unwrap().users[seed::SELF_ID].display_name,
        "Zoë 学生 🦊"
    );
    let out = c.toggle_private(now(), &rig.env.ctx());
    assert_eq!(c.message.as_deref(), Some("Profile set to private."));
    rig.drive(&mut c, out);
    assert!(rig.server.world.lock().unwrap().users[seed::SELF_ID].is_private);
    let out = c.toggle_show_hours(now(), &rig.env.ctx());
    assert_eq!(
        c.message.as_deref(),
        Some("Your study hours are hidden from friends.")
    );
    rig.drive(&mut c, out);
    let before = rig.server.log().len();
    c.toggle_auto_post();
    assert_eq!(c.message.as_deref(), Some("Auto-post enabled."));
    assert_eq!(rig.server.log().len(), before, "auto-post is local only");
}

#[test]
fn the_profile_dialog_loads_player_stats() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let out = c.on_startup(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let bob = c
        .friends()
        .unwrap()
        .friends
        .iter()
        .find(|f| f.display_name == "Bob")
        .unwrap()
        .clone();
    let out = c.open_profile(
        bob.user_id.clone(),
        bob.display_name.clone(),
        Some(bob.friend_code.clone()),
        bob.avatar.clone(),
    );
    assert!(c.viewing.as_ref().unwrap().loading);
    rig.drive(&mut c, out);
    let v = c.viewing.as_ref().unwrap();
    assert!(!v.loading && v.stats.as_ref().unwrap().hours_visible);
    // a stranger: production's "User is private." message and "Could not load stats."
    let out = c.open_profile(
        UserId::parse("synthetic-user-kenji").unwrap(),
        "Kenji".into(),
        None,
        Avatar::default_for("Kenji"),
    );
    rig.drive(&mut c, out);
    assert_eq!(c.message.as_deref(), Some("User is private."));
    assert!(c.viewing.as_ref().unwrap().stats.is_none());
    // self: computed locally, no request
    assert!(c
        .open_profile(
            synthetic_identity().user_id,
            "Me".into(),
            None,
            Avatar::default_for("Me")
        )
        .is_empty());
    // a late reply for a dialog that was closed and reopened for someone else is ignored
    let out = c.open_profile(bob.user_id.clone(), "Bob".into(), None, bob.avatar.clone());
    c.close_profile();
    rig.drive(&mut c, out);
    assert!(c.viewing.is_none());
}

#[test]
fn offline_sync_records_the_error_and_waits_for_the_schedule() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    rig.server.fault("/sync/v2", Fault::Drop);
    let mut c = existing(&rig);
    let out = c.manual_sync(now(), &rig.env.ctx());
    assert!(
        c.manual_sync(now(), &rig.env.ctx()).is_empty(),
        "one sync at a time"
    );
    rig.drive(&mut c, out);
    let sync = c.sync_status().unwrap();
    assert!(sync.last_sync_error.is_some());
    assert!(sync.last_synced_at.is_none());
    assert_eq!(c.message.as_deref(), Some("You're offline or the Social server can't be reached. Try again when you're connected."));
    assert!(
        c.on_hourly(now(), &rig.env.ctx()).is_empty(),
        "no retry before the next scheduled time"
    );
    assert!(!c.syncing);
}

#[test]
fn a_credential_without_a_profile_restores_it_from_the_server() {
    let mut rig = Rig::new(seed::demo(NOW, false));
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
        Box::new(MemorySocialPort::default()),
        false,
    );
    assert!(c.identity().is_some() && !c.active() && c.restoring_profile);
    let out = c.on_startup(now(), &rig.env.ctx());
    assert_eq!(out.len(), 1, "only the profile read first");
    rig.drive(&mut c, out);
    assert!(c.active());
    assert_eq!(c.profile().unwrap().friend_code.as_str(), seed::SELF_CODE);
    assert_eq!(c.profile().unwrap().display_name, seed::SELF_NAME);
}

#[test]
fn a_failed_profile_restore_says_so_and_try_again_recovers() {
    let mut rig = Rig::new(seed::demo(NOW, false));
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
        Box::new(MemorySocialPort::default()),
        false,
    );
    rig.server.fault("/player-stats", Fault::Drop);
    let out = c.on_startup(now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    assert!(c.restoring_profile && !c.active());
    assert!(
        c.restore_error.is_some(),
        "the failure is shown, not an endless 'One moment.'"
    );
    // "Try again" (the notice's button runs the manual sync path)
    let out = c.manual_sync(now(), &rig.env.ctx());
    assert_eq!(out.len(), 1, "the profile read again, nothing else yet");
    assert!(c.restore_error.is_none());
    rig.drive(&mut c, out);
    assert!(c.active() && !c.restoring_profile);
}

#[test]
fn nothing_is_written_when_nothing_changed() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    let out = c.tab_opened(&app, now(), &rig.env.ctx());
    rig.drive(&mut c, out);
    let writes = c.port_writes();
    for _ in 0..50 {
        let out = c.on_status_poll();
        rig.drive(&mut c, out);
    }
    assert_eq!(
        c.port_writes(),
        writes,
        "identical snapshots are not persisted again"
    );
}

#[test]
fn navigation_and_reconnect_stress_stays_bounded() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = existing(&rig);
    let app = rig.env.app.clone();
    for i in 0..500 {
        let out = c.tab_opened(&app, now(), &rig.env.ctx());
        if i % 25 == 0 {
            rig.drive(&mut c, out);
        } else {
            for o in out {
                o.cancel.cancel();
                c.on_reply(
                    o.token,
                    NetReply::Http(Err(NetError::Cancelled)),
                    now(),
                    &rig.env.ctx(),
                );
            }
        }
        c.tab_closed();
    }
    assert_eq!(c.pending_count(), 0);
    // 100 disconnect/reconnect cycles: dropped responses then normal ones
    for i in 0..100 {
        if i % 2 == 0 {
            rig.server.fault("/friends/status/v2", Fault::Drop);
        }
        c.tab_opened(&app, now(), &rig.env.ctx())
            .into_iter()
            .for_each(|o| {
                let r = rig.transport.execute(&rig.endpoint.origin(), &o.request);
                c.on_reply(
                    o.token,
                    NetReply::from_result(r, o.post),
                    now(),
                    &rig.env.ctx(),
                );
            });
        c.tab_closed();
    }
    assert_eq!(c.pending_count(), 0);
    let f = c.friends().unwrap();
    assert_eq!(
        f.friends.len(),
        4,
        "no duplicated Social state after reconnects"
    );
    assert_eq!(f.incoming.len(), 1);
    let (_, open, max_open) = rig.server.connections();
    assert!(
        max_open <= 4,
        "the transport pools connections; open now {open}, most at once {max_open}"
    );
}
