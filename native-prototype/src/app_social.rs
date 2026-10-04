//! The Social surface's Slint glue (Stage 22a): schedule, callbacks, pushes. Decisions live in
//! `social_controller`; this file only turns them into timers, requests and view data.
//!
//! Timers (all `slint::Timer`s that sleep between firings; none exists without an account):
//! - startup: presence + due auto-sync once the event loop runs; the session effect 2 s later;
//! - hourly: the auto-sync check;
//! - while the Social tab is visible: the 2-minute friend-status poll;
//! - after a session-count change: one sync 2 s later (debounced);
//! - the message banner's 3.2 s.
//! Pushes happen on replies and clicks only; a reply that changes nothing visible pushes the same
//! data, which Slint does not redraw.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode, VecModel,
};
use study_tracker_core::dashboard::format::format_minutes;
use study_tracker_core::social::avatar::{arena_hue, Avatar, AvatarStyle};
use study_tracker_core::social::friends::FriendResponse;
use study_tracker_core::social::identity::IdentityPhase;
use study_tracker_core::social::leaderboard::{
    bar_percent, top_minutes, LeaderboardPeriod, LeaderboardScope,
};
use study_tracker_core::social::stats::{monthly_stat, period_stat};
use study_tracker_core::social::time::{
    is_recently_active, SocialTimestamp, RECENTLY_ACTIVE_MS, SEEN_RECENT_MS,
    WABI_ATTENDANCE_WINDOW_MS,
};
use study_tracker_core::social::{RequestId, SocialIdentity, UserId};
use study_tracker_core::timer::WallTimestamp;

use crate::app_model::AppModel;
use crate::dashboard_view::ChronoLocalClock;
use crate::image_cache::ImageCache;
use crate::net::device::{app_metadata, device_identity, AppMetadata, DeviceIdentity};
use crate::net::endpoint::{Origin, SocialEndpoint};
use crate::net::images::{self, ImageKind};
use crate::net::worker::CancelToken;
use crate::net_jobs::{NetReply, Outgoing, Post};
use crate::persistence::social_credentials::FileCredentialStore;
use crate::persistence::social_port::FileSocialPort;
use crate::persistence::NativeStore;
use crate::social_controller::{SocialController, Subtab, SyncContext};
use crate::{MainWindow, SAvatar, SBoardRow, SFriendRow, SRequestRow, SStat, SocialView};

const STATUS_POLL: Duration = Duration::from_secs(2 * 60);
const HOURLY: Duration = Duration::from_secs(60 * 60);
const SESSION_DEBOUNCE: Duration = Duration::from_secs(2);
const BANNER: Duration = Duration::from_millis(3200);
const AVATAR_PX: u32 = 112;

struct Runtime {
    controller: SocialController,
    model: Rc<RefCell<AppModel>>,
    window: slint::Weak<MainWindow>,
    device: Option<DeviceIdentity>,
    app: AppMetadata,
    // timers
    startup: Timer,
    hourly: Timer,
    poll: Timer,
    session_debounce: Timer,
    banner: Timer,
    last_sessions: usize,
    shown_message: u64,
    // avatar photos (URL -> picture), fetched under the image policy
    avatars: ImageCache,
    pictures: HashMap<String, Image>,
    avatar_tokens: HashMap<u64, String>,
    next_avatar_token: u64,
    pushes: u64,
    // the friend-code field as last mirrored from the controller
    code_draft: String,
}

thread_local! {
    static RT: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    RT.with(|r| r.borrow_mut().as_mut().map(f))
}

fn now() -> WallTimestamp {
    crate::wall_now()
}

/// Reads `preferences.wabiCircleCompetitive` (production's `study-tracker-wabi-circle-competitive`).
fn load_competitive(store: &NativeStore) -> bool {
    store
        .load()
        .ok()
        .and_then(|(e, _)| {
            e.other
                .get("preferences")
                .and_then(|p| p.get("wabiCircleCompetitive"))
                .and_then(|v| v.as_bool())
        })
        .unwrap_or(false)
}

fn save_competitive(store: &NativeStore, on: bool) {
    let Ok((mut envelope, _)) = store.load() else {
        return;
    };
    let mut prefs = match envelope.other.remove("preferences") {
        Some(serde_json::Value::Object(m)) => m,
        _ => serde_json::Map::new(),
    };
    prefs.insert("wabiCircleCompetitive".into(), serde_json::Value::Bool(on));
    envelope
        .other
        .insert("preferences".into(), serde_json::Value::Object(prefs));
    if let Err(err) = store.save(&envelope) {
        log::warn!("social: could not save the Circle preference: {err}");
    }
}

