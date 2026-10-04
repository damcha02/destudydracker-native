//! Daily Skribbl's flood fill (Stage 22a): an exact port of production's `floodFill`
//! (`components/SkribblRoom.tsx`) over a 900x600 RGBA buffer: span fill with an explicit stack,
//! per-channel tolerance 40 against the clicked pixel's colour, alpha included.
//!
//! Iterative only (no recursion), and bounded: every pushed seed is a pixel whose colour still
//! matches the target, every filled pixel stops matching (the fill colour is checked against the
//! target first), so the work is O(pixels); an explicit cap guards the pathological case anyway.

/// `matchRgba(r, g, b, a, target, tolerance)`.
fn matches(px: &[u8], target: [u8; 4], tolerance: u8) -> bool {
    px.iter()
        .zip(target)
        .all(|(&c, t)| c.abs_diff(t) <= tolerance)
}

/// What a fill did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillOutcome {
    /// Pixels written (0 when the click colour already matched the fill colour).
    pub filled: usize,
    /// True if the defensive work cap stopped the fill early (never for a normal canvas).
    pub capped: bool,
}

/// Fills the region around `(x, y)` in a `width`x`height` RGBA buffer. Out-of-range points and a
/// wrongly sized buffer do nothing.
pub fn flood_fill(
    data: &mut [u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    fill: [u8; 4],
    tolerance: u8,
) -> FillOutcome {
    let none = FillOutcome {
        filled: 0,
        capped: false,
    };
    if x >= width || y >= height || data.len() != width * height * 4 {
        return none;
    }
    let at = |px: usize, py: usize| (py * width + px) * 4;
    let start = at(x, y);
    let target = [
        data[start],
        data[start + 1],
        data[start + 2],
        data[start + 3],
    ];
    if matches(&data[start..start + 4], fill, tolerance) {
        return none;
    }
    // Every filled pixel stops matching, so each pixel is filled at most once and each row span
    // pushes at most one seed per sub-span: 4x the pixel count is far above the real maximum.
    let budget = width * height * 4;
    let mut work = 0usize;
    let mut filled = 0usize;
    let mut stack: Vec<(usize, usize)> = vec![(x, y)];
    while let Some((px, py)) = stack.pop() {
        work += 1;
        if work > budget {
            return FillOutcome {
                filled,
                capped: true,
            };
        }
        if !matches(&data[at(px, py)..at(px, py) + 4], target, tolerance) {
            continue;
        }
        let mut left = px;
        while left > 0
            && matches(
                &data[at(left - 1, py)..at(left - 1, py) + 4],
                target,
                tolerance,
            )
        {
            left -= 1;
        }
        let mut right = px;
        while right < width - 1
            && matches(
                &data[at(right + 1, py)..at(right + 1, py) + 4],
                target,
                tolerance,
            )
        {
            right += 1;
        }
        for cx in left..=right {
            data[at(cx, py)..at(cx, py) + 4].copy_from_slice(&fill);
        }
        filled += right - left + 1;
        for row_y in [py.wrapping_sub(1), py + 1] {
            if row_y >= height {
                continue;
            }
            let mut in_span = false;
            for cx in left..=right {
                let m = matches(&data[at(cx, row_y)..at(cx, row_y) + 4], target, tolerance);
                if m && !in_span {
                    stack.push((cx, row_y));
                    in_span = true;
                } else if !m && in_span {
                    in_span = false;
                }
            }
        }
    }
    FillOutcome {
        filled,
        capped: false,
    }
}
