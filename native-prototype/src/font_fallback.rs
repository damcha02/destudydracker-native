//! CJK font fallback on Windows (Stage 22a Windows follow-up W22a-5).
//!
//! Slint lays text out with parley/fontique. For a run the requested family cannot cover,
//! fontique asks DirectWrite for one family per *script*, with no locale. On an English Windows
//! that answer for Han is a Japanese face (no 张/伟), so Simplified-Chinese names drew as boxes.
//! Chromium (production's WebView2) resolves the same text, with no `lang`, as below (probed with
//! `CSS.getPlatformFontsForNode` on this Windows installation; docs/stage22-social-network.md W.31):
//!
//! - Han (Simplified, Traditional, and the kanji in Japanese names): Microsoft YaHei;
//! - Hiragana / Katakana: Yu Gothic;
//! - Hangul: Malgun Gothic.
//!
//! This sets those as the per-script fallbacks of Slint's process-wide font collection
//! (`slint::fontique_010`, Slint's documented hook for extra glyph coverage), using installed
//! system families only: nothing is bundled or registered from disk. The other
//! installed CJK families follow as a reserve. A family that is not installed is skipped, and if
//! none is, the script keeps fontique's own choice. Named families (Arial, Georgia, Segoe UI
//! Symbol/Emoji) are untouched: fallback only applies to characters they lack, and Linux is not
//! affected at all.

#[cfg(windows)]
pub fn install() {
    use slint::fontique_010::fontique::{FallbackKey, Script};

    // (script, families in order). Han lists Chromium's pick first.
    const PLAN: &[(&str, &[&str])] = &[
        (
            "Hani",
            &[
                "Microsoft YaHei",
                "Microsoft JhengHei",
                "SimSun",
                "Yu Gothic",
                "Malgun Gothic",
            ],
        ),
        (
            "Hira",
            &["Yu Gothic", "Meiryo", "MS Gothic", "Microsoft YaHei"],
        ),
        (
            "Kana",
            &["Yu Gothic", "Meiryo", "MS Gothic", "Microsoft YaHei"],
        ),
        ("Hang", &["Malgun Gothic", "Gulim"]),
        ("Bopo", &["Microsoft JhengHei", "Microsoft YaHei"]),
    ];
    let mut collection = slint::fontique_010::shared_collection();
    for (script, families) in PLAN {
        let key = FallbackKey::new(Script::from_str_unchecked(script), None);
        let ids: Vec<_> = families
            .iter()
            .filter_map(|name| collection.family_id(name))
            .collect();
        if ids.is_empty() {
            log::info!("fonts: no installed family for {script}; keeping the system fallback");
            continue;
        }
        let names: Vec<&str> = families
            .iter()
            .copied()
            .filter(|name| collection.family_id(name).is_some())
            .collect();
        collection.set_fallbacks(key, ids.into_iter());
        log::info!("fonts: {script} falls back to {}", names.join(", "));
    }
}

#[cfg(not(windows))]
pub fn install() {}