pub fn install(
    window: &MainWindow,
    model: Rc<RefCell<AppModel>>,
    store_path: &Path,
    data_dir: &Path,
) {
    let endpoint = match SocialEndpoint::from_env() {
        Ok(e) => Some(e),
        Err(_) => {
            log::warn!(
                "social: {} is set but is not a loopback http://host:port URL; Social is disabled for this run",
                crate::net::endpoint::ENDPOINT_ENV
            );
            None
        }
    };
    crate::app_net::configure(endpoint.as_ref());
    let store = NativeStore::new(store_path.to_path_buf());
    // `STUDY_NATIVE_SOCIAL_COMPETITIVE=1` (parity captures) starts with the Wabi standings shown
    let competitive = load_competitive(&store)
        || std::env::var("STUDY_NATIVE_SOCIAL_COMPETITIVE").as_deref() == Ok("1");
    let controller = SocialController::new(
        endpoint,
        Box::new(FileCredentialStore::new(data_dir)),
        Box::new(FileSocialPort::new(NativeStore::new(
            store_path.to_path_buf(),
        ))),
        competitive,
    );
    let sessions = model.borrow().academic().state().sessions.len();
    RT.with(|r| {
        *r.borrow_mut() = Some(Runtime {
            controller,
            model,
            window: window.as_weak(),
            device: None,
            app: app_metadata(),
            startup: Timer::default(),
            hourly: Timer::default(),
            poll: Timer::default(),
            session_debounce: Timer::default(),
            banner: Timer::default(),
            last_sessions: sessions,
            shown_message: 0,
            avatars: ImageCache::new(64),
            pictures: HashMap::new(),
            avatar_tokens: HashMap::new(),
            next_avatar_token: 0,
            pushes: 0,
            code_draft: String::new(),
        })
    });
    bind(window);
    if std::env::var("STUDY_NATIVE_VIEW").as_deref() == Ok("social") {
        window.set_show_dashboard(false);
        window.set_show_social(true);
        window.set_circle_menu_open(true);
        let weak = window.as_weak();
        Timer::single_shot(Duration::from_millis(20), move || {
            if let Some(w) = weak.upgrade() {
                w.invoke_social_opened();
            }
        });
    }
    if let Ok(sub) = std::env::var("STUDY_NATIVE_SOCIAL_SUBTAB") {
        let i = ["feed", "leaderboard", "friends", "squad", "profile"]
            .iter()
            .position(|s| *s == sub);
        if let Some(i) = i {
            with(|rt| rt.controller.subtab = Subtab::ALL[i]);
        }
    }
    schedule_startup(window);
    push();
}

/// Runs `f` with the sync context (academic state, local clock, device and app metadata).
fn with_ctx<R>(rt: &mut Runtime, f: impl FnOnce(&mut SocialController, &SyncContext) -> R) -> R {
    if rt.device.is_none() {
        rt.device = Some(device_identity());
    }
    let model = Rc::clone(&rt.model);
    let model = model.borrow();
    let clock = ChronoLocalClock;
    let device = rt.device.clone().expect("set above");
    let ctx = SyncContext {
        academic: model.academic().state(),
        clock: &clock,
        device: &device,
        app: &rt.app,
    };
    f(&mut rt.controller, &ctx)
}

fn dispatch(out: Vec<Outgoing>) {
    for o in out {
        crate::app_net::submit(o, |token, reply| on_reply(token, reply));
    }
}

fn on_reply(token: u64, reply: NetReply) {
    let out =
        with(|rt| with_ctx(rt, |c, ctx| c.on_reply(token, reply, now(), ctx))).unwrap_or_default();
    dispatch(out);
    after_change();
}

/// Schedule bookkeeping after any controller change, then a push.
fn after_change() {
    let Some(window) = with(|rt| rt.window.upgrade()).flatten() else {
        return;
    };
    let established = with(|rt| rt.controller.identity().is_some()).unwrap_or(false);
    with(|rt| {
        if established && !rt.hourly.running() {
            rt.hourly.start(TimerMode::Repeated, HOURLY, || {
                let out =
                    with(|rt| with_ctx(rt, |c, ctx| c.on_hourly(now(), ctx))).unwrap_or_default();
                dispatch(out);
                after_change();
            });
        } else if !established {
            rt.hourly.stop();
        }
        let polling = established && rt.controller.tab_visible;
        if polling && !rt.poll.running() {
            rt.poll.start(TimerMode::Repeated, STATUS_POLL, || {
                let out = with(|rt| rt.controller.on_status_poll()).unwrap_or_default();
                dispatch(out);
                after_change();
            });
        } else if !polling {
            rt.poll.stop();
        }
    });
    show_message(&window);
    push();
}

