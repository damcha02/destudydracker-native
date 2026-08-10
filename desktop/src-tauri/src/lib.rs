#[cfg(target_os = "linux")]
use std::collections::HashMap;
use std::fs;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_notification::NotificationExt;

#[cfg(target_os = "linux")]
const LATEST_UPDATE_JSON_URL: &str =
    "https://github.com/damcha02/destudydracker/releases/latest/download/latest.json";
const TIMER_TRAY_ID: &str = "study-tracker-timer";
static LAST_TRAY_ICON_PHASE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
// Whether a verified study session is currently active, per the frontend's own eligibility check
// (App.tsx). Closing the main window only hides it (instead of quitting) while this is true, so
// an in-progress session survives the close button; closing while idle still quits as before.
static VERIFIED_SESSION_ACTIVE: OnceLock<Mutex<bool>> = OnceLock::new();
// Session-scoped (not persisted across restarts) — just enough to avoid repeating the
// hide-to-tray notice on every close click within a single run of the app.
static TRAY_CLOSE_NOTICE_SHOWN: OnceLock<Mutex<bool>> = OnceLock::new();

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInstallSupport {
    can_auto_install: bool,
    package_hint: String,
    runtime_channel: String,
    message: String,
}

#[cfg(target_os = "linux")]
#[derive(serde::Deserialize)]
struct LatestUpdateJson {
    version: String,
    platforms: HashMap<String, UpdatePlatform>,
}

