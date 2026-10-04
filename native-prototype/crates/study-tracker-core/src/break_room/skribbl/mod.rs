//! Daily Skribbl (Stage 22a): production's `components/SkribblRoom.tsx` as a pure state machine.
//!
//! What it is (from production, not from the name): one drawing prompt per **Europe/Zurich**
//! day, chosen by the Worker; three minutes to draw on a 900x600 canvas (brush, fill, undo,
//! clear); one submission per user per day; after submitting, a paginated gallery of today's
//! drawings with up/down votes (not on your own); and yesterday's winner. No rooms, turns,
//! guessing or live players. Production keeps **no** local Skribbl state: everything is
//! fetched when the modal opens and dropped when it closes, so nothing here is persisted.
//!
//! The machine takes events (responses, clicks, clock readings) and returns the requests the
//! application must make ([`Effect`]); it never performs I/O, reads a clock or holds pixels (the
//! canvas raster lives in the application, see `src/skribbl_canvas.rs`).
//!
//! Deliberate, documented differences from production (data-loss / busy-loop fixes only):
//! - the 3-minute countdown is deadline-based (`deadline - now`), not a 1 s `setInterval`
//!   decrement, so delayed frames, a hidden window or a busy UI cannot stretch it;
//! - expiry auto-submits **once**; production re-enters `drawing` at `0:00` after a failed submit
//!   and its effect immediately submits again, a tight retry loop while offline;
//! - a failed submission keeps the drawing (production unmounts the canvas while submitting, so
//!   the drawing is lost);
//! - vote responses are matched to the latest vote per drawing, so an out-of-order reply cannot
//!   overwrite a newer choice.

pub mod fill;
pub mod zurich;

use std::collections::HashMap;

use crate::social::ids::{DrawingId, UserId};

/// `CANVAS_W` x `CANVAS_H`.
pub const CANVAS_W: usize = 900;
pub const CANVAS_H: usize = 600;
/// `SKRIBBL_DRAW_SECONDS`.
pub const DRAW_SECONDS: u64 = 180;
/// `SKRIBBL_GALLERY_PAGE_SIZE` (the Worker clamps a requested limit to 12..=20).
pub const GALLERY_PAGE_SIZE: usize = 16;
/// The undo stack keeps the last 14 snapshots (`if (length > 14) shift()`).
pub const MAX_UNDO: usize = 14;
/// `floodFill`'s default tolerance.
pub const FILL_TOLERANCE: u8 = 40;
/// The timer turns red at or below 30 s (`data-low={timeLeft <= 30}`).
pub const LOW_TIME_SECONDS: u64 = 30;
/// `MAX_SKRIBBL_IMAGE_BYTES` (Worker): 1.5 MB.
pub const MAX_SUBMIT_BYTES: usize = 1_572_864;
/// Defensive: gallery rows kept across "Load more" pages. The Worker has no cap (it is bounded by
/// the day's submissions); this only bounds a hostile/pathological server.
pub const MAX_GALLERY_ROWS: usize = 2_000;

/// `PALETTE`, in production's order.
pub const PALETTE: [u32; 21] = [
    0x000000, 0x37474f, 0x795548, 0x6d4c41, 0xd84315, 0xe53935, 0xd81b60, 0x8e24aa, 0x5e35b1,
    0x3949ab, 0x1e88e5, 0x039be5, 0x00acc1, 0x00897b, 0x43a047, 0x7cb342, 0xc0ca33, 0xfdd835,
    0xffb300, 0xfb8c00, 0xffffff,
];
/// `BRUSH_SIZES` and the default (`useState(6)`).
pub const BRUSH_SIZES: [u32; 5] = [3, 6, 10, 16, 26];
pub const DEFAULT_BRUSH: u32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Brush,
    Fill,
}

/// `Phase` in production.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Loading,
    Intro,
    Drawing,
    Submitting,
    Submitted,
}

