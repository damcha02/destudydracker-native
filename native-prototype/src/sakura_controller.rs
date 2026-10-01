//! The Sakura palette's animation clock (Stage 19).
//!
//! ```text
//!  visibility rule (pure, tested)          one slint::Timer (only while it should run)
//!  SakuraWant { enabled, surface,     -->  tick: t = now - origin
//!               window_visible }            poses = core::appearance::sakura::petal_pose(t) x 22
//!                                           model.set_row_data(i, pose)   (fixed 22 rows, no alloc)
//!                                           texture_frame = t / 130 ms % 20
//! ```
//!
//! * **One clock.** There is exactly one `slint::Timer` for every petal layer (the window's and
//!   Wabi-Sabi's quiet-mode overlay both draw the same model), never a timer/thread per petal.
//! * **Bounded state.** 22 rows, created once; ticks overwrite them in place.
//! * **Nothing invisible is drawn.** The timer runs only while the palette is Sakura on a
//!   production surface of a shown, non-minimized window. Minimizing, hiding to the tray, opening a
//!   debug lab, or switching palette/style stops it; when it starts again the poses are simply
//!   evaluated at the current time (the effect is periodic and stateless), so a long hidden period
//!   costs nothing and no intermediate frame is ever rendered.
//! * **Time origin** is when the effect was switched on (like production, where the CSS animations
//!   start when `SakuraScatter` mounts) and is *not* reset by hide/show, so the petals continue
//!   where wall-clock time says they are.
//! * **Reduced motion** (Windows "Show animations" off = Chromium's `prefers-reduced-motion`): the
//!   petals are static at 20 % height like production's media query, while the texture GIF keeps
//!   animating as Chromium keeps animating GIFs - the clock then only ticks at the GIF's own 130 ms.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use slint::{Model, ModelRc, VecModel};
use study_tracker_core::appearance::sakura::{
    petal_pose, production_petals, reduced_motion_pose, texture_frame, PetalParams, PetalPose,
    PETAL_COUNT, TEXTURE_FRAME_MS,
};

use crate::PetalData;

/// Default petal cadence. Chosen by measurement (docs/stage19-garden-wabisabi.md, "Frame rate"):
/// production's CSS animations run at the display refresh rate (164 Hz on this machine), but its
/// faint 16-36 px petals move at most ~3.2 px and ~1.6 deg between 24 Hz frames (film cadence),
/// which looks the same here at well under half the cost of 60 Hz (12.7 % vs 20.8 % of one core,
/// 30 % at 165 Hz). Overridable with `STUDY_NATIVE_SAKURA_FPS` for measurements.
pub const DEFAULT_FPS: u32 = 24;

/// Inputs of the run/stop rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SakuraWant {
    /// The resolved appearance shows the Sakura effect (palette Sakura on Field Notebook/Wabi-Sabi).
    pub enabled: bool,
    /// A production surface is on screen (not a native debug lab).
    pub surface: bool,
    /// The window is shown, not minimized, not hidden to the tray.
    pub window_visible: bool,
}

