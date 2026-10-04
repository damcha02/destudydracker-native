//! Deterministic synthetic worlds. Every name, code and secret here is invented; the self user's
//! secret is the recognisable `TEST_SECRET_MUST_NOT_APPEAR`, so any leak into a log, screenshot
//! or diagnostic is easy to search for.

use serde_json::json;

use crate::world::{DrawingRow, FriendRequestRow, User, World};

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
