//! The mock Worker's state and handlers for the Stage 22a routes. Each handler follows the
//! production Worker's logic (`cloudflare/src/index.ts`): the same validation order, messages,
//! status codes, ordering and leaderboard rules (verified/baseline minutes only, migration 0019;
//! totals per migration 0022), Europe/Zurich days. The goldens recorded from the real Worker
//! (`tests/fixtures/social/worker-goldens.jsonl`) are the check that it does.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde_json::{json, Value};
use study_tracker_core::break_room::skribbl::zurich::zurich_date;
use study_tracker_core::dashboard::civil::CivilDate;
use study_tracker_core::timer::WallTimestamp;

use crate::http::{percent_decode, Request, Response};

#[derive(Debug, Clone)]
pub struct User {
    pub id: String,
    pub secret: String,
    pub friend_code: String,
    pub display_name: String,
    pub avatar: Value,
    pub is_private: bool,
    pub show_hours_to_friends: bool,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone)]
pub struct FriendRequestRow {
    pub id: String,
    pub from: String,
    pub to: String,
    pub status: &'static str,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct DrawingRow {
    pub id: String,
    pub date: String,
    pub user: String,
    pub key: String,
    pub mime: String,
    pub bytes: Vec<u8>,
    pub created_at: i64,
}

/// Everything the mock knows. Times are unix milliseconds from the mock clock.
#[derive(Debug, Default)]
pub struct World {
    pub now_ms: i64,
    pub origin: String,
    pub users: BTreeMap<String, User>,
    pub friendships: HashSet<(String, String)>,
    pub requests: Vec<FriendRequestRow>,
    /// competitive minutes per (user, date) - verified sessions and daily baselines
    pub daily: BTreeMap<(String, String), (u64, u64)>,
    /// leaderboard_baselines (overall only)
    pub baselines: HashMap<String, (u64, u64)>,
    pub themes: BTreeMap<String, String>,
    pub theme_pool: Vec<String>,
    pub drawings: Vec<DrawingRow>,
    pub votes: HashMap<(String, String), i64>,
    pub avatars: HashMap<String, (String, Vec<u8>)>,
    pub seq: u64,
}

fn sqlite_time(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = CivilDate::from_days(days).ymd();
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3_600_000,
        (rem / 60_000) % 60,
        (rem / 1000) % 60
    )
}

fn pair(a: &str, b: &str) -> (String, String) {
    if a < b {
        (a.into(), b.into())
    } else {
        (b.into(), a.into())
    }
}

/// `parseListAvatar`: a data: URI photo is hidden from lists.
fn list_avatar(user: &User) -> Value {
    if user.avatar["kind"] == "photo"
        && user.avatar["url"]
            .as_str()
            .is_some_and(|u| u.starts_with("data:image/"))
    {
        return json!({"kind": "letter", "letter": first_letter(&user.display_name), "style": "classic"});
    }
    user.avatar.clone()
}

fn first_letter(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map_or_else(|| "S".into(), |c| c.to_uppercase().collect())
}

fn clean_avatar(raw: &Value, name: &str) -> Value {
    let default = json!({"kind": "letter", "letter": first_letter(name), "style": "classic"});
    match raw["kind"].as_str() {
        Some("icon") if raw["icon"].is_string() => json!({"kind": "icon", "icon": raw["icon"]}),
        Some("letter") => {
            let letter = raw["letter"]
                .as_str()
                .filter(|l| l.len() == 1 && l.chars().all(|c| c.is_ascii_alphabetic()))
                .map_or_else(|| first_letter(name), str::to_uppercase);
            let style = raw["style"]
                .as_str()
                .filter(|s| {
                    ["classic", "serif", "cursive", "graffiti", "pixel", "mono"].contains(s)
                })
                .unwrap_or("classic");
            json!({"kind": "letter", "letter": letter, "style": style})
        }
        Some("photo")
            if raw["url"]
                .as_str()
                .is_some_and(|u| u.contains("/profile/avatar/")) =>
        {
            raw.clone()
        }
        _ => default,
    }
}

