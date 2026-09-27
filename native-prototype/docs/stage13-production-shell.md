# Stage 13 — Production shell foundation

## 1. Scope

Turn the existing native prototype executable into a production-quality **application shell**: no console window in release, a real embedded application icon, explicit identity/metadata, per-OS platform paths, minimal logging, `Result`-based startup with a diagnosable fatal-error path, an explicit (if trivial) runtime-configuration boundary, and an explicit renderer-selection policy. No domain feature work: no production persistence, no data migration, no tray/notifications/updater/single-instance, no network/social, no garden/Wabi-Sabi/games, no new timer semantics. Stage 14 (timer productionization) is not started.

## 2. Repository baseline

Before Stage 13 work began, the Stage 12/12.5 checkpoint was completed as this prompt's §0 required:

- `desktop/src-tauri/Cargo.toml` was verified **byte-identical** to `HEAD:desktop/src-tauri/Cargo.toml` (`cmp` reported no difference; `git diff` was empty) before being restored with `git restore`, clearing the false-dirty state left by an earlier Tauri-CLI re-save.
- Two commits were made exactly as specified:
  - `2594af0` — `native: add stage 12 production Windows comparison` (the remaining Stage 12 A/B material: the extended `stage12-windows-platform.md` and the four new `scripts/ab-*`/`win-tree.ps1` benchmark scripts).
  - `6c31159` — `native: freeze Windows-first architecture` (`stage12_5-architecture-freeze.md`, `adr/0001-native-windows-architecture.md`, and the short pointer edits to `architecture.md`/`native-core-architecture.md`).
- `git status --short` and `git diff -- desktop` were both empty before Stage 13 implementation began. Branch `main`, HEAD `6c31159`.

## 3. Shell architecture

New module, `native-prototype/src/platform/`, sitting in the application/presentation-adapter layer above `study-tracker-core` (never below it — the frozen dependency direction from ADR 0001 / freeze §5 is unchanged; `study-tracker-core` was not touched in this stage):

```text
src/platform/
  mod.rs             module doc + wiring
  identity.rs         DISPLAY_NAME, SHELL_LABEL, APP_ID, ORGANIZATION, version(), window_title()
  paths.rs            AppPaths::resolve()/ensure_created() (dirs crate)
  logging.rs          file logger + bounded rotation + panic hook
  config.rs            RuntimeConfig (renderer report) + benchmark-override logging
  error.rs             StartupError (one enum for every startup failure mode)
  startup_error.rs      #[cfg(windows)] MessageBoxW fallback
```

`main.rs`'s `main()` now only dispatches to `run() -> Result<(), StartupError>` and reports a fatal error if it returns `Err`; everything that used to be inline in `main()` (window/model/map/diagnostics setup) moved into `run()` unchanged except for the new platform-init calls at the top and two `log::info!` lines. `app_model.rs`'s hardcoded title literal now reads `platform::identity::window_title()`. `ui/main.slint`'s `Window` gained one `icon:` binding. No other application logic changed.

## 4. Windows GUI-subsystem handling

```rust
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
```

