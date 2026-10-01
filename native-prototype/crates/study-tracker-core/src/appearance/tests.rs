use super::sakura::*;
use super::*;

#[test]
fn production_defaults_are_field_notebook_default_palette_dark() {
    let prefs = AppearancePrefs::from_production(None, None, None);
    assert_eq!(prefs, AppearancePrefs::default());
    assert_eq!(prefs.style, AppStyle::FieldNotebook);
    assert_eq!(prefs.palette, Palette::Default);
    assert_eq!(prefs.theme, ThemeMode::Dark);
}

#[test]
fn every_production_id_round_trips() {
    for style in [
        AppStyle::Modern,
        AppStyle::FieldNotebook,
        AppStyle::WabiSabi,
    ] {
        assert_eq!(
            AppStyle::from_production(style.production_id()),
            Some(style)
        );
    }
    for palette in Palette::ALL {
        assert_eq!(
            Palette::from_production(palette.production_id()),
            Some(palette)
        );
    }
    for theme in [ThemeMode::Dark, ThemeMode::Light] {
        assert_eq!(
            ThemeMode::from_production(Some(theme.production_id())),
            theme
        );
    }
}

#[test]
fn unknown_and_legacy_values_fall_back_like_production() {
    // loadAppStyle / loadThemePalette fallbacks and the three renamed palettes.
    let prefs = AppearancePrefs::from_production(Some("brutalist"), Some("neon"), Some("dark"));
    assert_eq!(prefs.style, AppStyle::FieldNotebook);
    assert_eq!(prefs.palette, Palette::Default);
    assert_eq!(Palette::from_production("parchment"), Some(Palette::Paper));
    assert_eq!(Palette::from_production("cosmic"), Some(Palette::Retrowave));
    assert_eq!(Palette::from_production("grove"), Some(Palette::Forest));
    // `|| "dark"`: empty string is dark; any other string becomes data-theme and acts light.
    assert_eq!(ThemeMode::from_production(Some("")), ThemeMode::Dark);
    assert_eq!(ThemeMode::from_production(Some("sepia")), ThemeMode::Light);
}

#[test]
fn picker_offers_default_and_sakura_on_notebook_styles_and_everything_else_on_modern() {
    assert_eq!(
        Palette::offered_for(AppStyle::WabiSabi),
        vec![Palette::Default, Palette::Sakura]
    );
    assert_eq!(
        Palette::offered_for(AppStyle::FieldNotebook),
        vec![Palette::Default, Palette::Sakura]
    );
    let modern = Palette::offered_for(AppStyle::Modern);
    assert_eq!(modern.len(), 15);
    assert!(!modern.contains(&Palette::Sakura));
}

fn resolve(style: AppStyle, palette: Palette, theme: ThemeMode) -> ResolvedAppearance {
    AppearancePrefs {
        style,
        palette,
        theme,
    }
    .resolve()
}

#[test]
fn wabi_sabi_with_sakura_is_light_only_and_animated() {
    let r = resolve(AppStyle::WabiSabi, Palette::Sakura, ThemeMode::Dark);
    assert_eq!(r.rendered, RenderedStyle::WabiSabi);
    assert_eq!(r.scheme, ColorScheme::WabiLight);
    assert!(r.sakura && r.theme_locked && !r.dark);
}

#[test]
fn field_notebook_sakura_uses_the_pink_tokens_in_both_themes_without_a_lock() {
    for theme in [ThemeMode::Dark, ThemeMode::Light] {
        let r = resolve(AppStyle::FieldNotebook, Palette::Sakura, theme);
        assert_eq!(r.scheme, ColorScheme::FieldNotebookSakura);
        assert!(r.sakura);
        assert!(!r.theme_locked, "production only locks Wabi-Sabi");
        assert_eq!(
            r.dark,
            theme == ThemeMode::Dark,
            "the toggle icon still follows data-theme"
        );
    }
}

