//! Deterministic synthetic worlds. Every name, code and secret here is invented; the self user's
//! secret is the recognisable `TEST_SECRET_MUST_NOT_APPEAR`, so any leak into a log, screenshot
//! or diagnostic is easy to search for.

use serde_json::json;

use crate::world::{DrawingRow, FriendRequestRow, User, World};
use crate::world_22b::{
    CommentRow, JoinRow, MemberRow, MessageRow, PollRow, PostRow, ScoreRow, SquadRow,
};

pub const SELF_ID: &str = "synthetic-user-0001";
pub const SELF_SECRET: &str = "TEST_SECRET_MUST_NOT_APPEAR";
pub const SELF_CODE: &str = "SYNT-2345";
pub const SELF_NAME: &str = "Sam Synthetic";

fn user(id: &str, code: &str, name: &str, avatar: serde_json::Value, now: i64) -> User {
    User {
        id: id.into(),
        secret: format!("synthetic-secret-{id}"),
        friend_code: code.into(),
        display_name: name.into(),
        avatar,
        is_private: false,
        show_hours_to_friends: true,
        last_seen_at: now,
    }
}

/// A 900x600 PNG: white paper with a few filled shapes (deterministic per `seed`).
pub fn drawing_png(seed: u32) -> Vec<u8> {
    let (w, h) = (900u32, 600u32);
    let mut px = vec![255u8; (w * h * 4) as usize];
    let palette: [[u8; 3]; 6] = [
        [30, 136, 229],
        [229, 57, 53],
        [67, 160, 71],
        [253, 216, 53],
        [142, 36, 170],
        [0, 0, 0],
    ];
    let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(17);
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        s
    };
    for shape in 0..5 {
        let c = palette[(next() % 6) as usize];
        let (cx, cy) = ((next() % w) as i64, (next() % h) as i64);
        let r = 40 + (next() % 120) as i64;
        for y in (cy - r).max(0)..(cy + r).min(h as i64) {
            for x in (cx - r).max(0)..(cx + r).min(w as i64) {
                let inside = if shape % 2 == 0 {
                    (x - cx).pow(2) + (y - cy).pow(2) <= r * r
                } else {
                    true
                };
                if inside {
                    let i = ((y as u32 * w + x as u32) * 4) as usize;
                    px[i..i + 3].copy_from_slice(&c);
                }
            }
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&px).expect("png data");
    }
    out
}

/// A 128x128 PNG avatar photo (a coloured disc).
pub fn avatar_png() -> Vec<u8> {
    let n = 128u32;
    let mut px = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as i64 - 64, y as i64 - 64);
            let i = ((y * n + x) * 4) as usize;
            let inside = dx * dx + dy * dy <= 60 * 60;
            px[i..i + 4].copy_from_slice(&if inside {
                [79, 107, 74, 255]
            } else {
                [238, 230, 214, 255]
            });
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, n, n);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&px).expect("png data");
    }
    out
}

/// No users at all (a new account will be created by `/sync/v2`).
pub fn empty(now: i64) -> World {
    World::new("http://127.0.0.1", now)
}

/// Only the self user, already synced, with no friends (the "empty Friends" state).
pub fn self_only(now: i64) -> World {
    let mut w = empty(now);
    w.users.insert(
        SELF_ID.into(),
        User {
            secret: SELF_SECRET.into(),
            ..user(
                SELF_ID,
                SELF_CODE,
                SELF_NAME,
                json!({"kind": "letter", "letter": "S", "style": "classic"}),
                now,
            )
        },
    );
    w
}

