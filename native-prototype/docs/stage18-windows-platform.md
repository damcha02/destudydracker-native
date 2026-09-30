# Stage 18 — Windows platform integration

Status: implemented, **uncommitted**. Verdict: **PASS WITH CONCERNS** (see §29).

Reference: production Study Tracker **v0.1.67** (`desktop/`, read-only; synced in `e23871f`, see
`production-sync-0.1.67.md` for the audit, the release pipeline and the Windows/macOS update chains).

## 1. Pre-Stage-18 checkpoint and sync

* Stage 17 is committed as `713bdf9` ("native: complete stage 17 production dashboard migration and
  visual parity" — the message differs from the one requested; history was not rewritten).
* Production sync is `e23871f` ("chore: sync latest production reference"). It landed *after* Stage 17
  (the reverse of the preferred order) because the sync request arrived mid-turn. `desktop/` is unchanged
  since `e23871f`.

## 2. Stage 18 preflight (native compatibility with v0.1.67)

The audit classified two deltas as NATIVE FOLLOW-UP REQUIRED; both are in the Stage 16 academic model and
were fixed first, with tests, before any platform work:

| Delta | Change | Tests |
| --- | --- | --- |
| `Exam.kind` (`midterm/endterm/semester_end/project/session`) | `ExamKind` + `Exam.kind: Option<ExamKind>` (serde-defaulted, omitted when unset); import reads `kind` | `v0_1_67_exam_kinds_and_prep_tasks_survive_import` |
| `Task.prep` / `Task.prepOf` | `Task.prep: bool`, `Task.prep_of: Option<TaskId>` (serde-defaulted) ; import reads both | same test + `stores_written_before_the_new_fields_still_deserialize_and_omit_them_when_unset` |

Old native stores (written before these fields) still load, and unset fields are not written.

## 3. Production platform inventory (what the native app must match)

Tray (icon per phase, tooltip/title with remaining time, left click = show, menu Show/Quit), close-to-tray
while a study session runs (once-per-run notice), single instance (activate the first window), native
notifications on timer completion (focus finished / break finished), updater (Tauri, minisign, startup + 24 h
check, "New update available." notice, user-initiated install), AUMID/app identity, Explorer restart recovery.
Not migrated here by design: Social, autostart/deep links (later stages), installer (Stage 23).

## 4. Platform architecture

```
core (study-tracker-core, no platform)          pure: timer, academic, effects
  └─ app_model.rs      typed outputs only: NotificationRequest outbox, TrayTimerState
platform/ (pure, testable on any OS)            tray_model, notification (ledger, sink trait), updater/*
platform/ (windows only)                        win_host (tray+message window), single_instance,
                                                win_toast, win_util
app_platform.rs (windows)   wiring: close policy, hide/show, command dispatch, tray sync, notifications
app_updater.rs  (windows)   wiring: schedule, feed policy, notice UI
```

Rule kept: core never depends on platform; platform effects are typed values
(`NotificationRequest`, `PlatformCommand`, `TrayTimerState`). No polling loops were added: the existing
100 ms UI tick also drives tray sync (writes only on change); the updater uses sleeping `slint::Timer`s.

## 5. Tray

Message-only window + `Shell_NotifyIconW` (version 4). Icons are a pixel-port of production's generated
icons (per phase). Tooltip/title come from `tray_model` (pure, tested). `TaskbarCreated` re-adds the icon
after an Explorer restart. `Drop` removes the icon. **Verified on real Windows**: icon present (via
`Shell_NotifyIconGetRect`, `scripts/win-tray-probe.ps1`), removed on exit, left click restores, menu
Quit (driven through the real popup menu with keyboard input) terminates the process and removes the icon.

## 6. Window lifecycle

`CloseAction` (pure): running Study/Focus session → hide to tray (+ once-per-run notice); otherwise Quit.
Hidden windows skip property pushes (Stage 14 rule, extended from "minimized" to "hidden"). The event loop is
`run_event_loop_until_quit` so the tray keeps the process alive. 200 hide/show cycles: 0 failures.

## 7. Single instance

Named mutex (per app id **and** per data directory hash, so a separate profile/test run does not collide)
acquired before logging/persistence is opened. The second process posts `WM_APP+1` to the first's message
window (bounded 8 s retry, `AllowSetForegroundWindow`) and exits. **Only the primary ever opens the store**,
so concurrent persistence writers are impossible by construction. Secondary arguments/command line are not
interpreted at all (no untrusted input crosses the boundary — the message carries no payload).

## 8. Concurrent launch test

`scripts/stage18-lifecycle.ps1`: 8 near-simultaneous launches against an empty profile → **exactly 1
survivor**, one tray window. Unit test: 16 threads race `acquire` → exactly one `Primary`.

## 9. Notifications

`NotificationRequest` produced by the model (`notifications_for_effects`: only a real
`CompletionReason::CountdownElapsed`; reset/pause/recovery are silent), stamped with the completion's wall
time. `NotificationLedger::admit(kind, completion_millis)` makes delivery exactly-once per completion.
`deliver()` swallows sink errors, so a notification failure can never affect the session that was persisted.

## 10. Notification runtime verification

Real Windows, release build, Demo (10 s) timer: a real notification appeared once at completion (observed).
WinRT toasts need a registered AUMID; an unpackaged dev exe gets `0x80070490`, so `ToastSink` latches to a
tray balloon (which Windows 11 renders as a toast). The AUMID is registered under HKCU; a Start-menu shortcut
carrying it (proper toast identity) is Stage 23.

## 11. Exactly-once

Tests: one notification on live completion even with repeated Refresh; break-follows flow →
`FocusFinishedBreakNext` then `BreakFinished`, one session each; reset/pause silent; recovery after restart
silent; ledger rejects duplicates.

## 12. Sleep / background

Timer correctness is timestamp-based (unchanged), so sleep/resume and hidden windows cannot drift. Hidden or
minimized → no property pushes, 0 frames. **Machine sleep was not triggered** (per instructions, it needs a
manual test); the existing Stage 14 wall-clock recovery tests cover the logic. Manual check listed in §29.

## 13. Updater

Policy (mirrors production's channel rule): active only in an official release build *with* a compiled-in
feed URL (`STUDY_TRACKER_OFFICIAL_RELEASE=1`, `STUDY_TRACKER_UPDATE_FEED_URL`). The native prototype has
neither → **disabled**; it never contacts production's live feed (that would offer Tauri installers to a
native binary). Cadence: first check 15 s after start, then every 24 h, on a worker thread; the result
returns through `invoke_from_event_loop`. Notice: "New update available." with Download / dismiss.

## 14. Native updater architecture

`platform/updater/`: `version` (strict semver, path-safe `Display`), `manifest` (`latest.json`, Tauri shape),
`verify` (minisign, production public key), `staging` (atomic, version-derived names), `service`
(`check` / `download` / `install_staged`), `http` (WinHTTP client, `FileFetcher` for tests). All logic is
behind a `Fetcher` trait, so it is tested without a network.

## 15. Update security

* Authenticity = minisign/Ed25519 over the **artifact bytes**, verified in memory *before* anything touches
  disk; the manifest itself is untrusted. Trust root = production's public key, compiled in.
* Key id mismatch / wrong key / corrupt artifact / missing signature → rejected, nothing staged.
* **Downgrade/replay**: a feed version ≤ running version is "up to date"; the verified version is what is staged.
* HTTPS only, no credentials in URLs, size caps (manifest 1 MiB, artifact 512 MiB), bounded WinHTTP timeouts.
* Staged name comes only from the validated version (no server text in paths); `<data>/updates/<ver>/` only,
  canonical-path containment check, atomic `.part` → rename, stale versions removed.
* Nothing is ever executed by Stage 18 (`install_staged` returns `InstallNotSupported`).
* Debug-only `STUDY_NATIVE_UPDATE_TEST_PUBKEY`; the feed-file hook cannot weaken trust (key stays compiled in).
* Logs contain versions only, not URLs/signatures.
* Verified end to end: debug build + throw-away test key → feed file → available → downloaded → verified →
  staged; **release build with the same feed → "signature created with a different key", nothing staged**.

## 16. Update installation status

**Not implemented — deliberately.** Download + verify + stage works; install/relaunch needs the installer and
its silent/passive switches, elevation policy, and relaunch handshake (Stage 23). The notice says so
("Installing from inside the app arrives with the installer"). In-app update capability is preserved by
design (same key, same manifest format, same cadence), not yet end-to-end.

## 17. App identity

Single-instance and window-class names derive from `identity::APP_ID` (`com.damcha.studytracker.native-shell`)
plus the data-dir hash; AUMID under HKCU with icon written to `<data>/branding/icon.png`;
`SetCurrentProcessExplicitAppUserModelID` set. Distinct from production's identifiers.

## 18. Coexistence with installed production

Separate mutex/window class/AUMID/tray id, separate data directory (`STUDY_NATIVE_DATA_DIR`/native dir), no
registry keys shared with production. Production's real profile was not opened or inspected.

## 19. Shutdown

Quit (tray menu / idle close) → `quit_event_loop` → `shutdown()` removes the tray icon and destroys the
message window; mutex released on process exit. Verified: process exits, tray window gone, no zombies.
Hard kill leaves a stale icon until the mouse passes over the tray (Windows behaviour, unavoidable).

## 20. Performance (release, running study timer, 30 s window after 20 s settle)

| ID | Scenario | CPU % | Private MB | Frames / 10 s (after settle) |
| --- | --- | --- | --- | --- |
| W18-P0 | visible idle with tray | 0.10 | 120.8 | 0 |
| W18-P1 | minimized | 0.16 | 120.8 | 0 |
| W18-P2 | hidden to tray | 0.05 | 121.5 | 0 |
| W18-P6 | 200 hide/show cycles | — | 120.1 → 124.9 | — |
| W18-P7 | 50 second launches | median 1005 ms, max 1021 ms | 124.8 (no growth) | — |

Ticks stay ≈100 per 10 s (the existing 100 ms tick). The visible scenario shows 0 frames because the
Dashboard page is static between data changes (Stage 17 behaviour). Not separately measured: a visible Timer
page with a running countdown (unchanged Stage 14 path) and the updater check (disabled by default; one
worker thread for the duration of one request when enabled).
Scripts: `scripts/stage18-perf.ps1`, `scripts/stage18-lifecycle.ps1`.

## 21. Resource stability

200 hide/show cycles: +4.8 MB private, handles 379 → 401, threads 14 → 15; the following 50 second-instance
launches added 0 MB and handles/threads went *down* (395 / 10) → plateau, no leak trend.

## 22. Timer / Dashboard regression

Timer tests unchanged and green (187 bin + 90 core). Dashboard renders Quiet/Full unchanged; the update
notice is an `if` overlay that does not exist unless visible. No new recompute paths in the Dashboard.

## 23. Security review (summary)

| Topic | Result |
| --- | --- |
| Updater authenticity | minisign over artifact, pinned key, in-memory verify before staging |
| Downgrade / replay | refused (semver precedence) |
| Path traversal / device names | impossible: name built from validated version only; containment check |
| Temp-dir races / symlinks | staging under own data dir; `.part` + atomic rename; canonical-path containment |
| Arbitrary execution | none in Stage 18 |
| Malformed manifest | strict parse, size cap, tests incl. non-object platforms |
| IPC / second-instance input | payload-less `WM_APP+1`; arguments ignored |
| Log leakage | versions only |
| Keys | only the public key is in the repo; throw-away private test keys were deleted |
| Residual | Windows notification area icon spoofing by other same-user processes is out of scope |

## 24. Dependencies

* **New direct crate: `minisign-verify` 0.2.5** (small, verification-only, used by Tauri's own updater).
* `windows` 0.62 was already in the tree (via accesskit); only features were added (`UI_Notifications`,
  `Data_Xml_Dom`, `Foundation`). `windows-sys` features added: Shell, Threading, LibraryLoader, Gdi,
  Registry, Security, WinHttp.
* Release exe: **19,868,672 B (HEAD `e23871f`) → 20,233,216 B**, +364,544 B (+1.8 %).

## 25. Tests / checks

`cargo fmt --check` clean, `cargo test --workspace` (90 core + 187 bin, 0 failed), platform/updater suite
incl. real Tauri-signed fixtures, 16-thread mutex race, WinHTTP loopback with size limits, notification
exactly-once, tray model. Final results are in the final report.

## 26. Production integrity / user data

`desktop/` has no diff versus `e23871f`. No real production or user data was read, imported or modified; all
runs used throw-away `STUDY_NATIVE_DATA_DIR` profiles. No secrets/private keys in the repo; no release,
feed, or GitHub state touched; nothing pushed.

## 27. Deferred Stage 17 parity items (explicitly not absorbed)

Carried to Stage 23 as previously documented in the Stage 17 doc; Stage 18 changed none of them.

## 28. Stage 23 requirements this stage depends on

Installer + Start-menu shortcut with AUMID (real toasts), silent/passive install + relaunch handshake, NSIS
artifact naming to match `platform_keys`, release workflow publishing a native `latest.json`, macOS chain
(`darwin-*`, notarization, `.app.tar.gz`) — documented in `production-sync-0.1.67.md`.

## 29. Verdict and open concerns

**PASS WITH CONCERNS** — the only blocked item is final install/relaunch (Stage 23 packaging). Updater
security is intact and the single-instance design cannot produce concurrent writers.

Concerns / not done:
1. Install + relaunch not implemented (by design, above).
2. Machine sleep/resume not exercised manually — please run once: start a Focus timer, sleep the PC for ≥1 min,
   wake, confirm the remaining time matches wall-clock.
3. Real WinRT toast requires the Stage 23 shortcut; balloon fallback is used meanwhile.
4. Startup-time impact of platform init was not separately benchmarked (host creation happens once, ~ms).
5. Notice UI is minimal (no animation/theme polish) and not pixel-checked against production's notice.
