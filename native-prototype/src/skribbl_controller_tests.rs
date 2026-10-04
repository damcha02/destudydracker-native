//! Daily Skribbl end to end, headless: controller -> real `UreqTransport` -> `social-mock` on
//! loopback -> reply -> controller. Deterministic (synchronous driver, injected clocks).

use std::time::Duration;

use social_mock::{seed, Fault, MockServer};
use study_tracker_core::break_room::skribbl::{Phase, DRAW_SECONDS};
use study_tracker_core::social::{DeviceSecret, SocialIdentity, UserId};

use super::*;
use crate::net::endpoint::{LocalEndpoint, SocialEndpoint};
use crate::net::transport::{Transport, UreqTransport};

/// 2026-10-04 12:00 Europe/Zurich.
const NOW: i64 = 1_791_108_000_000;

struct Rig {
    server: MockServer,
    transport: UreqTransport,
    origin: Origin,
    requests: usize,
}

impl Rig {
    fn new(world: social_mock::World) -> Self {
        let server = MockServer::start(0, world).unwrap();
        seed::bind_origin(&mut server.world.lock().unwrap());
        let origin =
            SocialEndpoint::Test(LocalEndpoint::new("127.0.0.1", server.port()).unwrap()).origin();
        Self {
            server,
            transport: UreqTransport::new(),
            origin,
            requests: 0,
        }
    }

    /// Runs requests (and the requests their replies cause) until none remain.
    fn drive(&mut self, c: &mut SkribblController, mut out: Vec<Outgoing>) {
        let mut guard = 0;
        while let Some(o) = out.pop() {
            guard += 1;
            assert!(guard < 500, "runaway request loop");
            self.requests += 1;
            let result = if o.cancel.is_cancelled() {
                Err(NetError::Cancelled)
            } else {
                self.transport.execute(&self.origin, &o.request)
            };
            out.extend(c.on_reply(o.token, NetReply::from_result(result, o.post)));
        }
    }
}

fn me() -> SocialIdentity {
    SocialIdentity {
        user_id: UserId::parse(seed::SELF_ID).unwrap(),
        device_secret: DeviceSecret::parse(seed::SELF_SECRET).unwrap(),
    }
}

#[test]
fn without_an_identity_the_modal_makes_no_request() {
    let mut c = SkribblController::new();
    let out = c.open(None, None);
    assert!(out.is_empty());
    assert_eq!(c.phase(), Some(Phase::Intro));
    assert!(!c.session.as_ref().unwrap().configured);
    c.start_drawing(0);
    assert_eq!(c.phase(), Some(Phase::Intro));
    assert!(c.submit().is_empty() && c.retry().is_empty() && c.load_more().is_empty());
}

#[test]
fn fresh_day_draw_submit_gallery_vote() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = SkribblController::new();
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    let s = c.session.as_ref().unwrap();
    assert_eq!(
        (s.phase, s.theme.as_str(), s.theme_date.as_str()),
        (Phase::Intro, "Lighthouse in the fog", "2026-10-04")
    );
    assert_eq!(
        s.winner
            .as_ref()
            .map(|w| (w.display_name.as_str(), w.score)),
        Some(("Amélie", 4))
    );
    assert!(s.gallery.is_empty(), "the gallery unlocks after submitting");
    c.start_drawing(0);
    assert_eq!(c.phase(), Some(Phase::Drawing));
    c.canvas_mut().begin_stroke(100.0, 100.0, 10, 0x1e88e5);
    c.canvas_mut().extend_stroke(600.0, 400.0, 10, 0x1e88e5);
    c.canvas_mut().end_stroke();
    let out = c.submit();
    assert_eq!(out.len(), 1);
    assert!(c.submit().is_empty(), "double click submits once");
    rig.drive(&mut c, out);
    let s = c.session.as_ref().unwrap();
    assert_eq!(s.phase, Phase::Submitted);
    assert_eq!(s.gallery.len(), 7, "six others + mine");
    assert!(s.gallery.iter().any(|d| d.is_self));
    assert_eq!(rig.server.count("/skribbl/submit"), 1);
    // thumbnails were fetched and decoded for every row plus the own drawing (same URL)
    assert!(c.thumbs.len() >= 7);
    let own_url = s.my_image_url.clone().unwrap();
    assert!(matches!(
        c.thumbs.state(&own_url),
        Some(crate::image_cache::ImageState::Ready(_))
    ));
    // vote on someone else's drawing; the server's score wins
    let target = s
        .gallery
        .iter()
        .find(|d| !d.is_self && d.vote_score == 0)
        .unwrap()
        .id
        .clone();
    let out = c.vote(&target, 1);
    rig.drive(&mut c, out);
    let row = c
        .session
        .as_ref()
        .unwrap()
        .gallery
        .iter()
        .find(|d| d.id == target)
        .unwrap()
        .clone();
    assert_eq!((row.my_vote, row.vote_score), (1, 1));
    // reopening after submitting: straight to the gallery, the server remembers
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    let s = c.session.as_ref().unwrap();
    assert!(s.submitted && s.gallery.len() == 7 && s.phase == Phase::Intro);
}