fn schedule_startup(window: &MainWindow) {
    let weak = window.as_weak();
    with(|rt| {
        // production's mount effects; nothing happens without an account
        if rt.controller.identity().is_none() {
            return;
        }
        rt.startup.start(
            TimerMode::SingleShot,
            Duration::from_millis(10),
            move || {
                let out =
                    with(|rt| with_ctx(rt, |c, ctx| c.on_startup(now(), ctx))).unwrap_or_default();
                dispatch(out);
                if let Some(w) = weak.upgrade() {
                    schedule_session_sync(&w);
                }
                after_change();
            },
        );
    });
}

fn schedule_session_sync(_window: &MainWindow) {
    with(|rt| {
        if rt.controller.identity().is_none() {
            return;
        }
        rt.session_debounce
            .start(TimerMode::SingleShot, SESSION_DEBOUNCE, || {
                let out = with(|rt| with_ctx(rt, |c, ctx| c.on_sessions_changed(now(), ctx)))
                    .unwrap_or_default();
                dispatch(out);
                after_change();
            });
    });
}

/// Called after every academic change (the Dashboard refresh path): a session added/removed
/// schedules production's "sync 2 s after the session list changed".
pub fn after_academic_change(window: &MainWindow, model: &AppModel) {
    let count = model.academic().state().sessions.len();
    let changed = with(|rt| {
        let changed = rt.last_sessions != count;
        rt.last_sessions = count;
        changed
    })
    .unwrap_or(false);
    if changed {
        schedule_session_sync(window);
    }
}

fn show_message(window: &MainWindow) {
    let msg = with(|rt| {
        if rt.controller.message_seq == rt.shown_message {
            return None;
        }
        rt.shown_message = rt.controller.message_seq;
        rt.controller.message.clone()
    })
    .flatten();
    let Some(text) = msg else { return };
    window.set_app_message(text.into());
    let weak = window.as_weak();
    with(|rt| {
        rt.banner.start(TimerMode::SingleShot, BANNER, move || {
            if let Some(w) = weak.upgrade() {
                w.set_app_message("".into());
            }
            with(|rt| rt.controller.dismiss_message());
        })
    });
}

/// What Daily Skribbl needs: the established identity and the endpoint origin.
pub fn skribbl_access() -> (Option<SocialIdentity>, Option<Origin>) {
    with(|rt| (rt.controller.identity().cloned(), rt.controller.origin())).unwrap_or((None, None))
}

pub fn unread() -> bool {
    with(|rt| rt.controller.has_unread).unwrap_or(false)
}

// ---------------------------------------------------------------------------------- view

fn s_avatar(rt: &mut Runtime, a: &Avatar, name: &str, is_self: bool) -> SAvatar {
    let kind = match a {
        Avatar::Letter { style, .. } => match style {
            AvatarStyle::Classic => 0,
            AvatarStyle::Serif => 1,
            AvatarStyle::Cursive => 2,
            AvatarStyle::Graffiti => 3,
            AvatarStyle::Pixel => 4,
            AvatarStyle::Mono => 5,
        },
        Avatar::Icon { .. } => 6,
        Avatar::Photo { .. } => 7,
    };
    let mut photo = Image::default();
    let mut has_photo = false;
    if let Some(url) = a.remote_photo_url() {
        if let Some(img) = rt.pictures.get(url) {
            photo = img.clone();
            has_photo = true;
        } else {
            request_avatar(rt, url);
        }
    }
    // a photo not (yet) shown draws production's broken-image fallback: the initials
    let text = if kind == 7 && !has_photo {
        study_tracker_core::social::avatar::initials(name)
    } else {
        a.display_text(name)
    };
    let hue = arena_hue(name) as f64;
    SAvatar {
        text: text.into(),
        kind,
        hue: hue as i32,
        photo,
        has_photo,
        is_self,
        grad_a: oklch(0.67, 0.14, hue),
        grad_b: oklch(0.52, 0.16, hue + 34.0),
    }
}

/// CSS `oklch(L C h)` as an sRGB colour (gamut-clipped like the browser's fallback).
fn oklch(l: f64, c: f64, hue_deg: f64) -> slint::Color {
    let h = hue_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
    let lin = [
        4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_,
        -1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701_0 * s_,
    ];
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        let g = if v <= 0.003_130_8 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (g * 255.0).round() as u8
    };
    slint::Color::from_rgb_u8(enc(lin[0]), enc(lin[1]), enc(lin[2]))
}

