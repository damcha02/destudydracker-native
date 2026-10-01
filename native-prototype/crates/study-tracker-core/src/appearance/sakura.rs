//! The Sakura palette's falling petals, as a pure function of time (Stage 19).
//!
//! Production (`components/SakuraScatter.tsx` + `index.css`): 22 `<div class="sakura-petal">`
//! elements whose parameters come from a fixed-seed `mulberry32(0x534b5552)`, animated purely by
//! CSS: the outer element runs `sakura-fall` (linear, infinite, 14-28 s, negative delay up to 28 s,
//! falls `100vh + 80px` from `top: -60px` while drifting and spinning, fading in over the first 8 %
//! and out over the last 8 %), and the image inside runs `sakura-sway` (ease-in-out, infinite,
//! alternate, 3-6 s, `translateX(-14px..14px) rotate(-18deg..18deg)`).
//!
//! Because every input is deterministic, the whole effect is a function of one number - the
//! animation time - and a viewport size. Native evaluates that function from a single clock instead
//! of 44 independent CSS animations, so no per-petal timers, threads or allocations exist, the
//! population is fixed at 22, and nothing needs to be "advanced" while invisible: a frame after a
//! long hidden period is simply evaluated at the new time.

/// `mulberry32`, bit-exact with production's JavaScript (`SakuraScatter.tsx`). The JS seed is a
/// double that is never wrapped, but every operation that reads it converts to 32 bits first, so
/// wrapping u32 arithmetic is equivalent while the double stays exact (it does for millions of
/// draws).
#[derive(Debug, Clone)]
pub struct Mulberry32(u32);

impl Mulberry32 {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }

    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

pub const PETAL_COUNT: usize = 22;
pub const PETAL_SEED: u32 = 0x534b_5552;
/// `.sakura-petal { top: -60px }` and `translate3d(.., calc(100vh + 80px), 0)`.
const START_TOP: f64 = -60.0;
const FALL_EXTRA: f64 = 80.0;
const SWAY_X: f64 = 14.0;
const SWAY_DEG: f64 = 18.0;

/// One petal's fixed parameters (`ScatterDot`), in production's draw order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetalParams {
    /// 0 or 1: which of production's two blossom images (`BLOSSOMS[i % 2]`).
    pub image: u8,
    /// `left: <left>%` of the container width.
    pub left_percent: f64,
    /// Square box edge, CSS px (16..36).
    pub size: f64,
    /// `--petal-opacity` (0.22..0.42).
    pub opacity: f64,
    /// Horizontal drift over one fall, px (-80..80).
    pub drift: f64,
    /// Rotation over one fall, degrees (±180..540).
    pub spin: f64,
    /// `sakura-fall` duration, seconds (14..28).
    pub fall_seconds: f64,
    /// `animation-delay` (negative), seconds (-28..0).
    pub delay_seconds: f64,
    /// `sakura-sway` duration, seconds (3..6).
    pub sway_seconds: f64,
}

/// The 22 petals production renders, generated exactly like `SakuraScatter`'s `useMemo`.
pub fn production_petals() -> [PetalParams; PETAL_COUNT] {
    let mut rnd = Mulberry32::new(PETAL_SEED);
    std::array::from_fn(|i| {
        // Field order = production's object-literal evaluation order.
        let left_percent = rnd.next_f64() * 100.0;
        let size = 16.0 + rnd.next_f64() * 20.0;
        let opacity = 0.22 + rnd.next_f64() * 0.2;
        let drift = (rnd.next_f64() - 0.5) * 160.0;
        let sign = if rnd.next_f64() > 0.5 { 1.0 } else { -1.0 };
        let spin = sign * (180.0 + rnd.next_f64() * 360.0);
        let fall_seconds = 14.0 + rnd.next_f64() * 14.0;
        let delay_seconds = -rnd.next_f64() * 28.0;
        let sway_seconds = 3.0 + rnd.next_f64() * 3.0;
        PetalParams {
            image: (i % 2) as u8,
            left_percent,
            size,
            opacity,
            drift,
            spin,
            fall_seconds,
            delay_seconds,
            sway_seconds,
        }
    })
}

