//! The mock Worker against the real one: the golden scenario (`scripts/stage22-worker-goldens/
//! golden.test.ts`, recorded from the production Worker code) is replayed step by step against
//! `social-mock` through the real `UreqTransport` on loopback, and every response is compared
//! with the recording - status, plain-text messages exactly, JSON values after normalising what
//! is legitimately different (generated ids, timestamps, the random daily theme, the origin).

use std::time::Duration;

use serde_json::{json, Value};
use social_mock::world::{DrawingRow, World};
use social_mock::MockServer;

use crate::net::endpoint::{LocalEndpoint, SocialEndpoint};
use crate::net::http::{ApiPath, ApiRequest, Body, HttpResponse, Method, Priority, Target};
use crate::net::transport::{Transport, UreqTransport};

const GOLDENS: &str = include_str!("../../tests/fixtures/social/worker-goldens.jsonl");
/// When the goldens were recorded (Zurich 2026-10-04).
const RECORDED_AT_MS: i64 = 1_791_122_808_000;

struct Run {
    server: MockServer,
    transport: UreqTransport,
    endpoint: SocialEndpoint,
    mismatches: Vec<String>,
}

fn golden(name: &str) -> Value {
    GOLDENS
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| v["name"] == name)
        .unwrap_or_else(|| panic!("no golden {name}"))
}

/// Replaces values that legitimately differ between two servers with placeholders.
fn normalize(v: &Value, key: &str) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, v)| (k.clone(), normalize(v, k)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(|v| normalize(v, key)).collect()),
        Value::String(s) => {
            let placeholder = match key {
                "id" | "drawingId" | "requestId" => Some("<id>"),
                "syncedAt" | "createdAt" | "friendsSince" | "lastSeenAt" => Some("<ts>"),
                "theme" => Some("<theme>"),
                "imageUrl" | "url" => Some("<url>"),
                _ => None,
            };
            Value::String(placeholder.map_or_else(|| s.clone(), str::to_string))
        }
        other => other.clone(),
    }
}

impl Run {
    fn call(
        &mut self,
        name: &str,
        method: Method,
        path: &str,
        body: Option<Value>,
        form: Option<(String, Vec<u8>)>,
    ) -> HttpResponse {
        let (path, query) = match path.split_once('?') {
            Some((p, q)) => (
                p.to_string(),
                q.split('&')
                    .filter_map(|kv| kv.split_once('='))
                    .map(|(k, v)| {
                        (
                            if k == "userId" {
                                "userId"
                            } else {
                                "deviceSecret"
                            },
                            v.to_string(),
                        )
                    })
                    .collect(),
            ),
            None => (path.to_string(), Vec::new()),
        };
        let req = ApiRequest {
            method,
            target: Target::Image(path.clone()),
            query,
            body: match (body, form) {
                (Some(b), _) => Body::Json(b.to_string().into_bytes()),
                (None, Some((content_type, bytes))) => Body::Multipart {
                    content_type,
                    bytes,
                },
                _ => Body::None,
            },
            max_response: 4 * 1024 * 1024,
            timeout: Duration::from_secs(10),
            priority: Priority::Api,
        };
        let resp = self
            .transport
            .execute(&self.endpoint.origin(), &req)
            .expect("mock reachable");
        let want = golden(name);
        let want_status = want["status"].as_u64().unwrap() as u16;
        if resp.status != want_status {
            self.mismatches.push(format!(
                "{name}: status {} vs worker {want_status}",
                resp.status
            ));
            return resp;
        }
        match want["body"].as_str() {
            Some(text)
                if want["contentType"]
                    .as_str()
                    .is_some_and(|c| c.starts_with("application/json")) =>
            {
                let got: Value = serde_json::from_slice(&resp.body).unwrap_or(Value::Null);
                let want: Value = serde_json::from_str(text).unwrap();
                if normalize(&got, "") != normalize(&want, "") {
                    self.mismatches.push(format!(
                        "{name}:\n  mock   {}\n  worker {}",
                        normalize(&got, ""),
                        normalize(&want, "")
                    ));
                }
            }
            Some(text) => {
                if resp.body != text.as_bytes() {
                    self.mismatches.push(format!(
                        "{name}: text {:?} vs worker {text:?}",
                        String::from_utf8_lossy(&resp.body)
                    ));
                }
            }
            None => {
                if resp.content_type.as_deref() != want["contentType"].as_str() {
                    self.mismatches
                        .push(format!("{name}: content type {:?}", resp.content_type));
                }
            }
        }
        resp
    }
}

