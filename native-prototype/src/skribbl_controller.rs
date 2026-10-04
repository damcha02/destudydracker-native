//! Daily Skribbl's application controller (Stage 22a). Slint-free and network-free: it turns the
//! core state machine's effects into tagged [`Outgoing`] requests and applies [`NetReply`]s.
//!
//! Lifetime (production: everything lives in the modal component):
//! - `open` starts a session (generation `g`); every request carries a token registered for
//!   `g` and a cancel token;
//! - `close` cancels every pending request, bumps the generation and drops the session, the
//!   drawing and its undo history (nothing is persisted: production keeps no local Skribbl state);
//! - a reply whose token is unknown (cancelled, stale, from a previous opening) is ignored.
//!
//! Images: the gallery's thumbnails and the own drawing are fetched only after the image URL
//! policy accepted them (configured origin, `/skribbl/drawing/` route), decoded off the UI thread
//! at thumbnail size, and kept in a bounded cache that survives reopening (production's browser
//! cache keeps them too, `cache-control: max-age=1800`). The lightbox decodes one full-size copy.

use std::collections::HashMap;

use study_tracker_core::break_room::skribbl::{Effect, Phase, Session, ThemeInfo};
use study_tracker_core::social::ids::DrawingId;
use study_tracker_core::social::SocialIdentity;

use crate::image_cache::ImageCache;
use crate::net::endpoint::Origin;
use crate::net::http::NetError;
use crate::net::images::{self, ImageKind};
use crate::net::social_api;
use crate::net::worker::CancelToken;
use crate::net_jobs::{NetReply, Outgoing, Post, Tokens};
use crate::skribbl_canvas::SkribblCanvas;

/// Thumbnails: gallery cards (~160x107 logical) and the own drawing (max 240 px wide), with room
/// for a 2x display.
pub const THUMB_W: u32 = 480;
pub const THUMB_H: u32 = 320;
/// Thumbnails kept across openings (~600 KB each at most): ~14 MB worst case.
pub const THUMB_CACHE: usize = 24;

#[derive(Debug, Clone, PartialEq)]
enum Pending {
    Theme,
    Winner,
    Gallery { offset: usize },
    Submit,
    Vote { drawing: DrawingId, seq: u64 },
    Thumb { url: String },
    Full { url: String },
}

pub struct SkribblController {
    pub session: Option<Session>,
    /// The 900x600 raster (2 MiB): created when drawing starts, dropped when the modal closes.
    pub canvas: Option<SkribblCanvas>,
    identity: Option<SocialIdentity>,
    origin: Option<Origin>,
    tokens: Tokens,
    pending: HashMap<u64, (Pending, CancelToken)>,
    pub thumbs: ImageCache,
    /// The lightbox's full-size picture (one at a time).
    pub full: Option<(String, Option<crate::net::images::DecodedImage>)>,
    /// Counters for diagnostics and the stress checks.
    pub opened: u64,
    pub stale_replies: u64,
}