fn request_avatar(rt: &mut Runtime, url: &str) {
    let Some(origin) = rt.controller.origin() else {
        return;
    };
    if !rt.avatars.want(url) {
        return;
    }
    let Some(path) = images::allow(url, ImageKind::Avatar, &origin) else {
        log::warn!("social: an avatar URL was refused by the image policy");
        rt.avatars.failed(url);
        return;
    };
    rt.next_avatar_token += 1;
    let token = rt.next_avatar_token;
    rt.avatar_tokens.insert(token, url.to_string());
    let out = Outgoing {
        token,
        request: images::request(&path, ImageKind::Avatar),
        cancel: CancelToken::new(),
        post: Post::DecodeImage {
            kind: ImageKind::Avatar,
            max_w: AVATAR_PX,
            max_h: AVATAR_PX,
        },
    };
    // delivered asynchronously (never re-entering this borrow)
    crate::app_net::submit(out, |token, reply| {
        with(|rt| {
            if let Some(url) = rt.avatar_tokens.remove(&token) {
                match reply.image() {
                    Ok(img) => {
                        let pic =
                            Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                                &img.rgba, img.width, img.height,
                            ));
                        rt.avatars.loaded(&url, img);
                        rt.avatars.take_pixels(&url);
                        rt.pictures.insert(url, pic);
                    }
                    Err(crate::net::http::NetError::Cancelled) => rt.avatars.forget(&url),
                    Err(_) => rt.avatars.failed(&url),
                }
            }
            let keys: std::collections::HashSet<String> = rt.avatars.keys().cloned().collect();
            rt.pictures.retain(|k, _| keys.contains(k));
        });
        push();
    });
}

/// `formatProfileSeenAt`: "Oct 4, 11:40 AM" within two days, else "Oct 4".
pub fn seen_label(ts: SocialTimestamp, now: WallTimestamp) -> String {
    let local =
        chrono::DateTime::from_timestamp_millis(ts.0).map(|d| d.with_timezone(&chrono::Local));
    let Some(d) = local else { return String::new() };
    let date = d.format("%b %-d").to_string();
    if now.unix_millis - ts.0 < SEEN_RECENT_MS {
        format!("{date}, {}", d.format("%-I:%M %p"))
    } else {
        date
    }
}

/// `lastSocialSyncLabel`: `${formatDate} ${toLocaleTimeString(hour 2-digit, minute 2-digit)}`.
pub fn synced_label(ts: Option<SocialTimestamp>) -> String {
    let Some(ts) = ts else { return "Never".into() };
    chrono::DateTime::from_timestamp_millis(ts.0)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%b %-d %I:%M %p")
                .to_string()
        })
        .unwrap_or_else(|| "Never".into())
}

fn stat(value: String, label: String, icon: &str, tone: i32) -> SStat {
    SStat {
        value: value.into(),
        label: label.into(),
        icon: icon.into(),
        tone,
    }
}