fn user(id: &str, code: &str, name: &str, extra: Value) -> Value {
    let mut u = json!({"userId": id, "deviceSecret": format!("secret-{}", id.trim_start_matches("golden-")), "friendCode": code, "displayName": name});
    if let Value::Object(e) = extra {
        u.as_object_mut().unwrap().extend(e);
    }
    u
}

fn sync_body(u: &Value, minutes: u64, sessions: u64, stats: Value) -> Value {
    let mut user = u.clone();
    user["lifetimeStudyMinutes"] = json!(minutes);
    user["lifetimeStudySessions"] = json!(sessions);
    user["device"] =
        json!({"fingerprintHash": "0123456789abcdef", "label": "linux x86_64 development"});
    user["app"] =
        json!({"version": "0.1.0", "platform": "Linux x86_64", "runtimeChannel": "development"});
    json!({"user": user, "stats": stats, "feedPosts": []})
}

fn auth(u: &Value) -> Value {
    json!({"userId": u["userId"], "deviceSecret": u["deviceSecret"]})
}

fn with(mut base: Value, extra: Value) -> Value {
    base.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    base
}

fn png() -> Vec<u8> {
    // the same valid 2x2 white PNG the golden generator uploads
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 2, 2);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&[255; 16])
        .unwrap();
    out
}

fn form(u: &Value, date: &str, file_name: &str, mime: &str, data: Vec<u8>) -> (String, Vec<u8>) {
    crate::net::multipart::Multipart::new()
        .text("userId", u["userId"].as_str().unwrap())
        .text("deviceSecret", u["deviceSecret"].as_str().unwrap())
        .text("date", date)
        .file("image", file_name, mime, data)
        .finish()
        .unwrap()
}