impl SakuraWant {
    pub fn should_run(self) -> bool {
        self.enabled && self.surface && self.window_visible
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SakuraStats {
    /// Ticks that wrote new poses (each one is at most one rendered frame).
    pub frames: u64,
    pub starts: u64,
    pub stops: u64,
    /// Rows in the petal model (must stay at 22).
    pub live_petals: usize,
}

/// Opacity step of the pre-multiplied petal variants: production's petal opacity is 0..0.42, so 22
/// levels; the largest error is 0.01 of alpha (2.5/255), below what the 16-36 px petals can show.
pub const OPACITY_STEP: f64 = 0.02;
pub const OPACITY_LEVELS: usize = 22;

pub fn opacity_level(opacity: f64) -> usize {
    ((opacity / OPACITY_STEP).round().max(0.0) as usize).min(OPACITY_LEVELS - 1)
}

/// Each of the two blossom images at every opacity level, built once from the decoded assets. A
/// petal then switches between cached images instead of being drawn through an `opacity` layer
/// (measured: 22 opacity layers cost ~7 % of a core at 30 Hz and ~5 MB).
#[derive(Clone, Default)]
pub struct PetalImages {
    variants: Vec<Vec<slint::Image>>,
}

impl PetalImages {
    pub fn from_sources(sources: [slint::Image; 2]) -> Self {
        let variants = sources
            .iter()
            .map(|image| match image.to_rgba8() {
                Some(buffer) => (0..OPACITY_LEVELS)
                    .map(|level| {
                        let alpha = level as f64 * OPACITY_STEP;
                        let mut out = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(
                            buffer.width(),
                            buffer.height(),
                        );
                        for (dst, src) in out.make_mut_slice().iter_mut().zip(buffer.as_slice()) {
                            let a = (f64::from(src.a) * alpha).round();
                            let k = a / 255.0;
                            *dst = slint::Rgba8Pixel {
                                r: (f64::from(src.r) * k).round() as u8,
                                g: (f64::from(src.g) * k).round() as u8,
                                b: (f64::from(src.b) * k).round() as u8,
                                a: a as u8,
                            };
                        }
                        slint::Image::from_rgba8_premultiplied(out)
                    })
                    .collect(),
                None => vec![image.clone(); OPACITY_LEVELS],
            })
            .collect();
        Self { variants }
    }

    fn get(&self, image: u8, opacity: f64) -> slint::Image {
        self.variants
            .get(usize::from(image))
            .and_then(|levels| levels.get(opacity_level(opacity)))
            .cloned()
            .unwrap_or_default()
    }
}

pub fn pose_to_data(pose: &PetalPose, images: &PetalImages) -> PetalData {
    PetalData {
        x_frac: pose.left_fraction as f32,
        dx: pose.dx as f32,
        y: pose.center_y as f32,
        rot: pose.rotation_deg as f32,
        size: pose.size as f32,
        img: images.get(pose.image, pose.opacity),
    }
}

/// The pure part: parameters, origin, poses. Kept separate from the Slint timer so it is testable.
pub struct SakuraState {
    params: [PetalParams; PETAL_COUNT],
    model: Rc<VecModel<PetalData>>,
    origin: Option<Instant>,
    want: SakuraWant,
    reduced_motion: bool,
    stats: SakuraStats,
    texture: i32,
    images: PetalImages,
    /// The opacity level and image each row shows, to skip rewriting unchanged rows.
    shown: Vec<(usize, u8)>,
}

impl SakuraState {
    pub fn new() -> Self {
        let params = production_petals();
        let images = PetalImages::default();
        let rows = params
            .iter()
            .map(|p| pose_to_data(&petal_pose(p, 0.0, 980.0), &images))
            .collect::<Vec<_>>();
        Self {
            params,
            model: Rc::new(VecModel::from(rows)),
            origin: None,
            want: SakuraWant::default(),
            reduced_motion: false,
            stats: SakuraStats {
                live_petals: PETAL_COUNT,
                ..SakuraStats::default()
            },
            texture: 0,
            images,
            shown: Vec::new(),
        }
    }

    /// Provides the decoded blossom images (from the window's `SakuraAssets`), once.
    pub fn set_images(&mut self, images: PetalImages) {
        self.images = images;
        self.shown.clear();
    }

    pub fn model(&self) -> ModelRc<PetalData> {
        ModelRc::from(self.model.clone())
    }

    pub fn stats(&self) -> SakuraStats {
        SakuraStats {
            live_petals: self.model.row_count(),
            ..self.stats
        }
    }

    pub fn texture_frame(&self) -> i32 {
        self.texture
    }

    /// Records the new inputs; the effect's time origin is set when it is first enabled and cleared
    /// when the palette/style no longer shows it (a later re-enable starts over, like a remount).
    pub fn set_want(&mut self, want: SakuraWant, now: Instant) {
        if want.enabled && self.origin.is_none() {
            self.origin = Some(now);
        }
        if !want.enabled {
            self.origin = None;
        }
        self.want = want;
    }

    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.reduced_motion = reduced;
    }

    /// Writes the poses for `now` into the existing rows. Returns the elapsed animation time.
    pub fn update(&mut self, now: Instant, viewport_height: f32) -> f64 {
        let t = self.origin.map_or(0.0, |o| {
            now.saturating_duration_since(o).as_secs_f64() * 1000.0
        });
        let h = f64::from(viewport_height.max(1.0));
        for (i, p) in self.params.iter().enumerate() {
            let pose = if self.reduced_motion {
                reduced_motion_pose(p, h)
            } else {
                petal_pose(p, t, h)
            };
            // Unchanged rows are not rewritten: a row write always notifies the repeater.
            let key = (opacity_level(pose.opacity), pose.image);
            let old = self.model.row_data(i);
            let same_place = old.as_ref().is_some_and(|o| {
                o.x_frac == pose.left_fraction as f32
                    && o.dx == pose.dx as f32
                    && o.y == pose.center_y as f32
                    && o.rot == pose.rotation_deg as f32
            });
            if same_place && self.shown.get(i) == Some(&key) {
                continue;
            }
            self.model
                .set_row_data(i, pose_to_data(&pose, &self.images));
            if self.shown.len() <= i {
                self.shown.resize(i + 1, (usize::MAX, 0));
            }
            self.shown[i] = key;
        }
        self.texture = texture_frame(t) as i32;
        self.stats.frames += 1;
        t
    }
}

impl Default for SakuraState {
    fn default() -> Self {
        Self::new()
    }
}

/// The clock: owns the one Slint timer and drives a [`SakuraState`].
pub struct SakuraController {
    state: SakuraState,
    timer: slint::Timer,
    interval: Duration,
    /// The interval the timer was started with (`slint::Timer::interval` reports whole
    /// milliseconds, so it cannot be compared with a 41.666 ms `Duration`).
    running_interval: Option<Duration>,
}

impl SakuraController {
    pub fn new() -> Rc<RefCell<Self>> {
        let fps = std::env::var("STUDY_NATIVE_SAKURA_FPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|f| (1..=1000).contains(f))
            .unwrap_or(DEFAULT_FPS);
        Rc::new(RefCell::new(Self {
            state: SakuraState::new(),
            timer: slint::Timer::default(),
            interval: Duration::from_micros(1_000_000 / u64::from(fps)),
            running_interval: None,
        }))
    }

    pub fn state(&self) -> &SakuraState {
        &self.state
    }

    pub fn is_running(&self) -> bool {
        self.timer.running()
    }

    /// Applies the run/stop rule. `push` writes one frame's poses into the window (and is what the
    /// timer calls on every tick); it is invoked once immediately on start so the first visible
    /// frame is already correct.
    pub fn sync(
        this: &Rc<RefCell<Self>>,
        want: SakuraWant,
        reduced_motion: bool,
        push: impl Fn(&mut SakuraState) + 'static,
    ) {
        let now = Instant::now();
        let mut me = this.borrow_mut();
        me.state.set_want(want, now);
        me.state.set_reduced_motion(reduced_motion);
        let run = want.should_run();
        if !run {
            if me.timer.running() {
                me.timer.stop();
                me.running_interval = None;
                me.state.stats.stops += 1;
                log::info!("sakura: clock stopped ({want:?})");
            }
            return;
        }
        let interval = if reduced_motion {
            Duration::from_millis(u64::from(TEXTURE_FRAME_MS))
        } else {
            me.interval
        };
        if me.timer.running() && me.running_interval == Some(interval) {
            return;
        }
        if me.timer.running() {
            // only the cadence changed (reduced motion toggled): same clock, new interval
            me.state.stats.stops += 1;
        }
        me.running_interval = Some(interval);
        me.state.stats.starts += 1;
        log::info!(
            "sakura: clock started at {} ms/frame (reduced motion: {reduced_motion})",
            interval.as_millis()
        );
        push(&mut me.state);
        let weak: Weak<RefCell<Self>> = Rc::downgrade(this);
        let push = Rc::new(push);
        me.timer
            .start(slint::TimerMode::Repeated, interval, move || {
                let Some(this) = weak.upgrade() else { return };
                let mut me = this.borrow_mut();
                push(&mut me.state);
            });
    }

    /// Screenshot mode (`STUDY_NATIVE_SAKURA_TIME`): direct access to evaluate one frozen frame.
    pub fn state_mut_for_freeze(&mut self) -> &mut SakuraState {
        &mut self.state
    }

    pub fn stop(&mut self) {
        if self.timer.running() {
            self.timer.stop();
            self.running_interval = None;
            self.state.stats.stops += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_runs_only_when_enabled_on_a_production_surface_of_a_visible_window() {
        let all = SakuraWant {
            enabled: true,
            surface: true,
            window_visible: true,
        };
        assert!(all.should_run());
        assert!(
            !SakuraWant {
                enabled: false,
                ..all
            }
            .should_run(),
            "another palette/style"
        );
        assert!(
            !SakuraWant {
                surface: false,
                ..all
            }
            .should_run(),
            "debug lab"
        );
        assert!(
            !SakuraWant {
                window_visible: false,
                ..all
            }
            .should_run(),
            "minimized / tray"
        );
    }

    #[test]
    fn the_population_is_fixed_at_twenty_two_and_updated_in_place() {
        let mut s = SakuraState::new();
        let model = s.model();
        let now = Instant::now();
        s.set_want(
            SakuraWant {
                enabled: true,
                surface: true,
                window_visible: true,
            },
            now,
        );
        for i in 0..1000 {
            s.update(now + Duration::from_millis(33 * i), 980.0);
        }
        assert_eq!(model.row_count(), PETAL_COUNT);
        assert_eq!(s.stats().live_petals, PETAL_COUNT);
        assert_eq!(s.stats().frames, 1000);
    }

    #[test]
    fn hiding_does_not_reset_the_time_origin_but_disabling_does() {
        let mut s = SakuraState::new();
        let t0 = Instant::now();
        let on = SakuraWant {
            enabled: true,
            surface: true,
            window_visible: true,
        };
        s.set_want(on, t0);
        s.set_want(
            SakuraWant {
                window_visible: false,
                ..on
            },
            t0 + Duration::from_secs(5),
        );
        s.set_want(on, t0 + Duration::from_secs(60));
        let elapsed = s.update(t0 + Duration::from_secs(60), 980.0);
        assert!(
            (elapsed - 60_000.0).abs() < 1.0,
            "resumes at wall-clock position: {elapsed}"
        );
        s.set_want(
            SakuraWant {
                enabled: false,
                ..on
            },
            t0 + Duration::from_secs(61),
        );
        s.set_want(on, t0 + Duration::from_secs(70));
        let restarted = s.update(t0 + Duration::from_secs(70), 980.0);
        assert!(restarted < 1.0, "a palette switch starts the effect over");
    }

    #[test]
    fn a_frame_after_a_long_hidden_period_is_one_evaluation_not_a_replay() {
        let mut s = SakuraState::new();
        let t0 = Instant::now();
        s.set_want(
            SakuraWant {
                enabled: true,
                surface: true,
                window_visible: true,
            },
            t0,
        );
        s.update(t0, 980.0);
        let frames_before = s.stats().frames;
        s.update(t0 + Duration::from_secs(8 * 3600), 980.0);
        assert_eq!(s.stats().frames, frames_before + 1);
    }

    #[test]
    fn reduced_motion_poses_are_static_and_production_shaped() {
        let mut s = SakuraState::new();
        let t0 = Instant::now();
        s.set_want(
            SakuraWant {
                enabled: true,
                surface: true,
                window_visible: true,
            },
            t0,
        );
        s.set_reduced_motion(true);
        s.update(t0 + Duration::from_secs(3), 1000.0);
        let a = s.model().row_data(0).unwrap();
        s.update(t0 + Duration::from_secs(9), 1000.0);
        let b = s.model().row_data(0).unwrap();
        // (Slint images compare by identity, so compare the geometry.)
        let geometry = |d: &PetalData| (d.x_frac, d.dx, d.y, d.rot, d.size);
        assert_eq!(geometry(&a), geometry(&b), "no motion");
        assert_eq!(a.rot, 0.0);
    }
}
