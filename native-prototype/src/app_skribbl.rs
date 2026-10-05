//! Daily Skribbl's Slint glue (Stage 22a): the modal's callbacks, the network round trips and the
//! pushes. Game logic is in `study-tracker-core`, orchestration in `skribbl_controller`.
//!
//! Rendering discipline:
//! - the modal's data is pushed only when something changed (a reply, a click);
//! - strokes update the raster immediately, but its picture is uploaded at most once per ~16 ms
//!   (one coalescing single-shot timer), and only when the raster's revision moved;
//! - the countdown is one single-shot timer re-armed for the next second boundary of the
//!   deadline while drawing; while the window is minimized/hidden it keeps the deadline (so the
//!   automatic submission still happens on time) but pushes nothing - and on restore the next
//!   tick shows the correct time derived from the deadline, never a backlog of ticks.
//! - no timer exists unless the drawing phase is on screen.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use slint::{
    ComponentHandle, Image, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, Timer, TimerMode,
    VecModel,
};
use study_tracker_core::break_room::skribbl::{
    winner_score_label, Phase, Tool, CANVAS_H, CANVAS_W, PALETTE,
};
use study_tracker_core::social::ids::DrawingId;

use crate::image_cache::ImageState;
use crate::net::images::DecodedImage;
use crate::net_jobs::{NetReply, Outgoing};
use crate::skribbl_canvas::canvas_point;
use crate::skribbl_controller::SkribblController;
use crate::{MainWindow, SkribblData, SkribblRow};

struct Runtime {
    controller: SkribblController,
    window: slint::Weak<MainWindow>,
    pictures: HashMap<String, Image>,
    rows: Rc<VecModel<SkribblRow>>,
    canvas_revision: u64,
    canvas_timer: Timer,
    clock: Timer,
    pushes: u64,
    canvas_uploads: u64,
}

thread_local! {
    static RT: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Runtime) -> R) -> Option<R> {
    RT.with(|r| r.borrow_mut().as_mut().map(f))
}

/// Monotonic milliseconds for the drawing deadline.
fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

fn visible(window: &MainWindow) -> bool {
    #[cfg(windows)]
    let hidden = crate::app_platform::is_hidden_to_tray();
    #[cfg(not(windows))]
    let hidden = false;
    !window.window().is_minimized() && !hidden && window.window().is_visible()
}

fn to_image(img: &DecodedImage) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        &img.rgba, img.width, img.height,
    ))
}

