# Native Prototype Architecture

## Isolation

This prototype lives under `native-prototype/` so it can be built, tested, benchmarked, and eventually discarded without changing the existing Tauri, React, TypeScript, Cloudflare, updater, persistence, or timer code. The production-style application in this repository is reference material only for Stage 1.

The prototype is a separate Cargo package rather than a member of an existing workspace. That keeps dependency resolution and build behavior independent from the current desktop application.

## Why Slint

Slint was selected for this experiment because it provides native desktop windowing and a Rust integration path without React, HTML, CSS, a DOM, a WebView, Chromium, WebKit, or a JavaScript runtime. It gives enough declarative UI structure to prototype a native renderer while still allowing the application model to remain in ordinary Rust.

Stage 1 does not conclude that Slint is the final renderer. It establishes a small baseline that can later be compared with iced or a custom renderer.

## UI And Model Boundary

`src/app_model.rs` owns the current prototype state using only plain Rust types. It does not import or mention Slint.

`src/main.rs` is the thin presentation adapter. It creates the Rust model, applies simple commands, converts plain Rust strings and sample data into Slint properties, wires UI edit callbacks back into model commands, and starts the Slint event loop.

`ui/main.slint` is presentation only. It defines the native window, text samples, wrapping region, and native editable fields needed to validate text/layout/input behavior. It does not own domain behavior.

## Backend And Renderer

The prototype uses Slint `1.17.1` with default features disabled and these explicit features enabled:

- `compat-1-2`
- `std`
- `accessibility`
- `backend-winit`
- `renderer-femtovg`

This selects Winit for native window/event-loop integration and FemtoVG for rendering. Qt is not enabled. Tauri, browser engines, and JavaScript are not dependencies.

Slint normally uses event-driven rendering. Stage 1 and Stage 2 do not create timers, animations, background tasks, or an intentional continuous redraw loop. Future additions that use `Timer`, animation properties, live preview, or recurring state updates could cause periodic redraws and should be measured explicitly.

## Stage 2 Text Stack

Stage 2 exercises Slint's built-in `Text`, `LineEdit`, and `TextEdit` elements. `LineEdit` and `TextEdit` are standard Slint widgets backed by the lower-level `TextInput` element, which exposes selection, cursor positioning, copy/cut/paste, focus, and IME preedit support.

The selected renderer remains FemtoVG through the Winit backend. Slint's documentation describes FemtoVG as GPU accelerated with OpenGL and notes that text and path rendering quality can be sub-optimal, so Stage 2 checks text visually instead of assuming browser-equivalent rendering.

No font files are bundled. The current Linux environment is expected to use system font discovery and fallback. On the test system, Fontconfig reports Liberation Sans as the generic sans-serif match, Noto Sans CJK JP for Japanese/CJK coverage, and Noto Color Emoji for emoji coverage. This is environment-specific and not a cross-platform guarantee.

Custom fonts can be investigated later if system fallback is insufficient, but Stage 2 deliberately avoids bundling fonts so the first feasibility signal reflects normal desktop font discovery.

## Expected Cross-Platform Targets

The intended future target platforms are:

- Linux
- Windows
- macOS

Only platforms actually built or launched in a given environment should be considered verified. Stage 1 in this repository does not claim cross-platform verification beyond the checks reported by the person running them.

## Stage 1 Limitations

- No timer screen.
- No migrated production features.
- No persistence.
- No updater integration.
- No Cloudflare integration.
- No complex widgets.
- No benchmark conclusion about total memory savings yet.
- No verified accessibility behavior beyond compiling with Slint accessibility support.

## Likely Stage 2 Investigation

Stage 2 would likely compare real idle memory and CPU behavior against the current Tauri/WebView application, evaluate startup time and binary size tradeoffs, test representative text/layout complexity, and decide whether Slint should be compared against iced or a custom renderer before any production feature migration begins.