#[test]
fn default_palette_never_animates() {
    for style in [
        AppStyle::FieldNotebook,
        AppStyle::WabiSabi,
        AppStyle::Modern,
    ] {
        for theme in [ThemeMode::Dark, ThemeMode::Light] {
            assert!(!resolve(style, Palette::Default, theme).sakura);
        }
    }
    assert_eq!(
        resolve(AppStyle::WabiSabi, Palette::Default, ThemeMode::Dark).scheme,
        ColorScheme::WabiDark
    );
    assert_eq!(
        resolve(AppStyle::FieldNotebook, Palette::Default, ThemeMode::Light).scheme,
        ColorScheme::FieldNotebookLight
    );
}

#[test]
fn modern_renders_the_default_style_and_never_shows_sakura() {
    // Production's Modern picker never offers Sakura and SakuraScatter requires FN/Wabi.
    let r = resolve(AppStyle::Modern, Palette::Sakura, ThemeMode::Dark);
    assert_eq!(r.rendered, RenderedStyle::FieldNotebook);
    assert_eq!(r.scheme, ColorScheme::FieldNotebookDark);
    assert!(!r.sakura && !r.theme_locked);
    assert!(!AppStyle::Modern.is_supported_natively());
}

#[test]
fn modern_only_palettes_render_as_default_on_notebook_styles() {
    // Field Notebook/Wabi-Sabi tokens out-rank `[data-palette=..]` tokens in production's cascade.
    assert_eq!(
        resolve(AppStyle::WabiSabi, Palette::Forest, ThemeMode::Light),
        resolve(AppStyle::WabiSabi, Palette::Default, ThemeMode::Light)
    );
}

// ---- Sakura -----------------------------------------------------------------------------------

#[test]
fn mulberry32_matches_production_javascript() {
    // Values printed by production's exact `mulberry32` (SakuraScatter.tsx) under Node.
    let petals = production_petals();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let p0 = petals[0];
    assert!(close(p0.left_percent, 37.632_537_423_633_04));
    assert!(close(p0.size, 28.562_066_121_026_874));
    assert!(close(p0.opacity, 0.334_417_954_767_122_9));
    assert!(close(p0.drift, -21.933_611_556_887_627));
    assert!(close(p0.spin, -446.744_041_917_845_6));
    assert!(close(p0.fall_seconds, 14.384_559_892_583_638));
    assert!(close(p0.delay_seconds, -2.176_972_324_028_611));
    assert!(close(p0.sway_seconds, 5.188_551_621_744_409));
    let p7 = petals[7];
    assert!(close(p7.left_percent, 7.354_994_467_459_619));
    assert!(close(p7.spin, 386.376_677_323_132_75));
    assert!(close(p7.delay_seconds, -24.830_021_393_485_367));
    let p21 = petals[21];
    assert!(close(p21.size, 16.069_238_031_283_02));
    assert!(close(p21.fall_seconds, 26.900_178_595_446_05));
    assert!(close(p21.sway_seconds, 3.680_429_355_008_527_6));
    assert_eq!(p21.image, 1);
    assert_eq!(petals[0].image, 0);
}

#[test]
fn parameters_stay_inside_production_ranges() {
    for p in production_petals() {
        assert!((0.0..100.0).contains(&p.left_percent));
        assert!((16.0..36.0).contains(&p.size));
        assert!((0.22..0.42).contains(&p.opacity));
        assert!((-80.0..80.0).contains(&p.drift));
        assert!((180.0..540.0).contains(&p.spin.abs()));
        assert!((14.0..28.0).contains(&p.fall_seconds));
        assert!((-28.0..=0.0).contains(&p.delay_seconds));
        assert!((3.0..6.0).contains(&p.sway_seconds));
    }
}

fn angle_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(360.0);
    d.min(360.0 - d)
}

