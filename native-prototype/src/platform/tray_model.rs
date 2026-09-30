//! Pure (Windows-independent, unit-tested) tray and window-lifecycle *behavior* ported from
//! production v0.1.66's Tauri backend (`desktop/src-tauri/src/lib.rs`): the tray tooltip text, the
//! per-phase tray glyph, and the rule for what closing the window does. The Win32 adapter that
//! displays these lives in `platform::win_tray`; nothing here touches the OS.

use study_tracker_core::timer::TimerPhase;

/// What the tray has to show about the Timer (production's `set_timer_tray_state(phase, running,
/// remainingSeconds, verifiedSessionActive)`, minus the social flag - see [`close_action`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrayTimerState {
    pub phase: TimerPhase,
    pub running: bool,
    /// What the on-screen clock shows: remaining seconds for countdowns, elapsed for Endless.
    pub display_seconds: u64,
}

impl TrayTimerState {
    pub const IDLE: Self = Self {
        phase: TimerPhase::Idle,
        running: false,
        display_seconds: 0,
    };
}

/// `timer_tray_title`: `Idle`, `Study 24:59`, `Paused Break 03:00`, `Endless 12:03`.
pub fn tray_title(state: TrayTimerState) -> String {
    if state.phase == TimerPhase::Idle {
        return "Idle".into();
    }
    let label = match state.phase {
        TimerPhase::Study => "Study",
        TimerPhase::Break => "Break",
        TimerPhase::Exam => "Exam",
        TimerPhase::Stopwatch => "Endless",
        TimerPhase::Idle => "Timer",
    };
    // Production passes a u32; clamp instead of wrapping for absurd values.
    let seconds = state.display_seconds.min(u64::from(u32::MAX));
    let (minutes, secs) = (seconds / 60, seconds % 60);
    if state.running {
        format!("{label} {minutes:02}:{secs:02}")
    } else {
        format!("Paused {label} {minutes:02}:{secs:02}")
    }
}

/// `timer_tray_tooltip`: `Study Tracker - Study 24:59`.
pub fn tray_tooltip(state: TrayTimerState) -> String {
    format!("Study Tracker - {}", tray_title(state))
}

/// What closing the main window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Close = exit the process (production's default when nothing is running).
    Quit,
    /// Hide to the tray; the Timer keeps running and the tray "Quit" is the way out.
    HideToTray,
}

/// Production hides instead of quitting only while a *verified study session* is active:
/// `isSocialApiConfigured() && timer.running && phase != idle && phase != break`
/// (`App.tsx`'s tray effect -> `VERIFIED_SESSION_ACTIVE`). `isSocialApiConfigured()` is true in
/// every shipped build (a default API URL is compiled in), so the native rule assumes it; Social
/// itself is not migrated (Stage 21).
pub fn close_action(state: TrayTimerState) -> CloseAction {
    let session_active =
        state.running && !matches!(state.phase, TimerPhase::Idle | TimerPhase::Break);
    if session_active {
        CloseAction::HideToTray
    } else {
        CloseAction::Quit
    }
}

/// Production shows this notice once per run, the first time the window is hidden to the tray.
pub const HIDE_NOTICE_TITLE: &str = "Study Tracker is still running";
pub const HIDE_NOTICE_BODY: &str =
    "Your session keeps tracking in the tray. Right-click the tray icon to quit.";

/// Tracks the once-per-run hide notice (production: `TRAY_CLOSE_NOTICE_SHOWN`).
#[derive(Debug, Default)]
pub struct HideNoticeLatch {
    shown: bool,
}

impl HideNoticeLatch {
    /// `true` exactly the first time it is asked.
    pub fn first_time(&mut self) -> bool {
        !std::mem::replace(&mut self.shown, true)
    }
}

/// Tray icon side length (production: 32x32 RGBA).
pub const ICON_SIZE: usize = 32;