fn build_view(rt: &mut Runtime) -> SocialView {
    let c = &rt.controller;
    let state = match (&c.phase, c.configured()) {
        (_, false) => 0,
        (IdentityPhase::NoIdentity, _) => 1,
        (IdentityPhase::NewIdentity { .. }, _) => 2,
        (IdentityPhase::ExistingIdentity { .. }, _) if !c.active() => 3,
        _ => 4,
    };
    let mut v = SocialView {
        state,
        subtab: Subtab::ALL.iter().position(|s| *s == c.subtab).unwrap_or(0) as i32,
        incoming_count: c.incoming_count() as i32,
        busy: c.syncing,
        name_prompt: c.name_prompt_open,
        confirm_new: c.confirm_new_account,
        name_editing: c.name_editing,
        wabi_competitive: c.wabi_competitive,
        restore_error: c.restore_error.clone().unwrap_or_default().into(),
        ..Default::default()
    };
    if state != 4 {
        return v;
    }
    let now = now();
    let clock = ChronoLocalClock;
    let profile = c.profile().cloned().expect("active");
    let friends = c.friends().cloned().unwrap_or_default();
    let sync = c.sync_status().cloned().unwrap_or_default();
    let identity = c.identity().cloned().expect("active");
    let scope = c.scope;
    let period = c.period;
    let board = c.board(scope, period);
    let friends_weekly = c.board(LeaderboardScope::Friends, LeaderboardPeriod::Weekly);
    let global_weekly = c.board(LeaderboardScope::Global, LeaderboardPeriod::Weekly);
    let viewing = c.viewing.clone();
    v.invite_link = format!(
        "https://damcha02.github.io/destudydracker/?invite={}",
        crate::net::http::percent_encode(profile.friend_code.as_str())
    )
    .into();
    let incoming: Vec<SRequestRow> = friends
        .incoming
        .iter()
        .map(|r| SRequestRow {
            id: r.id.as_str().into(),
            name: r.from_display_name.clone().into(),
            code: r.from_friend_code.as_str().into(),
            avatar: s_avatar(rt, &r.from_avatar, &r.from_display_name, false),
        })
        .collect();
    let outgoing: Vec<SRequestRow> = friends
        .outgoing
        .iter()
        .map(|r| SRequestRow {
            id: r.id.as_str().into(),
            name: r.to_display_name.clone().into(),
            code: r.to_friend_code.as_str().into(),
            avatar: s_avatar(rt, &r.to_avatar, &r.to_display_name, false),
        })
        .collect();
    let friend_rows: Vec<SFriendRow> = friends
        .friends
        .iter()
        .map(|f| {
            let seen = f
                .last_seen_at
                .map(|t| seen_label(t, now))
                .unwrap_or_default();
            SFriendRow {
                id: f.user_id.as_str().into(),
                name: f.display_name.clone().into(),
                code: f.friend_code.as_str().into(),
                detail: if seen.is_empty() {
                    f.friend_code.as_str().into()
                } else {
                    format!("{} · seen {}", f.friend_code.as_str(), seen).into()
                },
                avatar: s_avatar(rt, &f.avatar, &f.display_name, false),
                live: is_recently_active(f.last_seen_at, now, RECENTLY_ACTIVE_MS),
            }
        })
        .collect();
    let mut seen: Vec<_> = friends
        .friends
        .iter()
        .filter(|f| is_recently_active(f.last_seen_at, now, WABI_ATTENDANCE_WINDOW_MS))
        .collect();
    seen.sort_by_key(|f| std::cmp::Reverse(f.last_seen_at.map_or(0, |t| t.0)));
    let attendance: Vec<SFriendRow> = seen
        .into_iter()
        .map(|f| SFriendRow {
            id: f.user_id.as_str().into(),
            name: f.display_name.clone().into(),
            code: f.friend_code.as_str().into(),
            detail: Default::default(),
            avatar: Default::default(),
            live: is_recently_active(f.last_seen_at, now, RECENTLY_ACTIVE_MS),
        })
        .collect();
    v.attendance = ModelRc::new(VecModel::from(attendance));
    v.incoming = ModelRc::new(VecModel::from(incoming));
    v.outgoing = ModelRc::new(VecModel::from(outgoing));
    v.friends = ModelRc::new(VecModel::from(friend_rows));
    // leaderboard
    v.scope = match scope {
        LeaderboardScope::Friends => 0,
        LeaderboardScope::Squad => 1,
        LeaderboardScope::Global => 2,
    };
    v.period = match period {
        LeaderboardPeriod::Daily => 0,
        LeaderboardPeriod::Weekly => 1,
        LeaderboardPeriod::Overall => 2,
    };
    v.arena_title = scope.arena_label().into();
    v.arena_subtitle = if scope == LeaderboardScope::Squad {
        "Daily Sprint".into()
    } else {
        period.label().into()
    };
    v.col_label = period.fn_column().to_uppercase().into();
    v.last_synced = synced_label(sync.last_synced_at).into();
    v.private_notice = scope == LeaderboardScope::Global && profile.is_private;
    let top = top_minutes(&board);
    let rows: Vec<SBoardRow> = board
        .iter()
        .map(|e| SBoardRow {
            id: e.user_id.as_str().into(),
            rank: e.rank as i32,
            name: e.display_name.clone().into(),
            code: e.friend_code.as_str().into(),
            hours: format_minutes(e.minutes).into(),
            sessions: e.sessions.to_string().into(),
            bar: bar_percent(e.minutes, top) as f32 / 100.0,
            is_self: e.is_self,
            avatar: s_avatar(
                rt,
                if e.is_self {
                    &profile.avatar
                } else {
                    &e.avatar
                },
                &e.display_name,
                e.is_self,
            ),
        })
        .collect();
    v.rows = ModelRc::new(VecModel::from(rows));
    // profile
    v.name = profile.display_name.clone().into();
    v.code = profile.friend_code.as_str().into();
    v.avatar = s_avatar(rt, &profile.avatar, &profile.display_name, true);
    v.sync_pill = if sync.last_sync_error.is_some() { 1 } else { 0 };
    v.sync_error = sync.last_sync_error.clone().unwrap_or_default().into();
    v.private = profile.is_private;
    v.auto_post = profile.auto_post_sessions;
    v.show_hours = profile.show_hours_to_friends;
    let model = Rc::clone(&rt.model);
    let model = model.borrow();
    let academic = model.academic().state();
    let d = period_stat(academic, LeaderboardPeriod::Daily, now, &clock);
    let w = period_stat(academic, LeaderboardPeriod::Weekly, now, &clock);
    let o = period_stat(academic, LeaderboardPeriod::Overall, now, &clock);
    let m = monthly_stat(academic, now, &clock);
    let my_global = (!profile.is_private)
        .then(|| {
            global_weekly
                .iter()
                .find(|e| e.user_id == identity.user_id)
                .map(|e| e.rank)
        })
        .flatten();
    let my_friend = friends_weekly
        .iter()
        .find(|e| e.user_id == identity.user_id)
        .map(|e| e.rank);
    let stats = vec![
        stat(
            format_minutes(d.minutes),
            format!("TODAY · {} SES.", d.sessions),
            "↯",
            1,
        ),
        stat(
            format_minutes(w.minutes),
            format!("THIS WEEK · {} SES.", w.sessions),
            "◆",
            2,
        ),
        stat(
            format_minutes(o.minutes),
            format!("ALL TIME · {} SES.", o.sessions),
            "★",
            3,
        ),
        stat(
            format_minutes(m.minutes),
            format!("THIS MONTH · {} SES.", m.sessions),
            "📅",
            0,
        ),
        stat(
            if profile.is_private {
                "Hidden".into()
            } else {
                my_global.map_or("—".into(), |r| format!("#{r}"))
            },
            "GLOBAL RANK".into(),
            "⚔",
            0,
        ),
        stat(
            my_friend.map_or("—".into(), |r| format!("#{r}")),
            "FRIENDS RANK".into(),
            "👥",
            0,
        ),
    ];
    v.stats = ModelRc::new(VecModel::from(stats));
    // the player dialog
    if let Some(view) = viewing {
        let is_self = view.user_id == identity.user_id;
        let is_friend = friends.is_friend(&view.user_id);
        let pending = view
            .friend_code
            .as_ref()
            .is_some_and(|code| friends.has_outgoing_to(code));
        v.viewing = true;
        v.v_id = view.user_id.as_str().into();
        v.v_name = view.display_name.clone().into();
        v.v_code = view
            .friend_code
            .as_ref()
            .map(|c| c.as_str().to_string())
            .unwrap_or_default()
            .into();
        v.v_avatar = s_avatar(rt, &view.avatar, &view.display_name, is_self);
        v.v_badge = (if is_self {
            "YOUR PROFILE"
        } else if is_friend {
            "FRIEND"
        } else if pending {
            "REQUEST PENDING"
        } else {
            "NOT FRIENDS"
        })
        .into();
        v.v_can_request = !is_self && !is_friend && !pending && view.friend_code.is_some();
        let seen = if is_self {
            sync.last_synced_at
        } else {
            view.stats.as_ref().and_then(|s| s.last_seen_at)
        };
        v.v_seen = seen.map(|t| seen_label(t, now)).unwrap_or_default().into();
        let periods = if is_self {
            Some(
                [
                    (d.minutes, d.sessions, d.last_active),
                    (w.minutes, w.sessions, w.last_active),
                    (o.minutes, o.sessions, o.last_active),
                ]
                .map(|(m, s, l)| (m, s, l.map(|l| l.to_iso()))),
            )
        } else {
            view.stats
                .as_ref()
                .and_then(|s| s.periods.clone())
                .map(|p| p.map(|x| (x.minutes, x.sessions, x.last_active_date)))
        };
        v.v_state = if view.loading {
            0
        } else if periods.is_some() {
            1
        } else if view.stats.is_some() {
            2
        } else {
            3
        };
        if let Some([(dm, ds, dl), (wm, ws, _), (om, os, _)]) = periods {
            v.v_stats = ModelRc::new(VecModel::from(vec![
                stat(format_minutes(dm), format!("TODAY · {ds} SES."), "↯", 1),
                stat(format_minutes(wm), format!("THIS WEEK · {ws} SES."), "◆", 2),
                stat(format_minutes(om), format!("ALL TIME · {os} SES."), "★", 3),
                stat(
                    dl.unwrap_or_else(|| "—".into()),
                    "LAST ACTIVE".into(),
                    "📅",
                    0,
                ),
            ]));
        }
    }
    v
}