/// `GET /skribbl/theme` (validated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeInfo {
    /// The Worker's `YYYY-MM-DD` (Europe/Zurich), echoed back on submit and gallery requests.
    pub date: String,
    pub theme: String,
    pub submitted: bool,
    pub drawing_id: Option<DrawingId>,
    pub image_url: Option<String>,
}

/// One gallery row (`SkribblDrawing`, validated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawing {
    pub id: DrawingId,
    pub user_id: UserId,
    pub display_name: String,
    pub vote_score: i64,
    pub vote_count: u64,
    /// -1, 0 (none) or 1.
    pub my_vote: i8,
    pub is_self: bool,
    pub image_url: String,
}

/// `POST /skribbl/gallery` (validated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryPage {
    pub drawings: Vec<Drawing>,
    pub next_offset: Option<usize>,
    pub has_more: bool,
}

/// `GET /skribbl/leaderboard`'s winner (validated). Production shows only name and score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Winner {
    pub display_name: String,
    pub score: i64,
}

/// Requests the application must make. `gen` ties every response to the open session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    LoadTheme,
    LoadWinner,
    LoadGallery {
        offset: usize,
    },
    /// Export the canvas as PNG and upload it for `date`.
    Submit {
        date: String,
    },
    Vote {
        drawing: DrawingId,
        vote: i8,
        seq: u64,
    },
}

/// The open modal's state.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub phase: Phase,
    /// False when there is no Social identity (`!socialConfigured`): the modal shows production's
    /// "Connect the Social tab once..." note and makes no request at all.
    pub configured: bool,
    pub theme: String,
    pub theme_date: String,
    pub submitted: bool,
    pub my_image_url: Option<String>,
    pub error: Option<String>,
    pub tool: Tool,
    pub color: u32,
    pub brush: u32,
    /// Monotonic milliseconds at which drawing time runs out (set by `start_drawing`).
    pub deadline_ms: Option<u64>,
    expiry_handled: bool,
    pub submitting: bool,
    pub gallery: Vec<Drawing>,
    pub gallery_has_more: bool,
    pub gallery_offset: usize,
    pub gallery_loading: bool,
    pub winner: Option<Winner>,
    winner_requested_for: Option<String>,
    pub expanded: Option<DrawingId>,
    vote_seq: u64,
    latest_vote: HashMap<DrawingId, (u64, Drawing)>,
}

impl Session {
    /// The modal mounting. With an identity the theme is fetched at once (production's mount
    /// effect); without one nothing is requested.
    pub fn open(configured: bool) -> (Self, Vec<Effect>) {
        let session = Self {
            phase: if configured {
                Phase::Loading
            } else {
                Phase::Intro
            },
            configured,
            theme: String::new(),
            theme_date: String::new(),
            submitted: false,
            my_image_url: None,
            error: None,
            tool: Tool::Brush,
            color: PALETTE[0],
            brush: DEFAULT_BRUSH,
            deadline_ms: None,
            expiry_handled: false,
            submitting: false,
            gallery: Vec::new(),
            gallery_has_more: false,
            gallery_offset: 0,
            gallery_loading: false,
            winner: None,
            winner_requested_for: None,
            expanded: None,
            vote_seq: 0,
            latest_vote: HashMap::new(),
        };
        let effects = if configured {
            vec![Effect::LoadTheme]
        } else {
            Vec::new()
        };
        (session, effects)
    }

    /// "Try again" (`loadTheme`).
    pub fn retry(&mut self) -> Vec<Effect> {
        if !self.configured {
            return Vec::new();
        }
        self.error = None;
        vec![Effect::LoadTheme]
    }

    /// The winner is fetched once the theme date is known and again whenever it changes
    /// (`useEffect(..., [themeDate])`).
    fn winner_effect(&mut self) -> Option<Effect> {
        if self.theme_date.is_empty()
            || self.winner_requested_for.as_deref() == Some(&self.theme_date)
        {
            return None;
        }
        self.winner_requested_for = Some(self.theme_date.clone());
        Some(Effect::LoadWinner)
    }