/// Where one petal is at one instant. `center_x` is split into a container-relative part and a
/// pixel offset so the same pose serves containers of different widths (the window and Wabi-Sabi's
/// quiet-mode overlay each host a petal layer, like production).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetalPose {
    pub left_fraction: f64,
    /// Added to `left_fraction * container_width` to get the image centre, px.
    pub dx: f64,
    /// Image centre, px from the container top.
    pub center_y: f64,
    pub rotation_deg: f64,
    pub opacity: f64,
    pub size: f64,
    pub image: u8,
}

/// CSS `ease-in-out` = `cubic-bezier(0.42, 0, 0.58, 1)`, solved for x like browsers do.
pub fn ease_in_out(x: f64) -> f64 {
    let (x1, y1, x2, y2) = (0.42, 0.0, 0.58, 1.0);
    let bez = |t: f64, a: f64, b: f64| {
        let mt = 1.0 - t;
        3.0 * mt * mt * t * a + 3.0 * mt * t * t * b + t * t * t
    };
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    // Bisection is plenty: monotone, 40 halvings < 1e-12.
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..40 {
        let mid = 0.5 * (lo + hi);
        if bez(mid, x1, x2) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bez(0.5 * (lo + hi), y1, y2)
}

/// `sakura-fall` opacity keyframes: 0 -> op (8 %) -> op (92 %) -> 0, linear between.
fn fall_opacity(progress: f64, peak: f64) -> f64 {
    if progress < 0.08 {
        peak * progress / 0.08
    } else if progress <= 0.92 {
        peak
    } else {
        peak * (1.0 - progress) / 0.08
    }
}

/// The pose at `time_ms` of animation time (all petals' animations start together at 0, like a
/// freshly mounted `SakuraScatter`), for a viewport `viewport_height` CSS px tall.
pub fn petal_pose(p: &PetalParams, time_ms: f64, viewport_height: f64) -> PetalPose {
    let fall_ms = p.fall_seconds * 1000.0;
    // Negative delay: the effect is already `-delay` into its first iteration at time 0.
    let local = (time_ms - p.delay_seconds * 1000.0).rem_euclid(fall_ms);
    let progress = local / fall_ms;
    let outer_rot = p.spin * progress;
    let ty = (viewport_height + FALL_EXTRA) * progress;
    let tx = p.drift * progress;

    let sway_ms = p.sway_seconds * 1000.0;
    let iteration = (time_ms / sway_ms).floor();
    let within = (time_ms - iteration * sway_ms) / sway_ms;
    // `alternate`: odd iterations run backwards; the timing function applies to the directed
    // progress (ease-in-out is symmetric, so this is also what the spec's order gives).
    let directed = if (iteration as i64).rem_euclid(2) == 0 {
        within
    } else {
        1.0 - within
    };
    let eased = ease_in_out(directed);
    let sway_x = -SWAY_X + 2.0 * SWAY_X * eased;
    let sway_rot = -SWAY_DEG + 2.0 * SWAY_DEG * eased;

    // Outer: translate(tx, ty) rotate(outer) about the box centre; inner: translateX(sway_x)
    // rotate(sway) about the same centre. The inner translation happens in the outer-rotated frame.
    let rad = outer_rot.to_radians();
    let half = p.size / 2.0;
    PetalPose {
        left_fraction: p.left_percent / 100.0,
        dx: half + tx + sway_x * rad.cos(),
        center_y: START_TOP + half + ty + sway_x * rad.sin(),
        rotation_deg: outer_rot + sway_rot,
        opacity: fall_opacity(progress, p.opacity),
        size: p.size,
        image: p.image,
    }
}

/// `@media (prefers-reduced-motion: reduce)`: no animation, `top: 20%`, the petal's own opacity,
/// no transform.
pub fn reduced_motion_pose(p: &PetalParams, container_height: f64) -> PetalPose {
    let half = p.size / 2.0;
    PetalPose {
        left_fraction: p.left_percent / 100.0,
        dx: half,
        center_y: 0.2 * container_height + half,
        rotation_deg: 0.0,
        opacity: p.opacity,
        size: p.size,
        image: p.image,
    }
}

/// Production's animated petal texture (`sakura-leaves-...gif`): 20 frames, 130 ms each, looping.
pub const TEXTURE_FRAMES: u32 = 20;
pub const TEXTURE_FRAME_MS: u32 = 130;

pub fn texture_frame(time_ms: f64) -> u32 {
    if time_ms <= 0.0 {
        return 0;
    }
    ((time_ms / f64::from(TEXTURE_FRAME_MS)).floor() as u64 % u64::from(TEXTURE_FRAMES)) as u32
}
