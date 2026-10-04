//! The Daily Skribbl drawing surface (Stage 22a): production's 900x600 `<canvas>` as a native
//! raster, owned by Rust (never a browser canvas).
//!
//! Production's operations, one for one (`components/SkribblRoom.tsx`):
//! - Start / Clear: fill white (`fillRect` #ffffff);
//! - pointer down with the brush: a filled dot of diameter `brushSize` at the point
//!   (`arc(x, y, size/2)`), then each move draws a round-capped, round-joined segment of width
//!   `brushSize` from the last point (`lineTo` + `stroke`);
//! - pointer down with the fill tool: production's span flood fill (`study-tracker-core`), RGBA,
//!   tolerance 40;
//! - undo: a snapshot is taken before every stroke, fill and clear; the 14 newest are kept.
//!
//! Points arrive in canvas coordinates already (the view maps the displayed size onto 900x600 and
//! clamps, like `getCanvasPoint`), so resizing the window never changes the drawing.
//!
//! Memory: the raster is 2.16 MB. Undo snapshots are run-length encoded rows (a typical drawing is
//! mostly flat colour: a few KB to a few hundred KB each); at most 14 are kept, so the worst case
//! (noise in every snapshot) is bounded at about 14 x 2.2 MB.

use study_tracker_core::break_room::skribbl::fill::flood_fill;
use study_tracker_core::break_room::skribbl::{CANVAS_H, CANVAS_W, FILL_TOLERANCE, MAX_UNDO};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// A run-length-encoded copy of the pixels: `(run, rgba)` pairs.
struct Snapshot(Vec<(u32, [u8; 4])>);

impl Snapshot {
    fn take(data: &[u8]) -> Self {
        let mut runs: Vec<(u32, [u8; 4])> = Vec::new();
        for px in data.chunks_exact(4) {
            let p = [px[0], px[1], px[2], px[3]];
            match runs.last_mut() {
                Some((n, last)) if *last == p => *n += 1,
                _ => runs.push((1, p)),
            }
        }
        runs.shrink_to_fit();
        Self(runs)
    }