impl World {
    pub fn new(origin: &str, now_ms: i64) -> Self {
        Self {
            origin: origin.to_string(),
            now_ms,
            theme_pool: vec![
                "Lighthouse in the fog".into(),
                "Cat wearing a wizard hat".into(),
                "Treehouse in Autumn".into(),
            ],
            ..Default::default()
        }
    }

    pub fn today(&self) -> String {
        zurich_date(WallTimestamp::from_unix_millis(self.now_ms)).to_iso()
    }

    pub fn yesterday(&self) -> String {
        zurich_date(WallTimestamp::from_unix_millis(self.now_ms))
            .add_days(-1)
            .to_iso()
    }

    fn week_start(&self) -> String {
        let today = zurich_date(WallTimestamp::from_unix_millis(self.now_ms));
        today
            .add_days(-((i64::from(today.weekday()) + 6) % 7))
            .to_iso()
    }

    fn next_id(&mut self, prefix: &str) -> String {
        self.seq += 1;
        format!("{prefix}-{:08}", self.seq)
    }

    fn verify(&self, user_id: &str, secret: &str) -> Result<&User, Response> {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Err(Response::text(400, "Missing userId."));
        }
        if secret.trim().is_empty() {
            return Err(Response::text(400, "Missing deviceSecret."));
        }
        let user = self
            .users
            .get(user_id)
            .ok_or_else(|| Response::text(404, "User has not synced a profile yet."))?;
        if user.secret != secret.trim() {
            return Err(Response::text(403, "Invalid device secret."));
        }
        Ok(user)
    }

    fn touch(&mut self, user_id: &str) {
        let now = self.now_ms;
        if let Some(u) = self.users.get_mut(user_id) {
            u.last_seen_at = now;
        }
    }

    pub fn snapshot(&self, user_id: &str, include_caches: bool) -> Value {
        let mut friends: Vec<&User> = self
            .friendships
            .iter()
            .filter_map(|(a, b)| {
                if a == user_id {
                    self.users.get(b)
                } else if b == user_id {
                    self.users.get(a)
                } else {
                    None
                }
            })
            .collect();
        friends.sort_by(|a, b| a.display_name.cmp(&b.display_name));
        let req = |r: &FriendRequestRow| {
            let (from, to) = (&self.users[&r.from], &self.users[&r.to]);
            json!({
                "id": r.id, "fromUserId": r.from, "toUserId": r.to,
                "fromDisplayName": from.display_name, "toDisplayName": to.display_name,
                "fromFriendCode": from.friend_code, "toFriendCode": to.friend_code,
                "status": r.status, "createdAt": sqlite_time(r.created_at),
                "fromAvatar": list_avatar(from), "toAvatar": list_avatar(to),
            })
        };
        let mut incoming: Vec<&FriendRequestRow> = self
            .requests
            .iter()
            .filter(|r| r.to == user_id && r.status == "pending")
            .collect();
        incoming.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        let mut outgoing: Vec<&FriendRequestRow> = self
            .requests
            .iter()
            .filter(|r| r.from == user_id && r.status == "pending")
            .collect();
        outgoing.sort_by_key(|r| std::cmp::Reverse(r.created_at));
        let mut social = json!({
            "friends": friends.iter().map(|u| json!({
                "userId": u.id, "displayName": u.display_name, "friendCode": u.friend_code,
                "friendsSince": sqlite_time(self.now_ms), "lastSeenAt": sqlite_time(u.last_seen_at), "avatar": list_avatar(u),
            })).collect::<Vec<_>>(),
            "incomingFriendRequests": incoming.into_iter().map(req).collect::<Vec<_>>(),
            "outgoingFriendRequests": outgoing.into_iter().map(req).collect::<Vec<_>>(),
            "squad": null, "outgoingSquadRequests": [], "incomingSquadRequests": [], "squadMessages": [],
        });
        if include_caches {
            let board = |scope: &str| {
                json!({
                    "daily": self.leaderboard(user_id, scope, "daily"),
                    "weekly": self.leaderboard(user_id, scope, "weekly"),
                    "overall": self.leaderboard(user_id, scope, "overall"),
                })
            };
            social["cachedLeaderboards"] = json!({"global": board("global"), "friends": board("friends"), "squad": {"daily": [], "weekly": [], "overall": []}});
            social["cachedFeeds"] = json!({"global": [], "friends": []});
            social["cachedSquadScoreLeaderboards"] =
                json!({"daily": [], "season": [], "overall": []});
        }
        json!({ "social": social })
    }

    fn friend_ids(&self, user_id: &str) -> Vec<String> {
        self.friendships
            .iter()
            .filter_map(|(a, b)| {
                if a == user_id {
                    Some(b.clone())
                } else if b == user_id {
                    Some(a.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// `getLeaderboard` (global / friends; squad is Stage 22b and empty here).
    pub fn leaderboard(&self, user_id: &str, scope: &str, period: &str) -> Vec<Value> {
        if scope == "squad" {
            return Vec::new();
        }
        let allowed: Option<HashSet<String>> = (scope == "friends").then(|| {
            let mut s: HashSet<String> = self.friend_ids(user_id).into_iter().collect();
            s.insert(user_id.to_string());
            s
        });
        let (today, week) = (self.today(), self.week_start());
        let mut rows: Vec<(String, u64, u64, Option<String>, &User)> = self
            .users
            .values()
            .filter(|u| scope != "global" || !u.is_private)
            .filter(|u| allowed.as_ref().map_or(true, |a| a.contains(&u.id)))
            .filter(|u| scope != "friends" || u.id == user_id || u.show_hours_to_friends)
            .map(|u| {
                let mut minutes = 0;
                let mut sessions = 0;
                let mut last: Option<String> = None;
                for ((uid, date), (m, s)) in &self.daily {
                    if uid != &u.id {
                        continue;
                    }
                    let counted = match period {
                        "daily" => *date == today,
                        "weekly" => *date >= week,
                        _ => true,
                    };
                    if counted {
                        minutes += m;
                        sessions += s;
                        if period != "overall"
                            && last.as_deref().map_or(true, |l| date.as_str() > l)
                        {
                            last = Some(date.clone());
                        }
                    }
                    if period == "overall" && last.as_deref().map_or(true, |l| date.as_str() > l) {
                        last = Some(date.clone());
                    }
                }
                if period == "overall" {
                    let (bm, bs) = self.baselines.get(&u.id).copied().unwrap_or((0, 0));
                    minutes += bm;
                    sessions += bs;
                }
                (u.display_name.clone(), minutes, sessions, last, u)
            })
            .collect();
        // overall joins competitive_user_totals (every user has a row only once synced)
        rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        rows.truncate(50);
        rows.into_iter()
            .enumerate()
            .map(|(i, (name, minutes, sessions, last, u))| {
                json!({
                    "userId": u.id, "displayName": name, "friendCode": u.friend_code,
                    "minutes": minutes, "sessions": sessions, "lastActiveDate": last,
                    "avatar": list_avatar(u), "rank": i + 1, "isSelf": u.id == user_id,
                })
            })
            .collect()
    }

    fn drawing_url(&self, key: &str) -> String {
        let encoded: String = key
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        format!("{}/skribbl/drawing/{encoded}", self.origin)
    }

    fn ensure_theme(&mut self) -> String {
        let today = self.today();
        if let Some(t) = self.themes.get(&today) {
            return t.clone();
        }
        let yesterday_theme = self.themes.get(&self.yesterday()).cloned();
        let pick = self
            .theme_pool
            .iter()
            .find(|t| Some(*t) != yesterday_theme.as_ref())
            .cloned()
            .unwrap_or_else(|| "Lighthouse in the fog".into());
        self.themes.insert(today, pick.clone());
        pick
    }

    fn winner(&self, date: &str) -> Option<(&DrawingRow, i64)> {
        let mut best: Option<(&DrawingRow, i64)> = None;
        for d in self.drawings.iter().filter(|d| d.date == date) {
            let score: i64 = self
                .votes
                .iter()
                .filter(|((did, _), _)| did == &d.id)
                .map(|(_, v)| *v)
                .sum();
            match best {
                Some((b, s)) if s > score || (s == score && b.created_at <= d.created_at) => {}
                _ => best = Some((d, score)),
            }
        }
        best
    }

    pub fn handle(&mut self, req: &Request) -> Response {
        let json_body = || serde_json::from_slice::<Value>(&req.body).unwrap_or(Value::Null);
        let field = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_string();
        let route = (req.method.as_str(), req.path.as_str());
        let result: Result<Response, Response> = (|| match route {
            ("GET", "/health") => Ok(Response::json(&json!({"ok": true}))),
            ("POST", "/sync/v2") => {
                let body = json_body();
                let user = &body["user"];
                if !user.is_object() {
                    return Ok(Response::text(400, "Missing user identity."));
                }
                if !body["stats"].is_array() {
                    return Ok(Response::text(400, "Missing stats."));
                }
                let (id, secret, code) = (
                    field(user, "userId").trim().to_string(),
                    field(user, "deviceSecret"),
                    field(user, "friendCode").trim().to_uppercase(),
                );
                if id.is_empty() || secret.is_empty() || code.is_empty() {
                    return Ok(Response::text(400, "Missing user identity."));
                }
                if let Some(existing) = self.users.get(&id) {
                    if existing.secret != secret {
                        return Ok(Response::text(403, "Invalid device secret."));
                    }
                }
                if self
                    .users
                    .values()
                    .any(|u| u.friend_code == code && u.id != id)
                {
                    return Ok(Response::text(409, "Friend code is already in use."));
                }
                let name = {
                    let n: String = field(user, "displayName").trim().chars().take(48).collect();
                    if n.is_empty() {
                        "Student".to_string()
                    } else {
                        n
                    }
                };
                let avatar = clean_avatar(&user["avatar"], &name);
                let now = self.now_ms;
                let entry = self.users.entry(id.clone()).or_insert_with(|| User {
                    id: id.clone(),
                    secret: secret.clone(),
                    friend_code: code.clone(),
                    display_name: name.clone(),
                    avatar: avatar.clone(),
                    is_private: false,
                    show_hours_to_friends: true,
                    last_seen_at: now,
                });
                entry.friend_code = code;
                entry.display_name = name;
                entry.avatar = avatar;
                entry.is_private = user["isPrivate"].as_bool().unwrap_or(false);
                entry.show_hours_to_friends = user["showHoursToFriends"].as_bool().unwrap_or(true);
                entry.last_seen_at = now;
                Ok(Response::json(
                    &json!({"ok": true, "syncedAt": crate::iso(self.now_ms)}),
                ))
            }
            ("POST", "/presence") => {
                let body = json_body();
                self.verify(&field(&body, "userId"), &field(&body, "deviceSecret"))?;
                self.touch(&field(&body, "userId"));
                Ok(Response::json(&json!({"ok": true})))
            }
            ("POST", "/friends/status/v2") | ("GET", "/friends/status/v2") => {
                let body = json_body();
                let id = field(&body, "userId");
                self.verify(&id, &field(&body, "deviceSecret"))?;
                self.touch(&id);
                Ok(Response::json(&self.snapshot(id.trim(), false)))
            }
            ("POST", "/friends/request") => {
                let body = json_body();
                let from = field(&body, "userId").trim().to_string();
                self.verify(&from, &field(&body, "deviceSecret"))?;
                let code = field(&body, "friendCode").trim().to_uppercase();
                let Some(target) = self
                    .users
                    .values()
                    .find(|u| u.friend_code == code)
                    .map(|u| u.id.clone())
                else {
                    return Ok(Response::text(404, "No user with that friend code exists."));
                };
                if target == from {
                    return Ok(Response::text(400, "You cannot add yourself."));
                }
                if self.friendships.contains(&pair(&from, &target)) {
                    return Ok(Response::text(409, "You are already friends."));
                }
                if let Some(r) = self
                    .requests
                    .iter_mut()
                    .find(|r| r.from == target && r.to == from && r.status == "pending")
                {
                    r.status = "accepted";
                    self.friendships.insert(pair(&from, &target));
                    return Ok(Response::json(&self.snapshot(&from, true)));
                }
                let now = self.now_ms;
                if let Some(r) = self
                    .requests
                    .iter_mut()
                    .find(|r| r.from == from && r.to == target)
                {
                    r.status = "pending";
                } else {
                    let id = self.next_id("request");
                    self.requests.push(FriendRequestRow {
                        id,
                        from: from.clone(),
                        to: target,
                        status: "pending",
                        created_at: now,
                    });
                }
                Ok(Response::json(&self.snapshot(&from, true)))
            }
            ("POST", "/friends/respond") => {
                let body = json_body();
                let user = field(&body, "userId").trim().to_string();
                self.verify(&user, &field(&body, "deviceSecret"))?;
                let rid = field(&body, "requestId");
                let Some(r) = self
                    .requests
                    .iter_mut()
                    .find(|r| r.id == rid && r.to == user && r.status == "pending")
                else {
                    return Ok(Response::text(404, "Friend request not found."));
                };
                let accepted = body["response"] == "accepted";
                r.status = if accepted { "accepted" } else { "declined" };
                let p = pair(&r.from, &r.to);
                if accepted {
                    self.friendships.insert(p);
                }
                Ok(Response::json(&self.snapshot(&user, true)))
            }
            ("POST", "/leaderboard") | ("GET", "/leaderboard") => {
                let body = json_body();
                let id = field(&body, "userId").trim().to_string();
                self.verify(&id, &field(&body, "deviceSecret"))?;
                let scope = match body["scope"].as_str() {
                    Some(s @ ("friends" | "squad")) => s,
                    _ => "global",
                };
                let period = match body["period"].as_str() {
                    Some(p @ ("daily" | "overall")) => p,
                    _ => "weekly",
                };
                Ok(Response::json(
                    &json!({"entries": self.leaderboard(&id, scope, period)}),
                ))
            }
            ("POST", "/player-stats") => {
                let body = json_body();
                let id = field(&body, "userId").trim().to_string();
                self.verify(&id, &field(&body, "deviceSecret"))?;
                let target_id = field(&body, "targetUserId").trim().to_string();
                let Some(target) = self.users.get(&target_id).cloned() else {
                    return Ok(Response::text(404, "User not found."));
                };
                let friends = self.friendships.contains(&pair(&id, &target_id));
                if !friends && target_id != id {
                    return Ok(Response::text(403, "User is private."));
                }
                let visible = target_id == id || !friends || target.show_hours_to_friends;
                let stat = |period: &str| {
                    self.leaderboard(&target_id, "global", period)
                        .into_iter()
                        .find(|e| e["userId"] == target_id.as_str())
                        .map_or(json!({"minutes": 0, "sessions": 0, "lastActiveDate": null}), |e| json!({"minutes": e["minutes"], "sessions": e["sessions"], "lastActiveDate": e["lastActiveDate"]}))
                };
                Ok(Response::json(&json!({
                    "displayName": target.display_name, "friendCode": target.friend_code, "avatar": target.avatar.clone(),
                    "lastSeenAt": sqlite_time(target.last_seen_at), "hoursVisible": visible,
                    "daily": if visible { stat("daily") } else { Value::Null },
                    "weekly": if visible { stat("weekly") } else { Value::Null },
                    "overall": if visible { stat("overall") } else { Value::Null },
                })))
            }
            ("GET", "/skribbl/theme") => {
                let q = |k: &str| {
                    req.query
                        .iter()
                        .find(|(qk, _)| qk == k)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default()
                };
                let id = q("userId");
                self.verify(&id, &q("deviceSecret"))?;
                self.touch(&id);
                let theme = self.ensure_theme();
                let today = self.today();
                let mine = self
                    .drawings
                    .iter()
                    .find(|d| d.user == id && d.date == today);
                Ok(Response::json(&json!({
                    "date": today, "theme": theme, "submitted": mine.is_some(),
                    "drawingId": mine.map(|d| d.id.clone()), "imageUrl": mine.map(|d| self.drawing_url(&d.key)),
                })))
            }
            ("POST", "/skribbl/gallery") | ("GET", "/skribbl/gallery") => {
                let body = json_body();
                let id = field(&body, "userId").trim().to_string();
                self.verify(&id, &field(&body, "deviceSecret"))?;
                self.touch(&id);
                let date = match body["date"].as_str().map(str::trim) {
                    None | Some("") => self.today(),
                    Some(d) if d.len() == 10 && CivilDate::parse_iso(d).is_some() => d.to_string(),
                    Some(_) => return Ok(Response::text(400, "Invalid date. Use YYYY-MM-DD.")),
                };
                let limit = body["limit"].as_f64().unwrap_or(16.0).clamp(12.0, 20.0) as usize;
                let offset = body["offset"].as_f64().unwrap_or(0.0).max(0.0) as usize;
                let mut rows: Vec<&DrawingRow> =
                    self.drawings.iter().filter(|d| d.date == date).collect();
                rows.sort_by(|a, b| {
                    a.created_at
                        .cmp(&b.created_at)
                        .then_with(|| a.id.cmp(&b.id))
                });
                let total = rows.len();
                let page: Vec<Value> = rows.iter().skip(offset).take(limit).map(|d| {
                    let score: i64 = self.votes.iter().filter(|((did, _), _)| did == &d.id).map(|(_, v)| *v).sum();
                    let count = self.votes.keys().filter(|(did, _)| did == &d.id).count();
                    let mine = self.votes.get(&(d.id.clone(), id.clone()));
                    json!({
                        "id": d.id, "userId": d.user, "displayName": self.users[&d.user].display_name,
                        "voteScore": score, "voteCount": count, "myVote": mine, "isSelf": d.user == id,
                        "imageUrl": self.drawing_url(&d.key), "createdAt": sqlite_time(d.created_at),
                    })
                }).collect();
                let shown = offset + page.len();
                Ok(Response::json(&json!({
                    "date": date, "drawings": page, "total": total,
                    "nextOffset": if shown < total { json!(shown) } else { Value::Null }, "hasMore": shown < total,
                })))
            }
            ("POST", "/skribbl/submit") => {
                let form =
                    crate::multipart::parse(req.content_type.as_deref().unwrap_or(""), &req.body);
                let text = |k: &str| {
                    form.iter()
                        .find(|p| p.name == k)
                        .map(|p| String::from_utf8_lossy(&p.data).trim().to_string())
                        .unwrap_or_default()
                };
                let id = text("userId");
                self.verify(&id, &text("deviceSecret"))?;
                let today = self.today();
                let requested = text("date");
                if !requested.is_empty() && requested != today {
                    return Ok(Response::text(
                        400,
                        "Drawings are only accepted for today's theme.",
                    ));
                }
                if self
                    .drawings
                    .iter()
                    .any(|d| d.user == id && d.date == today)
                {
                    return Ok(Response::text(
                        409,
                        "You already submitted a drawing today.",
                    ));
                }
                let Some(image) = form
                    .iter()
                    .find(|p| p.name == "image" && p.file_name.is_some())
                else {
                    return Ok(Response::text(400, "Missing drawing image."));
                };
                let mime = image.content_type.clone().unwrap_or_default();
                if mime != "image/png" && mime != "image/webp" {
                    return Ok(Response::text(400, "Use PNG or WebP images."));
                }
                if image.data.len() > 1_572_864 {
                    return Ok(Response::text(
                        413,
                        "Drawing is too large. Keep it under 1.5 MB.",
                    ));
                }
                self.ensure_theme();
                let ext = if mime == "image/png" { "png" } else { "webp" };
                let key = format!("drawings/{today}/{id}.{ext}");
                let drawing_id = self.next_id("drawing");
                let now = self.now_ms;
                self.drawings.push(DrawingRow {
                    id: drawing_id.clone(),
                    date: today.clone(),
                    user: id,
                    key: key.clone(),
                    mime,
                    bytes: image.data.clone(),
                    created_at: now,
                });
                Ok(Response::json(
                    &json!({"ok": true, "drawingId": drawing_id, "date": today, "imageUrl": self.drawing_url(&key)}),
                ))
            }
            ("POST", "/skribbl/vote") => {
                let body = json_body();
                let id = field(&body, "userId").trim().to_string();
                self.verify(&id, &field(&body, "deviceSecret"))?;
                let drawing_id: String =
                    field(&body, "drawingId").trim().chars().take(80).collect();
                let vote = body["vote"].as_f64().unwrap_or(0.0);
                if ![-1.0, 0.0, 1.0].contains(&vote) {
                    return Ok(Response::text(400, "Invalid vote value."));
                }
                let Some(d) = self.drawings.iter().find(|d| d.id == drawing_id) else {
                    return Ok(Response::text(404, "Drawing not found."));
                };
                if d.user == id {
                    return Ok(Response::text(403, "You cannot vote on your own drawing."));
                }
                if d.date != self.today() {
                    return Ok(Response::text(400, "Voting is closed for this drawing."));
                }
                let key = (drawing_id.clone(), id);
                if vote == 0.0 {
                    self.votes.remove(&key);
                } else {
                    self.votes.insert(key, vote as i64);
                }
                let score: i64 = self
                    .votes
                    .iter()
                    .filter(|((did, _), _)| did == &drawing_id)
                    .map(|(_, v)| *v)
                    .sum();
                Ok(Response::json(&json!({"ok": true, "score": score})))
            }
            ("GET", "/skribbl/leaderboard") => {
                let q = |k: &str| {
                    req.query
                        .iter()
                        .find(|(qk, _)| qk == k)
                        .map(|(_, v)| v.clone())
                        .unwrap_or_default()
                };
                let id = q("userId");
                self.verify(&id, &q("deviceSecret"))?;
                self.touch(&id);
                let yesterday = self.yesterday();
                let winner = self.winner(&yesterday).map(|(d, score)| json!({
                    "date": yesterday, "drawingId": d.id, "userId": d.user, "displayName": self.users[&d.user].display_name,
                    "score": score, "imageUrl": self.drawing_url(&d.key),
                }));
                Ok(Response::json(
                    &json!({"date": yesterday, "winner": winner}),
                ))
            }
            ("GET", path) if path.starts_with("/skribbl/drawing/") => {
                let key = percent_decode(&path["/skribbl/drawing/".len()..]);
                match self.drawings.iter().find(|d| d.key == key) {
                    Some(d) => Ok(Response {
                        status: 200,
                        content_type: if d.mime == "image/png" {
                            "image/png"
                        } else {
                            "image/webp"
                        },
                        body: d.bytes.clone(),
                        extra_headers: vec![("cache-control", "public, max-age=1800".into())],
                    }),
                    None => Ok(Response::text(404, "Drawing not found or purged.")),
                }
            }
            ("GET", path) if path.starts_with("/profile/avatar/") => {
                let key = percent_decode(&path["/profile/avatar/".len()..]);
                match self.avatars.get(&key) {
                    Some((_, bytes)) => Ok(Response {
                        status: 200,
                        content_type: "image/png",
                        body: bytes.clone(),
                        extra_headers: vec![("cache-control", "public, max-age=86400".into())],
                    }),
                    None => Ok(Response::text(404, "Avatar not found.")),
                }
            }
            _ => Ok(Response::text(404, "Not found.")),
        })();
        result.unwrap_or_else(|e| e)
    }
}