    pub fn theme_loaded(&mut self, theme: ThemeInfo) -> Vec<Effect> {
        self.theme = theme.theme;
        self.theme_date = theme.date;
        self.submitted = theme.submitted;
        self.my_image_url = theme.image_url;
        self.phase = Phase::Intro;
        let mut effects = Vec::new();
        if self.submitted {
            effects.extend(self.load_gallery(0));
        }
        effects.extend(self.winner_effect());
        effects
    }

    /// A theme failure (`setTheme(""); setSubmitted(false); setError(message); setPhase("intro")`).
    pub fn theme_failed(&mut self, message: String) {
        self.theme.clear();
        self.submitted = false;
        self.error = Some(message);
        self.phase = Phase::Intro;
    }

    /// "Start Drawing": error cleared, a fresh 3-minute deadline. The application clears the
    /// canvas to white and the undo stack.
    pub fn start_drawing(&mut self, now_ms: u64) {
        if self.phase != Phase::Intro || self.submitted || !self.configured {
            return;
        }
        self.error = None;
        self.deadline_ms = Some(now_ms + DRAW_SECONDS * 1000);
        self.expiry_handled = false;
        self.phase = Phase::Drawing;
    }

    /// Whole seconds left, rounded up (production shows 3:00 for the first second).
    pub fn seconds_left(&self, now_ms: u64) -> u64 {
        match self.deadline_ms {
            Some(deadline) => deadline.saturating_sub(now_ms).div_ceil(1000),
            None => DRAW_SECONDS,
        }
    }

    /// `m:ss`.
    pub fn time_display(&self, now_ms: u64) -> String {
        let left = self.seconds_left(now_ms);
        format!("{}:{:02}", left / 60, left % 60)
    }

    pub fn time_low(&self, now_ms: u64) -> bool {
        self.seconds_left(now_ms) <= LOW_TIME_SECONDS
    }

    /// Clock reading while drawing: at the deadline, exactly one auto-submit.
    pub fn tick(&mut self, now_ms: u64) -> Vec<Effect> {
        if self.phase == Phase::Drawing && !self.expiry_handled && self.seconds_left(now_ms) == 0 {
            self.expiry_handled = true;
            return self.submit();
        }
        Vec::new()
    }

    /// "Submit drawing" (`submitNow`), guarded against double submission.
    pub fn submit(&mut self) -> Vec<Effect> {
        if self.submitting || self.phase != Phase::Drawing {
            return Vec::new();
        }
        self.submitting = true;
        self.error = None;
        self.phase = Phase::Submitting;
        vec![Effect::Submit {
            date: self.theme_date.clone(),
        }]
    }

    pub fn submit_succeeded(&mut self, image_url: String) -> Vec<Effect> {
        if self.phase != Phase::Submitting {
            return Vec::new();
        }
        self.submitting = false;
        self.submitted = true;
        self.my_image_url = Some(image_url);
        self.phase = Phase::Submitted;
        self.deadline_ms = None;
        self.load_gallery(0)
    }

    /// A failed upload returns to drawing with the message; the drawing itself is kept and the
    /// user submits again by hand (no automatic retry).
    pub fn submit_failed(&mut self, message: String) {
        if self.phase != Phase::Submitting {
            return;
        }
        self.submitting = false;
        self.error = Some(message);
        self.phase = Phase::Drawing;
    }

    /// `loadGallery(offset)`; ignored while a gallery request is already running (the buttons
    /// are disabled then).
    pub fn load_gallery(&mut self, offset: usize) -> Vec<Effect> {
        if !self.configured || self.gallery_loading {
            return Vec::new();
        }
        self.gallery_loading = true;
        vec![Effect::LoadGallery { offset }]
    }