/// The populated demo world used by the stress tests and the visual-parity captures.
/// `self_submitted`: whether the self user already drew today's theme.
pub fn demo(now: i64, self_submitted: bool) -> World {
    let mut w = self_only(now);
    let minute = 60_000;
    let friends = [
        (
            "synthetic-friend-bob",
            "BOBB-2345",
            "Bob",
            json!({"kind": "icon", "icon": "🦊"}),
            now - 5 * minute,
        ),
        (
            "synthetic-friend-zoe",
            "ZOEE-2345",
            "张伟 Zoë 🦊",
            json!({"kind": "letter", "letter": "Z", "style": "cursive"}),
            now - 3 * 60 * minute,
        ),
        (
            "synthetic-friend-amelie",
            "AMEL-2345",
            "Amélie",
            json!({"kind": "photo", "name": "amelie.png", "url": "AVATAR_URL", "mimeType": "image/png"}),
            now - 20 * minute,
        ),
        (
            "synthetic-friend-rtl",
            "SHLM-2345",
            "שלום עולם",
            json!({"kind": "letter", "letter": "S", "style": "serif"}),
            now - 30 * 60 * minute,
        ),
    ];
    for (id, code, name, avatar, seen) in friends {
        let mut u = user(id, code, name, avatar, now);
        u.last_seen_at = seen;
        w.users.insert(id.into(), u);
        w.friendships.insert(if SELF_ID < id {
            (SELF_ID.into(), id.into())
        } else {
            (id.into(), SELF_ID.into())
        });
    }
    let others = [
        (
            "synthetic-user-kenji",
            "KENJ-2345",
            "Kenji 健二",
            json!({"kind": "letter", "letter": "K", "style": "pixel"}),
        ),
        (
            "synthetic-user-priya",
            "PRYA-2345",
            "Priya",
            json!({"kind": "letter", "letter": "P", "style": "mono"}),
        ),
        (
            "synthetic-user-dan",
            "DANN-2345",
            "Dan Private",
            json!({"kind": "letter", "letter": "D", "style": "classic"}),
        ),
        (
            "synthetic-user-long",
            "LONG-2345",
            "A very long display name that goes on and on and",
            json!({"kind": "letter", "letter": "A", "style": "graffiti"}),
        ),
    ];
    for (id, code, name, avatar) in others {
        w.users.insert(
            id.into(),
            user(id, code, name, avatar, now - 2 * 24 * 60 * minute),
        );
    }
    w.users.get_mut("synthetic-user-dan").unwrap().is_private = true;
    // the photo avatar points at the mock's own origin (patched once the port is known)
    w.avatars.insert(
        "avatars/synthetic-friend-amelie/1.png".into(),
        ("image/png".into(), avatar_png()),
    );
    w.requests.push(FriendRequestRow {
        id: "request-incoming-kenji".into(),
        from: "synthetic-user-kenji".into(),
        to: SELF_ID.into(),
        status: "pending",
        created_at: now - 60 * minute,
    });
    w.requests.push(FriendRequestRow {
        id: "request-outgoing-priya".into(),
        from: SELF_ID.into(),
        to: "synthetic-user-priya".into(),
        status: "pending",
        created_at: now - 90 * minute,
    });
    // verified minutes (what the leaderboards count since migration 0019)
    let today = w.today();
    let yesterday = w.yesterday();
    for (id, date, minutes, sessions) in [
        (SELF_ID, &today, 95, 3),
        ("synthetic-friend-bob", &today, 140, 4),
        ("synthetic-friend-zoe", &today, 95, 2),
        ("synthetic-friend-amelie", &today, 50, 1),
        ("synthetic-friend-rtl", &yesterday, 30, 1),
        ("synthetic-user-kenji", &today, 210, 5),
        ("synthetic-user-dan", &today, 400, 6),
        ("synthetic-friend-bob", &yesterday, 60, 2),
    ] {
        w.daily
            .insert((id.into(), date.clone()), (minutes, sessions));
    }
    w.baselines.insert("synthetic-user-long".into(), (5000, 90));
    w.baselines.insert(SELF_ID.into(), (1200, 40));
    // Daily Skribbl: today's theme, other users' drawings, votes, and yesterday's winner
    w.themes
        .insert(yesterday.clone(), "Treehouse in Autumn".into());
    w.themes
        .insert(today.clone(), "Lighthouse in the fog".into());
    let artists = [
        "synthetic-friend-bob",
        "synthetic-friend-zoe",
        "synthetic-friend-amelie",
        "synthetic-user-kenji",
        "synthetic-user-priya",
        "synthetic-friend-rtl",
    ];
    for (i, artist) in artists.iter().enumerate() {
        w.drawings.push(DrawingRow {
            id: format!("drawing-today-{i}"),
            date: today.clone(),
            user: (*artist).into(),
            key: format!("drawings/{today}/{artist}.png"),
            mime: "image/png".into(),
            bytes: drawing_png(i as u32 + 1),
            created_at: now - (60 - i as i64) * minute,
        });
    }
    if self_submitted {
        w.drawings.push(DrawingRow {
            id: "drawing-today-self".into(),
            date: today.clone(),
            user: SELF_ID.into(),
            key: format!("drawings/{today}/{SELF_ID}.png"),
            mime: "image/png".into(),
            bytes: drawing_png(99),
            created_at: now - 10 * minute,
        });
        w.votes
            .insert(("drawing-today-0".into(), SELF_ID.into()), 1);
    }
    for (voter, drawing, vote) in [
        ("synthetic-user-kenji", "drawing-today-0", 1),
        ("synthetic-friend-zoe", "drawing-today-0", 1),
        ("synthetic-friend-bob", "drawing-today-3", -1),
    ] {
        w.votes.insert((drawing.into(), voter.into()), vote);
    }
    for (i, artist) in ["synthetic-friend-amelie", "synthetic-friend-bob"]
        .iter()
        .enumerate()
    {
        w.drawings.push(DrawingRow {
            id: format!("drawing-yesterday-{i}"),
            date: yesterday.clone(),
            user: (*artist).into(),
            key: format!("drawings/{yesterday}/{artist}.png"),
            mime: "image/png".into(),
            bytes: drawing_png(50 + i as u32),
            created_at: now - 26 * 60 * minute,
        });
    }
    for voter in [
        "synthetic-user-kenji",
        "synthetic-friend-zoe",
        "synthetic-user-priya",
        SELF_ID,
    ] {
        w.votes
            .insert(("drawing-yesterday-0".into(), voter.into()), 1);
    }
    w
}