impl SkribblController {
    pub fn new() -> Self {
        Self {
            session: None,
            canvas: None,
            identity: None,
            origin: None,
            tokens: Tokens::default(),
            pending: HashMap::new(),
            thumbs: ImageCache::new(THUMB_CACHE),
            full: None,
            opened: 0,
            stale_replies: 0,
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    fn send(&mut self, pending: Pending) -> Option<Outgoing> {
        let identity = self.identity.as_ref()?;
        let (request, post) = match &pending {
            Pending::Theme => (social_api::skribbl_theme(identity), Post::None),
            Pending::Winner => (social_api::skribbl_leaderboard(identity), Post::None),
            Pending::Gallery { offset } => {
                let date = self
                    .session
                    .as_ref()
                    .map(|s| s.theme_date.clone())
                    .unwrap_or_default();
                (
                    social_api::skribbl_gallery(identity, &date, *offset),
                    Post::None,
                )
            }
            Pending::Vote { drawing, .. } => {
                let vote = self.vote_value(drawing);
                (
                    social_api::skribbl_vote(identity, drawing, vote)?,
                    Post::None,
                )
            }
            Pending::Thumb { url } | Pending::Full { url } => {
                let path = images::allow(url, ImageKind::SkribblDrawing, self.origin.as_ref()?)?;
                let (w, h) = if matches!(pending, Pending::Thumb { .. }) {
                    (THUMB_W, THUMB_H)
                } else {
                    (900, 600)
                };
                (
                    images::request(&path, ImageKind::SkribblDrawing),
                    Post::DecodeImage {
                        kind: ImageKind::SkribblDrawing,
                        max_w: w,
                        max_h: h,
                    },
                )
            }
            Pending::Submit => return None, // built by `submit_with_png`
        };
        let token = self.tokens.next();
        let cancel = CancelToken::new();
        self.pending.insert(token, (pending, cancel.clone()));
        Some(Outgoing {
            token,
            request,
            cancel,
            post,
        })
    }

    fn vote_value(&self, drawing: &DrawingId) -> i8 {
        self.session
            .as_ref()
            .and_then(|s| s.gallery.iter().find(|d| &d.id == drawing))
            .map_or(0, |d| d.my_vote)
    }

    fn run_effects(&mut self, effects: Vec<Effect>) -> Vec<Outgoing> {
        let mut out = Vec::new();
        for effect in effects {
            match effect {
                Effect::LoadTheme => out.extend(self.send(Pending::Theme)),
                Effect::LoadWinner => out.extend(self.send(Pending::Winner)),
                Effect::LoadGallery { offset } => {
                    out.extend(self.send(Pending::Gallery { offset }))
                }
                Effect::Vote { drawing, seq, .. } => {
                    out.extend(self.send(Pending::Vote { drawing, seq }))
                }
                Effect::Submit { date } => out.extend(self.submit_with_png(&date)),
            }
        }
        out.extend(self.request_images());
        out
    }

    /// PNG export and the upload request; a drawing the server would refuse (over 1.5 MB) fails
    /// locally and is never sent.
    fn submit_with_png(&mut self, date: &str) -> Option<Outgoing> {
        let identity = self.identity.clone()?;
        let png = self.canvas_mut().export_png();
        match social_api::skribbl_submit(&identity, png, date) {
            Ok(request) => {
                let token = self.tokens.next();
                let cancel = CancelToken::new();
                self.pending
                    .insert(token, (Pending::Submit, cancel.clone()));
                Some(Outgoing {
                    token,
                    request,
                    cancel,
                    post: Post::None,
                })
            }
            Err(err) => {
                if let Some(s) = self.session.as_mut() {
                    s.submit_failed(err.user_message("Submission failed. Try again."));
                }
                None
            }
        }
    }

    /// Thumbnails for the shown gallery rows and the own drawing.
    fn request_images(&mut self) -> Vec<Outgoing> {
        let Some(session) = &self.session else {
            return Vec::new();
        };
        let mut urls: Vec<String> = session
            .gallery
            .iter()
            .map(|d| d.image_url.clone())
            .collect();
        if let Some(own) = &session.my_image_url {
            urls.insert(0, own.clone());
        }
        let mut out = Vec::new();
        for url in urls {
            let allowed = self
                .origin
                .as_ref()
                .is_some_and(|o| images::allow(&url, ImageKind::SkribblDrawing, o).is_some());
            if !allowed {
                if self.thumbs.want(&url) {
                    log::warn!("skribbl: a drawing URL was refused by the image policy");
                    self.thumbs.failed(&url);
                }
                continue;
            }
            if self.thumbs.want(&url) {
                out.extend(self.send(Pending::Thumb { url }));
            }
        }
        out
    }

    /// Opens the modal. `identity` None = production's `!socialConfigured` (no request at all).
    pub fn open(
        &mut self,
        identity: Option<SocialIdentity>,
        origin: Option<Origin>,
    ) -> Vec<Outgoing> {
        self.close();
        self.identity = identity;
        self.origin = origin;
        self.opened += 1;
        let configured = self.identity.is_some() && self.origin.is_some();
        let (session, effects) = Session::open(configured);
        self.session = Some(session);
        self.thumbs.clear_failed();
        self.run_effects(effects)
    }

    /// Closes the modal: cancels everything in flight and frees the drawing.
    pub fn close(&mut self) {
        for (_, (pending, cancel)) in self.pending.drain() {
            cancel.cancel();
            if let Pending::Thumb { url } = pending {
                self.thumbs.forget(&url);
            }
        }
        self.session = None;
        self.full = None;
        self.canvas = None;
    }

    /// The raster, created on first use.
    pub fn canvas_mut(&mut self) -> &mut SkribblCanvas {
        self.canvas.get_or_insert_with(SkribblCanvas::new)
    }

    pub fn is_open(&self) -> bool {
        self.session.is_some()
    }

    pub fn phase(&self) -> Option<Phase> {
        self.session.as_ref().map(|s| s.phase)
    }

    pub fn retry(&mut self) -> Vec<Outgoing> {
        let effects = self
            .session
            .as_mut()
            .map(Session::retry)
            .unwrap_or_default();
        self.run_effects(effects)
    }

    pub fn start_drawing(&mut self, now_ms: u64) {
        if let Some(s) = self.session.as_mut() {
            s.start_drawing(now_ms);
            if s.phase == Phase::Drawing {
                self.canvas = Some(SkribblCanvas::new());
            }
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Vec<Outgoing> {
        let effects = self
            .session
            .as_mut()
            .map(|s| s.tick(now_ms))
            .unwrap_or_default();
        self.run_effects(effects)
    }

    pub fn submit(&mut self) -> Vec<Outgoing> {
        let effects = self
            .session
            .as_mut()
            .map(Session::submit)
            .unwrap_or_default();
        self.run_effects(effects)
    }

    pub fn load_more(&mut self) -> Vec<Outgoing> {
        let effects = match self.session.as_mut() {
            Some(s) => {
                let offset = s.gallery_offset;
                s.load_gallery(offset)
            }
            None => Vec::new(),
        };
        self.run_effects(effects)
    }

    pub fn refresh_gallery(&mut self) -> Vec<Outgoing> {
        let effects = self
            .session
            .as_mut()
            .map(|s| s.load_gallery(0))
            .unwrap_or_default();
        self.run_effects(effects)
    }

    pub fn vote(&mut self, drawing: &DrawingId, vote: i8) -> Vec<Outgoing> {
        let effects = self
            .session
            .as_mut()
            .map(|s| s.cast_vote(drawing, vote))
            .unwrap_or_default();
        self.run_effects(effects)
    }

    /// The lightbox (a full-size decode of one drawing).
    pub fn expand(&mut self, drawing: Option<DrawingId>) -> Vec<Outgoing> {
        let Some(s) = self.session.as_mut() else {
            return Vec::new();
        };
        s.expand(drawing);
        let url = s
            .expanded
            .as_ref()
            .and_then(|id| s.gallery.iter().find(|d| &d.id == id))
            .map(|d| d.image_url.clone());
        match url {
            Some(url) if self.full.as_ref().map(|(u, _)| u) != Some(&url) => {
                self.full = Some((url.clone(), None));
                self.send(Pending::Full { url }).into_iter().collect()
            }
            Some(_) => Vec::new(),
            None => {
                self.full = None;
                Vec::new()
            }
        }
    }

    pub fn on_reply(&mut self, token: u64, reply: NetReply) -> Vec<Outgoing> {
        let Some((pending, _)) = self.pending.remove(&token) else {
            self.stale_replies += 1;
            return Vec::new();
        };
        if let NetReply::Http(Err(NetError::Cancelled))
        | NetReply::Image(Err(NetError::Cancelled)) = &reply
        {
            if let Pending::Thumb { url } = &pending {
                self.thumbs.forget(url);
            }
            return Vec::new();
        }
        let Some(session) = self.session.as_mut() else {
            return Vec::new();
        };
        let effects = match pending {
            Pending::Theme => match reply.http().and_then(|r| social_api::parse_theme(&r)) {
                Ok(theme) => session.theme_loaded(theme),
                Err(err) => {
                    session.theme_failed(theme_error(&err));
                    Vec::new()
                }
            },
            Pending::Winner => {
                // failures are ignored (`.catch(() => undefined)`)
                if let Ok(winner) = reply
                    .http()
                    .and_then(|r| social_api::parse_skribbl_leaderboard(&r))
                {
                    session.winner_loaded(winner);
                }
                Vec::new()
            }
            Pending::Gallery { offset } => {
                match reply.http().and_then(|r| social_api::parse_gallery(&r)) {
                    Ok(page) => session.gallery_loaded(offset, page),
                    Err(_) => session.gallery_failed(),
                }
                Vec::new()
            }
            Pending::Submit => match reply.http().and_then(|r| social_api::parse_submit(&r)) {
                Ok(url) => session.submit_succeeded(url),
                Err(err) => {
                    session.submit_failed(err.user_message("Submission failed. Try again."));
                    Vec::new()
                }
            },
            Pending::Vote { drawing, seq } => {
                match reply.http().and_then(|r| social_api::parse_vote(&r)) {
                    Ok(score) => session.vote_succeeded(&drawing, seq, score),
                    Err(_) => session.vote_failed(&drawing, seq),
                }
                Vec::new()
            }
            Pending::Thumb { url } => {
                match reply.image() {
                    Ok(img) => self.thumbs.loaded(&url, img),
                    Err(_) => self.thumbs.failed(&url),
                }
                Vec::new()
            }
            Pending::Full { url } => {
                if let Some((u, slot)) = self.full.as_mut() {
                    if *u == url {
                        *slot = reply.image().ok();
                    }
                }
                Vec::new()
            }
        };
        self.run_effects(effects)
    }

    /// The submitted/own drawing and theme, for tests and diagnostics.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn theme(&self) -> Option<ThemeInfo> {
        self.session.as_ref().map(|s| ThemeInfo {
            date: s.theme_date.clone(),
            theme: s.theme.clone(),
            submitted: s.submitted,
            drawing_id: None,
            image_url: s.my_image_url.clone(),
        })
    }
}

impl Default for SkribblController {
    fn default() -> Self {
        Self::new()
    }
}

/// `loadTheme`'s error text: production maps a missing route to its own sentence and otherwise
/// shows the server's message or "Could not reach the Daily Skribbl server.".
fn theme_error(err: &NetError) -> String {
    match err.server_text() {
        Some(t) if t.to_lowercase().contains("not found") => {
            "Daily Skribbl isn't live on the server yet. If this keeps happening, re-deploy the worker.".into()
        }
        Some(t) => t.to_string(),
        None => match err {
            NetError::Offline | NetError::Timeout => "Could not reach the Daily Skribbl server.".into(),
            other => other.user_message("Could not reach the Daily Skribbl server."),
        },
    }
}

#[cfg(test)]
#[path = "skribbl_controller_tests.rs"]
mod tests;