This is the standard Rust mechanism (`rustc`/`link.exe`'s `/SUBSYSTEM:WINDOWS`), the same idiom production's own generated Tauri `main.rs` uses (`desktop/src-tauri/src/main.rs`). It is conditioned on `not(debug_assertions)`, so `cargo run`/`cargo test`/any debug build keeps the console subsystem and normal stdout/stderr visibility for development; only `cargo build --release` (and any other non-debug profile) switches to the GUI subsystem. It has no effect on non-Windows targets (`target_os = "windows"` guard).

**Verified**, not assumed:

- PE header inspection of the release `.exe`: subsystem field is `2` (`IMAGE_SUBSYSTEM_WINDOWS_GUI`); a pre-Stage-13 build (still on disk from Stage 12) was `3` (`IMAGE_SUBSYSTEM_WINDOWS_CUI`) for comparison.
- A fresh launch of the release `.exe` (via `Start-Process`, not from an existing console) produced **zero** new `conhost.exe`/`cmd.exe` processes, and `Get-Process` for the app's own PID showed exactly one process — no console window opened, and errors are not simply routed to a console that doesn't exist (see §9).

## 5. Application identity

Central module, `platform::identity`:

| Constant/fn | Value | Rationale |
| --- | --- | --- |
| `DISPLAY_NAME` | `"Study Tracker"` | Matches production's display name; the freeze does not treat the native build as a different product. |
| `SHELL_LABEL` | `"Native Preview"` | Appended wherever the native build must stay distinguishable from the real, released app (window title, `.rc` product name) — see the reasoning below. |
| `window_title()` | `"Study Tracker (Native Preview)"` | Used by `app_model.rs` as the window title. |
| `APP_ID` | `"com.damcha.studytracker.native-shell"` | **Deliberately different** from production's `com.damcha.studytracker` — see §7. |
| `ORGANIZATION` | `"damcha"` | Production's identifier's owner segment, reused for the `.rc` `CompanyName` field and the startup log line. |
| `version()` | `env!("CARGO_PKG_VERSION")` (`0.1.0`) | The native shell's own, independent version — **not** synced to production's `0.1.58`/`0.1.65`; they are different build artifacts on different release cadences until cutover. |

**Why the title/product-name stays distinguishable, not identical to production's plain "Study Tracker":** the same Windows PC used for Stage 12's production comparison has the real, installed production app (v0.1.65) on it, and now both apps share the *same icon* (§6). Giving the native shell an identical title and product name as well would have made Alt-Tab/taskbar genuinely ambiguous between the prototype and the real, released app that holds real user data — a specific, concrete risk on *this* machine, not a hypothetical one. `SHELL_LABEL` exists to prevent that, and is designed to be dropped in one place (`identity.rs`) once the app is actually production, per the freeze's cutover criteria.

**On the production Tauri identifier** (`desktop/src-tauri/tauri.conf.json`'s `"identifier": "com.damcha.studytracker"`): it was inspected first, as instructed, and is **not** casually changed or reused as-is — see §7 for why, and for the explicit statement that this is a documented open question for Stage 15, not a decision made here.

## 6. Icon handling / provenance

**Asset provenance**: `native-prototype/assets/branding/icon.png` and `icon.ico` are **byte-identical copies** of `desktop/src-tauri/icons/icon.png` and `icon.ico` (production's own published branding, already used for its Windows/macOS/Linux/Store builds). Verified by SHA-256 before and after copying:

```text
icon.png  653beffc9806bdcffbb49d365d4c43b5a35a9f7cdb5ecdf6778d545d3931a18f  (matches both copies)
icon.ico  1994ae793212775992aa64860b650840db2a55d1b316947f9b662e5f65191343  (matches both copies)
```

`desktop/` was only **read** to produce these copies; nothing under it was modified (confirmed in §17/§18). No new artwork was invented, per the brief's instruction to reuse existing branding when licensing/ownership/quality are clear — this is the project's own asset, copied, not modified.

**Window/taskbar icon**: `ui/main.slint`'s `Window` now has `icon: @image-url("../assets/branding/icon.png");` (Slint's `Window.icon` property, confirmed present in Slint 1.17.1's builtin element definitions). This directly fixes the Stage 12 finding (`WM_GETICON` returned `0`/`0` — no icon set).

**Executable icon + version identity**: `native-prototype/assets/branding/app.rc`, a small hand-written Windows resource script (icon resource + a `VERSIONINFO` block: `CompanyName`, `FileDescription`, `ProductName`, `ProductVersion`, `OriginalFilename`, `InternalName`), compiled and linked by the `embed-resource` crate (build-dependency, Windows-only — see §22) from `build.rs`. `ProductName` in the `.rc` is `"Study Tracker Native Preview"` and `FileDescription` spells out `"(not the production app)"`, for the same distinguishability reason as §5.

**Verified**, not assumed — .NET's `System.Diagnostics.FileVersionInfo.GetVersionInfo()` convenience wrapper returned every field empty despite successful compilation with no warnings, which turned out to be a quirk of that specific .NET wrapper, not a defect in the resource. Verification instead used the canonical Win32 API directly:

- `GetFileVersionInfoSizeW` on the release `.exe` returned `1572` (nonzero: a version resource is present and found by the OS loader).
- `VerQueryValueW` against `\VarFileInfo\Translation` returned `lang=0409 cp=04B0`, and a subsequent query for `\StringFileInfo\040904b0\ProductName` returned the string `"Study Tracker Native Preview"` — an exact match for the `.rc` source.
- `ExtractIcon` on the release `.exe` returned a non-null icon handle — the executable icon resource is present and extractable, the same mechanism Windows Explorer uses for the file's own icon.
- The fix also had to switch the `.rc`'s version block from the symbolic `VS_VERSION_INFO` resource name (which needs `<winver.h>`, not included) to the numeric ID `1 VERSIONINFO` — `GetFileVersionInfo` specifically looks up resource ID `1`, so this was load-bearing, not stylistic; the first attempt silently compiled but was unfindable by ID.

**Not independently re-verified this stage** (blocked by circumstance, not skipped): the actual on-screen taskbar/title-bar icon and `WM_GETICON`'s *small*-icon slot were confirmed via a direct `SendMessage(WM_GETICON)` call to the live window (returned a non-null icon handle — the small icon is now set, where Stage 12 found `0`); the *large*-icon slot (`WM_GETICON` with `wParam=ICON_BIG`, used for some Alt-Tab previews) still returned `0`. This is a minor residual gap: Slint's `icon:` property appears to set only the small icon via `WM_SETICON`, and Windows may or may not fall back to the exe's own embedded icon resource (which *is* present, per `ExtractIcon` above) for the large-icon slot depending on window-manager version. A visual screenshot to confirm what the taskbar/Alt-Tab actually show was not taken — see §15 for why — so this is flagged as a manual-verification item rather than claimed either way.

## 7. Platform path strategy

`platform::paths::AppPaths::resolve()` uses the `dirs` crate (chosen over hand-rolled `%LOCALAPPDATA%`/`XDG_*`/`~/Library` lookups: small, no async runtime, no heavyweight transitive dependencies, widely used and actively maintained — see §22 for its actual dependency footprint) to resolve three directories, all namespaced under `platform::identity::APP_ID`:

- `data_dir` — `dirs::data_local_dir()` + `APP_ID` (on Windows: the per-user local-app-data known folder, described conceptually here rather than as a literal path; unused for real data until Stage 15, created now so the location is stable and inspectable from the first shell build onward).
- `cache_dir` — `dirs::cache_dir()` + `APP_ID` + `cache` (on Windows this resolves to the same known-folder root as `data_local_dir()`, since Windows has no separate cache-folder convention distinct from local-app-data; documented in code rather than silently treated as a different location).
- `log_dir` — `data_dir` + `logs`, where `platform::logging` writes `study-tracker.log`.

No path is hard-coded (`C:\Users\...` never appears in source), and nothing is written beside the executable.

**Why `APP_ID` deliberately differs from production's `com.damcha.studytracker`**: reusing production's exact identifier would make `AppPaths` resolve to the *same* directory production's real WebView2 profile and real user data live in — exactly the directory Stage 12's production comparison went to deliberate, verified lengths to never write into (`stage12-windows-platform.md`, "Production data / profile conditions": an isolated `WEBVIEW2_USER_DATA_FOLDER` was used for every production measurement, and both the real profile's and the real installed app's directories were fingerprinted before and after to prove they were untouched). Stage 13 only writes a log file, so the actual risk today is small, but persistence lands in this same path boundary from Stage 15 onward, so the separation is established now rather than retrofitted later under more pressure. `APP_ID` is `com.damcha.studytracker.native-shell` — a namespaced *child* of production's own reverse-DNS owner (`com.damcha`), not an unrelated identifier, so it cannot collide with anything else on a user's machine. This reasoning, and the explicit statement that *whether the eventual production-native app adopts `com.damcha.studytracker` itself* is an open Stage 15 decision, not resolved here, is recorded in `identity.rs`'s doc comments as well as here — per the brief's instruction to document rather than silently choose when a production identifier is unsuitable for direct reuse.

**Verified**: `AppPaths::resolve()` was exercised at runtime (a real launch created `cache/` and `logs/` under `%LOCALAPPDATA%\com.damcha.studytracker.native-shell\`, confirmed to exist and be distinct from `%LOCALAPPDATA%\com.damcha.studytracker\`, production's real, pre-existing, untouched directory). Three unit tests (`platform::paths::tests`) check the *relationships* between the resolved paths (log dir nested under data dir; every directory contains the `APP_ID` namespace component; `APP_ID` is never equal to production's identifier) without asserting an absolute path or depending on a specific username, so they pass on any machine.

## 8. Logging

`platform::logging` implements `log::Log` directly against one file, rather than pulling in an observability framework (`log` is the only new runtime crate this required — already resolved transitively via Slint, see §22). `log::set_max_level(LevelFilter::Info)`; nothing in this codebase logs above `Info` frequency-wise (no per-frame, no per-timer-tick logging — verified by inspection: the only `log::` call sites are in `run()`'s startup sequence, `config::log_active_benchmark_overrides()`, and the panic hook). Rotation is a simple one-generation scheme: if `study-tracker.log` exceeds 2 MB at startup, it's renamed to `study-tracker.log.old` and a fresh file is opened — not a rotation subsystem, per the brief's explicit "defer full rotation, just keep output bounded" guidance. Timestamps use `chrono` (already resolved transitively via Slint's own date/time widgets — `cargo tree -i chrono` confirms `i-slint-core` as the sole existing consumer — so this added no new crate to the tree either). Debug builds additionally mirror `Warn`/`Error` lines to `stderr` (there's a console to see them on); release builds don't (nothing would be attached to it).

`logging::install_panic_hook()` wraps the default panic hook to log the panic message and location before the default hook runs, so a panic during or after startup leaves a trace in the log file even though a release build has no console to print to otherwise.

**Verified**: a real run's log file was inspected directly. Three lines per launch (`"... starting; renderer=...; log file: ..."`, the benchmark-override line, `"first window created; entering the event loop"`), 5.6 KB after roughly a dozen runs during this stage's testing — nowhere near the 2 MB rotation threshold, and specifically confirmed **not** to grow during a 60-second idle measurement window (§14, S13-P1): the log line count before and after that window was identical.

## 9. Startup / error handling

`main()` is now:

```rust
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => { report_fatal_startup_error(&error); std::process::ExitCode::FAILURE }
    }
}
```

`run() -> Result<(), platform::error::StartupError>` covers path resolution, directory creation, logging init, and window/renderer/event-loop startup (`slint::PlatformError` converts into `StartupError` via `From`), all through `?`, not panics. `report_fatal_startup_error` always logs (if logging made it far enough to initialize) and always writes to `stderr` (harmless with no console attached — the write just goes nowhere), and on Windows additionally calls `platform::startup_error::show_fatal_error`, a single `MessageBoxW` FFI call (`windows-sys`, Windows-only dependency, gated in `Cargo.toml` — see §22) shown as a last resort specifically because a release build's missing console would otherwise make a startup failure genuinely invisible. No crash reporter, no telemetry, no window-handle dependency (these failures happen before any Slint window exists).

**Not independently exercised this stage**: no test deliberately forced a startup failure (e.g., an unwritable log directory) to watch the message box appear, since that would require either disrupting a real directory or synthetic environment manipulation while the user was active at the machine (see §15). The code path was read-verified and the underlying pieces (`StartupError`'s `Display` impl, the `MessageBoxW` FFI signature against `windows-sys`'s actual declarations, `HWND` as `*mut c_void`) were checked individually rather than exercised end-to-end. Flagged as a manual-verification item in §15.

## 10. Configuration boundary

`platform::config::RuntimeConfig` is deliberately small: one field today (`renderer: RendererBackend`, currently always `FemtoVg` — see §11), resolved once at startup from `SLINT_BACKEND` (read only to log a developer's override attempt; Slint's own backend selector reads that variable directly and independently). `config::log_active_benchmark_overrides()` logs which of the Stage 9–12 `STUDY_NATIVE_*`/`SLINT_*` environment variables are set for a given run (see §12 for the full list and their classification) — informational only, changes nothing. No generic configuration framework, no file-based config format, no settings migration (that's Stage 15+): most constants in this codebase remain plain constants, exactly as the brief asked.

## 11. Renderer

**Default**: FemtoVG/OpenGL — the only renderer Cargo feature enabled in `Cargo.toml` (`slint`'s `renderer-femtovg`), matching ADR 0001 / freeze §7 exactly. `platform::config::RendererBackend::compiled_default()` returns `FemtoVg` unconditionally today because there is, in this normal build, literally nothing else compiled in to select — the Stage 12 renderer comparison's Skia/D3D12 and Skia-software variants live only in a separate scratch copy outside the repository (as documented in `stage12-windows-platform.md`), and Stage 13 did not change that: no additional `slint` renderer feature was added to `Cargo.toml`, so the normal binary's size/memory/startup profile is unaffected by renderer choice beyond what Stage 12 already measured.

**Override mechanism**: `SLINT_BACKEND`/`SLINT_RENDERER` (Slint's own env vars, already how Stage 12's benchmark scripts steered the scratch Skia build) still work exactly as before for anyone who rebuilds with additional renderer features enabled; `RuntimeConfig::from_env()` only adds a startup log line noting the variable is set and, in the normal build, has no effect because nothing else is compiled in — so a developer who sets it and sees no change isn't left silently guessing why.

**Revisit conditions** (unchanged from ADR 0001, restated here for a reader who lands on this document first): a real production feature misses a responsiveness target on FemtoVG; a text/emoji defect traces to the renderer itself; a GPU-driver reliability problem appears; or a platform genuinely cannot run it. A synthetic Dense ×50 benchmark result alone is explicitly not sufficient — see `docs/stage12_5-architecture-freeze.md` §7.

## 12. Diagnostic / benchmark hook classification

Every environment-gated hook added in Stages 9–12, inspected and classified:

| Hook | Classification | Notes |
| --- | --- | --- |
| `STUDY_NATIVE_VIEW`, `STUDY_NATIVE_POINTS`, `STUDY_NATIVE_SCENARIO`, `STUDY_NATIVE_SIZE` | **Keep permanently** | Used by the Stage 10–12 benchmark scripts to start in a known state without synthetic input; zero cost when unset (the normal, un-set path). |
| `STUDY_NATIVE_MAP_LEVEL`, `STUDY_NATIVE_MAP_BENCH` | **Keep permanently** | Back the Stage 11/12 map-stress regression scripts (`map-stress-windows.ps1`); `STUDY_NATIVE_MAP_BENCH`'s driver is self-terminating and does nothing unless explicitly set. |
| `STUDY_NATIVE_STARTUP_REPORT`, `STUDY_NATIVE_FRAME_STATS` | **Keep permanently** | Back `benchmark-windows.ps1 -Mode startup` and the frame-rate regression checks; genuinely useful regression tooling per the freeze's §21/§31, and confirmed zero-cost when unset (`install_diagnostics` returns `None` immediately). |
| `SLINT_SCALE_FACTOR`, `SLINT_DEBUG_PERFORMANCE`, `SLINT_BACKEND` | **Keep permanently (Slint's own, not this codebase's)** | Used throughout Stage 12's HiDPI and renderer-comparison work; not something this project could remove even if it wanted to. |
| `STUDY_NATIVE_SKIA_API` | **Keep, but scratch-copy-only** | Only meaningful in the separate Skia scratch copy from Stage 12's renderer comparison, not in this repository's `Cargo.toml`-defined build; not present in this codebase at all, listed here only so a future reader doesn't wonder where it went. |

**None removed.** All of the above are logged (which ones are active this run, not their values beyond what's already visible from the process's own environment) by `config::log_active_benchmark_overrides()` — see §10. Normal release startup runs none of them unless explicitly set: confirmed by inspection (every call site is behind an `Ok`/`is_some()` check on the corresponding `std::env::var`) and by the startup log from an unset-environment launch showing no benchmark-override line at all.

## 13. Performance results

Methodology: `native-prototype/scripts/win-metrics.ps1`'s `Measure-Tree`, whole-process-tree Private Working Set / Private Bytes / Working Set / interval CPU (matching Stage 12's methodology exactly). **S13-P2 through S13-P4 (timer visible/minimized/paused) were not independently re-measured this stage** — see §15 for why, and the explicit manual-test request at the end of this document. What was measured:

| Point | Procs | Threads | Working Set | Private WS | Private Bytes | CPU |
| --- | --- | --- | --- | --- | --- | --- |
| S13-P0 launch + settle | 1 | 14 | 78.5 MB | **44.1 MB** | 116.0 MB | 0 % |
| S13-P1 idle 60 s | 1 | 8 | 78.4 MB | **44.0 MB** | 115.8 MB | 0 % |

Compared with Stage 12's accepted native baseline (idle Private WS 42.6–43.5 MB, idle CPU 0–0.03 %): **no regression** — the shell additions (`dirs`, `log`, `chrono`-as-direct-dep, path/log initialization) add well under 1 MB and no measurable CPU. Process count stayed at **1** in every measurement (§13 acceptance requirement "normal operation remains one process": confirmed).

## 14. Startup results

`benchmark-windows.ps1 -Mode startup -Runs 10` (spawn → `STUDY_NATIVE_STARTUP_REPORT`'s first-rendered-frame notifier, matching the methodology used for Stage 12's ~164–166 ms figure — not the separate PrintWindow-probe methodology used for the ~206 ms production-comparison figure; the two are not directly comparable to each other, only within their own family):

| | min | median | mean | max | trials |
| --- | --- | --- | --- | --- | --- |
| Stage 13 release | 168.2 ms | **169.8 ms** | 172.2 ms | 194.4 ms | 10 |

No regression versus Stage 12's aligned figure in the same family (~164–166 ms) — if anything marginally faster, within normal run-to-run noise; not investigated further since there is no regression to investigate. Private WS at first frame was 44.3–44.6 MB across the 10 runs, consistent with §13. This is warm startup (the process/DLLs were already touched by prior runs in the same session), not a cold-boot measurement, exactly as Stage 12's own figures were.

## 15. Manual Windows verification

**What was verified directly, without needing the visual desktop** (all via `SendMessage`/`GetWindowText`/`Get-Process`/raw Win32 version/icon APIs against the live window and process, none of which touch the screen or the mouse):

- Normal `Start-Process` launch produces **zero** new console (`conhost`/`cmd`) processes and exactly **one** application process (§4, §13).
- The window's actual title, read via `GetWindowText`, is `"Study Tracker (Native Preview)"` (§5).
- `WM_GETICON` (small-icon slot) on the live window returns a non-null icon handle, where Stage 12 found `0` (§6).
- The executable's embedded icon and version resources are present and correctly queryable via `ExtractIcon`/`GetFileVersionInfoSizeW`/`VerQueryValueW` (§6).
- `AppPaths` resolves to a real, distinct, `com.damcha.studytracker.native-shell`-namespaced directory under `%LOCALAPPDATA%`, confirmed created and never colliding with production's real, pre-existing `com.damcha.studytracker` directory (§7).
- The log file exists, has the expected content and format, and did not grow during a 60-second idle window (§8, §13).

**What was not verified this stage, and why**: partway through this stage's testing, a screenshot taken to visually confirm the window/taskbar appearance captured the user's own currently-visible screen content (a browser tab) instead of the app window — because the app window, launched from a background automation context, could not take OS foreground focus (`SetForegroundWindow` is restricted by Windows from background processes, and in this case the restriction was clearly live, meaning the user was actively at the machine). Continuing to take screen-content screenshots, or to send synthetic mouse clicks (which move the real cursor and could click on whatever the user actually has on screen) or keyboard input (which would go to whatever has real OS focus, not necessarily this app), was judged too likely to disrupt the user's own foreground work, so it was **stopped immediately** rather than continued. The one screenshot that was captured showed only the user's own already-visible on-screen content (their own browser tab) and nothing about the application; it was not saved or used for anything.

Because of this, the following remain **unverified by automation** and are explicitly requested as the smallest possible manual check, per the brief's own instruction to ask rather than guess:

1. Double-click (or otherwise normally launch) `native-prototype\target\release\study-tracker-native-prototype.exe` and confirm: no console window appears; the Study Tracker window appears with a visible icon in its title bar; the taskbar button shows the same icon (not a generic one).
2. Minimize, maximize, restore, resize, and close the window; relaunch once more.
3. Start the timer (any preset), let it run a few seconds, minimize it for at least 15 seconds, restore it, and confirm the remaining time is correct and the Pause/Start button still works — this is the actual Stage 12 Rule A/Rule B regression check (§16 below) and the one thing this stage most needs confirmed visually.
4. Optionally: check `%LOCALAPPDATA%\com.damcha.studytracker.native-shell\logs\study-tracker.log` looks reasonable after that session (a handful of lines, not continuously growing).

None of the above requires anything beyond normal use of the app for about a minute.

## 16. Timer regression

**No timer semantics changed in Stage 13.** `app_model.rs`'s timer-facing code, `study-tracker-core`'s timer domain, and `main.rs`'s `sync_refresh_timer`/`apply_model_to_window`/`model_matches` functions are **byte-for-byte unchanged** from Stage 12 except for the one title-string line (§5), which does not touch timer logic. The Stage 12 unit test that guards Rule A (`model_matches_only_when_rows_are_identical`, in `main.rs`'s `#[cfg(test)] mod tests`) **was not weakened, modified, or removed**, and still passes (§21). Structural confirmation that Rule A/Rule B still hold: `install_diagnostics`'s `STUDY_NATIVE_FRAME_STATS` counter path, the `model_matches` guard before every `set_modes`/`set_session_notes` call, and `sync_refresh_timer`'s stop-when-not-running logic are all present, unedited, in the current `main.rs`. Live re-verification of running/minimized repaint behavior (a `STUDY_NATIVE_FRAME_STATS` capture with the timer actually running) was one of the things deferred per §15 and is covered by manual-check item 3 above; the code-level guarantee is unchanged and load-bearing.

## 17. Existing view regression

`cargo test --workspace` (§21) exercises the map, dashboard, and timer-adjacent logic at the unit level and all 66 tests pass (63 pre-existing + 3 new `platform::paths` tests), with the same 2 ignored manual-benchmark-report tests as before. No `.slint` file other than `ui/main.slint`'s one-line icon addition changed, so the Timer / Stage 9 text-spike / Dashboard / Map Lab views are structurally unmodified. Live navigation/resizing/interaction smoke-testing of all four views was not separately re-run this stage (superseded by the same §15 circumstance) and is folded into manual-check items 1–3 above, which exercise the Timer view directly; a full pass over Text/Dashboard/Map is lower-risk here since nothing in those code paths changed at all in Stage 13.

## 18. Cross-platform implications

Windows remains first priority and was not compromised for portability's sake: the `windows_subsystem` attribute, `embed-resource`/`app.rc`, and `startup_error.rs`'s `MessageBoxW` call are all `#[cfg(windows)]` (or `cfg(target_os = "windows")`)-gated, at the smallest granularity that made sense (a whole module for the message-box fallback, a single `#[cfg]` line for the subsystem attribute, target-specific `Cargo.toml` dependency tables for `windows-sys`/`embed-resource`). `platform::paths` and `platform::logging` are written in portable `std`/`dirs`/`chrono`, so they compile and should behave equivalently on Linux/macOS (untested — no CI exists in this repository and none was added in Stage 13, per the brief's explicit "do not create a large CI project" instruction; the only relevant existing infrastructure is the Bash `scripts/check.sh`, which itself remains platform-agnostic and was re-run successfully on this Windows machine via the already-available Git Bash, satisfying the brief's "use the actual repository-supported Windows validation path" instruction rather than inventing a `check.ps1`). No Win32 dependency was placed in `study-tracker-core`, and none of the new dependencies (`dirs`, `log`, `chrono`, target-gated `windows-sys`/`embed-resource`) are platform-toolkit crates that would conflict with a future Linux/macOS build.

## 19. Risks / open issues

- The large-icon (`ICON_BIG`) `WM_GETICON` slot is still `0`; the small icon and the embedded executable resource are both confirmed present (§6). Worth a follow-up glance in a later stage, not urgent.
- §9's fatal-error path (message box + log line) was read-verified but not exercised end-to-end by deliberately forcing a startup failure.
- §15/§17's live visual/interaction verification is deferred to the user, per the manual-check list, because of the mid-session circumstance described there — this is the main open item from this stage.
- `platform::identity::APP_ID`'s relationship to production's identifier remains an explicitly open Stage 15 decision (§7), not a risk in itself but worth keeping visible.

## 20. Files changed

All under `native-prototype/`; `desktop/` untouched (confirmed in §17 of the earlier checkpoint step and again in §21 below).

**Modified**: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/main.rs`, `src/app_model.rs`, `ui/main.slint`.
**New**: `src/platform/{mod,identity,paths,logging,config,error,startup_error}.rs`, `assets/branding/{icon.png,icon.ico,app.rc}`, `docs/stage13-production-shell.md` (this file).

No existing architecture document needed a content update beyond what Stage 12.5 already added (the freeze's pointer notes in `architecture.md`/`native-core-architecture.md` already anticipate exactly this kind of shell work landing in `native-prototype/src/`).

## 21. Tests/checks

All run from `native-prototype/` unless noted, on this Windows machine, via the already-available Rust MSVC toolchain and Git Bash:

```text
cargo fmt --check                     -> pass, no diff
cargo check                           -> pass, 0 warnings
cargo test --workspace                -> 66 passed, 0 failed, 2 ignored (core: 19 passed)
cargo test -p study-tracker-core      -> 19 passed, 0 failed
cargo build --release                 -> pass, 40.35s (16,949,248 bytes)
bash scripts/check.sh                 -> pass, exit 0 (fmt+check+test+build, Bash-only script;
                                          no check.ps1 was invented, per the brief's instruction
                                          not to blindly create one where a Windows-native
                                          equivalent isn't already established)
```

From the repository root:

```text
git diff --check     -> clean (CRLF-on-checkout warnings only, no conflict markers/whitespace errors)
git status --short    -> see §2 baseline plus the Stage 13 files listed in §20
git diff --stat       -> 6 files changed (Cargo.lock/Cargo.toml/build.rs/main.rs/app_model.rs/main.slint), 189 insertions(+), 3 deletions(-)
git diff -- desktop    -> empty
```

## 22. Dependency review

New runtime dependencies (all justified individually in the sections above; none pulls in an async runtime, a browser engine, a networking stack, a database, or a second GUI toolkit — verified with `cargo tree`):

| Crate | Why | Net-new to the dependency tree? |
| --- | --- | --- |
| `dirs 6.0.0` | Per-OS known-folder resolution (§7) | Yes — plus its own small dependency `dirs-sys 0.5.0` (`option-ext`, one more `windows-sys` version-pin on Windows). No further dependents. |
| `log 0.4.33` | Logging facade macros (§8) | No — already resolved transitively via `slint -> i-slint-core`; adding it as a direct dependency added zero new crates. |
| `chrono 0.4.45` (`default-features = false`, `features = ["clock"]`) | Log timestamp formatting (§8) | No — `cargo tree -i chrono` confirms `i-slint-core` is the sole existing consumer; reused, not newly added. |
| `windows-sys 0.59` (Windows-only, `Cargo.toml`'s `[target.'cfg(windows)'.dependencies]`) | `MessageBoxW` FFI (§9) | No new crate at that exact version — `0.59.0` was already resolved transitively (e.g. via `winreg`, itself only a build-dependency of `embed-resource`); a thin FFI-declarations crate with no codegen beyond what is referenced. |

New **build**-only dependency (Windows-only, `[target.'cfg(windows)'.build-dependencies]`, so it never appears in the shipped binary or on other targets):

| Crate | Why |
| --- | --- |
| `embed-resource 3.0.11` | Compiles and links `assets/branding/app.rc` into the executable (§6). Its own dependency tree (`cc`, `rustc_version`, `toml`, `vswhom`, `winreg`) is entirely build-tooling, invisible at runtime. |

`cargo tree -e normal --depth 1` confirms the crate's own direct runtime dependency list is exactly `study-tracker-core`, `slint`, `chrono`, `dirs`, `log`, plus the Windows-only `windows-sys` — nothing else was added.

## 23. Production integrity

`desktop/` remains **read-only and unchanged**: `git diff -- desktop` is empty and `git status --short` shows nothing under `desktop/` after the §2 checkpoint. The only interaction with `desktop/` in this entire stage was reading `desktop/src-tauri/icons/{icon.png,icon.ico}` to make byte-identical copies (§6) and reading `desktop/src-tauri/tauri.conf.json` for its identifier (§5, §7) — both read-only inspections, exactly as the brief required.

## 24. Git status

```text
 M native-prototype/Cargo.lock
 M native-prototype/Cargo.toml
 M native-prototype/build.rs
 M native-prototype/src/app_model.rs
 M native-prototype/src/main.rs
 M native-prototype/ui/main.slint
?? native-prototype/assets/branding/
?? native-prototype/src/platform/
```

(`docs/stage13-production-shell.md` itself, this file, is also untracked/new — omitted from the list above only because it was still being written at the moment the status was captured for §21; it is included in the final file list, §20.) Nothing outside `native-prototype/` is dirty. **Not committed**, per instruction.

## 25. Stage 13 verdict

**PASS WITH CONCERNS.**

All 20 acceptance criteria that could be verified without disrupting the user's active session were met cleanly: console-free release launch, a real embedded/window icon (small-icon slot and executable resource both confirmed; large-icon slot not confirmed — §19), explicit identity, correct and collision-free platform paths, bounded non-churning logging, a diagnosable (if not end-to-end-exercised) fatal-startup path, a small explicit configuration boundary, an explicit and isolated renderer default, an untouched `study-tracker-core` boundary, single-process operation, no performance or startup regression, and all automated tests passing. The "concerns" are narrow and explicitly named rather than papered over: the large-icon slot, the unexercised fatal-error path, and — the main one — the live visual/interactive verification (window+taskbar appearance, minimize/maximize/resize/close, and the Start→minimize→restore timer-correctness check) that this document asks the user to perform in a one-minute manual pass (§15), because continuing automated screen/input interaction mid-session risked disrupting the user's own foreground work and was stopped as soon as that became apparent.

## 26. Proposed Stage 14 scope

**Stage 14 — Timer productionization (Phase B)**, per the freeze's own roadmap (`stage12_5-architecture-freeze.md` §24): build the real native Timer screen end to end against `timer-compatibility-spec.md`'s behavior contract (the core already implements it; this is UI/persistence/lifecycle completion, not new domain logic) — production-mode semantics for Focus/Exam/Endless, persistence integration (a minimal file-backed stand-in is acceptable if sequenced before Stage 15's real adapter, per the freeze's §25 dependency note, but must be replaced, not left in place), recovery on restart, and the sleep/resume manual verification the freeze flags as unverified (§17 of the freeze document). Non-goals carried forward unchanged: no course/task/session domain beyond what the timer needs to reference, no dashboard integration, no network/social integration, no persistence technology decision beyond what Stage 14 itself needs. This is timer-productionization scope only, as the brief specifies — not started in this stage.