#[cfg(target_os = "linux")]
#[derive(serde::Deserialize)]
struct UpdatePlatform {
    url: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LinuxUpdateDownload {
    version: String,
    package_type: String,
    file_path: String,
    install_command: String,
    message: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceIdentity {
    fingerprint_hash: String,
    label: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeHttpResponse {
    status: u16,
    body: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryFile {
    name: String,
    path: String,
    extension: String,
    kind: String,
    size_bytes: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct VaultNoteFile {
    title: String,
    path: String,
    saved_at: u64,
}

fn tray_icon_for_phase(phase: &str) -> Image<'static> {
    const SIZE: usize = 32;
    let mut pixels = vec![0; SIZE * SIZE * 4];

    let (top, bottom, glyph) = match phase {
        "study" => ([90, 213, 172, 255], [47, 122, 255, 255], Some('s')),
        "break" => ([255, 207, 104, 255], [255, 107, 107, 255], Some('b')),
        "exam" => ([255, 129, 129, 255], [150, 72, 255, 255], Some('e')),
        "stopwatch" => ([90, 213, 172, 255], [47, 122, 255, 255], Some('s')),
        _ => ([143, 180, 255, 255], [222, 83, 128, 255], None),
    };

    for y in 0..SIZE {
        for x in 0..SIZE {
            let index = (y * SIZE + x) * 4;
            let dx = x as f32 - 15.5;
            let dy = y as f32 - 15.5;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 15.5 {
                continue;
            }

            let t = y as f32 / (SIZE - 1) as f32;
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

    match glyph {
        Some('s') => draw_snake_s(&mut pixels, SIZE),
        Some('b') => draw_break_mark(&mut pixels, SIZE),
        Some('e') => draw_exam_e(&mut pixels, SIZE),
        _ => draw_leaf(&mut pixels, SIZE),
    }

    Image::new_owned(pixels, SIZE as u32, SIZE as u32)
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
    let min_x = (cx - radius - 1.0).floor() as i32;
    let max_x = (cx + radius + 1.0).ceil() as i32;
    let min_y = (cy - radius - 1.0).floor() as i32;
    let max_y = (cy + radius + 1.0).ceil() as i32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
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

fn timer_tray_title(phase: &str, running: bool, remaining_seconds: u32) -> String {
    if phase == "idle" {
        return "Idle".into();
    }

    let minutes = remaining_seconds / 60;
    let seconds = remaining_seconds % 60;
    let label = match phase {
        "study" => "Study",
        "break" => "Break",
        "exam" => "Exam",
        "stopwatch" => "Endless",
        _ => "Timer",
    };
    if !running {
        return format!("Paused {label} {minutes:02}:{seconds:02}");
    }
    format!("{label} {minutes:02}:{seconds:02}")
}

fn timer_tray_tooltip(phase: &str, running: bool, remaining_seconds: u32) -> String {
    format!(
        "Study Tracker - {}",
        timer_tray_title(phase, running, remaining_seconds)
    )
}

fn fnv1a64(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

#[cfg(target_os = "linux")]
fn stable_machine_id() -> String {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(value) = fs::read_to_string(path) {
            let value = value.trim();
            if !value.is_empty() {
                return value.into();
            }
        }
    }
    std::env::var("HOSTNAME").unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn stable_machine_id() -> String {
    std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|stdout| {
            stdout.lines().find_map(|line| {
                let value = line.split("IOPlatformUUID").nth(1)?.split('=').nth(1)?.trim();
                Some(value.trim_matches('"').to_string())
            })
        })
        .unwrap_or_default()
}

#[cfg(target_os = "windows")]
fn stable_machine_id() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_default()
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn stable_machine_id() -> String {
    String::new()
}

#[cfg(target_os = "macos")]
fn enable_native_fullscreen(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};

    if let Ok(ns_window) = window.ns_window() {
        unsafe {
            let ns_window: &NSWindow = &*ns_window.cast();
            let behavior =
                ns_window.collectionBehavior() | NSWindowCollectionBehavior::FullScreenPrimary;
            ns_window.setCollectionBehavior(behavior);
        }
    }
}

#[tauri::command]
fn get_device_identity() -> DeviceIdentity {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let machine_id = stable_machine_id();
    let fingerprint_source = format!(
        "study-tracker-social-device-v1\nos={os}\narch={arch}\nmachine={machine_id}"
    );
    DeviceIdentity {
        fingerprint_hash: fnv1a64(&fingerprint_source),
        label: format!("{os} {arch} {}", runtime_channel()),
    }
}

fn setup_timer_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItem::with_id(app, "show", "Show Study Tracker", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    TrayIconBuilder::with_id(TIMER_TRAY_ID)
        .icon(tray_icon_for_phase("idle"))
        .tooltip("Study Tracker - Idle")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(window) = tray.app_handle().get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[tauri::command]
fn set_timer_tray_state(
    app: AppHandle,
    phase: String,
    running: bool,
    remaining_seconds: u32,
    verified_session_active: bool,
) -> Result<(), String> {
    if let Ok(mut active) = VERIFIED_SESSION_ACTIVE
        .get_or_init(|| Mutex::new(false))
        .lock()
    {
        *active = verified_session_active;
    }

    let Some(tray) = app.tray_by_id(TIMER_TRAY_ID) else {
        return Ok(());
    };
    let icon_phase = if phase == "idle" {
        "idle"
    } else {
        phase.as_str()
    };
    let icon_phase = icon_phase.to_string();
    let icon_phase_lock = LAST_TRAY_ICON_PHASE.get_or_init(|| Mutex::new(None));
    let should_update_icon = icon_phase_lock
        .lock()
        .map(|current| current.as_deref() != Some(icon_phase.as_str()))
        .unwrap_or(true);
    if should_update_icon {
        tray.set_icon(Some(tray_icon_for_phase(&icon_phase)))
            .map_err(|error| error.to_string())?;
        if let Ok(mut current) = icon_phase_lock.lock() {
            *current = Some(icon_phase);
        }
    }
    tray.set_tooltip(Some(timer_tray_tooltip(&phase, running, remaining_seconds)))
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn sanitize_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            _ => character,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn canonical_existing_dir(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err(format!("{label} does not exist or is not a folder."));
    }
    path.canonicalize()
        .map_err(|error| format!("Could not resolve {label}: {error}"))
}

fn ensure_path_inside(root: &Path, path: &Path) -> Result<(), String> {
    let root = canonical_existing_dir(root, "Vault folder")?;
    let candidate = if path.exists() {
        path.canonicalize()
            .map_err(|error| format!("Could not resolve file path: {error}"))?
    } else {
        let parent = path
            .parent()
            .ok_or("Could not resolve file parent folder.")?;
        let parent = parent
            .canonicalize()
            .map_err(|error| format!("Could not resolve file parent folder: {error}"))?;
        parent.join(
            path.file_name()
                .ok_or("Could not resolve file name.")?,
        )
    };

    if candidate.starts_with(&root) {
        Ok(())
    } else {
        Err("Requested file is outside the selected vault.".into())
    }
}

#[tauri::command]
fn create_obsidian_vault(base_path: String, vault_name: String) -> Result<String, String> {
    let vault_name = sanitize_segment(&vault_name);
    if vault_name.is_empty() {
        return Err("Vault name cannot be empty.".into());
    }

    let vault_path = PathBuf::from(base_path).join(vault_name);
    fs::create_dir_all(&vault_path).map_err(|error| error.to_string())?;

    for directory in [".obsidian", "Daily", "Notes", "References", "Summaries"] {
        fs::create_dir_all(vault_path.join(directory)).map_err(|error| error.to_string())?;
    }

    let readme = "# Study Tracker Vault\n\nThis vault was created by Study Tracker.\n\nUse Notes for your own markdown notes, References for course links, and Summaries for PDFs, formula sheets, and cheatsheet images.\n";
    fs::write(vault_path.join("README.md"), readme).map_err(|error| error.to_string())?;

    Ok(vault_path.to_string_lossy().to_string())
}

fn daily_note_path(vault_path: &str, note_date: &str) -> PathBuf {
    Path::new(vault_path)
        .join("Daily")
        .join(format!("{}.md", sanitize_segment(note_date)))
}

fn notes_dir_path(vault_path: &str) -> PathBuf {
    Path::new(vault_path).join("Notes")
}

fn note_path(vault_path: &str, note_title: &str) -> Result<PathBuf, String> {
    let note_title = sanitize_segment(note_title);
    if note_title.is_empty() {
        return Err("Note title cannot be empty.".into());
    }

    Ok(notes_dir_path(vault_path).join(format!("{note_title}.md")))
}

fn reference_note_path(
    vault_path: &str,
    semester_name: &str,
    course_name: &str,
) -> Result<PathBuf, String> {
    let semester_name = sanitize_segment(semester_name);
    let course_name = sanitize_segment(course_name);

    if semester_name.is_empty() || course_name.is_empty() {
        return Err("Semester and course are required.".into());
    }

    Ok(Path::new(vault_path)
        .join("References")
        .join(semester_name)
        .join(format!("{}.md", course_name)))
}

fn summaries_dir_path(
    vault_path: &str,
    semester_name: &str,
    course_name: &str,
) -> Result<PathBuf, String> {
    let semester_name = sanitize_segment(semester_name);
    let course_name = sanitize_segment(course_name);

    if semester_name.is_empty() || course_name.is_empty() {
        return Err("Semester and course are required.".into());
    }

    Ok(Path::new(vault_path)
        .join("Summaries")
        .join(semester_name)
        .join(course_name))
}

fn supported_summary_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    match extension.as_str() {
        "pdf" | "png" | "jpg" | "jpeg" | "webp" => Some(extension),
        _ => None,
    }
}

fn summary_file_kind(extension: &str) -> String {
    if extension == "pdf" {
        "pdf".into()
    } else {
        "image".into()
    }
}

fn unique_summary_destination(directory: &Path, file_name: &str) -> PathBuf {
    let original = Path::new(file_name);
    let stem = original
        .file_stem()
        .map(|value| sanitize_segment(&value.to_string_lossy()))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "summary".into());
    let extension = original
        .extension()
        .map(|value| value.to_string_lossy().to_string())
        .filter(|value| !value.is_empty());

    let mut candidate = directory.join(file_name);
    let mut index = 2;
    while candidate.exists() {
        let next_name = match &extension {
            Some(extension) => format!("{}-{}.{}", stem, index, extension),
            None => format!("{}-{}", stem, index),
        };
        candidate = directory.join(next_name);
        index += 1;
    }

    candidate
}

#[tauri::command]
fn link_obsidian_vault(vault_path: String) -> Result<String, String> {
    let root = PathBuf::from(&vault_path);
    if !root.exists() {
        return Err("Selected vault folder does not exist.".into());
    }
    if !root.is_dir() {
        return Err("Selected path is not a folder.".into());
    }

    for directory in [".obsidian", "Daily", "Notes", "References", "Summaries"] {
        fs::create_dir_all(root.join(directory)).map_err(|error| error.to_string())?;
    }

    Ok(root.to_string_lossy().to_string())
}

#[tauri::command]
fn read_daily_note(vault_path: String, note_date: String) -> Result<Option<String>, String> {
    let root = PathBuf::from(&vault_path);
    let note_path = daily_note_path(&vault_path, &note_date);
    ensure_path_inside(&root, &note_path)?;
    if !note_path.exists() {
        return Ok(None);
    }

    fs::read_to_string(note_path)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_daily_note(
    vault_path: String,
    note_date: String,
    content: String,
) -> Result<String, String> {
    let root = PathBuf::from(vault_path);
    let daily_dir = root.join("Daily");
    fs::create_dir_all(&daily_dir).map_err(|error| error.to_string())?;

    let note_path = daily_dir.join(format!("{}.md", sanitize_segment(&note_date)));
    ensure_path_inside(&root, &note_path)?;
    fs::write(&note_path, content).map_err(|error| error.to_string())?;

    Ok(note_path.to_string_lossy().to_string())
}

#[tauri::command]
fn list_notes(vault_path: String) -> Result<Vec<VaultNoteFile>, String> {
    let root = PathBuf::from(&vault_path);
    let notes_dir = notes_dir_path(&vault_path);
    fs::create_dir_all(&notes_dir).map_err(|error| error.to_string())?;
    ensure_path_inside(&root, &notes_dir)?;

    let mut notes = fs::read_dir(notes_dir)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|extension| extension.to_str()) != Some("md") {
                return None;
            }

            let title = path.file_stem()?.to_string_lossy().to_string();
            let saved_at = entry
                .metadata()
                .ok()?
                .modified()
                .ok()?
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis() as u64;
            Some(VaultNoteFile {
                title,
                path: path.to_string_lossy().to_string(),
                saved_at,
            })
        })
        .collect::<Vec<_>>();
    notes.sort_by(|left, right| right.saved_at.cmp(&left.saved_at));

    Ok(notes)
}

#[tauri::command]
fn read_note(vault_path: String, note_title: String) -> Result<Option<String>, String> {
    let root = PathBuf::from(&vault_path);
    let note_path = note_path(&vault_path, &note_title)?;
    ensure_path_inside(&root, &note_path)?;
    if !note_path.exists() {
        return Ok(None);
    }

    fs::read_to_string(note_path)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_note(vault_path: String, note_title: String, content: String) -> Result<String, String> {
    let root = PathBuf::from(&vault_path);
    let notes_dir = notes_dir_path(&vault_path);
    fs::create_dir_all(&notes_dir).map_err(|error| error.to_string())?;
    let note_path = note_path(&vault_path, &note_title)?;
    ensure_path_inside(&root, &note_path)?;
    fs::write(&note_path, content).map_err(|error| error.to_string())?;

    Ok(note_path.to_string_lossy().to_string())
}

#[tauri::command]
fn delete_note(vault_path: String, note_title: String) -> Result<(), String> {
    let root = PathBuf::from(&vault_path);
    let note_path = note_path(&vault_path, &note_title)?;
    ensure_path_inside(&root, &note_path)?;
    if note_path.exists() {
        fs::remove_file(note_path).map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn read_reference_note(
    vault_path: String,
    semester_name: String,
    course_name: String,
) -> Result<Option<String>, String> {
    let root = PathBuf::from(&vault_path);
    let note_path = reference_note_path(&vault_path, &semester_name, &course_name)?;
    if let Some(parent) = note_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    ensure_path_inside(&root, &note_path)?;
    if !note_path.exists() {
        return Ok(None);
    }

    fs::read_to_string(note_path)
        .map(Some)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn write_reference_note(
    vault_path: String,
    semester_name: String,
    course_name: String,
    content: String,
) -> Result<String, String> {
    let root = PathBuf::from(&vault_path);
    let note_path = reference_note_path(&vault_path, &semester_name, &course_name)?;
    let reference_dir = note_path
        .parent()
        .ok_or("Could not resolve reference folder.")?;
    fs::create_dir_all(reference_dir).map_err(|error| error.to_string())?;
    ensure_path_inside(&root, &note_path)?;
    fs::write(&note_path, content).map_err(|error| error.to_string())?;

    Ok(note_path.to_string_lossy().to_string())
}

#[tauri::command]
fn list_summary_files(
    vault_path: String,
    semester_name: String,
    course_name: String,
) -> Result<Vec<SummaryFile>, String> {
    let root = PathBuf::from(&vault_path);
    let summaries_dir = summaries_dir_path(&vault_path, &semester_name, &course_name)?;
    fs::create_dir_all(&summaries_dir).map_err(|error| error.to_string())?;
    ensure_path_inside(&root, &summaries_dir)?;
    if !summaries_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(&summaries_dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(extension) = supported_summary_extension(&path) else {
            continue;
        };
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_else(|| "summary".into());

        files.push(SummaryFile {
            name,
            path: path.to_string_lossy().to_string(),
            kind: summary_file_kind(&extension),
            extension,
            size_bytes: metadata.len(),
        });
    }

    files.sort_by_key(|file| file.name.to_lowercase());
    Ok(files)
}

#[tauri::command]
fn import_summary_files(
    vault_path: String,
    semester_name: String,
    course_name: String,
    file_paths: Vec<String>,
) -> Result<Vec<SummaryFile>, String> {
    let root = PathBuf::from(&vault_path);
    let summaries_dir = summaries_dir_path(&vault_path, &semester_name, &course_name)?;
    fs::create_dir_all(&summaries_dir).map_err(|error| error.to_string())?;
    ensure_path_inside(&root, &summaries_dir)?;

    for source in file_paths {
        let source_path = PathBuf::from(&source);
        if !source_path.is_file() {
            return Err(format!("{} is not a file.", source));
        }
        if supported_summary_extension(&source_path).is_none() {
            return Err(
                "Only PDF, PNG, JPG, JPEG, and WEBP files can be added to summaries.".into(),
            );
        }

        let file_name = source_path
            .file_name()
            .map(|value| sanitize_segment(&value.to_string_lossy()))
            .filter(|value| !value.is_empty())
            .ok_or("Could not read the selected file name.")?;
        let destination = unique_summary_destination(&summaries_dir, &file_name);
        fs::copy(&source_path, &destination).map_err(|error| error.to_string())?;
    }

    list_summary_files(vault_path, semester_name, course_name)
}

#[tauri::command]
fn read_summary_pdf(vault_path: String, path: String) -> Result<Vec<u8>, String> {
    let summaries_root = PathBuf::from(&vault_path).join("Summaries");
    let file_path = PathBuf::from(&path);
    ensure_path_inside(&summaries_root, &file_path)?;
    if !file_path.is_file() {
        return Err("Selected summary file does not exist.".into());
    }
    if file_path
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .as_deref()
        != Some("pdf")
    {
        return Err("Only PDF summary files can be opened in the PDF viewer.".into());
    }

    fs::read(file_path).map_err(|error| error.to_string())
}

#[tauri::command]
fn export_daily_note(
    vault_path: String,
    note_date: String,
    content: String,
) -> Result<String, String> {
    let root = PathBuf::from(vault_path);
    let daily_dir = root.join("Daily");
    fs::create_dir_all(&daily_dir).map_err(|error| error.to_string())?;

    let file_name = format!("{}.md", sanitize_segment(&note_date));
    let note_path = daily_dir.join(file_name);
    ensure_path_inside(&root, &note_path)?;
    fs::write(&note_path, content).map_err(|error| error.to_string())?;

    Ok(note_path.to_string_lossy().to_string())
}

#[tauri::command]
fn get_update_install_support() -> UpdateInstallSupport {
    let runtime_channel = runtime_channel();
    if runtime_channel == "development" {
        return UpdateInstallSupport {
            can_auto_install: false,
            package_hint: "development".into(),
            runtime_channel,
            message: "Updates are disabled in development builds. Update with: git pull, npm install, then npm run tauri:dev.".into(),
        };
    }

    if runtime_channel != "official-release" {
        return UpdateInstallSupport {
            can_auto_install: false,
            package_hint: "source-build".into(),
            runtime_channel,
            message: "Updates are disabled for source builds. Update by pulling the repository and rebuilding from source.".into(),
        };
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        return UpdateInstallSupport {
            can_auto_install: true,
            package_hint: "native".into(),
            runtime_channel,
            message: "Automatic updates are supported for this install.".into(),
        };
    }

    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("APPIMAGE").is_some() {
            return UpdateInstallSupport {
                can_auto_install: true,
                package_hint: "appimage".into(),
                runtime_channel,
                message: "Automatic updates are supported for this AppImage install.".into(),
            };
        }

        if linux_distribution_family() == "source" {
            return UpdateInstallSupport {
                can_auto_install: false,
                package_hint: "source-linux".into(),
                runtime_channel,
                message: "A new version is available. For Arch, Hyprland-heavy setups, and unsupported Linux distributions, update by pulling the repository and rebuilding from source.".into(),
            };
        }

        return UpdateInstallSupport {
            can_auto_install: false,
            package_hint: "manual-linux".into(),
            runtime_channel,
            message: "Automatic installation is disabled for Linux .deb and .rpm installs. Download the matching package, then run the install command shown by the app.".into(),
        };
    }

    #[allow(unreachable_code)]
    UpdateInstallSupport {
        can_auto_install: false,
        package_hint: "unsupported".into(),
        runtime_channel,
        message: "Automatic installation is not supported on this platform.".into(),
    }
}

fn runtime_channel() -> String {
    if cfg!(debug_assertions) {
        return "development".into();
    }

    if option_env!("STUDY_TRACKER_OFFICIAL_RELEASE") == Some("1") {
        return "official-release".into();
    }

    "source-build".into()
}

#[cfg(target_os = "linux")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(target_os = "linux")]
fn linux_distribution_family() -> String {
    let os_release = fs::read_to_string("/etc/os-release")
        .unwrap_or_default()
        .to_lowercase();

    if os_release.contains("debian")
        || os_release.contains("ubuntu")
        || os_release.contains("linuxmint")
        || os_release.contains("pop")
    {
        return "deb".into();
    }

    if os_release.contains("fedora")
        || os_release.contains("rhel")
        || os_release.contains("centos")
        || os_release.contains("opensuse")
        || os_release.contains("suse")
    {
        return "rpm".into();
    }

    "source".into()
}

#[cfg(target_os = "linux")]
fn file_name_from_url(url: &str) -> Result<String, String> {
    url.rsplit('/')
        .next()
        .filter(|file_name| !file_name.trim().is_empty())
        .map(|file_name| file_name.to_string())
        .ok_or("Could not determine update file name.".into())
}

#[tauri::command]
async fn download_linux_update_package() -> Result<LinuxUpdateDownload, String> {
    #[cfg(not(target_os = "linux"))]
    {
        return Err("Manual package downloads are only available on Linux.".into());
    }

    #[cfg(target_os = "linux")]
    {
        let latest = reqwest::get(LATEST_UPDATE_JSON_URL)
            .await
            .map_err(|error| format!("Could not fetch update metadata: {error}"))?
            .error_for_status()
            .map_err(|error| format!("GitHub returned an update metadata error: {error}"))?
            .json::<LatestUpdateJson>()
            .await
            .map_err(|error| format!("Could not parse update metadata: {error}"))?;

        let family = linux_distribution_family();
        if family == "source" {
            return Err("For Arch, Hyprland-heavy setups, and unsupported Linux distributions, update by running: git pull && npm install && npm run tauri:build".into());
        }

        let platform_key = match family.as_str() {
            "deb" => "linux-x86_64-deb",
            "rpm" => "linux-x86_64-rpm",
            _ => "linux-x86_64-deb",
        };

        let platform = latest
            .platforms
            .get(platform_key)
            .or_else(|| latest.platforms.get("linux-x86_64"))
            .ok_or("No Linux update package was found in the latest release.".to_string())?;

        let file_name = file_name_from_url(&platform.url)?;
        let home = std::env::var("HOME")
            .map_err(|_| "Could not determine your home folder.".to_string())?;
        let downloads_dir = PathBuf::from(home).join("Downloads");
        fs::create_dir_all(&downloads_dir)
            .map_err(|error| format!("Could not create Downloads folder: {error}"))?;
        let file_path = downloads_dir.join(file_name);

        let mut response = reqwest::get(&platform.url)
            .await
            .map_err(|error| format!("Could not download update package: {error}"))?
            .error_for_status()
            .map_err(|error| format!("GitHub returned a download error: {error}"))?;

        let mut file = fs::File::create(&file_path)
            .map_err(|error| format!("Could not save update package: {error}"))?;
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("Could not read update package download: {error}"))?
        {
            file.write_all(&chunk)
                .map_err(|error| format!("Could not save update package: {error}"))?;
        }

        let file_path_string = file_path.to_string_lossy().to_string();
        let quoted_path = shell_quote(&file_path_string);
        let install_command = match family.as_str() {
            "deb" => format!("sudo apt install {quoted_path}"),
            "rpm" => {
                let os_release = fs::read_to_string("/etc/os-release")
                    .unwrap_or_default()
                    .to_lowercase();
                if os_release.contains("opensuse") || os_release.contains("suse") {
                    format!("sudo zypper install {quoted_path}")
                } else {
                    format!("sudo dnf install {quoted_path}")
                }
            }
            _ => format!("sudo apt install {quoted_path}"),
        };

        let message = match family.as_str() {
            "deb" => "Downloaded the Debian/Ubuntu package. Run the command below to install it.",
            "rpm" => "Downloaded the RPM package. Run the command below to install it.",
            _ => "Downloaded the Linux package. Run the command below to install it.",
        };

        Ok(LinuxUpdateDownload {
            version: latest.version,
            package_type: family,
            file_path: file_path_string,
            install_command,
            message: message.into(),
        })
    }
}

#[tauri::command]
async fn native_social_sync(endpoint: String, body: String) -> Result<NativeHttpResponse, String> {
    let response = reqwest::Client::new()
        .post(endpoint)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|error| format!("Native social sync request failed: {error}"))?;
    let status = response.status().as_u16();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read native social sync response: {error}"))?;
    Ok(NativeHttpResponse { status, body })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(not(target_os = "macos"))]
            if let Some(window) = app.get_webview_window("main") {
                window.set_decorations(false)?;
            }
            setup_timer_tray(app.handle())?;
            #[cfg(target_os = "macos")]
            if let Some(window) = app.get_webview_window("main") {
                enable_native_fullscreen(&window);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };
            let session_active = VERIFIED_SESSION_ACTIVE
                .get_or_init(|| Mutex::new(false))
                .lock()
                .map(|guard| *guard)
                .unwrap_or(false);
            if !session_active {
                return;
            }
            // A study session is running — hide instead of quitting so it isn't interrupted.
            // The tray's own "Quit" menu item remains the real, visible way to fully exit.
            api.prevent_close();
            let _ = window.hide();

            let already_shown = TRAY_CLOSE_NOTICE_SHOWN.get_or_init(|| Mutex::new(false));
            let should_notify = already_shown
                .lock()
                .map(|mut shown| {
                    let first_time = !*shown;
                    *shown = true;
                    first_time
                })
                .unwrap_or(false);
            if should_notify {
                let _ = window
                    .app_handle()
                    .notification()
                    .builder()
                    .title("Study Tracker is still running")
                    .body("Your session keeps tracking in the tray. Right-click the tray icon to quit.")
                    .show();
            }
        })
        .invoke_handler(tauri::generate_handler![
            create_obsidian_vault,
            link_obsidian_vault,
            read_daily_note,
            write_daily_note,
            list_notes,
            read_note,
            write_note,
            delete_note,
            read_reference_note,
            write_reference_note,
            list_summary_files,
            import_summary_files,
            read_summary_pdf,
            export_daily_note,
            get_update_install_support,
            download_linux_update_package,
            native_social_sync,
            get_device_identity,
            set_timer_tray_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