#[test]
fn the_mock_answers_the_golden_scenario_like_the_production_worker() {
    let mut world = World::new("http://127.0.0.1", RECORDED_AT_MS);
    world.theme_pool = vec!["Golden theme".into()];
    let server = MockServer::start(0, world).unwrap();
    let endpoint = SocialEndpoint::Test(LocalEndpoint::new("127.0.0.1", server.port()).unwrap());
    let mut r = Run {
        server,
        transport: UreqTransport::new(),
        endpoint,
        mismatches: Vec::new(),
    };
    let ada = user(
        "golden-ada",
        "ADAA-2345",
        "Ada Lovelace",
        json!({"avatar": {"kind": "letter", "letter": "a", "style": "pixel"}}),
    );
    let bob = user(
        "golden-bob",
        "BOBB-2345",
        "Bob",
        json!({"avatar": {"kind": "icon", "icon": "🦊"}}),
    );
    let cho = user(
        "golden-cho",
        "CHOO-2345",
        "張偉 — Zoë 🦊",
        json!({"avatar": {"kind": "photo", "name": "me.webp", "url": "https://test.local/profile/avatar/avatars%2Fgolden-cho%2F1.webp", "mimeType": "image/webp"}}),
    );
    let dan = user(
        "golden-dan",
        "DANN-2345",
        "Dan Private",
        json!({"isPrivate": true}),
    );
    let eve = user(
        "golden-eve",
        "EVEE-2345",
        "Eve",
        json!({"showHoursToFriends": false}),
    );
    let rtl = user("golden-rtl", "RTLL-2345", "שלום עולם", json!({}));
    let long = user(
        "golden-long",
        "LONG-2345",
        "A very long display name that goes on and on and on forever",
        json!({}),
    );
    let today = "2026-10-04";
    let post = Method::Post;
    r.call(
        "sync-create-ada",
        post,
        "/sync/v2",
        Some(sync_body(
            &ada,
            50,
            2,
            json!([{"date": today, "minutes": 50, "sessions": 2}]),
        )),
        None,
    );
    for (u, n) in [
        (&bob, "golden-bob"),
        (&cho, "golden-cho"),
        (&dan, "golden-dan"),
        (&eve, "golden-eve"),
        (&rtl, "golden-rtl"),
        (&long, "golden-long"),
    ] {
        r.call(
            &format!("sync-create-{n}"),
            post,
            "/sync/v2",
            Some(sync_body(u, 0, 0, json!([]))),
            None,
        );
    }
    let mut wrong = ada.clone();
    wrong["deviceSecret"] = json!("not-the-secret");
    r.call(
        "sync-wrong-secret",
        post,
        "/sync/v2",
        Some(sync_body(&wrong, 0, 0, json!([]))),
        None,
    );
    r.call(
        "sync-missing-user",
        post,
        "/sync/v2",
        Some(json!({"stats": []})),
        None,
    );
    r.call("sync-duplicate-code", post, "/sync/v2", Some(sync_body(&json!({"userId": "golden-dup", "deviceSecret": "secret-dup", "friendCode": "ADAA-2345", "displayName": "Dup"}), 0, 0, json!([]))), None);
    r.call("presence-ok", post, "/presence", Some(with(auth(&ada), json!({"app": {"version": "0.1.0", "platform": "Linux x86_64", "runtimeChannel": "development"}}))), None);
    r.call(
        "presence-unknown-user",
        post,
        "/presence",
        Some(json!({"userId": "nobody", "deviceSecret": "x"})),
        None,
    );
    r.call(
        "friends-status-empty",
        post,
        "/friends/status/v2",
        Some(auth(&ada)),
        None,
    );
    r.call(
        "friend-request-to-bob",
        post,
        "/friends/request",
        Some(with(auth(&ada), json!({"friendCode": "BOBB-2345"}))),
        None,
    );
    let bob_view = r.call(
        "friends-status-bob-incoming",
        post,
        "/friends/status/v2",
        Some(auth(&bob)),
        None,
    );
    let rid = serde_json::from_slice::<Value>(&bob_view.body).unwrap()["social"]
        ["incomingFriendRequests"][0]["id"]
        .clone();
    r.call(
        "friend-respond-accept",
        post,
        "/friends/respond",
        Some(with(
            auth(&bob),
            json!({"requestId": rid, "response": "accepted"}),
        )),
        None,
    );
    r.call(
        "friend-respond-again",
        post,
        "/friends/respond",
        Some(with(
            auth(&bob),
            json!({"requestId": rid, "response": "accepted"}),
        )),
        None,
    );
    r.call(
        "friend-request-unknown-code",
        post,
        "/friends/request",
        Some(with(auth(&ada), json!({"friendCode": "ZZZZ-9999"}))),
        None,
    );
    r.call(
        "friend-request-self",
        post,
        "/friends/request",
        Some(with(auth(&ada), json!({"friendCode": "ADAA-2345"}))),
        None,
    );
    r.call(
        "friend-request-already-friends",
        post,
        "/friends/request",
        Some(with(auth(&ada), json!({"friendCode": "BOBB-2345"}))),
        None,
    );
    r.call(
        "friend-request-to-cho",
        post,
        "/friends/request",
        Some(with(auth(&ada), json!({"friendCode": "chOO-2345 "}))),
        None,
    );
    r.call(
        "friend-request-to-cho-again",
        post,
        "/friends/request",
        Some(with(auth(&ada), json!({"friendCode": "CHOO-2345"}))),
        None,
    );
    r.call(
        "friend-request-eve-to-ada",
        post,
        "/friends/request",
        Some(with(auth(&eve), json!({"friendCode": "ADAA-2345"}))),
        None,
    );
    r.call(
        "friends-status-ada-mixed",
        post,
        "/friends/status/v2",
        Some(auth(&ada)),
        None,
    );
    let before = r.call(
        "friends-status-ada-before-decline",
        post,
        "/friends/status/v2",
        Some(auth(&ada)),
        None,
    );
    let incoming = serde_json::from_slice::<Value>(&before.body).unwrap()["social"]
        ["incomingFriendRequests"][0]["id"]
        .clone();
    r.call(
        "friend-respond-decline",
        post,
        "/friends/respond",
        Some(with(
            auth(&ada),
            json!({"requestId": incoming, "response": "declined"}),
        )),
        None,
    );
    r.call(
        "friend-request-reciprocal",
        post,
        "/friends/request",
        Some(with(auth(&cho), json!({"friendCode": "ADAA-2345"}))),
        None,
    );
    r.call(
        "friend-request-eve-again",
        post,
        "/friends/request",
        Some(with(auth(&eve), json!({"friendCode": "ADAA-2345"}))),
        None,
    );
    let pending = r.call(
        "friends-status-ada-eve-pending",
        post,
        "/friends/status/v2",
        Some(auth(&ada)),
        None,
    );
    let eve_req = serde_json::from_slice::<Value>(&pending.body).unwrap()["social"]
        ["incomingFriendRequests"][0]["id"]
        .clone();
    r.call(
        "friend-respond-accept-eve",
        post,
        "/friends/respond",
        Some(with(
            auth(&ada),
            json!({"requestId": eve_req, "response": "accepted"}),
        )),
        None,
    );
    for (u, n) in [(&rtl, "golden-rtl"), (&long, "golden-long")] {
        r.call(
            &format!("friend-request-{n}"),
            post,
            "/friends/request",
            Some(with(auth(u), json!({"friendCode": "ADAA-2345"}))),
            None,
        );
    }
    r.call(
        "friends-status-ada-final",
        post,
        "/friends/status/v2",
        Some(auth(&ada)),
        None,
    );
    r.call(
        "friends-status-wrong-secret",
        post,
        "/friends/status/v2",
        Some(json!({"userId": "golden-ada", "deviceSecret": "nope"})),
        None,
    );
    {
        let mut w = r.server.world.lock().unwrap();
        for (id, date, m, s) in [
            ("golden-ada", today, 50, 2),
            ("golden-bob", today, 120, 3),
            ("golden-cho", today, 50, 1),
            ("golden-eve", today, 80, 1),
            ("golden-dan", today, 300, 5),
            ("golden-bob", "2026-10-03", 60, 1),
        ] {
            w.daily.insert((id.into(), date.into()), (m, s));
        }
        w.baselines.insert("golden-long".into(), (1000, 20));
    }
    for scope in ["friends", "global", "squad"] {
        for period in ["daily", "weekly", "overall"] {
            r.call(
                &format!("leaderboard-{scope}-{period}"),
                post,
                "/leaderboard",
                Some(with(auth(&ada), json!({"scope": scope, "period": period}))),
                None,
            );
        }
    }
    r.call(
        "leaderboard-unknown-user",
        post,
        "/leaderboard",
        Some(
            json!({"userId": "nobody", "deviceSecret": "x", "scope": "global", "period": "daily"}),
        ),
        None,
    );
    for (name, target) in [
        ("player-stats-friend", "golden-bob"),
        ("player-stats-hidden-hours", "golden-eve"),
        ("player-stats-self", "golden-ada"),
        ("player-stats-not-friend", "golden-dan"),
        ("player-stats-missing", "ghost"),
    ] {
        r.call(
            name,
            post,
            "/player-stats",
            Some(with(auth(&ada), json!({"targetUserId": target}))),
            None,
        );
    }
    let q = "?userId=golden-ada&deviceSecret=secret-ada";
    r.call(
        "skribbl-leaderboard-no-winner",
        Method::Get,
        &format!("/skribbl/leaderboard{q}"),
        None,
        None,
    );
    r.call(
        "skribbl-theme-fresh",
        Method::Get,
        &format!("/skribbl/theme{q}"),
        None,
        None,
    );
    r.call(
        "skribbl-theme-unknown-user",
        Method::Get,
        "/skribbl/theme?userId=nobody&deviceSecret=x",
        None,
        None,
    );
    r.call(
        "skribbl-gallery-empty",
        post,
        "/skribbl/gallery",
        Some(with(
            auth(&ada),
            json!({"date": today, "offset": 0, "limit": 16}),
        )),
        None,
    );
    r.call(
        "skribbl-submit-wrong-date",
        post,
        "/skribbl/submit",
        None,
        Some(form(&ada, "2026-10-03", "drawing.png", "image/png", png())),
    );
    r.call(
        "skribbl-submit-gif",
        post,
        "/skribbl/submit",
        None,
        Some(form(&ada, today, "drawing.gif", "image/gif", png())),
    );
    r.call(
        "skribbl-submit-too-large",
        post,
        "/skribbl/submit",
        None,
        Some(form(
            &ada,
            today,
            "drawing.png",
            "image/png",
            vec![0; 1_572_865],
        )),
    );
    let ok = r.call(
        "skribbl-submit-ok",
        post,
        "/skribbl/submit",
        None,
        Some(form(&ada, today, "drawing.png", "image/png", png())),
    );
    r.call(
        "skribbl-submit-duplicate",
        post,
        "/skribbl/submit",
        None,
        Some(form(&ada, today, "drawing.png", "image/png", png())),
    );
    r.call(
        "skribbl-theme-submitted",
        Method::Get,
        &format!("/skribbl/theme{q}"),
        None,
        None,
    );
    let url = serde_json::from_slice::<Value>(&ok.body).unwrap()["imageUrl"]
        .as_str()
        .unwrap()
        .to_string();
    let path = url.splitn(4, '/').nth(3).map(|p| format!("/{p}")).unwrap();
    r.call("skribbl-drawing-png", Method::Get, &path, None, None);
    r.call(
        "skribbl-drawing-missing",
        Method::Get,
        "/skribbl/drawing/drawings%2Fnope.png",
        None,
        None,
    );
    for i in 0..19u32 {
        let u = json!({"userId": format!("golden-artist-{i}"), "deviceSecret": format!("secret-artist-{i}"), "friendCode": format!("ART{}-2345", char::from(b'A' + i as u8)), "displayName": format!("Artist {i}")});
        let t = &r.transport;
        let origin = r.endpoint.origin();
        t.execute(
            &origin,
            &ApiRequest {
                method: Method::Post,
                target: Target::Api(ApiPath::SyncV2),
                query: vec![],
                body: Body::Json(sync_body(&u, 0, 0, json!([])).to_string().into_bytes()),
                max_response: 1 << 20,
                timeout: Duration::from_secs(5),
                priority: Priority::Api,
            },
        )
        .unwrap();
        let (ct, bytes) = form(&u, today, "drawing.png", "image/png", png());
        t.execute(
            &origin,
            &ApiRequest {
                method: Method::Post,
                target: Target::Api(ApiPath::SkribblSubmit),
                query: vec![],
                body: Body::Multipart {
                    content_type: ct,
                    bytes,
                },
                max_response: 1 << 20,
                timeout: Duration::from_secs(5),
                priority: Priority::Api,
            },
        )
        .unwrap();
    }
    let page1 = r.call(
        "skribbl-gallery-page1",
        post,
        "/skribbl/gallery",
        Some(with(
            auth(&ada),
            json!({"date": today, "offset": 0, "limit": 16}),
        )),
        None,
    );
    let page1: Value = serde_json::from_slice(&page1.body).unwrap();
    let page2 = r.call(
        "skribbl-gallery-page2",
        post,
        "/skribbl/gallery",
        Some(with(
            auth(&ada),
            json!({"date": today, "offset": page1["nextOffset"], "limit": 16}),
        )),
        None,
    );
    let page2: Value = serde_json::from_slice(&page2.body).unwrap();
    let drawings = page1["drawings"].as_array().unwrap();
    let other = drawings.iter().find(|d| d["isSelf"] == false).unwrap()["id"].clone();
    let own = drawings.iter().find(|d| d["isSelf"] == true).unwrap()["id"].clone();
    for (name, who, id, vote) in [
        ("skribbl-vote-up", &ada, &other, 1),
        ("skribbl-vote-down", &ada, &other, -1),
        ("skribbl-vote-down-bob", &bob, &other, -1),
        ("skribbl-vote-clear", &ada, &other, 0),
        ("skribbl-vote-own", &ada, &own, 1),
        ("skribbl-vote-invalid", &ada, &other, 2),
    ] {
        r.call(
            name,
            post,
            "/skribbl/vote",
            Some(with(auth(who), json!({"drawingId": id, "vote": vote}))),
            None,
        );
    }
    r.call(
        "skribbl-vote-missing",
        post,
        "/skribbl/vote",
        Some(with(auth(&ada), json!({"drawingId": "nope", "vote": 1}))),
        None,
    );
    r.call(
        "skribbl-vote-up-again",
        post,
        "/skribbl/vote",
        Some(with(auth(&ada), json!({"drawingId": other, "vote": 1}))),
        None,
    );
    let after = r.call(
        "skribbl-gallery-after-votes",
        post,
        "/skribbl/gallery",
        Some(with(
            auth(&ada),
            json!({"date": today, "offset": 0, "limit": 16}),
        )),
        None,
    );
    let after: Value = serde_json::from_slice(&after.body).unwrap();
    {
        let mut w = r.server.world.lock().unwrap();
        for (id, who) in [("y1", "golden-bob"), ("y2", "golden-cho")] {
            w.drawings.push(DrawingRow {
                id: id.into(),
                date: "2026-10-03".into(),
                user: who.into(),
                key: format!("drawings/y/{who}.png"),
                mime: "image/png".into(),
                bytes: png(),
                created_at: RECORDED_AT_MS - 86_400_000,
            });
        }
        w.votes.insert(("y2".into(), "golden-ada".into()), 1);
        w.votes.insert(("y2".into(), "golden-eve".into()), 1);
        w.votes.insert(("y1".into(), "golden-ada".into()), -1);
    }
    r.call(
        "skribbl-leaderboard-winner",
        Method::Get,
        &format!("/skribbl/leaderboard{q}"),
        None,
        None,
    );

    // Drawings submitted in the same second are ordered by their (random) UUID on the Worker and by
    // a sequence on the mock, so which 16 land on page 1 legitimately differs. Compare the pages
    // as sets instead: the union of both pages, page sizes, and the voted rows.
    r.mismatches.retain(|m| {
        !m.starts_with("skribbl-gallery-page") && !m.starts_with("skribbl-gallery-after-votes")
    });
    let users = |pages: &[&Value]| {
        let mut v: Vec<String> = pages
            .iter()
            .flat_map(|p| {
                p["drawings"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d["userId"].as_str().unwrap().to_string())
            })
            .collect();
        v.sort();
        v
    };
    let (g1, g2) = (
        serde_json::from_str::<Value>(golden("skribbl-gallery-page1")["body"].as_str().unwrap())
            .unwrap(),
        serde_json::from_str::<Value>(golden("skribbl-gallery-page2")["body"].as_str().unwrap())
            .unwrap(),
    );
    assert_eq!(
        users(&[&page1, &page2]),
        users(&[&g1, &g2]),
        "the same 20 drawings across both pages"
    );
    for (mock, worker) in [(&page1, &g1), (&page2, &g2)] {
        for k in ["total", "nextOffset", "hasMore"] {
            assert_eq!(mock[k], worker[k], "{k}");
        }
        assert_eq!(
            mock["drawings"].as_array().unwrap().len(),
            worker["drawings"].as_array().unwrap().len()
        );
    }
    let voted = |p: &Value| {
        let mut v: Vec<String> = p["drawings"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| !d["myVote"].is_null() || d["voteCount"] != 0)
            .map(|d| {
                format!(
                    "{} {} {} {}",
                    d["myVote"], d["voteCount"], d["voteScore"], d["isSelf"]
                )
            })
            .collect();
        v.sort();
        v
    };
    let ga = serde_json::from_str::<Value>(
        golden("skribbl-gallery-after-votes")["body"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        voted(&after),
        voted(&ga),
        "the voted drawing reads the same (myVote 1, two votes, score 0)"
    );
    assert!(
        r.mismatches.is_empty(),
        "{} mismatch(es):\n{}",
        r.mismatches.len(),
        r.mismatches.join("\n")
    );
    assert!(r.server.count("/sync/v2") >= 7);
    r.server.stop();
}