/// Production's generated tray glyph for a phase, ported from `tray_icon_for_phase` (gradient
/// disc + hand-drawn glyph): `s` for Study/Endless, pause bars for Break, `e` for Exam, a leaf
/// when idle. Returns straight RGBA, row-major, `ICON_SIZE` squared.
pub fn tray_icon_rgba(phase: TimerPhase) -> Vec<u8> {
    let size = ICON_SIZE;
    let mut pixels = vec![0u8; size * size * 4];
    let (top, bottom): ([u8; 4], [u8; 4]) = match phase {
        TimerPhase::Study | TimerPhase::Stopwatch => ([90, 213, 172, 255], [47, 122, 255, 255]),
        TimerPhase::Break => ([255, 207, 104, 255], [255, 107, 107, 255]),
        TimerPhase::Exam => ([255, 129, 129, 255], [150, 72, 255, 255]),
        TimerPhase::Idle => ([143, 180, 255, 255], [222, 83, 128, 255]),
    };
    for y in 0..size {
        for x in 0..size {
            let index = (y * size + x) * 4;
            let (dx, dy) = (x as f32 - 15.5, y as f32 - 15.5);
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 15.5 {
                continue;
            }
            let t = y as f32 / (size - 1) as f32;
            let edge_alpha = if distance > 14.0 {
                ((15.5 - distance) / 1.5).clamp(0.0, 1.0)
            } else {
                1.0
            };
            for channel in 0..3 {
                pixels[index + channel] =
                    ((top[channel] as f32 * (1.0 - t)) + (bottom[channel] as f32 * t)) as u8;
            }
            pixels[index + 3] = (255.0 * edge_alpha) as u8;
        }
    }
    match phase {
        TimerPhase::Study | TimerPhase::Stopwatch => draw_snake_s(&mut pixels, size),
        TimerPhase::Break => draw_break_mark(&mut pixels, size),
        TimerPhase::Exam => draw_exam_e(&mut pixels, size),
        TimerPhase::Idle => draw_leaf(&mut pixels, size),
    }
    pixels
}

fn put_pixel(pixels: &mut [u8], size: usize, x: i32, y: i32, color: [u8; 4]) {
    if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
        return;
    }
    let index = ((y as usize * size) + x as usize) * 4;
    let alpha = color[3] as f32 / 255.0;
    for channel in 0..3 {
        pixels[index + channel] =
            (color[channel] as f32 * alpha + pixels[index + channel] as f32 * (1.0 - alpha)) as u8;
    }
    pixels[index + 3] = 255;
}

fn draw_disc(pixels: &mut [u8], size: usize, cx: f32, cy: f32, radius: f32, color: [u8; 4]) {
    let (min_x, max_x) = (
        (cx - radius - 1.0).floor() as i32,
        (cx + radius + 1.0).ceil() as i32,
    );
    let (min_y, max_y) = (
        (cy - radius - 1.0).floor() as i32,
        (cy + radius + 1.0).ceil() as i32,
    );
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (dx, dy) = (x as f32 - cx, y as f32 - cy);
            if (dx * dx + dy * dy).sqrt() <= radius {
                put_pixel(pixels, size, x, y, color);
            }
        }
    }
}

fn draw_line(
    pixels: &mut [u8],
    size: usize,
    from: (f32, f32),
    to: (f32, f32),
    width: f32,
    color: [u8; 4],
) {
    let steps = ((to.0 - from.0).abs().max((to.1 - from.1).abs()) * 2.0).ceil() as i32;
    for step in 0..=steps.max(1) {
        let t = step as f32 / steps.max(1) as f32;
        draw_disc(
            pixels,
            size,
            from.0 + (to.0 - from.0) * t,
            from.1 + (to.1 - from.1) * t,
            width / 2.0,
            color,
        );
    }
}

fn draw_snake_s(pixels: &mut [u8], size: usize) {
    let color = [8, 16, 24, 245];
    let points = [
        (21.5, 9.0),
        (16.0, 7.0),
        (10.5, 9.0),
        (10.0, 13.0),
        (15.8, 15.5),
        (21.2, 18.0),
        (20.0, 22.4),
        (14.0, 24.0),
        (9.8, 21.5),
    ];
    for pair in points.windows(2) {
        draw_line(pixels, size, pair[0], pair[1], 4.3, color);
    }
    draw_disc(pixels, size, 22.2, 8.5, 1.4, [250, 255, 255, 255]);
}

fn draw_break_mark(pixels: &mut [u8], size: usize) {
    let color = [29, 18, 24, 245];
    draw_line(pixels, size, (11.0, 9.0), (11.0, 23.0), 4.5, color);
    draw_line(pixels, size, (21.0, 9.0), (21.0, 23.0), 4.5, color);
}