#[test]
fn a_second_submission_on_the_same_day_is_refused_by_the_server() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut a = SkribblController::new();
    let out = a.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut a, out);
    a.start_drawing(0);
    // another device of the same account submits first
    let mut b = SkribblController::new();
    let out = b.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut b, out);
    b.start_drawing(0);
    let out = b.submit();
    rig.drive(&mut b, out);
    let out = a.submit();
    rig.drive(&mut a, out);
    let s = a.session.as_ref().unwrap();
    assert_eq!(s.phase, Phase::Drawing, "the drawing is kept");
    assert_eq!(
        s.error.as_deref(),
        Some("You already submitted a drawing today.")
    );
}

#[test]
fn the_deadline_submits_exactly_once_even_after_a_long_hidden_period() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = SkribblController::new();
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    c.start_drawing(1_000);
    assert!(c.tick(1_000 + DRAW_SECONDS * 1000 - 1).is_empty());
    let out = c.tick(1_000 + 3_600_000); // the window was minimized for an hour
    assert_eq!(out.len(), 1);
    rig.drive(&mut c, out);
    assert_eq!(c.phase(), Some(Phase::Submitted));
    for t in 0..50 {
        assert!(c.tick(5_000_000 + t).is_empty());
    }
    assert_eq!(rig.server.count("/skribbl/submit"), 1);
}

#[test]
fn offline_timeout_and_server_errors_fail_gracefully_without_retry_loops() {
    // offline: nothing listens
    let mut c = SkribblController::new();
    let dead = Origin {
        secure: false,
        host: "127.0.0.1".into(),
        port: 9,
    };
    let out = c.open(Some(me()), Some(dead.clone()));
    let transport = UreqTransport::new();
    let mut requests = 0;
    let mut queue = out;
    while let Some(o) = queue.pop() {
        requests += 1;
        queue.extend(c.on_reply(
            o.token,
            NetReply::from_result(transport.execute(&dead, &o.request), o.post),
        ));
    }
    assert_eq!(requests, 1, "one theme request, no automatic retry");
    let s = c.session.as_ref().unwrap();
    assert_eq!(
        (s.phase, s.error.as_deref()),
        (
            Phase::Intro,
            Some("Could not reach the Daily Skribbl server.")
        )
    );
    // the user can still try again, which is one request again
    assert_eq!(c.retry().len(), 1);

    // a 5xx never shows its body; a missing route maps to production's sentence
    let mut rig = Rig::new(seed::demo(NOW, false));
    rig.server.fault(
        "/skribbl/theme",
        Fault::Status(500, b"D1_ERROR: internal".to_vec()),
    );
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    assert_eq!(
        c.session.as_ref().unwrap().error.as_deref(),
        Some("The Social server had a problem. Try again later.")
    );
    rig.server
        .fault("/skribbl/theme", Fault::Status(404, b"Not found.".to_vec()));
    let out = c.retry();
    rig.drive(&mut c, out);
    assert_eq!(c.session.as_ref().unwrap().error.as_deref(), Some("Daily Skribbl isn't live on the server yet. If this keeps happening, re-deploy the worker."));
    // a malformed theme
    rig.server.fault(
        "/skribbl/theme",
        Fault::Body(b"{\"date\":\"nope\"".to_vec()),
    );
    let out = c.retry();
    rig.drive(&mut c, out);
    assert!(c.session.as_ref().unwrap().error.is_some());
    // a dropped response (connection closed)
    rig.server.fault("/skribbl/theme", Fault::Drop);
    let out = c.retry();
    rig.drive(&mut c, out);
    assert_eq!(
        c.session.as_ref().unwrap().error.as_deref(),
        Some("Could not reach the Daily Skribbl server.")
    );
}

#[test]
fn a_slow_server_times_out_within_the_requests_bound() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = SkribblController::new();
    let mut out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.server
        .fault("/skribbl/theme", Fault::Delay(Duration::from_millis(1500)));
    for o in &mut out {
        o.request.timeout = Duration::from_millis(300);
    }
    let started = std::time::Instant::now();
    rig.drive(&mut c, out);
    assert!(started.elapsed() < Duration::from_millis(1400));
    assert_eq!(
        c.session.as_ref().unwrap().error.as_deref(),
        Some("Could not reach the Daily Skribbl server.")
    );
}

