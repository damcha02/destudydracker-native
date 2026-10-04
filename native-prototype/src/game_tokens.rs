//! The Break Room game modals' colours (Stage 20).
//!
//! Production styles every game modal from the app-wide CSS variables (`--surface`, `--surface-2`,
//! `--surface-inset`, `--border`, `--accent`, `--text`, `--muted`), so the modals follow the style,
//! theme and palette, and derives many colours with `color-mix(in oklch, ...)`. In the Field
//! Notebook style those variables are exactly the FN tokens; Wabi-Sabi sets its own (measured from
//! production, light and dark; Wabi + Sakura uses the light set). The OKLCH mixes are computed here
//! with CSS Color 4 semantics (premultiplied alpha, shorter-arc hue), never approximated in sRGB.

use slint::Color;

use crate::GameTokens;

const ACHROMATIC: f32 = 0.02;

fn to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn from_linear(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB -> OKLCh (L, C, h in degrees; `None` hue when achromatic).
fn oklch(c: Color) -> (f32, f32, Option<f32>) {
    let [r, g, b] = [c.red(), c.green(), c.blue()].map(|v| to_linear(f32::from(v) / 255.0));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let aa = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    let chroma = (aa * aa + bb * bb).sqrt();
    // Chromium treats a near-grey colour's hue as powerless ("none") when mixing: measured on
    // production, a surface of chroma 0.008-0.012 takes the other colour's hue outright, while a
    // chroma of 0.023 keeps its own.
    let hue = (chroma >= ACHROMATIC).then(|| bb.atan2(aa).to_degrees().rem_euclid(360.0));
    (ll, chroma, hue)
}

fn from_oklch(l: f32, c: f32, h: f32, alpha: f32) -> Color {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let r = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let bl = -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    let q = |v: f32| (from_linear(v).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color::from_argb_u8(
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        q(r),
        q(g),
        q(bl),
    )
}

/// `color-mix(in oklch, a <wa*100>%, b)`. `b = None` is `transparent`.
pub fn mix_oklch(a: Color, wa: f32, b: Option<Color>) -> Color {
    let (la, ca, ha) = oklch(a);
    let alpha_a = f32::from(a.alpha()) / 255.0;
    let Some(b) = b else {
        // transparent: premultiplied interpolation leaves a's colour with a's weight as alpha
        return from_oklch(la, ca, ha.unwrap_or(0.0), alpha_a * wa);
    };
    let (lb, cb, hb) = oklch(b);
    let alpha_b = f32::from(b.alpha()) / 255.0;
    let wb = 1.0 - wa;
    let alpha = alpha_a * wa + alpha_b * wb;
    let premul = |va: f32, vb: f32| (va * alpha_a * wa + vb * alpha_b * wb) / alpha.max(1e-6);
    let hue = match (ha, hb) {
        (Some(x), Some(y)) => {
            let mut d = y - x;
            if d > 180.0 {
                d -= 360.0;
            } else if d < -180.0 {
                d += 360.0;
            }
            (x + d * wb).rem_euclid(360.0)
        }
        (Some(x), None) | (None, Some(x)) => x,
        (None, None) => 0.0,
    };
    from_oklch(premul(la, lb), premul(ca, cb), hue, alpha)
}

fn hex(v: u32) -> Color {
    Color::from_rgb_u8((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// The seven CSS variables the modals read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Base {
    pub surface: Color,
    pub surface_2: Color,
    pub inset: Color,
    pub border: Color,
    pub accent: Color,
    pub text: Color,
    pub muted: Color,
    /// Stage 22a (Daily Skribbl): `--accent-strong`, `--accent-soft`, `--accent-line`, `--danger`.
    pub accent_strong: Color,
    pub accent_soft: Color,
    pub accent_line: Color,
    pub danger: Color,
}

fn hexa(v: u32, a: u8) -> Color {
    Color::from_argb_u8(a, (v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// Wabi-Sabi's own modal variables (production, measured).
pub fn wabi_base(dark: bool) -> Base {
    if dark {
        Base {
            surface: hex(0x211f19),
            surface_2: hex(0x262418),
            inset: hex(0x1c1b14),
            border: hex(0x4c4634),
            accent: hex(0x7fa077),
            text: hex(0xece7d8),
            muted: hex(0xa39d89),
            accent_strong: hex(0x97b78f),
            accent_soft: hexa(0x7fa077, 0x1f),
            accent_line: hexa(0x7fa077, 0x57),
            danger: hex(0xd97a5e),
        }
    } else {
        Base {
            surface: hex(0xf4f1e8),
            surface_2: hex(0xefebe0),
            inset: hex(0xf0ece1),
            border: hex(0xddd8c8),
            accent: hex(0x4f6b4a),
            text: hex(0x1c1d19),
            muted: hex(0x6f7268),
            accent_strong: hex(0x374d33),
            accent_soft: hexa(0x4f6b4a, 0x1a),
            accent_line: hexa(0x4f6b4a, 0x52),
            danger: hex(0xb0472e),
        }
    }
}

pub fn tokens(b: Base) -> GameTokens {
    let black = hex(0x000000);
    GameTokens {
        surface: b.surface,
        surface_2: b.surface_2,
        inset: b.inset,
        border: b.border,
        accent: b.accent,
        text: b.text,
        muted: b.muted,
        wordle_bg: mix_oklch(b.surface, 0.94, Some(b.accent)),
        geodle_bg: mix_oklch(b.surface, 0.94, Some(hex(0x00b894))),
        flaggle_bg: mix_oklch(b.surface, 0.94, Some(hex(0x8fb4ff))),
        travle_bg: mix_oklch(b.surface, 0.94, Some(hex(0xd5f5df))),
        accent_glow_20: mix_oklch(b.accent, 0.20, None),
        accent_glow_24: mix_oklch(b.accent, 0.24, None),
        tile_border: mix_oklch(b.border, 0.85, Some(b.text)),
        tile_filled_border: mix_oklch(b.text, 0.5, Some(b.border)),
        input_border: mix_oklch(b.border, 0.80, Some(b.text)),
        hover_14: mix_oklch(b.accent, 0.14, None),
        hover_16: mix_oklch(b.accent, 0.16, None),
        hard_active_bg: mix_oklch(hex(0xf08c00), 0.18, Some(b.surface)),
        flaggle_row_bg: mix_oklch(b.inset, 0.72, None),
        geodle_table_bg: mix_oklch(b.inset, 0.76, None),
        geodle_rule: mix_oklch(b.border, 0.70, None),
        tooltip_bg: mix_oklch(b.surface, 0.92, Some(black)),
        durak_rule: mix_oklch(b.border, 0.50, None),
        durak_selected_bg: mix_oklch(b.accent, 0.10, Some(hex(0xffffff))),
        durak_undefended_bg: mix_oklch(b.muted, 0.08, Some(hex(0xffffff))),
        travle_card_bg: mix_oklch(b.surface, 0.84, None),
        travle_route_bg: mix_oklch(b.accent, 0.10, None),
        travle_map_border: mix_oklch(b.border, 0.82, Some(hex(0x8b7b60))),
        travle_won_bg: mix_oklch(b.surface, 0.90, Some(hex(0x1f2a21))),
        travle_lost_bg: mix_oklch(b.surface, 0.90, Some(hex(0x2a1f24))),
        travle_stat_bg: mix_oklch(b.surface, 0.82, None),
        accent_strong: b.accent_strong,
        accent_soft: b.accent_soft,
        accent_line: b.accent_line,
        danger: b.danger,
        danger_12: mix_oklch(b.danger, 0.12, None),
        danger_45: mix_oklch(b.danger, 0.45, None),
        accent_8: mix_oklch(b.accent, 0.08, None),
        surface_85: mix_oklch(b.surface, 0.85, None),
        surface_92: mix_oklch(b.surface, 0.92, None),
        surface_96: mix_oklch(b.surface, 0.96, None),
        amber_18: mix_oklch(hex(0xf9a825), 0.18, None),
        green_18: mix_oklch(hex(0x43a047), 0.18, None),
        red_18: mix_oklch(hex(0xe53935), 0.18, None),
        black_72: mix_oklch(black, 0.72, None),
        white_18: mix_oklch(hex(0xffffff), 0.18, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(c: Color) -> (u8, u8, u8) {
        (c.red(), c.green(), c.blue())
    }

    #[test]
    fn oklch_mixes_match_productions_computed_colours() {
        // FN dark: color-mix(in oklch, #24221e 94%, #eee6d6) = oklch(0.293133 0.008853 84.49)
        let fn_dark = Base {
            surface: hex(0x24221e),
            surface_2: hex(0x2b2823),
            inset: hex(0x201e1a),
            border: hex(0x58503f),
            accent: hex(0xeee6d6),
            text: hex(0xeee6d6),
            muted: hex(0xaaa08d),
            accent_strong: hex(0xfff7e8),
            accent_soft: hexa(0xeee6d6, 0x14),
            accent_line: hexa(0xeee6d6, 0x52),
            danger: hex(0xc47a4d),
        };
        let t = tokens(fn_dark);
        // Skribbl's theme card: production computes oklch(0.926912 0.0230387 84.4921 / 0.08)
        assert_eq!(t.accent_8.alpha(), 20);
        assert_eq!(rgb(t.accent_8), rgb(hex(0xeee6d6)));
        let want = from_oklch(0.293_133, 0.008_853, 84.492, 1.0);
        let (a, b) = (rgb(t.wordle_bg), rgb(want));
        assert!(
            (i16::from(a.0) - i16::from(b.0)).abs() <= 1
                && (i16::from(a.2) - i16::from(b.2)).abs() <= 1,
            "{a:?} vs {b:?}"
        );
        // tile borders measured on the production wordle board: oklch(0.680483 0.025868 85.09) filled,
        // oklch(0.507982 0.027849 85.52) empty
        let filled = rgb(from_oklch(0.680_483, 0.025_868, 85.095, 1.0));
        let empty = rgb(from_oklch(0.507_982, 0.027_849, 85.517, 1.0));
        let near = |x: (u8, u8, u8), y: (u8, u8, u8)| {
            (i16::from(x.0) - i16::from(y.0)).abs() <= 1
                && (i16::from(x.1) - i16::from(y.1)).abs() <= 1
        };
        assert!(
            near(rgb(t.tile_filled_border), filled),
            "{:?} {filled:?}",
            rgb(t.tile_filled_border)
        );
        assert!(
            near(rgb(t.tile_border), empty),
            "{:?} {empty:?}",
            rgb(t.tile_border)
        );
        // transparent mixes keep the colour and take the weight as alpha
        assert_eq!(t.hover_14.alpha(), 36);
        assert_eq!(rgb(t.hover_14), rgb(hex(0xeee6d6)));
        // a near-grey surface mixed with a saturated colour takes that colour's hue (production:
        // geodle oklch(0.279349 0.015573 172.078), flaggle oklch(0.283856 0.014396 263.79))
        let geodle = rgb(from_oklch(0.279_349, 0.015_573, 172.078, 1.0));
        let flaggle = rgb(from_oklch(0.283_856, 0.014_396, 263.79, 1.0));
        assert!(
            near(rgb(t.geodle_bg), geodle),
            "{:?} {geodle:?}",
            rgb(t.geodle_bg)
        );
        assert!(
            near(rgb(t.flaggle_bg), flaggle),
            "{:?} {flaggle:?}",
            rgb(t.flaggle_bg)
        );
        // Wabi light: oklch(0.930277 0.015299 140.752)
        let wabi = tokens(wabi_base(false));
        let want = rgb(from_oklch(0.930_277, 0.015_299, 140.752, 1.0));
        assert!(
            near(rgb(wabi.wordle_bg), want),
            "{:?} {want:?}",
            rgb(wabi.wordle_bg)
        );
        // round trip
        assert_eq!(
            rgb(mix_oklch(hex(0x4f6b4a), 1.0, Some(hex(0)))),
            (0x4f, 0x6b, 0x4a)
        );
    }
}