fn draw_exam_e(pixels: &mut [u8], size: usize) {
    let color = [255, 250, 235, 250];
    draw_line(pixels, size, (10.0, 8.0), (10.0, 24.0), 4.0, color);
    draw_line(pixels, size, (10.0, 8.5), (22.5, 8.5), 4.0, color);
    draw_line(pixels, size, (10.0, 16.0), (20.0, 16.0), 4.0, color);
    draw_line(pixels, size, (10.0, 23.5), (22.5, 23.5), 4.0, color);
}

fn draw_leaf(pixels: &mut [u8], size: usize) {
    let color = [8, 16, 24, 230];
    draw_line(pixels, size, (10.0, 21.0), (22.0, 9.0), 2.5, color);
    draw_line(pixels, size, (11.0, 20.0), (9.5, 13.0), 3.2, color);
    draw_line(pixels, size, (9.5, 13.0), (16.5, 9.5), 3.2, color);
    draw_line(pixels, size, (16.5, 9.5), (22.5, 9.2), 3.2, color);
    draw_line(pixels, size, (22.5, 9.2), (20.0, 16.5), 3.2, color);
    draw_line(pixels, size, (20.0, 16.5), (11.0, 20.0), 3.2, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(phase: TimerPhase, running: bool, secs: u64) -> TrayTimerState {
        TrayTimerState {
            phase,
            running,
            display_seconds: secs,
        }
    }

    #[test]
    fn titles_and_tooltips_match_production_strings() {
        assert_eq!(tray_title(TrayTimerState::IDLE), "Idle");
        assert_eq!(tray_tooltip(TrayTimerState::IDLE), "Study Tracker - Idle");
        assert_eq!(
            tray_title(st(TimerPhase::Study, true, 25 * 60)),
            "Study 25:00"
        );
        assert_eq!(
            tray_title(st(TimerPhase::Study, false, 12 * 60 + 3)),
            "Paused Study 12:03"
        );
        assert_eq!(tray_title(st(TimerPhase::Break, true, 59)), "Break 00:59");
        assert_eq!(
            tray_title(st(TimerPhase::Exam, true, 7199)),
            "Exam 119:59",
            "minutes are not wrapped into hours, like production"
        );
        assert_eq!(
            tray_title(st(TimerPhase::Stopwatch, true, 3725)),
            "Endless 62:05"
        );
        assert_eq!(
            tray_tooltip(st(TimerPhase::Break, false, 180)),
            "Study Tracker - Paused Break 03:00"
        );
    }

    #[test]
    fn closing_hides_only_while_a_study_like_session_is_running() {
        assert_eq!(close_action(TrayTimerState::IDLE), CloseAction::Quit);
        assert_eq!(
            close_action(st(TimerPhase::Study, true, 10)),
            CloseAction::HideToTray
        );
        assert_eq!(
            close_action(st(TimerPhase::Exam, true, 10)),
            CloseAction::HideToTray
        );
        assert_eq!(
            close_action(st(TimerPhase::Stopwatch, true, 10)),
            CloseAction::HideToTray
        );
        assert_eq!(
            close_action(st(TimerPhase::Study, false, 10)),
            CloseAction::Quit,
            "paused: close quits (production requires running)"
        );
        assert_eq!(
            close_action(st(TimerPhase::Break, true, 10)),
            CloseAction::Quit,
            "breaks never pin the app"
        );
    }

    #[test]
    fn the_hide_notice_fires_once_per_run() {
        let mut latch = HideNoticeLatch::default();
        assert!(latch.first_time());
        assert!(!latch.first_time());
        assert!(!latch.first_time());
    }

    #[test]
    fn icons_are_32x32_rgba_and_differ_per_phase() {
        let idle = tray_icon_rgba(TimerPhase::Idle);
        assert_eq!(idle.len(), ICON_SIZE * ICON_SIZE * 4);
        assert_eq!(idle[3], 0, "corners are transparent (a disc)");
        let centre = (16 * ICON_SIZE + 16) * 4;
        assert_eq!(idle[centre + 3], 255);
        let phases = [
            TimerPhase::Idle,
            TimerPhase::Study,
            TimerPhase::Break,
            TimerPhase::Exam,
        ];
        for (i, a) in phases.iter().enumerate() {
            for b in &phases[i + 1..] {
                assert_ne!(tray_icon_rgba(*a), tray_icon_rgba(*b), "{a:?} vs {b:?}");
            }
        }
        assert_eq!(
            tray_icon_rgba(TimerPhase::Study),
            tray_icon_rgba(TimerPhase::Stopwatch),
            "Endless shares the study glyph"
        );
    }
}
