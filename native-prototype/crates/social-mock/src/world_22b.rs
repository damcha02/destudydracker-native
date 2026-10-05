//! The mock Worker's Stage 22b routes: the feed (posts from `/sync/v2`, reactions, polls,
//! comments, edits, deletes, images), the profile avatar upload, squads (snapshot, search,
//! details, join/request, respond, leave, chat, roles, kick, settings, scoreboards), verified
//! sessions (start, heartbeat, finish, offline reconcile), announcements, opt-in telemetry and
//! the owner-only usage view. Each follows `cloudflare/src/index.ts`: validation order, messages,
//! status codes, ordering, limits and the permission rules.

use std::collections::{BTreeMap, HashSet};

use serde_json::{json, Value};

use crate::http::{percent_decode, Request, Response};
use crate::world::World;

/// `R2_OWNER_FRIEND_CODE`.
pub const OWNER_CODE: &str = "ZRWL-WKNF";
pub const MAX_SQUAD_MEMBERS: usize = 4;
const FEED_IMAGE_TTL_MS: i64 = 5 * 24 * 60 * 60 * 1000;
const HEARTBEAT_MS: i64 = 15 * 60 * 1000;
const GRACE_MS: i64 = 5 * 60 * 1000;
const NORMAL_CREDIT_GRACE_MS: i64 = 2 * 60 * 60 * 1000;
const MAX_SESSION_MS: i64 = 4 * 60 * 60 * 1000;