    fn restore(&self, data: &mut [u8]) {
        let mut i = 0;
        for (n, p) in &self.0 {
            for _ in 0..*n {
                data[i..i + 4].copy_from_slice(p);
                i += 4;
            }
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn bytes(&self) -> usize {
        self.0.len() * 8
    }
}

pub struct SkribblCanvas {
    pixmap: Pixmap,
    undo: Vec<Snapshot>,
    last: Option<(f32, f32)>,
    drawing: bool,
    /// Bumped on every pixel change, so the view only re-uploads a changed raster.
    revision: u64,
}

fn rgb(c: u32) -> (u8, u8, u8) {
    ((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

impl Default for SkribblCanvas {
    fn default() -> Self {
        Self::new()
    }
}

impl SkribblCanvas {
    pub fn new() -> Self {
        let mut pixmap = Pixmap::new(CANVAS_W as u32, CANVAS_H as u32).expect("900x600 pixmap");
        pixmap.fill(tiny_skia::Color::WHITE);
        Self {
            pixmap,
            undo: Vec::new(),
            last: None,
            drawing: false,
            revision: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn undo_depth(&self) -> usize {
        self.undo.len()
    }

    /// Opaque RGBA (premultiplied equals straight: the paper is white and every colour opaque).
    pub fn rgba(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Bytes held by the undo stack (for diagnostics/stress checks).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn undo_bytes(&self) -> usize {
        self.undo.iter().map(Snapshot::bytes).sum()
    }

    fn snapshot_for_undo(&mut self) {
        self.undo.push(Snapshot::take(self.pixmap.data()));
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
    }

    fn touched(&mut self) {
        self.revision += 1;
    }

    /// `startDrawing`: white paper, empty undo stack.
    pub fn reset(&mut self) {
        self.pixmap.fill(tiny_skia::Color::WHITE);
        self.undo.clear();
        self.last = None;
        self.drawing = false;
        self.touched();
    }

    /// `clearCanvas`: undoable white fill.
    pub fn clear(&mut self) {
        self.snapshot_for_undo();
        self.pixmap.fill(tiny_skia::Color::WHITE);
        self.touched();
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo.pop() else {
            return false;
        };
        snapshot.restore(self.pixmap.data_mut());
        self.touched();
        true
    }

    fn paint(color: u32) -> Paint<'static> {
        let (r, g, b) = rgb(color);
        let mut paint = Paint::default();
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = true;
        paint
    }

    /// Pointer down with the brush: snapshot, then a dot.
    pub fn begin_stroke(&mut self, x: f32, y: f32, size: u32, color: u32) {
        self.snapshot_for_undo();
        if let Some(path) = PathBuilder::from_circle(x, y, size as f32 / 2.0) {
            self.pixmap.fill_path(
                &path,
                &Self::paint(color),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        self.last = Some((x, y));
        self.drawing = true;
        self.touched();
    }

    /// Pointer move while drawing: one round segment from the last point.
    pub fn extend_stroke(&mut self, x: f32, y: f32, size: u32, color: u32) {
        let (Some((lx, ly)), true) = (self.last, self.drawing) else {
            return;
        };
        let mut pb = PathBuilder::new();
        pb.move_to(lx, ly);
        pb.line_to(x, y);
        if let Some(path) = pb.finish() {
            let stroke = Stroke {
                width: size as f32,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            self.pixmap.stroke_path(
                &path,
                &Self::paint(color),
                &stroke,
                Transform::identity(),
                None,
            );
        }
        self.last = Some((x, y));
        self.touched();
    }

    /// Pointer up / cancel / leave.
    pub fn end_stroke(&mut self) {
        self.drawing = false;
        self.last = None;
    }

    pub fn is_drawing(&self) -> bool {
        self.drawing
    }

    /// Pointer down with the fill tool: snapshot, then the flood fill.
    pub fn fill_at(&mut self, x: usize, y: usize, color: u32) {
        self.snapshot_for_undo();
        let (r, g, b) = rgb(color);
        let out = flood_fill(
            self.pixmap.data_mut(),
            CANVAS_W,
            CANVAS_H,
            x,
            y,
            [r, g, b, 255],
            FILL_TOLERANCE,
        );
        if out.capped {
            log::warn!("skribbl: flood fill hit its work cap");
        }
        self.touched();
    }

    /// The upload: a 900x600 RGB PNG (the paper is opaque, so no alpha channel is needed).
    pub fn export_png(&self) -> Vec<u8> {
        let mut rgb = Vec::with_capacity(CANVAS_W * CANVAS_H * 3);
        for px in self.pixmap.data().chunks_exact(4) {
            rgb.extend_from_slice(&px[..3]);
        }
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, CANVAS_W as u32, CANVAS_H as u32);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.set_compression(png::Compression::High);
            let Ok(mut writer) = enc.write_header() else {
                return Vec::new();
            };
            if writer.write_image_data(&rgb).is_err() {
                return Vec::new();
            }
        }
        out
    }
}

/// `getCanvasPoint`: a position on the displayed canvas (logical pixels, `w` x `h`) mapped onto
/// 900x600, rounded and clamped to the raster.
pub fn canvas_point(px: f32, py: f32, w: f32, h: f32) -> (usize, usize) {
    let map = |v: f32, size: f32, n: usize| -> usize {
        if size <= 0.0 || !v.is_finite() {
            return 0;
        }
        let scaled = (v * (n as f32 / size)).round();
        scaled.clamp(0.0, (n - 1) as f32) as usize
    };
    (map(px, w, CANVAS_W), map(py, h, CANVAS_H))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(c: &SkribblCanvas, x: usize, y: usize) -> [u8; 4] {
        let i = (y * CANVAS_W + x) * 4;
        let d = c.rgba();
        [d[i], d[i + 1], d[i + 2], d[i + 3]]
    }

    #[test]
    fn strokes_dots_fill_and_undo_follow_production() {
        let mut c = SkribblCanvas::new();
        assert_eq!(px(&c, 10, 10), [255, 255, 255, 255], "white paper");
        c.begin_stroke(100.0, 100.0, 10, 0xe53935);
        assert_eq!(px(&c, 100, 100), [229, 57, 53, 255], "the dot");
        c.extend_stroke(200.0, 100.0, 10, 0xe53935);
        assert_eq!(px(&c, 150, 100), [229, 57, 53, 255], "the segment");
        assert_eq!(px(&c, 150, 120), [255, 255, 255, 255]);
        c.end_stroke();
        c.extend_stroke(300.0, 300.0, 10, 0xe53935);
        assert_eq!(
            px(&c, 250, 200),
            [255, 255, 255, 255],
            "no stroke after release"
        );
        assert_eq!(c.undo_depth(), 1, "one snapshot per stroke");
        c.fill_at(5, 5, 0x1e88e5);
        assert_eq!(px(&c, 899, 599), [30, 136, 229, 255]);
        assert_eq!(
            px(&c, 150, 100),
            [229, 57, 53, 255],
            "the stroke is a border"
        );
        c.undo();
        assert_eq!(px(&c, 899, 599), [255, 255, 255, 255], "undo the fill");
        c.undo();
        assert_eq!(px(&c, 150, 100), [255, 255, 255, 255], "undo the stroke");
        assert!(!c.undo(), "nothing left");
        c.clear();
        assert_eq!(c.undo_depth(), 1, "clear is undoable");
    }

    #[test]
    fn the_undo_stack_keeps_the_14_newest_and_stays_small() {
        let mut c = SkribblCanvas::new();
        for i in 0..40u32 {
            c.begin_stroke(10.0 + i as f32 * 20.0, 300.0, 26, 0x000000 + i);
            c.extend_stroke(10.0 + i as f32 * 20.0, 500.0, 26, i);
            c.end_stroke();
        }
        assert_eq!(c.undo_depth(), MAX_UNDO);
        assert!(
            c.undo_bytes() < 14 * 256 * 1024,
            "flat drawings compress well below the raw 14 x 2.16 MB: {}",
            c.undo_bytes()
        );
        for _ in 0..MAX_UNDO {
            assert!(c.undo());
        }
        // the oldest 26 strokes were dropped from history, the canvas holds them still
        assert_ne!(px(&c, 10, 400), [255, 255, 255, 255]);
    }

    #[test]
    fn repeated_full_fills_and_resets_do_not_grow() {
        let mut c = SkribblCanvas::new();
        for i in 0..100u32 {
            c.fill_at(450, 300, if i % 2 == 0 { 0x43a047 } else { 0xffffff });
        }
        assert_eq!(c.undo_depth(), MAX_UNDO);
        c.reset();
        assert_eq!((c.undo_depth(), px(&c, 1, 1)), (0, [255, 255, 255, 255]));
    }

    #[test]
    fn png_export_is_900x600_and_small_for_normal_drawings() {
        let mut c = SkribblCanvas::new();
        c.begin_stroke(100.0, 100.0, 16, 0x000000);
        c.extend_stroke(800.0, 500.0, 16, 0x000000);
        c.end_stroke();
        let png = c.export_png();
        assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
        let decoder = png::Decoder::new(std::io::Cursor::new(&png));
        let info = decoder.read_info().unwrap().info().clone();
        assert_eq!((info.width, info.height), (900, 600));
        assert!(png.len() < 100_000, "{} bytes", png.len());
    }

    #[test]
    fn worst_case_noise_can_exceed_the_server_limit_and_is_caught_before_upload() {
        let mut c = SkribblCanvas::new();
        let mut s = 12345u32;
        for p in c.pixmap.data_mut().chunks_exact_mut(4) {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            p[..3].copy_from_slice(&s.to_le_bytes()[..3]);
            p[3] = 255;
        }
        let png = c.export_png();
        assert!(
            png.len() > study_tracker_core::break_room::skribbl::MAX_SUBMIT_BYTES,
            "{}",
            png.len()
        );
        // the API layer refuses it locally (never uploaded)
        let id = study_tracker_core::social::SocialIdentity::mint([1; 16], [2; 16]);
        assert!(matches!(
            crate::net::social_api::skribbl_submit(&id, png, "2026-10-04"),
            Err(crate::net::http::NetError::PayloadTooLarge(_))
        ));
    }

    #[test]
    fn canvas_points_map_and_clamp_like_get_canvas_point() {
        assert_eq!(canvas_point(0.0, 0.0, 920.0, 613.3), (0, 0));
        assert_eq!(canvas_point(920.0, 613.3, 920.0, 613.3), (899, 599));
        assert_eq!(canvas_point(460.0, 306.65, 920.0, 613.3), (450, 300));
        assert_eq!(
            canvas_point(-50.0, 9999.0, 920.0, 613.3),
            (0, 599),
            "outside the canvas clamps"
        );
        // the logical result does not depend on the displayed size
        assert_eq!(
            canvas_point(230.0, 153.3, 460.0, 306.65),
            canvas_point(460.0, 306.6, 920.0, 613.3)
        );
        assert_eq!(canvas_point(f32::NAN, 1.0, 0.0, 0.0), (0, 0));
    }
}