#[test]
fn closing_cancels_in_flight_work_and_late_replies_change_nothing() {
    let rig = Rig::new(seed::demo(NOW, true));
    let mut c = SkribblController::new();
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    assert_eq!(c.pending_count(), 1, "only the theme until it answers");
    c.close();
    assert!(out.iter().all(|o| o.cancel.is_cancelled()));
    assert_eq!(c.pending_count(), 0);
    // the replies arrive anyway (already on the wire): ignored
    for o in out {
        let r = rig.transport.execute(&rig.origin, &o.request);
        assert!(c
            .on_reply(o.token, NetReply::from_result(r, o.post))
            .is_empty());
    }
    assert!(c.session.is_none());
    assert!(c.stale_replies >= 1);
}

#[test]
fn a_day_rollover_between_loading_and_submitting_gets_the_servers_answer() {
    let mut rig = Rig::new(seed::demo(NOW, false));
    let mut c = SkribblController::new();
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    c.start_drawing(0);
    rig.server.set_now(NOW + 13 * 3_600_000); // 01:00 Zurich the next day
    let out = c.submit();
    rig.drive(&mut c, out);
    let s = c.session.as_ref().unwrap();
    assert_eq!(
        s.error.as_deref(),
        Some("Drawings are only accepted for today's theme.")
    );
    assert_eq!(s.phase, Phase::Drawing);
}

#[test]
fn image_urls_outside_the_policy_are_never_fetched() {
    let mut rig = Rig::new(seed::demo(NOW, true));
    let mut c = SkribblController::new();
    let gallery = format!(
        r#"{{"date":"2026-10-04","drawings":[
            {{"id":"evil1","userId":"u1","displayName":"Evil","voteScore":0,"voteCount":0,"myVote":null,"isSelf":false,"imageUrl":"https://evil.example/skribbl/drawing/x"}},
            {{"id":"evil2","userId":"u2","displayName":"Evil 2","voteScore":0,"voteCount":0,"myVote":null,"isSelf":false,"imageUrl":"{}/admin"}},
            {{"id":"ok","userId":"u3","displayName":"Fine","voteScore":0,"voteCount":0,"myVote":null,"isSelf":false,"imageUrl":"{}/skribbl/drawing/drawings%2F2026-10-04%2Fsynthetic-friend-bob.png"}}
        ],"total":3,"nextOffset":null,"hasMore":false}}"#,
        rig.server.url(),
        rig.server.url()
    );
    rig.server
        .fault("/skribbl/gallery", Fault::Body(gallery.into_bytes()));
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    let log = rig.server.log();
    assert!(!log.iter().any(|(_, p)| p == "/admin"), "{log:?}");
    assert!(matches!(
        c.thumbs.state("https://evil.example/skribbl/drawing/x"),
        Some(crate::image_cache::ImageState::Failed)
    ));
    assert_eq!(
        log.iter()
            .filter(|(_, p)| p.starts_with("/skribbl/drawing/"))
            .count(),
        2,
        "the good row and the own drawing"
    );
}

#[test]
fn many_open_close_cycles_keep_everything_bounded() {
    let mut rig = Rig::new(seed::demo(NOW, true));
    let mut c = SkribblController::new();
    for i in 0..500 {
        let out = c.open(Some(me()), Some(rig.origin.clone()));
        if i % 50 == 0 {
            rig.drive(&mut c, out); // some cycles complete...
        } // ...most are closed with requests still pending
        c.close();
        assert_eq!(c.pending_count(), 0);
    }
    assert!(c.thumbs.len() <= THUMB_CACHE);
    assert!(c.full.is_none() && c.session.is_none());
    assert!(c.canvas.is_none(), "closing drops the 2 MiB raster");
    assert_eq!(c.opened, 500);
}

#[test]
fn the_lightbox_decodes_one_full_size_copy() {
    let mut rig = Rig::new(seed::demo(NOW, true));
    let mut c = SkribblController::new();
    let out = c.open(Some(me()), Some(rig.origin.clone()));
    rig.drive(&mut c, out);
    let id = c.session.as_ref().unwrap().gallery[0].id.clone();
    let out = c.expand(Some(id.clone()));
    assert_eq!(out.len(), 1);
    rig.drive(&mut c, out);
    let (_, img) = c.full.as_ref().unwrap();
    assert_eq!(img.as_ref().map(|i| (i.width, i.height)), Some((900, 600)));
    assert!(c.expand(Some(id)).is_empty(), "already decoded");
    assert!(c.expand(None).is_empty());
    assert!(c.full.is_none());
}
