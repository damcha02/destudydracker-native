//! Appearance presentation (Stage 19): the single place that turns a
//! [`ResolvedAppearance`](study_tracker_core::appearance::ResolvedAppearance) into colours.
//!
//! ```text
//! AppearancePrefs (persisted)  ->  core::appearance::resolve  ->  ColorScheme
//!                                                                     |
//!                         fn_tokens / ws_tokens / chrome_tokens  <----+   (this file)
//!                                     |
//!                       Slint globals FN.t, WS.t, Chrome.t   (every surface reads only these)
//! ```
//!
//! Every value is production's computed CSS custom property for that style/palette/theme (read with
//! `scripts/visual-parity/capture-prod.mjs` + `vars.js`; tables in
//! docs/stage19-garden-wabisabi.md). Nothing else in the app hard-codes a style colour, so adding
//! a scheme is one table here, not a search through the UI for `if wabi` branches.

use slint::{Color, ModelRc, SharedString, VecModel};
use study_tracker_core::appearance::{
    AppStyle, AppearancePrefs, ColorScheme, Palette, RenderedStyle, ResolvedAppearance,
};

use crate::{ChoiceCard, ChromeTokens, FnTokens, WsTokens};

fn hex(rgb: u32) -> Color {
    Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}
fn hexa(rgb: u32, alpha: u8) -> Color {
    Color::from_argb_u8(alpha, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// Field Notebook tokens for a scheme (Wabi-Sabi schemes never draw Field Notebook surfaces, but
/// still get a sensible table so nothing is ever undefined).
pub fn fn_tokens(scheme: ColorScheme) -> FnTokens {
    match scheme {
        ColorScheme::FieldNotebookLight | ColorScheme::WabiLight => FnTokens {
            desk: hex(0xe6dfd1),
            surface: hex(0xfbf8f0),
            surface_2: hex(0xf4efe3),
            surface_inset: hex(0xf6f1e5),
            paper_1: hex(0xf4efe3),
            paper_2: hex(0xe4dccb),
            paper_3: hex(0xefe8da),
            slip: hex(0xf0e6c8),
            ink: hex(0x23211d),
            ink_2: hex(0x3d382f),
            ink_3: hex(0x6d6656),
            ink_4: hex(0x8b8474),
            line: hex(0xcec3ae),
            line_soft: hex(0xded5c3),
            rule_dot: hex(0xd5cbb7),
            rule_hard: hex(0x23211d),
            stamp: hex(0x9c5a34),
            link: hex(0x3c5f7a),
            blue: hex(0x4a5f86),
            green: hex(0x4f6b4a),
            accent: hex(0x23211d),
            accent_line: hexa(0x23211d, 0x5c),
            accent_soft: hexa(0x23211d, 0x12),
            danger_soft: hexa(0x9c5a34, 0x1f),
            ring_track: hex(0xe6dfd1),
            ok: hex(0x4f6b4a),
            steady: hex(0x4a5f86),
            critical: hex(0x9c5a34),
            os_dark: false,
        },
        // `:root[data-app-style="field-notebook"][data-palette="sakura"]` (identical whether
        // `data-theme` is dark or light; `color-scheme: light`).
        ColorScheme::FieldNotebookSakura => FnTokens {
            desk: hex(0xf7eeed),
            surface: hex(0xfae6e7),
            surface_2: hex(0xfcecf0),
            surface_inset: hex(0xf4bbc9),
            paper_1: hex(0xfcecf0),
            paper_2: hex(0xf4bbc9),
            paper_3: hex(0xfae6e7),
            slip: hex(0xfcecf0),
            ink: hex(0x3d2230),
            ink_2: hex(0x5a3447),
            ink_3: hex(0x8a6070),
            ink_4: hex(0xb08a9a),
            line: hex(0xf6a6bb),
            line_soft: hex(0xf4bbc9),
            rule_dot: hex(0xf6a6bb),
            rule_hard: hex(0xf191ac),
            stamp: hex(0xf191ac),
            link: hex(0x8a4a5e),
            blue: hex(0x8a4a6e),
            green: hex(0xb07085),
            accent: hex(0xf191ac),
            accent_line: hexa(0xf191ac, 0x52),
            accent_soft: hexa(0xf191ac, 0x24),
            danger_soft: hexa(0xc05070, 0x24),
            ring_track: hex(0xf4bbc9),
            ok: hex(0xd98a9f),
            steady: hex(0xb08a9a),
            critical: hex(0xc05070),
            os_dark: false,
        },
        ColorScheme::FieldNotebookDark | ColorScheme::WabiDark => FnTokens {
            desk: hex(0x191814),
            surface: hex(0x24221e),
            surface_2: hex(0x2b2823),
            surface_inset: hex(0x201e1a),
            paper_1: hex(0x2b2823),
            paper_2: hex(0x363126),
            paper_3: hex(0x2e2a23),
            slip: hex(0x3a3020),
            ink: hex(0xeee6d6),
            ink_2: hex(0xd7cdbb),
            ink_3: hex(0xaaa08d),
            ink_4: hex(0x817867),
            line: hex(0x58503f),
            line_soft: hex(0x3f392e),
            rule_dot: hex(0x584f3e),
            rule_hard: hex(0xeee6d6),
            stamp: hex(0xc47a4d),
            link: hex(0x8ea9c5),
            blue: hex(0x7187b4),
            green: hex(0x73956a),
            accent: hex(0xeee6d6),
            accent_line: hexa(0xeee6d6, 0x52),
            accent_soft: hexa(0xeee6d6, 0x14),
            danger_soft: hexa(0x9c5a34, 0x1f),
            ring_track: hex(0x3d382f),
            ok: hex(0x73956a),
            steady: hex(0x7187b4),
            critical: hex(0xc47a4d),
            os_dark: true,
        },
    }
}

/// Wabi-Sabi tokens (`:root[data-app-style="wabi-sabi"]`, `[data-theme="dark"]`). Sakura on
/// Wabi-Sabi is light-only and keeps these light tokens (`body` stays `--wabi-desk`).
pub fn ws_tokens(scheme: ColorScheme) -> WsTokens {
    match scheme {
        ColorScheme::WabiDark | ColorScheme::FieldNotebookDark => WsTokens {
            desk: hex(0x17160f),
            paper: hex(0x211f19),
            paper_1: hex(0x262418),
            paper_2: hex(0x302d21),
            ink: hex(0xece7d8),
            ink_3: hex(0xa39d89),
            muted: hex(0xa39d89),
            faint: hex(0x837d6a),
            sage: hex(0x7fa077),
            sage_deep: hex(0x97b78f),
            vermilion: hex(0xd97a5e),
            line: hex(0x4c4634),
            rule_hard: hex(0xece7d8),
            rule_soft: hex(0x363424),
            rule_dot: hex(0x453f2d),
            ring_track: hex(0x363424),
            ok: hex(0x7fa077),
            steady: hex(0xa39d89),
            watch: hex(0xc2a355),
            critical: hex(0xd97a5e),
            on_ink: hex(0x211f19),
            dark: true,
        },
        _ => WsTokens {
            desk: hex(0xe9e5d8),
            paper: hex(0xf4f1e8),
            paper_1: hex(0xefebe0),
            paper_2: hex(0xe5e1d0),
            ink: hex(0x1c1d19),
            ink_3: hex(0x6f7268),
            muted: hex(0x8d8b7d),
            faint: hex(0xa8a494),
            sage: hex(0x4f6b4a),
            sage_deep: hex(0x374d33),
            vermilion: hex(0xb0472e),
            line: hex(0xddd8c8),
            rule_hard: hex(0x1c1d19),
            rule_soft: hex(0xddd8c8),
            rule_dot: hex(0xcdc8b4),
            ring_track: hex(0xddd8c8),
            ok: hex(0x4f6b4a),
            steady: hex(0x6f7268),
            watch: hex(0x8a7226),
            critical: hex(0xb0472e),
            on_ink: hex(0xf4f1e8),
            dark: false,
        },
    }
}

/// The menu/panel skin, derived from the active style's own tokens (production's `.topbar-menu`
/// and `.settings-panel` read the same CSS variables the style defines).
pub fn chrome_tokens(resolved: &ResolvedAppearance) -> ChromeTokens {
    let s = |v: &str| SharedString::from(v);
    match resolved.rendered {
        RenderedStyle::WabiSabi => {
            let w = ws_tokens(resolved.scheme);
            ChromeTokens {
                // `.settings-panel-backdrop`: the desk at 70 % (production also blurs; Slint cannot)
                backdrop: w.desk.with_alpha(0.7),
                panel_bg: w.paper,
                ink: w.ink,
                ink_3: w.ink_3,
                muted: w.muted,
                card_bg: w.paper_1,
                card_border: w.line,
                card_active_bg: w.paper,
                control_bg: w.paper_1,
                control_border: w.line,
                rule_dot: w.rule_dot,
                menu_item: if w.dark { hex(0xd3ccb8) } else { hex(0x3f4239) },
                shadow: w.ink.with_alpha(0.08),
                serif: s("Georgia"),
                sans: s("Arial"),
                mono: s("Consolas"),
                label_font: s("Arial"),
                label_weight: 700,
                menu_caption: false,
                menu_item_font: s("Arial"),
                menu_item_size: 16.0,
                menu_item_weight: 700,
                square_swatches: true,
            }
        }
        RenderedStyle::FieldNotebook => {
            let f = fn_tokens(resolved.scheme);
            ChromeTokens {
                backdrop: if f.os_dark {
                    hexa(0x23211d, 0x38)
                } else {
                    f.ink.with_alpha(0.22)
                },
                panel_bg: f.surface,
                ink: f.ink,
                ink_3: f.ink_3,
                muted: f.ink_4,
                card_bg: f.paper_1,
                card_border: f.line_soft,
                card_active_bg: f.surface,
                control_bg: f.paper_1,
                control_border: f.line,
                rule_dot: f.rule_dot,
                menu_item: f.ink,
                shadow: if f.os_dark {
                    hexa(0x000000, 0x47)
                } else {
                    f.ink.with_alpha(0.08)
                },
                serif: s("Georgia"),
                sans: s("Arial"),
                mono: s("Consolas"),
                label_font: s("Consolas"),
                label_weight: 400,
                menu_caption: true,
                menu_item_font: s("Georgia"),
                menu_item_size: 14.0,
                menu_item_weight: 400,
                square_swatches: false,
            }
        }
    }
}

/// Production's theme picker (`themePalettes` filtered by style): for Field Notebook and
/// Wabi-Sabi exactly Default and Sakura, with Wabi-Sabi's own descriptions and Default swatch.
pub fn palette_choices(prefs: AppearancePrefs) -> Vec<(Palette, ChoiceCard)> {
    let wabi = prefs.style == AppStyle::WabiSabi;
    Palette::offered_for(prefs.style)
        .into_iter()
        .map(|palette| {
            let (title, desc, card) = match palette {
                Palette::Default => (
                    "Default",
                    if wabi {
                        "Moss, paper and ink at rest."
                    } else {
                        "Editorial blue-grey with soft study accents."
                    },
                    // `oklch(0.70 0.10 245)`; on Wabi-Sabi a flat moss square.
                    if wabi {
                        (0, hex(0x4f6b4a), hex(0x4f6b4a), hex(0x4f6b4a))
                    } else {
                        (0, hex(0x67a5d9), hex(0x67a5d9), hex(0x67a5d9))
                    },
                ),
                Palette::Sakura => (
                    "Sakura",
                    if wabi {
                        "Petals loved for falling."
                    } else {
                        "Whitish-pink notebook \u{2014} fallen petals."
                    },
                    (1, hex(0xf7eeed), hex(0xf191ac), hex(0xf191ac)),
                ),
                // Modern-only palettes never reach this list for the two migrated styles.
                other => (
                    "",
                    other.production_id(),
                    (0, hex(0x888888), hex(0x888888), hex(0x888888)),
                ),
            };
            (
                palette,
                ChoiceCard {
                    title: title.into(),
                    desc: desc.into(),
                    swatch_kind: card.0,
                    a: card.1,
                    b: card.2,
                    c: card.3,
                    active: prefs.palette == palette,
                    enabled: true,
                },
            )
        })
        .collect()
}

/// Production's style picker (`appStyles`). Modern is listed like production but cannot be chosen
/// yet: its dashboards (and the Knowledge Garden they host) are not migrated.
pub fn style_choices(prefs: AppearancePrefs) -> Vec<(AppStyle, ChoiceCard)> {
    [
        (
            AppStyle::Modern,
            "Modern",
            "Current rounded study dashboard with palette themes.",
            (1, hex(0x8fb4ff), hex(0x98c379), hex(0x98c379)),
        ),
        (
            AppStyle::FieldNotebook,
            "Field Notebook",
            "Paper, ink, course tabs, ruled ledgers, and study-circle social styling.",
            (2, hex(0xfbf8f0), hex(0x23211d), hex(0x9c5a34)),
        ),
        (
            AppStyle::WabiSabi,
            "Wabi-Sabi \u{4f98}\u{5bc2}",
            "The beauty of what is humble, impermanent and unfinished - a cracked bowl, moss on stone, the quiet of a single breath.",
            (3, hex(0xe9e5d8), hex(0x4f6b4a), hex(0xb0472e)),
        ),
    ]
    .into_iter()
    .map(|(style, title, desc, card)| {
        (
            style,
            ChoiceCard {
                title: title.into(),
                desc: desc.into(),
                swatch_kind: card.0,
                a: card.1,
                b: card.2,
                c: card.3,
                active: prefs.style == style,
                enabled: style.is_supported_natively(),
            },
        )
    })
    .collect()
}

pub fn cards_model(cards: Vec<ChoiceCard>) -> ModelRc<ChoiceCard> {
    ModelRc::new(VecModel::from(cards))
}

/// The Wabi-Sabi score ring's arc as an SVG path in a 36x36 box: radius 15.5 around the centre,
/// starting at 12 o'clock and running clockwise for `score` percent (production draws a dashed
/// circle rotated by -90deg; a round cap is added by the stroke).
pub fn score_arc(score: u32) -> String {
    let fraction = f64::from(score.min(100)) / 100.0;
    if fraction <= 0.0 {
        return String::new();
    }
    let (cx, cy, r) = (18.0_f64, 18.0_f64, 15.5_f64);
    if fraction >= 1.0 {
        return format!(
            "M {cx} {:.3} A {r} {r} 0 1 1 {:.3} {:.3} A {r} {r} 0 1 1 {cx} {:.3}",
            cy - r,
            cx,
            cy + r,
            cy - r
        );
    }
    let angle = fraction * std::f64::consts::TAU;
    let (x, y) = (cx + r * angle.sin(), cy - r * angle.cos());
    let large = if fraction > 0.5 { 1 } else { 0 };
    format!("M {cx} {:.3} A {r} {r} 0 {large} 1 {x:.3} {y:.3}", cy - r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tracker_core::appearance::ThemeMode;

    #[test]
    fn every_scheme_has_complete_distinct_tables() {
        let dark = fn_tokens(ColorScheme::FieldNotebookDark);
        let light = fn_tokens(ColorScheme::FieldNotebookLight);
        let sakura = fn_tokens(ColorScheme::FieldNotebookSakura);
        assert_ne!(dark.desk, light.desk);
        assert_eq!(
            sakura.desk,
            hex(0xf7eeed),
            "production --fn-desk under Sakura"
        );
        assert!(!sakura.os_dark, "Sakura sets color-scheme: light");
        assert_eq!(ws_tokens(ColorScheme::WabiLight).desk, hex(0xe9e5d8));
        assert_eq!(ws_tokens(ColorScheme::WabiDark).desk, hex(0x17160f));
    }

    #[test]
    fn the_stage_17_dark_defaults_are_unchanged() {
        // fn/palette.slint's default table (= production dark) and this table must agree.
        let dark = fn_tokens(ColorScheme::FieldNotebookDark);
        assert_eq!(dark.desk, hex(0x191814));
        assert_eq!(dark.surface, hex(0x24221e));
        assert_eq!(dark.ink, hex(0xeee6d6));
        assert_eq!(dark.stamp, hex(0xc47a4d));
        assert_eq!(dark.critical, dark.stamp);
    }

    #[test]
    fn wabi_sabi_offers_default_and_sakura_with_its_own_copy() {
        let prefs = AppearancePrefs {
            style: AppStyle::WabiSabi,
            palette: Palette::Sakura,
            theme: ThemeMode::Light,
        };
        let cards = palette_choices(prefs);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].1.desc.as_str(), "Moss, paper and ink at rest.");
        assert!(cards[1].1.active);
        let styles = style_choices(prefs);
        assert!(
            !styles[0].1.enabled,
            "Modern is listed but not selectable natively"
        );
        assert!(styles[2].1.active && styles[1].1.enabled);
    }

    #[test]
    fn the_score_arc_runs_clockwise_from_twelve_o_clock() {
        assert_eq!(score_arc(0), "");
        assert_eq!(score_arc(25), "M 18 2.500 A 15.5 15.5 0 0 1 33.500 18.000");
        assert!(
            score_arc(75).contains(" 0 1 1 "),
            "large-arc flag past half"
        );
        assert!(
            score_arc(100).matches('A').count() == 2,
            "a full circle needs two arcs"
        );
    }
}