#[test]
fn poses_match_chromium_frozen_at_nine_seconds() {
    // Chromium (production build, 1520x980, every animation paused at currentTime = 9000 ms):
    // image centre, total rotation and opacity per petal, from computed transform matrices. The
    // probe rounds the box to whole pixels (offsetLeft/offsetWidth), hence the 1 px tolerance.
    let chromium: [(usize, f64, f64, f64, f64); 8] = [
        (0, 559.794, 775.923, 0.13, 0.3344),
        (1, 298.494, 723.168, 155.241, 0.2269),
        (3, 432.1, 969.527, 5.633, 0.1144),
        (7, 144.212, 494.437, 178.267, 0.3293),
        (12, 1565.205, 412.004, -168.418, 0.386),
        (15, -30.108, 867.255, 133.948, 0.2747),
        (19, 1433.354, 23.33, -29.608, 0.2693),
        (21, 991.463, 807.018, 7.224, 0.3559),
    ];
    let petals = production_petals();
    for (i, cx, cy, rot, op) in chromium {
        let pose = petal_pose(&petals[i], 9000.0, 980.0);
        let x = pose.left_fraction * 1520.0 + pose.dx;
        assert!((x - cx).abs() < 1.0, "petal {i}: x {x} vs chromium {cx}");
        assert!(
            (pose.center_y - cy).abs() < 1.0,
            "petal {i}: y {} vs {cy}",
            pose.center_y
        );
        assert!(
            angle_diff(pose.rotation_deg, rot) < 1.0,
            "petal {i}: rot {} vs {rot}",
            pose.rotation_deg
        );
        assert!(
            (pose.opacity - op).abs() < 0.002,
            "petal {i}: opacity {} vs {op}",
            pose.opacity
        );
    }
}

#[test]
fn a_petal_fades_in_and_out_at_the_ends_of_its_fall() {
    let p = production_petals()[0];
    let fall_ms = p.fall_seconds * 1000.0;
    // time at which local progress is exactly 0 (start of an iteration)
    let start = p.delay_seconds * 1000.0 + fall_ms;
    assert!(petal_pose(&p, start, 900.0).opacity < 1e-6);
    let at_8 = petal_pose(&p, start + fall_ms * 0.08, 900.0).opacity;
    assert!((at_8 - p.opacity).abs() < 1e-9);
    let mid = petal_pose(&p, start + fall_ms * 0.5, 900.0);
    assert!((mid.opacity - p.opacity).abs() < 1e-9);
    assert!(petal_pose(&p, start + fall_ms * 0.999, 900.0).opacity < 0.01);
}

#[test]
fn the_fall_is_periodic_so_any_elapsed_time_is_a_valid_frame() {
    let p = production_petals()[5];
    let fall_ms = p.fall_seconds * 1000.0;
    let a = petal_pose(&p, 1234.0, 980.0);
    let b = petal_pose(&p, 1234.0 + fall_ms * 1000.0, 980.0);
    // The fall repeats every fall_ms; the sway has its own period, so compare the fall parts only.
    assert!((a.opacity - b.opacity).abs() < 1e-6);
    // Thirty hidden minutes later the pose is simply evaluated at the new time - finite, in range.
    let later = petal_pose(&p, 30.0 * 60_000.0, 980.0);
    assert!(later.center_y.is_finite() && later.center_y > -80.0 && later.center_y < 1100.0);
}

#[test]
fn ease_in_out_is_the_css_curve() {
    assert_eq!(ease_in_out(0.0), 0.0);
    assert_eq!(ease_in_out(1.0), 1.0);
    assert!((ease_in_out(0.5) - 0.5).abs() < 1e-9);
    // cubic-bezier(.42,0,.58,1) at x=0.25 (reference value from the bezier definition)
    assert!((ease_in_out(0.25) - 0.129_161).abs() < 1e-4);
    assert!((ease_in_out(0.25) + ease_in_out(0.75) - 1.0).abs() < 1e-9);
}

#[test]
fn reduced_motion_poses_are_static_at_twenty_percent() {
    for p in production_petals() {
        let a = reduced_motion_pose(&p, 1000.0);
        assert_eq!(a.rotation_deg, 0.0);
        assert_eq!(a.opacity, p.opacity);
        assert!((a.center_y - (200.0 + p.size / 2.0)).abs() < 1e-9);
    }
}

#[test]
fn texture_frames_cycle_every_130_ms() {
    assert_eq!(texture_frame(0.0), 0);
    assert_eq!(texture_frame(129.0), 0);
    assert_eq!(texture_frame(130.0), 1);
    assert_eq!(texture_frame(130.0 * 19.0), 19);
    assert_eq!(texture_frame(130.0 * 20.0), 0);
    assert_eq!(texture_frame(-5.0), 0);
}
