# Stage 8 Cross-Platform Viability

## Executive Summary

Stage 8 is a bounded viability spike for the native architecture:

```text
Slint UI -> native adapter -> study-tracker-core
```

The Stage 4 Slint timer screen now drives the renderer-independent `study-tracker-core` timer state instead of a demo-local timer implementation. This validates that UI callbacks can flow through a thin native adapter into domain logic while keeping the core independent from Slint, Winit, FemtoVG, GUI APIs, platform APIs, storage, networking, and runtime services.

Linux is the only runtime-verified platform in this environment. Windows and macOS are documentation-verified only unless those platforms are actually built and launched on their native OSes.

## Platform Verification Matrix

| Platform | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Linux x86_64 | VERIFIED | Built, tested, release-built, and smoke-launched on Linux Wayland/X11-capable host | Current host: Arch Linux x86_64, Wayland session with `DISPLAY=:0` and `WAYLAND_DISPLAY=wayland-1` |
| Windows x86_64 | DOCUMENTATION-VERIFIED | Rust and Slint list Windows as a supported desktop target; Winit is cross-platform | Not built or launched here; Linux cross-compilation would not prove runtime behavior, installer behavior, fonts, accessibility, tray, notifications, or updater integration |
| macOS aarch64/x86_64 | DOCUMENTATION-VERIFIED | Rust and Slint list macOS as a supported desktop target; Winit is cross-platform | Not built or launched here; Linux cannot provide a valid macOS runtime, signing/notarization, accessibility, fonts, menu, notification, or updater result |

## Linux Verification

Linux verification covers the current prototype only, not the full production app.

Verified scope:

- `cargo fmt --check`
- `cargo check`
- `cargo test --workspace`
- `cargo test -p study-tracker-core`
- `cargo build --release`
- `./scripts/check.sh`
- Release binary launch smoke test on the current Linux desktop session

Runtime smoke result:

- The release binary launched on the current Linux desktop session and stayed open until the timeout terminated it.

Automated model/adapter behavior validated by unit tests:

- The timer starts through the Slint callback path.
- Running display is derived from `study-tracker-core` through `ClockObservation`.
- Pause freezes the remaining display.
- Resume continues from the paused remaining time.
- Reset restores the selected mode duration.
- Mode switching remains blocked while running and allowed while idle.

## Windows Assessment

Windows remains viable but unverified in this environment.

Positive signals:

- Rust supports `x86_64-pc-windows-msvc`.
- Slint supports native desktop Windows targets.
- The prototype keeps domain logic in `study-tracker-core`, which does not use OS APIs.
- Platform-facing dependencies are isolated to the adapter/UI package.

Unknowns requiring real Windows validation:

- MSVC toolchain and native build setup.
- GPU/OpenGL behavior for the FemtoVG renderer.
- Font fallback and text metrics.
- Accessibility backend behavior.
- DPI scaling and window behavior.
- Notifications, tray, updater, filesystem, and autostart once those adapters exist.
- Packaging/signing strategy.

## macOS Assessment

macOS remains viable but unverified in this environment.

Positive signals:

- Rust supports `aarch64-apple-darwin` and `x86_64-apple-darwin` targets.
- Slint supports native desktop macOS targets.
- The timer core is platform-independent and headless-testable.
- The current architecture leaves macOS-specific behavior outside the core.

Unknowns requiring real macOS validation:

- Native macOS build using Apple SDKs.
- Apple Silicon and Intel runtime behavior.
- GPU/OpenGL behavior for the FemtoVG renderer.
- Font fallback, text metrics, and emoji/CJK rendering.
- Accessibility permissions and behavior.
- Menu bar, tray/menu extra, notifications, updater, sandboxing, signing, and notarization.

## Dependency Audit

`study-tracker-core` dependency tree:

- `serde` with derive support.

The core does not depend on:

- Slint.
- Winit.
- FemtoVG/OpenGL.
- Tauri.
- React or JavaScript tooling.
- WebView, Chromium, or WebKit.
- Async runtimes.
- HTTP/network clients.
- File dialogs, tray, notifications, updater, or OS GUI APIs.

The prototype package depends on Slint with explicit features:

- `compat-1-2`
- `std`
- `accessibility`
- `backend-winit`
- `renderer-femtovg`

That means cross-platform risk is concentrated in the adapter/UI layer and future platform adapters, not in timer domain logic.

## Adapter Integration

Stage 8 rewired `native-prototype/src/app_model.rs` so the Slint-facing model owns an `AppTimer` containing `study_tracker_core::timer::TimerState`.

Command mapping:

- UI start from idle -> `TimerCommand::Start`.
- UI start from paused -> `TimerCommand::Resume`.
- UI pause -> `TimerCommand::Pause`.
- UI reset -> `TimerCommand::Reset`.
- UI refresh tick -> `TimerCommand::ObserveTime`.

The adapter creates `ClockObservation` values from an `Instant` origin and a wall-clock origin. Live countdown display is monotonic during one process run, while the core remains ready for wall-clock persistence snapshots later.

The Slint file remains presentation-only. `src/main.rs` stays as the adapter that maps model snapshots into Slint properties and maps UI callbacks into commands.

## Core Boundary Check

The Stage 8 adapter keeps the required boundary intact:

- Core owns deterministic timer transitions and events.
- Adapter owns clock observation construction and UI command translation.
- UI owns presentation.
- Persistence, networking, notifications, tray, updater, Cloudflare sync, and filesystem integrations remain outside the core.

This is the right dependency direction for a future native migration. The screen can evolve without making the timer core renderer-specific, and the timer core can be tested without launching a GUI.

## Risks And Follow-Ups

Risks:

- Linux verification does not prove Windows/macOS runtime behavior.
- Slint/FemtoVG rendering may vary by GPU driver and platform.
- Text rendering, fallback fonts, IME behavior, accessibility, and DPI need native OS checks.
- Future tray, notifications, updater, filesystem, and packaging work may introduce platform-specific complexity.
- Current smoke testing is manual; there is no automated GUI integration test.

Recommended next steps before any broad migration:

1. Run native Windows build and smoke test on Windows.
2. Run native macOS Apple Silicon build and smoke test on macOS.
3. Run native macOS Intel build if Intel support remains a product requirement.
4. Add a small adapter-level test seam for timer commands without Slint launch.
5. Keep extracting domains into `study-tracker-core` before porting more screens.

## Decision

Stage 8 result: CONDITIONAL GO for continuing native feasibility work.

Reasoning:

- The core architecture works for the timer path.
- The domain boundary stayed clean.
- Linux verification passed for the prototype.
- Windows and macOS remain plausible but not runtime-verified.

Do not treat this as approval to migrate the full production app yet. The next gate should require at least one real Windows and one real macOS runtime smoke test.