    pub fn gallery_loaded(&mut self, offset: usize, page: GalleryPage) {
        self.gallery_loading = false;
        if offset == 0 {
            self.gallery = page.drawings;
        } else {
            let known: std::collections::HashSet<DrawingId> =
                self.gallery.iter().map(|d| d.id.clone()).collect();
            // a page that overlaps rows already shown (new submissions shift the offsets) must not
            // duplicate them
            self.gallery
                .extend(page.drawings.into_iter().filter(|d| !known.contains(&d.id)));
        }
        self.gallery.truncate(MAX_GALLERY_ROWS);
        self.gallery_has_more = page.has_more && self.gallery.len() < MAX_GALLERY_ROWS;
        self.gallery_offset = page.next_offset.unwrap_or(offset);
    }

    /// Gallery failures are ignored silently ("Gallery is secondary").
    pub fn gallery_failed(&mut self) {
        self.gallery_loading = false;
    }

    pub fn winner_loaded(&mut self, winner: Option<Winner>) {
        self.winner = winner;
    }

    /// `castVote`: own drawings cannot be voted; voting the current choice again clears it.
    /// Applies the optimistic change and returns the request.
    pub fn cast_vote(&mut self, id: &DrawingId, vote: i8) -> Vec<Effect> {
        let Some(row) = self.gallery.iter_mut().find(|d| &d.id == id) else {
            return Vec::new();
        };
        if row.is_self || !matches!(vote, -1 | 1) {
            return Vec::new();
        }
        let previous = row.clone();
        let next = if row.my_vote == vote { 0 } else { vote };
        row.vote_score = row.vote_score - i64::from(previous.my_vote) + i64::from(next);
        let had = u64::from(previous.my_vote != 0);
        row.vote_count = (row.vote_count + u64::from(next != 0)).saturating_sub(had);
        row.my_vote = next;
        self.vote_seq += 1;
        let seq = self.vote_seq;
        // the state to roll back to is the one before the *first* unanswered vote
        let rollback = self
            .latest_vote
            .get(id)
            .map_or(previous, |(_, before)| before.clone());
        self.latest_vote.insert(id.clone(), (seq, rollback));
        vec![Effect::Vote {
            drawing: id.clone(),
            vote: next,
            seq,
        }]
    }

    /// The server's score for the latest vote on that drawing (older replies are ignored).
    pub fn vote_succeeded(&mut self, id: &DrawingId, seq: u64, score: i64) {
        if self.latest_vote.get(id).map(|(s, _)| *s) != Some(seq) {
            return;
        }
        self.latest_vote.remove(id);
        if let Some(row) = self.gallery.iter_mut().find(|d| &d.id == id) {
            row.vote_score = score;
        }
    }

    /// Rolls the row back and shows "Vote could not be saved." (only for the latest vote).
    pub fn vote_failed(&mut self, id: &DrawingId, seq: u64) {
        if self.latest_vote.get(id).map(|(s, _)| *s) != Some(seq) {
            return;
        }
        if let Some((_, before)) = self.latest_vote.remove(id) {
            if let Some(row) = self.gallery.iter_mut().find(|d| &d.id == id) {
                *row = before;
            }
        }
        self.error = Some("Vote could not be saved.".to_string());
    }

    pub fn expand(&mut self, id: Option<DrawingId>) {
        self.expanded = id.filter(|id| self.gallery.iter().any(|d| &d.id == id));
    }

    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
    }

    pub fn set_color(&mut self, rgb: u32) {
        self.color = rgb & 0x00ff_ffff;
    }

    pub fn set_brush(&mut self, size: u32) {
        if BRUSH_SIZES.contains(&size) {
            self.brush = size;
        }
    }

    /// The gallery heading's count (`drawings.length` - the rows loaded so far).
    pub fn gallery_count_label(&self) -> String {
        let n = self.gallery.len();
        format!("{n} drawing{}", if n == 1 { "" } else { "s" })
    }
}

/// `{score} point(s)`.
pub fn winner_score_label(score: i64) -> String {
    format!("{score} point{}", if score == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests;