#[derive(Debug, Clone)]
pub struct PostRow {
    pub id: String,
    pub user: String,
    pub kind: String,
    pub subject: String,
    pub detail: String,
    pub note: String,
    pub icon: String,
    pub minutes: u64,
    pub preset: String,
    pub created_at: String,
    pub image_key: Option<String>,
    pub image_mime: Option<String>,
    pub image_expires: Option<i64>,
    pub image_expired_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PollRow {
    pub question: String,
    pub multiple: bool,
    /// (option id, text) in sort order
    pub options: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct CommentRow {
    pub id: String,
    pub post: String,
    pub user: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct SquadRow {
    pub id: String,
    pub name: String,
    pub private: bool,
    pub created_by: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct MemberRow {
    pub squad: String,
    pub user: String,
    pub role: String,
    pub joined_at: i64,
}

#[derive(Debug, Clone)]
pub struct JoinRow {
    pub id: String,
    pub squad: String,
    pub user: String,
    pub status: &'static str,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct MessageRow {
    pub id: String,
    pub squad: String,
    pub user: String,
    pub body: String,
    pub created_at: i64,
}

#[derive(Debug, Clone)]
pub struct VerifiedRow {
    pub id: String,
    pub user: String,
    pub started: i64,
    pub last_heartbeat: i64,
    pub finished: bool,
    pub credited: u64,
}

/// One squad's scored day (`squad_daily_scores`).
#[derive(Debug, Clone)]
pub struct ScoreRow {
    pub squad: String,
    pub date: String,
    pub points: i64,
    pub total_minutes: u64,
    pub member_count: u64,
}

/// What a telemetry heartbeat carried (the test checks the keys).
#[derive(Debug, Clone)]
pub struct TelemetryHit {
    pub keys: Vec<String>,
    pub install_id: String,
}

#[derive(Debug, Default)]
pub struct Extra {
    pub posts: Vec<PostRow>,
    pub polls: BTreeMap<String, PollRow>,
    /// (post, option, user)
    pub poll_votes: Vec<(String, String, String)>,
    /// (post, user, emoji)
    pub reactions: Vec<(String, String, String)>,
    pub comments: Vec<CommentRow>,
    pub images: BTreeMap<String, Vec<u8>>,
    pub squads: Vec<SquadRow>,
    pub members: Vec<MemberRow>,
    pub joins: Vec<JoinRow>,
    pub messages: Vec<MessageRow>,
    pub scores: Vec<ScoreRow>,
    pub verified: Vec<VerifiedRow>,
    /// (anchor, gap end ms) per reconcile call
    pub reconciles: Vec<(String, i64)>,
    pub offline_credit: BTreeMap<(String, String), u64>,
    pub telemetry: Vec<TelemetryHit>,
    /// (id, title, body, target version, active)
    pub announcements: Vec<(String, String, String, Option<String>, bool)>,
    pub r2_paused: bool,
    pub r2_warning: bool,
}

fn sqlite_time(ms: i64) -> String {
    crate::iso(ms).replace('T', " ")[..19].to_string()
}

fn encode(key: &str) -> String {
    key.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// `cleanText(value, max)`: trimmed, cut at `max` UTF-16 units.
fn clean_text(v: &Value, max: usize) -> String {
    let s = match v {
        Value::String(s) => s.trim().to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let mut units = 0;
    s.chars()
        .take_while(|c| {
            units += c.len_utf16();
            units <= max
        })
        .collect()
}

fn role_rank(role: &str) -> u8 {
    match role {
        "leader" => 4,
        "co_leader" => 3,
        "elder" => 2,
        _ => 1,
    }
}

fn can_kick(actor: &str, target: &str) -> bool {
    match actor {
        "leader" => target != "leader",
        "co_leader" => role_rank(target) < role_rank("co_leader"),
        "elder" => target == "member",
        _ => false,
    }
}

fn can_change_role(actor: &str, target: &str, next: &str) -> bool {
    match actor {
        "leader" => next != "leader",
        "co_leader" => role_rank(target) < 3 && role_rank(next) < 3,
        _ => false,
    }
}

fn mime_static(mime: &str) -> &'static str {
    match mime {
        "image/png" => "image/png",
        "image/jpeg" => "image/jpeg",
        "image/gif" => "image/gif",
        _ => "image/webp",
    }
}

impl World {
    fn uid(body: &Value) -> String {
        body["userId"].as_str().unwrap_or("").trim().to_string()
    }

    fn auth(&self, body: &Value) -> Result<String, Response> {
        let id = Self::uid(body);
        self.verify_pub(&id, body["deviceSecret"].as_str().unwrap_or(""))?;
        Ok(id)
    }

    fn is_owner(&self, id: &str) -> bool {
        self.users
            .get(id)
            .is_some_and(|u| u.friend_code == OWNER_CODE)
    }

    pub fn r2_usage(&self) -> Value {
        json!({
            "month": crate::iso(self.now_ms)[..7].to_string(),
            "storageBytes": self.x.images.values().map(Vec::len).sum::<usize>(),
            "classAOps": 12, "classBOps": 340,
            "warning": self.x.r2_warning || self.x.r2_paused, "paused": self.x.r2_paused,
            "limits": {
                "storageWarningBytes": 7_000_000_000u64, "storageHardBytes": 8_000_000_000u64,
                "classAWarningMonthly": 700_000, "classAHardMonthly": 800_000,
                "classBWarningMonthly": 7_000_000, "classBHardMonthly": 8_000_000,
            },
        })
    }

    // -------------------------------------------------------------------------------- feed

    /// `upsertFeedPosts` (called by `/sync/v2`).
    pub fn upsert_feed_posts(&mut self, user: &str, posts: &Value) -> Result<(), Response> {
        let Some(list) = posts.as_array() else {
            return Ok(());
        };
        let list: Vec<&Value> = list.iter().take(25).collect();
        for p in &list {
            let id = clean_text(&p["id"], 80);
            if self.x.posts.iter().any(|r| r.id == id && r.user != user) {
                return Err(Response::text(409, "A feed post belongs to another user."));
            }
        }
        for p in list {
            let id = clean_text(&p["id"], 80);
            if id.is_empty() {
                continue;
            }
            let created = p["createdAt"]
                .as_str()
                .filter(|s| s.len() > 11 && s.as_bytes()[10] == b'T')
                .map(str::to_string)
                .unwrap_or_else(|| crate::iso(self.now_ms));
            let row = PostRow {
                id: id.clone(),
                user: user.to_string(),
                kind: if p["type"] == "milestone" {
                    "milestone"
                } else {
                    "session"
                }
                .into(),
                subject: clean_text(&p["subject"], 80),
                detail: clean_text(&p["detail"], 80),
                note: clean_text(&p["note"], 220),
                icon: clean_text(&p["icon"], 8),
                minutes: p["minutes"]
                    .as_f64()
                    .unwrap_or(0.0)
                    .clamp(0.0, 1440.0)
                    .round() as u64,
                preset: clean_text(&p["presetLabel"], 60),
                created_at: created,
                image_key: None,
                image_mime: None,
                image_expires: None,
                image_expired_at: None,
            };
            match self.x.posts.iter_mut().find(|r| r.id == id) {
                Some(existing) => {
                    existing.subject = row.subject;
                    existing.detail = row.detail;
                    existing.note = row.note;
                    existing.icon = row.icon;
                    existing.minutes = row.minutes;
                    existing.preset = row.preset;
                }
                None => self.x.posts.push(row),
            }
            if p.get("poll").is_some() {
                let poll = &p["poll"];
                let question = clean_text(&poll["question"], 180)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut seen = HashSet::new();
                let options: Vec<(String, String)> = poll["options"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|o| {
                                let text = clean_text(&o["text"], 100)
                                    .split_whitespace()
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                (!text.is_empty() && seen.insert(text.to_lowercase())).then(|| {
                                    let oid = clean_text(&o["id"], 80);
                                    (
                                        if oid.is_empty() {
                                            format!("opt-{text}")
                                        } else {
                                            oid
                                        },
                                        text,
                                    )
                                })
                            })
                            .take(12)
                            .collect()
                    })
                    .unwrap_or_default();
                if question.is_empty() || options.len() < 2 {
                    self.x.polls.remove(&id);
                } else {
                    self.x.polls.insert(
                        id.clone(),
                        PollRow {
                            question,
                            multiple: poll["multiple"].as_bool().unwrap_or(false),
                            options,
                        },
                    );
                }
            }
        }
        Ok(())
    }

    fn can_view_post(&self, user: &str, post: &str) -> Result<(), Response> {
        let Some(p) = self.x.posts.iter().find(|p| p.id == post) else {
            return Err(Response::text(404, "Feed post not found."));
        };
        let private = self.users.get(&p.user).is_some_and(|u| u.is_private);
        if p.user == user || !private || self.are_friends(user, &p.user) {
            Ok(())
        } else {
            Err(Response::text(403, "You cannot react to this feed post."))
        }
    }

    fn poll_json(&self, user: &str, post: &str) -> Value {
        let Some(poll) = self.x.polls.get(post) else {
            return Value::Null;
        };
        let options: Vec<Value> = poll
            .options
            .iter()
            .map(|(id, text)| {
                let votes = self
                    .x
                    .poll_votes
                    .iter()
                    .filter(|(p, o, _)| p == post && o == id)
                    .count();
                let selected = self
                    .x
                    .poll_votes
                    .iter()
                    .any(|(p, o, u)| p == post && o == id && u == user);
                json!({"id": id, "text": text, "votes": votes, "selected": selected})
            })
            .collect();
        let total: u64 = options
            .iter()
            .map(|o| o["votes"].as_u64().unwrap_or(0))
            .sum();
        json!({"question": poll.question, "multiple": poll.multiple, "options": options, "totalVotes": total})
    }

    /// `getFeed`.
    pub fn feed(&self, user: &str, scope: &str) -> Vec<Value> {
        let friends: HashSet<String> = self.friend_ids_pub(user).into_iter().collect();
        let mut visible: HashSet<String> = friends.clone();
        visible.insert(user.to_string());
        let mut posts: Vec<&PostRow> = self
            .x
            .posts
            .iter()
            .filter(|p| {
                if scope == "friends" {
                    visible.contains(&p.user)
                } else {
                    self.users.get(&p.user).is_some_and(|u| !u.is_private)
                }
            })
            .collect();
        posts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        posts.truncate(40);
        posts
            .into_iter()
            .filter_map(|p| {
                let author = self.users.get(&p.user)?;
                let mut counts: BTreeMap<String, u64> = BTreeMap::new();
                let mut reacted = serde_json::Map::new();
                let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
                let mut rows: Vec<&(String, String, String)> =
                    self.x.reactions.iter().filter(|(pid, _, _)| pid == &p.id).collect();
                rows.sort_by(|a, b| {
                    a.2.cmp(&b.2).then_with(|| {
                        self.users[&a.1].display_name.cmp(&self.users[&b.1].display_name)
                    })
                });
                for (_, u, e) in rows {
                    *counts.entry(e.clone()).or_default() += 1;
                    if u == user {
                        reacted.insert(e.clone(), json!(true));
                    }
                    let r = &self.users[u];
                    if !r.is_private || visible.contains(u) {
                        by.entry(e.clone()).or_default().push(r.display_name.clone());
                    }
                }
                let mut reactions = serde_json::Map::new();
                for k in ["fire", "brain", "clap"] {
                    reactions.insert(k.into(), json!(0));
                }
                for (k, n) in counts {
                    reactions.insert(k, json!(n));
                }
                let mut comments: Vec<&CommentRow> =
                    self.x.comments.iter().filter(|c| c.post == p.id).collect();
                comments.sort_by_key(|c| c.created_at);
                let comments: Vec<Value> = comments
                    .into_iter()
                    .filter(|c| {
                        let u = &self.users[&c.user];
                        !u.is_private || visible.contains(&c.user)
                    })
                    .map(|c| self.comment_json(c, user))
                    .collect();
                let live_image = p
                    .image_key
                    .as_ref()
                    .filter(|_| p.image_expires.is_some_and(|e| e > self.now_ms));
                Some(json!({
                    "id": p.id, "userId": p.user, "displayName": author.display_name,
                    "friendCode": author.friend_code, "avatar": self.list_avatar_pub(author),
                    "type": p.kind, "subject": p.subject, "detail": p.detail, "note": p.note,
                    "icon": p.icon, "minutes": p.minutes, "presetLabel": p.preset,
                    "createdAt": p.created_at, "imageKey": live_image,
                    "imageMimeType": p.image_mime, "imageExpiresAt": p.image_expires.map(crate::iso),
                    "imageExpiredAt": p.image_expired_at,
                    "isSelf": p.user == user,
                    "imageUrl": live_image.map(|k| format!("{}/feed/image/{}", self.origin, encode(k))),
                    "poll": self.poll_json(user, &p.id),
                    "reactions": reactions, "reacted": reacted, "reactedBy": by,
                    "comments": comments,
                }))
            })
            .collect()
    }

    fn comment_json(&self, c: &CommentRow, viewer: &str) -> Value {
        let u = &self.users[&c.user];
        json!({
            "id": c.id, "postId": c.post, "userId": c.user, "displayName": u.display_name,
            "friendCode": u.friend_code, "avatar": self.list_avatar_pub(u), "body": c.body,
            "createdAt": sqlite_time(c.created_at), "isSelf": c.user == viewer,
        })
    }

    // ------------------------------------------------------------------------------ squads

    fn membership(&self, user: &str) -> Option<&MemberRow> {
        self.x.members.iter().find(|m| m.user == user)
    }

    fn member_count(&self, squad: &str) -> usize {
        self.x.members.iter().filter(|m| m.squad == squad).count()
    }

    fn user_total(&self, user: &str) -> (u64, u64) {
        let mut t = self.baselines_pub(user);
        for ((u, _), (m, s)) in self.daily.iter() {
            if u == user {
                t.0 += m;
                t.1 += s;
            }
        }
        t
    }

    fn member_json(&self, m: &MemberRow, viewer: &str, today_only: Option<&str>) -> Value {
        let u = &self.users[&m.user];
        let (minutes, sessions) = match today_only {
            Some(date) => self
                .daily
                .get(&(m.user.clone(), date.to_string()))
                .copied()
                .unwrap_or((0, 0)),
            None => self.user_total(&m.user),
        };
        json!({
            "userId": m.user, "displayName": u.display_name, "friendCode": u.friend_code,
            "avatar": self.list_avatar_pub(u), "role": m.role, "joinedAt": sqlite_time(m.joined_at),
            "lastSeenAt": sqlite_time(u.last_seen_at), "minutes": minutes, "sessions": sessions,
            "isSelf": m.user == viewer,
        })
    }

    fn sorted_members(&self, squad: &str) -> Vec<&MemberRow> {
        let mut ms: Vec<&MemberRow> = self.x.members.iter().filter(|m| m.squad == squad).collect();
        ms.sort_by(|a, b| {
            role_rank(&b.role)
                .cmp(&role_rank(&a.role))
                .then(a.joined_at.cmp(&b.joined_at))
        });
        ms
    }

    /// `getSquadSnapshot`.
    pub fn squad_snapshot(&self, user: &str) -> Value {
        let Some(me) = self.membership(user) else {
            let mut out: Vec<&JoinRow> = self
                .x
                .joins
                .iter()
                .filter(|j| j.user == user && j.status == "pending")
                .collect();
            out.sort_by_key(|j| std::cmp::Reverse(j.created_at));
            let outgoing: Vec<Value> = out
                .into_iter()
                .filter_map(|j| {
                    let s = self.x.squads.iter().find(|s| s.id == j.squad)?;
                    Some(json!({"id": j.id, "squadId": j.squad, "squadName": s.name, "isPrivate": s.private, "status": j.status, "createdAt": sqlite_time(j.created_at)}))
                })
                .collect();
            return json!({"squad": null, "outgoingSquadRequests": outgoing, "incomingSquadRequests": [], "squadMessages": []});
        };
        let squad = self.x.squads.iter().find(|s| s.id == me.squad).cloned();
        let members: Vec<Value> = self
            .sorted_members(&me.squad)
            .into_iter()
            .map(|m| self.member_json(m, user, None))
            .collect();
        let (tm, ts) =
            self.x
                .members
                .iter()
                .filter(|m| m.squad == me.squad)
                .fold((0, 0), |(a, b), m| {
                    let t = self.user_total(&m.user);
                    (a + t.0, b + t.1)
                });
        let incoming: Vec<Value> = if role_rank(&me.role) > 1 {
            let mut rows: Vec<&JoinRow> = self
                .x
                .joins
                .iter()
                .filter(|j| j.squad == me.squad && j.status == "pending")
                .collect();
            rows.sort_by_key(|j| std::cmp::Reverse(j.created_at));
            rows.into_iter()
                .map(|j| {
                    let u = &self.users[&j.user];
                    json!({"id": j.id, "squadId": j.squad, "userId": j.user, "displayName": u.display_name, "friendCode": u.friend_code, "avatar": self.list_avatar_pub(u), "status": j.status, "createdAt": sqlite_time(j.created_at)})
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut msgs: Vec<&MessageRow> = self
            .x
            .messages
            .iter()
            .filter(|m| {
                m.squad == me.squad
                    && self
                        .membership(&m.user)
                        .is_some_and(|x| x.squad == me.squad)
            })
            .collect();
        msgs.sort_by_key(|m| std::cmp::Reverse(m.created_at));
        msgs.truncate(60);
        msgs.reverse();
        let messages: Vec<Value> = msgs
            .into_iter()
            .map(|m| {
                let u = &self.users[&m.user];
                let role = self.membership(&m.user).map_or("member".to_string(), |x| x.role.clone());
                json!({"id": m.id, "squadId": m.squad, "userId": m.user, "displayName": u.display_name, "friendCode": u.friend_code, "avatar": self.list_avatar_pub(u), "role": role, "body": m.body, "createdAt": sqlite_time(m.created_at), "isSelf": m.user == user})
            })
            .collect();
        json!({
            "squad": squad.map(|s| json!({
                "id": s.id, "name": s.name, "isPrivate": s.private, "createdByUserId": s.created_by,
                "createdAt": sqlite_time(s.created_at), "totalMinutes": tm, "totalSessions": ts,
                "memberCount": members.len(), "myRole": me.role, "members": members,
            })),
            "incomingSquadRequests": incoming, "outgoingSquadRequests": [], "squadMessages": messages,
        })
    }

    /// `getSquadScoreLeaderboard`.
    pub fn squad_scores(&self, period: &str) -> Vec<Value> {
        if period == "daily" {
            let today = self.today();
            let mut rows: Vec<(String, String, bool, usize, usize, u64, u64)> = self
                .x
                .squads
                .iter()
                .filter_map(|s| {
                    let ms: Vec<&MemberRow> =
                        self.x.members.iter().filter(|m| m.squad == s.id).collect();
                    if ms.is_empty() {
                        return None;
                    }
                    let mut total = 0;
                    let mut sessions = 0;
                    let mut active = 0;
                    for m in &ms {
                        let (mm, ss) = self
                            .daily
                            .get(&(m.user.clone(), today.clone()))
                            .copied()
                            .unwrap_or((0, 0));
                        total += mm;
                        sessions += ss;
                        if mm > 0 {
                            active += 1;
                        }
                    }
                    Some((
                        s.id.clone(),
                        s.name.clone(),
                        s.private,
                        ms.len(),
                        active,
                        total,
                        sessions,
                    ))
                })
                .collect();
            let avg = |r: &(String, String, bool, usize, usize, u64, u64)| {
                if r.4 > 0 {
                    r.5 as f64 / r.4 as f64
                } else {
                    0.0
                }
            };
            rows.sort_by(|a, b| {
                avg(b)
                    .partial_cmp(&avg(a))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(b.5.cmp(&a.5))
                    .then(a.1.cmp(&b.1))
            });
            rows.truncate(50);
            let mut prev: Option<f64> = None;
            let mut prev_rank = 0;
            return rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let a = avg(r);
                    let rank = if prev == Some(a) { prev_rank } else { i + 1 };
                    prev = Some(a);
                    prev_rank = rank;
                    let points = match rank {
                        1 => 3,
                        2 => 2,
                        3 => 1,
                        _ => 0,
                    };
                    json!({"squadId": r.0, "squadName": r.1, "isPrivate": r.2, "memberCount": r.3, "totalMinutes": r.5, "totalSessions": r.6, "averageMinutes": a, "rank": rank, "points": points})
                })
                .collect();
        }
        let mut agg: BTreeMap<String, (i64, u64, u64, u64)> = BTreeMap::new();
        for s in &self.x.scores {
            let in_period = period != "season"
                || (s.date.as_str() >= "2026-09-15" && s.date.as_str() <= "2026-12-18");
            if !in_period || s.date.as_str() < "2026-07-29" {
                continue;
            }
            let e = agg.entry(s.squad.clone()).or_default();
            e.0 += s.points;
            e.1 += s.total_minutes;
            e.2 += s.member_count;
            e.3 += 1;
        }
        let mut rows: Vec<(String, (i64, u64, u64, u64))> = agg.into_iter().collect();
        let avg = |v: &(i64, u64, u64, u64)| {
            if v.2 > 0 {
                v.1 as f64 / v.2 as f64
            } else {
                0.0
            }
        };
        rows.sort_by(|a, b| {
            b.1 .0.cmp(&a.1 .0).then(
                avg(&b.1)
                    .partial_cmp(&avg(&a.1))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });
        let mut prev: Option<i64> = None;
        let mut prev_rank = 0;
        rows.iter()
            .enumerate()
            .filter_map(|(i, (sid, v))| {
                let s = self.x.squads.iter().find(|s| &s.id == sid)?;
                let rank = if prev == Some(v.0) { prev_rank } else { i + 1 };
                prev = Some(v.0);
                prev_rank = rank;
                Some(json!({"squadId": sid, "squadName": s.name, "isPrivate": s.private, "memberCount": self.member_count(sid), "totalMinutes": v.1, "totalSessions": 0, "averageMinutes": avg(v), "rank": rank, "points": v.0, "scoredDays": v.3}))
            })
            .collect()
    }

    /// The squad members' leaderboard (`getSquadLeaderboard`).
    pub fn squad_member_board(&self, user: &str, period: &str) -> Vec<Value> {
        let Some(me) = self.membership(user) else {
            return Vec::new();
        };
        let ids: HashSet<String> = self
            .x
            .members
            .iter()
            .filter(|m| m.squad == me.squad)
            .map(|m| m.user.clone())
            .collect();
        self.leaderboard(user, "global_all", period)
            .into_iter()
            .filter(|e| e["userId"].as_str().is_some_and(|id| ids.contains(id)))
            .enumerate()
            .map(|(i, mut e)| {
                e["rank"] = json!(i + 1);
                e
            })
            .collect()
    }

    fn details_json(&self, user: &str, squad: &str) -> Option<Value> {
        let s = self.x.squads.iter().find(|s| s.id == squad)?;
        let today = self.today();
        let yesterday = self.yesterday();
        let members: Vec<Value> = self
            .sorted_members(squad)
            .into_iter()
            .map(|m| self.member_json(m, user, Some(&today)))
            .collect();
        let count = members.len();
        let (mut yt, mut ya) = (0u64, 0u64);
        for m in self.x.members.iter().filter(|m| m.squad == squad) {
            let (mm, _) = self
                .daily
                .get(&(m.user.clone(), yesterday.clone()))
                .copied()
                .unwrap_or((0, 0));
            yt += mm;
            if mm > 0 {
                ya += 1;
            }
        }
        let tm: u64 = members
            .iter()
            .map(|m| m["minutes"].as_u64().unwrap_or(0))
            .sum();
        let ts: u64 = members
            .iter()
            .map(|m| m["sessions"].as_u64().unwrap_or(0))
            .sum();
        let pending = self
            .x
            .joins
            .iter()
            .any(|j| j.squad == squad && j.user == user && j.status == "pending");
        let action = match self.membership(user) {
            Some(m) if m.squad == squad => "current",
            Some(_) => "unavailable",
            None if count >= MAX_SQUAD_MEMBERS => "full",
            None if pending => "pending",
            None if s.private => "request",
            None => "join",
        };
        Some(json!({
            "id": s.id, "name": s.name, "isPrivate": s.private, "createdByUserId": s.created_by,
            "createdAt": sqlite_time(s.created_at), "totalMinutes": tm, "totalSessions": ts,
            "memberCount": count, "maxMembers": MAX_SQUAD_MEMBERS, "statsDate": today,
            "previousDayDate": yesterday, "previousDayTotalMinutes": yt,
            "previousDayAverageMinutes": if ya > 0 { yt as f64 / ya as f64 } else { 0.0 },
            "action": action, "members": members,
        }))
    }

    fn add_member(&mut self, squad: &str, user: &str, role: &str) {
        let now = self.now_ms;
        self.x.members.push(MemberRow {
            squad: squad.into(),
            user: user.into(),
            role: role.into(),
            joined_at: now,
        });
    }

    // ---------------------------------------------------------------------- verified sessions

    fn credit(&mut self, user: &str, start: i64, minutes: u64) {
        for i in 0..minutes {
            let date = study_tracker_core::break_room::skribbl::zurich::zurich_date(
                study_tracker_core::timer::WallTimestamp::from_unix_millis(
                    start + i as i64 * 60_000,
                ),
            )
            .to_iso();
            let e = self.daily.entry((user.to_string(), date)).or_insert((0, 0));
            e.0 += 1;
        }
        if minutes > 0 {
            let date = self.today();
            self.daily
                .entry((user.to_string(), date))
                .or_insert((0, 0))
                .1 += 1;
        }
    }

    fn finish_session(&mut self, idx: usize) -> u64 {
        let now = self.now_ms;
        let row = self.x.verified[idx].clone();
        if row.finished {
            return 0;
        }
        let boundary =
            (row.started + MAX_SESSION_MS).min(row.last_heartbeat + NORMAL_CREDIT_GRACE_MS);
        let end = now.min(boundary);
        let minutes = ((end - row.started).max(0) / 60_000) as u64;
        self.x.verified[idx].finished = true;
        self.x.verified[idx].credited = minutes;
        self.credit(&row.user, row.started, minutes);
        minutes
    }

    /// `settleStaleVerifiedSession`.
    pub fn settle_stale(&mut self, user: &str) {
        let now = self.now_ms;
        if let Some(i) = self
            .x
            .verified
            .iter()
            .position(|v| v.user == user && !v.finished)
        {
            let v = &self.x.verified[i];
            if now - v.started >= MAX_SESSION_MS
                || now - v.last_heartbeat >= HEARTBEAT_MS + GRACE_MS
            {
                self.finish_session(i);
            }
        }
    }

    /// Everything 22b; `None` = not a 22b route.
    pub fn handle_22b(&mut self, req: &Request) -> Option<Response> {
        let body = || serde_json::from_slice::<Value>(&req.body).unwrap_or(Value::Null);
        let route = (req.method.as_str(), req.path.as_str());
        let r: Result<Response, Response> = (|| match route {
            ("POST", "/feed") | ("GET", "/feed") => {
                let b = body();
                let id = self.auth(&b)?;
                self.touch_pub(&id);
                let scope = if b["scope"] == "friends" {
                    "friends"
                } else {
                    "global"
                };
                let mut out = json!({"feed": self.feed(&id, scope)});
                if self.is_owner(&id) {
                    out["r2Usage"] = self.r2_usage();
                }
                Ok(Response::json(&out))
            }
            ("POST", "/feed/react") => {
                let b = body();
                let id = self.auth(&b)?;
                let post = clean_text(&b["postId"], 80);
                self.can_view_post(&id, &post)?;
                let requested = clean_text(&b["emoji"], 8);
                let emoji = if !requested.is_empty() && requested.chars().count() <= 4 {
                    requested
                } else {
                    "fire".into()
                };
                if let Some(i) = self
                    .x
                    .reactions
                    .iter()
                    .position(|(p, u, e)| p == &post && u == &id && e == &emoji)
                {
                    self.x.reactions.remove(i);
                } else {
                    self.x.reactions.push((post, id, emoji));
                }
                Ok(Response::json(&json!({"ok": true})))
            }
            ("POST", "/feed/poll/vote") => {
                let b = body();
                let id = self.auth(&b)?;
                let post = clean_text(&b["postId"], 80);
                let option = clean_text(&b["optionId"], 80);
                self.can_view_post(&id, &post).map_err(|e| {
                    if e.status == 403 {
                        Response::text(403, "You cannot vote on this poll.")
                    } else {
                        e
                    }
                })?;
                let Some(poll) = self.x.polls.get(&post) else {
                    return Ok(Response::text(404, "Poll option not found."));
                };
                if !poll.options.iter().any(|(o, _)| o == &option) {
                    return Ok(Response::text(404, "Poll option not found."));
                }
                let multiple = poll.multiple;
                if let Some(i) = self
                    .x
                    .poll_votes
                    .iter()
                    .position(|(p, o, u)| p == &post && o == &option && u == &id)
                {
                    self.x.poll_votes.remove(i);
                } else {
                    if !multiple {
                        self.x
                            .poll_votes
                            .retain(|(p, _, u)| !(p == &post && u == &id));
                    }
                    self.x.poll_votes.push((post.clone(), option, id.clone()));
                }
                Ok(Response::json(
                    &json!({"ok": true, "poll": self.poll_json(&id, &post)}),
                ))
            }
            ("POST", "/feed/comment") => {
                let b = body();
                let id = self.auth(&b)?;
                let post = clean_text(&b["postId"], 80);
                self.can_view_post(&id, &post).map_err(|e| {
                    if e.status == 403 {
                        Response::text(403, "You cannot comment on this feed post.")
                    } else {
                        e
                    }
                })?;
                let text = clean_text(&b["body"], 220);
                if text.is_empty() {
                    return Ok(Response::text(400, "Comment cannot be empty."));
                }
                let cid = self.next_id_pub("comment");
                let row = CommentRow {
                    id: cid,
                    post,
                    user: id.clone(),
                    body: text,
                    created_at: self.now_ms,
                };
                let json = self.comment_json(&row, &id);
                self.x.comments.push(row);
                Ok(Response::json(&json!({"ok": true, "comment": json})))
            }
            ("POST", "/feed/update") => {
                let b = body();
                let id = self.auth(&b)?;
                let post = clean_text(&b["postId"], 80);
                let Some(p) = self.x.posts.iter_mut().find(|p| p.id == post) else {
                    return Ok(Response::text(404, "Feed post not found."));
                };
                if p.user != id {
                    return Ok(Response::text(403, "You can only edit your own posts."));
                }
                p.note = clean_text(&b["note"], 220);
                Ok(Response::json(&json!({"ok": true})))
            }
            ("POST", "/feed/delete") => {
                let b = body();
                let id = self.auth(&b)?;
                let post = clean_text(&b["postId"], 80);
                let Some(i) = self.x.posts.iter().position(|p| p.id == post) else {
                    return Ok(Response::text(404, "Feed post not found."));
                };
                if self.x.posts[i].user != id {
                    return Ok(Response::text(403, "You can only delete your own posts."));
                }
                let p = self.x.posts.remove(i);
                if let Some(k) = p.image_key {
                    self.x.images.remove(&k);
                }
                self.x.comments.retain(|c| c.post != post);
                self.x.reactions.retain(|(pid, _, _)| pid != &post);
                self.x.polls.remove(&post);
                self.x.poll_votes.retain(|(pid, _, _)| pid != &post);
                Ok(Response::json(&json!({"ok": true})))
            }
            ("POST", "/feed/image") => {
                let form =
                    crate::multipart::parse(req.content_type.as_deref().unwrap_or(""), &req.body);
                let text = |k: &str| {
                    form.iter()
                        .find(|p| p.name == k)
                        .map(|p| String::from_utf8_lossy(&p.data).to_string())
                        .unwrap_or_default()
                };
                let id = text("userId").trim().to_string();
                self.verify_pub(&id, &text("deviceSecret"))?;
                let post = clean_text(&json!(text("postId")), 80);
                let Some(pi) = self.x.posts.iter().position(|p| p.id == post) else {
                    return Ok(Response::text(404, "Feed post not found."));
                };
                if self.x.posts[pi].user != id {
                    return Ok(Response::text(
                        403,
                        "You can only edit your own post image.",
                    ));
                }
                let Some(image) = form
                    .iter()
                    .find(|p| p.name == "image" && p.file_name.is_some())
                else {
                    return Ok(Response::text(400, "Missing image."));
                };
                let mime = image.content_type.clone().unwrap_or_default();
                if !["image/png", "image/jpeg", "image/webp", "image/gif"].contains(&mime.as_str())
                {
                    return Ok(Response::text(400, "Use PNG, JPEG, WebP, or GIF images."));
                }
                if image.data.len() > 5 * 1024 * 1024 {
                    return Ok(Response::text(
                        413,
                        "Image is too large. Use an image under 5 MB.",
                    ));
                }
                if self.x.r2_paused {
                    return Ok(Response::text(
                        429,
                        "Image uploads are paused to keep R2 usage below the free tier.",
                    ));
                }
                let ext = match mime.as_str() {
                    "image/png" => "png",
                    "image/webp" => "webp",
                    "image/gif" => "gif",
                    _ => "jpg",
                };
                let n = self.next_id_pub("img");
                let key = format!("feed-posts/{post}/{n}.{ext}");
                if let Some(old) = self.x.posts[pi].image_key.take() {
                    self.x.images.remove(&old);
                }
                self.x.images.insert(key.clone(), image.data.clone());
                let expires = self.now_ms + FEED_IMAGE_TTL_MS;
                let p = &mut self.x.posts[pi];
                p.image_key = Some(key.clone());
                p.image_mime = Some(mime.clone());
                p.image_expires = Some(expires);
                p.image_expired_at = None;
                let mut out = json!({"ok": true, "imageKey": key, "imageMimeType": mime, "imageExpiresAt": crate::iso(expires), "imageExpiredAt": null, "imageUrl": format!("{}/feed/image/{}", self.origin, encode(&key))});
                if self.is_owner(&id) {
                    out["r2Usage"] = self.r2_usage();
                }
                Ok(Response::json(&out))
            }
            ("POST", "/feed/image/delete") => {
                let b = body();
                let id = self.auth(&b)?;
                let post = clean_text(&b["postId"], 80);
                let Some(p) = self.x.posts.iter_mut().find(|p| p.id == post) else {
                    return Ok(Response::text(404, "Feed post not found."));
                };
                if p.user != id {
                    return Ok(Response::text(
                        403,
                        "You can only edit your own post image.",
                    ));
                }
                if let Some(k) = p.image_key.take() {
                    self.x.images.remove(&k);
                }
                p.image_mime = None;
                p.image_expires = None;
                p.image_expired_at = None;
                let mut out = json!({"ok": true});
                if self.is_owner(&id) {
                    out["r2Usage"] = self.r2_usage();
                }
                Ok(Response::json(&out))
            }
            ("GET", path) if path.starts_with("/feed/image/") => {
                let key = percent_decode(&path["/feed/image/".len()..]);
                let now = self.now_ms;
                let Some(p) = self.x.posts.iter().find(|p| {
                    p.image_key.as_deref() == Some(key.as_str())
                        && p.image_expires.is_some_and(|e| e > now)
                }) else {
                    return Ok(Response::text(404, "Image not found."));
                };
                let mime = mime_static(p.image_mime.as_deref().unwrap_or(""));
                match self.x.images.get(&key) {
                    Some(bytes) => Ok(Response {
                        status: 200,
                        content_type: mime,
                        body: bytes.clone(),
                        extra_headers: vec![("cache-control", "public, max-age=3600".into())],
                    }),
                    None => Ok(Response::text(404, "Image not found.")),
                }
            }
            ("POST", "/profile/avatar") => {
                let form =
                    crate::multipart::parse(req.content_type.as_deref().unwrap_or(""), &req.body);
                let text = |k: &str| {
                    form.iter()
                        .find(|p| p.name == k)
                        .map(|p| String::from_utf8_lossy(&p.data).to_string())
                        .unwrap_or_default()
                };
                let id = text("userId").trim().to_string();
                self.verify_pub(&id, &text("deviceSecret"))?;
                let Some(image) = form
                    .iter()
                    .find(|p| p.name == "image" && p.file_name.is_some())
                else {
                    return Ok(Response::text(400, "Missing avatar image."));
                };
                let mime = image.content_type.clone().unwrap_or_default();
                if !["image/png", "image/jpeg", "image/webp", "image/gif"].contains(&mime.as_str())
                {
                    return Ok(Response::text(400, "Use PNG, JPEG, WebP, or GIF images."));
                }
                if image.data.len() > 256 * 1024 {
                    return Ok(Response::text(413, "Avatar image is too large."));
                }
                let n = self.next_id_pub("avatar");
                let key = format!("avatars/{id}/{n}");
                self.avatars
                    .insert(key.clone(), (mime.clone(), image.data.clone()));
                let name = {
                    let n = clean_text(&json!(text("name")), 180);
                    if n.is_empty() {
                        "photo".to_string()
                    } else {
                        n
                    }
                };
                let avatar = json!({"kind": "photo", "name": name, "url": format!("{}/profile/avatar/{}", self.origin, encode(&key)), "mimeType": mime});
                if let Some(u) = self.users.get_mut(&id) {
                    u.avatar = avatar.clone();
                }
                Ok(Response::json(&json!({"avatar": avatar})))
            }
            ("POST", "/squads/create") => {
                let b = body();
                let id = self.auth(&b)?;
                if self.membership(&id).is_some() {
                    return Ok(Response::text(
                        409,
                        "Leave your current squad before creating a new one.",
                    ));
                }
                let name = clean_text(&b["name"], 48)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if name.is_empty() {
                    return Ok(Response::text(400, "Missing squad name."));
                }
                let sid = self.next_id_pub("squad");
                let now = self.now_ms;
                self.x.squads.push(SquadRow {
                    id: sid.clone(),
                    name,
                    private: b["isPrivate"].as_bool().unwrap_or(false),
                    created_by: id.clone(),
                    created_at: now,
                });
                self.add_member(&sid, &id, "leader");
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/search") | ("GET", "/squads/search") => {
                let b = body();
                let id = self.auth(&b)?;
                let q = clean_text(&b["query"], 48).to_lowercase();
                let mine = self.membership(&id).is_some();
                let mut rows: Vec<Value> = self.x.squads.iter().filter(|s| s.name.to_lowercase().contains(&q)).map(|s| {
                    let count = self.member_count(&s.id);
                    let total: u64 = self.x.members.iter().filter(|m| m.squad == s.id).map(|m| self.daily.iter().filter(|((u, _), _)| u == &m.user).map(|(_, v)| v.0).sum::<u64>()).sum();
                    let pending = self.x.joins.iter().any(|j| j.squad == s.id && j.user == id && j.status == "pending");
                    let action = if mine { "unavailable" } else if count >= MAX_SQUAD_MEMBERS { "full" } else if pending { "pending" } else if s.private { "request" } else { "join" };
                    json!({"id": s.id, "name": s.name, "isPrivate": s.private, "createdAt": sqlite_time(s.created_at), "memberCount": count, "maxMembers": MAX_SQUAD_MEMBERS, "totalMinutes": total, "totalSessions": 0, "action": action})
                }).collect();
                rows.sort_by(|a, b| {
                    b["memberCount"]
                        .as_u64()
                        .cmp(&a["memberCount"].as_u64())
                        .then(b["totalMinutes"].as_u64().cmp(&a["totalMinutes"].as_u64()))
                        .then(a["name"].as_str().cmp(&b["name"].as_str()))
                });
                rows.truncate(30);
                Ok(Response::json(&json!({"squads": rows})))
            }
            ("POST", "/squads/details") | ("GET", "/squads/details") => {
                let b = body();
                let id = self.auth(&b)?;
                let sid = clean_text(&b["squadId"], 80);
                if sid.is_empty() {
                    return Ok(Response::text(400, "Missing squadId."));
                }
                match self.details_json(&id, &sid) {
                    Some(d) => Ok(Response::json(&json!({"squad": d}))),
                    None => Ok(Response::text(404, "Squad not found.")),
                }
            }
            ("POST", "/squads/join") => {
                let b = body();
                let id = self.auth(&b)?;
                if self.membership(&id).is_some() {
                    return Ok(Response::text(
                        409,
                        "Leave your current squad before joining another one.",
                    ));
                }
                let sid = clean_text(&b["squadId"], 80);
                let Some(s) = self.x.squads.iter().find(|s| s.id == sid).cloned() else {
                    return Ok(Response::text(404, "Squad not found."));
                };
                if self.member_count(&sid) >= MAX_SQUAD_MEMBERS {
                    return Ok(Response::text(409, "That squad is already full."));
                }
                if s.private {
                    let now = self.now_ms;
                    if let Some(j) = self
                        .x
                        .joins
                        .iter_mut()
                        .find(|j| j.squad == sid && j.user == id)
                    {
                        j.status = "pending";
                    } else {
                        let jid = self.next_id_pub("squad-request");
                        self.x.joins.push(JoinRow {
                            id: jid,
                            squad: sid,
                            user: id.clone(),
                            status: "pending",
                            created_at: now,
                        });
                    }
                } else {
                    self.add_member(&sid, &id, "member");
                }
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/respond") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(actor) = self
                    .membership(&id)
                    .cloned()
                    .filter(|m| role_rank(&m.role) > 1)
                else {
                    return Ok(Response::text(403, "You cannot manage squad requests."));
                };
                let rid = b["requestId"].as_str().unwrap_or("").to_string();
                let Some(ji) =
                    self.x.joins.iter().position(|j| {
                        j.id == rid && j.squad == actor.squad && j.status == "pending"
                    })
                else {
                    return Ok(Response::text(404, "Squad request not found."));
                };
                if b["response"] == "accepted" {
                    let user = self.x.joins[ji].user.clone();
                    if self.membership(&user).is_some() {
                        return Ok(Response::text(409, "That user is already in a squad."));
                    }
                    if self.member_count(&actor.squad) >= MAX_SQUAD_MEMBERS {
                        return Ok(Response::text(409, "That squad is already full."));
                    }
                    self.add_member(&actor.squad, &user, "member");
                    self.x.joins[ji].status = "accepted";
                } else {
                    self.x.joins[ji].status = "declined";
                }
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/leave") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(me) = self.membership(&id).cloned() else {
                    return Ok(Response::text(404, "You are not in a squad."));
                };
                if self.member_count(&me.squad) <= 1 {
                    self.x.messages.retain(|m| m.squad != me.squad);
                    self.x.joins.retain(|j| j.squad != me.squad);
                    self.x.members.retain(|m| m.squad != me.squad);
                    self.x.squads.retain(|s| s.id != me.squad);
                    return Ok(Response::json(&self.full_snapshot(&id)));
                }
                self.x.members.retain(|m| m.user != id);
                if me.role == "leader" {
                    let next = self
                        .sorted_members(&me.squad)
                        .first()
                        .map(|m| m.user.clone());
                    if let Some(n) = next {
                        if let Some(m) = self.x.members.iter_mut().find(|m| m.user == n) {
                            m.role = "leader".into();
                        }
                    }
                }
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/chat") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(me) = self.membership(&id).cloned() else {
                    return Ok(Response::text(403, "Join a squad before chatting."));
                };
                let text = clean_text(&b["body"], 500);
                if text.is_empty() {
                    return Ok(Response::text(400, "Message cannot be empty."));
                }
                let mid = self.next_id_pub("message");
                let now = self.now_ms;
                self.x.messages.push(MessageRow {
                    id: mid,
                    squad: me.squad,
                    user: id.clone(),
                    body: text,
                    created_at: now,
                });
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/chat/delete") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(me) = self.membership(&id).cloned() else {
                    return Ok(Response::text(403, "Join a squad before managing chat."));
                };
                let mid = clean_text(&b["messageId"], 80);
                let Some(i) = self
                    .x
                    .messages
                    .iter()
                    .position(|m| m.id == mid && m.squad == me.squad && m.user == id)
                else {
                    return Ok(Response::text(404, "Message not found."));
                };
                self.x.messages.remove(i);
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/promote") | ("POST", "/squads/demote") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(actor) = self.membership(&id).cloned() else {
                    return Ok(Response::text(404, "You are not in a squad."));
                };
                let target = b["targetUserId"].as_str().unwrap_or("").trim().to_string();
                if target == id {
                    return Ok(Response::text(400, "You cannot change your own rank."));
                }
                let Some(t) = self
                    .x
                    .members
                    .iter()
                    .find(|m| m.squad == actor.squad && m.user == target)
                    .cloned()
                else {
                    return Ok(Response::text(404, "Squad member not found."));
                };
                let next = b["role"]
                    .as_str()
                    .filter(|r| matches!(*r, "leader" | "co_leader" | "elder" | "member"))
                    .unwrap_or("member");
                if !can_change_role(&actor.role, &t.role, next) {
                    return Ok(Response::text(403, "You cannot assign that rank."));
                }
                if let Some(m) = self
                    .x
                    .members
                    .iter_mut()
                    .find(|m| m.squad == actor.squad && m.user == target)
                {
                    m.role = next.into();
                }
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/kick") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(actor) = self.membership(&id).cloned() else {
                    return Ok(Response::text(404, "You are not in a squad."));
                };
                let target = b["targetUserId"].as_str().unwrap_or("").trim().to_string();
                if target == id {
                    return Ok(Response::text(400, "Use leave squad instead."));
                }
                let Some(t) = self
                    .x
                    .members
                    .iter()
                    .find(|m| m.squad == actor.squad && m.user == target)
                    .cloned()
                else {
                    return Ok(Response::text(404, "Squad member not found."));
                };
                if !can_kick(&actor.role, &t.role) {
                    return Ok(Response::text(403, "You cannot kick that member."));
                }
                self.x
                    .members
                    .retain(|m| !(m.squad == actor.squad && m.user == target));
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/settings") => {
                let b = body();
                let id = self.auth(&b)?;
                let Some(actor) = self.membership(&id).cloned().filter(|m| m.role == "leader")
                else {
                    return Ok(Response::text(
                        403,
                        "Only the squad leader can edit squad settings.",
                    ));
                };
                let name = clean_text(&b["name"], 48)
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if name.is_empty() {
                    return Ok(Response::text(400, "Missing squad name."));
                }
                if let Some(s) = self.x.squads.iter_mut().find(|s| s.id == actor.squad) {
                    s.name = name;
                    s.private = b["isPrivate"].as_bool().unwrap_or(false);
                }
                Ok(Response::json(&self.full_snapshot(&id)))
            }
            ("POST", "/squads/scoreboard") | ("GET", "/squads/scoreboard") => {
                let b = body();
                self.auth(&b)?;
                let period = b["period"]
                    .as_str()
                    .filter(|p| matches!(*p, "daily" | "overall"))
                    .unwrap_or("season");
                Ok(Response::json(
                    &json!({"entries": self.squad_scores(period)}),
                ))
            }
            ("POST", "/verified-session/start") => {
                let b = body();
                let id = self.auth(&b)?;
                let now = self.now_ms;
                if let Some(i) = self
                    .x
                    .verified
                    .iter()
                    .position(|v| v.user == id && !v.finished)
                {
                    if now - self.x.verified[i].started < MAX_SESSION_MS {
                        let v = &self.x.verified[i];
                        return Ok(Response::json(
                            &json!({"sessionId": v.id, "startedAt": crate::iso(v.started), "resumed": true}),
                        ));
                    }
                    self.finish_session(i);
                }
                let sid = self.next_id_pub("verified");
                self.x.verified.push(VerifiedRow {
                    id: sid.clone(),
                    user: id,
                    started: now,
                    last_heartbeat: now,
                    finished: false,
                    credited: 0,
                });
                Ok(Response::json(
                    &json!({"sessionId": sid, "startedAt": crate::iso(now), "resumed": false}),
                ))
            }
            ("POST", "/verified-session/heartbeat") => {
                let b = body();
                let id = self.auth(&b)?;
                let sid = clean_text(&b["sessionId"], 80);
                if sid.is_empty() {
                    return Ok(Response::text(400, "Missing sessionId."));
                }
                let now = self.now_ms;
                match self
                    .x
                    .verified
                    .iter_mut()
                    .find(|v| v.id == sid && v.user == id && !v.finished)
                {
                    Some(v) => {
                        v.last_heartbeat = now;
                        Ok(Response::json(&json!({"ok": true})))
                    }
                    None => Ok(Response::text(404, "Verified session not found.")),
                }
            }
            ("POST", "/verified-session/finish") => {
                let b = body();
                let id = self.auth(&b)?;
                let sid = clean_text(&b["sessionId"], 80);
                let Some(i) = self
                    .x
                    .verified
                    .iter()
                    .position(|v| v.id == sid && v.user == id && !v.finished)
                else {
                    return Ok(Response::text(404, "Verified session not found."));
                };
                let credited = self.finish_session(i);
                Ok(Response::json(
                    &json!({"ok": true, "creditedMinutes": credited, "finishedAt": crate::iso(self.now_ms)}),
                ))
            }
            ("POST", "/verified-session/reconcile-offline") => {
                let b = body();
                let id = self.auth(&b)?;
                let anchor = clean_text(&b["anchorSessionId"], 80);
                let Some(a) = self
                    .x
                    .verified
                    .iter()
                    .find(|v| v.id == anchor && v.user == id)
                    .cloned()
                else {
                    return Ok(Response::text(404, "Verified session not found."));
                };
                let mut gap_start = if a.finished {
                    a.started + a.credited as i64 * 60_000
                } else {
                    (a.started + MAX_SESSION_MS).min(a.last_heartbeat + NORMAL_CREDIT_GRACE_MS)
                };
                if let Some(covered) = self
                    .x
                    .reconciles
                    .iter()
                    .filter(|(x, _)| x == &anchor)
                    .map(|(_, e)| *e)
                    .max()
                {
                    gap_start = gap_start.max(covered);
                }
                let now = self.now_ms;
                let mut claimed = 0u64;
                let mut credited = 0u64;
                let cap = (((now - gap_start).max(0) as f64 / 60_000.0) / 1440.0 * 1200.0)
                    .max((now - gap_start).max(0) as f64 / 60_000.0 + 5.0)
                    .min(1440.0);
                let mut remaining = cap.floor() as u64;
                let ints: Vec<(i64, i64)> = b["intervals"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .take(500)
                            .filter_map(|i| {
                                let s = study_tracker_core::social::SocialTimestamp::parse(
                                    i["startedAt"].as_str()?,
                                )?
                                .0;
                                let e = study_tracker_core::social::SocialTimestamp::parse(
                                    i["endedAt"].as_str()?,
                                )?
                                .0;
                                (e > s)
                                    .then_some((s.max(gap_start), e.min(now)))
                                    .filter(|(s, e)| e > s)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                for (s, e) in ints {
                    let m = ((e - s) / 60_000) as u64;
                    claimed += m;
                    let take = m.min(remaining);
                    remaining -= take;
                    credited += take;
                    if take > 0 {
                        *self
                            .x
                            .offline_credit
                            .entry((id.clone(), self.today()))
                            .or_default() += take;
                    }
                }
                self.x.reconciles.push((anchor, now));
                Ok(Response::json(
                    &json!({"ok": true, "creditedMinutes": credited, "cappedFromClaimedMinutes": claimed.saturating_sub(credited), "flagged": false}),
                ))
            }
            ("GET", "/announcements/current") => {
                let version = req
                    .query
                    .iter()
                    .find(|(k, _)| k == "appVersion")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let older = |cur: &str, target: &str| {
                    let p = |v: &str| {
                        v.split('.')
                            .map(|x| x.parse::<u64>().unwrap_or(0))
                            .collect::<Vec<_>>()
                    };
                    p(cur) < p(target)
                };
                let a = self
                    .x
                    .announcements
                    .iter()
                    .rev()
                    .find(|(_, _, _, t, active)| {
                        *active && t.as_ref().is_none_or(|t| older(&version, t))
                    });
                Ok(Response::json(
                    &json!({"announcement": a.map(|(id, title, body, t, _)| json!({"id": id, "title": title, "body": body, "targetVersion": t, "createdAt": crate::iso(self.now_ms), "expiresAt": null}))}),
                ))
            }
            ("POST", "/announcements/update-notice") => {
                let b = body();
                let id = self.auth(&b)?;
                if !self.is_owner(&id) {
                    return Ok(Response::text(
                        403,
                        "Only the app owner can notify users about updates.",
                    ));
                }
                let version = clean_text(&b["targetVersion"], 40);
                if version.is_empty()
                    || !version
                        .split('.')
                        .all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
                {
                    return Ok(Response::text(400, "Missing valid target version."));
                }
                let aid = format!("update-{version}-{}", self.now_ms);
                self.x.announcements.push((
                    aid.clone(),
                    "New update available.".into(),
                    "Go to Settings to update the app.".into(),
                    Some(version.clone()),
                    true,
                ));
                Ok(Response::json(
                    &json!({"ok": true, "id": aid, "targetVersion": version}),
                ))
            }
            ("POST", "/telemetry/heartbeat") => {
                let b = body();
                let install = clean_text(&b["installId"], 80);
                if install.is_empty() {
                    return Ok(Response::text(400, "Missing installId."));
                }
                let keys = b
                    .as_object()
                    .map(|o| o.keys().cloned().collect())
                    .unwrap_or_default();
                self.x.telemetry.push(TelemetryHit {
                    keys,
                    install_id: install,
                });
                Ok(Response::json(&json!({"ok": true})))
            }
            ("POST", "/admin/usage") | ("GET", "/admin/usage") => {
                let b = body();
                let id = self.auth(&b)?;
                if !self.is_owner(&id) {
                    return Ok(Response::text(
                        403,
                        "Only the app owner can view usage data.",
                    ));
                }
                let users: Vec<Value> = self.users.values().map(|u| json!({"displayName": u.display_name, "friendCode": u.friend_code, "lastSeenAt": sqlite_time(u.last_seen_at), "deviceLabel": "Synthetic device", "deviceFingerprintHash": "0123456789abcdef", "appVersion": "0.1.68", "appPlatform": "linux", "appRuntimeChannel": "development", "appSeenAt": sqlite_time(u.last_seen_at), "isFlagged": 0, "flaggedReason": null, "signupCountry": "CH", "signupAsn": 64512, "signupAsOrganization": "Synthetic ISP"})).collect();
                let telemetry: Vec<Value> = self.x.telemetry.iter().map(|t| json!({"installId": t.install_id, "appVersion": "0.1.68", "appPlatform": "linux", "appRuntimeChannel": "development", "createdAt": sqlite_time(self.now_ms), "lastSeenAt": sqlite_time(self.now_ms)})).collect();
                Ok(Response::json(&json!({
                    "summary": {"userCount": users.len(), "active24h": users.len(), "active7d": users.len(), "usersWithAppVersion": users.len(), "flaggedCount": 0},
                    "users": users, "flaggedUsers": [], "abuseEvents": [{"eventType": "honeypot", "country": "XX", "asOrganization": "Synthetic", "path": "/.env", "userAgent": "curl/8", "userId": null, "detail": "synthetic", "createdAt": sqlite_time(self.now_ms)}],
                    "telemetry": telemetry,
                })))
            }
            _ => Err(Response::text(0, "")),
        })();
        match r {
            Err(e) if e.status == 0 => None,
            Ok(r) | Err(r) => Some(r),
        }
    }
}