pub fn push() {
    let Some(window) = with(|rt| rt.window.upgrade()).flatten() else {
        return;
    };
    let data = with(|rt| {
        rt.pushes += 1;
        build_view(rt)
    });
    if let Some(v) = data {
        window.set_social(v);
    }
    let cleared = with(|rt| {
        let cleared = !rt.code_draft.is_empty() && rt.controller.friend_code_draft.is_empty();
        if cleared {
            rt.code_draft.clear();
        }
        cleared
    })
    .unwrap_or(false);
    if cleared {
        window.set_social_code_draft("".into());
    }
    let unread = unread();
    if window.get_social_unread() != unread {
        window.set_social_unread(unread);
    }
}

// --------------------------------------------------------------------------- callbacks

fn act(f: impl FnOnce(&mut SocialController, &SyncContext, &AppMetadata) -> Vec<Outgoing>) {
    let out = with(|rt| {
        let app = rt.app.clone();
        with_ctx(rt, |c, ctx| f(c, ctx, &app))
    })
    .unwrap_or_default();
    dispatch(out);
    after_change();
}

fn person_from_view(
    id: &str,
) -> Option<(
    UserId,
    String,
    Option<study_tracker_core::social::FriendCode>,
    Avatar,
)> {
    with(|rt| {
        let c = &rt.controller;
        let uid = UserId::parse(id)?;
        let f = c.friends()?;
        if let Some(friend) = f.friends.iter().find(|x| x.user_id == uid) {
            return Some((
                uid,
                friend.display_name.clone(),
                Some(friend.friend_code.clone()),
                friend.avatar.clone(),
            ));
        }
        for scope in [LeaderboardScope::Friends, LeaderboardScope::Global] {
            for period in LeaderboardPeriod::ALL {
                if let Some(e) = c
                    .board(scope, period)
                    .into_iter()
                    .find(|e| e.user_id == uid)
                {
                    return Some((uid, e.display_name, Some(e.friend_code), e.avatar));
                }
            }
        }
        None
    })
    .flatten()
}

