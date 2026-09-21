//! Viewport: world <-> screen mapping with bounded zoom and clamped pan.
//!
//! State is `(zoom, centre)` in f64 rather than an accumulated pixel offset, so long sessions of
//! pan/zoom cannot drift: every transform is recomputed from those two values.

use super::{Bounds, Point, WORLD_HEIGHT, WORLD_WIDTH};

pub const MIN_ZOOM: f64 = 1.0;
pub const MAX_ZOOM: f64 = 64.0;
const MIN_VIEWPORT: f64 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    width: f64,
    height: f64,
    zoom: f64,
    center: Point,
}

impl Viewport {
    pub fn new(width: f64, height: f64) -> Self {
        let mut v = Self {
            width: MIN_VIEWPORT,
            height: MIN_VIEWPORT,
            zoom: MIN_ZOOM,
            center: (WORLD_WIDTH / 2.0, WORLD_HEIGHT / 2.0),
        };
        v.resize(width, height);
        v
    }

    pub fn size(&self) -> (f64, f64) {
        (self.width, self.height)
    }

    pub fn screen_centre(&self) -> Point {
        (self.width / 2.0, self.height / 2.0)
    }

    pub fn zoom(&self) -> f64 {
        self.zoom
    }

    pub fn center(&self) -> Point {
        self.center
    }

    pub fn resize(&mut self, width: f64, height: f64) {
        if width.is_finite() && height.is_finite() {
            self.width = width.max(MIN_VIEWPORT);
            self.height = height.max(MIN_VIEWPORT);
            self.clamp_center();
        }
    }

    /// Pixels per world unit at zoom 1 (whole world fits).
    pub fn base_scale(&self) -> f64 {
        (self.width / WORLD_WIDTH).min(self.height / WORLD_HEIGHT)
    }

    /// Pixels per world unit at the current zoom.
    pub fn scale(&self) -> f64 {
        self.base_scale() * self.zoom
    }

    pub fn world_to_screen(&self, p: Point) -> Point {
        let s = self.scale();
        (
            (p.0 - self.center.0) * s + self.width / 2.0,
            (p.1 - self.center.1) * s + self.height / 2.0,
        )
    }

    pub fn screen_to_world(&self, p: Point) -> Point {
        let s = self.scale();
        (
            (p.0 - self.width / 2.0) / s + self.center.0,
            (p.1 - self.height / 2.0) / s + self.center.1,
        )
    }

    /// Zoom by `factor` keeping the world point under `screen` fixed. Bounded by MIN/MAX_ZOOM.
    pub fn zoom_at(&mut self, screen: Point, factor: f64) {
        if !(screen.0.is_finite() && screen.1.is_finite() && factor.is_finite() && factor > 0.0) {
            return;
        }
        let anchor = self.screen_to_world(screen);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let s = self.scale();
        self.center = (
            anchor.0 - (screen.0 - self.width / 2.0) / s,
            anchor.1 - (screen.1 - self.height / 2.0) / s,
        );
        self.clamp_center();
    }

    /// Centre = `origin` moved opposite to the pointer delta (in screen px).
    pub fn pan_from(&mut self, origin: Point, dx: f64, dy: f64) {
        if !(dx.is_finite() && dy.is_finite()) {
            return;
        }
        let s = self.scale();
        self.center = (origin.0 - dx / s, origin.1 - dy / s);
        self.clamp_center();
    }

    pub fn fit_bounds(&mut self, b: &Bounds) {
        if !b.is_valid() {
            return;
        }
        let padded = b.expanded((b.width().max(b.height()) * 0.15).max(2.0));
        let zoom_x = self.width / (padded.width().max(1e-6) * self.base_scale());
        let zoom_y = self.height / (padded.height().max(1e-6) * self.base_scale());
        self.zoom = zoom_x.min(zoom_y).clamp(MIN_ZOOM, MAX_ZOOM);
        self.center = padded.center();
        self.clamp_center();
    }

    pub fn reset(&mut self) {
        self.zoom = MIN_ZOOM;
        self.center = (WORLD_WIDTH / 2.0, WORLD_HEIGHT / 2.0);
        self.clamp_center();
    }

    /// Keep the world centre inside the world so the map can never be lost off-screen.
    fn clamp_center(&mut self) {
        let s = self.scale();
        let half_w = self.width / 2.0 / s;
        let half_h = self.height / 2.0 / s;
        // When the whole world fits on an axis it stays centred; otherwise the viewport edge
        // may not leave the world edge.
        self.center.0 = if half_w * 2.0 >= WORLD_WIDTH {
            WORLD_WIDTH / 2.0
        } else {
            self.center.0.clamp(half_w, WORLD_WIDTH - half_w)
        };
        self.center.1 = if half_h * 2.0 >= WORLD_HEIGHT {
            WORLD_HEIGHT / 2.0
        } else {
            self.center.1.clamp(half_h, WORLD_HEIGHT - half_h)
        };
    }
}