fn rgb_color(c: u32) -> slint::Color {
    slint::Color::from_rgb_u8((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

pub fn install(window: &MainWindow) {
    RT.with(|r| {
        *r.borrow_mut() = Some(Runtime {
            controller: SkribblController::new(),
            window: window.as_weak(),
            pictures: HashMap::new(),
            rows: Rc::new(VecModel::default()),
            canvas_revision: u64::MAX,
            canvas_timer: Timer::default(),
            clock: Timer::default(),
            pushes: 0,
            canvas_uploads: 0,
        })
    });
    bind(window);
}

fn dispatch(out: Vec<Outgoing>) {
    for o in out {
        crate::app_net::submit(o, |token, reply| on_reply(token, reply));
    }
}

fn on_reply(token: u64, reply: NetReply) {
    let out = with(|rt| rt.controller.on_reply(token, reply)).unwrap_or_default();
    dispatch(out);
    push();
}

/// The Break Room opened Daily Skribbl (after the play was logged).
pub fn open() {
    let (identity, origin) = crate::app_social::skribbl_access();
    let out = with(|rt| rt.controller.open(identity, origin)).unwrap_or_default();
    dispatch(out);
    push();
}

/// The modal closed: everything in flight is cancelled; the drawing is dropped.
pub fn close() {
    with(|rt| {
        rt.controller.close();
        rt.clock.stop();
        rt.canvas_timer.stop();
        rt.pictures
            .retain(|k, _| rt.controller.thumbs.state(k).is_some());
        // the window's copy of the raster goes too; the next drawing uploads afresh
        rt.canvas_revision = u64::MAX;
        if let Some(w) = rt.window.upgrade() {
            w.set_skribbl_canvas(Image::default());
        }
    });
}

pub fn is_open() -> bool {
    with(|rt| rt.controller.is_open()).unwrap_or(false)
}

/// Writes the modal's data (the only writer).
pub fn push() {
    let Some(window) = with(|rt| rt.window.upgrade()).flatten() else {
        return;
    };
    let data = with(|rt| {
        rt.pushes += 1;
        let c = &mut rt.controller;
        let Some(s) = c.session.as_ref() else {
            return None;
        };
        // move freshly decoded pictures to the view, drop pictures the cache let go of
        let mut urls: Vec<String> = s.gallery.iter().map(|d| d.image_url.clone()).collect();
        if let Some(u) = &s.my_image_url {
            urls.push(u.clone());
        }
        for url in &urls {
            if let Some(img) = c.thumbs.take_pixels(url) {
                rt.pictures.insert(url.clone(), to_image(&img));
            }
        }
        let keys: std::collections::HashSet<String> = c.thumbs.keys().cloned().collect();
        rt.pictures.retain(|k, _| keys.contains(k));
        let state_of = |url: &str, pictures: &HashMap<String, Image>| -> (i32, Image) {
            match c.thumbs.state(url) {
                Some(ImageState::Shown) | Some(ImageState::Ready(_)) => match pictures.get(url) {
                    Some(img) => (1, img.clone()),
                    None => (0, Image::default()),
                },
                Some(ImageState::Failed) => (2, Image::default()),
                _ => (0, Image::default()),
            }
        };
        let rows: Vec<SkribblRow> = s
            .gallery
            .iter()
            .map(|d| {
                let (state, image) = state_of(&d.image_url, &rt.pictures);
                SkribblRow {
                    id: d.id.as_str().into(),
                    name: d.display_name.clone().into(),
                    alt: format!("Drawing by {}", d.display_name).into(),
                    score: d.vote_score.to_string().into(),
                    my_vote: i32::from(d.my_vote),
                    is_self: d.is_self,
                    image,
                    state,
                }
            })
            .collect();
        sync_gallery(&rt.rows, rows);
        let (own_state, own) = match &s.my_image_url {
            Some(u) => state_of(u, &rt.pictures),
            None => (-1, Image::default()),
        };
        let phase = match s.phase {
            Phase::Loading => 0,
            Phase::Intro => 1,
            Phase::Drawing => 2,
            Phase::Submitting => 3,
            Phase::Submitted => 4,
        };
        let (lightbox, lightbox_ready, lightbox_image) = match &c.full {
            Some((_, Some(img))) => (true, true, to_image(img)),
            Some((_, None)) => (true, false, Image::default()),
            None => (false, false, Image::default()),
        };
        let lightbox_alt = s
            .expanded
            .as_ref()
            .and_then(|id| s.gallery.iter().find(|d| &d.id == id))
            .map(|d| {
                format!(
                    "Drawing by {}",
                    if d.is_self { "you" } else { &d.display_name }
                )
            })
            .unwrap_or_default();
        Some(SkribblData {
            configured: s.configured,
            phase,
            theme: s.theme.clone().into(),
            submitted: s.submitted,
            error: s.error.clone().unwrap_or_default().into(),
            own_state,
            own,
            tool: if s.tool == Tool::Brush { 0 } else { 1 },
            color: rgb_color(s.color),
            color_index: PALETTE
                .iter()
                .position(|p| *p == s.color)
                .map_or(-1, |i| i as i32),
            brush: s.brush as i32,
            submitting: s.submitting,
            undo_depth: c.canvas.as_ref().map_or(0, |cv| cv.undo_depth()) as i32,
            gallery: ModelRc::from(rt.rows.clone()),
            gallery_count: s.gallery_count_label().into(),
            gallery_loading: s.gallery_loading,
            has_more: s.gallery_has_more,
            has_winner: s.winner.is_some(),
            winner_name: s
                .winner
                .as_ref()
                .map(|w| w.display_name.clone())
                .unwrap_or_default()
                .into(),
            winner_score: s
                .winner
                .as_ref()
                .map(|w| winner_score_label(w.score))
                .unwrap_or_default()
                .into(),
            lightbox,
            lightbox_ready,
            lightbox_image,
            lightbox_alt: lightbox_alt.into(),
        })
    })
    .flatten();
    if let Some(d) = data {
        window.set_skribbl(d);
    }
    upload_canvas(&window);
    arm_clock(&window);
}

/// Updates the gallery rows in place, except a row whose picture changed (it finished loading,
/// failed, or was replaced): that row is removed and inserted again, so its card is created with
/// the picture. Windows verification (W22a-2): a thumbnail whose image arrived after its card's
/// first paint (`set_row_data`) kept painting nothing under femtovg, while cards created with the
/// image - the user's own drawing, or every card after reopening the modal - showed it.
fn sync_gallery(cache: &VecModel<SkribblRow>, rows: Vec<SkribblRow>) {
    if cache.row_count() != rows.len() {
        cache.set_vec(rows);
        return;
    }
    for (i, row) in rows.into_iter().enumerate() {
        let Some(old) = cache.row_data(i) else {
            continue;
        };
        if old == row {
            continue;
        }
        if old.state != row.state || old.image != row.image {
            cache.remove(i);
            cache.insert(i, row);
        } else {
            cache.set_row_data(i, row);
        }
    }
}

/// Uploads the raster if it changed since the last upload.
fn upload_canvas(window: &MainWindow) {
    let image = with(|rt| {
        let canvas = rt.controller.canvas.as_ref()?;
        if canvas.revision() == rt.canvas_revision {
            return None;
        }
        rt.canvas_revision = canvas.revision();
        rt.canvas_uploads += 1;
        let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            canvas.rgba(),
            CANVAS_W as u32,
            CANVAS_H as u32,
        );
        Some(Image::from_rgba8(buffer))
    })
    .flatten();
    if let Some(img) = image {
        window.set_skribbl_canvas(img);
    }
}

/// A stroke changed the raster: upload it within one frame, once.
fn schedule_canvas(window: &MainWindow) {
    let weak = window.as_weak();
    with(|rt| {
        if rt.canvas_timer.running() {
            return;
        }
        rt.canvas_timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(16),
            move || {
                if let Some(w) = weak.upgrade() {
                    upload_canvas(&w);
                }
            },
        );
    });
}

/// The countdown: one single-shot timer per displayed second, only while drawing.
fn arm_clock(window: &MainWindow) {
    let weak = window.as_weak();
    with(|rt| {
        let drawing = rt.controller.phase() == Some(Phase::Drawing);
        let deadline = rt.controller.session.as_ref().and_then(|s| s.deadline_ms);
        if !drawing || deadline.is_none() {
            rt.clock.stop();
            return;
        }
        let now = now_ms();
        let deadline = deadline.unwrap_or(now);
        let left = deadline.saturating_sub(now);
        // the next moment the displayed seconds change (or the deadline itself)
        let next = if left == 0 {
            250
        } else {
            (left % 1000).max(1) + 2
        };
        rt.clock.start(
            TimerMode::SingleShot,
            Duration::from_millis(next),
            move || {
                let Some(w) = weak.upgrade() else { return };
                on_clock(&w);
            },
        );
    });
}

fn on_clock(window: &MainWindow) {
    let now = now_ms();
    let out = with(|rt| rt.controller.tick(now)).unwrap_or_default();
    let submitted_now = !out.is_empty();
    dispatch(out);
    if visible(window) {
        let (text, low) = with(|rt| {
            let s = rt.controller.session.as_ref();
            (
                s.map(|s| s.time_display(now)).unwrap_or_default(),
                s.is_some_and(|s| s.time_low(now)),
            )
        })
        .unwrap_or_default();
        if window.get_skribbl_timer() != text.as_str() {
            window.set_skribbl_timer(text.into());
        }
        if window.get_skribbl_low() != low {
            window.set_skribbl_low(low);
        }
    }
    if submitted_now {
        push();
    } else {
        arm_clock(window);
    }
}

fn act(f: impl FnOnce(&mut SkribblController) -> Vec<Outgoing>) {
    let out = with(|rt| f(&mut rt.controller)).unwrap_or_default();
    dispatch(out);
    push();
}

fn bind(window: &MainWindow) {
    window.on_skribbl_retry(|| act(|c| c.retry()));
    let weak = window.as_weak();
    window.on_skribbl_start(move || {
        with(|rt| rt.controller.start_drawing(now_ms()));
        if let Some(w) = weak.upgrade() {
            w.set_skribbl_timer("3:00".into());
            w.set_skribbl_low(false);
        }
        push();
    });
    window.on_skribbl_tool(|i| {
        with(|rt| {
            if let Some(s) = rt.controller.session.as_mut() {
                s.set_tool(if i == 0 { Tool::Brush } else { Tool::Fill });
            }
        });
        push();
    });
    window.on_skribbl_size(|size| {
        with(|rt| {
            if let Some(s) = rt.controller.session.as_mut() {
                s.set_brush(size.max(0) as u32);
            }
        });
        push();
    });
    window.on_skribbl_color(|i| {
        with(|rt| {
            if let (Some(s), Some(c)) = (
                rt.controller.session.as_mut(),
                PALETTE.get(i.max(0) as usize),
            ) {
                s.set_color(*c);
            }
        });
        push();
    });
    window.on_skribbl_custom_color(|text| {
        if let Some(v) = parse_custom_color(&text) {
            with(|rt| {
                if let Some(s) = rt.controller.session.as_mut() {
                    s.set_color(v);
                }
            });
        }
        push();
    });
    window.on_skribbl_undo(|| {
        with(|rt| rt.controller.canvas_mut().undo());
        push();
    });
    window.on_skribbl_clear(|| {
        with(|rt| rt.controller.canvas_mut().clear());
        push();
    });
    let weak = window.as_weak();
    window.on_skribbl_pointer(move |kind, x, y, w, h| {
        let Some(win) = weak.upgrade() else { return };
        let changed = with(|rt| {
            let c = &mut rt.controller;
            let Some(s) = c.session.as_ref() else {
                return false;
            };
            if s.phase != Phase::Drawing {
                return false;
            }
            let (tool, color, brush) = (s.tool, s.color, s.brush);
            let (px, py) = canvas_point(x, y, w, h);
            let canvas = c.canvas_mut();
            match (kind, tool) {
                (0, Tool::Fill) => {
                    canvas.fill_at(px, py, color);
                    true
                }
                (0, Tool::Brush) => {
                    canvas.begin_stroke(px as f32, py as f32, brush, color);
                    true
                }
                (1, Tool::Brush) if canvas.is_drawing() => {
                    canvas.extend_stroke(px as f32, py as f32, brush, color);
                    true
                }
                (2, _) => {
                    canvas.end_stroke();
                    false
                }
                _ => false,
            }
        })
        .unwrap_or(false);
        if changed {
            schedule_canvas(&win);
            if kind == 0 {
                push(); // the undo button's state
            }
        }
    });
    window.on_skribbl_submit(|| act(SkribblController::submit));
    window.on_skribbl_refresh(|| act(SkribblController::refresh_gallery));
    window.on_skribbl_more(|| act(SkribblController::load_more));
    window.on_skribbl_vote(|id, v| {
        if let Some(id) = DrawingId::parse(&id) {
            act(|c| c.vote(&id, v.clamp(-1, 1) as i8));
        }
    });
    window.on_skribbl_expand(|id| {
        if let Some(id) = DrawingId::parse(&id) {
            act(|c| c.expand(Some(id)));
        }
    });
    window.on_skribbl_collapse(|| act(|c| c.expand(None)));
}

/// Diagnostics (STATS line).
pub fn report() -> String {
    with(|rt| {
        format!(
            "skribbl_open={} skribbl_pushes={} skribbl_canvas_uploads={} skribbl_pending={} skribbl_thumbs={} skribbl_pictures={}",
            rt.controller.is_open(),
            rt.pushes,
            rt.canvas_uploads,
            rt.controller.pending_count(),
            rt.controller.thumbs.len(),
            rt.pictures.len()
        )
    })
    .unwrap_or_default()
}

/// Rows currently shown (tests/diagnostics).
#[allow(dead_code)]
pub fn row_count() -> usize {
    with(|rt| rt.rows.row_count()).unwrap_or(0)
}

/// The custom colour: `#rrggbb` typed in the field, or `rgb(r,g,b)` from the HSV picker
/// (production's `<input type="color">` hands over `#rrggbb` the same way).
fn parse_custom_color(text: &str) -> Option<u32> {
    let t = text.trim();
    if let Some(inner) = t.strip_prefix("rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<u32> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<u32>().ok())
            .collect();
        return (parts.len() == 3 && parts.iter().all(|c| *c <= 255))
            .then(|| (parts[0] << 16) | (parts[1] << 8) | parts[2]);
    }
    let hex = t.trim_start_matches('#');
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod custom_color_tests {
    #[test]
    fn custom_colours_parse_from_the_field_and_the_picker() {
        assert_eq!(super::parse_custom_color("#ff8800"), Some(0xff8800));
        assert_eq!(super::parse_custom_color(" 00ff00 "), Some(0x00ff00));
        assert_eq!(
            super::parse_custom_color("rgb(255, 136, 0)"),
            Some(0xff8800)
        );
        assert_eq!(super::parse_custom_color("rgb(256,0,0)"), None);
        assert_eq!(super::parse_custom_color("#ff88"), None);
        assert_eq!(super::parse_custom_color("+1ff88"), None);
    }
}