fn bind(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_social_opened(move || {
        act(|c, ctx, app| c.tab_opened(app, now(), ctx));
        if let Some(w) = weak.upgrade() {
            w.set_social_prompt_draft("".into());
        }
    });
    window.on_social_subtab(|i| {
        if let Some(sub) = Subtab::ALL.get(i.max(0) as usize).copied() {
            act(|c, ctx, app| c.set_subtab(sub, app, now(), ctx));
        }
    });
    window.on_social_send_request(|code| {
        // the controller clears its copy once the server accepted; `push` mirrors that
        with(|rt| {
            rt.controller.friend_code_draft = code.to_string();
            rt.code_draft = code.to_string();
        });
        act(|c, _, _| c.send_friend_request(&code));
    });
    window.on_social_respond(|id, accept| {
        if let Some(id) = RequestId::parse(&id) {
            act(|c, _, _| {
                c.respond(
                    &id,
                    if accept {
                        FriendResponse::Accepted
                    } else {
                        FriendResponse::Declined
                    },
                )
            });
        }
    });
    window.on_social_open_person(|id| {
        let target = person_from_view(&id);
        let own = with(|rt| {
            rt.controller
                .identity()
                .is_some_and(|i| i.user_id.as_str() == id.as_str())
        })
        .unwrap_or(false);
        let target = target.or_else(|| {
            own.then(|| {
                with(|rt| {
                    let p = rt.controller.profile()?.clone();
                    Some((
                        rt.controller.identity()?.user_id.clone(),
                        p.display_name,
                        Some(p.friend_code),
                        p.avatar,
                    ))
                })
                .flatten()
            })
            .flatten()
        });
        if let Some((uid, name, code, avatar)) = target {
            act(|c, _, _| c.open_profile(uid, name, code, avatar));
        }
    });
    window.on_social_close_person(|| {
        with(|rt| rt.controller.close_profile());
        push();
    });
    let weak = window.as_weak();
    window.on_social_copy_code(move || {
        let code = with(|rt| {
            rt.controller
                .profile()
                .map(|p| p.friend_code.as_str().to_string())
        })
        .flatten();
        if let (Some(w), Some(code)) = (weak.upgrade(), code) {
            w.invoke_copy_to_clipboard(code.into());
            with(|rt| rt.controller.message = None);
            set_message("Friend code copied.");
        }
    });
    let weak = window.as_weak();
    window.on_social_copy_invite(move || {
        let link = with(|rt| {
            rt.controller.profile().map(|p| {
                format!(
                    "https://damcha02.github.io/destudydracker/?invite={}",
                    crate::net::http::percent_encode(p.friend_code.as_str())
                )
            })
        })
        .flatten();
        if let (Some(w), Some(link)) = (weak.upgrade(), link) {
            w.invoke_copy_to_clipboard(link.into());
            set_message("Friend invite link copied.");
        }
    });
    window.on_social_refresh(|| act(|c, ctx, _| c.manual_sync(now(), ctx)));
    window.on_social_scope(|i| {
        let scope = [
            LeaderboardScope::Friends,
            LeaderboardScope::Squad,
            LeaderboardScope::Global,
        ][i.clamp(0, 2) as usize];
        act(|c, _, _| c.set_scope(scope));
    });
    window.on_social_period(|i| {
        let period = LeaderboardPeriod::ALL[i.clamp(0, 2) as usize];
        act(|c, _, _| c.set_period(period));
    });
    let weak = window.as_weak();
    window.on_social_edit_name(move || {
        with(|rt| rt.controller.start_name_edit());
        if let Some(w) = weak.upgrade() {
            let draft = with(|rt| rt.controller.name_draft.clone()).unwrap_or_default();
            w.set_social_name_draft(draft.into());
        }
        push();
    });
    window.on_social_save_name(|name| act(|c, ctx, _| c.save_name(&name, now(), ctx)));
    window.on_social_cancel_name(|| {
        with(|rt| rt.controller.cancel_name_edit());
        push();
    });
    window.on_social_toggle_private(|| act(|c, ctx, _| c.toggle_private(now(), ctx)));
    window.on_social_toggle_auto_post(|| {
        with(|rt| rt.controller.toggle_auto_post());
        after_change();
    });
    window.on_social_toggle_hours(|| act(|c, ctx, _| c.toggle_show_hours(now(), ctx)));
    window.on_social_create_account(|| {
        with(|rt| rt.controller.ask_new_account());
        push();
    });
    window.on_social_cancel_account(|| {
        with(|rt| rt.controller.cancel_new_account());
        push();
    });
    let weak = window.as_weak();
    window.on_social_confirm_account(move || {
        let mut random = [0u8; 40];
        if getrandom::fill(&mut random).is_err() {
            set_message("Could not create the Social account.");
            return;
        }
        act(|c, ctx, _| c.create_account(random, now(), ctx));
        random.iter_mut().for_each(|b| *b = 0);
        if let Some(w) = weak.upgrade() {
            schedule_startup(&w);
        }
    });
    window.on_social_dismiss_prompt(|| {
        with(|rt| rt.controller.name_prompt_open = false);
        push();
    });
    window.on_social_save_prompt(|name| act(|c, ctx, _| c.save_name(&name, now(), ctx)));
    window.on_social_wabi_competitive(|on| {
        with(|rt| rt.controller.set_wabi_competitive(on));
        if let Some(path) = STORE_PATH.with(|p| p.borrow().clone()) {
            save_competitive(&NativeStore::new(path), on);
        }
        push();
    });
}

