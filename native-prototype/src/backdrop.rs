//! Stage 22b: the blurred backdrop under lightboxes and Social dialogs (production's
//! `backdrop-filter: blur(10px)`), which Slint/FemtoVG cannot draw. When an overlay that wants it
//! opens, one frame is presented with the overlays hidden and read back, a quarter-size copy is
//! blurred on the CPU (three box passes approximate the Gaussian) and shown under the overlay's dim.
//! It costs one extra frame and a few milliseconds per opening; nothing runs while it stays open.
//! Without a snapshot (unsupported renderer, error) the overlay simply dims, as in Stage 22a.

use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};

use crate::{Backdrop, MainWindow};

/// CSS `blur(10px)`: a Gaussian with σ = 10 logical px.
const SIGMA_LOGICAL: f32 = 10.0;
/// The blur is computed at a quarter of the window's physical size.
const SCALE: u32 = 4;

pub fn bind(window: &MainWindow) {
    let weak = window.as_weak();
    window.on_backdrop_changed(move |wanted| {
        let Some(w) = weak.upgrade() else { return };
        let b = w.global::<Backdrop>();
        if !wanted {
            b.set_capturing(false);
            b.set_ready(false);
            b.set_blur(Image::default());
            return;
        }
        // The snapshot reads back the last presented frame, so first present one with the
        // overlays hidden (and whatever closed in the same click, e.g. the menu, gone): it shows
        // exactly what was on screen before the overlay opened, then the overlay appears blurred.
        b.set_capturing(true);
        w.window().request_redraw();
        let weak = w.as_weak();
        slint::Timer::single_shot(std::time::Duration::from_millis(40), move || {
            let Some(w) = weak.upgrade() else { return };
            let b = w.global::<Backdrop>();
            if !b.get_capturing() {
                return; // closed again before the frame
            }
            let shot = w.window().take_snapshot();
            b.set_capturing(false);
            match shot {
                Ok(shot) => {
                    let image = blurred(&shot, w.window().scale_factor());
                    b.set_blur(image);
                    b.set_ready(true);
                }
                Err(error) => {
                    log::warn!("backdrop blur unavailable: {error}");
                    b.set_ready(false);
                }
            }
        });
    });
}

fn blurred(shot: &SharedPixelBuffer<Rgba8Pixel>, scale_factor: f32) -> Image {
    let (w, h) = (shot.width(), shot.height());
    let (sw, sh) = ((w / SCALE).max(1), (h / SCALE).max(1));
    let small = downscale(shot.as_slice(), w, h, sw, sh);
    let sigma = SIGMA_LOGICAL * scale_factor.max(0.5) / SCALE as f32;
    let blurred = gaussian_ish(small, sw as usize, sh as usize, sigma);
    let mut out = SharedPixelBuffer::<Rgba8Pixel>::new(sw, sh);
    out.make_mut_slice().copy_from_slice(&blurred);
    Image::from_rgba8(out)
}

/// Averages each `SCALE`×`SCALE` block (the last row/column of blocks may be partial).
fn downscale(px: &[Rgba8Pixel], w: u32, h: u32, sw: u32, sh: u32) -> Vec<Rgba8Pixel> {
    let mut out = Vec::with_capacity((sw * sh) as usize);
    for by in 0..sh {
        for bx in 0..sw {
            let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for y in by * SCALE..((by + 1) * SCALE).min(h) {
                for x in bx * SCALE..((bx + 1) * SCALE).min(w) {
                    let p = px[(y * w + x) as usize];
                    r += u32::from(p.r);
                    g += u32::from(p.g);
                    b += u32::from(p.b);
                    a += u32::from(p.a);
                    n += 1;
                }
            }
            let n = n.max(1);
            out.push(Rgba8Pixel {
                r: (r / n) as u8,
                g: (g / n) as u8,
                b: (b / n) as u8,
                a: (a / n) as u8,
            });
        }
    }
    out
}

/// Three box blurs (horizontal and vertical each) whose combined variance matches `sigma`.
fn gaussian_ish(mut px: Vec<Rgba8Pixel>, w: usize, h: usize, sigma: f32) -> Vec<Rgba8Pixel> {
    // three passes of a box of width 2r+1 have variance 3 * ((2r+1)^2 - 1) / 12
    let r = (((4.0 * sigma * sigma + 1.0).sqrt() - 1.0) / 2.0)
        .round()
        .max(1.0) as usize;
    let mut tmp = px.clone();
    for _ in 0..3 {
        box_pass(&px, &mut tmp, w, h, r, true);
        box_pass(&tmp, &mut px, w, h, r, false);
    }
    px
}

/// One box pass along rows (`horizontal`) or columns, edges clamped.
fn box_pass(
    src: &[Rgba8Pixel],
    dst: &mut [Rgba8Pixel],
    w: usize,
    h: usize,
    r: usize,
    horizontal: bool,
) {
    let (lines, len) = if horizontal { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| {
        if horizontal {
            line * w + i
        } else {
            i * w + line
        }
    };
    let span = (2 * r + 1) as u32;
    for line in 0..lines {
        let get = |i: isize| src[at(line, i.clamp(0, len as isize - 1) as usize)];
        let mut acc = [0u32; 4];
        for i in -(r as isize)..=(r as isize) {
            let p = get(i);
            acc[0] += u32::from(p.r);
            acc[1] += u32::from(p.g);
            acc[2] += u32::from(p.b);
            acc[3] += u32::from(p.a);
        }
        for i in 0..len {
            dst[at(line, i)] = Rgba8Pixel {
                r: (acc[0] / span) as u8,
                g: (acc[1] / span) as u8,
                b: (acc[2] / span) as u8,
                a: (acc[3] / span) as u8,
            };
            let out = get(i as isize - r as isize);
            let inn = get(i as isize + r as isize + 1);
            acc[0] = acc[0] + u32::from(inn.r) - u32::from(out.r);
            acc[1] = acc[1] + u32::from(inn.g) - u32::from(out.g);
            acc[2] = acc[2] + u32::from(inn.b) - u32::from(out.b);
            acc[3] = acc[3] + u32::from(inn.a) - u32::from(out.a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(v: u8) -> Rgba8Pixel {
        Rgba8Pixel {
            r: v,
            g: v,
            b: v,
            a: 255,
        }
    }

    #[test]
    fn a_flat_image_stays_flat_and_a_step_softens() {
        let flat = vec![px(90); 40 * 30];
        let out = gaussian_ish(flat, 40, 30, 2.5);
        assert!(out.iter().all(|p| p.r == 90 && p.a == 255));
        // a hard black/white edge becomes a ramp
        let step: Vec<Rgba8Pixel> = (0..40 * 10)
            .map(|i| if i % 40 < 20 { px(0) } else { px(255) })
            .collect();
        let out = gaussian_ish(step, 40, 10, 2.5);
        let row: Vec<u8> = out[..40].iter().map(|p| p.r).collect();
        assert!(row[19] > 40 && row[19] < 215, "{row:?}");
        assert!(row.windows(2).all(|w| w[0] <= w[1]));
        assert!(row[0] < 10 && row[39] > 245);
    }

    #[test]
    fn downscale_averages_blocks() {
        let src: Vec<Rgba8Pixel> = (0..8 * 4)
            .map(|i| if i % 8 < 4 { px(0) } else { px(200) })
            .collect();
        let out = downscale(&src, 8, 4, 2, 1);
        assert_eq!(out[0].r, 0);
        assert_eq!(out[1].r, 200);
    }
}
