# ADR 0001 — Native Windows-first architecture for Study Tracker

## Status

**Accepted** (Stage 12.5, following Stages 0–12 feasibility work and a same-machine production comparison).

## Context

Study Tracker ships today as `desktop/`: Tauri 2 + WebView2 + React 19, with a Cloudflare Worker social backend. Stages 0–12 built and measured a parallel native prototype (`native-prototype/`) on Linux and, for Stage 12 onward, on real Windows hardware, to answer one question: is a Rust + Slint native architecture good enough, on Windows specifically, to justify migrating the real product?

Stage 12 measured the native prototype on a real Windows 11 PC (build/tests, renderer identity, memory, CPU, startup, map stress, text/emoji, editing/clipboard, HiDPI, wheel, accessibility via UI Automation, shell behaviour) and found one real defect (a 100 ms UI-model replacement that caused 20 fps of unnecessary repaint) which was fixed and verified. A same-machine follow-up built the actual production `desktop/` app in release mode (isolated WebView2 profile, real user data never touched) and measured its whole process tree against the native prototype using identical methodology. See `native-prototype/docs/stage12-windows-platform.md` for full data; this ADR does not repeat the tables.

## Decision

Freeze the following for the real native Study Tracker:

1. **Language**: Rust for the shared core, application layer, and native host/platform adapters. No C/C++ rewrite is considered; none was investigated because nothing in the evidence suggested a blocker that Rust cannot address.
2. **UI**: Slint, on Winit, is the native UI framework, Windows-first, Linux and macOS secondary/tertiary.
3. **Renderer**: FemtoVG over OpenGL (current default) remains the default Windows renderer. This is a default, not a permanent lock — see the renderer policy in `stage12_5-architecture-freeze.md` for exact revisit conditions.
4. **Core boundary**: `study-tracker-core` stays independent of Slint, Winit, FemtoVG, Skia, Win32, Tauri, React, WebView, filesystem/network implementation details, and notification/tray/updater APIs. Dependency direction is one-way: `Slint UI -> application adapter -> study-tracker-core -> effects out to persistence/network/platform adapters`.
5. **Platform strategy**: Windows-first, Linux secondary, macOS third. Shared logic stays shared unless measured evidence, accessibility, or reliability requires a platform-specific adapter.
6. **Migration approach**: incremental, production-as-reference, no big-bang rewrite. `desktop/` stays the released application and the behavioural reference until parity is proven and cutover criteria (defined in the freeze document) are met.

## Consequences

- Teams can now build real domain/persistence/platform code against a frozen boundary instead of re-litigating it per stage.
- The native prototype directory keeps its `native-prototype/` name until Stage 12.5's promotion criteria are met (see the freeze document); no disruptive rename happens now.
- `desktop/` remains untouched and released; no user-facing change happens because of this ADR.
- Renderer, IME, emoji, and accessibility gaps identified in Stage 12 remain open risks tracked as explicit gates, not blockers to starting migration.
- Future agents must not reopen the language, UI framework, or core-boundary decisions without new evidence of a concrete blocker (see "Revisit conditions").

## Alternatives considered

- **C/C++ core**: not investigated; out of scope per explicit user direction across Stages 0–12 and this stage. No evidence suggests Rust is a limiting factor.
- **Skia (Direct3D 12) as the default Windows renderer**: measured in Stage 12. Faster on dense synthetic vector-stress data (Dense ×50), but higher idle memory (about 94 MB vs 43 MB Private WS), slower startup (about +60–100 ms), and much worse on a single huge vector path (about 5 fps vs 76 fps). Not adopted as default; kept as a documented, swappable alternative.
- **Skia software surface** (what plain `renderer-skia` silently selects on Windows): very low memory on this fast desktop CPU, but slow on the single-huge-path case and its memory advantage is expected to not generalize to a weak CPU. Not adopted.
- **Keep production as-is (no native migration)**: rejected. The same-machine comparison shows production's WebView2 architecture costs roughly 2.8–4× the private memory and roughly 4× the startup time of the native prototype for equivalent idle/timer behaviour, and its shipped implementation currently burns about 13–19% of one CPU core while the timer is running, paused, or even minimized, from continuous CSS animation that is not gated by visibility. That specific CPU cost is an application-level choice, not an inherent Tauri requirement, but the memory/process-count/startup gap is architectural (WebView2's multi-process model). Both are real reasons to prefer the native architecture for a "leave it open for hours" product.
- **Rewrite from scratch as a single big-bang release**: rejected in favor of incremental migration with `desktop/` as the reference, to avoid a multi-month release with no working intermediate state and to avoid data-loss risk.

## Evidence

- `native-prototype/docs/stage12-windows-platform.md` — Windows renderer identification, memory/CPU/startup methodology and results, map stress and retention experiment, text/emoji/editing/IME/HiDPI/wheel/accessibility findings, renderer comparison, and the production Tauri/WebView2 same-machine comparison (process-tree breakdown, timer CPU attribution to specific CSS animations, long-run stability for both apps, startup comparison, same-machine A/B table).
- `native-prototype/docs/stage11-maps-rendering.md`, `stage10-dashboard-charts.md`, `stage9-text-ime-accessibility.md` — realistic map, dashboard/chart, and text/IME/accessibility feasibility spikes preceding Stage 12.
- `native-prototype/docs/native-core-architecture.md`, `timer-compatibility-spec.md` — the existing renderer-independent core boundary and the timer domain model this ADR keeps.
- `native-prototype/docs/stage12_5-architecture-freeze.md` — the full policy document this ADR summarizes (persistence, network, platform-service boundaries, performance principles, migration roadmap, testing strategy, cutover criteria).

## Revisit conditions

Do not reopen without new, specific evidence:

- **Language (Rust)**: revisit only if a concrete, measured blocker is found that no available Rust crate/binding can address.
- **UI framework (Slint)**: revisit only if a real production feature cannot meet a measured responsiveness or accessibility target that no Slint configuration or adapter can fix.
- **Default renderer (FemtoVG)**: revisit only if one of — a real (not synthetic) production workload misses a responsiveness target on the default renderer; a text/emoji rendering blocker is traced to the renderer; a GPU-driver compatibility/reliability problem appears; a platform (not a single benchmark number) requires a different renderer. A synthetic Dense ×50 stress result alone is not sufficient.
- **Core boundary**: revisit only if a specific platform/UI capability cannot be expressed through an adapter without leaking a UI/platform type into the core.
- **Migration strategy (incremental)**: revisit only with explicit user direction.