thread_local! {
    static STORE_PATH: RefCell<Option<std::path::PathBuf>> = const { RefCell::new(None) };
}

/// Remembers the store path for the Circle preference.
pub fn set_store_path(path: &Path) {
    STORE_PATH.with(|p| *p.borrow_mut() = Some(path.to_path_buf()));
}

fn set_message(text: &str) {
    with(|rt| {
        rt.controller.message = Some(text.to_string());
        rt.controller.message_seq += 1;
    });
    after_change();
}

/// The surface changed: leaving the Social tab stops its polling.
pub fn surface_changed(window: &MainWindow) {
    if !window.get_show_social() {
        let was = with(|rt| {
            let was = rt.controller.tab_visible;
            rt.controller.tab_closed();
            was
        })
        .unwrap_or(false);
        if was {
            after_change();
        }
    }
}

pub fn shutdown() {
    with(|rt| {
        rt.controller.shutdown();
        rt.startup.stop();
        rt.hourly.stop();
        rt.poll.stop();
        rt.session_debounce.stop();
        rt.banner.stop();
    });
}

pub fn report() -> String {
    with(|rt| {
        format!(
            "social_state={:?} social_pending={} social_requests={} social_coalesced={} social_writes={} social_pushes={} social_avatars={}",
            match &rt.controller.phase {
                IdentityPhase::NoIdentity => "none",
                IdentityPhase::NewIdentity { .. } => "new",
                IdentityPhase::ExistingIdentity { .. } => "existing",
            },
            rt.controller.pending_count(),
            rt.controller.requests_made,
            rt.controller.coalesced,
            rt.controller.port_writes(),
            rt.pushes,
            rt.pictures.len()
        )
    })
    .unwrap_or_default()
}