/// A W x H photo-like PNG (a gradient with a disc), for feed images.
pub fn photo_png(w: u32, h: u32, seed: u32) -> Vec<u8> {
    let mut px = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            let disc = (fx - 0.62).powi(2) + (fy - 0.4).powi(2) < 0.04;
            let base = [
                (90.0 + 120.0 * fy) as u8,
                (120.0 + 80.0 * fx) as u8,
                (160.0 + (seed % 60) as f32) as u8,
            ];
            px[i..i + 4].copy_from_slice(&if disc {
                [250, 214, 120, 255]
            } else {
                [base[0], base[1], base[2], 255]
            });
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        writer.write_image_data(&px).expect("png data");
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn post(
    id: &str,
    user: &str,
    subject: &str,
    detail: &str,
    note: &str,
    icon: &str,
    minutes: u64,
    at: i64,
) -> PostRow {
    PostRow {
        id: id.into(),
        user: user.into(),
        kind: "session".into(),
        subject: subject.into(),
        detail: detail.into(),
        note: note.into(),
        icon: icon.into(),
        minutes,
        preset: "Focus".into(),
        created_at: crate::iso(at),
        image_key: None,
        image_mime: None,
        image_expires: None,
        image_expired_at: None,
    }
}

/// Stage 22b data on top of [`demo`]: feed posts (text, poll, image, comments, reactions, a
/// milestone, an expired image), squads (the user's own as leader, public/private/full others,
/// join requests, chat) and Squad Arena scores. `in_squad = false` leaves the user squadless.
pub fn add_22b(w: &mut World, in_squad: bool) {
    let now = w.now_ms;
    let minute = 60_000;
    // a user in no squad (the join request below)
    w.users.insert(
        "synthetic-user-mia".into(),
        user(
            "synthetic-user-mia",
            "MIAA-2345",
            "Mia",
            json!({"kind": "letter", "letter": "M", "style": "classic"}),
            now - 90 * minute,
        ),
    );
    let x = &mut w.x;
    x.posts.push(post(
        "post-bob-1",
        "synthetic-friend-bob",
        "Linear Algebra",
        "2h 20m · Focus",
        "eigenvalues finally make sense",
        "✦",
        140,
        now - 25 * minute,
    ));
    x.posts.push(post(
        "post-zoe-1",
        "synthetic-friend-zoe",
        "Organic Chemistry",
        "1h 35m · Exam",
        "张伟 says: 反应机理 ✓",
        "⚔",
        95,
        now - 70 * minute,
    ));
    let mut img = post(
        "post-amelie-1",
        "synthetic-friend-amelie",
        "Art History",
        "50m · Focus",
        "notes from the museum trip",
        "✦",
        50,
        now - 110 * minute,
    );
    img.image_key = Some("feed-posts/post-amelie-1/img-1.png".into());
    img.image_mime = Some("image/png".into());
    img.image_expires = Some(now + 3 * 24 * 60 * minute);
    x.images.insert(
        "feed-posts/post-amelie-1/img-1.png".into(),
        photo_png(640, 420, 7),
    );
    x.posts.push(img);
    let mut expired = post(
        "post-rtl-1",
        "synthetic-friend-rtl",
        "Hebrew",
        "30m · Focus",
        "שלום עולם",
        "✦",
        30,
        now - 30 * 60 * minute,
    );
    expired.image_expired_at = Some(crate::iso(now - 60 * minute));
    x.posts.push(expired);
    x.posts.push(post(
        "post-self-1",
        SELF_ID,
        "Analysis II",
        "1h 35m · Focus",
        "only 5 billion things to go...",
        "✦",
        95,
        now - 4 * 60 * minute,
    ));
    x.posts.push(post(
        "post-kenji-1",
        "synthetic-user-kenji",
        "Kanji drills",
        "3h 30m · Focus",
        "健二: 漢字 x 200",
        "✦",
        210,
        now - 5 * 60 * minute,
    ));
    let mut ms = post(
        "post-bob-ms",
        "synthetic-friend-bob",
        "",
        "100 hours",
        "100 hours of focus",
        "🏆",
        0,
        now - 26 * 60 * minute,
    );
    ms.kind = "milestone".into();
    x.posts.push(ms);
    x.polls.insert(
        "post-bob-1".into(),
        PollRow {
            question: "Best study snack?".into(),
            multiple: false,
            options: vec![
                ("opt-bob-1".into(), "Apples".into()),
                ("opt-bob-2".into(), "Dark chocolate".into()),
                ("opt-bob-3".into(), "Coffee, obviously".into()),
            ],
        },
    );
    x.polls.insert(
        "post-self-1".into(),
        PollRow {
            question: "Which topic next?".into(),
            multiple: true,
            options: vec![
                ("opt-self-1".into(), "Series".into()),
                ("opt-self-2".into(), "Integrals".into()),
            ],
        },
    );
    for (p, o, u) in [
        ("post-bob-1", "opt-bob-2", "synthetic-friend-zoe"),
        ("post-bob-1", "opt-bob-2", "synthetic-friend-amelie"),
        ("post-bob-1", "opt-bob-3", "synthetic-user-kenji"),
        ("post-self-1", "opt-self-1", "synthetic-friend-bob"),
    ] {
        x.poll_votes.push((p.into(), o.into(), u.into()));
    }
    for (p, u, e) in [
        ("post-bob-1", "synthetic-friend-zoe", "fire"),
        ("post-bob-1", SELF_ID, "fire"),
        ("post-bob-1", "synthetic-friend-amelie", "brain"),
        ("post-bob-1", "synthetic-user-kenji", "🎯"),
        ("post-self-1", "synthetic-friend-bob", "clap"),
        ("post-zoe-1", "synthetic-friend-bob", "❤️"),
    ] {
        x.reactions.push((p.into(), u.into(), e.into()));
    }
    for (i, (p, u, body, ago)) in [
        ("post-bob-1", "synthetic-friend-zoe", "same, finally!", 20),
        ("post-bob-1", SELF_ID, "nice work", 15),
        ("post-self-1", "synthetic-friend-bob", "go go go 🚀", 180),
        (
            "post-self-1",
            "synthetic-friend-amelie",
            "Amélie: très bien",
            170,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        x.comments.push(CommentRow {
            id: format!("comment-seed-{i}"),
            post: p.into(),
            user: u.into(),
            body: body.into(),
            created_at: now - ago * minute,
        });
    }
    // squads
    let squads = [
        ("squad-owl", "Night Owls", false, "synthetic-friend-bob"),
        ("squad-lib", "Library Ghosts", true, "synthetic-user-kenji"),
        ("squad-full", "Full House", false, "synthetic-user-priya"),
        ("squad-math", "Maths Circle", false, "synthetic-user-long"),
        ("squad-quiet", "Quiet Corner", true, "synthetic-friend-rtl"),
    ];
    for (i, (id, name, private, by)) in squads.iter().enumerate() {
        x.squads.push(SquadRow {
            id: (*id).into(),
            name: (*name).into(),
            private: *private,
            created_by: (*by).into(),
            created_at: now - (40 - i as i64) * 24 * 60 * minute,
        });
    }
    let mut members = vec![
        ("squad-lib", "synthetic-user-kenji", "leader"),
        ("squad-full", "synthetic-user-priya", "leader"),
        ("squad-full", "synthetic-user-dan", "co_leader"),
        ("squad-full", "synthetic-friend-amelie", "elder"),
        ("squad-full", "synthetic-friend-rtl", "member"),
        ("squad-math", "synthetic-user-long", "leader"),
    ];
    if in_squad {
        members.extend([
            ("squad-owl", SELF_ID, "leader"),
            ("squad-owl", "synthetic-friend-bob", "co_leader"),
            ("squad-owl", "synthetic-friend-zoe", "member"),
        ]);
    } else {
        members.extend([
            ("squad-owl", "synthetic-friend-bob", "leader"),
            ("squad-owl", "synthetic-friend-zoe", "member"),
        ]);
    }
    for (i, (s, u, r)) in members.into_iter().enumerate() {
        x.members.push(MemberRow {
            squad: s.into(),
            user: u.into(),
            role: r.into(),
            joined_at: now - (30 - i as i64) * 24 * 60 * minute,
        });
    }
    if in_squad {
        // the user leads a private squad with one join request (from a squadless user)
        x.squads[0].private = true;
        x.joins.push(JoinRow {
            id: "squad-request-mia".into(),
            squad: "squad-owl".into(),
            user: "synthetic-user-mia".into(),
            status: "pending",
            created_at: now - 45 * minute,
        });
        for (i, (u, body, ago)) in [
            ("synthetic-friend-bob", "library at 9?", 95),
            (SELF_ID, "yes, see you there", 90),
            ("synthetic-friend-zoe", "我也来 🦊", 60),
            ("synthetic-friend-bob", "bring snacks", 30),
        ]
        .into_iter()
        .enumerate()
        {
            x.messages.push(MessageRow {
                id: format!("message-seed-{i}"),
                squad: "squad-owl".into(),
                user: u.into(),
                body: body.into(),
                created_at: now - ago * minute,
            });
        }
    } else {
        x.joins.push(JoinRow {
            id: "squad-request-self".into(),
            squad: "squad-quiet".into(),
            user: SELF_ID.into(),
            status: "pending",
            created_at: now - 45 * minute,
        });
    }
    for (s, date, pts, mins, mc) in [
        ("squad-owl", "2026-09-20", 3, 410, 3),
        ("squad-owl", "2026-09-21", 2, 300, 3),
        ("squad-lib", "2026-09-20", 2, 260, 1),
        ("squad-lib", "2026-09-21", 3, 380, 1),
        ("squad-full", "2026-09-20", 1, 500, 4),
        ("squad-full", "2026-09-21", 1, 420, 4),
        ("squad-math", "2026-08-10", 3, 200, 1),
    ] {
        x.scores.push(ScoreRow {
            squad: s.into(),
            date: date.into(),
            points: pts,
            total_minutes: mins,
            member_count: mc,
        });
    }
}

/// Points seeded photo avatars at the server's real origin (call after `MockServer::start`).
pub fn bind_origin(world: &mut World) {
    let origin = world.origin.clone();
    for u in world.users.values_mut() {
        if u.avatar["url"] == "AVATAR_URL" {
            u.avatar["url"] = json!(format!(
                "{origin}/profile/avatar/avatars%2F{}%2F1.png",
                u.id
            ));
        }
    }
}
